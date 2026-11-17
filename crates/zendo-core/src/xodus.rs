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
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;
use serde::Deserialize;

use crate::{Details, Game, Launch, Scan, Source, engines, paths};

/// Where the launcher keeps its profile, when it is installed.
#[must_use]
pub fn profile() -> Option<PathBuf> {
    let dir = paths::config_home()?.join("xodus-nexus");
    dir.is_dir().then_some(dir)
}

/// The marker the launcher writes on line 2 of the scripts it makes. A
/// `launch-game` without it is some other file, and is never started.
const MARKER: &str = "# X-Xodus-Nexus-Profile=";

/// `domsvoboda/nexus-launcher`, whose releases carry the Linux `AppImage`.
const REPO: &str = "domsvoboda/nexus-launcher";

/// What the launcher is called in Zendo's words.
const LABEL: &str = "Nexus Launcher";

/// What the file in Zendo's launchers folder is called.
const APPIMAGE: &str = "Nexus.AppImage";

/// The `AppImage` Zendo downloaded, once it is in place.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
    engines::launchers_dir().map(|dir| dir.join(APPIMAGE))
}

/// The `AppImage`, when it is there.
#[must_use]
pub fn installed() -> Option<PathBuf> {
    appimage().filter(|path| path.is_file())
}

/// How many games the launcher's profile lists as ready.
#[must_use]
pub fn count() -> usize {
    profile().map_or(0, |profile| scan(Some(&profile)).games.len())
}

/// The `AppImage` of the latest release, for this machine.
///
/// # Errors
///
/// When GitHub cannot be reached, the release has no `AppImage` for this
/// machine, or it has no checksum to check the download against.
pub fn release() -> anyhow::Result<engines::Release> {
    let (tag, asset) = engines::latest_asset(REPO, LABEL, |name| {
        appimage_for(name, std::env::consts::ARCH)
    })?;
    Ok(engines::Release {
        tag,
        asset,
        service: None,
    })
}

/// Whether an `AppImage`'s name is the one this machine runs: the release
/// names them `Xodus-Nexus-<version>-x86_64.AppImage` and `-arm64`, and the
/// checksum file beside them is not an `AppImage`.
fn appimage_for(name: &str, arch: &str) -> bool {
    name.ends_with(".AppImage")
        && match arch {
            "x86_64" => name.contains("x86_64"),
            "aarch64" => name.contains("arm64") || name.contains("aarch64"),
            _ => false,
        }
}

/// Downloads the `AppImage`, checks it against its digest and puts it in
/// place, executable, reporting the bytes as it goes; `stop` set ends it.
///
/// # Errors
///
/// When the download fails or is stopped, the checksum does not match, or
/// the file cannot be put in place; the `AppImage` there before stays then.
pub fn install(
    release: &engines::Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    engines::bring(release, APPIMAGE, stop, progress)
}

/// Opens the launcher in its own window: it writes its profile and shortcuts
/// the first time, and there the user installs its runtime and signs in.
/// The caller waits for it, so the profile can be read once it ends.
///
/// # Errors
///
/// When the `AppImage` is not there or cannot be started.
pub fn open() -> anyhow::Result<std::process::Child> {
    let appimage = installed().context("Nexus Launcher is not installed yet")?;
    crate::launch::spawn_logged(std::process::Command::new(appimage), "nexus")
}

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
    fn takes_the_appimage_for_this_machine() {
        let name = "Xodus-Nexus-0.5.0-x86_64.AppImage";
        assert!(super::appimage_for(name, "x86_64"));
        assert!(!super::appimage_for(name, "aarch64"));
        assert!(super::appimage_for(
            "Xodus-Nexus-0.5.0-arm64.AppImage",
            "aarch64"
        ));
        assert!(
            super::appimage_for("Xodus-Nexus-0.5.0-aarch64.AppImage", "aarch64"),
            "either name the release gives it"
        );
        assert!(
            !super::appimage_for("Xodus-Nexus-0.5.0-x86_64.AppImage.sha512sum", "x86_64"),
            "the checksum file is not the AppImage"
        );
        assert!(!super::appimage_for(name, "riscv64"), "no AppImage here");
    }

    #[test]
    fn no_profile_is_no_game_and_no_problem() {
        let scan = scan(None);
        assert!(scan.games.is_empty());
        assert!(scan.problems.is_empty());
    }
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_the_real_latest_release() {
        let release = super::release().unwrap();
        println!(
            "{} {} {} {}",
            release.tag, release.asset.name, release.asset.size, release.asset.sha256
        );
        assert!(super::appimage_for(
            &release.asset.name,
            std::env::consts::ARCH
        ));
        assert_eq!(release.asset.sha256.len(), 64, "a digest to check against");
    }

    /// Downloads the real `AppImage` into a throwaway home; run with
    /// `XDG_DATA_HOME=.../zendo-xodus-live/data XDG_CACHE_HOME=.../zendo-xodus-live/cache
    /// cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and writes to a throwaway home"]
    fn installs_the_real_launcher() {
        let data = std::env::var("XDG_DATA_HOME").unwrap_or_default();
        assert!(
            data.contains("zendo-xodus-live"),
            "set XDG_DATA_HOME and XDG_CACHE_HOME to a scratch folder named zendo-xodus-live"
        );
        let release = super::release().unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let at = super::install(&release, &stop, &mut |_, _| {}).unwrap();
        assert_eq!(super::installed(), Some(at.clone()));
        let bytes = std::fs::read(&at).unwrap();
        assert!(bytes.starts_with(b"\x7fELF"), "an AppImage is an ELF file");
        let mode =
            std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&at).unwrap().permissions());
        assert_eq!(mode & 0o111, 0o111, "it can be run");
        println!(
            "{} {} -> {} bytes",
            release.tag,
            release.asset.name,
            bytes.len()
        );
    }
}
