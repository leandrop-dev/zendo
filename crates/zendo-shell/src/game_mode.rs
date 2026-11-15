//! Game Mode, toolkit-free: the full-screen, controller-first screen of one
//! game large (the hero) over a row of covers (the carousel) taken from a
//! shelf. What it holds is the shelf, the game in the hero and the Start
//! menu; the window draws it and starts the game.
//!
//! These rules came from the Iced window's `app/game_mode.rs` and `hero.rs`,
//! deleted at the cut-over (item 54).

use zendo_core::Game;
use zendo_core::browse::Move;

use crate::{format, t};

/// The tile's width and the room around it, as the carousel lays them out.
pub const TILE_WIDTH: f32 = 110.0;
pub const TILE_PADDING: f32 = 4.0;
/// The gap between tiles, and the margin of the screen.
pub const TILE_GAP: f32 = 8.0;
pub const MARGIN: f32 = 24.0;
/// The most lines of a description the hero shows.
const DESCRIPTION_LINES: usize = 4;

/// What the carousel lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    /// The games played lately, the latest first.
    Continue,
    /// Every game the library shows.
    All,
}

impl Shelf {
    pub const ALL: [Self; 2] = [Self::Continue, Self::All];

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Continue => t!("shelf-continue"),
            Self::All => t!("gm-all-games"),
        }
    }
}

/// The Start menu's choices, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuChoice {
    Resume,
    Library,
    Leave,
}

impl MenuChoice {
    pub const ALL: [Self; 3] = [Self::Resume, Self::Library, Self::Leave];

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Resume => t!("gm-resume"),
            Self::Library => t!("gm-to-library"),
            Self::Leave => t!("gm-leave"),
        }
    }
}

/// The games played, the latest first, hidden ones left out.
#[must_use]
pub fn played_games(games: &[Game]) -> Vec<&Game> {
    let mut played: Vec<&Game> = games
        .iter()
        .filter(|game| !game.hidden && game.played.last.is_some())
        .collect();
    played.sort_by_key(|game| std::cmp::Reverse(game.played.last));
    played
}

/// The games on a shelf, in its order.
#[must_use]
pub fn shelf_games(games: &[Game], shelf: Shelf) -> Vec<&Game> {
    match shelf {
        Shelf::Continue => played_games(games),
        Shelf::All => games.iter().filter(|game| !game.hidden).collect(),
    }
}

/// "STEAM · 12 h · today": the line over a game's title.
#[must_use]
pub fn meta_line(game: &Game, now: u64) -> String {
    let mut parts = vec![format::source(game.source)];
    if game.played.minutes > 0 {
        parts.push(format::play_time(game.played.minutes));
    }
    if let Some(last) = game.played.last {
        parts.push(format::ago(last, now));
    }
    parts.join(" · ").to_uppercase()
}

/// What the hero says about a game: the summary its store or Steam gave
/// (its first lines) when there is one, else who made it and for what.
#[must_use]
pub fn description(summary: Option<&str>, game: &Game) -> String {
    if let Some(summary) = summary {
        let lines: Vec<&str> = summary
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(DESCRIPTION_LINES)
            .collect();
        if !lines.is_empty() {
            return lines.join("\n");
        }
    }
    let mut parts: Vec<String> = Vec::new();
    parts.extend(game.details.developer.clone());
    parts.extend(game.details.platform.clone());
    if parts.is_empty() {
        t!("gm-no-description")
    } else {
        parts.join(" · ")
    }
}

/// How many tiles fit in a window this wide, odd so one sits in the middle.
#[must_use]
pub fn tiles_that_fit(width: f32) -> usize {
    let room = (width - 2.0 * MARGIN).max(TILE_WIDTH);
    let each = TILE_WIDTH + 2.0 * TILE_PADDING + TILE_GAP;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of tiles"
    )]
    let fit = (room / each) as usize;
    fit.max(1) | 1
}

/// The first of `count` tiles shown around `index` among `len`: centred, and
/// held inside the shelf at both ends.
#[must_use]
pub fn first_tile(index: usize, len: usize, count: usize) -> usize {
    index
        .saturating_sub(count / 2)
        .min(len.saturating_sub(count))
}

