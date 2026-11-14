//! The window's colours: a theme file (Zendo's own by default, or one of the
//! user's), or the KDE colour scheme in use, followed as it changes.
//!
//! The same rules as the Iced window's `app/style.rs` and `app/themes.rs`,
//! without the toolkit: the built-in themes are the files in
//! `crates/zendo/themes/`, the user's are read from `~/.config/zendo/themes/`
//! (see [`zendo_core::theme`]), and one named as a built-in replaces it.

use std::path::Path;
use std::time::SystemTime;

use zendo_core::kde::Scheme;
use zendo_core::theme::{Rgb, Theme};

/// The themes built into Zendo: each file's name and text.
const BUILT_IN: &[(&str, &str)] = &[("zendo", include_str!("../../zendo/themes/zendo.toml"))];

/// The theme used when none is chosen, or the one chosen is gone.
const DEFAULT: &str = "zendo";

/// What `config.toml`'s `theme` names for the KDE colour scheme.
const SYSTEM: &str = "system";

/// The window's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    /// The top bar, a step away from the background.
    pub sidebar: Rgb,
    /// Cards, inputs, chips.
    pub surface: Rgb,
    /// A surface under the pointer, a track.
    pub raised: Rgb,
    /// The rim of cards and inputs.
    pub line: Rgb,
    pub foreground: Rgb,
    /// Text that matters less.
    pub dim: Rgb,
    pub accent: Rgb,
    pub good: Rgb,
    pub bad: Rgb,
    /// Dark text on a light background.
    pub light: bool,
}

impl Palette {
    /// Zendo's own, as `themes/zendo.toml` gives them.
    pub const ZENDO: Self = Self {
        background: Rgb(0x17, 0x17, 0x17),
        sidebar: Rgb(0x12, 0x12, 0x12),
        surface: Rgb(0x22, 0x22, 0x22),
        raised: Rgb(0x2c, 0x2c, 0x2c),
        line: Rgb(0x30, 0x30, 0x30),
        foreground: Rgb(0xcc, 0xd0, 0xcf),
        dim: Rgb(0x80, 0x84, 0x83),
        accent: Rgb(0xf2, 0x56, 0x23),
        good: Rgb(0x7f, 0xb0, 0x69),
        bad: Rgb(0xe0, 0x5a, 0x4c),
        light: false,
    };

    /// A palette from a theme: its background, text and accent, with the
    /// greys it leaves out mixed between the first two as Zendo's own are.
    /// A light theme gets a darker green and red when it brings none.
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        let background = theme.background;
        let foreground = theme.foreground;
        let mix = |amount: f32| blend(background, foreground, amount);
        let or_mixed = |given: Option<Rgb>, amount: f32| given.unwrap_or_else(|| mix(amount));
        let (good, bad) = if theme.light {
            (Rgb(0x3f, 0x7a, 0x34), Rgb(0xb0, 0x3a, 0x30))
        } else {
            (Self::ZENDO.good, Self::ZENDO.bad)
        };
        Self {
            background,
            sidebar: theme.sidebar.unwrap_or_else(|| {
                if theme.light {
                    mix(0.04)
                } else {
                    blend(background, Rgb(0, 0, 0), 0.22)
                }
            }),
            surface: or_mixed(theme.surface, 0.06),
            raised: or_mixed(theme.raised, 0.115),
            line: or_mixed(theme.line, 0.14),
            foreground,
            dim: or_mixed(theme.dim, 0.58),
            accent: theme.accent,
            good: theme.positive.unwrap_or(good),
            bad: theme.negative.unwrap_or(bad),
            light: theme.light,
        }
    }
}

/// `from` moved `amount` (0 to 1) of the way to `to`, channel by channel.
fn blend(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let step = |a: u8, b: u8| {
        let (a, b) = (f32::from(a) / 255.0, f32::from(b) / 255.0);
        channel(a + (b - a) * amount)
    };
    Rgb(step(from.0, to.0), step(from.1, to.1), step(from.2, to.2))
}

