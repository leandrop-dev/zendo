//! Bethesda games' plugins (`.esm`, `.esp`, `.esl` in `Data`): which load
//! and in what order. The game reads that from `Plugins.txt` in its folder
//! of the prefix's `AppData/Local`, in one of three ways ([`Style`]); a
//! plugin's header names the masters it needs loaded before it.
//!
//! Each game's own plugins ([`Rules::base`]) load first in the order
//! Bethesda released them; the first `implicit` of them always load, and
//! are not the user's to turn off, nor are Creation Club plugins the game
//! lists or, for New Vegas, plugins a `.nam` file names.
//!
//! The games' folders, files and plugin names were checked against LOOT's
//! libloadorder (GPL-3.0, read for facts only).

#[cfg(test)]
use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::{Game, Launch, Source};

/// How a game reads its plugin list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `plugins.txt` lists the plugins on; they load in the order of the
    /// files' modified times (Fallout 3, New Vegas).
    Timestamps,
    /// `plugins.txt` lists the plugins on, `loadorder.txt` every plugin in
    /// order (Skyrim).
    TextFile,
    /// `plugins.txt` lists every plugin in order, the ones on marked with
    /// `*`; the implicit ones are left out (Skyrim Special Edition,
    /// Fallout 4).
    Asterisk,
}

/// Where and how a game keeps its plugin list.
#[derive(Debug, PartialEq, Eq)]
pub struct Rules {
    /// Its folder in `AppData/Local`, the copy from Steam's.
    pub appdata: &'static str,
    /// Other stores' copies: a file only that store installs in the game's
    /// folder, and the copy's folder in `AppData/Local`.
    pub stores: &'static [(&'static str, &'static str)],
    pub style: Style,
    /// Its own plugins, in the order they load.
    pub base: &'static [&'static str],
    /// How many of `base` always load.
    pub implicit: usize,
    /// The length of a record's header: 20 for Oblivion, 24 after.
    pub header: usize,
    /// The header flag of a light plugin (`0x200`, `0x100` for Starfield).
    pub light: u32,
    /// The file in the game's folder listing Creation Club plugins, which
    /// load when there, after the game's own.
    pub ccc: Option<&'static str>,
    /// The game's folder in `Documents/My Games`, where its own `ccc` file
    /// wins over the game folder's (Starfield).
    pub my_games: Option<&'static str>,
    /// A `.nam` file loads the plugin of its name (New Vegas).
    pub nam: bool,
    /// The game's `.ini`, whose `bUseMyGamesDirectory=0` puts the list in
    /// the game's folder instead (Oblivion).
    pub ini: Option<&'static str>,
}

const RULES: Rules = Rules {
    appdata: "",
    stores: &[],
    style: Style::Timestamps,
    base: &[],
    implicit: 1,
    header: 24,
    light: 0x200,
    ccc: None,
    my_games: None,
    nam: false,
    ini: None,
};

pub const OBLIVION: Rules = Rules {
    appdata: "Oblivion",
    base: &["Oblivion.esm"],
    header: 20,
    ini: Some("Oblivion.ini"),
    ..RULES
};

pub const FALLOUT3: Rules = Rules {
    appdata: "Fallout3",
    base: &[
        "Fallout3.esm",
        "Anchorage.esm",
        "ThePitt.esm",
        "BrokenSteel.esm",
        "PointLookout.esm",
        "Zeta.esm",
    ],
    ..RULES
};

pub const NEW_VEGAS: Rules = Rules {
    appdata: "FalloutNV",
    stores: &[("EOSSDK-Win32-Shipping.dll", "FalloutNV_Epic")],
    base: &[
        "FalloutNV.esm",
        "DeadMoney.esm",
        "HonestHearts.esm",
        "OldWorldBlues.esm",
        "LonesomeRoad.esm",
        "GunRunnersArsenal.esm",
        "ClassicPack.esm",
        "MercenaryPack.esm",
        "TribalPack.esm",
        "CaravanPack.esm",
    ],
    nam: true,
    ..RULES
};

