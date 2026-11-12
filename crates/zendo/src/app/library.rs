//! The Library page: a header with the search, the store and sort chips,
//! and the grid of covers.

use iced::widget::{button, column, container, image, row, scrollable, space, stack, text};
use std::collections::BTreeSet;
use std::path::Path;

use iced::{Alignment, Color, ContentFit, Element, Length};
use zendo_core::{Game, Sort, Source};

use super::categories::CategoriesMessage;
use super::style::{self, BOLD, SPACE_L, SPACE_S, SPACE_XL, colors};
use super::top_bar;
use super::{Message, Zendo, centered, format, grid_id, now};
use crate::i18n::t;

pub const COVER_WIDTH: f32 = 180.0;
pub const COVER_HEIGHT: f32 = 270.0;
/// Two lines of the title and one of the line under it.
const CAPTION_HEIGHT: f32 = 60.0;
const CARD_PADDING: f32 = 6.0;
pub const CARD_WIDTH: f32 = COVER_WIDTH + 2.0 * CARD_PADDING;
pub const CARD_HEIGHT: f32 = COVER_HEIGHT + CAPTION_HEIGHT + 4.0 + 2.0 * CARD_PADDING;
pub const SPACING: f32 = 16.0;
/// The progress line along a cover's foot.
const PROGRESS_HEIGHT: f32 = 3.0;
/// Around the grid, inside its scrollable.
pub const GRID_PADDING: f32 = 24.0;
/// Room kept for the scrollbar beside the grid.
const SCROLLBAR: f32 = 12.0;

/// How many days back a game counts as recently played.
pub const RECENT_DAYS: u64 = 30;

/// Which of the games the view chips show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

/// How many cards fit across a window this wide.
pub fn columns_for(window_width: f32) -> usize {
    let room = window_width - 2.0 * GRID_PADDING - SCROLLBAR + SPACING;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "widths are positive and far below usize's range"
    )]
    let columns = (room / (CARD_WIDTH + SPACING)).floor().max(1.0) as usize;
    columns
}

/// The narrowest window: the top bar, or two columns if they are wider.
pub fn min_width() -> f32 {
    (2.0 * GRID_PADDING + SCROLLBAR + 2.0 * CARD_WIDTH + SPACING).max(top_bar::MIN_WIDTH)
}

