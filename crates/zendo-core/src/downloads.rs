//! Installing, updating, repairing and uninstalling store games through
//! legendary, gogdl and nile, one at a time. The window runs each engine as
//! a child process and reads its progress; there is no background process
//! of Zendo's own. The queue is kept in
//! `$XDG_STATE_HOME/zendo/downloads.json`, so what was left picks up on the
//! next start.
//!
//! The games Zendo installed are read back from each engine's own record in
//! Zendo's folder for it: legendary's `installed.json`, nile's
//! `nile/installed.json`. gogdl keeps none, so Zendo writes an
//! `installed.json` for it in the shape Heroic uses.
//!
//! Each game goes in a folder of its own, picked when its install starts
//! and kept with the job, so a paused install carries on in the same place
//! and a cancelled one can be cleared away. An interrupted engine keeps
//! what it wrote: legendary a resume file in its `tmp` folder, gogdl a
//! `.gogdl-resume` in the game's folder, nile the files it finished.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::heroic::{self, Installed};
use crate::moving::{self, Moved};
use crate::owned::{self, Owned, StoreKey};
use crate::stores::Store;
use crate::{Details, Game, Launch, Scan, Source, paths};
use crate::{gog_import, gog_setup, info, mods};

/// The language gogdl downloads; Heroic asks the user, Zendo does not yet.
pub(crate) const GOG_LANGUAGE: &str = "en-US";

/// How long an engine has to stop after being interrupted before it is
/// killed.
const STOP_GRACE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Install,
    Update,
    Repair,
    Uninstall,
    /// Installing DLC into a game Zendo installed.
    #[serde(rename = "add-dlc")]
    AddDlc,
    /// Uninstalling DLC from a game Zendo installed.
    #[serde(rename = "remove-dlc")]
    RemoveDlc,
    /// Moving a game Zendo installed to another folder ([`Job::to`]).
    Move,
    /// Taking a game already on disk, in [`Job::path`], as Zendo's own.
    Import,
}

impl Action {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Update => "Update",
            Self::Repair => "Repair",
            Self::Uninstall => "Uninstall",
            Self::AddDlc => "Install DLC",
            Self::RemoveDlc => "Uninstall DLC",
            Self::Move => "Move",
            Self::Import => "Import",
        }
    }

    #[must_use]
    pub fn doing(self) -> &'static str {
        match self {
            Self::Install => "Installing",
            Self::Update => "Updating",
            Self::Repair => "Repairing",
            Self::Uninstall => "Uninstalling",
            Self::AddDlc => "Installing DLC",
            Self::RemoveDlc => "Uninstalling DLC",
            Self::Move => "Moving",
            Self::Import => "Importing",
        }
    }
}

/// One thing to do to one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub store: StoreKey,
    /// The store's own id for the game.
    pub app: String,
    pub title: String,
    pub action: Action,
    /// As the engine names it: `Windows` or `Mac` for legendary and nile,
    /// `windows`, `linux` or `osx` for gogdl.
    pub platform: String,
    /// For an install, the folder the game's own folder is made in;
    /// otherwise the game's folder (for an import, the one chosen).
    pub path: PathBuf,
    /// For an install, the game's own folder, once the job has started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<PathBuf>,
    /// Held in the queue until resumed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// Store ids of DLC: for an install, the ones that come with the game;
    /// for [`Action::AddDlc`] and [`Action::RemoveDlc`], the ones added or
    /// removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dlc: Vec<String>,
    /// For [`Action::Move`], the folder the game's folder goes into; it
    /// keeps its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<PathBuf>,
}

/// The build Zendo installs from a store: GOG's Linux build when there is
/// one, otherwise the Windows build.
#[must_use]
pub fn install_platform(game: &Owned) -> &'static str {
    match game.store() {
        Some(Store::Gog) if game.platforms.iter().any(|p| p == "Linux") => "linux",
        Some(Store::Gog) => "windows",
        Some(Store::Epic | Store::Amazon) | None => "Windows",
    }
}

impl Job {
    /// The game's id in the library and the Store.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}:{}", self.store().source_key(), self.app)
    }

    #[must_use]
    pub fn store(&self) -> Store {
        self.store.into()
    }

    /// Installing a store game into a folder of its own in `dir`, for the
    /// platform [`install_platform`] picks, with the DLC in `dlc`.
    #[must_use]
    pub fn install(game: &Owned, dir: PathBuf, dlc: Vec<String>) -> Option<Self> {
        let (_, app) = game.id.split_once(':')?;
        Some(Self {
            store: game.store()?.into(),
            app: app.to_owned(),
            title: game.title.clone(),
            action: Action::Install,
            platform: install_platform(game).to_owned(),
            path: dir,
            folder: None,
            paused: false,
            dlc,
            to: None,
        })
    }

    /// Importing a store game from `folder`, the game's own folder, with
    /// the DLC the engine finds there.
    #[must_use]
    pub fn import(game: &Owned, folder: PathBuf) -> Option<Self> {
        let mut job = Self::install(game, folder, Vec::new())?;
        job.action = Action::Import;
        Some(job)
    }

    /// Installing or uninstalling DLC of a game Zendo installed from Epic
    /// or GOG.
    #[must_use]
    pub fn for_dlc(game: &Game, action: Action, dlc: Vec<String>) -> Option<Self> {
        if !matches!(action, Action::AddDlc | Action::RemoveDlc)
            || !matches!(game.source, Source::Epic | Source::Gog)
            || dlc.is_empty()
        {
            return None;
        }
        let mut job = Self::for_game(game, action)?;
        job.dlc = dlc;
        Some(job)
    }

    /// Updating, repairing or uninstalling a game Zendo installed.
    #[must_use]
    pub fn for_game(game: &Game, action: Action) -> Option<Self> {
        let Launch::Engine { app, .. } = &game.launch else {
            return None;
        };
        let store = match game.source {
            Source::Epic => Store::Epic,
            Source::Gog => Store::Gog,
            Source::Amazon => Store::Amazon,
            _ => return None,
        };
        Some(Self {
            store: store.into(),
            app: app.clone(),
            title: game.title.clone(),
            action,
            platform: game
                .details
                .platform
                .clone()
                .unwrap_or_else(|| "Windows".to_owned()),
            path: game.details.install_path.clone()?,
            folder: None,
            paused: false,
            dlc: Vec::new(),
            to: None,
        })
    }

    /// Moving a game Zendo installed into the folder `to`, where its own
    /// folder goes under the same name.
    #[must_use]
    pub fn for_move(game: &Game, to: PathBuf) -> Option<Self> {
        let mut job = Self::for_game(game, Action::Move)?;
        job.to = Some(to);
        Some(job)
    }

    /// Where a move puts the game's folder.
    #[must_use]
    pub fn moved_to(&self) -> Option<PathBuf> {
        Some(self.to.as_ref()?.join(self.path.file_name()?))
    }
}

/// A folder name from a title, with what Windows and Wine cannot take left
/// out.
#[must_use]
pub fn folder_name(title: &str) -> String {
    let name: String = title
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect();
    let name = name.trim().trim_start_matches('.').trim();
    if name.is_empty() {
        "Game".to_owned()
    } else {
        name.to_owned()
    }
}

