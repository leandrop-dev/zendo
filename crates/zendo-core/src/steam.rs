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

pub fn scan(roots: &[PathBuf]) -> Scan {
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
        install_dir: get_str(state, "installdir").map(str::to_owned),
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
}
