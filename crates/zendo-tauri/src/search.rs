//! Search Nexus: the sheet over a game's page that searches its mods on
//! Nexus, shows them a page at a time with their pictures, lists a mod's
//! files, and takes one into the game's mods: straight away with Premium,
//! else through the mod's page in Zendo's Nexus window.
//!
//! The state and words are `zendo_shell::nexus_search`'s, shared with the
//! Slint window; each question to Nexus runs on a thread, and the pictures are
//! kept as files for the page's asset protocol.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::config::Config;
use zendo_core::mods::Overview;
use zendo_core::nexus::{self, Link};
use zendo_core::nexus_search::{self, Order};
use zendo_shell::artwork::WORKERS;
use zendo_shell::nexus_search::{self as shell, FilesState, FilesStep, Search};
use zendo_shell::sign_in_window::{self, FromSite, Handle};
use zendo_shell::t;

use crate::library::LibraryState;
use crate::mods::ModsHandle;
use crate::settings::{self, Setting};

/// The sheet, its key and adult setting, its pictures and Nexus's window.
pub struct SearchState {
    search: Mutex<Option<Search>>,
    /// The pictures' files, by the mod's id.
    thumbs: Mutex<HashMap<u64, String>>,
    /// Set to stop the pictures of a search on its way.
    stop: Mutex<Arc<AtomicBool>>,
    /// The key the sheet asks Nexus with, read once as it opens.
    key: Mutex<Option<String>>,
    adult: Mutex<bool>,
    /// Nexus's window, while it is open.
    site: Mutex<Option<Handle>>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            search: Mutex::new(None),
            thumbs: Mutex::new(HashMap::new()),
            stop: Mutex::new(Arc::new(AtomicBool::new(false))),
            key: Mutex::new(None),
            adult: Mutex::new(false),
            site: Mutex::new(None),
        }
    }
}

fn lock(state: &SearchState) -> MutexGuard<'_, Option<Search>> {
    state.search.lock().unwrap_or_else(PoisonError::into_inner)
}

fn thumbs(state: &SearchState) -> MutexGuard<'_, HashMap<u64, String>> {
    state.thumbs.lock().unwrap_or_else(PoisonError::into_inner)
}

fn key_of(state: &SearchState) -> Option<String> {
    state
        .key
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn adult_of(state: &SearchState) -> bool {
    *state.adult.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A chip: an order, and whether it is the one.
#[derive(Debug, Serialize)]
pub struct ChipDto {
    id: i32,
    label: String,
    chosen: bool,
}

/// One row: a mod found (0), one of its files (1), or a line about its files
/// (2).
#[derive(Debug, Default, Serialize)]
pub struct SearchRowDto {
    kind: i32,
    name: String,
    version: String,
    byline: String,
    summary: String,
    numbers: String,
    have: String,
    adult: String,
    thumb: Option<String>,
    files_label: String,
    page_label: String,
    file_facts: String,
    file_action: String,
    file_have: String,
    bad: bool,
    /// The mod a press is for, and a file of it.
    mod_id: u64,
    file_id: u64,
}

/// The sheet as the page draws it.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the sheet is drawn by"
)]
#[derive(Debug, Default, Serialize)]
pub struct SearchSheetDto {
    open: bool,
    title: String,
    words: String,
    adult: bool,
    orders: Vec<ChipDto>,
    /// Why searching is limited (no key, no Premium); empty when it is not.
    limit: String,
    rows: Vec<SearchRowDto>,
    more: bool,
    asking: bool,
    footer: String,
    footer_bad: bool,
}

/// The thumbnail's file, downloaded once and kept in the cache.
fn thumb_file(mod_id: u64, url: &str) -> Option<String> {
    let dir = zendo_shell::artwork::thumb_dir()?.with_file_name("nexus-thumbs");
    for ext in ["jpg", "png", "webp", "gif"] {
        let kept = dir.join(format!("{mod_id}.{ext}"));
        if kept.is_file() {
            return Some(kept.display().to_string());
        }
    }
    let bytes = zendo_core::steamgriddb::thumbnail(url).ok()?;
    let ext = zendo_core::covers::image_extension(&bytes)?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{mod_id}.{ext}"));
    std::fs::write(&path, &bytes).ok()?;
    Some(path.display().to_string())
}