/// A folder for the game in `dir` that does not exist yet: its title, or
/// its title and a number.
fn new_folder(dir: &Path, title: &str) -> PathBuf {
    let name = folder_name(title);
    let mut folder = dir.join(&name);
    let mut number = 2;
    while folder.exists() || folder.is_symlink() {
        folder = dir.join(format!("{name} ({number})"));
        number += 1;
    }
    folder
}

/// How far a download is, from the lines the engine logs. legendary and
/// nile write `= Progress: 12.34% (5/40), Running for 00:00:10, ETA:
/// 00:01:10`, gogdl `= Progress: 12.34 5000/40000, …` in bytes written of
/// what is left; all three follow it with `= Downloaded: 12.00 MiB,
/// Written: …` and ` + Download\t- 3.20 MiB/s`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent: Option<f32>,
    pub downloaded_mib: Option<f32>,
    pub speed_mib: Option<f32>,
    /// `hh:mm:ss`.
    pub eta: Option<String>,
    /// gogdl's bytes written so far and to write in all, this run.
    pub written: Option<(u64, u64)>,
}

impl Progress {
    /// Takes in one line of the engine's log; true when something changed.
    pub fn read(&mut self, line: &str) -> bool {
        let before = self.clone();
        if let Some(rest) = after(line, "Progress: ") {
            if let Some(percent) = leading_number(rest) {
                self.percent = Some(percent.clamp(0.0, 100.0));
            }
            let counts = rest.split_whitespace().nth(1).and_then(|counts| {
                let (done, total) = counts.trim_end_matches(',').split_once('/')?;
                Some((done.parse().ok()?, total.parse().ok()?))
            });
            if counts.is_some() {
                self.written = counts;
            }
        }
        if let Some(eta) = after(line, "ETA: ") {
            let eta: String = eta.chars().take(8).collect();
            if eta.len() == 8 && eta.bytes().filter(|b| *b == b':').count() == 2 {
                self.eta = Some(eta);
            }
        }
        if let Some(mib) = after(line, "Downloaded: ").and_then(leading_number) {
            self.downloaded_mib = Some(mib);
        }
        if let Some(mib) = after(line, "Download\t- ").and_then(leading_number) {
            self.speed_mib = Some(mib);
        }
        *self != before
    }
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|at| &line[at + marker.len()..])
}

fn leading_number(text: &str) -> Option<f32> {
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    text[..end].parse().ok()
}

/// What a running job tells the window.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Progress(Progress),
    /// A step without a percentage, such as asking GOG about the game.
    Step(String),
    /// The game's own folder, picked as the install starts; the job keeps
    /// it from then on.
    Folder(PathBuf),
}

/// How a job ended, when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Done,
    Stopped,
}

/// Asks a running job to stop: the engine is interrupted, as Ctrl+C
/// would, so it can keep what it downloaded for next time.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn asked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Runs a job to its end, telling `report` how it goes. It blocks; the
/// window runs it on a thread of its own.
///
/// # Errors
///
/// When the engine is missing or fails, or the game is not where the
/// engine said it would be afterwards.
pub fn run(job: &Job, stop: &Stop, report: &mut dyn FnMut(Event)) -> anyhow::Result<Ended> {
    let store = job.store();
    let ended = match (store, job.action) {
        (_, Action::Move) => move_game(job, stop, report)?,
        (_, Action::Import) => import_game(job, stop, report)?,
        (Store::Gog, Action::Install) => gog_install(job, stop, report)?,
        (Store::Gog, Action::Update | Action::Repair | Action::AddDlc | Action::RemoveDlc) => {
            gog_refresh(job, stop, report)?
        }
        (Store::Gog, Action::Uninstall) => {
            report(Event::Step("Removing the game's files…".to_owned()));
            gog_uninstall(job)?;
            Ended::Done
        }
        (Store::Amazon, Action::AddDlc | Action::RemoveDlc) => {
            bail!("nile installs no DLC")
        }
        (Store::Epic, Action::AddDlc | Action::RemoveDlc) => epic_dlc(job, &job.dlc, stop, report)?,
        (Store::Epic, Action::Update) => {
            let ended = engine(
                store,
                &engine_args(job, &job.path, &workers()),
                stop,
                report,
            )?;
            if ended == Ended::Done {
                // legendary updates the game alone; its DLC have versions
                // of their own.
                let dlc = installed_dlc(store, &job.app);
                if dlc.is_empty() {
                    ended
                } else {
                    report(Event::Step("Updating its DLC…".to_owned()));
                    epic_dlc(job, &dlc, stop, report)?
                }
            } else {
                ended
            }
        }
        (Store::Epic | Store::Amazon, Action::Install) => {
            fs::create_dir_all(&job.path)
                .with_context(|| format!("could not make {}", job.path.display()))?;
            let folder = if let Some(folder) = &job.folder {
                folder.clone()
            } else {
                let folder = new_folder(&job.path, &job.title);
                report(Event::Folder(folder.clone()));
                folder
            };
            engine(store, &engine_args(job, &folder, &workers()), stop, report)?
        }
        (Store::Epic | Store::Amazon, _) => engine(
            store,
            &engine_args(job, &job.path, &workers()),
            stop,
            report,
        )?,
    };
    if ended == Ended::Done {
        let (installed, problems) = installs(store);
        // A record Zendo could not read is not a record saying "not
        // installed": say so instead of blaming the engine.
        if let Some(problem) = problems.first() {
            bail!(
                "could not tell whether {} is installed: {problem}",
                job.title
            );
        }
        let installed = installed.iter().any(|game| game.app == job.app);
        let uninstalling = job.action == Action::Uninstall;
        if uninstalling && installed {
            bail!("{} is still installed", job.title);
        }
        if !uninstalling && !installed {
            bail!(
                "{} ended without installing {}",
                store.engine_name(),
                job.title
            );
        }
        let dlc = installed_dlc(store, &job.app);
        match job.action {
            Action::AddDlc if job.dlc.iter().any(|id| !dlc.contains(id)) => {
                bail!("{} ended without installing the DLC", store.engine_name())
            }
            Action::RemoveDlc if job.dlc.iter().any(|id| dlc.contains(id)) => {
                bail!("the DLC is still installed")
            }
            _ => {}
        }
    }
    Ok(ended)
}

/// The store ids of the DLC installed with a game Zendo installed.
fn installed_dlc(store: Store, app: &str) -> Vec<String> {
    known(store)
        .into_iter()
        .find(|game| game.app == app)
        .map(|game| game.install.dlc.into_iter().map(|dlc| dlc.id).collect())
        .unwrap_or_default()
}

/// Installs or uninstalls Epic DLC, one at a time: legendary takes one app
/// per command, and puts a DLC in its game's folder by itself. For an
/// update, installing with `--update-only` updates them.
fn epic_dlc(
    job: &Job,
    dlc: &[String],
    stop: &Stop,
    report: &mut dyn FnMut(Event),
) -> anyhow::Result<Ended> {
    let workers = workers();
    for (index, id) in dlc.iter().enumerate() {
        if dlc.len() > 1 {
            report(Event::Step(format!("DLC {} of {}…", index + 1, dlc.len())));
        }
        let words: &[&str] = match job.action {
            Action::RemoveDlc => &["uninstall", id, "-y"],
            Action::Update => &[
                "install",
                id,
                "--update-only",
                "--platform",
                &job.platform,
                "-y",
            ],
            _ => &["install", id, "--platform", &job.platform, "-y"],
        };
        let mut args: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        if job.action != Action::RemoveDlc {
            args.extend_from_slice(&workers);
        }
        if engine(Store::Epic, &args, stop, report)? == Ended::Stopped {
            return Ok(Ended::Stopped);
        }
    }
    Ok(Ended::Done)
}

