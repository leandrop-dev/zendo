//! A game's page: its wide art with the title over it, the actions, and
//! what its store records about the install.

use std::f32::consts::{FRAC_PI_2, PI};
use std::path::Path;

use iced::gradient::Linear;
use iced::widget::{button, column, container, image, row, scrollable, space, stack, text};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Font, Length, Radians};
use zendo_core::owned::Owned;
use zendo_core::{Game, Size, Source};

use super::downloads::UpdateState;
use super::store::InfoState;
use super::style::{self, ACCENT, BACKGROUND, BOLD, DIM, LINE, SURFACE};
use super::{Message, Page, Zendo, format, game_scroll_id, now};

const HERO_HEIGHT: f32 = 340.0;
const COVER_WIDTH: f32 = 140.0;
const COVER_HEIGHT: f32 = 210.0;
const RADIUS: f32 = 14.0;
const LABEL_WIDTH: f32 = 170.0;

impl Zendo {
    pub(super) fn game_page<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let trail = row![
            button(text("← Library").size(14))
                .padding(0)
                .style(style::link)
                .on_press(Message::Navigate(Page::Library)),
            text("/").size(14).color(DIM),
            text(&game.title).size(14).color(DIM),
        ]
        .spacing(8);

        let status = self
            .status
            .clone()
            .unwrap_or_else(|| "Ready when you are".to_owned());
        let actions = row![
            self.play_buttons(game),
            button(text(if game.pinned { "Unpin" } else { "Pin" }).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::TogglePinned),
            button(text(if game.hidden { "Unhide" } else { "Hide" }).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::ToggleHidden),
            space::horizontal(),
            text(status).size(14).color(DIM),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let owned = self.store.owned(&game.id);
        let update = matches!(
            self.downloads.updates.get(&game.id),
            Some(UpdateState::Available(_))
        );
        let mut page = column![trail, game_hero(game, update), actions]
            .spacing(20)
            .padding(24);
        if game.installed_by_zendo() {
            page = page.push(self.store_actions(game));
            page = page.push(self.launch_card(game));
        }
        let facts = store_facts(
            owned,
            self.store.info.get(&game.id),
            game.details.size.is_some(),
        );
        page = page.push(details(game, facts));
        if let Some(summary) = owned.and_then(|owned| owned.summary.as_deref()) {
            page = page.push(about_card(summary));
        }
        scrollable(page)
            .id(game_scroll_id())
            .height(Length::Fill)
            .into()
    }
}

/// What the wide art at the top of a game's page shows.
pub struct Hero<'a> {
    pub title: &'a str,
    pub source: Source,
    pub art: Option<&'a Path>,
    pub cover: Option<&'a Path>,
    pub badges: Vec<(String, Color)>,
    pub developer: Option<&'a str>,
    pub facts: Vec<String>,
}

fn game_hero(game: &Game, update: bool) -> Element<'_, Message> {
    let mut badges = Vec::new();
    if update {
        badges.push(("UPDATE AVAILABLE".to_owned(), ACCENT));
    }
    if let Some(platform) = &game.details.platform {
        badges.push((platform_label(platform), DIM));
    }
    if game.pinned {
        badges.push(("PINNED".to_owned(), ACCENT));
    }
    if game.hidden {
        badges.push(("HIDDEN".to_owned(), DIM));
    }
    hero(Hero {
        title: &game.title,
        source: game.source,
        art: game.hero.as_deref(),
        cover: game.cover.as_deref(),
        badges,
        developer: game.details.developer.as_deref(),
        facts: facts(game),
    })
}

pub fn hero(hero: Hero<'_>) -> Element<'_, Message> {
    let art: Element<'_, Message> = if let Some(path) = hero.art {
        image(image::Handle::from_path(path))
            .width(Length::Fill)
            .height(HERO_HEIGHT)
            .content_fit(ContentFit::Cover)
            .border_radius(RADIUS)
            .into()
    } else {
        let tint = style::source_color(hero.source);
        shade(
            Linear::new(Radians(FRAC_PI_2))
                .add_stop(0.0, SURFACE)
                .add_stop(1.0, Color { a: 0.25, ..tint }),
        )
    };
    // Darkened toward the bottom and the left, where the title sits.
    let down = shade(
        Linear::new(Radians(PI))
            .add_stop(0.0, Color::TRANSPARENT)
            .add_stop(
                0.45,
                Color {
                    a: 0.2,
                    ..BACKGROUND
                },
            )
            .add_stop(
                1.0,
                Color {
                    a: 0.95,
                    ..BACKGROUND
                },
            ),
    );
    let across = shade(
        Linear::new(Radians(FRAC_PI_2))
            .add_stop(
                0.0,
                Color {
                    a: 0.7,
                    ..BACKGROUND
                },
            )
            .add_stop(0.6, Color::TRANSPARENT),
    );

    let mut badges = row![style::badge(
        hero.source.label(),
        style::source_color(hero.source)
    )]
    .spacing(6);
    for (label, color) in hero.badges {
        badges = badges.push(style::badge(label, color));
    }
    let mut info = column![badges, text(hero.title).size(38).font(BOLD)].spacing(8);
    if let Some(developer) = hero.developer {
        info = info.push(text(developer).size(15).color(DIM));
    }
    let facts: Vec<Element<'_, Message>> = hero.facts.into_iter().map(fact_chip).collect();
    if !facts.is_empty() {
        info = info.push(row(facts).spacing(8));
    }

    let mut content = row![info.width(Length::Fill)]
        .spacing(24)
        .align_y(Alignment::End);
    if let Some(cover) = hero.cover {
        content = content.push(
            image(image::Handle::from_path(cover))
                .width(COVER_WIDTH)
                .height(COVER_HEIGHT)
                .content_fit(ContentFit::Cover)
                .border_radius(10.0),
        );
    }
    let content = container(content)
        .padding(28)
        .width(Length::Fill)
        .height(HERO_HEIGHT)
        .align_y(Alignment::End);

    stack![art, down, across, content].into()
}

