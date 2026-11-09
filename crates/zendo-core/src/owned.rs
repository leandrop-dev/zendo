//! The games each signed-in store says the user owns, installed or not.
//! Epic's list comes from `legendary list --json`, Amazon's from nile's
//! `library.json` after `nile library sync`, GOG's from GOG's own services
//! with gogdl's token, as gogdl has no list of its own: the Galaxy library
//! gives the ids, GOG's games database each game's title and art. Steam's
//! comes from its Web API (`IPlayerService/GetOwnedGames`) with the token
//! of Zendo's own Steam sign-in; its sizes from the Steam client's cache.
//!
//! Each list is kept in `$XDG_CACHE_HOME/zendo/store/<store>.json`, so the
//! Store page shows at once and asks again only on Refresh or a sign-in.
//! Ids are the library's (`epic:…`, `gog:…`, `amazon:…`, `steam:…`), so a
//! game's covers are shared between the two.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::covers::{self, Artwork, Kind};
use crate::stores::Store;
use crate::{Source, appinfo, gog_playtime, paths, steam};

const STEAM_OWNED: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
/// Steam's public store details of an app, no key needed.
const STEAM_DETAILS: &str = "https://store.steampowered.com/api/appdetails";
/// Steam's images for an app, by its id and the file's name.
const STEAM_IMAGES: &str = "https://shared.steamstatic.com/store_item_assets/steam/apps";

/// A game a store says the user owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    #[serde(rename = "store")]
    pub shop: Shop,
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
    /// The minutes played, where the store counts them (Steam; GOG once
    /// the game's page asked).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u64>,
    /// When it was last played, in seconds since 1970, where the store
    /// says (Steam).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<u64>,
    /// The minutes played in the last two weeks (Steam).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub recent_minutes: u64,
    /// The minutes played on each system, as Steam counts them since it
    /// began to; older time is in `minutes` only.
    #[serde(default, skip_serializing_if = "PlayedOn::is_empty")]
    pub played_on: PlayedOn,
    /// Shared by the user's Steam family, not owned.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
    /// The space the store's requirements ask for, in bytes (Steam's
    /// "Storage: 8 GB available space"); an estimate, not the download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<u64>,
    /// A Steam game's size, as the Steam client's own cache counts it;
    /// filled in after loading, not kept in the cache.
    #[serde(skip)]
    pub sizes: Option<appinfo::Sizes>,
    /// Images on disk, filled in after loading; not kept in the cache.
    #[serde(skip)]
    pub cover: Option<PathBuf>,
    #[serde(skip)]
    pub hero: Option<PathBuf>,
}

/// The minutes Steam counted on each system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayedOn {
    pub linux: u64,
    pub windows: u64,
    pub mac: u64,
    /// On a Steam Deck.
    pub deck: u64,
}

impl PlayedOn {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_zero(minutes: &u64) -> bool {
    *minutes == 0
}

/// Where an owned game comes from: a store Zendo installs from, or Steam,
/// which installs its own games. Written in the cache as `store`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shop {
    Steam,
    Epic,
    Gog,
    Amazon,
}

impl Shop {
    pub const ALL: [Self; 4] = [Self::Steam, Self::Epic, Self::Gog, Self::Amazon];

    /// The store Zendo installs it through; `None` for Steam.
    #[must_use]
    pub fn store(self) -> Option<Store> {
        match self {
            Self::Steam => None,
            Self::Epic => Some(Store::Epic),
            Self::Gog => Some(Store::Gog),
            Self::Amazon => Some(Store::Amazon),
        }
    }

    #[must_use]
    pub fn source(self) -> Source {
        self.store().map_or(Source::Steam, Store::source)
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        self.source().label()
    }

    /// The first half of its games' ids, and its cache file's name.
    fn key(self) -> &'static str {
        self.store().map_or("steam", Store::source_key)
    }
}

impl From<Store> for Shop {
    fn from(store: Store) -> Self {
        match store {
            Store::Epic => Self::Epic,
            Store::Gog => Self::Gog,
            Store::Amazon => Self::Amazon,
        }
    }
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
    /// The store Zendo installs it through; `None` for a Steam game.
    #[must_use]
    pub fn store(&self) -> Option<Store> {
        self.shop.store()
    }

    #[must_use]
    pub fn source(&self) -> Source {
        self.shop.source()
    }

