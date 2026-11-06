//! umu's id for a store game, which picks the protonfixes it applies: asked
//! of the umu database (`umu_api.php?store=gog&codename=<id>`, answered
//! with `[{"title": …, "umu_id": "umu-20920"}]`) and kept in
//! `$XDG_CACHE_HOME/zendo/umu.json`. A game the database does not know
//! runs as `umu-default`, with no fixes, and is asked about again after a
//! week.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{paths, time};

const API: &str = "https://umu.openwinecomponents.org/umu_api.php";
const UNKNOWN: &str = "umu-default";
const WEEK: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    /// Unix seconds.
    asked: u64,
    id: Option<String>,
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
pub fn game_id(store: &str, app: &str) -> String {
    let file = paths::cache_home().map(|dir| dir.join("zendo/umu.json"));
    let mut kept: HashMap<String, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let key = format!("{store}:{app}");
    let now = time::now();
    if let Some(entry) = kept.get(&key)
        && (entry.id.is_some() || now.saturating_sub(entry.asked) < WEEK)
    {
        return entry.id.clone().unwrap_or_else(|| UNKNOWN.to_owned());
    }
    // Offline is no answer: asked again next time.
    let Ok(id) = ask(store, app) else {
        return UNKNOWN.to_owned();
    };
    kept.insert(
        key,
        Kept {
            asked: now,
            id: id.clone(),
        },
    );
    if let Some(file) = file
        && let Ok(json) = serde_json::to_vec(&kept)
    {
        let _ = paths::write_atomic(&file, &json);
    }
    id.unwrap_or_else(|| UNKNOWN.to_owned())
}

fn ask(store: &str, app: &str) -> anyhow::Result<Option<String>> {
    #[derive(Deserialize)]
    struct Entry {
        umu_id: Option<String>,
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into();
    let body = agent
        .get(API)
        .query("store", store)
        .query("codename", app)
        .call()?
        .into_body()
        .read_to_string()?;
    let entries: Vec<Entry> = serde_json::from_str(&body)?;
    Ok(entries.into_iter().find_map(|entry| entry.umu_id))
}