/// The download workers the user set, read when a job starts.
fn workers() -> Vec<String> {
    crate::config::Config::load_file()
        .map(|config| config.workers_args())
        .unwrap_or_default()
}

/// legendary's and nile's arguments for a job, with `workers` for those
/// that download; `folder` is the game's own folder for an install.
fn engine_args(job: &Job, folder: &Path, workers: &[String]) -> Vec<String> {
    let mut args = words_for(job, folder);
    if matches!(
        job.action,
        Action::Install | Action::Update | Action::Repair
    ) {
        args.extend_from_slice(workers);
    }
    args
}

fn words_for(job: &Job, folder: &Path) -> Vec<String> {
    let app = job.app.clone();
    let base = folder
        .parent()
        .unwrap_or(&job.path)
        .to_string_lossy()
        .into_owned();
    let name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = folder.to_string_lossy().into_owned();
    let words: Vec<&str> = match (job.store(), job.action) {
        (Store::Epic, Action::Install) => vec![
            "install",
            &app,
            "--base-path",
            &base,
            "--game-folder",
            &name,
            "--platform",
            &job.platform,
            "--skip-sdl",
            if job.dlc.is_empty() {
                "--skip-dlcs"
            } else {
                "--with-dlcs"
            },
            "-y",
        ],
        (Store::Epic, Action::Update) => vec!["install", &app, "--update-only", "--skip-sdl", "-y"],
        (Store::Epic, Action::Repair) => vec!["repair", &app, "-y"],
        (Store::Epic, Action::Uninstall) => vec!["uninstall", &app, "-y"],
        (Store::Amazon, Action::Install) => vec!["install", "--path", &folder, &app],
        (Store::Amazon, Action::Update) => vec!["update", &app],
        (Store::Amazon, Action::Repair) => vec!["verify", &app],
        (Store::Amazon, Action::Uninstall) => vec!["uninstall", &app],
        (Store::Gog, _)
        | (_, Action::AddDlc | Action::RemoveDlc | Action::Move | Action::Import) => {
            unreachable!("gogdl's jobs, DLC jobs, moves and imports have their own steps")
        }
    };
    words.into_iter().map(str::to_owned).collect()
}

/// Runs an engine, reading its log for progress, until it ends or is
/// stopped. It runs in a process group of its own, so stopping reaches
/// the download workers it starts too.
fn engine(
    store: Store,
    args: &[String],
    stop: &Stop,
    report: &mut dyn FnMut(Event),
) -> anyhow::Result<Ended> {
    Ok(engine_told(store, args, stop, report)?.0)
}

