//! hyprmonk's colours and the shapes every page shares.

use iced::widget::{button, container, progress_bar, text, text_input};
use iced::{Background, Border, Color, Element, Font, Theme, font};
use zendo_core::Source;

pub const BACKGROUND: Color = Color::from_rgb8(0x17, 0x17, 0x17);
pub const SIDEBAR: Color = Color::from_rgb8(0x12, 0x12, 0x12);
pub const SURFACE: Color = Color::from_rgb8(0x22, 0x22, 0x22);
pub const RAISED: Color = Color::from_rgb8(0x2c, 0x2c, 0x2c);
pub const LINE: Color = Color::from_rgb8(0x30, 0x30, 0x30);
pub const TEXT: Color = Color::from_rgb8(0xcc, 0xd0, 0xcf);
pub const DIM: Color = Color::from_rgb8(0x80, 0x84, 0x83);
pub const ACCENT: Color = Color::from_rgb8(0xf2, 0x56, 0x23);
pub const GOOD: Color = Color::from_rgb8(0x7f, 0xb0, 0x69);
pub const BAD: Color = Color::from_rgb8(0xe0, 0x5a, 0x4c);

pub const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

pub fn theme() -> Theme {
    Theme::custom(
        "Zendo",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: ACCENT,
            success: GOOD,
            warning: Color::from_rgb8(0xe0, 0xb0, 0x4c),
            danger: BAD,
        },
    )
}

/// Each store's colour on its badges.
pub fn source_color(source: Source) -> Color {
    match source {
        Source::Steam => Color::from_rgb8(0x66, 0xa8, 0xe0),
        Source::Epic => Color::from_rgb8(0xd8, 0xd8, 0xd8),
        Source::Gog => Color::from_rgb8(0xb3, 0x7f, 0xe0),
        Source::Amazon => Color::from_rgb8(0xf0, 0xa0, 0x30),
        Source::Heroic => Color::from_rgb8(0xe0, 0x6c, 0x75),
        Source::Desktop => GOOD,
    }
}

/// A small label on a dark ground, readable over a cover.
pub fn badge<'a, M: 'a>(label: impl text::IntoFragment<'a>, color: Color) -> Element<'a, M> {
    container(text(label).size(11).font(BOLD).color(color))
        .padding([2, 7])
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.82,
                ..BACKGROUND
            })),
            border: Border {
                color: Color { a: 0.5, ..color },
                width: 1.0,
                radius: 5.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A badge that is a link: the `ProtonDB` rating. It fills with its colour
/// under the pointer.
pub fn pill(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if hovered {
                Color { a: 0.3, ..color }
            } else {
                Color {
                    a: 0.82,
                    ..BACKGROUND
                }
            })),
            text_color: color,
            border: Border {
                color: Color {
                    a: if hovered { 1.0 } else { 0.5 },
                    ..color
                },
                width: 1.0,
                radius: 5.0.into(),
            },
            ..button::Style::default()
        }
    }
}

/// A raised panel: the details card, a settings section.
pub fn card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn sidebar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SIDEBAR)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// The one action that matters on a page: Play, Save.
pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0xff, 0x6d, 0x3d),
        button::Status::Disabled => Color { a: 0.4, ..ACCENT },
        button::Status::Active => ACCENT,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: BACKGROUND,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => RAISED,
            _ => SURFACE,
        })),
        text_color: if status == button::Status::Disabled {
            DIM
        } else {
            TEXT
        },
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    }
}

/// A button that deletes something, once it has asked.
pub fn danger(_: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgb8(0xf0, 0x6e, 0x60),
        button::Status::Disabled => Color { a: 0.4, ..BAD },
        button::Status::Active => BAD,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: BACKGROUND,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A button that reads as text: links, "← Library".
pub fn link(_: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: None,
        text_color: match status {
            button::Status::Hovered | button::Status::Pressed => ACCENT,
            _ => DIM,
        },
        ..button::Style::default()
    }
}

/// A filter or sort chip, lit when chosen.
pub fn chip(chosen: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if chosen {
                Color { a: 0.16, ..ACCENT }
            } else if hovered {
                RAISED
            } else {
                SURFACE
            })),
            text_color: if chosen { ACCENT } else { TEXT },
            border: Border {
                color: if chosen { ACCENT } else { LINE },
                width: 1.0,
                radius: 16.0.into(),
            },
            ..button::Style::default()
        }
    }
}

/// An entry in the sidebar.
pub fn nav(current: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: (current || hovered).then_some(Background::Color(if current {
                SURFACE
            } else {
                Color { a: 0.5, ..SURFACE }
            })),
            text_color: if current { TEXT } else { DIM },
            border: Border {
                color: if current { LINE } else { Color::TRANSPARENT },
                width: 1.0,
                radius: 8.0.into(),
            },
            ..button::Style::default()
        }
    }
}

pub fn input(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(SURFACE),
        border: Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                ACCENT
            } else {
                LINE
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: DIM,
        placeholder: DIM,
        value: TEXT,
        selection: Color { a: 0.35, ..ACCENT },
    }
}

/// A download's bar: the accent on a raised track.
pub fn progress(_: &Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(RAISED),
        bar: Background::Color(ACCENT),
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
    }
}