    /// A game known by the store's own id for it.
    #[must_use]
    pub fn new(shop: impl Into<Shop>, id: &str, title: String) -> Self {
        let shop = shop.into();
        Self {
            id: format!("{}:{id}", shop.key()),
            shop,
            title,
            developer: None,
            publisher: None,
            released: None,
            genres: Vec::new(),
            summary: None,
            platforms: Vec::new(),
            cover_url: None,
            hero_url: None,
            minutes: None,
            last_played: None,
            recent_minutes: 0,
            played_on: PlayedOn::default(),
            shared: false,
            space: None,
            sizes: None,
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
        self.shop.source()
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

fn cache_file(shop: Shop) -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join(format!("zendo/store/{}.json", shop.key())))
}

/// The list kept from the last time the store was asked, with the images
/// already downloaded; empty when there is none.
#[must_use]
pub fn cached(shop: Shop) -> Vec<Owned> {
    let mut games: Vec<Owned> = cache_file(shop)
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    with_images(&mut games);
    with_sizes(&mut games);
    with_gog_minutes(&mut games);
    games
}

/// Removes the kept list, as after signing out.
pub fn forget(shop: Shop) {
    if let Some(file) = cache_file(shop) {
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
                .or_else(|| steam_cached(game, kind))
                .or_else(|| covers::cached(&game.id, kind));
            match kind {
                Kind::Cover => game.cover = image,
                Kind::Hero => game.hero = image,
            }
        }
    }
}

/// GOG's totals kept from the games' pages ([`gog_playtime::totals`]).
fn with_gog_minutes(games: &mut [Owned]) {
    let totals = gog_playtime::totals();
    for game in games {
        if let Some(&minutes) = totals.get(&game.id) {
            game.minutes = Some(game.minutes.unwrap_or(0).max(minutes));
        }
    }
}

/// The Steam games' sizes, from the Steam client's cache.
fn with_sizes(games: &mut [Owned]) {
    let app = |game: &Owned| {
        game.id
            .strip_prefix("steam:")
            .and_then(|app| app.parse::<u64>().ok())
    };
    let apps: HashSet<u64> = games.iter().filter_map(app).collect();
    if apps.is_empty() {
        return;
    }
    let sizes = appinfo::sizes(&steam::roots(), &apps);
    for game in games {
        game.sizes = app(game).and_then(|app| sizes.get(&app).copied());
    }
}

/// Asks the store for the games owned, keeps the list and returns it.
///
/// # Errors
///
/// When no one is signed in, the engine fails or the store cannot be
/// reached.
pub fn fetch(store: Store) -> anyhow::Result<Vec<Owned>> {
    let games = match store {
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
    keep(store.into(), games)
}

/// Sorts a store's list, keeps it in the cache and finds its images.
pub(crate) fn keep(shop: Shop, mut games: Vec<Owned>) -> anyhow::Result<Vec<Owned>> {
    games.sort_by_cached_key(|game| game.title.to_lowercase());
    games.dedup_by(|a, b| a.id == b.id);
    if let Some(file) = cache_file(shop) {
        let json = serde_json::to_vec(&games)?;
        paths::write_atomic(&file, &json)
            .with_context(|| format!("could not write {}", file.display()))?;
    }
    with_images(&mut games);
    with_sizes(&mut games);
    with_gog_minutes(&mut games);
    Ok(games)
}

/// Steam's own copy of a Steam game's image, which it keeps for the games
/// in its library, installed or not.
fn steam_cached(game: &Owned, kind: Kind) -> Option<PathBuf> {
    let app = game.id.strip_prefix("steam:")?;
    steam::cached_image(app, steam_image(kind))
}

fn steam_image(kind: Kind) -> &'static str {
    match kind {
        Kind::Cover => "library_600x900.jpg",
        Kind::Hero => "library_hero.jpg",
    }
}

/// The name Steam on this computer knows an account by, by its Steam ID.
#[must_use]
pub fn steam_user_named(id: &str) -> Option<String> {
    steam::user_named(&steam::roots(), id)
}

/// What Steam's Web API says of a sign-in's token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SteamAnswer {
    /// The games the account owns.
    Games(Vec<Owned>),
    /// Steam does not take the token.
    Rejected,
    /// Steam took the token but lists nothing: the account owns no games.
    Nothing,
}

