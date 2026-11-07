//! Starting a game, telling whether it still runs, and stopping it.
//!
//! A game Zendo starts itself runs in a process group of its own, started
//! from the home folder (a relative path in a launcher's settings then
//! means the same wherever Zendo was started). It counts as running while
//! any process of the group is alive: legendary, for one, starts the game
//! and exits at once, and the game's processes stay in the group. Stop
//! signals the whole group.
//!
//! A game handed to Steam or Heroic runs under their processes instead, so
//! it is found by its install folder: a process whose program is in it or
//! whose arguments name it (Steam's `reaper` and Proton's script, which
//! last as long as the game, Heroic's `umu-run` or `wine`), with everything
//! those processes started. Stop signals those.
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
use crate::launch_options::LaunchOptions;
use crate::stores::Store;
use crate::{Game, Launch, Source, State, downloads, paths, proton, umu};

/// A game just started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub watch: Watch,
    /// Where its output goes, for the games Zendo starts itself.
    pub log: Option<PathBuf>,
}

/// How Zendo tells a game it started still runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watch {
    /// The process group Zendo started it in.
    Group(u32),
    /// The game's install folder, for a game its launcher runs.
    Folder(PathBuf),
    /// Not at all: a link with no install folder known.
    Nothing,
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
    let (mut command, log, folder) = match &game.launch {
        Launch::Uri { uri } => {
            let mut command = Command::new("xdg-open");
            command.arg(uri);
            let folder = game.details.install_path.clone().filter(|dir| dir.is_dir());
            (command, None, Some(folder))
        }
        Launch::Command { program, args, dir } => {
            let options = options_for(game)?;
            let mut words = options.wrappers();
            words.push(program.clone());
            words.extend(args.iter().cloned());
            words.extend(options.arguments()?);
            let mut command = Command::new(&words[0]);
            command.args(&words[1..]).envs(options.environment()?);
            if let Some(dir) = dir.as_ref().filter(|dir| dir.is_dir()) {
                command.current_dir(dir);
            }
            (command, None, None)
        }
        Launch::Local { program, args, dir } => {
            let options = options_for(game)?;
            let command = local_command(game, program, args, dir.as_deref(), &options)?;
            make_prefix(&command)?;
            (command, log_path(&game.id), None)
        }
        Launch::Engine { app, .. } => {
            let command = engine_command(game, app)?;
            make_prefix(&command)?;
            let log = log_path(&game.id);
            (command, log, None)
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
    let watch = match folder {
        None => Watch::Group(group),
        Some(Some(folder)) => Watch::Folder(folder),
        Some(None) => Watch::Nothing,
    };
    let _ = State::update(|state| state.launched(&game.id));
    // Reap it when it ends, so it does not linger as a zombie; the rest of
    // the group carries on.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(Started { watch, log })
}

/// Starts the game and leaves it running, for `zendo launch`.
///
/// # Errors
///
/// As [`start`].
pub fn launch(game: &Game) -> anyhow::Result<()> {
    start(game).map(|_| ())
}

/// Whether the game still runs: any process of its group, or of those
/// found by its folder, alive (and not a zombie).
#[must_use]
pub fn running(watch: &Watch) -> bool {
    match watch {
        Watch::Group(group) => processes().iter().any(|process| process.group == *group),
        Watch::Folder(folder) => !in_folder(folder).is_empty(),
        Watch::Nothing => false,
    }
}

/// Asks the game to quit: its windows to close, as their X button would,
/// so it can save first; a game with no window that takes that request
/// (no X11 window, or none open) is sent SIGTERM instead, which native
/// games take as a quit but a Windows game under Proton does not live
/// through. Returns whether windows were asked.
#[must_use]
pub fn stop(watch: &Watch) -> bool {
    let pids: std::collections::HashSet<u32> = match watch {
        Watch::Group(group) => processes()
            .into_iter()
            .filter(|process| process.group == *group)
            .map(|process| process.pid)
            .collect(),
        Watch::Folder(folder) => in_folder(folder).into_iter().collect(),
        Watch::Nothing => return false,
    };
    if crate::windows::ask_to_close(&pids) > 0 {
        return true;
    }
    signal(watch, "-TERM");
    false
}

/// Ends every process of the game at once, for a game that is frozen.
pub fn kill(watch: &Watch) {
    signal(watch, "-KILL");
}

fn signal(watch: &Watch, signal: &str) {
    let own = own_group();
    let targets: Vec<String> = match watch {
        // 0 is the caller's own group and 1 init's: never a game's.
        Watch::Group(group) if *group > 1 && i64::from(*group) != i64::from(own) => {
            vec![format!("-{group}")]
        }
        Watch::Folder(folder) => in_folder(folder)
            .into_iter()
            .filter(|&pid| pid > 1 && pid != std::process::id())
            .map(|pid| pid.to_string())
            .collect(),
        _ => Vec::new(),
    };
    if targets.is_empty() {
        return;
    }
    let _ = Command::new("kill")
        .arg(signal)
        .arg("--")
        .args(targets)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// A live process, as `/proc/<pid>/stat` has it.
struct Process {
    pid: u32,
    parent: u32,
    group: u32,
}

/// Every process alive, zombies left out.
fn processes() -> Vec<Process> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse().ok()?;
            let stat = fs::read_to_string(entry.path().join("stat")).ok()?;
            // `pid (name) state ppid pgrp …`; the name may hold spaces.
            let (_, rest) = stat.rsplit_once(") ")?;
            let mut fields = rest.split_whitespace();
            if fields.next()? == "Z" {
                return None;
            }
            let parent = fields.next()?.parse().ok()?;
            let group = fields.next()?.parse().ok()?;
            Some(Process { pid, parent, group })
        })
        .collect()
}

