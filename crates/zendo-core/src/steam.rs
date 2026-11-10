//! Steam: the library folders in `libraryfolders.vdf` and the installed
//! apps in each folder's `appmanifest_*.acf`. Covers come from Steam's own
//! cache in `appcache/librarycache`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use keyvalues_parser::{Obj, Vdf};

use crate::{Details, Game, Launch, Played, Scan, Size, Source, paths};

/// `StateFlags` bit for an app whose files are all on disk.
const FULLY_INSTALLED: u32 = 4;

/// Installed apps that are tools Steam uses to run games, not games. Steam
/// marks them only in the binary `appinfo.vdf`, so they are told by name.
const TOOL_PREFIXES: [&str; 3] = [
    "Proton",
    "Steam Linux Runtime",
    "Steamworks Common Redistributables",
];

/// Where Steam may be installed: native, the `~/.steam` link, Flatpak.
#[must_use]
pub fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(data) = paths::data_home() {
        roots.push(data.join("Steam"));
    }
    if let Some(home) = paths::home() {
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    roots
}

pub(crate) fn scan(roots: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    let mut seen_roots = HashSet::new();
    let mut seen_folders = HashSet::new();
    for root in roots {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        if !root.join("steamapps").is_dir() || !seen_roots.insert(root.clone()) {
            continue;
        }
        let played = play_records(&root, &mut scan.problems);
        for folder in library_folders(&root, &mut scan.problems) {
            let folder = folder.canonicalize().unwrap_or(folder);
            if seen_folders.insert(folder.clone()) {
                scan_folder(&root, &folder.join("steamapps"), &played, &mut scan);
            }
        }
    }
    scan
}

/// Where Steam installed an app, a tool included, when all its files are
/// there: `steamapps/common/<installdir>` in the library its manifest is in.
pub(crate) fn app_folder(roots: &[PathBuf], app: &str) -> Option<PathBuf> {
    let mut seen = HashSet::new();
    for root in roots {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        if !root.join("steamapps").is_dir() {
            continue;
        }
        for folder in library_folders(&root, &mut Vec::new()) {
            let steamapps = folder.join("steamapps");
            if !seen.insert(steamapps.clone()) {
                continue;
            }
            let manifest = steamapps.join(format!("appmanifest_{app}.acf"));
            let Some(dir) = fs::read_to_string(&manifest)
                .ok()
                .and_then(|text| installed_dir(&text))
            else {
                continue;
            };
            let path = steamapps.join("common").join(dir);
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    None
}

/// `StateFlags` bits for an app Steam is working on: an update or install
/// running, paused, started. "Update required" (2) alone is left out: an
/// installed game with automatic updates off keeps it until it is played.
const UPDATE_RUNNING: u32 = 256;
const UPDATE_PAUSED: u32 = 512;
const UPDATE_STARTED: u32 = 1024;

/// How far Steam is with an app, from its manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Queued, or getting ready to download.
    Waiting,
    Downloading {
        done: u64,
        total: u64,
    },
    /// Downloaded; Steam is writing the files into place.
    Installing {
        done: u64,
        total: u64,
    },
    Paused {
        done: u64,
        total: u64,
    },
    Installed,
}

/// An app Steam knows on this computer, and how far it is with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub app: String,
    pub name: String,
    pub progress: Progress,
}

/// Every app with a manifest in Steam's libraries: the installed ones, and
/// those Steam is downloading, installing or updating. Steam writes the
/// manifest as soon as an install starts.
#[must_use]
pub fn states() -> Vec<AppState> {
    let mut states = Vec::new();
    let mut seen = HashSet::new();
    for root in roots() {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        if !root.join("steamapps").is_dir() {
            continue;
        }
        for folder in library_folders(&root, &mut Vec::new()) {
            let steamapps = folder.join("steamapps");
            if !seen.insert(steamapps.clone()) {
                continue;
            }
            let Ok(entries) = fs::read_dir(&steamapps) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                if let Some(state) = fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|text| parse_state(&text, &steamapps.join("downloading")))
                {
                    states.push(state);
                }
            }
        }
    }
    states
}

