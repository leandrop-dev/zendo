//! `nxm://` links handed to the window that is open. The browser starts
//! `zendo nxm://…`; that process passes the link over a socket in
//! `$XDG_RUNTIME_DIR/zendo/` (a folder only the user can open) and ends, or
//! opens the window itself when none listens.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// The longest link taken; Nexus's are far shorter.
const LONGEST: u64 = 4096;

fn socket() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("zendo/links.sock"))
}

/// Hands a link to the open window; `false` when there is none.
#[must_use]
pub fn send(link: &str) -> bool {
    socket().is_some_and(|path| send_to(&path, link))
}

fn send_to(path: &Path, link: &str) -> bool {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    stream.write_all(format!("{link}\n").as_bytes()).is_ok()
}

/// Hands the links other `zendo` processes pass over while the window is
/// open to `sink`, from a thread of its own, until `sink` returns `false`.
/// When another window already listens, this one takes none.
pub fn listen(mut sink: impl FnMut(String) -> bool + Send + 'static) {
    thread::spawn(move || {
        if let Some(listener) = socket().and_then(|path| bind(&path)) {
            accept(&listener, &mut sink);
        }
    });
}

fn bind(path: &Path) -> Option<UnixListener> {
    let dir = path.parent()?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .ok()?;
    // Ours alone, even when the folder was there before, and only if it is
    // a folder: a symlink planted here is not ours to chmod.
    if !fs::symlink_metadata(dir).ok()?.is_dir() {
        return None;
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).ok()?;
    if let Ok(meta) = fs::symlink_metadata(path) {
        if UnixStream::connect(path).is_ok() {
            // Another window listens.
            return None;
        }
        // Left by a window that ended without cleaning up.
        if !meta.file_type().is_socket() {
            return None;
        }
        fs::remove_file(path).ok()?;
    }
    UnixListener::bind(path).ok()
}

fn accept(listener: &UnixListener, sink: &mut impl FnMut(String) -> bool) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        if BufReader::new(stream.take(LONGEST))
            .read_line(&mut line)
            .is_ok()
        {
            let received = line.trim();
            if !received.is_empty() && !sink(received.to_owned()) {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_reaches_the_window_listening_and_only_one_listens() {
        let dir = std::env::temp_dir().join(format!("zendo-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("zendo/links.sock");
        assert!(
            !send_to(&path, "nxm://catquest/mods/1/files/2"),
            "nobody listens yet"
        );

        let listener = bind(&path).unwrap();
        let mode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        assert!(bind(&path).is_none(), "a second window takes no links");
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::spawn(move || accept(&listener, &mut |link| sender.send(link).is_ok()));
        assert!(send_to(&path, "nxm://catquest/mods/1/files/2"));
        let got = receiver.recv_timeout(Duration::from_secs(5)).ok();
        assert_eq!(got.as_deref(), Some("nxm://catquest/mods/1/files/2"));

        // A socket left by a window that ended is taken over.
        drop(receiver);
        let stale = dir.join("stale/links.sock");
        drop(UnixListener::bind({
            fs::create_dir_all(stale.parent().unwrap()).unwrap();
            &stale
        }));
        assert!(stale.exists());
        assert!(bind(&stale).is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
