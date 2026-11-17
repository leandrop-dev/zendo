//! Greenlight, the Xbox home-streaming client: it streams a console's own
//! screen and lists no games, so it is a program Zendo opens rather than a
//! library source (`docs/reference.md`, "Xbox Series S").
//!
//! Zendo brings its `AppImage` from its GitHub release, checking it against
//! the digest GitHub publishes, the way it brings Nexus Launcher; a copy from
//! Flathub or on `PATH` is used just as well. Its `--connect=<console id>`
//! starts a stream straight away, but the console id is Greenlight's to know,
//! not Zendo's, so the window is opened with its own list
//! (`docs/reference.md`).

use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;

use crate::{engines, paths};

/// The app's id on Flathub.
pub const FLATPAK: &str = "io.github.unknownskl.greenlight";

/// Its repository, whose latest release carries the Linux `AppImage`.
const REPO: &str = "unknownskl/greenlight";

/// What the program is called in Zendo's words.
const LABEL: &str = "Greenlight";

/// What the `AppImage` in Zendo's launchers folder is called.
const APPIMAGE: &str = "Greenlight.AppImage";

/// The names its GitHub release or a package gives the program.
const PROGRAMS: [&str; 2] = ["greenlight", "Greenlight"];

/// The Flatpak's desktop file, under an install's export folder.
fn exported(root: &Path) -> Option<PathBuf> {
    let file = root
        .join("exports/share/applications")
        .join(format!("{FLATPAK}.desktop"));
    file.is_file().then_some(file)
}

/// The Flatpak's desktop file, in the user's install or the system's.
fn flatpak() -> Option<PathBuf> {
    paths::data_home()
        .and_then(|dir| exported(&dir.join("flatpak")))
        .or_else(|| exported(Path::new("/var/lib/flatpak")))
}

/// The `AppImage` Zendo downloaded, once it is in place.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
    engines::launchers_dir().map(|dir| dir.join(APPIMAGE))
}

/// The program found: Zendo's `AppImage`, the Flatpak, or a copy on `PATH`.
#[must_use]
pub fn installed() -> Option<PathBuf> {
    appimage()
        .filter(|path| path.is_file())
        .or_else(flatpak)
        .or_else(|| PROGRAMS.iter().find_map(|name| engines::find(name)))
}

/// Whether an asset's name is the Linux `AppImage`: the release also holds
/// Windows, macOS and Debian builds, and the Flatpak.
fn appimage_is(name: &str) -> bool {
    name.ends_with(".AppImage")
}

/// The `AppImage` of the latest release.
///
/// # Errors
///
/// When GitHub cannot be reached, the release has no `AppImage`, or it has no
/// digest to check the download against.
pub fn release() -> anyhow::Result<engines::Release> {
    let (tag, asset) = engines::latest_asset(REPO, LABEL, appimage_is)?;
    Ok(engines::Release {
        tag,
        asset,
        service: None,
    })
}

/// Downloads the `AppImage`, checks it and puts it in place, executable;
/// `stop` set ends the download early.
///
/// # Errors
///
/// As [`engines::bring`].
pub fn install(
    release: &engines::Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    engines::bring(release, APPIMAGE, stop, progress)
}

/// What opens Greenlight: Zendo's `AppImage` first, then the Flatpak, then a
/// program on `PATH`. Never the other way round: the `AppImage` is the one Zendo
/// brought, its own.
fn opener(
    appimage: Option<&Path>,
    flatpak: Option<&Path>,
    program: Option<&Path>,
) -> Option<std::process::Command> {
    if let Some(appimage) = appimage {
        return Some(std::process::Command::new(appimage));
    }
    if flatpak.is_some() {
        let mut command = std::process::Command::new("flatpak");
        command.args(["run", FLATPAK]);
        return Some(command);
    }
    program.map(std::process::Command::new)
}