/// An app's state from its manifest. Steam writes `BytesDownloaded` only
/// at the end of a download, so while one runs, how far it is comes from
/// the files Steam has written into `downloading/<app>/`, out of
/// `BytesToStage` (Steam did not preallocate them in the downloads tried).
fn parse_state(text: &str, downloading: &Path) -> Option<AppState> {
    let vdf = Vdf::from(keyvalues_parser::parse(text).ok()?);
    let manifest = vdf.value.get_obj()?;
    let app = get_str(manifest, "appid")?.to_owned();
    let flags: u32 = get_str(manifest, "StateFlags")?.parse().ok()?;
    let number = |key| get_u64(manifest, key).unwrap_or(0);
    let (downloaded, to_download) = (number("BytesDownloaded"), number("BytesToDownload"));
    let (staged, to_stage) = (number("BytesStaged"), number("BytesToStage"));
    let busy = UPDATE_RUNNING | UPDATE_PAUSED | UPDATE_STARTED;
    let installed = flags & FULLY_INSTALLED != 0 && flags & busy == 0;
    let written = if installed || to_stage == 0 {
        0
    } else {
        folder_size(&downloading.join(&app)).min(to_stage)
    };
    let progress = if installed {
        Progress::Installed
    } else if flags & UPDATE_PAUSED != 0 && written > 0 {
        Progress::Paused {
            done: written,
            total: to_stage,
        }
    } else if flags & UPDATE_PAUSED != 0 {
        Progress::Paused {
            done: downloaded,
            total: to_download,
        }
    } else if to_download > 0 && downloaded < to_download && written > 0 {
        Progress::Downloading {
            done: written,
            total: to_stage,
        }
    } else if to_download > 0 && downloaded < to_download {
        Progress::Downloading {
            done: downloaded,
            total: to_download,
        }
    } else if to_stage > 0 && staged < to_stage {
        Progress::Installing {
            done: staged,
            total: to_stage,
        }
    } else {
        Progress::Waiting
    };
    Some(AppState {
        name: get_str(manifest, "name")
            .filter(|name| !name.is_empty())
            .map_or_else(|| format!("Steam app {app}"), str::to_owned),
        app,
        progress,
    })
}

/// The bytes in the files under a folder; 0 when there is none.
fn folder_size(folder: &Path) -> u64 {
    let mut total = 0;
    let mut pending = vec![folder.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map_or(0, |meta| meta.len());
            }
        }
    }
    total
}

/// Steam's log of its downloads and verifies, in the first root that has
/// one.
fn content_log() -> Option<PathBuf> {
    roots()
        .into_iter()
        .map(|root| root.join("logs/content_log.txt"))
        .find(|log| log.is_file())
}

/// Where Steam's content log ends now, so that what Steam does from here on
/// can be read with [`verifies`]; 0 when there is no log.
#[must_use]
pub fn log_end() -> u64 {
    content_log()
        .and_then(|log| fs::metadata(log).ok())
        .map_or(0, |meta| meta.len())
}

/// A verify Steam's content log tells of, and where in the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyEvent {
    /// The byte in the log where its line starts.
    pub at: u64,
    pub app: String,
    /// Started (`Start validating appID 620`) or, after that, finished
    /// (`AppID 620 scheduler finished`).
    pub finished: bool,
}

/// The verifies started and finished in Steam's content log from a byte
/// on; from the start when the log is now shorter (Steam began a new one).
#[must_use]
pub fn verifies(from: u64) -> Vec<VerifyEvent> {
    let Some(text) = content_log().and_then(|log| fs::read(log).ok()) else {
        return Vec::new();
    };
    let from = usize::try_from(from)
        .ok()
        .filter(|from| *from <= text.len())
        .unwrap_or(0);
    parse_verifies(&String::from_utf8_lossy(&text[from..]), from as u64)
}

