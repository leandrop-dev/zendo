//! Whether a game's anti-cheat lets it play on Linux, from
//! [AreWeAntiCheatYet](https://areweanticheatyet.com)'s list (MIT), as
//! Heroic warns of it.
//!
//! The list is the project's `games.json`, downloaded at most once a day
//! into `$XDG_CACHE_HOME/zendo/areweanticheatyet.json`, as Heroic does. Each
//! game has a status, the anti-cheats it ships, notes and updates with
//! their sources, and its ids in the stores: `steam` (an app id) or `epic`
//! (a namespace). Heroic matches only the Epic namespace; Zendo matches the
//! Steam app id too, which it finds for any game (as for `ProtonDB`).

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use serde::Deserialize;

use crate::{paths, store_tasks};

/// Where the project keeps its list.
const URL: &str =
    "https://raw.githubusercontent.com/AreWeAntiCheatYet/AreWeAntiCheatYet/HEAD/games.json";

/// How long the list kept is used before it is asked again.
const FRESH: Duration = Duration::from_hours(24);

/// A game's standing, with the site's meanings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Status {
    /// The developers enabled the anti-cheat on Linux, or it works under
    /// Proton as it is.
    Supported,
    /// It plays online, maybe after some tinkering, without getting round
    /// the anti-cheat.
    Running,
    /// The developers said they will enable it.
    Planned,
    /// It will not play online because of the anti-cheat.
    Broken,
    /// The developers said they will not enable it.
    Denied,
    /// A status this version of Zendo does not know.
    #[serde(other)]
    Unknown,
}

impl Status {
    /// Whether online play will not work on Linux, or is not known to.
    #[must_use]
    pub fn warns(self) -> bool {
        matches!(self, Self::Broken | Self::Denied | Self::Unknown)
    }
}

/// A source the list gives for what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub text: String,
    /// A web page, when the list gives one.
    pub link: Option<String>,
}

/// A game on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub status: Status,
    /// The anti-cheats it ships, as the list names them.
    pub anti_cheats: Vec<String>,
    pub notes: Vec<Note>,
    /// The newest change to its status, with its date as the list writes it.
    pub latest: Option<(Note, String)>,
    /// Its name in the site's addresses.
    pub slug: String,
}

impl Entry {
    /// The game's page on `AreWeAntiCheatYet`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://areweanticheatyet.com/game/{}", self.slug)
    }
}

/// Downloads the list unless the one kept is less than a day old. It
/// blocks on the network.
///
/// # Errors
///
/// When the list cannot be downloaded or kept; one kept before stays.
pub fn refresh() -> anyhow::Result<()> {
    let file = cache_file().context("no cache folder for AreWeAntiCheatYet's list")?;
    let fresh = fs::metadata(&file)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|when| SystemTime::now().duration_since(when).ok())
        .is_some_and(|age| age < FRESH);
    if fresh {
        return Ok(());
    }
    let text = crate::http::agent(Duration::from_secs(10))
        .get(URL)
        .call()
        .context("could not download AreWeAntiCheatYet's list")?
        .into_body()
        .read_to_string()
        .context("AreWeAntiCheatYet's list")?;
    // Kept only when it reads, so a broken answer leaves the old one.
    let games = parse(&text).context("AreWeAntiCheatYet's list is not valid")?;
    anyhow::ensure!(
        !games.is_empty(),
        "AreWeAntiCheatYet's list has no entry Zendo can read"
    );
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    paths::write_atomic(&file, text.as_bytes())
        .with_context(|| format!("could not write {}", file.display()))
}

