//! The XDG base directories, read from the environment.

use std::env;
use std::path::PathBuf;

pub fn home() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn xdg_home(var: &str, fallback: &str) -> Option<PathBuf> {
    env::var_os(var)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|home| home.join(fallback)))
}

pub fn config_home() -> Option<PathBuf> {
    xdg_home("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> Option<PathBuf> {
    xdg_home("XDG_DATA_HOME", ".local/share")
}

/// `XDG_DATA_DIRS`, or the spec's default when it is unset or empty.
pub fn data_dirs() -> Vec<PathBuf> {
    let value = env::var("XDG_DATA_DIRS").unwrap_or_default();
    let value = if value.trim().is_empty() {
        "/usr/local/share:/usr/share"
    } else {
        value.as_str()
    };
    value
        .split(':')
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .collect()
}

pub fn cache_home() -> Option<PathBuf> {
    xdg_home("XDG_CACHE_HOME", ".cache")
}

pub fn state_home() -> Option<PathBuf> {
    xdg_home("XDG_STATE_HOME", ".local/state")
}

/// Writes through a temporary file renamed into place, so a reader never
/// sees half a file and a crash never leaves one.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)
}
