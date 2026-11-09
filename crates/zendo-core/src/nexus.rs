//! Nexus Mods (`https://api.nexusmods.com/v1`, with the user's personal API
//! key in the `apikey` header): checking the key, reading the `nxm://`
//! links the site's "Mod manager download" buttons open, turning one into
//! a file to download, and listing a mod's files so an update shows.
//!
//! Without Premium, Nexus gives a download only for a link made by the
//! site, whose `key` and `expires` say the user pressed the button there;
//! with Premium a file id is enough. Nexus allows 20,000 calls a day, then
//! 500 an hour; Zendo makes three per download and one per mod when it
//! checks for updates.

use std::fs::{self};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{launch, paths, protondb};

pub const SITE: &str = "https://www.nexusmods.com";
const API: &str = "https://api.nexusmods.com/v1";
/// Where the user finds their personal key ("API Access").
pub const KEY_PAGE: &str = "https://www.nexusmods.com/users/myaccount?tab=api";
/// How long the list of games Nexus has is kept before it is asked again.
const GAMES_KEPT: Duration = Duration::from_hours(7 * 24);

/// Who a key belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
    pub name: String,
    #[serde(default)]
    pub is_premium: bool,
}

/// What Nexus says of a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCheck {
    Accepted(User),
    Rejected,
}

/// One `nxm://` link: a file of a mod, and the site's leave to download it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The game's name on Nexus (`skyrimspecialedition`).
    pub domain: String,
    pub mod_id: u64,
    pub file_id: u64,
    /// The site's download key and when it runs out (Unix seconds); a
    /// Premium account needs neither.
    pub key: Option<String>,
    pub expires: Option<u64>,
}

impl Link {
    /// Reads `nxm://<game>/mods/<mod>/files/<file>?key=…&expires=…`.
    ///
    /// # Errors
    ///
    /// When it is not such a link; collections are not supported yet.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let text = text.trim();
        let rest = text
            .get(..6)
            .filter(|scheme| scheme.eq_ignore_ascii_case("nxm://"))
            .and_then(|_| text.get(6..))
            .context("not an nxm:// link")?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
        let [domain, "mods", mod_id, "files", file_id] = parts.as_slice() else {
            if parts.get(1) == Some(&"collections") {
                bail!("Nexus collections are not supported yet: download their mods one by one");
            }
            bail!("not a link to a mod's file: {path}");
        };
        let domain = domain.to_ascii_lowercase();
        if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
            bail!("not a game on Nexus: {domain}");
        }
        let number = |what: &str, value: &str| {
            value
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0)
                .with_context(|| format!("not a {what} id: {value}"))
        };
        let mut link = Self {
            domain,
            mod_id: number("mod", mod_id)?,
            file_id: number("file", file_id)?,
            key: None,
            expires: None,
        };
        for pair in query.split('&') {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = crate::http::percent_decode(value);
            match name {
                "key" if !value.is_empty() => link.key = Some(value),
                "expires" => link.expires = value.parse().ok(),
                _ => {}
            }
        }
        Ok(link)
    }

    /// The mod's page on the site, on its files tab.
    #[must_use]
    pub fn page(&self) -> String {
        mod_page_url(&self.domain, self.mod_id, Some(self.file_id))
    }
}

/// A mod's page, or one of its files on the files tab.
#[must_use]
pub fn mod_page_url(domain: &str, mod_id: u64, file_id: Option<u64>) -> String {
    match file_id {
        Some(file) => format!("{SITE}/{domain}/mods/{mod_id}?tab=files&file_id={file}"),
        None => format!("{SITE}/{domain}/mods/{mod_id}"),
    }
}

/// A game's mods on the site.
#[must_use]
pub fn game_url(domain: &str) -> String {
    format!("{SITE}/{domain}/mods/")
}

/// A mod, as its page says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModPage {
    /// Missing while the mod is hidden or under moderation.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: String,
}

/// One file of a mod.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileInfo {
    pub file_id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub size_kb: Option<u64>,
    /// `MAIN`, `OPTIONAL`, `OLD_VERSION`, `ARCHIVED`…
    #[serde(default)]
    pub category_name: Option<String>,
    #[serde(default)]
    pub uploaded_timestamp: i64,
}

/// A file the author marked as replaced by a newer one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileUpdate {
    pub old_file_id: u64,
    pub new_file_id: u64,
}

