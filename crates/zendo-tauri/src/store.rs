//! The Store page: the games the signed-in stores say the user owns, carried
//! from `zendo_shell::store::StoreBrowser` (which is also the Slint window's)
//! and its lists asked of each store. Install and the queue are the Downloads
//! part of this slice.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::owned::{self, Owned, Shop};
use zendo_core::xodus;
use zendo_shell::format;
use zendo_shell::steam_list;
use zendo_shell::store::{StoreBrowser, StoreChip, StoreChips, StorePick};
use zendo_shell::t;

use crate::accounts;
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
fn accounts_changed(app: &AppHandle) {
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

/// A store game's page: its facts and whether the store's engine can install
/// it. Installing is the queue (the next part of the slice).
#[derive(Debug, Serialize)]
pub struct OwnedDto {
    id: String,
    title: String,
    source_line: String,
    cover: Option<String>,
    installed: bool,
    details: Vec<(String, String)>,
    install: String,
    install_on: bool,
    note: Option<(String, i32)>,
}

#[tauri::command]
pub fn owned_get(id: String, app: AppHandle) -> Result<OwnedDto, String> {
    let state = app.state::<StoreState>();
    let browser = browser(&state);
    let owned = browser
        .owned(&id)
        .cloned()
        .ok_or_else(|| format!("no store game with the id {id:?}"))?;
    let installed = browser.is_installed(&id);
    Ok(OwnedDto {
        id: owned.id.clone(),
        title: owned.title.clone(),
        source_line: format!("{} · {}", owned.shop.label(), owned.id),
        cover: owned.cover.as_ref().map(|path| path.display().to_string()),
        installed,
        details: fact_rows(&owned, installed),
        install: if installed {
            t!("action-label", action = "uninstall")
        } else {
            t!("install")
        },
        install_on: true,
        note: None,
    })
}

fn fact_rows(owned: &Owned, installed: bool) -> Vec<(String, String)> {
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