/// Game Mode, while it is on.
#[derive(Debug, Clone)]
pub struct GameMode {
    pub shelf: Shelf,
    /// The game in the hero: an index into the shelf.
    pub index: usize,
    /// The Start menu, open on this choice.
    pub menu: Option<usize>,
    /// The library's own grid is showing, not the Game Mode screen.
    pub browsing: bool,
    /// The game whose Steam facts were last asked for as the hero's, so a
    /// game is asked for once each time it takes the hero.
    pub facts_for: Option<String>,
}

impl Default for GameMode {
    fn default() -> Self {
        Self {
            shelf: Shelf::Continue,
            index: 0,
            menu: None,
            browsing: false,
            facts_for: None,
        }
    }
}

impl GameMode {
    /// The shelf shown: the one chosen, or the next with games when it is
    /// empty (Continue with nothing played shows every game).
    #[must_use]
    pub fn shown_shelf(&self, games: &[Game]) -> Shelf {
        if !shelf_games(games, self.shelf).is_empty() {
            return self.shelf;
        }
        Shelf::ALL
            .into_iter()
            .find(|shelf| !shelf_games(games, *shelf).is_empty())
            .unwrap_or(self.shelf)
    }

    /// The place of the hero in the shown shelf, kept inside it.
    #[must_use]
    pub fn hero_index(&self, games: &[Game]) -> usize {
        let len = shelf_games(games, self.shown_shelf(games)).len();
        self.index.min(len.saturating_sub(1))
    }

