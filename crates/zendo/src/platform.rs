//! The shell's platform side: the tray that hides the window, the controller
//! that sends the frontend the same moves as the keyboard, and the KDE colour
//! scheme, watched as it changes. Each runs on a thread of its own and hands
//! the window events; the tray and the controller themselves are
//! `zendo-shell`'s.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
use zendo_shell::gamepad::{self, Pad};
use zendo_shell::theme::Themes;
use zendo_shell::tray::{self, Tray, TrayMessage};

use crate::commands;

/// How often Zendo looks whether the KDE colour scheme changed.
const LOOK: Duration = Duration::from_secs(2);

/// What the window keeps between calls.
pub struct Shell {
    /// The colours in use, and the choice they come from.
    pub themes: Arc<Mutex<Themes>>,
    /// The setting asks for the tray when the window closes.
    pub wants_tray: AtomicBool,
    /// The tray is there (it was asked for and could be made).
    pub tray: AtomicBool,
}

impl Shell {
    pub fn new(themes: Themes, wants_tray: bool) -> Self {
        Self {
            themes: Arc::new(Mutex::new(themes)),
            wants_tray: AtomicBool::new(wants_tray),
            tray: AtomicBool::new(false),
        }
    }
}

thread_local! {
    /// The tray icon, while there is one: dropping it takes it out. Tauri's
    /// setup and its window events run on the window's thread.
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// The window is hidden for a game's launch, not by the user: the game's end
/// brings it back.
static HID_FOR_GAME: AtomicBool = AtomicBool::new(false);

/// Wires the window: its start size, the tray, the controller and the colour
/// scheme.
pub fn start(app: &AppHandle, window: &WebviewWindow, fullscreen: bool) -> tauri::Result<()> {
    if fullscreen {
        window.set_fullscreen(true)?;
    }
    let there = start_tray(app);
    app.state::<Shell>().tray.store(there, Ordering::Relaxed);
    // With a tray the window is never closed: closing hides it, and the tray
    // brings it back. Without one, closing quits, as before.
    let handle = app.clone();
    let target = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if handle.state::<Shell>().tray.load(Ordering::Relaxed) {
                api.prevent_close();
                let _ = target.hide();
            } else if !crate::downloads::close_requested(&handle) {
                // A store's job runs: the Downloads page asks first.
                api.prevent_close();
            }
        }
    });

    start_pad(app.clone());
    let themes = Arc::clone(&app.state::<Shell>().themes);
    watch_theme(app.clone(), themes);
    Ok(())
}

/// The tray follows the setting: made now when it is turned on, and taken
/// out when it is turned off. Closing quits when there is none.
pub fn set_tray(app: &AppHandle, on: bool) {
    let shell = app.state::<Shell>();
    shell.wants_tray.store(on, Ordering::Relaxed);
    let there = if on {
        if shell.tray.load(Ordering::Relaxed) {
            true
        } else {
            start_tray(app)
        }
    } else {
        TRAY.with(|slot| *slot.borrow_mut() = None);
        false
    };
    shell.tray.store(there, Ordering::Relaxed);
}

/// A game is up: hides the window to the tray, when the setting asks for it
/// and the tray is there.
pub(crate) fn hide_for_launch(app: &AppHandle) {
    if !app.state::<Shell>().tray.load(Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window("main")
        && window.is_visible().unwrap_or(false)
    {
        HID_FOR_GAME.store(true, Ordering::Relaxed);
        let _ = window.hide();
    }
}

/// The game ended: brings back a window hidden for its launch, and only that
/// one (one the user hid to the tray stays there).
pub(crate) fn show_after_game(app: &AppHandle) {
    if !HID_FOR_GAME.swap(false, Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Puts the icon in the tray when the Window setting asks for it; `true` when
/// it is there.
fn start_tray(app: &AppHandle) -> bool {
    if !app.state::<Shell>().wants_tray.load(Ordering::Relaxed) {
        return false;
    }
    let Some(tray) = Tray::spawn() else {
        // No tray in this desktop: closing keeps quitting, so the window is
        // never lost.
        return false;
    };
    TRAY.with(|slot| *slot.borrow_mut() = Some(tray));
    let handle = app.clone();
    tray::events(move |message| {
        let Some(window) = handle.get_webview_window("main") else {
            return false;
        };
        match message {
            TrayMessage::Toggle if window.is_visible().unwrap_or(false) => {
                let _ = window.hide();
            }
            TrayMessage::Toggle | TrayMessage::Open => {
                let _ = window.show();
                let _ = window.set_focus();
            }
            TrayMessage::Quit => {
                // A store's job runs: the Downloads page asks first.
                if crate::downloads::close_requested(&handle) {
                    handle.exit(0);
                }
            }
        }
        true
    });
    true
}

/// Hands what the controller asks for to the frontend, which moves the same
/// cursor the keyboard does.
fn start_pad(app: AppHandle) {
    gamepad::events(move |pad| app.emit("pad", pad_name(pad)).is_ok());
}

/// A pad's move, as the frontend hears it.
fn pad_name(pad: Pad) -> &'static str {
    match pad {
        Pad::Left => "left",
        Pad::Right => "right",
        Pad::Up => "up",
        Pad::Down => "down",
        Pad::Activate => "activate",
        Pad::Back => "back",
        Pad::Sort => "sort",
        Pad::Hide => "hide",
        Pad::ShowHidden => "show-hidden",
        Pad::ToggleGameMode => "toggle-game-mode",
        Pad::PrevShelf => "prev-shelf",
        Pad::NextShelf => "next-shelf",
    }
}

/// Looks at `kdeglobals` every so often, and says so when the colours changed
/// while the theme is the KDE scheme. The thread ends with the process.
fn watch_theme(app: AppHandle, themes: Arc<Mutex<Themes>>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(LOOK);
            let Ok(mut themes) = themes.lock() else {
                return;
            };
            if themes.look() {
                let _ = app.emit("theme_changed", commands::palette(&themes.palette()));
            }
        }
    });
}
