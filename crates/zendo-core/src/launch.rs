//! Starting a game, telling whether it still runs, and stopping it.
//!
//! A game Zendo starts itself runs in a process group of its own, started
//! from the home folder (a relative path in a launcher's settings then
//! means the same wherever Zendo was started). It counts as running while
//! any process of the group is alive: legendary, for one, starts the game
//! and exits at once, and the game's processes stay in the group. Stop
//! signals the group's processes one by one, those started since the game
//! was: a group's id is given again once the group has ended, and a later
//! group with it is never taken for the game's ([`Group`]).
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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use anyhow::{Context, bail};

use crate::config::Config;
use crate::launch_options::LaunchOptions;
use crate::runtimes::{self, Runtime};
use crate::steam_runtime::Runtime as SteamRuntime;
use crate::store_tasks::{self, Chosen};
use crate::stores::Store;
use crate::{Game, Launch, Source, State, downloads, paths, proton, umu};
use crate::{cloud_saves, eos_overlay};

/// A game just started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub watch: Watch,
    /// Where its output goes, for the games Zendo starts itself.
    pub log: Option<PathBuf>,
    /// Something that went wrong on the way without stopping the game.
    pub warning: Option<String>,
}

/// How Zendo tells a game it started still runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watch {
    /// The process group Zendo started it in.
    Group(Group),
    /// The game's install folder, for a game its launcher runs.
    Folder(PathBuf),
    /// Not at all: a link with no install folder known.
    Nothing,
}

/// The process group a game was started in. Its id is its first process's
/// pid, which the system gives again once the group has ended; so the
/// group is known by that process's start time too, its processes are
/// those started since, and once it is seen ended it stays ended.
#[derive(Debug, Clone)]
pub struct Group {
    id: u32,
    /// The first process's start, in clock ticks since boot; `None` when
    /// it could not be read.
    since: Option<u64>,
    ended: Arc<AtomicBool>,
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.since == other.since
    }
}

impl Eq for Group {}