const SKYRIM_BASE: &[&str] = &[
    "Skyrim.esm",
    "Update.esm",
    "Dawnguard.esm",
    "HearthFires.esm",
    "Dragonborn.esm",
];

pub const SKYRIM: Rules = Rules {
    appdata: "Skyrim",
    style: Style::TextFile,
    base: SKYRIM_BASE,
    // `Update.esm` always loads too.
    implicit: 2,
    ..RULES
};

pub const SKYRIM_SE: Rules = Rules {
    appdata: "Skyrim Special Edition",
    stores: &[
        ("Galaxy64.dll", "Skyrim Special Edition GOG"),
        ("EOSSDK-Win64-Shipping.dll", "Skyrim Special Edition EPIC"),
    ],
    style: Style::Asterisk,
    base: SKYRIM_BASE,
    implicit: 5,
    ccc: Some("Skyrim.ccc"),
    ..RULES
};

pub const FALLOUT4: Rules = Rules {
    appdata: "Fallout4",
    stores: &[("EOSSDK-Win64-Shipping.dll", "Fallout4 EPIC")],
    style: Style::Asterisk,
    base: &[
        "Fallout4.esm",
        "DLCRobot.esm",
        "DLCworkshop01.esm",
        "DLCCoast.esm",
        "DLCworkshop02.esm",
        "DLCworkshop03.esm",
        "DLCNukaWorld.esm",
        "DLCUltraHighResolution.esm",
    ],
    implicit: 8,
    ccc: Some("Fallout4.ccc"),
    ..RULES
};

const STARFIELD_BASE: &[&str] = &[
    "Starfield.esm",
    "Constellation.esm",
    "OldMars.esm",
    "ShatteredSpace.esm",
    "SFBGS003.esm",
    "SFBGS004.esm",
    "SFBGS006.esm",
    "SFBGS007.esm",
    "SFBGS008.esm",
    "SFBGS00D.esm",
    "SFBGS047.esm",
    "SFBGS050.esm",
];

pub const STARFIELD: Rules = Rules {
    appdata: "Starfield",
    style: Style::Asterisk,
    base: STARFIELD_BASE,
    implicit: STARFIELD_BASE.len(),
    light: 0x100,
    ccc: Some("Starfield.ccc"),
    my_games: Some("Starfield"),
    ..RULES
};

/// The names the game's list files go by: it reads `Plugins.txt`, its
/// launcher writes `plugins.txt`; one already there keeps its spelling.
pub const PLUGINS_TXT: &str = "Plugins.txt";
pub const LOADORDER_TXT: &str = "loadorder.txt";

impl Rules {
    /// The game's folder in `AppData/Local`, for the store its copy came
    /// from.
    #[must_use]
    pub fn appdata_for(&self, game_dir: &Path) -> &'static str {
        self.stores
            .iter()
            .find(|(file, _)| game_dir.join(file).exists())
            .map_or(self.appdata, |(_, folder)| folder)
    }

    /// Whether the game keeps its list in its own folder (its `.ini` says
    /// `bUseMyGamesDirectory=0`).
    #[must_use]
    pub fn list_in_game(&self, game_dir: &Path) -> bool {
        let Some(ini) = self.ini else {
            return false;
        };
        let Ok(bytes) = std::fs::read(game_dir.join(ini)) else {
            return false;
        };
        ini_value(
            &String::from_utf8_lossy(&bytes),
            "General",
            "bUseMyGamesDirectory",
        )
        .is_some_and(|value| value == "0")
    }

    /// Plugins that load whenever they are there, besides the game's own:
    /// those its Creation Club file lists (the one in `my_games` first,
    /// when given), and for New Vegas those a `.nam` file names.
    #[must_use]
    pub fn also_implicit(
        &self,
        data: &Path,
        game_dir: &Path,
        my_games: Option<&Path>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(ccc) = self.ccc {
            let files = my_games
                .into_iter()
                .map(|dir| dir.join(ccc))
                .chain([game_dir.join(ccc)]);
            for file in files {
                if let Ok(bytes) = std::fs::read(&file) {
                    names.extend(
                        String::from_utf8_lossy(&bytes)
                            .lines()
                            .map(str::trim)
                            .filter(|line| !line.is_empty())
                            .map(str::to_owned),
                    );
                    break;
                }
            }
        }
        if self.nam {
            let mut stems: Vec<String> = std::fs::read_dir(data)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    // Any case, as `is_plugin` reads the other extensions.
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let path = Path::new(&name);
                    let stem = path.file_stem()?.to_str()?;
                    path.extension()?
                        .to_str()?
                        .eq_ignore_ascii_case("nam")
                        .then(|| stem.to_owned())
                })
                .collect();
            stems.sort();
            for stem in stems {
                names.push(format!("{stem}.esm"));
                names.push(format!("{stem}.esp"));
            }
        }
        names
    }
}

