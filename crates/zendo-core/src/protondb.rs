//! How well a game runs through Proton, from [ProtonDB](https://www.protondb.com)'s
//! summary of its players' reports (`/api/v1/reports/summaries/<appid>.json`,
//! answered with `{"tier": "gold", "total": 37, …}`, a 404 when it has
//! none). `ProtonDB` knows games by Steam app id:
//!
//! - a Steam game has its own;
//! - a store game takes umu's id for it when that is a Steam app id
//!   (`umu-435790`);
//! - any other game the Steam store's search finds by its exact title
//!   (letters and digits compared, case aside), or none.
//!
//! `ProtonDB`'s own pages call a game with a Linux build Native, from
//! Steam's platforms for it, not from the reports; Zendo asks Steam the same
//! (`/api/appdetails?appids=<id>&filters=platforms`).
//!
//! Answers are kept in `$XDG_CACHE_HOME/zendo/protondb.json` and asked
//! again after a week, as ratings move.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{paths, time, umu};

const SUMMARY: &str = "https://www.protondb.com/api/v1/reports/summaries";
const SEARCH: &str = "https://store.steampowered.com/api/storesearch/";
const DETAILS: &str = "https://store.steampowered.com/api/appdetails";
/// What the cache's entries hold; older entries are asked again.
const VERSION: u32 = 2;
const WEEK: u64 = 7 * 24 * 60 * 60;

/// A game's standing on `ProtonDB`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rating {
    /// The Steam app id `ProtonDB` knows it by.
    pub app: u32,
    /// `platinum`, `gold`, `silver`, `bronze`, `borked` or `pending`;
    /// `None` when no one reported on it yet.
    pub tier: Option<String>,
    pub reports: u32,
    /// Steam has a Linux build of it.
    #[serde(default)]
    pub native: bool,
}

impl Rating {
    /// The game's page on `ProtonDB`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://www.protondb.com/app/{}", self.app)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    #[serde(default)]
    version: u32,
    /// Unix seconds.
    asked: u64,
    rating: Option<Rating>,
}

