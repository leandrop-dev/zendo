//! The About page.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use super::Message;
use super::style::{self, BOLD, DIM};

const REPOSITORY: &str = "https://github.com/leandrop-dev/zendo";

pub fn page<'a>() -> Element<'a, Message> {
    let links = row![
        button(text("Source code").size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Open(REPOSITORY.to_owned())),
        button(text("Report a problem").size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Open(format!("{REPOSITORY}/issues"))),
    ]
    .spacing(8);

    let card = |title: &'a str, lines: &[&'a str]| -> Element<'a, Message> {
        container(
            column![text(title).size(18).font(BOLD)]
                .extend(lines.iter().map(|line| text(*line).size(14).into()))
                .spacing(8),
        )
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
    };

    scrollable(
        column![
            column![
                text("Zendo").size(40).font(BOLD),
                text(concat!("Version ", env!("CARGO_PKG_VERSION")))
                    .size(14)
                    .color(DIM),
            ]
            .spacing(2),
            text(
                "Every game on this machine in one library: Steam, Epic, GOG, Amazon \
                 and the rest, each started the way its own launcher would."
            )
            .size(15),
            links,
            card(
                "Licence",
                &["MIT. Zendo reads the launchers' files and never writes to them."]
            ),
            card(
                "Built on",
                &[
                    "Iced, the window and everything in it",
                    "gilrs, for controllers",
                    "keyvalues-parser, for Steam's files",
                    "ureq, serde and toml",
                    "SteamGridDB, for covers no store has",
                ]
            ),
            card(
                "Ideas from",
                &[
                    "Hikari: the sidebar, the library and the game pages",
                    "omarchy-games-menu: typing anywhere to filter",
                    "Heroic: where its games are kept, and its launch links",
                ]
            ),
        ]
        .spacing(20)
        .padding(24)
        .max_width(860),
    )
    .height(Length::Fill)
    .into()
}
