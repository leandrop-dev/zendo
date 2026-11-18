//! Nexus Launcher's games: PC Game Pass titles the Microsoft Store installed,
//! and the titles the account owns (`owned`, for the Store page).
//!
//! The launcher (`domsvoboda/nexus-launcher`, MIT, formerly `xodus-nexus`)
//! keeps a profile in `$XDG_CONFIG_HOME/xodus-nexus` with
//! `launcher-state.json`. A record there is a game the Microsoft Store
//! installed; it is started through the launcher's own
//! `shortcuts/launch-game <id>` script. Zendo reads the records and starts
//! the games, as it does Steam's and Heroic's. Since item 62 it also
//! installs Game Pass games itself, into `<games>/GamePass`, and writes a
//! record for each so the launcher lists it and prepares its runner; the
//! `.zendo-xbox.json` marker in the folder says the install is Zendo's.

use std::collections::HashMap;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;
use serde::Deserialize;

use crate::owned::{Owned, Shop};
use crate::{Details, Game, Launch, Scan, Source, engines, paths};

/// Where the launcher keeps its profile, when it is installed.
#[must_use]
pub fn profile() -> Option<PathBuf> {
    let dir = paths::config_home()?.join("xodus-nexus");
    dir.is_dir().then_some(dir)
}

/// The marker the launcher writes on line 2 of the scripts it makes. A
/// `launch-game` without it is some other file, and is never started.
const MARKER: &str = "# X-Xodus-Nexus-Profile=";

/// `domsvoboda/nexus-launcher`, whose releases carry the Linux `AppImage`.
const REPO: &str = "domsvoboda/nexus-launcher";

/// What the launcher is called in Zendo's words.
const LABEL: &str = "Nexus Launcher";

/// What the file in Zendo's launchers folder is called.
const APPIMAGE: &str = "Nexus.AppImage";

/// The `AppImage` Zendo downloaded, once it is in place.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
    engines::launchers_dir().map(|dir| dir.join(APPIMAGE))
}

/// The `AppImage`, when it is there.
#[must_use]
pub fn installed() -> Option<PathBuf> {
    appimage().filter(|path| path.is_file())
}

/// How many games the launcher's profile lists as ready.
#[must_use]
pub fn count() -> usize {
    profile().map_or(0, |profile| scan(Some(&profile)).games.len())
}

/// The `AppImage` of the latest release, for this machine.
///
/// # Errors
///
/// When GitHub cannot be reached, the release has no `AppImage` for this
/// machine, or it has no checksum to check the download against.
pub fn release() -> anyhow::Result<engines::Release> {
    let (tag, asset) = engines::latest_asset(REPO, LABEL, |name| {
        appimage_for(name, std::env::consts::ARCH)
    })?;
    Ok(engines::Release {
        tag,
        asset,
        service: None,
    })
}

/// The Store's catalogue of product information.
const CATALOGUE: &str = "https://displaycatalog.mp.microsoft.com/v7.0/products";

/// How many product ids one catalogue query asks for.
const BATCH: usize = 20;

/// The market the Store answers for, as `resolve-package` asks by default.
const MARKET: &str = "US";