/// As [`engine`], with the last error the engine logged when it ended
/// well: legendary and nile end well when they refuse.
fn engine_told(
    store: Store,
    args: &[String],
    stop: &Stop,
    report: &mut dyn FnMut(Event),
) -> anyhow::Result<(Ended, Option<String>)> {
    let mut command = store.command()?;
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command
        .spawn()
        .with_context(|| format!("could not start {}", store.engine_name()))?;

    let (sender, lines) = mpsc::channel::<String>();
    let streams: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|out| Box::new(out) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|err| Box::new(err) as Box<dyn Read + Send>),
    ];
    for stream in streams.into_iter().flatten() {
        let sender = sender.clone();
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
    }
    drop(sender);

    let mut progress = Progress::default();
    let mut tail: VecDeque<String> = VecDeque::new();
    let mut interrupted: Option<Instant> = None;
    let mut exited: Option<Instant> = None;
    loop {
        match lines.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                if progress.read(&line) {
                    report(Event::Progress(progress.clone()));
                }
                if !line.trim().is_empty() {
                    if tail.len() == 30 {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if stop.asked() {
            match interrupted {
                None => {
                    signal_group(child.id(), "-INT");
                    interrupted = Some(Instant::now());
                }
                // gogdl can hang on a download thread after Ctrl+C.
                Some(at) if at.elapsed() > STOP_GRACE => {
                    signal_group(child.id(), "-KILL");
                    let _ = child.kill();
                }
                Some(_) => {}
            }
        }
        // A worker left behind can hold the log open after the engine ends.
        if exited.is_none() && matches!(child.try_wait(), Ok(Some(_))) {
            exited = Some(Instant::now());
        }
        if exited.is_some_and(|at| at.elapsed() > Duration::from_secs(5)) {
            break;
        }
    }
    let status = child
        .wait()
        .with_context(|| format!("{} did not end cleanly", store.engine_name()))?;
    if stop.asked() {
        return Ok((Ended::Stopped, None));
    }
    if !status.success() {
        bail!(
            "{}: {}",
            store.engine_name(),
            reason(&tail, &status.to_string())
        );
    }
    Ok((Ended::Done, last_error(&tail)))
}

/// Sends a signal to the engine's process group: `-INT` as Ctrl+C in a
/// terminal would, `-KILL` when that went unheard.
fn signal_group(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([signal, "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// The line that says why an engine failed: the last error it logged, or
/// else its last line.
fn reason(tail: &VecDeque<String>, status: &str) -> String {
    last_error(tail)
        .or_else(|| tail.back().map(|line| clean(line)))
        .unwrap_or_else(|| format!("failed ({status})"))
}

/// The last error line in an engine's log, without its level and module.
fn last_error(tail: &VecDeque<String>) -> Option<String> {
    tail.iter()
        .rev()
        .find(|line| line.contains("ERROR") || line.contains("CRITICAL") || line.contains("Error"))
        .map(|line| clean(line))
}

/// A log line without what comes before the message: legendary and
/// gogdl's `[module] LEVEL: `, nile's `LEVEL [MODULE]:\t`.
fn clean(line: &str) -> String {
    let line = line.trim();
    let line = line.split_once("] ").map_or(line, |(_, rest)| rest);
    let line = ["ERROR: ", "CRITICAL: "]
        .iter()
        .find_map(|level| line.strip_prefix(level))
        .unwrap_or(line);
    let line = ["ERROR [", "CRITICAL ["]
        .iter()
        .find_map(|level| line.strip_prefix(level))
        .and_then(|rest| rest.split_once("]:"))
        .map_or(line, |(_, message)| message.trim());
    line.chars().take(240).collect()
}

/// gogdl's arguments, with the DLC to have installed afterwards.
fn gog_args(job: &Job, verb: &str, path: &Path, dlc: &[String]) -> anyhow::Result<Vec<String>> {
    let support = Store::Gog
        .dir()
        .context("no home folder for gogdl's files")?
        .join("support")
        .join(&job.app);
    // An imported game keeps the language it was installed in.
    let language =
        gog_record_field(&job.app, "language").unwrap_or_else(|| GOG_LANGUAGE.to_owned());
    let mut args: Vec<String> = [
        verb,
        &job.app,
        "--platform",
        &job.platform,
        "--path",
        &path.to_string_lossy(),
        "--support",
        &support.to_string_lossy(),
        "--lang",
        &language,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if dlc.is_empty() {
        args.push("--skip-dlcs".to_owned());
    } else {
        args.extend(["--with-dlcs".to_owned(), "--dlcs".to_owned(), dlc.join(",")]);
    }
    Ok(args)
}

fn gog_install(job: &Job, stop: &Stop, report: &mut dyn FnMut(Event)) -> anyhow::Result<Ended> {
    report(Event::Step("Asking GOG about the game…".to_owned()));
    let info = info::gog(&job.app, &job.platform)?;
    fs::create_dir_all(&job.path)
        .with_context(|| format!("could not make {}", job.path.display()))?;
    // gogdl compares with the manifest of the last install it finished; one
    // left from before an uninstall makes it find nothing to download.
    if !read_gog_records()?
        .1
        .iter()
        .any(|record| is_record_of(record, &job.app))
    {
        forget_gog_manifest(&job.app);
    }
    let expected = info.folder_name.as_ref().map(|name| job.path.join(name));
    if job.folder.is_none()
        && let Some(expected) = &expected
    {
        // Only a download gogdl left half done is carried on.
        if expected.exists() && !expected.join(".gogdl-resume").is_file() {
            bail!(
                "{} is there already; move or delete it, then install again",
                expected.display()
            );
        }
        report(Event::Folder(expected.clone()));
    }
    let before = subfolders(&job.path);
    // A resumed download counts only what is left; shown against the
    // whole game, it carries on from where the bar was.
    let whole = info.disk_size();
    let mut args = gog_args(job, "download", &job.path, &job.dlc)?;
    args.extend(workers());
    let ended = engine(Store::Gog, &args, stop, &mut |event| {
        report(overall(event, whole));
    })?;
    if ended == Ended::Stopped {
        return Ok(ended);
    }
    // gogdl puts the game in a folder named after GOG's install directory;
    // its Linux installers may not say which, so look for the new one.
    let folder = expected
        .filter(|path| path.is_dir())
        .or_else(|| {
            subfolders(&job.path)
                .into_iter()
                .find(|path| !before.contains(path))
        })
        .context("gogdl ended, but the game's folder is not there")?;
    record_gog(job, &folder, &info, &job.dlc)?;
    Ok(Ended::Done)
}

fn gog_refresh(job: &Job, stop: &Stop, report: &mut dyn FnMut(Event)) -> anyhow::Result<Ended> {
    report(Event::Step("Asking GOG about the game…".to_owned()));
    let info = info::gog(&job.app, &job.platform)?;
    let verb = if job.action == Action::Repair {
        "repair"
    } else {
        "update"
    };
    // gogdl deletes what its new list of files leaves out, so the DLC to
    // keep are named every time; adding or removing DLC is an update with
    // a new list.
    let mut dlc = gog_installed_dlc(&job.app)?;
    match job.action {
        Action::AddDlc => {
            for id in &job.dlc {
                if !dlc.contains(id) {
                    dlc.push(id.clone());
                }
            }
        }
        Action::RemoveDlc => dlc.retain(|id| !job.dlc.contains(id)),
        _ => {}
    }
    let mut args = gog_args(job, verb, &job.path, &dlc)?;
    args.extend(workers());
    let ended = engine(Store::Gog, &args, stop, report)?;
    if ended == Ended::Done {
        record_gog(job, &job.path, &info, &dlc)?;
    }
    Ok(ended)
}

/// gogdl's progress as a share of the whole game, `whole` bytes on disk.
fn overall(event: Event, whole: u64) -> Event {
    let Event::Progress(mut progress) = event else {
        return event;
    };
    if let Some((done, left)) = progress.written
        && whole > 0
        && left <= whole
    {
        #[expect(
            clippy::cast_precision_loss,
            reason = "two decimals of a percentage; the precision lost is far below"
        )]
        let share = (whole - left + done.min(left)) as f64 / whole as f64;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a share of 0 to 100 fits an f32"
        )]
        let percent = (share * 100.0) as f32;
        progress.percent = Some(percent.clamp(0.0, 100.0));
    }
    Event::Progress(progress)
}

/// Removes gogdl's manifest of a game, kept from its last finished install.
fn forget_gog_manifest(app: &str) {
    if let Some(dir) = Store::Gog.dir() {
        let _ = fs::remove_file(dir.join("heroic_gogdl/manifests").join(app));
    }
}

/// The GOG build Zendo installed, from its record.
pub(crate) fn gog_build(app: &str) -> Option<String> {
    gog_record_field(app, "buildId")
}

/// One text field of Zendo's record of a GOG game.
fn gog_record_field(app: &str, field: &str) -> Option<String> {
    read_gog_records()
        .ok()?
        .1
        .iter()
        .find(|record| is_record_of(record, app))?
        .get(field)?
        .as_str()
        .map(str::to_owned)
}

fn subfolders(dir: &Path) -> HashSet<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

fn gog_record_file() -> anyhow::Result<PathBuf> {
    Ok(Store::Gog
        .dir()
        .context("no home folder for gogdl's files")?
        .join("installed.json"))
}

/// Zendo's record of the GOG games it installed, as Heroic writes its own:
/// `{"installed": [{"appName": …, "install_path": …}, …]}`.
fn read_gog_records() -> anyhow::Result<(PathBuf, Vec<serde_json::Value>)> {
    let file = gog_record_file()?;
    let records = match fs::read_to_string(&file) {
        Ok(text) => {
            let value: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("could not read {}", file.display()))?;
            value
                .get("installed")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default()
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err).with_context(|| format!("could not read {}", file.display())),
    };
    Ok((file, records))
}

fn write_gog_records(file: &Path, records: &[serde_json::Value]) -> anyhow::Result<()> {
    let json = serde_json::to_vec_pretty(&serde_json::json!({ "installed": records }))?;
    paths::write_atomic(file, &json).with_context(|| format!("could not write {}", file.display()))
}

fn is_record_of(record: &serde_json::Value, app: &str) -> bool {
    record.get("appName").and_then(serde_json::Value::as_str) == Some(app)
}

/// Writes the game's record; `dlc` are the DLC now installed with it.
fn record_gog(
    job: &Job,
    folder: &Path,
    info: &info::GogInfo,
    dlc: &[String],
) -> anyhow::Result<()> {
    let (file, mut records) = read_gog_records()?;
    records.retain(|record| !is_record_of(record, &job.app));
    let size = info.disk_size()
        + info
            .dlc()
            .iter()
            .filter(|item| dlc.contains(&item.id))
            .filter_map(|item| item.install_size)
            .sum::<u64>();
    records.push(serde_json::json!({
        "appName": job.app,
        "platform": job.platform,
        "install_path": folder,
        "install_size": (size > 0).then_some(size),
        "version": info.version_name,
        "buildId": info.build_id,
        "language": GOG_LANGUAGE,
        "is_dlc": false,
        "installedDLCs": dlc,
    }));
    write_gog_records(&file, &records)
}

