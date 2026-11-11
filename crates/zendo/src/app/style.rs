//! The window's colours and the shapes every page shares. The colours are
//! one palette for the whole window, made from a theme
//! ([`Colors::from_theme`]) and changed with [`set_colors`]; the pages read
//! it with [`colors`].

use std::sync::{PoisonError, RwLock};

use iced::widget::{
    button, checkbox as checkbox_widget, container, progress_bar, text, text_input,
};
use iced::{Background, Border, Color, Element, Font, Theme, font};
use zendo_core::Source;
use zendo_core::theme::Theme as ThemeFile;
use zendo_core::tone::Rgb;

/// The window's colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub background: Color,
    /// The sidebar, a step away from the background.
    pub sidebar: Color,
    /// Cards, inputs, chips.
    pub surface: Color,
    /// A surface under the pointer, a track.
    pub raised: Color,
    /// The rim of cards and inputs.
    pub line: Color,
    pub text: Color,
    /// Text that matters less.
    pub dim: Color,
    pub accent: Color,
    pub good: Color,
    pub bad: Color,
    /// Dark text on a light background.
    pub light: bool,
}

impl Colors {
    /// Zendo's own, as `themes/zendo.toml` gives them: the colours until a
    /// theme is read, and when none can be.
    pub const ZENDO: Self = Self {
        background: Color::from_rgb8(0x17, 0x17, 0x17),
        sidebar: Color::from_rgb8(0x12, 0x12, 0x12),
        surface: Color::from_rgb8(0x22, 0x22, 0x22),
        raised: Color::from_rgb8(0x2c, 0x2c, 0x2c),
        line: Color::from_rgb8(0x30, 0x30, 0x30),
        text: Color::from_rgb8(0xcc, 0xd0, 0xcf),
        dim: Color::from_rgb8(0x80, 0x84, 0x83),
        accent: Color::from_rgb8(0xf2, 0x56, 0x23),
        good: Color::from_rgb8(0x7f, 0xb0, 0x69),
        bad: Color::from_rgb8(0xe0, 0x5a, 0x4c),
        light: false,
    };

    /// A palette from a theme: its background, text and accent, with the
    /// greys it leaves out mixed between the first two as Zendo's own are.
    /// A light theme gets a darker green and red when it brings none.
    pub fn from_theme(theme: &ThemeFile) -> Self {
        let background = color(theme.background);
        let text = color(theme.foreground);
        let mix = |amount: f32| blend(background, text, amount);
        let or_mixed = |given: Option<Rgb>, amount: f32| given.map_or_else(|| mix(amount), color);
        let (good, bad) = if theme.light {
            (
                Color::from_rgb8(0x3f, 0x7a, 0x34),
                Color::from_rgb8(0xb0, 0x3a, 0x30),
            )
        } else {
            (Self::ZENDO.good, Self::ZENDO.bad)
        };
        Self {
            background,
            sidebar: theme.sidebar.map_or_else(
                || {
                    if theme.light {
                        mix(0.04)
                    } else {
                        blend(background, Color::BLACK, 0.22)
                    }
                },
                color,
            ),
            surface: or_mixed(theme.surface, 0.06),
            raised: or_mixed(theme.raised, 0.115),
            line: or_mixed(theme.line, 0.14),
            text,
            dim: or_mixed(theme.dim, 0.58),
            accent: color(theme.accent),
            good: theme.positive.map_or(good, color),
            bad: theme.negative.map_or(bad, color),
            light: theme.light,
        }
    }

    /// A colour under the pointer: lighter on a dark palette, darker on a
    /// light one.
    fn hovered(&self, color: Color) -> Color {
        if self.light {
            blend(color, Color::BLACK, 0.12)
        } else {
            blend(color, Color::WHITE, 0.15)
        }
    }
}

fn color(Rgb(r, g, b): Rgb) -> Color {
    Color::from_rgb8(r, g, b)
}

/// `from` moved `amount` of the way to `to`.
fn blend(from: Color, to: Color, amount: f32) -> Color {
    let step = |a: f32, b: f32| a + (b - a) * amount;
    Color::from_rgb(step(from.r, to.r), step(from.g, to.g), step(from.b, to.b))
}

static CURRENT: RwLock<Colors> = RwLock::new(Colors::ZENDO);

/// The window's colours now.
pub fn colors() -> Colors {
    // A palette is only ever replaced whole, so one left by a panic
    // elsewhere is still a whole palette.
    *CURRENT.read().unwrap_or_else(PoisonError::into_inner)
}

/// Changes the window's colours; the next frame shows them.
pub fn set_colors(colors: Colors) {
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = colors;
}

pub const BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..Font::DEFAULT
};

