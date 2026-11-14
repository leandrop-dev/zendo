//! The Library page's state, with no toolkit in it: what the filters let
//! through, the cursor, the marked games, the chips with their counts, what
//! a card says and what the page says when there is no grid. `library_ui`
//! carries it to the Slint window; the tests here need no window.
//!
//! Every change answers with an [`Outcome`]: what to write to `state.json`,
//! which rows to draw again and whether the grid should scroll.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use zendo_core::browse::{self, Geometry, Move};
use zendo_core::covers::Kind;
use zendo_core::{Category, Filter, Game, Library, SavedFilter, Sort, Source, Status, View};
use zendo_shell::{format, t};

/// A card's outer size and the gap between cards, as the Iced window had them.
pub const CARD_HEIGHT: f32 = 370.0;
pub const SPACING: f32 = 16.0;

/// The grid's rows, for the cursor's scrolling. A little room under the
/// cursor's card, so its border is not cut by the window's foot.
pub const GEOMETRY: Geometry = Geometry {
    card_height: CARD_HEIGHT,
    spacing: SPACING,
    padding: 4.0,
};

/// What a chip does when pressed.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    View(View),
    /// "All": every store, category and status.
    Clear,
    Source(Source),
    Sort(Sort),
    Status(Status),
    Category(String),
    Saved(String),
    BulkPin(bool),
    BulkHide(bool),
    ClearMarks,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub label: String,
    pub count: Option<usize>,
    pub chosen: bool,
    pub pick: Pick,
}

fn chip(label: String, count: Option<usize>, chosen: bool, pick: Pick) -> Chip {
    Chip {
        label,
        count,
        chosen,
        pick,
    }
}

/// The chip rows of the Library, and the top bar's view chips.
#[derive(Debug, Default)]
pub struct Chips {
    pub views: Vec<Chip>,
    pub sources: Vec<Chip>,
    pub sorts: Vec<Chip>,
    pub organize: Vec<Chip>,
    /// "3 marked", when any are.
    pub marked: Option<String>,
    pub bulk: Vec<Chip>,
}

/// What a card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct CardData {
    pub id: String,
    pub title: String,
    /// "STEAM · 12 h", the line under the title.
    pub caption: String,
    /// The caption is in the accent colour (a hidden game), not the dim one.
    pub caption_accent: bool,
    pub faded: bool,
    pub badge: Option<String>,
    pub progress: Option<f32>,
    pub marked: bool,
    pub cover: Option<PathBuf>,
}

/// What the page says in place of the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Empty {
    pub title: String,
    pub about: Option<String>,
}

/// What a change to the state asks of the window.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub save: Option<Save>,
    pub rows: Rows,
    pub scroll: Scroll,
}

/// A write to `state.json`.
#[derive(Debug, Clone, PartialEq)]
pub enum Save {
    Sort(Sort),
    Pinned(Vec<String>, bool),
    Hidden(Vec<String>, bool),
}

/// Which rows of the grid to draw again.
#[derive(Debug, Default, PartialEq)]
pub enum Rows {
    #[default]
    Same,
    /// The cards at these places in the shown list changed.
    Marks(Vec<usize>),
    All,
}

#[derive(Debug, Default, PartialEq)]
pub enum Scroll {
    #[default]
    Keep,
    Top,
    To(f32),
}