/// A mod's files, and which replace which.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Files {
    #[serde(default)]
    pub files: Vec<FileInfo>,
    #[serde(default)]
    pub file_updates: Vec<FileUpdate>,
}

impl Files {
    #[must_use]
    pub fn get(&self, file_id: u64) -> Option<&FileInfo> {
        self.files.iter().find(|file| file.file_id == file_id)
    }

    /// The files that replaced this one, in order, newest last.
    fn after(&self, file_id: u64) -> Vec<u64> {
        let mut chain = Vec::new();
        let mut at = file_id;
        // The newest when a file was replaced twice; a loop ends it.
        while let Some(next) = self
            .file_updates
            .iter()
            .filter(|update| update.old_file_id == at)
            .map(|update| update.new_file_id)
            .max()
        {
            if next == file_id || chain.contains(&next) {
                break;
            }
            chain.push(next);
            at = next;
        }
        chain
    }

    /// The newest file that replaced this one, when there is one.
    #[must_use]
    pub fn newest(&self, file_id: u64) -> Option<&FileInfo> {
        self.after(file_id)
            .iter()
            .rev()
            .find_map(|&next| self.get(next))
    }

    /// Whether `new` replaced `old`, directly or after others.
    #[must_use]
    pub fn replaces(&self, new: u64, old: u64) -> bool {
        self.after(old).contains(&new)
    }
}

/// A game Nexus has mods for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusGame {
    pub domain_name: String,
    pub name: String,
}

#[derive(Deserialize)]
struct DownloadUrl {
    #[serde(rename = "URI")]
    uri: String,
}

fn api_agent() -> ureq::Agent {
    crate::http::answering_agent(Duration::from_secs(30))
}

/// One call to the API, its reply read as `T`.
fn get<T: DeserializeOwned>(key: &str, path: &str, query: &[(&str, &str)]) -> anyhow::Result<T> {
    let mut request = api_agent()
        .get(format!("{API}/{path}"))
        .header("apikey", key.trim())
        .header("Application-Name", "Zendo")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .header("Accept", "application/json");
    for (name, value) in query {
        request = request.query(*name, *value);
    }
    let response = request.call().context("could not reach Nexus Mods")?;
    match response.status().as_u16() {
        200..=299 => {}
        401 => bail!("Nexus turned the key down: set it again in Settings"),
        403 if path.ends_with("download_link.json") => bail!(
            "Nexus gives this file only to Premium accounts, or from the site: press \
             Mod manager download on the mod's page"
        ),
        404 => bail!("Nexus does not have this (anymore): {path}"),
        410 => bail!("the download link from Nexus ran out: press the button on the site again"),
        429 => bail!("Nexus's limit of calls for now was reached; try again in an hour"),
        code => bail!("Nexus answered {code} to {path}"),
    }
    let text = response
        .into_body()
        .read_to_string()
        .with_context(|| format!("Nexus {path}"))?;
    serde_json::from_str(&text).with_context(|| format!("Nexus {path}: unexpected reply"))
}

/// Asks Nexus whose key this is.
///
/// # Errors
///
/// When Nexus cannot be reached or answers with anything but yes or no.
pub fn check_key(key: &str) -> anyhow::Result<KeyCheck> {
    let response = api_agent()
        .get(format!("{API}/users/validate.json"))
        .header("apikey", key.trim())
        .header("Application-Name", "Zendo")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .call()
        .context("could not reach Nexus Mods")?;
    match response.status().as_u16() {
        200..=299 => {
            let text = response.into_body().read_to_string()?;
            let user: User =
                serde_json::from_str(&text).context("Nexus users/validate: unexpected reply")?;
            Ok(KeyCheck::Accepted(user))
        }
        401 | 403 => Ok(KeyCheck::Rejected),
        code => bail!("Nexus answered {code}"),
    }
}

/// # Errors
///
/// When Nexus cannot be reached or does not have the mod.
pub fn mod_page(key: &str, domain: &str, mod_id: u64) -> anyhow::Result<ModPage> {
    get(key, &format!("games/{domain}/mods/{mod_id}.json"), &[])
}

/// # Errors
///
/// As [`mod_page`].
pub fn files(key: &str, domain: &str, mod_id: u64) -> anyhow::Result<Files> {
    get(
        key,
        &format!("games/{domain}/mods/{mod_id}/files.json"),
        &[],
    )
}