/// Asks Steam's Web API for the games an account owns, with a sign-in's
/// access token.
pub(crate) fn ask_owned(token: &str, steamid: &str) -> anyhow::Result<SteamAnswer> {
    let agent = crate::http::agent(Duration::from_secs(30));
    // Steam's Web API answers 429 now and then even to a first request in
    // hours; a few tries a moment apart get through.
    let mut tries = 0;
    let response = loop {
        tries += 1;
        let response = agent
            .get(STEAM_OWNED)
            .query("access_token", token)
            .query("steamid", steamid)
            .query("include_appinfo", "1")
            .query("include_played_free_games", "1")
            .query("format", "json")
            .call();
        match response {
            Ok(response) => break response,
            Err(ureq::Error::StatusCode(429)) if tries < 4 => {
                std::thread::sleep(Duration::from_secs(2 * tries));
            }
            Err(ureq::Error::StatusCode(401 | 403)) => return Ok(SteamAnswer::Rejected),
            Err(ureq::Error::StatusCode(code)) => bail!("Steam answered {code}"),
            Err(err) => return Err(err).context("could not reach Steam"),
        }
    };
    parse_steam(&response.into_body().read_to_string()?)
}

/// What Steam's store says about a game, for its page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreDetails {
    pub summary: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub released: Option<String>,
    pub genres: Vec<String>,
    pub platforms: Vec<String>,
    /// The space the requirements ask for, in bytes.
    pub space: Option<u64>,
}

impl StoreDetails {
    /// Fills in what a game's entry lacks.
    pub fn fill(&self, game: &mut Owned) {
        game.summary = game.summary.take().or_else(|| self.summary.clone());
        game.developer = game.developer.take().or_else(|| self.developer.clone());
        game.publisher = game.publisher.take().or_else(|| self.publisher.clone());
        game.released = game.released.take().or_else(|| self.released.clone());
        if game.genres.is_empty() {
            game.genres.clone_from(&self.genres);
        }
        if game.platforms.is_empty() {
            game.platforms.clone_from(&self.platforms);
        }
        game.space = game.space.or(self.space);
    }
}

/// Asks Steam's store about an app; `None` when the store has no page for
/// it (removed from sale, a tool).
///
/// # Errors
///
/// When Steam's store cannot be reached or answers oddly.
pub fn steam_details(app: &str) -> anyhow::Result<Option<StoreDetails>> {
    let text = crate::http::agent(Duration::from_secs(30))
        .get(STEAM_DETAILS)
        .query("appids", app)
        .query("l", "english")
        .call()
        .context("could not reach Steam's store")?
        .into_body()
        .read_to_string()?;
    parse_steam_details(&text, app)
}

fn parse_steam_details(text: &str, app: &str) -> anyhow::Result<Option<StoreDetails>> {
    #[derive(Deserialize)]
    struct Entry {
        success: bool,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        about_the_game: Option<String>,
        short_description: Option<String>,
        #[serde(default)]
        developers: Vec<String>,
        #[serde(default)]
        publishers: Vec<String>,
        release_date: Option<Released>,
        #[serde(default)]
        genres: Vec<Genre>,
        platforms: Option<HashMap<String, bool>>,
        /// An object with `minimum` and `recommended`, or `[]` when the
        /// store lists none.
        pc_requirements: Option<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Released {
        date: Option<String>,
    }
    #[derive(Deserialize)]
    struct Genre {
        description: String,
    }
    let mut reply: HashMap<String, Entry> =
        serde_json::from_str(text).context("Steam's store details: not JSON")?;
    let Some(data) = reply
        .remove(app)
        .filter(|entry| entry.success)
        .and_then(|entry| entry.data)
    else {
        return Ok(None);
    };
    let joined = |names: &[String]| (!names.is_empty()).then(|| names.join(", "));
    let platforms = data.platforms.unwrap_or_default();
    Ok(Some(StoreDetails {
        summary: data
            .about_the_game
            .as_deref()
            .and_then(plain_text)
            .or_else(|| data.short_description.as_deref().and_then(plain_text)),
        developer: joined(&data.developers),
        publisher: joined(&data.publishers),
        released: data
            .release_date
            .and_then(|released| released.date)
            .filter(|date| !date.is_empty()),
        genres: data
            .genres
            .into_iter()
            .map(|genre| genre.description)
            .collect(),
        platforms: [("windows", "Windows"), ("mac", "Mac"), ("linux", "Linux")]
            .into_iter()
            .filter(|(key, _)| platforms.get(*key) == Some(&true))
            .map(|(_, name)| name.to_owned())
            .collect(),
        space: data.pc_requirements.as_ref().and_then(|wanted| {
            ["minimum", "recommended"]
                .into_iter()
                .filter_map(|level| wanted.get(level)?.as_str())
                .find_map(space_wanted)
        }),
    }))
}

/// The space a store's requirements ask for, in bytes: "Storage: 8 GB
/// available space", or "Hard Drive:" in older pages. GB counts as 1024 MB,
/// as Windows does.
fn space_wanted(html: &str) -> Option<u64> {
    const LABELS: [&str; 4] = ["storage:", "hard drive:", "hard disk space:", "disk space:"];
    let text = plain_text(html)?.to_lowercase();
    let line = text
        .lines()
        .find(|line| LABELS.iter().any(|label| line.starts_with(label)))?;
    let rest = line.split_once(':')?.1.trim_start();
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    let value: f64 = number.replace(',', ".").parse().ok()?;
    let unit = rest[number.len()..].trim_start();
    let scale: f64 = if unit.starts_with("tb") {
        1024.0 * 1024.0 * 1024.0 * 1024.0
    } else if unit.starts_with("gb") {
        1024.0 * 1024.0 * 1024.0
    } else if unit.starts_with("mb") {
        1024.0 * 1024.0
    } else {
        return None;
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive estimate, far below u64's range"
    )]
    let bytes = (value * scale) as u64;
    (bytes > 0).then_some(bytes)
}