    /// The game in the hero.
    #[must_use]
    pub fn hero<'a>(&self, games: &'a [Game]) -> Option<&'a Game> {
        shelf_games(games, self.shown_shelf(games))
            .get(self.hero_index(games))
            .copied()
    }

    /// One tile along, stopping at the ends.
    pub fn step(&mut self, by: isize, games: &[Game]) {
        let shown = self.shown_shelf(games);
        let len = shelf_games(games, shown).len();
        self.shelf = shown;
        self.index = self
            .index
            .saturating_add_signed(by)
            .min(len.saturating_sub(1));
    }

    /// The next shelf with games (or the one before), from the first game.
    pub fn cycle_shelf(&mut self, by: isize, games: &[Game]) {
        let current = self.shown_shelf(games);
        let at = Shelf::ALL
            .iter()
            .position(|shelf| *shelf == current)
            .unwrap_or(0);
        let count = Shelf::ALL.len();
        for turn in 1..=count {
            #[expect(clippy::cast_possible_wrap, reason = "two shelves")]
            let next = (at as isize + by * turn as isize).rem_euclid(count as isize) as usize;
            let target = Shelf::ALL[next];
            if !shelf_games(games, target).is_empty() {
                self.shelf = target;
                self.index = 0;
                break;
            }
        }
    }

    pub fn set_shelf(&mut self, chosen: Shelf) {
        self.shelf = chosen;
        self.index = 0;
    }

    pub fn open_menu(&mut self) {
        self.menu = Some(0);
    }

    /// Moves along the menu, stopping at the ends.
    pub fn move_menu(&mut self, step: Move) {
        let Some(at) = self.menu else {
            return;
        };
        let last = MenuChoice::ALL.len() - 1;
        self.menu = Some(match step {
            Move::Up | Move::Left => at.saturating_sub(1),
            Move::Down | Move::Right => (at + 1).min(last),
            Move::PageUp | Move::First => 0,
            Move::PageDown | Move::Last => last,
        });
    }

    /// The menu's choice under the cursor.
    #[must_use]
    pub fn menu_choice(&self) -> Option<MenuChoice> {
        self.menu.and_then(|at| MenuChoice::ALL.get(at).copied())
    }

    /// Closes the menu and does what it chose. True when Game Mode ends.
    pub fn choose(&mut self, choice: MenuChoice) -> bool {
        self.menu = None;
        match choice {
            MenuChoice::Resume => false,
            MenuChoice::Library => {
                self.browsing = true;
                false
            }
            MenuChoice::Leave => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn game(id: &str) -> Game {
        Game::new(
            id.into(),
            id.to_uppercase(),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        )
    }

    fn played(id: &str, last: u64) -> Game {
        let mut game = game(id);
        game.played.last = Some(last);
        game
    }

    #[test]
    fn played_games_come_latest_first_and_leave_hidden_ones_out() {
        let mut hidden = played("c", 200);
        hidden.hidden = true;
        let games = [played("a", 100), played("b", 300), hidden, game("d")];
        let ids: Vec<&str> = played_games(&games)
            .iter()
            .map(|game| game.id.as_str())
            .collect();
        assert_eq!(ids, ["b", "a"]);
    }

    #[test]
    fn the_meta_line_names_the_store_time_and_when() {
        let mut game = game("a");
        assert_eq!(
            meta_line(&game, 0),
            format::source(Source::Desktop).to_uppercase()
        );
        game.played.minutes = 90;
        game.played.last = Some(0);
        let line = meta_line(&game, 0);
        assert!(
            line.contains(&format::play_time(90).to_uppercase()),
            "{line}"
        );
        assert!(line.contains(&format::ago(0, 0).to_uppercase()), "{line}");
    }

    #[test]
    fn the_description_is_the_first_lines_or_who_made_it() {
        let mut game = game("a");
        let long = "one\n\n two \nthree\nfour\nfive";
        assert_eq!(description(Some(long), &game), "one\n two \nthree\nfour");
        assert_eq!(description(Some("  \n"), &game), t!("gm-no-description"));
        game.details.developer = Some("Valve".into());
        game.details.platform = Some("Linux".into());
        assert_eq!(description(None, &game), "Valve · Linux");
    }

    #[test]
    fn the_carousel_shows_an_odd_number_of_tiles() {
        for width in [960.0, 1360.0, 1920.0, 2560.0] {
            assert_eq!(tiles_that_fit(width) % 2, 1, "{width}");
        }
        assert!(tiles_that_fit(1920.0) > tiles_that_fit(960.0));
    }

    #[test]
    fn the_tiles_are_centred_and_held_inside_the_shelf() {
        assert_eq!(first_tile(10, 20, 7), 7);
        assert_eq!(first_tile(0, 20, 7), 0, "at the start");
        assert_eq!(first_tile(19, 20, 7), 13, "at the end");
        assert_eq!(first_tile(2, 4, 4), 0, "a shelf no longer than the row");
    }

    #[test]
    fn nothing_played_shows_every_game_and_moving_stops_at_the_ends() {
        let games = [game("a"), game("b"), game("c")];
        let mut mode = GameMode::default();
        assert_eq!(mode.shown_shelf(&games), Shelf::All);
        assert_eq!(mode.hero(&games).unwrap().id, "a");
        mode.step(-1, &games);
        assert_eq!(mode.index, 0, "not before the first");
        mode.step(1, &games);
        mode.step(1, &games);
        mode.step(1, &games);
        assert_eq!(mode.index, 2, "not past the last");
        assert_eq!(mode.hero(&games).unwrap().id, "c");
    }

    #[test]
    fn shelves_cycle_past_an_empty_one() {
        let mut games = vec![game("a"), game("b")];
        let mut mode = GameMode::default();
        mode.cycle_shelf(1, &games);
        assert_eq!(mode.shelf, Shelf::All, "Continue is empty, so All stays");
        games[1].played.last = Some(5);
        mode.cycle_shelf(1, &games);
        assert_eq!(mode.shelf, Shelf::Continue);
        assert_eq!(mode.hero(&games).unwrap().id, "b");
        mode.cycle_shelf(-1, &games);
        assert_eq!(mode.shelf, Shelf::All);
        assert_eq!(mode.index, 0);
    }

    #[test]
    fn the_menu_moves_and_its_choices_do_their_part() {
        let mut mode = GameMode::default();
        mode.move_menu(Move::Down);
        assert_eq!(mode.menu, None, "a closed menu does not move");
        mode.open_menu();
        assert_eq!(mode.menu_choice(), Some(MenuChoice::Resume));
        mode.move_menu(Move::Down);
        mode.move_menu(Move::Down);
        mode.move_menu(Move::Down);
        assert_eq!(
            mode.menu_choice(),
            Some(MenuChoice::Leave),
            "stops at the end"
        );
        mode.move_menu(Move::Up);
        let choice = mode.menu_choice().unwrap();
        assert!(!mode.choose(choice));
        assert!(
            mode.browsing && mode.menu.is_none(),
            "Library shows the grid"
        );
        mode.open_menu();
        assert!(mode.choose(MenuChoice::Leave));
        assert!(!mode.choose(MenuChoice::Resume));
    }
}