/// Where to download a file from: with the link's key without Premium.
///
/// # Errors
///
/// When Nexus refuses: no Premium and no key, or the key ran out.
pub fn download_url(key: &str, link: &Link) -> anyhow::Result<String> {
    let path = format!(
        "games/{}/mods/{}/files/{}/download_link.json",
        link.domain, link.mod_id, link.file_id
    );
    let expires = link.expires.map(|expires| expires.to_string());
    let query: Vec<(&str, &str)> = match (&link.key, &expires) {
        (Some(key), Some(expires)) => vec![("key", key), ("expires", expires)],
        _ => Vec::new(),
    };
    let urls: Vec<DownloadUrl> = get(key, &path, &query)?;
    urls.into_iter()
        .map(|url| url.uri)
        .find(|uri| uri.starts_with("https://"))
        .context("Nexus gave no place to download the file from")
}

/// Downloads `url` into `to`, saying how far it got every 4 MiB; `stop`
/// ends it. A broken download leaves nothing.
///
/// # Errors
///
/// When it breaks off, is stopped, or cannot be written.
pub fn download(
    url: &str,
    to: &Path,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<()> {
    let result = download_into(url, to, stop, progress);
    if result.is_err() {
        let _ = fs::remove_file(to);
    }
    result
}

fn download_into(
    url: &str,
    to: &Path,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<()> {
    crate::http::download(url, to, "the file from Nexus", None, stop, progress)
}

/// A file downloaded from Nexus, ready to add as a mod.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub archive: PathBuf,
    /// The mod's name on its page.
    pub mod_name: String,
    pub file: FileInfo,
    /// The mod's files, to tell an update from a new mod.
    pub files: Files,
}

/// Asks Nexus about the link's file and downloads it into `folder`.
///
/// # Errors
///
/// When Nexus refuses or the download fails.
pub fn fetch(
    key: &str,
    link: &Link,
    folder: &Path,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> anyhow::Result<Fetched> {
    let page = mod_page(key, &link.domain, link.mod_id)?;
    let files = files(key, &link.domain, link.mod_id)?;
    let file = files
        .get(link.file_id)
        .cloned()
        .context("Nexus does not list this file for the mod")?;
    let url = download_url(key, link)?;
    fs::create_dir_all(folder).with_context(|| format!("could not make {}", folder.display()))?;
    // Only the name: the API's text is not trusted to be a path.
    let name = Path::new(&file.file_name)
        .file_name()
        .map(|name| paths::safe_name(&name.to_string_lossy()))
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .unwrap_or_else(|| format!("{}-{}.zip", link.mod_id, link.file_id));
    let archive = folder.join(&name);
    let partial = folder.join(format!(".{name}.part"));
    download(&url, &partial, stop, progress)?;
    fs::rename(&partial, &archive)
        .with_context(|| format!("could not keep {}", archive.display()))?;
    Ok(Fetched {
        archive,
        mod_name: page
            .name
            .filter(|name| !name.is_empty())
            .unwrap_or(file.name.clone()),
        file,
        files,
    })
}

/// The mod a downloaded archive is a file of, and which file: by its
/// mod when known, else by the ids its name holds as Nexus names its
/// files ([`ids_in_name`]), the file being the one with the archive's
/// name. `None` when none matches.
///
/// # Errors
///
/// When Nexus cannot be asked.
pub fn identify(
    key: &str,
    domain: &str,
    archive: &Path,
    mod_id: Option<u64>,
) -> anyhow::Result<Option<(u64, Fetched)>> {
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let candidates = match mod_id {
        Some(id) => vec![id],
        None => ids_in_name(&name).into_iter().take(3).collect(),
    };
    for id in candidates {
        let files = match files(key, domain, id) {
            Ok(files) => files,
            // Not a mod of this game: the next id, maybe.
            Err(_) if mod_id.is_none() => continue,
            Err(err) => return Err(err),
        };
        let Some(file) = files
            .files
            .iter()
            .find(|file| file.file_name == name)
            .cloned()
        else {
            continue;
        };
        let page = mod_page(key, domain, id)?;
        let fetched = Fetched {
            archive: archive.to_path_buf(),
            mod_name: page
                .name
                .filter(|name| !name.is_empty())
                .unwrap_or(file.name.clone()),
            file,
            files,
        };
        return Ok(Some((id, fetched)));
    }
    Ok(None)
}

/// The mod ids a name could hold, as Nexus names its files
/// (`<name>-<mod id>-<version>-<upload time>.<ext>`): the parts all digits
/// between the name and the upload time, the likeliest first.
#[must_use]
pub fn ids_in_name(name: &str) -> Vec<u64> {
    let stem = name
        .rsplit_once('.')
        .filter(|(_, ext)| ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .map_or(name, |(stem, _)| stem);
    let parts: Vec<&str> = stem.split('-').collect();
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    // The upload time: Unix seconds, ten digits these days.
    let Some((last, middle)) = parts.split_last() else {
        return Vec::new();
    };
    if parts.len() < 3 || last.len() < 9 || !digits(last) {
        return Vec::new();
    }
    middle[1..]
        .iter()
        .filter(|part| digits(part))
        .filter_map(|part| part.parse().ok())
        .filter(|id| *id > 0)
        .collect()
}

/// The folder where Zendo's Nexus window keeps the site's cookies, so the
/// site remembers the sign-in: `$XDG_DATA_HOME/zendo/nexus-web`.
#[must_use]
pub fn web_profile_dir() -> Option<PathBuf> {
    paths::data_home().map(|data| data.join("zendo/nexus-web"))
}

fn games_path() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/nexus-games.json"))
}

/// The games Nexus has, as last asked; empty before the first time.
#[must_use]
pub fn cached_games() -> Vec<NexusGame> {
    games_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Asks Nexus for its games when the copy kept is older than a week.
///
/// # Errors
///
/// When Nexus cannot be reached or the copy cannot be written.
pub fn refresh_games(key: &str) -> anyhow::Result<()> {
    let path = games_path().context("no cache folder")?;
    let fresh = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age < GAMES_KEPT);
    if fresh {
        return Ok(());
    }
    let games: Vec<NexusGame> = get(key, "games.json", &[])?;
    let text = serde_json::to_vec(&games)?;
    match paths::write_atomic(&path, &text) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("could not write {}", path.display())),
    }
}

