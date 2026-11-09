//! Cloud saves for the Windows games Zendo installed from Epic and GOG,
//! through their engines, by hand or before and after the game (on unless
//! the game's launch options turn it off). Each sync's outcome is kept,
//! by game, in `$XDG_DATA_HOME/zendo/cloud-saves-status.json` for the
//! game's page.
//!
//! - Epic: `legendary sync-saves <app> --accept-path`, which finds the
//!   save folder from the game's `CloudSaveFolder` and the Wine prefix in
//!   `WINEPREFIX` (so the prefix must have been started once), and keeps
//!   it. Without `--accept-path`, `-y` skips a game whose folder it has
//!   not kept yet. legendary exits well when it skips a game, so its log
//!   tells what it did.
//! - GOG: the game's save locations from GOG's remote config for its
//!   client (`clientId` in `goggame-<id>.info`), each a path with
//!   placeholders (`<?SAVED_GAMES?>/CD Projekt Red/Cyberpunk 2077`) put
//!   into the prefix, then `gogdl save-sync` for each, with the time of
//!   its last sync, which gogdl prints back and Zendo keeps in
//!   `$XDG_DATA_HOME/zendo/cloud-saves.json`. On a conflict (both sides
//!   changed) gogdl moves nothing; Zendo then keeps the old time and asks
//!   for Upload or Download.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::launch_options::LaunchOptions;
use crate::stores::Store;
use crate::{Game, Launch, Source, launch, paths};

/// GOG's settings for a client, the save locations among them.
const GOG_REMOTE_CONFIG: &str = "https://remote-config.gog.com/components/galaxy_client/clients";

/// Which way to sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Each way, as the times say.
    Both,
    /// Only bring the cloud's newer saves, as before the game.
    Fetch,
    /// Only send the newer saves here, as after the game.
    Send,
    /// Replace the cloud's with these.
    ForceUpload,
    /// Replace these with the cloud's.
    ForceDownload,
}

impl How {
    fn flag(self) -> Option<&'static str> {
        match self {
            Self::Both => None,
            Self::Fetch => Some("--skip-upload"),
            Self::Send => Some("--skip-download"),
            Self::ForceUpload => Some("--force-upload"),
            Self::ForceDownload => Some("--force-download"),
        }
    }
}

/// What a sync did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    /// The saves are in sync (nothing may have needed to move).
    Synced,
    /// The store keeps no cloud saves for the game.
    NoCloudSaves,
}

/// How the game's last sync went, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// Seconds since 1970.
    pub at: u64,
    pub state: State,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Synced,
    NoCloudSaves,
    Failed(String),
}

/// Whether the game is one whose saves Zendo syncs: a Windows game Zendo
/// installed from Epic or GOG.
#[must_use]
pub fn applies(game: &Game) -> bool {
    matches!(game.launch, Launch::Engine { .. })
        && matches!(game.source, Source::Epic | Source::Gog)
        && launch::needs_proton(game)
}

/// Whether the game's prefix has been started, which the engines need to
/// find its save folders.
#[must_use]
pub fn ready(game: &Game) -> bool {
    launch::prefix(game).is_some_and(|prefix| prefix.join("user.reg").is_file())
}

/// Whether the saves sync around the game, with these options: they do
/// unless turned off, once the prefix is there (before its first start
/// there is nothing to sync into).
#[must_use]
pub fn wanted(game: &Game, options: &LaunchOptions) -> bool {
    options.syncs_saves() && applies(game) && ready(game)
}

/// Whether the saves will sync as the game ends, as its options say.
#[must_use]
pub fn wanted_for(game: &Game) -> bool {
    LaunchOptions::load(&game.id).is_ok_and(|options| wanted(game, &options))
}

/// Each game's last sync, by id.
#[must_use]
pub fn statuses() -> BTreeMap<String, Status> {
    load_map(STATUS_FILE)
}

/// Syncs the game's saves, and keeps how it went for the game's page.
///
/// # Errors
///
/// When the game's saves cannot be synced: no prefix yet, the engine
/// fails, or a conflict waits for Upload or Download.
pub fn sync(game: &Game, how: How) -> anyhow::Result<Done> {
    if !applies(game) {
        bail!("Zendo syncs the saves of the Windows games it installed from Epic and GOG");
    }
    let result = sync_now(game, how);
    let state = match &result {
        Ok(Done::Synced) => State::Synced,
        Ok(Done::NoCloudSaves) => State::NoCloudSaves,
        Err(err) => State::Failed(format!("{err:#}")),
    };
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    // Only the page's line is lost when it cannot be written: the sync
    // itself went as it says.
    let _ = keep_in(STATUS_FILE, &game.id, Status { at, state });
    result
}

fn sync_now(game: &Game, how: How) -> anyhow::Result<Done> {
    let Launch::Engine { app, .. } = &game.launch else {
        unreachable!("checked by applies");
    };
    let prefix = launch::prefix(game).context("no folder for the game's Wine prefix")?;
    if !prefix.join("user.reg").is_file() {
        bail!(
            "start {} once first: its saves are found in its prefix",
            game.title
        );
    }
    match game.source {
        Source::Epic => sync_epic(app, &prefix, how),
        _ => sync_gog(game, app, &prefix, how),
    }
}