/// A value in an `.ini` file's section, names in any case.
fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            inside = name.trim().eq_ignore_ascii_case(section);
        } else if inside
            && let Some((name, value)) = line.split_once('=')
            && name.trim().eq_ignore_ascii_case(key)
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

/// Whether a file name is a plugin's.
#[must_use]
pub fn is_plugin(name: &str) -> bool {
    matches!(extension(name).as_deref(), Some("esm" | "esp" | "esl"))
}

fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
}

/// What a plugin's header says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    /// Flagged a master (it loads with the `.esm` files).
    pub master: bool,
    /// Flagged light (`.esl`-like, Skyrim SE, Fallout 4 and Starfield).
    pub light: bool,
    /// The plugins it needs loaded before it.
    pub masters: Vec<String>,
}

/// Reads a plugin's `TES4` record, as the game's rules say records go.
///
/// # Errors
///
/// When the file cannot be read or is not a plugin.
pub fn read_header(path: &Path, rules: &Rules) -> io::Result<Header> {
    let mut start = [0_u8; 24];
    // `Rules` is public and has no constructor: a header outside the
    // records a plugin has is refused rather than panicking on a slice.
    if !(12..=start.len()).contains(&rules.header) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a record header is 12 to 24 bytes",
        ));
    }
    let mut file = File::open(path)?;
    file.read_exact(&mut start[..rules.header])?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a plugin");
    if &start[..4] != b"TES4" {
        return Err(bad());
    }
    let size = u32::from_le_bytes([start[4], start[5], start[6], start[7]]);
    let flags = u32::from_le_bytes([start[8], start[9], start[10], start[11]]);
    if size > 1 << 20 {
        return Err(bad());
    }
    let mut data = vec![0_u8; size as usize];
    file.read_exact(&mut data)?;
    let mut header = Header {
        master: flags & 0x1 != 0,
        light: flags & rules.light != 0,
        masters: Vec::new(),
    };
    let mut at = 0;
    // A field longer than 65535 bytes is announced by an `XXXX` field.
    let mut long: Option<usize> = None;
    while at + 6 <= data.len() {
        let kind = &data[at..at + 4];
        let short = usize::from(u16::from_le_bytes([data[at + 4], data[at + 5]]));
        let length = long.take().unwrap_or(short);
        let body = data.get(at + 6..at + 6 + length).ok_or_else(bad)?;
        match kind {
            b"XXXX" if body.len() >= 4 => {
                long = Some(u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize);
            }
            b"MAST" => {
                let name = body.split(|byte| *byte == 0).next().unwrap_or_default();
                // Windows-1252 in practice; names are ASCII nearly always.
                header
                    .masters
                    .push(name.iter().map(|&byte| char::from(byte)).collect());
            }
            _ => {}
        }
        at += 6 + length;
    }
    Ok(header)
}

/// A plugin found in the game's folder or a mod that is on.
#[derive(Debug, Clone)]
pub struct Found {
    pub name: String,
    /// The mod it comes in; `None` for the game's own.
    pub from: Option<String>,
    /// `None` when its header could not be read.
    pub header: Option<Header>,
}

/// A plugin in the order it loads, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub name: String,
    pub enabled: bool,
    /// The mod it comes in; `None` for the game's own.
    pub from: Option<String>,
    /// Loads among the masters: an `.esm` or `.esl`, or flagged one.
    pub master: bool,
    pub rank: Rank,
    /// Masters it needs that are not on.
    pub missing: Vec<String>,
    /// Masters it needs that load after it.
    pub after: Vec<String>,
    /// Its header could not be read.
    pub unreadable: bool,
}

