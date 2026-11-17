//! Nexus Launcher's games: PC Game Pass titles the Microsoft Store installed.
//!
//! The launcher (`domsvoboda/nexus-launcher`, MIT, formerly `xodus-nexus`)
//! keeps a profile in `$XDG_CONFIG_HOME/xodus-nexus` with
//! `launcher-state.json`. A record there is a game the Microsoft Store
//! installed; it is started through the launcher's own
//! `shortcuts/launch-game <id>` script. Read only, as Steam and Heroic are
//! read: Zendo lists these games and starts them, and never installs,
//! updates or uninstalls them (`docs/reference.md`, "Nexus Launcher").

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{Details, Game, Launch, Scan, Source, paths};

/// Where the launcher keeps its profile, when it is installed.
#[must_use]
pub fn profile() -> Option<PathBuf> {
    let dir = paths::config_home()?.join("xodus-nexus");
    dir.is_dir().then_some(dir)
}

/// The marker the launcher writes on line 2 of the scripts it makes. A
/// `launch-game` without it is some other file, and is never started.
const MARKER: &str = "# X-Xodus-Nexus-Profile=";

/// The games the launcher's profile lists, or one problem.
pub(crate) fn scan(profile: Option<&Path>) -> Scan {
    let mut scan = Scan::default();
    let Some(profile) = profile else {
        return scan;
    };
    let file = profile.join("launcher-state.json");
    let path = file.display().to_string();
    let state: State = match fs::read(&file)
        .map_err(|err| err.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| err.to_string()))
    {
        Ok(state) => state,
        Err(err) => {
            scan.problems.push(format!("{path}: {err}"));
            return scan;
        }
    };
    let Some(shortcut) = script(profile) else {
        scan.problems.push(format!(
            "{}: missing, or not a script the launcher wrote",
            profile.join("shortcuts/launch-game").display()
        ));
        return scan;
    };
    for record in state.games {
        if let Some(game) = game(&record, &shortcut) {
            scan.games.push(game);
        }
    }
    scan
}

/// The script that starts a game by its id, when the launcher wrote it.
fn script(profile: &Path) -> Option<PathBuf> {
    let link = profile.join("shortcuts/launch-game");
    let second = fs::read_to_string(&link).ok()?.lines().nth(1)?.to_owned();
    second.starts_with(MARKER).then_some(link)
}

/// One game, when it is installed and can be started.
fn game(record: &Record, shortcut: &Path) -> Option<Game> {
    let id = record.id.trim();
    if id.is_empty() || record.install_state.as_deref() != Some("ready") {
        return None;
    }
    let path = PathBuf::from(record.path.as_deref()?.trim());
    if !path.is_absolute() {
        return None;
    }
    let title = match record.title.trim() {
        "" => id.to_owned(),
        title => title.to_owned(),
    };
    let mut game = Game::new(
        format!("xbox:{id}"),
        title,
        Source::Xbox,
        Launch::Command {
            program: shortcut.display().to_string(),
            args: vec![id.to_owned()],
            dir: None,
        },
    );
    game.details = Details {
        developer: record.publisher.clone(),
        platform: Some("windows".to_owned()),
        install_path: Some(path),
        ..Details::default()
    };
    // The launcher carries the cover inside its record, as a `data:` URL the
    // covers fetch knows to read; the wide art is the Store's own address.
    game.cover_url = record.cover_data_url.clone().filter(|url| !url.is_empty());
    game.hero_url = record
        .hero_url
        .clone()
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
    Some(game)
}

/// `launcher-state.json`, as much of it as Zendo reads.
#[derive(Debug, Deserialize)]
struct State {
    #[serde(default)]
    games: Vec<Record>,
}

/// One game in the launcher's state.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    install_state: Option<String>,
    #[serde(default)]
    cover_data_url: Option<String>,
    #[serde(default)]
    hero_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with `launcher-state.json` and the launcher's script.
    fn profile(name: &str, state: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zendo-xodus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("shortcuts")).expect("the test's folders");
        fs::write(dir.join("launcher-state.json"), state).expect("the state file");
        fs::write(
            dir.join("shortcuts/launch-game"),
            format!(
                "#!/bin/sh\n{MARKER}{}\nexec xodus-cli launch \"$@\"\n",
                dir.display()
            ),
        )
        .expect("the launcher's script");
        dir
    }

    fn by_id<'a>(scan: &'a Scan, id: &str) -> &'a Game {
        scan.games
            .iter()
            .find(|game| game.id == id)
            .expect("the game")
    }

    #[test]
    fn reads_the_games_that_are_ready_and_startable() {
        let dir = profile(
            "ready",
            r#"{
                "games": [
                    {
                        "id": "0123456789abcdef",
                        "title": "Halo Infinite",
                        "publisher": "Xbox Game Studios",
                        "storeId": "9NNX1VVR9WKB",
                        "path": "/home/me/Games/Halo Infinite",
                        "installState": "ready",
                        "executable": "HaloInfinite.exe",
                        "coverDataUrl": "data:image/png;base64,iVBORw0KGgo=",
                        "heroUrl": "https://store-images.s-microsoft.com/hero.jpg"
                    },
                    { "id": "notready", "title": "Installing", "path": "/home/me/Games/X", "installState": "installing" },
                    { "id": "relative", "title": "Relative", "path": "Games/X", "installState": "ready" },
                    { "id": "nopath", "title": "No path", "installState": "ready" }
                ]
            }"#,
        );
        let scan = scan(Some(&dir));
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        assert_eq!(scan.games.len(), 1, "only the ready, absolute game");
        let game = by_id(&scan, "xbox:0123456789abcdef");
        assert_eq!(game.title, "Halo Infinite");
        assert_eq!(game.source, Source::Xbox);
        assert_eq!(
            game.launch,
            Launch::Command {
                program: dir.join("shortcuts/launch-game").display().to_string(),
                args: vec!["0123456789abcdef".to_owned()],
                dir: None,
            }
        );
        assert_eq!(
            game.details.install_path.as_deref(),
            Some(Path::new("/home/me/Games/Halo Infinite"))
        );
        assert_eq!(game.details.developer.as_deref(), Some("Xbox Game Studios"));
        assert_eq!(
            game.cover_url.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_title_that_is_empty_takes_the_id() {
        let dir = profile(
            "untitled",
            r#"{"games":[{"id":"aaa","title":"","path":"/games/a","installState":"ready"}]}"#,
        );
        let scan = scan(Some(&dir));
        assert_eq!(by_id(&scan, "xbox:aaa").title, "aaa");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_that_is_not_the_launchers_gives_a_problem_and_no_games() {
        let dir = profile(
            "foreign",
            r#"{"games":[{"id":"aaa","title":"A","path":"/games/a","installState":"ready"}]}"#,
        );
        fs::write(dir.join("shortcuts/launch-game"), "#!/bin/sh\nrm -rf /\n")
            .expect("another script");
        let scan = scan(Some(&dir));
        assert!(scan.games.is_empty(), "a foreign script is not started");
        assert_eq!(scan.problems.len(), 1);
        assert!(
            scan.problems[0].contains("launch-game"),
            "{:?}",
            scan.problems
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_profile_is_no_game_and_no_problem() {
        let scan = scan(None);
        assert!(scan.games.is_empty());
        assert!(scan.problems.is_empty());
    }
}
