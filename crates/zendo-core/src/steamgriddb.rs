//! `SteamGridDB`'s API (`https://www.steamgriddb.com/api/v2`, with the
//! user's key as a bearer token): the calls the automatic cover downloads
//! make, and the ones the cover picker makes to let the user choose:
//! searching games by title and listing a game's covers or wide images.

use anyhow::Context;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::covers::{self, Kind};
use crate::{heroic, time};

pub(crate) const API: &str = "https://www.steamgriddb.com/api/v2";
/// Steam's portrait grid sizes.
pub(crate) const PORTRAIT: &str = "600x900,342x482,660x930";

/// A game `SteamGridDB` knows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Found {
    pub id: u64,
    pub name: String,
    /// Unix seconds.
    #[serde(default)]
    pub release_date: Option<i64>,
}

impl Found {
    /// The year it came out, when `SteamGridDB` says.
    #[must_use]
    pub fn year(&self) -> Option<i64> {
        self.release_date.map(time::year_of)
    }
}

/// One image a game has on `SteamGridDB`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Image {
    pub id: u64,
    pub url: String,
    /// A smaller copy, for choosing.
    pub thumb: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// `alternate`, `blurred`, `material`…
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub author: Option<Author>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Author {
    pub name: String,
}

/// One page of a game's images, and whether there are more.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub images: Vec<Image>,
    pub more: bool,
}

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    data: Option<T>,
    #[serde(default)]
    page: u32,
    #[serde(default)]
    total: u32,
    #[serde(default)]
    limit: u32,
}

/// One call, its reply whole; `None` when `SteamGridDB` does not know what
/// was asked (404).
fn get<T: DeserializeOwned>(
    agent: &ureq::Agent,
    key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> anyhow::Result<Option<Envelope<T>>> {
    let mut request = agent
        .get(format!("{API}/{path}"))
        .header("Authorization", format!("Bearer {}", key.trim()));
    for (name, value) in query {
        request = request.query(*name, *value);
    }
    let response = request
        .call()
        .with_context(|| format!("SteamGridDB {path}"))?;
    match response.status().as_u16() {
        200..=299 => {}
        404 => return Ok(None),
        code => {
            return Err(anyhow::Error::new(ureq::Error::StatusCode(code))
                .context(format!("SteamGridDB {path}")));
        }
    }
    let text = response
        .into_body()
        .read_to_string()
        .with_context(|| format!("SteamGridDB {path}"))?;
    let envelope: Envelope<T> = serde_json::from_str(&text)
        .with_context(|| format!("SteamGridDB {path}: unexpected reply"))?;
    Ok(Some(envelope).filter(|envelope| envelope.success))
}

/// One call: its `data`, or `None` when the game is unknown.
pub(crate) fn api<T: DeserializeOwned>(
    agent: &ureq::Agent,
    key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> anyhow::Result<Option<T>> {
    Ok(get::<T>(agent, key, path, query)?.and_then(|envelope| envelope.data))
}

pub(crate) fn is_status(err: &anyhow::Error, code: u16) -> bool {
    matches!(err.downcast_ref::<ureq::Error>(), Some(ureq::Error::StatusCode(c)) if *c == code)
}

/// The games whose names match a title, best first.
///
/// # Errors
///
/// When `SteamGridDB` cannot be reached or refuses the key.
pub fn search(key: &str, title: &str) -> anyhow::Result<Vec<Found>> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(Vec::new());
    }
    let path = format!("search/autocomplete/{}", heroic::percent_encode(title));
    Ok(api(&covers::agent(), key, &path, &[])?.unwrap_or_default())
}

/// The game with this Steam app id.
///
/// # Errors
///
/// As [`search`].
pub fn steam_game(key: &str, app: &str) -> anyhow::Result<Option<Found>> {
    api(&covers::agent(), key, &format!("games/steam/{app}"), &[])
}

