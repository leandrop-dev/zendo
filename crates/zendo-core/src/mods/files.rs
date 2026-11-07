//! Folder helpers the rest of `mods` shares.

use anyhow::bail;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// A folder's files, from it, in order; an `InvalidData` error for a link.
pub(super) fn walk(root: &Path) -> io::Result<Vec<PathBuf>> {
    fn visit(root: &Path, at: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(root.join(at))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = at.join(entry.file_name());
            if kind.is_symlink() {
                return Err(io::Error::new(ErrorKind::InvalidData, "a link"));
            } else if kind.is_dir() {
                visit(root, &path, found)?;
            } else {
                found.push(path);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    match visit(root, Path::new(""), &mut found) {
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        result => result?,
    }
    found.sort();
    Ok(found)
}

/// A path under `base` as the folders and files already there spell it,
/// case aside; the parts not there yet as given.
pub(super) fn case_in(base: &Path, path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for part in path.components() {
        let name = part.as_os_str();
        let found = fs::read_dir(base.join(&resolved)).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name())
                .find(|entry| {
                    entry
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&name.to_string_lossy())
                })
        });
        resolved.push(found.as_deref().unwrap_or(name));
    }
    resolved
}

/// A folder's subfolders and files, by name.
pub(super) fn entries(dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    dirs.sort();
    files.sort();
    (dirs, files)
}

/// A folder inside the game's: relative, never above it.
pub(super) fn check_relative(folder: &Path) -> anyhow::Result<()> {
    if folder
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        Ok(())
    } else {
        bail!("the mod folder must be inside the game's folder, as Data or BepInEx/plugins")
    }
}

/// A mod's name from its archive: `SkyUI_5_2_SE.7z` is `SkyUI_5_2_SE`.
pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
