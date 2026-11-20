//! The art picker over a game's page: "Change cover" and "Change backdrop"
//! open a sheet that searches `SteamGridDB` for the game's title and lists the
//! covers, or the wide images, of the game chosen; a click makes one the
//! game's own, before Steam's and the stores'.
//!
//! The picker and its rules are `zendo_shell::artwork`'s, shared with the
//! Slint window. This part keeps it while it is open, runs its effects on
//! threads and keeps the thumbnails as files for the page to draw.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::covers::{self, Kind};
use zendo_core::steamgriddb::{self, Found, Image};
use zendo_shell::artwork::{self, Art, Effect, GAMES_SHOWN, WORKERS};
use zendo_shell::t;

use crate::library::{CoverChanged, LibraryState};
use crate::store::StoreState;

/// The picker, the thumbnails kept as files, and the flag that stops them.
pub struct ArtState {
    art: Mutex<Art>,
    /// The files of the thumbnails downloaded so far, by the image's id.
    thumbs: Mutex<HashMap<u64, String>>,
    /// Set to stop the thumbnails of the search or list on its way.
    stop: Mutex<Arc<AtomicBool>>,
}

impl Default for ArtState {
    fn default() -> Self {
        Self {
            art: Mutex::new(Art::new(true)),
            thumbs: Mutex::new(HashMap::new()),
            stop: Mutex::new(Arc::new(AtomicBool::new(false))),
        }
    }
}

fn lock(state: &ArtState) -> MutexGuard<'_, Art> {
    state.art.lock().unwrap_or_else(PoisonError::into_inner)
}

fn thumbs(state: &ArtState) -> MutexGuard<'_, HashMap<u64, String>> {
    state.thumbs.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A chip: the tab or game, its place and whether it is the one.
#[derive(Debug, Serialize)]
pub struct ChipDto {
    id: i32,
    label: String,
    chosen: bool,
}

/// An image to choose: its file once downloaded, and its size, style and
/// author.
#[derive(Debug, Serialize)]
pub struct ThumbDto {
    image: u64,
    path: Option<String>,
    about: String,
}

/// The sheet as the page draws it.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the sheet is drawn by"
)]
#[derive(Debug, Default, Serialize)]
pub struct ArtSheetDto {
    open: bool,
    title: String,
    /// "{title} · Change cover"-like heading, from the sheet's words.
    heading: String,
    /// 0 covers, 1 wide images.
    kind: i32,
    query: String,
    tabs: Vec<ChipDto>,
    games: Vec<ChipDto>,
    thumbs: Vec<ThumbDto>,
    /// Another page of images can be asked for.
    more: bool,
    /// Something is on its way.
    busy: bool,
    /// What it is: "Searching…", "Looking for images…", "Saving…".
    status: String,
    note: String,
    /// The note is the one that asks for a key: Settings is offered.
    needs_key: bool,
    automatic_label: String,
}

/// A thumbnail arrived: its file, for the page to fill the row in.
#[derive(Clone, Debug, Serialize)]
pub struct ArtThumb {
    image: u64,
    path: String,
}

fn dto(app: &AppHandle) -> ArtSheetDto {
    let state = app.state::<ArtState>();
    let art = lock(&state);
    let Some(picker) = art.picker.as_ref() else {
        return ArtSheetDto::default();
    };
    let kept = thumbs(&state);
    ArtSheetDto {
        open: true,
        title: picker.title.clone(),
        heading: t!("art-title", title = picker.title.as_str()),
        kind: i32::from(picker.kind == Kind::Hero),
        query: picker.query.clone(),
        tabs: vec![
            ChipDto {
                id: 0,
                label: t!("art-cover"),
                chosen: picker.kind == Kind::Cover,
            },
            ChipDto {
                id: 1,
                label: t!("art-hero"),
                chosen: picker.kind == Kind::Hero,
            },
        ],
        games: picker
            .games
            .iter()
            .take(GAMES_SHOWN)
            .enumerate()
            .map(|(at, game)| ChipDto {
                id: i32::try_from(at).unwrap_or(i32::MAX),
                label: artwork::game_label(game),
                chosen: picker.game == Some(game.id),
            })
            .collect(),
        thumbs: picker
            .images
            .iter()
            .map(|found| ThumbDto {
                image: found.id,
                path: kept.get(&found.id).cloned(),
                about: artwork::thumb_about(found),
            })
            .collect(),
        more: picker.more,
        busy: picker.busy.is_some(),
        status: picker.busy.clone().unwrap_or_default(),
        note: picker.note.clone().unwrap_or_default(),
        needs_key: picker.needs_key,
        automatic_label: if picker.chosen {
            Art::automatic_label(picker.kind)
        } else {
            String::new()
        },
    }
}

/// The sheet was written to, or an answer came in: the page asks again.
fn changed(app: &AppHandle) {
    let _ = app.emit("art_changed", ());
}

