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
        self.compare_with_titles(a, b, &a.title.to_lowercase(), &b.title.to_lowercase())
    }

    /// [`Sort::compare`], with the titles already lowered: a sort takes
    /// them once per game instead of twice per comparison.
    #[must_use]
    pub fn compare_with_titles(self, a: &Game, b: &Game, a_title: &str, b_title: &str) -> Ordering {
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
        order.then_with(|| a_title.cmp(b_title))
    }
}

/// How many days back a game counts as recently played.
pub const RECENT_DAYS: u64 = 30;

/// Which of the games the library's view chips show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum View {
    /// Every game not hidden.
    #[default]
    All,
    Favorites,
    /// Played in the last [`RECENT_DAYS`].
    Recent,
    /// Only the games hidden by the user.
    Hidden,
}

impl View {
    pub const ALL: [Self; 4] = [Self::All, Self::Favorites, Self::Recent, Self::Hidden];

    /// Whether the view shows `game`, `now` being Unix seconds.
    #[must_use]
    pub fn shows(self, game: &Game, now: u64) -> bool {
        match self {
            Self::Hidden => game.hidden,
            Self::All => !game.hidden,
            Self::Favorites => !game.hidden && game.pinned,
            Self::Recent => {
                !game.hidden
                    && game
                        .played
                        .last
                        .is_some_and(|last| now.saturating_sub(last) <= RECENT_DAYS * 86_400)
            }
        }
    }
}

/// Where the user is with a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Backlog,
    Playing,
    Completed,
    Abandoned,
}

impl Status {
    pub const ALL: [Self; 4] = [
        Self::Backlog,
        Self::Playing,
        Self::Completed,
        Self::Abandoned,
    ];

    /// The word the `zendo status` command takes.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Playing => "playing",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    /// The status a command's word names, whatever its case.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim().to_lowercase();
        Self::ALL.into_iter().find(|status| status.name() == word)
    }
}

/// What the library shows: a saved filter is one of these under a name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    pub source: Option<crate::Source>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<Status>,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub sort: Sort,
}

/// A saved filter, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedFilter {
    pub name: String,
    pub filter: Filter,
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

