//! GE-Proton, downloaded from its GitHub releases into Steam's
//! compatibility tools folder (`~/.local/share/Steam/compatibilitytools.d`),
//! where Steam, Heroic, umu and Zendo's own launch options all find it.
//!
//! Each release has a tarball and its `.sha512sum`: `GE-Proton11-12-x86_64.tar.gz`
//! since GE-Proton11, `GE-Proton10-4.tar.gz` before. The tarball is
//! downloaded into Zendo's cache, checked against the sum, unpacked with
//! `tar` into a folder beside the others that is renamed into place only
//! when complete, and deleted.

use std::fs::{self};

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::paths;
use crate::proton::Proton;

const RELEASES: &str = "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases";

/// A GE-Proton release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `GE-Proton11-12`.
    pub name: String,
    /// `2026-11-03`.
    pub published: String,
    pub tarball: String,
    pub size: u64,
    pub checksum: Option<String>,
}

/// The folder Zendo downloads into: the user's own compatibility tools
/// folder, which Steam reads.
#[must_use]
pub fn tools_dir() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("Steam/compatibilitytools.d"))
}

fn agent() -> ureq::Agent {
    crate::http::download_agent()
}

/// How many releases a page of GitHub's list holds.
pub const PAGE: usize = 30;

/// A page of the releases with a build for this machine, newest first
/// (page 1 the latest), and whether GitHub has more after it.
///
/// # Errors
///
/// When GitHub cannot be reached or answers oddly (it allows 60 requests an
/// hour without an account).
pub fn releases(page: u32) -> anyhow::Result<(Vec<Release>, bool)> {
    #[derive(Deserialize)]
    struct Entry {
        tag_name: String,
        published_at: Option<String>,
        #[serde(default)]
        prerelease: bool,
        assets: Vec<Asset>,
    }
    #[derive(Deserialize)]
    struct Asset {
        name: String,
        size: u64,
        browser_download_url: String,
    }
    let body = agent()
        .get(RELEASES)
        .query("per_page", PAGE.to_string())
        .query("page", page.to_string())
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("could not reach GitHub")?
        .into_body()
        .read_to_string()?;
    let entries: Vec<Entry> =
        serde_json::from_str(&body).context("GitHub's list of releases answered oddly")?;
    let arch = std::env::consts::ARCH;
    let more = entries.len() == PAGE;
    let releases = entries
        .into_iter()
        .filter(|entry| !entry.prerelease)
        .filter_map(|entry| {
            let name = entry.tag_name;
            let wanted = [format!("{name}-{arch}.tar.gz"), format!("{name}.tar.gz")];
            let tarball = entry
                .assets
                .iter()
                .find(|asset| wanted.contains(&asset.name))?;
            let sum = format!("{}.sha512sum", tarball.name.trim_end_matches(".tar.gz"));
            let checksum = entry
                .assets
                .iter()
                .find(|asset| asset.name == sum)
                .map(|asset| asset.browser_download_url.clone());
            Some(Release {
                published: entry
                    .published_at
                    .map(|date| date.chars().take(10).collect())
                    .unwrap_or_default(),
                tarball: tarball.browser_download_url.clone(),
                size: tarball.size,
                checksum,
                name,
            })
        })
        .collect();
    Ok((releases, more))
}

/// Whether a Proton found on the machine is this release.
#[must_use]
pub fn is_release(proton: &Proton, release: &Release) -> bool {
    proton.name == release.name
        || proton.name == format!("{}-{}", release.name, std::env::consts::ARCH)
}

/// Downloads, checks and unpacks a release, reporting the bytes downloaded
/// and the whole as it goes; `stop` set ends it early. Returns the folder
/// it went into.
///
/// # Errors
///
/// When the download fails or is stopped, the sum does not match, or the
/// tarball cannot be unpacked; nothing is left behind then.
pub fn install(
    release: &Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    let tools = tools_dir().context("no home folder for Steam's compatibility tools")?;
    let cache = paths::cache_home()
        .context("no cache folder to download into")?
        .join("zendo/ge-proton");
    fs::create_dir_all(&cache).with_context(|| format!("could not make {}", cache.display()))?;
    clear_leftovers(&cache, &tools);
    let tarball = cache.join(format!("{}.tar.gz", release.name));
    let result = download(release, &tarball, stop, progress)
        .and_then(|()| check(release, &tarball))
        .and_then(|()| unpack(&tarball, &tools, &release.name));
    let _ = fs::remove_file(&tarball);
    result
}

