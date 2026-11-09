//! Moving a game's folder: a rename when the new place is on the same
//! drive, otherwise a copy and then the old folder deleted, as Heroic does
//! with rsync. legendary's `move` only renames, and gogdl and nile have no
//! move at all.
//!
//! A copy goes into a hidden folder beside the new place
//! (`.<name>.zendo-move`), and is renamed into place once it holds as many
//! files and bytes as the old one, so the new place never has half a game.
//! Stopping, or a failure, deletes that folder and leaves the old one as it
//! was. Files keep their permissions and modified times; links are copied
//! as links.

use std::fs::{self, File, FileTimes, Metadata};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

/// How much of a file is copied between checks for Stop.
const CHUNK: usize = 1 << 20;

/// How often a copy says how far it is.
const REPORT_EVERY: Duration = Duration::from_millis(250);

/// How a move ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moved {
    /// Renamed: the old folder is gone already.
    Renamed,
    /// Copied: the old folder is still there, to delete once the game's
    /// records say the new place.
    Copied,
    Stopped,
}

/// Files and bytes under a folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Count {
    files: u64,
    bytes: u64,
}

/// Moves the folder `from` to `to`, which must not exist yet. While a copy
/// runs, `progress` gets the bytes copied and the bytes in all, and
/// `stopped` is asked between pieces of a file.
///
/// # Errors
///
/// When `to` is there already or inside `from`, or a file cannot be read
/// or written (a full drive says so).
pub(crate) fn move_folder(
    from: &Path,
    to: &Path,
    stopped: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<Moved> {
    if to.starts_with(from) {
        bail!("{} is inside the game's folder", to.display());
    }
    if to.exists() || to.is_symlink() {
        bail!(
            "{} is there already; move or delete it, then try again",
            to.display()
        );
    }
    let parent = to.parent().context("the new place has no parent folder")?;
    fs::create_dir_all(parent).with_context(|| format!("could not make {}", parent.display()))?;
    match fs::rename(from, to) {
        Ok(()) => return Ok(Moved::Renamed),
        Err(err) if err.kind() == ErrorKind::CrossesDevices => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("could not move {} to {}", from.display(), to.display()));
        }
    }
    let staging = staging(to)?;
    clear_staging(&staging)?;
    let result = copy_whole(from, &staging, stopped, progress);
    match result {
        Ok(true) => {}
        Ok(false) => {
            clear_staging(&staging)?;
            return Ok(Moved::Stopped);
        }
        Err(err) => {
            let _ = clear_staging(&staging);
            return Err(err);
        }
    }
    fs::rename(&staging, to).with_context(|| format!("could not name {}", to.display()))?;
    Ok(Moved::Copied)
}

/// The hidden folder a copy goes into before it is named `to`.
fn staging(to: &Path) -> anyhow::Result<PathBuf> {
    let name = to
        .file_name()
        .context("the new place has no name")?
        .to_string_lossy();
    Ok(to.with_file_name(format!(".{name}.zendo-move")))
}

/// Deletes a copy left unfinished, by this move or one before it.
fn clear_staging(staging: &Path) -> anyhow::Result<()> {
    match fs::remove_dir_all(staging) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("could not delete {}", staging.display())),
    }
}

/// Copies `from` into `to` and checks the copy; false when stopped.
fn copy_whole(
    from: &Path,
    to: &Path,
    stopped: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<bool> {
    let total = count(from)?;
    let mut copy = Copy {
        stopped,
        progress,
        done: 0,
        total: total.bytes,
        reported: Instant::now(),
    };
    (copy.progress)(0, total.bytes);
    if !copy.folder(from, to)? {
        return Ok(false);
    }
    let copied = count(to)?;
    if copied != total {
        bail!(
            "the copy has {} files and {} bytes, the game's folder {} and {}",
            copied.files,
            copied.bytes,
            total.files,
            total.bytes
        );
    }
    Ok(true)
}

/// Files (links among them) and their bytes under `dir`.
fn count(dir: &Path) -> anyhow::Result<Count> {
    let mut total = Count::default();
    for entry in fs::read_dir(dir).with_context(|| format!("could not read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("could not read {}", dir.display()))?;
        let meta = entry
            .metadata()
            .with_context(|| format!("could not read {}", entry.path().display()))?;
        if meta.is_dir() {
            let inner = count(&entry.path())?;
            total.files += inner.files;
            total.bytes += inner.bytes;
        } else {
            total.files += 1;
            if meta.is_file() {
                total.bytes += meta.len();
            }
        }
    }
    Ok(total)
}

struct Copy<'a> {
    stopped: &'a dyn Fn() -> bool,
    progress: &'a mut dyn FnMut(u64, u64),
    done: u64,
    total: u64,
    reported: Instant,
}

impl Copy<'_> {
    /// Copies a folder and what it holds; false when stopped.
    fn folder(&mut self, from: &Path, to: &Path) -> anyhow::Result<bool> {
        fs::create_dir(to).with_context(|| format!("could not make {}", to.display()))?;
        for entry in
            fs::read_dir(from).with_context(|| format!("could not read {}", from.display()))?
        {
            let entry = entry.with_context(|| format!("could not read {}", from.display()))?;
            let source = entry.path();
            let target = to.join(entry.file_name());
            let meta = entry
                .metadata()
                .with_context(|| format!("could not read {}", source.display()))?;
            let kind = meta.file_type();
            let going = if kind.is_dir() {
                self.folder(&source, &target)?
            } else if kind.is_symlink() {
                let link = fs::read_link(&source)
                    .with_context(|| format!("could not read {}", source.display()))?;
                symlink(&link, &target).map_err(|err| written(err, &target))?;
                true
            } else if kind.is_file() {
                self.file(&source, &target, &meta)?
            } else {
                bail!("{} is not a file, folder or link", source.display());
            };
            if !going {
                return Ok(false);
            }
        }
        keep_times_and_mode(to, &fs::metadata(from)?)?;
        Ok(true)
    }

    /// Copies a file a piece at a time; false when stopped.
    fn file(&mut self, from: &Path, to: &Path, meta: &Metadata) -> anyhow::Result<bool> {
        let mut source =
            File::open(from).with_context(|| format!("could not read {}", from.display()))?;
        let mut target = File::create_new(to).map_err(|err| written(err, to))?;
        let mut buffer = vec![0; CHUNK];
        loop {
            if (self.stopped)() {
                return Ok(false);
            }
            let read = match source.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => {
                    return Err(err).with_context(|| format!("could not read {}", from.display()));
                }
            };
            target
                .write_all(&buffer[..read])
                .map_err(|err| written(err, to))?;
            self.done += read as u64;
            if self.reported.elapsed() >= REPORT_EVERY {
                self.reported = Instant::now();
                (self.progress)(self.done, self.total);
            }
        }
        target.sync_all().map_err(|err| written(err, to))?;
        drop(target);
        keep_times_and_mode(to, meta)?;
        Ok(true)
    }
}

