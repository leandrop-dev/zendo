//! The window: opened with Zendo's class, painted from the theme, hidden to
//! the tray and brought back from it. What the Iced window's `app::run` and
//! its tray handling do, in Slint.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use anyhow::Context;
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use zendo_core::config::Config;
use zendo_core::theme::{Rgb, Theme};
use zendo_shell::tray::{self, Tray, TrayMessage};
use zendo_shell::{APP_ID, i18n};

use crate::theme::{Palette, Themes};
use crate::{Colors, MainWindow, library_ui, tr};

/// How often Zendo looks whether the KDE colour scheme changed.
const LOOK: Duration = Duration::from_secs(2);

/// Opens the window; `fullscreen` is Game Mode's start (item 53) and `link`
/// an `nxm://` link it was started for (item 52).
#[expect(
    clippy::needless_pass_by_value,
    reason = "the signature is `cli::RunWindow`'s, and the Nexus pages (item 52) take the link"
)]
pub fn run(fullscreen: bool, link: Option<String>) -> anyhow::Result<()> {
    i18n::set_from_settings();
    let config = Config::load_file().ok();
    let opaque = config.as_ref().is_some_and(|config| config.opaque);
    let hide_to_tray = config
        .as_ref()
        .is_some_and(|config| config.minimize_to_tray);
    let theme = config.and_then(|config| config.theme);

    // The backend first: the app id, which the compositor matches the window
    // by (the class `zendo`, which blur rules name), needs one.
    slint::BackendSelector::new()
        .select()
        .context("no window system to open the window on")?;
    slint::set_xdg_app_id(APP_ID).context("could not set the window's class")?;

    let window = MainWindow::new().context("could not make the window")?;
    tr::install(&window);
    let themes = Rc::new(RefCell::new(Themes::chosen(
        theme.as_deref(),
        Theme::dir().as_deref(),
    )));
    library_ui::install(&window, Rc::clone(&themes));
    if opaque {
        window.set_translucency(1.0);
    }
    paint(&window, themes.borrow().palette());
    let watcher = follow_scheme(&window, &themes);
    if fullscreen {
        window.window().set_fullscreen(true);
    }
    // Opened by a mod page's `nxm://` link: the Nexus pages come in item 52.
    let _ = &link;

    // The tray is there only when the setting asks for it and it could be
    // made; then closing the window hides it, and the tray brings it back.
    set_tray(&window, hide_to_tray);
    let weak = window.as_weak();
    window.window().on_close_requested(move || {
        // Read at the close, as Settings can turn the tray on and off.
        if !TRAY_THERE.get() {
            // Closing for good while a download runs asks first.
            if !library_ui::quit_requested() {
                return CloseRequestResponse::KeepWindowShown;
            }
            let _ = slint::quit_event_loop();
        } else if let Some(window) = weak.upgrade() {
            remember_size(&window);
        }
        CloseRequestResponse::HideWindow
    });

    window.show().context("could not show the window")?;
    // Until `quit_event_loop`: hiding the last window to the tray must not
    // end the program.
    slint::run_event_loop_until_quit().context("the window's event loop failed")?;
    drop(watcher);
    Ok(())
}

thread_local! {
    /// The tray icon, while there is one: dropping it takes it out.
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
    /// The tray's messages are being heard.
    static TRAY_HEARD: Cell<bool> = const { Cell::new(false) };
}

/// Puts the icon in the tray, or takes it out (Settings' Window switch).
/// Closing the window hides it only while the icon is there; one hidden when
/// the icon goes is shown again, so the window is never lost.
pub fn set_tray(window: &MainWindow, on: bool) {
    let tray = if on { Tray::spawn() } else { None };
    let there = tray.is_some();
    TRAY.replace(tray);
    TRAY_THERE.set(there);
    if there && !TRAY_HEARD.replace(true) {
        let weak = window.as_weak();
        tray::events(move |message| {
            weak.upgrade_in_event_loop(move |window| handle_tray(&window, message))
                .is_ok()
        });
    }
    if !there && !window.window().is_visible() {
        let _ = window.show();
    }
}

thread_local! {
    /// The size the window had when it was last hidden to the tray. Only
    /// the window's own thread asks, so it need not be shared.
    static LAST_SIZE: Cell<Option<slint::LogicalSize>> = const { Cell::new(None) };
}

thread_local! {
    /// The tray is there (the setting asked for it and it was made).
    static TRAY_THERE: Cell<bool> = const { Cell::new(false) };
}

/// A game is up: hides the window to the tray, when the Window setting asks
/// for it and the tray is there.
pub fn hide_for_launch(window: &MainWindow) {
    if TRAY_THERE.get() && window.window().is_visible() {
        remember_size(window);
        let _ = window.hide();
    }
}

/// Notes the window's size, for the window that comes back: a Wayland app
/// cannot unminimise, so hiding closes the window and showing makes a new
/// one, which would open at the default size.
fn remember_size(window: &MainWindow) {
    let size = window
        .window()
        .size()
        .to_logical(window.window().scale_factor());
    LAST_SIZE.set(Some(size));
}

/// A click or a menu item from the tray.
fn handle_tray(window: &MainWindow, message: TrayMessage) {
    match message {
        TrayMessage::Toggle if window.window().is_visible() => {
            remember_size(window);
            let _ = window.hide();
        }
        TrayMessage::Toggle | TrayMessage::Open => {
            // Already showing: nothing to bring back.
            if !window.window().is_visible() {
                if let Some(size) = LAST_SIZE.get() {
                    window.window().set_size(size);
                }
                let _ = window.show();
            }
        }
        TrayMessage::Quit => {
            if library_ui::quit_requested() {
                let _ = slint::quit_event_loop();
            } else if !window.window().is_visible() {
                // The question is in the window, so the window comes back to
                // ask it.
                if let Some(size) = LAST_SIZE.get() {
                    window.window().set_size(size);
                }
                let _ = window.show();
            }
        }
    }
}

/// Looks at `kdeglobals` every so often, and repaints when it changed while
/// the theme is the KDE scheme (`look` does nothing otherwise: Settings can
/// choose System at any time). The timer lives as long as the value.
fn follow_scheme(window: &MainWindow, themes: &Rc<RefCell<Themes>>) -> Timer {
    let timer = Timer::default();
    let (weak, themes) = (window.as_weak(), Rc::clone(themes));
    timer.start(TimerMode::Repeated, LOOK, move || {
        let changed = themes.borrow_mut().look();
        if changed && let Some(window) = weak.upgrade() {
            paint(&window, themes.borrow().palette());
            library_ui::scheme_followed();
        }
    });
    timer
}

/// Sets the window's colours.
pub fn paint(window: &MainWindow, palette: Palette) {
    let colors = window.global::<Colors>();
    colors.set_background(colour(palette.background));
    colors.set_sidebar(colour(palette.sidebar));
    colors.set_surface(colour(palette.surface));
    colors.set_raised(colour(palette.raised));
    colors.set_line(colour(palette.line));
    colors.set_foreground(colour(palette.foreground));
    colors.set_dim(colour(palette.dim));
    colors.set_accent(colour(palette.accent));
    colors.set_good(colour(palette.good));
    colors.set_bad(colour(palette.bad));
    colors.set_light(palette.light);
}

fn colour(rgb: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.0, rgb.1, rgb.2)
}