/// The processes of a game its launcher runs: those whose program is in
/// its folder or whose arguments name it, and everything they started.
#[must_use]
pub fn in_folder(folder: &Path) -> Vec<u32> {
    let all = processes();
    let own = std::process::id();
    let mut found: Vec<u32> = all
        .iter()
        .filter(|process| process.pid != own && names_folder(process.pid, folder))
        .map(|process| process.pid)
        .collect();
    // Their children, and theirs: Proton's wine processes, the game itself.
    let mut index = 0;
    while index < found.len() {
        let parent = found[index];
        for process in &all {
            if process.parent == parent && !found.contains(&process.pid) {
                found.push(process.pid);
            }
        }
        index += 1;
    }
    found
}

/// Whether the process's program is in the folder or its arguments name
/// it.
fn names_folder(pid: u32, folder: &Path) -> bool {
    let proc = PathBuf::from(format!("/proc/{pid}"));
    if fs::read_link(proc.join("exe")).is_ok_and(|exe| exe.starts_with(folder)) {
        return true;
    }
    let Ok(cmdline) = fs::read(proc.join("cmdline")) else {
        return false;
    };
    let folder = folder.to_string_lossy();
    let folder = folder.trim_end_matches('/');
    cmdline
        .split(|&byte| byte == 0)
        .any(|arg| mentions(&String::from_utf8_lossy(arg), folder))
}

/// Whether an argument is the folder or a path inside it, alone or after
/// an option's `=`.
fn mentions(arg: &str, folder: &str) -> bool {
    arg.match_indices(folder).any(|(at, _)| {
        let before = arg[..at].chars().next_back();
        let after = arg[at + folder.len()..].chars().next();
        matches!(before, None | Some('=' | '"' | '\'' | ' '))
            && matches!(after, None | Some('/' | '"' | '\''))
    })
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
    logs_dir().map(|dir| dir.join(format!("{name}.log")))
}

/// Where each game's last launch is logged.
#[must_use]
pub fn logs_dir() -> Option<PathBuf> {
    paths::state_home().map(|dir| dir.join("zendo/logs"))
}

/// The Wine prefix a Windows game Zendo starts runs in.
#[must_use]
pub fn prefix(game: &Game) -> Option<PathBuf> {
    let dir = Config::load_file()
        .ok()
        .and_then(|config| config.install_dir())?;
    Some(prefix_in(&dir, game))
}

/// The game's prefix in the games folder `dir`: `Prefixes/<title>`, or for
/// a game added by hand, its id's name, which stays when its title changes.
#[must_use]
pub fn prefix_in(dir: &Path, game: &Game) -> PathBuf {
    let name = match game.id.strip_prefix("local:") {
        Some(name) if game.source == Source::Local => name.to_owned(),
        _ => downloads::folder_name(&game.title),
    };
    dir.join("Prefixes").join(name)
}