/// A page of a game's covers (portrait, in Steam's sizes) or wide images,
/// still ones only; the first page is 0.
///
/// # Errors
///
/// As [`search`].
pub fn images(key: &str, game: u64, kind: Kind, page: u32) -> anyhow::Result<Page> {
    let agent = covers::agent();
    let page = page.to_string();
    let (path, query): (String, Vec<(&str, &str)>) = match kind {
        Kind::Cover => (
            format!("grids/game/{game}"),
            vec![
                ("dimensions", PORTRAIT),
                ("types", "static"),
                ("page", &page),
            ],
        ),
        Kind::Hero => (
            format!("heroes/game/{game}"),
            vec![("types", "static"), ("page", &page)],
        ),
    };
    let Some(envelope) = get::<Vec<Image>>(&agent, key, &path, &query)? else {
        return Ok(Page::default());
    };
    let images = envelope.data.unwrap_or_default();
    let shown = (envelope.page + 1).saturating_mul(envelope.limit.max(1));
    Ok(Page {
        more: !images.is_empty() && shown < envelope.total,
        images,
    })
}

/// An image's bytes, checked to be one, for showing a thumbnail.
///
/// # Errors
///
/// When it cannot be downloaded or is not an image.
pub fn thumbnail(url: &str) -> anyhow::Result<Vec<u8>> {
    covers::download(&covers::agent(), url).map(|(bytes, _)| bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_page_of_images() {
        let text = r#"{"success":true,"page":0,"total":41,"limit":50,"data":[
            {"id":40269,"score":0,"style":"alternate","width":600,"height":900,
             "nsfw":false,"mime":"image/jpeg",
             "url":"https://cdn2.steamgriddb.com/grid/1a.jpg",
             "thumb":"https://cdn2.steamgriddb.com/thumb/1a.jpg",
             "author":{"name":"Someone","steam64":"1"}}]}"#;
        let envelope: Envelope<Vec<Image>> = serde_json::from_str(text).unwrap();
        let images = envelope.data.unwrap();
        assert_eq!(images[0].id, 40269);
        assert_eq!(images[0].author.as_ref().unwrap().name, "Someone");
        assert_eq!(envelope.total, 41);
    }

    /// Asks the real `SteamGridDB` with the key in Zendo's settings, as in
    /// `cargo test -p zendo-core steamgriddb -- --ignored`.
    #[test]
    #[ignore = "asks SteamGridDB over the network, with the key in Settings"]
    fn finds_a_game_and_its_images() {
        let config = crate::config::Config::load().unwrap();
        let key = config
            .steamgriddb_key
            .expect("a SteamGridDB key in Settings");
        let found = search(&key, "Jet Set Radio Future").unwrap();
        let game = found
            .iter()
            .find(|game| game.name == "Jet Set Radio Future")
            .unwrap();
        assert_eq!(game.year(), Some(2002));
        let covers = images(&key, game.id, Kind::Cover, 0).unwrap();
        assert_ne!(covers.images.len(), 0);
        assert!(covers.images.iter().all(|image| image.height > image.width));
        let heroes = images(&key, game.id, Kind::Hero, 0).unwrap();
        assert!(heroes.images.iter().all(|image| image.width > image.height));
        let bytes = thumbnail(&covers.images[0].thumb).unwrap();
        assert!(bytes.len() > 1000);
        let portal = steam_game(&key, "620").unwrap().unwrap();
        assert_eq!(portal.name, "Portal 2");
        assert_eq!(search(&key, "  ").unwrap().len(), 0);
    }

    #[test]
    fn gives_a_games_year() {
        let found: Found = serde_json::from_str(
            r#"{"id":5248415,"name":"Jet Set Radio Future","verified":true,
                "types":[],"release_date":1014336000}"#,
        )
        .unwrap();
        assert_eq!(found.year(), Some(2002));
        let found: Found = serde_json::from_str(r#"{"id":1,"name":"No date","types":[]}"#).unwrap();
        assert_eq!(found.year(), None);
    }
}