/// Runs the launcher's backend and returns its answer, from the first `{`:
/// its diagnostics come before it, one line each.
fn backend(args: &[&str]) -> anyhow::Result<String> {
    let cli = cli().context("Nexus Launcher's backend is not installed")?;
    let output = std::process::Command::new(&cli)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .context("could not run Nexus Launcher's backend")?;
    if !output.status.success() {
        anyhow::bail!(
            "Nexus Launcher's backend could not {}: {}",
            args.first().copied().unwrap_or("answer"),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text
        .find('{')
        .context("Nexus Launcher's backend gave no answer")?;
    Ok(text[start..].to_owned())
}

/// The launcher's backend program, at the path its `current.json` names.
fn cli() -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct Current {
        #[serde(rename = "binaryPath")]
        binary: PathBuf,
    }
    let file = profile()?.join("xodus-backends/current.json");
    let bytes = fs::read(file).ok()?;
    let current: Current = serde_json::from_slice(&bytes).ok()?;
    current.binary.is_file().then_some(current.binary)
}

/// Whether Zendo can install and start a Game Pass game: the launcher's
/// backend and its runtime Wine are both there.
#[must_use]
pub fn ready() -> bool {
    cli().is_some() && runtime_wine().is_some()
}

/// The folder the launcher's runtimes live in: its settings say where, else
/// the data folder.
fn runtime_root() -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        settings: Settings,
    }
    #[derive(Default, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Settings {
        #[serde(default)]
        runtime_root: Option<PathBuf>,
    }
    let from_settings = profile()
        .and_then(|profile| fs::read(profile.join("launcher-state.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<State>(&bytes).ok())
        .and_then(|state| state.settings.runtime_root)
        .filter(|root| root.is_dir());
    from_settings.or_else(|| {
        paths::data_home()
            .map(|dir| dir.join("xodus-nexus/runtimes"))
            .filter(|root| root.is_dir())
    })
}

/// The Wine of the launcher's runtime, when the launcher installed one and
/// Zendo can start a game with it.
#[must_use]
pub fn runtime_wine() -> Option<PathBuf> {
    runtime_wine_in(&runtime_root()?)
}

/// The Wine of the installed runtime under `root`: the `nexus-cachyos`
/// build first, then the plain one, then any other in name order. Each
/// variant's
/// `current.json` names its build; the `wine` in it must be there.
fn runtime_wine_in(root: &Path) -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct Current {
        #[serde(rename = "buildId")]
        build_id: String,
    }
    let mut variants: Vec<PathBuf> = fs::read_dir(root.join("variants"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    variants.sort_by_key(|path| {
        let name = path.file_name().and_then(|name| name.to_str());
        match name {
            Some("nexus-cachyos") => (0, path.clone()),
            Some("nexus") => (1, path.clone()),
            _ => (2, path.clone()),
        }
    });
    for variant in variants {
        let Ok(bytes) = fs::read(variant.join("current.json")) else {
            continue;
        };
        let Ok(current) = serde_json::from_slice::<Current>(&bytes) else {
            continue;
        };
        if !one_component(&current.build_id) {
            continue;
        }
        let wine = variant
            .join("releases")
            .join(&current.build_id)
            .join("files/bin/wine");
        if wine.is_file() {
            return Some(wine);
        }
    }
    None
}

/// Whether a name read from a file is one folder and no more.
fn one_component(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(parts.next(), Some(std::path::Component::Normal(_))) && parts.next().is_none()
}

/// The file Zendo writes in a folder it installed, and reads back.
pub const RECORD: &str = ".zendo-xbox.json";

/// What Zendo remembers about a Game Pass game it installed: enough to list
/// it, start it and show where it came from without asking the Store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Marker {
    pub schema: u32,
    /// The Microsoft Store product id.
    pub product: String,
    pub title: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub hero: Option<String>,
}

/// Writes the marker into the game's folder.
///
/// # Errors
///
/// When the folder is gone or the file cannot be written.
pub fn write_marker(folder: &Path, marker: &Marker) -> anyhow::Result<()> {
    let path = folder.join(RECORD);
    let text = serde_json::to_string_pretty(marker).context("the marker could not be written")?;
    // Written whole or not at all: a marker caught half written would make
    // the game Zendo's no longer, for good.
    paths::write_atomic(&path, text.as_bytes())
        .with_context(|| format!("could not write {}", path.display()))
}

/// The marker in a folder, when it wrote one Zendo can read.
#[must_use]
pub fn read_marker(folder: &Path) -> Option<Marker> {
    let bytes = fs::read(folder.join(RECORD)).ok()?;
    let marker: Marker = serde_json::from_slice(&bytes).ok()?;
    (marker.schema == 1).then_some(marker)
}

/// Whether Nexus Launcher is running, by Electron's lock in its profile.
#[must_use]
pub fn running() -> bool {
    profile().is_some_and(|profile| running_in(&profile))
}

fn running_in(profile: &Path) -> bool {
    profile.join("SingletonLock").exists()
}

/// Zendo's record id for a product: stable, so a reinstall keeps the same
/// launcher record and `compatdata` folder. FNV-1a, as 16 hex.
#[must_use]
pub fn record_id(product: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!("zendo:{product}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Where the launcher's state file is, once its profile exists.
fn state_path() -> Option<PathBuf> {
    paths::config_home().map(|dir| dir.join("xodus-nexus/launcher-state.json"))
}

/// The launcher's `compatdata` folder for one record: the prefix and the
/// prepared runner it makes when the game is first played.
#[must_use]
pub fn compatdata(id: &str) -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("xodus-nexus/compatdata").join(id))
}

/// The launcher's state, read as JSON so the fields Zendo does not know
/// survive a write.
fn state_at(path: &Path) -> anyhow::Result<serde_json::Value> {
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_slice(&bytes).context("Nexus Launcher's state was odd")
}

/// Writes the state beside the old one, then renames it over, so a failed
/// write leaves the launcher with its state; the temporary name is the
/// writer's own, as two writes can overlap.
fn write_state(path: &Path, state: &serde_json::Value) -> anyhow::Result<()> {
    let text = serde_json::to_vec_pretty(state).context("the state could not be written")?;
    paths::write_atomic(path, &text).with_context(|| format!("could not put {}", path.display()))
}

/// The account the launcher records for its games, from an existing record
/// or the backend.
fn account_of(state: &serde_json::Value) -> Option<String> {
    let from_game = state["games"]
        .as_array()?
        .iter()
        .find_map(|game| game["licenseAccount"].as_str().map(str::to_owned));
    from_game.or_else(|| {
        let answer: Library = serde_json::from_str(&backend(&["library", "--json"]).ok()?).ok()?;
        answer.account
    })
}

/// A moment as the launcher writes it: `2026-11-17T15:16:09.000Z`.
fn iso_now() -> String {
    let seconds = crate::time::now();
    let (year, month, day) =
        crate::time::civil_from_days(i64::try_from(seconds / 86_400).unwrap_or(i64::MAX));
    let in_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

/// A cover as the inline `data:` URL the launcher keeps.
fn data_url(bytes: &[u8]) -> String {
    use base64::Engine as _;
    let mime = if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else {
        "application/octet-stream"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// The record Nexus Launcher keeps for one of its games, as Zendo builds it
/// for a folder it installed (`docs/reference.md`, "Nexus Launcher").
fn record(
    folder: &Path,
    marker: &Marker,
    id: &str,
    cover: Option<&str>,
    account: Option<&str>,
) -> serde_json::Value {
    let now = iso_now();
    serde_json::json!({
        "id": id,
        "title": marker.title.clone(),
        "path": folder,
        "storeId": marker.product.clone(),
        "publisher": marker.publisher.clone(),
        "installedVersion": marker.version.clone(),
        "packageName": marker.package.clone(),
        "executable": marker.executable.clone(),
        "coverDataUrl": cover,
        "installState": "ready",
        "vcRedistEncrypted": false,
        "addedAt": now,
        "installedAt": now,
        "licenseAccount": account,
        "offerProductId": marker.product.clone(),
        "coverUrl": marker.cover.clone(),
        "heroUrl": marker.hero.clone(),
        "launchTools": {
            "dualSenseCompatibility": false,
            "mangoHud": false,
            "gamescope": false,
            "gamescopeArgs": "",
            "launchOptions": ""
        },
        "locale": "",
        "runtime": "nexus-cachyos",
        "runtimePrefixes": {},
        "runtimePrefixIdentity": {"layout": "game"}
    })
}

/// Registers a game Zendo installed with Nexus Launcher: from now on the
/// launcher lists it, prepares it and plays it. The cover and the account
/// are best effort; the record is not.
///
/// # Errors
///
/// When the launcher is running, its state cannot be read or written, or the
/// game was never run by it (no profile).
pub fn register(folder: &Path, marker: &Marker) -> anyhow::Result<String> {
    if running() {
        anyhow::bail!("close Nexus Launcher first: it must not be running");
    }
    let path = state_path().context("Nexus Launcher has not run on this machine")?;
    let mut state = state_at(&path)?;
    let id = record_id(&marker.product);
    let account = account_of(&state);
    let cover = marker.cover.as_deref().and_then(fetch_cover);
    let record = record(folder, marker, &id, cover.as_deref(), account.as_deref());
    add_record(&mut state, record, &marker.product)?;
    write_state(&path, &state)?;
    Ok(id)
}

/// Adds the record, replacing one for the same id or product.
fn add_record(
    state: &mut serde_json::Value,
    record: serde_json::Value,
    product: &str,
) -> anyhow::Result<()> {
    let games = state["games"]
        .as_array_mut()
        .context("Nexus Launcher's state has no games list")?;
    games.retain(|game| {
        game["id"].as_str() != record["id"].as_str() && game["storeId"].as_str() != Some(product)
    });
    games.push(record);
    Ok(())
}

/// Removes the record with that id, leaving the rest of the state alone.
fn remove_record(state: &mut serde_json::Value, id: &str) -> anyhow::Result<()> {
    let games = state["games"]
        .as_array_mut()
        .context("Nexus Launcher's state has no games list")?;
    games.retain(|game| game["id"].as_str() != Some(id));
    Ok(())
}

/// Removes the record with that id, leaving the launcher's other games and
/// settings as they are.
///
/// # Errors
///
/// When the launcher is running, or its state cannot be read or written.
pub fn unregister(id: &str) -> anyhow::Result<()> {
    if running() {
        anyhow::bail!("close Nexus Launcher first: it must not be running");
    }
    let path = state_path().context("Nexus Launcher has not run on this machine")?;
    let mut state = state_at(&path)?;
    remove_record(&mut state, id)?;
    write_state(&path, &state)
}

/// The bytes of a cover, for the record's inline copy.
fn fetch_cover(url: &str) -> Option<String> {
    let bytes = crate::http::download_agent()
        .get(url)
        .call()
        .ok()?
        .into_body()
        .read_to_vec()
        .ok()?;
    (!bytes.is_empty()).then(|| data_url(&bytes))
}

/// The launcher's backend as a command, with its arguments.
fn command(args: &[&str]) -> anyhow::Result<std::process::Command> {
    let cli = cli().context("Nexus Launcher's backend is not installed")?;
    let mut command = std::process::Command::new(cli);
    command.args(args);
    Ok(command)
}

/// The room left beside a finished package, so a full disk does not have to
/// end an install at the last byte.
const HEADROOM: u64 = 1 << 30;

/// The bytes free on the file system `path` is on, through `df`.
///
/// # Errors
///
/// When `df` is missing or cannot read the file system.
pub fn free_bytes(path: &Path) -> anyhow::Result<u64> {
    let output = std::process::Command::new("df")
        .args(["-B1", "--output=avail"])
        .arg(path)
        .output()
        .context("could not ask df for the free space")?;
    if !output.status.success() {
        anyhow::bail!(
            "df could not read {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| line.trim().parse::<u64>().ok())
        .next_back()
        .context("df gave no free space")
}

/// Whether `bytes` fit where `path` is, with [`HEADROOM`] to spare.
///
/// # Errors
///
/// When [`free_bytes`] cannot tell.
pub fn fits(path: &Path, bytes: u64) -> anyhow::Result<bool> {
    Ok(free_bytes(path)? >= bytes.saturating_add(HEADROOM))
}

/// One progress event of the backend's `streaming` protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamEvent {
    #[serde(default)]
    completed_bytes: u64,
    #[serde(default)]
    total_bytes: u64,
}

/// A `NEXUS_PROGRESS` line's bytes done and total, when the line is one.
fn stream_event(line: &str) -> Option<(u64, u64)> {
    let payload = line.strip_prefix("NEXUS_PROGRESS ")?;
    let event: StreamEvent = serde_json::from_str(payload).ok()?;
    Some((event.completed_bytes, event.total_bytes))
}

/// Downloads and extracts a Store product into `folder` with the backend's
/// `streaming`, reporting the bytes done and the total as it goes. The
/// backend's journal in the folder makes a second run carry on from where
/// the first stopped.
///
/// # Errors
///
/// When the backend is missing, refuses, or cannot be started.
pub fn stream(
    product: &str,
    folder: &Path,
    stop: &crate::downloads::Stop,
    report: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<crate::downloads::Ended> {
    use std::io::{BufRead, BufReader, Read};
    use std::process::Stdio;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::{Duration, Instant};

    fs::create_dir_all(folder).with_context(|| format!("could not make {}", folder.display()))?;
    let mut command = command(&[
        "streaming",
        product,
        &folder.to_string_lossy(),
        "--progress-json",
    ])?;
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command
        .spawn()
        .context("could not start Nexus Launcher's backend")?;
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
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
    }
    drop(sender);
    let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut interrupted: Option<Instant> = None;
    let mut exited: Option<Instant> = None;
    loop {
        match lines.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                if let Some((done, total)) = stream_event(&line) {
                    report(done, total);
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
                    crate::downloads::signal_group(child.id(), "-INT");
                    interrupted = Some(Instant::now());
                }
                Some(at) if at.elapsed() > Duration::from_secs(15) => {
                    crate::downloads::signal_group(child.id(), "-KILL");
                    let _ = child.kill();
                }
                Some(_) => {}
            }
        }
        if exited.is_none() && matches!(child.try_wait(), Ok(Some(_))) {
            exited = Some(Instant::now());
        }
        if exited.is_some_and(|at| at.elapsed() > Duration::from_secs(5)) {
            break;
        }
    }
    let status = child
        .wait()
        .context("Nexus Launcher's backend did not end cleanly")?;
    if stop.asked() {
        return Ok(crate::downloads::Ended::Stopped);
    }
    if !status.success() {
        let why = tail
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| format!("exit {}", status.code().unwrap_or(-1)));
        anyhow::bail!("Nexus Launcher's backend could not install: {why}");
    }
    Ok(crate::downloads::Ended::Done)
}

/// Whether an `AppImage`'s name is the one this machine runs: the release
/// names them `Xodus-Nexus-<version>-x86_64.AppImage` and `-arm64`, and the
/// checksum file beside them is not an `AppImage`.
fn appimage_for(name: &str, arch: &str) -> bool {
    name.ends_with(".AppImage")
        && match arch {
            "x86_64" => name.contains("x86_64"),
            "aarch64" => name.contains("arm64") || name.contains("aarch64"),
            _ => false,
        }
}

/// Downloads the `AppImage`, checks it against its digest and puts it in
/// place, executable, reporting the bytes as it goes; `stop` set ends it.
///
/// # Errors
///
/// When the download fails or is stopped, the checksum does not match, or
/// the file cannot be put in place; the `AppImage` there before stays then.
pub fn install(
    release: &engines::Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    engines::bring(release, APPIMAGE, stop, progress)
}

/// Opens the launcher in its own window: it writes its profile and shortcuts
/// the first time, and there the user installs its runtime and signs in.
/// The caller waits for it, so the profile can be read once it ends.
///
/// # Errors
///
/// When the `AppImage` is not there or cannot be started.
pub fn open() -> anyhow::Result<std::process::Child> {
    let appimage = installed().context("Nexus Launcher is not installed yet")?;
    crate::launch::spawn_logged(std::process::Command::new(appimage), "nexus")
}

/// The games the launcher's profile lists, or one problem.
pub(crate) fn scan(profile: Option<&Path>) -> Scan {
    let mut scan = Scan::default();
    let Some(profile) = profile else {
        return scan;
    };
    let file = profile.join("launcher-state.json");
    let path = file.display().to_string();
    let state: State = match fs::read(&file)
        .map_err(|err| err.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| err.to_string()))
    {
        Ok(state) => state,
        Err(err) => {
            scan.problems.push(format!("{path}: {err}"));
            return scan;
        }
    };
    let Some(shortcut) = script(profile) else {
        scan.problems.push(format!(
            "{}: missing, or not a script the launcher wrote",
            profile.join("shortcuts/launch-game").display()
        ));
        return scan;
    };
    for record in state.games {
        if let Some(game) = game(&record, &shortcut) {
            scan.games.push(game);
        }
    }
    scan
}

/// The script that starts a game by its id, when the launcher wrote it.
fn script(profile: &Path) -> Option<PathBuf> {
    let link = profile.join("shortcuts/launch-game");
    let second = fs::read_to_string(&link).ok()?.lines().nth(1)?.to_owned();
    second.starts_with(MARKER).then_some(link)
}

/// One game, when it is installed and can be started.
fn game(record: &Record, shortcut: &Path) -> Option<Game> {
    let id = record.id.trim();
    if id.is_empty() || record.install_state.as_deref() != Some("ready") {
        return None;
    }
    let path = PathBuf::from(record.path.as_deref()?.trim());
    if !path.is_absolute() {
        return None;
    }
    let title = match record.title.trim() {
        "" => id.to_owned(),
        title => title.to_owned(),
    };
    // The Store's product id is the one the Store page knows the game by, so
    // the library's id is its own; the launcher's id is what starts it.
    let library_id = match record.store_id.as_deref().map(str::trim) {
        Some(product) if !product.is_empty() => product.to_owned(),
        _ => id.to_owned(),
    };
    let mut game = Game::new(
        format!("xbox:{library_id}"),
        title,
        Source::Xbox,
        Launch::Command {
            program: shortcut.display().to_string(),
            args: vec![id.to_owned()],
            dir: None,
        },
    );
    game.details = Details {
        developer: record.publisher.clone(),
        platform: Some("windows".to_owned()),
        install_path: Some(path),
        ..Details::default()
    };
    // The launcher carries the cover inside its record, as a `data:` URL the
    // covers fetch knows to read; the wide art is the Store's own address.
    game.cover_url = record.cover_data_url.clone().filter(|url| !url.is_empty());
    game.hero_url = record
        .hero_url
        .clone()
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
    Some(game)
}

/// `launcher-state.json`, as much of it as Zendo reads.
#[derive(Debug, Deserialize)]
struct State {
    #[serde(default)]
    games: Vec<Record>,
}

/// One game in the launcher's state.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    path: Option<String>,
    /// The Store's product id, which the account's entitlements are keyed by.
    #[serde(default, rename = "storeId")]
    store_id: Option<String>,
    #[serde(default)]
    install_state: Option<String>,
    #[serde(default)]
    cover_data_url: Option<String>,
    #[serde(default)]
    hero_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with `launcher-state.json` and the launcher's script.
    fn profile(name: &str, state: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zendo-xodus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("shortcuts")).expect("the test's folders");
        fs::write(dir.join("launcher-state.json"), state).expect("the state file");
        fs::write(
            dir.join("shortcuts/launch-game"),
            format!(
                "#!/bin/sh\n{MARKER}{}\nexec xodus-cli launch \"$@\"\n",
                dir.display()
            ),
        )
        .expect("the launcher's script");
        dir
    }

    fn by_id<'a>(scan: &'a Scan, id: &str) -> &'a Game {
        scan.games
            .iter()
            .find(|game| game.id == id)
            .expect("the game")
    }

    /// A runtimes folder with the variants given, each naming its build.
    fn runtimes(name: &str, variants: &[(&str, &str, bool)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zendo-xodus-runtime-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (family, build, wine) in variants {
            let base = dir.join("variants").join(family);
            fs::create_dir_all(&base).unwrap();
            fs::write(
                base.join("current.json"),
                format!(r#"{{"schema":1,"family":"{family}","buildId":"{build}"}}"#),
            )
            .unwrap();
            if *wine {
                let bin = base.join("releases").join(build).join("files/bin");
                fs::create_dir_all(&bin).unwrap();
                fs::write(bin.join("wine"), "").unwrap();
            }
        }
        dir
    }

    #[test]
    fn takes_the_runtimes_wine_cachyos_first() {
        let dir = runtimes(
            "both",
            &[("nexus", "aaa", true), ("nexus-cachyos", "bbb", true)],
        );
        let wine = runtime_wine_in(&dir).unwrap();
        assert!(wine.ends_with("nexus-cachyos/releases/bbb/files/bin/wine"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn takes_the_plain_runtime_when_it_is_the_only_one() {
        let dir = runtimes("plain", &[("nexus", "aaa", true)]);
        let wine = runtime_wine_in(&dir).unwrap();
        assert!(wine.ends_with("nexus/releases/aaa/files/bin/wine"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_a_build_id_that_is_a_path_and_a_missing_wine() {
        let dir = runtimes("paths", &[("nexus", "..", true)]);
        assert_eq!(runtime_wine_in(&dir), None, "a build id is one folder");
        fs::remove_dir_all(&dir).unwrap();
        let dir = runtimes("gone", &[("nexus", "aaa", false)]);
        assert_eq!(runtime_wine_in(&dir), None, "the wine must be there");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_the_streamings_progress_lines_and_ignores_the_rest() {
        // As the backend wrote it for Retro Classics on 2026-11-17.
        let line = r#"NEXUS_PROGRESS {"schema":1,"sequence":4,"event":"progress","phase":"downloading","completedBytes":1048576,"totalBytes":84879403,"bytesPerSecond":2097152,"canPause":true,"paused":false}"#;
        assert_eq!(stream_event(line), Some((1_048_576, 84_879_403)));
        assert_eq!(stream_event("Preparing download"), None);
        assert_eq!(stream_event("NEXUS_PROGRESS not json"), None);
    }

    #[test]
    fn says_whether_a_package_fits() {
        let dir = std::env::temp_dir();
        let Ok(free) = free_bytes(&dir) else {
            return; // no df on this machine
        };
        assert!(free > 0);
        assert!(fits(&dir, 1).unwrap());
        assert!(!fits(&dir, u64::MAX).unwrap());
    }

    fn marker() -> Marker {
        Marker {
            schema: 1,
            product: "9N98Z825TNFW".into(),
            title: "Sugar Rush".into(),
            publisher: Some("Microsoft Studios".into()),
            package: Some("Microsoft.BadgerWin10".into()),
            version: Some("1.18.19068.0".into()),
            executable: Some("Sugar.exe".into()),
            bytes: Some(1024),
            cover: Some("https://example.test/cover.png".into()),
            hero: None,
        }
    }

    #[test]
    fn writes_and_reads_its_marker() {
        let dir = std::env::temp_dir().join(format!("zendo-xodus-marker-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let folder = dir.join("GamePass/Sugar Rush");
        fs::create_dir_all(&folder).unwrap();
        write_marker(&folder, &marker()).unwrap();
        let read = read_marker(&folder).unwrap();
        assert_eq!(read.product, "9N98Z825TNFW");
        assert_eq!(read.version.as_deref(), Some("1.18.19068.0"));
        assert_eq!(read.bytes, Some(1024));
        // A marker from a later schema is not read.
        let mut later = marker();
        later.schema = 2;
        write_marker(&folder, &later).unwrap();
        assert!(read_marker(&folder).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_a_record_after_the_product() {
        let id = record_id("9N98Z825TNFW");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, record_id("9N98Z825TNFW"), "stable across calls");
        assert_ne!(id, record_id("9MTVJ3HHTQGS"));
    }

    #[test]
    fn builds_the_launchers_record() {
        let value = record(
            Path::new("/home/me/Games/GamePass/Sugar Rush"),
            &marker(),
            "0123456789abcdef",
            Some("data:image/jpeg;base64,AA=="),
            Some("me@example.test"),
        );
        assert_eq!(value["id"], "0123456789abcdef");
        assert_eq!(value["storeId"], "9N98Z825TNFW");
        assert_eq!(value["installState"], "ready");
        assert_eq!(value["runtime"], "nexus-cachyos");
        assert_eq!(value["runtimePrefixIdentity"]["layout"], "game");
        assert_eq!(value["launchTools"]["mangoHud"], false);
        assert_eq!(value["licenseAccount"], "me@example.test");
        assert_eq!(value["coverDataUrl"], "data:image/jpeg;base64,AA==");
        assert!(
            value["addedAt"]
                .as_str()
                .is_some_and(|at| at.ends_with('Z') && at.len() == 24),
            "{value}"
        );
    }

    #[test]
    fn keeps_one_record_for_a_product_and_removes_it() {
        let mut state = serde_json::json!({
            "games": [{"id": "other", "storeId": "OTHER"}],
            "downloads": [{"id": "a-job"}],
            "settings": {"closeToTray": false}
        });
        let first = record(Path::new("/g/One"), &marker(), "aaa", None, None);
        let second = record(Path::new("/g/Two"), &marker(), "aaa", None, None);
        add_record(&mut state, first, "9N98Z825TNFW").unwrap();
        add_record(&mut state, second, "9N98Z825TNFW").unwrap();
        assert_eq!(state["games"].as_array().unwrap().len(), 2, "one per game");
        assert_eq!(state["downloads"][0]["id"], "a-job", "the rest survives");
        assert_eq!(state["settings"]["closeToTray"], false);
        remove_record(&mut state, "aaa").unwrap();
        assert_eq!(state["games"].as_array().unwrap().len(), 1);
        assert_eq!(state["games"][0]["id"], "other");
    }

    #[test]
    fn sees_the_launchers_lock_and_makes_data_urls() {
        let dir = std::env::temp_dir().join(format!("zendo-xodus-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(!running_in(&dir));
        fs::write(dir.join("SingletonLock"), "").unwrap();
        assert!(running_in(&dir));
        fs::remove_dir_all(&dir).unwrap();
        assert!(data_url(b"\xff\xd8\xffx").starts_with("data:image/jpeg;base64,"));
        assert!(data_url(b"\x89PNG\r\n\x1a\n").starts_with("data:image/png;base64,"));
    }

    #[test]
    fn reads_the_games_that_are_ready_and_startable() {
        let dir = profile(
            "ready",
            r#"{
                "games": [
                    {
                        "id": "0123456789abcdef",
                        "title": "Halo Infinite",
                        "publisher": "Xbox Game Studios",
                        "storeId": "9NNX1VVR9WKB",
                        "path": "/home/me/Games/Halo Infinite",
                        "installState": "ready",
                        "executable": "HaloInfinite.exe",
                        "coverDataUrl": "data:image/png;base64,iVBORw0KGgo=",
                        "heroUrl": "https://store-images.s-microsoft.com/hero.jpg"
                    },
                    { "id": "notready", "title": "Installing", "path": "/home/me/Games/X", "installState": "installing" },
                    { "id": "relative", "title": "Relative", "path": "Games/X", "installState": "ready" },
                    { "id": "nopath", "title": "No path", "installState": "ready" }
                ]
            }"#,
        );
        let scan = scan(Some(&dir));
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        assert_eq!(scan.games.len(), 1, "only the ready, absolute game");
        // The Store's product id names it, so the Store page knows it; the
        // launcher's id is what starts it.
        let game = by_id(&scan, "xbox:9NNX1VVR9WKB");
        assert_eq!(game.title, "Halo Infinite");
        assert_eq!(game.source, Source::Xbox);
        assert_eq!(
            game.launch,
            Launch::Command {
                program: dir.join("shortcuts/launch-game").display().to_string(),
                args: vec!["0123456789abcdef".to_owned()],
                dir: None,
            }
        );
        assert_eq!(
            game.details.install_path.as_deref(),
            Some(Path::new("/home/me/Games/Halo Infinite"))
        );
        assert_eq!(game.details.developer.as_deref(), Some("Xbox Game Studios"));
        assert_eq!(
            game.cover_url.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_without_its_store_id_takes_the_launchers() {
        let dir = profile(
            "nostore",
            r#"{"games":[{"id":"aaa","title":"A","path":"/games/a","installState":"ready"}]}"#,
        );
        let scan = scan(Some(&dir));
        assert_eq!(by_id(&scan, "xbox:aaa").title, "A");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_title_that_is_empty_takes_the_id() {
        let dir = profile(
            "untitled",
            r#"{"games":[{"id":"aaa","title":"","path":"/games/a","installState":"ready"}]}"#,
        );
        let scan = scan(Some(&dir));
        assert_eq!(by_id(&scan, "xbox:aaa").title, "aaa");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_that_is_not_the_launchers_gives_a_problem_and_no_games() {
        let dir = profile(
            "foreign",
            r#"{"games":[{"id":"aaa","title":"A","path":"/games/a","installState":"ready"}]}"#,
        );
        fs::write(dir.join("shortcuts/launch-game"), "#!/bin/sh\nrm -rf /\n")
            .expect("another script");
        let scan = scan(Some(&dir));
        assert!(scan.games.is_empty(), "a foreign script is not started");
        assert_eq!(scan.problems.len(), 1);
        assert!(
            scan.problems[0].contains("launch-game"),
            "{:?}",
            scan.problems
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn takes_the_appimage_for_this_machine() {
        let name = "Xodus-Nexus-0.5.0-x86_64.AppImage";
        assert!(super::appimage_for(name, "x86_64"));
        assert!(!super::appimage_for(name, "aarch64"));
        assert!(super::appimage_for(
            "Xodus-Nexus-0.5.0-arm64.AppImage",
            "aarch64"
        ));
        assert!(
            super::appimage_for("Xodus-Nexus-0.5.0-aarch64.AppImage", "aarch64"),
            "either name the release gives it"
        );
        assert!(
            !super::appimage_for("Xodus-Nexus-0.5.0-x86_64.AppImage.sha512sum", "x86_64"),
            "the checksum file is not the AppImage"
        );
        assert!(!super::appimage_for(name, "riscv64"), "no AppImage here");
    }

    #[test]
    fn reads_the_entitlements_then_the_store_catalogue() {
        // The backend writes its diagnostics before the answer.
        let answer = "NEXUS_SESSION_DIAGNOSTIC_V1 ticket-response-received\n\
                      NEXUS_LIBRARY_DIAGNOSTIC_V1 complete 0 0\n\
                      {\"account\":\"me@example.com\",\"entitlements\":[\
                      {\"productId\":\"9AAA\",\"status\":\"Active\"},\
                      {\"productId\":\"9BBB\",\"status\":\"Active\"},\
                      {\"productId\":\"9CCC\",\"status\":\"Revoked\"}]}";
        let json = &answer[answer.find('{').unwrap()..];
        let library: Library = serde_json::from_str(json).unwrap();
        let ids: Vec<&str> = library
            .entitlements
            .iter()
            .filter(|entitlement| entitlement.status == "Active")
            .map(|entitlement| entitlement.product_id.as_str())
            .collect();
        assert_eq!(ids, ["9AAA", "9BBB"], "a revoked one is not owned");

        let catalogue = Catalogue {
            products: vec![
                Product {
                    id: "9AAA".into(),
                    schema: "ProductGame;1".into(),
                    localized: vec![Localized {
                        title: "Halo Infinite".into(),
                        short_title: None,
                        developer: Some("343 Industries".into()),
                        publisher: Some("Xbox Game Studios".into()),
                        short_description: Some("Spartans. ".into()),
                        description: None,
                        images: vec![
                            Art {
                                purpose: "Poster".into(),
                                uri: "//img/p".into(),
                            },
                            Art {
                                purpose: "SuperHeroArt".into(),
                                uri: "//img/h".into(),
                            },
                        ],
                    }],
                },
                Product {
                    id: "9BBB".into(),
                    schema: "ProductAddOn;3".into(),
                    localized: vec![Localized {
                        title: "Some Skins".into(),
                        short_title: None,
                        developer: None,
                        publisher: None,
                        short_description: None,
                        description: None,
                        images: vec![],
                    }],
                },
            ],
        };
        let games: Vec<Owned> = catalogue.products.iter().filter_map(owned_game).collect();
        assert_eq!(games.len(), 1, "an add-on is not a game");
        let game = &games[0];
        assert_eq!(game.id, "xbox:9AAA");
        assert_eq!(game.title, "Halo Infinite");
        assert_eq!(game.developer.as_deref(), Some("343 Industries"));
        assert_eq!(game.publisher.as_deref(), Some("Xbox Game Studios"));
        assert_eq!(game.summary.as_deref(), Some("Spartans."));
        assert_eq!(game.platforms, ["Windows"]);
        assert_eq!(game.cover_url.as_deref(), Some("https://img/p?w=600"));
        assert_eq!(game.hero_url.as_deref(), Some("https://img/h?w=1920"));
    }

    #[test]
    fn reads_a_title_id_and_orders_achievements() {
        let dir = std::env::temp_dir().join(format!(
            "zendo-xodus-title-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("MicrosoftGame.config"),
            "<?xml version=\"1.0\"?>\n<Game><TitleId>61B215AE</TitleId></Game>\n",
        )
        .unwrap();
        assert_eq!(title_id(&dir), Some(1_639_060_910));
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(title_id(Path::new("/nowhere")), None, "no config, no title");

        let answer: super::Achieved = serde_json::from_str(
            r#"{"account":"me","items":[
                 {"name":"Broken","description":"Defeat The Unbreakable.","secret":false,"revoked":false,"unlockedAt":null},
                 {"name":"With Light Comes Hope","description":"Destroy the Night Beacon Base.","secret":false,"revoked":false,"unlockedAt":"2026-11-16T20:00:00.000Z"},
                 {"name":"Secret","description":"?","secret":true,"revoked":false,"unlockedAt":null},
                 {"name":"Gone","description":"","secret":false,"revoked":true,"unlockedAt":null}]}"#,
        )
        .unwrap();
        let mut list: Vec<crate::achievements::Achievement> = answer
            .items
            .into_iter()
            .filter(|item| !item.revoked)
            .map(|item| crate::achievements::Achievement {
                name: item.name,
                description: item.description,
                hidden: item.secret,
                unlocked: item.unlocked_at.as_deref().and_then(crate::time::parse_utc),
                rarity: None,
                icon: None,
            })
            .collect();
        list.sort_by(|one, other| match (one.unlocked, other.unlocked) {
            (Some(one), Some(other)) => other.cmp(&one),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => one.name.cmp(&other.name),
        });
        let names: Vec<&str> = list.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(
            names,
            ["With Light Comes Hope", "Broken", "Secret"],
            "unlocked first, then the locked by name; a revoked one left out"
        );
        assert_eq!(
            list[0].unlocked,
            Some(1_794_859_200),
            "2026-11-16T20:00:00Z"
        );
        assert!(list[2].hidden, "a secret one, kept back until unlocked");
    }

    #[test]
    fn picks_the_desktop_package_and_says_whether_it_can_install() {
        let resolved: super::Resolved = serde_json::from_str(
            r#"{"choices":[
                 {"contentId":"a","downloadBytes":5506641920,"kind":"Game","packageFormat":"XVC","platform":"Windows.Xbox","supported":false},
                 {"contentId":"b","downloadBytes":5399841994,"kind":"Game","packageFormat":"EAPPX","platform":"Windows.Desktop","supported":false}],
                "market":"US","rootProductId":"9NJRX71M5X9P","schema":1,"status":"unavailable"}"#,
        )
        .unwrap();
        let picked = resolved
            .choices
            .iter()
            .find(|choice| choice.platform == "Windows.Desktop")
            .expect("the desktop build");
        assert_eq!(picked.download_bytes, Some(5_399_841_994), "the PC's build");
        assert_eq!(picked.package_format, "EAPPX");
        assert!(
            !picked.supported || resolved.status == "unavailable",
            "Cuphead is not available on a PC"
        );

        let ready: super::Resolved = serde_json::from_str(
            r#"{"choices":[{"downloadBytes":2087256064,"kind":"Game","packageFormat":"MSIXVC","platform":"Windows.Desktop","supported":true}],"status":"choice"}"#,
        )
        .unwrap();
        assert!(ready.choices[0].supported && ready.status != "unavailable");
        assert_eq!(ready.choices[0].download_bytes, Some(2_087_256_064));
    }

    #[test]
    fn a_title_that_is_empty_takes_the_short_one() {
        let product = Product {
            id: "9AAA".into(),
            schema: "ProductGame;1".into(),
            localized: vec![Localized {
                title: " ".into(),
                short_title: Some("Retro Classics".into()),
                developer: None,
                publisher: None,
                short_description: None,
                description: None,
                images: vec![],
            }],
        };
        assert_eq!(owned_game(&product).unwrap().title, "Retro Classics");
    }

    #[test]
    fn no_profile_is_no_game_and_no_problem() {
        let scan = scan(None);
        assert_eq!(scan.games, [] as [Game; 0]);
        assert_eq!(scan.problems, [] as [std::string::String; 0]);
    }
}

/// The Game Pass games the account owns: the launcher's backend lists its
/// entitlements, and the Store's catalogue names them and gives their art.
///
/// # Errors
///
/// When the backend or the Store cannot be asked, or answer oddly.
pub fn owned() -> anyhow::Result<Vec<Owned>> {
    let ids = entitlements()?;
    let products = catalogue(&ids)?;
    crate::owned::keep(Shop::Xbox, products.iter().filter_map(owned_game).collect())
}

/// The product ids the account holds, from the backend's `library --json`.
fn entitlements() -> anyhow::Result<Vec<String>> {
    let answer: Library = serde_json::from_str(&backend(&["library", "--json"])?)
        .context("Nexus Launcher's backend's list was odd")?;
    Ok(answer
        .entitlements
        .iter()
        .filter(|entitlement| entitlement.status == "Active")
        .map(|entitlement| entitlement.product_id.clone())
        .collect())
}

/// The catalogue's products for those ids, a batch at a time.
fn catalogue(ids: &[String]) -> anyhow::Result<Vec<Product>> {
    let agent = crate::http::download_agent();
    let mut products = Vec::new();
    for batch in ids.chunks(BATCH) {
        let url = format!(
            "{CATALOGUE}?bigIds={}&market={MARKET}&languages=en-US&fieldsTemplate=Details",
            batch.join(",")
        );
        let body = agent
            .get(&url)
            .call()
            .context("could not reach the Microsoft Store's catalogue")?
            .into_body()
            .read_to_string()
            .context("the Store's catalogue answer was cut short")?;
        let answer: Catalogue =
            serde_json::from_str(&body).context("the Store's catalogue answer was odd")?;
        products.extend(answer.products);
    }
    Ok(products)
}

/// One owned game, when the product is a game and not an add-on.
fn owned_game(product: &Product) -> Option<Owned> {
    if !product.schema.starts_with("ProductGame") {
        return None;
    }
    let local = product.localized.first()?;
    let title = match local.title.trim() {
        "" => local.short_title.as_deref()?.trim().to_owned(),
        title => title.to_owned(),
    };
    if title.is_empty() {
        return None;
    }
    let mut game = Owned::new(Shop::Xbox, &product.id, title);
    game.developer.clone_from(&local.developer);
    game.publisher.clone_from(&local.publisher);
    game.summary = local
        .short_description
        .clone()
        .or_else(|| local.description.clone())
        .map(|summary| summary.trim().to_owned())
        .filter(|summary| !summary.is_empty());
    game.platforms = vec!["Windows".to_owned()];
    game.cover_url = local.image("Poster", 600);
    game.hero_url = local
        .image("SuperHeroArt", 1920)
        .or_else(|| local.image("TitledHeroArt", 1920));
    Some(game)
}

/// What the Store would install for a product on this machine: whether it
/// can at all, and how many bytes it would download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Whether this PC can install it.
    pub supported: bool,
    /// The Store's own word for the product: `choice` when it has a package,
    /// `unavailable` when it has none for this machine.
    pub status: String,
    /// The bytes of the build picked, when the Store says.
    pub download_bytes: Option<u64>,
    /// The build picked, for the Technical card: `EAPPX`, `MSIXVC`, `XVC`.
    pub format: Option<String>,
}

/// What the Store would install for a product, from the backend's
/// `resolve-package`: this asks the Store, not the launcher, so it needs
/// nothing installed but the backend.
///
/// # Errors
///
/// When the backend is not installed, cannot be run, or answers oddly.
pub fn package(product: &str) -> anyhow::Result<Package> {
    let answer: Resolved = serde_json::from_str(&backend(&[
        "resolve-package",
        product,
        "--json",
        "--market",
        MARKET,
    ])?)
    .context("Nexus Launcher's backend's package was odd")?;
    // The desktop build is the one a PC runs; the Xbox one is for a console.
    let picked = answer
        .choices
        .iter()
        .find(|choice| choice.platform == "Windows.Desktop")
        .or_else(|| answer.choices.first());
    Ok(Package {
        supported: picked.is_some_and(|choice| choice.supported) && answer.status != "unavailable",
        status: answer.status,
        download_bytes: picked.and_then(|choice| choice.download_bytes),
        format: picked.map(|choice| choice.package_format.clone()),
    })
}

/// The Xbox title id the backend wants, in decimal: the `<TitleId>` in the
/// `MicrosoftGame.config` beside the game's executable, which is hex.
#[must_use]
pub fn title_id(install: &Path) -> Option<u64> {
    let text = fs::read_to_string(install.join("MicrosoftGame.config")).ok()?;
    let start = text.find("<TitleId>")? + "<TitleId>".len();
    let end = text[start..].find("</TitleId>")? + start;
    u64::from_str_radix(text[start..end].trim(), 16).ok()
}

/// The account's achievements for a title, as the backend gives them, their
/// pictures downloaded beside the other achievements': unlocked first,
/// newest first, then the locked by name.
///
/// # Errors
///
/// When the backend is not installed, cannot be run, or answers oddly.
pub fn achievements(title: u64) -> anyhow::Result<Vec<crate::achievements::Achievement>> {
    let answer: Achieved =
        serde_json::from_str(&backend(&["achievements", &title.to_string(), "--json"])?)
            .context("Nexus Launcher's backend's achievements were odd")?;
    let icons = download_icons(&answer);
    let mut list: Vec<crate::achievements::Achievement> = answer
        .items
        .into_iter()
        .filter(|item| !item.revoked)
        .map(|item| crate::achievements::Achievement {
            name: item.name,
            description: item.description,
            hidden: item.secret,
            unlocked: item.unlocked_at.as_deref().and_then(crate::time::parse_utc),
            rarity: None,
            icon: item
                .icon_url
                .as_ref()
                .and_then(|url| icons.get(url).cloned().flatten()),
        })
        .collect();
    list.sort_by(|one, other| match (one.unlocked, other.unlocked) {
        (Some(one), Some(other)) => other.cmp(&one),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => one.name.cmp(&other.name),
    });
    Ok(list)
}

/// The pictures the achievements name, each address once, downloaded into
/// the achievements' folder; one that cannot be downloaded is left out.
fn download_icons(answer: &Achieved) -> HashMap<String, Option<PathBuf>> {
    let mut urls: Vec<&str> = answer
        .items
        .iter()
        .filter_map(|item| item.icon_url.as_deref())
        .collect();
    urls.sort_unstable();
    urls.dedup();
    let mut icons: HashMap<String, Option<PathBuf>> = HashMap::new();
    let Some(dir) = crate::achievements::icons_dir().filter(|dir| fs::create_dir_all(dir).is_ok())
    else {
        return icons;
    };
    for url in urls {
        let file = crate::http::cached_image(&dir, url);
        icons.insert(url.to_owned(), file);
    }
    icons
}

/// The backend's answer about a title's achievements.
#[derive(Deserialize)]
struct Achieved {
    #[serde(default)]
    items: Vec<AchievedItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AchievedItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    revoked: bool,
    #[serde(default)]
    unlocked_at: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
}

/// The backend's answer about a product's packages.
#[derive(Deserialize)]
struct Resolved {
    #[serde(default)]
    status: String,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Choice {
    #[serde(default)]
    platform: String,
    #[serde(default)]
    package_format: String,
    #[serde(default)]
    download_bytes: Option<u64>,
    #[serde(default)]
    supported: bool,
}

/// The backend's answer about the account's entitlements.
#[derive(Deserialize)]
struct Library {
    #[serde(default)]
    account: Option<String>,
    #[serde(default)]
    entitlements: Vec<Entitlement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entitlement {
    product_id: String,
    #[serde(default)]
    status: String,
}

/// The catalogue's answer.
#[derive(Deserialize)]
struct Catalogue {
    #[serde(rename = "Products", default)]
    products: Vec<Product>,
}

/// One product of the catalogue.
#[derive(Deserialize)]
struct Product {
    #[serde(rename = "ProductId")]
    id: String,
    /// `ProductGame;1` for a game, `ProductAddOn;3` for an add-on.
    #[serde(rename = "ProductBSchema", default)]
    schema: String,
    #[serde(rename = "LocalizedProperties", default)]
    localized: Vec<Localized>,
}

#[derive(Deserialize)]
struct Localized {
    #[serde(rename = "ProductTitle", default)]
    title: String,
    #[serde(rename = "ShortTitle", default)]
    short_title: Option<String>,
    #[serde(rename = "DeveloperName", default)]
    developer: Option<String>,
    #[serde(rename = "PublisherName", default)]
    publisher: Option<String>,
    #[serde(rename = "ShortDescription", default)]
    short_description: Option<String>,
    #[serde(rename = "ProductDescription", default)]
    description: Option<String>,
    #[serde(rename = "Images", default)]
    images: Vec<Art>,
}

impl Localized {
    /// The address of one image, at a width, as the Store serves it; the
    /// catalogue gives it without a scheme.
    fn image(&self, purpose: &str, width: u32) -> Option<String> {
        let art = self.images.iter().find(|art| art.purpose == purpose)?;
        if art.uri.is_empty() {
            return None;
        }
        let separator = if art.uri.contains('?') { '&' } else { '?' };
        Some(format!("https:{}{separator}w={width}", art.uri))
    }
}

#[derive(Deserialize)]
struct Art {
    #[serde(rename = "ImagePurpose", default)]
    purpose: String,
    #[serde(rename = "Uri", default)]
    uri: String,
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_the_real_latest_release() {
        let release = super::release().unwrap();
        println!(
            "{} {} {} {}",
            release.tag, release.asset.name, release.asset.size, release.asset.sha256
        );
        assert!(super::appimage_for(
            &release.asset.name,
            std::env::consts::ARCH
        ));
        assert_eq!(release.asset.sha256.len(), 64, "a digest to check against");
    }

    /// Asks the real backend for Minecraft Legends' achievements; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn reads_real_achievements() {
        let list = super::achievements(1_639_060_910).unwrap();
        println!("{} achievements", list.len());
        for one in list.iter().take(3) {
            println!(
                "{} | {} | unlocked {:?} | hidden {}",
                one.name, one.description, one.unlocked, one.hidden
            );
        }
        assert!(!list.is_empty(), "the title has achievements");
    }

    /// Asks the real backend what two products would install; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn resolves_real_packages() {
        for product in ["9NBLGGH2JHXJ", "9NJRX71M5X9P"] {
            let package = super::package(product).unwrap();
            println!(
                "{product} supported={} status={} bytes={:?} format={:?}",
                package.supported, package.status, package.download_bytes, package.format
            );
            assert_ne!(package.status, "");
            assert!(
                !package.supported || package.download_bytes.is_some(),
                "an installable build has a size"
            );
        }
    }

    /// Downloads the real `AppImage` into a throwaway home; run with
    /// `XDG_DATA_HOME=.../zendo-xodus-live/data XDG_CACHE_HOME=.../zendo-xodus-live/cache
    /// cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and writes to a throwaway home"]
    fn installs_the_real_launcher() {
        let data = std::env::var("XDG_DATA_HOME").unwrap_or_default();
        assert!(
            data.contains("zendo-xodus-live"),
            "set XDG_DATA_HOME and XDG_CACHE_HOME to a scratch folder named zendo-xodus-live"
        );
        let release = super::release().unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let at = super::install(&release, &stop, &mut |_, _| {}).unwrap();
        assert_eq!(super::installed(), Some(at.clone()));
        let bytes = std::fs::read(&at).unwrap();
        assert!(bytes.starts_with(b"\x7fELF"), "an AppImage is an ELF file");
        let mode =
            std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&at).unwrap().permissions());
        assert_eq!(mode & 0o111, 0o111, "it can be run");
        println!(
            "{} {} -> {} bytes",
            release.tag,
            release.asset.name,
            bytes.len()
        );
    }
    /// Asks the real backend and the Store for the account's games; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn lists_the_owned_games() {
        let games = super::owned().unwrap();
        println!("{} games", games.len());
        for game in games.iter().take(5) {
            println!(
                "{} | {} | {} | cover {}",
                game.id,
                game.title,
                game.publisher.as_deref().unwrap_or(""),
                game.cover_url.is_some()
            );
        }
        assert!(!games.is_empty(), "the account owns a game");
        assert!(games.iter().all(|game| game.id.starts_with("xbox:")));
        assert!(games.iter().all(|game| !game.title.is_empty()));
        assert!(
            games.iter().any(|game| game.cover_url.is_some()),
            "art found"
        );
    }
}