/// Where a plugin may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// One of the game's own that always loads; not the user's to turn off.
    Implicit,
    /// One of the game's own, in its place first.
    Base,
    /// Any other, moved among its kind.
    Free,
}

/// Puts the plugins found in order: the game's own first as Bethesda
/// released them, then those that also always load (`also`, from
/// [`Rules::also_implicit`]), then the masters and then the rest, each in
/// the order `stored` keeps, new ones last. A plugin `stored` does not
/// name is on when `default_on` says.
#[must_use]
pub fn arrange(
    rules: &Rules,
    found: Vec<Found>,
    also: &[String],
    stored: &[(String, bool)],
    default_on: impl Fn(&Found) -> bool,
) -> Vec<Plugin> {
    let key = |name: &str| name.to_ascii_lowercase();
    let stored_on = |name: &str| {
        stored
            .iter()
            .find(|(other, _)| other.eq_ignore_ascii_case(name))
            .map(|(_, on)| *on)
    };
    let mut left = found;
    let mut take = |name: &str| {
        left.iter()
            .position(|found| key(&found.name) == key(name))
            .map(|at| left.remove(at))
    };
    let mut ordered: Vec<(Found, Rank)> = Vec::new();
    for (index, base) in rules.base.iter().enumerate() {
        if let Some(found) = take(base) {
            let rank = if index < rules.implicit {
                Rank::Implicit
            } else {
                Rank::Base
            };
            ordered.push((found, rank));
        }
    }
    for name in also {
        if let Some(found) = take(name) {
            ordered.push((found, Rank::Implicit));
        }
    }
    let mut rest: Vec<Found> = stored.iter().filter_map(|(name, _)| take(name)).collect();
    rest.append(&mut left);
    let is_master = |found: &Found| {
        matches!(extension(&found.name).as_deref(), Some("esm" | "esl"))
            || found.header.as_ref().is_some_and(|header| header.master)
    };
    let (masters, others): (Vec<Found>, Vec<Found>) = rest.into_iter().partition(is_master);
    ordered.extend(
        masters
            .into_iter()
            .chain(others)
            .map(|found| (found, Rank::Free)),
    );

    let mut plugins: Vec<Plugin> = ordered
        .into_iter()
        .map(|(found, rank)| Plugin {
            enabled: rank == Rank::Implicit
                || stored_on(&found.name).unwrap_or_else(|| default_on(&found)),
            master: is_master(&found),
            unreadable: found.header.is_none(),
            // The masters it names, sorted into the two below once the
            // order is known.
            missing: found
                .header
                .map(|header| header.masters)
                .unwrap_or_default(),
            name: found.name,
            from: found.from,
            rank,
            after: Vec::new(),
        })
        .collect();
    let on: Vec<String> = plugins
        .iter()
        .filter(|plugin| plugin.enabled)
        .map(|plugin| key(&plugin.name))
        .collect();
    for plugin in &mut plugins {
        let masters = std::mem::take(&mut plugin.missing);
        if !plugin.enabled {
            continue;
        }
        let mine = key(&plugin.name);
        let position = on.iter().position(|name| *name == mine).unwrap_or_default();
        for master in masters {
            match on.iter().position(|name| *name == key(&master)) {
                None => plugin.missing.push(master),
                Some(other) if other > position => plugin.after.push(master),
                Some(_) => {}
            }
        }
    }
    plugins
}

/// The order with one plugin moved a place earlier (`up`) or later:
/// only past a neighbour of its kind (master or not), never among the
/// game's own. `None` when it cannot move that way.
#[must_use]
pub fn moved(plugins: &[Plugin], name: &str, up: bool) -> Option<Vec<(String, bool)>> {
    let at = plugins.iter().position(|plugin| plugin.name == name)?;
    let to = neighbour(plugins, at, up)?;
    let mut order: Vec<(String, bool)> = plugins
        .iter()
        .map(|plugin| (plugin.name.clone(), plugin.enabled))
        .collect();
    order.swap(at, to);
    Some(order)
}

