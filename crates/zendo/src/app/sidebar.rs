//! The pages, and a summary of the library under them.

use std::collections::HashSet;

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Length};

use super::style::{self, ACCENT, BOLD, DIM};
use super::{Message, Page, Zendo, format};
use zendo_core::Game;

pub const WIDTH: f32 = 224.0;

impl Zendo {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let games: Vec<_> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| !game.hidden)
            .collect();
        let in_library = matches!(self.page, Page::Library | Page::Game(_));

        let entry = |label: &'static str, detail: String, page: Page, current: bool| {
            button(
                row![
                    text(label).size(14),
                    space::horizontal(),
                    text(detail).size(12).color(DIM)
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 12])
            .style(style::nav(current))
            .on_press(Message::Navigate(page))
        };
        let heading = |label: &'static str| {
            container(text(label).size(11).font(BOLD).color(DIM)).padding([12, 12])
        };

        let summary = summary(&games);

        let brand = row![
            text("Zendo").size(22).font(BOLD),
            text("•").size(22).color(ACCENT)
        ]
        .spacing(2);

        container(
            column![
                container(brand).padding([8, 12]),
                heading("BROWSE"),
                entry(
                    "Library",
                    games.len().to_string(),
                    Page::Library,
                    in_library
                ),
                entry(
                    "Store",
                    match self.owned_count() {
                        0 => String::new(),
                        n => n.to_string(),
                    },
                    Page::Store,
                    matches!(self.page, Page::Store | Page::Owned(_))
                ),
                entry(
                    "Downloads",
                    match self.downloads.queue.len() {
                        0 => String::new(),
                        n => n.to_string(),
                    },
                    Page::Downloads,
                    self.page == Page::Downloads
                ),
                heading("SYSTEM"),
                entry(
                    "Accounts",
                    match self.accounts.signed_in() {
                        0 => String::new(),
                        n => format!("{n} of 3"),
                    },
                    Page::Accounts,
                    self.page == Page::Accounts
                ),
                entry(
                    "Settings",
                    String::new(),
                    Page::Settings,
                    self.page == Page::Settings
                ),
                entry(
                    "About",
                    String::new(),
                    Page::About,
                    self.page == Page::About
                ),
                space::vertical(),
                summary,
            ]
            .spacing(2)
            .padding(16)
            .width(WIDTH)
            .height(Length::Fill),
        )
        .style(style::sidebar)
        .into()
    }
}

/// Games, sources and play time, at the foot of the sidebar.
fn summary<'a>(games: &[&Game]) -> Element<'a, Message> {
    let minutes: u64 = games.iter().map(|game| game.played.minutes).sum();
    let stores = games
        .iter()
        .map(|game| game.source)
        .collect::<HashSet<_>>()
        .len();
    container(
        column![
            text("YOUR LIBRARY").size(11).font(BOLD).color(DIM),
            text(format::count(games.len(), "game", "games"))
                .size(20)
                .font(BOLD),
            text(format!(
                "{} · {}",
                format::count(stores, "source", "sources"),
                format::play_time(minutes)
            ))
            .size(12)
            .color(DIM),
        ]
        .spacing(4),
    )
    .padding(14)
    .width(Length::Fill)
    .style(style::card)
    .into()
}
