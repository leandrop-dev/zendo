//! What Zendo remembers between runs, in `$XDG_STATE_HOME/zendo/state.json`:
//! the games it started, the ones hidden from the window, and the sort.
//! The window and the `zendo` command both write it, each change read,
//! made and written back at once.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{Game, Played, paths, time};

/// The order of the grid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    /// Last played first; games never played after them, by title.
    #[default]
    Recent,
    /// Longest played first, then most started.
    MostPlayed,
    Title,
}

impl Sort {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Recent => Self::MostPlayed,
            Self::MostPlayed => Self::Title,
            Self::Title => Self::Recent,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "Recent",
            Self::MostPlayed => "Most played",
            Self::Title => "A–Z",
        }
    }

    /// The order of two games: pinned ones first, then this order, then by
    /// title.
    #[must_use]
    pub fn compare(self, a: &Game, b: &Game) -> Ordering {
        // `None` sorts before `Some`, so reversed it puts the never played
        // last.
        let recent = || b.played.last.cmp(&a.played.last);
        let order = b.pinned.cmp(&a.pinned).then_with(|| match self {
            Self::Recent => recent(),
            Self::MostPlayed => b
                .played
                .minutes
                .cmp(&a.played.minutes)
                .then(b.played.launches.cmp(&a.played.launches))
                .then_with(recent),
            Self::Title => Ordering::Equal,
        });
        order.then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    #[serde(default)]
    launches: u32,
    #[serde(default)]
    last_launched: Option<u64>,
    /// Timed only for games Zendo runs itself (`.desktop` ones), while its
    /// window stays open; the launchers time their own.
    #[serde(default)]
    minutes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    games: BTreeMap<String, Record>,
    #[serde(default)]
    hidden: BTreeSet<String>,
    #[serde(default)]
    pinned: BTreeSet<String>,
    #[serde(default)]
    pub sort: Sort,
}

impl State {
    /// Where the state is kept.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::state_home().map(|dir| dir.join("zendo/state.json"))
    }

    /// The saved state, or an empty one when there is none yet.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("{}: not valid state", path.display())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// Reads the state, changes it and writes it back, holding the file's
    /// lock, so the window's threads and the `zendo` command lose nothing
    /// of each other's.
    ///
    /// # Errors
    ///
    /// When the file cannot be locked, read, parsed or written.
    pub fn update(change: impl FnOnce(&mut Self)) -> anyhow::Result<Self> {
        let path = Self::path().context("no home folder to keep Zendo's state in")?;
        paths::locked(&path, || {
            let mut state = Self::load()?;
            change(&mut state);
            let json = serde_json::to_vec_pretty(&state)?;
            paths::write_atomic(&path, &json)
                .with_context(|| format!("could not write {}", path.display()))?;
            Ok(state)
        })
    }

    pub fn launched(&mut self, id: &str) {
        let record = self.games.entry(id.to_owned()).or_default();
        record.launches = record.launches.saturating_add(1);
        record.last_launched = Some(time::now());
    }

    pub fn add_minutes(&mut self, id: &str, minutes: u64) {
        let record = self.games.entry(id.to_owned()).or_default();
        record.minutes = record.minutes.saturating_add(minutes);
    }

    pub fn set_hidden(&mut self, id: &str, hidden: bool) {
        if hidden {
            self.hidden.insert(id.to_owned());
        } else {
            self.hidden.remove(id);
        }
    }

    #[must_use]
    pub fn is_hidden(&self, id: &str) -> bool {
        self.hidden.contains(id)
    }

    pub fn set_pinned(&mut self, id: &str, pinned: bool) {
        if pinned {
            self.pinned.insert(id.to_owned());
        } else {
            self.pinned.remove(id);
        }
    }

    #[must_use]
    pub fn is_pinned(&self, id: &str) -> bool {
        self.pinned.contains(id)
    }

    #[must_use]
    pub fn played(&self, id: &str) -> Played {
        self.games
            .get(id)
            .map_or_else(Played::default, |record| Played {
                last: record.last_launched,
                minutes: record.minutes,
                launches: record.launches,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_launches_hidden_games_and_the_sort() {
        let mut state = State::default();
        state.launched("steam:620");
        state.launched("steam:620");
        state.add_minutes("steam:620", 30);
        state.set_hidden("desktop:tool", true);
        state.sort = Sort::MostPlayed;

        let state: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        let played = state.played("steam:620");
        assert_eq!((played.launches, played.minutes), (2, 30));
        assert!(played.last.is_some());
        assert!(state.is_hidden("desktop:tool"));
        assert_eq!(state.sort, Sort::MostPlayed);
        assert_eq!(state.played("steam:1"), Played::default());
    }

    #[test]
    fn sorts_by_recent_most_played_and_title() {
        let game = |title: &str, last: Option<u64>, minutes: u64| {
            let mut game = Game::new(
                title.to_owned(),
                title.to_owned(),
                crate::Source::Desktop,
                crate::Launch::Uri { uri: String::new() },
            );
            game.played = Played {
                last,
                minutes,
                launches: 0,
            };
            game
        };
        let games = [
            game("b never", None, 0),
            game("a never", None, 0),
            game("old long", Some(10), 500),
            game("new short", Some(20), 5),
        ];
        let order = |sort: Sort| {
            let mut sorted: Vec<&Game> = games.iter().collect();
            sorted.sort_by(|a, b| sort.compare(a, b));
            sorted
                .iter()
                .map(|game| game.title.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            order(Sort::Recent),
            ["new short", "old long", "a never", "b never"]
        );
        assert_eq!(
            order(Sort::MostPlayed),
            ["old long", "new short", "a never", "b never"]
        );
        assert_eq!(
            order(Sort::Title),
            ["a never", "b never", "new short", "old long"]
        );

        let mut pinned = game("b never", None, 0);
        pinned.pinned = true;
        let games = [games[2].clone(), pinned];
        let mut sorted: Vec<&Game> = games.iter().collect();
        sorted.sort_by(|a, b| Sort::MostPlayed.compare(a, b));
        assert_eq!(sorted[0].title, "b never", "a pinned game leads");
    }

    #[test]
    fn reads_a_file_from_an_older_zendo() {
        let state: State = serde_json::from_str(r#"{"hidden": ["a"]}"#).unwrap();
        assert!(state.is_hidden("a"));
        assert_eq!(state.sort, Sort::Recent);
    }
}
