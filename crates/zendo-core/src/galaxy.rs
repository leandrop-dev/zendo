//! GOG Galaxy for the GOG games Zendo installed, as Heroic gives them:
//! comet, which answers a game's Galaxy SDK as the GOG account signed in
//! (achievements, the game's own sign-ins, the DLC it checks), runs
//! beside the game; and for a Windows game, its prefix gets the stand-in
//! Galaxy service and the registry key naming Galaxy's folder, which some
//! games look for before they talk to Galaxy.
//!
//! comet (Apache-2.0, `imLinguin/comet`) is run as a separate program, from
//! `PATH` or Heroic's folder. Its stand-in service,
//! `GalaxyCommunication.exe`, is Zendo's own copy in
//! `$XDG_DATA_HOME/zendo/galaxy`, else Heroic's, else downloaded from
//! comet's release (`dummy-service.zip`) when first needed. comet takes gogdl's tokens from Heroic's
//! `auth.json`, which has gogdl's own layout, or from its command line,
//! where other programs could read them. So Zendo points comet's
//! `XDG_CONFIG_PATH` at a folder of its own whose
//! `heroic/gog_store/auth.json` is a link to Zendo's gogdl tokens, after
//! gogdl has refreshed them; comet reads them once and writes nothing
//! back. Its own files (achievements kept for offline play, Galaxy's
//! libraries) go to `$XDG_DATA_HOME/zendo/comet`.

use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::config::Config;
use crate::stores::{self, Store};
use crate::{Game, Launch, Source, launch, paths};

/// Where the stand-in service goes in a prefix, as Windows names it.
const SERVICE: &str = r"C:\ProgramData\GOG.com\Galaxy\redists\GalaxyCommunication.exe";
/// The same, under the prefix's `drive_c`.
const SERVICE_IN_PREFIX: &str =
    "drive_c/ProgramData/GOG.com/Galaxy/redists/GalaxyCommunication.exe";
/// The stand-in service's file name.
const SERVICE_FILE: &str = "GalaxyCommunication.exe";
/// comet's release asset holding it.
const SERVICE_ZIP: &str =
    "https://github.com/imLinguin/comet/releases/latest/download/dummy-service.zip";
/// How long comet gets to stop on its own after the game.
const STOP_WAIT: Duration = Duration::from_secs(5);

/// Whether the game is one Zendo installed from GOG.
#[must_use]
pub fn applies(game: &Game) -> bool {
    game.source == Source::Gog
        && matches!(&game.launch, Launch::Engine { runner, .. } if runner == "gogdl")
}

/// Whether the game starts with GOG Galaxy: it is one Zendo installed
/// from GOG, and Settings has not turned Galaxy off.
#[must_use]
pub fn wanted(game: &Game) -> bool {
    applies(game) && Config::load_file().is_ok_and(|config| config.gog_galaxy)
}

/// Whether the game's prefix still lacks the stand-in Galaxy service.
#[must_use]
pub fn service_needed(game: &Game) -> bool {
    wanted(game)
        && launch::needs_proton(game)
        && launch::prefix(game).is_some_and(|prefix| !prefix.join(SERVICE_IN_PREFIX).is_file())
}

/// Puts the stand-in Galaxy service into the game's prefix and registers
/// it, with the registry key naming Galaxy's folder, as Heroic does. The
/// service's file there marks it done, so a step that fails is not tried
/// at every start.
///
/// # Errors
///
/// When the service cannot be found or downloaded, or a step fails (said
/// with the log).
pub fn install_service(game: &Game) -> anyhow::Result<()> {
    let source = service_exe()?;
    let prefix = launch::prefix(game).context("no Wine prefix for the game")?;
    let log = service_log_path(&game.id).context("no folder for the log")?;
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&log, "")?;
    let target = prefix.join(SERVICE_IN_PREFIX);
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    fs::copy(&source, &target)
        .with_context(|| format!("could not copy the service into {}", prefix.display()))?;
    let binpath = format!("binpath={SERVICE}");
    in_prefix(
        game,
        &["sc", "create", "GalaxyCommunication", &binpath],
        &log,
    )?;
    in_prefix(
        game,
        &[
            "reg",
            "add",
            r"HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths",
            "/v",
            "client",
            "/t",
            "REG_SZ",
            "/d",
            r"C:\Program Files\GOG Galaxy",
            "/f",
        ],
        &log,
    )
}

/// Where Zendo keeps the stand-in service it downloaded.
fn service_home() -> Option<PathBuf> {
    paths::data_home().map(|home| home.join("zendo/galaxy").join(SERVICE_FILE))
}

/// The stand-in service's program: Zendo's own copy, else Heroic's, else
/// comet's release downloaded into Zendo's folder.
fn service_exe() -> anyhow::Result<PathBuf> {
    let own = service_home().context("no data folder for the Galaxy service")?;
    if own.is_file() {
        return Ok(own);
    }
    if let Some(heroic) = stores::bundled_windows(SERVICE_FILE) {
        return Ok(heroic);
    }
    download_service(&own)?;
    Ok(own)
}

