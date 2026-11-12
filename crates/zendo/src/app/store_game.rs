//! A store game's page, laid out as an installed game's: the hero, the
//! actions, then About and the DLC owned beside the details, in two columns
//! on a wide window.

use std::f32::consts::{FRAC_PI_2, PI};
use std::path::Path;

use iced::gradient::Linear;
use iced::widget::{button, column, container, image, row, scrollable, space, stack, text};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Font, Length, Radians};
use zendo_core::owned::{Owned, PlayedOn};
use zendo_core::{Size, Source};

use super::format;
use super::game::{GameViewMessage, TWO_COLUMNS, about_card, card, label_row, plain, store_facts};
use super::store::InfoState;
use super::style::{self, BOLD, colors};
use super::{Message, Page, Zendo, game_scroll_id, now};
use crate::i18n::t;

impl Zendo {
    pub(super) fn owned_page<'a>(&'a self, game: &'a Owned) -> Element<'a, Message> {
        let installed = self.is_installed(&game.id);
        let trail = row![
            button(text(t!("store-back")).size(14))
                .padding(0)
                .style(style::link)
                .on_press(Message::Navigate(Page::Store)),
            text("/").size(14).color(colors().dim),
            text(&game.title).size(14).color(colors().dim),
        ]
        .spacing(8);

        let mut badges: Vec<(String, Color)> = game
            .platforms
            .iter()
            .map(|platform| (platform.clone(), colors().dim))
            .collect();
        if installed {
            badges.push((t!("badge-installed"), colors().good));
        }
        let art = hero(Hero {
            title: &game.title,
            source: game.source(),
            art: game.hero.as_deref(),
            cover: game.cover.as_deref(),
            badges,
            developer: game.developer.as_deref(),
            facts: game
                .minutes
                .filter(|minutes| *minutes > 0)
                .map(|minutes| t!("played-time", time = format::play_time(minutes)))
                .into_iter()
                .collect(),
            pills: self.hero_pills(&game.id),
            id: &game.id,
        });

        let mut page = column![trail, art, self.owned_actions(game, installed)]
            .spacing(20)
            .padding(24);

        let mut main: Vec<Element<'a, Message>> = Vec::new();
        if let Some(summary) = &game.summary {
            main.push(about_card(summary));
        }
        self.add_anti_cheat_card(&game.id, &mut main);
        if let Some(dlc) = self.owned_dlc_card(game) {
            main.push(dlc);
        }
        main.extend(self.achievements_card(&game.id));
        main.extend(self.requirements_card(&game.id));
        let side = column![
            self.owned_details_card(game, installed),
            self.owned_technical_card(game)
        ]
        .spacing(20);

        let two = self.window_width >= TWO_COLUMNS;
        page = if main.is_empty() {
            page.push(side)
        } else if two {
            page.push(
                row![
                    column(main).spacing(20).width(Length::FillPortion(3)),
                    side.width(Length::FillPortion(2)),
                ]
                .spacing(20),
            )
        } else {
            page.extend(main).push(side)
        };
        scrollable(page)
            .id(game_scroll_id())
            .height(Length::Fill)
            .into()
    }

