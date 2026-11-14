//! Opens a folder, a file or a web page in the program the desktop chooses
//! for it.

use std::process::{Command, Stdio};
use std::thread;

/// Hands `target` (a path or a link) to `xdg-open`, which runs on in the
/// background.
///
/// # Errors
///
/// When `xdg-open` could not be started.
pub fn target(target: &str) -> Result<(), String> {
    let mut child = Command::new("xdg-open")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not open {target}: {err}"))?;
    thread::spawn(move || child.wait());
    Ok(())
}
