//! The About page.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use super::Message;
use super::style::{self, BOLD, colors};
use crate::i18n::t;

const REPOSITORY: &str = "https://github.com/leandrop-dev/zendo";

pub fn page<'a>() -> Element<'a, Message> {
    let links = row![
        button(text(t!("about-source-code")).size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Open(REPOSITORY.to_owned())),
        button(text(t!("about-report-problem")).size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Open(format!("{REPOSITORY}/issues"))),
    ]
    .spacing(8);

    let card = |title: String, lines: Vec<String>| -> Element<'a, Message> {
        container(
            column![text(title).size(18).font(BOLD)]
                .extend(lines.into_iter().map(|line| text(line).size(14).into()))
                .spacing(8),
        )
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
    };

    scrollable(
        container(
            column![
                column![
                    text("Zendo").size(40).font(BOLD),
                    text(t!("about-version", version = env!("CARGO_PKG_VERSION")))
                        .size(14)
                        .color(colors().dim),
                ]
                .spacing(2),
                text(t!("about-intro")).size(15),
                links,
                card(t!("about-licence"), vec![t!("about-licence-text")]),
                card(
                    t!("about-built-on"),
                    vec![
                        t!("about-built-iced"),
                        t!("about-built-gilrs"),
                        t!("about-built-keyvalues"),
                        t!("about-built-ureq"),
                        t!("about-built-steamgriddb"),
                        t!("about-built-fluent"),
                    ]
                ),
                card(
                    t!("about-ideas-from"),
                    vec![
                        t!("about-ideas-hikari"),
                        t!("about-ideas-omarchy"),
                        t!("about-ideas-heroic"),
                    ]
                ),
            ]
            .spacing(20)
            .padding(24)
            .max_width(860),
        )
        .center_x(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}
