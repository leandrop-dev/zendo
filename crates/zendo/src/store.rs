//! The Store page: the games the signed-in stores say the user owns, carried
//! from `zendo_shell::store::StoreBrowser` and its lists asked of each
//! store. Install and the queue are `downloads.rs`'s.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::downloads;
use zendo_core::info::{self, GameInfo};
use zendo_core::owned::{self, Owned, Shop};
use zendo_core::xodus;
use zendo_shell::format;
use zendo_shell::steam_list;
use zendo_shell::store::{StoreBrowser, StoreChip, StoreChips, StorePick};
use zendo_shell::store_facts::{self, InfoState};
use zendo_shell::t;

use crate::accounts;
use crate::downloads as queue;
use crate::library::{CardDto, LibraryState};

/// The Store page's state.
pub struct StoreState {
    pub(crate) browser: Mutex<StoreBrowser>,
    lists_loaded: AtomicBool,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            browser: Mutex::new(StoreBrowser::new()),
            lists_loaded: AtomicBool::new(false),
        }
    }
}

fn browser(state: &StoreState) -> MutexGuard<'_, StoreBrowser> {
    state.browser.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Serialize)]
pub struct StoreChipDto {
    key: String,
    label: String,
    count: Option<usize>,
    chosen: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct StoreChipsDto {
    sources: Vec<StoreChipDto>,
    sorts: Vec<StoreChipDto>,
}

#[derive(Debug, Serialize)]
pub struct StoreEmptyDto {
    title: String,
    /// No store is signed in: the page offers the way to Accounts.
    accounts: bool,
}

#[derive(Debug, Serialize)]
pub struct StoreDto {
    ready: bool,
    cards: Vec<CardDto>,
    chips: StoreChipsDto,
    cursor: usize,
    empty: Option<StoreEmptyDto>,
    search: String,
    status: Option<String>,
}

/// The Store page as it stands; the first call loads the cache and asks the
/// signed-in stores with no list yet.
#[tauri::command]
pub fn store_get(app: AppHandle, library: State<'_, LibraryState>) -> StoreDto {
    accounts::ready(&app);
    start(&app, &library);
    dto(&app)
}

fn start(app: &AppHandle, library: &LibraryState) {
    let state = app.state::<StoreState>();
    if state.lists_loaded.swap(true, Ordering::Relaxed) {
        return;
    }
    let ids: HashSet<String> = library.games().iter().map(|game| game.id.clone()).collect();
    {
        let mut browser = browser(&state);
        let _ = browser.set_installed(ids);
        let lists: Vec<(Shop, Vec<Owned>)> = Shop::ALL
            .into_iter()
            .map(|shop| (shop, owned::cached(shop)))
            .collect();
        let _ = browser.cached(lists);
    }
    accounts_changed(app);
    let _ = app.emit("store_changed", ());
}

/// Which stores are signed in, and whose games the page lists.
pub(crate) fn accounts_changed(app: &AppHandle) {
    let steam = accounts::steam_session(app).is_some();
    let xbox = xodus::installed().is_some();
    let mut listed = HashSet::new();
    let mut connected = Vec::new();
    for shop in Shop::ALL {
        let (listed_here, connected_here) = match shop {
            Shop::Xbox => (xbox, xbox),
            Shop::Steam => (steam, steam),
            engine => match engine.store() {
                Some(store) => (
                    !accounts::signed_out(app, store),
                    accounts::signed_in(app, store),
                ),
                None => (false, false),
            },
        };
        if listed_here {
            listed.insert(shop);
        }
        if connected_here {
            connected.push(shop);
        }
    }
    {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.set_accounts(listed, connected);
    }
    fetch_missing(app);
}

/// Asks every signed-in store that has no list yet.
fn fetch_missing(app: &AppHandle) {
    let missing: Vec<Shop> = {
        let state = app.state::<StoreState>();
        let browser = browser(&state);
        browser
            .connected()
            .iter()
            .copied()
            .filter(|shop| !browser.has_list(*shop))
            .collect()
    };
    for shop in missing {
        fetch_owned(app, shop);
    }
}

/// Asks a store for its games, on a thread.
fn fetch_owned(app: &AppHandle, shop: Shop) {
    {
        let state = app.state::<StoreState>();
        if !browser(&state).start_asking(shop) {
            return;
        }
    }
    let handle = app.clone();
    if shop == Shop::Xbox {
        std::thread::spawn(move || {
            let result = xodus::owned().map_err(|err| format!("{err:#}"));
            fetched(&handle, shop, result, &[]);
        });
        return;
    }
    if let Some(store) = shop.store() {
        std::thread::spawn(move || {
            let result = owned::fetch(store).map_err(|err| format!("{err:#}"));
            match result {
                Ok((games, problems)) => fetched(&handle, shop, Ok(games), &problems),
                Err(err) => fetched(&handle, shop, Err(err), &[]),
            }
        });
        return;
    }
    let Some(session) = accounts::steam_session(app) else {
        let state = app.state::<StoreState>();
        browser(&state).stop_asking(shop);
        return;
    };
    std::thread::spawn(move || {
        let result = steam_list::fetch_signed_in(session).map_err(|err| format!("{err:#}"));
        match result {
            Ok(steam_list::SteamFetch::Games { games, renewed, .. }) => {
                if let Some(session) = renewed {
                    crate::settings::save(
                        &handle,
                        crate::settings::Setting::SteamSession(Some(session.to_text())),
                    );
                }
                fetched(&handle, shop, Ok(games), &[]);
            }
            Ok(steam_list::SteamFetch::SignedOut) => {
                crate::settings::save(&handle, crate::settings::Setting::SteamSession(None));
                fetched(&handle, shop, Err(t!("steam-sign-in-again")), &[]);
            }
            Err(err) => fetched(&handle, shop, Err(err), &[]),
        }
    });
}

fn fetched(app: &AppHandle, shop: Shop, result: Result<Vec<Owned>, String>, problems: &[String]) {
    let kept = result.is_ok();
    {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.fetched(shop, result, problems);
    }
    let _ = app.emit("store_changed", ());
    if kept {
        let _ = app.emit("library_changed", ());
    }
}

#[tauri::command]
pub fn store_pick(key: String, app: AppHandle) -> StoreDto {
    if let Some(pick) = pick_of(&key) {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.pick(pick);
    }
    dto(&app)
}

#[tauri::command]
pub fn store_search(text: String, app: AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    let _ = browser.set_search(&text);
    drop(browser);
    dto(&app)
}

#[tauri::command]
pub fn store_cycle_sort(app: AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    let _ = browser.cycle_sort();
    drop(browser);
    dto(&app)
}

#[tauri::command]
pub fn store_click(id: String, app: AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    browser(&state).click(&id);
    dto(&app)
}

#[tauri::command]
pub fn store_step(step: String, app: AppHandle) -> StoreDto {
    if let Some(step) = move_of(&step) {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.step(step);
    }
    dto(&app)
}

#[tauri::command]
pub fn store_fit(columns: usize, height: f32, app: AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    let _ = browser.set_columns(columns);
    browser.scrolled(0.0, height);
    drop(browser);
    dto(&app)
}

#[tauri::command]
pub fn store_back(app: AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    let _ = browser.back();
    drop(browser);
    dto(&app)
}

/// Asks every store again (Refresh).
/// A store was signed in or out: the page lists what the accounts say now,
/// and a store with no list yet is asked.
#[tauri::command]
pub fn store_accounts_changed(app: AppHandle) -> StoreDto {
    accounts_changed(&app);
    dto(&app)
}

/// The store's list is forgotten, as after signing out of it.
pub(crate) fn forget(app: &AppHandle, shop: Shop) {
    {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.forget(shop);
    }
    owned::forget(shop);
    let _ = app.emit("store_changed", ());
}

#[tauri::command]
pub fn store_refresh(app: AppHandle) -> StoreDto {
    let connected: Vec<Shop> = {
        let state = app.state::<StoreState>();
        browser(&state).connected().to_vec()
    };
    for shop in connected {
        fetch_owned(&app, shop);
    }
    dto(&app)
}

/// A store game's page: its facts, and what the queue would do with it.
#[derive(Debug, Serialize)]
pub struct OwnedDto {
    id: String,
    title: String,
    source_line: String,
    cover: Option<String>,
    installed: bool,
    details: Vec<(String, String)>,
    /// 0 installed, 1 Install and Import, 2 queued, 3 running, 4 paused.
    job: i32,
    /// What the running button says, such as "Installing…".
    job_label: String,
    /// How far, 0 to 100; -1 for no bar.
    progress: f32,
    /// "Also install its DLC (n)", empty when there is none to give.
    dlc: String,
    dlc_on: bool,
    /// What installing it would do, or why it cannot be.
    note: String,
}

#[tauri::command]
pub fn owned_get(id: String, app: AppHandle) -> Result<OwnedDto, String> {
    let (owned, installed, info) = {
        let state = app.state::<StoreState>();
        let browser = browser(&state);
        let owned = browser
            .owned(&id)
            .cloned()
            .ok_or_else(|| format!("no store game with the id {id:?}"))?;
        let installed = browser.is_installed(&id);
        let info = browser.info.get(&id).cloned();
        (owned, installed, info)
    };
    // Sizes, versions and the DLC are asked once a session, on a thread.
    want_info(&app, &owned);
    Ok(page(&app, &owned, installed, info.as_ref()))
}

/// A store game's page as it is now.
pub(crate) fn page(
    app: &AppHandle,
    owned: &Owned,
    installed: bool,
    info: Option<&InfoState>,
) -> OwnedDto {
    let install = queue::install_state(app, owned, installed);
    OwnedDto {
        id: owned.id.clone(),
        title: owned.title.clone(),
        source_line: format!("{} · {}", owned.shop.label(), owned.id),
        cover: owned.cover.as_ref().map(|path| path.display().to_string()),
        installed,
        details: fact_rows(owned, installed, info),
        job: install.job,
        job_label: install.label,
        progress: install.progress,
        dlc: install.dlc,
        dlc_on: install.dlc_on,
        note: install.note,
    }
}

/// A store game's page, asked again by the frontend after a press.
pub(crate) fn page_of(app: &AppHandle, id: &str) -> Result<OwnedDto, String> {
    let (owned, installed, info) = {
        let state = app.state::<StoreState>();
        let browser = browser(&state);
        let owned = browser
            .owned(id)
            .cloned()
            .ok_or_else(|| format!("no store game with the id {id:?}"))?;
        let installed = browser.is_installed(id);
        let info = browser.info.get(id).cloned();
        (owned, installed, info)
    };
    Ok(page(app, &owned, installed, info.as_ref()))
}

fn fact_rows(owned: &Owned, installed: bool, info: Option<&InfoState>) -> Vec<(String, String)> {
    let mut rows = vec![(t!("fact-store"), owned.shop.label().to_owned())];
    if owned.shared {
        rows.push((t!("fact-owner"), t!("store-shared")));
    }
    rows.push((
        t!("fact-status"),
        if installed {
            t!("installed")
        } else {
            t!("fact-not-installed")
        },
    ));
    if !owned.platforms.is_empty() {
        rows.push((t!("fact-platforms"), owned.platforms.join(", ")));
    }
    if let Some(developer) = &owned.developer {
        rows.push((t!("fact-developer"), developer.clone()));
    }
    rows.extend(store_facts::facts(Some(owned), info, installed));
    if let Some(minutes) = owned.minutes {
        rows.push((
            t!("fact-play-time"),
            if minutes == 0 {
                t!("fact-never")
            } else {
                format::play_time(minutes)
            },
        ));
    }
    rows
}

// What the stores say of a game.

/// Asks the game's store, on a thread, how big it is and its latest version,
/// unless that was asked already or Zendo is not signed in to the store; for
/// a Steam game, asks Steam's store for its page; for a Game Pass game, asks
/// the launcher's backend what it would install.
fn want_info(app: &AppHandle, owned: &Owned) {
    {
        let state = app.state::<StoreState>();
        if browser(&state).info.contains_key(&owned.id) {
            return;
        }
    }
    if let Some(app_id) = owned.id.strip_prefix("steam:") {
        want_steam_details(app, &owned.id, app_id);
        return;
    }
    if let Some(product) = owned.id.strip_prefix("xbox:") {
        want_xbox_package(app, &owned.id, product);
        return;
    }
    let Some((store, app_id)) = owned::store_of(&owned.id) else {
        return;
    };
    if !accounts::signed_in(app, store) {
        return;
    }
    let platform = downloads::install_platform(owned);
    asking(app, &owned.id);
    let handle = app.clone();
    let id = owned.id.clone();
    let app_id = app_id.to_owned();
    let platform = platform.to_owned();
    std::thread::spawn(move || {
        let result = info::fetch(store, &app_id, &platform).map_err(|err| format!("{err:#}"));
        arrived(&handle, &id, result.map(InfoState::Got));
    });
}

fn want_xbox_package(app: &AppHandle, id: &str, product: &str) {
    asking(app, id);
    let handle = app.clone();
    let id = id.to_owned();
    let product = product.to_owned();
    std::thread::spawn(move || {
        let result = xodus::package(&product).map_err(|err| format!("{err:#}"));
        match result {
            Ok(package) => {
                let size = package.download_bytes;
                {
                    let state = handle.state::<StoreState>();
                    let mut browser = browser(&state);
                    let _ = browser.set_package(&id, package);
                }
                arrived(
                    &handle,
                    &id,
                    Ok(InfoState::Got(GameInfo {
                        download_size: size,
                        ..GameInfo::default()
                    })),
                );
            }
            Err(err) => arrived(&handle, &id, Err(err)),
        }
    });
}

/// Asks Steam's store about a Steam game the list knows only by name; once a
/// session.
fn want_steam_details(app: &AppHandle, id: &str, app_id: &str) {
    let known = {
        let state = app.state::<StoreState>();
        let browser = browser(&state);
        browser.owned(id).is_none_or(|game| {
            (game.summary.is_some() || game.developer.is_some()) && game.space.is_some()
        })
    };
    if known {
        return;
    }
    asking(app, id);
    let handle = app.clone();
    let id = id.to_owned();
    let app_id = app_id.to_owned();
    std::thread::spawn(move || {
        // Through the shared cache, so the game page's facts and this are
        // asked once between them.
        let result = match app_id.parse::<u32>() {
            Ok(number) => zendo_core::steam_facts::details(number),
            Err(_) => owned::steam_details(&app_id),
        }
        .map_err(|err| format!("{err:#}"));
        match result {
            Ok(details) => {
                if let Some(details) = details {
                    let changed = {
                        let state = handle.state::<StoreState>();
                        let mut browser = browser(&state);
                        browser.fill_steam(&id, &details)
                    };
                    if changed
                        && let Some(games) = {
                            let state = handle.state::<StoreState>();
                            let browser = browser(&state);
                            browser.list(Shop::Steam).map(<[Owned]>::to_vec)
                        }
                    {
                        let _ = owned::save(Shop::Steam, &games);
                    }
                }
                arrived(&handle, &id, Ok(InfoState::Got(info::GameInfo::default())));
            }
            Err(err) => arrived(&handle, &id, Err(err)),
        }
    });
}

/// The game is being asked about: the page says so.
fn asking(app: &AppHandle, id: &str) {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    browser.info.insert(id.to_owned(), InfoState::Asking);
}

/// The store's answer is in: kept, and the page asks again.
fn arrived(app: &AppHandle, id: &str, result: Result<InfoState, String>) {
    let state = app.state::<StoreState>();
    let mut browser = browser(&state);
    browser
        .info
        .insert(id.to_owned(), result.unwrap_or_else(InfoState::Failed));
    drop(browser);
    let _ = app.emit("store_changed", ());
}

/// The library changed (an install ended, a game was added): the Store's
/// cards say which games are installed.
pub(crate) fn library_changed(app: &AppHandle) {
    let ids: HashSet<String> = app
        .state::<LibraryState>()
        .games()
        .iter()
        .map(|game| game.id.clone())
        .collect();
    {
        let state = app.state::<StoreState>();
        let mut browser = browser(&state);
        let _ = browser.set_installed(ids);
    }
    let _ = app.emit("store_changed", ());
}

// The DTO.

pub(crate) fn dto(app: &AppHandle) -> StoreDto {
    let state = app.state::<StoreState>();
    let browser = browser(&state);
    let cards: Vec<CardDto> = (0..browser.len())
        .filter_map(|place| browser.card(place))
        .map(CardDto::from)
        .collect();
    StoreDto {
        ready: true,
        cards,
        chips: chips(browser.chips()),
        cursor: browser.cursor(),
        empty: browser.empty().map(|empty| StoreEmptyDto {
            title: empty.title,
            accounts: empty.accounts,
        }),
        search: browser.search().to_owned(),
        status: browser.problems().first().cloned(),
    }
}

fn chips(chips: StoreChips) -> StoreChipsDto {
    StoreChipsDto {
        sources: chips.sources.into_iter().map(chip).collect(),
        sorts: chips.sorts.into_iter().map(chip).collect(),
    }
}

fn chip(chip: StoreChip) -> StoreChipDto {
    StoreChipDto {
        key: key_of(chip.pick),
        label: chip.label,
        count: chip.count,
        chosen: chip.chosen,
    }
}

fn key_of(pick: StorePick) -> String {
    match pick {
        StorePick::All => "all".to_owned(),
        StorePick::Shop(shop) => format!("shop:{}", shop_key(shop)),
        StorePick::ToggleInstalled => "installed".to_owned(),
        StorePick::Sort(sort) => format!("sort:{}", sort_name(sort)),
    }
}

fn pick_of(key: &str) -> Option<StorePick> {
    let (kind, value) = key.split_once(':').unwrap_or((key, ""));
    Some(match kind {
        "all" => StorePick::All,
        "installed" => StorePick::ToggleInstalled,
        "shop" => StorePick::Shop(shop_of(value)?),
        "sort" => StorePick::Sort(sort_of(value)?),
        _ => return None,
    })
}

fn shop_key(shop: Shop) -> &'static str {
    match shop {
        Shop::Steam => "steam",
        Shop::Epic => "epic",
        Shop::Gog => "gog",
        Shop::Amazon => "amazon",
        Shop::Xbox => "xbox",
    }
}

fn shop_of(name: &str) -> Option<Shop> {
    Some(match name {
        "steam" => Shop::Steam,
        "epic" => Shop::Epic,
        "gog" => Shop::Gog,
        "amazon" => Shop::Amazon,
        "xbox" => Shop::Xbox,
        _ => return None,
    })
}

fn sort_name(sort: zendo_core::Sort) -> &'static str {
    match sort {
        zendo_core::Sort::Recent => "recent",
        zendo_core::Sort::MostPlayed => "most-played",
        zendo_core::Sort::Title => "title",
    }
}

fn sort_of(name: &str) -> Option<zendo_core::Sort> {
    Some(match name {
        "recent" => zendo_core::Sort::Recent,
        "most-played" => zendo_core::Sort::MostPlayed,
        "title" => zendo_core::Sort::Title,
        _ => return None,
    })
}

fn move_of(step: &str) -> Option<zendo_core::browse::Move> {
    use zendo_core::browse::Move;
    Some(match step {
        "left" => Move::Left,
        "right" => Move::Right,
        "up" => Move::Up,
        "down" => Move::Down,
        "page-up" => Move::PageUp,
        "page-down" => Move::PageDown,
        "first" => Move::First,
        "last" => Move::Last,
        _ => return None,
    })
}
