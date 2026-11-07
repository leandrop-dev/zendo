//! The games Zendo knows: where their mods go, their names on Nexus,
//! and how they keep their plugin order.

use crate::Game;
use crate::Source;
use crate::plugins;
use crate::protondb;

/// A game whose mods Zendo knows where to put.
#[derive(Debug, PartialEq, Eq)]
pub struct Known {
    /// Its names in the stores.
    pub titles: &'static [&'static str],
    /// Its app ids on Steam, editions included.
    pub steam: &'static [u32],
    /// Its name on Nexus Mods, in links and the API.
    pub nexus: &'static str,
    /// Where mods go, from the game's folder; empty for the folder itself.
    pub folder: &'static str,
    /// A top folder some archives wrap their files in that is the mod
    /// folder itself (`Data` for Skyrim), whose contents are taken.
    pub strip: &'static [&'static str],
    /// The folders a mod packed right starts with; folders around them are
    /// removed until one shows.
    pub tops: &'static [&'static str],
    /// How the game keeps its plugin order, for Bethesda's.
    pub plugins: Option<&'static plugins::Rules>,
}

/// The games Zendo knows, from where each one's mods go.
pub const KNOWN: &[Known] = &[
    Known {
        titles: &["Stardew Valley"],
        steam: &[413_150],
        nexus: "stardewvalley",
        folder: "Mods",
        strip: &["Mods"],
        tops: &[],
        plugins: None,
    },
    Known {
        titles: &[
            "The Elder Scrolls V: Skyrim Special Edition",
            "Skyrim Special Edition",
            "The Elder Scrolls V: Skyrim Anniversary Edition",
        ],
        steam: &[489_830],
        nexus: "skyrimspecialedition",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::SKYRIM_SE),
    },
    Known {
        titles: &["The Elder Scrolls V: Skyrim"],
        steam: &[72_850],
        nexus: "skyrim",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::SKYRIM),
    },
    Known {
        titles: &["Fallout 4"],
        steam: &[377_160],
        nexus: "fallout4",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::FALLOUT4),
    },
    Known {
        titles: &[
            "The Elder Scrolls IV: Oblivion",
            "The Elder Scrolls IV: Oblivion Game of the Year Edition",
            "The Elder Scrolls IV: Oblivion Game of the Year Edition Deluxe",
        ],
        steam: &[22_330, 900_883],
        nexus: "oblivion",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::OBLIVION),
    },
    Known {
        titles: &["Fallout 3", "Fallout 3 - Game of the Year Edition"],
        steam: &[22_300, 22_370],
        nexus: "fallout3",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::FALLOUT3),
    },
    Known {
        titles: &["Fallout: New Vegas", "Fallout New Vegas"],
        steam: &[22_380],
        nexus: "newvegas",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::NEW_VEGAS),
    },
    Known {
        titles: &["Starfield"],
        steam: &[1_716_740],
        nexus: "starfield",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
        plugins: Some(&plugins::STARFIELD),
    },
    Known {
        titles: &["Cyberpunk 2077"],
        steam: &[1_091_500],
        nexus: "cyberpunk2077",
        folder: "",
        strip: &[],
        tops: &["archive", "bin", "r6", "red4ext", "engine", "mods"],
        plugins: None,
    },
];

/// The game Zendo knows this one as: by Steam id, or by title.
#[must_use]
pub fn known(game: &Game) -> Option<&'static Known> {
    let steam = game
        .id
        .strip_prefix("steam:")
        .filter(|_| game.source == Source::Steam)
        .and_then(|app| app.parse::<u32>().ok());
    let title = protondb::plain(&game.title);
    KNOWN.iter().find(|known| match steam {
        Some(app) => known.steam.contains(&app),
        None => known
            .titles
            .iter()
            .any(|name| protondb::plain(name) == title),
    })
}
