//! hyprmonk's tone: the colours of the desktop's theme, which hyprmonk
//! writes for Zendo to `~/.local/state/hyprmonk/theme/tone.json` whenever
//! the tone saved in its Settings, or the accent it takes from the
//! wallpaper, changes:
//!
//! ```json
//! {
//!   "version": 1,
//!   "name": "noir",
//!   "label": "Noir",
//!   "light": false,
//!   "background": "#171717",
//!   "foreground": "#ccd0cf",
//!   "accent": "#f25623",
//!   "positive": "#88a57d",
//!   "negative": "#e05a4c"
//! }
//! ```
//!
//! `positive` and `negative` may be left out; Zendo then uses its own green
//! and red. Keys it does not know are ignored, so hyprmonk can add more.
//! The path is hyprmonk's own state directory, which it keeps under
//! `$HOME/.local/state` whatever `XDG_STATE_HOME` says.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::paths;

/// The newest file layout Zendo reads.
const VERSION: u32 = 1;

/// A colour as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl FromStr for Rgb {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> anyhow::Result<Self> {
        let hex = text
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .with_context(|| format!("“{text}” is not a #rrggbb colour"))?;
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16);
        Ok(Self(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// The tone hyprmonk has saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tone {
    /// Its key in hyprmonk: `noir`, `kanso`, `wallpaper`.
    pub name: String,
    /// Its name as hyprmonk's Settings show it: Noir, Kanso.
    pub label: String,
    /// Dark text on a light background.
    pub light: bool,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    /// A calm green for what went well.
    pub positive: Option<Rgb>,
    /// A red for what went wrong.
    pub negative: Option<Rgb>,
}

/// The file as written, before its colours are checked.
#[derive(Deserialize)]
struct File {
    version: u32,
    name: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    light: bool,
    background: String,
    foreground: String,
    accent: String,
    positive: Option<String>,
    negative: Option<String>,
}

impl Tone {
    /// Where hyprmonk writes it.
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::home().map(|home| home.join(".local/state/hyprmonk/theme/tone.json"))
    }

    /// The tone in the file: `None` when there is no file, as without
    /// hyprmonk.
    ///
    /// # Errors
    ///
    /// When the file cannot be read, is not a tone, or is of a newer
    /// layout than Zendo knows.
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
        Self::parse(&text)
            .with_context(|| format!("{} is not a tone Zendo can read", path.display()))
            .map(Some)
    }

    /// Whether hyprmonk is on this computer: its state directory is
    /// there, whether or not it has written a tone yet.
    #[must_use]
    pub fn hyprmonk_found() -> bool {
        paths::home().is_some_and(|home| home.join(".local/state/hyprmonk").is_dir())
    }

    /// When the file last changed, to notice a new tone; `None` without it.
    #[must_use]
    pub fn modified() -> Option<SystemTime> {
        fs::metadata(Self::path()?).ok()?.modified().ok()
    }

    /// A tone from the file's text.
    ///
    /// # Errors
    ///
    /// When it is not JSON in the layout above, a colour is not `#rrggbb`,
    /// or its version is newer than Zendo knows.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let file: File = serde_json::from_str(text).context("not a tone's JSON")?;
        if file.version > VERSION {
            bail!(
                "it is version {}, and this Zendo reads up to {VERSION}",
                file.version
            );
        }
        let optional = |colour: Option<String>| colour.as_deref().map(str::parse).transpose();
        Ok(Self {
            label: if file.label.is_empty() {
                file.name.clone()
            } else {
                file.label
            },
            name: file.name,
            light: file.light,
            background: file.background.parse()?,
            foreground: file.foreground.parse()?,
            accent: file.accent.parse()?,
            positive: optional(file.positive)?,
            negative: optional(file.negative)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_tone() {
        let tone = Tone::parse(
            r##"{"version": 1, "name": "kanso", "label": "Kanso", "light": false,
                "background": "#090e13", "foreground": "#C5C9C7", "accent": "#8ba4b0",
                "positive": "#87a987", "terminal": ["#090e13"]}"##,
        )
        .unwrap();
        assert_eq!(tone.label, "Kanso");
        assert_eq!(tone.background, Rgb(0x09, 0x0e, 0x13));
        assert_eq!(tone.foreground, Rgb(0xc5, 0xc9, 0xc7));
        assert_eq!(tone.positive, Some(Rgb(0x87, 0xa9, 0x87)));
        assert_eq!(tone.negative, None);
    }

    #[test]
    fn needs_its_colours_and_a_known_version() {
        let without_accent =
            r##"{"version": 1, "name": "x", "background": "#000000", "foreground": "#ffffff"}"##;
        assert!(Tone::parse(without_accent).is_err());
        let short = r##"{"version": 1, "name": "x", "background": "#000", "foreground": "#ffffff", "accent": "#ff0000"}"##;
        assert!(Tone::parse(short).is_err());
        let newer = r##"{"version": 2, "name": "x", "background": "#000000", "foreground": "#ffffff", "accent": "#ff0000"}"##;
        assert!(Tone::parse(newer).is_err());
        let fine = r##"{"version": 1, "name": "x", "background": "#000000", "foreground": "#ffffff", "accent": "#ff0000"}"##;
        assert_eq!(Tone::parse(fine).unwrap().label, "x");
    }

    #[test]
    fn reads_colours() {
        assert_eq!("#f25623".parse::<Rgb>().unwrap(), Rgb(0xf2, 0x56, 0x23));
        assert!("f25623".parse::<Rgb>().is_err());
        assert!("#f2562".parse::<Rgb>().is_err());
        assert!("#gg5623".parse::<Rgb>().is_err());
    }
}
