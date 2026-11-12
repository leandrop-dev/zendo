//! The colour scheme KDE Plasma has applied, read from
//! `$XDG_CONFIG_HOME/kdeglobals`, which Plasma rewrites whenever the scheme
//! or the accent changes:
//!
//! ```ini
//! [General]
//! ColorScheme=BreezeDark
//! AccentColor=61,120,198          # only when an accent was chosen
//!
//! [Colors:Window]
//! BackgroundNormal=37,39,47
//! ForegroundNormal=227,226,230
//!
//! [Colors:View]
//! DecorationFocus=61,120,198
//! ```
//!
//! The window's background and text come from `Colors:Window`; the accent
//! from `General`'s `AccentColor`, else `Colors:View`'s `DecorationFocus`,
//! else `Colors:Selection`'s `BackgroundNormal`. Colours are `r,g,b`; a
//! fourth number (alpha) is ignored.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{Context, bail};

use crate::ini::Group;
use crate::paths;
use crate::theme::Rgb;

/// The scheme Plasma has applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    /// Its name: `BreezeDark`.
    pub name: String,
    /// Dark text on a light background.
    pub light: bool,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
}

/// A colour as `r,g,b` (or `r,g,b,a`).
fn colour(text: &str) -> anyhow::Result<Rgb> {
    let channels: Vec<u8> = text
        .split(',')
        .map(|part| part.trim().parse::<u8>())
        .collect::<Result<_, _>>()
        .with_context(|| format!("“{text}” is not an r,g,b colour"))?;
    match channels[..] {
        [r, g, b] | [r, g, b, _] => Ok(Rgb(r, g, b)),
        _ => bail!("“{text}” is not an r,g,b colour"),
    }
}

/// A rough brightness, 0 to 255, by the usual weights.
fn brightness(Rgb(r, g, b): Rgb) -> f32 {
    0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)
}

impl Scheme {
    /// Where Plasma writes it.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::config_home().map(|dir| dir.join("kdeglobals"))
    }

    /// Whether this computer has a `kdeglobals`, applied or not.
    #[must_use]
    pub fn found() -> bool {
        Self::path().is_some_and(|path| path.is_file())
    }

    /// When the file last changed, to notice a new scheme; `None` without it.
    #[must_use]
    pub fn modified() -> Option<SystemTime> {
        fs::metadata(Self::path()?).ok()?.modified().ok()
    }

    /// The scheme in the file: `None` when there is no file, or it holds no
    /// colours yet, as on a desktop that never applied one.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or a colour in it is not `r,g,b`.
    pub fn read() -> anyhow::Result<Option<Self>> {
        let Some(path) = Self::path() else {
            return Ok(None);
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| format!("could not read {}", path.display()));
            }
        };
        Self::parse(&text).with_context(|| format!("{} has a bad colour", path.display()))
    }

    /// A scheme from the file's text.
    ///
    /// # Errors
    ///
    /// When a colour it uses is not `r,g,b`.
    pub fn parse(text: &str) -> anyhow::Result<Option<Self>> {
        let window = Group::parse(text, "[Colors:Window]");
        let (Some(background), Some(foreground)) = (
            window.get("BackgroundNormal"),
            window.get("ForegroundNormal"),
        ) else {
            return Ok(None);
        };
        let general = Group::parse(text, "[General]");
        let accent = general
            .get("AccentColor")
            .filter(|accent| !accent.is_empty())
            .or_else(|| Group::parse(text, "[Colors:View]").get("DecorationFocus"))
            .or_else(|| Group::parse(text, "[Colors:Selection]").get("BackgroundNormal"));
        let Some(accent) = accent else {
            return Ok(None);
        };
        let (background, foreground) = (colour(background)?, colour(foreground)?);
        Ok(Some(Self {
            name: general.get("ColorScheme").unwrap_or("KDE").to_owned(),
            light: brightness(background) > brightness(foreground),
            background,
            foreground,
            accent: colour(accent)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLASMONK: &str = "[General]\nColorScheme=PlasmonkA\n\n\
        [Colors:Selection]\nBackgroundNormal=73,83,100\n\n\
        [Colors:View]\nDecorationFocus=61,120,198\n\n\
        [Colors:Window]\nBackgroundNormal=37,39,47\nForegroundNormal=227,226,230\n";

    #[test]
    fn reads_a_scheme() {
        let scheme = Scheme::parse(PLASMONK).unwrap().unwrap();
        assert_eq!(scheme.name, "PlasmonkA");
        assert_eq!(scheme.background, Rgb(37, 39, 47));
        assert_eq!(scheme.foreground, Rgb(227, 226, 230));
        assert_eq!(scheme.accent, Rgb(61, 120, 198));
        assert!(!scheme.light);
    }

    #[test]
    fn a_chosen_accent_wins() {
        let text = PLASMONK.replace("ColorScheme=PlasmonkA", "AccentColor=1,2,3,255");
        assert_eq!(Scheme::parse(&text).unwrap().unwrap().accent, Rgb(1, 2, 3));
    }

    #[test]
    fn falls_back_to_the_selection_colour() {
        let text = PLASMONK.replace("DecorationFocus", "Other");
        assert_eq!(
            Scheme::parse(&text).unwrap().unwrap().accent,
            Rgb(73, 83, 100)
        );
    }

    #[test]
    fn a_light_scheme_is_light() {
        let text = "[Colors:Window]\nBackgroundNormal=239,240,241\nForegroundNormal=35,38,39\n\
            [Colors:View]\nDecorationFocus=61,174,233\n";
        let scheme = Scheme::parse(text).unwrap().unwrap();
        assert!(scheme.light);
        assert_eq!(scheme.name, "KDE");
    }

    #[test]
    fn no_colours_is_no_scheme() {
        assert_eq!(Scheme::parse("[General]\nColorScheme=X\n").unwrap(), None);
        assert_eq!(Scheme::parse("").unwrap(), None);
    }

    #[test]
    fn a_bad_colour_is_an_error() {
        assert!(Scheme::parse(&PLASMONK.replace("37,39,47", "37,39")).is_err());
        assert!(Scheme::parse(&PLASMONK.replace("37,39,47", "37,39,x")).is_err());
        assert!(Scheme::parse(&PLASMONK.replace("37,39,47", "37,39,300")).is_err());
    }
}
