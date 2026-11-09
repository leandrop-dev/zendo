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

pub mod appinfo;
pub mod cloud_saves;
pub mod config;
pub mod covers;
mod desktop;
pub mod downloads;
pub mod eos_overlay;
pub mod fomod;
pub mod galaxy;
pub mod ge_proton;
pub mod gog_playtime;
pub mod gog_setup;
mod heroic;
mod http;
pub mod info;
pub mod known_fixes;
pub mod launch;
pub mod launch_options;
pub mod local;
pub mod mods;
mod moving;
pub mod nexus;
pub mod nexus_search;
pub mod owned;
mod paths;
pub mod plugins;
pub mod prefix_tools;
pub mod proton;
pub mod protondb;
pub mod report;
pub mod runtimes;
pub mod secrets;
pub mod state;
pub mod steam;
pub mod steam_runtime;
pub mod steam_web;
pub mod steamgriddb;
pub mod store_tasks;
pub mod stores;
pub mod theme;
mod time;
pub mod tone;
mod umu;
mod windows;

use std::collections::{BTreeMap, HashSet};
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
    /// Added by hand in Zendo.
    Local,
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
            Self::Local => "Added",
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
    /// Installed by Zendo through a store engine (`legendary`, `gogdl`,
    /// `nile`), and started through its `launch`.
    Engine { runner: String, app: String },
    /// Added by hand: a program, through umu and Proton when it is a
    /// Windows one.
    Local {
        program: PathBuf,
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

/// How much room a game takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Size {
    Bytes(u64),
    /// The store's own wording, when it keeps no number (Heroic's GOG
    /// records say "75.85 GiB").
    Label(String),
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
        match self {
            Self::Label(label) => f.write_str(label),
            Self::Bytes(bytes) => {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "shown to two decimals; the precision lost is far below that"
                )]
                let mut value = *bytes as f64;
                let mut unit = 0;
                while value >= 1024.0 && unit < UNITS.len() - 1 {
                    value /= 1024.0;
                    unit += 1;
                }
                if unit == 0 {
                    write!(f, "{bytes} B")
                } else {
                    write!(f, "{value:.2} {}", UNITS[unit])
                }
            }
        }
    }
}

/// What the game's source records about the install, each when known.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Details {
    pub developer: Option<String>,
    /// As the store writes it: `Windows`, `linux`, …
    pub platform: Option<String>,
    pub version: Option<String>,
    pub install_path: Option<PathBuf>,
    pub size: Option<Size>,
    /// The DLC installed with it, as its engine records them.
    pub dlc: Vec<InstalledDlc>,
}

/// A DLC installed with a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledDlc {
    /// The store's id for it: legendary's app name, GOG's product id.
    pub id: String,
    /// legendary records it; Heroic's and Zendo's GOG records do not.
    pub title: Option<String>,
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
    /// A wide image for the game's page, the same way.
    pub hero: Option<PathBuf>,
    pub hero_url: Option<String>,
    pub details: Details,
    pub played: Played,
    /// Hidden from the window by the user.
    pub hidden: bool,
    /// Pinned by the user: pinned games lead the grid in every order.
    pub pinned: bool,
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
            hero: None,
            hero_url: None,
            details: Details::default(),
            played: Played::default(),
            hidden: false,
            pinned: false,
        }
    }

    /// The program that starts it, as people would say it.
    #[must_use]
    pub fn launcher(&self) -> String {
        match (&self.launch, self.source) {
            (Launch::Uri { .. }, Source::Steam) => "Steam".to_owned(),
            (Launch::Uri { uri }, _) => {
                let runner = uri.rsplit_once("runner=").map_or("", |(_, runner)| runner);
                format!("Heroic ({runner})")
            }
            (Launch::Command { program, .. }, _) => program.clone(),
            (Launch::Engine { runner, .. }, _) => format!("Zendo ({runner})"),
            (Launch::Local { .. }, _) if launch::needs_proton(self) => {
                "Zendo (umu-launcher)".to_owned()
            }
            (Launch::Local { .. }, _) => "Zendo".to_owned(),
        }
    }

    /// Installed by Zendo, so Zendo can update, repair and uninstall it.
    #[must_use]
    pub fn installed_by_zendo(&self) -> bool {
        matches!(self.launch, Launch::Engine { .. })
    }

    /// The launch as one line, for the details card and logs.
    #[must_use]
    pub fn command_line(&self) -> String {
        match &self.launch {
            Launch::Uri { uri } => format!("xdg-open {uri}"),
            Launch::Engine { runner, app } if launch::needs_proton(self) => {
                format!("{runner} launch {app} --no-wine --wrapper umu-run")
            }
            Launch::Engine { runner, app } => format!("{runner} launch {app}"),
            Launch::Command { program, args, .. } => words(program, args),
            Launch::Local { program, args, .. } => {
                let line = words(&program.to_string_lossy(), args);
                if launch::needs_proton(self) {
                    format!("umu-run {line}")
                } else {
                    line
                }
            }
        }
    }
}