impl Zendo {
    pub(super) fn library_page(&self) -> Element<'_, Message> {
        let visible = self.visible();
        let body: Element<'_, Message> = match &self.library {
            None => centered(text(t!("library-looking")).color(colors().dim)),
            Some(library) if library.games.is_empty() => centered(
                column![
                    text(t!("library-none-found")).size(20),
                    text(t!("library-none-found-about")).color(colors().dim),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            ),
            Some(_) if visible.is_empty() && !self.search.is_empty() => centered(
                text(t!("library-no-match", search = self.search.as_str())).color(colors().dim),
            ),
            Some(_) if visible.is_empty() && (self.source.is_some() || self.category.is_some()) => {
                centered(text(t!("library-none-here")).color(colors().dim))
            }
            Some(_) if visible.is_empty() => centered(
                text(match self.view {
                    View::All => t!("library-all-hidden"),
                    View::Favorites => t!("library-no-favorites"),
                    View::Recent => t!("library-no-recent"),
                    View::Hidden => t!("library-no-hidden"),
                })
                .color(colors().dim),
            ),
            Some(_) => self.grid(&visible),
        };
        column![
            self.view_row(),
            self.source_row(),
            self.organize_row(),
            body
        ]
        .spacing(4)
        .into()
    }

    /// The view chips with their counts, then Add game and Refresh.
    fn view_row(&self) -> Element<'_, Message> {
        let games: Vec<&Game> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .collect();
        let now = now();
        let hidden = games.iter().filter(|game| game.hidden).count();
        let mut chips: Vec<Element<'_, Message>> = Vec::new();
        for view in View::ALL {
            // Hidden is offered only when something is hidden, or is open.
            if view == View::Hidden && hidden == 0 && self.view != View::Hidden {
                continue;
            }
            let count = games.iter().filter(|game| view.shows(game, now)).count();
            chips.push(chip(
                match view {
                    View::All => t!("library-all"),
                    View::Favorites => t!("library-favorites"),
                    View::Recent => t!("library-recent"),
                    View::Hidden => t!("library-hidden"),
                },
                Some(count),
                self.view == view,
                Message::SetView(view),
            ));
        }
        let refresh = button(style::button_text(
            if self.scanning {
                t!("library-scanning")
            } else {
                t!("library-refresh")
            }
            .to_uppercase(),
        ))
        .padding([4, 12])
        .style(style::secondary)
        .on_press_maybe((!self.scanning).then_some(Message::Refresh));
        row![
            row(chips).spacing(SPACE_S).wrap().vertical_spacing(SPACE_S),
            space::horizontal(),
            button(style::button_text(t!("library-add-game").to_uppercase()))
                .padding([4, 12])
                .style(style::secondary)
                .on_press(Message::Local(super::local::LocalMessage::Add)),
            refresh,
        ]
        .spacing(SPACE_S)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: SPACE_XL,
            right: GRID_PADDING,
            bottom: SPACE_S,
            left: GRID_PADDING,
        })
        .into()
    }

    /// The games the view lets through, before the store and category
    /// chips narrow them.
    fn in_view(&self) -> Vec<&Game> {
        let now = now();
        self.library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| self.view.shows(game, now))
            .collect()
    }

    /// The store chips, then the sort chips with their key.
    fn source_row(&self) -> Element<'_, Message> {
        let games = self.in_view();
        let mut sources: Vec<Element<'_, Message>> = vec![chip(
            t!("library-all"),
            Some(games.len()),
            self.source.is_none() && self.category.is_none(),
            Message::ClearFilters,
        )];
        for source in Source::ALL {
            let count = games.iter().filter(|game| game.source == source).count();
            if count > 0 {
                sources.push(chip(
                    format::source(source),
                    Some(count),
                    self.source == Some(source),
                    Message::SetSource(Some(source)),
                ));
            }
        }
        let mut sorts: Vec<Element<'_, Message>> = vec![
            container(style::label(&t!("library-sort-keys")))
                .padding([6, 0])
                .into(),
        ];
        for sort in [Sort::Recent, Sort::MostPlayed, Sort::Title] {
            sorts.push(chip(
                format::sort(sort),
                None,
                self.sort == sort,
                Message::SetSort(sort),
            ));
        }
        row![
            row(sources).spacing(SPACE_S),
            row(sorts).spacing(SPACE_S).align_y(Alignment::Center),
        ]
        .spacing(SPACE_L)
        .padding([4.0, GRID_PADDING])
        .wrap()
        .vertical_spacing(SPACE_S)
        .into()
    }

    /// Organize: the user's categories, the way to change them, and what
    /// is marked in the grid.
    fn organize_row(&self) -> Element<'_, Message> {
        let games = self.in_view();
        let mut chips: Vec<Element<'_, Message>> = vec![
            container(style::label(&t!("library-organize")))
                .padding([6, 0])
                .into(),
        ];
        for category in &self.categories {
            let count = games
                .iter()
                .filter(|game| game.categories.contains(&category.name))
                .count();
            chips.push(chip(
                category.name.clone(),
                Some(count),
                self.category.as_deref() == Some(category.name.as_str()),
                Message::SetCategory(Some(category.name.clone())),
            ));
        }
        chips.push(chip(
            t!("categories-open"),
            (!self.categories.is_empty()).then_some(self.categories.len()),
            false,
            Message::Categories(CategoriesMessage::Open(BTreeSet::new())),
        ));
        self.selection_chips(&mut chips);
        row(chips)
            .spacing(SPACE_S)
            .align_y(Alignment::Center)
            .padding([4.0, GRID_PADDING])
            .wrap()
            .vertical_spacing(SPACE_S)
            .into()
    }

    /// How many games are marked, the way to assign them all, and the way
    /// to leave them, when any are.
    fn selection_chips<'a>(&'a self, chips: &mut Vec<Element<'a, Message>>) {
        if self.selected_games.is_empty() {
            return;
        }
        chips.push(
            container(
                text(t!("library-selected", count = self.selected_games.len()))
                    .size(13)
                    .color(colors().accent),
            )
            .padding([6, 0])
            .into(),
        );
        chips.push(chip(
            t!("categories-assign"),
            None,
            false,
            Message::Categories(CategoriesMessage::Open(self.selected_games.clone())),
        ));
        chips.push(chip(
            t!("library-clear-selection"),
            None,
            false,
            Message::ClearSelection,
        ));
    }

    fn grid<'a>(&'a self, visible: &[&'a Game]) -> Element<'a, Message> {
        let selected = self.selected.min(visible.len().saturating_sub(1));
        let now = now();
        let cards = visible.iter().enumerate().map(|(index, game)| {
            let (line, line_color) = if game.hidden {
                (t!("library-hidden"), colors().accent)
            } else {
                (played_line(game, now), colors().dim)
            };
            card(Card {
                title: &game.title,
                source: game.source,
                cover: game.cover.as_deref(),
                faded: game.hidden,
                badge: if self.running.is_running(&game.id) {
                    Some((t!("badge-running"), colors().good))
                } else {
                    game.pinned.then(|| (t!("badge-favorite"), colors().accent))
                },
                progress: game.achievements.and_then(progress_of),
                check: Some((
                    self.selected_games.contains(&game.id),
                    Message::ToggleSelectedAt(game.id.clone()),
                )),
                line,
                line_color,
                selected: index == selected,
                on_press: Message::Clicked(game.id.clone()),
            })
        });
        let window = Window {
            scroll_top: self.scroll_top,
            height: self.viewport_height,
        };
        cards_grid(cards, self.columns, window, super::Grid::Library)
    }
}

