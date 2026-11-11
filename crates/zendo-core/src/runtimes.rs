//! The anti-cheat runtimes Proton needs for games that ship Easy Anti-Cheat
//! or `BattlEye` with Linux support turned on. Proton reads their folders
//! from `PROTON_EAC_RUNTIME` and `PROTON_BATTLEYE_RUNTIME`; Steam installs
//! them as tools of their own (apps 1826330 and 1161040) and umu does not
//! fetch them. Zendo uses Steam's when it has them, else a copy of its own
//! in `$XDG_DATA_HOME/zendo/runtimes/`, downloaded from the list Lutris
//! keeps (<https://lutris.net/api/runtimes>), as Heroic does. Each copy
//! keeps the date of its build in `current_version`.
//!
//! Lutris publishes no checksums for these; the download is over HTTPS
//! from its GitHub releases.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::{paths, steam};

const LIST: &str = "https://lutris.net/api/runtimes";

/// How deep in a game's folder its anti-cheat folder is looked for.
const DEPTH: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Runtime {
    Eac,
    BattlEye,
}

impl Runtime {
    pub const ALL: [Self; 2] = [Self::Eac, Self::BattlEye];

    /// Its name in Lutris's list, and its folder's in Zendo's.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Eac => "eac_runtime",
            Self::BattlEye => "battleye_runtime",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Eac => "Easy Anti-Cheat",
            Self::BattlEye => "BattlEye",
        }
    }

    fn variable(self) -> &'static str {
        match self {
            Self::Eac => "PROTON_EAC_RUNTIME",
            Self::BattlEye => "PROTON_BATTLEYE_RUNTIME",
        }
    }

    fn steam_app(self) -> &'static str {
        match self {
            Self::Eac => "1826330",
            Self::BattlEye => "1161040",
        }
    }

    /// The folder inside that Proton reads, there in every complete copy.
    fn inner(self) -> &'static str {
        match self {
            Self::Eac => "v2",
            Self::BattlEye => "v1",
        }
    }

    /// Whether a folder in a game's files is this anti-cheat's.
    fn marks(self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        match self {
            Self::Eac => name == "easyanticheat" || name == "easyanticheat_eos",
            Self::BattlEye => name == "battleye",
        }
    }

    fn complete(self, path: &Path) -> bool {
        path.join(self.inner()).is_dir()
    }
}

/// A runtime on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The tool Steam installed.
    Steam(PathBuf),
    /// Zendo's own copy, with its build's date.
    Zendo {
        path: PathBuf,
        version: Option<String>,
    },
}

impl Found {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Steam(path) | Self::Zendo { path, .. } => path,
        }
    }
}

/// Where Zendo keeps its own copies.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo/runtimes"))
}

/// The runtime to use: Steam's, else Zendo's own.
#[must_use]
pub fn find(runtime: Runtime) -> Option<Found> {
    if let Some(path) = steam::app_folder(&steam::roots(), runtime.steam_app())
        .filter(|path| runtime.complete(path))
    {
        return Some(Found::Steam(path));
    }
    own(runtime)
}

/// Zendo's own copy, whether or not Steam has one too.
#[must_use]
pub fn own(runtime: Runtime) -> Option<Found> {
    let path = dir()?.join(runtime.key());
    runtime.complete(&path).then(|| Found::Zendo {
        version: fs::read_to_string(path.join("current_version"))
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty()),
        path,
    })
}

/// The variables that point Proton at each runtime on this machine. Proton
/// uses one only for a game that ships that anti-cheat, so both are set
/// whenever they are there.
#[must_use]
pub fn environment() -> Vec<(&'static str, OsString)> {
    Runtime::ALL
        .into_iter()
        .filter_map(|runtime| {
            find(runtime).map(|found| (runtime.variable(), found.path().as_os_str().to_owned()))
        })
        .collect()
}

/// The anti-cheats a game's files ship: a folder named for one, in the
/// game's folder or a few levels down.
#[must_use]
pub fn needed_by(folder: &Path) -> Vec<Runtime> {
    let mut found = Vec::new();
    let mut level = vec![folder.to_path_buf()];
    for _ in 0..DEPTH {
        let mut next = Vec::new();
        for dir in level {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                for runtime in Runtime::ALL {
                    if runtime.marks(&name) && !found.contains(&runtime) {
                        found.push(runtime);
                    }
                }
                next.push(entry.path());
            }
        }
        level = next;
    }
    found.sort_by_key(|runtime| Runtime::ALL.iter().position(|r| r == runtime));
    found
}

/// The runtimes a game's files need that are not on this machine.
#[must_use]
pub fn missing_for(folder: &Path) -> Vec<Runtime> {
    needed_by(folder)
        .into_iter()
        .filter(|runtime| find(*runtime).is_none())
        .collect()
}