/// The game's rating, from the cache or asked anew: `None` when no Steam
/// app id could be found for it. It blocks on the network; the window
/// calls it on a thread.
///
/// # Errors
///
/// When `ProtonDB` or Steam cannot be reached or answer oddly; nothing is
/// kept then, so the next look asks again.
pub fn rating(id: &str, title: &str) -> anyhow::Result<Option<Rating>> {
    let file = paths::cache_home().map(|dir| dir.join("zendo/protondb.json"));
    let mut kept: HashMap<String, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let now = time::now();
    if let Some(entry) = kept.get(id)
        && entry.version == VERSION
        && now.saturating_sub(entry.asked) < WEEK
    {
        return Ok(entry.rating.clone());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        // ProtonDB answers a missing game with a 404, not an error.
        .http_status_as_error(false)
        .build()
        .into();
    let rating = match steam_app(&agent, id, title)? {
        Some(app) => Some(Rating {
            native: on_linux(&agent, app)?,
            ..summary(&agent, app)?
        }),
        None => None,
    };
    kept.insert(
        id.to_owned(),
        Kept {
            version: VERSION,
            asked: now,
            rating: rating.clone(),
        },
    );
    if let Some(file) = file
        && let Ok(json) = serde_json::to_vec(&kept)
    {
        let _ = paths::write_atomic(&file, &json);
    }
    Ok(rating)
}

/// The Steam app id for a Zendo game id and title.
fn steam_app(agent: &ureq::Agent, id: &str, title: &str) -> anyhow::Result<Option<u32>> {
    let (store, app) = id.split_once(':').unwrap_or(("", id));
    if store == "steam" {
        return Ok(app.parse().ok());
    }
    if matches!(store, "epic" | "gog" | "amazon")
        && let Some(app) = umu::game_id(umu::store_name(store), app, title)
            .strip_prefix("umu-")
            .and_then(|number| number.parse().ok())
    {
        return Ok(Some(app));
    }
    search(agent, title)
}

/// The Steam app whose name is the title.
fn search(agent: &ureq::Agent, title: &str) -> anyhow::Result<Option<u32>> {
    #[derive(Deserialize)]
    struct Found {
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        id: u32,
        name: String,
        #[serde(rename = "type")]
        kind: String,
    }
    let mut response = agent
        .get(SEARCH)
        .query("term", title)
        .query("cc", "us")
        .query("l", "en")
        .call()
        .context("could not reach the Steam store")?;
    if !response.status().is_success() {
        anyhow::bail!("the Steam store answered {}", response.status());
    }
    let body = response.body_mut().read_to_string()?;
    let found: Found =
        serde_json::from_str(&body).context("the Steam store's search answered oddly")?;
    let wanted = plain(title);
    Ok(found
        .items
        .into_iter()
        .find(|item| item.kind == "app" && plain(&item.name) == wanted)
        .map(|item| item.id))
}

/// A title's letters and digits, lowercase, so `Bō: Path of the Teal
/// Lotus™` is the same as `BŌ PATH OF THE TEAL LOTUS`.
pub(crate) fn plain(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether Steam lists a Linux build of the app; false for an app it no
/// longer lists.
fn on_linux(agent: &ureq::Agent, app: u32) -> anyhow::Result<bool> {
    let mut response = agent
        .get(DETAILS)
        .query("appids", app.to_string())
        .query("filters", "platforms")
        .call()
        .context("could not reach the Steam store")?;
    if !response.status().is_success() {
        anyhow::bail!("the Steam store answered {}", response.status());
    }
    let body = response.body_mut().read_to_string()?;
    let details: serde_json::Value =
        serde_json::from_str(&body).context("the Steam store's details answered oddly")?;
    Ok(details
        .pointer(&format!("/{app}/data/platforms/linux"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false))
}

fn summary(agent: &ureq::Agent, app: u32) -> anyhow::Result<Rating> {
    #[derive(Deserialize)]
    struct Summary {
        tier: Option<String>,
        total: Option<u32>,
    }
    let mut response = agent
        .get(format!("{SUMMARY}/{app}.json"))
        .call()
        .context("could not reach ProtonDB")?;
    let status = response.status();
    if status == 404 {
        return Ok(Rating {
            app,
            tier: None,
            reports: 0,
            native: false,
        });
    }
    if !status.is_success() {
        anyhow::bail!("ProtonDB answered {status}");
    }
    let body = response.body_mut().read_to_string()?;
    let summary: Summary = serde_json::from_str(&body).context("ProtonDB answered oddly")?;
    Ok(Rating {
        app,
        tier: summary.tier.map(|tier| tier.to_lowercase()),
        reports: summary.total.unwrap_or(0),
        native: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_titles_by_their_letters_and_digits() {
        assert_eq!(
            plain("Bō: Path of the Teal Lotus™"),
            plain("BŌ PATH OF THE TEAL LOTUS")
        );
        assert_ne!(
            plain("Gunslugs 3 Rogue Tactics"),
            plain("Gunslugs : Rogue Tactics")
        );
        assert_ne!(plain("10 Second Ninja X"), plain("10 Second Ninja X Demo"));
    }

    #[test]
    fn links_to_the_games_page() {
        let rating = Rating {
            app: 435_790,
            tier: Some("platinum".into()),
            reports: 37,
            native: false,
        };
        assert_eq!(rating.url(), "https://www.protondb.com/app/435790");
    }
}

#[cfg(test)]
mod online {
    use super::*;

    /// Asks the real services; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn finds_real_games() {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        for (id, title) in [
            ("steam:785890", "Hexologic"),
            (
                "amazon:13a0d6e1-fce9-4b6c-9f3a-582a0f04b6b7",
                "10 Second Ninja X",
            ),
            ("gog:1882554410", "Bō: Path of the Teal Lotus"),
            ("gog:1299228602", "Gunslugs 3 Rogue Tactics"),
        ] {
            let app = steam_app(&agent, id, title).unwrap();
            let rating = app.map(|app| Rating {
                native: on_linux(&agent, app).unwrap(),
                ..summary(&agent, app).unwrap()
            });
            println!("{title}: {rating:?}");
        }
    }
}