/// Replaces Zendo's copy of the service with the one in the comet release
/// Zendo just installed, checked against the release's digest, so comet
/// and its service match.
pub(crate) fn replace_service(
    asset: &crate::engines::Asset,
    stop: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    let target = service_home().context("no data folder for the Galaxy service")?;
    download_service_from(&asset.url, Some(&asset.sha256), stop, &target)
}

/// Downloads comet's latest `dummy-service.zip` for [`service_exe`]. That
/// release's digest is not asked for here, so the program is only checked
/// to be a Windows executable.
fn download_service(target: &Path) -> anyhow::Result<()> {
    download_service_from(
        SERVICE_ZIP,
        None,
        &std::sync::atomic::AtomicBool::new(false),
        target,
    )
}

/// Downloads the zip, checks it against `sha256` when given, unpacks it
/// beside `target` and moves the program into place.
fn download_service_from(
    url: &str,
    sha256: Option<&str>,
    stop: &std::sync::atomic::AtomicBool,
    target: &Path,
) -> anyhow::Result<()> {
    let dir = target.parent().context("no folder for the service")?;
    fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    let zip = dir.join("dummy-service.zip");
    let unpacked = dir.join("unpacking");
    let _ = fs::remove_dir_all(&unpacked);
    fs::create_dir_all(&unpacked)?;
    let result = (|| {
        crate::http::download(
            url,
            &zip,
            "GOG Galaxy's stand-in service",
            None,
            stop,
            &mut |_, _| {},
        )?;
        if let Some(sha256) = sha256 {
            crate::engines::check_sha256(&zip, sha256)?;
        }
        crate::mods::add::unpack(&zip, &unpacked)?;
        let found = find_file(&unpacked, SERVICE_FILE)
            .with_context(|| format!("{SERVICE_FILE} is not in comet's dummy-service.zip"))?;
        anyhow::ensure!(
            is_pe(&found),
            "{SERVICE_FILE} in the download is not a Windows program"
        );
        fs::rename(&found, target)
            .with_context(|| format!("could not move the service to {}", target.display()))
    })();
    let _ = fs::remove_file(&zip);
    let _ = fs::remove_dir_all(&unpacked);
    result
}

/// The first file called `name` under `dir`, however deep.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|file| file == name) {
            return Some(path);
        }
    }
    None
}

/// Whether the file starts as a Windows program does (`MZ`).
fn is_pe(file: &Path) -> bool {
    let mut start = [0_u8; 2];
    fs::File::open(file)
        .and_then(|mut open| io::Read::read_exact(&mut open, &mut start))
        .is_ok()
        && &start == b"MZ"
}

/// Runs a Wine program in the game's prefix, through umu, into `log`.
fn in_prefix(game: &Game, words: &[&str], log: &Path) -> anyhow::Result<()> {
    let mut command = launch::prefix_command(game, None)?;
    command.args(words);
    let mut file = fs::OpenOptions::new().append(true).open(log)?;
    io::Write::write_all(
        &mut file,
        format!("\n{}\n\n", launch::describe(&command)).as_bytes(),
    )?;
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        .status()
        .context("could not start umu-run")?;
    if !status.success() {
        anyhow::bail!("{} ended with {status}; see {}", words[0], log.display());
    }
    Ok(())
}

/// comet, running beside a game; stopped when dropped.
#[derive(Debug)]
pub struct Comet {
    child: Child,
}