/// The game on Nexus with this title, or with the part of it before
/// " - " or ":" (`Fallout 3 - Game of the Year Edition` is Nexus's
/// `Fallout 3`).
#[must_use]
pub fn domain_for(title: &str, games: &[NexusGame]) -> Option<String> {
    let short = [" - ", ":"]
        .iter()
        .filter_map(|mark| title.split_once(mark).map(|(before, _)| before))
        .min_by_key(|before| before.len());
    [Some(title), short]
        .into_iter()
        .flatten()
        .map(protondb::plain)
        .filter(|wanted| !wanted.is_empty())
        .find_map(|wanted| {
            games
                .iter()
                .find(|game| protondb::plain(&game.name) == wanted)
                .map(|game| game.domain_name.clone())
        })
}

/// The name Nexus gives a game, or its domain.
#[must_use]
pub fn game_name(domain: &str, games: &[NexusGame]) -> String {
    games
        .iter()
        .find(|game| game.domain_name == domain)
        .map_or_else(|| domain.to_owned(), |game| game.name.clone())
}

/// The desktop file that hands `nxm://` links to Zendo.
pub const HANDLER: &str = "zendo-nxm.desktop";

/// What opens `nxm://` links on this desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handler {
    Zendo,
    /// Zendo's desktop file, starting another copy of Zendo (a build
    /// folder's, before a package was installed): its `Exec` line.
    OtherZendo(String),
    /// Another program's desktop file.
    Other(String),
    Nothing,
}

