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
    /// The DLC the user owns for it, by title.
    pub dlc: Vec<Dlc>,
}

/// A DLC the user owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dlc {
    /// What the engine installs it by: legendary's app name, GOG's product
    /// id. An Epic DLC that comes with the game has its catalog id.
    pub id: String,
    pub title: String,
    /// False for an Epic DLC that comes with the game, nothing to install.
    pub installable: bool,
    /// GOG's Windows builds say; the others do not.
    pub download_size: Option<u64>,
    pub install_size: Option<u64>,
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
                dlc: info.dlc(),
                version: info.version_name,
            })
        }
        Store::Epic => {
            let info = legendary(app, platform)?;
            let manifest = info.manifest.unwrap_or_default();
            let game = info.game.unwrap_or_default();
            Ok(GameInfo {
                download_size: manifest.download_size,
                install_size: manifest.disk_size,
                dlc: game.dlc(),
                version: game.version,
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
                ..GameInfo::default()
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
    game: Option<LegendaryGame>,
    install: Option<LegendaryVersion>,
    manifest: Option<LegendaryManifest>,
}

#[derive(Debug, Default, Deserialize)]
struct LegendaryGame {
    version: Option<String>,
    #[serde(default)]
    owned_dlc: Vec<LegendaryDlc>,
}

/// One of `owned_dlc`: `app_name` is null for one that comes with the game,
/// and `installable` is its release info, a list, or null.
#[derive(Debug, Deserialize)]
struct LegendaryDlc {
    app_name: Option<String>,
    title: Option<String>,
    installable: Option<serde_json::Value>,
    id: Option<String>,
}

impl LegendaryGame {
    fn dlc(&self) -> Vec<Dlc> {
        let mut dlc: Vec<Dlc> = self
            .owned_dlc
            .iter()
            .filter_map(|item| {
                let installable = item.app_name.is_some()
                    && item
                        .installable
                        .as_ref()
                        .is_some_and(|info| !info.is_null() && info != &serde_json::json!([]));
                let id = item.app_name.clone().or_else(|| item.id.clone())?;
                Some(Dlc {
                    title: item.title.clone().unwrap_or_else(|| id.clone()),
                    id,
                    installable,
                    download_size: None,
                    install_size: None,
                })
            })
            .collect();
        sort_dlc(&mut dlc);
        dlc
    }
}

fn sort_dlc(dlc: &mut [Dlc]) {
    dlc.sort_by_key(|item| item.title.to_lowercase());
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
    #[serde(default)]
    dlcs: Vec<GogDlc>,
}

/// One of `dlcs`: the id is a string in GOG's newer builds and a number in
/// its Linux installers; `size` is there for Windows builds, per language
/// as for the game.
#[derive(Debug, Deserialize)]
struct GogDlc {
    id: serde_json::Value,
    title: Option<String>,
    #[serde(default)]
    size: HashMap<String, GogSize>,
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

/// The files every language shares plus the ones for Zendo's.
fn language_sizes(size: &HashMap<String, GogSize>, pick: impl Fn(&GogSize) -> u64) -> u64 {
    ["*", GOG_LANGUAGE]
        .iter()
        .filter_map(|key| size.get(*key))
        .map(pick)
        .fold(0, u64::saturating_add)
}

impl GogInfo {
    fn sizes(&self, pick: impl Fn(&GogSize) -> u64) -> u64 {
        language_sizes(&self.size, pick)
    }

    pub(crate) fn dlc(&self) -> Vec<Dlc> {
        let mut dlc: Vec<Dlc> = self
            .dlcs
            .iter()
            .filter_map(|item| {
                let id = match &item.id {
                    serde_json::Value::String(id) => id.clone(),
                    serde_json::Value::Number(id) => id.to_string(),
                    _ => return None,
                };
                Some(Dlc {
                    title: item.title.clone().unwrap_or_else(|| id.clone()),
                    id,
                    installable: true,
                    download_size: nonzero(language_sizes(&item.size, |size| size.download_size)),
                    install_size: nonzero(language_sizes(&item.size, |size| size.disk_size)),
                })
            })
            .collect();
        sort_dlc(&mut dlc);
        dlc
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
    fn reads_the_owned_dlc() {
        let info: LegendaryInfo = serde_json::from_str(
            r#"{"game": {"version": "1", "owned_dlc": [
                {"app_name": "Wings", "title": "Wings Pack", "installable": [{"appId": "Wings"}], "id": "c1"},
                {"app_name": null, "title": "A Skin", "installable": null, "id": "c2"}]}}"#,
        )
        .unwrap();
        let dlc = info.game.unwrap().dlc();
        let titles: Vec<(&str, &str, bool)> = dlc
            .iter()
            .map(|item| (item.id.as_str(), item.title.as_str(), item.installable))
            .collect();
        assert_eq!(
            titles,
            [("c2", "A Skin", false), ("Wings", "Wings Pack", true)]
        );

        let info: GogInfo = serde_json::from_str(
            r#"{"dlcs": [{"title": "Soundtrack", "id": "17", "size": {"*": {"download_size": 5, "disk_size": 9}, "en-US": {"download_size": 1, "disk_size": 1}}},
                         {"title": "Bonus", "id": 18}]}"#,
        )
        .unwrap();
        let dlc = info.dlc();
        assert_eq!(dlc[0].id, "18");
        assert_eq!(dlc[0].download_size, None);
        assert_eq!(dlc[1].title, "Soundtrack");
        assert_eq!(
            (dlc[1].download_size, dlc[1].install_size),
            (Some(6), Some(10))
        );
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