impl Drop for Comet {
    /// Asks comet to stop as Ctrl+C would, and ends it if it has not
    /// after a few seconds.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        // Its own group, so everything it started ends with it.
        crate::downloads::signal_group(self.child.id(), "-INT");
        let asked = Instant::now();
        while asked.elapsed() < STOP_WAIT {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts comet for the game, with the GOG account signed in in Zendo;
/// none when the game does not start with Galaxy or no one is signed in.
///
/// # Errors
///
/// When comet is missing, or the account's tokens or name cannot be had
/// (offline, say); the game starts without it.
pub fn start(game: &Game) -> anyhow::Result<Option<Comet>> {
    if !wanted(game) {
        return Ok(None);
    }
    let comet = stores::bundled("comet")
        .context("comet is not installed: download it in Settings → Engines")?;
    // Refreshes gogdl's tokens, which comet then reads.
    let Some(credentials) = Store::Gog.gog_token()? else {
        return Ok(None);
    };
    let username =
        stores::gog_username(&credentials).context("could not ask GOG for the account's name")?;
    let config = link_tokens()?;
    let data = paths::data_home().context("no data folder")?.join("zendo");
    let log = log_path(&game.id).context("no folder for the log")?;
    let mut command = Command::new(comet);
    command
        .args(["--from-heroic", "--username", &username, "--quit"])
        .env("XDG_CONFIG_PATH", &config)
        .env("XDG_DATA_HOME", &data)
        .stdin(Stdio::null())
        // Its own group: the game's is watched to tell when the game ends.
        .process_group(0);
    let (stdout, stderr) = launch::log_to(&log, &command)?;
    let child = command
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .context("could not start comet")?;
    Ok(Some(Comet { child }))
}

/// The folder comet takes for its `XDG_CONFIG_PATH`, its
/// `heroic/gog_store/auth.json` made a link to gogdl's tokens.
fn link_tokens() -> anyhow::Result<PathBuf> {
    let gog = Store::Gog.dir().context("no data folder")?;
    let config = comet_config(&gog);
    let link = config.join("heroic/gog_store/auth.json");
    if let Some(dir) = link.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    match fs::remove_file(&link) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => {
            return Err(err).with_context(|| format!("could not replace {}", link.display()));
        }
        _ => {}
    }
    std::os::unix::fs::symlink(gog.join("auth.json"), &link)
        .with_context(|| format!("could not link {}", link.display()))?;
    Ok(config)
}

fn comet_config(gog: &Path) -> PathBuf {
    gog.join("comet")
}

/// Removes comet's link to the tokens and its own files, as GOG is signed
/// out.
///
/// # Errors
///
/// When they cannot be removed.
pub fn forget() -> anyhow::Result<()> {
    let folders = [
        Store::Gog.dir().map(|gog| comet_config(&gog)),
        paths::data_home().map(|dir| dir.join("zendo/comet")),
    ];
    for dir in folders.into_iter().flatten() {
        match fs::remove_dir_all(&dir) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => {
                return Err(err).with_context(|| format!("could not remove {}", dir.display()));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Where comet's output for the game goes.
#[must_use]
pub fn log_path(id: &str) -> Option<PathBuf> {
    launch::log_path(id).map(|log| log.with_extension("comet.log"))
}

/// Where the stand-in service's setup was last logged.
#[must_use]
pub fn service_log_path(id: &str) -> Option<PathBuf> {
    launch::log_path(id).map(|log| log.with_extension("galaxy.log"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(source: Source, launch: Launch) -> Game {
        Game::new("gog:1".into(), "Sugar".into(), source, launch)
    }

    #[test]
    fn only_gog_games_zendo_installed_get_galaxy() {
        let engine = Launch::Engine {
            runner: "gogdl".into(),
            app: "1".into(),
        };
        assert!(applies(&game(Source::Gog, engine.clone())));
        let other = Launch::Engine {
            runner: "legendary".into(),
            app: "1".into(),
        };
        assert!(!applies(&game(Source::Epic, other)));
        let by_heroic = Launch::Uri {
            uri: "heroic://launch/gog/1".into(),
        };
        assert!(!applies(&game(Source::Gog, by_heroic)));
    }

    #[test]
    fn finds_the_service_in_an_unpacked_zip_and_checks_it_is_a_program() {
        let dir = std::env::temp_dir().join(format!("zendo-galaxy-{}", std::process::id()));
        let inner = dir.join("dummy-service/bin");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join(SERVICE_FILE), b"MZ\x90\0").unwrap();
        fs::write(dir.join("README.md"), b"MZ").unwrap();
        let found = find_file(&dir, SERVICE_FILE).unwrap();
        assert_eq!(found, inner.join(SERVICE_FILE));
        assert!(is_pe(&found));
        assert!(!is_pe(&dir.join("missing.exe")));
        fs::write(&found, b"<html>").unwrap();
        assert!(!is_pe(&found));
        assert!(find_file(&dir, "other.exe").is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_service_goes_where_windows_looks_for_it() {
        let windows = SERVICE.trim_start_matches(r"C:\").replace('\\', "/");
        assert_eq!(format!("drive_c/{windows}"), SERVICE_IN_PREFIX);
    }

    /// Through the real umu and Proton, in a prefix of the games folder that
    /// `XDG_CONFIG_HOME`'s `config.toml` names, logging under
    /// `XDG_STATE_HOME`: point all at a folder to throw away.
    #[test]
    #[ignore = "runs umu and Proton; needs ZENDO_GALAXY_TEST and a throwaway XDG_CONFIG_HOME"]
    fn puts_the_service_into_a_prefix() {
        if std::env::var_os("ZENDO_GALAXY_TEST").is_none() {
            return;
        }
        let game = game(
            Source::Local,
            Launch::Local {
                program: "/nowhere/game.exe".into(),
                args: Vec::new(),
                dir: None,
            },
        );
        install_service(&game).unwrap();
        let prefix = launch::prefix(&game).unwrap();
        assert!(prefix.join(SERVICE_IN_PREFIX).is_file());
        // Wine keeps its own case for the keys it already has, and the
        // services under ControlSet001, which CurrentControlSet links to.
        let system = fs::read_to_string(prefix.join("system.reg"))
            .unwrap()
            .to_lowercase();
        assert!(
            system.contains(r"[system\\controlset001\\services\\galaxycommunication]"),
            "the service is registered"
        );
        assert!(
            system.contains(r"[software\\wow6432node\\gog.com\\galaxyclient\\paths]"),
            "Galaxy's folder is named"
        );
    }

    #[test]
    fn logs_beside_the_games() {
        let comet = log_path("gog:1").unwrap();
        assert_eq!(comet.file_name().unwrap(), "gog_1.comet.log");
        let service = service_log_path("gog:1").unwrap();
        assert_eq!(service.file_name().unwrap(), "gog_1.galaxy.log");
    }
}
