//! Zendo's library: the games found on this machine and how to start them.
//!
//! Every source is read without writing to it: Steam's library folders,
//! Heroic's installed games and `.desktop` files with the `Game` category.
//! A source that is not installed gives no games and no problem; a file that
//! is there but cannot be read is reported in [`Library::problems`].
//!
//! Zendo's own files are [`config`] (what the user sets), [`state`] (what
//! Zendo remembers: launches, hidden games, the sort) and [`covers`] (the
//! images it downloaded).

pub mod config;
pub mod covers;
mod desktop;
mod heroic;
mod launch;
mod paths;
pub mod state;
mod steam;
mod time;

use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;

pub use launch::launch;
pub use state::{Sort, State};

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

/// When and how much a game was played, from its launcher's records and
/// Zendo's own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Played {
    /// Unix seconds.
    pub last: Option<u64>,
    pub minutes: u64,
    /// Times Zendo started it.
    pub launches: u32,
}

impl Played {
    /// Both records together: the later date, the larger playtime. The
    /// launchers and Zendo time different sessions, but a launcher's total
    /// usually holds Zendo's too, so they are not added up.
    fn merge(self, other: Self) -> Self {
        Self {
            last: self.last.max(other.last),
            minutes: self.minutes.max(other.minutes),
            launches: self.launches.max(other.launches),
        }
    }
}

/// One game in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Game {
    /// Unique across sources: `steam:570`, `epic:Fortnite`, `desktop:foo`.
    pub id: String,
    pub title: String,
    pub source: Source,
    pub launch: Launch,
    /// A portrait cover on disk: the source's own, or one Zendo downloaded.
    pub cover: Option<PathBuf>,
    /// A portrait image the store publishes, downloaded into Zendo's cache
    /// when there is no cover on disk.
    pub cover_url: Option<String>,
    pub played: Played,
    /// Hidden from the window by the user.
    pub hidden: bool,
}

impl Game {
    /// A game as a scanner finds it, before Zendo's state is applied.
    #[must_use]
    pub fn new(id: String, title: String, source: Source, launch: Launch) -> Self {
        Self {
            id,
            title,
            source,
            launch,
            cover: None,
            cover_url: None,
            played: Played::default(),
            hidden: false,
        }
    }
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
    /// Reads every source this user has, with Zendo's state and covers.
    #[must_use]
    pub fn scan() -> Self {
        let mut scan = steam::scan(&steam::roots());
        scan.extend(heroic::scan(&heroic::roots()));
        scan.extend(desktop::scan(&desktop::dirs()));
        let state = match State::load() {
            Ok(state) => state,
            Err(err) => {
                scan.problems.push(format!("{err:#}"));
                State::default()
            }
        };
        let mut library = Self::from_scan(scan);
        library.apply(&state);
        for game in library.games.iter_mut().filter(|game| game.cover.is_none()) {
            game.cover = covers::cached(&game.id);
        }
        library
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

    /// Zendo's records of each game: its launches and whether it is hidden.
    pub fn apply(&mut self, state: &State) {
        for game in &mut self.games {
            game.played = game.played.merge(state.played(&game.id));
            game.hidden = state.is_hidden(&game.id);
        }
    }

    /// The game with this id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|game| game.id == id)
    }
}