/// Whether the plugin at `at` can move a place earlier (`up`) or later.
#[must_use]
pub fn can_move(plugins: &[Plugin], at: usize, up: bool) -> bool {
    neighbour(plugins, at, up).is_some()
}

fn neighbour(plugins: &[Plugin], at: usize, up: bool) -> Option<usize> {
    let to = if up { at.checked_sub(1)? } else { at + 1 };
    let (one, other) = (plugins.get(at)?, plugins.get(to)?);
    (one.rank == Rank::Free && other.rank == Rank::Free && one.master == other.master).then_some(to)
}

/// The files the game reads, with what they should hold for this order
/// (each plugin's name and whether it is on); `implicit` are the plugins
/// that always load, which some lists leave out.
#[must_use]
pub fn files_for(
    style: Style,
    order: &[(String, bool)],
    implicit: &[String],
) -> Vec<(&'static str, String)> {
    let implicit = |name: &str| {
        implicit
            .iter()
            .any(|other| other.eq_ignore_ascii_case(name))
    };
    let lines = |lines: Vec<String>| {
        let mut text = lines.join("\r\n");
        text.push_str("\r\n");
        text
    };
    let on: Vec<String> = order
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(name, _)| name.clone())
        .collect();
    match style {
        Style::Timestamps => vec![(PLUGINS_TXT, lines(on))],
        Style::TextFile => vec![
            (
                PLUGINS_TXT,
                lines(on.into_iter().filter(|name| !implicit(name)).collect()),
            ),
            (
                LOADORDER_TXT,
                lines(order.iter().map(|(name, _)| name.clone()).collect()),
            ),
        ],
        Style::Asterisk => vec![(
            PLUGINS_TXT,
            lines(
                order
                    .iter()
                    .filter(|(name, _)| !implicit(name))
                    .map(|(name, enabled)| {
                        if *enabled {
                            format!("*{name}")
                        } else {
                            name.clone()
                        }
                    })
                    .collect(),
            ),
        )],
    }
}

/// The plugins a `plugins.txt` names, and whether each is on.
#[must_use]
pub fn read_list(text: &str, style: Style) -> Vec<(String, bool)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| match line.strip_prefix('*') {
            Some(name) => (name.to_owned(), true),
            None => (line.to_owned(), style != Style::Asterisk),
        })
        .collect()
}

