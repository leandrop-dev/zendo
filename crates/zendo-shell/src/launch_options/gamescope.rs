//! gamescope's part of the launch options sheet, under its switch: how its
//! window shows, the size it shows at and the game's, how it scales, the
//! game's frames a second, and more of its options as typed.

use std::fmt;

use zendo_core::launch_options::{Filter, GAMESCOPE_SOFTEST, LaunchOptions, Scaler};

use super::{Pick, digits, number};
use crate::t;

/// How gamescope's window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Window,
    Borderless,
    FullScreen,
}

impl WindowKind {
    pub const ALL: [Self; 3] = [Self::Window, Self::Borderless, Self::FullScreen];

    #[must_use]
    pub fn of(options: &LaunchOptions) -> Self {
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
    pub game_width: String,
    pub game_height: String,
    pub fps: String,
}

fn shown(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

impl Fields {
    #[must_use]
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

    /// Puts the fields into the options.
    ///
    /// # Errors
    ///
    /// What is wrong when they do not go: one of the game's sizes alone.
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

#[must_use]
pub fn scaler_label(scaler: Scaler) -> String {
    match scaler {
        Scaler::Auto => t!("options-scaler-auto"),
        Scaler::Integer => t!("options-scaler-integer"),
        Scaler::Fit => t!("options-scaler-fit"),
        Scaler::Fill => t!("options-scaler-fill"),
        Scaler::Stretch => t!("options-scaler-stretch"),
    }
}

#[must_use]
pub fn filter_label(filter: Filter) -> String {
    match filter {
        Filter::Linear => t!("options-filter-linear"),
        Filter::Nearest => t!("options-filter-nearest"),
        Filter::Fsr => "FSR (AMD)".to_owned(),
        Filter::Nis => "NIS (NVIDIA)".to_owned(),
        Filter::Pixel => t!("options-filter-pixel"),
        Filter::Sgsr => "SGSR (Snapdragon)".to_owned(),
    }
}

/// The scalers, the setting unset first.
#[must_use]
pub fn scalers() -> Vec<Pick<Scaler>> {
    Pick::list(
        t!("options-gamescope-own"),
        Scaler::ALL.map(|scaler| (scaler, scaler_label(scaler))),
    )
}

/// The filters, the setting unset first.
#[must_use]
pub fn filters() -> Vec<Pick<Filter>> {
    Pick::list(
        t!("options-gamescope-own"),
        Filter::ALL.map(|filter| (filter, filter_label(filter))),
    )
}

/// The sharpness values, the setting unset first.
#[must_use]
pub fn sharpness() -> Vec<Pick<u8>> {
    Pick::list(
        t!("options-gamescope-own"),
        (0..=GAMESCOPE_SOFTEST).map(|n| {
            let label = match n {
                0 => t!("options-fsr-sharpest"),
                GAMESCOPE_SOFTEST => t!("options-gamescope-softest"),
                n => n.to_string(),
            };
            (n, label)
        }),
    )
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