/// The game's entry in the list, by its Zendo id and the Steam app id
/// found for it: `None` when the list has no such game. The list is
/// downloaded first when it is a day old ([`refresh`]); offline, the one
/// kept is used. It blocks on the network; the window calls it on a
/// thread.
///
/// # Errors
///
/// When no list is kept and it cannot be downloaded.
pub fn for_game(id: &str, steam_app: Option<u32>) -> anyhow::Result<Option<Entry>> {
    let refreshed = refresh();
    let file = cache_file().context("no cache folder for AreWeAntiCheatYet's list")?;
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(err) => {
            refreshed?;
            return Err(err).with_context(|| format!("could not read {}", file.display()));
        }
    };
    let games = parse(&text).context("AreWeAntiCheatYet's list is not valid")?;
    let namespace = id
        .strip_prefix("epic:")
        .and_then(store_tasks::epic_metadata)
        .and_then(|metadata| {
            metadata
                .pointer("/metadata/namespace")?
                .as_str()
                .map(str::to_owned)
        });
    Ok(lookup(games, steam_app, namespace.as_deref()))
}

/// The game with the Epic namespace, else with the Steam app id.
fn lookup(mut games: Vec<Game>, steam_app: Option<u32>, namespace: Option<&str>) -> Option<Entry> {
    let by_namespace = |game: &Game| {
        namespace.is_some_and(|namespace| {
            game.store_ids
                .epic
                .as_ref()
                .is_some_and(|epic| epic.namespace.eq_ignore_ascii_case(namespace))
        })
    };
    let by_app = |game: &Game| {
        steam_app.is_some_and(|app| game.store_ids.steam.as_deref() == Some(&app.to_string()))
    };
    let index = games
        .iter()
        .position(by_namespace)
        .or_else(|| games.iter().position(by_app))?;
    let game = games.swap_remove(index);
    Some(Entry {
        name: game.name,
        status: game.status,
        anti_cheats: game.anticheats,
        notes: game
            .notes
            .into_iter()
            .filter_map(|note| {
                let mut parts = note.into_iter().map(Option::unwrap_or_default);
                let text = parts.next().filter(|text| !text.trim().is_empty())?;
                Some(Note {
                    text,
                    link: parts.next().filter(|link| link.starts_with("https://")),
                })
            })
            .collect(),
        latest: game.updates.into_iter().last().map(|update| {
            (
                Note {
                    text: update.name,
                    link: Some(update.reference).filter(|link| link.starts_with("https://")),
                },
                update.date,
            )
        }),
        slug: game.slug,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    name: String,
    slug: String,
    status: Status,
    #[serde(default)]
    anticheats: Vec<String>,
    /// Each a text and a link, `null` when it has none.
    #[serde(default)]
    notes: Vec<Vec<Option<String>>>,
    #[serde(default)]
    updates: Vec<Update>,
    #[serde(default)]
    store_ids: StoreIds,
}

#[derive(Deserialize, Default)]
struct StoreIds {
    steam: Option<String>,
    epic: Option<Epic>,
}

#[derive(Deserialize)]
struct Epic {
    namespace: String,
}

#[derive(Deserialize)]
struct Update {
    name: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    reference: String,
}

/// The list, skipping an entry that does not read rather than losing every
/// other game's status to it.
fn parse(text: &str) -> serde_json::Result<Vec<Game>> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(text)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect())
}

