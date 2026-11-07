//! The games each signed-in store says the user owns, installed or not.
//! Epic's list comes from `legendary list --json`, Amazon's from nile's
//! `library.json` after `nile library sync`, GOG's from GOG's own services
//! with gogdl's token, as gogdl has no list of its own: the Galaxy library
//! gives the ids, GOG's games database each game's title and art.
//!
//! Each list is kept in `$XDG_CACHE_HOME/zendo/store/<store>.json`, so the
//! Store page shows at once and asks again only on Refresh or a sign-in.
//! Ids are the library's (`epic:…`, `gog:…`, `amazon:…`), so a game's
//! covers are shared between the two.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::covers::{self, Artwork, Kind};
use crate::stores::Store;
use crate::{Source, paths};

/// A game a store says the user owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    pub store: StoreKey,
    pub title: String,
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// The first release, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    /// What the store says the game is about, as plain text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// `Windows`, `Mac`, `Linux`, as far as the store says.
    pub platforms: Vec<String>,
    pub cover_url: Option<String>,
    pub hero_url: Option<String>,
    /// Images on disk, filled in after loading; not kept in the cache.
    #[serde(skip)]
    pub cover: Option<PathBuf>,
    #[serde(skip)]
    pub hero: Option<PathBuf>,
}

/// [`Store`] as written in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreKey {
    Epic,
    Gog,
    Amazon,
}

impl From<Store> for StoreKey {
    fn from(store: Store) -> Self {
        match store {
            Store::Epic => Self::Epic,
            Store::Gog => Self::Gog,
            Store::Amazon => Self::Amazon,
        }
    }
}

impl From<StoreKey> for Store {
    fn from(key: StoreKey) -> Self {
        match key {
            StoreKey::Epic => Self::Epic,
            StoreKey::Gog => Self::Gog,
            StoreKey::Amazon => Self::Amazon,
        }
    }
}

impl Owned {
    #[must_use]
    pub fn store(&self) -> Store {
        self.store.into()
    }

    /// A game known by the store's own id for it.
    #[must_use]
    pub fn new(store: Store, id: &str, title: String) -> Self {
        Self {
            id: format!("{}:{id}", store.source_key()),
            store: store.into(),
            title,
            developer: None,
            publisher: None,
            released: None,
            genres: Vec::new(),
            summary: None,
            platforms: Vec::new(),
            cover_url: None,
            hero_url: None,
            cover: None,
            hero: None,
        }
    }
}

impl Artwork for Owned {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn source(&self) -> Source {
        self.store().source()
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

fn cache_file(store: Store) -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join(format!("zendo/store/{}.json", store.source_key())))
}

/// The list kept from the last time the store was asked, with the images
/// already downloaded; empty when there is none.
#[must_use]
pub fn cached(store: Store) -> Vec<Owned> {
    let mut games: Vec<Owned> = cache_file(store)
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    with_images(&mut games);
    games
}

/// Removes the kept list, as after signing out.
pub fn forget(store: Store) {
    if let Some(file) = cache_file(store) {
        let _ = fs::remove_file(file);
    }
}

fn with_images(games: &mut [Owned]) {
    let chosen = covers::Choices::load();
    for game in games {
        for kind in [Kind::Cover, Kind::Hero] {
            let image = chosen
                .get(&game.id, kind)
                .map(Path::to_path_buf)
                .or_else(|| covers::cached(&game.id, kind));
            match kind {
                Kind::Cover => game.cover = image,
                Kind::Hero => game.hero = image,
            }
        }
    }
}

/// Asks the store for the games owned, keeps the list and returns it.
///
/// # Errors
///
/// When no one is signed in, the engine fails or the store cannot be
/// reached.
pub fn fetch(store: Store) -> anyhow::Result<Vec<Owned>> {
    let mut games = match store {
        Store::Epic => {
            let output = store.run_checked(&["list", "--json"])?;
            parse_legendary(&output.stdout)?
        }
        Store::Amazon => {
            store.run_checked(&["library", "sync"])?;
            let file = store
                .dir()
                .context("no home folder")?
                .join("nile/library.json");
            let bytes =
                fs::read(&file).with_context(|| format!("could not read {}", file.display()))?;
            parse_nile(&bytes)?
        }
        Store::Gog => gog()?,
    };
    games.sort_by_cached_key(|game| game.title.to_lowercase());
    games.dedup_by(|a, b| a.id == b.id);
    if let Some(file) = cache_file(store) {
        let json = serde_json::to_vec(&games)?;
        paths::write_atomic(&file, &json)
            .with_context(|| format!("could not write {}", file.display()))?;
    }
    with_images(&mut games);
    Ok(games)
}