fn sync_epic(app: &str, prefix: &Path, how: How) -> anyhow::Result<Done> {
    let mut args = vec!["-y", "sync-saves", app, "--accept-path"];
    args.extend(how.flag());
    let output = Store::Epic.run_checked_with(&args, &[("WINEPREFIX", prefix.as_os_str())])?;
    epic_done(&String::from_utf8_lossy(&output.stderr))
}

/// What legendary did, from its log: it only names a game it checks, and
/// skips one whose save folder it cannot work out.
fn epic_done(log: &str) -> anyhow::Result<Done> {
    if !log.contains("Checking \"") {
        return Ok(Done::NoCloudSaves);
    }
    if log.contains("unprocessed variables") {
        bail!("legendary could not work out where the game keeps its saves in its prefix");
    }
    if log.contains("skipping due to --yes") {
        bail!("legendary has no save folder for the game yet");
    }
    if log.contains("Unable to lock install data") {
        bail!("another legendary is busy with the installed games: try again");
    }
    Ok(Done::Synced)
}

/// A save location GOG lists for a game.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Location {
    name: String,
    location: String,
}

fn sync_gog(game: &Game, app: &str, prefix: &Path, how: How) -> anyhow::Result<Done> {
    let folder = game
        .details
        .install_path
        .as_deref()
        .context("no record of where the game is")?;
    let Some(client) = gog_client(folder, app)? else {
        return Ok(Done::NoCloudSaves);
    };
    let locations = gog_locations(&client)?;
    if locations.is_empty() {
        return Ok(Done::NoCloudSaves);
    }
    for location in locations {
        let path = gog_path(&location.location, prefix, folder)?;
        fs::create_dir_all(&path).with_context(|| format!("could not make {}", path.display()))?;
        let key = format!("{}/{}", game.id, location.name);
        let since = load_map::<String>(TIMES_FILE)
            .remove(&key)
            .unwrap_or_else(|| "0".into());
        let path_text = path.to_string_lossy();
        let mut args = vec![
            "save-sync",
            &path_text,
            app,
            "--ts",
            &since,
            "--name",
            &location.name,
            "--os",
            "windows",
        ];
        args.extend(how.flag());
        let output = Store::Gog.run_checked_with(&args, &[])?;
        let errors = String::from_utf8_lossy(&output.stderr);
        if errors.contains("Files in conflict") {
            bail!(
                "{}'s saves changed here and in GOG's cloud since the last sync: \
                 choose Upload or Download",
                game.title
            );
        }
        let now = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if now.parse::<f64>().is_ok() {
            keep_in(TIMES_FILE, &key, now)?;
        }
    }
    Ok(Done::Synced)
}

/// The game's client id, from its info file; none for a game GOG keeps no
/// saves for.
fn gog_client(folder: &Path, app: &str) -> anyhow::Result<Option<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Info {
        client_id: Option<String>,
    }
    let file = folder.join(format!("goggame-{app}.info"));
    let text =
        fs::read_to_string(&file).with_context(|| format!("could not read {}", file.display()))?;
    let info: Info =
        serde_json::from_str(&text).with_context(|| format!("{} is not valid", file.display()))?;
    Ok(info.client_id)
}

/// The Windows save locations GOG lists for the client; none when cloud
/// saves are off for it.
fn gog_locations(client: &str) -> anyhow::Result<Vec<Location>> {
    let text = crate::http::agent(Duration::from_secs(20))
        .get(format!("{GOG_REMOTE_CONFIG}/{client}"))
        .query("component_version", "2.0.45")
        .call()
        .context("could not ask GOG where the game keeps its saves")?
        .into_body()
        .read_to_string()?;
    parse_locations(&text)
}

fn parse_locations(text: &str) -> anyhow::Result<Vec<Location>> {
    #[derive(Deserialize)]
    struct Config {
        content: BTreeMap<String, Platform>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Platform {
        cloud_storage: Option<Cloud>,
    }
    #[derive(Deserialize)]
    struct Cloud {
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        locations: Vec<Location>,
    }
    let config: Config = serde_json::from_str(text).context("GOG's settings for the game")?;
    Ok(config
        .content
        .get("Windows")
        .and_then(|platform| platform.cloud_storage.as_ref())
        .filter(|cloud| cloud.enabled)
        .map(|cloud| cloud.locations.clone())
        .unwrap_or_default())
}

/// A location's path, its placeholders put into the prefix, as Proton
/// makes it (the user `steamuser`).
fn gog_path(location: &str, prefix: &Path, install: &Path) -> anyhow::Result<PathBuf> {
    let user = prefix.join("drive_c/users/steamuser");
    let places = [
        ("<?INSTALL?>", install.to_path_buf()),
        ("<?DOCUMENTS?>", user.join("Documents")),
        ("<?SAVED_GAMES?>", user.join("Saved Games")),
        (
            "<?APPLICATION_DATA_LOCAL_LOW?>",
            user.join("AppData/LocalLow"),
        ),
        ("<?APPLICATION_DATA_LOCAL?>", user.join("AppData/Local")),
        ("<?APPLICATION_DATA_ROAMING?>", user.join("AppData/Roaming")),
    ];
    let mut path = location.replace('\\', "/");
    for (placeholder, place) in places {
        path = path.replace(placeholder, &place.to_string_lossy());
    }
    if let Some(start) = path.find("<?") {
        let unknown: String = path[start..].chars().take_while(|&c| c != '/').collect();
        bail!("GOG names a save folder Zendo does not know: {unknown}");
    }
    Ok(PathBuf::from(path))
}

/// The time of each GOG location's last sync, by game and location
/// (`<game id>/<location>`), to give gogdl: `0` before the first.
const TIMES_FILE: &str = "cloud-saves.json";
/// Each game's last sync, by id.
const STATUS_FILE: &str = "cloud-saves-status.json";

fn file(name: &str) -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo").join(name))
}