impl Group {
    /// The group a process just started leads, read while the process is
    /// there (a zombie too, before it is waited for).
    #[must_use]
    pub fn led_by(pid: u32) -> Self {
        let since = fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| parse_stat(pid, &stat))
            .map(|process| process.start);
        Self::new(pid, since)
    }

    /// A group by its id and its first process's start.
    #[must_use]
    pub fn new(id: u32, since: Option<u64>) -> Self {
        Self {
            id,
            since,
            ended: Arc::default(),
        }
    }

    /// The group's processes alive now; none, for good, once it ended.
    fn members(&self) -> Vec<u32> {
        if self.ended.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let all = processes();
        let since = self.since.unwrap_or(0);
        // The first process's pid held by a later process: the id was given
        // again, so the game's group had ended.
        let reused = self.since.is_some_and(|since| {
            all.iter()
                .any(|process| process.pid == self.id && process.start != since)
        });
        let own = std::process::id();
        let members: Vec<u32> = if reused {
            Vec::new()
        } else {
            all.iter()
                .filter(|process| process.group == self.id && process.start >= since)
                .map(|process| process.pid)
                .filter(|&pid| pid > 1 && pid != own)
                .collect()
        };
        if members.is_empty() {
            self.ended.store(true, Ordering::Relaxed);
        }
        members
    }
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
    if !matches!(game.launch, Launch::Uri { .. }) {
        let before = LaunchOptions::load(&game.id)?.before;
        if !before.trim().is_empty() {
            run_script(game, &before, Script::Before)?;
        }
    }
    let mut warning = None;
    let (mut command, log, folder) = match &game.launch {
        Launch::Uri { uri } => {
            let mut command = Command::new("xdg-open");
            command.arg(uri);
            let folder = game.details.install_path.clone().filter(|dir| dir.is_dir());
            (command, None, Some(folder))
        }
        Launch::Command { program, args, dir } => {
            let options = options_for(game)?;
            let mut words = wrappers_for(&options, false)?;
            words.push(program.clone());
            words.extend(args.iter().cloned());
            words.extend(options.arguments()?);
            let mut command = Command::new(&words[0]);
            command
                .args(&words[1..])
                .envs(options.environment_for(false)?);
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
            warning = before_engine(game);
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
    // Read before the child is waited for, so it is there to read.
    let group = Group::led_by(child.id());
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
    Ok(Started {
        watch,
        log,
        warning,
    })
}

/// What a store game's launch options ask before it starts: its cloud
/// saves brought, Epic's overlay on or off; what went wrong, which does
/// not stop the game.
fn before_engine(game: &Game) -> Option<String> {
    let options = LaunchOptions::load(&game.id).ok()?;
    let saves = cloud_saves::wanted(game, &options)
        .then(|| cloud_saves::sync(game, cloud_saves::How::Fetch).err())
        .flatten()
        .map(|err| format!("{}'s cloud saves: {err:#}", game.title));
    saves.or_else(|| apply_overlay(game, &options))
}

/// Turns Epic's overlay on or off in a Windows Epic game's prefix, as its
/// launch options say.
fn apply_overlay(game: &Game, options: &LaunchOptions) -> Option<String> {
    if game.source != Source::Epic || !needs_proton(game) {
        return None;
    }
    let prefix = prefix(game)?;
    eos_overlay::set_in(&prefix, options.eos_overlay)
        .err()
        .map(|err| format!("{}: the EOS overlay: {err:#}", game.title))
}

/// Runs the command the game's launch options give for after it ends.
///
/// # Errors
///
/// When the options cannot be read, or the command fails.
pub fn after(game: &Game) -> anyhow::Result<()> {
    if matches!(game.launch, Launch::Uri { .. }) {
        return Ok(());
    }
    let options = LaunchOptions::load(&game.id)?;
    // The saves first, so the script finds them as they are.
    let saves = if cloud_saves::wanted(game, &options) {
        cloud_saves::sync(game, cloud_saves::How::Send)
            .map(drop)
            .with_context(|| format!("{}'s cloud saves", game.title))
    } else {
        Ok(())
    };
    let script = if options.after.trim().is_empty() {
        Ok(())
    } else {
        run_script(game, &options.after, Script::After)
    };
    saves.and(script)
}

#[derive(Debug, Clone, Copy)]
enum Script {
    Before,
    After,
}

/// A script from the game's launch options, through `sh -c`: from the
/// game's folder, with its id, title, folder and Wine prefix in its
/// environment.
fn script_command(game: &Game, line: &str) -> Command {
    let mut command = Command::new("sh");
    command
        .args(["-c", line])
        .stdin(Stdio::null())
        .env("ZENDO_GAME_ID", &game.id)
        .env("ZENDO_GAME_TITLE", &game.title);
    match game
        .details
        .install_path
        .as_ref()
        .filter(|dir| dir.is_dir())
    {
        Some(folder) => {
            command.env("ZENDO_GAME_FOLDER", folder).current_dir(folder);
        }
        None => {
            if let Some(home) = paths::home() {
                command.current_dir(home);
            }
        }
    }
    let proton = matches!(game.launch, Launch::Engine { .. } | Launch::Local { .. });
    if proton
        && needs_proton(game)
        && let Some(prefix) = prefix(game)
    {
        command.env("WINEPREFIX", prefix);
    }
    command
}

/// Runs a script and waits for it; a failure says how it ended and the
/// last thing it wrote to its errors.
fn run_script(game: &Game, line: &str, when: Script) -> anyhow::Result<()> {
    let when = match when {
        Script::Before => "before",
        Script::After => "after",
    };
    let output = script_command(game, line)
        .output()
        .with_context(|| format!("could not run the script {when} {}", game.title))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.lines().rev().find(|line| !line.trim().is_empty()) {
        Some(last) => bail!(
            "the script {when} {} failed ({}): {}",
            game.title,
            output.status,
            last.trim()
        ),
        None => bail!(
            "the script {when} {} failed ({})",
            game.title,
            output.status
        ),
    }
}

/// The program the launch options choose instead of the store's, as the
/// engine takes it: legendary a path in the game's folder, relative to
/// it; gogdl a whole path.
fn chosen_program(
    game: &Game,
    store: Store,
    options: &LaunchOptions,
) -> anyhow::Result<Option<String>> {
    let Some(program) = options
        .executable
        .as_deref()
        .map(str::trim)
        .filter(|program| !program.is_empty())
    else {
        return Ok(None);
    };
    let folder = game
        .details
        .install_path
        .as_deref()
        .context("no record of where the game is")?;
    let path = Path::new(program);
    Ok(Some(match store {
        Store::Epic if path.is_absolute() => path
            .strip_prefix(folder)
            .with_context(|| format!("legendary runs only a program in {}'s folder", game.title))?
            .to_string_lossy()
            .into_owned(),
        Store::Epic => program.to_owned(),
        Store::Gog => folder.join(path).to_string_lossy().into_owned(),
        Store::Amazon => return Ok(None),
    }))
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
        Watch::Group(group) => !group.members().is_empty(),
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
        Watch::Group(group) => group.members().into_iter().collect(),
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
    let pids = match watch {
        Watch::Group(group) => group.members(),
        Watch::Folder(folder) => in_folder(folder)
            .into_iter()
            .filter(|&pid| pid > 1 && pid != std::process::id())
            .collect(),
        Watch::Nothing => Vec::new(),
    };
    let targets: Vec<String> = pids.iter().map(u32::to_string).collect();
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
    /// When it started, in clock ticks since boot.
    start: u64,
}

/// A `/proc/<pid>/stat`: `pid (name) state ppid pgrp …`, the start the
/// 22nd field; the name may hold spaces and parentheses. `None` for a
/// zombie.
fn parse_stat(pid: u32, stat: &str) -> Option<Process> {
    let (_, rest) = stat.rsplit_once(") ")?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    if *fields.first()? == "Z" {
        return None;
    }
    Some(Process {
        pid,
        parent: fields.get(1)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        start: fields.get(19)?.parse().ok()?,
    })
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
            parse_stat(pid, &stat)
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

/// What runs a Windows game: umu-run, or Steam Linux Runtime 4.0 and
/// Proton when the launch options leave umu out, and the environment
/// they read.
struct Umu {
    words: Vec<String>,
    env: Vec<(&'static str, std::ffi::OsString)>,
}

fn umu_setup(
    game: &Game,
    options: &LaunchOptions,
    store: Option<(&str, &str)>,
) -> anyhow::Result<Umu> {
    let prefix = prefix(game).context("no folder for the game's Wine prefix")?;
    let game_id = match store {
        Some((store, app)) => umu::game_id(store, app, &game.title),
        None => umu::game_id("local", &game.id, &game.title),
    };
    let mut env = vec![
        ("WINEPREFIX", prefix.clone().into_os_string()),
        ("GAMEID", game_id.clone().into()),
    ];
    if let Some((store, _)) = store {
        env.push(("STORE", store.into()));
    }
    // The one chosen, or the default when it is gone; without any, umu
    // downloads UMU-Proton on the first launch.
    let chosen = options
        .proton
        .as_deref()
        .and_then(proton::named)
        .or_else(proton::default);
    let words = if options.no_umu {
        let proton = chosen.context(
            "no Proton is installed to run the game without umu: download GE-Proton in Settings",
        )?;
        let runtime = SteamRuntime::Four.find().context(
            "Proton needs Steam Linux Runtime 4.0 to run without umu: install it in Steam, \
             or start a game once with umu, which keeps its own",
        )?;
        link_pfx(&prefix)?;
        env.extend(without_umu(game, &prefix, &game_id, &proton.path, &runtime));
        let mut words = SteamRuntime::Four.words(&runtime);
        words.push(proton.path.join("proton").to_string_lossy().into_owned());
        words.push("waitforexitandrun".into());
        words
    } else {
        let umu = which("umu-run")
            .context("umu-launcher is needed to start Windows games: install it (umu-launcher)")?;
        if let Some(proton) = chosen {
            env.push(("PROTONPATH", proton.path.into_os_string()));
        }
        vec![umu.to_string_lossy().into_owned()]
    };
    // umu leaves the anti-cheat runtimes to Steam; outside it, Proton
    // finds them only through these.
    env.extend(runtimes::environment());
    Ok(Umu { words, env })
}

/// What umu would tell Proton, for Proton run without it: the prefix as
/// Steam's data folder for the game (`pfx` in it leading back to it), no
/// Steam client, Proton and the runtime as the tools to mount, umu's id
/// for protonfixes, and the Steam app id that id holds (0 for none).
fn without_umu(
    game: &Game,
    prefix: &Path,
    game_id: &str,
    proton: &Path,
    runtime: &Path,
) -> Vec<(&'static str, std::ffi::OsString)> {
    let app = game_id
        .strip_prefix("umu-")
        .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or("0");
    let tools = format!("{}:{}", proton.display(), runtime.display());
    let folder = game
        .details
        .install_path
        .clone()
        .unwrap_or_default()
        .into_os_string();
    vec![
        ("UMU_ID", game_id.into()),
        ("STEAM_COMPAT_DATA_PATH", prefix.into()),
        ("STEAM_COMPAT_CLIENT_INSTALL_PATH", "".into()),
        ("STEAM_COMPAT_INSTALL_PATH", folder),
        ("STEAM_COMPAT_TOOL_PATHS", tools.clone().into()),
        ("STEAM_COMPAT_MOUNTS", tools.into()),
        ("STEAM_COMPAT_APP_ID", app.into()),
        ("SteamAppId", app.into()),
        ("SteamGameId", app.into()),
    ]
}

/// umu-run with a Windows game's Proton, prefix and environment, for a
/// program run in its prefix rather than the game: `game_id` is umu's
/// `GAMEID` (which protonfixes reads), the game's own when `None`.
/// Through umu even when the launch options leave it out: umu is what
/// runs winetricks, and the prefix is the same either way.
///
/// # Errors
///
/// When the game is not a Windows one Zendo starts, umu is missing, or
/// its launch options or prefix cannot be read or made.
pub(crate) fn prefix_command(game: &Game, game_id: Option<&str>) -> anyhow::Result<Command> {
    let store = match (&game.launch, game.source) {
        (Launch::Engine { app, .. }, Source::Epic) => Some((store_key(Store::Epic), app)),
        (Launch::Engine { app, .. }, Source::Gog) => Some((store_key(Store::Gog), app)),
        (Launch::Engine { app, .. }, Source::Amazon) => Some((store_key(Store::Amazon), app)),
        (Launch::Local { .. }, Source::Local) => None,
        _ => bail!("Zendo keeps no Wine prefix for {}", game.title),
    };
    if !needs_proton(game) {
        bail!("{} is not a Windows game", game.title);
    }
    let mut options = LaunchOptions::load(&game.id)?;
    options.no_umu = false;
    let store = store.map(|(key, app)| (umu::store_name(key), app.as_str()));
    let mut umu = umu_setup(game, &options, store)?;
    if let Some(id) = game_id
        && let Some(pair) = umu.env.iter_mut().find(|(key, _)| *key == "GAMEID")
    {
        pair.1 = id.into();
    }
    let mut command = Command::new(&umu.words[0]);
    command
        .args(&umu.words[1..])
        .envs(umu.env)
        .envs(options.environment_for(true)?);
    make_prefix(&command)?;
    if let Some(home) = paths::home() {
        command.current_dir(home);
    }
    Ok(command)
}

/// Starts a log afresh at `log`, headed with how `command` was started,
/// for its output.
///
/// # Errors
///
/// When the log cannot be written.
pub(crate) fn log_to(log: &Path, command: &Command) -> anyhow::Result<(Stdio, Stdio)> {
    open_log(log, command)?.stdio()
}

/// Makes the prefix with `pfx` in it leading back to itself, as umu does,
/// so Proton, which keeps the prefix in `pfx` of its data folder, uses it.
fn link_pfx(prefix: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(prefix).with_context(|| format!("could not make {}", prefix.display()))?;
    let pfx = prefix.join("pfx");
    if pfx.symlink_metadata().is_err() {
        std::os::unix::fs::symlink(".", &pfx)
            .with_context(|| format!("could not make {}", pfx.display()))?;
    }
    Ok(())
}

/// The game's wrappers, and for a native game the Steam Linux Runtime its
/// launch options choose.
fn wrappers_for(options: &LaunchOptions, windows: bool) -> anyhow::Result<Vec<String>> {
    let mut words = options.wrappers();
    if !windows && let Some(runtime) = options.steam_runtime {
        let folder = runtime.find().with_context(|| {
            format!("{} is not installed: install it in Steam", runtime.label())
        })?;
        words.extend(runtime.words(&folder));
    }
    Ok(words)
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
    let windows = needs_proton(game);
    let mut words = wrappers_for(options, windows)?;
    let mut env = Vec::new();
    if windows {
        let umu = umu_setup(game, options, None)?;
        words.extend(umu.words);
        env = umu.env;
    }
    words.push(program.to_string_lossy().into_owned());
    words.extend(args.iter().cloned());
    words.extend(options.arguments()?);
    let mut command = Command::new(&words[0]);
    command
        .args(&words[1..])
        .envs(env)
        .envs(options.environment_for(windows)?);
    if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
        command.current_dir(dir);
    }
    Ok(command)
}

/// The anti-cheat runtimes a game Zendo starts through Proton ships and
/// this machine lacks, to download before it starts. Steam's and Heroic's
/// games get theirs from their launcher.
#[must_use]
pub fn missing_runtimes(game: &Game) -> Vec<Runtime> {
    if !needs_proton(game) {
        return Vec::new();
    }
    let folder = match &game.launch {
        Launch::Engine { .. } => game.details.install_path.clone(),
        Launch::Local { program, dir, .. } => dir
            .clone()
            .or_else(|| program.parent().map(Path::to_path_buf)),
        Launch::Uri { .. } | Launch::Command { .. } => None,
    };
    folder.map_or_else(Vec::new, |folder| runtimes::missing_for(&folder))
}

/// Downloads the anti-cheat runtimes [`missing_runtimes`] names, telling
/// `report` which one it is on. A failure need not stop the launch: the
/// game may run, without what its anti-cheat guards.
///
/// # Errors
///
/// When Lutris's list cannot be read or a download fails.
pub fn fetch_runtimes(game: &Game, report: &mut dyn FnMut(Runtime)) -> anyhow::Result<()> {
    let missing = missing_runtimes(game);
    if missing.is_empty() {
        return Ok(());
    }
    let remotes = runtimes::latest()?;
    for runtime in missing {
        let remote = remotes
            .iter()
            .find(|remote| remote.runtime == runtime)
            .with_context(|| format!("Lutris lists no {} runtime", runtime.label()))?;
        report(runtime);
        runtimes::install(remote, &AtomicBool::new(false), &mut |_, _| {})?;
    }
    Ok(())
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
    let windows = needs_proton(game);
    let mut wrapper = wrappers_for(options, windows)?;
    if windows {
        let store_key = umu::store_name(store_key(store));
        let umu = umu_setup(game, options, Some((store_key, app)))?;
        command.envs(umu.env);
        wrapper.extend(umu.words);
    }
    command.envs(options.environment_for(windows)?);
    let program = chosen_program(game, store, options)?;
    let task = options
        .task
        .as_deref()
        .and_then(|key| store_tasks::chosen(game, key));
    match store {
        Store::Epic | Store::Amazon => {
            command.args(["launch", app]);
            if let Some(program) = &program {
                command.args(["--override-exe", program]);
            }
            if store == Store::Epic {
                if let Some(language) = &options.language {
                    command.args(["--language", language]);
                }
                if options.offline {
                    command.arg("--offline");
                }
                if options.skip_update_check {
                    command.arg("--skip-version-check");
                }
            }
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
            if let Some(program) = &program {
                command.args(["--override-exe", program]);
            }
            if let Some(Chosen::Index(index)) = task {
                command.args(["--prefer-task", &index.to_string()]);
            }
        }
    }
    if windows {
        command.arg("--no-wine");
    }
    if !wrapper.is_empty() {
        let line: Vec<String> = wrapper.iter().map(|word| quote(word)).collect();
        command.args(["--wrapper", &line.join(" ")]);
    }
    // The engines pass on what they do not know to the game: Epic's
    // option's arguments, then the game's own.
    if let Some(Chosen::Arguments(arguments)) = &task {
        command.args(shlex::split(arguments).unwrap_or_default());
    }
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
    fn runs_the_program_chosen_instead_of_the_stores() {
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
            executable: Some("bin/sugar".into()),
            ..LaunchOptions::default()
        };
        let command =
            engine_command_with(&game, "1", Store::Gog, &options, Command::new("gogdl")).unwrap();
        assert_eq!(
            describe(&command),
            "gogdl launch /games/Sugar 1 --platform linux --override-exe /games/Sugar/bin/sugar"
        );
        // legendary takes it relative to the game's folder, and nothing
        // outside it.
        game.source = Source::Epic;
        let whole = LaunchOptions {
            executable: Some("/games/Sugar/bin/sugar".into()),
            ..LaunchOptions::default()
        };
        let command =
            engine_command_with(&game, "1", Store::Epic, &whole, Command::new("legendary"))
                .unwrap();
        assert_eq!(
            describe(&command),
            "legendary launch 1 --override-exe bin/sugar"
        );
        let outside = LaunchOptions {
            executable: Some("/elsewhere/sugar".into()),
            ..LaunchOptions::default()
        };
        assert!(
            engine_command_with(&game, "1", Store::Epic, &outside, Command::new("legendary"))
                .is_err()
        );
    }

    #[test]
    fn tells_legendary_the_language() {
        let mut game = Game::new(
            "epic:Sugar".into(),
            "Rocket League".into(),
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        game.details.platform = Some("linux".into());
        let options = LaunchOptions {
            language: Some("pt-BR".into()),
            offline: true,
            skip_update_check: true,
            ..LaunchOptions::default()
        };
        let command = engine_command_with(
            &game,
            "Sugar",
            Store::Epic,
            &options,
            Command::new("legendary"),
        )
        .unwrap();
        assert_eq!(
            describe(&command),
            "legendary launch Sugar --language pt-BR --offline --skip-version-check"
        );
    }

    #[test]
    fn tells_proton_what_umu_would_without_it() {
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.install_path = Some("/games/Sugar".into());
        let env = without_umu(
            &game,
            Path::new("/games/Prefixes/Sugar"),
            "umu-252950",
            Path::new("/proton"),
            Path::new("/runtime"),
        );
        let value = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string_lossy().into_owned())
        };
        assert_eq!(
            value("STEAM_COMPAT_DATA_PATH").as_deref(),
            Some("/games/Prefixes/Sugar")
        );
        assert_eq!(
            value("STEAM_COMPAT_CLIENT_INSTALL_PATH").as_deref(),
            Some("")
        );
        assert_eq!(
            value("STEAM_COMPAT_TOOL_PATHS").as_deref(),
            Some("/proton:/runtime")
        );
        assert_eq!(value("SteamAppId").as_deref(), Some("252950"));
        assert_eq!(value("UMU_ID").as_deref(), Some("umu-252950"));
        let other = without_umu(
            &game,
            Path::new("/p"),
            "umu-default",
            Path::new("/a"),
            Path::new("/b"),
        );
        assert!(other.contains(&("SteamAppId", "0".into())));

        let dir = std::env::temp_dir().join(format!("zendo-pfx-{}", std::process::id()));
        let prefix = dir.join("Prefixes/Sugar");
        link_pfx(&prefix).unwrap();
        link_pfx(&prefix).unwrap();
        assert_eq!(fs::read_link(prefix.join("pfx")).unwrap(), Path::new("."));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn runs_the_scripts_with_the_game_in_their_environment() {
        let mut game = Game::new(
            "local:game".into(),
            "Game".into(),
            Source::Local,
            Launch::Command {
                program: "game".into(),
                args: Vec::new(),
                dir: None,
            },
        );
        game.details.install_path = Some(std::env::temp_dir());
        let command = script_command(&game, "echo hi");
        let env: Vec<String> = command
            .get_envs()
            .map(|(name, value)| {
                format!(
                    "{}={}",
                    name.to_string_lossy(),
                    value.unwrap_or_default().to_string_lossy()
                )
            })
            .collect();
        assert!(
            env.contains(&"ZENDO_GAME_ID=local:game".to_owned()),
            "{env:?}"
        );
        assert!(env.contains(&"ZENDO_GAME_TITLE=Game".to_owned()), "{env:?}");
        assert!(!env.iter().any(|pair| pair.starts_with("WINEPREFIX=")));
        assert_eq!(
            command.get_current_dir(),
            Some(std::env::temp_dir().as_path())
        );
        assert!(run_script(&game, "true", Script::Before).is_ok());
        let error = run_script(&game, "echo nope >&2; exit 3", Script::After).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the script after Game failed (exit status: 3): nope"
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
        let watch = Watch::Group(Group::led_by(child.id()));
        assert!(running(&watch));
        kill(&watch);
        let _ = child.wait();
        assert!(!running(&watch));
    }

    #[test]
    fn a_group_id_given_again_is_not_the_games() {
        let mut child = Command::new("sleep")
            .arg("5")
            .process_group(0)
            .spawn()
            .unwrap();
        let real = Group::led_by(child.id());
        assert!(real.since.is_some());
        // The same id, but its first process started later: as when the
        // system gives the id again after the game's group ended.
        let later = Group::new(child.id(), real.since.map(|since| since + 1_000_000));
        assert!(!running(&Watch::Group(later.clone())));
        // Once seen ended, it stays so; nothing is signalled.
        kill(&Watch::Group(later));
        assert!(
            running(&Watch::Group(real.clone())),
            "the real game still runs"
        );
        let earlier = Group::new(child.id(), real.since.map(|since| since.saturating_sub(1)));
        assert!(
            !running(&Watch::Group(earlier)),
            "its pid started at another time"
        );
        kill(&Watch::Group(real.clone()));
        let _ = child.wait();
        assert!(!running(&Watch::Group(real)));
    }

    #[test]
    fn reads_a_process_line_with_spaces_in_its_name() {
        let stat = "42 (Game (x64) .exe) S 7 42 42 0 -1 4194560 1 0 0 0 0 0 0 0 20 0 3 0 98765 1 2";
        let process = parse_stat(42, stat).unwrap();
        assert_eq!(
            (process.parent, process.group, process.start),
            (7, 42, 98765)
        );
        assert!(parse_stat(42, "42 (z) Z 7 42 42 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 5").is_none());
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
