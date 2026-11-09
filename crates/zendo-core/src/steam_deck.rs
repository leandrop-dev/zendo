//! How a game does on the Steam Deck, from Valve's review of it, as
//! Heroic shows it: the store's
//! `saleaction/ajaxgetdeckappcompatibilityreport?nAppID=<id>`, answered
//! with `results.resolved_category` and `results.resolved_items`, or an
//! empty `results` list for a game Valve does not know.
//!
//! The category is 0 Unknown, 1 Unsupported, 2 Playable, 3 Verified
//! (Half-Life: Alyx 1, Fallout 3 2, Cyberpunk 2077 3, on 2026-11-09; the
//! names in that order are Steam's own). Each item is a test's result,
//! a `loc_token` such as `#SteamDeckVerified_TestResult_InterfaceTextIsLegible`
//! and a `display_type`: 4 passed, 3 a warning, 2 why it is unsupported,
//! 1 something to know.
//!
//! Games are known by Steam app id, found as for `ProtonDB`. Answers are
//! kept in `$XDG_CACHE_HOME/zendo/steamdeck.json` and asked again after a
//! week, as Valve reviews games again.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{paths, time};

const REPORT: &str = "https://store.steampowered.com/saleaction/ajaxgetdeckappcompatibilityreport";
/// What each `loc_token` starts with.
const TOKEN_PREFIX: &str = "#SteamDeckVerified_TestResult_";
const WEEK: u64 = 7 * 24 * 60 * 60;

/// Valve's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    Unsupported,
    Playable,
    Verified,
}

/// What a test's result says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Passed,
    Warning,
    /// Why the game is unsupported.
    Blocker,
    /// Something to know, neither good nor bad.
    Info,
}

/// One test's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub kind: Kind,
    /// The test, without Steam's prefix: `InterfaceTextIsNotLegible`.
    pub test: String,
}

/// Valve's review of a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// The Steam app id.
    pub app: u32,
    pub category: Category,
    pub notes: Vec<Note>,
}

impl Report {
    /// The game's page on the Steam store, which shows the review.
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://store.steampowered.com/app/{}", self.app)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Kept {
    /// Unix seconds.
    asked: u64,
    report: Option<Report>,
}

/// The review of the Steam app, from the cache or asked anew: `None` when
/// Valve has not reviewed it. It blocks on the network; the window calls
/// it on a thread.
///
/// # Errors
///
/// When the store cannot be reached or answers oddly; nothing is kept
/// then, so the next look asks again.
pub fn report(app: u32) -> anyhow::Result<Option<Report>> {
    let file = paths::cache_home().map(|dir| dir.join("zendo/steamdeck.json"));
    let mut kept: HashMap<u32, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let now = time::now();
    if let Some(entry) = kept.get(&app)
        && now.saturating_sub(entry.asked) < WEEK
    {
        return Ok(entry.report.clone());
    }
    let text = crate::http::agent(Duration::from_secs(10))
        .get(REPORT)
        .query("nAppID", app.to_string())
        .call()
        .context("could not ask Steam about the Steam Deck")?
        .into_body()
        .read_to_string()
        .context("Steam's Steam Deck report")?;
    let report = parse(app, &text)?;
    kept.insert(
        app,
        Kept {
            asked: now,
            report: report.clone(),
        },
    );
    if let Some(file) = file
        && let Ok(json) = serde_json::to_vec(&kept)
    {
        let _ = paths::write_atomic(&file, &json);
    }
    Ok(report)
}