#[derive(Debug)]
pub struct Browser {
    /// `false` while the first scan runs.
    loaded: bool,
    games: Vec<Game>,
    problems: usize,
    categories: Vec<Category>,
    saved: Vec<SavedFilter>,
    filter: Filter,
    /// The games the filter lets through, as places in `games`.
    shown: Vec<usize>,
    cursor: usize,
    marked: BTreeSet<String>,
    columns: usize,
    scroll_top: f32,
    viewport: f32,
    status: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

impl Browser {
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded: false,
            games: Vec::new(),
            problems: 0,
            categories: Vec::new(),
            saved: Vec::new(),
            filter: Filter::default(),
            shown: Vec::new(),
            cursor: 0,
            marked: BTreeSet::new(),
            columns: 5,
            scroll_top: 0.0,
            viewport: 800.0,
            status: None,
        }
    }

    // What the window reads.

    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor.min(self.shown.len().saturating_sub(1))
    }

    #[must_use]
    pub fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub fn search(&self) -> &str {
        &self.filter.search
    }

    #[cfg(test)]
    #[must_use]
    pub fn current(&self) -> &Filter {
        &self.filter
    }

    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    #[must_use]
    pub fn problems(&self) -> usize {
        self.problems
    }

    #[must_use]
    pub fn games(&self) -> &[Game] {
        &self.games
    }

    #[cfg(test)]
    #[must_use]
    pub fn marked(&self) -> &BTreeSet<String> {
        &self.marked
    }

    /// The game under the cursor.
    #[must_use]
    pub fn selected(&self) -> Option<&Game> {
        self.shown
            .get(self.cursor())
            .map(|&index| &self.games[index])
    }

    fn selected_id(&self) -> Option<String> {
        self.selected().map(|game| game.id.clone())
    }

    /// The card at this place in the shown list.
    #[must_use]
    pub fn card(&self, place: usize) -> Option<CardData> {
        let game = self.games.get(*self.shown.get(place)?)?;
        let now = now();
        let played = format::played_line(game, now);
        let (line, caption_accent) = if game.hidden {
            (t!("library-hidden"), true)
        } else if let Some(status) = game.status {
            (
                format!("{} · {played}", format::play_status(status).to_uppercase()),
                false,
            )
        } else {
            (played, false)
        };
        Some(CardData {
            id: game.id.clone(),
            title: game.title.clone(),
            caption: format!("{} · {line}", format::source(game.source).to_uppercase()),
            caption_accent,
            faded: game.hidden,
            badge: game.pinned.then(|| t!("badge-favorite")),
            progress: game.achievements.and_then(format::progress_of),
            marked: self.marked.contains(&game.id),
            cover: game.cover.clone(),
        })
    }

    /// What the page says instead of the grid, when it has no cards.
    #[must_use]
    pub fn empty(&self) -> Option<Empty> {
        let (title, about) = if !self.loaded {
            (t!("library-looking"), None)
        } else if self.games.is_empty() {
            (
                t!("library-none-found"),
                Some(t!("library-none-found-about")),
            )
        } else if !self.shown.is_empty() {
            return None;
        } else if !self.filter.search.is_empty() {
            (
                t!("library-no-match", search = self.filter.search.as_str()),
                None,
            )
        } else if self.filter.source.is_some()
            || self.filter.category.is_some()
            || self.filter.status.is_some()
        {
            (t!("library-none-here"), None)
        } else {
            (
                match self.filter.view {
                    View::All => t!("library-all-hidden"),
                    View::Favorites => t!("library-no-favorites"),
                    View::Recent => t!("library-no-recent"),
                    View::Hidden => t!("library-no-hidden"),
                },
                None,
            )
        };
        Some(Empty { title, about })
    }

    /// The keys the footer lists.
    #[must_use]
    pub fn hints(&self) -> String {
        if self.marked.is_empty() {
            t!("hints-library")
        } else {
            t!("hints-selection")
        }
    }

    /// The chips, with the counts the games give them now.
    #[must_use]
    pub fn chips(&self) -> Chips {
        let at = now();
        let filter = &self.filter;

        let hidden = self.games.iter().filter(|game| game.hidden).count();
        let views = View::ALL
            .into_iter()
            // Hidden is offered only when something is hidden, or is open.
            .filter(|view| *view != View::Hidden || hidden > 0 || filter.view == View::Hidden)
            .map(|view| {
                let count = self
                    .games
                    .iter()
                    .filter(|game| view.shows(game, at))
                    .count();
                chip(
                    format::view(view),
                    Some(count),
                    filter.view == view,
                    Pick::View(view),
                )
            })
            .collect();

        let in_view: Vec<&Game> = self
            .games
            .iter()
            .filter(|game| filter.view.shows(game, at))
            .collect();
        let mut sources = vec![chip(
            t!("library-all"),
            Some(in_view.len()),
            filter.source.is_none() && filter.category.is_none() && filter.status.is_none(),
            Pick::Clear,
        )];
        for source in Source::ALL {
            let count = in_view.iter().filter(|game| game.source == source).count();
            if count > 0 {
                sources.push(chip(
                    format::source(source),
                    Some(count),
                    filter.source == Some(source),
                    Pick::Source(source),
                ));
            }
        }
        let sorts = [Sort::Recent, Sort::MostPlayed, Sort::Title]
            .into_iter()
            .map(|sort| {
                chip(
                    format::sort(sort),
                    None,
                    filter.sort == sort,
                    Pick::Sort(sort),
                )
            })
            .collect();

        let (marked, bulk) = self.bulk_chips();
        Chips {
            views,
            sources,
            sorts,
            organize: self.organize_chips(&in_view),
            marked,
            bulk,
        }
    }

    /// Organize: the play statuses, then the collections and saved views.
    fn organize_chips(&self, in_view: &[&Game]) -> Vec<Chip> {
        let filter = &self.filter;
        let mut organize = Vec::new();
        for status in Status::ALL {
            let count = in_view
                .iter()
                .filter(|game| game.status == Some(status))
                .count();
            // A status nobody has is offered only while it is chosen.
            if count == 0 && filter.status != Some(status) {
                continue;
            }
            organize.push(chip(
                format::play_status(status),
                Some(count),
                filter.status == Some(status),
                Pick::Status(status),
            ));
        }
        // The collections, hand-picked and saved views in one list by name.
        let mut collections: Vec<Chip> = Vec::new();
        for category in &self.categories {
            let count = in_view
                .iter()
                .filter(|game| game.categories.contains(&category.name))
                .count();
            collections.push(chip(
                category.name.clone(),
                Some(count),
                filter.category.as_deref() == Some(category.name.as_str()),
                Pick::Category(category.name.clone()),
            ));
        }
        for saved in &self.saved {
            collections.push(chip(
                saved.name.clone(),
                None,
                saved.filter == self.filter,
                Pick::Saved(saved.name.clone()),
            ));
        }
        collections.sort_by_cached_key(|chip| (chip.label.to_lowercase(), chip.label.clone()));
        organize.extend(collections);
        organize
    }

    /// How many are marked, and the bulk chips that would change a marked
    /// game; nothing when none are marked.
    fn bulk_chips(&self) -> (Option<String>, Vec<Chip>) {
        if self.marked.is_empty() {
            return (None, Vec::new());
        }
        let marked: Vec<&Game> = self
            .games
            .iter()
            .filter(|game| self.marked.contains(&game.id))
            .collect();
        let any = |test: fn(&Game) -> bool| marked.iter().any(|game| test(game));
        let chip = |key: &str, pick: Pick| Chip {
            label: match key {
                "pin" => t!("library-bulk-pin"),
                "unpin" => t!("library-bulk-unpin"),
                "hide" => t!("library-bulk-hide"),
                "unhide" => t!("library-bulk-unhide"),
                _ => t!("library-clear-selection"),
            },
            count: None,
            chosen: false,
            pick,
        };
        let mut chips = Vec::new();
        if any(|game| !game.pinned) {
            chips.push(chip("pin", Pick::BulkPin(true)));
        }
        if any(|game| game.pinned) {
            chips.push(chip("unpin", Pick::BulkPin(false)));
        }
        if any(|game| !game.hidden) {
            chips.push(chip("hide", Pick::BulkHide(true)));
        }
        if any(|game| game.hidden) {
            chips.push(chip("unhide", Pick::BulkHide(false)));
        }
        chips.push(chip("clear", Pick::ClearMarks));
        (
            Some(t!("library-selected", count = self.marked.len())),
            chips,
        )
    }

    // What changes it.

    /// The library as a scan read it, with what was saved. `sort` is the
    /// saved sort, taken on the first scan only.
    pub fn loaded(
        &mut self,
        library: Library,
        sort: Option<Sort>,
        categories: Vec<Category>,
        saved: Vec<SavedFilter>,
    ) -> Outcome {
        let id = self.selected_id();
        if let Some(sort) = sort.filter(|_| !self.loaded) {
            self.filter.sort = sort;
        }
        self.loaded = true;
        self.problems = library.problems.len();
        self.games = library.games;
        self.categories = categories;
        self.saved = saved;
        let present: BTreeSet<&str> = self.games.iter().map(|game| game.id.as_str()).collect();
        self.marked.retain(|id| present.contains(id.as_str()));
        self.status = None;
        self.reselect(id)
    }

    /// A game's image was downloaded. Only a cover changes a card.
    pub fn set_image(&mut self, id: &str, kind: Kind, path: PathBuf) -> Outcome {
        let Some(game) = self.games.iter_mut().find(|game| game.id == id) else {
            return Outcome::default();
        };
        match kind {
            Kind::Cover => game.cover = Some(path),
            Kind::Hero => game.hero = Some(path),
        }
        match self.place_of(id).filter(|_| kind == Kind::Cover) {
            Some(place) => Outcome {
                rows: Rows::Marks(vec![place]),
                ..Outcome::default()
            },
            None => Outcome::default(),
        }
    }

    /// The columns the window's width gives.
    pub fn set_columns(&mut self, columns: usize) -> Outcome {
        let columns = columns.max(1);
        if columns == self.columns {
            return Outcome::default();
        }
        self.columns = columns;
        Outcome {
            rows: Rows::All,
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    /// How far the grid is scrolled, and how tall its window is.
    pub fn scrolled(&mut self, top: f32, height: f32) {
        self.scroll_top = top.max(0.0);
        if height > 0.0 {
            self.viewport = height;
        }
    }

    /// The rows on screen, and two above and below, as places in the grid.
    #[must_use]
    pub fn rows_near(&self) -> std::ops::Range<usize> {
        let pitch = CARD_HEIGHT + SPACING;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "non-negative, and far below usize's range"
        )]
        let (first, last) = (
            (self.scroll_top / pitch) as usize,
            ((self.scroll_top + self.viewport) / pitch).ceil() as usize,
        );
        first.saturating_sub(2)..last + 2
    }

    pub fn say(&mut self, status: Option<String>) {
        self.status = status;
    }

    /// A chip was pressed.
    pub fn pick(&mut self, pick: &Pick) -> Outcome {
        match pick {
            Pick::View(view) => self.refilter(|filter| filter.view = *view),
            Pick::Clear => self.refilter(|filter| {
                filter.source = None;
                filter.category = None;
                filter.status = None;
            }),
            Pick::Source(source) => self.refilter(|filter| filter.source = Some(*source)),
            Pick::Sort(sort) => self.set_sort(*sort),
            Pick::Status(status) => self.refilter(|filter| {
                filter.status = if filter.status == Some(*status) {
                    None
                } else {
                    Some(*status)
                };
            }),
            Pick::Category(name) => self.refilter(|filter| {
                filter.category = if filter.category.as_ref() == Some(name) {
                    None
                } else {
                    Some(name.clone())
                };
            }),
            Pick::Saved(name) => self.apply_saved(name),
            Pick::BulkPin(pinned) => self.bulk_pinned(*pinned),
            Pick::BulkHide(hidden) => self.bulk_hidden(*hidden),
            Pick::ClearMarks => self.clear_marks(),
        }
    }

    pub fn cycle_sort(&mut self) -> Outcome {
        self.set_sort(self.filter.sort.next())
    }

    fn set_sort(&mut self, sort: Sort) -> Outcome {
        let mut outcome = self.refilter(|filter| filter.sort = sort);
        outcome.save = Some(Save::Sort(sort));
        outcome
    }

    /// Puts a saved view on the library. A store with no games, or a
    /// collection that is gone, is left off rather than showing nothing.
    fn apply_saved(&mut self, name: &str) -> Outcome {
        let Some(saved) = self.saved.iter().find(|saved| saved.name == name) else {
            return Outcome::default();
        };
        let mut wanted = saved.filter.clone();
        if let Some(source) = wanted.source
            && !self.games.iter().any(|game| game.source == source)
        {
            wanted.source = None;
        }
        if let Some(category) = &wanted.category
            && !self.categories.iter().any(|known| &known.name == category)
        {
            wanted.category = None;
        }
        let sort = wanted.sort;
        let mut outcome = self.refilter(|filter| *filter = wanted);
        outcome.save = Some(Save::Sort(sort));
        outcome
    }

    /// The search field's text changed.
    pub fn set_search(&mut self, search: &str) -> Outcome {
        if search == self.filter.search {
            return Outcome::default();
        }
        search.clone_into(&mut self.filter.search);
        self.filter_changed()
    }

    /// Typing with no field focused starts the search.
    pub fn type_text(&mut self, typed: &str) -> Outcome {
        self.filter.search.push_str(typed);
        self.filter_changed()
    }

    pub fn backspace(&mut self) -> Outcome {
        self.filter.search.pop();
        self.filter_changed()
    }

    /// Escape: the marks go first, then the search.
    pub fn back(&mut self) -> Outcome {
        if !self.marked.is_empty() {
            self.clear_marks()
        } else if !self.filter.search.is_empty() {
            self.filter.search.clear();
            self.filter_changed()
        } else {
            Outcome::default()
        }
    }

    pub fn show_hidden(&mut self) -> Outcome {
        self.refilter(|filter| {
            filter.view = if filter.view == View::Hidden {
                View::All
            } else {
                View::Hidden
            };
        })
    }

    /// A card was clicked: the cursor goes to it.
    pub fn click(&mut self, id: &str) {
        if let Some(place) = self.place_of(id) {
            self.cursor = place;
        }
    }

    pub fn step(&mut self, step: Move) -> Outcome {
        if self.shown.is_empty() {
            return Outcome::default();
        }
        let rows = browse::rows_in(self.viewport, GEOMETRY);
        self.cursor = browse::step(self.cursor, self.shown.len(), self.columns, rows, step);
        Outcome {
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    /// Marks a game, or leaves it; `None` is the card under the cursor.
    pub fn toggle_mark(&mut self, id: Option<&str>) -> Outcome {
        let Some(id) = id.map(str::to_owned).or_else(|| self.selected_id()) else {
            return Outcome::default();
        };
        if !self.marked.remove(&id) {
            self.marked.insert(id.clone());
        }
        self.marks_changed(&[id])
    }

    fn clear_marks(&mut self) -> Outcome {
        let ids: Vec<String> = std::mem::take(&mut self.marked).into_iter().collect();
        self.marks_changed(&ids)
    }

    fn marks_changed(&self, ids: &[String]) -> Outcome {
        let places = ids.iter().filter_map(|id| self.place_of(id)).collect();
        Outcome {
            rows: Rows::Marks(places),
            ..Outcome::default()
        }
    }

    /// Hides the game under the cursor, or brings it back.
    pub fn toggle_hidden(&mut self) -> Outcome {
        let Some(id) = self.selected_id() else {
            return Outcome::default();
        };
        let Some(game) = self.games.iter_mut().find(|game| game.id == id) else {
            return Outcome::default();
        };
        game.hidden = !game.hidden;
        let (hidden, title) = (game.hidden, game.title.clone());
        self.status = Some(if hidden {
            t!("status-hid", title = title)
        } else {
            t!("status-unhid", title = title)
        });
        // A hidden game leaves the grid; the cursor stays in place.
        let mut outcome = self.reselect(Some(id.clone()));
        outcome.save = Some(Save::Hidden(vec![id], hidden));
        outcome
    }

    /// Makes the game under the cursor a favorite, or not one.
    pub fn toggle_pinned(&mut self) -> Outcome {
        let Some(id) = self.selected_id() else {
            return Outcome::default();
        };
        let Some(game) = self.games.iter_mut().find(|game| game.id == id) else {
            return Outcome::default();
        };
        game.pinned = !game.pinned;
        let (pinned, title) = (game.pinned, game.title.clone());
        self.status = Some(if pinned {
            t!("status-pinned", title = title)
        } else {
            t!("status-unpinned", title = title)
        });
        let mut outcome = self.reselect(Some(id.clone()));
        outcome.save = Some(Save::Pinned(vec![id], pinned));
        outcome
    }

    /// Makes every marked game a favorite, or not one: the same for all, so
    /// a mixed selection ends alike. The marks stay.
    fn bulk_pinned(&mut self, pinned: bool) -> Outcome {
        let id = self.selected_id();
        let changed = browse::set_pinned(&mut self.games, &self.marked, pinned);
        if changed == 0 {
            return Outcome::default();
        }
        self.status = Some(if pinned {
            t!("status-bulk-pinned", count = changed)
        } else {
            t!("status-bulk-unpinned", count = changed)
        });
        let mut outcome = self.reselect(id);
        outcome.save = Some(Save::Pinned(self.marked.iter().cloned().collect(), pinned));
        outcome
    }

    /// Hides every marked game, or brings it back. They leave the grid, so
    /// the marks go too.
    fn bulk_hidden(&mut self, hidden: bool) -> Outcome {
        let id = self.selected_id();
        let changed = browse::set_hidden(&mut self.games, &self.marked, hidden);
        if changed == 0 {
            return Outcome::default();
        }
        let ids: Vec<String> = std::mem::take(&mut self.marked).into_iter().collect();
        self.status = Some(if hidden {
            t!("status-bulk-hid", count = changed)
        } else {
            t!("status-bulk-unhid", count = changed)
        });
        let mut outcome = self.reselect(id);
        outcome.save = Some(Save::Hidden(ids, hidden));
        outcome
    }

    // Behind the changes.

    fn place_of(&self, id: &str) -> Option<usize> {
        self.shown
            .iter()
            .position(|&index| self.games[index].id == id)
    }

    /// Changes what the grid shows, keeping the cursor on its game.
    fn refilter(&mut self, change: impl FnOnce(&mut Filter)) -> Outcome {
        let id = self.selected_id();
        change(&mut self.filter);
        self.reselect(id)
    }

    /// The search changed: back to the first card, at the top.
    fn filter_changed(&mut self) -> Outcome {
        self.recompute();
        self.cursor = 0;
        self.status = None;
        Outcome {
            rows: Rows::All,
            scroll: Scroll::Top,
            ..Outcome::default()
        }
    }

    fn recompute(&mut self) {
        self.shown = browse::visible_indices(&self.games, &self.filter, now());
    }

    /// Recomputes the shown games and puts the cursor on `id` again, or
    /// where it was when that game is gone.
    fn reselect(&mut self, id: Option<String>) -> Outcome {
        self.recompute();
        let kept = id.and_then(|id| self.place_of(&id));
        self.cursor = kept.unwrap_or_else(|| self.cursor.min(self.shown.len().saturating_sub(1)));
        Outcome {
            rows: Rows::All,
            scroll: self.reveal(),
            ..Outcome::default()
        }
    }

    /// Scrolls just enough to show the cursor's card.
    fn reveal(&self) -> Scroll {
        browse::reveal_y(
            self.cursor(),
            self.columns,
            self.scroll_top,
            self.viewport,
            GEOMETRY,
        )
        .map_or(Scroll::Keep, Scroll::To)
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::Launch;

    use super::*;

    fn game(n: usize) -> Game {
        Game::new(
            format!("desktop:{n}"),
            format!("Game {n:02}"),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        )
    }

    fn browser(count: usize, columns: usize) -> Browser {
        let mut browser = Browser::new();
        let library = Library {
            games: (0..count).map(game).collect(),
            problems: Vec::new(),
        };
        let _ = browser.loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        let _ = browser.set_columns(columns);
        browser
    }

    fn titles(browser: &Browser) -> Vec<String> {
        (0..browser.len())
            .filter_map(|place| browser.card(place))
            .map(|card| card.title)
            .collect()
    }

    #[test]
    fn the_first_scan_takes_the_saved_sort_and_a_rescan_does_not() {
        let mut browser = browser(2, 3);
        assert_eq!(browser.current().sort, Sort::Title);
        let _ = browser.pick(&Pick::Sort(Sort::MostPlayed));
        let library = Library {
            games: vec![game(0)],
            problems: Vec::new(),
        };
        let _ = browser.loaded(library, Some(Sort::Recent), Vec::new(), Vec::new());
        assert_eq!(browser.current().sort, Sort::MostPlayed);
    }

    #[test]
    fn typing_filters_by_every_word_and_back_clears_it() {
        let mut browser = browser(12, 3);
        let _ = browser.type_text("game 1");
        assert_eq!(titles(&browser), ["Game 01", "Game 10", "Game 11"]);
        let _ = browser.back();
        assert_eq!(browser.len(), 12);
        let _ = browser.type_text("zz");
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-no-match", search = "zz"))
        );
        let _ = browser.backspace();
        let _ = browser.backspace();
        assert!(browser.empty().is_none());
    }

    #[test]
    fn back_takes_the_marks_before_the_search() {
        let mut browser = browser(3, 3);
        let _ = browser.type_text("game");
        let _ = browser.toggle_mark(None);
        let _ = browser.back();
        assert!(browser.marked().is_empty());
        assert_eq!(browser.search(), "game");
        let _ = browser.back();
        assert_eq!(browser.search(), "");
    }

    #[test]
    fn arrows_move_the_cursor_and_the_grid_follows_it() {
        // Seven games in three columns: rows 0-2, 3-5 and a short 6.
        let mut browser = browser(7, 3);
        let _ = browser.step(Move::Down);
        assert_eq!(browser.cursor(), 3);
        let _ = browser.step(Move::Last);
        assert_eq!(browser.cursor(), 6);
        let _ = browser.step(Move::Down);
        assert_eq!(browser.cursor(), 6, "the last row has nowhere below");
        // A window one card high: the second row is below it, so the grid
        // scrolls just far enough.
        browser.scrolled(0.0, CARD_HEIGHT + 2.0 * GEOMETRY.padding);
        let outcome = browser.step(Move::First);
        assert_eq!(outcome.scroll, Scroll::Keep);
        let outcome = browser.step(Move::Down);
        assert_eq!(outcome.scroll, Scroll::To(CARD_HEIGHT + SPACING));
    }

    #[test]
    fn a_filter_keeps_the_cursor_on_its_game() {
        let mut browser = browser(6, 3);
        let _ = browser.step(Move::Down);
        let id = browser.selected().map(|game| game.id.clone());
        let _ = browser.pick(&Pick::Sort(Sort::Recent));
        assert_eq!(browser.selected().map(|game| game.id.clone()), id);
    }

    #[test]
    fn marks_are_bulk_changed_and_hiding_clears_them() {
        let mut browser = browser(3, 3);
        let _ = browser.toggle_mark(None);
        let _ = browser.step(Move::Right);
        let outcome = browser.toggle_mark(None);
        assert_eq!(outcome.rows, Rows::Marks(vec![1]));
        assert_eq!(browser.marked().len(), 2);
        // Nothing is a favorite yet: Favorite is offered, Unfavorite is not.
        let chips = browser.chips();
        let picks: Vec<&Pick> = chips.bulk.iter().map(|chip| &chip.pick).collect();
        assert!(picks.contains(&&Pick::BulkPin(true)));
        assert!(!picks.contains(&&Pick::BulkPin(false)));
        let outcome = browser.pick(&Pick::BulkPin(true));
        assert!(matches!(outcome.save, Some(Save::Pinned(ids, true)) if ids.len() == 2));
        assert_eq!(browser.marked().len(), 2, "pinning keeps the marks");
        let outcome = browser.pick(&Pick::BulkHide(true));
        assert!(matches!(outcome.save, Some(Save::Hidden(ids, true)) if ids.len() == 2));
        assert!(
            browser.marked().is_empty(),
            "hidden games leave, and the marks with them"
        );
        assert_eq!(browser.len(), 1);
    }

    #[test]
    fn the_hidden_chip_is_offered_only_when_something_is_hidden_or_open() {
        let mut browser = browser(2, 3);
        let has_hidden = |browser: &Browser| {
            browser
                .chips()
                .views
                .iter()
                .any(|chip| chip.pick == Pick::View(View::Hidden))
        };
        assert!(!has_hidden(&browser));
        let _ = browser.toggle_hidden();
        assert!(has_hidden(&browser));
        let _ = browser.pick(&Pick::View(View::Hidden));
        let _ = browser.toggle_hidden();
        assert!(has_hidden(&browser), "open, so still offered");
    }

    #[test]
    fn the_empty_states_say_why() {
        let mut browser = Browser::new();
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-looking"))
        );
        let _ = browser.loaded(Library::default(), None, Vec::new(), Vec::new());
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-none-found"))
        );
        let mut browser = browser_with_one_hidden();
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-all-hidden"))
        );
        let _ = browser.pick(&Pick::View(View::Hidden));
        assert!(browser.empty().is_none());
        let _ = browser.pick(&Pick::View(View::Favorites));
        let _ = browser.pick(&Pick::Source(Source::Steam));
        assert_eq!(
            browser.empty().map(|empty| empty.title),
            Some(t!("library-none-here"))
        );
    }

    fn browser_with_one_hidden() -> Browser {
        let mut hidden = game(0);
        hidden.hidden = true;
        let mut browser = Browser::new();
        let library = Library {
            games: vec![hidden],
            problems: Vec::new(),
        };
        let _ = browser.loaded(library, None, Vec::new(), Vec::new());
        browser
    }

    #[test]
    fn collections_and_saved_views_are_one_list_by_name() {
        let mut browser = browser(2, 3);
        browser.categories = vec![Category {
            name: "co-op".into(),
            games: BTreeSet::new(),
        }];
        browser.saved = vec![SavedFilter {
            name: "Backlog".into(),
            filter: Filter::default(),
        }];
        let names: Vec<String> = browser
            .chips()
            .organize
            .into_iter()
            .map(|chip| chip.label)
            .collect();
        assert_eq!(names, ["Backlog", "co-op"]);
    }

    #[test]
    fn a_saved_view_leaves_off_a_store_with_no_games() {
        let mut browser = browser(2, 3);
        browser.saved = vec![SavedFilter {
            name: "Steam".into(),
            filter: Filter {
                source: Some(Source::Steam),
                sort: Sort::MostPlayed,
                ..Filter::default()
            },
        }];
        let outcome = browser.pick(&Pick::Saved("Steam".into()));
        assert_eq!(outcome.save, Some(Save::Sort(Sort::MostPlayed)));
        assert_eq!(browser.current().source, None);
        assert_eq!(browser.len(), 2);
    }
}
