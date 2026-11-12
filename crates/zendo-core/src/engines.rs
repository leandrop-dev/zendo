//! The programs Zendo drives, downloaded from their own GitHub releases into
//! `$XDG_DATA_HOME/zendo/engines/<engine>/`: legendary (Epic), gogdl (GOG),
//! nile (Amazon), comet (GOG Galaxy) and umu-launcher. Fedora packages none
//! of them but an old legendary, and Heroic is not needed.
//!
//! No release has a checksum file, but GitHub's API gives each asset a
//! `digest` (`sha256:<hex>`); an asset without one is refused. The download
//! goes into Zendo's cache, is checked with `sha256sum`, is built in a
//! hidden folder beside the others and renamed into place when complete.
//!
//! [`find`] is the one lookup of a program: Zendo's own copy, then `PATH`,
//! then Heroic's bundle, or `PATH` first when Settings says so.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::config::Config;
use crate::paths;

/// A program Zendo can download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    Legendary,
    Gogdl,
    Nile,
    Comet,
    Umu,
}

impl Engine {
    pub const ALL: [Self; 5] = [
        Self::Legendary,
        Self::Gogdl,
        Self::Nile,
        Self::Comet,
        Self::Umu,
    ];

    /// The program's name, as it is run.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::Legendary => "legendary",
            Self::Gogdl => "gogdl",
            Self::Nile => "nile",
            Self::Comet => "comet",
            Self::Umu => "umu-run",
        }
    }

    /// What Settings calls it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Umu => "umu-launcher",
            other => other.program(),
        }
    }

    /// What it is for, in a few words.
    #[must_use]
    pub fn purpose(self) -> &'static str {
        match self {
            Self::Legendary => "Epic Games",
            Self::Gogdl => "GOG",
            Self::Nile => "Amazon Games",
            Self::Comet => "GOG Galaxy",
            Self::Umu => "Windows games",
        }
    }

    /// The engine whose program is called this.
    #[must_use]
    pub fn by_program(program: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.program() == program)
    }

    fn repo(self) -> &'static str {
        match self {
            Self::Legendary => "derrod/legendary",
            Self::Gogdl => "Heroic-Games-Launcher/heroic-gogdl",
            Self::Nile => "imLinguin/nile",
            Self::Comet => "imLinguin/comet",
            Self::Umu => "Open-Wine-Components/umu-launcher",
        }
    }

    /// The folder's name under Zendo's engines folder.
    fn folder(self) -> &'static str {
        match self {
            Self::Umu => "umu",
            other => other.program(),
        }
    }

    /// The release asset for this CPU; `None` for umu, whose one asset is
    /// found by its ending, and for a CPU none is built for.
    fn asset_name(self, arch: &str) -> Option<&'static str> {
        let arm = match arch {
            "x86_64" => false,
            "aarch64" => true,
            _ => return None,
        };
        Some(match (self, arm) {
            (Self::Legendary, false) => "legendary_linux_x64",
            (Self::Legendary, true) => "legendary_linux_arm64",
            (Self::Gogdl, false) => "gogdl_linux_x86_64",
            (Self::Gogdl, true) => "gogdl_linux_arm64",
            (Self::Nile, false) => "nile_linux_x86_64",
            (Self::Nile, true) => "nile_linux_arm64",
            (Self::Comet, false) => "comet-x86_64-unknown-linux-gnu",
            (Self::Comet, true) => "comet-aarch64-unknown-linux-gnu",
            (Self::Umu, _) => return None,
        })
    }

    /// Zendo's folder for this engine.
    #[must_use]
    pub fn dir(self) -> Option<PathBuf> {
        root().map(|root| root.join(self.folder()))
    }

    /// Where Zendo's copy of the program goes.
    #[must_use]
    pub fn path(self) -> Option<PathBuf> {
        self.dir().map(|dir| dir.join(self.program()))
    }

    /// The release Zendo's copy came from, when it has one.
    #[must_use]
    pub fn installed(self) -> Option<String> {
        let tag = fs::read_to_string(self.dir()?.join("release")).ok()?;
        let tag = tag.trim();
        (!tag.is_empty() && self.path()?.is_file()).then(|| tag.to_owned())
    }
}

/// Zendo's engines folder.
fn root() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo/engines"))
}

/// A file a release holds that Zendo downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lower-case hex of the SHA-256.
    pub sha256: String,
}

/// A release of an engine, with the asset for this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub asset: Asset,
    /// comet's `dummy-service.zip`, which goes with comet's release.
    pub service: Option<Asset>,
}