fn parse_verifies(text: &str, offset: u64) -> Vec<VerifyEvent> {
    let mut events = Vec::new();
    let mut started: HashSet<&str> = HashSet::new();
    let mut at = offset;
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len() as u64;
        // `[2026-11-08 00:52:37] Start validating appID 242820 on BuildID …`
        let said = line
            .split_once("] ")
            .map_or(line, |(_, said)| said)
            .trim_end();
        if let Some(rest) = said.strip_prefix("Start validating appID ") {
            let app = rest.split_whitespace().next().unwrap_or_default();
            if !app.is_empty() {
                started.insert(app);
                events.push(VerifyEvent {
                    at: start,
                    app: app.to_owned(),
                    finished: false,
                });
            }
        } else if let Some(rest) = said.strip_prefix("AppID ")
            && let Some((app, what)) = rest.split_once(' ')
            && what.starts_with("scheduler finished")
            && started.remove(app)
        {
            events.push(VerifyEvent {
                at: start,
                app: app.to_owned(),
                finished: true,
            });
        }
    }
    events
}

/// What Zendo can ask Steam to do with an app; Steam shows its own dialog
/// or progress for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Its install dialog, where the user picks the library.
    Install,
    /// Its uninstall question.
    Uninstall,
    /// Verify the game's files, fetching again what is missing or damaged.
    Verify,
}

impl Request {
    fn uri(self, app: &str) -> String {
        let verb = match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Verify => "validate",
        };
        format!("steam://{verb}/{app}")
    }
}

/// Asks Steam to do something with an app. Started as `steam -silent`, so a
/// Steam that was not running opens to the tray rather than its window (one
/// already running may still raise its window); `xdg-open` when there is no
/// `steam` command (a Flatpak Steam).
///
/// # Errors
///
/// When neither can be started.
pub fn ask(request: Request, app: &str) -> anyhow::Result<()> {
    let uri = request.uri(app);
    let spawn = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    let child = match spawn("steam", &["-silent", &uri]) {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => spawn("xdg-open", &[&uri])
            .map_err(|err| anyhow::anyhow!("could not ask Steam: {err}"))?,
        Err(err) => anyhow::bail!("could not start Steam: {err}"),
    };
    // Reaped on a thread: a Steam started here keeps running.
    let mut child = child;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// A manifest's `installdir`, when its app is fully installed.
fn installed_dir(text: &str) -> Option<String> {
    let vdf = Vdf::from(keyvalues_parser::parse(text).ok()?);
    let state = vdf.value.get_obj()?;
    let flags: u32 = get_str(state, "StateFlags")?.parse().ok()?;
    if flags & FULLY_INSTALLED == 0 {
        return None;
    }
    get_str(state, "installdir")
        .filter(|dir| !dir.is_empty() && !dir.contains('/') && *dir != "..")
        .map(str::to_owned)
}

/// The folders listed in the root's `libraryfolders.vdf`, or the root alone
/// when the file is missing.
fn library_folders(root: &Path, problems: &mut Vec<String>) -> Vec<PathBuf> {
    let file = root.join("steamapps/libraryfolders.vdf");
    let Ok(text) = fs::read_to_string(&file) else {
        return vec![root.to_path_buf()];
    };
    match parse_library_folders(&text) {
        Ok(paths) if !paths.is_empty() => paths,
        Ok(_) => vec![root.to_path_buf()],
        Err(err) => {
            problems.push(format!("{}: {err}", file.display()));
            vec![root.to_path_buf()]
        }
    }
}

fn parse_library_folders(text: &str) -> anyhow::Result<Vec<PathBuf>> {
    let vdf = Vdf::from(keyvalues_parser::parse(text)?);
    let Some(folders) = vdf.value.get_obj() else {
        return Ok(Vec::new());
    };
    // Entries are "0", "1", ...; older files also hold plain strings here.
    Ok(folders
        .values()
        .flatten()
        .filter_map(|value| value.get_obj())
        .filter_map(|folder| get_str(folder, "path"))
        .map(PathBuf::from)
        .collect())
}

/// A Steam account signed in on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// The 64-bit `Steam ID` Steam's Web API takes.
    pub id: String,
    /// The name friends see.
    pub name: String,
}