/// What a download Zendo was closed in the middle of left: tarballs in
/// the cache, and half-unpacked folders beside the Protons.
fn clear_leftovers(cache: &Path, tools: &Path) {
    for entry in fs::read_dir(cache).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().ends_with(".tar.gz") {
            let _ = fs::remove_file(entry.path());
        }
    }
    for entry in fs::read_dir(tools).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".zendo-") && name.ends_with(".partial") {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn download(
    release: &Release,
    to: &Path,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<()> {
    let size = Some(release.size).filter(|&size| size > 0);
    crate::http::download(
        &release.tarball,
        to,
        &release.name,
        size,
        stop,
        &mut |done, total| progress(done, total.unwrap_or(done)),
    )
}

/// Compares the tarball with the release's `.sha512sum`, through
/// `sha512sum`.
fn check(release: &Release, tarball: &Path) -> anyhow::Result<()> {
    let Some(url) = &release.checksum else {
        bail!(
            "{} has no checksum to check the download against",
            release.name
        );
    };
    let sums = agent()
        .get(url)
        .call()
        .context("could not download the checksum")?
        .into_body()
        .read_to_string()?;
    let wanted = sums
        .split_whitespace()
        .next()
        .context("the checksum file is empty")?
        .to_lowercase();
    let output = Command::new("sha512sum")
        .arg(tarball)
        .stdin(Stdio::null())
        .output()
        .context("could not run sha512sum")?;
    if !output.status.success() {
        bail!("sha512sum failed on the download");
    }
    let found = String::from_utf8_lossy(&output.stdout);
    if found.split_whitespace().next() != Some(wanted.as_str()) {
        bail!(
            "the download of {} does not match its checksum",
            release.name
        );
    }
    Ok(())
}

/// Unpacks into a hidden folder beside the others, then moves the Proton
/// folder in it into place.
fn unpack(tarball: &Path, tools: &Path, name: &str) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(tools).with_context(|| format!("could not make {}", tools.display()))?;
    let partial = tools.join(format!(".zendo-{name}.partial"));
    if partial.exists() {
        fs::remove_dir_all(&partial)
            .with_context(|| format!("could not clear {}", partial.display()))?;
    }
    fs::create_dir(&partial)?;
    let result = (|| {
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(tarball)
            .arg("-C")
            .arg(&partial)
            .arg("--no-same-owner")
            .stdin(Stdio::null())
            .status()
            .context("could not run tar")?;
        if !status.success() {
            bail!("tar could not unpack {name}");
        }
        // The tarball holds one folder, named for the release.
        let folder = fs::read_dir(&partial)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.join("proton").is_file())
            .with_context(|| format!("{name}'s tarball holds no Proton"))?;
        let target = tools.join(folder.file_name().context("an odd folder name")?);
        if target.exists() {
            bail!("{} is already there", target.display());
        }
        fs::rename(&folder, &target)
            .with_context(|| format!("could not move {name} into place"))?;
        Ok(target)
    })();
    let _ = fs::remove_dir_all(&partial);
    result
}

/// Whether Zendo may remove this Proton: one in the user's own tools
/// folder, not a distribution's or one Steam installed.
#[must_use]
pub fn removable(proton: &Proton) -> bool {
    let Some(tools) = tools_dir().and_then(|dir| fs::canonicalize(dir).ok()) else {
        return false;
    };
    fs::symlink_metadata(&proton.path).is_ok_and(|meta| meta.is_dir())
        && fs::canonicalize(&proton.path).is_ok_and(|path| path.parent() == Some(tools.as_path()))
}

/// Deletes a Proton from the user's own tools folder.
///
/// # Errors
///
/// When it is not one Zendo may remove ([`removable`]), or cannot be
/// deleted.
pub fn remove(proton: &Proton) -> anyhow::Result<()> {
    if !removable(proton) {
        bail!(
            "{} is not in {}; Zendo only removes what is there",
            proton.name,
            tools_dir().map_or_else(String::new, |dir| dir.display().to_string())
        );
    }
    fs::remove_dir_all(&proton.path)
        .with_context(|| format!("could not delete {}", proton.path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_a_tarball_into_place() {
        let dir = std::env::temp_dir().join(format!("zendo-ge-{}", std::process::id()));
        let source = dir.join("source/GE-Proton99-1");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("proton"), "#!/bin/sh\n").unwrap();
        let tarball = dir.join("GE-Proton99-1.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&tarball)
            .arg("-C")
            .arg(dir.join("source"))
            .arg("GE-Proton99-1")
            .status()
            .unwrap();
        assert!(status.success());
        let tools = dir.join("tools");
        let target = unpack(&tarball, &tools, "GE-Proton99-1").unwrap();
        assert_eq!(target, tools.join("GE-Proton99-1"));
        assert!(target.join("proton").is_file());
        let left: Vec<_> = fs::read_dir(&tools).unwrap().collect();
        assert_eq!(left.len(), 1, "no partial folder left");
        assert!(
            unpack(&tarball, &tools, "GE-Proton99-1").is_err(),
            "already there"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn knows_a_release_by_either_name() {
        let release = Release {
            name: "GE-Proton11-12".into(),
            published: "2026-11-03".into(),
            tarball: String::new(),
            size: 0,
            checksum: None,
        };
        let proton = |name: &str| Proton {
            name: name.into(),
            path: PathBuf::new(),
        };
        assert!(is_release(&proton("GE-Proton11-12"), &release));
        assert!(is_release(
            &proton(&format!("GE-Proton11-12-{}", std::env::consts::ARCH)),
            &release
        ));
        assert!(!is_release(&proton("GE-Proton11-1"), &release));
    }

    #[test]
    fn removes_only_from_the_users_tools_folder() {
        let system = Proton {
            name: "proton-cachyos-slr".into(),
            path: "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr".into(),
        };
        assert!(!removable(&system));
        assert!(remove(&system).is_err());
    }
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_real_releases() {
        let (releases, more) = super::releases(2).unwrap();
        println!("more after page 2: {more}");
        for release in releases {
            println!(
                "{} {} {} {}",
                release.name,
                release.published,
                release.size,
                release.checksum.is_some()
            );
        }
    }
}