#[derive(Deserialize)]
struct Entry {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

impl RawAsset {
    fn checked(&self) -> anyhow::Result<Asset> {
        let sha256 = self
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .with_context(|| {
                format!(
                    "{} has no checksum to check the download against",
                    self.name
                )
            })?
            .to_lowercase();
        Ok(Asset {
            name: self.name.clone(),
            url: self.browser_download_url.clone(),
            size: self.size,
            sha256,
        })
    }
}

/// The release's asset for a CPU, in GitHub's answer.
fn pick(engine: Engine, arch: &str, entry: &Entry) -> anyhow::Result<Release> {
    let wanted = engine.asset_name(arch);
    let asset = entry
        .assets
        .iter()
        .find(|asset| match wanted {
            Some(name) => asset.name == name,
            None => engine == Engine::Umu && asset.name.ends_with("-zipapp.tar"),
        })
        .with_context(|| {
            format!(
                "{} {} has no build for this machine",
                engine.label(),
                entry.tag_name
            )
        })?
        .checked()?;
    let service = if engine == Engine::Comet {
        let raw = entry
            .assets
            .iter()
            .find(|asset| asset.name == "dummy-service.zip")
            .with_context(|| format!("comet {} has no dummy-service.zip", entry.tag_name))?;
        Some(raw.checked()?)
    } else {
        None
    };
    Ok(Release {
        tag: entry.tag_name.clone(),
        asset,
        service,
    })
}

/// The latest release of an engine, from GitHub (60 requests an hour are
/// allowed without an account).
///
/// # Errors
///
/// When GitHub cannot be reached or answers oddly, the release is only a
/// pre-release, or it has no build for this machine or no checksum.
pub fn latest(engine: Engine) -> anyhow::Result<Release> {
    let body = crate::http::download_agent()
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            engine.repo()
        ))
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("could not reach GitHub")?
        .into_body()
        .read_to_string()?;
    let entry: Entry =
        serde_json::from_str(&body).context("GitHub's answer about the release was odd")?;
    if entry.prerelease {
        bail!("{}'s latest release is a pre-release", engine.label());
    }
    pick(engine, std::env::consts::ARCH, &entry)
}

/// Downloads, checks and puts a release in place, reporting the bytes
/// downloaded and the whole as it goes; `stop` set ends it early.
///
/// # Errors
///
/// When the download fails or is stopped, the checksum does not match, or
/// the files cannot be put in place; the copy there before stays then.
pub fn install(
    engine: Engine,
    release: &Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    let root = root().context("no home folder for Zendo's engines")?;
    let cache = paths::cache_home()
        .context("no cache folder to download into")?
        .join("zendo/engines");
    fs::create_dir_all(&cache).with_context(|| format!("could not make {}", cache.display()))?;
    fs::create_dir_all(&root).with_context(|| format!("could not make {}", root.display()))?;
    clear_leftovers(&cache, &root);
    let file = cache.join(&release.asset.name);
    let result = (|| {
        let size = Some(release.asset.size).filter(|&size| size > 0);
        crate::http::download(
            &release.asset.url,
            &file,
            engine.label(),
            size,
            stop,
            &mut |done, total| progress(done, total.unwrap_or(done)),
        )?;
        check_sha256(&file, &release.asset.sha256)?;
        place(engine, &file, &release.tag, &root)
    })();
    let _ = fs::remove_file(&file);
    let path = result?;
    if let Some(service) = &release.service {
        crate::galaxy::replace_service(service, stop)
            .context("comet is installed, but its Galaxy service could not be replaced")?;
    }
    Ok(path)
}

