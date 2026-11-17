//! Battle.net: the games the Battle.net client installed inside a Wine,
//! Proton or Bottles prefix, read from the Agent's `product.db` under the
//! prefix's `drive_c`.
//!
//! Read-only, like Steam's and Heroic's imports: Zendo never installs or
//! updates a Battle.net game. It starts one through the prefix's own client,
//! `<Battle.net.exe> --exec=launch <code>`, with `wine`, `umu-run` or
//! `bottles-cli` doing the running. The database is a protobuf-like wire
//! format, not SQLite; only the fields Zendo needs are read, everything else
//! is skipped.
//!
//! The product codes are Battle.net's own; most also serve as the launch
//! code, and [`CATALOGUE`] lists the few that differ. Artwork in the game's
//! own folder comes first; `SteamGridDB` fills in the rest, as for every other
//! source.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::launch_options::LaunchOptions;
use crate::{Details, Game, Launch, Played, Scan, Source, WineRunner, paths};

/// The Agent's database, under a prefix's `drive_c`.
const PRODUCT_DB: &str = "drive_c/ProgramData/Battle.net/Agent/product.db";

/// The client's program, under a prefix's `drive_c`, in the places Blizzard
/// writes it.
const CLIENTS: [&str; 4] = [
    "drive_c/Program Files (x86)/Battle.net/Battle.net.exe",
    "drive_c/Program Files/Battle.net/Battle.net.exe",
    "drive_c/Program Files (x86)/Battle.net/Battle.net Launcher.exe",
    "drive_c/Program Files/Battle.net/Battle.net Launcher.exe",
];

/// A database bigger than this is not one the Agent wrote.
const MAX_DB: u64 = 16 * 1024 * 1024;

/// Artwork a game's folder may carry, the first found winning.
const COVERS: [&str; 5] = [
    "cover.png",
    "cover.jpg",
    "cover.webp",
    "library_600x900.jpg",
    "library_600x900.png",
];
const HEROES: [&str; 4] = [
    "library_hero.png",
    "library_hero.jpg",
    "header.jpg",
    "header.png",
];