/// Writes a store's list to the cache as it is now, as after filling in
/// what Steam's store said about a game.
///
/// # Errors
///
/// When the cache cannot be written.
pub fn save(shop: Shop, games: &[Owned]) -> anyhow::Result<()> {
    let Some(file) = cache_file(shop) else {
        return Ok(());
    };
    paths::write_atomic(&file, &serde_json::to_vec(games)?)
        .with_context(|| format!("could not write {}", file.display()))
}

fn parse_steam(text: &str) -> anyhow::Result<SteamAnswer> {
    #[derive(Deserialize)]
    struct Reply {
        response: Response,
    }
    #[derive(Deserialize)]
    struct Response {
        games: Option<Vec<Game>>,
    }
    #[derive(Deserialize)]
    struct Game {
        appid: u64,
        name: Option<String>,
        #[serde(default)]
        playtime_forever: u64,
        #[serde(default)]
        playtime_2weeks: u64,
        #[serde(default)]
        playtime_linux_forever: u64,
        #[serde(default)]
        playtime_windows_forever: u64,
        #[serde(default)]
        playtime_mac_forever: u64,
        #[serde(default)]
        playtime_deck_forever: u64,
        /// 0 when never played.
        #[serde(default)]
        rtime_last_played: u64,
    }
    let reply: Reply = serde_json::from_str(text).context("Steam's list of games: not JSON")?;
    let Some(games) = reply.response.games.filter(|games| !games.is_empty()) else {
        return Ok(SteamAnswer::Nothing);
    };
    Ok(SteamAnswer::Games(
        games
            .into_iter()
            .map(|game| {
                let mut owned = steam_game(game.appid, game.name, game.playtime_forever);
                owned.last_played = Some(game.rtime_last_played).filter(|time| *time > 0);
                owned.recent_minutes = game.playtime_2weeks;
                owned.played_on = PlayedOn {
                    linux: game.playtime_linux_forever,
                    windows: game.playtime_windows_forever,
                    mac: game.playtime_mac_forever,
                    deck: game.playtime_deck_forever,
                };
                owned
            })
            .collect(),
    ))
}