/// The DLC Zendo's record says are installed with a GOG game.
fn gog_installed_dlc(app: &str) -> anyhow::Result<Vec<String>> {
    Ok(read_gog_records()?
        .1
        .iter()
        .find(|record| is_record_of(record, app))
        .and_then(|record| record.get("installedDLCs"))
        .and_then(serde_json::Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}

/// gogdl has no uninstall: Zendo deletes the folder it recorded, after
/// making sure it is a game's folder and not something bigger.
fn gog_uninstall(job: &Job) -> anyhow::Result<()> {
    let (file, mut records) = read_gog_records()?;
    let recorded = records
        .iter()
        .find(|record| is_record_of(record, &job.app))
        .and_then(|record| record.get("install_path"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .context("Zendo has no record of installing this game")?;
    let others: Vec<PathBuf> = records
        .iter()
        .filter(|record| !is_record_of(record, &job.app))
        .filter_map(|record| record.get("install_path")?.as_str().map(PathBuf::from))
        .collect();
    let install_dir = crate::config::Config::load_file()
        .ok()
        .and_then(|config| config.install_dir());
    let holds_others = others.iter().any(|other| other.starts_with(&recorded));
    if !safe_to_delete(&recorded) || holds_others || install_dir.as_deref() == Some(&*recorded) {
        bail!(
            "will not delete {}: it does not look like a game's folder",
            recorded.display()
        );
    }
    match fs::remove_dir_all(&recorded) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("could not delete {}", recorded.display()));
        }
    }
    if let Some(dir) = Store::Gog.dir() {
        let _ = fs::remove_dir_all(dir.join("support").join(&job.app));
    }
    forget_gog_manifest(&job.app);
    records.retain(|record| !is_record_of(record, &job.app));
    write_gog_records(&file, &records)
}

/// A game's folder is absolute, at least three levels deep (as in
/// `/mnt/games/Game`), not the home folder, one above it or one right in
/// it (`~/Games` itself), and not a link.
fn safe_to_delete(path: &Path) -> bool {
    let deep_enough = path.is_absolute() && path.components().count() >= 4;
    let home = paths::home();
    let near_home = home
        .as_deref()
        .is_some_and(|home| home.starts_with(path) || path.parent() == Some(home));
    deep_enough && !near_home && !path.is_symlink()
}

/// Moves a game's folder into [`Job::to`], then has the engine's record
/// say the new place, then deletes the old folder when it was copied.
/// A move carried on after a stop or a crash picks up where it was: the
/// folder moved and the record not yet, or the record changed and the old
/// copy still there.
fn move_game(job: &Job, stop: &Stop, report: &mut dyn FnMut(Event)) -> anyhow::Result<Ended> {
    let old = &job.path;
    let new = job.moved_to().context("the move has no folder to go to")?;
    if new == *old {
        bail!("{} is in that folder already", job.title);
    }
    let recorded = recorded_path(job.store(), &job.app);
    if recorded.as_ref() == Some(&new) {
        if old.exists() {
            delete_old(job, old, report)?;
        }
        return Ok(Ended::Done);
    }
    if recorded.as_ref() != Some(old) {
        bail!(
            "{} records {} in another folder than the move's",
            job.store().engine_name(),
            job.title
        );
    }
    let moved = if !old.exists() && new.is_dir() {
        Moved::Renamed
    } else {
        report(Event::Step("Moving the game's files…".to_owned()));
        let started = Instant::now();
        moving::move_folder(old, &new, &|| stop.asked(), &mut |done, total| {
            report(Event::Progress(copy_progress(
                done,
                total,
                started.elapsed(),
            )));
        })?
    };
    if moved == Moved::Stopped {
        return Ok(Ended::Stopped);
    }
    report(Event::Step("Writing down the new folder…".to_owned()));
    if let Err(err) = record_move(job, &new, report) {
        let put_back = match moved {
            Moved::Renamed => fs::rename(&new, old).map_err(anyhow::Error::from),
            _ => fs::remove_dir_all(&new).map_err(anyhow::Error::from),
        };
        return Err(match put_back {
            Ok(()) => err,
            Err(back) => err.context(format!(
                "and the files are left in {} ({back:#})",
                new.display()
            )),
        });
    }
    // What else knows the folder; the game is in its new place whatever
    // they say, so they do not fail the move.
    let _ = mods::moved(&job.id(), old, &new);
    if let Some(game) = scan().games.into_iter().find(|game| game.id == job.id()) {
        let _ = gog_setup::forget(&game);
    }
    if moved == Moved::Copied {
        delete_old(job, old, report)?;
    }
    Ok(Ended::Done)
}

/// Deletes the folder a game was copied out of, once nothing records it.
fn delete_old(job: &Job, old: &Path, report: &mut dyn FnMut(Event)) -> anyhow::Result<()> {
    let holds_a_game = recorded_folders()?
        .iter()
        .any(|path| path.starts_with(old) || old.starts_with(path));
    if holds_a_game || !safe_to_delete(old) {
        bail!(
            "{} moved, but its old folder {} was left: it does not look like only this game's",
            job.title,
            old.display()
        );
    }
    report(Event::Step("Deleting the old folder…".to_owned()));
    fs::remove_dir_all(old).with_context(|| {
        format!(
            "{} moved, but its old folder {} could not be deleted",
            job.title,
            old.display()
        )
    })
}

/// A copy's progress in the shape a download's has.
fn copy_progress(done: u64, total: u64, elapsed: Duration) -> Progress {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a percentage and a speed to one decimal; the precision lost is far below"
    )]
    let (done_f, total_f) = (done as f64, total as f64);
    let percent = if total == 0 {
        100.0
    } else {
        done_f / total_f * 100.0
    };
    let seconds = elapsed.as_secs_f64();
    let speed = (seconds > 0.0).then(|| done_f / seconds);
    let eta = speed.filter(|speed| *speed > 0.0).map(|speed| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "seconds left, positive and far below u64's limit"
        )]
        let left = ((total_f - done_f) / speed) as u64;
        format!("{:02}:{:02}:{:02}", left / 3600, left / 60 % 60, left % 60)
    });
    #[expect(clippy::cast_possible_truncation, reason = "shown to one decimal")]
    Progress {
        percent: Some(percent as f32),
        downloaded_mib: None,
        speed_mib: speed.map(|speed| (speed / 1_048_576.0) as f32),
        eta,
        written: Some((done, total)),
    }
}

/// The folder the engine records the game in.
fn recorded_path(store: Store, app: &str) -> Option<PathBuf> {
    known(store)
        .into_iter()
        .find(|game| game.app == app)
        .and_then(|game| game.install.path)
}