/// A new search or list: the thumbnails still coming stop.
fn restart(app: &AppHandle) {
    let state = app.state::<ArtState>();
    let mut stop = state.stop.lock().unwrap_or_else(PoisonError::into_inner);
    let old = std::mem::replace(&mut *stop, Arc::new(AtomicBool::new(false)));
    old.store(true, Ordering::Relaxed);
}

fn stop(app: &AppHandle) {
    let state = app.state::<ArtState>();
    state
        .stop
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .store(true, Ordering::Relaxed);
}

fn wants(app: &AppHandle, generation: u64) -> bool {
    let state = app.state::<ArtState>();
    let art = lock(&state);
    art.wants(generation)
}

#[tauri::command]
pub fn art_get(app: AppHandle) -> ArtSheetDto {
    dto(&app)
}

/// Change cover (0) or Change backdrop (1). The key may be kept in the
/// keyring, which must not be asked on the window's thread, so the form opens
/// from a thread of its own and says `art_changed`.
#[tauri::command]
pub fn art_open(id: String, kind: i32, app: AppHandle) -> ArtSheetDto {
    let game = {
        let library = app.state::<LibraryState>();
        library.game(&id)
    };
    let Some(game) = game else {
        return dto(&app);
    };
    let kind = if kind == 0 { Kind::Cover } else { Kind::Hero };
    let handle = app.clone();
    std::thread::spawn(move || {
        let key = zendo_core::config::Config::load()
            .ok()
            .and_then(|config| config.steamgriddb_key.clone());
        {
            let state = handle.state::<ArtState>();
            // The pictures of the picker before may still be on their way.
            restart(&handle);
            thumbs(&state).clear();
            let mut art = lock(&state);
            let effect = art.open(&game.id, &game.title, kind, key);
            drop(art);
            changed(&handle);
            run(&handle, effect);
        }
    });
    dto(&app)
}

#[tauri::command]
pub fn art_query(text: String, app: AppHandle) -> ArtSheetDto {
    {
        let state = app.state::<ArtState>();
        lock(&state).set_query(text);
    }
    dto(&app)
}

/// Enter and the Search button: the title typed is searched again.
#[tauri::command]
pub fn art_search(app: AppHandle) -> ArtSheetDto {
    let effect = {
        let state = app.state::<ArtState>();
        lock(&state).search_typed()
    };
    run(&app, effect);
    dto(&app)
}

/// The cover or wide image tab.
#[tauri::command]
pub fn art_kind(kind: i32, app: AppHandle) -> ArtSheetDto {
    let kind = if kind == 0 { Kind::Cover } else { Kind::Hero };
    let effect = {
        let state = app.state::<ArtState>();
        lock(&state).set_kind(kind)
    };
    run(&app, effect);
    dto(&app)
}

/// A game of those found, by its place among the first ones shown.
#[tauri::command]
pub fn art_game(at: i32, app: AppHandle) -> ArtSheetDto {
    let found = {
        let state = app.state::<ArtState>();
        let art = lock(&state);
        usize::try_from(at)
            .ok()
            .and_then(|at| art.picker.as_ref()?.games.get(at))
            .map(|game| game.id)
    };
    let Some(game) = found else {
        return dto(&app);
    };
    let effect = {
        let state = app.state::<ArtState>();
        lock(&state).pick_game(game)
    };
    run(&app, effect);
    dto(&app)
}

/// An image pressed: it is downloaded and kept as the game's own.
#[tauri::command]
pub fn art_choose(image: u64, app: AppHandle) -> ArtSheetDto {
    let effect = {
        let state = app.state::<ArtState>();
        lock(&state).choose(image)
    };
    run(&app, effect);
    dto(&app)
}

#[tauri::command]
pub fn art_more(app: AppHandle) -> ArtSheetDto {
    let effect = {
        let state = app.state::<ArtState>();
        lock(&state).more()
    };
    run(&app, effect);
    dto(&app)
}

/// "Use the automatic one": the image chosen by hand is forgotten, the
/// stores' copy takes back the one found by itself, and a scan puts the
/// library's back.
#[tauri::command]
pub fn art_automatic(app: AppHandle) -> ArtSheetDto {
    let done = {
        let state = app.state::<ArtState>();
        lock(&state).automatic()
    };
    let Some(done) = done else {
        changed(&app);
        return dto(&app);
    };
    stop(&app);
    {
        let image = covers::cached(&done.id, done.kind);
        let store = app.state::<StoreState>();
        let mut browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
        browser.put_image(&done.id, done.kind, image.as_deref());
    }
    let _ = app.emit("store_changed", ());
    // The game's own copy is read again from what is on disk now that the
    // chosen file is gone; the page hears `library_loaded`.
    crate::library::rescan(app.clone());
    changed(&app);
    dto(&app)
}