/// A Steam game for the Store's list: its title (or "Steam app N"), Steam's
/// images and the minutes played.
pub(crate) fn steam_game(app: u64, name: Option<String>, minutes: u64) -> Owned {
    let app = app.to_string();
    let title = name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Steam app {app}"));
    let mut owned = Owned::new(Shop::Steam, &app, title);
    owned.cover_url = Some(format!("{STEAM_IMAGES}/{app}/{}", steam_image(Kind::Cover)));
    owned.hero_url = Some(format!("{STEAM_IMAGES}/{app}/{}", steam_image(Kind::Hero)));
    owned.minutes = Some(minutes);
    owned
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

    #[test]
    fn reads_steams_owned_games() {
        let json = r#"{"response": {"game_count": 2, "games": [
            {"appid": 620, "name": "Portal 2", "playtime_forever": 754, "img_icon_url": "x",
             "playtime_2weeks": 30, "playtime_linux_forever": 500, "playtime_windows_forever": 4,
             "playtime_mac_forever": 0, "playtime_deck_forever": 0, "rtime_last_played": 1700000000},
            {"appid": 4000, "name": " ", "playtime_forever": 0, "rtime_last_played": 0}]}}"#;
        let SteamAnswer::Games(games) = parse_steam(json).unwrap() else {
            panic!("games expected");
        };
        assert_eq!(games[0].id, "steam:620");
        assert_eq!(games[0].title, "Portal 2");
        assert_eq!(games[0].shop, Shop::Steam);
        assert_eq!(games[0].store(), None);
        assert_eq!(games[0].minutes, Some(754));
        assert_eq!(games[0].last_played, Some(1_700_000_000));
        assert_eq!(games[0].recent_minutes, 30);
        assert_eq!(
            games[0].played_on,
            PlayedOn {
                linux: 500,
                windows: 4,
                ..PlayedOn::default()
            }
        );
        assert_eq!(games[1].last_played, None);
        assert!(games[1].played_on.is_empty());
        assert_eq!(
            games[0].cover_url.as_deref(),
            Some(
                "https://shared.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg"
            )
        );
        assert_eq!(games[1].title, "Steam app 4000");
        // A private profile: Steam answers, with no games in it.
        assert_eq!(
            parse_steam(r#"{"response": {}}"#).unwrap(),
            SteamAnswer::Nothing
        );
    }

    #[test]
    fn reads_steams_store_details() {
        let json = r#"{"620": {"success": true, "data": {
            "about_the_game": "<p>Portal 2 &amp; more.</p>",
            "developers": ["Valve"], "publishers": ["Valve"],
            "release_date": {"coming_soon": false, "date": "18 Apr, 2011"},
            "genres": [{"id": "1", "description": "Action"}],
            "platforms": {"windows": true, "mac": false, "linux": true},
            "pc_requirements": {"minimum": "<strong>Minimum:</strong><br><ul><li><strong>Memory:</strong> 2 GB RAM<br></li><li><strong>Storage:</strong> 8 GB available space<br></li></ul>"}}}}"#;
        let details = parse_steam_details(json, "620").unwrap().unwrap();
        assert_eq!(details.space, Some(8 * 1024 * 1024 * 1024));
        assert_eq!(details.summary.as_deref(), Some("Portal 2 & more."));
        assert_eq!(details.developer.as_deref(), Some("Valve"));
        assert_eq!(details.released.as_deref(), Some("18 Apr, 2011"));
        assert_eq!(details.platforms, ["Windows", "Linux"]);
        assert_eq!(
            parse_steam_details(r#"{"1": {"success": false}}"#, "1").unwrap(),
            None
        );
        let mut game = Owned::new(Shop::Steam, "620", "Portal 2".into());
        game.developer = Some("Kept".into());
        details.fill(&mut game);
        assert_eq!(game.developer.as_deref(), Some("Kept"));
        assert_eq!(game.genres, ["Action"]);
    }

    #[test]
    fn reads_the_space_a_game_wants() {
        let li = |line: &str| {
            format!("<ul><li><strong>OS:</strong> Windows<br></li><li>{line}</li></ul>")
        };
        assert_eq!(
            space_wanted(&li("<strong>Hard Drive:</strong> 500 MB HD space")),
            Some(500 * 1024 * 1024)
        );
        assert_eq!(
            space_wanted(&li("<strong>Storage:</strong> 1.5 GB available space")),
            Some(1_610_612_736)
        );
        assert_eq!(space_wanted(&li("<strong>Memory:</strong> 2 GB RAM")), None);
        assert_eq!(space_wanted(&li("<strong>Storage:</strong> lots")), None);
        // No requirements: Steam sends an empty list.
        let json = r#"{"1": {"success": true, "data": {"pc_requirements": []}}}"#;
        assert_eq!(parse_steam_details(json, "1").unwrap().unwrap().space, None);
    }

    #[test]
    fn keeps_the_cache_readable() {
        let game = Owned::new(Store::Gog, "1", "Game".into());
        let json = serde_json::to_string(&game).unwrap();
        assert!(json.contains(r#""store":"gog""#), "{json}");
        let read: Owned = serde_json::from_str(&json).unwrap();
        assert_eq!(read.shop, Shop::Gog);
    }
}
