//! What a library grid shows and how its cursor moves, with no toolkit: the
//! games a [`Filter`] lets through, in order, the cursor's steps over rows of
//! cards, where to scroll to keep the cursor in view, and the bulk changes to
//! the marked games. Both windows (Iced's and Slint's) use it.

use std::collections::BTreeSet;

use crate::{Filter, Game};

/// A step of the cursor over the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
}

/// The games the filter lets through, in its sort order. Every word of the
/// search must be in the title, whatever its case.
#[must_use]
pub fn visible<'a>(games: &'a [Game], filter: &Filter, now: u64) -> Vec<&'a Game> {
    visible_indices(games, filter, now)
        .into_iter()
        .map(|index| &games[index])
        .collect()
}

/// [`visible`], as positions in `games`: for a window that keeps the list
/// and does not want to borrow it.
#[must_use]
pub fn visible_indices(games: &[Game], filter: &Filter, now: u64) -> Vec<usize> {
    let words: Vec<String> = filter
        .search
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    // The lowered title once per game, not twice per comparison.
    let mut shown: Vec<(usize, &Game, String)> = games
        .iter()
        .enumerate()
        .filter(|(_, game)| filter.view.shows(game, now))
        .filter(|(_, game)| filter.source.is_none_or(|source| game.source == source))
        .filter(|(_, game)| {
            filter
                .category
                .as_ref()
                .is_none_or(|name| game.categories.contains(name))
        })
        .filter(|(_, game)| {
            filter
                .status
                .is_none_or(|status| game.status == Some(status))
        })
        .map(|(index, game)| (index, game, game.title.to_lowercase()))
        .filter(|(_, _, title)| words.iter().all(|word| title.contains(word.as_str())))
        .collect();
    shown.sort_by(|(_, a, at), (_, b, bt)| filter.sort.compare_with_titles(a, b, at, bt));
    shown.into_iter().map(|(index, ..)| index).collect()
}

/// Where the cursor goes after `step`, in a grid of `count` cards in rows of
/// `columns`, with `page_rows` rows on screen. Without cards, nothing moves.
#[must_use]
pub fn step(current: usize, count: usize, columns: usize, page_rows: usize, step: Move) -> usize {
    if count == 0 {
        return current;
    }
    let columns = columns.max(1);
    let page = columns * page_rows.max(1);
    let last = count - 1;
    let current = current.min(last);
    match step {
        Move::Left => current.saturating_sub(1),
        Move::Right => (current + 1).min(last),
        Move::Up => current.checked_sub(columns).unwrap_or(current),
        Move::Down => {
            let below = current + columns;
            // From the row above the last, a short last row still counts.
            if below <= last || current / columns < last / columns {
                below.min(last)
            } else {
                current
            }
        }
        Move::PageUp => current.saturating_sub(page),
        Move::PageDown => (current + page).min(last),
        Move::First => 0,
        Move::Last => last,
    }
}

/// The size of a grid's cards, for [`reveal_y`]: toolkits differ in them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub card_height: f32,
    /// Between rows.
    pub spacing: f32,
    /// Around the grid, inside its scrolling area.
    pub padding: f32,
}

/// Where the grid scrolls to show the card at `selected`: just enough, or
/// `None` when it is in view already.
#[must_use]
pub fn reveal_y(
    selected: usize,
    columns: usize,
    scroll_top: f32,
    viewport: f32,
    geometry: Geometry,
) -> Option<f32> {
    #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
    let row = (selected / columns.max(1)) as f32;
    let top = row * (geometry.card_height + geometry.spacing);
    let bottom = top + geometry.card_height + 2.0 * geometry.padding;
    if top < scroll_top {
        Some(top)
    } else if bottom > scroll_top + viewport {
        Some(bottom - viewport)
    } else {
        None
    }
}

/// How many rows of cards fit in a viewport this tall, at least one.
#[must_use]
pub fn rows_in(viewport: f32, geometry: Geometry) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number of rows; heights are never negative"
    )]
    let rows = ((viewport + geometry.spacing) / (geometry.card_height + geometry.spacing)) as usize;
    rows.max(1)
}

