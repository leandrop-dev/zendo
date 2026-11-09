//! gamescope's part of the launch options sheet, under its switch: how its
//! window shows, the size it shows at and the game's, how it scales, the
//! game's frames a second, and more of its options as typed.

use std::fmt;

use iced::widget::{column, pick_list, row, text, text_input};
use iced::{Alignment, Element};
use zendo_core::launch_options::{Filter, GAMESCOPE_SOFTEST, LaunchOptions, Scaler};

use super::{Pick, Sheet, SheetMessage, field, number, size_input};
use crate::app::Message;
use crate::app::style::{self, colors};
use crate::i18n::t;

/// How gamescope's window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Window,
    Borderless,
    FullScreen,
}

impl WindowKind {
    const ALL: [Self; 3] = [Self::Window, Self::Borderless, Self::FullScreen];

    fn of(options: &LaunchOptions) -> Self {
        if options.gamescope_fullscreen {
            Self::FullScreen
        } else if options.gamescope_borderless {
            Self::Borderless
        } else {
            Self::Window
        }
    }
}

impl fmt::Display for WindowKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            Self::Window => t!("options-gamescope-window"),
            Self::Borderless => t!("options-gamescope-borderless"),
            Self::FullScreen => t!("options-full-screen"),
        })
    }
}

#[derive(Debug, Clone)]
pub enum Setting {
    Window(WindowKind),
    GameWidth(String),
    GameHeight(String),
    Scaler(Pick<Scaler>),
    Filter(Pick<Filter>),
    Sharpness(Pick<u8>),
    Fps(String),
    More(String),
}

/// What is typed in its fields until it is saved.
#[derive(Debug, Default)]
pub struct Fields {
    game_width: String,
    game_height: String,
    fps: String,
}