fn dto(app: &AppHandle) -> SearchSheetDto {
    let state = app.state::<SearchState>();
    let guard = lock(&state);
    let Some(search) = guard.as_ref() else {
        return SearchSheetDto::default();
    };
    let has_key = key_of(&state).is_some();
    let premium = crate::nexus::premium(app) == Some(true);
    let overview = {
        let mods = app.state::<ModsHandle>();
        let mods = mods.lock();
        mods.overview(&search.game).cloned()
    };
    let kept = thumbs(&state);
    let mut sheet = SearchSheetDto {
        open: true,
        title: t!("search-title", title = search.title.as_str()),
        words: search.words.clone(),
        adult: adult_of(&state),
        orders: Order::ALL
            .iter()
            .enumerate()
            .map(|(at, order)| ChipDto {
                id: i32::try_from(at).unwrap_or(i32::MAX),
                label: shell::order_name(*order),
                chosen: search.order == *order,
            })
            .collect(),
        limit: if !has_key {
            t!("search-needs-key")
        } else if !premium {
            t!("search-not-premium")
        } else {
            String::new()
        },
        rows: rows_of(search, overview.as_ref(), has_key, premium, &kept),
        more: search.has_more(),
        asking: search.asking,
        ..SearchSheetDto::default()
    };
    (sheet.footer, sheet.footer_bad) = search.footer();
    sheet
}

fn rows_of(
    search: &Search,
    overview: Option<&Overview>,
    has_key: bool,
    premium: bool,
    pictures: &HashMap<u64, String>,
) -> Vec<SearchRowDto> {
    let (have, have_files) = shell::have(overview, &search.domain);
    let mut rows = Vec::new();
    for hit in &search.hits {
        let open = search
            .open
            .as_ref()
            .filter(|(id, _)| *id == hit.mod_id)
            .map(|(_, state)| state);
        rows.push(SearchRowDto {
            kind: 0,
            name: hit.name.clone(),
            version: hit.version.clone(),
            byline: shell::byline(hit),
            summary: hit.summary.clone(),
            numbers: shell::numbers(hit),
            have: if have.contains(&hit.mod_id) {
                t!("search-have")
            } else {
                String::new()
            },
            adult: if hit.adult {
                t!("search-adult-badge")
            } else {
                String::new()
            },
            thumb: pictures.get(&hit.mod_id).cloned(),
            files_label: if !has_key {
                String::new()
            } else if open.is_some() {
                t!("search-hide-files")
            } else {
                t!("search-files")
            },
            page_label: t!("search-page"),
            mod_id: hit.mod_id,
            ..SearchRowDto::default()
        });
        let Some(state) = open else {
            continue;
        };
        match state {
            FilesState::Asking => rows.push(note(t!("search-asking-files"), false)),
            FilesState::Failed(err) => rows.push(note(err.clone(), true)),
            FilesState::Got(files) if files.is_empty() => {
                rows.push(note(t!("search-no-files"), false));
            }
            FilesState::Got(files) => {
                for file in files {
                    let have_it = have_files.contains(&(hit.mod_id, file.file_id));
                    rows.push(SearchRowDto {
                        kind: 1,
                        name: file.name.clone(),
                        file_facts: shell::file_facts(file),
                        file_action: if have_it {
                            String::new()
                        } else if premium {
                            t!("search-download")
                        } else {
                            t!("search-get-on-nexus")
                        },
                        file_have: if have_it {
                            t!("search-have-file")
                        } else {
                            String::new()
                        },
                        mod_id: hit.mod_id,
                        file_id: file.file_id,
                        ..SearchRowDto::default()
                    });
                }
            }
        }
    }
    rows
}

fn note(text: String, bad: bool) -> SearchRowDto {
    SearchRowDto {
        kind: 2,
        name: text,
        bad,
        ..SearchRowDto::default()
    }
}

fn changed(app: &AppHandle) {
    let _ = app.emit("search_changed", ());
}

/// A new search: the pictures on their way stop.
fn restart_pictures(app: &AppHandle) {
    let state = app.state::<SearchState>();
    let mut stop = state.stop.lock().unwrap_or_else(PoisonError::into_inner);
    let old = std::mem::replace(&mut *stop, Arc::new(AtomicBool::new(false)));
    old.store(true, Ordering::Relaxed);
}

fn game_overview(app: &AppHandle, id: &str) -> Option<Overview> {
    let mods = app.state::<ModsHandle>();
    let mods = mods.lock();
    mods.overview(id).cloned()
}

#[tauri::command]
pub fn search_get(app: AppHandle) -> SearchSheetDto {
    dto(&app)
}