/// Opens Greenlight, in its own window; the console is chosen there.
///
/// # Errors
///
/// When it is not installed, or cannot be started.
pub fn open() -> anyhow::Result<Child> {
    let appimage = appimage().filter(|path| path.is_file());
    let program = PROGRAMS.iter().find_map(|name| engines::find(name));
    let command = opener(
        appimage.as_deref(),
        flatpak().as_deref(),
        program.as_deref(),
    )
    .context("Greenlight is not installed: bring it from its release")?;
    crate::launch::spawn_logged(command, "greenlight")
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn takes_the_appimage_of_a_release() {
        assert!(super::appimage_is("Greenlight-2.4.2.AppImage"));
        assert!(
            !super::appimage_is("Greenlight-2.4.2.AppImage.sha256sum"),
            "a checksum is not the program"
        );
        assert!(!super::appimage_is("greenlight-desktop_2.4.2_amd64.deb"));
        assert!(!super::appimage_is("greenlight.flatpak"));
    }

    /// The `AppImage` Zendo brought comes first: a Flatpak there too, or a
    /// program on `PATH`, does not hide it.
    #[test]
    fn opens_the_appimage_before_a_flatpak() {
        let dir = std::env::temp_dir().join(format!(
            "zendo-greenlight-open-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        fs::create_dir_all(&dir).unwrap();
        let appimage = dir.join("Greenlight.AppImage");
        let flatpak = dir.join("io.github.unknownskl.greenlight.desktop");
        let program = dir.join("greenlight");
        for file in [&appimage, &flatpak, &program] {
            fs::write(file, "").unwrap();
        }
        let command =
            super::opener(Some(&appimage), Some(&flatpak), Some(&program)).expect("a command");
        assert_eq!(
            command.get_program(),
            std::ffi::OsStr::new(&appimage),
            "the AppImage Zendo brought, not the Flatpak"
        );
        let command = super::opener(None, Some(&flatpak), Some(&program)).expect("a command");
        assert_eq!(command.get_program(), std::ffi::OsStr::new("flatpak"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                std::ffi::OsStr::new("run"),
                std::ffi::OsStr::new(super::FLATPAK)
            ]
        );
        let command = super::opener(None, None, Some(&program)).expect("a command");
        assert_eq!(command.get_program(), std::ffi::OsStr::new(&program));
        assert!(super::opener(None, None, None).is_none(), "nothing to open");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn knows_the_flatpaks_desktop_file() {
        let dir = std::env::temp_dir().join(format!(
            "zendo-greenlight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        assert_eq!(super::exported(&dir), None, "no Flatpak install there");
        let file = dir
            .join("exports/share/applications")
            .join(format!("{}.desktop", super::FLATPAK));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "[Desktop Entry]\nName=Greenlight\n").unwrap();
        assert_eq!(super::exported(&dir), Some(file));
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with
    /// `cargo test -p zendo-core greenlight::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_the_real_latest_release() {
        let release = super::release().unwrap();
        println!(
            "{} {} {} {}",
            release.tag, release.asset.name, release.asset.size, release.asset.sha256
        );
        assert!(super::appimage_is(&release.asset.name));
        assert_eq!(release.asset.sha256.len(), 64, "a digest to check against");
    }

    /// Downloads the real `AppImage` into a throwaway home; run with
    /// `XDG_DATA_HOME=.../zendo-greenlight-live/data
    /// XDG_CACHE_HOME=.../zendo-greenlight-live/cache
    /// cargo test -p zendo-core greenlight::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and writes to a throwaway home"]
    fn installs_the_real_appimage() {
        let data = std::env::var("XDG_DATA_HOME").unwrap_or_default();
        assert!(
            data.contains("zendo-greenlight-live"),
            "set XDG_DATA_HOME and XDG_CACHE_HOME to a scratch folder named zendo-greenlight-live"
        );
        let release = super::release().unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let at = super::install(&release, &stop, &mut |_, _| {}).unwrap();
        assert_eq!(super::installed(), Some(at.clone()));
        let bytes = std::fs::read(&at).unwrap();
        assert!(
            bytes.starts_with(b"\x7fELF"),
            "an `AppImage` is an ELF file"
        );
        let mode =
            std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&at).unwrap().permissions());
        assert_eq!(mode & 0o111, 0o111, "it can be run");
        println!(
            "{} {} -> {} bytes",
            release.tag,
            release.asset.name,
            bytes.len()
        );
    }
}
