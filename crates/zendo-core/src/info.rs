//! What a store says about one game, asked of its engine: how much there is
//! to download and how much room it takes, and the latest version. For a
//! game Zendo installed, the same answer says whether an update waits.
//!
//! - legendary: `info <app> --json`, whose `manifest` has `download_size`
//!   and `disk_size`, `game` the latest version and `install` the one
//!   installed.
//! - gogdl: `info <id>`, sizes per language and the latest build.
//! - nile: `install <id> --info --json`, the files' total size only; its
//!   updates come from `list-updates --json`, a list of ids.

use std::collections::HashMap;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::downloads::{self, GOG_LANGUAGE};
use crate::stores::Store;
use crate::{Game, Launch, Source};

/// Sizes are in bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameInfo {
    pub download_size: Option<u64>,
    pub install_size: Option<u64>,
    pub version: Option<String>,
    /// When the latest build came out, `YYYY-MM-DD`; GOG says, the others
    /// do not.
    pub updated: Option<String>,
}

/// Asks the store about a game, for the platform Zendo would install or
/// has installed.
///
/// # Errors
///
/// When no one is signed in, the engine fails or says something unexpected.
pub fn fetch(store: Store, app: &str, platform: &str) -> anyhow::Result<GameInfo> {
    match store {
        Store::Gog => {
            let info = gog(app, platform)?;
            Ok(GameInfo {
                download_size: nonzero(info.sizes(|size| size.download_size)),
                install_size: nonzero(info.sizes(|size| size.disk_size)),
                updated: info.published(),
                version: info.version_name,
            })
        }
        Store::Epic => {
            let info = legendary(app, platform)?;
            let manifest = info.manifest.unwrap_or_default();
            Ok(GameInfo {
                download_size: manifest.download_size,
                install_size: manifest.disk_size,
                version: info.game.and_then(|game| game.version),
                updated: None,
            })
        }
        Store::Amazon => {
            #[derive(Deserialize)]
            struct Info {
                download_size: Option<u64>,
            }
            let output = store.run_checked(&["install", app, "--info", "--json"])?;
            let info: Info = serde_json::from_str(last_json(&output.stdout)?)
                .context("could not read what nile said about the game")?;
            // nile downloads the files as they are, so both sizes match.
            Ok(GameInfo {
                download_size: info.download_size,
                install_size: info.download_size,
                version: None,
                updated: None,
            })
        }
    }
}

/// Whether a game Zendo installed has a newer version in its store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    UpToDate,
    Available { version: Option<String> },
}

/// Asks the store whether a game Zendo installed has an update.
///
/// # Errors
///
/// When the game is not Zendo's, no one is signed in or the engine fails.
pub fn check_update(game: &Game) -> anyhow::Result<Update> {
    let Launch::Engine { app, .. } = &game.launch else {
        bail!("{} was not installed by Zendo", game.title);
    };
    let platform = game.details.platform.as_deref().unwrap_or("Windows");
    match game.source {
        Source::Epic => {
            let info = legendary(app, platform)?;
            let latest = info.game.and_then(|game| game.version);
            let installed = info
                .install
                .and_then(|install| install.version)
                .or_else(|| game.details.version.clone());
            Ok(compare(installed.as_deref(), latest))
        }
        Source::Gog => {
            let info = gog(app, platform)?;
            // The build tells versions apart where the names may not.
            match (downloads::gog_build(app), &info.build_id) {
                (Some(installed), Some(latest)) if installed == *latest => Ok(Update::UpToDate),
                (Some(_), Some(_)) => Ok(Update::Available {
                    version: info.version_name,
                }),
                _ => Ok(compare(game.details.version.as_deref(), info.version_name)),
            }
        }
        Source::Amazon => {
            let output = Store::Amazon.run(&["list-updates", "--json"])?;
            let ids: Vec<String> = serde_json::from_str(last_json(&output.stdout)?)
                .context("could not read nile's list of updates")?;
            Ok(if ids.iter().any(|id| id == app) {
                Update::Available { version: None }
            } else {
                Update::UpToDate
            })
        }
        _ => bail!("{} has no store to ask", game.title),
    }
}

fn compare(installed: Option<&str>, latest: Option<String>) -> Update {
    match (installed, latest) {
        (Some(installed), Some(latest)) if installed != latest => Update::Available {
            version: Some(latest),
        },
        _ => Update::UpToDate,
    }
}

fn nonzero(size: u64) -> Option<u64> {
    (size > 0).then_some(size)
}

/// The last line of an engine's output that is JSON: they log to the same
/// stream on some versions.
fn last_json(stdout: &[u8]) -> anyhow::Result<&str> {
    let text = std::str::from_utf8(stdout).context("the engine wrote something unreadable")?;
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with('{') || line.starts_with('['))
        .context("the engine said nothing about the game")
}