/// Has the engine's record say the game's new folder: legendary's
/// `move --skip-move` for the game and each DLC in its folder, Zendo's own
/// record for gogdl, nile's `installed.json`.
fn record_move(job: &Job, new: &Path, report: &mut dyn FnMut(Event)) -> anyhow::Result<()> {
    let store = job.store();
    match store {
        Store::Epic => {
            let base = new.parent().context("the new folder has no parent")?;
            // Not stopped halfway: the records change together.
            let stop = Stop::default();
            for app in legendary_apps_in(&job.path)? {
                let args: Vec<String> = ["move", &app, &base.to_string_lossy(), "--skip-move"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                engine(store, &args, &stop, report)?;
            }
        }
        Store::Gog => {
            let (file, mut records) = read_gog_records()?;
            let record = records
                .iter_mut()
                .find(|record| is_record_of(record, &job.app))
                .context("Zendo has no record of installing this game")?;
            record["install_path"] = serde_json::json!(new);
            write_gog_records(&file, &records)?;
        }
        Store::Amazon => {
            let file = store
                .dir()
                .context("no home folder for nile's files")?
                .join("nile/installed.json");
            let text = fs::read_to_string(&file)
                .with_context(|| format!("could not read {}", file.display()))?;
            let mut records: Vec<serde_json::Value> = serde_json::from_str(&text)
                .with_context(|| format!("could not read {}", file.display()))?;
            let record = records
                .iter_mut()
                .find(|record| {
                    record.get("id").and_then(serde_json::Value::as_str) == Some(&job.app)
                })
                .context("nile has no record of installing this game")?;
            record["path"] = serde_json::json!(new);
            let json = serde_json::to_vec_pretty(&records)?;
            paths::write_atomic(&file, &json)
                .with_context(|| format!("could not write {}", file.display()))?;
        }
    }
    // legendary ends well when it refuses, so its record is read back.
    if recorded_path(store, &job.app).as_deref() != Some(new) {
        bail!(
            "{} still records {} in {}",
            store.engine_name(),
            job.title,
            job.path.display()
        );
    }
    Ok(())
}

/// Takes the game in [`Job::path`] as Zendo's own: legendary's and nile's
/// `import`, which check the files against the store's list; for GOG,
/// what gogdl reads in the folder, the manifest of that build and Zendo's
/// record. Nothing in the folder is changed or deleted, whatever happens.
fn import_game(job: &Job, stop: &Stop, report: &mut dyn FnMut(Event)) -> anyhow::Result<Ended> {
    let store = job.store();
    let folder = &job.path;
    if !folder.is_dir() {
        bail!("{} is not a folder", folder.display());
    }
    if known(store).iter().any(|game| game.app == job.app) {
        bail!("Zendo has {} installed already", job.title);
    }
    let taken = Store::ALL
        .into_iter()
        .flat_map(known)
        .filter_map(|game| game.install.path)
        .find(|path| path.starts_with(folder) || folder.starts_with(path));
    if let Some(taken) = taken {
        bail!("{} holds another game of Zendo's", taken.display());
    }
    report(Event::Step("Checking the game's files…".to_owned()));
    let path = folder.to_string_lossy().into_owned();
    let words: Vec<&str> = match store {
        Store::Epic => vec![
            "import",
            &job.app,
            &path,
            "--with-dlcs",
            "--platform",
            &job.platform,
            "-y",
        ],
        Store::Amazon => vec!["import", "--path", &path, &job.app],
        Store::Gog => {
            gog_import(job, folder, report)?;
            return Ok(Ended::Done);
        }
    };
    let args: Vec<String> = words.into_iter().map(str::to_owned).collect();
    let (ended, error) = engine_told(store, &args, stop, report)?;
    let (imported, problems) = installs(store);
    if let Some(problem) = problems.first() {
        bail!(
            "could not tell whether {} was imported: {problem}",
            job.title
        );
    }
    if ended == Ended::Done && !imported.iter().any(|game| game.app == job.app) {
        bail!(
            "{} did not import {}: {}",
            store.engine_name(),
            job.title,
            error.unwrap_or_else(|| "it gave no reason".to_owned())
        );
    }
    Ok(ended)
}

/// Imports a GOG game: the folder must hold this game, then gogdl's
/// manifest is remade and Zendo's record written with the folder's build,
/// language and DLC.
fn gog_import(job: &Job, folder: &Path, report: &mut dyn FnMut(Event)) -> anyhow::Result<()> {
    let found = gog_import::read_folder(folder)?;
    if found.app.as_deref() != Some(job.app.as_str()) {
        bail!(
            "{} holds {}, not {}",
            folder.display(),
            found.title.as_deref().unwrap_or("another game"),
            job.title
        );
    }
    report(Event::Step(
        "Asking GOG for the game's list of files…".to_owned(),
    ));
    gog_import::restore_manifest(&job.app, &found)?;
    let size = moving::size(folder).ok().filter(|size| *size > 0);
    let (file, mut records) = read_gog_records()?;
    records.retain(|record| !is_record_of(record, &job.app));
    records.push(serde_json::json!({
        "appName": job.app,
        "platform": found.platform,
        "install_path": folder,
        "install_size": size,
        "version": found.version,
        "buildId": found.build,
        "language": found.language.as_deref().unwrap_or(GOG_LANGUAGE),
        "is_dlc": false,
        "installedDLCs": found.dlcs,
    }));
    write_gog_records(&file, &records)
}

/// legendary's apps installed in a folder: the game and its DLC, which
/// legendary records in the game's folder too.
fn legendary_apps_in(folder: &Path) -> anyhow::Result<Vec<String>> {
    let file = Store::Epic
        .dir()
        .context("no home folder for legendary's files")?
        .join("installed.json");
    let text =
        fs::read_to_string(&file).with_context(|| format!("could not read {}", file.display()))?;
    let records: HashMap<String, serde_json::Value> = serde_json::from_str(&text)
        .with_context(|| format!("could not read {}", file.display()))?;
    let mut apps: Vec<String> = records
        .into_iter()
        .filter(|(_, record)| {
            record
                .get("install_path")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|path| Path::new(path) == folder)
        })
        .map(|(app, _)| app)
        .collect();
    apps.sort();
    Ok(apps)
}

/// Clears away what a cancelled install left: the game's folder and what
/// the engine kept to carry on. Nothing is removed for a game the engine
/// records as installed, nor a folder outside the one the job installs
/// into or holding a game that is.
///
/// # Errors
///
/// When the folder cannot be deleted, or does not look like the game's.
pub fn discard(job: &Job) -> anyhow::Result<()> {
    if job.action != Action::Install {
        return Ok(());
    }
    let store = job.store();
    let (installed, problems) = installs(store);
    // Deleting is not undone: a record Zendo could not read is a reason to
    // stop, not a reason to decide the game is not installed.
    if let Some(problem) = problems.first() {
        bail!(
            "could not tell whether {} is installed: {problem}",
            job.title
        );
    }
    if installed.iter().any(|game| game.app == job.app) {
        return Ok(());
    }
    if let Some(dir) = store.dir() {
        match store {
            // legendary names it after the app, as its `clean_filename`
            // leaves ordinary ids.
            Store::Epic => {
                let _ = fs::remove_file(dir.join("tmp").join(format!("{}.resume", job.app)));
            }
            Store::Gog => {
                let _ = fs::remove_dir_all(dir.join("support").join(&job.app));
                forget_gog_manifest(&job.app);
            }
            Store::Amazon => {}
        }
    }
    let Some(folder) = &job.folder else {
        return Ok(());
    };
    if !folder.exists() {
        return Ok(());
    }
    let recorded = recorded_folders()?;
    let inside = folder.parent() == Some(job.path.as_path());
    let holds_a_game = recorded
        .iter()
        .any(|path| path.starts_with(folder) || folder.starts_with(path));
    if !inside || holds_a_game || !safe_to_delete(folder) {
        bail!(
            "left {} alone: it does not look like this game's folder",
            folder.display()
        );
    }
    fs::remove_dir_all(folder).with_context(|| format!("could not delete {}", folder.display()))
}

/// What Zendo installed from this store, from the engine's own record, and
/// anything that record could not be read from.
fn installs(store: Store) -> (Vec<Installed>, Vec<String>) {
    let mut scan = Scan::default();
    let installed = installs_into(store, &mut scan);
    (installed, scan.problems)
}

/// The same list, where an unreadable record only means "not known", as for
/// a lookup that changes nothing.
fn known(store: Store) -> Vec<Installed> {
    installs(store).0
}

/// Every folder a store records a game in. Refuses when a record cannot be
/// read, because callers use this to decide whether a folder may go.
fn recorded_folders() -> anyhow::Result<Vec<PathBuf>> {
    let mut folders = Vec::new();
    for store in Store::ALL {
        let (installed, problems) = installs(store);
        if let Some(problem) = problems.first() {
            bail!("could not tell where games are installed: {problem}");
        }
        folders.extend(installed.into_iter().filter_map(|game| game.install.path));
    }
    Ok(folders)
}