/// A runtime as Lutris lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub runtime: Runtime,
    pub url: String,
    /// When it was built, as Lutris says: `2024-01-14T19:38:26.671443Z`.
    pub version: String,
}

/// The latest of each runtime, from Lutris's list.
///
/// # Errors
///
/// When Lutris cannot be reached or answers oddly.
pub fn latest() -> anyhow::Result<Vec<Remote>> {
    let text = crate::http::agent(Duration::from_secs(30))
        .get(LIST)
        .call()
        .context("could not reach Lutris's list of runtimes")?
        .into_body()
        .read_to_string()?;
    parse_list(&text)
}

fn parse_list(text: &str) -> anyhow::Result<Vec<Remote>> {
    #[derive(Deserialize)]
    struct Entry {
        name: String,
        url: Option<String>,
        created_at: Option<String>,
    }
    let entries: Vec<Entry> =
        serde_json::from_str(text).context("Lutris's list of runtimes is not what Zendo knows")?;
    Ok(Runtime::ALL
        .into_iter()
        .filter_map(|runtime| {
            let entry = entries.iter().find(|entry| entry.name == runtime.key())?;
            Some(Remote {
                runtime,
                url: entry
                    .url
                    .clone()
                    .filter(|url| url.starts_with("https://"))?,
                version: entry.created_at.clone().unwrap_or_default(),
            })
        })
        .collect())
}

/// Downloads a runtime into Zendo's folder, in place of an older copy,
/// saying how far it got.
///
/// # Errors
///
/// When the download fails or is stopped, or the tarball is not the
/// runtime; the older copy stays then.
pub fn install(
    remote: &Remote,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    let dir = dir().context("no home folder to keep the runtime in")?;
    let cache = paths::cache_home()
        .context("no cache folder to download into")?
        .join("zendo/runtimes");
    fs::create_dir_all(&cache).with_context(|| format!("could not make {}", cache.display()))?;
    let tarball = cache.join(format!("{}.tar.xz", remote.runtime.key()));
    let result = crate::http::download(
        &remote.url,
        &tarball,
        remote.runtime.label(),
        None,
        stop,
        &mut |done, total| progress(done, total.unwrap_or(done)),
    )
    .and_then(|()| unpack(&tarball, &dir, remote.runtime, &remote.version));
    let _ = fs::remove_file(&tarball);
    result
}

