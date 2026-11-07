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
use crate::i18n::t;

const HERO_HEIGHT: f32 = 340.0;
const COVER_WIDTH: f32 = 140.0;
const COVER_HEIGHT: f32 = 210.0;
const RADIUS: f32 = 14.0;
const LABEL_WIDTH: f32 = 170.0;

impl Zendo {
    pub(super) fn game_page<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let trail = row![
            button(text(t!("game-back")).size(14))
                .padding(0)
                .style(style::link)
                .on_press(Message::Navigate(Page::Library)),
            text("/").size(14).color(DIM),
            text(&game.title).size(14).color(DIM),
        ]
        .spacing(8);

        let status = self.status.clone().unwrap_or_else(|| t!("game-ready"));
        let actions = row![
            self.play_buttons(game),
            launch_options_button(game),
            self.local_buttons(game),
            button(
                text(if game.pinned {
                    t!("game-unpin")
                } else {
                    t!("game-pin")
                })
                .size(14)
            )
            .padding([10, 18])
            .style(style::secondary)
            .on_press(Message::TogglePinned),
            button(
                text(if game.hidden {
                    t!("game-unhide")
                } else {
                    t!("game-hide")
                })
                .size(14)
            )
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
        let mut page = column![
            trail,
            game_hero(game, update, self.rating_pill(&game.id)),
            actions
        ]
        .spacing(20)
        .padding(24);
        if game.installed_by_zendo() {
            page = page.push(self.store_actions(game));
            page = page.push(self.launch_card(game));
        }
        if game.source == Source::Local {
            page = page.push(self.launch_card(game));
        }
        if let Some(mods) = self.mods_card(game) {
            page = page.push(mods);
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
    /// The `ProtonDB` pill, beside the badges.
    pub rating: Option<Element<'a, Message>>,
    /// The game whose art "Change art" picks.
    pub id: &'a str,
}

fn game_hero<'a>(
    game: &'a Game,
    update: bool,
    rating: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut badges = Vec::new();
    if update {
        badges.push((t!("badge-update"), ACCENT));
    }
    if let Some(platform) = &game.details.platform {
        badges.push((platform_label(platform), DIM));
    }
    if game.pinned {
        badges.push((t!("badge-pinned"), ACCENT));
    }
    if game.hidden {
        badges.push((t!("badge-hidden"), DIM));
    }
    hero(Hero {
        title: &game.title,
        source: game.source,
        art: game.hero.as_deref(),
        cover: game.cover.as_deref(),
        badges,
        developer: game.details.developer.as_deref(),
        facts: facts(game),
        rating,
        id: &game.id,
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
        format::source(hero.source),
        style::source_color(hero.source)
    )]
    .spacing(6);
    for (label, color) in hero.badges {
        badges = badges.push(style::badge(label, color));
    }
    if let Some(rating) = hero.rating {
        badges = badges.push(rating);
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
    let change = container(
        button(text(t!("game-change-art")).size(13))
            .padding([6, 12])
            .style(style::secondary)
            .on_press(Message::Art(super::artwork::ArtMessage::Open(
                hero.id.to_owned(),
            ))),
    )
    .padding(16)
    .width(Length::Fill)
    .align_x(Alignment::End);

    stack![art, down, across, content, change].into()
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
        facts.push(t!(
            "played-time",
            time = format::play_time(game.played.minutes)
        ));
    }
    if let Some(last) = game.played.last {
        facts.push(t!("game-last-played", when = format::ago(last, now())));
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
) -> Vec<(String, String)> {
    let mut facts = Vec::new();
    match info {
        Some(InfoState::Asking) => facts.push((t!("fact-download-size"), t!("fact-asking"))),
        Some(InfoState::Got(info)) => {
            if let Some(bytes) = info.download_size {
                facts.push((t!("fact-download-size"), Size::Bytes(bytes).to_string()));
            }
            if let Some(bytes) = info.install_size.filter(|_| !installed) {
                facts.push((t!("fact-size-on-disk"), Size::Bytes(bytes).to_string()));
            }
            if let Some(version) = &info.version {
                facts.push((t!("fact-latest-version"), version.clone()));
            }
            if let Some(day) = &info.updated {
                facts.push((t!("fact-latest-build"), day.clone()));
            }
        }
        Some(InfoState::Failed(err)) => {
            facts.push((
                t!("fact-store-details"),
                t!("fact-store-failed", error = err.as_str()),
            ));
        }
        None => {}
    }
    if let Some(owned) = owned {
        if let Some(publisher) = &owned.publisher {
            facts.push((t!("fact-publisher"), publisher.clone()));
        }
        if let Some(day) = &owned.released {
            facts.push((t!("fact-released"), day.clone()));
        }
        if !owned.genres.is_empty() {
            facts.push((t!("fact-genres"), owned.genres.join(", ")));
        }
    }
    facts
}

/// What the store says the game is about.
pub fn about_card(summary: &str) -> Element<'_, Message> {
    container(
        column![
            text(t!("game-about")).size(18).font(BOLD),
            text(summary).size(14).line_height(1.5),
        ]
        .spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn details(game: &Game, facts: Vec<(String, String)>) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    let mut add = |label: String, value: Element<'static, Message>| {
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

    add(t!("fact-store"), plain(format::source(game.source)));
    add(t!("fact-started-by"), plain(game.launcher()));
    if let Some(platform) = &game.details.platform {
        add(t!("fact-platform"), plain(platform_label(platform)));
    }
    if let Some(version) = &game.details.version {
        add(t!("fact-version"), plain(version.clone()));
    }
    if let Some(size) = &game.details.size {
        add(t!("fact-install-size"), plain(size.to_string()));
    }
    for (label, value) in facts {
        add(label, plain(value));
    }
    if let Some(path) = &game.details.install_path {
        let shown = path.display().to_string();
        let open = button(text(t!("files-open-folder")).size(13))
            .padding([4, 10])
            .style(style::secondary)
            .on_press_maybe(
                path.is_dir()
                    .then(|| Message::Open(path.to_string_lossy().into_owned())),
            );
        add(
            t!("fact-install-path"),
            row![text(shown).size(13).font(Font::MONOSPACE), open]
                .spacing(12)
                .align_y(Alignment::Center)
                .into(),
        );
    }
    add(
        t!("fact-last-played"),
        plain(
            game.played
                .last
                .map_or_else(|| t!("fact-never"), |last| format::ago(last, now())),
        ),
    );
    if game.played.minutes > 0 {
        add(
            t!("fact-play-time"),
            plain(format::play_time(game.played.minutes)),
        );
    }
    add(
        t!("fact-started-from-zendo"),
        plain(t!("count-times", count = game.played.launches)),
    );
    add(t!("fact-launch-command"), code(game.command_line()));
    add(t!("fact-zendo-id"), code(game.id.clone()));

    container(
        column![
            text(t!("game-details")).size(18).font(BOLD),
            column(rows).spacing(12)
        ]
        .spacing(16),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

/// Opens the launch options sheet, for the games Zendo starts itself.
fn launch_options_button(game: &Game) -> Element<'_, Message> {
    if !super::launch_options::applies(game) {
        return space::horizontal().width(0).into();
    }
    button(text(t!("game-launch-options")).size(14))
        .padding([10, 18])
        .style(style::secondary)
        .on_press(Message::Sheet(super::launch_options::SheetMessage::Open))
        .into()
}
