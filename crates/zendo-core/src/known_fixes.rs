//! Known fixes for the Windows games Zendo installed from Epic, GOG and
//! Amazon: the winetricks components a game needs, and programs from its
//! folder to run in its prefix, from Heroic's list, applied before a game
//! starts as Heroic applies them the first time a game is set up.
//!
//! The list is Heroic's `known_fixes.json`, from the
//! `Heroic-Games-Launcher/known-fixes` repository, which states no licence:
//! Zendo downloads it as Heroic does, at most once a day, into
//! `$XDG_CACHE_HOME/zendo/known_fixes.json`, and never ships it. It is
//! keyed by store (`epic`, `gog`, `amazon`) and the store's id; each game
//! may have `winetricks` verbs, `runInPrefix` paths relative to the
//! game's folder, `notes` on each, and `envVariables`. The last are left
//! out: every one is `USE_FAKE_EPIC_EXE`, a switch of Heroic's own (it
//! starts some Rockstar games through a stand-in Epic launcher), which
//! the games themselves do not read.
//!
//! Each fix is tried once per game and prefix, kept in the prefix's
//! `.zendo-known-fixes.json`, so one added to the list later is applied
//! at the next start. The list comes from the network, so a verb is taken
//! only as a winetricks verb (as typed in the Wine prefix card), and a
//! program only inside the game's folder.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use serde::Deserialize;

use crate::prefix_tools::{self, Tool};
use crate::{Game, Launch, Source, launch, paths};

/// Where Heroic keeps the list.
const URL: &str =
    "https://raw.githubusercontent.com/Heroic-Games-Launcher/known-fixes/main/known_fixes.json";

/// How long the list kept is used before it is asked again.
const FRESH: Duration = Duration::from_hours(24);

/// The fixes tried in a prefix, by game id.
const DONE_FILE: &str = ".zendo-known-fixes.json";

/// One game's fixes, as the list has them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Fixes {
    #[serde(default)]
    pub winetricks: Vec<String>,
    #[serde(default, rename = "runInPrefix")]
    pub run_in_prefix: Vec<String>,
    /// Why each fix is there, by verb or path.
    #[serde(default)]
    pub notes: BTreeMap<String, String>,
}

impl Fixes {
    /// Each fix as Zendo keeps it tried: the verb, or `run:` and the path.
    fn keys(&self) -> impl Iterator<Item = String> + '_ {
        self.winetricks
            .iter()
            .cloned()
            .chain(self.run_in_prefix.iter().map(|path| format!("run:{path}")))
    }

    /// Whether there is anything to apply.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.winetricks.is_empty() && self.run_in_prefix.is_empty()
    }
}

/// How applying went: the fixes that failed, with why.
#[derive(Debug, Default)]
pub struct Applied {
    pub failed: Vec<String>,
}

/// The list's key for the game's store, and its id there; none for a game
/// Zendo did not install, or a native one.
fn store_key(game: &Game) -> Option<(&'static str, &str)> {
    let Launch::Engine { app, .. } = &game.launch else {
        return None;
    };
    if !launch::needs_proton(game) {
        return None;
    }
    let store = match game.source {
        Source::Epic => "epic",
        Source::Gog => "gog",
        Source::Amazon => "amazon",
        _ => return None,
    };
    Some((store, app))
}

/// Whether the game can have known fixes: a Windows game Zendo installed
/// from Epic, GOG or Amazon.
#[must_use]
pub fn applies(game: &Game) -> bool {
    store_key(game).is_some()
}

/// The game's fixes in the list kept; none when there is no list yet.
#[must_use]
pub fn for_game(game: &Game) -> Option<Fixes> {
    let (store, app) = store_key(game)?;
    let text = fs::read_to_string(cache_file()?).ok()?;
    find(&text, store, app)
}

fn find(text: &str, store: &str, app: &str) -> Option<Fixes> {
    let mut list: BTreeMap<String, BTreeMap<String, Fixes>> = serde_json::from_str(text).ok()?;
    list.get_mut(store)?
        .remove(app)
        .filter(|fixes| !fixes.is_empty())
}

