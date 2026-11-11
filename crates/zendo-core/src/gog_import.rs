//! Taking a GOG game already on disk as Zendo's own, as Heroic does:
//! `gogdl import <folder>` reads what the folder's `goggame-<id>.info`
//! says (the game, its build and language, the DLC there), and gogdl's
//! manifest for that build is fetched from GOG's content system, so that
//! updates and repairs compare against the files there instead of
//! downloading the whole game again (gogdl's repair refuses without one).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use flate2::read::ZlibDecoder;
use serde::Deserialize;

use crate::http;
use crate::stores::Store;

/// GOG's builds of a game for one system.
const BUILDS: &str = "https://content-system.gog.com/products";

/// The most a manifest may take, packed or not; a big game's is a few MiB.
const MANIFEST_LIMIT: u64 = 256 << 20;

/// What gogdl read in a game's folder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Folder {
    /// GOG's id for the game; a Linux installer's folder may not say.
    #[serde(rename = "appName")]
    pub app: Option<String>,
    #[serde(rename = "buildId")]
    pub build: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "installedLanguage")]
    pub language: Option<String>,
    /// The ids of the DLC in the folder.
    #[serde(default)]
    pub dlcs: Vec<String>,
    /// `windows`, `osx` or `linux`.
    pub platform: String,
    #[serde(rename = "versionName")]
    pub version: Option<String>,
}

/// Asks gogdl what the folder holds.
///
/// # Errors
///
/// When gogdl fails or finds no GOG game there.
pub(crate) fn read_folder(folder: &Path) -> anyhow::Result<Folder> {
    let output = Store::Gog.run(&["import", &folder.to_string_lossy()])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // It prints one line of JSON, or a sentence when it found no game.
    let found = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<Folder>(line).ok());
    match found {
        Some(found) if output.status.success() => Ok(found),
        _ => {
            let said = crate::stores::last_line(&output);
            let why = if said.is_empty() {
                String::new()
            } else {
                format!(" ({said})")
            };
            bail!(
                "GOG's files are not in {}{why}: choose the folder that has the game's goggame-….info",
                folder.display()
            );
        }
    }
}

#[derive(Debug, Deserialize)]
struct Builds {
    #[serde(default)]
    items: Vec<Build>,
}

#[derive(Debug, Deserialize)]
struct Build {
    #[serde(rename = "build_id")]
    id: String,
    #[serde(default)]
    generation: u32,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    urls: Vec<Url>,
    /// Older answers give one link instead.
    #[serde(default)]
    link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Url {
    url: String,
}

/// Fetches gogdl's manifest for the build in the folder (or the latest
/// one outside a branch when the folder does not say) and writes it where
/// gogdl reads it, with the system, language and DLC gogdl adds to it.
/// Linux installers' games have none.
///
/// # Errors
///
/// When GOG cannot be reached or answers
/// with something else.
pub(crate) fn restore_manifest(app: &str, found: &Folder) -> anyhow::Result<()> {
    if found.platform == "linux" {
        return Ok(());
    }
    let agent = http::agent(Duration::from_mins(1));
    let mut request = agent.get(format!(
        "{BUILDS}/{app}/os/{}/builds?generation=2",
        found.platform
    ));
    // GOG lists a game's public builds to anyone; signed in, the others too.
    if let Ok(Some(credentials)) = Store::Gog.gog_token() {
        request = request.header(
            "Authorization",
            format!("Bearer {}", credentials.access_token),
        );
    }
    let text = request
        .call()
        .context("could not ask GOG for the game's builds")?
        .into_body()
        .read_to_string()
        .context("could not read GOG's builds")?;
    let builds: Builds = serde_json::from_str(&text).context("could not read GOG's builds")?;
    let build = pick_build(&builds.items, found.build.as_deref())
        .context("GOG lists no build of the game for this system")?;
    let url = build
        .urls
        .first()
        .map(|url| url.url.clone())
        .or_else(|| build.link.clone())
        .context("GOG gave no address for the build's manifest")?;
    let packed = agent
        .get(&url)
        .call()
        .context("could not fetch the game's manifest from GOG")?
        .into_body()
        .with_config()
        .limit(MANIFEST_LIMIT)
        .read_to_vec()
        .context("could not fetch the game's manifest from GOG")?;
    let text = if build.generation == 2 {
        unpack(&packed)?
    } else {
        packed
    };
    let manifest = with_install(&text, found)?;
    let path = manifest_path(app).context("no home folder for gogdl's files")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("could not make {}", dir.display()))?;
    }
    crate::paths::write_atomic(&path, &manifest)
        .with_context(|| format!("could not write {}", path.display()))
}

