//! Heroic: the games it installed from Epic (legendary), GOG (gogdl) and
//! Amazon (nile), and the ones added to it by hand. Each store keeps its own
//! files under Heroic's config folder; launching goes back through Heroic
//! with a `heroic://launch` link, so its settings for the game apply.
//!
//! Heroic's cache of each store's library (`store_cache/*_library.json`)
//! gives titles and a portrait image's URL, and `store/timestamp.json` when
//! and how long each game was played.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{Game, Launch, Played, Scan, Source, paths, time};

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
        let mut found = Vec::new();
        epic(root, &mut found, &mut scan);
        gog(root, &mut found, &mut scan);
        amazon(root, &mut found, &mut scan);
        sideloaded(root, &mut found, &mut scan);
        let played = timestamps(root, &mut scan);
        for mut game in found {
            if let Some(record) = played.get(game.id.split_once(':').map_or("", |(_, app)| app)) {
                game.played = *record;
            }
            scan.games.push(game);
        }
    }
    scan
}

/// An entry of a Heroic library file. The files differ in their top key
/// (`games` or `library`) but share these fields.
#[derive(Deserialize)]
struct LibraryGame {
    app_name: String,
    title: Option<String>,
    art_square: Option<String>,
    is_installed: Option<bool>,
}

#[derive(Deserialize)]
struct LibraryFile {
    #[serde(default, alias = "library")]
    games: Vec<LibraryGame>,
}

/// Titles and portrait images by app name, from a Heroic library file.
fn library(file: &Path, scan: &mut Scan) -> HashMap<String, LibraryGame> {
    read_json::<LibraryFile>(file, scan)
        .map(|library| {
            library
                .games
                .into_iter()
                .map(|game| (game.app_name.clone(), game))
                .collect()
        })
        .unwrap_or_default()
}

/// `legendaryConfig/legendary/installed.json`: app name to installed game.
fn epic(root: &Path, found: &mut Vec<Game>, scan: &mut Scan) {
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
    let library = library(&root.join("store_cache/legendary_library.json"), scan);
    for (app_name, game) in installed.into_iter().filter(|(_, game)| !game.is_dlc) {
        let known = library.get(&app_name);
        found.push(heroic_game(
            Source::Epic,
            "legendary",
            &app_name,
            game.title,
            known,
        ));
    }
}

/// `gog_store/installed.json` lists what is installed; the titles are in
/// Heroic's cache of the GOG library, `store_cache/gog_library.json`.
fn gog(root: &Path, found: &mut Vec<Game>, scan: &mut Scan) {
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
    let library = library(&root.join("store_cache/gog_library.json"), scan);
    for game in installed.installed.into_iter().filter(|game| !game.is_dlc) {
        let known = library.get(&game.app_name);
        found.push(heroic_game(Source::Gog, "gog", &game.app_name, None, known));
    }
}

/// `nile_config/nile/installed.json` lists what is installed; the titles are
/// in Heroic's `store_cache/nile_library.json`, or failing that in nile's
/// own `library.json`.
fn amazon(root: &Path, found: &mut Vec<Game>, scan: &mut Scan) {
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
    let library = library(&root.join("store_cache/nile_library.json"), scan);
    let titles: HashMap<String, String> = read_json::<Vec<Entry>>(&dir.join("library.json"), scan)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| Some((entry.product.id, entry.product.title?)))
        .collect();
    for game in installed {
        let title = titles.get(&game.id).cloned();
        let known = library.get(&game.id);
        found.push(heroic_game(Source::Amazon, "nile", &game.id, title, known));
    }
}

/// `sideload_apps/library.json`: games added to Heroic by hand.
fn sideloaded(root: &Path, found: &mut Vec<Game>, scan: &mut Scan) {
    let library = library(&root.join("sideload_apps/library.json"), scan);
    for (app_name, game) in &library {
        if game.is_installed != Some(false) {
            found.push(heroic_game(
                Source::Heroic,
                "sideload",
                app_name,
                None,
                Some(game),
            ));
        }
    }
}

