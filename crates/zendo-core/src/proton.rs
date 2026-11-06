//! The Proton builds on this machine, for running Windows games through
//! umu: Steam's compatibility tools folders (where GE-Proton, UMU-Proton
//! and distribution packages such as proton-cachyos go) and the Protons
//! Steam installed as games.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proton {
    /// The folder's name, as Steam lists it: `GE-Proton10-4`,
    /// `proton-cachyos-slr`, `Proton 9.0 (Beta)`.
    pub name: String,
    pub path: PathBuf,
}

/// Where Steam and the packages put compatibility tools.
fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data) = paths::data_home() {
        dirs.push(data.join("Steam/compatibilitytools.d"));
    }
    if let Some(home) = paths::home() {
        dirs.push(home.join(".steam/root/compatibilitytools.d"));
        dirs.push(home.join(".steam/steam/compatibilitytools.d"));
    }
    dirs.push(PathBuf::from("/usr/share/steam/compatibilitytools.d"));
    dirs.push(PathBuf::from("/usr/local/share/steam/compatibilitytools.d"));
    if let Some(data) = paths::data_home() {
        dirs.push(data.join("Steam/steamapps/common"));
    }
    dirs
}

/// Every Proton found, the one [`default`] picks first, each once.
#[must_use]
pub fn installed() -> Vec<Proton> {
    let mut found: Vec<Proton> = Vec::new();
    for dir in tool_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.join("proton").is_file() {
                continue;
            }
            // `.steam/root` is usually a link to the same Steam folder.
            let real = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if found
                .iter()
                .any(|proton| fs::canonicalize(&proton.path).ok().as_ref() == Some(&real))
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            found.push(Proton { name, path });
        }
    }
    found.sort_by(|a, b| preference(&a.name, &b.name));
    found
}

/// The Proton a game runs with unless told otherwise: the newest GE-Proton
/// or UMU-Proton, which carry the protonfixes umu applies, then a
/// distribution's Proton, then Valve's. `None` lets umu download
/// UMU-Proton itself.
#[must_use]
pub fn default() -> Option<Proton> {
    installed().into_iter().next()
}

/// Finds a Proton by its name, or takes a path as it is.
#[must_use]
pub fn named(name: &str) -> Option<Proton> {
    let path = Path::new(name);
    if path.is_absolute() {
        return path.join("proton").is_file().then(|| Proton {
            name: path.file_name().map_or_else(
                || name.to_owned(),
                |name| name.to_string_lossy().into_owned(),
            ),
            path: path.to_path_buf(),
        });
    }
    installed().into_iter().find(|proton| proton.name == name)
}

fn rank(name: &str) -> u8 {
    let lower = name.to_lowercase();
    if lower.starts_with("ge-proton") || lower.starts_with("umu-proton") {
        0
    } else if lower.contains("cachyos") || lower.contains("tkg") {
        1
    } else if lower.contains("experimental") {
        3
    } else {
        2
    }
}

/// Best first: by [`rank`], then the newest version.
fn preference(a: &str, b: &str) -> Ordering {
    rank(a)
        .cmp(&rank(b))
        .then_with(|| version(b).cmp(&version(a)))
        .then_with(|| a.cmp(b))
}

/// The numbers in a name, in order: `GE-Proton10-4` is `[10, 4]`.
fn version(name: &str) -> Vec<u32> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_the_newest_ge_proton() {
        let mut names = vec![
            "Proton 9.0 (Beta)",
            "proton-cachyos-slr",
            "GE-Proton9-27",
            "Proton - Experimental",
            "GE-Proton10-4",
            "UMU-Proton-9.0-3.2",
        ];
        names.sort_by(|a, b| preference(a, b));
        assert_eq!(
            names,
            [
                "GE-Proton10-4",
                "GE-Proton9-27",
                "UMU-Proton-9.0-3.2",
                "proton-cachyos-slr",
                "Proton 9.0 (Beta)",
                "Proton - Experimental",
            ]
        );
    }
}