/// Makes the prefix the command's `WINEPREFIX` names, when it has one.
fn make_prefix(command: &Command) -> anyhow::Result<()> {
    if let Some(prefix) = command
        .get_envs()
        .find(|(key, _)| *key == "WINEPREFIX")
        .and_then(|(_, value)| value)
    {
        fs::create_dir_all(prefix)
            .with_context(|| format!("could not make {}", Path::new(prefix).display()))?;
    }
    Ok(())
}

/// What umu-run needs for a Windows game: the path to umu-run, and the
/// environment it reads.
struct Umu {
    program: String,
    env: Vec<(&'static str, std::ffi::OsString)>,
}

fn umu_setup(
    game: &Game,
    options: &LaunchOptions,
    store: Option<(&str, &str)>,
) -> anyhow::Result<Umu> {
    let umu = which("umu-run")
        .context("umu-launcher is needed to start Windows games: install it (umu-launcher)")?;
    let prefix = prefix(game).context("no folder for the game's Wine prefix")?;
    let mut env = vec![("WINEPREFIX", prefix.into_os_string())];
    match store {
        Some((store, app)) => {
            env.push(("GAMEID", umu::game_id(store, app, &game.title).into()));
            env.push(("STORE", store.into()));
        }
        None => env.push((
            "GAMEID",
            umu::game_id("local", &game.id, &game.title).into(),
        )),
    }
    // The one chosen, or the default when it is gone; without any, umu
    // downloads UMU-Proton on the first launch.
    let chosen = options.proton.as_deref().and_then(proton::named);
    if let Some(proton) = chosen.or_else(proton::default) {
        env.push(("PROTONPATH", proton.path.into_os_string()));
    }
    if options.xbox_controllers {
        env.push(("PROTON_PREFER_SDL", "1".into()));
    }
    Ok(Umu {
        program: umu.to_string_lossy().into_owned(),
        env,
    })
}

/// A game added by hand: its wrappers, umu-run for a Windows program, the
/// program and its arguments, from its own folder.
fn local_command(
    game: &Game,
    program: &Path,
    args: &[String],
    dir: Option<&Path>,
    options: &LaunchOptions,
) -> anyhow::Result<Command> {
    let mut words = options.wrappers();
    let mut env = Vec::new();
    if needs_proton(game) {
        let umu = umu_setup(game, options, None)?;
        words.push(umu.program);
        env = umu.env;
    }
    words.push(program.to_string_lossy().into_owned());
    words.extend(args.iter().cloned());
    words.extend(options.arguments()?);
    let mut command = Command::new(&words[0]);
    command
        .args(&words[1..])
        .envs(env)
        .envs(options.environment()?);
    if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
        command.current_dir(dir);
    }
    Ok(command)
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

/// The game's launch options, with every tool they turn on installed.
fn options_for(game: &Game) -> anyhow::Result<LaunchOptions> {
    let options = LaunchOptions::load(&game.id)?;
    for (program, name) in options.tools() {
        if which(program).is_none() {
            bail!(
                "{name} is on in {}'s launch options, but {program} is not installed",
                game.title
            );
        }
    }
    Ok(options)
}

/// The engine's own launch, with umu-run as the wrapper for a Windows
/// game, inside the wrappers the game's launch options turn on.
fn engine_command(game: &Game, app: &str) -> anyhow::Result<Command> {
    let store = match game.source {
        Source::Epic => Store::Epic,
        Source::Gog => Store::Gog,
        Source::Amazon => Store::Amazon,
        _ => bail!("{} has no store engine to start it", game.title),
    };
    let options = options_for(game)?;
    engine_command_with(game, app, store, &options, store.command()?)
}

/// [`engine_command`] from the engine's bare command.
fn engine_command_with(
    game: &Game,
    app: &str,
    store: Store,
    options: &LaunchOptions,
    mut command: Command,
) -> anyhow::Result<Command> {
    let mut wrapper = options.wrappers();
    let windows = needs_proton(game);
    if windows {
        let store_key = umu::store_name(store_key(store));
        let umu = umu_setup(game, options, Some((store_key, app)))?;
        command.envs(umu.env);
        wrapper.push(umu.program);
    }
    command.envs(options.environment()?);
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
    if windows {
        command.arg("--no-wine");
    }
    if !wrapper.is_empty() {
        let line: Vec<String> = wrapper.iter().map(|word| quote(word)).collect();
        command.args(["--wrapper", &line.join(" ")]);
    }
    // The engines pass on what they do not know to the game.
    command.args(options.arguments()?);
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
pub(crate) fn which(name: &str) -> Option<PathBuf> {
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
    fn a_native_game_gets_its_wrappers_arguments_and_environment() {
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.platform = Some("linux".into());
        game.details.install_path = Some("/games/Sugar".into());
        let options = LaunchOptions {
            mangohud: true,
            gamemode: true,
            xbox_controllers: true,
            arguments: "-windowed --name 'Lea P'".into(),
            environment: "DXVK_HUD=fps".into(),
            ..LaunchOptions::default()
        };
        let command =
            engine_command_with(&game, "1", Store::Gog, &options, Command::new("gogdl")).unwrap();
        assert_eq!(
            describe(&command),
            "DXVK_HUD=fps gogdl launch /games/Sugar 1 --platform linux \
             --wrapper 'gamemoderun mangohud' -windowed --name 'Lea P'"
        );
    }

    #[test]
    fn a_linux_game_added_by_hand_runs_from_its_folder() {
        let dir = std::env::temp_dir();
        let program = dir.join("zendo-test-game");
        let mut game = Game::new(
            "local:game".into(),
            "Game".into(),
            Source::Local,
            Launch::Local {
                program: program.clone(),
                args: vec!["--level".into(), "two words".into()],
                dir: Some(dir.clone()),
            },
        );
        game.details.platform = Some("linux".into());
        let options = LaunchOptions {
            gamemode: true,
            arguments: "-fullscreen".into(),
            ..LaunchOptions::default()
        };
        let command = local_command(
            &game,
            &program,
            &["--level".into(), "two words".into()],
            Some(&dir),
            &options,
        )
        .unwrap();
        assert_eq!(
            describe(&command),
            format!(
                "gamemoderun {} --level 'two words' -fullscreen",
                program.display()
            )
        );
        assert_eq!(command.get_current_dir(), Some(dir.as_path()));
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
        let watch = Watch::Group(child.id());
        assert!(running(&watch));
        kill(&watch);
        let _ = child.wait();
        assert!(!running(&watch));
    }

    #[test]
    fn finds_a_game_by_the_folder_its_launcher_names() {
        let dir = std::env::temp_dir().join(format!("zendo-folder-{}", std::process::id()));
        let game = dir.join("My Game");
        fs::create_dir_all(&game).unwrap();
        // As Steam's reaper would: the game's path among its arguments,
        // and the game itself a child of it.
        let script = format!("sleep 30 & wait # {}/Game.exe", game.display());
        let mut child = Command::new("sh")
            .args(["-c", &script, &format!("{}/Game.exe", game.display())])
            .process_group(0)
            .spawn()
            .unwrap();
        let watch = Watch::Folder(game.clone());
        let mut found = Vec::new();
        for _ in 0..50 {
            found = in_folder(&game);
            if found.len() >= 2 {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(found.contains(&child.id()));
        assert!(found.len() >= 2, "the sleep it started is found too");
        assert!(!running(&Watch::Folder(dir.join("My Gam"))));
        assert!(!running(&Watch::Folder(dir.join("My Game 2"))));
        kill(&watch);
        let _ = child.wait();
        for _ in 0..50 {
            if !running(&watch) {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!running(&watch));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_argument_names_a_folder_only_whole() {
        let folder = "/games/Foo";
        assert!(mentions("/games/Foo/foo.exe", folder));
        assert!(mentions("/games/Foo", folder));
        assert!(mentions("--path=/games/Foo", folder));
        assert!(!mentions("/games/Foo Bar/foo.exe", folder));
        assert!(!mentions("/games/Foobar", folder));
        assert!(!mentions("/other/games/Foo", folder));
    }
}
