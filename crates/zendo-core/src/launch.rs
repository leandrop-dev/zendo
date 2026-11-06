//! Starting a game, telling whether it still runs, and stopping it.
//!
//! A game runs in a process group of its own, started from the home folder
//! (a relative path in a launcher's settings then means the same wherever
//! Zendo was started). It counts as running while any process of the group
//! is alive: legendary, for one, starts the game and exits at once, and
//! the game's processes stay in the group. Stop signals the whole group.
//!
//! The games Zendo installed start through their engine's own `launch`
//! (which knows the store's arguments, such as Epic's sign-in token), with
//! umu-run as the wrapper for Windows games: Proton, a prefix of the game's
//! own in `<install_dir>/Prefixes/<title>`, and umu's fixes for the game.
//! Each launch's output goes to `$XDG_STATE_HOME/zendo/logs/<id>.log`.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use anyhow::{Context, bail};

use crate::config::Config;
use crate::stores::Store;
use crate::{Game, Launch, Source, State, downloads, paths, proton, umu};

/// A game just started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// The process group the game runs in.
    pub group: u32,
    /// Where its output goes, for the games Zendo starts itself.
    pub log: Option<PathBuf>,
    /// Whether Zendo can tell when it ends: not for a game handed to Steam
    /// or Heroic.
    pub tracked: bool,
}

/// Starts the game without waiting for it, and notes the launch in Zendo's
/// state. Network lookups happen on the way (umu's id for the game), so
/// the window calls it on a thread.
///
/// # Errors
///
/// When the program, or `xdg-open` for a link, cannot be started. Failing
/// to note the launch is not an error: the game is already running.
pub fn start(game: &Game) -> anyhow::Result<Started> {
    let (mut command, log, tracked) = match &game.launch {
        Launch::Uri { uri } => {
            let mut command = Command::new("xdg-open");
            command.arg(uri);
            (command, None, false)
        }
        Launch::Command { program, args, dir } => {
            let mut command = Command::new(program);
            command.args(args);
            if let Some(dir) = dir.as_ref().filter(|dir| dir.is_dir()) {
                command.current_dir(dir);
            }
            (command, None, true)
        }
        Launch::Engine { app, .. } => {
            let command = engine_command(game, app)?;
            if let Some(prefix) = command
                .get_envs()
                .find(|(key, _)| *key == "WINEPREFIX")
                .and_then(|(_, value)| value)
            {
                fs::create_dir_all(prefix)
                    .with_context(|| format!("could not make {}", Path::new(prefix).display()))?;
            }
            let log = log_path(&game.id);
            (command, log, true)
        }
    };
    if let Launch::Uri { .. } | Launch::Engine { .. } = game.launch
        && let Some(home) = paths::home()
    {
        command.current_dir(home);
    }
    let output = match &log {
        Some(log) => open_log(log, &command)?,
        None => Output::Null,
    };
    let (stdout, stderr) = output.stdio()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .with_context(|| format!("could not start {}", game.title))?;
    let group = child.id();
    let _ = State::update(|state| state.launched(&game.id));
    // Reap it when it ends, so it does not linger as a zombie; the rest of
    // the group carries on.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(Started {
        group,
        log,
        tracked,
    })
}

/// Starts the game and leaves it running, for `zendo launch`.
///
/// # Errors
///
/// As [`start`].
pub fn launch(game: &Game) -> anyhow::Result<()> {
    start(game).map(|_| ())
}

/// Whether any process of the group is alive (and not a zombie).
#[must_use]
pub fn running(group: u32) -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let stat = fs::read_to_string(entry.path().join("stat")).unwrap_or_default();
        // `pid (name) state ppid pgrp …`; the name may hold spaces.
        let Some((_, rest)) = stat.rsplit_once(") ") else {
            return false;
        };
        let mut fields = rest.split_whitespace();
        let state = fields.next();
        let pgrp = fields.nth(1).and_then(|pgrp| pgrp.parse::<u32>().ok());
        pgrp == Some(group) && state != Some("Z")
    })
}

/// Asks every process of the game's group to end, as closing it would.
pub fn stop(group: u32) {
    signal(group, "-TERM");
}

/// Ends every process of the game's group at once, for a game that is
/// frozen.
pub fn kill(group: u32) {
    signal(group, "-KILL");
}