fn installs_into(store: Store, scan: &mut Scan) -> Vec<Installed> {
    let Some(dir) = store.dir() else {
        return Vec::new();
    };
    match store {
        Store::Epic => heroic::legendary_installs(&dir.join("installed.json"), scan),
        Store::Gog => heroic::gog_installs(&dir.join("installed.json"), scan),
        Store::Amazon => heroic::nile_installs(&dir.join("nile"), scan),
    }
}

/// The games Zendo installed, with titles and images from the Store's
/// lists.
pub(crate) fn scan() -> Scan {
    let mut scan = Scan::default();
    for store in Store::ALL {
        let installed = installs_into(store, &mut scan);
        if installed.is_empty() {
            continue;
        }
        let known: HashMap<String, Owned> = owned::cached(store.into())
            .into_iter()
            .map(|game| (game.id.clone(), game))
            .collect();
        for game in installed {
            let id = format!("{}:{}", store.source_key(), game.app);
            scan.games.push(zendo_game(store, id, game, &known));
        }
    }
    scan
}

fn zendo_game(store: Store, id: String, game: Installed, known: &HashMap<String, Owned>) -> Game {
    let owned = known.get(&id);
    let title = game
        .title
        .clone()
        .or_else(|| owned.map(|owned| owned.title.clone()))
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| game.app.clone());
    let mut found = Game::new(
        id,
        title,
        store.source(),
        Launch::Engine {
            runner: store.engine_name().to_owned(),
            app: game.app,
        },
    );
    found.cover_url = owned.and_then(|owned| owned.cover_url.clone());
    found.hero_url = owned.and_then(|owned| owned.hero_url.clone());
    found.details = Details {
        developer: owned.and_then(|owned| owned.developer.clone()),
        platform: game.install.platform,
        version: game.install.version.filter(|version| !version.is_empty()),
        install_path: game.install.path,
        size: game.install.size,
        dlc: game.install.dlc,
    };
    found
}

fn queue_file() -> Option<PathBuf> {
    paths::state_home().map(|dir| dir.join("zendo/downloads.json"))
}

/// The jobs left from last time.
///
/// # Errors
///
/// When the file exists but cannot be read.
pub fn load_queue() -> anyhow::Result<Vec<Job>> {
    let Some(file) = queue_file() else {
        return Ok(Vec::new());
    };
    match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("could not read {}", file.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).with_context(|| format!("could not read {}", file.display())),
    }
}