/// The account Steam signed in last, from `config/loginusers.vdf`: the one
/// marked `MostRecent`, else the latest by `timestamp`.
#[must_use]
pub fn user(roots: &[PathBuf]) -> Option<User> {
    roots.iter().find_map(|root| {
        fs::read_to_string(root.join("config/loginusers.vdf"))
            .ok()
            .and_then(|text| parse_users(&text))
    })
}

/// The name Steam on this computer knows an account by, when it does.
#[must_use]
pub fn user_named(roots: &[PathBuf], id: &str) -> Option<String> {
    roots.iter().find_map(|root| {
        let text = fs::read_to_string(root.join("config/loginusers.vdf")).ok()?;
        users(&text)
            .into_iter()
            .find(|(_, user)| user.id == id)
            .map(|(_, user)| user.name)
            .filter(|name| !name.is_empty())
    })
}

fn parse_users(text: &str) -> Option<User> {
    users(text)
        .into_iter()
        .max_by_key(|(order, _)| *order)
        .map(|(_, user)| user)
}

/// The accounts in `loginusers.vdf`, each with whether it is the most
/// recent and when it signed in.
fn users(text: &str) -> Vec<((bool, u64), User)> {
    let Ok(parsed) = keyvalues_parser::parse(text) else {
        return Vec::new();
    };
    let vdf = Vdf::from(parsed);
    let Some(accounts) = vdf.value.get_obj() else {
        return Vec::new();
    };
    accounts
        .iter()
        .filter(|(id, _)| id.len() == 17 && id.bytes().all(|byte| byte.is_ascii_digit()))
        .filter_map(|(id, values)| {
            let user = values.first()?.get_obj()?;
            let recent = get_str(user, "MostRecent") == Some("1");
            let when = get_u64(user, "timestamp").unwrap_or(0);
            let name = get_str(user, "PersonaName")
                .or_else(|| get_str(user, "AccountName"))
                .unwrap_or_default();
            Some((
                (recent, when),
                User {
                    id: id.to_string(),
                    name: name.to_owned(),
                },
            ))
        })
        .collect()
}

/// Steam's cached image for an app, installed or not: `library_600x900.jpg`
/// or `library_hero.jpg`. Steam keeps them for the games in its library.
#[must_use]
pub fn cached_image(app: &str, name: &str) -> Option<PathBuf> {
    roots()
        .iter()
        .find_map(|root| library_image(root, app, name))
}

/// Each account's `LastPlayed` and `Playtime` (minutes) per app, from
/// `userdata/<account>/config/localconfig.vdf`, the latest and longest
/// across accounts.
fn play_records(root: &Path, problems: &mut Vec<String>) -> HashMap<String, Played> {
    let mut played: HashMap<String, Played> = HashMap::new();
    let Ok(accounts) = fs::read_dir(root.join("userdata")) else {
        return played;
    };
    for account in accounts.filter_map(Result::ok) {
        let file = account.path().join("config/localconfig.vdf");
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        match parse_play_records(&text) {
            Ok(records) => {
                for (app, record) in records {
                    let entry = played.entry(app).or_default();
                    *entry = entry.merge(record);
                }
            }
            Err(err) => problems.push(format!("{}: {err}", file.display())),
        }
    }
    played
}

fn parse_play_records(text: &str) -> anyhow::Result<Vec<(String, Played)>> {
    let vdf = Vdf::from(keyvalues_parser::parse(text)?);
    let apps = ["Software", "Valve", "Steam", "apps"]
        .iter()
        .try_fold(vdf.value.get_obj(), |obj, key| {
            obj.map(|obj| get_obj(obj, key))
        })
        .flatten();
    let Some(apps) = apps else {
        return Ok(Vec::new());
    };
    Ok(apps
        .iter()
        .filter_map(|(app, values)| {
            let record = values.first()?.get_obj()?;
            let played = Played {
                last: get_u64(record, "LastPlayed").filter(|&when| when > 0),
                minutes: get_u64(record, "Playtime").unwrap_or(0),
                launches: 0,
            };
            Some((app.to_string(), played))
        })
        .collect())
}

