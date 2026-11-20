//! Game Mode: the full-screen, controller-first screen of one game large (the
//! hero) over a row of covers (the carousel) taken from a shelf, and the Start
//! menu. The rules (the shelves, the hero, the menu) are
//! [`zendo_shell::game_mode`]'s, kept in `zendo-shell`; this part holds
//! the state while it is on and writes the screen.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::Game;
use zendo_core::browse::Move;
use zendo_shell::game_mode::{self, GameMode, MenuChoice, Shelf};
use zendo_shell::running;

use crate::library::LibraryState;

/// The screen's state while it is on, and the window's width.
pub struct GameModeState {
    mode: Mutex<Option<GameMode>>,
    width: Mutex<f32>,
}

impl Default for GameModeState {
    fn default() -> Self {
        Self {
            mode: Mutex::new(None),
            width: Mutex::new(1920.0),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChipDto {
    label: String,
    count: String,
    chosen: bool,
}

#[derive(Debug, Serialize)]
pub struct TileDto {
    title: String,
    /// The cover in the cache, if the game has one.
    cover: Option<String>,
    current: bool,
}

#[derive(Debug, Serialize)]
pub struct HeroDto {
    id: String,
    title: String,
    /// "STEAM · 12 h · today".
    meta: String,
    description: String,
    cover: Option<String>,
}

/// The screen as it stands.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the screen is drawn by"
)]
#[derive(Debug, Default, Serialize)]
pub struct GameModeDto {
    on: bool,
    browsing: bool,
    empty: bool,
    width: f32,
    shelves: Vec<ChipDto>,
    tiles: Vec<TileDto>,
    hero: Option<HeroDto>,
    menu_open: bool,
    menu: Vec<ChipDto>,
}

fn mode(app: &AppHandle) -> Option<GameMode> {
    let state = app.state::<GameModeState>();
    state
        .mode
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn set(app: &AppHandle, mode: Option<GameMode>) {
    let state = app.state::<GameModeState>();
    *state.mode.lock().unwrap_or_else(PoisonError::into_inner) = mode;
}

fn width(app: &AppHandle) -> f32 {
    let state = app.state::<GameModeState>();
    *state.width.lock().unwrap_or_else(PoisonError::into_inner)
}

fn dto(app: &AppHandle) -> GameModeDto {
    let Some(current) = mode(app) else {
        return GameModeDto {
            width: width(app),
            ..GameModeDto::default()
        };
    };
    let games = app.state::<LibraryState>().games();
    let shown = current.shown_shelf(&games);
    let list = game_mode::shelf_games(&games, shown);
    let index = current.hero_index(&games);
    let width = width(app);
    let count = game_mode::tiles_that_fit(width).min(list.len());
    let first = game_mode::first_tile(index, list.len(), count);
    let tile = |game: &Game| TileDto {
        title: game.title.clone(),
        cover: game.cover.as_ref().map(|path| path.display().to_string()),
        current: false,
    };
    let tiles: Vec<TileDto> = list
        .iter()
        .enumerate()
        .skip(first)
        .take(count)
        .map(|(at, game)| TileDto {
            current: at == index,
            ..tile(game)
        })
        .collect();
    let hero = list.get(index).map(|game| HeroDto {
        id: game.id.clone(),
        title: game.title.clone(),
        meta: game_mode::meta_line(game, running::now()),
        description: game_mode::description(summary_of(app, game).as_deref(), game),
        cover: game.cover.as_ref().map(|path| path.display().to_string()),
    });
    GameModeDto {
        on: true,
        browsing: current.browsing,
        empty: list.is_empty(),
        width,
        shelves: Shelf::ALL
            .iter()
            .map(|shelf| ChipDto {
                label: shelf.label(),
                count: game_mode::shelf_games(&games, *shelf).len().to_string(),
                chosen: *shelf == shown,
            })
            .collect(),
        tiles,
        hero,
        menu_open: current.menu.is_some(),
        menu: MenuChoice::ALL
            .iter()
            .enumerate()
            .map(|(at, choice)| ChipDto {
                label: choice.label(),
                count: String::new(),
                chosen: current.menu == Some(at),
            })
            .collect(),
    }
}

/// The hero's summary: what the Store's list says of the game, when it was
/// asked; the hero says who made it otherwise.
fn summary_of(app: &AppHandle, game: &Game) -> Option<String> {
    let state = app.state::<crate::store::StoreState>();
    let browser = state.browser.lock().unwrap_or_else(PoisonError::into_inner);
    browser
        .owned(&game.id)
        .and_then(|owned| owned.summary.clone())
}

fn set_fullscreen(app: &AppHandle, on: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_fullscreen(on);
    }
}

#[tauri::command]
pub fn game_mode_get(app: AppHandle) -> GameModeDto {
    dto(&app)
}

/// The window's width changed: how many tiles fit.
#[tauri::command]
pub fn game_mode_sized(width: f32, app: AppHandle) -> GameModeDto {
    {
        let state = app.state::<GameModeState>();
        *state.width.lock().unwrap_or_else(PoisonError::into_inner) = width;
    }
    dto(&app)
}

/// F11 and Select: Game Mode on or off, with the window full screen and back.
#[tauri::command]
pub fn game_mode_toggle(app: AppHandle) -> GameModeDto {
    if mode(&app).is_some() {
        leave(&app);
    } else {
        enter(&app);
    }
    let _ = app.emit("game_mode_changed", ());
    dto(&app)
}

/// Zendo started with `--game-mode`: the window is full screen already.
#[tauri::command]
pub fn game_mode_start(app: AppHandle) -> GameModeDto {
    enter(&app);
    let _ = app.emit("game_mode_changed", ());
    dto(&app)
}

fn enter(app: &AppHandle) {
    set(app, Some(GameMode::default()));
    set_fullscreen(app, true);
}

fn leave(app: &AppHandle) {
    set(app, None);
    set_fullscreen(app, false);
}

/// A change of the screen's state, drawn again.
fn changed(app: &AppHandle, change: impl FnOnce(&mut GameMode, &[Game])) -> GameModeDto {
    {
        let games = app.state::<LibraryState>().games();
        let state = app.state::<GameModeState>();
        let mut slot = state.mode.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(current) = slot.as_mut() {
            change(current, &games);
        }
    }
    dto(app)
}

#[tauri::command]
pub fn game_mode_shelf(at: usize, app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| {
        if let Some(shelf) = Shelf::ALL.get(at) {
            mode.set_shelf(*shelf);
        }
    })
}

