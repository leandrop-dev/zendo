//! Bethesda games' plugins (`.esm`, `.esp`, `.esl` in `Data`): which load
//! and in what order. The game reads that from `plugins.txt` in its folder
//! of the prefix's `AppData/Local`, in one of three ways ([`Style`]); a
//! plugin's header names the masters it needs loaded before it.
//!
//! Each game's own plugins ([`Rules::base`]) load first in the order
//! Bethesda released them; the first `implicit` of them always load, and
//! are not the user's to turn off.

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
    /// Its folder in `AppData/Local`.
    pub appdata: &'static str,
    pub style: Style,
    /// Its own plugins, in the order they load.
    pub base: &'static [&'static str],
    /// How many of `base` always load.
    pub implicit: usize,
}

pub const FALLOUT3: Rules = Rules {
    appdata: "Fallout3",
    style: Style::Timestamps,
    base: &[
        "Fallout3.esm",
        "Anchorage.esm",
        "ThePitt.esm",
        "BrokenSteel.esm",
        "PointLookout.esm",
        "Zeta.esm",
    ],
    implicit: 1,
};

pub const NEW_VEGAS: Rules = Rules {
    appdata: "FalloutNV",
    style: Style::Timestamps,
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
    implicit: 1,
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
    implicit: 2,
};

pub const SKYRIM_SE: Rules = Rules {
    appdata: "Skyrim Special Edition",
    style: Style::Asterisk,
    base: SKYRIM_BASE,
    implicit: 5,
};

pub const FALLOUT4: Rules = Rules {
    appdata: "Fallout4",
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
};

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
    /// Flagged light (`.esl`-like, Skyrim SE and Fallout 4).
    pub light: bool,
    /// The plugins it needs loaded before it.
    pub masters: Vec<String>,
}

/// Reads a plugin's `TES4` record, with the 24-byte record header of every
/// game here.
///
/// # Errors
///
/// When the file cannot be read or is not a plugin.
pub fn read_header(path: &Path) -> io::Result<Header> {
    let mut start = [0_u8; 24];
    let mut file = File::open(path)?;
    file.read_exact(&mut start)?;
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
        light: flags & 0x200 != 0,
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
/// released them, then the masters and then the rest, each in the order
/// `stored` keeps, new ones last. A plugin `stored` does not name is on
/// when `default_on` says.
#[must_use]
pub fn arrange(
    rules: &Rules,
    found: Vec<Found>,
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

/// The files the game reads, with what they should hold for this order:
/// each plugin's name and whether it is on.
#[must_use]
pub fn files_for(rules: &Rules, order: &[(String, bool)]) -> Vec<(&'static str, String)> {
    let implicit = |name: &str| {
        rules.base[..rules.implicit]
            .iter()
            .any(|base| base.eq_ignore_ascii_case(name))
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
    match rules.style {
        Style::Timestamps => vec![("plugins.txt", lines(on))],
        Style::TextFile => vec![
            (
                "plugins.txt",
                lines(on.into_iter().filter(|name| !implicit(name)).collect()),
            ),
            (
                "loadorder.txt",
                lines(order.iter().map(|(name, _)| name.clone()).collect()),
            ),
        ],
        Style::Asterisk => vec![(
            "plugins.txt",
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
        let header = read_header(&path).unwrap();
        assert_eq!(
            header,
            Header {
                master: true,
                light: true,
                masters: vec!["Fallout3.esm".into(), "Anchorage.esm".into()],
            }
        );
        fs::write(&path, b"TES3 an old one").unwrap();
        assert!(read_header(&path).is_err());
        let _ = fs::remove_dir_all(&dir);
        assert!(is_plugin("Mod.ESP") && is_plugin("a.esl") && !is_plugin(".esp"));
        assert!(!is_plugin("readme.txt"));
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
            files_for(&SKYRIM_SE, &order),
            [("plugins.txt", "Mod.esp\r\n*Other.esp\r\n".to_owned())]
        );
        assert_eq!(
            files_for(&SKYRIM, &order),
            [
                ("plugins.txt", "Dawnguard.esm\r\nOther.esp\r\n".to_owned()),
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
            files_for(&FALLOUT3, &fallout),
            [("plugins.txt", "Fallout3.esm\r\nMod.esp\r\n".to_owned())]
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
        let plugins = arrange(&FALLOUT3, found, &stored, |found| found.from.is_some());
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

    /// Reads the headers of a real game's plugins, as in
    /// `ZENDO_TEST_DATA=<game>/Data cargo test -p zendo-core plugins -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the plugins in the folder in ZENDO_TEST_DATA"]
    fn reads_a_real_games_plugins() {
        let data = std::env::var("ZENDO_TEST_DATA").expect("ZENDO_TEST_DATA");
        let mut read = 0;
        for entry in fs::read_dir(data).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_plugin(&name) {
                continue;
            }
            let header = read_header(&entry.path()).unwrap();
            println!(
                "{name}: master {}, needs {:?}",
                header.master, header.masters
            );
            read += 1;
        }
        assert_ne!(read, 0);
    }
}