/// Unpacks into a hidden folder beside the copies, writes the version,
/// then swaps it in for the older copy.
fn unpack(tarball: &Path, dir: &Path, runtime: Runtime, version: &str) -> anyhow::Result<PathBuf> {
    let key = runtime.key();
    fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    let partial = dir.join(format!(".zendo-{key}.partial"));
    let old = dir.join(format!(".zendo-{key}.old"));
    for leftover in [&partial, &old] {
        if leftover.exists() {
            fs::remove_dir_all(leftover)
                .with_context(|| format!("could not clear {}", leftover.display()))?;
        }
    }
    fs::create_dir(&partial).with_context(|| format!("could not make {}", partial.display()))?;
    let result = (|| {
        let status = Command::new("tar")
            .arg("-xJf")
            .arg(tarball)
            .arg("-C")
            .arg(&partial)
            .arg("--no-same-owner")
            .stdin(Stdio::null())
            .status()
            .context("could not run tar")?;
        if !status.success() {
            bail!("tar could not unpack the {} runtime", runtime.label());
        }
        // The tarball holds one folder, whatever it is named.
        let folder = fs::read_dir(&partial)
            .with_context(|| format!("could not read {}", partial.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| runtime.complete(path))
            .with_context(|| format!("the download holds no {} runtime", runtime.label()))?;
        let version_file = folder.join("current_version");
        fs::write(&version_file, version)
            .with_context(|| format!("could not write {}", version_file.display()))?;
        let target = dir.join(key);
        if target.exists() {
            fs::rename(&target, &old)
                .with_context(|| format!("could not move {} aside", target.display()))?;
        }
        if let Err(err) = fs::rename(&folder, &target) {
            let _ = fs::rename(&old, &target);
            return Err(err).with_context(|| format!("could not move {key} into place"));
        }
        Ok(target)
    })();
    let _ = fs::remove_dir_all(&partial);
    let _ = fs::remove_dir_all(&old);
    result
}

/// Removes Zendo's own copy; Steam's is Steam's to remove.
///
/// # Errors
///
/// When the folder cannot be deleted.
pub fn remove(runtime: Runtime) -> anyhow::Result<()> {
    let Some(path) = dir().map(|dir| dir.join(runtime.key())) else {
        return Ok(());
    };
    if path.exists() {
        fs::remove_dir_all(&path)
            .with_context(|| format!("could not delete {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zendo-runtimes-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tells_which_anti_cheat_a_game_ships() {
        let game = scratch("game");
        assert_eq!(needed_by(&game), Vec::new());
        fs::create_dir_all(game.join("Binaries/Win64/EasyAntiCheat")).unwrap();
        assert_eq!(needed_by(&game), [Runtime::Eac]);
        fs::create_dir_all(game.join("BattlEye")).unwrap();
        assert_eq!(needed_by(&game), [Runtime::Eac, Runtime::BattlEye]);
        // A file of that name is not the folder.
        let other = scratch("other");
        fs::write(other.join("BattlEye"), "").unwrap();
        fs::create_dir_all(other.join("a/b/c/EasyAntiCheat")).unwrap();
        assert_eq!(needed_by(&other), Vec::new(), "too deep, and a file");
        fs::remove_dir_all(&game).unwrap();
        fs::remove_dir_all(&other).unwrap();
    }

    #[test]
    fn reads_lutris_list() {
        let remotes = parse_list(
            r#"[{"id": 16, "name": "battleye_runtime", "created_at": "2024-05-21T19:35:07Z", "url": "https://github.com/lutris/buildbot/releases/download/2024-05-21/battleeye_runtime.tar.xz"},
                {"id": 3, "name": "dxvk", "url": "https://example.com/dxvk.tar.xz"},
                {"id": 22, "name": "eac_runtime", "created_at": "2024-01-14T19:38:26Z", "url": "http://insecure.example/eac.tar.xz"}]"#,
        )
        .unwrap();
        assert_eq!(remotes.len(), 1, "plain http is refused: {remotes:?}");
        assert_eq!(remotes[0].runtime, Runtime::BattlEye);
        assert_eq!(remotes[0].version, "2024-05-21T19:35:07Z");
    }

    #[test]
    fn unpacks_in_place_of_the_older_copy() {
        let dir = scratch("unpack");
        let source = dir.join("source/battleeye_runtime");
        fs::create_dir_all(source.join("v1/lib64")).unwrap();
        fs::write(source.join("BEClient_x64.so"), "new").unwrap();
        let tarball = dir.join("be.tar.xz");
        let status = Command::new("tar")
            .arg("-cJf")
            .arg(&tarball)
            .arg("-C")
            .arg(dir.join("source"))
            .arg("battleeye_runtime")
            .status()
            .unwrap();
        assert!(status.success());
        let runtimes = dir.join("runtimes");
        fs::create_dir_all(runtimes.join("battleye_runtime/v1")).unwrap();
        fs::write(runtimes.join("battleye_runtime/BEClient_x64.so"), "old").unwrap();
        let target = unpack(&tarball, &runtimes, Runtime::BattlEye, "2024-05-21").unwrap();
        assert_eq!(target, runtimes.join("battleye_runtime"));
        assert_eq!(
            fs::read_to_string(target.join("BEClient_x64.so")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read_to_string(target.join("current_version")).unwrap(),
            "2024-05-21"
        );
        let left: Vec<_> = fs::read_dir(&runtimes).unwrap().collect();
        assert_eq!(left.len(), 1, "nothing left beside it");
        // A tarball of something else leaves the copy as it was.
        assert!(unpack(&tarball, &runtimes, Runtime::Eac, "x").is_err());
        assert!(runtimes.join("battleye_runtime/v1").is_dir());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_steams_own_copy() {
        let root = scratch("steam");
        let steamapps = root.join("steamapps");
        fs::create_dir_all(steamapps.join("common/Proton EasyAntiCheat Runtime/v2")).unwrap();
        fs::write(
            steamapps.join("appmanifest_1826330.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1826330\"\n\t\"name\"\t\t\"Proton EasyAntiCheat Runtime\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"Proton EasyAntiCheat Runtime\"\n}\n",
        )
        .unwrap();
        let found = steam::app_folder(std::slice::from_ref(&root), "1826330").unwrap();
        assert!(Runtime::Eac.complete(&found));
        assert_eq!(
            steam::app_folder(std::slice::from_ref(&root), "1161040"),
            None
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore = "needs the network"]
    fn downloads_the_real_runtimes() {
        let dir = scratch("real");
        for remote in latest().unwrap() {
            println!("{} {} {}", remote.runtime.key(), remote.version, remote.url);
            let tarball = dir.join(format!("{}.tar.xz", remote.runtime.key()));
            crate::http::download(
                &remote.url,
                &tarball,
                remote.runtime.label(),
                None,
                &AtomicBool::new(false),
                &mut |_, _| {},
            )
            .unwrap();
            let path = unpack(&tarball, &dir, remote.runtime, &remote.version).unwrap();
            assert!(remote.runtime.complete(&path));
            assert_eq!(
                fs::read_to_string(path.join("current_version")).unwrap(),
                remote.version
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