/// Every prefix Battle.net may be installed in: Wine's own, Wine's and
/// Bottles' folders, the Bottles Flatpak's, `~/Games`, and the prefixes
/// Steam's Proton made.
#[must_use]
pub fn prefixes() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(home) = paths::home() {
        add(&home.join(".wine"), &mut found);
        children(&home.join(".local/share/wineprefixes"), &mut found);
        children(
            &home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
            &mut found,
        );
        children(&home.join("Games"), &mut found);
    }
    if let Some(data) = paths::data_home() {
        children(&data.join("wineprefixes"), &mut found);
        children(&data.join("bottles/bottles"), &mut found);
    }
    // The prefix Zendo made for its own Battle.net installation.
    if let Some(prefix) = managed_prefix() {
        add(&prefix, &mut found);
    }
    for root in crate::steam::roots() {
        let mut problems = Vec::new();
        for library in crate::steam::library_folders(&root, &mut problems) {
            let compat = library.join("steamapps/compatdata");
            let Ok(entries) = fs::read_dir(&compat) else {
                continue;
            };
            for entry in entries.flatten() {
                let prefix = entry.path().join("pfx");
                if prefix.is_dir() {
                    add(&prefix, &mut found);
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Keeps the prefix when it holds a `product.db`.
fn add(prefix: &Path, found: &mut Vec<PathBuf>) {
    if prefix.join(PRODUCT_DB).is_file() && !found.iter().any(|seen| seen == prefix) {
        found.push(prefix.to_path_buf());
    }
}

/// Adds each child of `parent` that holds a `product.db`.
fn children(parent: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            add(&entry.path(), found);
        }
    }
}

pub(crate) fn scan(prefixes: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    for prefix in prefixes {
        let db = prefix.join(PRODUCT_DB);
        let path = db.display().to_string();
        let Ok(meta) = fs::metadata(&db) else {
            continue;
        };
        if meta.len() > MAX_DB {
            scan.problems
                .push(format!("{path}: too big to be the Agent's database"));
            continue;
        }
        let bytes = match fs::read(&db) {
            Ok(bytes) => bytes,
            Err(err) => {
                scan.problems.push(format!("{path}: {err}"));
                continue;
            }
        };
        let Some(products) = products(&bytes) else {
            scan.problems
                .push(format!("{path}: could not be read as the Agent's database"));
            continue;
        };
        let last = last_played(prefix);
        for product in products {
            if let Some(game) = game(prefix, &product, &last) {
                scan.games.push(game);
            }
        }
    }
    scan
}

/// One game from a product the Agent knows, when it is installed or
/// playable and its folder can be found.
fn game(prefix: &Path, product: &Product, last: &HashMap<String, u64>) -> Option<Game> {
    let code = product.code.trim();
    if code.is_empty() || is_tool(code) || !(product.installed || product.playable) {
        return None;
    }
    let install_path = unix_path(prefix, product.install_path.as_deref()?)?;
    let (title, launch) = catalogue(code);
    let mut game = Game::new(
        format!("battlenet:{code}@{}", fingerprint(prefix)),
        title,
        Source::BattleNet,
        Launch::Prefix {
            prefix: prefix.to_path_buf(),
            runner: runner(prefix),
            bottle: bottle(prefix),
            program: client(prefix),
            args: vec![format!("--exec=launch {launch}")],
        },
    );
    game.details = Details {
        platform: Some("windows".to_owned()),
        install_path: Some(install_path.clone()),
        ..Details::default()
    };
    game.played = Played {
        last: last.get(&code.to_ascii_lowercase()).copied(),
        ..Played::default()
    };
    game.cover = art(&install_path, &COVERS);
    game.hero = art(&install_path, &HEROES);
    Some(game)
}

/// The Agent's and Blizzard's own tools, not games.
fn is_tool(code: &str) -> bool {
    code.eq_ignore_ascii_case("agent") || code.eq_ignore_ascii_case("bna")
}

/// The client in the prefix, when it is there.
fn client_in(prefix: &Path) -> Option<PathBuf> {
    CLIENTS
        .iter()
        .map(|relative| prefix.join(relative))
        .find(|path| path.is_file())
}

/// The client in the prefix, the first place Blizzard writes it to.
fn client(prefix: &Path) -> PathBuf {
    client_in(prefix).unwrap_or_else(|| prefix.join(CLIENTS[0]))
}

/// What runs the client: a Bottles prefix, a Proton one, Wine, or Zendo's
/// own Battle.net prefix.
///
/// Zendo's own prefix runs on Proton even though nothing marks it as one:
/// Blizzard's client crashes in stock Wine before its sign-in (a breakpoint
/// exception at its loading screen, `docs/history.md` item 58c), and
/// `umu-run` with a Proton is the way it runs on Linux.
fn runner(prefix: &Path) -> WineRunner {
    if prefix.join("bottle.yml").is_file() {
        return WineRunner::Bottles;
    }
    let steam_style = prefix.file_name().is_some_and(|name| name == "pfx")
        && prefix
            .parent()
            .is_some_and(|parent| parent.join("version").is_file());
    if steam_style || prefix.join("version").is_file() || managed(prefix) {
        WineRunner::Proton
    } else {
        WineRunner::Wine
    }
}

/// Whether the prefix is the one Zendo makes for Battle.net, the games
/// folder's `Prefixes/Battle.net` ([`managed_prefix`]): read by its place,
/// so no configuration is needed to tell.
fn managed(prefix: &Path) -> bool {
    prefix.file_name().is_some_and(|name| name == "Battle.net")
        && prefix
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "Prefixes")
}

/// The bottle's name, read from `bottle.yml`; the folder's when the file
/// does not say.
fn bottle(prefix: &Path) -> Option<String> {
    if !prefix.join("bottle.yml").is_file() {
        return None;
    }
    if let Ok(text) = fs::read_to_string(prefix.join("bottle.yml")) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("Name:") {
                let value = value.trim().trim_matches(['"', '\'']);
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    prefix
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Blizzard's installer, which follows through to the versioned file.
pub const INSTALLER_URL: &str = "https://us.battle.net/download/getInstallerForGame?os=win&locale=enUS&version=LIVE&gameProgram=BATTLENET_APP";

/// The prefix Zendo installs Battle.net into: the games folder's
/// `Prefixes/Battle.net`.
#[must_use]
pub fn managed_prefix() -> Option<PathBuf> {
    let dir = crate::config::Config::load_file().ok()?.install_dir()?;
    Some(dir.join("Prefixes").join("Battle.net"))
}

/// Where the installer is kept while and after it is downloaded.
#[must_use]
pub fn installer_path() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/battlenet/Battle.net-Setup.exe"))
}

/// Downloads Blizzard's installer, saying how far it got. It has no checksum
/// to check it against; HTTPS is the only guarantee.
///
/// # Errors
///
/// When there is no cache folder, or the download fails or is stopped.
pub fn download_installer(
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<PathBuf> {
    let path = installer_path().context("no cache folder for the installer")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    crate::http::download(
        INSTALLER_URL,
        &path,
        "the Battle.net installer",
        None,
        stop,
        progress,
    )?;
    Ok(path)
}

/// The prefix and client found, Zendo's own prefix first.
#[must_use]
pub fn installed() -> Option<(PathBuf, PathBuf)> {
    let mut candidates = Vec::new();
    if let Some(prefix) = managed_prefix() {
        candidates.push(prefix);
    }
    candidates.extend(prefixes());
    candidates
        .into_iter()
        .find_map(|prefix| client_in(&prefix).map(|client| (prefix, client)))
}

/// How many games the prefixes hold.
#[must_use]
pub fn count() -> usize {
    scan(&prefixes()).games.len()
}

/// Runs Blizzard's installer in Zendo's prefix, with what runs that prefix
/// (a Proton, as [`runner`] says). The window it opens is the user's to
/// click through; the process ends when it does.
///
/// # Errors
///
/// When there is no games folder, the installer is not downloaded, or Wine
/// cannot be started.
pub fn run_installer() -> anyhow::Result<std::process::Child> {
    let prefix = managed_prefix().context("set the games folder in Settings first")?;
    let installer = installer_path().context("no cache folder for the installer")?;
    if !installer.is_file() {
        bail!("download the Battle.net installer first");
    }
    fs::create_dir_all(&prefix).with_context(|| format!("could not make {}", prefix.display()))?;
    let mut warning = None;
    let command = crate::launch::in_prefix(
        &prefix,
        runner(&prefix),
        None,
        &installer,
        &[],
        &LaunchOptions::default(),
        &mut warning,
    )?;
    crate::launch::spawn_logged(command, "battlenet-installer")
}

/// Opens the client found, so the user signs in or plays from it.
///
/// # Errors
///
/// When Battle.net is not installed, or its runner cannot be started.
pub fn open_client() -> anyhow::Result<()> {
    let (prefix, client) = installed().context("Battle.net is not installed yet")?;
    let runner = runner(&prefix);
    let bottle = bottle(&prefix);
    let mut warning = None;
    let command = crate::launch::in_prefix(
        &prefix,
        runner,
        bottle.as_deref(),
        &client,
        &[],
        &LaunchOptions::default(),
        &mut warning,
    )?;
    let mut child = crate::launch::spawn_logged(command, "battlenet")?;
    // Reaped when it ends, so it does not linger as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// A short, stable tag for the prefix, so the same product in two prefixes
/// is two games. FNV-1a, which does not change between runs or versions.
fn fingerprint(prefix: &Path) -> String {
    let canonical = prefix
        .canonicalize()
        .unwrap_or_else(|_| prefix.to_path_buf());
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in canonical.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")[..8].to_owned()
}

/// A Windows path from the database as a path under the prefix: through the
/// prefix's `dosdevices` when the drive is there, else `drive_c` for `C:`.
fn unix_path(prefix: &Path, windows: &str) -> Option<PathBuf> {
    let path = windows.trim().replace('\\', "/");
    let mut chars = path.chars();
    let drive = chars.next()?.to_ascii_lowercase();
    if chars.next()? != ':' || !drive.is_ascii_alphabetic() {
        return None;
    }
    let rest = path[2..].trim_start_matches('/');
    let device = prefix.join("dosdevices").join(format!("{drive}:"));
    if let Ok(real) = device.canonicalize() {
        return Some(real.join(rest));
    }
    (drive == 'c').then(|| prefix.join("drive_c").join(rest))
}

/// The first of `names` the folder carries.
fn art(folder: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| folder.join(name))
        .find(|path| path.is_file())
}

/// When each product was last played, by product code in lower case.
fn last_played(prefix: &Path) -> HashMap<String, u64> {
    let mut played: HashMap<String, u64> = HashMap::new();
    let users = prefix.join("drive_c/users");
    let Ok(entries) = fs::read_dir(&users) else {
        return played;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let config = entry
            .path()
            .join("AppData/Roaming/Battle.net/Battle.net.config");
        let Ok(bytes) = fs::read(&config) else {
            continue;
        };
        let Ok(config) = serde_json::from_slice::<Config>(&bytes) else {
            continue;
        };
        for (code, game) in config.games {
            let Some(stamp) = game.last_played.as_ref().and_then(stamp) else {
                continue;
            };
            let seen = played.entry(code.to_ascii_lowercase()).or_default();
            *seen = (*seen).max(stamp);
        }
    }
    played
}

#[derive(Deserialize)]
struct Config {
    #[serde(default, rename = "Games")]
    games: HashMap<String, ConfigGame>,
}

#[derive(Deserialize)]
struct ConfigGame {
    #[serde(default, rename = "LastPlayed")]
    last_played: Option<serde_json::Value>,
}

/// A `LastPlayed` as a Unix time: a Windows FILETIME, milliseconds or
/// seconds, whichever the value is big enough to be.
fn stamp(value: &serde_json::Value) -> Option<u64> {
    let value = match value {
        serde_json::Value::Number(number) => number.as_i64()?,
        serde_json::Value::String(text) => text.trim().parse().ok()?,
        _ => return None,
    };
    if value <= 0 {
        return None;
    }
    if value > 10_000_000_000_000_000 {
        let unix = value / 10_000_000 - 11_644_473_600;
        return u64::try_from(unix).ok().filter(|&unix| unix > 0);
    }
    if value > 10_000_000_000 {
        return u64::try_from(value / 1000).ok();
    }
    u64::try_from(value).ok()
}

/// A product's title and the code the client takes for it.
fn catalogue(code: &str) -> (String, String) {
    let lower = code.to_ascii_lowercase();
    if let Some((_, title, launch)) = CATALOGUE
        .iter()
        .find(|(known, _, _)| *known == lower.as_str())
    {
        return ((*title).to_owned(), (*launch).to_owned());
    }
    (prettify(code), code.to_owned())
}

/// A title from a code the catalogue does not name: `some_game` → "Some
/// game".
fn prettify(code: &str) -> String {
    let title = code.replace('_', " ");
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => title,
    }
}

/// The products Zendo names, with the launch code the client takes for each.
/// A product not here is titled from its code and launches with the code as
/// it is.
const CATALOGUE: &[(&str, &str, &str)] = &[
    ("s1", "StarCraft Remastered", "S1"),
    ("s2", "StarCraft II", "S2"),
    ("sca", "StarCraft Anthology", "Starcraft"),
    ("wow", "World of Warcraft", "WoW"),
    (
        "wow_classic",
        "World of Warcraft Classic",
        "WoW_wow_classic",
    ),
    (
        "wow_classic_era",
        "World of Warcraft Classic Era",
        "WoW_wow_classic_era",
    ),
    (
        "wow_classic_ptr",
        "World of Warcraft Classic PTR",
        "WoW_wow_classic",
    ),
    (
        "wow_classic_era_ptr",
        "World of Warcraft Classic Era PTR",
        "WoW_wow_classic_era",
    ),
    ("wow_beta", "World of Warcraft Beta", "WoW"),
    ("pro", "Overwatch 2", "Pro"),
    ("prometheus", "Overwatch 2", "Pro"),
    ("prometheus_dev", "Overwatch 2 PTR", "Pro"),
    ("w1", "Warcraft: Orcs & Humans", "W1"),
    ("w1r", "Warcraft I: Remastered", "W1R"),
    ("w2bn", "Warcraft II: Battle.net Edition", "W2BN"),
    ("w2r", "Warcraft II: Remastered", "W2R"),
    ("w3", "Warcraft III: Reforged", "W3"),
    ("w3roc", "Warcraft III: Reign of Chaos", "Warcraft III"),
    ("w3tft", "Warcraft III: The Frozen Throne", "Warcraft III"),
    ("wild", "Warcraft III: Reforged", "W3"),
    ("gryphon", "Warcraft Rumble", "GRY"),
    ("hsb", "Hearthstone", "WTCG"),
    ("wtcg", "Hearthstone", "WTCG"),
    ("hero", "Heroes of the Storm", "Hero"),
    ("d2", "Diablo II", "Diablo II"),
    ("d2lod", "Diablo II: Lord of Destruction", "Diablo II"),
    ("osi", "Diablo II: Resurrected", "OSI"),
    ("d3", "Diablo III", "D3"),
    ("d3cn", "Diablo III", "D3CN"),
    ("fenris", "Diablo IV", "Fen"),
    ("fenris_beta", "Diablo IV Beta", "Fen"),
    ("fenris_ptr", "Diablo IV PTR", "Fen"),
    ("anbs", "Diablo Immortal", "ANBS"),
    ("viper", "Call of Duty: Black Ops 4", "VIPR"),
    ("vipr", "Call of Duty: Black Ops 4", "VIPR"),
    ("odin", "Call of Duty: Modern Warfare", "ODIN"),
    ("lazarus", "Call of Duty: MW2 Campaign Remastered", "LAZR"),
    ("lazr", "Call of Duty: MW2 Campaign Remastered", "LAZR"),
    ("zeus", "Call of Duty: Black Ops Cold War", "ZEUS"),
    ("auks", "Call of Duty: Modern Warfare II", "AUKS"),
    ("codhq", "Call of Duty HQ", "CODHQ"),
    ("fore", "Call of Duty: Vanguard", "FORE"),
    ("rtro", "Blizzard Arcade Collection", "RTRO"),
    ("wlby", "Crash Bandicoot 4: It's About Time", "WLBY"),
];

/// A product record in the database, the fields Zendo reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Product {
    code: String,
    install_path: Option<String>,
    installed: bool,
    playable: bool,
}

/// A protobuf wire reader: varints, length-delimited bytes, and skipping the
/// fields Zendo does not read.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.data.len()
    }

    fn varint(&mut self) -> Option<u64> {
        let mut value = 0_u64;
        let mut shift = 0_u32;
        loop {
            let byte = *self.data.get(self.at)?;
            self.at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn field(&mut self) -> Option<(u32, u8)> {
        let key = self.varint()?;
        let field = u32::try_from(key >> 3).ok()?;
        let wire = u8::try_from(key & 7).ok()?;
        (field > 0).then_some((field, wire))
    }

    fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = usize::try_from(self.varint()?).ok()?;
        let end = self.at.checked_add(len)?;
        let slice = self.data.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn skip(&mut self, wire: u8) -> Option<()> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => self.at = self.at.checked_add(8).filter(|&at| at <= self.data.len())?,
            2 => {
                self.bytes()?;
            }
            5 => self.at = self.at.checked_add(4).filter(|&at| at <= self.data.len())?,
            _ => return None,
        }
        Some(())
    }
}