/// Escape, Close and a press outside.
#[tauri::command]
pub fn art_close(app: AppHandle) -> ArtSheetDto {
    stop(&app);
    {
        let state = app.state::<ArtState>();
        lock(&state).close();
        thumbs(&state).clear();
    }
    changed(&app);
    dto(&app)
}

/// Runs what the picker asked for, on threads that say `art_changed` and
/// start what follows.
fn run(app: &AppHandle, effect: Option<Effect>) {
    let Some(effect) = effect else {
        return;
    };
    match effect {
        Effect::Search {
            generation,
            key,
            query,
            steam,
        } => spawn_search(app, generation, key, query, steam),
        Effect::List {
            generation,
            key,
            game,
            kind,
            page,
        } => spawn_list(app, generation, key, game, kind, page),
        Effect::Thumbs { generation, wanted } => spawn_thumbs(app, generation, wanted),
        Effect::Choose { id, kind, image } => spawn_choose(app, id, kind, image),
    }
}

fn spawn_search(
    app: &AppHandle,
    generation: u64,
    key: String,
    query: String,
    steam: Option<String>,
) {
    restart(app);
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = search(&key, &query, steam.as_deref());
        let next = {
            let state = handle.state::<ArtState>();
            lock(&state).found(generation, result)
        };
        changed(&handle);
        run(&handle, next);
    });
}

/// The title searched, and a Steam game's own entry first.
fn search(key: &str, query: &str, steam: Option<&str>) -> Result<Vec<Found>, String> {
    let found = (|| -> anyhow::Result<Vec<Found>> {
        let mut found = Vec::new();
        if let Some(app) = steam
            && let Some(game) = steamgriddb::steam_game(key, app)?
        {
            found.push(game);
        }
        for game in steamgriddb::search(key, query)? {
            if !found.iter().any(|kept: &Found| kept.id == game.id) {
                found.push(game);
            }
        }
        Ok(found)
    })();
    found.map_err(|err| format!("{err:#}"))
}

fn spawn_list(app: &AppHandle, generation: u64, key: String, game: u64, kind: Kind, page: u32) {
    if page == 0 {
        restart(app);
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = steamgriddb::images(&key, game, kind, page).map_err(|err| format!("{err:#}"));
        let next = {
            let state = handle.state::<ArtState>();
            lock(&state).listed(generation, result)
        };
        changed(&handle);
        run(&handle, next);
    });
}

fn spawn_thumbs(app: &AppHandle, generation: u64, wanted: Vec<(u64, String)>) {
    let stop = {
        let state = app.state::<ArtState>();
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
                let Some((image, url)) = next else {
                    return;
                };
                // A picker closed or another image list: no file is written.
                if !wants(&handle, generation) {
                    continue;
                }
                let Some(path) = thumb_file(image, &url) else {
                    continue;
                };
                if !wants(&handle, generation) {
                    continue;
                }
                {
                    let state = handle.state::<ArtState>();
                    thumbs(&state).insert(image, path.clone());
                }
                let _ = handle.emit("art_thumb", ArtThumb { image, path });
            }
        });
    }
}

/// A thumbnail's file, downloaded once and kept in the cache.
fn thumb_file(image: u64, url: &str) -> Option<String> {
    let dir = artwork::thumb_dir()?;
    for ext in ["jpg", "png", "webp", "gif"] {
        let kept = dir.join(format!("{image}.{ext}"));
        if kept.is_file() {
            return Some(kept.display().to_string());
        }
    }
    let bytes = steamgriddb::thumbnail(url).ok()?;
    let ext = covers::image_extension(&bytes)?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{image}.{ext}"));
    std::fs::write(&path, &bytes).ok()?;
    Some(path.display().to_string())
}

fn spawn_choose(app: &AppHandle, id: String, kind: Kind, image: Image) {
    let handle = app.clone();
    std::thread::spawn(move || {
        match covers::choose(&id, kind, &image).map_err(|err| format!("{err:#}")) {
            Ok(path) => {
                let wanted = id.clone();
                let saved = path.clone();
                handle
                    .state::<LibraryState>()
                    .change(move |browser| browser.set_image(&wanted, kind, saved));
                {
                    let store = handle.state::<StoreState>();
                    let mut browser = store.browser.lock().unwrap_or_else(PoisonError::into_inner);
                    browser.set_image(&id, kind, &path);
                }
                let _ = handle.emit(
                    "cover_changed",
                    CoverChanged {
                        id: id.clone(),
                        path: path.display().to_string(),
                    },
                );
                let _ = handle.emit("store_changed", ());
                let taken = {
                    let state = handle.state::<ArtState>();
                    lock(&state).take_for(&id).is_some()
                };
                if taken {
                    stop(&handle);
                }
                crate::game::emit_game_changed(&handle, &id);
                changed(&handle);
            }
            Err(err) => {
                {
                    let state = handle.state::<ArtState>();
                    lock(&state).choose_failed(&id, err);
                }
                changed(&handle);
            }
        }
    });
}