/// Downloads the list again when the kept one is a day old or missing.
/// A failure keeps the old one, and is said.
///
/// # Errors
///
/// When the list cannot be downloaded or kept.
pub fn refresh() -> anyhow::Result<()> {
    let file = cache_file().context("no cache folder for the known fixes")?;
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
        .context("could not download Heroic's known fixes")?
        .into_body()
        .read_to_string()
        .context("Heroic's known fixes")?;
    // Kept only when it reads, so a broken answer leaves the old one.
    serde_json::from_str::<serde_json::Value>(&text)
        .context("Heroic's known fixes are not valid")?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    paths::write_atomic(&file, text.as_bytes())
        .with_context(|| format!("could not write {}", file.display()))
}

/// The game's fixes not tried yet in its prefix.
#[must_use]
pub fn pending(game: &Game) -> Option<Fixes> {
    let fixes = for_game(game)?;
    let prefix = launch::prefix(game)?;
    let tried = tried_in(&prefix, &game.id);
    let pending = Fixes {
        winetricks: fixes
            .winetricks
            .iter()
            .filter(|verb| !tried.contains(*verb))
            .cloned()
            .collect(),
        run_in_prefix: fixes
            .run_in_prefix
            .iter()
            .filter(|path| !tried.contains(&format!("run:{path}")))
            .cloned()
            .collect(),
        notes: fixes.notes,
    };
    (!pending.is_empty()).then_some(pending)
}

/// Applies `fixes` in the game's prefix, saying each to `report`, and
/// keeps them tried there, failed or not, so a game is not held at every
/// start (the Wine prefix card applies them again).
///
/// # Errors
///
/// When the prefix's record cannot be written.
pub fn apply(game: &Game, fixes: &Fixes, report: &mut dyn FnMut(&str)) -> anyhow::Result<Applied> {
    let mut applied = Applied::default();
    let verbs: Vec<String> = fixes
        .winetricks
        .iter()
        .filter(|verb| {
            let ok = prefix_tools::components(verb).is_ok();
            if !ok {
                applied
                    .failed
                    .push(format!("{verb}: not a winetricks component"));
            }
            ok
        })
        .cloned()
        .collect();
    if !verbs.is_empty() {
        report(&verbs.join(" "));
        if let Err(err) = prefix_tools::run(game, &Tool::Components(verbs)) {
            applied.failed.push(format!("{err:#}"));
        }
    }
    for path in &fixes.run_in_prefix {
        let Some(program) = inside_game(game, path) else {
            applied
                .failed
                .push(format!("{path}: not in the game's folder"));
            continue;
        };
        report(path);
        if let Err(err) = prefix_tools::run(game, &Tool::Program(program)) {
            applied.failed.push(format!("{path}: {err:#}"));
        }
    }
    if let Some(prefix) = launch::prefix(game) {
        mark_tried(&prefix, &game.id, fixes.keys())?;
    }
    Ok(applied)
}

/// `path` in the game's folder, when it stays there and is a file.
fn inside_game(game: &Game, path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    let stays = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if !stays {
        return None;
    }
    let program = game.details.install_path.as_ref()?.join(relative);
    program.is_file().then_some(program)
}

fn cache_file() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/known_fixes.json"))
}

/// The fixes tried in the prefix, for the game.
fn tried_in(prefix: &Path, id: &str) -> BTreeSet<String> {
    read_tried(prefix).remove(id).unwrap_or_default()
}

