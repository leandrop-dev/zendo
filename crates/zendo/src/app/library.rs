//! The Library page: a header with the search, the store and sort chips,
//! and the grid of covers.

use iced::widget::{
    button, column, container, image, row, scrollable, space, stack, text, text_input,
};
use std::path::Path;

use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length};
use zendo_core::{Game, Sort, Source};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, centered, format, grid_id, now, search_id, sidebar};
use crate::i18n::t;

pub const COVER_WIDTH: f32 = 180.0;
pub const COVER_HEIGHT: f32 = 270.0;
const CAPTION_HEIGHT: f32 = 42.0;
const CARD_PADDING: f32 = 6.0;
pub const CARD_WIDTH: f32 = COVER_WIDTH + 2.0 * CARD_PADDING;
pub const CARD_HEIGHT: f32 = COVER_HEIGHT + CAPTION_HEIGHT + 4.0 + 2.0 * CARD_PADDING;
pub const SPACING: f32 = 16.0;
/// Around the grid, inside its scrollable.
pub const GRID_PADDING: f32 = 24.0;
/// Room kept for the scrollbar beside the grid.
const SCROLLBAR: f32 = 12.0;

const SOURCES: [Source; 7] = [
    Source::Steam,
    Source::Epic,
    Source::Gog,
    Source::Amazon,
    Source::Heroic,
    Source::Desktop,
    Source::Local,
];

/// How many cards fit across a window this wide, beside the sidebar.
pub fn columns_for(window_width: f32) -> usize {
    let room = window_width - sidebar::WIDTH - 2.0 * GRID_PADDING - SCROLLBAR + SPACING;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "widths are positive and far below usize's range"
    )]
    let columns = (room / (CARD_WIDTH + SPACING)).floor().max(1.0) as usize;
    columns
}

/// The narrowest window: the sidebar and two columns.
pub fn min_width() -> f32 {
    sidebar::WIDTH + 2.0 * GRID_PADDING + SCROLLBAR + 2.0 * CARD_WIDTH + SPACING
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
            Some(_) if visible.is_empty() && self.source.is_some() => {
                centered(text(t!("library-none-here")).color(colors().dim))
            }
            Some(_) if visible.is_empty() => {
                centered(text(t!("library-all-hidden")).color(colors().dim))
            }
            Some(_) => self.grid(&visible),
        };
        column![self.library_header(), self.toolbar(), body]
            .spacing(4)
            .into()
    }

    fn library_header(&self) -> Element<'_, Message> {
        let games: Vec<&Game> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .collect();
        let shown = games.iter().filter(|game| !game.hidden).count();
        let hidden = games.len() - shown;
        let stores = SOURCES
            .iter()
            .filter(|source| games.iter().any(|game| game.source == **source))
            .count();
        let mut subtitle = t!(
            "library-subtitle",
            games = t!("count-games", count = shown),
            sources = t!("count-sources", count = stores)
        );
        if hidden > 0 {
            subtitle = t!(
                "library-subtitle-hidden",
                subtitle = subtitle,
                hidden = hidden
            );
        }

        let refresh = button(
            text(if self.scanning {
                t!("library-scanning")
            } else {
                t!("library-refresh")
            })
            .size(14),
        )
        .padding([8, 14])
        .style(style::secondary)
        .on_press_maybe((!self.scanning).then_some(Message::Refresh));
        row![
            column![
                text(t!("page-library")).size(28).font(BOLD),
                text(subtitle).size(13).color(colors().dim),
            ]
            .spacing(2),
            space::horizontal(),
            text_input(&t!("library-search"), &self.search)
                .id(search_id())
                .on_input(Message::Search)
                .on_submit(Message::Activate)
                .padding([8, 12])
                .size(14)
                .width(280)
                .style(style::input),
            button(text(t!("library-add-game")).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::Local(super::local::LocalMessage::Add)),
            refresh,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 20.0,
            right: GRID_PADDING,
            bottom: 8.0,
            left: GRID_PADDING,
        })
        .into()
    }

    /// Store chips, then the sort and the hidden games.
    fn toolbar(&self) -> Element<'_, Message> {
        let games: Vec<&Game> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| self.show_hidden || !game.hidden)
            .collect();
        let hidden = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| game.hidden)
            .count();

        let mut chips: Vec<Element<'_, Message>> = vec![chip(
            t!("library-all"),
            Some(games.len()),
            self.source.is_none(),
            Message::SetSource(None),
        )];
        for source in SOURCES {
            let count = games.iter().filter(|game| game.source == source).count();
            if count > 0 {
                chips.push(chip(
                    format::source(source),
                    Some(count),
                    self.source == Some(source),
                    Message::SetSource(Some(source)),
                ));
            }
        }
        chips.push(space::horizontal().width(16).into());
        chips.push(
            container(text(t!("library-sort")).size(13).color(colors().dim))
                .padding([6, 0])
                .into(),
        );
        for sort in [Sort::Recent, Sort::MostPlayed, Sort::Title] {
            chips.push(chip(
                format::sort(sort),
                None,
                self.sort == sort,
                Message::SetSort(sort),
            ));
        }
        if hidden > 0 || self.show_hidden {
            chips.push(space::horizontal().width(16).into());
            chips.push(chip(
                t!("library-show-hidden"),
                Some(hidden),
                self.show_hidden,
                Message::ShowHidden,
            ));
        }
        row(chips)
            .spacing(8)
            .padding([4.0, GRID_PADDING])
            .wrap()
            .vertical_spacing(8)
            .into()
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
                    game.pinned.then(|| (t!("badge-pinned"), colors().accent))
                },
                line,
                line_color,
                selected: index == selected,
                on_press: Message::Clicked(index),
            })
        });
        cards_grid(cards, self.columns)
    }
}

