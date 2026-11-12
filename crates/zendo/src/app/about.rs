//! The About page.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use super::Message;
use super::game::card;
use super::style::{self, BOLD, BUTTON, SPACE_L, SPACE_S, SPACE_XL, SPACE_XS, button_text, colors};
use crate::i18n::t;

const REPOSITORY: &str = "https://github.com/leandrop-dev/zendo";

pub fn page<'a>() -> Element<'a, Message> {
    let links = row![
        button(button_text(t!("about-source-code")))
            .padding(BUTTON)
            .style(style::secondary)
            .on_press(Message::Open(REPOSITORY.to_owned())),
        button(button_text(t!("about-report-problem")))
            .padding(BUTTON)
            .style(style::secondary)
            .on_press(Message::Open(format!("{REPOSITORY}/issues"))),
    ]
    .spacing(SPACE_S);

    let block = |title: String, lines: Vec<String>| -> Element<'a, Message> {
        card(
            title,
            column(lines.into_iter().map(|line| text(line).size(14).into()))
                .spacing(SPACE_S)
                .into(),
        )
    };

    let intro = container(
        column![
            column![
                text("Zendo").size(40).font(BOLD),
                text(t!("about-version", version = env!("CARGO_PKG_VERSION")))
                    .size(14)
                    .color(colors().dim),
            ]
            .spacing(SPACE_XS),
            text(t!("about-intro")).size(15),
            links,
        ]
        .spacing(SPACE_L),
    )
    .padding(SPACE_L)
    .width(Length::Fill)
    .style(style::card);

    scrollable(
        container(
            column![
                intro,
                block(t!("about-licence"), vec![t!("about-licence-text")]),
                block(
                    t!("about-built-on"),
                    vec![
                        t!("about-built-iced"),
                        t!("about-built-gilrs"),
                        t!("about-built-keyvalues"),
                        t!("about-built-ureq"),
                        t!("about-built-steamgriddb"),
                        t!("about-built-fluent"),
                        t!("about-built-font"),
                    ]
                ),
            ]
            .spacing(SPACE_L)
            .padding(SPACE_XL)
            .max_width(860),
        )
        .center_x(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}