fn read_tried(prefix: &Path) -> BTreeMap<String, BTreeSet<String>> {
    fs::read(prefix.join(DONE_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn mark_tried(prefix: &Path, id: &str, keys: impl Iterator<Item = String>) -> anyhow::Result<()> {
    let mut all = read_tried(prefix);
    all.entry(id.to_owned()).or_default().extend(keys);
    fs::create_dir_all(prefix).with_context(|| format!("could not make {}", prefix.display()))?;
    let file = prefix.join(DONE_FILE);
    paths::write_atomic(&file, &serde_json::to_vec_pretty(&all)?)
        .with_context(|| format!("could not write {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As the list has them (trimmed).
    const LIST: &str = r#"{
        "amazon": {},
        "epic": {
            "0a2d9f6403244d12969e11da6713137b": {
                "title": "Fall Guys",
                "notes": {"vcrun2022": "Game prompts to install it."},
                "winetricks": ["vcrun2022"],
                "runInPrefix": ["EpicOnlineServices/EpicOnlineServicesInstaller.exe"]
            },
            "0828ecd4297c4f89bfcc45420e695b67": {
                "title": "Grand Theft Auto: San Andreas",
                "envVariables": {"USE_FAKE_EPIC_EXE": true}
            }
        },
        "gog": {}
    }"#;

    #[test]
    fn finds_a_games_fixes_by_store_and_id() {
        let fixes = find(LIST, "epic", "0a2d9f6403244d12969e11da6713137b").unwrap();
        assert_eq!(fixes.winetricks, ["vcrun2022"]);
        assert_eq!(
            fixes.run_in_prefix,
            ["EpicOnlineServices/EpicOnlineServicesInstaller.exe"]
        );
        assert_eq!(
            fixes.keys().collect::<Vec<_>>(),
            [
                "vcrun2022",
                "run:EpicOnlineServices/EpicOnlineServicesInstaller.exe"
            ]
        );
        assert_eq!(
            find(LIST, "epic", "0828ecd4297c4f89bfcc45420e695b67"),
            None,
            "only Heroic's own switch: nothing for Zendo"
        );
        assert_eq!(find(LIST, "gog", "0a2d9f6403244d12969e11da6713137b"), None);
        assert_eq!(find("not json", "epic", "x"), None);
    }

    #[test]
    fn runs_programs_only_from_the_games_folder() {
        let dir = std::env::temp_dir().join(format!("zendo-known-fixes-{}", std::process::id()));
        fs::create_dir_all(dir.join("Eos")).unwrap();
        fs::write(dir.join("Eos/setup.exe"), b"").unwrap();
        let mut game = Game::new(
            "epic:x".into(),
            "Sugar".into(),
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "x".into(),
            },
        );
        game.details.install_path = Some(dir.clone());
        assert_eq!(
            inside_game(&game, "Eos/setup.exe"),
            Some(dir.join("Eos/setup.exe"))
        );
        assert_eq!(inside_game(&game, "../setup.exe"), None);
        assert_eq!(inside_game(&game, "/usr/bin/true"), None);
        assert_eq!(inside_game(&game, "Eos/missing.exe"), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_each_games_tried_fixes_in_the_prefix() {
        let dir = std::env::temp_dir().join(format!("zendo-known-tried-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        mark_tried(&dir, "epic:x", ["vcrun2022".to_owned()].into_iter()).unwrap();
        mark_tried(&dir, "epic:x", ["run:a.exe".to_owned()].into_iter()).unwrap();
        mark_tried(&dir, "gog:1", ["d3dx9".to_owned()].into_iter()).unwrap();
        assert_eq!(
            tried_in(&dir, "epic:x"),
            BTreeSet::from(["vcrun2022".to_owned(), "run:a.exe".to_owned()])
        );
        assert_eq!(
            tried_in(&dir, "gog:1"),
            BTreeSet::from(["d3dx9".to_owned()])
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_windows_games_zendo_installed_have_fixes() {
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        assert_eq!(store_key(&game), Some(("gog", "1")));
        game.details.platform = Some("linux".into());
        assert_eq!(store_key(&game), None);
    }

    /// Downloads Heroic's list and reads every game in it. Point
    /// `XDG_CACHE_HOME` at a folder to throw away.
    #[test]
    #[ignore = "downloads Heroic's list; needs ZENDO_KNOWN_FIXES_TEST and a throwaway XDG_CACHE_HOME"]
    fn reads_every_game_in_heroics_list() {
        if std::env::var_os("ZENDO_KNOWN_FIXES_TEST").is_none() {
            return;
        }
        refresh().unwrap();
        let text = fs::read_to_string(cache_file().unwrap()).unwrap();
        let list: BTreeMap<String, BTreeMap<String, serde_json::Value>> =
            serde_json::from_str(&text).unwrap();
        let mut read = 0;
        for (store, games) in &list {
            for (app, game) in games {
                let has = ["winetricks", "runInPrefix"]
                    .iter()
                    .any(|key| game[key].as_array().is_some_and(|all| !all.is_empty()));
                assert_eq!(find(&text, store, app).is_some(), has, "{store} {app}");
                read += usize::from(has);
            }
        }
        eprintln!("{read} games with fixes");
        assert!(read > 0);
    }
}