/// What a download Zendo was closed in the middle of left.
fn clear_leftovers(cache: &Path, root: &Path) {
    for entry in fs::read_dir(cache).into_iter().flatten().flatten() {
        let _ = fs::remove_file(entry.path());
    }
    for entry in fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let hidden_work = Path::new(&name)
            .extension()
            .is_some_and(|ext| ext == "partial" || ext == "old");
        if name.starts_with('.') && hidden_work {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Compares a file with its SHA-256, through `sha256sum`.
pub(crate) fn check_sha256(file: &Path, wanted: &str) -> anyhow::Result<()> {
    let output = Command::new("sha256sum")
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .context("could not run sha256sum")?;
    if !output.status.success() {
        bail!("sha256sum failed on the download");
    }
    let found = String::from_utf8_lossy(&output.stdout);
    if found.split_whitespace().next() != Some(wanted) {
        bail!(
            "the download of {} does not match its checksum",
            file.file_name().map_or_else(
                || "a file".to_owned(),
                |name| name.to_string_lossy().into_owned()
            )
        );
    }
    Ok(())
}

/// Builds the engine's folder in a hidden one beside the others, with the
/// program (named for what it runs as) and the `release` file, and swaps it
/// for the one there. A program running from the old folder keeps its file.
fn place(engine: Engine, file: &Path, tag: &str, root: &Path) -> anyhow::Result<PathBuf> {
    let name = engine.folder();
    let partial = root.join(format!(".{name}.partial"));
    let old = root.join(format!(".{name}.old"));
    for stale in [&partial, &old] {
        if stale.exists() {
            fs::remove_dir_all(stale)
                .with_context(|| format!("could not clear {}", stale.display()))?;
        }
    }
    fs::create_dir(&partial).with_context(|| format!("could not make {}", partial.display()))?;
    let built = (|| {
        let program = partial.join(engine.program());
        if engine == Engine::Umu {
            let unpacked = partial.join("unpacked");
            fs::create_dir(&unpacked)?;
            let status = Command::new("tar")
                .arg("-xf")
                .arg(file)
                .arg("-C")
                .arg(&unpacked)
                .arg("--no-same-owner")
                .stdin(Stdio::null())
                .status()
                .context("could not run tar")?;
            if !status.success() {
                bail!("tar could not unpack umu-launcher");
            }
            let run = unpacked.join("umu/umu-run");
            if !run.is_file() {
                bail!("umu-launcher's download holds no umu/umu-run");
            }
            fs::rename(&run, &program)?;
            fs::remove_dir_all(&unpacked)?;
        } else {
            fs::copy(file, &program)
                .with_context(|| format!("could not copy {} into place", engine.label()))?;
        }
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755))?;
        fs::write(partial.join("release"), format!("{tag}\n"))?;
        Ok(())
    })();
    if let Err(err) = built {
        let _ = fs::remove_dir_all(&partial);
        return Err(err);
    }
    let target = root.join(name);
    let had = target.exists();
    if had {
        fs::rename(&target, &old).with_context(|| format!("could not move {name} aside"))?;
    }
    if let Err(err) = fs::rename(&partial, &target) {
        if had {
            let _ = fs::rename(&old, &target);
        }
        let _ = fs::remove_dir_all(&partial);
        return Err(err).with_context(|| format!("could not move {name} into place"));
    }
    let _ = fs::remove_dir_all(&old);
    Ok(target.join(engine.program()))
}

/// Deletes Zendo's copy of an engine.
///
/// # Errors
///
/// When the folder cannot be deleted.
pub fn remove(engine: Engine) -> anyhow::Result<()> {
    let dir = engine.dir().context("no home folder for Zendo's engines")?;
    match fs::remove_dir_all(&dir) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("could not delete {}", dir.display()))
        }
        _ => Ok(()),
    }
}

/// Whose copy of a program was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Zendo,
    Path,
    Heroic,
}

/// A program as [`find`] sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub path: PathBuf,
    pub origin: Origin,
}

/// The program: Zendo's own copy, then `PATH`, then Heroic's bundle; `PATH`
/// first when `engines_on_path_first` is on in `config.toml`, which is read
/// on each call.
#[must_use]
pub fn find(program: &str) -> Option<PathBuf> {
    find_with_origin(program).map(|found| found.path)
}

/// [`find`], saying whose copy it is.
#[must_use]
pub fn find_with_origin(program: &str) -> Option<Located> {
    let path_first = Config::load_file().is_ok_and(|config| config.engines_on_path_first);
    let path = std::env::var_os("PATH").unwrap_or_default();
    find_in(
        program,
        root().as_deref(),
        path_first,
        &path,
        &crate::stores::heroic_engine_dirs(),
    )
}

fn find_in(
    program: &str,
    own: Option<&Path>,
    path_first: bool,
    path: &std::ffi::OsStr,
    heroic: &[PathBuf],
) -> Option<Located> {
    let own = || {
        let engine = Engine::by_program(program)?;
        let file = own?.join(engine.folder()).join(program);
        is_executable(&file).then_some(Located {
            path: file,
            origin: Origin::Zendo,
        })
    };
    let on_path = || {
        std::env::split_paths(path)
            .map(|dir| dir.join(program))
            .find(|file| is_executable(file))
            .map(|path| Located {
                path,
                origin: Origin::Path,
            })
    };
    let in_heroic = || {
        heroic
            .iter()
            .map(|dir| dir.join(program))
            .find(|file| is_executable(file))
            .map(|path| Located {
                path,
                origin: Origin::Heroic,
            })
    };
    if path_first {
        on_path().or_else(own).or_else(in_heroic)
    } else {
        own().or_else(on_path).or_else(in_heroic)
    }
}

