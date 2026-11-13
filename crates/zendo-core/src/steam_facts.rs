//! What Steam's store says about a game, for any game the Steam app id is
//! known for: its description, who made it, genres and features, the
//! Metacritic score and screenshots. Asked once for each app id and kept
//! in `$XDG_CACHE_HOME/zendo/steam_facts.json` for a week; the first
//! screenshots' small images are kept as files in
//! `$XDG_CACHE_HOME/zendo/screenshots/`, as the window draws images from
//! files.
//!
//! The app id is the one the user gave the game, else a Steam game's own,
//! else the one `ProtonDB`'s lookup found ([`app_for`]).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::owned::{self, StoreDetails};
use crate::{http, paths, protondb, time};

const VERSION: u32 = 1;
const WEEK: u64 = 7 * 24 * 60 * 60;
/// How many screenshots are downloaded for a game.
pub const MAX_SHOTS: usize = 8;

/// A game's Steam store facts, with its screenshots as files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub details: StoreDetails,
    pub shots: Vec<Shot>,
}

/// A screenshot: the small image on disk, and the full-size one's address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub thumb: PathBuf,
    pub full: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Kept {
    #[serde(default)]
    version: u32,
    /// Unix seconds.
    asked: u64,
    /// `None`: the store has no page for the app.
    details: Option<StoreDetails>,
}

impl Kept {
    fn fresh(&self, now: u64) -> bool {
        self.version == VERSION && now.saturating_sub(self.asked) < WEEK
    }
}

/// The Steam app id for a game without asking the network: the one the
/// user gave it, else a Steam game's own, else the one `ProtonDB`'s lookup
/// found for it before. `None` when none is known (yet).
#[must_use]
pub fn app_for(id: &str, chosen: Option<u32>) -> Option<u32> {
    if chosen.is_some() {
        return chosen;
    }
    if let Some(app) = id.strip_prefix("steam:") {
        return app.parse().ok();
    }
    protondb::cached_app(id, chosen)
}

/// What Steam's store says about the app, from the cache or asked anew:
/// `None` when the store has no page for it. It blocks on the network; the
/// window calls it on a thread.
///
/// # Errors
///
/// When Steam's store cannot be reached or answers oddly; nothing is kept
/// then, so the next look asks again.
pub fn details(app: u32) -> anyhow::Result<Option<StoreDetails>> {
    let file = paths::cache_home().map(|dir| dir.join("zendo/steam_facts.json"));
    let key = app.to_string();
    let kept: HashMap<String, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let now = time::now();
    if let Some(entry) = kept.get(&key).filter(|entry| entry.fresh(now)) {
        return Ok(entry.details.clone());
    }
    let found = owned::steam_details(&key).context("could not ask Steam's store")?;
    if let Some(file) = file {
        // Read again under the lock: another writer may have added an
        // entry while the network answered.
        let _ = paths::locked(&file, || {
            let mut kept: HashMap<String, Kept> = fs::read(&file)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            kept.insert(
                key.clone(),
                Kept {
                    version: VERSION,
                    asked: now,
                    details: found.clone(),
                },
            );
            Ok(paths::write_atomic(&file, &serde_json::to_vec(&kept)?)?)
        });
    }
    Ok(found)
}

/// [`details`] with the first screenshots' small images downloaded (or
/// found again in the cache folder). One that cannot be downloaded is left
/// out.
///
/// # Errors
///
/// As [`details`].
pub fn for_app(app: u32) -> anyhow::Result<Option<Facts>> {
    let Some(details) = details(app)? else {
        return Ok(None);
    };
    let shots = shots_of(&details);
    Ok(Some(Facts { details, shots }))
}

fn shots_of(details: &StoreDetails) -> Vec<Shot> {
    let wanted = &details.screenshots[..details.screenshots.len().min(MAX_SHOTS)];
    let Some(dir) = paths::cache_home().map(|dir| dir.join("zendo/screenshots")) else {
        return Vec::new();
    };
    if wanted.is_empty() || fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }
    let mut files: Vec<Option<PathBuf>> = vec![None; wanted.len()];
    std::thread::scope(|scope| {
        for (shot, file) in wanted.iter().zip(&mut files) {
            let dir = &dir;
            scope.spawn(move || *file = http::cached_image(dir, &shot.thumb));
        }
    });
    wanted
        .iter()
        .zip(files)
        .filter_map(|(shot, file)| {
            Some(Shot {
                thumb: file?,
                full: shot.full.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_kept_for_a_week_in_this_format() {
        let kept = |version, asked| Kept {
            version,
            asked,
            details: None,
        };
        assert!(kept(VERSION, 1000).fresh(1000 + WEEK - 1));
        assert!(!kept(VERSION, 1000).fresh(1000 + WEEK));
        assert!(!kept(VERSION - 1, 1000).fresh(1000));
    }

    #[test]
    fn finds_the_app_without_asking() {
        // The user's id comes first, even for a Steam game's id.
        assert_eq!(app_for("steam:70", Some(620)), Some(620));
        assert_eq!(app_for("steam:70", None), Some(70));
        assert_eq!(app_for("steam:nonsense", None), None);
        // Nothing is known of a game never looked up.
        assert_eq!(app_for("test:never-looked-up-38", None), None);
    }
}
