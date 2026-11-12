//! Proton builds downloaded from their GitHub releases into Steam's
//! compatibility tools folder (`~/.local/share/Steam/compatibilitytools.d`),
//! where Steam, Heroic, umu and Zendo's own launch options all find them:
//! GE-Proton and Proton-CachyOS.
//!
//! Each release has a tarball and its `.sha512sum`. GE-Proton's tarball is
//! `GE-Proton11-12-x86_64.tar.gz` since GE-Proton11, `GE-Proton10-4.tar.gz`
//! before; Proton-CachyOS's is `proton-cachyos-<version>-slr-<variant>.tar.xz`
//! (`x86_64`, `x86_64_v3` for CPUs that have it, `arm64`). The tarball is
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

/// Which Proton a release is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Build {
    #[default]
    Ge,
    CachyOs,
}

impl Build {
    pub const ALL: [Self; 2] = [Self::Ge, Self::CachyOs];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ge => "GE-Proton",
            Self::CachyOs => "Proton-CachyOS",
        }
    }

    fn releases_url(self) -> &'static str {
        match self {
            Self::Ge => "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases",
            Self::CachyOs => "https://api.github.com/repos/CachyOS/proton-cachyos/releases",
        }
    }
}

/// A Proton release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub build: Build,
    /// `GE-Proton11-12`, `proton-cachyos-11.0-20261015-slr-x86_64_v3`: the
    /// folder the tarball unpacks to.
    pub name: String,
    /// `2026-11-03`.
    pub published: String,
    /// The tarball's file name.
    pub file: String,
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

/// The Proton-CachyOS builds this CPU can run, best first.
fn cachyos_variants() -> Vec<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        let v3 = is_x86_feature_detected!("avx2")
            && is_x86_feature_detected!("bmi1")
            && is_x86_feature_detected!("bmi2")
            && is_x86_feature_detected!("fma")
            && is_x86_feature_detected!("f16c")
            && is_x86_feature_detected!("lzcnt")
            && is_x86_feature_detected!("movbe");
        if v3 {
            return vec!["x86_64_v3", "x86_64"];
        }
        vec!["x86_64"]
    }
    #[cfg(target_arch = "aarch64")]
    {
        vec!["arm64"]
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        Vec::new()
    }
}

/// A page of the releases with a build for this machine, newest first
/// (page 1 the latest), and whether GitHub has more after it.
///
/// # Errors
///
/// When GitHub cannot be reached or answers oddly (it allows 60 requests an
/// hour without an account).
pub fn releases(build: Build, page: u32) -> anyhow::Result<(Vec<Release>, bool)> {
    let body = agent()
        .get(build.releases_url())
        .query("per_page", PAGE.to_string())
        .query("page", page.to_string())
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("could not reach GitHub")?
        .into_body()
        .read_to_string()?;
    parse_releases(build, &body, std::env::consts::ARCH, &cachyos_variants())
}

fn parse_releases(
    build: Build,
    body: &str,
    arch: &str,
    variants: &[&str],
) -> anyhow::Result<(Vec<Release>, bool)> {
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
    let entries: Vec<Entry> =
        serde_json::from_str(body).context("GitHub's list of releases answered oddly")?;
    let more = entries.len() == PAGE;
    let releases = entries
        .into_iter()
        .filter(|entry| !entry.prerelease)
        .filter_map(|entry| {
            let wanted: Vec<String> = match build {
                Build::Ge => vec![
                    format!("{}-{arch}.tar.gz", entry.tag_name),
                    format!("{}.tar.gz", entry.tag_name),
                ],
                Build::CachyOs => {
                    let version = entry.tag_name.strip_prefix("cachyos-")?;
                    variants
                        .iter()
                        .map(|variant| format!("proton-cachyos-{version}-{variant}.tar.xz"))
                        .collect()
                }
            };
            // The first name wanted that the release has.
            let tarball = wanted
                .iter()
                .find_map(|want| entry.assets.iter().find(|asset| &asset.name == want))?;
            let name = tarball
                .name
                .trim_end_matches(".tar.gz")
                .trim_end_matches(".tar.xz")
                .to_owned();
            let sum = format!("{name}.sha512sum");
            let checksum = entry
                .assets
                .iter()
                .find(|asset| asset.name == sum)
                .map(|asset| asset.browser_download_url.clone());
            Some(Release {
                build,
                published: entry
                    .published_at
                    .map(|date| date.chars().take(10).collect())
                    .unwrap_or_default(),
                file: tarball.name.clone(),
                tarball: tarball.browser_download_url.clone(),
                size: tarball.size,
                checksum,
                name,
            })
        })
        .collect();
    Ok((releases, more))
}

