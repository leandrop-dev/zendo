//! Themes for Zendo's window, each a TOML file named after it. Zendo's own
//! are built into it; the user's own go in `$XDG_CONFIG_HOME/zendo/themes/`,
//! and one named as a built-in replaces it:
//!
//! ```toml
//! # ~/.config/zendo/themes/dusk.toml: chosen as "dusk"
//! name = "Dusk"              # as Settings lists it
//! light = false              # dark text on a light background when true
//! background = "#1a1b26"
//! foreground = "#c0caf5"     # the text
//! accent = "#e0af68"
//! # Optional: a green and a red; Zendo's own when left out.
//! positive = "#9ece6a"
//! negative = "#f7768e"
//! # Optional: the greys, each mixed from the background and the text when
//! # left out.
//! sidebar = "#16161e"
//! surface = "#24283b"
//! raised = "#2f3549"
//! line = "#3b4261"
//! dim = "#737aa2"
//! ```
//!
//! hyprmonk's tone ([`crate::tone`]) becomes a theme too.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::paths;
use crate::tone::{Rgb, Tone};

/// A theme's colours, as its file gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Its file's name without `.toml`: what `config.toml`'s `theme` says.
    pub id: String,
    /// As Settings lists it.
    pub name: String,
    pub light: bool,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub positive: Option<Rgb>,
    pub negative: Option<Rgb>,
    pub sidebar: Option<Rgb>,
    pub surface: Option<Rgb>,
    pub raised: Option<Rgb>,
    pub line: Option<Rgb>,
    pub dim: Option<Rgb>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    name: Option<String>,
    #[serde(default)]
    light: bool,
    background: String,
    foreground: String,
    accent: String,
    positive: Option<String>,
    negative: Option<String>,
    sidebar: Option<String>,
    surface: Option<String>,
    raised: Option<String>,
    line: Option<String>,
    dim: Option<String>,
}

impl Theme {
    /// Where the user's own themes go.
    #[must_use]
    pub fn dir() -> Option<PathBuf> {
        paths::config_home().map(|dir| dir.join("zendo/themes"))
    }

    /// A theme from a file's text; `id` is its file's name.
    ///
    /// # Errors
    ///
    /// When it is not TOML in the layout above, has a key it does not
    /// know, or a colour is not `#rrggbb`.
    pub fn parse(id: &str, text: &str) -> anyhow::Result<Self> {
        let file: File = toml::from_str(text)?;
        let optional = |colour: Option<String>| -> anyhow::Result<Option<Rgb>> {
            colour.as_deref().map(str::parse).transpose()
        };
        Ok(Self {
            id: id.to_owned(),
            name: file.name.unwrap_or_else(|| id.to_owned()),
            light: file.light,
            background: file.background.parse()?,
            foreground: file.foreground.parse()?,
            accent: file.accent.parse()?,
            positive: optional(file.positive)?,
            negative: optional(file.negative)?,
            sidebar: optional(file.sidebar)?,
            surface: optional(file.surface)?,
            raised: optional(file.raised)?,
            line: optional(file.line)?,
            dim: optional(file.dim)?,
        })
    }

    /// hyprmonk's tone as a theme, its greys left to mix.
    #[must_use]
    pub fn from_tone(tone: &Tone) -> Self {
        Self {
            id: "hyprmonk".to_owned(),
            name: tone.label.clone(),
            light: tone.light,
            background: tone.background,
            foreground: tone.foreground,
            accent: tone.accent,
            positive: tone.positive,
            negative: tone.negative,
            sidebar: None,
            surface: None,
            raised: None,
            line: None,
            dim: None,
        }
    }

    /// The themes in a folder, by name, and a line for each file that is
    /// not one. A missing folder has none.
    #[must_use]
    pub fn read_dir(dir: &Path) -> (Vec<Self>, Vec<String>) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return (Vec::new(), Vec::new()),
            Err(err) => return (Vec::new(), vec![format!("{}: {err}", dir.display())]),
        };
        let mut themes = Vec::new();
        let mut problems = Vec::new();
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.extension().is_none_or(|ext| ext != "toml") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let read = fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|text| Self::parse(id, &text))
                .with_context(|| path.display().to_string());
            match read {
                Ok(theme) => themes.push(theme),
                Err(err) => problems.push(format!("{err:#}")),
            }
        }
        themes.sort_by(|a, b| a.id.cmp(&b.id));
        problems.sort();
        (themes, problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_theme() {
        let theme = Theme::parse(
            "dusk",
            "name = \"Dusk\"\nbackground = \"#1a1b26\"\nforeground = \"#c0caf5\"\naccent = \"#e0af68\"\ndim = \"#737aa2\"\n",
        )
        .unwrap();
        assert_eq!(theme.name, "Dusk");
        assert_eq!(theme.accent, Rgb(0xe0, 0xaf, 0x68));
        assert_eq!(theme.dim, Some(Rgb(0x73, 0x7a, 0xa2)));
        assert_eq!(theme.surface, None);
        assert!(!theme.light);
    }

    #[test]
    fn refuses_a_misspelt_key_or_a_bad_colour() {
        let colours = "background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#ff0000\"\n";
        assert_eq!(Theme::parse("plain", colours).unwrap().name, "plain");
        assert!(Theme::parse("x", &format!("{colours}acent = \"#ff0000\"\n")).is_err());
        assert!(Theme::parse("x", &colours.replace("#ff0000", "red")).is_err());
    }

    #[test]
    fn reads_a_folder_and_names_what_it_could_not() {
        let dir = std::env::temp_dir().join(format!("zendo-themes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(Theme::read_dir(&dir.join("none")), (Vec::new(), Vec::new()));
        let colours = "background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#ff0000\"\n";
        fs::write(dir.join("b.toml"), colours).unwrap();
        fs::write(dir.join("a.toml"), colours).unwrap();
        fs::write(dir.join("broken.toml"), "background = 1\n").unwrap();
        fs::write(dir.join("notes.txt"), "not a theme").unwrap();
        let (themes, problems) = Theme::read_dir(&dir);
        let ids: Vec<&str> = themes.iter().map(|theme| theme.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("broken.toml"), "{problems:?}");
    }
}