fn parse(app: u32, text: &str) -> anyhow::Result<Option<Report>> {
    #[derive(Deserialize)]
    struct Answer {
        success: u8,
        results: Results,
    }
    /// An object for a game Valve knows, an empty list for one it does not.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Results {
        Known {
            resolved_category: u8,
            #[serde(default)]
            resolved_items: Vec<Item>,
        },
        /// Only an empty list reads as this.
        None([(); 0]),
    }
    #[derive(Deserialize)]
    struct Item {
        display_type: u8,
        loc_token: String,
    }

    let answer: Answer =
        serde_json::from_str(text).context("Steam's Steam Deck report is not valid")?;
    anyhow::ensure!(
        answer.success == 1,
        "Steam did not give the Steam Deck report"
    );
    let Results::Known {
        resolved_category,
        resolved_items,
    } = answer.results
    else {
        return Ok(None);
    };
    let category = match resolved_category {
        1 => Category::Unsupported,
        2 => Category::Playable,
        3 => Category::Verified,
        _ => return Ok(None),
    };
    let notes = resolved_items
        .into_iter()
        .filter_map(|item| {
            let kind = match item.display_type {
                4 => Kind::Passed,
                3 => Kind::Warning,
                2 => Kind::Blocker,
                1 => Kind::Info,
                _ => return None,
            };
            let name = item.loc_token.strip_prefix(TOKEN_PREFIX)?;
            Some(Note {
                kind,
                test: name.to_owned(),
            })
        })
        .collect();
    Ok(Some(Report {
        app,
        category,
        notes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_valves_review() {
        // Fallout 3's answer, 2026-11-09, cut down.
        let text = r##"{"success": 1, "results": {"appid": 22370, "resolved_category": 2,
            "resolved_items": [
              {"display_type": 3, "loc_token": "#SteamDeckVerified_TestResult_NativeResolutionNotDefault"},
              {"display_type": 4, "loc_token": "#SteamDeckVerified_TestResult_InterfaceTextIsLegible"},
              {"display_type": 1, "loc_token": "#SteamDeckVerified_TestResult_FirstTimeSetupRequiresActiveInternetConnection"},
              {"display_type": 9, "loc_token": "#SteamDeckVerified_TestResult_Later"},
              {"display_type": 3, "loc_token": "#Other_Token"}
            ], "steam_deck_blog_url": "", "search_id": null}}"##;
        let report = parse(22_370, text).unwrap().unwrap();
        assert_eq!(report.category, Category::Playable);
        assert_eq!(
            report.notes,
            [
                Note {
                    kind: Kind::Warning,
                    test: "NativeResolutionNotDefault".into()
                },
                Note {
                    kind: Kind::Passed,
                    test: "InterfaceTextIsLegible".into()
                },
                Note {
                    kind: Kind::Info,
                    test: "FirstTimeSetupRequiresActiveInternetConnection".into()
                },
            ],
            "unknown kinds and tokens left out"
        );
        assert_eq!(report.url(), "https://store.steampowered.com/app/22370");

        let alyx = r##"{"success": 1, "results": {"resolved_category": 1, "resolved_items": [
            {"display_type": 2, "loc_token": "#SteamDeckVerified_TestResult_SteamOSDoesNotSupport_VR"}]}}"##;
        let alyx = parse(546_560, alyx).unwrap().unwrap();
        assert_eq!(alyx.category, Category::Unsupported);
        assert_eq!(alyx.notes[0].kind, Kind::Blocker);
    }

    #[test]
    fn knows_no_review_for_a_game_valve_did_not_review() {
        assert!(
            parse(1, r#"{"success": 1, "results": []}"#)
                .unwrap()
                .is_none()
        );
        let unknown =
            r#"{"success": 1, "results": {"resolved_category": 0, "resolved_items": []}}"#;
        assert!(parse(1, unknown).unwrap().is_none());
        assert!(parse(1, r#"{"success": 2, "results": []}"#).is_err());
        assert!(parse(1, "<html>").is_err());
    }

    #[test]
    #[ignore = "asks the real Steam store"]
    fn asks_steam() {
        let cyberpunk = report(1_091_500).unwrap().unwrap();
        assert_eq!(cyberpunk.category, Category::Verified);
        assert!(report(9_999_999).unwrap().is_none());
    }
}