/// Gives `path` the permissions and times of the original.
fn keep_times_and_mode(path: &Path, meta: &Metadata) -> anyhow::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(meta.permissions().mode()))
        .with_context(|| format!("could not set the permissions of {}", path.display()))?;
    let mut times = FileTimes::new();
    if let Ok(modified) = meta.modified() {
        times = times.set_modified(modified);
    }
    if let Ok(accessed) = meta.accessed() {
        times = times.set_accessed(accessed);
    }
    File::open(path)
        .and_then(|file| file.set_times(times))
        .with_context(|| format!("could not set the times of {}", path.display()))
}

/// A failed write, saying so plainly when the drive is full.
fn written(err: std::io::Error, path: &Path) -> anyhow::Error {
    if err.kind() == ErrorKind::StorageFull {
        let drive = path.parent().unwrap_or(path);
        anyhow::anyhow!("there is not enough room in {}", drive.display())
    } else {
        anyhow::Error::new(err).context(format!("could not write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zendo-moving-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch folder");
        dir
    }

    fn game(dir: &Path) -> PathBuf {
        let game = dir.join("Game");
        fs::create_dir_all(game.join("bin")).expect("game folder");
        fs::write(game.join("bin/game.exe"), vec![7; 3 * CHUNK + 5]).expect("exe");
        fs::write(game.join("readme.txt"), "hello").expect("readme");
        fs::set_permissions(game.join("readme.txt"), fs::Permissions::from_mode(0o750))
            .expect("mode");
        symlink("bin/game.exe", game.join("start")).expect("link");
        game
    }

    #[test]
    fn a_rename_on_the_same_drive() {
        let dir = scratch("rename");
        let from = game(&dir);
        let to = dir.join("elsewhere/Game");
        let moved = move_folder(&from, &to, &|| false, &mut |_, _| {}).expect("moved");
        assert_eq!(moved, Moved::Renamed);
        assert!(!from.exists());
        assert_eq!(
            fs::read_to_string(to.join("readme.txt")).expect("readme"),
            "hello"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_copy_keeps_files_links_modes_and_times() {
        let dir = scratch("copy");
        let from = game(&dir);
        let to = dir.join("Copy");
        let mut last = (0, 0);
        assert!(
            copy_whole(&from, &to, &|| false, &mut |done, total| last =
                (done, total))
            .expect("copied")
        );
        assert_eq!(last.1, 3 * CHUNK as u64 + 10);
        assert_eq!(
            fs::read(to.join("bin/game.exe")).expect("exe"),
            fs::read(from.join("bin/game.exe")).expect("exe")
        );
        assert_eq!(
            fs::read_link(to.join("start")).expect("link"),
            Path::new("bin/game.exe")
        );
        let meta = fs::metadata(to.join("readme.txt")).expect("readme");
        assert_eq!(meta.permissions().mode() & 0o777, 0o750);
        assert_eq!(
            meta.modified().expect("time"),
            fs::metadata(from.join("readme.txt"))
                .expect("readme")
                .modified()
                .expect("time")
        );
        assert_eq!(count(&to).expect("count"), count(&from).expect("count"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stopped_copy_reports_stopped() {
        let dir = scratch("stopped");
        let from = game(&dir);
        let to = dir.join("Copy");
        let asked = Cell::new(0);
        let stopped = || {
            asked.set(asked.get() + 1);
            asked.get() > 2
        };
        assert!(!copy_whole(&from, &to, &stopped, &mut |_, _| {}).expect("stopped"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_a_place_taken_or_inside_the_game() {
        let dir = scratch("refuses");
        let from = game(&dir);
        let taken = dir.join("Taken");
        fs::create_dir_all(&taken).expect("taken");
        assert!(move_folder(&from, &taken, &|| false, &mut |_, _| {}).is_err());
        assert!(move_folder(&from, &from.join("bin/Game"), &|| false, &mut |_, _| {}).is_err());
        assert!(from.join("readme.txt").is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_staging_folder_is_hidden_beside_the_new_place() {
        assert_eq!(
            staging(Path::new("/mnt/games/Cat Quest")).expect("staging"),
            Path::new("/mnt/games/.Cat Quest.zendo-move")
        );
    }
}
