//! The launchers' files, watched so the library can rescan by itself when
//! one changes: Steam's `libraryfolders.vdf` and each `appmanifest_*.acf`,
//! Heroic's `installed.json` records, and the `.desktop` applications
//! folders. [`signature`] gives their modification times as one sorted list;
//! a list that differs from the last one means something changed and the
//! library should be read again. Only launchers' files are watched, never
//! Zendo's own, so Zendo's changes do not rescan it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::{desktop, heroic, steam};

/// The watched files and folders, each with its modification time in
/// nanoseconds since the epoch, sorted.
#[must_use]
pub fn signature() -> Vec<(PathBuf, u64)> {
    let mut times = Vec::new();
    for root in steam::roots() {
        let Ok(root) = root.canonicalize() else {
            continue;
        };
        if !root.join("steamapps").is_dir() {
            continue;
        }
        note(&mut times, &root.join("steamapps/libraryfolders.vdf"));
        for folder in steam::library_folders(&root, &mut Vec::new()) {
            note_dir(&mut times, &folder.join("steamapps"));
        }
    }
    for root in heroic::roots() {
        note(
            &mut times,
            &root.join("legendaryConfig/legendary/installed.json"),
        );
        note(&mut times, &root.join("gog_store/installed.json"));
        note(&mut times, &root.join("nile_config/nile/installed.json"));
        note(&mut times, &root.join("sideload_apps/library.json"));
    }
    for dir in desktop::dirs() {
        note(&mut times, &dir);
    }
    times.sort();
    times
}

/// A path's modification time, when it is there.
fn note(times: &mut Vec<(PathBuf, u64)>, path: &Path) {
    if let Some(when) = mtime(path) {
        times.push((path.to_path_buf(), when));
    }
}

/// A Steam `steamapps` folder: its own time, and each manifest's, so a
/// manifest added, removed or changed is noticed.
fn note_dir(times: &mut Vec<(PathBuf, u64)>, dir: &Path) {
    note(times, dir);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "acf") {
            note(times, &path);
        }
    }
}

/// A path's modification time in nanoseconds since the epoch, when it can be
/// read.
fn mtime(path: &Path) -> Option<u64> {
    let nanos = fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    u64::try_from(nanos).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_each_file_with_its_time() {
        let dir = std::env::temp_dir().join(format!("zendo-watch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("appmanifest_1.acf");
        fs::write(&file, "x").unwrap();
        let mut times = Vec::new();
        note_dir(&mut times, &dir);
        assert_eq!(times.len(), 2, "the folder and its manifest: {times:?}");
        assert_eq!(times[0].0, dir);
        assert_eq!(times[1].0, file);
        assert!(times.iter().all(|(_, when)| *when > 0));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_path_has_no_time() {
        let mut times = Vec::new();
        note(&mut times, Path::new("/no/such/zendo-watch-file"));
        assert_eq!(times, [] as [(PathBuf, u64); 0]);
    }
}