/// One of the user's categories, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub name: String,
    /// The games in it, by id.
    pub games: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    games: BTreeMap<String, Record>,
    #[serde(default)]
    hidden: BTreeSet<String>,
    #[serde(default)]
    pinned: BTreeSet<String>,
    /// The user's categories, each by the name it was given to the ids of the
    /// games in it. A game may be in several.
    #[serde(default)]
    categories: BTreeMap<String, BTreeSet<String>>,
    /// Each game's achievements as last seen, unlocked and total, for the
    /// library's progress line before its page is opened.
    #[serde(default)]
    achievements: BTreeMap<String, (u32, u32)>,
    /// Each game's play status; a game with none is not in the map.
    #[serde(default)]
    status: BTreeMap<String, Status>,
    /// The user's saved filters, by the name they were given.
    #[serde(default)]
    filters: BTreeMap<String, Filter>,
    /// The Steam app id the user gave a game that Zendo could not match, or
    /// matched wrongly. Never for a Steam game, which has its own.
    #[serde(default)]
    steam_apps: BTreeMap<String, u32>,
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
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(mut state) => {
                    state.separate_names();
                    Ok(state)
                }
                Err(err) => {
                    // A corrupt file must not lock the window out of every
                    // change: it is kept as `.bad` and the next start is
                    // empty.
                    let bad = path.with_extension("json.bad");
                    let why = match fs::rename(&path, &bad) {
                        Ok(()) => format!(
                            "{} was not valid state and is now {}; starting empty",
                            path.display(),
                            bad.display()
                        ),
                        Err(move_err) => format!(
                            "{}: not valid state (and could not be moved aside: {move_err})",
                            path.display()
                        ),
                    };
                    Err(err).with_context(|| why)
                }
            },
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

    /// Keeps how many achievements a game has and how many are unlocked.
    pub fn set_achievements(&mut self, id: &str, unlocked: u32, total: u32) {
        self.achievements.insert(id.to_owned(), (unlocked, total));
    }

    /// Unlocked and total, when they were seen.
    #[must_use]
    pub fn achievements_of(&self, id: &str) -> Option<(u32, u32)> {
        self.achievements.get(id).copied()
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

    /// Every category, by name, each with the games in it.
    #[must_use]
    pub fn categories(&self) -> Vec<Category> {
        let mut categories: Vec<Category> = self
            .categories
            .iter()
            .map(|(name, games)| Category {
                name: name.clone(),
                games: games.clone(),
            })
            .collect();
        categories
            .sort_by_cached_key(|category| (category.name.to_lowercase(), category.name.clone()));
        categories
    }

    /// The names of the categories holding a game, sorted.
    #[must_use]
    pub fn categories_of(&self, id: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .categories
            .iter()
            .filter(|(_, games)| games.contains(id))
            .map(|(name, _)| name.clone())
            .collect();
        names.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
        names
    }

    /// Categories and saved filters are both collections to the user, so
    /// they share one set of names. A saved filter named like a category (a
    /// file from before they were one list) becomes "Name (filter)", or
    /// "Name (filter 2)" and on, when that is taken too.
    fn separate_names(&mut self) {
        let clashing: Vec<String> = self
            .filters
            .keys()
            .filter(|name| key_in(&self.categories, name).is_some())
            .cloned()
            .collect();
        for name in clashing {
            let Some(filter) = self.filters.remove(&name) else {
                continue;
            };
            let mut candidate = format!("{name} (filter)");
            let mut n = 2;
            while key_in(&self.categories, &candidate).is_some()
                || key_in(&self.filters, &candidate).is_some()
            {
                candidate = format!("{name} (filter {n})");
                n += 1;
            }
            self.filters.insert(candidate, filter);
        }
    }

    /// Makes a category, empty. False when the name is blank or already
    /// taken, by a category or a saved filter, whatever its case. The name
    /// is kept as the user typed it.
    pub fn add_category(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || self.category_key(name).is_some()
            || key_in(&self.filters, name).is_some()
        {
            return false;
        }
        self.categories.insert(name.to_owned(), BTreeSet::new());
        true
    }

    /// Gives a category another name, keeping its games. False when it is
    /// unknown, the new name is blank, or it names another category; a
    /// change of case only is allowed.
    pub fn rename_category(&mut self, from: &str, to: &str) -> bool {
        let to = to.trim();
        let Some(from) = self.category_key(from) else {
            return false;
        };
        if to.is_empty() {
            return false;
        }
        if let Some(taken) = self.category_key(to)
            && taken != from
        {
            return false;
        }
        if key_in(&self.filters, to).is_some() {
            return false;
        }
        let Some(games) = self.categories.remove(&from) else {
            return false;
        };
        self.categories.insert(to.to_owned(), games);
        // A saved filter names its category: it follows the rename.
        let lowered = from.to_lowercase();
        for filter in self.filters.values_mut() {
            if filter
                .category
                .as_ref()
                .is_some_and(|name| name.to_lowercase() == lowered)
            {
                filter.category = Some(to.to_owned());
            }
        }
        true
    }

    /// Drops a category and which games were in it. A saved filter that
    /// named it stays, without it.
    pub fn delete_category(&mut self, name: &str) -> bool {
        let Some(key) = self.category_key(name) else {
            return false;
        };
        let lowered = key.to_lowercase();
        for filter in self.filters.values_mut() {
            if filter
                .category
                .as_ref()
                .is_some_and(|name| name.to_lowercase() == lowered)
            {
                filter.category = None;
            }
        }
        self.categories.remove(&key).is_some()
    }

    /// Sets a game's play status, or clears it with `None`.
    pub fn set_status(&mut self, id: &str, status: Option<Status>) {
        match status {
            Some(status) => {
                self.status.insert(id.to_owned(), status);
            }
            None => {
                self.status.remove(id);
            }
        }
    }

    #[must_use]
    pub fn status_of(&self, id: &str) -> Option<Status> {
        self.status.get(id).copied()
    }

    /// Sets the Steam app id for a game, or clears it with `None`. Returns
    /// false, changing nothing, for a Steam game (it has its own id) and
    /// for app id 0.
    pub fn set_steam_app(&mut self, id: &str, app: Option<u32>) -> bool {
        if id.starts_with("steam:") || app == Some(0) {
            return false;
        }
        match app {
            Some(app) => {
                self.steam_apps.insert(id.to_owned(), app);
            }
            None => {
                self.steam_apps.remove(id);
            }
        }
        true
    }

    /// The Steam app id the user gave the game.
    #[must_use]
    pub fn steam_app_of(&self, id: &str) -> Option<u32> {
        self.steam_apps.get(id).copied()
    }

    /// Every saved filter, by name.
    #[must_use]
    pub fn filters(&self) -> Vec<SavedFilter> {
        let mut filters: Vec<SavedFilter> = self
            .filters
            .iter()
            .map(|(name, filter)| SavedFilter {
                name: name.clone(),
                filter: filter.clone(),
            })
            .collect();
        filters.sort_by_cached_key(|saved| (saved.name.to_lowercase(), saved.name.clone()));
        filters
    }

    /// Saves a filter under a name; a name already taken by a filter,
    /// whatever its case, is overwritten and keeps its spelling. False when
    /// the name is blank or a category has it.
    pub fn save_filter(&mut self, name: &str, filter: Filter) -> bool {
        let name = name.trim();
        if name.is_empty() || self.category_key(name).is_some() {
            return false;
        }
        let key = key_in(&self.filters, name).unwrap_or_else(|| name.to_owned());
        self.filters.insert(key, filter);
        true
    }

    /// Gives a saved filter another name. False when it is unknown, the new
    /// name is blank, or it names another filter; a change of case only is
    /// allowed.
    pub fn rename_filter(&mut self, from: &str, to: &str) -> bool {
        let to = to.trim();
        let Some(from) = key_in(&self.filters, from) else {
            return false;
        };
        if to.is_empty() {
            return false;
        }
        if let Some(taken) = key_in(&self.filters, to)
            && taken != from
        {
            return false;
        }
        if self.category_key(to).is_some() {
            return false;
        }
        let Some(filter) = self.filters.remove(&from) else {
            return false;
        };
        self.filters.insert(to.to_owned(), filter);
        true
    }

    pub fn delete_filter(&mut self, name: &str) -> bool {
        key_in(&self.filters, name)
            .and_then(|key| self.filters.remove(&key))
            .is_some()
    }

    /// Puts a game in a category, or takes it out. False when the category
    /// does not exist: a game is never made to fit a name by accident.
    pub fn set_category(&mut self, name: &str, id: &str, member: bool) -> bool {
        let Some(name) = self.category_key(name) else {
            return false;
        };
        let Some(games) = self.categories.get_mut(&name) else {
            return false;
        };
        if member {
            games.insert(id.to_owned());
        } else {
            games.remove(id);
        }
        true
    }

    /// A category's name as it is stored, found whatever the case or the
    /// spaces around it.
    fn category_key(&self, name: &str) -> Option<String> {
        // The same lowering as `categories()` and `categories_of()`, so
        // the name two categories are told apart by is the one they are
        // ordered by.
        key_in(&self.categories, name)
    }
}

