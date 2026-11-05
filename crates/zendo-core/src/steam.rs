//! Steam: the library folders in `libraryfolders.vdf` and the installed
//! apps in each folder's `appmanifest_*.acf`. Covers come from Steam's own
//! cache in `appcache/librarycache`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use keyvalues_parser::{Obj, Vdf};

use crate::{Game, Launch, Scan, Source, paths};

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
        for folder in library_folders(&root, &mut scan.problems) {
            let folder = folder.canonicalize().unwrap_or(folder);
            if seen_folders.insert(folder.clone()) {
                scan_folder(&root, &folder.join("steamapps"), &mut scan);
            }
        }
    }
    scan
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

fn scan_folder(root: &Path, steamapps: &Path, scan: &mut Scan) {
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
            Ok(Some(app)) => scan.games.push(app.into_game(root)),
            Ok(None) => {}
            Err(err) => scan.problems.push(format!("{}: {err}", manifest.display())),
        }
    }
}

struct App {
    id: String,
    name: String,
}

impl App {
    fn into_game(self, root: &Path) -> Game {
        Game {
            cover: cover(root, &self.id),
            launch: Launch::Uri {
                uri: format!("steam://rungameid/{}", self.id),
            },
            id: format!("steam:{}", self.id),
            title: self.name,
            source: Source::Steam,
        }
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
    }))
}

/// Steam's portrait cover for the app: `librarycache/<id>/` in current
/// clients, `librarycache/<id>_library_600x900.jpg` in older ones.
fn cover(root: &Path, id: &str) -> Option<PathBuf> {
    let cache = root.join("appcache/librarycache");
    [
        cache.join(id).join("library_600x900.jpg"),
        cache.join(format!("{id}_library_600x900.jpg")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// A string value, matching the key without regard to case: Steam writes
/// `appid` in some manifests and `AppID` in others.
fn get_str<'a>(obj: &'a Obj<'_>, key: &str) -> Option<&'a str> {
    obj.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .and_then(|(_, values)| values.first())
        .and_then(|value| value.get_str())
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