/// Iced's theme from the window's colours: what the widgets Zendo does
/// not style itself (scroll bars, pick lists, togglers) are drawn in.
pub fn theme() -> Theme {
    let colors = colors();
    Theme::custom(
        "Zendo",
        iced::theme::Palette {
            background: colors.background,
            text: colors.text,
            primary: colors.accent,
            success: colors.good,
            warning: Color::from_rgb8(0xe0, 0xb0, 0x4c),
            danger: colors.bad,
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
        Source::Desktop => colors().good,
        Source::Local => colors().accent,
    }
}

/// A small label on a dark ground, readable over a cover.
pub fn badge<'a, M: 'a>(label: impl text::IntoFragment<'a>, color: Color) -> Element<'a, M> {
    container(text(label).size(11).font(BOLD).color(color))
        .padding([2, 7])
        .style(move |_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.82,
                ..colors().background
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
                    ..colors().background
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
        background: Some(Background::Color(colors().surface)),
        border: Border {
            color: colors().line,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn sidebar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors().sidebar)),
        border: Border {
            color: colors().line,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// The one action that matters on a page: Play, Save.
pub fn primary(_: &Theme, status: button::Status) -> button::Style {
    let colors = colors();
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => colors.hovered(colors.accent),
        button::Status::Disabled => Color {
            a: 0.4,
            ..colors.accent
        },
        button::Status::Active => colors.accent,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: colors.background,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn secondary(_: &Theme, status: button::Status) -> button::Style {
    let colors = colors();
    let disabled = status == button::Status::Disabled;
    button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => colors.raised,
            // A disabled one is not a surface the user can press.
            button::Status::Disabled => Color {
                a: 0.4,
                ..colors.surface
            },
            button::Status::Active => colors.surface,
        })),
        text_color: if disabled { colors.dim } else { colors.text },
        border: Border {
            color: if disabled {
                Color {
                    a: 0.4,
                    ..colors.line
                }
            } else {
                colors.line
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    }
}

/// A button that deletes something, once it has asked.
pub fn danger(_: &Theme, status: button::Status) -> button::Style {
    let colors = colors();
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => colors.hovered(colors.bad),
        button::Status::Disabled => Color {
            a: 0.4,
            ..colors.bad
        },
        button::Status::Active => colors.bad,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: colors.background,
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
            button::Status::Hovered | button::Status::Pressed => colors().accent,
            _ => colors().dim,
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
                Color {
                    a: 0.16,
                    ..colors().accent
                }
            } else if hovered {
                colors().raised
            } else {
                colors().surface
            })),
            text_color: if chosen {
                colors().accent
            } else {
                colors().text
            },
            border: Border {
                color: if chosen {
                    colors().accent
                } else {
                    colors().line
                },
                width: 1.0,
                radius: 16.0.into(),
            },
            ..button::Style::default()
        }
    }
}

/// The mark in a cover's corner: a box outlined when empty, filled when the
/// card is marked. Drawn without a glyph, so no font can move it or turn it
/// into a missing character.
pub fn square(marked: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let fill = if marked {
            colors().good
        } else if hovered {
            colors().raised
        } else {
            Color::TRANSPARENT
        };
        button::Style {
            background: Some(Background::Color(fill)),
            text_color: colors().text,
            border: Border {
                color: if marked { colors().good } else { colors().line },
                width: 1.5,
                radius: 6.0.into(),
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
                colors().surface
            } else {
                Color {
                    a: 0.5,
                    ..colors().surface
                }
            })),
            text_color: if current { colors().text } else { colors().dim },
            border: Border {
                color: if current {
                    colors().line
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 8.0.into(),
            },
            ..button::Style::default()
        }
    }
}

/// A tick box: the accent when ticked, rather than Iced's green, which is
/// not in the palette. Disabled keeps the state but fades it.
pub fn checkbox(_: &Theme, status: checkbox_widget::Status) -> checkbox_widget::Style {
    let colors = colors();
    let (hovered, checked, disabled) = match status {
        checkbox_widget::Status::Active { is_checked } => (false, is_checked, false),
        checkbox_widget::Status::Hovered { is_checked } => (true, is_checked, false),
        checkbox_widget::Status::Disabled { is_checked } => (false, is_checked, true),
    };
    let (background, border, icon) = if checked {
        let accent = if disabled {
            Color {
                a: 0.4,
                ..colors.accent
            }
        } else {
            colors.accent
        };
        (accent, accent, colors.background)
    } else {
        (
            if hovered {
                colors.raised
            } else {
                colors.surface
            },
            colors.line,
            colors.text,
        )
    };
    checkbox_widget::Style {
        background: Background::Color(background),
        icon_color: icon,
        border: Border {
            color: border,
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: Some(if disabled { colors.dim } else { colors.text }),
    }
}

pub fn input(_: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(colors().surface),
        border: Border {
            color: if matches!(status, text_input::Status::Focused { .. }) {
                colors().accent
            } else {
                colors().line
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        icon: colors().dim,
        placeholder: colors().dim,
        value: colors().text,
        selection: Color {
            a: 0.35,
            ..colors().accent
        },
    }
}

/// A download's bar: the accent on a raised track.
pub fn progress(_: &Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(colors().raised),
        bar: Background::Color(colors().accent),
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
    }
}
