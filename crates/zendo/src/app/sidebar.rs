//! The pages, and a summary of the library under them.

use std::collections::HashSet;

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Length};

use super::style::{self, BOLD, colors};
use super::{Message, Page, Zendo, format};
use crate::i18n::t;
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

        let entry = |label: String, detail: String, page: Page, current: bool| {
            button(
                row![
                    text(label).size(14),
                    space::horizontal(),
                    text(detail).size(12).color(colors().dim)
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 12])
            .style(style::nav(current))
            .on_press(Message::Navigate(page))
        };
        let heading = |label: String| {
            container(text(label).size(11).font(BOLD).color(colors().dim)).padding([12, 12])
        };

        let summary = summary(&games);

        let brand = row![
            text("Zendo").size(22).font(BOLD),
            text("•").size(22).color(colors().accent)
        ]
        .spacing(2);

        container(
            column![
                container(brand).padding([8, 12]),
                heading(t!("sidebar-browse")),
                entry(
                    t!("page-library"),
                    games.len().to_string(),
                    Page::Library,
                    in_library
                ),
                entry(
                    t!("page-store"),
                    match self.owned_count() {
                        0 => String::new(),
                        n => n.to_string(),
                    },
                    Page::Store,
                    matches!(self.page, Page::Store | Page::Owned(_))
                ),
                entry(
                    t!("page-downloads"),
                    match self.downloads.queue.len() {
                        0 => String::new(),
                        n => n.to_string(),
                    },
                    Page::Downloads,
                    self.page == Page::Downloads
                ),
                heading(t!("sidebar-system")),
                entry(
                    t!("page-accounts"),
                    match self.accounts.signed_in() {
                        0 => String::new(),
                        n => t!("sidebar-signed-in", count = n, stores = 3),
                    },
                    Page::Accounts,
                    self.page == Page::Accounts
                ),
                entry(
                    t!("page-settings"),
                    String::new(),
                    Page::Settings,
                    self.page == Page::Settings
                ),
                entry(
                    t!("page-about"),
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
            text(t!("sidebar-your-library"))
                .size(11)
                .font(BOLD)
                .color(colors().dim),
            text(t!("count-games", count = games.len()))
                .size(20)
                .font(BOLD),
            text(t!(
                "sidebar-summary",
                sources = t!("count-sources", count = stores),
                time = format::play_time(minutes)
            ))
            .size(12)
            .color(colors().dim),
        ]
        .spacing(4),
    )
    .padding(14)
    .width(Length::Fill)
    .style(style::card)
    .into()
}