/// Cards in rows of `columns`, in the page's scrollable.
/// What part of a grid is on screen: how far it is scrolled, and how tall
/// its window is.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub scroll_top: f32,
    pub height: f32,
}

/// Rows built beyond those on screen, above and below, so a scroll shows
/// cards at once.
const SPARE_ROWS: usize = 2;

/// The rows of a grid of `count` cards worth building for the part on
/// screen: the first, and how many.
fn rows_on_screen(count: usize, columns: usize, window: Window) -> (usize, usize) {
    let rows = count.div_ceil(columns);
    let row_height = CARD_HEIGHT + SPACING;
    // Before the first scroll the window's height is not known yet.
    let height = if window.height > 0.0 {
        window.height
    } else {
        2000.0
    };
    let top = (window.scroll_top - GRID_PADDING).max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "non-negative, and far below usize's range"
    )]
    let (first, last) = (
        (top / row_height) as usize,
        ((top + height) / row_height).ceil() as usize,
    );
    let first = first.saturating_sub(SPARE_ROWS).min(rows);
    let last = (last + SPARE_ROWS).min(rows);
    (first, last.saturating_sub(first))
}

/// A grid of cards, of which only the rows near the screen are built:
/// with a thousand games, laying out every card (and its cover) made each
/// redraw slow. The rows left out are space of the same height, so the
/// scroll bar and the cursor's place stay right.
pub fn cards_grid<'a>(
    cards: impl ExactSizeIterator<Item = Element<'a, Message>>,
    columns: usize,
    window: Window,
    page: super::Grid,
) -> Element<'a, Message> {
    // The count is the cards': two numbers could disagree, and then the
    // spacers and the scroll bar would lie.
    let count = cards.len();
    let columns = columns.max(1);
    let total = count.div_ceil(columns);
    let (first, shown) = rows_on_screen(count, columns, window);
    let row_height = CARD_HEIGHT + SPACING;
    #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
    let space_for = |rows: usize| rows as f32 * row_height - SPACING;
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    if first > 0 {
        rows.push(space::vertical().height(space_for(first)).into());
    }
    let mut current = Vec::with_capacity(columns);
    for card in cards.skip(first * columns).take(shown * columns) {
        current.push(card);
        if current.len() == columns {
            rows.push(row(std::mem::take(&mut current)).spacing(SPACING).into());
        }
    }
    if !current.is_empty() {
        rows.push(row(current).spacing(SPACING).into());
    }
    let after = total - first - shown;
    if after > 0 {
        rows.push(space::vertical().height(space_for(after)).into());
    }
    // A fixed width, centred, so a short last row stays in its columns.
    #[expect(clippy::cast_precision_loss, reason = "column counts are small")]
    let width = columns as f32 * (CARD_WIDTH + SPACING) - SPACING;
    let grid = column(rows).spacing(SPACING).width(width);
    scrollable(container(grid).center_x(Length::Fill).padding(GRID_PADDING))
        .id(grid_id())
        .on_scroll(move |viewport| Message::Scrolled(page, viewport))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A toggle in a page's toolbar, with a count when given.
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers hand over a label they just built"
)]
pub(super) fn chip<'a>(
    label: String,
    count: Option<usize>,
    chosen: bool,
    message: Message,
) -> Element<'a, Message> {
    let mut content = row![text(label.to_uppercase()).size(13)].spacing(6);
    if let Some(count) = count {
        content = content.push(text(count.to_string()).size(12).color(if chosen {
            colors().accent
        } else {
            colors().dim
        }));
    }
    button(content.align_y(Alignment::Center))
        .padding([4, 12])
        .style(style::chip(chosen))
        .on_press(message)
        .into()
}