/// Asks `xdg-mime` what opens `nxm://` links.
#[must_use]
pub fn handler() -> Handler {
    let Some(xdg_mime) = launch::which("xdg-mime") else {
        return Handler::Nothing;
    };
    let answer = std::process::Command::new(xdg_mime)
        .args(["query", "default", "x-scheme-handler/nxm"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();
    match answer.as_str() {
        "" => Handler::Nothing,
        HANDLER => zendo_handler(),
        _ => Handler::Other(answer),
    }
}

/// Whether Zendo's desktop file starts this Zendo.
fn zendo_handler() -> Handler {
    let written = paths::data_home()
        .map(|dir| dir.join("applications").join(HANDLER))
        .and_then(|path| std::fs::read_to_string(path).ok());
    let (Some(written), Ok(exe)) = (written, std::env::current_exe()) else {
        return Handler::Zendo;
    };
    which_zendo(&written, &exe)
}

fn which_zendo(written: &str, exe: &Path) -> Handler {
    if written == handler_entry(exe) {
        return Handler::Zendo;
    }
    let exec = written
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .unwrap_or_default();
    Handler::OtherZendo(exec.trim_end_matches(" %u").trim_matches('"').to_owned())
}

/// Makes this Zendo, the program at `exe`, open `nxm://` links: a hidden
/// desktop file in the user's applications, set as the default with
/// `xdg-mime`.
///
/// # Errors
///
/// When the file cannot be written or `xdg-mime` is missing or fails.
pub fn become_handler(exe: &Path) -> anyhow::Result<PathBuf> {
    let xdg_mime = launch::which("xdg-mime")
        .context("xdg-mime is missing (xdg-utils): it sets what opens links")?;
    let path = paths::data_home()
        .context("no home folder for the desktop file")?
        .join("applications")
        .join(HANDLER);
    paths::write_atomic(&path, handler_entry(exe).as_bytes())
        .with_context(|| format!("could not write {}", path.display()))?;
    let status = std::process::Command::new(xdg_mime)
        .args(["default", HANDLER, "x-scheme-handler/nxm"])
        .status()
        .context("could not run xdg-mime")?;
    if !status.success() {
        bail!("xdg-mime could not set Zendo for nxm:// links ({status})");
    }
    Ok(path)
}

fn handler_entry(exe: &Path) -> String {
    // The desktop entry spec's quoting: inside double quotes, `"`, `` ` ``,
    // `$` and `\` take a backslash; `%` is doubled.
    let mut quoted = String::from('"');
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Zendo (Nexus Mods links)\n\
         Comment=Downloads mods from Nexus Mods' Mod manager download buttons\n\
         Exec={quoted} %u\n\
         MimeType=x-scheme-handler/nxm;\n\
         NoDisplay=true\n\
         Terminal=false\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_mod_id_in_a_nexus_file_name() {
        assert_eq!(
            ids_in_name("Content Patcher 2.5.3-1915-2-5-3-1735000000.zip"),
            [1915, 2, 5, 3]
        );
        assert_eq!(
            ids_in_name("SkyUI_5_2_SE-12604-5-2SE.7z"),
            Vec::<u64>::new()
        );
        assert_eq!(
            ids_in_name("BepInEx_win_x64_5.4.23.6.zip"),
            Vec::<u64>::new()
        );
        assert_eq!(ids_in_name("Mod-1700000000"), Vec::<u64>::new());
        assert_eq!(ids_in_name("Mod-77-1700000000"), [77]);
    }

    #[test]
    fn tells_this_zendo_from_another_copy() {
        let installed = Path::new("/usr/bin/zendo");
        let built = Path::new("/home/me/zendo/target/release/zendo");
        assert_eq!(
            which_zendo(&handler_entry(installed), installed),
            Handler::Zendo
        );
        assert_eq!(
            which_zendo(&handler_entry(built), installed),
            Handler::OtherZendo(built.display().to_string())
        );
    }

    #[test]
    fn reads_a_mod_manager_link() {
        let link = Link::parse(
            "nxm://SkyrimSpecialEdition/mods/266/files/12345?key=ab%2Dc_9&expires=1790000000&user_id=1",
        )
        .unwrap();
        assert_eq!(
            link,
            Link {
                domain: "skyrimspecialedition".into(),
                mod_id: 266,
                file_id: 12345,
                key: Some("ab-c_9".into()),
                expires: Some(1_790_000_000),
            }
        );
        let premium = Link::parse("nxm://catquest/mods/7/files/30").unwrap();
        assert_eq!((&premium.key, premium.expires), (&None, None));
        assert_eq!(
            premium.page(),
            "https://www.nexusmods.com/catquest/mods/7?tab=files&file_id=30"
        );
        for bad in [
            "https://nexusmods.com/x",
            "nxm://cat quest/mods/1/files/2",
            "nxm://catquest/mods/x/files/2",
            "nxm://catquest/mods/1/files/0",
            "nxm://catquest/mods/1",
            "nxm://../mods/1/files/2",
        ] {
            assert!(Link::parse(bad).is_err(), "{bad}");
        }
        let err = Link::parse("nxm://skyrimspecialedition/collections/abc/revisions/3")
            .unwrap_err()
            .to_string();
        assert!(err.contains("collections"), "{err}");
    }

    #[test]
    fn writes_a_desktop_file_that_opens_links() {
        let entry = handler_entry(Path::new("/home/me/my games/zendo"));
        assert!(
            entry.contains("Exec=\"/home/me/my games/zendo\" %u\n"),
            "{entry}"
        );
        assert!(entry.contains("MimeType=x-scheme-handler/nxm;\n"));
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\n"));
        let odd = handler_entry(Path::new("/a$b/100%/z\"y"));
        assert!(odd.contains(r#"Exec="/a\$b/100%%/z\"y" %u"#), "{odd}");
    }

    #[test]
    fn follows_a_files_updates_to_the_newest() {
        let files: Files = serde_json::from_str(
            r#"{"files":[
                {"file_id":1,"name":"Main","version":"1.0","file_name":"m-1.zip","category_name":"OLD_VERSION","uploaded_timestamp":1},
                {"file_id":2,"name":"Main","version":"1.1","file_name":"m-2.zip","category_name":"OLD_VERSION","uploaded_timestamp":2},
                {"file_id":3,"name":"Main","version":"1.2","file_name":"m-3.zip","category_name":"MAIN","uploaded_timestamp":3},
                {"file_id":9,"name":"Extra","version":"1.0","file_name":"x.zip","category_name":"OPTIONAL","uploaded_timestamp":3}],
             "file_updates":[
                {"old_file_id":1,"new_file_id":2,"old_file_name":"m-1.zip","new_file_name":"m-2.zip","uploaded_timestamp":2},
                {"old_file_id":2,"new_file_id":3,"old_file_name":"m-2.zip","new_file_name":"m-3.zip","uploaded_timestamp":3}]}"#,
        )
        .unwrap();
        assert_eq!(files.newest(1).map(|file| file.file_id), Some(3));
        assert_eq!(files.newest(3).map(|file| file.file_id), None);
        assert_eq!(files.newest(9), None);
        assert!(files.replaces(3, 1));
        assert!(!files.replaces(9, 1));
        assert!(!files.replaces(1, 3));
        // An author's loop ends rather than spinning.
        let mut looped = files.clone();
        looped.file_updates.push(FileUpdate {
            old_file_id: 3,
            new_file_id: 1,
        });
        assert_eq!(looped.after(1), vec![2, 3]);
    }

    #[test]
    fn finds_a_game_on_nexus_by_its_title() {
        let games = vec![
            NexusGame {
                domain_name: "catquest".into(),
                name: "Cat Quest".into(),
            },
            NexusGame {
                domain_name: "skyrimspecialedition".into(),
                name: "Skyrim Special Edition".into(),
            },
        ];
        assert_eq!(domain_for("Cat Quest", &games).as_deref(), Some("catquest"));
        assert_eq!(
            domain_for("CAT QUEST™", &games).as_deref(),
            Some("catquest")
        );
        assert_eq!(domain_for("Dog Quest", &games), None);
        let fallout = vec![
            NexusGame {
                domain_name: "fallout".into(),
                name: "Fallout".into(),
            },
            NexusGame {
                domain_name: "fallout3".into(),
                name: "Fallout 3".into(),
            },
        ];
        assert_eq!(
            domain_for("Fallout 3 - Game of the Year Edition", &fallout).as_deref(),
            Some("fallout3")
        );
        assert_eq!(
            domain_for("Fallout 3: Broken Steel", &fallout).as_deref(),
            Some("fallout3")
        );
        assert_eq!(domain_for("Fallout", &fallout).as_deref(), Some("fallout"));
        assert_eq!(game_name("catquest", &games), "Cat Quest");
        assert_eq!(game_name("unknown", &games), "unknown");
    }

    /// Asks the real Nexus with the key in Zendo's settings, as in
    /// `cargo test -p zendo-core nexus -- --ignored`.
    #[test]
    #[ignore = "asks Nexus Mods over the network, with the key in Settings"]
    fn asks_nexus_about_a_key_and_a_mod() {
        let config = crate::config::Config::load().unwrap();
        let key = config.nexus_key.expect("a Nexus key in Settings");
        let KeyCheck::Accepted(user) = check_key(&key).unwrap() else {
            panic!("the key was turned down");
        };
        assert_ne!(user.name.len(), 0);
        assert_eq!(check_key("not-a-key").unwrap(), KeyCheck::Rejected);
        // SMAPI, Stardew Valley's mod loader.
        let page = mod_page(&key, "stardewvalley", 2400).unwrap();
        assert!(page.name.is_some_and(|name| !name.is_empty()));
        let files = files(&key, "stardewvalley", 2400).unwrap();
        assert_ne!(files.files.len(), 0);
        refresh_games(&key).unwrap();
        let games = cached_games();
        assert_eq!(
            domain_for("Stardew Valley", &games).as_deref(),
            Some("stardewvalley")
        );
    }
}
