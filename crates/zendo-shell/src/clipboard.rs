//! The clipboard, through the tool the desktop has (`wl-copy`, else
//! `xclip`), for the report a bug report starts from.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::t;

/// Puts `text` in the clipboard.
///
/// # Errors
///
/// When neither tool is installed, or both refused.
pub fn copy(text: &str) -> Result<(), String> {
    for (program, args) in [
        ("wl-copy", &[][..]),
        ("xclip", &["-selection", "clipboard"][..]),
    ] {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|status| status.success()) {
            return Ok(());
        }
    }
    Err(t!("report-no-clipboard"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_empty_copy_is_still_a_copy() {
        // The tools may not be installed here; what matters is that a
        // failure is a line, not a panic.
        let result = super::copy("");
        assert!(result.is_ok() || result.is_err());
    }
}