/// Where gogdl keeps the manifest of a game's last install.
fn manifest_path(app: &str) -> Option<PathBuf> {
    Some(Store::Gog.dir()?.join("heroic_gogdl/manifests").join(app))
}

/// The build in the folder, or else the latest outside a branch.
fn pick_build<'a>(builds: &'a [Build], wanted: Option<&str>) -> Option<&'a Build> {
    wanted
        .and_then(|wanted| builds.iter().find(|build| build.id == wanted))
        .or_else(|| builds.iter().find(|build| build.branch.is_none()))
}

/// A generation 2 manifest, packed with zlib.
fn unpack(packed: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut text = Vec::new();
    ZlibDecoder::new(packed)
        .take(MANIFEST_LIMIT)
        .read_to_end(&mut text)
        .context("could not unpack the game's manifest")?;
    Ok(text)
}

/// The manifest with what gogdl adds when it installs: the system, the
/// language installed and the DLC, as `{"id": …}`.
fn with_install(text: &[u8], found: &Folder) -> anyhow::Result<Vec<u8>> {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(text).context("GOG's manifest is not JSON")?;
    let object = manifest
        .as_object_mut()
        .context("GOG's manifest is not an object")?;
    object.insert("HGLPlatform".into(), found.platform.clone().into());
    object.insert(
        "HGLInstallLanguage".into(),
        found
            .language
            .clone()
            .unwrap_or_else(|| crate::downloads::GOG_LANGUAGE.to_owned())
            .into(),
    );
    object.insert(
        "HGLdlcs".into(),
        found
            .dlcs
            .iter()
            .map(|id| serde_json::json!({ "id": id }))
            .collect(),
    );
    Ok(serde_json::to_vec(&manifest)?)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    fn build(id: &str, branch: Option<&str>) -> Build {
        Build {
            id: id.into(),
            generation: 2,
            branch: branch.map(Into::into),
            urls: Vec::new(),
            link: None,
        }
    }

    #[test]
    fn reads_what_gogdl_found_in_a_folder() {
        let line = r#"{"appName": "1423049311", "buildId": "601", "title": "Cyberpunk 2077", "tasks": null, "installedLanguage": "pt-BR", "dlcs": ["1256837418"], "platform": "windows", "versionName": "2.5"}"#;
        let found: Folder = serde_json::from_str(line).unwrap();
        assert_eq!(found.app.as_deref(), Some("1423049311"));
        assert_eq!(found.language.as_deref(), Some("pt-BR"));
        assert_eq!(found.dlcs, ["1256837418"]);
        // A Linux installer's folder may not name the game.
        let linux = r#"{"appName": null, "buildId": null, "title": "T", "tasks": null, "installedLanguage": "en-US", "dlcs": [], "platform": "linux", "versionName": "1"}"#;
        assert_eq!(serde_json::from_str::<Folder>(linux).unwrap().app, None);
    }

    #[test]
    fn picks_the_folders_build_or_the_latest_outside_a_branch() {
        let builds = [build("3", Some("beta")), build("2", None), build("1", None)];
        assert_eq!(pick_build(&builds, Some("1")).unwrap().id, "1");
        assert_eq!(pick_build(&builds, Some("9")).unwrap().id, "2");
        assert_eq!(pick_build(&builds, None).unwrap().id, "2");
    }

    #[test]
    fn unpacks_a_manifest_and_adds_what_gogdl_adds() {
        let mut packed = ZlibEncoder::new(Vec::new(), Compression::default());
        packed
            .write_all(br#"{"version": 2, "baseProductId": "1", "depots": []}"#)
            .unwrap();
        let text = unpack(&packed.finish().unwrap()).unwrap();
        let found = Folder {
            app: Some("1".into()),
            build: None,
            title: None,
            language: None,
            dlcs: vec!["7".into()],
            platform: "windows".into(),
            version: None,
        };
        let manifest: serde_json::Value =
            serde_json::from_slice(&with_install(&text, &found).unwrap()).unwrap();
        assert_eq!(manifest["baseProductId"], "1");
        assert_eq!(manifest["HGLPlatform"], "windows");
        assert_eq!(manifest["HGLInstallLanguage"], "en-US");
        assert_eq!(manifest["HGLdlcs"], serde_json::json!([{ "id": "7" }]));
    }
}
