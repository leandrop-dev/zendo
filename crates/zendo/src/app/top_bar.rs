//! The top bar: the name, the tabs (Library, Store, Stats), on Library the
//! view chips, the search, and the Downloads and Settings icon buttons; under
//! Settings, the row that switches between Settings, Accounts and About.

use iced::widget::{button, container, row, space, text, text_input, tooltip};
use iced::{Alignment, Background, Element, Length};

use super::game_mode::GameModeMessage;
use super::icons::{self, Icon};
use super::store::StoreMessage;
use super::style::{self, BOLD, SPACE_L, SPACE_M, SPACE_S, SPACE_XL, SPACE_XS, colors};
use super::{Message, Page, Zendo, search_id, store_search_id};
use crate::i18n::t;
use zendo_core::owned::Shop;

/// The search box's width.
const SEARCH_WIDTH: f32 = 220.0;

/// An icon button's icon, and the button's height (the width is the
/// height when there is no count).
const ICON_SIZE: f32 = 18.0;
const ICON_BUTTON: f32 = 34.0;

/// The narrowest window the bar fits in: the name, three tabs, the view
/// chips, the search and the two icon buttons.
pub const MIN_WIDTH: f32 = 1100.0;

impl Zendo {
    pub(super) fn top_bar(&self) -> Element<'_, Message> {
        let games = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| !game.hidden)
            .count();

        let tab = |label: String, count: String, page: Page, current: bool| {
            let mut content = row![text(label.to_uppercase()).size(13).font(BOLD)]
                .spacing(SPACE_S)
                .align_y(Alignment::Center);
            if !count.is_empty() {
                content = content.push(text(count).size(12).color(colors().dim));
            }
            button(content)
                .padding([SPACE_S, SPACE_M])
                .style(style::tab(current))
                .on_press(Message::Navigate(page))
        };

        let brand = row![
            text("ZENDO").size(20).font(BOLD),
            text("•").size(20).color(colors().accent)
        ]
        .spacing(2);

        let bar = row![
            container(brand).padding([0.0, SPACE_L]),
            tab(
                t!("page-library"),
                games.to_string(),
                Page::Library,
                matches!(self.page, Page::Library | Page::Game(_)),
            ),
            tab(
                t!("page-store"),
                count(self.owned_count()),
                Page::Store,
                matches!(self.page, Page::Store | Page::Owned(_)),
            ),
            tab(
                t!("page-stats"),
                String::new(),
                Page::Stats,
                self.page == Page::Stats,
            ),
            space::horizontal(),
            self.view_chips(),
            self.search_box(),
            self.game_mode_chip(),
            icon_button(
                Icon::Download,
                &t!("page-downloads"),
                count(self.downloads.queue.len() + self.steam_installs.count()),
                self.page == Page::Downloads,
                Message::Navigate(Page::Downloads),
            ),
            icon_button(
                Icon::Settings,
                &t!("page-settings"),
                String::new(),
                self.in_settings(),
                Message::Navigate(Page::Settings),
            ),
        ]
        .spacing(SPACE_S)
        .align_y(Alignment::Center)
        .padding([SPACE_M, SPACE_XL]);

        column_of([
            container(bar)
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(colors().sidebar)),
                    ..container::Style::default()
                })
                .into(),
            rule(),
        ])
    }

    /// While Game Mode is on, the way back to its screen from any page;
    /// nothing otherwise.
    fn game_mode_chip(&self) -> Element<'_, Message> {
        if self.game_mode.is_none() {
            return space::horizontal().width(0).into();
        }
        button(style::button_text(t!("gm-screen")))
            .padding([SPACE_S, SPACE_M])
            .style(style::chip(false))
            .on_press(Message::GameMode(GameModeMessage::Show))
            .into()
    }

    /// Whether the page is one of the three the Settings button holds.
    pub(super) fn in_settings(&self) -> bool {
        matches!(self.page, Page::Settings | Page::Accounts | Page::About)
    }

    /// The search: the library's on Library, the Store's on Store, and
    /// nothing elsewhere, where Enter means something else.
    fn search_box(&self) -> Element<'_, Message> {
        match self.page {
            Page::Library => text_input(&t!("library-search"), &self.search)
                .id(search_id())
                .on_input(Message::Search)
                .on_submit(Message::Activate)
                .padding([SPACE_S, SPACE_M])
                .size(14)
                .width(SEARCH_WIDTH)
                .style(style::input)
                .into(),
            Page::Store => text_input(&t!("store-search"), &self.store.search)
                .id(store_search_id())
                .on_input(|search| Message::Store(StoreMessage::Search(search)))
                .on_submit(Message::Activate)
                .padding([SPACE_S, SPACE_M])
                .size(14)
                .width(SEARCH_WIDTH)
                .style(style::input)
                .into(),
            _ => space::horizontal().width(0).into(),
        }
    }

    /// Under Settings: Settings, Accounts and About.
    pub(super) fn settings_tabs(&self) -> Element<'_, Message> {
        let tab = |label: String, detail: String, page: Page| {
            let current = self.page == page;
            let mut content = row![text(label.to_uppercase()).size(12).font(BOLD)]
                .spacing(SPACE_S)
                .align_y(Alignment::Center);
            if !detail.is_empty() {
                content = content.push(text(detail).size(12).color(colors().dim));
            }
            button(content)
                .padding([SPACE_S, SPACE_M])
                .style(style::chip(current))
                .on_press(Message::Navigate(page))
        };
        row![
            tab(t!("page-settings"), String::new(), Page::Settings),
            tab(
                t!("page-accounts"),
                // The stores signed in, and Steam once its key is set.
                match self.connected().len() {
                    0 => String::new(),
                    n => t!("top-bar-signed-in", count = n, stores = Shop::ALL.len()),
                },
                Page::Accounts,
            ),
            tab(t!("page-about"), String::new(), Page::About),
        ]
        .spacing(SPACE_S)
        .padding([SPACE_M, SPACE_XL])
        .into()
    }
}

/// A button in the top bar that is an icon, with a count beside it when
/// there is one. Its name is in the tooltip.
fn icon_button(
    icon: Icon,
    label: &str,
    count: String,
    current: bool,
    message: Message,
) -> Element<'static, Message> {
    let mut content = row![icons::icon(icon, ICON_SIZE, style::icon_color(current))]
        .spacing(SPACE_S)
        .align_y(Alignment::Center);
    if !count.is_empty() {
        content = content.push(text(count).size(12).color(colors().dim));
    }
    let button = button(container(content).center_y(Length::Fill))
        .padding([0.0, SPACE_S])
        .height(ICON_BUTTON)
        .style(style::icon_button(current))
        .on_press(message);
    tooltip(
        button,
        container(text(label.to_uppercase()).size(12))
            .padding([SPACE_XS, SPACE_S])
            .style(style::card),
        tooltip::Position::Bottom,
    )
    .gap(SPACE_XS)
    .into()
}

/// A count for a tab, nothing when there is none.
fn count(n: usize) -> String {
    match n {
        0 => String::new(),
        n => n.to_string(),
    }
}

/// A 1px line across the window.
fn rule<'a>() -> Element<'a, Message> {
    container(space::vertical().height(1))
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(colors().line)),
            ..container::Style::default()
        })
        .into()
}

/// The elements one under the other, full width.
fn column_of<'a>(items: impl IntoIterator<Item = Element<'a, Message>>) -> Element<'a, Message> {
    iced::widget::Column::with_children(items)
        .width(Length::Fill)
        .into()
}