#[derive(Debug, Default, Deserialize)]
struct LegendaryInfo {
    game: Option<LegendaryVersion>,
    install: Option<LegendaryVersion>,
    manifest: Option<LegendaryManifest>,
}

#[derive(Debug, Default, Deserialize)]
struct LegendaryVersion {
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LegendaryManifest {
    disk_size: Option<u64>,
    download_size: Option<u64>,
}

fn legendary(app: &str, platform: &str) -> anyhow::Result<LegendaryInfo> {
    let output = Store::Epic.run_checked(&["info", app, "--json", "--platform", platform])?;
    serde_json::from_str(last_json(&output.stdout)?)
        .context("could not read what legendary said about the game")
}

/// What `gogdl info` says about a game.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct GogInfo {
    pub folder_name: Option<String>,
    #[serde(rename = "versionName")]
    pub version_name: Option<String>,
    #[serde(rename = "buildId")]
    pub build_id: Option<String>,
    #[serde(default)]
    size: HashMap<String, GogSize>,
    #[serde(default)]
    builds: GogBuilds,
}

#[derive(Debug, Default, Deserialize)]
struct GogSize {
    #[serde(default)]
    download_size: u64,
    #[serde(default)]
    disk_size: u64,
}

#[derive(Debug, Default, Deserialize)]
struct GogBuilds {
    #[serde(default)]
    items: Vec<GogBuild>,
}

#[derive(Debug, Default, Deserialize)]
struct GogBuild {
    build_id: Option<String>,
    date_published: Option<String>,
}

impl GogInfo {
    /// The files every language shares plus the ones for Zendo's.
    fn sizes(&self, pick: impl Fn(&GogSize) -> u64) -> u64 {
        ["*", GOG_LANGUAGE]
            .iter()
            .filter_map(|key| self.size.get(*key))
            .map(pick)
            .sum()
    }

    pub(crate) fn disk_size(&self) -> u64 {
        self.sizes(|size| size.disk_size)
    }

    /// The day the build gogdl would install was published.
    fn published(&self) -> Option<String> {
        self.builds
            .items
            .iter()
            .find(|build| build.build_id.is_some() && build.build_id == self.build_id)
            .or_else(|| self.builds.items.first())
            .and_then(|build| build.date_published.as_deref())
            .and_then(|date| date.get(..10))
            .map(str::to_owned)
    }
}

pub(crate) fn gog(app: &str, platform: &str) -> anyhow::Result<GogInfo> {
    let output =
        Store::Gog.run_checked(&["info", app, "--platform", platform, "--lang", GOG_LANGUAGE])?;
    serde_json::from_str(last_json(&output.stdout)?)
        .context("could not read what gogdl said about the game")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gogdls_info() {
        let stdout = br#"[API] INFO: Getting Dependencies repository
{"size": {"*": {"download_size": 10, "disk_size": 100}, "en-US": {"download_size": 1, "disk_size": 20}, "de-DE": {"disk_size": 7}}, "dlcs": [], "buildId": "5", "languages": ["en-US"], "folder_name": "Game Folder", "versionName": "1.2", "builds": {"items": [{"build_id": "4", "date_published": "2025-01-01T00:00:00+0000"}, {"build_id": "5", "date_published": "2026-04-14T06:27:22+0000"}]}}"#;
        let info: GogInfo = serde_json::from_str(last_json(stdout).unwrap()).unwrap();
        assert_eq!(info.disk_size(), 120);
        assert_eq!(info.sizes(|size| size.download_size), 11);
        assert_eq!(info.folder_name.as_deref(), Some("Game Folder"));
        assert_eq!(info.version_name.as_deref(), Some("1.2"));
        assert_eq!(info.published().as_deref(), Some("2026-04-14"));
    }

    #[test]
    fn reads_legendarys_info() {
        let info: LegendaryInfo = serde_json::from_str(
            r#"{"game": {"app_name": "Sugar", "version": "1.1"}, "install": {"version": "1.0"},
                "manifest": {"disk_size": 300, "download_size": 120, "type": "binary"}}"#,
        )
        .unwrap();
        assert_eq!(info.manifest.as_ref().unwrap().disk_size, Some(300));
        assert_eq!(info.game.unwrap().version.as_deref(), Some("1.1"));
        // Not installed, legendary sets the section to null.
        let info: LegendaryInfo = serde_json::from_str(
            r#"{"game": {"version": "1"}, "install": null, "manifest": null}"#,
        )
        .unwrap();
        assert!(info.install.is_none());
    }

    #[test]
    fn tells_an_update_from_the_same_version() {
        assert_eq!(compare(Some("1.0"), Some("1.0".into())), Update::UpToDate);
        assert_eq!(
            compare(Some("1.0"), Some("1.1".into())),
            Update::Available {
                version: Some("1.1".into())
            }
        );
        assert_eq!(compare(None, Some("1.1".into())), Update::UpToDate);
    }
}
