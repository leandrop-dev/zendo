//! Images Zendo downloads, kept in `$XDG_CACHE_HOME/zendo/covers` and named
//! by game id: a portrait cover for the grid and a wide hero for the game's
//! page. A game's store image comes first (Heroic records both for its
//! games), `SteamGridDB` second when the user has set a key. A game with no
//! image anywhere is noted in `misses.json` and asked about again after a
//! week, not on every start.
//!
//! An image the user picks in the cover picker goes in `chosen/cover/` or
//! `chosen/hero/`, named `<id>.<SteamGridDB image id>.<ext>` (a new name
//! for each pick, so the window shows the new one), and comes before any
//! other, Steam's own included.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;

use crate::config::Config;
use crate::steamgriddb::{self, PORTRAIT, is_status};
use crate::{Game, Source, paths, time};

const EXTENSIONS: [&str; 3] = ["jpg", "png", "webp"];
const RETRY_AFTER: u64 = 7 * 24 * 3600;

/// Which image of a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Portrait, for the grid.
    Cover,
    /// Wide, for the top of the game's page.
    Hero,
}

impl Kind {
    /// What follows the game's id in the file name, before the extension.
    fn suffix(self) -> &'static str {
        match self {
            Self::Cover => "",
            Self::Hero => ".hero",
        }
    }

    /// The folder of the images chosen by hand, under the covers folder.
    fn chosen_folder(self) -> &'static str {
        match self {
            Self::Cover => "chosen/cover",
            Self::Hero => "chosen/hero",
        }
    }

    /// The key in `misses.json`; covers keep the bare id they had before
    /// heroes were fetched.
    fn miss_key(self, id: &str) -> String {
        match self {
            Self::Cover => id.to_owned(),
            Self::Hero => format!("{id}#hero"),
        }
    }
}

/// Something with a cover and a wide image: an installed game, or one the
/// user owns in a store.
pub trait Artwork {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn source(&self) -> Source;
    /// The image the store publishes.
    fn url(&self, kind: Kind) -> Option<&str>;
    /// The image already on disk.
    fn current(&self, kind: Kind) -> Option<&std::path::Path>;
}

impl Artwork for Game {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn source(&self) -> Source {
        self.source
    }
    fn url(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Cover => self.cover_url.as_deref(),
            Kind::Hero => self.hero_url.as_deref(),
        }
    }
    fn current(&self, kind: Kind) -> Option<&std::path::Path> {
        match kind {
            Kind::Cover => self.cover.as_deref(),
            Kind::Hero => self.hero.as_deref(),
        }
    }
}
#[must_use]
pub fn dir() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/covers"))
}

/// The file name for a game's image, without the extension: the id with
/// anything outside `[A-Za-z0-9._-]` as `_`, then the kind's suffix.
fn stem(id: &str, kind: Kind) -> String {
    let mut stem: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    stem.push_str(kind.suffix());
    stem
}

/// The image already downloaded for this game.
#[must_use]
pub fn cached(id: &str, kind: Kind) -> Option<PathBuf> {
    let dir = dir()?;
    let stem = stem(id, kind);
    EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!(
            "zendo/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/leandrop-dev/zendo)"
        ))
        // A refused key is an answer here, not a failure.
        .http_status_as_error(false)
        .build()
        .into()
}

/// What `SteamGridDB` says of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCheck {
    Accepted,
    Rejected,
}

/// Asks `SteamGridDB` whether it takes this key, with one small search.
///
/// # Errors
///
/// When the site cannot be reached or answers with anything but yes or no.
pub fn check_key(key: &str) -> anyhow::Result<KeyCheck> {
    let response = agent()
        .get(format!("{}/search/autocomplete/zendo", steamgriddb::API))
        .header("Authorization", format!("Bearer {}", key.trim()))
        .call()
        .context("could not reach SteamGridDB")?;
    match response.status().as_u16() {
        200..=299 => Ok(KeyCheck::Accepted),
        401 | 403 => Ok(KeyCheck::Rejected),
        code => anyhow::bail!("SteamGridDB answered {code}"),
    }
}

/// Downloads covers and heroes, one image at a time.
pub struct Fetcher {
    agent: ureq::Agent,
    key: Option<String>,
    dir: PathBuf,
    misses: BTreeMap<String, u64>,
    /// `SteamGridDB`'s game for each non-Steam game searched this run, so a
    /// game's cover and hero take one search.
    found: HashMap<String, Option<u64>>,
}

impl Fetcher {
    /// `None` when the user turned downloads off or there is no cache folder.
    #[must_use]
    pub fn new(config: &Config) -> Option<Self> {
        if !config.download_covers {
            return None;
        }
        let dir = dir()?;
        let misses = fs::read(dir.join("misses.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Some(Self {
            agent: agent(),
            key: config.steamgriddb_key.clone(),
            dir,
            misses,
            found: HashMap::new(),
        })
    }

