//! Epic's EOS overlay (friends, achievements' pop-ups, the store over the
//! game), which legendary installs once (`eos-overlay install`, into
//! Zendo's folder here) and turns on or off in a Wine prefix (`enable` or
//! `disable --prefix`), by writing or removing `OverlayPath` under
//! `Software\Epic Games\EOS` in the prefix's `user.reg`. Zendo turns it
//! on or off before an Epic game starts, as its launch options say.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::paths;
use crate::stores::Store;

/// The `user.reg` section legendary writes the overlay's path into.
const SECTION: &str = "[Software\\\\Epic Games\\\\EOS]";

/// Where Zendo has legendary install it.
#[must_use]
pub fn folder() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo/eos-overlay"))
}

/// Whether it is installed in Zendo's folder.
#[must_use]
pub fn installed() -> bool {
    folder()
        .and_then(|folder| fs::read_dir(folder).ok())
        .is_some_and(|mut entries| entries.next().is_some())
}

/// Installs it, or updates it when installed. It blocks while legendary
/// downloads it.
///
/// # Errors
///
/// When legendary is missing or fails.
pub fn install() -> anyhow::Result<()> {
    let folder = folder().context("no home folder to install the overlay in")?;
    if installed() {
        Store::Epic.run_checked(&["-y", "eos-overlay", "update"])?;
    } else {
        let path = folder.to_string_lossy();
        Store::Epic.run_checked(&["-y", "eos-overlay", "install", "--path", &path])?;
    }
    Ok(())
}

/// Removes it.
///
/// # Errors
///
/// When legendary is missing or fails.
pub fn remove() -> anyhow::Result<()> {
    Store::Epic.run_checked(&["-y", "eos-overlay", "remove"])?;
    Ok(())
}

/// Whether it is on in the prefix: its `user.reg` has the overlay's path.
#[must_use]
pub fn enabled_in(prefix: &Path) -> bool {
    fs::read_to_string(prefix.join("user.reg")).is_ok_and(|registry| has_overlay(&registry))
}

fn has_overlay(registry: &str) -> bool {
    let mut inside = false;
    for line in registry.lines() {
        if line.starts_with('[') {
            inside = line
                .get(..SECTION.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(SECTION));
        } else if inside && line.starts_with("\"OverlayPath\"=") {
            return true;
        }
    }
    false
}

/// Turns it on or off in the prefix, when that changes anything; nothing
/// before the prefix's first start, which makes its `user.reg`.
///
/// # Errors
///
/// When it is to be turned on but is not installed, or legendary fails.
pub fn set_in(prefix: &Path, on: bool) -> anyhow::Result<()> {
    if !prefix.join("user.reg").is_file() || enabled_in(prefix) == on {
        return Ok(());
    }
    if on && !installed() {
        anyhow::bail!("the EOS overlay is not installed: install it in Settings");
    }
    let prefix = prefix.to_string_lossy();
    let action = if on { "enable" } else { "disable" };
    Store::Epic.run_checked(&["-y", "eos-overlay", action, "--prefix", &prefix])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_overlays_path_in_its_section_only() {
        let on = "WINE REGISTRY Version 2\n\n\
            [Software\\\\Epic Games\\\\EOS] 1731000000\n\
            #time=1db\n\
            \"OverlayPath\"=\"Z:/home/me/.local/share/zendo/eos-overlay\"\n\n\
            [Software\\\\Wine] 1731000000\n";
        assert!(has_overlay(on));
        let elsewhere = "[Software\\\\Other] 1\n\"OverlayPath\"=\"x\"\n";
        assert!(!has_overlay(elsewhere));
        assert!(!has_overlay(""));
        // Nothing to do in a prefix not started yet.
        let missing = std::env::temp_dir().join("zendo-no-such-prefix");
        assert!(set_in(&missing, true).is_ok());
    }
}