/// A key of `map` as it is stored, found whatever the case or the spaces
/// around the name.
fn key_in<V>(map: &BTreeMap<String, V>, name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    map.keys().find(|key| key.to_lowercase() == name).cloned()
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
    fn keeps_a_steam_app_id_the_user_gave() {
        let mut state = State::default();
        assert!(state.set_steam_app("local:portal", Some(620)));
        // A Steam game has its own id; 0 is no app.
        assert!(!state.set_steam_app("steam:70", Some(620)));
        assert!(!state.set_steam_app("local:portal", Some(0)));
        let mut state: State =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(state.steam_app_of("local:portal"), Some(620));
        assert_eq!(state.steam_app_of("steam:70"), None);
        assert!(state.set_steam_app("local:portal", None));
        assert_eq!(state.steam_app_of("local:portal"), None);
        // A file from before the field has none.
        let old: State = serde_json::from_str("{}").unwrap();
        assert_eq!(old.steam_app_of("local:portal"), None);
    }

    #[test]
    fn a_state_from_when_there_was_an_up_next_queue_still_loads() {
        let state: State =
            serde_json::from_str(r#"{"hidden":["a"],"up_next":["gog:1","steam:2"]}"#).unwrap();
        assert!(state.is_hidden("a"));
        let saved = serde_json::to_string(&state).unwrap();
        assert!(!saved.contains("up_next"), "{saved}");
    }

    #[test]
    fn keeps_achievements_and_loads_a_state_without_them() {
        let mut state = State::default();
        assert_eq!(state.achievements_of("gog:1"), None);
        state.set_achievements("gog:1", 4, 15);
        let state: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(state.achievements_of("gog:1"), Some((4, 15)));
        let old: State = serde_json::from_str(r#"{"hidden":["a"]}"#).unwrap();
        assert_eq!(old.achievements_of("a"), None);
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
        assert_eq!(state.categories(), [] as [Category; 0]);
    }

    #[test]
    fn keeps_categories_through_a_round_trip() {
        let mut state = State::default();
        assert!(state.add_category("Co-op"));
        assert!(state.add_category("Backlog"));
        assert!(state.set_category("Co-op", "steam:620", true));
        assert!(state.set_category("Backlog", "steam:620", true));

        let state: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(state.categories_of("steam:620"), ["Backlog", "Co-op"]);
        let categories = state.categories();
        assert_eq!(
            categories
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Backlog", "Co-op"]
        );
        assert_eq!(
            categories[1].games.iter().collect::<Vec<_>>(),
            ["steam:620"]
        );
        assert_eq!(state.categories_of("steam:1"), [] as [String; 0]);
    }

    #[test]
    fn adds_no_empty_or_duplicate_category() {
        let mut state = State::default();
        assert!(!state.add_category(""));
        assert!(!state.add_category("   "));
        assert!(state.add_category("Co-op"));
        assert!(!state.add_category("co-op"));
        assert_eq!(state.categories().len(), 1);
        assert_eq!(state.categories()[0].name, "Co-op");
        // The case rule is the order's, Unicode lowering, not ASCII.
        assert!(state.add_category("Été"));
        assert!(!state.add_category("ÉTÉ"));
        assert_eq!(state.categories().len(), 2);
    }

    #[test]
    fn renames_keeps_games_and_refuses_a_clash() {
        let mut state = State::default();
        assert!(state.add_category("Co-op"));
        assert!(state.set_category("Co-op", "steam:620", true));

        assert!(state.rename_category("Co-op", "Co-op nights"));
        assert_eq!(state.categories_of("steam:620"), ["Co-op nights"]);
        assert!(
            state
                .categories_of("steam:620")
                .contains(&"Co-op nights".to_owned())
        );

        assert!(state.add_category("Backlog"));
        assert!(!state.rename_category("Co-op nights", "Backlog"));
        assert_eq!(state.categories().len(), 2, "the clash changed nothing");
        assert!(
            state
                .categories_of("steam:620")
                .contains(&"Co-op nights".to_owned())
        );

        assert!(!state.rename_category("unknown", "Elsewhere"));
        assert!(!state.rename_category("Backlog", "  "));

        // A rename that only changes the case is not a clash with itself.
        assert!(state.rename_category("Backlog", "backlog"));
        assert_eq!(state.categories()[0].name, "backlog");
    }

    #[test]
    fn deletes_a_category_with_its_games() {
        let mut state = State::default();
        assert!(state.add_category("Co-op"));
        assert!(state.set_category("Co-op", "steam:620", true));
        assert!(state.delete_category("Co-op"));
        assert_eq!(state.categories(), [] as [Category; 0]);
        assert_eq!(state.categories_of("steam:620"), [] as [String; 0]);
        assert!(!state.delete_category("Co-op"));
        assert!(!state.delete_category("never there"));
    }

    #[test]
    fn sets_a_category_only_where_it_exists() {
        let mut state = State::default();
        assert!(!state.set_category("Co-op", "steam:620", true));
        assert_eq!(state.categories(), [] as [Category; 0], "nothing was made");
        assert!(state.add_category("Co-op"));
        assert!(state.set_category("Co-op", "steam:620", true));
        assert_eq!(state.categories_of("steam:620"), ["Co-op"]);
        assert!(state.set_category("Co-op", "steam:620", false));
        assert_eq!(state.categories_of("steam:620"), [] as [String; 0]);
        // The names are found whatever their case or spaces around them.
        assert!(state.set_category("co-op", "steam:730", true));
        assert_eq!(state.categories_of("steam:730"), ["Co-op"]);
        assert!(state.set_category(" Co-op ", "steam:740", true));
        assert_eq!(state.categories_of("steam:740"), ["Co-op"]);
    }

    #[test]
    fn keeps_a_status_and_clears_it() {
        let mut state = State::default();
        assert_eq!(state.status_of("steam:1"), None);
        state.set_status("steam:1", Some(Status::Backlog));
        state.set_status("steam:1", Some(Status::Playing));
        assert_eq!(state.status_of("steam:1"), Some(Status::Playing));
        let state: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(state.status_of("steam:1"), Some(Status::Playing));
        let mut state = state;
        state.set_status("steam:1", None);
        assert_eq!(state.status_of("steam:1"), None);
        assert_eq!(Status::parse(" Completed "), Some(Status::Completed));
        assert_eq!(Status::parse("none"), None);
    }

    #[test]
    fn an_older_file_has_no_status_and_no_filters() {
        let state: State = serde_json::from_str(r#"{"hidden":["a"]}"#).unwrap();
        assert_eq!(state.status_of("a"), None);
        assert_eq!(state.filters(), [] as [SavedFilter; 0]);
    }

    fn a_filter() -> Filter {
        Filter {
            view: View::Favorites,
            source: Some(crate::Source::Gog),
            category: Some("Co-op".into()),
            status: Some(Status::Backlog),
            search: "doom".into(),
            sort: Sort::Title,
        }
    }

    #[test]
    fn keeps_saved_filters_through_a_round_trip() {
        let mut state = State::default();
        assert!(state.save_filter(" Evening ", a_filter()));
        assert!(!state.save_filter("  ", a_filter()));
        let state: State = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        let filters = state.filters();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].name, "Evening");
        assert_eq!(filters[0].filter, a_filter());
        // A filter written with only some fields reads with defaults.
        let state: State = serde_json::from_str(r#"{"filters":{"x":{"search":"a"}}}"#).unwrap();
        assert_eq!(state.filters()[0].filter.view, View::All);
    }

    #[test]
    fn saving_over_a_name_in_another_case_replaces_it() {
        let mut state = State::default();
        assert!(state.save_filter("Evening", Filter::default()));
        assert!(state.save_filter("evening", a_filter()));
        let filters = state.filters();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].name, "Evening", "it keeps its spelling");
        assert_eq!(filters[0].filter, a_filter());
    }

    #[test]
    fn renames_and_deletes_a_saved_filter() {
        let mut state = State::default();
        assert!(state.save_filter("One", Filter::default()));
        assert!(state.save_filter("Two", a_filter()));
        assert!(!state.rename_filter("One", "two"), "a clash");
        assert!(!state.rename_filter("One", " "));
        assert!(!state.rename_filter("missing", "Three"));
        assert!(state.rename_filter("One", "one"), "a change of case");
        assert!(state.rename_filter("Two", "Three"));
        assert_eq!(state.filters()[1].filter, a_filter());
        assert!(state.delete_filter("THREE"));
        assert!(!state.delete_filter("Three"));
        assert_eq!(state.filters().len(), 1);
    }

    #[test]
    fn a_category_rename_and_delete_reach_the_saved_filters() {
        let mut state = State::default();
        assert!(state.add_category("Co-op"));
        assert!(state.save_filter("Nights", a_filter()));
        assert!(state.rename_category("Co-op", "Couch"));
        assert_eq!(state.filters()[0].filter.category.as_deref(), Some("Couch"));
        assert!(state.delete_category("couch"));
        let saved = &state.filters()[0];
        assert_eq!(saved.filter.category, None);
        assert_eq!(saved.filter.search, "doom", "the rest is kept");
    }

    #[test]
    fn a_category_and_a_saved_filter_cannot_share_a_name() {
        let mut state = State::default();
        assert!(state.add_category("Co-op"));
        assert!(!state.save_filter("co-op", a_filter()), "a category has it");
        assert!(state.save_filter("Nights", a_filter()));
        assert!(!state.add_category("NIGHTS"), "a filter has it");
        assert!(state.add_category("Backlog"));
        assert!(!state.rename_category("Backlog", "nights"));
        assert!(state.save_filter("Evening", a_filter()));
        assert!(!state.rename_filter("Evening", "backlog"));
        assert!(state.rename_filter("Evening", "Late"));
        assert_eq!(state.categories().len(), 2);
        assert_eq!(state.filters().len(), 2);
    }

    #[test]
    fn an_older_file_with_a_shared_name_keeps_both() {
        let mut state: State = serde_json::from_str(
            r#"{"categories":{"Co-op":["a"],"Co-op (filter)":[]},
                "filters":{"co-op":{"search":"x"},"Co-op (filter)":{"search":"y"},"Plain":{}}}"#,
        )
        .unwrap();
        state.separate_names();
        let names: Vec<String> = state
            .filters()
            .into_iter()
            .map(|saved| saved.name)
            .collect();
        assert_eq!(
            names,
            ["co-op (filter 2)", "Co-op (filter) (filter)", "Plain"],
            "both clashing filters were renamed, none lost"
        );
        assert_eq!(state.categories().len(), 2);
        assert!(!state.filters().iter().any(|saved| {
            state
                .categories()
                .iter()
                .any(|category| category.name.to_lowercase() == saved.name.to_lowercase())
        }));
    }

    #[test]
    fn categories_sorts_by_lowercase_name() {
        let mut state = State::default();
        assert!(state.add_category("beta"));
        assert!(state.add_category("Alpha"));
        assert!(state.add_category("alpha 2"));
        assert_eq!(
            state
                .categories()
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "alpha 2", "beta"]
        );
    }
}