fn scan_folder(root: &Path, steamapps: &Path, played: &HashMap<String, Played>, scan: &mut Scan) {
    let Ok(entries) = fs::read_dir(steamapps) else {
        return;
    };
    let mut manifests: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "acf")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("appmanifest_"))
        })
        .collect();
    manifests.sort();
    for manifest in manifests {
        match read_manifest(&manifest) {
            Ok(Some(app)) => {
                let record = played.get(&app.id).copied().unwrap_or_default();
                scan.games.push(app.into_game(root, steamapps, record));
            }
            Ok(None) => {}
            Err(err) => scan.problems.push(format!("{}: {err}", manifest.display())),
        }
    }
}

struct App {
    id: String,
    name: String,
    last_played: Option<u64>,
    install_dir: Option<String>,
    size: Option<u64>,
    build: Option<String>,
}

impl App {
    fn into_game(self, root: &Path, steamapps: &Path, played: Played) -> Game {
        let mut game = Game::new(
            format!("steam:{}", self.id),
            self.name,
            Source::Steam,
            Launch::Uri {
                uri: format!("steam://rungameid/{}", self.id),
            },
        );
        game.cover = library_image(root, &self.id, "library_600x900.jpg");
        game.hero = library_image(root, &self.id, "library_hero.jpg");
        game.played = played.merge(Played {
            last: self.last_played,
            ..Played::default()
        });
        game.details = Details {
            install_path: self
                .install_dir
                .map(|dir| steamapps.join("common").join(dir)),
            size: self.size.filter(|&size| size > 0).map(Size::Bytes),
            version: self.build.map(|build| format!("build {build}")),
            ..Details::default()
        };
        game
    }
}

/// The app a manifest describes, or `None` for a tool or an app still
/// downloading.
fn read_manifest(path: &Path) -> anyhow::Result<Option<App>> {
    let text = fs::read_to_string(path)?;
    parse_manifest(&text)
}

fn parse_manifest(text: &str) -> anyhow::Result<Option<App>> {
    let vdf = Vdf::from(keyvalues_parser::parse(text)?);
    let state = vdf
        .value
        .get_obj()
        .ok_or_else(|| anyhow::anyhow!("no AppState block"))?;
    let id = get_str(state, "appid").ok_or_else(|| anyhow::anyhow!("no appid"))?;
    if !id.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("appid {id:?} is not a number");
    }
    let name = get_str(state, "name").unwrap_or(id);
    let flags: u32 = get_str(state, "StateFlags")
        .and_then(|flags| flags.parse().ok())
        .unwrap_or(0);
    if flags & FULLY_INSTALLED == 0 || TOOL_PREFIXES.iter().any(|tool| name.starts_with(tool)) {
        return Ok(None);
    }
    Ok(Some(App {
        id: id.to_owned(),
        name: name.to_owned(),
        last_played: get_u64(state, "LastPlayed").filter(|&when| when > 0),
        install_dir: get_str(state, "installdir")
            .filter(|dir| !dir.is_empty() && *dir != "." && *dir != ".." && !dir.contains('/'))
            .map(str::to_owned),
        size: get_u64(state, "SizeOnDisk"),
        build: get_str(state, "buildid").map(str::to_owned),
    }))
}