/// A rounded panel the hero's size filled with a gradient.
fn shade<'a>(gradient: Linear) -> Element<'a, Message> {
    container(space::horizontal())
        .width(Length::Fill)
        .height(HERO_HEIGHT)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(gradient.into())),
            border: Border {
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The few numbers worth seeing first.
fn facts(game: &Game) -> Vec<String> {
    let mut facts = Vec::new();
    if game.played.minutes > 0 {
        facts.push(format!("{} played", format::play_time(game.played.minutes)));
    }
    if let Some(last) = game.played.last {
        facts.push(format!("Last played {}", format::ago(last, now())));
    }
    if let Some(size) = &game.details.size {
        facts.push(size.to_string());
    }
    facts
}

fn fact_chip<'a>(fact: String) -> Element<'a, Message> {
    container(text(fact).size(13))
        .padding([4, 10])
        .style(|_| container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..SURFACE })),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 14.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Stores write platforms as `Windows`, `windows`, `linux`; shown alike.
fn platform_label(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "windows" => "Windows".to_owned(),
        "linux" => "Linux".to_owned(),
        "mac" | "osx" => "macOS".to_owned(),
        _ => platform.to_owned(),
    }
}

/// What the store says about a game: sizes and the latest version asked of
/// its engine, then what its list records. `installed` leaves out the size
/// on disk, which the install's own record gives.
pub fn store_facts(
    owned: Option<&Owned>,
    info: Option<&InfoState>,
    installed: bool,
) -> Vec<(&'static str, String)> {
    let mut facts = Vec::new();
    match info {
        Some(InfoState::Asking) => facts.push(("Download size", "Asking the store…".to_owned())),
        Some(InfoState::Got(info)) => {
            if let Some(bytes) = info.download_size {
                facts.push(("Download size", Size::Bytes(bytes).to_string()));
            }
            if let Some(bytes) = info.install_size.filter(|_| !installed) {
                facts.push(("Size on disk", Size::Bytes(bytes).to_string()));
            }
            if let Some(version) = &info.version {
                facts.push(("Latest version", version.clone()));
            }
            if let Some(day) = &info.updated {
                facts.push(("Latest build", day.clone()));
            }
        }
        Some(InfoState::Failed(err)) => {
            facts.push(("Store details", format!("The store could not say: {err}")));
        }
        None => {}
    }
    if let Some(owned) = owned {
        if let Some(publisher) = &owned.publisher {
            facts.push(("Publisher", publisher.clone()));
        }
        if let Some(day) = &owned.released {
            facts.push(("Released", day.clone()));
        }
        if !owned.genres.is_empty() {
            facts.push(("Genres", owned.genres.join(", ")));
        }
    }
    facts
}

/// What the store says the game is about.
pub fn about_card(summary: &str) -> Element<'_, Message> {
    container(
        column![
            text("About").size(18).font(BOLD),
            text(summary).size(14).line_height(1.5),
        ]
        .spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn details<'a>(game: &'a Game, facts: Vec<(&'static str, String)>) -> Element<'a, Message> {
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    let mut add = |label: &'static str, value: Element<'static, Message>| {
        rows.push(
            row![
                text(label).size(14).color(DIM).width(LABEL_WIDTH),
                container(value).width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .into(),
        );
    };
    let plain = |value: String| -> Element<'static, Message> { text(value).size(14).into() };
    let code = |value: String| -> Element<'static, Message> {
        text(value).size(13).font(Font::MONOSPACE).into()
    };

    add("Store", plain(game.source.label().to_owned()));
    add("Started by", plain(game.launcher()));
    if let Some(platform) = &game.details.platform {
        add("Platform", plain(platform_label(platform)));
    }
    if let Some(version) = &game.details.version {
        add("Version", plain(version.clone()));
    }
    if let Some(size) = &game.details.size {
        add("Install size", plain(size.to_string()));
    }
    for (label, value) in facts {
        add(label, plain(value));
    }
    if let Some(path) = &game.details.install_path {
        let shown = path.display().to_string();
        let open = button(text("Open folder").size(13))
            .padding([4, 10])
            .style(style::secondary)
            .on_press_maybe(
                path.is_dir()
                    .then(|| Message::Open(path.to_string_lossy().into_owned())),
            );
        add(
            "Install path",
            row![text(shown).size(13).font(Font::MONOSPACE), open]
                .spacing(12)
                .align_y(Alignment::Center)
                .into(),
        );
    }
    add(
        "Last played",
        plain(
            game.played
                .last
                .map_or_else(|| "Never".to_owned(), |last| format::ago(last, now())),
        ),
    );
    if game.played.minutes > 0 {
        add("Play time", plain(format::play_time(game.played.minutes)));
    }
    add(
        "Started from Zendo",
        plain(format::count(
            game.played.launches as usize,
            "time",
            "times",
        )),
    );
    add("Launch command", code(game.command_line()));
    add("Zendo id", code(game.id.clone()));

    container(
        column![
            text("Game details").size(18).font(BOLD),
            column(rows).spacing(12)
        ]
        .spacing(16),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}