/// A tile pressed: the hero becomes that game.
#[tauri::command]
pub fn game_mode_pick(at: usize, app: AppHandle) -> GameModeDto {
    let first = {
        let games = app.state::<LibraryState>().games();
        match mode(&app) {
            Some(current) => first_visible(&app, &current, &games),
            None => 0,
        }
    };
    changed(&app, |mode, _| mode.index = first.saturating_add(at))
}

/// Left and Right on the carousel: one tile along.
#[tauri::command]
pub fn game_mode_step(by: isize, app: AppHandle) -> GameModeDto {
    changed(&app, |mode, games| mode.step(by, games))
}

/// Tab and the pad's shoulders: the next shelf with games.
#[tauri::command]
pub fn game_mode_cycle_shelf(by: isize, app: AppHandle) -> GameModeDto {
    changed(&app, |mode, games| mode.cycle_shelf(by, games))
}

/// The place of the first tile shown, among the shelf's games.
fn first_visible(app: &AppHandle, mode: &GameMode, games: &[Game]) -> usize {
    let len = game_mode::shelf_games(games, mode.shown_shelf(games)).len();
    let count = game_mode::tiles_that_fit(width(app)).min(len);
    game_mode::first_tile(mode.hero_index(games), len, count)
}

#[tauri::command]
pub fn game_mode_open_menu(app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| mode.open_menu())
}

#[tauri::command]
pub fn game_mode_close_menu(app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| mode.menu = None)
}

/// Escape on the screen: the Start menu opens, or closes.
#[tauri::command]
pub fn game_mode_back(app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| {
        mode.menu = if mode.menu.is_some() { None } else { Some(0) };
    })
}

#[tauri::command]
pub fn game_mode_menu_step(step: String, app: AppHandle) -> GameModeDto {
    let Some(step) = move_of(&step) else {
        return dto(&app);
    };
    changed(&app, |mode, _| mode.move_menu(step))
}

/// A choice of the Start menu; `leave` ends Game Mode.
#[derive(Debug, Serialize)]
pub struct ChoiceDto {
    page: GameModeDto,
    leave: bool,
}

#[tauri::command]
pub fn game_mode_choose(at: usize, app: AppHandle) -> ChoiceDto {
    let Some(choice) = MenuChoice::ALL.get(at).copied() else {
        return ChoiceDto {
            page: dto(&app),
            leave: false,
        };
    };
    let mut leave = false;
    changed(&app, |mode, _| leave = mode.choose(choice));
    if leave {
        leave_mode(&app);
    }
    let _ = app.emit("game_mode_changed", ());
    ChoiceDto {
        page: dto(&app),
        leave,
    }
}

fn leave_mode(app: &AppHandle) {
    leave(app);
}

/// Plays the game in the hero.
#[tauri::command]
pub fn game_mode_play(app: AppHandle) -> GameModeDto {
    let id = {
        let Some(current) = mode(&app) else {
            return dto(&app);
        };
        let games = app.state::<LibraryState>().games();
        current.hero(&games).map(|game| game.id.clone())
    };
    let Some(id) = id else {
        return dto(&app);
    };
    let _ = crate::game::start(&app, &id);
    // The launch moves the game to the front of Continue playing: the hero
    // follows it instead of staying on the same place.
    changed(&app, |mode, games| {
        let shown = mode.shown_shelf(games);
        if let Some(at) = game_mode::shelf_games(games, shown)
            .iter()
            .position(|game| game.id == id)
        {
            mode.index = at;
        }
    })
}

/// The id of the game in the hero, for its own page.
#[tauri::command]
pub fn game_mode_details(app: AppHandle) -> Option<String> {
    let current = mode(&app)?;
    let games = app.state::<LibraryState>().games();
    current.hero(&games).map(|game| game.id.clone())
}

/// The Library's own grid again (Library in the Start menu).
#[tauri::command]
pub fn game_mode_browse(app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| mode.browsing = true)
}

/// The screen again, after the Library's grid.
#[tauri::command]
pub fn game_mode_show(app: AppHandle) -> GameModeDto {
    changed(&app, |mode, _| mode.browsing = false)
}

fn move_of(step: &str) -> Option<Move> {
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
