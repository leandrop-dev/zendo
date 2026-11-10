//! The window's colours: a theme file, Zendo's own by default, or, on a
//! computer with hyprmonk, hyprmonk's tone, followed as it changes.
//! Settings' Theme section chooses.
//!
//! The built-in themes are the files in `crates/zendo/themes/`, listed in
//! [`BUILT_IN`]; the user's own are read from `~/.config/zendo/themes/`
//! (see [`zendo_core::theme`]), and one named as a built-in replaces it.

use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, row, text};
use iced::{Element, Task};
use zendo_core::theme::Theme;
use zendo_core::tone::Tone;

use super::settings::section;
use super::style::{self, Colors, colors};
use super::{Message, Zendo};
use crate::i18n::t;

/// How often Zendo looks whether hyprmonk wrote a new tone.
const LOOK: Duration = Duration::from_secs(2);

/// The themes built into Zendo: each file's name and text. Zendo's own
/// comes first; a new theme is a file in `themes/` and a line here.
const BUILT_IN: &[(&str, &str)] = &[("zendo", include_str!("../../themes/zendo.toml"))];

/// The theme used when none is chosen, or the one chosen is gone.
const DEFAULT: &str = "zendo";

/// What `config.toml`'s `theme` names for hyprmonk's tone.
const HYPRMONK: &str = "hyprmonk";

/// A theme Settings offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A theme file, by its name.
    File(String),
    /// hyprmonk's tone when it writes one, else Zendo's own.
    Hyprmonk,
}

impl Default for Choice {
    fn default() -> Self {
        Self::File(DEFAULT.to_owned())
    }
}

impl Choice {
    /// What `config.toml`'s `theme` says for it.
    pub fn tag(&self) -> &str {
        match self {
            Self::File(id) => id,
            Self::Hyprmonk => HYPRMONK,
        }
    }

    /// The one the settings name; Zendo's own for none.
    pub fn chosen(tag: Option<&str>) -> Self {
        match tag {
            Some(HYPRMONK) => Self::Hyprmonk,
            Some(id) if !id.trim().is_empty() => Self::File(id.to_owned()),
            _ => Self::default(),
        }
    }
}

/// What the window shows, beyond the theme's name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum Found {
    /// A theme file, as chosen.
    #[default]
    File,
    /// The chosen theme file is not there: Zendo's own shows.
    Gone(String),
    /// No `tone.json`: hyprmonk writes none yet.
    Missing,
    /// hyprmonk's tone, by its name.
    Following(String),
    /// Why the tone could not be read; the colours stay as they were.
    Failed(String),
}

#[derive(Debug, Default)]
pub struct Themes {
    pub choice: Choice,
    /// The built-in themes and the user's, by name.
    files: Vec<Theme>,
    /// A line for each of the user's files that is not a theme.
    problems: Vec<String>,
    found: Found,
    /// When `tone.json` last changed, as last read.
    modified: Option<SystemTime>,
}

/// The built-in themes, with the user's from `dir` over them, by name; and
/// a line for each file that is not a theme.
fn all_themes(dir: Option<&Path>) -> (Vec<Theme>, Vec<String>) {
    let mut themes: Vec<Theme> = BUILT_IN
        .iter()
        .filter_map(|(id, text)| Theme::parse(id, text).ok())
        .collect();
    let (own, problems) = dir.map(Theme::read_dir).unwrap_or_default();
    for theme in own {
        match themes.iter_mut().find(|known| known.id == theme.id) {
            Some(known) => *known = theme,
            None => themes.push(theme),
        }
    }
    // Zendo's own first, then by name.
    themes.sort_by_key(|theme| (theme.id != DEFAULT, theme.name.to_lowercase()));
    (themes, problems)
}

impl Themes {
    /// The settings' theme, from the built-in themes and the user's in
    /// `dir`, painted on the window.
    pub fn chosen(tag: Option<&str>, dir: Option<&Path>) -> Self {
        let (files, problems) = all_themes(dir);
        let mut themes = Self {
            choice: Choice::chosen(tag),
            files,
            problems,
            ..Self::default()
        };
        themes.apply();
        themes
    }