/// Keeps the queue for the next start; an empty queue removes the file.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save_queue(jobs: &[Job]) -> anyhow::Result<()> {
    let file = queue_file().context("no home folder to keep the downloads in")?;
    if jobs.is_empty() {
        return match fs::remove_file(&file) {
            Err(err) if err.kind() != ErrorKind::NotFound => {
                Err(err).with_context(|| format!("could not remove {}", file.display()))
            }
            _ => Ok(()),
        };
    }
    let json = serde_json::to_vec_pretty(jobs)?;
    paths::write_atomic(&file, &json).with_context(|| format!("could not write {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_progress_from_each_engine() {
        let mut progress = Progress::default();
        // legendary and nile.
        assert!(progress.read(
            "[DLManager] INFO: = Progress: 12.34% (5/40), Running for 00:00:10, ETA: 00:01:10"
        ));
        assert!(progress.read("[DLManager] INFO: = Downloaded: 120.50 MiB, Written: 118.00 MiB"));
        assert!(
            progress.read(
                "[DLManager] INFO:  + Download\t- 3.20 MiB/s (raw) / 6.10 MiB/s (decompressed)"
            )
        );
        assert!(
            !progress.read("[DLManager] INFO:  + Disk\t- 6.00 MiB/s (write) / 0.00 MiB/s (read)")
        );
        assert_eq!(
            progress,
            Progress {
                percent: Some(12.34),
                downloaded_mib: Some(120.5),
                speed_mib: Some(3.2),
                eta: Some("00:01:10".into()),
                written: None,
            }
        );
        // gogdl writes the percentage without a sign.
        assert!(progress.read(
            "[PROGRESS] INFO: = Progress: 50.00 100/200, Running for: 00:00:20, ETA: 00:00:20"
        ));
        assert_eq!(progress.percent, Some(50.0));
        assert_eq!(progress.eta.as_deref(), Some("00:00:20"));
        assert_eq!(progress.written, Some((100, 200)));
        // legendary's counts are files, in brackets, and are left alone.
        let mut legendary = Progress::default();
        legendary.read("= Progress: 12.34% (5/40), Running for 00:00:10, ETA: 00:01:10");
        assert_eq!(legendary.written, None);
    }

    #[test]
    fn a_resumed_gog_download_carries_on_where_it_was() {
        // 300 of 1000 bytes were written last time; 70 of the 700 left now.
        let progress = Progress {
            percent: Some(10.0),
            written: Some((70, 700)),
            ..Progress::default()
        };
        let Event::Progress(progress) = overall(Event::Progress(progress), 1000) else {
            unreachable!()
        };
        assert_eq!(progress.percent, Some(37.0));
    }

    #[test]
    fn says_why_an_engine_failed() {
        let tail: VecDeque<String> = [
            "[cli] INFO: Preparing download for \"Game\"...",
            "[cli] ERROR: Not enough available disk space! 1.00 GiB < 5.00 GiB",
            "[cli] INFO: Done",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            reason(&tail, "exit status: 1"),
            "Not enough available disk space! 1.00 GiB < 5.00 GiB"
        );
        assert_eq!(
            reason(&VecDeque::new(), "exit status: 1"),
            "failed (exit status: 1)"
        );
    }

    #[test]
    fn builds_each_engines_arguments() {
        let job = Job {
            store: StoreKey::Epic,
            app: "Sugar".into(),
            title: "Sugar".into(),
            action: Action::Install,
            platform: "Windows".into(),
            path: "/home/me/Games".into(),
            folder: None,
            paused: false,
            dlc: Vec::new(),
            to: None,
        };
        let folder = Path::new("/home/me/Games/Sugar Rush");
        assert_eq!(
            engine_args(&job, folder, &[]).join(" "),
            "install Sugar --base-path /home/me/Games --game-folder Sugar Rush --platform Windows --skip-sdl --skip-dlcs -y"
        );
        let job = Job {
            store: StoreKey::Amazon,
            ..job
        };
        assert_eq!(
            engine_args(&job, folder, &[]),
            ["install", "--path", "/home/me/Games/Sugar Rush", "Sugar"]
        );
        let job = Job {
            action: Action::Repair,
            ..job
        };
        assert_eq!(engine_args(&job, &job.path, &[]), ["verify", "Sugar"]);
        assert_eq!(job.id(), "amazon:Sugar");
        // The workers go with what downloads, not with an uninstall.
        let workers = ["--max-workers".to_owned(), "4".to_owned()];
        assert_eq!(
            engine_args(&job, &job.path, &workers),
            ["verify", "Sugar", "--max-workers", "4"]
        );
        let job = Job {
            action: Action::Uninstall,
            ..job
        };
        assert_eq!(
            engine_args(&job, &job.path, &workers),
            ["uninstall", "Sugar"]
        );
    }

    #[test]
    fn names_the_dlc_to_install_or_keep() {
        let job = Job {
            store: StoreKey::Epic,
            app: "Sugar".into(),
            title: "Sugar".into(),
            action: Action::Install,
            platform: "Windows".into(),
            path: "/home/me/Games".into(),
            folder: None,
            paused: false,
            dlc: vec!["Wings".into()],
            to: None,
        };
        assert!(
            engine_args(&job, Path::new("/home/me/Games/Sugar"), &[])
                .contains(&"--with-dlcs".to_owned())
        );
        let job = Job {
            store: StoreKey::Gog,
            app: "1207658924".into(),
            action: Action::Update,
            ..job
        };
        let args = gog_args(&job, "update", &job.path, &["17".into(), "18".into()]).unwrap();
        assert!(
            args.join(" ").ends_with("--with-dlcs --dlcs 17,18"),
            "{args:?}"
        );
        let args = gog_args(&job, "update", &job.path, &[]).unwrap();
        assert_eq!(args.last().map(String::as_str), Some("--skip-dlcs"));
    }

    #[test]
    fn keeps_a_dlc_job_in_the_queue() {
        let job = Job {
            store: StoreKey::Gog,
            app: "1207658924".into(),
            title: "UT".into(),
            action: Action::AddDlc,
            platform: "windows".into(),
            path: "/home/me/Games/UT".into(),
            folder: None,
            paused: false,
            dlc: vec!["17".into()],
            to: None,
        };
        let json = serde_json::to_string(&job).unwrap();
        assert!(
            json.contains(r#""action":"add-dlc""#) && json.contains(r#""dlc":["17"]"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Job>(&json).unwrap(), job);
        // A queue written before DLC has none.
        let old = r#"{"store":"gog","app":"1","title":"T","action":"update","platform":"windows","path":"/g"}"#;
        assert_eq!(
            serde_json::from_str::<Job>(old).unwrap().dlc,
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_move_keeps_its_folder_in_the_queue_and_its_name() {
        let mut game = Game::new(
            "epic:Sugar".into(),
            "Sugar Rush".into(),
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        game.details.install_path = Some("/home/me/Games/Sugar Rush".into());
        let job = Job::for_move(&game, "/mnt/games".into()).unwrap();
        assert_eq!(job.action, Action::Move);
        assert_eq!(job.path, Path::new("/home/me/Games/Sugar Rush"));
        assert_eq!(
            job.moved_to().as_deref(),
            Some(Path::new("/mnt/games/Sugar Rush"))
        );
        let json = serde_json::to_string(&job).unwrap();
        assert!(
            json.contains(r#""action":"move""#) && json.contains(r#""to":"/mnt/games""#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Job>(&json).unwrap(), job);
        // Not for a game Zendo did not install.
        game.launch = Launch::Uri {
            uri: "heroic://launch/Sugar".into(),
        };
        assert!(Job::for_move(&game, "/mnt/games".into()).is_none());
    }

    #[test]
    fn a_copy_tells_its_progress_as_a_download_does() {
        let progress = copy_progress(256 << 20, 1 << 30, Duration::from_secs(2));
        assert_eq!(progress.percent, Some(25.0));
        assert_eq!(progress.speed_mib, Some(128.0));
        assert_eq!(progress.eta.as_deref(), Some("00:00:06"));
        assert_eq!(progress.written, Some((256 << 20, 1 << 30)));
        // An empty game is done at once.
        assert_eq!(copy_progress(0, 0, Duration::ZERO).percent, Some(100.0));
    }

    #[test]
    fn an_import_takes_the_folder_chosen_as_the_games() {
        let game = Owned::new(Store::Epic, "Sugar", "Sugar Rush".into());
        let job = Job::import(&game, "/mnt/old/Sugar Rush".into()).unwrap();
        assert_eq!(job.action, Action::Import);
        assert_eq!(job.path, Path::new("/mnt/old/Sugar Rush"));
        assert_eq!(job.platform, "Windows");
        assert!(job.dlc.is_empty() && job.to.is_none());
        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains(r#""action":"import""#), "{json}");
    }

    #[test]
    fn says_why_an_engine_refused() {
        let tail: VecDeque<String> = [
            "[cli] INFO: Loading installed games...",
            "[cli] ERROR: Game executable could not be found at \"/g/Game.exe\", please verify that the specified path is correct.",
            "[cli] INFO: Goodbye",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            last_error(&tail).as_deref(),
            Some(
                "Game executable could not be found at \"/g/Game.exe\", please verify that the specified path is correct."
            )
        );
        // nile's own shape.
        let nile: VecDeque<String> =
            ["ERROR [IMPORT]:\tThere are missing or corrupted files for Sugar. Failed import."]
                .into_iter()
                .map(str::to_owned)
                .collect();
        assert_eq!(
            last_error(&nile).as_deref(),
            Some("There are missing or corrupted files for Sugar. Failed import.")
        );
        let quiet: VecDeque<String> = ["[cli] INFO: Finished.".to_owned()].into();
        assert_eq!(last_error(&quiet), None);
        assert_eq!(reason(&quiet, "exit 1"), "INFO: Finished.");
    }

    #[test]
    fn picks_gogs_linux_build_when_there_is_one() {
        let mut game = Owned::new(Store::Gog, "1207658924", "Game".into());
        game.platforms = vec!["Windows".into(), "Linux".into()];
        let job = Job::install(&game, "/home/me/Games".into(), Vec::new()).unwrap();
        assert_eq!(
            (job.app.as_str(), job.platform.as_str()),
            ("1207658924", "linux")
        );
        game.platforms = vec!["Windows".into()];
        assert_eq!(
            Job::install(&game, "/g".into(), Vec::new())
                .unwrap()
                .platform,
            "windows"
        );
    }

    #[test]
    fn gives_each_game_a_folder_of_its_own() {
        assert_eq!(
            folder_name("Bō: Path of the Teal Lotus"),
            "Bō Path of the Teal Lotus"
        );
        assert_eq!(folder_name("../.."), "Game");
        assert_eq!(folder_name(" A/B? "), "AB");
        let dir = std::env::temp_dir().join(format!("zendo-folders-{}", std::process::id()));
        fs::create_dir_all(dir.join("Sugar")).unwrap();
        fs::create_dir_all(dir.join("Sugar (2)")).unwrap();
        assert_eq!(new_folder(&dir, "Sugar"), dir.join("Sugar (3)"));
        assert_eq!(new_folder(&dir, "Salt"), dir.join("Salt"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_where_a_paused_install_goes() {
        let mut game = Owned::new(Store::Epic, "Sugar", "Sugar".into());
        game.platforms = vec!["Windows".into()];
        let mut job = Job::install(&game, "/home/me/Games".into(), Vec::new()).unwrap();
        let json = serde_json::to_string(&job).unwrap();
        assert!(
            !json.contains("paused") && !json.contains("folder"),
            "{json}"
        );
        job.paused = true;
        job.folder = Some("/home/me/Games/Sugar".into());
        let back: Job = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
        assert_eq!(back, job);
    }

    #[test]
    fn deletes_only_a_games_folder() {
        assert!(safe_to_delete(Path::new("/mnt/games/Game")));
        assert!(!safe_to_delete(Path::new("/home/me")));
        assert!(!safe_to_delete(Path::new("/home")));
        assert!(!safe_to_delete(Path::new("Games/Game/x")));
        if let Some(home) = paths::home() {
            assert!(!safe_to_delete(&home));
            assert!(!safe_to_delete(&home.join("Games")));
            assert!(safe_to_delete(&home.join("Games/Game")));
        }
    }
}