/// Makes every marked game a favorite, or not one. Returns how many changed.
pub fn set_pinned(games: &mut [Game], marked: &BTreeSet<String>, pinned: bool) -> usize {
    change(games, marked, |game| {
        let changed = game.pinned != pinned;
        game.pinned = pinned;
        changed
    })
}

/// Hides every marked game, or brings it back. Returns how many changed.
pub fn set_hidden(games: &mut [Game], marked: &BTreeSet<String>, hidden: bool) -> usize {
    change(games, marked, |game| {
        let changed = game.hidden != hidden;
        game.hidden = hidden;
        changed
    })
}

fn change(
    games: &mut [Game],
    marked: &BTreeSet<String>,
    mut apply: impl FnMut(&mut Game) -> bool,
) -> usize {
    games
        .iter_mut()
        .filter(|game| marked.contains(&game.id))
        // Every marked game is visited, so each one's change is applied.
        .map(&mut apply)
        .filter(|changed| *changed)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Launch, Source};

    fn game(id: &str, title: &str) -> Game {
        Game::new(
            id.into(),
            title.into(),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        )
    }

    const GEOMETRY: Geometry = Geometry {
        card_height: 100.0,
        spacing: 10.0,
        padding: 20.0,
    };

    #[test]
    fn typing_filters_by_every_word() {
        let games = [game("a", "Half Life"), game("b", "Life Is Strange")];
        let filter = Filter {
            search: "LIFE half".into(),
            ..Filter::default()
        };
        let shown = visible(&games, &filter, 0);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].id, "a");
    }

    #[test]
    fn arrows_move_through_the_grid() {
        // Seven cards in three columns: rows 0-2, 3-5 and a short 6.
        let go = |from, direction| step(from, 7, 3, 2, direction);
        assert_eq!(go(1, Move::Up), 1);
        assert_eq!(go(1, Move::Down), 4);
        assert_eq!(go(5, Move::Down), 6);
        assert_eq!(go(6, Move::Down), 6);
        assert_eq!(go(6, Move::Right), 6);
        assert_eq!(go(0, Move::Left), 0);
        assert_eq!(go(2, Move::Last), 6);
        assert_eq!(go(6, Move::PageUp), 0);
        assert_eq!(go(0, Move::PageDown), 6);
        assert_eq!(step(3, 0, 3, 2, Move::Down), 3, "no cards, no move");
    }

    #[test]
    fn reveals_the_selected_row_just_enough() {
        let pitch = GEOMETRY.card_height + GEOMETRY.spacing;
        let card = GEOMETRY.card_height + 2.0 * GEOMETRY.padding;
        // Row 0 is above a scroll of one row: back to the top.
        assert_eq!(reveal_y(0, 3, pitch + 1.0, 800.0, GEOMETRY), Some(0.0));
        // Already in view: nothing moves.
        assert_eq!(reveal_y(0, 3, 0.0, card + 1.0, GEOMETRY), None);
        // Row 1 is below a viewport that shows only row 0.
        assert_eq!(reveal_y(3, 3, 0.0, pitch, GEOMETRY), Some(card));
    }

    #[test]
    fn a_page_is_at_least_a_row() {
        assert_eq!(rows_in(0.0, GEOMETRY), 1);
        assert_eq!(rows_in(340.0, GEOMETRY), 3);
    }

    #[test]
    fn bulk_changes_count_only_what_changed() {
        let mut games = [game("a", "A"), game("b", "B"), game("c", "C")];
        games[1].pinned = true;
        let marked: BTreeSet<String> = ["a".to_owned(), "b".to_owned()].into();
        assert_eq!(set_pinned(&mut games, &marked, true), 1);
        assert!(games[0].pinned && games[1].pinned && !games[2].pinned);
        assert_eq!(set_pinned(&mut games, &marked, true), 0);
        assert_eq!(set_hidden(&mut games, &marked, true), 2);
        assert!(!games[2].hidden);
        assert_eq!(set_hidden(&mut games, &marked, false), 2);
    }
}