fn is_executable(file: &Path) -> bool {
    file.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tag: &str, assets: &[(&str, Option<&str>)]) -> Entry {
        Entry {
            tag_name: tag.into(),
            prerelease: false,
            assets: assets
                .iter()
                .map(|(name, digest)| RawAsset {
                    name: (*name).into(),
                    size: 10,
                    browser_download_url: format!("https://example/{name}"),
                    digest: digest.map(str::to_owned),
                })
                .collect(),
        }
    }

    const SUM: &str = "sha256:d73519514df9d52c50c6feec629373cc3a0d9504e087b6c7372a8ca8a95e7bfe";

    #[test]
    fn picks_each_engines_asset_for_each_cpu() {
        let legendary = entry(
            "0.21.1",
            &[
                ("legendary_linux_arm64", Some(SUM)),
                ("legendary_linux_x64", Some(SUM)),
                ("legendary_macOS.zip", Some(SUM)),
            ],
        );
        let x64 = pick(Engine::Legendary, "x86_64", &legendary).unwrap();
        assert_eq!(x64.asset.name, "legendary_linux_x64");
        assert_eq!(
            pick(Engine::Legendary, "aarch64", &legendary)
                .unwrap()
                .asset
                .name,
            "legendary_linux_arm64"
        );
        let gogdl = entry(
            "v1.4.0",
            &[
                ("gogdl_linux_arm64", Some(SUM)),
                ("gogdl_linux_x86_64", Some(SUM)),
            ],
        );
        assert_eq!(
            pick(Engine::Gogdl, "x86_64", &gogdl).unwrap().asset.name,
            "gogdl_linux_x86_64"
        );
        let nile = entry(
            "v1.2.0",
            &[
                ("nile_linux_arm64", Some(SUM)),
                ("nile_linux_x86_64", Some(SUM)),
            ],
        );
        assert_eq!(
            pick(Engine::Nile, "aarch64", &nile).unwrap().asset.name,
            "nile_linux_arm64"
        );
        let comet = entry(
            "v0.4.1",
            &[
                ("comet-x86_64-unknown-linux-gnu", Some(SUM)),
                ("comet-aarch64-unknown-linux-gnu", Some(SUM)),
                ("dummy-service.zip", Some(SUM)),
            ],
        );
        let found = pick(Engine::Comet, "x86_64", &comet).unwrap();
        assert_eq!(found.asset.name, "comet-x86_64-unknown-linux-gnu");
        assert_eq!(found.service.unwrap().name, "dummy-service.zip");
        let umu = entry(
            "1.4.4",
            &[
                ("umu-launcher-1.4.4.fc44.x86_64.rpm", Some(SUM)),
                ("umu-launcher-1.4.4-zipapp.tar", Some(SUM)),
            ],
        );
        assert_eq!(
            pick(Engine::Umu, "aarch64", &umu).unwrap().asset.name,
            "umu-launcher-1.4.4-zipapp.tar"
        );
    }

    #[test]
    fn refuses_a_release_without_a_build_or_a_checksum() {
        // legendary 0.20.34 shipped a bare `legendary`.
        let old = entry("0.20.34", &[("legendary", Some(SUM))]);
        let err = pick(Engine::Legendary, "x86_64", &old).unwrap_err();
        assert!(format!("{err:#}").contains("no build for this machine"));
        let bare = entry("0.21.1", &[("legendary_linux_x64", None)]);
        let err = pick(Engine::Legendary, "x86_64", &bare).unwrap_err();
        assert!(format!("{err:#}").contains("no checksum"));
        let short = entry("0.21.1", &[("legendary_linux_x64", Some("sha256:abc"))]);
        assert!(pick(Engine::Legendary, "x86_64", &short).is_err());
        assert!(pick(Engine::Legendary, "riscv64", &old).is_err());
        let no_service = entry("v0.4.1", &[("comet-x86_64-unknown-linux-gnu", Some(SUM))]);
        assert!(pick(Engine::Comet, "x86_64", &no_service).is_err());
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zendo-engines-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn program(dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let file = dir.join(name);
        fs::write(&file, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        file
    }

    #[test]
    fn finds_zendos_copy_first_or_path_first() {
        let dir = scratch("find");
        let own = program(&dir.join("own/legendary"), "legendary");
        let on_path = program(&dir.join("bin"), "legendary");
        let heroic = program(&dir.join("heroic"), "legendary");
        let path = std::env::join_paths([dir.join("bin")]).unwrap();
        let heroics = [dir.join("heroic")];
        let own_dir = dir.join("own");
        let at = |first| find_in("legendary", Some(&own_dir), first, &path, &heroics).unwrap();
        assert_eq!(
            at(false),
            Located {
                path: own.clone(),
                origin: Origin::Zendo
            }
        );
        assert_eq!(
            at(true),
            Located {
                path: on_path,
                origin: Origin::Path
            }
        );
        fs::remove_file(&own).unwrap();
        fs::remove_file(dir.join("bin/legendary")).unwrap();
        assert_eq!(
            at(false),
            Located {
                path: heroic,
                origin: Origin::Heroic
            }
        );
        assert_eq!(
            find_in("nothing", Some(&own_dir), false, &path, &heroics),
            None
        );
        // Not executable: not found.
        fs::set_permissions(
            dir.join("heroic/legendary"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(
            find_in("legendary", Some(&own_dir), false, &path, &heroics),
            None
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn puts_a_binary_in_place_and_swaps_it_for_the_old_one() {
        let dir = scratch("place");
        let root = dir.join("engines");
        fs::create_dir_all(&root).unwrap();
        let download = dir.join("gogdl_linux_x86_64");
        fs::write(&download, "one").unwrap();
        let at = place(Engine::Gogdl, &download, "v1", &root).unwrap();
        assert_eq!(at, root.join("gogdl/gogdl"));
        assert_eq!(fs::read_to_string(&at).unwrap(), "one");
        assert!(is_executable(&at));
        assert_eq!(
            fs::read_to_string(root.join("gogdl/release")).unwrap(),
            "v1\n"
        );
        fs::write(&download, "two").unwrap();
        place(Engine::Gogdl, &download, "v2", &root).unwrap();
        assert_eq!(fs::read_to_string(&at).unwrap(), "two");
        let left: Vec<_> = fs::read_dir(&root).unwrap().collect();
        assert_eq!(left.len(), 1, "no partial or old folder left");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unpacks_umus_zipapp_as_umu_run() {
        let dir = scratch("umu");
        let source = dir.join("source/umu");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("umu-run"), "#!/usr/bin/env python3\n").unwrap();
        std::os::unix::fs::symlink("umu-run", source.join("umu_run.py")).unwrap();
        let tar = dir.join("umu-launcher-1.4.4-zipapp.tar");
        let status = Command::new("tar")
            .arg("-cf")
            .arg(&tar)
            .arg("-C")
            .arg(dir.join("source"))
            .arg("umu")
            .status()
            .unwrap();
        assert!(status.success());
        let root = dir.join("engines");
        fs::create_dir_all(&root).unwrap();
        let at = place(Engine::Umu, &tar, "1.4.4", &root).unwrap();
        assert_eq!(at, root.join("umu/umu-run"));
        assert!(is_executable(&at));
        assert_eq!(Engine::Umu.program(), "umu-run");
        let names: Vec<_> = fs::read_dir(root.join("umu"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "umu-run and release: {names:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn checks_a_download_against_its_digest() {
        let dir = scratch("sum");
        let file = dir.join("a");
        fs::write(&file, "abc").unwrap();
        // SHA-256 of "abc".
        let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        check_sha256(&file, good).unwrap();
        assert!(check_sha256(&file, &"0".repeat(64)).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_the_real_latest_releases() {
        for engine in super::Engine::ALL {
            let release = super::latest(engine).unwrap();
            println!(
                "{} {} {} {}",
                engine.label(),
                release.tag,
                release.asset.name,
                release.asset.size
            );
        }
    }

    /// Installs three engines into a throwaway home and runs them; run with
    /// `cargo test -p zendo-core engines::online::installs -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and writes to a throwaway home"]
    fn installs_and_runs_real_engines() {
        let data = std::env::var("XDG_DATA_HOME").unwrap_or_default();
        assert!(
            data.contains("zendo-engines-live"),
            "set XDG_DATA_HOME and XDG_CACHE_HOME to a scratch folder named zendo-engines-live"
        );
        for engine in [
            super::Engine::Gogdl,
            super::Engine::Nile,
            super::Engine::Umu,
        ] {
            let release = super::latest(engine).unwrap();
            let stop = std::sync::atomic::AtomicBool::new(false);
            let at = super::install(engine, &release, &stop, &mut |_, _| {}).unwrap();
            assert_eq!(super::find(engine.program()), Some(at.clone()));
            assert_eq!(engine.installed().as_deref(), Some(release.tag.as_str()));
            let out = std::process::Command::new(&at)
                .arg("--version")
                .output()
                .unwrap();
            println!(
                "{} {} -> {}{}",
                engine.label(),
                release.tag,
                String::from_utf8_lossy(&out.stdout).trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            );
            assert!(out.status.success(), "{} --version failed", engine.label());
        }
    }
}