/// `store/timestamp.json`: `lastPlayed` (an ISO-8601 date) and
/// `totalPlayed` (minutes) by app name. Heroic's store reads dots in keys
/// as nesting, so an Amazon id like `amzn1.adg.product.…` is stored as
/// nested objects and is joined back here.
fn timestamps(root: &Path, scan: &mut Scan) -> HashMap<String, Played> {
    fn walk(prefix: &str, value: &serde_json::Value, out: &mut HashMap<String, Played>) {
        let Some(object) = value.as_object() else {
            return;
        };
        if object.contains_key("lastPlayed") || object.contains_key("totalPlayed") {
            let last = object
                .get("lastPlayed")
                .and_then(serde_json::Value::as_str)
                .and_then(time::parse_utc);
            let minutes = object
                .get("totalPlayed")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            out.insert(
                prefix.to_owned(),
                Played {
                    last,
                    minutes,
                    launches: 0,
                },
            );
            return;
        }
        for (key, child) in object {
            let key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            walk(&key, child, out);
        }
    }

    let mut played = HashMap::new();
    if let Some(value) = read_json::<serde_json::Value>(&root.join("store/timestamp.json"), scan) {
        walk("", &value, &mut played);
    }
    played
}

fn heroic_game(
    source: Source,
    runner: &str,
    app_name: &str,
    title: Option<String>,
    known: Option<&LibraryGame>,
) -> Game {
    let title = title
        .or_else(|| known.and_then(|game| game.title.clone()))
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| app_name.to_owned());
    let mut game = Game::new(
        format!("{}:{app_name}", source_key(source)),
        title,
        source,
        Launch::Uri {
            uri: format!(
                "heroic://launch?appName={}&runner={runner}",
                percent_encode(app_name)
            ),
        },
    );
    // A store's image is a URL; one picked by hand for a sideloaded game
    // may be a file.
    match known.and_then(|game| game.art_square.as_deref()) {
        Some(art) if art.starts_with("https://") || art.starts_with("http://") => {
            game.cover_url = Some(art.to_owned());
        }
        Some(art) => {
            let path = PathBuf::from(art.strip_prefix("file://").unwrap_or(art));
            game.cover = Some(path).filter(|path| path.is_absolute() && path.is_file());
        }
        None => {}
    }
    game
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
pub(crate) fn percent_encode(text: &str) -> String {
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

    fn by_id(scan: &Scan, id: &str) -> Game {
        scan.games
            .iter()
            .find(|game| game.id == id)
            .unwrap()
            .clone()
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
    fn takes_cover_urls_from_the_store_caches() {
        let scan = scan(&[fixture()]);
        assert_eq!(
            by_id(&scan, "epic:Fortnite").cover_url.as_deref(),
            Some("https://cdn.example/fortnite-portrait.jpg")
        );
        assert_eq!(
            by_id(&scan, "amazon:amzn1.adg.product.0001")
                .cover_url
                .as_deref(),
            Some("https://cdn.example/amazon-portrait.jpg")
        );
        assert_eq!(by_id(&scan, "gog:1500000000").cover_url, None);
    }

    #[test]
    fn reads_play_times_even_for_dotted_ids() {
        let scan = scan(&[fixture()]);
        let amazon = by_id(&scan, "amazon:amzn1.adg.product.0001").played;
        assert_eq!(amazon.last, Some(1_793_927_320));
        assert_eq!(amazon.minutes, 3);
        let gog = by_id(&scan, "gog:1207658924").played;
        assert_eq!((gog.last, gog.minutes), (None, 1090));
    }

    #[test]
    fn launches_through_heroic() {
        let game = heroic_game(Source::Amazon, "nile", "amzn1.adg product/1", None, None);
        assert_eq!(
            game.launch,
            Launch::Uri {
                uri: "heroic://launch?appName=amzn1.adg%20product%2F1&runner=nile".into()
            }
        );
    }
}
