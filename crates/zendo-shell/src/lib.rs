//! The parts of Zendo's window that need no toolkit, kept apart from the
//! Slint code that draws it: the window's words ([`i18n`]), the tray,
//! controllers, `nxm://` links, Discord's presence, and the two helper
//! windows that run as processes of their own (the sign-in window and the
//! file dialog, which need GTK's loop apart from the toolkit's).
//!
//! What runs on a thread of its own hands its events to a closure, which
//! returns `false` once nothing wants them any more; each toolkit turns
//! that into its own kind of stream or event.

pub mod artwork;
pub mod cards;
pub mod cli;
pub mod clipboard;
pub mod collections;
pub mod dlc;
pub mod file_dialog;
pub mod format;
pub mod game;
pub mod game_mode;
pub mod gamepad;
pub mod gog;
pub mod i18n;
pub mod installer;
pub mod launch_options;
pub mod library;
pub mod links;
pub mod mods;
pub mod nexus;
pub mod nexus_search;
pub mod open;
pub mod prefix;
pub mod presence;
pub mod queue;
pub mod running;
pub mod sign_in_window;
pub mod steam_installs;
pub mod steam_list;
pub mod store;
pub mod store_facts;
pub mod theme;
pub mod tray;

/// Used by [`t!`]'s expansion in the crates that call it.
#[doc(hidden)]
pub use fluent_bundle;

/// The application id: the window's class on X11 and its app id on Wayland,
/// which the compositor's rules (blur, for one) match on.
pub const APP_ID: &str = "zendo";