/// Cards in rows of `columns`, in the page's scrollable.
pub fn cards_grid<'a>(
    cards: impl Iterator<Item = Element<'a, Message>>,
    columns: usize,
) -> Element<'a, Message> {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut current = Vec::with_capacity(columns);
    for card in cards {
        current.push(card);
        if current.len() == columns {
            rows.push(row(std::mem::take(&mut current)).spacing(SPACING).into());
        }
    }
    if !current.is_empty() {
        rows.push(row(current).spacing(SPACING).into());
    }
    // A fixed width, centred, so a short last row stays in its columns.
    #[expect(clippy::cast_precision_loss, reason = "column counts are small")]
    let width = columns as f32 * (CARD_WIDTH + SPACING) - SPACING;
    let grid = column(rows).spacing(SPACING).width(width);
    scrollable(container(grid).center_x(Length::Fill).padding(GRID_PADDING))
        .id(grid_id())
        .on_scroll(Message::Scrolled)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn chip<'a>(
    label: String,
    count: Option<usize>,
    chosen: bool,
    message: Message,
) -> Element<'a, Message> {
    let mut content = row![text(label).size(13)].spacing(6);
    if let Some(count) = count {
        content = content.push(text(count.to_string()).size(12).color(if chosen {
            colors().accent
        } else {
            colors().dim
        }));
    }
    button(content.align_y(Alignment::Center))
        .padding([5, 12])
        .style(style::chip(chosen))
        .on_press(message)
        .into()
}

/// The line under a card's title: how much it was played.
fn played_line(game: &Game, now: u64) -> String {
    match (game.played.minutes, game.played.last) {
        (0, None) => t!("played-never"),
        (0, Some(last)) => t!("played-when", when = format::ago(last, now)),
        (minutes, _) => t!("played-time", time = format::play_time(minutes)),
    }
}

/// What a card shows.
pub struct Card<'a> {
    pub title: &'a str,
    pub source: Source,
    pub cover: Option<&'a Path>,
    pub faded: bool,
    /// A label in the cover's top right corner.
    pub badge: Option<(String, Color)>,
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
        line,
        line_color,
        selected,
        on_press,
    } = card;
    let cover: Element<'_, Message> = match cover {
        Some(path) => image(image::Handle::from_path(path))
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .content_fit(ContentFit::Cover)
            .border_radius(8.0)
            .opacity(if faded { 0.35_f32 } else { 1.0_f32 })
            .into(),
        None => container(text(title).size(18).center())
            .padding(12)
            .center(Length::Fill)
            .width(COVER_WIDTH)
            .height(COVER_HEIGHT)
            .style(|_| container::Style {
                background: Some(Background::Color(colors().surface)),
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    };
    let mut badges = row![style::badge(
        format::source(source),
        style::source_color(source)
    )]
    .width(COVER_WIDTH);
    if let Some((label, color)) = badge {
        badges = badges
            .push(space::horizontal())
            .push(style::badge(label, color));
    }
    let art = stack![cover, container(badges).padding(8).width(COVER_WIDTH)];

    let caption = column![
        text(title).size(14).wrapping(text::Wrapping::None),
        text(line).size(12).color(line_color),
    ]
    .spacing(2)
    .height(CAPTION_HEIGHT)
    .clip(true);

    button(column![art, caption].spacing(4))
        .padding(CARD_PADDING)
        .width(CARD_WIDTH)
        .height(CARD_HEIGHT)
        .on_press(on_press)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered | button::Status::Pressed => colors().surface,
                _ => Color::TRANSPARENT,
            })),
            text_color: colors().text,
            border: Border {
                color: if selected {
                    colors().accent
                } else {
                    Color::TRANSPARENT
                },
                width: 2.0,
                radius: 12.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_whole_cards_beside_the_sidebar() {
        assert_eq!(columns_for(100.0), 1);
        let five =
            sidebar::WIDTH + 2.0 * GRID_PADDING + SCROLLBAR + 5.0 * CARD_WIDTH + 4.0 * SPACING;
        assert_eq!(columns_for(five), 5);
        assert_eq!(columns_for(five - 1.0), 4);
        assert_eq!(columns_for(min_width()), 2);
    }
}
