//! The commands the frontend calls, each a thin wrapper over `zendo-core` or
//! `zendo-shell`, with results typed for the frontend.

use std::collections::BTreeMap;

use serde::Serialize;
use tauri::{State, WebviewWindow};
use zendo_core::theme::Rgb;
use zendo_shell::fluent_bundle::{FluentArgs, FluentValue};
use zendo_shell::i18n::{self, Language};
use zendo_shell::theme::Palette;

use crate::platform::Shell;

/// Where Zendo's source is, for About's buttons.
const REPOSITORY: &str = "https://github.com/leandrop-dev/zendo";

/// Everything the shell needs when it starts: its words, its colours, and
/// how many languages and which one.
#[derive(Debug, Serialize)]
pub struct Bootstrap {
    version: String,
    /// "Version 1.0.1", in the window's words.
    version_line: String,
    /// Where the source is, for About's buttons.
    repository: String,
    language: String,
    languages: Vec<LanguageJson>,
    /// The current language's messages that take no values, by id. Messages
    /// with values go through [`tr`].
    messages: BTreeMap<String, String>,
    palette: PaletteJson,
    fullscreen: bool,
    /// Whether closing the window hides it to the tray.
    tray: bool,
    /// The FPS harness is on (`ZENDO_FPS` in the environment).
    fps: bool,
}

#[derive(Debug, Serialize)]
struct LanguageJson {
    tag: String,
    name: String,
}

/// The window's colours, as CSS can use them.
#[derive(Debug, Clone, Serialize)]
pub struct PaletteJson {
    background: String,
    sidebar: String,
    surface: String,
    raised: String,
    line: String,
    foreground: String,
    dim: String,
    accent: String,
    good: String,
    bad: String,
    /// Text drawn on the accent colour.
    on_accent: String,
    /// A light theme: dark text on a light background.
    light: bool,
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its state and window by value"
)]
pub fn bootstrap(shell: State<'_, Shell>, window: WebviewWindow) -> Result<Bootstrap, String> {
    let themes = shell
        .themes
        .lock()
        .map_err(|_| "the theme could not be read".to_owned())?;
    Ok(Bootstrap {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        version_line: zendo_shell::t!("about-version", version = env!("CARGO_PKG_VERSION")),
        repository: REPOSITORY.to_owned(),
        language: i18n::current().tag().to_owned(),
        languages: Language::ALL
            .iter()
            .map(|language| LanguageJson {
                tag: language.tag().to_owned(),
                name: language.name().to_owned(),
            })
            .collect(),
        messages: i18n::current_catalogue().into_iter().collect(),
        palette: palette(&themes.palette()),
        fullscreen: window.is_fullscreen().map_err(|err| err.to_string())?,
        tray: shell.tray.load(std::sync::atomic::Ordering::Relaxed),
        fps: std::env::var_os("ZENDO_FPS").is_some(),
    })
}

/// A message with values: `tr("count-games", { count: 3 })`.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]
pub fn tr(id: String, args: Option<BTreeMap<String, serde_json::Value>>) -> String {
    let mut fluent = FluentArgs::new();
    let mut given = false;
    for (name, value) in args.unwrap_or_default() {
        fluent.set(name, fluent_value(&value));
        given = true;
    }
    i18n::tr(&id, if given { Some(&fluent) } else { None })
}

/// F11, and the pad's Select for now: full screen on or off, and which.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its window by value"
)]
pub fn window_fullscreen_toggle(window: WebviewWindow) -> Result<bool, String> {
    let fullscreen = window.is_fullscreen().map_err(|err| err.to_string())?;
    window
        .set_fullscreen(!fullscreen)
        .map_err(|err| err.to_string())?;
    Ok(!fullscreen)
}

/// Opens a link with the desktop's browser, for the store rows that carry one.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its argument by value"
)]
pub fn open_link(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("{url:?} is not a web address"));
    }
    zendo_shell::open::target(&url)
}

/// A value the frontend sends, as Fluent takes it.
fn fluent_value(value: &serde_json::Value) -> FluentValue<'static> {
    match value {
        serde_json::Value::String(text) => FluentValue::from(text.clone()),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map_or_else(|| FluentValue::from(number.to_string()), FluentValue::from),
        other => FluentValue::from(other.to_string()),
    }
}

/// The palette as CSS colours.
pub fn palette(palette: &Palette) -> PaletteJson {
    PaletteJson {
        background: hex(palette.background),
        sidebar: hex(palette.sidebar),
        surface: hex(palette.surface),
        raised: hex(palette.raised),
        line: hex(palette.line),
        foreground: hex(palette.foreground),
        dim: hex(palette.dim),
        accent: hex(palette.accent),
        good: hex(palette.good),
        bad: hex(palette.bad),
        on_accent: hex(on_accent(palette)),
        light: palette.light,
    }
}

fn hex(rgb: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

/// Dark or light text on the accent colour: whichever of the background and
/// the foreground is further from it.
fn on_accent(palette: &Palette) -> Rgb {
    let accent = luminance(palette.accent);
    if (accent - luminance(palette.background)).abs()
        >= (accent - luminance(palette.foreground)).abs()
    {
        palette.background
    } else {
        palette.foreground
    }
}

fn luminance(rgb: Rgb) -> f32 {
    (0.2126 * f32::from(rgb.0) + 0.7152 * f32::from(rgb.1) + 0.0722 * f32::from(rgb.2)) / 255.0
}