    /// Reads the user's themes again, as Settings opens: a file added
    /// since shows. The chosen one is applied again, edits and all.
    pub fn reload(&mut self, dir: Option<&Path>) {
        (self.files, self.problems) = all_themes(dir);
        if matches!(self.choice, Choice::File(_)) {
            self.apply();
        }
    }

    /// Sets the window's colours from the choice, reading the tone again
    /// when it needs it.
    pub(super) fn apply(&mut self) {
        self.modified = None;
        let id = match &self.choice {
            Choice::File(id) => id.clone(),
            Choice::Hyprmonk => return self.apply_tone(),
        };
        if let Some(theme) = self.files.iter().find(|theme| theme.id == id) {
            style::set_colors(Colors::from_theme(theme));
            self.found = Found::File;
        } else {
            style::set_colors(self.default_colors());
            self.found = Found::Gone(id);
        }
    }

    fn apply_tone(&mut self) {
        self.modified = Tone::modified();
        match Tone::read() {
            Ok(Some(tone)) => {
                style::set_colors(Colors::from_theme(&Theme::from_tone(&tone)));
                self.found = Found::Following(tone.label);
            }
            Ok(None) => {
                style::set_colors(self.default_colors());
                self.found = Found::Missing;
            }
            // Perhaps caught half written: looked at again next time.
            Err(err) => {
                self.modified = None;
                self.found = Found::Failed(format!("{err:#}"));
            }
        }
    }

    /// Zendo's own, from its file (the user's, when it replaces it).
    fn default_colors(&self) -> Colors {
        self.files
            .iter()
            .find(|theme| theme.id == DEFAULT)
            .map_or(Colors::ZENDO, Colors::from_theme)
    }

    /// Whether the window follows hyprmonk, so the tone is watched.
    pub fn watching(&self) -> bool {
        self.choice == Choice::Hyprmonk
    }

    /// A look at `tone.json`: a new one repaints the window.
    pub fn look(&mut self) {
        if self.watching() && Tone::modified() != self.modified {
            self.apply();
        }
    }

    /// What the window shows, for Settings, and whether all is well.
    fn note(&self) -> Option<(String, bool)> {
        match &self.found {
            Found::File => None,
            Found::Gone(id) => Some((t!("theme-gone", theme = id.as_str()), false)),
            Found::Missing => Some((t!("theme-missing"), true)),
            Found::Following(label) => Some((t!("theme-following", tone = label.as_str()), true)),
            Found::Failed(err) => Some((t!("theme-failed", error = err.as_str()), false)),
        }
    }
}

impl Zendo {
    pub(super) fn set_theme(&mut self, choice: &Choice) -> Task<Message> {
        let task = self.save_setting(super::settings::Setting::Theme(Some(
            choice.tag().to_owned(),
        )));
        self.themes.choice = Choice::chosen(self.config.theme.as_deref());
        self.themes.apply();
        task
    }

    pub(super) fn look_at_tone(&mut self) -> Task<Message> {
        self.themes.look();
        Task::none()
    }

    pub(super) fn theme_section(&self) -> Element<'_, Message> {
        let themes = &self.themes;
        let chip = |label: String, choice: Choice| {
            button(text(label).size(14))
                .padding([5, 12])
                .style(style::chip(themes.choice == choice))
                .on_press(Message::SetTheme(choice))
        };
        let mut choices = row![].spacing(8);
        for theme in &themes.files {
            choices = choices.push(chip(theme.name.clone(), Choice::File(theme.id.clone())));
        }
        // hyprmonk's only where it is, or already chosen: elsewhere its tone
        // cannot come.
        if themes.choice == Choice::Hyprmonk || Tone::hyprmonk_found() {
            choices = choices.push(chip(t!("theme-hyprmonk"), Choice::Hyprmonk));
        }
        let colors = colors();
        let mut body = column![choices.wrap().vertical_spacing(8)].spacing(10);
        if let Some((note, fine)) = themes.note() {
            body = body.push(
                text(note)
                    .size(13)
                    .color(if fine { colors.dim } else { colors.bad }),
            );
        }
        for problem in &themes.problems {
            body = body.push(
                text(t!("theme-not-read", problem = problem.as_str()))
                    .size(13)
                    .color(colors.bad),
            );
        }
        let folder = Theme::dir().map_or_else(String::new, |dir| dir.display().to_string());
        section(t!("theme-title"), t!("theme-about", folder = folder), body)
    }
}