/// A channel from 0 to 1 as a byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0 to 255 and rounded first"
)]
fn channel(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A theme Settings offers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Choice {
    /// A theme file, by its name.
    File(String),
    /// The KDE colour scheme in use when there is one, else Zendo's own.
    System,
}

impl Choice {
    /// The one the settings name; Zendo's own for none, and for a
    /// `hyprmonk` saved by an earlier Zendo.
    fn chosen(tag: Option<&str>) -> Self {
        match tag {
            Some(SYSTEM) => Self::System,
            Some(id) if !id.trim().is_empty() && id != "hyprmonk" => Self::File(id.to_owned()),
            _ => Self::File(DEFAULT.to_owned()),
        }
    }
}

/// The built-in themes, with the user's from `dir` over them, by name.
fn all_themes(dir: Option<&Path>) -> Vec<Theme> {
    let mut themes: Vec<Theme> = BUILT_IN
        .iter()
        .filter_map(|(id, text)| Theme::parse(id, text).ok())
        .collect();
    // A file that is not a theme is Settings' to say (item 50).
    let (own, _problems) = dir.map(Theme::read_dir).unwrap_or_default();
    for theme in own {
        match themes.iter_mut().find(|known| known.id == theme.id) {
            Some(known) => *known = theme,
            None => themes.push(theme),
        }
    }
    themes
}

/// The window's colours, from the choice.
#[derive(Debug)]
pub struct Themes {
    choice: Choice,
    files: Vec<Theme>,
    /// When `kdeglobals` last changed, as last read.
    modified: Option<SystemTime>,
    palette: Palette,
}

impl Themes {
    /// The settings' theme (`config.toml`'s `theme`), from the built-in
    /// themes and the user's in `dir`.
    #[must_use]
    pub fn chosen(tag: Option<&str>, dir: Option<&Path>) -> Self {
        let mut themes = Self {
            choice: Choice::chosen(tag),
            files: all_themes(dir),
            modified: None,
            palette: Palette::ZENDO,
        };
        themes.apply();
        themes
    }

    /// The colours as last read.
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.palette
    }

    /// Whether the window follows the KDE scheme, so `kdeglobals` is watched.
    #[must_use]
    pub fn watching(&self) -> bool {
        self.choice == Choice::System
    }

    /// A look at `kdeglobals`: `true` when the colours changed.
    pub fn look(&mut self) -> bool {
        if !self.watching() || Scheme::modified() == self.modified {
            return false;
        }
        let before = self.palette;
        self.apply();
        self.palette != before
    }

    /// Sets the colours from the choice, reading the KDE scheme again when
    /// it needs it.
    fn apply(&mut self) {
        self.modified = None;
        let id = match &self.choice {
            Choice::File(id) => id.clone(),
            Choice::System => return self.apply_scheme(),
        };
        self.palette = self
            .files
            .iter()
            .find(|theme| theme.id == id)
            .map_or_else(|| self.default_palette(), Palette::from_theme);
    }

    fn apply_scheme(&mut self) {
        self.modified = Scheme::modified();
        match Scheme::read() {
            Ok(Some(scheme)) => {
                self.palette = Palette::from_theme(&Theme::from_scheme(&scheme));
            }
            Ok(None) => self.palette = self.default_palette(),
            // Perhaps caught half written: looked at again next time, the
            // colours as they were.
            Err(_) => self.modified = None,
        }
    }

    /// Zendo's own, from its file (the user's, when it replaces it).
    fn default_palette(&self) -> Palette {
        self.files
            .iter()
            .find(|theme| theme.id == DEFAULT)
            .map_or(Palette::ZENDO, Palette::from_theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme(text: &str) -> Theme {
        Theme::parse("test", text).unwrap()
    }

    #[test]
    fn zendos_own_file_gives_the_built_in_colours() {
        let own = theme(BUILT_IN[0].1);
        assert_eq!(Palette::from_theme(&own), Palette::ZENDO);
    }

    #[test]
    fn a_theme_without_greys_has_them_mixed() {
        let dusk =
            theme("background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#ff0000\"\n");
        let palette = Palette::from_theme(&dusk);
        // 6 % of the way from black to white is 15 of 255.
        assert_eq!(palette.surface, Rgb(15, 15, 15));
        assert_eq!(palette.raised, Rgb(29, 29, 29));
        // A dark theme's sidebar is the background pulled to black.
        assert_eq!(palette.sidebar, Rgb(0, 0, 0));
        assert_eq!(palette.good, Palette::ZENDO.good);
    }

    #[test]
    fn a_light_theme_gets_darker_greens_and_reds() {
        let paper = theme(
            "light = true\nbackground = \"#ffffff\"\nforeground = \"#000000\"\naccent = \"#0000ff\"\n",
        );
        let palette = Palette::from_theme(&paper);
        assert!(palette.light);
        assert_eq!(palette.good, Rgb(0x3f, 0x7a, 0x34));
        assert_eq!(palette.bad, Rgb(0xb0, 0x3a, 0x30));
    }

    #[test]
    fn the_choice_names_a_file_the_scheme_or_zendos_own() {
        assert_eq!(Choice::chosen(None), Choice::File("zendo".into()));
        assert_eq!(Choice::chosen(Some("system")), Choice::System);
        assert_eq!(Choice::chosen(Some("dusk")), Choice::File("dusk".into()));
        assert_eq!(
            Choice::chosen(Some("hyprmonk")),
            Choice::File("zendo".into())
        );
        assert_eq!(Choice::chosen(Some("  ")), Choice::File("zendo".into()));
    }

    #[test]
    fn a_theme_that_is_gone_shows_zendos_own() {
        let themes = Themes::chosen(Some("no-such-theme"), None);
        assert_eq!(themes.palette(), Palette::ZENDO);
        assert!(!themes.watching());
    }
}