/// One of Zendo's maps; empty when missing or unreadable.
fn load_map<T: DeserializeOwned>(name: &str) -> BTreeMap<String, T> {
    file(name)
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn keep_in<T: Serialize + DeserializeOwned>(name: &str, key: &str, value: T) -> anyhow::Result<()> {
    let file = file(name).context("no home folder to keep the cloud saves' state in")?;
    let mut map = load_map::<T>(name);
    map.insert(key.to_owned(), value);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    paths::write_atomic(&file, &serde_json::to_vec_pretty(&map)?)
        .with_context(|| format!("could not write {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gogs_windows_save_locations() {
        let text = r#"{"content":{
            "MacOS":{"cloudStorage":{"enabled":false,"locations":[]}},
            "Windows":{"cloudStorage":{"enabled":true,"locations":[
                {"name":"saves","location":"<?SAVED_GAMES?>/CD Projekt Red/Cyberpunk 2077"}]}},
            "cloudStorage":{"cloudStorage":null}}}"#;
        assert_eq!(
            parse_locations(text).unwrap(),
            [Location {
                name: "saves".into(),
                location: "<?SAVED_GAMES?>/CD Projekt Red/Cyberpunk 2077".into(),
            }]
        );
        let off = r#"{"content":{"Windows":{"cloudStorage":{"enabled":false,
            "locations":[{"name":"saves","location":"x"}]}}}}"#;
        assert_eq!(parse_locations(off).unwrap(), []);
    }

    #[test]
    fn puts_gogs_placeholders_into_the_prefix() {
        let prefix = Path::new("/games/Prefixes/Cyberpunk 2077");
        let install = Path::new("/games/Cyberpunk 2077");
        assert_eq!(
            gog_path(
                "<?SAVED_GAMES?>/CD Projekt Red/Cyberpunk 2077",
                prefix,
                install
            )
            .unwrap(),
            Path::new(
                "/games/Prefixes/Cyberpunk 2077/drive_c/users/steamuser/Saved Games/CD Projekt Red/Cyberpunk 2077"
            )
        );
        assert_eq!(
            gog_path(
                "<?APPLICATION_DATA_LOCAL_LOW?>\\Studio\\Game",
                prefix,
                install
            )
            .unwrap(),
            Path::new(
                "/games/Prefixes/Cyberpunk 2077/drive_c/users/steamuser/AppData/LocalLow/Studio/Game"
            )
        );
        assert_eq!(
            gog_path("<?INSTALL?>/saves", prefix, install).unwrap(),
            Path::new("/games/Cyberpunk 2077/saves")
        );
        let error = gog_path("<?APPLICATION_SUPPORT?>/x", prefix, install).unwrap_err();
        assert_eq!(
            error.to_string(),
            "GOG names a save folder Zendo does not know: <?APPLICATION_SUPPORT?>"
        );
    }

    #[test]
    fn reads_what_legendary_did_from_its_log() {
        let checked = "[cli] INFO: Got 1 remote save game(s)\n\
            [cli] INFO: Checking \"Cat Quest\" (051e)\n";
        assert_eq!(
            epic_done(&format!(
                "{checked}[cli] INFO: Downloading remote savegame..."
            ))
            .unwrap(),
            Done::Synced
        );
        assert_eq!(
            epic_done("[cli] INFO: Got 0 remote save game(s)\n").unwrap(),
            Done::NoCloudSaves,
            "a game without cloud saves is not named"
        );
        let unknown = format!(
            "{checked}[cli] WARNING: Path contains unprocessed variables, please enter the \
             correct path manually."
        );
        assert!(epic_done(&unknown).is_err());
    }

    #[test]
    fn passes_the_way_to_sync_to_the_engines() {
        assert_eq!(How::Both.flag(), None);
        assert_eq!(How::Fetch.flag(), Some("--skip-upload"));
        assert_eq!(How::Send.flag(), Some("--skip-download"));
        assert_eq!(How::ForceUpload.flag(), Some("--force-upload"));
    }
}