/// The small box in a cover's corner that marks the card.
fn check_button<'a>(ticked: bool, message: Message) -> Element<'a, Message> {
    button(space::horizontal().width(0))
        .width(20)
        .height(20)
        .padding(0)
        .style(style::square(ticked))
        .on_press(message)
        .into()
}

/// The share of achievements unlocked, for a card's line: nothing for a
/// game with none.
pub fn progress_of((unlocked, total): (u32, u32)) -> Option<f32> {
    #[expect(clippy::cast_precision_loss, reason = "counts of achievements")]
    (total > 0).then(|| unlocked.min(total) as f32 / total as f32)
}

/// The line under a card's title: how much it was played.
pub(super) fn played_line(game: &Game, now: u64) -> String {
    match (game.played.minutes, game.played.last) {
        (0, None) => t!("played-never"),
        (0, Some(last)) => t!("played-when", when = format::ago(last, now)),
        (minutes, _) => t!("played-time", time = format::play_time(minutes)),
    }
}

/// The foot of a cover: the title over a fade (a cover with none shows it
/// in the middle already), then the progress line.
fn cover_foot(title: &str, has_cover: bool, progress: Option<f32>) -> Element<'_, Message> {
    let mut foot = column![].width(Length::Fill);
    if has_cover {
        foot = foot.push(
            container(
                text(title.to_uppercase())
                    .size(12)
                    .font(BOLD)
                    .wrapping(text::Wrapping::WordOrGlyph),
            )
            .padding(iced::Padding {
                top: SPACE_L,
                ..iced::Padding::new(SPACE_S)
            })
            .width(Length::Fill)
            .max_height(COVER_HEIGHT / 2.0)
            .clip(true)
            .style(style::cover_fade),
        );
    }
    if let Some(share) = progress {
        foot = foot.push(
            container(
                container(space::vertical().height(PROGRESS_HEIGHT))
                    .width(COVER_WIDTH * share.clamp(0.0, 1.0))
                    .style(|_| container::Style {
                        background: Some(iced::Background::Color(colors().accent)),
                        ..container::Style::default()
                    }),
            )
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors().raised)),
                ..container::Style::default()
            }),
        );
    }
    foot.into()
}

/// What a card shows.
pub struct Card<'a> {
    pub title: &'a str,
    pub source: Source,
    pub cover: Option<&'a Path>,
    pub faded: bool,
    /// A label in the cover's top right corner.
    pub badge: Option<(String, Color)>,
    /// A box in the cover's top left corner and the message that marks the
    /// card, when the grid can mark.
    pub check: Option<(bool, Message)>,
    /// A thin line along the cover's foot, filled this much (0 to 1).
    pub progress: Option<f32>,
    pub line: String,
    pub line_color: Color,
    pub selected: bool,
    pub on_press: Message,
}