    /// The store, platforms and makers, what the store says of sizes and
    /// versions, and the time the store counted.
    fn owned_details_card<'a>(&'a self, game: &'a Owned, installed: bool) -> Element<'a, Message> {
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        let mut add = |label: String, value: Element<'a, Message>| {
            rows.push(label_row(label, value));
        };
        add(t!("fact-store"), plain(game.shop.label().to_owned()));
        if game.shared {
            add(t!("fact-owner"), plain(t!("store-shared")));
        }
        add(
            t!("fact-status"),
            if installed {
                text(t!("installed")).size(14).color(colors().good).into()
            } else {
                plain(t!("fact-not-installed"))
            },
        );
        if !game.platforms.is_empty() {
            add(t!("fact-platforms"), plain(game.platforms.join(", ")));
        }
        if let Some(developer) = &game.developer {
            add(t!("fact-developer"), plain(developer.clone()));
        }
        for (label, value) in store_facts(Some(game), self.store.info.get(&game.id), false) {
            add(label, plain(value));
        }
        if let Some(minutes) = game.minutes {
            add(
                t!("fact-play-time"),
                plain(if minutes == 0 {
                    t!("fact-never")
                } else {
                    format::play_time(minutes)
                }),
            );
        }
        if let Some(played) = game.last_played {
            add(t!("fact-last-played"), plain(format::ago(played, now())));
        }
        if game.recent_minutes > 0 {
            add(
                t!("fact-recent-time"),
                plain(format::play_time(game.recent_minutes)),
            );
        }
        if let Some(systems) = played_on(game.played_on) {
            add(t!("fact-played-on"), plain(systems));
        }
        card(t!("game-details"), column(rows).spacing(12).into())
    }

    /// The DLC the store says the user owns, with their sizes.
    fn owned_dlc_card<'a>(&'a self, game: &'a Owned) -> Option<Element<'a, Message>> {
        let Some(InfoState::Got(info)) = self.store.info.get(&game.id) else {
            return None;
        };
        if info.dlc.is_empty() {
            return None;
        }
        let mut list = column![].spacing(10);
        for dlc in &info.dlc {
            let note = if !dlc.installable {
                t!("dlc-comes-with")
            } else if let Some(bytes) = dlc.install_size.or(dlc.download_size) {
                Size::Bytes(bytes).to_string()
            } else {
                String::new()
            };
            list = list.push(
                row![
                    text(&dlc.title).size(14).width(Length::Fill),
                    text(note).size(13).color(colors().dim),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        }
        Some(card(t!("dlc-title"), list.into()))
    }

    /// Zendo's id and the store's, folded until asked for.
    fn owned_technical_card<'a>(&'a self, game: &'a Owned) -> Element<'a, Message> {
        let open = self.game_view.technical;
        let header = button(
            row![
                text(t!("game-technical")).size(18).font(BOLD),
                space::horizontal(),
                text(if open { "−" } else { "+" })
                    .size(18)
                    .color(colors().dim),
            ]
            .align_y(Alignment::Center),
        )
        .padding(0)
        .style(style::link)
        .on_press(Message::GameView(GameViewMessage::ToggleTechnical));
        let mut body = column![header].spacing(16);
        if open {
            body = body.push(label_row(
                t!("fact-zendo-id"),
                text(game.id.clone()).size(12).font(Font::MONOSPACE).into(),
            ));
        }
        container(body)
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }
}

/// "Linux 10 h · Windows 45 min": the systems Steam counted time on.
fn played_on(minutes: PlayedOn) -> Option<String> {
    let systems: Vec<String> = [
        (t!("system-linux"), minutes.linux),
        (t!("system-deck"), minutes.deck),
        (t!("system-windows"), minutes.windows),
        (t!("system-mac"), minutes.mac),
    ]
    .into_iter()
    .filter(|(_, minutes)| *minutes > 0)
    .map(|(system, minutes)| format!("{system} {}", format::play_time(minutes)))
    .collect();
    (!systems.is_empty()).then(|| systems.join(" · "))
}

const HERO_HEIGHT: f32 = 340.0;
const COVER_WIDTH: f32 = 140.0;
const COVER_HEIGHT: f32 = 210.0;
const RADIUS: f32 = 14.0;

/// What the wide art at the top of a game's page shows.
pub struct Hero<'a> {
    pub title: &'a str,
    pub source: Source,
    pub art: Option<&'a Path>,
    pub cover: Option<&'a Path>,
    pub badges: Vec<(String, Color)>,
    pub developer: Option<&'a str>,
    pub facts: Vec<String>,
    /// The `ProtonDB` and Steam Deck pills, beside the badges.
    pub pills: Vec<Element<'a, Message>>,
    /// The game whose art "Change art" picks.
    pub id: &'a str,
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
                .add_stop(0.0, colors().surface)
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
                    ..colors().background
                },
            )
            .add_stop(
                1.0,
                Color {
                    a: 0.95,
                    ..colors().background
                },
            ),
    );
    let across = shade(
        Linear::new(Radians(FRAC_PI_2))
            .add_stop(
                0.0,
                Color {
                    a: 0.7,
                    ..colors().background
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
    badges = badges.extend(hero.pills);
    let mut info = column![badges, text(hero.title).size(38).font(BOLD)].spacing(8);
    if let Some(developer) = hero.developer {
        info = info.push(text(developer).size(14).color(colors().dim));
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

fn fact_chip<'a>(fact: String) -> Element<'a, Message> {
    container(text(fact).size(13))
        .padding([4, 10])
        .style(|_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.7,
                ..colors().surface
            })),
            border: Border {
                color: colors().line,
                width: 1.0,
                radius: 14.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}
