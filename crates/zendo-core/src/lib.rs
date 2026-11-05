//! Zendo's library: the games found on this machine and how to start them.
//!
//! Every source is read without writing to it: Steam's library folders,
//! Heroic's installed games and `.desktop` files with the `Game` category.
//! A source that is not installed gives no games and no problem; a file that
//! is there but cannot be read is reported in [`Library::problems`].

mod desktop;
mod heroic;
mod launch;
mod paths;
mod steam;

use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;

pub use launch::launch;

/// Where a game was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Gog,
    Amazon,
    /// Added to Heroic by hand ("sideloaded").
    Heroic,
    /// A `.desktop` file in the `Game` category.
    Desktop,
}

impl Source {
    /// The name shown to people.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::Epic => "Epic",
            Self::Gog => "GOG",
            Self::Amazon => "Amazon",
            Self::Heroic => "Heroic",
            Self::Desktop => "Desktop",
        }
    }
}

/// How a game is started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Launch {
    /// A URL handed to the program that owns its scheme (`steam://`,
    /// `heroic://`) through `xdg-open`.
    Uri { uri: String },
    /// A program run directly, as a `.desktop` file's `Exec` says.
    Command {
        program: String,
        args: Vec<String>,
        dir: Option<PathBuf>,
    },
}

/// One game in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Game {
    /// Unique across sources: `steam:570`, `epic:Fortnite`, `desktop:foo`.
    pub id: String,
    pub title: String,
    pub source: Source,
    pub launch: Launch,
    /// A portrait cover (2:3) already on disk, when the source keeps one.
    pub cover: Option<PathBuf>,
}

/// What a scanner found.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    pub games: Vec<Game>,
    pub problems: Vec<String>,
}

impl Scan {
    fn extend(&mut self, other: Scan) {
        self.games.extend(other.games);
        self.problems.extend(other.problems);
    }
}

/// Every game found, sorted by title.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Library {
    pub games: Vec<Game>,
    /// Files that exist but could not be read, one line each.
    pub problems: Vec<String>,
}

impl Library {
    /// Reads every source this user has.
    #[must_use]
    pub fn scan() -> Self {
        let mut scan = steam::scan(&steam::roots());
        scan.extend(heroic::scan(&heroic::roots()));
        scan.extend(desktop::scan(&desktop::dirs()));
        Self::from_scan(scan)
    }

    fn from_scan(scan: Scan) -> Self {
        let mut seen = HashSet::new();
        let mut games: Vec<Game> = scan
            .games
            .into_iter()
            .filter(|game| seen.insert(game.id.clone()))
            .collect();
        games.sort_by_cached_key(|game| (game.title.to_lowercase(), game.id.clone()));
        Self {
            games,
            problems: scan.problems,
        }
    }

    /// The game with this id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|game| game.id == id)
    }
}
