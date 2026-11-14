//! The parts of Zendo's window that need no toolkit, so the Iced window and
//! the Slint one can share them: the window's words ([`i18n`]), the tray,
//! controllers, `nxm://` links, Discord's presence, and the two helper
//! windows that run as processes of their own (the sign-in window and the
//! file dialog, which need GTK's loop apart from the toolkit's).
//!
//! What runs on a thread of its own hands its events to a closure, which
//! returns `false` once nothing wants them any more; each toolkit turns
//! that into its own kind of stream or event.

pub mod cli;
pub mod file_dialog;
pub mod format;
pub mod gamepad;
pub mod i18n;
pub mod links;
pub mod presence;
pub mod sign_in_window;
pub mod tray;

/// Used by [`t!`]'s expansion in the crates that call it.
#[doc(hidden)]
pub use fluent_bundle;

/// The application id: the window's class on X11 and its app id on Wayland,
/// which the compositor's rules (blur, for one) match on.
pub const APP_ID: &str = "zendo";