    /// Whether this game needs this image and there is somewhere to look.
    #[must_use]
    pub fn wants(&self, game: &dyn Artwork, kind: Kind) -> bool {
        let recently_missed = self
            .misses
            .get(&kind.miss_key(game.id()))
            .is_some_and(|&when| time::now().saturating_sub(when) < RETRY_AFTER);
        game.current(kind).is_none()
            && !recently_missed
            && (game.url(kind).is_some() || self.key.is_some())
    }

    /// Finds and saves an image. `Ok(None)` when no source has one, which
    /// is remembered; an error (offline, a server down) is not, so the next
    /// start tries again.
    ///
    /// # Errors
    ///
    /// When a request fails for another reason than the game not being
    /// known, or the image cannot be saved.
    pub fn fetch(&mut self, game: &dyn Artwork, kind: Kind) -> anyhow::Result<Option<PathBuf>> {
        let mut url = game.url(kind).map(str::to_owned);
        if url.is_none() {
            url = self.steamgriddb(game, kind)?;
        }
        let Some(url) = url else {
            self.missed(&kind.miss_key(game.id()));
            return Ok(None);
        };
        let (bytes, ext) = download(&self.agent, &url)?;
        let stem = stem(game.id(), kind);
        let path = self.dir.join(format!("{stem}.{ext}"));
        paths::write_atomic(&path, &bytes)
            .with_context(|| format!("could not save {}", path.display()))?;
        for other in EXTENSIONS.iter().filter(|other| **other != ext) {
            let _ = fs::remove_file(self.dir.join(format!("{stem}.{other}")));
        }
        Ok(Some(path))
    }

    fn missed(&mut self, key: &str) {
        self.misses.insert(key.to_owned(), time::now());
        if let Ok(json) = serde_json::to_vec_pretty(&self.misses) {
            let _ = paths::write_atomic(&self.dir.join("misses.json"), &json);
        }
    }

    /// The URL of an image on `SteamGridDB`: by app id for Steam games, by
    /// the first search result for the title otherwise.
    fn steamgriddb(&mut self, game: &dyn Artwork, kind: Kind) -> anyhow::Result<Option<String>> {
        #[derive(Deserialize)]
        struct Image {
            url: String,
            width: Option<u32>,
            height: Option<u32>,
        }

        let Some(key) = self.key.clone() else {
            return Ok(None);
        };
        let endpoint = match kind {
            Kind::Cover => "grids",
            Kind::Hero => "heroes",
        };
        let path =
            if let (Source::Steam, Some(app)) = (game.source(), game.id().strip_prefix("steam:")) {
                format!("{endpoint}/steam/{app}")
            } else {
                let Some(id) = self.search(&key, game)? else {
                    return Ok(None);
                };
                format!("{endpoint}/game/{id}")
            };

        if kind == Kind::Hero {
            let heroes = steamgriddb::api::<Vec<Image>>(&self.agent, &key, &path, &[])?;
            return Ok(heroes.and_then(|heroes| heroes.into_iter().next().map(|hero| hero.url)));
        }
        // Ask for Steam's portrait sizes; should the filter be turned down,
        // look through the unfiltered list for an image taller than wide,
        // and take none rather than a banner.
        let portrait = [("dimensions", PORTRAIT)];
        let grid = match steamgriddb::api::<Vec<Image>>(&self.agent, &key, &path, &portrait) {
            Ok(found) => found.and_then(|grids| grids.into_iter().next()),
            Err(err) if is_status(&err, 400) => {
                steamgriddb::api::<Vec<Image>>(&self.agent, &key, &path, &[])?.and_then(|grids| {
                    grids.into_iter().find(
                        |grid| matches!((grid.width, grid.height), (Some(w), Some(h)) if h > w),
                    )
                })
            }
            Err(err) => return Err(err),
        };
        Ok(grid.map(|grid| grid.url))
    }

    /// `SteamGridDB`'s id for a game, from its title.
    fn search(&mut self, key: &str, game: &dyn Artwork) -> anyhow::Result<Option<u64>> {
        #[derive(Deserialize)]
        struct Found {
            id: u64,
        }
        if let Some(&id) = self.found.get(game.id()) {
            return Ok(id);
        }
        let search = format!(
            "search/autocomplete/{}",
            crate::heroic::percent_encode(game.title())
        );
        let id = steamgriddb::api::<Vec<Found>>(&self.agent, key, &search, &[])?
            .and_then(|found| found.first().map(|first| first.id));
        self.found.insert(game.id().to_owned(), id);
        Ok(id)
    }
}