pub fn card(card: Card<'_>) -> Element<'_, Message> {
    let Card {
        title,
        source,
        cover,
        faded,
        badge,
        check,
        progress,
        line,
        line_color,
        selected,
        on_press,
    } = card;
    let ticked = check.as_ref().is_some_and(|(ticked, _)| *ticked);
    let has_cover = cover.is_some();
    let cover: Element<'_, Message> = match cover {
        Some(path) => image(image::Handle::from_path(path))
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .content_fit(ContentFit::Cover)
            .border_radius(style::RADIUS_PANEL)
            .opacity(if faded { 0.35_f32 } else { 1.0_f32 })
            .into(),
        None => container(text(title).size(18).center())
            .padding(12)
            .center(Length::Fill)
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .style(style::placeholder)
            .into(),
    };
    // The mark box top left, the badge top right.
    let mut corners = row![].width(Length::Fill);
    if let Some((ticked, message)) = check {
        corners = corners.push(check_button(ticked, message));
    }
    corners = corners.push(space::horizontal());
    if let Some((label, color)) = badge {
        corners = corners.push(style::badge(label, color));
    }
    let foot = cover_foot(title, has_cover, progress);
    let art = stack![
        cover,
        container(column![
            container(corners).padding(SPACE_S),
            space::vertical(),
            foot
        ])
        .width(COVER_WIDTH)
        .height(COVER_HEIGHT),
    ];

    let caption = column![
        text(title).size(14).wrapping(text::Wrapping::WordOrGlyph),
        text(format!(
            "{} · {line}",
            format::source(source).to_uppercase()
        ))
        .size(12)
        .color(line_color),
    ]
    .spacing(2)
    .height(CAPTION_HEIGHT)
    .clip(true);

    button(column![art, caption].spacing(4))
        .padding(CARD_PADDING)
        .width(CARD_WIDTH)
        .height(CARD_HEIGHT)
        .on_press(on_press)
        .style(style::card_button(ticked, selected))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_says_how_much_was_played() {
        let mut game = Game::new(
            "desktop:x".into(),
            "X".into(),
            Source::Desktop,
            zendo_core::Launch::Uri { uri: String::new() },
        );
        assert_eq!(played_line(&game, 0), t!("played-never"));
        game.played.last = Some(0);
        assert_eq!(
            played_line(&game, 0),
            t!("played-when", when = format::ago(0, 0))
        );
        game.played.minutes = 90;
        assert_eq!(
            played_line(&game, 0),
            t!("played-time", time = format::play_time(90))
        );
    }

    #[test]
    fn fits_whole_cards_across_the_window() {
        assert_eq!(columns_for(100.0), 1);
        let five = 2.0 * GRID_PADDING + SCROLLBAR + 5.0 * CARD_WIDTH + 4.0 * SPACING;
        assert_eq!(columns_for(five), 5);
        assert_eq!(columns_for(five - 1.0), 4);
        assert!(columns_for(min_width()) >= 2);
    }

    #[test]
    fn builds_only_the_rows_near_the_screen() {
        let row = CARD_HEIGHT + SPACING;
        let window = |scroll_top: f32| Window {
            scroll_top,
            height: 3.0 * row,
        };
        // 1,000 cards in 5 columns: 200 rows.
        assert_eq!(rows_on_screen(1000, 5, window(0.0)), (0, 3 + SPARE_ROWS));
        let (first, shown) = rows_on_screen(1000, 5, window(GRID_PADDING + 50.0 * row));
        assert_eq!(first, 50 - SPARE_ROWS);
        assert_eq!(shown, 3 + 2 * SPARE_ROWS);
        // At the end, no rows past the last.
        let (first, shown) = rows_on_screen(1000, 5, window(GRID_PADDING + 199.0 * row));
        assert_eq!(first + shown, 200);
        assert_eq!(rows_on_screen(3, 5, window(0.0)), (0, 1));
        assert_eq!(rows_on_screen(0, 5, window(0.0)), (0, 0));
    }
}
