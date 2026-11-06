//! Covers Zendo downloads, kept in `$XDG_CACHE_HOME/zendo/covers` and named
//! by game id. A game's store image comes first (Heroic records one for its
//! games), `SteamGridDB` second when the user has set a key. A game with no
//! cover anywhere is noted in `misses.json` and asked about again after a
//! week, not on every start.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;

use crate::config::Config;
use crate::{Game, Source, heroic, paths, time};

const STEAMGRIDDB: &str = "https://www.steamgriddb.com/api/v2";
/// Steam's portrait grid sizes.
const PORTRAIT: &str = "600x900,342x482,660x930";
const EXTENSIONS: [&str; 3] = ["jpg", "png", "webp"];
const RETRY_AFTER: u64 = 7 * 24 * 3600;

#[must_use]
pub fn dir() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/covers"))
}

/// The file name for a game's cover, without the extension: the id with
/// anything outside `[A-Za-z0-9._-]` as `_`.
fn stem(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The cover already downloaded for this game.
#[must_use]
pub fn cached(id: &str) -> Option<PathBuf> {
    let dir = dir()?;
    let stem = stem(id);
    EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
}

/// Downloads covers, one game at a time.
pub struct Fetcher {
    agent: ureq::Agent,
    key: Option<String>,
    dir: PathBuf,
    misses: BTreeMap<String, u64>,
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
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(concat!(
                "zendo/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/leandrop-dev/zendo)"
            ))
            .build()
            .into();
        Some(Self {
            agent,
            key: config.steamgriddb_key.clone(),
            dir,
            misses,
        })
    }

    /// Whether this game needs a cover and there is somewhere to look.
    #[must_use]
    pub fn wants(&self, game: &Game) -> bool {
        let recently_missed = self
            .misses
            .get(&game.id)
            .is_some_and(|&when| time::now().saturating_sub(when) < RETRY_AFTER);
        game.cover.is_none() && !recently_missed && (game.cover_url.is_some() || self.key.is_some())
    }

    /// Finds and saves a cover. `Ok(None)` when no source has one, which is
    /// remembered; an error (offline, a server down) is not, so the next
    /// start tries again.
    ///
    /// # Errors
    ///
    /// When a request fails for another reason than the game not being
    /// known, or the cover cannot be saved.
    pub fn fetch(&mut self, game: &Game) -> anyhow::Result<Option<PathBuf>> {
        let mut url = game.cover_url.clone();
        if url.is_none() {
            url = self.steamgriddb(game)?;
        }
        let Some(url) = url else {
            self.missed(&game.id);
            return Ok(None);
        };
        let bytes = self
            .agent
            .get(&url)
            .call()
            .with_context(|| format!("could not download {url}"))?
            .into_body()
            .read_to_vec()
            .with_context(|| format!("could not download {url}"))?;
        let ext = image_extension(&bytes).with_context(|| format!("{url} is not an image"))?;
        let stem = stem(&game.id);
        let path = self.dir.join(format!("{stem}.{ext}"));
        paths::write_atomic(&path, &bytes)
            .with_context(|| format!("could not save {}", path.display()))?;
        for other in EXTENSIONS.iter().filter(|other| **other != ext) {
            let _ = fs::remove_file(self.dir.join(format!("{stem}.{other}")));
        }
        Ok(Some(path))
    }

    fn missed(&mut self, id: &str) {
        self.misses.insert(id.to_owned(), time::now());
        if let Ok(json) = serde_json::to_vec_pretty(&self.misses) {
            let _ = paths::write_atomic(&self.dir.join("misses.json"), &json);
        }
    }

    /// The URL of a portrait grid on `SteamGridDB`: by app id for Steam
    /// games, by the first search result for the title otherwise.
    fn steamgriddb(&self, game: &Game) -> anyhow::Result<Option<String>> {
        #[derive(Deserialize)]
        struct Found {
            id: u64,
        }
        #[derive(Deserialize)]
        struct Grid {
            url: String,
            width: Option<u32>,
            height: Option<u32>,
        }

        let Some(key) = &self.key else {
            return Ok(None);
        };
        let grids =
            if let (Source::Steam, Some(app)) = (game.source, game.id.strip_prefix("steam:")) {
                format!("grids/steam/{app}")
            } else {
                let search = format!(
                    "search/autocomplete/{}",
                    heroic::percent_encode(&game.title)
                );
                let Some(found) = self.api::<Vec<Found>>(key, &search, None)? else {
                    return Ok(None);
                };
                let Some(first) = found.first() else {
                    return Ok(None);
                };
                format!("grids/game/{}", first.id)
            };

        // Ask for Steam's portrait sizes. Which `dimensions` values the API
        // takes is not in anything read so far, so if it turns the filter
        // down, look through the unfiltered list for an image taller than
        // wide, and take none rather than a banner.
        let grids_found = match self.api::<Vec<Grid>>(key, &grids, Some(PORTRAIT)) {
            Ok(found) => found.and_then(|grids| grids.into_iter().next()),
            Err(err) if is_status(&err, 400) => {
                self.api::<Vec<Grid>>(key, &grids, None)?.and_then(|grids| {
                    grids.into_iter().find(
                        |grid| matches!((grid.width, grid.height), (Some(w), Some(h)) if h > w),
                    )
                })
            }
            Err(err) => return Err(err),
        };
        Ok(grids_found.map(|grid| grid.url))
    }

    /// One `SteamGridDB` call: its `data`, or `None` when the game is unknown.
    fn api<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
        path: &str,
        dimensions: Option<&str>,
    ) -> anyhow::Result<Option<T>> {
        #[derive(Deserialize)]
        struct Envelope<T> {
            #[serde(default)]
            success: bool,
            data: Option<T>,
        }
        let mut request = self
            .agent
            .get(format!("{STEAMGRIDDB}/{path}"))
            .header("Authorization", format!("Bearer {key}"));
        if let Some(dimensions) = dimensions {
            request = request.query("dimensions", dimensions);
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(err) => {
                return Err(anyhow::Error::new(err).context(format!("SteamGridDB {path}")));
            }
        };
        let text = response
            .into_body()
            .read_to_string()
            .with_context(|| format!("SteamGridDB {path}"))?;
        let envelope: Envelope<T> = serde_json::from_str(&text)
            .with_context(|| format!("SteamGridDB {path}: unexpected reply"))?;
        Ok(envelope.data.filter(|_| envelope.success))
    }
}

fn is_status(err: &anyhow::Error, code: u16) -> bool {
    matches!(err.downcast_ref::<ureq::Error>(), Some(ureq::Error::StatusCode(c)) if *c == code)
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
            stem("amazon:amzn1.adg.product.x/y"),
            "amazon_amzn1.adg.product.x_y"
        );
    }

    #[test]
    fn tells_images_by_their_signature() {
        assert_eq!(image_extension(&[0xff, 0xd8, 0xff, 0xe0]), Some("jpg"));
        assert_eq!(image_extension(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_extension(b"<html>"), None);
    }
}