/// The products in a `product.db`, or `None` when it is not one.
fn products(data: &[u8]) -> Option<Vec<Product>> {
    let mut reader = Reader::new(data);
    let mut found = Vec::new();
    while !reader.done() {
        let (field, wire) = reader.field()?;
        if field == 1 && wire == 2 {
            found.push(product(reader.bytes()?)?);
        } else {
            reader.skip(wire)?;
        }
    }
    Some(found)
}

/// One product record: field 1 uid, field 2 product code, field 3 install
/// settings, field 4 cached state.
fn product(data: &[u8]) -> Option<Product> {
    let mut reader = Reader::new(data);
    let mut product = Product::default();
    while !reader.done() {
        let (field, wire) = reader.field()?;
        match (field, wire) {
            (2, 2) => product.code = text(reader.bytes()?)?,
            (3, 2) => product.install_path = user_settings(reader.bytes()?),
            (4, 2) => {
                let (installed, playable) = cached_state(reader.bytes()?)?;
                product.installed = installed;
                product.playable = playable;
            }
            _ => reader.skip(wire)?,
        }
    }
    Some(product)
}

/// Field 1 is the Windows install path.
fn user_settings(data: &[u8]) -> Option<String> {
    let mut reader = Reader::new(data);
    let mut path = None;
    while !reader.done() {
        let (field, wire) = reader.field()?;
        if field == 1 && wire == 2 {
            path = Some(text(reader.bytes()?)?);
        } else {
            reader.skip(wire)?;
        }
    }
    path
}