/// `legendary list --json`: legendary's game records, DLC left out.
fn parse_legendary(json: &[u8]) -> anyhow::Result<Vec<Owned>> {
    #[derive(Deserialize)]
    struct Game {
        app_name: String,
        app_title: Option<String>,
        #[serde(default)]
        metadata: Metadata,
        #[serde(default)]
        asset_infos: HashMap<String, Value>,
    }
    #[derive(Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct Metadata {
        developer: Option<String>,
        description: Option<String>,
        #[serde(default)]
        key_images: Vec<Image>,
        main_game_item: Option<Value>,
    }
    #[derive(Deserialize)]
    struct Image {
        #[serde(rename = "type")]
        kind: String,
        url: String,
    }

    let games: Vec<Game> = serde_json::from_slice(json).context("legendary list: not JSON")?;
    Ok(games
        .into_iter()
        .filter(|game| game.metadata.main_game_item.is_none())
        .map(|game| {
            let image = |kind: &str| {
                game.metadata
                    .key_images
                    .iter()
                    .find(|image| image.kind == kind)
                    .map(|image| image.url.clone())
            };
            let mut owned = Owned::new(
                Store::Epic,
                &game.app_name,
                game.app_title
                    .clone()
                    .unwrap_or_else(|| game.app_name.clone()),
            );
            owned.cover_url = image("DieselGameBoxTall");
            owned.hero_url = image("DieselGameBox");
            owned.developer.clone_from(&game.metadata.developer);
            owned.summary = game.metadata.description.as_deref().and_then(plain_text);
            let mut platforms: Vec<String> = game.asset_infos.keys().cloned().collect();
            platforms.sort();
            owned.platforms = platforms;
            owned
        })
        .collect())
}

