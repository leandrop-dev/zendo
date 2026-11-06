//! Choosing a file in the desktop's own dialog: `zendo pick-file <title>`,
//! a second process like the sign-in window, since GTK needs its own loop
//! apart from Iced's. GTK's native chooser goes through the desktop portal
//! when there is one, so it is the dialog every other app shows. The path
//! chosen goes to stdout; closing it writes nothing.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, bail};
use gtk::prelude::*;

/// `zendo pick-file <title>`: shows the dialog, prints the file chosen.
pub fn main(title: &str) -> ExitCode {
    gtk::glib::set_prgname(Some(crate::app::APP_ID));
    gtk::glib::set_application_name("Zendo");
    if let Err(err) = gtk::init() {
        eprintln!("zendo: could not start GTK: {err}");
        return ExitCode::FAILURE;
    }
    let dialog = gtk::FileChooserNative::new(
        Some(title),
        None::<&gtk::Window>,
        gtk::FileChooserAction::Open,
        Some("Choose"),
        Some("Cancel"),
    );
    if let Some(home) = std::env::var_os("HOME") {
        let games = PathBuf::from(home).join("Games");
        let start = if games.is_dir() {
            games
        } else {
            games.parent().map(PathBuf::from).unwrap_or_default()
        };
        let _ = dialog.set_current_folder(start);
    }
    if dialog.run() != gtk::ResponseType::Accept {
        return ExitCode::SUCCESS;
    }
    let Some(path) = dialog.file().and_then(|file| file.path()) else {
        return ExitCode::SUCCESS;
    };
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{}", path.display())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Opens the dialog and waits for it: the file chosen, or `None` when it
/// was closed. It blocks; the window calls it on a thread.
///
/// # Errors
///
/// When the dialog cannot be started or fails.
pub fn pick(title: &str) -> anyhow::Result<Option<PathBuf>> {
    let exe = std::env::current_exe().context("could not find Zendo's own program")?;
    let output = Command::new(exe)
        .args(["pick-file", title])
        .stdin(Stdio::null())
        .output()
        .context("could not open the file dialog")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("zendo: "))
            .map_or_else(|| output.status.to_string(), str::to_owned);
        bail!("the file dialog failed: {reason}");
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_owned();
    Ok((!path.is_empty()).then(|| PathBuf::from(path)))
}