/// Field 1 is the base state: field 1 installed, field 2 playable.
fn cached_state(data: &[u8]) -> Option<(bool, bool)> {
    let mut reader = Reader::new(data);
    let mut state = None;
    while !reader.done() {
        let (field, wire) = reader.field()?;
        if field == 1 && wire == 2 {
            state = Some(base_state(reader.bytes()?)?);
        } else {
            reader.skip(wire)?;
        }
    }
    state
}

fn base_state(data: &[u8]) -> Option<(bool, bool)> {
    let mut reader = Reader::new(data);
    let mut installed = false;
    let mut playable = false;
    while !reader.done() {
        let (field, wire) = reader.field()?;
        if wire == 0 && (field == 1 || field == 2) {
            let value = reader.varint()? != 0;
            if field == 1 {
                installed = value;
            } else {
                playable = value;
            }
        } else {
            reader.skip(wire)?;
        }
    }
    Some((installed, playable))
}

fn text(data: &[u8]) -> Option<String> {
    std::str::from_utf8(data).ok().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zendo-battlenet-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn varint(out: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            out.push(u8::try_from(value & 0x7f).unwrap() | 0x80);
            value >>= 7;
        }
        out.push(u8::try_from(value).unwrap());
    }

    fn key(out: &mut Vec<u8>, field: u32, wire: u8) {
        varint(out, (u64::from(field) << 3) | u64::from(wire));
    }

    fn string(out: &mut Vec<u8>, field: u32, value: &str) {
        key(out, field, 2);
        varint(out, value.len() as u64);
        out.extend(value.as_bytes());
    }

    fn message(out: &mut Vec<u8>, field: u32, body: &[u8]) {
        key(out, field, 2);
        varint(out, body.len() as u64);
        out.extend(body);
    }

    fn boolean(out: &mut Vec<u8>, field: u32, value: bool) {
        key(out, field, 0);
        varint(out, u64::from(value));
    }

    /// A `product.db` as the Agent writes one, for the products given.
    fn product_db(products: &[(&str, &str, bool, bool)]) -> Vec<u8> {
        let mut db = Vec::new();
        for (uid, code, installed, playable) in products {
            let mut settings = Vec::new();
            string(&mut settings, 1, &format!(r"C:\Games\{code}"));
            let mut state = Vec::new();
            boolean(&mut state, 1, *installed);
            boolean(&mut state, 2, *playable);
            let mut cached = Vec::new();
            message(&mut cached, 1, &state);
            let mut record = Vec::new();
            string(&mut record, 1, uid);
            string(&mut record, 2, code);
            message(&mut record, 3, &settings);
            message(&mut record, 4, &cached);
            message(&mut db, 1, &record);
        }
        db
    }

    #[test]
    fn reads_the_products_in_a_database() {
        let db = product_db(&[
            ("wow", "wow", true, true),
            ("pro", "pro", true, false),
            ("agent", "agent", true, true),
            ("absent", "d3", false, false),
        ]);
        let found = products(&db).unwrap();
        assert_eq!(found.len(), 4);
        assert_eq!(found[0].code, "wow");
        assert!(found[0].installed && found[0].playable);
        assert_eq!(found[0].install_path.as_deref(), Some(r"C:\Games\wow"));
        assert_eq!(found[1].code, "pro");
        assert!(found[1].installed && !found[1].playable);
        assert!(!found[3].installed && !found[3].playable);
    }

    #[test]
    fn refuses_something_that_is_not_a_database() {
        // A key of field 0 is not a field.
        assert!(products(&[0x00]).is_none());
        // A length past the end.
        assert!(products(&[0x0a, 0xff]).is_none());
    }

    #[test]
    fn skips_fields_it_does_not_read() {
        let mut db = Vec::new();
        let mut record = Vec::new();
        string(&mut record, 1, "uid");
        string(&mut record, 2, "d4");
        string(&mut record, 9, "something else");
        message(&mut db, 1, &record);
        let found = products(&db).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "d4");
    }

    #[test]
    fn makes_a_game_with_the_client_and_its_launch_code() {
        let dir = scratch("game");
        let prefix = dir.join("pfx");
        let client = prefix.join("drive_c/Program Files (x86)/Battle.net/Battle.net.exe");
        fs::create_dir_all(client.parent().unwrap()).unwrap();
        fs::write(&client, "").unwrap();
        let db = prefix.join(PRODUCT_DB);
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        fs::write(&db, product_db(&[("wow", "wow", true, true)])).unwrap();

        let scan = scan(std::slice::from_ref(&prefix));
        assert_eq!(scan.games.len(), 1, "{:?}", scan.problems);
        let game = &scan.games[0];
        assert_eq!(game.title, "World of Warcraft");
        assert_eq!(game.source, Source::BattleNet);
        assert!(game.id.starts_with("battlenet:wow@"));
        match &game.launch {
            Launch::Prefix {
                prefix: at,
                runner,
                program,
                args,
                ..
            } => {
                assert_eq!(at, &prefix);
                assert_eq!(*runner, WineRunner::Wine);
                assert_eq!(program, &client);
                assert_eq!(args, &["--exec=launch WoW"]);
            }
            other => panic!("unexpected launch: {other:?}"),
        }
        assert_eq!(
            game.details.install_path.as_deref(),
            Some(prefix.join("drive_c/Games/wow").as_path())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn leaves_the_tools_and_the_absent_out() {
        let dir = scratch("tools");
        let prefix = dir.join("prefix");
        fs::create_dir_all(&prefix).unwrap();
        let db = prefix.join(PRODUCT_DB);
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        fs::write(
            &db,
            product_db(&[
                ("agent", "agent", true, true),
                ("bna", "bna", true, true),
                ("gone", "d3", false, false),
            ]),
        )
        .unwrap();
        let scan = scan(&[prefix]);
        assert!(scan.games.is_empty(), "{:?}", scan.games);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_products_it_knows_and_the_rest_from_their_code() {
        assert_eq!(catalogue("wow"), ("World of Warcraft".into(), "WoW".into()));
        assert_eq!(
            catalogue("W3TFT"),
            (
                "Warcraft III: The Frozen Throne".into(),
                "Warcraft III".into()
            )
        );
        assert_eq!(
            catalogue("unknown_game"),
            ("Unknown game".into(), "unknown_game".into())
        );
    }

    #[test]
    fn maps_windows_paths_into_the_prefix() {
        let prefix = Path::new("/pfx");
        assert_eq!(
            unix_path(prefix, r"C:\Program Files\Game"),
            Some(PathBuf::from("/pfx/drive_c/Program Files/Game"))
        );
        assert_eq!(
            unix_path(prefix, r"D:\Games\Game"),
            None,
            "another drive without a dosdevices link"
        );
        assert_eq!(unix_path(prefix, "relative"), None);
    }

    #[test]
    fn turns_a_filetime_and_a_big_number_into_unix_time() {
        // 2020-01-01 00:00:00 UTC as a FILETIME.
        assert_eq!(
            stamp(&serde_json::json!(132_223_104_000_000_000_i64)),
            Some(1_577_836_800)
        );
        assert_eq!(
            stamp(&serde_json::json!(1_577_836_800_000_i64)),
            Some(1_577_836_800)
        );
        assert_eq!(
            stamp(&serde_json::json!(1_577_836_800_u64)),
            Some(1_577_836_800)
        );
        assert_eq!(stamp(&serde_json::json!("1577836800")), Some(1_577_836_800));
        assert_eq!(stamp(&serde_json::json!(0)), None);
    }

    #[test]
    fn tells_bottles_proton_and_wine_apart() {
        let dir = scratch("runner");
        let wine = dir.join("wine");
        fs::create_dir_all(&wine).unwrap();
        assert_eq!(runner(&wine), WineRunner::Wine);

        // A Steam-style Proton prefix: `pfx` beside a `version` file.
        let compat = dir.join("compatdata/123");
        let pfx = compat.join("pfx");
        fs::create_dir_all(&pfx).unwrap();
        fs::write(compat.join("version"), "proton").unwrap();
        assert_eq!(runner(&pfx), WineRunner::Proton);

        // A Bottles prefix: `bottle.yml` names the bottle.
        let bottles = dir.join("bottles/Games");
        fs::create_dir_all(&bottles).unwrap();
        fs::write(bottles.join("bottle.yml"), "Name: \"Games\"\n").unwrap();
        assert_eq!(runner(&bottles), WineRunner::Bottles);
        assert_eq!(bottle(&bottles).as_deref(), Some("Games"));

        // Zendo's own prefix, whatever made it: Battle.net's client crashes
        // in stock Wine, so it runs on Proton.
        let managed = dir.join("Prefixes/Battle.net");
        fs::create_dir_all(&managed).unwrap();
        assert_eq!(runner(&managed), WineRunner::Proton);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_the_client_in_a_prefix() {
        let dir = scratch("client");
        let prefix = dir.join("prefix");
        assert_eq!(client_in(&prefix), None);
        let client = prefix.join(CLIENTS[0]);
        fs::create_dir_all(client.parent().unwrap()).unwrap();
        fs::write(&client, "").unwrap();
        assert_eq!(client_in(&prefix), Some(client));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_a_last_played_from_the_clients_config() {
        let dir = scratch("played");
        let prefix = dir.join("prefix");
        let config =
            prefix.join("drive_c/users/steamuser/AppData/Roaming/Battle.net/Battle.net.config");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, r#"{"Games":{"wow":{"LastPlayed":1577836800}}}"#).unwrap();
        let last = last_played(&prefix);
        assert_eq!(last.get("wow"), Some(&1_577_836_800));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The real installer, into a scratch cache; run with
    /// `cargo test -p zendo-core battlenet::tests::downloads_the_real_installer -- --ignored`.
    #[test]
    #[ignore = "needs the network and a scratch XDG_CACHE_HOME"]
    fn downloads_the_real_installer() {
        let cache = std::env::var("XDG_CACHE_HOME").unwrap_or_default();
        assert!(
            cache.contains("zendo-battlenet-live"),
            "set XDG_CACHE_HOME to a scratch folder named zendo-battlenet-live"
        );
        let stop = AtomicBool::new(false);
        let path = download_installer(&stop, &mut |_, _| {}).unwrap();
        let start = fs::read(&path).unwrap();
        assert_eq!(&start[..2], b"MZ", "the installer is a Windows program");
    }
}