/// nile's `library.json`: one entitlement per game.
fn parse_nile(json: &[u8]) -> anyhow::Result<Vec<Owned>> {
    #[derive(Deserialize)]
    struct Entry {
        product: Product,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Product {
        id: String,
        title: Option<String>,
        #[serde(rename = "productDetail")]
        detail: Option<Detail>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Detail {
        icon_url: Option<String>,
        details: Option<Details>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Details {
        developer: Option<String>,
        publisher: Option<String>,
        release_date: Option<String>,
        #[serde(default)]
        genres: Vec<String>,
        short_description: Option<String>,
        background_url1: Option<String>,
        background_url2: Option<String>,
    }

    let entries: Vec<Entry> = serde_json::from_slice(json).context("nile library: not JSON")?;
    Ok(entries
        .into_iter()
        .map(|entry| {
            let product = entry.product;
            let mut owned = Owned::new(
                Store::Amazon,
                &product.id,
                product.title.clone().unwrap_or_else(|| product.id.clone()),
            );
            if let Some(detail) = product.detail {
                // Amazon's icon is the portrait, as Heroic shows it.
                owned.cover_url = detail.icon_url;
                if let Some(details) = detail.details {
                    owned.developer = details.developer;
                    owned.publisher = details.publisher;
                    owned.released = details.release_date.as_deref().and_then(day);
                    owned.genres = details.genres;
                    owned.summary = details.short_description.as_deref().and_then(plain_text);
                    owned.hero_url = details.background_url1.or(details.background_url2);
                }
            }
            owned.platforms = vec!["Windows".to_owned()];
            owned
        })
        .collect())
}

fn agent() -> ureq::Agent {
    crate::http::answering_agent(Duration::from_secs(20))
}

/// GOG: the Galaxy library's GOG releases, each looked up in GOG's games
/// database. Each game's record is kept with its `ETag`, so asking again
/// costs a "not changed" per game.
fn gog() -> anyhow::Result<Vec<Owned>> {
    #[derive(Deserialize)]
    struct Page {
        #[serde(default)]
        items: Vec<Release>,
        next_page_token: Option<String>,
    }
    #[derive(Deserialize)]
    struct Release {
        platform_id: String,
        external_id: Value,
        certificate: Option<String>,
    }

    let credentials = Store::Gog.gog_token()?.context("not signed in to GOG")?;
    let agent = agent();
    let mut releases = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut request = agent
            .get(format!(
                "https://galaxy-library.gog.com/users/{}/releases",
                credentials.user_id
            ))
            .header(
                "Authorization",
                format!("Bearer {}", credentials.access_token),
            );
        if let Some(token) = &token {
            request = request.query("page_token", token);
        }
        let response = request.call().context("could not reach GOG")?;
        if !response.status().is_success() {
            bail!("GOG's library answered {}", response.status());
        }
        let page: Page = serde_json::from_str(&response.into_body().read_to_string()?)
            .context("GOG's library: unexpected reply")?;
        releases.extend(page.items);
        match page.next_page_token.filter(|token| !token.is_empty()) {
            Some(next) => token = Some(next),
            None => break,
        }
    }

    let dir = paths::cache_home()
        .context("no home folder")?
        .join("zendo/store/gogdb");
    let mut games = Vec::new();
    for release in releases
        .into_iter()
        .filter(|release| release.platform_id == "gog")
    {
        let id = match &release.external_id {
            Value::String(id) => id.clone(),
            other => other.to_string(),
        };
        let record = gamesdb(
            &agent,
            &dir,
            &id,
            release.certificate.as_deref(),
            &credentials.access_token,
        )?;
        if let Some(owned) = record.as_ref().and_then(from_gamesdb) {
            games.push(owned);
        }
    }
    Ok(games)
}

/// One game's record in GOG's games database, from the cache when GOG says
/// it has not changed. `None` when GOG does not know the game.
fn gamesdb(
    agent: &ureq::Agent,
    dir: &std::path::Path,
    id: &str,
    certificate: Option<&str>,
    token: &str,
) -> anyhow::Result<Option<Value>> {
    let file = dir.join(format!("{}.json", id.replace(['/', '.'], "_")));
    let kept: Option<Value> = fs::read(&file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let etag = kept
        .as_ref()
        .and_then(|kept| kept.get("etag")?.as_str().map(str::to_owned));
    let mut request = agent
        .get(format!(
            "https://gamesdb.gog.com/platforms/gog/external_releases/{id}"
        ))
        .header("Authorization", format!("Bearer {token}"));
    if let Some(certificate) = certificate {
        request = request.header("X-GOG-Library-Cert", certificate);
    }
    if let Some(etag) = &etag {
        request = request.header("If-None-Match", etag);
    }
    let response = request
        .call()
        .context("could not reach GOG's games database")?;
    match response.status().as_u16() {
        304 => return Ok(kept),
        404 => return Ok(None),
        200..=299 => {}
        code => bail!("GOG's games database answered {code} for {id}"),
    }
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut record: Value = serde_json::from_str(&response.into_body().read_to_string()?)
        .with_context(|| format!("GOG's games database: unexpected reply for {id}"))?;
    if let (Some(etag), Some(object)) = (etag, record.as_object_mut()) {
        object.insert("etag".to_owned(), Value::String(etag));
    }
    let _ = paths::write_atomic(&file, &serde_json::to_vec(&record)?);
    Ok(Some(record))
}

/// A games-database record as an owned game: games and mods shown in the
/// library only, as GOG Galaxy lists them.
fn from_gamesdb(record: &Value) -> Option<Owned> {
    let kind = record.get("type")?.as_str()?;
    let game = record.get("game")?;
    if !matches!(kind, "game" | "mod") || game.get("visible_in_library")? != &Value::Bool(true) {
        return None;
    }
    let id = match record.get("external_id")? {
        Value::String(id) => id.clone(),
        other => other.to_string(),
    };
    let localized = |value: Option<&Value>| {
        value
            .and_then(|value| value.get("*"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned)
    };
    let title = localized(record.get("title")).or_else(|| localized(game.get("title")))?;
    let image = |key: &str| {
        game.get(key)?
            .get("url_format")?
            .as_str()
            .map(|format| format.replace("{formatter}", "").replace("{ext}", "jpg"))
    };
    let mut owned = Owned::new(Store::Gog, &id, title);
    owned.cover_url = image("vertical_cover");
    owned.hero_url = image("background").or_else(|| image("logo"));
    owned.developer = game
        .get("developers")
        .and_then(Value::as_array)
        .map(|developers| {
            developers
                .iter()
                .filter_map(|developer| developer.get("name")?.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|developers| !developers.is_empty());
    owned.publisher = names(game.get("publishers"), |publisher| {
        publisher.get("name")?.as_str()
    });
    owned.released = game
        .get("first_release_date")
        .and_then(Value::as_str)
        .and_then(day);
    owned.genres = game
        .get("genres")
        .and_then(Value::as_array)
        .map(|genres| {
            genres
                .iter()
                .filter_map(|genre| genre.get("name")?.get("*")?.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    owned.summary = localized(record.get("summary"))
        .or_else(|| localized(game.get("summary")))
        .as_deref()
        .and_then(plain_text);
    owned.platforms = record
        .get("supported_operating_systems")
        .and_then(Value::as_array)
        .map(|systems| {
            systems
                .iter()
                .filter_map(|system| match system.get("slug")?.as_str()? {
                    "windows" => Some("Windows".to_owned()),
                    "osx" => Some("Mac".to_owned()),
                    "linux" => Some("Linux".to_owned()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Some(owned)
}

/// The names in a list of records, joined; `None` when there are none.
fn names(list: Option<&Value>, name: impl Fn(&Value) -> Option<&str>) -> Option<String> {
    let names: Vec<&str> = list?.as_array()?.iter().filter_map(name).collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// The day of a date the stores write as `2021-06-15T00:00:00+0000` or
/// `2021-06-15`.
fn day(date: &str) -> Option<String> {
    let day = date.get(..10)?;
    let ok = day.bytes().enumerate().all(|(index, byte)| {
        if index == 4 || index == 7 {
            byte == b'-'
        } else {
            byte.is_ascii_digit()
        }
    });
    ok.then(|| day.to_owned())
}

/// A store's description as plain text: tags dropped, the common entities
/// written out, blank runs of lines closed up.
fn plain_text(html: &str) -> Option<String> {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let name = tag.trim_start_matches('/').to_lowercase();
                if name.starts_with("br") || name.starts_with('p') || name.starts_with("li") {
                    text.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => text.push(c),
        }
    }
    let text = text
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim) {
        if !line.is_empty() || lines.last().is_some_and(|last| !last.is_empty()) {
            lines.push(line);
        }
    }
    while lines.last().is_some_and(|last| last.is_empty()) {
        lines.pop();
    }
    let text = lines.join("\n");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_store_descriptions_as_plain_text() {
        let html = "<b>Ever wanted a shop?</b>\n<br>\n\n<br>\nPlan it &amp; run it.<br><br><br>";
        assert_eq!(
            plain_text(html).as_deref(),
            Some("Ever wanted a shop?\n\nPlan it & run it.")
        );
        assert_eq!(plain_text("<br>"), None);
        assert_eq!(
            day("2021-06-15T00:00:00+0000").as_deref(),
            Some("2021-06-15")
        );
        assert_eq!(day("June 2021"), None);
    }

    #[test]
    fn reads_legendarys_list() {
        let json = br#"[
          {"app_name": "Fortnite", "app_title": "Fortnite",
           "asset_infos": {"Windows": {}, "Mac": {}},
           "metadata": {"developer": "Epic Games",
             "keyImages": [{"type": "DieselGameBox", "url": "https://e/wide.jpg"},
                           {"type": "DieselGameBoxTall", "url": "https://e/tall.jpg"}]}},
          {"app_name": "SomeDlc", "app_title": "Some DLC",
           "metadata": {"mainGameItem": {"id": "x"}}}
        ]"#;
        let games = parse_legendary(json).unwrap();
        assert_eq!(games.len(), 1);
        let game = &games[0];
        assert_eq!(game.id, "epic:Fortnite");
        assert_eq!(game.cover_url.as_deref(), Some("https://e/tall.jpg"));
        assert_eq!(game.hero_url.as_deref(), Some("https://e/wide.jpg"));
        assert_eq!(game.developer.as_deref(), Some("Epic Games"));
        assert_eq!(game.platforms, ["Mac", "Windows"]);
    }

    #[test]
    fn reads_niles_library() {
        let json = br#"[{"id": "e1", "product": {"id": "amzn1.adg.product.1", "title": "Two Point",
            "productDetail": {"iconUrl": "https://a/icon.jpg",
              "details": {"developer": "Two Point Studios", "backgroundUrl1": "https://a/bg.jpg"}}}}]"#;
        let games = parse_nile(json).unwrap();
        assert_eq!(games[0].id, "amazon:amzn1.adg.product.1");
        assert_eq!(games[0].cover_url.as_deref(), Some("https://a/icon.jpg"));
        assert_eq!(games[0].hero_url.as_deref(), Some("https://a/bg.jpg"));
    }

    #[test]
    fn reads_gogs_games_database() {
        let record: Value = serde_json::from_str(
            r#"{"type": "game", "external_id": "1158493447",
                "title": {"*": "Prey", "en-US": "Prey"},
                "supported_operating_systems": [{"slug": "windows"}, {"slug": "linux"}],
                "game": {"visible_in_library": true,
                  "developers": [{"name": "Arkane Studios"}],
                  "vertical_cover": {"url_format": "https://images.gog.com/abc{formatter}.{ext}?namespace=gamesdb"},
                  "background": {"url_format": "https://images.gog.com/bg{formatter}.{ext}?namespace=gamesdb"}}}"#,
        )
        .unwrap();
        let game = from_gamesdb(&record).unwrap();
        assert_eq!(game.id, "gog:1158493447");
        assert_eq!(game.title, "Prey");
        assert_eq!(
            game.cover_url.as_deref(),
            Some("https://images.gog.com/abc.jpg?namespace=gamesdb")
        );
        assert_eq!(game.platforms, ["Windows", "Linux"]);

        // Amazon Luna titles and hidden entries are not games to list.
        let spam: Value = serde_json::from_str(
            r#"{"type": "spam", "external_id": "1", "title": {"*": "X"}, "game": {"visible_in_library": false}}"#,
        )
        .unwrap();
        assert!(from_gamesdb(&spam).is_none());
    }
}
