//! The XDG base directories, read from the environment.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// A file name from an id: anything outside `[A-Za-z0-9._-]` as `_`, and
/// a name of dots alone (`.`, `..`) or none as underscores, so it never
/// names the folder itself or the one above.
pub fn safe_name(id: &str) -> String {
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.chars().all(|c| c == '.') {
        "_".repeat(name.len().max(1))
    } else {
        name
    }
}

pub fn cache_home() -> Option<PathBuf> {
    xdg_home("XDG_CACHE_HOME", ".cache")
}

pub fn state_home() -> Option<PathBuf> {
    xdg_home("XDG_STATE_HOME", ".local/state")
}

/// Writes through a temporary file renamed into place, so a reader never
/// sees half a file. The file and its folder are synced before and after
/// the rename, so a crash or power cut leaves the old file or the new one,
/// never an empty one (btrfs and ext4 guard a rename that replaces a file,
/// not one that makes a new file, as Apply's `applied.json` is).
///
/// # Errors
///
/// When the folder cannot be made or the file written.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, Access::Default)
}

/// [`write_atomic`] for a file only its owner may read, such as one holding
/// a key.
///
/// # Errors
///
/// As [`write_atomic`].
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, Access::Owner)
}

/// Who may read a file written.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    /// As the umask says.
    Default,
    /// Its owner alone (0600).
    Owner,
}

fn write_with(path: &Path, bytes: &[u8], access: Access) -> io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
    }
    let temporary = temporary(path);
    let written = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        if access == Access::Owner {
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        // The mode applies only to a new file; a leftover keeps its own.
        if access == Access::Owner {
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(err) = written {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    fs::rename(&temporary, path).inspect_err(|_| {
        // The rename failed, so the temporary is still ours to remove:
        // `write_private` writes secrets into it.
        let _ = fs::remove_file(&temporary);
    })?;
    if let Some(dir) = dir {
        // The file is in place; a failed fsync costs durability, not the
        // write, and reporting it would leave the caller out of step.
        let _ = fs::File::open(dir)?.sync_all();
    }
    Ok(())
}

/// Runs `work` holding an advisory lock on `<path>.lock`, so the window's
/// threads and the `zendo` command change a file one at a time instead of
/// one losing the other's change.
///
/// # Errors
///
/// When the lock file cannot be made or locked, or as `work`.
pub fn locked<T>(path: &Path, work: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    use anyhow::Context;

    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    let lock_path = PathBuf::from(name);
    if let Some(dir) = lock_path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("could not open {}", lock_path.display()))?;
    lock.lock()
        .with_context(|| format!("could not lock {}", lock_path.display()))?;
    // The lock goes when `lock` is dropped, after `work`.
    work()
}

/// A name for the temporary file, other than any other writer's, in this
/// process or another.
fn temporary(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zendo-paths-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn safe_names_stay_inside_their_folder() {
        assert_eq!(safe_name("steam:22370"), "steam_22370");
        assert_eq!(safe_name("local:my-game.v2"), "local_my-game.v2");
        assert_eq!(safe_name("a/../b"), "a_.._b");
        assert_eq!(safe_name(".."), "__");
        assert_eq!(safe_name("."), "_");
        assert_eq!(safe_name(""), "_");
    }

    #[test]
    fn writes_replace_the_file_and_private_ones_are_the_owners_alone() {
        let dir = scratch("write");
        let path = dir.join("new/state.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");

        let key = dir.join("config.toml");
        // A world-readable leftover becomes the owner's alone.
        fs::write(&key, "old").unwrap();
        fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&key, b"key = 1").unwrap();
        let mode = fs::metadata(&key).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read(&key).unwrap(), b"key = 1");
        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !names
                .iter()
                .any(|name| Path::new(name).extension().is_some_and(|ext| ext == "tmp")),
            "{names:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_locked_change_waits_for_the_one_before() {
        let dir = scratch("lock");
        let path = dir.join("count");
        fs::write(&path, "0").unwrap();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for _ in 0..25 {
                        locked(&path, || {
                            let count: u32 = fs::read_to_string(&path)?.parse()?;
                            write_atomic(&path, (count + 1).to_string().as_bytes())?;
                            Ok(())
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), "200");
        let _ = fs::remove_dir_all(&dir);
    }
}