/// The prefix's `AppData/Local`: Steam's `compatdata/<app>/pfx` beside
/// the game's library, or the prefix Zendo gives the games it starts.
/// `None` until the prefix has been made (the game started once).
#[must_use]
pub fn appdata_local(game: &Game, game_dir: &Path) -> Option<PathBuf> {
    let prefix = match (&game.launch, game.source) {
        (_, Source::Steam) => {
            let app = game.id.strip_prefix("steam:")?;
            // `<library>/steamapps/common/<game>`.
            game_dir
                .parent()?
                .parent()?
                .join("compatdata")
                .join(app)
                .join("pfx")
        }
        (Launch::Local { .. } | Launch::Engine { .. }, _) => crate::launch::prefix(game)?,
        _ => return None,
    };
    let user = prefix.join("drive_c/users/steamuser");
    user.is_dir().then(|| user.join("AppData/Local"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A plugin's first bytes: a `TES4` record with these flags, a `HEDR`
    /// and a `MAST` (with its `DATA`) per master.
    pub(crate) fn plugin(flags: u32, masters: &[&str]) -> Vec<u8> {
        let mut fields = Vec::new();
        let mut field = |kind: &[u8], body: &[u8]| {
            fields.extend_from_slice(kind);
            fields.extend_from_slice(&u16::try_from(body.len()).unwrap().to_le_bytes());
            fields.extend_from_slice(body);
        };
        field(b"HEDR", &[0; 12]);
        field(b"CNAM", b"someone\0");
        for master in masters {
            let mut name = master.as_bytes().to_vec();
            name.push(0);
            field(b"MAST", &name);
            field(b"DATA", &[0; 8]);
        }
        let mut bytes = b"TES4".to_vec();
        bytes.extend_from_slice(&u32::try_from(fields.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&fields);
        bytes.extend_from_slice(b"GRUP");
        bytes
    }

    #[test]
    fn reads_a_plugins_flags_and_masters() {
        let dir = std::env::temp_dir().join(format!("zendo-plugins-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Mod.esp");
        fs::write(&path, plugin(0x201, &["Fallout3.esm", "Anchorage.esm"])).unwrap();
        let header = read_header(&path, &FALLOUT4).unwrap();
        assert_eq!(
            header,
            Header {
                master: true,
                light: true,
                masters: vec!["Fallout3.esm".into(), "Anchorage.esm".into()],
            }
        );
        fs::write(&path, b"TES3 an old one").unwrap();
        assert!(read_header(&path, &FALLOUT4).is_err());
        let _ = fs::remove_dir_all(&dir);
        assert!(is_plugin("Mod.ESP") && is_plugin("a.esl") && !is_plugin(".esp"));
        assert!(!is_plugin("readme.txt"));
    }

    fn implicit(rules: &Rules) -> Vec<String> {
        rules.base[..rules.implicit]
            .iter()
            .map(|&name| name.to_owned())
            .collect()
    }

    /// A scratch folder, removed after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("zendo-plugins-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_oblivions_shorter_record_header() {
        let scratch = Scratch::new("oblivion");
        // Oblivion's records have no version fields: 20 bytes, not 24.
        let mut bytes = plugin(1, &["Oblivion.esm"]);
        bytes.drain(20..24);
        let path = scratch.0.join("Mod.esp");
        fs::write(&path, bytes).unwrap();
        let header = read_header(&path, &OBLIVION).unwrap();
        assert!(header.master);
        assert_eq!(header.masters, ["Oblivion.esm"]);
        // Starfield's light flag is another bit.
        fs::write(&path, plugin(0x100, &[])).unwrap();
        assert!(read_header(&path, &STARFIELD).unwrap().light);
        assert!(!read_header(&path, &FALLOUT4).unwrap().light);
    }

    #[test]
    fn finds_each_stores_folder_and_what_always_loads() {
        let scratch = Scratch::new("stores");
        let game = scratch.0.join("game");
        let data = game.join("Data");
        fs::create_dir_all(&data).unwrap();
        assert_eq!(SKYRIM_SE.appdata_for(&game), "Skyrim Special Edition");
        fs::write(game.join("EOSSDK-Win64-Shipping.dll"), "").unwrap();
        assert_eq!(SKYRIM_SE.appdata_for(&game), "Skyrim Special Edition EPIC");
        fs::write(game.join("Galaxy64.dll"), "").unwrap();
        assert_eq!(SKYRIM_SE.appdata_for(&game), "Skyrim Special Edition GOG");
        assert_eq!(FALLOUT4.appdata_for(&game), "Fallout4 EPIC");
        assert_eq!(NEW_VEGAS.appdata_for(&game), "FalloutNV");

        // Oblivion keeps its list in the game's folder when its ini says so.
        assert!(!OBLIVION.list_in_game(&game));
        fs::write(
            game.join("Oblivion.ini"),
            "[Display]\nbUseMyGamesDirectory=0\n",
        )
        .unwrap();
        assert!(!OBLIVION.list_in_game(&game), "another section");
        fs::write(
            game.join("Oblivion.ini"),
            "[General]\r\nSLocalSavePath=Saves\\\r\n busemygamesdirectory = 0\r\n",
        )
        .unwrap();
        assert!(OBLIVION.list_in_game(&game));
        assert!(!FALLOUT3.list_in_game(&game));

        fs::write(
            game.join("Skyrim.ccc"),
            "ccBGSSSE001-Fish.esm\r\n\r\nccQDRSSE001-SurvivalMode.esl\r\n",
        )
        .unwrap();
        assert_eq!(
            SKYRIM_SE.also_implicit(&data, &game, None),
            ["ccBGSSSE001-Fish.esm", "ccQDRSSE001-SurvivalMode.esl"]
        );
        // Starfield's own in My Games wins, even empty.
        let my_games = scratch.0.join("My Games/Starfield");
        fs::create_dir_all(&my_games).unwrap();
        fs::write(game.join("Starfield.ccc"), "SFBGS00A.esm\n").unwrap();
        assert_eq!(
            STARFIELD.also_implicit(&data, &game, Some(&my_games)),
            ["SFBGS00A.esm"]
        );
        fs::write(my_games.join("Starfield.ccc"), "").unwrap();
        assert_eq!(
            STARFIELD.also_implicit(&data, &game, Some(&my_games)),
            Vec::<String>::new()
        );

        fs::write(data.join("TribalPack.nam"), "").unwrap();
        assert_eq!(
            NEW_VEGAS.also_implicit(&data, &game, None),
            ["TribalPack.esm", "TribalPack.esp"]
        );

        // Those found load after the game's own, always on.
        let found = vec![
            found("Mod.esp", Some("Mod"), false, &[]),
            found("ccBGSSSE001-Fish.esm", None, true, &["Skyrim.esm"]),
            found("Skyrim.esm", None, true, &[]),
        ];
        let also = SKYRIM_SE.also_implicit(&data, &game, None);
        let plugins = arrange(
            &SKYRIM_SE,
            found,
            &also,
            &[("ccBGSSSE001-Fish.esm".into(), false)],
            |_| true,
        );
        assert_eq!(plugins[1].name, "ccBGSSSE001-Fish.esm");
        assert_eq!(plugins[1].rank, Rank::Implicit);
        assert!(plugins[1].enabled);
        let order = order(&plugins);
        let names: Vec<String> = plugins
            .iter()
            .filter(|plugin| plugin.rank == Rank::Implicit)
            .map(|plugin| plugin.name.clone())
            .collect();
        assert_eq!(
            files_for(Style::Asterisk, &order, &names),
            [(PLUGINS_TXT, "*Mod.esp\r\n".to_owned())]
        );
    }

    fn order(plugins: &[Plugin]) -> Vec<(String, bool)> {
        plugins
            .iter()
            .map(|plugin| (plugin.name.clone(), plugin.enabled))
            .collect()
    }

    #[test]
    fn writes_each_games_list_its_way() {
        let order: Vec<(String, bool)> = [
            ("Skyrim.esm", true),
            ("Update.esm", true),
            ("Dawnguard.esm", true),
            ("Mod.esp", false),
            ("Other.esp", true),
        ]
        .iter()
        .map(|(name, on)| ((*name).to_owned(), *on))
        .collect();
        assert_eq!(
            files_for(SKYRIM_SE.style, &order, &implicit(&SKYRIM_SE)),
            [("Plugins.txt", "Mod.esp\r\n*Other.esp\r\n".to_owned())]
        );
        assert_eq!(
            files_for(SKYRIM.style, &order, &implicit(&SKYRIM)),
            [
                ("Plugins.txt", "Dawnguard.esm\r\nOther.esp\r\n".to_owned()),
                (
                    "loadorder.txt",
                    "Skyrim.esm\r\nUpdate.esm\r\nDawnguard.esm\r\nMod.esp\r\nOther.esp\r\n"
                        .to_owned()
                ),
            ]
        );
        let fallout: Vec<(String, bool)> = vec![
            ("Fallout3.esm".into(), true),
            ("Zeta.esm".into(), false),
            ("Mod.esp".into(), true),
        ];
        assert_eq!(
            files_for(FALLOUT3.style, &fallout, &implicit(&FALLOUT3)),
            [("Plugins.txt", "Fallout3.esm\r\nMod.esp\r\n".to_owned())]
        );
        assert_eq!(
            read_list("# a comment\r\n*A.esp\r\nB.esp\r\n", Style::Asterisk),
            [("A.esp".to_owned(), true), ("B.esp".to_owned(), false)]
        );
        assert_eq!(
            read_list("Fallout3.esm\nMod.esp\n", Style::Timestamps),
            [
                ("Fallout3.esm".to_owned(), true),
                ("Mod.esp".to_owned(), true)
            ]
        );
    }

    fn found(name: &str, from: Option<&str>, master: bool, masters: &[&str]) -> Found {
        Found {
            name: name.into(),
            from: from.map(Into::into),
            header: Some(Header {
                master,
                light: false,
                masters: masters.iter().map(|&master| master.into()).collect(),
            }),
        }
    }

    #[test]
    fn puts_the_games_own_first_then_masters_and_tells_what_is_missing() {
        // As Steam's Fallout 3 installs them, the DLC dated before the game.
        let found = vec![
            found("Anchorage.esm", None, true, &["Fallout3.esm"]),
            found("BrokenSteel.esm", None, true, &["Fallout3.esm"]),
            found("Fallout3.esm", None, true, &[]),
            found("Zeta.esm", None, true, &["Fallout3.esm"]),
            found(
                "Patch.esp",
                Some("Patch"),
                false,
                &["Fallout3.esm", "Lib.esp"],
            ),
            found("Lib.esp", Some("Lib"), false, &["Fallout3.esm"]),
            found("Needs.esp", Some("Needs"), false, &["Gone.esm"]),
            found("Big.esp", Some("Big"), true, &["Fallout3.esm"]),
            Found {
                name: "Broken.esp".into(),
                from: Some("Broken".into()),
                header: None,
            },
        ];
        let stored = vec![
            ("Fallout3.esm".to_owned(), false),
            ("Zeta.esm".to_owned(), false),
            ("Patch.esp".to_owned(), true),
            ("Lib.esp".to_owned(), true),
        ];
        let plugins = arrange(&FALLOUT3, found, &[], &stored, |found| found.from.is_some());
        let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Fallout3.esm",
                "Anchorage.esm",
                "BrokenSteel.esm",
                "Zeta.esm",
                "Big.esp",
                "Patch.esp",
                "Lib.esp",
                "Needs.esp",
                "Broken.esp",
            ]
        );
        let on: Vec<bool> = plugins.iter().map(|plugin| plugin.enabled).collect();
        // The main master stays on; Anchorage, new to the list, takes the
        // default; Zeta stays off as stored.
        assert_eq!(
            on,
            [true, false, false, false, true, true, true, true, true]
        );
        assert_eq!(
            (plugins[0].rank, plugins[1].rank),
            (Rank::Implicit, Rank::Base)
        );
        assert!(plugins[4].master && !plugins[5].master);
        assert_eq!(plugins[5].after, ["Lib.esp"]);
        assert_eq!(plugins[7].missing, ["Gone.esm"]);
        assert!(plugins[8].unreadable && plugins[8].missing.is_empty());

        // Lib moves before Patch; not past Big (a master) or into the base.
        let order = moved(&plugins, "Lib.esp", true).unwrap();
        assert_eq!(order[5].0, "Lib.esp");
        assert!(moved(&plugins, "Patch.esp", true).is_none());
        assert!(moved(&plugins, "Big.esp", true).is_none());
        assert!(moved(&plugins, "Zeta.esm", false).is_none());
        assert!(moved(&plugins, "Broken.esp", false).is_none());
    }

    /// Reads the headers of a real game's plugins (`ZENDO_TEST_OBLIVION=1`
    /// for Oblivion's), as in
    /// `ZENDO_TEST_DATA=<game>/Data cargo test -p zendo-core plugins -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the plugins in the folder in ZENDO_TEST_DATA"]
    fn reads_a_real_games_plugins() {
        let data = std::env::var("ZENDO_TEST_DATA").expect("ZENDO_TEST_DATA");
        // Every game but Oblivion has the same record header.
        let rules = if std::env::var_os("ZENDO_TEST_OBLIVION").is_some() {
            &OBLIVION
        } else {
            &FALLOUT3
        };
        let mut read = 0;
        for entry in fs::read_dir(data).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_plugin(&name) {
                continue;
            }
            let header = read_header(&entry.path(), rules).unwrap();
            println!(
                "{name}: master {}, needs {:?}",
                header.master, header.masters
            );
            read += 1;
        }
        assert_ne!(read, 0);
    }
}
