//! Heroic: the games it installed from Epic (legendary), GOG (gogdl) and
//! Amazon (nile), and the ones added to it by hand. Each store keeps its own
//! files under Heroic's config folder; launching goes back through Heroic
//! with a `heroic://launch` link, so its settings for the game apply.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{Game, Launch, Scan, Source, paths};

/// Where Heroic keeps its config: native and Flatpak.
pub fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(config) = paths::config_home() {
        roots.push(config.join("heroic"));
    }
    if let Some(home) = paths::home() {
        roots.push(home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"));
    }
    roots
}

pub fn scan(roots: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    for root in roots.iter().filter(|root| root.is_dir()) {
        epic(root, &mut scan);
        gog(root, &mut scan);
        amazon(root, &mut scan);
        sideloaded(root, &mut scan);
    }
    scan
}

/// `legendaryConfig/legendary/installed.json`: app name to installed game.
fn epic(root: &Path, scan: &mut Scan) {
    #[derive(Deserialize)]
    struct Installed {
        title: Option<String>,
        #[serde(default)]
        is_dlc: bool,
    }

    let file = root.join("legendaryConfig/legendary/installed.json");
    let Some(installed) = read_json::<HashMap<String, Installed>>(&file, scan) else {
        return;
    };
    for (app_name, game) in installed {
        if !game.is_dlc {
            scan.games.push(heroic_game(
                Source::Epic,
                "legendary",
                &app_name,
                game.title,
            ));
        }
    }
}

/// `gog_store/installed.json` lists what is installed; the titles are in
/// Heroic's cache of the GOG library, `store_cache/gog_library.json`.
fn gog(root: &Path, scan: &mut Scan) {
    #[derive(Deserialize)]
    struct InstalledFile {
        #[serde(default)]
        installed: Vec<Installed>,
    }
    #[derive(Deserialize)]
    struct Installed {
        #[serde(rename = "appName")]
        app_name: String,
        #[serde(default)]
        is_dlc: bool,
    }

    let file = root.join("gog_store/installed.json");
    let Some(installed) = read_json::<InstalledFile>(&file, scan) else {
        return;
    };
    let titles = library_titles(&root.join("store_cache/gog_library.json"), scan);
    for game in installed.installed.into_iter().filter(|game| !game.is_dlc) {
        let title = titles.get(&game.app_name).cloned();
        scan.games
            .push(heroic_game(Source::Gog, "gog", &game.app_name, title));
    }
}

/// `nile_config/nile/installed.json` lists what is installed; the titles are
/// in nile's `library.json`.
fn amazon(root: &Path, scan: &mut Scan) {
    #[derive(Deserialize)]
    struct Installed {
        id: String,
    }
    #[derive(Deserialize)]
    struct Entry {
        product: Product,
    }
    #[derive(Deserialize)]
    struct Product {
        id: String,
        title: Option<String>,
    }

    let dir = root.join("nile_config/nile");
    let Some(installed) = read_json::<Vec<Installed>>(&dir.join("installed.json"), scan) else {
        return;
    };
    let titles: HashMap<String, String> = read_json::<Vec<Entry>>(&dir.join("library.json"), scan)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| Some((entry.product.id, entry.product.title?)))
        .collect();
    for game in installed {
        let title = titles.get(&game.id).cloned();
        scan.games
            .push(heroic_game(Source::Amazon, "nile", &game.id, title));
    }
}

/// `sideload_apps/library.json`: games added to Heroic by hand.
fn sideloaded(root: &Path, scan: &mut Scan) {
    let file = root.join("sideload_apps/library.json");
    let Some(library) = read_json::<LibraryFile>(&file, scan) else {
        return;
    };
    for game in library.games {
        if game.is_installed != Some(false) {
            scan.games.push(heroic_game(
                Source::Heroic,
                "sideload",
                &game.app_name,
                game.title,
            ));
        }
    }
}

/// Heroic's library files: `{"games": [{"app_name": …, "title": …}]}`.
#[derive(Deserialize)]
struct LibraryFile {
    #[serde(default)]
    games: Vec<LibraryGame>,
}

#[derive(Deserialize)]
struct LibraryGame {
    app_name: String,
    title: Option<String>,
    is_installed: Option<bool>,
}

fn library_titles(file: &Path, scan: &mut Scan) -> HashMap<String, String> {
    read_json::<LibraryFile>(file, scan)
        .map(|library| {
            library
                .games
                .into_iter()
                .filter_map(|game| Some((game.app_name, game.title?)))
                .collect()
        })
        .unwrap_or_default()
}

fn heroic_game(source: Source, runner: &str, app_name: &str, title: Option<String>) -> Game {
    Game {
        id: format!("{}:{app_name}", source_key(source)),
        title: title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| app_name.to_owned()),
        source,
        launch: Launch::Uri {
            uri: format!(
                "heroic://launch?appName={}&runner={runner}",
                percent_encode(app_name)
            ),
        },
        cover: None,
    }
}

fn source_key(source: Source) -> &'static str {
    match source {
        Source::Epic => "epic",
        Source::Gog => "gog",
        Source::Amazon => "amazon",
        _ => "heroic",
    }
}

/// Encodes everything but RFC 3986's unreserved characters.
fn percent_encode(text: &str) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// The parsed file, or `None` when it is missing (the store is not used) or
/// broken (reported as a problem).
fn read_json<T: DeserializeOwned>(file: &Path, scan: &mut Scan) -> Option<T> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return None,
        Err(err) => {
            scan.problems.push(format!("{}: {err}", file.display()));
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            scan.problems.push(format!("{}: {err}", file.display()));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/heroic")
    }

    #[test]
    fn finds_every_store_and_skips_dlc() {
        let scan = scan(&[fixture()]);
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        let mut found: Vec<(&str, &str)> = scan
            .games
            .iter()
            .map(|game| (game.id.as_str(), game.title.as_str()))
            .collect();
        found.sort_unstable();
        assert_eq!(
            found,
            [
                ("amazon:amzn1.adg.product.0001", "Amazon Game"),
                ("epic:Fortnite", "Fortnite"),
                ("gog:1207658924", "Unreal Tournament 2004"),
                ("gog:1500000000", "1500000000"),
                ("heroic:abc123", "Hand-added Game"),
            ]
        );
    }

    #[test]
    fn launches_through_heroic() {
        let game = heroic_game(Source::Amazon, "nile", "amzn1.adg product/1", None);
        assert_eq!(
            game.launch,
            Launch::Uri {
                uri: "heroic://launch?appName=amzn1.adg%20product%2F1&runner=nile".into()
            }
        );
    }
}