fn cache_file() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/areweanticheatyet.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Entries in the list's own shape (2026-11-09), cut down.
    const LIST: &str = r#"[
      {"url": "https://rust.facepunch.com/", "name": "Rust", "logo": "", "native": false,
       "status": "Denied", "reference": "https://example.org/rust", "anticheats": ["Easy Anti-Cheat", "VAC"],
       "notes": [], "slug": "rust", "storeIds": {"steam": "252490"},
       "updates": [{"name": "Denied", "date": "Sep 1, 2022", "reference": "https://example.org/a"},
                   {"name": "Well, maybe no support for Linux after all",
                    "date": "Sep 26, 2022, 8:29 PM GMT+2", "reference": "https://example.org/b"}],
       "dateChanged": "2024-08-11T23:02:07.337Z"},
      {"url": "", "name": "Fortnite", "logo": "", "native": false, "status": "Denied",
       "reference": "", "anticheats": ["Easy Anti-Cheat"],
       "notes": [["Works on Xbox-Cloud", "https://example.org/cloud"], ["No link", null], [""], [null]],
       "slug": "fortnite", "storeIds": {"epic": {"namespace": "fn", "slug": "fortnite"}},
       "updates": [], "dateChanged": "2022-01-01T00:00:00.000Z"},
      {"name": "Elden Ring", "status": "Supported", "anticheats": ["Easy Anti-Cheat"],
       "slug": "elden-ring", "storeIds": {"steam": "1245620"}, "notes": [], "updates": []},
      {"name": "Later", "status": "Sideways", "slug": "later", "storeIds": {}}
    ]"#;

    fn games() -> Vec<Game> {
        parse(LIST).unwrap()
    }

    #[test]
    fn skips_an_entry_that_does_not_read() {
        let text = r#"[
          {"name": "Good", "status": "Supported", "slug": "good", "storeIds": {}},
          {"status": "Supported", "slug": "no-name"},
          {"name": "Bad status", "status": 12, "slug": "bad", "storeIds": {}}
        ]"#;
        let games = parse(text).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Good");
    }

    #[test]
    fn finds_a_game_by_steam_app_or_epic_namespace() {
        let rust = lookup(games(), Some(252_490), None).unwrap();
        assert_eq!(rust.name, "Rust");
        assert_eq!(rust.status, Status::Denied);
        assert_eq!(rust.anti_cheats, ["Easy Anti-Cheat", "VAC"]);
        assert_eq!(rust.url(), "https://areweanticheatyet.com/game/rust");
        let (latest, date) = rust.latest.unwrap();
        assert_eq!(latest.text, "Well, maybe no support for Linux after all");
        assert_eq!(latest.link.as_deref(), Some("https://example.org/b"));
        assert_eq!(date, "Sep 26, 2022, 8:29 PM GMT+2");

        let fortnite = lookup(games(), None, Some("FN")).unwrap();
        assert_eq!(fortnite.name, "Fortnite");
        assert_eq!(
            fortnite.notes,
            [
                Note {
                    text: "Works on Xbox-Cloud".into(),
                    link: Some("https://example.org/cloud".into()),
                },
                Note {
                    text: "No link".into(),
                    link: None,
                },
            ],
            "an empty note dropped"
        );
        assert!(fortnite.latest.is_none());

        // The namespace wins over a Steam app id matching another game.
        let both = lookup(games(), Some(1_245_620), Some("fn")).unwrap();
        assert_eq!(both.name, "Fortnite");
        assert!(lookup(games(), Some(1), Some("other")).is_none());
        assert!(lookup(games(), None, None).is_none());
    }

    /// Run with `XDG_CACHE_HOME` and `XDG_DATA_HOME` in a scratch folder,
    /// the data one holding `zendo/stores/epic/metadata/Fortnite.json`.
    #[test]
    #[ignore = "downloads the real list"]
    fn finds_games_in_the_real_list() {
        let rust = for_game("steam:252490", Some(252_490)).unwrap().unwrap();
        assert_eq!((rust.name.as_str(), rust.status), ("Rust", Status::Denied));
        let fortnite = for_game("epic:Fortnite", None).unwrap().unwrap();
        assert_eq!(fortnite.status, Status::Denied);
        assert!(for_game("gog:1", None).unwrap().is_none());
    }

    #[test]
    fn warns_when_online_play_will_not_work() {
        let later = games()
            .into_iter()
            .find(|game| game.name == "Later")
            .unwrap();
        assert_eq!(later.status, Status::Unknown, "a new status still reads");
        assert!(Status::Denied.warns());
        assert!(Status::Broken.warns());
        assert!(Status::Unknown.warns());
        assert!(!Status::Supported.warns());
        assert!(!Status::Running.warns());
        assert!(!Status::Planned.warns());
    }
}