/// One of Steam's cached images for the app (`library_600x900.jpg`, the
/// portrait cover; `library_hero.jpg`, the wide one): in
/// `librarycache/<id>/` in current clients, `librarycache/<id>_<name>` in
/// older ones.
fn library_image(root: &Path, id: &str, name: &str) -> Option<PathBuf> {
    let cache = root.join("appcache/librarycache");
    [
        cache.join(id).join(name),
        cache.join(format!("{id}_{name}")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// A string value, matching the key without regard to case: Steam writes
/// `appid` in some manifests and `AppID` in others.
fn get_str<'a>(obj: &'a Obj<'_>, key: &str) -> Option<&'a str> {
    get(obj, key).and_then(|value| value.get_str())
}

fn get_u64(obj: &Obj<'_>, key: &str) -> Option<u64> {
    get_str(obj, key).and_then(|value| value.parse().ok())
}

fn get_obj<'a>(obj: &'a Obj<'_>, key: &str) -> Option<&'a Obj<'a>> {
    get(obj, key).and_then(|value| value.get_obj())
}

fn get<'a>(obj: &'a Obj<'_>, key: &str) -> Option<&'a keyvalues_parser::Value<'a>> {
    obj.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .and_then(|(_, values)| values.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/steam")
    }

    #[test]
    fn finds_installed_games_and_skips_tools_and_downloads() {
        let scan = scan(&[fixture()]);
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        let titles: Vec<&str> = scan.games.iter().map(|game| game.title.as_str()).collect();
        assert_eq!(titles, ["Portal 2"]);
        let game = &scan.games[0];
        assert_eq!(game.id, "steam:620");
        assert_eq!(
            game.launch,
            Launch::Uri {
                uri: "steam://rungameid/620".into()
            }
        );
        assert!(
            game.cover
                .as_ref()
                .is_some_and(|cover| cover.ends_with("620/library_600x900.jpg"))
        );
        assert_eq!(game.hero, None);
        assert!(
            game.details
                .install_path
                .as_ref()
                .is_some_and(|path| path.ends_with("steamapps/common/Portal 2"))
        );
        assert_eq!(game.details.size, Some(Size::Bytes(12_884_901_888)));
        assert_eq!(game.details.version.as_deref(), Some("build 4567"));
    }

    #[test]
    fn lists_library_folders_and_skips_old_string_entries() {
        let folders = parse_library_folders(
            r#""libraryfolders"
            {
                "contentstatsid" "123"
                "0" { "path" "/home/me/.local/share/Steam" "apps" { "620" "123" } }
                "1" { "path" "/mnt/games/SteamLibrary" }
            }"#,
        )
        .unwrap();
        assert_eq!(
            folders,
            [
                PathBuf::from("/home/me/.local/share/Steam"),
                PathBuf::from("/mnt/games/SteamLibrary")
            ]
        );
    }

    #[test]
    fn reads_when_and_how_long_each_app_was_played() {
        let records = parse_play_records(
            r#""UserLocalConfigStore"
            {
                "Software" { "valve" { "Steam" { "apps" {
                    "620" { "LastPlayed" "1700000000" "Playtime" "491" }
                    "70" { "LastPlayed" "0" }
                } } } }
            }"#,
        )
        .unwrap();
        let get = |app: &str| records.iter().find(|(id, _)| id == app).unwrap().1;
        assert_eq!(get("620").last, Some(1_700_000_000));
        assert_eq!(get("620").minutes, 491);
        assert_eq!(get("70").last, None);
    }

    #[test]
    fn reads_manifest_keys_in_any_case() {
        let app =
            parse_manifest(r#""AppState" { "AppID" "70" "name" "Half-Life" "StateFlags" "6" }"#)
                .unwrap()
                .unwrap();
        assert_eq!((app.id.as_str(), app.name.as_str()), ("70", "Half-Life"));
    }

    #[test]
    fn reports_a_broken_manifest() {
        assert!(parse_manifest(r#""AppState" { "name" "No id" }"#).is_err());
    }

    #[test]
    fn picks_the_account_signed_in_last() {
        let text = r#""users"
{
	"76561197960265729"
	{
		"AccountName"		"older"
		"PersonaName"		"Older"
		"timestamp"		"1700000000"
	}
	"76561197960265730"
	{
		"AccountName"		"newer"
		"PersonaName"		"Newer"
		"timestamp"		"1800000000"
	}
}
"#;
        assert_eq!(
            parse_users(text),
            Some(User {
                id: "76561197960265730".into(),
                name: "Newer".into()
            })
        );
        let marked = text.replace("\"Older\"", "\"Older\"\n\t\t\"MostRecent\"\t\t\"1\"");
        assert_eq!(
            parse_users(&marked).map(|user| user.name),
            Some("Older".into())
        );
        assert_eq!(parse_users("\"users\"\n{\n}\n"), None);
    }

    #[test]
    fn reads_verifies_from_steams_log() {
        let log = "[2026-11-08 00:52:28] AppID 242820 scheduler finished : removed\n\
                   [2026-11-08 00:52:37] Start validating appID 242820 on BuildID 4183311\n\
                   [2026-11-08 00:52:37] AppID 242820 App update changed : Running Update,\n\
                   [2026-11-08 00:52:38] Start validating appID 1070560 on BuildID 26115262\n\
                   [2026-11-08 00:52:38] AppID 242820 scheduler finished : removed\n";
        let events = parse_verifies(log, 100);
        let said: Vec<(&str, bool)> = events
            .iter()
            .map(|event| (event.app.as_str(), event.finished))
            .collect();
        // The first "finished" ends an update, not a verify: left out.
        assert_eq!(
            said,
            [("242820", false), ("1070560", false), ("242820", true)]
        );
        let first = log.find("[2026-11-08 00:52:37]").unwrap() as u64;
        assert_eq!(events[0].at, 100 + first);
    }

    #[test]
    fn tells_how_far_an_install_is() {
        let manifest = |flags: u32, downloaded: u64, staged: u64| {
            format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"242820\"\n\t\"name\"\t\t\"140\"\n\
                 \t\"StateFlags\"\t\t\"{flags}\"\n\t\"BytesToDownload\"\t\t\"1000\"\n\
                 \t\"BytesDownloaded\"\t\t\"{downloaded}\"\n\t\"BytesToStage\"\t\t\"4000\"\n\
                 \t\"BytesStaged\"\t\t\"{staged}\"\n}}\n"
            )
        };
        let nowhere = Path::new("/nonexistent/downloading");
        let progress = |text: String| parse_state(&text, nowhere).map(|state| state.progress);
        assert_eq!(
            progress(manifest(1026, 250, 0)),
            Some(Progress::Downloading {
                done: 250,
                total: 1000
            })
        );
        assert_eq!(
            progress(manifest(1026, 1000, 3000)),
            Some(Progress::Installing {
                done: 3000,
                total: 4000
            })
        );
        assert_eq!(
            progress(manifest(1538, 250, 0)),
            Some(Progress::Paused {
                done: 250,
                total: 1000
            })
        );
        assert_eq!(
            progress(manifest(2, 0, 0)),
            Some(Progress::Downloading {
                done: 0,
                total: 1000
            })
        );
        assert_eq!(progress(manifest(4, 1000, 4000)), Some(Progress::Installed));
        // An installed game waiting for an update stays installed; one Steam
        // is updating is busy again.
        assert_eq!(progress(manifest(6, 0, 0)), Some(Progress::Installed));
        assert_eq!(
            progress(manifest(1030, 0, 0)),
            Some(Progress::Downloading {
                done: 0,
                total: 1000
            })
        );
        assert_eq!(
            parse_state(&manifest(4, 1000, 4000), nowhere).unwrap().name,
            "140"
        );
        // While Steam downloads, the files it has written say how far.
        let downloading =
            std::env::temp_dir().join(format!("zendo-steam-dl-{}", std::process::id()));
        fs::create_dir_all(downloading.join("242820/bin")).unwrap();
        fs::write(downloading.join("242820/bin/game"), vec![0u8; 1500]).unwrap();
        assert_eq!(
            parse_state(&manifest(1026, 0, 0), &downloading).map(|state| state.progress),
            Some(Progress::Downloading {
                done: 1500,
                total: 4000
            })
        );
        fs::remove_dir_all(&downloading).unwrap();
    }

    #[test]
    #[ignore = "reads this computer's Steam"]
    fn reads_this_computers_steam() {
        for state in states() {
            println!("{} {} {:?}", state.app, state.name, state.progress);
        }
        for event in verifies(0) {
            println!(
                "verify {} at {} finished {}",
                event.app, event.at, event.finished
            );
        }
    }
}