/// Search Nexus, from the Mods card: the key and the adult setting are read
/// on a thread (the keyring may prompt), and the sheet opens from it.
#[tauri::command]
pub fn search_open(id: String, app: AppHandle) -> SearchSheetDto {
    let handle = app.clone();
    std::thread::spawn(move || {
        let config = Config::load().unwrap_or_default();
        let key = config.nexus_key.clone();
        let adult = config.nexus_adult;
        let Some(game) = (handle.state::<LibraryState>()).game(&id) else {
            return;
        };
        let Some(domain) = game_overview(&handle, &id).and_then(|overview| overview.nexus) else {
            return;
        };
        {
            let state = handle.state::<SearchState>();
            *key_of_slot(&state) = key;
            *state.adult.lock().unwrap_or_else(PoisonError::into_inner) = adult;
            thumbs(&state).clear();
            *lock(&state) = Some(Search::new(&id, game.title, domain));
        }
        restart_pictures(&handle);
        run(&handle, false);
        changed(&handle);
    });
    dto(&app)
}

fn key_of_slot(state: &SearchState) -> MutexGuard<'_, Option<String>> {
    state.key.lock().unwrap_or_else(PoisonError::into_inner)
}

#[tauri::command]
pub fn search_words(text: String, app: AppHandle) -> SearchSheetDto {
    {
        let state = app.state::<SearchState>();
        let mut guard = lock(&state);
        if let Some(search) = guard.as_mut() {
            search.words = text;
        }
    }
    dto(&app)
}

/// Enter and Search, and a new order or adult setting: from the start.
#[tauri::command]
pub fn search_run(more: bool, app: AppHandle) -> SearchSheetDto {
    run(&app, more);
    dto(&app)
}

#[tauri::command]
pub fn search_order(at: usize, app: AppHandle) -> SearchSheetDto {
    {
        let state = app.state::<SearchState>();
        let mut guard = lock(&state);
        if let (Some(order), Some(search)) = (Order::ALL.get(at).copied(), guard.as_mut()) {
            search.order = order;
        }
    }
    run(&app, false);
    dto(&app)
}

#[tauri::command]
pub fn search_adult(app: AppHandle) -> SearchSheetDto {
    let on = {
        let state = app.state::<SearchState>();
        let mut adult = state.adult.lock().unwrap_or_else(PoisonError::into_inner);
        *adult = !*adult;
        *adult
    };
    settings::save(&app, Setting::NexusAdult(on));
    run(&app, false);
    dto(&app)
}

/// Files on a mod: Nexus is asked for them, or they are hidden again.
#[tauri::command]
pub fn search_files(mod_id: u64, app: AppHandle) -> SearchSheetDto {
    let state = app.state::<SearchState>();
    let key = key_of(&state);
    let (step, domain) = {
        let mut guard = lock(&state);
        let Some(search) = guard.as_mut() else {
            return dto(&app);
        };
        (
            search.toggle_files(mod_id, key.is_some()),
            search.domain.clone(),
        )
    };
    if let (FilesStep::Ask(mod_id), Some(key)) = (step, key) {
        let handle = app.clone();
        std::thread::spawn(move || {
            let result = nexus::files(&key, &domain, mod_id).map_err(|err| format!("{err:#}"));
            {
                let state = handle.state::<SearchState>();
                let mut guard = lock(&state);
                if let Some(search) = guard.as_mut() {
                    search.got_files(mod_id, result);
                }
            }
            changed(&handle);
        });
    }
    dto(&app)
}

/// A file's page, or a mod's, in Nexus's window: the link or file it gives
/// is taken into the game's mods.
#[tauri::command]
pub fn search_page(mod_id: u64, file: Option<u64>, app: AppHandle) -> SearchSheetDto {
    let handle = app.clone();
    std::thread::spawn(move || site(&handle, mod_id, file));
    dto(&app)
}

/// A file of a mod found: into the game's mods with Premium, else the file's
/// Mod manager download in Nexus's window.
#[tauri::command]
pub fn search_download(mod_id: u64, file: u64, app: AppHandle) -> SearchSheetDto {
    if crate::nexus::premium(&app) == Some(true) {
        let link = {
            let state = app.state::<SearchState>();
            let guard = lock(&state);
            guard
                .as_ref()
                .map(|search| (search.game.clone(), search.link(mod_id, file)))
        };
        if let Some((game, link)) = link {
            crate::nexus::say(&app, t!("search-downloading"));
            crate::nexus::download(&app, &game, link, false);
        }
    } else {
        let handle = app.clone();
        std::thread::spawn(move || site(&handle, mod_id, Some(file)));
    }
    dto(&app)
}

#[tauri::command]
pub fn search_close(app: AppHandle) -> SearchSheetDto {
    {
        let state = app.state::<SearchState>();
        restart_pictures(&app);
        if let Some(open) = state
            .site
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            open.close();
        }
        thumbs(&state).clear();
        *lock(&state) = None;
    }
    changed(&app);
    dto(&app)
}