fn signal(group: u32, signal: &str) {
    // 0 is the caller's own group and 1 init's: never a game's.
    if group <= 1 || i64::from(group) == i64::from(own_group()) {
        return;
    }
    let _ = Command::new("kill")
        .args([signal, "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Zendo's own process group, from `/proc/self/stat`.
fn own_group() -> i32 {
    fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let (_, rest) = stat.rsplit_once(") ")?;
            rest.split_whitespace().nth(2)?.parse().ok()
        })
        .unwrap_or(-1)
}

/// Where a game's last launch was logged.
#[must_use]
pub fn log_path(id: &str) -> Option<PathBuf> {
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    paths::state_home().map(|dir| dir.join(format!("zendo/logs/{name}.log")))
}

/// The Wine prefix a Windows game Zendo installed runs in.
#[must_use]
pub fn prefix(game: &Game) -> Option<PathBuf> {
    let dir = Config::load()
        .ok()
        .and_then(|config| config.install_dir())?;
    Some(
        dir.join("Prefixes")
            .join(downloads::folder_name(&game.title)),
    )
}

/// Whether the game needs Proton: a Windows build.
#[must_use]
pub fn needs_proton(game: &Game) -> bool {
    !game
        .details
        .platform
        .as_deref()
        .is_some_and(|platform| platform.eq_ignore_ascii_case("linux"))
}

/// The engine's own launch, with umu-run as the wrapper for a Windows
/// game.
fn engine_command(game: &Game, app: &str) -> anyhow::Result<Command> {
    let store = match game.source {
        Source::Epic => Store::Epic,
        Source::Gog => Store::Gog,
        Source::Amazon => Store::Amazon,
        _ => bail!("{} has no store engine to start it", game.title),
    };
    let mut command = store.command()?;
    let wrapper = if needs_proton(game) {
        let umu = which("umu-run")
            .context("umu-launcher is needed to start Windows games: install it (umu-launcher)")?;
        let prefix = prefix(game).context("no folder for the game's Wine prefix")?;
        let store_key = umu::store_name(store_key(store));
        command
            .env("WINEPREFIX", &prefix)
            .env("GAMEID", umu::game_id(store_key, app))
            .env("STORE", store_key);
        // Without one, umu downloads UMU-Proton on the first launch.
        if let Some(proton) = proton::default() {
            command.env("PROTONPATH", &proton.path);
        }
        Some(quote(&umu.to_string_lossy()))
    } else {
        None
    };
    match store {
        Store::Epic | Store::Amazon => {
            command.args(["launch", app]);
        }
        Store::Gog => {
            let path = game
                .details
                .install_path
                .as_ref()
                .context("no record of where the game is")?;
            let platform = game
                .details
                .platform
                .as_deref()
                .unwrap_or("windows")
                .to_lowercase();
            command
                .arg("launch")
                .arg(path)
                .args([app, "--platform", &platform]);
        }
    }
    if let Some(wrapper) = wrapper {
        command.args(["--no-wine", "--wrapper", &wrapper]);
    }
    Ok(command)
}

fn store_key(store: Store) -> &'static str {
    match store {
        Store::Epic => "epic",
        Store::Gog => "gog",
        Store::Amazon => "amazon",
    }
}

/// A program on `PATH`.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|file| file.is_file())
}

/// A word as a shell would read it back, for the engines' `--wrapper`,
/// which they split as a shell would.
fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+=:,@".contains(c))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The command as one line, the environment Zendo set first.
#[must_use]
pub fn describe(command: &Command) -> String {
    let mut line = String::new();
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            let _ = write!(
                line,
                "{}={} ",
                key.to_string_lossy(),
                quote(&value.to_string_lossy())
            );
        }
    }
    line.push_str(&quote(&command.get_program().to_string_lossy()));
    for arg in command.get_args() {
        line.push(' ');
        line.push_str(&quote(&arg.to_string_lossy()));
    }
    line
}

enum Output {
    Null,
    Log(File),
}

impl Output {
    fn stdio(self) -> anyhow::Result<(Stdio, Stdio)> {
        Ok(match self {
            Self::Null => (Stdio::null(), Stdio::null()),
            Self::Log(file) => (Stdio::from(file.try_clone()?), Stdio::from(file)),
        })
    }
}

/// Starts the game's log afresh, headed with how it was started.
fn open_log(log: &Path, command: &Command) -> anyhow::Result<Output> {
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    let mut file =
        File::create(log).with_context(|| format!("could not write {}", log.display()))?;
    writeln!(
        file,
        "Started by Zendo at {} (Unix time)\n{}\n",
        crate::time::now(),
        describe(command)
    )?;
    Ok(Output::Log(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_what_a_shell_would_split() {
        assert_eq!(quote("/usr/bin/umu-run"), "/usr/bin/umu-run");
        assert_eq!(quote("/home/me/My Games"), "'/home/me/My Games'");
        assert_eq!(quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn names_each_games_log_safely() {
        let log = log_path("amazon:amzn1.adg.product.1/x").unwrap();
        assert!(log.ends_with("zendo/logs/amazon_amzn1.adg.product.1_x.log"));
    }

    #[test]
    fn tells_a_running_group_from_one_that_ended() {
        let mut child = Command::new("sleep")
            .arg("5")
            .process_group(0)
            .spawn()
            .unwrap();
        let group = child.id();
        assert!(running(group));
        kill(group);
        let _ = child.wait();
        assert!(!running(group));
    }
}