/// Downloads an image: its bytes, and the extension their signature gives.
pub(crate) fn download(agent: &ureq::Agent, url: &str) -> anyhow::Result<(Vec<u8>, &'static str)> {
    // Epic hands out some image addresses with bare spaces in them.
    let url = url.replace(' ', "%20");
    let response = agent
        .get(&url)
        .call()
        .with_context(|| format!("could not download {url}"))?;
    let status = response.status();
    anyhow::ensure!(status.is_success(), "could not download {url}: {status}");
    let bytes = response
        .into_body()
        .read_to_vec()
        .with_context(|| format!("could not download {url}"))?;
    let ext = image_extension(&bytes).with_context(|| format!("{url} is not an image"))?;
    Ok((bytes, ext))
}

/// The images chosen by hand, read once for a whole library.
#[derive(Debug, Default)]
pub struct Choices {
    found: HashMap<(String, Kind), PathBuf>,
}

impl Choices {
    #[must_use]
    pub fn load() -> Self {
        let mut found = HashMap::new();
        if let Some(dir) = dir() {
            for kind in [Kind::Cover, Kind::Hero] {
                let Ok(entries) = fs::read_dir(dir.join(kind.chosen_folder())) else {
                    continue;
                };
                for entry in entries.flatten() {
                    if let Some(base) = chosen_base(&entry.file_name().to_string_lossy()) {
                        found.insert((base.to_owned(), kind), entry.path());
                    }
                }
            }
        }
        Self { found }
    }

    /// The image chosen for this game, if one was.
    #[must_use]
    pub fn get(&self, id: &str, kind: Kind) -> Option<&Path> {
        self.found
            .get(&(stem(id, Kind::Cover), kind))
            .map(PathBuf::as_path)
    }
}

/// The image chosen for one game, if one was.
#[must_use]
pub fn chosen(id: &str, kind: Kind) -> Option<PathBuf> {
    Choices::load().get(id, kind).map(Path::to_path_buf)
}

/// The game's part of a chosen image's name, `<stem>.<number>.<ext>`.
fn chosen_base(name: &str) -> Option<&str> {
    let (rest, ext) = name.rsplit_once('.')?;
    let (base, number) = rest.rsplit_once('.')?;
    let numbered = !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit());
    (EXTENSIONS.contains(&ext) && numbered && !base.is_empty()).then_some(base)
}

/// Downloads an image picked on `SteamGridDB` as the game's own, in place
/// of any chosen before.
///
/// # Errors
///
/// When it cannot be downloaded or saved.
pub fn choose(id: &str, kind: Kind, image: &steamgriddb::Image) -> anyhow::Result<PathBuf> {
    let folder = dir()
        .context("no cache folder for covers")?
        .join(kind.chosen_folder());
    let (bytes, ext) = download(&agent(), &image.url)?;
    let path = folder.join(format!("{}.{}.{ext}", stem(id, Kind::Cover), image.id));
    paths::write_atomic(&path, &bytes)
        .with_context(|| format!("could not save {}", path.display()))?;
    remove_chosen(&folder, id, Some(&path))?;
    Ok(path)
}

/// Goes back to the image found by itself.
///
/// # Errors
///
/// When the chosen image cannot be deleted.
pub fn forget_choice(id: &str, kind: Kind) -> anyhow::Result<()> {
    match dir() {
        Some(dir) => remove_chosen(&dir.join(kind.chosen_folder()), id, None),
        None => Ok(()),
    }
}

fn remove_chosen(folder: &Path, id: &str, keep: Option<&Path>) -> anyhow::Result<()> {
    let base = stem(id, Kind::Cover);
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("could not read {}", folder.display()));
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let theirs = chosen_base(&entry.file_name().to_string_lossy()) == Some(base.as_str());
        if theirs && Some(path.as_path()) != keep {
            fs::remove_file(&path)
                .with_context(|| format!("could not delete {}", path.display()))?;
        }
    }
    Ok(())
}

/// The file extension for an image's bytes, read from its signature.
fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xff, 0xd8, 0xff, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_safely() {
        assert_eq!(
            stem("amazon:amzn1.adg.product.x/y", Kind::Cover),
            "amazon_amzn1.adg.product.x_y"
        );
        assert_eq!(stem("steam:620", Kind::Hero), "steam_620.hero");
        assert_eq!(Kind::Hero.miss_key("steam:620"), "steam:620#hero");
    }

    #[test]
    fn knows_a_chosen_image_by_its_name() {
        assert_eq!(chosen_base("steam_620.40269.jpg"), Some("steam_620"));
        assert_eq!(
            chosen_base("amazon_amzn1.adg.product.x.7.png"),
            Some("amazon_amzn1.adg.product.x")
        );
        assert_eq!(chosen_base("steam_620.jpg"), None);
        assert_eq!(chosen_base("steam_620.40269.jpg.123.tmp"), None);
        assert_eq!(chosen_base("steam_620.x1.jpg"), None);
    }

    #[test]
    fn tells_images_by_their_signature() {
        assert_eq!(image_extension(&[0xff, 0xd8, 0xff, 0xe0]), Some("jpg"));
        assert_eq!(image_extension(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_extension(b"<html>"), None);
    }
}