/// Asks Nexus for the search as it stands: from the start, or the next page.
fn run(app: &AppHandle, more: bool) {
    let adult = adult_of(&app.state::<SearchState>());
    let ask = {
        let state = app.state::<SearchState>();
        let mut guard = lock(&state);
        let Some(search) = guard.as_mut() else {
            return;
        };
        if more {
            search.more(adult)
        } else {
            Some(search.restart(adult))
        }
    };
    if !more {
        restart_pictures(app);
        thumbs(&app.state::<SearchState>()).clear();
    }
    let Some(ask) = ask else {
        return;
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = nexus_search::search(&ask.query).map_err(|err| format!("{err:#}"));
        let wanted = {
            let state = handle.state::<SearchState>();
            let mut guard = lock(&state);
            guard
                .as_mut()
                .and_then(|search| search.found(ask.generation, result))
        };
        changed(&handle);
        if let Some(wanted) = wanted.filter(|wanted| !wanted.is_empty()) {
            spawn_thumbs(&handle, ask.generation, wanted);
        }
    });
}

fn spawn_thumbs(app: &AppHandle, generation: u64, wanted: Vec<(u64, String)>) {
    let stop = {
        let state = app.state::<SearchState>();
        Arc::clone(&state.stop.lock().unwrap_or_else(PoisonError::into_inner))
    };
    let queue = Arc::new(Mutex::new(wanted.into_iter()));
    for _ in 0..WORKERS {
        let queue = Arc::clone(&queue);
        let stop = Arc::clone(&stop);
        let handle = app.clone();
        std::thread::spawn(move || {
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let next = {
                    let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
                    queue.next()
                };
                let Some((mod_id, url)) = next else {
                    return;
                };
                if !wants(&handle, generation) {
                    continue;
                }
                let Some(path) = thumb_file(mod_id, &url) else {
                    continue;
                };
                if !wants(&handle, generation) {
                    continue;
                }
                {
                    let state = handle.state::<SearchState>();
                    thumbs(&state).insert(mod_id, path.clone());
                }
                let _ = handle.emit("search_thumb", SearchThumb { mod_id, path });
            }
        });
    }
}

/// A picture arrived: its file, for the page to fill the row in.
#[derive(Clone, Debug, Serialize)]
pub struct SearchThumb {
    mod_id: u64,
    path: String,
}

fn wants(app: &AppHandle, generation: u64) -> bool {
    let state = app.state::<SearchState>();
    let guard = lock(&state);
    guard
        .as_ref()
        .is_some_and(|search| search.generation == generation)
}

/// Nexus's window on a mod's page, or one file's Mod manager download, and
/// what it gave.
fn site(app: &AppHandle, mod_id: u64, file_id: Option<u64>) {
    let (game, domain) = {
        let state = app.state::<SearchState>();
        let guard = lock(&state);
        let Some(search) = guard.as_ref() else {
            return;
        };
        (search.game.clone(), search.domain.clone())
    };
    let Some(game) = (app.state::<LibraryState>()).game(&game) else {
        return;
    };
    let folder = match zendo_core::mods::downloads_dir(&game) {
        Ok(folder) => folder,
        Err(err) => {
            crate::nexus::say(app, format!("{err:#}"));
            return;
        }
    };
    let url = shell::site_url(&domain, mod_id, file_id);
    {
        let state = app.state::<SearchState>();
        let mut site = state.site.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(open) = site.take() {
            open.close();
        }
        *site = Some(Handle::default());
    }
    let handle = {
        let state = app.state::<SearchState>();
        let site = state.site.lock().unwrap_or_else(PoisonError::into_inner);
        site.clone().unwrap_or_default()
    };
    crate::nexus::say(app, t!("search-site-open"));
    let result = sign_in_window::open_nexus(&url, &folder, &handle);
    match result {
        Ok(None) => {}
        Ok(Some(FromSite::Link(text))) => match Link::parse(&text) {
            Ok(link) => crate::nexus::download(app, &game.id, link, false),
            Err(err) => crate::nexus::say(app, format!("{err:#}")),
        },
        Ok(Some(FromSite::File(archive))) => {
            let key = key_of(&app.state::<SearchState>());
            let job = {
                let mods = app.state::<ModsHandle>();
                let mut mods = mods.lock();
                mods.add_from_site(&game.id, archive, key, domain, mod_id, true)
            };
            let library = app.state::<LibraryState>();
            crate::mods::run(app, &library, job);
        }
        Err(err) => crate::nexus::say(app, format!("{err:#}")),
    }
    changed(app);
}