fn shown(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

impl Fields {
    pub fn of(options: &LaunchOptions) -> Self {
        Self {
            game_width: shown(options.gamescope_game_width),
            game_height: shown(options.gamescope_game_height),
            fps: shown(options.gamescope_fps),
        }
    }

    pub fn set(&mut self, options: &mut LaunchOptions, setting: Setting) {
        match setting {
            Setting::Window(kind) => {
                options.gamescope_fullscreen = kind == WindowKind::FullScreen;
                options.gamescope_borderless = kind == WindowKind::Borderless;
            }
            Setting::GameWidth(width) => self.game_width = digits(&width),
            Setting::GameHeight(height) => self.game_height = digits(&height),
            Setting::Scaler(pick) => options.gamescope_scaler = pick.value,
            Setting::Filter(pick) => options.gamescope_filter = pick.value,
            Setting::Sharpness(pick) => options.gamescope_sharpness = pick.value,
            Setting::Fps(fps) => self.fps = digits(&fps),
            Setting::More(more) => options.gamescope_more = more,
        }
    }

    /// Puts the fields into the options; what is wrong when they do not go.
    pub fn keep(&self, options: &mut LaunchOptions) -> Result<(), String> {
        options.gamescope_game_width = number(&self.game_width);
        options.gamescope_game_height = number(&self.game_height);
        options.gamescope_fps = number(&self.fps);
        if options.gamescope_game_width.is_some() != options.gamescope_game_height.is_some() {
            return Err(t!("options-game-size-both"));
        }
        Ok(())
    }
}

fn scaler_label(scaler: Scaler) -> String {
    match scaler {
        Scaler::Auto => t!("options-scaler-auto"),
        Scaler::Integer => t!("options-scaler-integer"),
        Scaler::Fit => t!("options-scaler-fit"),
        Scaler::Fill => t!("options-scaler-fill"),
        Scaler::Stretch => t!("options-scaler-stretch"),
    }
}

fn filter_label(filter: Filter) -> String {
    match filter {
        Filter::Linear => t!("options-filter-linear"),
        Filter::Nearest => t!("options-filter-nearest"),
        Filter::Fsr => "FSR (AMD)".to_owned(),
        Filter::Nis => "NIS (NVIDIA)".to_owned(),
        Filter::Pixel => t!("options-filter-pixel"),
        Filter::Sgsr => "SGSR (Snapdragon)".to_owned(),
    }
}

/// gamescope's settings, under its switch.
pub fn view(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    let fields = &sheet.gamescope;
    let set = |setting| Message::Sheet(SheetMessage::GamescopeSet(setting));
    let sizes = column![
        size_row(
            t!("options-output-size"),
            (&sheet.width, SheetMessage::Width),
            (&sheet.height, SheetMessage::Height),
        ),
        size_row(
            t!("options-game-size"),
            (&fields.game_width, |value| {
                SheetMessage::GamescopeSet(Setting::GameWidth(value))
            }),
            (&fields.game_height, |value| {
                SheetMessage::GamescopeSet(Setting::GameHeight(value))
            }),
        ),
        text(t!("options-sizes-about")).size(13).color(colors().dim),
    ]
    .spacing(8);
    let scaling = scaling(options);
    column![
        labelled(
            t!("options-gamescope-shows"),
            pick_list(
                WindowKind::ALL,
                Some(WindowKind::of(options)),
                move |kind| set(Setting::Window(kind)),
            )
            .text_size(14)
            .padding([8, 12]),
        ),
        sizes,
        scaling,
        labelled(
            t!("options-gamescope-fps"),
            text_input(&t!("options-gamescope-fps-empty"), &fields.fps)
                .on_input(move |value| set(Setting::Fps(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .width(200)
                .style(style::input),
        ),
        field(
            t!("options-gamescope-more"),
            t!("options-gamescope-more-about"),
            text_input("--adaptive-sync", &options.gamescope_more)
                .on_input(move |value| set(Setting::More(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
    ]
    .spacing(14)
    .padding([0, 20])
    .into()
}

/// The fit, the scaling and its sharpening.
fn scaling(options: &LaunchOptions) -> Element<'_, Message> {
    let set = |setting| Message::Sheet(SheetMessage::GamescopeSet(setting));
    let scalers = Pick::list(
        t!("options-gamescope-own"),
        Scaler::ALL.map(|scaler| (scaler, scaler_label(scaler))),
    );
    let filters = Pick::list(
        t!("options-gamescope-own"),
        Filter::ALL.map(|filter| (filter, filter_label(filter))),
    );
    let mut scaling = row![
        labelled(
            t!("options-scaler"),
            pick_list(
                scalers.clone(),
                Pick::chosen(&scalers, options.gamescope_scaler),
                move |pick| set(Setting::Scaler(pick)),
            )
            .text_size(14)
            .padding([8, 12]),
        ),
        labelled(
            t!("options-filter"),
            pick_list(
                filters.clone(),
                Pick::chosen(&filters, options.gamescope_filter),
                move |pick| set(Setting::Filter(pick)),
            )
            .text_size(14)
            .padding([8, 12]),
        ),
    ]
    .spacing(16);
    if options.gamescope_filter.is_some_and(Filter::sharpens) {
        let sharpness = Pick::list(
            t!("options-gamescope-own"),
            (0..=GAMESCOPE_SOFTEST).map(|n| {
                let label = match n {
                    0 => t!("options-fsr-sharpest"),
                    GAMESCOPE_SOFTEST => t!("options-gamescope-softest"),
                    n => n.to_string(),
                };
                (n, label)
            }),
        );
        scaling = scaling.push(labelled(
            t!("options-fsr-sharpening"),
            pick_list(
                sharpness.clone(),
                Pick::chosen(&sharpness, options.gamescope_sharpness),
                move |pick| set(Setting::Sharpness(pick)),
            )
            .text_size(14)
            .padding([8, 12]),
        ));
    }
    scaling.into()
}

/// A setting with its name before it.
fn labelled<'a>(label: String, input: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![text(label).size(14).color(colors().dim), input.into()]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
}

/// A width and a height.
fn size_row<'a>(
    label: String,
    (width, on_width): (&'a str, fn(String) -> SheetMessage),
    (height, on_height): (&'a str, fn(String) -> SheetMessage),
) -> Element<'a, Message> {
    row![
        text(label).size(14).color(colors().dim).width(110),
        size_input(&t!("options-width"), width, on_width),
        text("×").size(14).color(colors().dim),
        size_input(&t!("options-height"), height, on_height),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_window_kind_and_both_game_sizes() {
        let mut options = LaunchOptions::default();
        let mut fields = Fields::of(&options);
        fields.set(&mut options, Setting::Window(WindowKind::Borderless));
        assert!(options.gamescope_borderless && !options.gamescope_fullscreen);
        fields.set(&mut options, Setting::Window(WindowKind::FullScreen));
        assert_eq!(WindowKind::of(&options), WindowKind::FullScreen);
        assert!(!options.gamescope_borderless);
        fields.set(&mut options, Setting::GameWidth("12x80".into()));
        assert!(fields.keep(&mut options).is_err(), "a width needs a height");
        fields.set(&mut options, Setting::GameHeight("720".into()));
        fields.set(&mut options, Setting::Fps("60".into()));
        fields.keep(&mut options).unwrap();
        assert_eq!(
            (
                options.gamescope_game_width,
                options.gamescope_game_height,
                options.gamescope_fps
            ),
            (Some(1280), Some(720), Some(60))
        );
    }
}