/// The name without the CPU variant Proton-CachyOS adds.
fn without_variant(name: &str) -> &str {
    ["-x86_64_v3", "-x86_64", "-arm64"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(name)
}

/// Whether a Proton found on the machine is this release (for
/// Proton-CachyOS, whichever CPU variant of it).
#[must_use]
pub fn is_release(proton: &Proton, release: &Release) -> bool {
    match release.build {
        Build::Ge => {
            proton.name == release.name
                || proton.name == format!("{}-{}", release.name, std::env::consts::ARCH)
        }
        Build::CachyOs => without_variant(&proton.name) == without_variant(&release.name),
    }
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
    let tarball = cache.join(&release.file);
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
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".tar.gz") || name.ends_with(".tar.xz") {
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
    fs::create_dir(&partial).with_context(|| format!("could not make {}", partial.display()))?;
    let result = (|| {
        let status = Command::new("tar")
            .arg("-xf")
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
            build: Build::Ge,
            name: "GE-Proton11-12".into(),
            published: "2026-11-03".into(),
            file: "GE-Proton11-12-x86_64.tar.gz".into(),
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

    /// What GitHub answers for Proton-CachyOS, as seen on 2026-11-12.
    const CACHYOS: &str = r#"[{
        "tag_name": "cachyos-11.0-20261015-slr", "prerelease": false,
        "published_at": "2026-10-20T10:00:00Z",
        "assets": [
          {"name": "proton-cachyos-11.0-20261015-slr-arm64.tar.xz", "size": 367630532, "browser_download_url": "https://x/arm64"},
          {"name": "proton-cachyos-11.0-20261015-slr-x86_64.tar.xz", "size": 347335620, "browser_download_url": "https://x/v2"},
          {"name": "proton-cachyos-11.0-20261015-slr-x86_64.sha512sum", "size": 177, "browser_download_url": "https://x/v2.sum"},
          {"name": "proton-cachyos-11.0-20261015-slr-x86_64_v3.tar.xz", "size": 341465288, "browser_download_url": "https://x/v3"},
          {"name": "proton-cachyos-11.0-20261015-slr-x86_64_v3.sha512sum", "size": 180, "browser_download_url": "https://x/v3.sum"}
        ]}]"#;

    #[test]
    fn picks_the_cachyos_build_for_the_cpu() {
        let (v3, _) =
            parse_releases(Build::CachyOs, CACHYOS, "x86_64", &["x86_64_v3", "x86_64"]).unwrap();
        assert_eq!(v3.len(), 1);
        assert_eq!(v3[0].name, "proton-cachyos-11.0-20261015-slr-x86_64_v3");
        assert_eq!(
            v3[0].file,
            "proton-cachyos-11.0-20261015-slr-x86_64_v3.tar.xz"
        );
        assert_eq!(v3[0].checksum.as_deref(), Some("https://x/v3.sum"));
        assert_eq!(v3[0].published, "2026-10-20");
        let (plain, _) = parse_releases(Build::CachyOs, CACHYOS, "x86_64", &["x86_64"]).unwrap();
        assert_eq!(plain[0].tarball, "https://x/v2");
        let (arm, _) = parse_releases(Build::CachyOs, CACHYOS, "aarch64", &["arm64"]).unwrap();
        assert_eq!(arm[0].tarball, "https://x/arm64");
        assert_eq!(arm[0].checksum, None, "arm64 sum absent in this fixture");
        let (none, _) = parse_releases(Build::CachyOs, CACHYOS, "riscv64", &[]).unwrap();
        assert_eq!(none, []);
    }

    #[test]
    fn knows_a_cachyos_release_by_either_cpu_variant() {
        let (found, _) =
            parse_releases(Build::CachyOs, CACHYOS, "x86_64", &["x86_64_v3", "x86_64"]).unwrap();
        let proton = |name: &str| Proton {
            name: name.into(),
            path: PathBuf::new(),
        };
        let release = &found[0];
        assert!(is_release(
            &proton("proton-cachyos-11.0-20261015-slr-x86_64_v3"),
            release
        ));
        assert!(is_release(
            &proton("proton-cachyos-11.0-20261015-slr-x86_64"),
            release
        ));
        assert!(!is_release(
            &proton("proton-cachyos-11.0-20261014-slr-x86_64_v3"),
            release
        ));
        assert!(!is_release(&proton("proton-cachyos-slr"), release));
    }

    #[test]
    fn unpacks_an_xz_tarball() {
        let dir = std::env::temp_dir().join(format!("zendo-cachy-{}", std::process::id()));
        let source = dir.join("source/proton-cachyos-99.0-20260101-slr-x86_64");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("proton"), "#!/bin/sh\n").unwrap();
        let tarball = dir.join("p.tar.xz");
        let status = Command::new("tar")
            .arg("-cJf")
            .arg(&tarball)
            .arg("-C")
            .arg(dir.join("source"))
            .arg("proton-cachyos-99.0-20260101-slr-x86_64")
            .status()
            .unwrap();
        assert!(status.success());
        let tools = dir.join("tools");
        let target = unpack(&tarball, &tools, "proton-cachyos-99.0-20260101-slr-x86_64").unwrap();
        assert!(target.join("proton").is_file());
        fs::remove_dir_all(&dir).unwrap();
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
        for build in super::Build::ALL {
            let (releases, more) = super::releases(build, 1).unwrap();
            println!("{} more after page 1: {more}", build.label());
            for release in releases.into_iter().take(3) {
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
}