/// A tick every [`LOOK`], from a thread, for as long as it is listened to.
pub fn ticks() -> impl Stream<Item = ()> {
    iced::stream::channel(1, async |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            while sender.unbounded_send(()).is_ok() {
                thread::sleep(LOOK);
            }
        });
        while let Some(()) = receiver.next().await {
            if output.send(()).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn reads_the_settings_choice() {
        assert_eq!(Choice::chosen(None), Choice::File("zendo".into()));
        assert_eq!(Choice::chosen(Some("hyprmonk")), Choice::Hyprmonk);
        assert_eq!(Choice::chosen(Some("dusk")), Choice::File("dusk".into()));
        assert_eq!(Choice::chosen(Some(" ")), Choice::File("zendo".into()));
    }

    /// Every built-in theme reads, and Zendo's own file holds exactly the
    /// colours the window starts with.
    #[test]
    fn the_built_in_themes_read() {
        for (id, text) in BUILT_IN {
            Theme::parse(id, text).unwrap_or_else(|err| panic!("themes/{id}.toml: {err:#}"));
        }
        let (themes, problems) = all_themes(None);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(themes[0].id, "zendo");
        assert_eq!(Colors::from_theme(&themes[0]), Colors::ZENDO);
    }

    /// The user's file replaces a built-in of its name and adds the rest;
    /// a chosen file that is gone shows Zendo's own and says so.
    #[test]
    fn the_users_themes_join_the_built_in_ones() {
        let dir = std::env::temp_dir().join(format!("zendo-window-themes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let colours = "background = \"#000000\"\nforeground = \"#ffffff\"\naccent = \"#ff0000\"\n";
        fs::write(
            dir.join("zendo.toml"),
            format!("name = \"Mine\"\n{colours}"),
        )
        .unwrap();
        fs::write(
            dir.join("aurora.toml"),
            format!("name = \"Aurora\"\n{colours}"),
        )
        .unwrap();
        let (themes, problems) = all_themes(Some(&dir));
        assert_eq!(problems, Vec::<String>::new());
        let names: Vec<&str> = themes.iter().map(|theme| theme.name.as_str()).collect();
        assert_eq!(names, ["Mine", "Aurora"]);

        let gone = Themes::chosen(Some("dusk"), None);
        assert_eq!(gone.found, Found::Gone("dusk".into()));
        assert_eq!(gone.default_colors(), Colors::ZENDO);
        assert!(!gone.watching());
    }

    /// hyprmonk's Noir is the tone Zendo's own colours come from: mixed
    /// from it, they come out within a shade of them.
    #[test]
    fn mixes_a_tone_as_zendos_own() {
        let noir = Tone::parse(
            r##"{"version": 1, "name": "noir", "label": "Noir",
                "background": "#171717", "foreground": "#ccd0cf", "accent": "#f25623"}"##,
        )
        .unwrap();
        let mixed = Colors::from_theme(&Theme::from_tone(&noir));
        let own = Colors::ZENDO;
        let close = |a: iced::Color, b: iced::Color| {
            [(a.r, b.r), (a.g, b.g), (a.b, b.b)]
                .iter()
                .all(|(x, y)| (x - y).abs() <= 3.0 / 255.0)
        };
        for (name, a, b) in [
            ("background", mixed.background, own.background),
            ("sidebar", mixed.sidebar, own.sidebar),
            ("surface", mixed.surface, own.surface),
            ("raised", mixed.raised, own.raised),
            ("line", mixed.line, own.line),
            ("text", mixed.text, own.text),
            ("dim", mixed.dim, own.dim),
            ("accent", mixed.accent, own.accent),
        ] {
            assert!(close(a, b), "{name}: {a:?} against {b:?}");
        }
        assert_eq!(mixed.good, own.good, "no positive: Zendo's green");
    }
}