/// A program and its arguments as one line, words with spaces quoted.
fn words(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(|word| {
            if word.contains(char::is_whitespace) {
                format!("\"{word}\"")
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
        // After Heroic's: a game both installed keeps Heroic's launch.
        scan.extend(downloads::scan());
        scan.extend(desktop::scan(&desktop::dirs()));
        scan.extend(local::scan());
        let state = match State::load() {
            Ok(state) => state,
            Err(err) => {
                scan.problems.push(format!("{err:#}"));
                State::default()
            }
        };
        let mut library = Self::from_scan(scan);
        library.apply(&state);
        library.apply_gog_totals(&gog_playtime::totals());
        let chosen = covers::Choices::load();
        for game in &mut library.games {
            // One chosen by hand comes before Steam's and the stores'.
            if let Some(cover) = chosen.get(&game.id, covers::Kind::Cover) {
                game.cover = Some(cover.to_path_buf());
            }
            if let Some(hero) = chosen.get(&game.id, covers::Kind::Hero) {
                game.hero = Some(hero.to_path_buf());
            }
            if game.cover.is_none() {
                game.cover = covers::cached(&game.id, covers::Kind::Cover);
            }
            if game.hero.is_none() {
                game.hero = covers::cached(&game.id, covers::Kind::Hero);
            }
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

    /// Zendo's records of each game: its launches, and whether it is hidden
    /// or pinned.
    pub fn apply(&mut self, state: &State) {
        for game in &mut self.games {
            game.played = game.played.merge(state.played(&game.id));
            game.hidden = state.is_hidden(&game.id);
            game.pinned = state.is_pinned(&game.id);
        }
    }

    /// GOG's totals, in minutes, where they are more than the launchers'
    /// count: they hold every client's sessions.
    pub fn apply_gog_totals(&mut self, totals: &BTreeMap<String, u64>) {
        for game in &mut self.games {
            if let Some(&minutes) = totals.get(&game.id) {
                game.played.minutes = game.played.minutes.max(minutes);
            }
        }
    }

    /// The game with this id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|game| game.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gogs_totals_count_where_they_are_more() {
        let game = |id: &str, minutes| {
            let mut game = Game::new(
                id.into(),
                id.into(),
                Source::Gog,
                Launch::Engine {
                    runner: "gogdl".into(),
                    app: id.into(),
                },
            );
            game.played.minutes = minutes;
            game
        };
        let mut library = Library {
            games: vec![game("gog:1", 30), game("gog:2", 500), game("gog:3", 7)],
            problems: Vec::new(),
        };
        let totals = BTreeMap::from([("gog:1".to_owned(), 120), ("gog:2".to_owned(), 90)]);
        library.apply_gog_totals(&totals);
        let minutes: Vec<u64> = library
            .games
            .iter()
            .map(|game| game.played.minutes)
            .collect();
        assert_eq!(minutes, [120, 500, 7]);
    }

    #[test]
    fn writes_sizes_for_people() {
        assert_eq!(Size::Bytes(512).to_string(), "512 B");
        assert_eq!(Size::Bytes(374_178_085).to_string(), "356.84 MiB");
        assert_eq!(Size::Label("75.85 GiB".into()).to_string(), "75.85 GiB");
    }

    #[test]
    fn says_what_starts_a_game() {
        let heroic = Game::new(
            "gog:1".into(),
            "A".into(),
            Source::Gog,
            Launch::Uri {
                uri: "heroic://launch?appName=1&runner=gog".into(),
            },
        );
        assert_eq!(heroic.launcher(), "Heroic (gog)");
        let desktop = Game::new(
            "desktop:x".into(),
            "X".into(),
            Source::Desktop,
            Launch::Command {
                program: "xonotic".into(),
                args: vec!["-basedir".into(), "/opt/x data".into()],
                dir: None,
            },
        );
        assert_eq!(desktop.command_line(), "xonotic -basedir \"/opt/x data\"");
    }
}
