//! umu's id for a store game, which picks the protonfixes it applies: asked
//! of the umu database by the store's id (`umu_api.php?store=gog&codename=
//! <id>`, answered with `[{"title": …, "umu_id": "umu-20920"}]`), and when
//! it has no entry for that store, by title. The title search matches part
//! of a title ("Cat" finds four games), so each id it gives is looked up
//! (`umu_api.php?umu_id=<id>`, which names its titles) and taken only when
//! one of its titles is the game's, letters and digits compared, case
//! aside. Kept in `$XDG_CACHE_HOME/zendo/umu.json`. A game the database
//! does not know runs as `umu-default`, with only umu's general fixes, and
//! is asked about again after a week.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::protondb::plain;
use crate::{paths, time};

const API: &str = "https://umu.openwinecomponents.org/umu_api.php";
const UNKNOWN: &str = "umu-default";
const WEEK: u64 = 7 * 24 * 60 * 60;
/// How many ids a title search may give before Zendo stops looking them
/// up: a short title matching many is no match.
const CANDIDATES: usize = 5;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    /// Unix seconds.
    asked: u64,
    id: Option<String>,
    /// Asked by title too; entries from before that are asked again.
    #[serde(default)]
    titled: bool,
}

/// The store as umu's database names it.
#[must_use]
pub fn store_name(source_key: &str) -> &str {
    match source_key {
        "epic" => "egs",
        other => other,
    }
}

/// umu's id for the game, from the cache or the database; `umu-default`
/// when the database does not know it or cannot be reached.
#[must_use]
pub fn game_id(store: &str, app: &str, title: &str) -> String {
    let file = paths::cache_home().map(|dir| dir.join("zendo/umu.json"));
    let mut kept: HashMap<String, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let key = format!("{store}:{app}");
    let now = time::now();
    if let Some(entry) = kept.get(&key)
        && (entry.id.is_some() || (entry.titled && now.saturating_sub(entry.asked) < WEEK))
    {
        return entry.id.clone().unwrap_or_else(|| UNKNOWN.to_owned());
    }
    // Offline is no answer: asked again next time.
    let agent = agent();
    // A game added by hand has no store to ask.
    let by_store = if store == "local" {
        Ok(None)
    } else {
        ask(&agent, store, app)
    };
    let asked = by_store.and_then(|id| match id {
        Some(id) => Ok(Some(id)),
        None => by_title(&agent, title),
    });
    let Ok(id) = asked else {
        return UNKNOWN.to_owned();
    };
    kept.insert(
        key,
        Kept {
            asked: now,
            id: id.clone(),
            titled: true,
        },
    );
    if let Some(file) = file
        && let Ok(json) = serde_json::to_vec(&kept)
    {
        let _ = paths::write_atomic(&file, &json);
    }
    id.unwrap_or_else(|| UNKNOWN.to_owned())
}

#[derive(Deserialize)]
struct Entry {
    title: Option<String>,
    umu_id: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into()
}

fn entries(agent: &ureq::Agent, query: &[(&str, &str)]) -> anyhow::Result<Vec<Entry>> {
    let mut request = agent.get(API);
    for (key, value) in query {
        request = request.query(*key, *value);
    }
    let body = request.call()?.into_body().read_to_string()?;
    Ok(serde_json::from_str(&body)?)
}

/// The id the database has for the store's own id.
fn ask(agent: &ureq::Agent, store: &str, app: &str) -> anyhow::Result<Option<String>> {
    let found = entries(agent, &[("store", store), ("codename", app)])?;
    Ok(found.into_iter().find_map(|entry| entry.umu_id))
}

/// The id of the game with exactly this title, from any store.
fn by_title(agent: &ureq::Agent, title: &str) -> anyhow::Result<Option<String>> {
    let wanted = plain(title);
    if wanted.is_empty() {
        return Ok(None);
    }
    let mut ids: Vec<String> = entries(agent, &[("title", title)])?
        .into_iter()
        .filter_map(|entry| entry.umu_id)
        .collect();
    ids.dedup();
    if ids.len() > CANDIDATES {
        return Ok(None);
    }
    for id in ids {
        let named = entries(agent, &[("umu_id", &id)])?;
        if named
            .iter()
            .filter_map(|entry| entry.title.as_deref())
            .any(|name| plain(name) == wanted)
        {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod online {
    /// Asks the umu database; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn finds_games_by_their_exact_title() {
        let agent = super::agent();
        for (title, wanted) in [
            ("Cat Quest", Some("umu-593280")),
            ("10 Second Ninja X", Some("umu-435790")),
            ("Cat", None),
            ("Home", None),
            ("Costume Quest", None),
        ] {
            let found = super::by_title(&agent, title).unwrap();
            assert_eq!(found.as_deref(), wanted, "{title}");
        }
    }
}
