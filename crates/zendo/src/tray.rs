//! The system tray icon, for hiding the window to the tray instead of
//! quitting: Zendo's icon, an Open and a Quit, and a left click that shows
//! or hides the window. It runs through tray-icon's ksni backend, the
//! freedesktop `StatusNotifierItem` that Waybar and most panels read; that
//! backend keeps a thread of its own, so the window's thread asks it
//! nothing. With no tray (stock GNOME without an extension), Settings'
//! Window section is left off, and this is never created.

use std::thread;
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, TrayIconBuilder, TrayIconEvent};

use crate::i18n::t;

/// The icon's size, in pixels, both ways.
const SIZE: u32 = 64;

/// The icon, as PNG bytes built into the binary (the same art as the
/// desktop icon, rasterised from `data/zendo.svg`).
const ICON_PNG: &[u8] = include_bytes!("../tray-icon.png");

/// Something the tray asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMessage {
    /// The icon was left-clicked: show the window, or hide it.
    Toggle,
    /// The Open menu item.
    Open,
    /// The Quit menu item.
    Quit,
}

/// The tray icon, kept in the tray for as long as this value lives.
pub struct Tray {
    /// Kept alive: dropping it takes the icon out of the tray.
    _icon: tray_icon::TrayIcon,
    /// Kept alive beside the icon; the backend holds its own copy too.
    _menu: Menu,
}

impl Tray {
    /// Puts Zendo's icon and menu in the tray. `None` when the icon or the
    /// menu cannot be made.
    #[must_use]
    pub fn spawn() -> Option<Self> {
        let rgba = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
        let icon = Icon::from_rgba(rgba.into_raw(), SIZE, SIZE).ok()?;
        let open = MenuItem::with_id("open", t!("tray-open"), true, None);
        let quit = MenuItem::with_id("quit", t!("tray-quit"), true, None);
        let menu = Menu::with_items(&[&open, &quit]).ok()?;
        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("Zendo")
            .with_title("Zendo")
            .build()
            .ok()?;
        Some(Self {
            _icon: icon,
            _menu: menu,
        })
    }
}

/// The tray's messages, as they come: left clicks and the menu's items.
pub fn events() -> impl Stream<Item = TrayMessage> {
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            loop {
                if let Ok(TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                }) = TrayIconEvent::receiver().try_recv()
                    && sender.unbounded_send(TrayMessage::Toggle).is_err()
                {
                    return;
                }
                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    let message = match event.id().as_ref() {
                        "open" => TrayMessage::Open,
                        "quit" => TrayMessage::Quit,
                        _ => continue,
                    };
                    if sender.unbounded_send(message).is_err() {
                        return;
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
    })
}
