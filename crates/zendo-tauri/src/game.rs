//! The game page's commands: one game with what its header and tiles show,
//! its launch and stop, and the three marks it can change. The page's other
//! cards follow in the next parts of the slice.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::launch;
use zendo_core::{Game, Source, State as CoreState, Status};
use zendo_shell::game::{details, sessions_of, tiles_of};
use zendo_shell::running::{self, Effect, Names, RunMessage};

use crate::cloud::{self, CloudState};
use crate::jobs::{self, DlcCardDto, JobLineDto, ManageDto, UpdateDto};
use crate::library::LibraryState;
use crate::platform;
use crate::prefix::{PrefixDto, PrefixState, prefix_of};

/// A game's page: the header, the tiles, the statuses and how it is running.
#[derive(Debug, Serialize)]
pub struct GameDto {
    id: String,
    title: String,
    source_line: String,
    cover: Option<String>,
    hero: Option<String>,
    pinned: bool,
    hidden: bool,
    status: Option<String>,
    /// 0 idle (Play), 1 starting, 2 running (Stop), 3 stopping.
    run_state: i32,
    line: String,
    tiles: Vec<TileDto>,
    statuses: Vec<StatusDto>,
    /// A game added by hand: its page edits and removes it.
    local: bool,
    details: Vec<FactDto>,
    /// The session counts and lengths, empty until the game has one.
    sessions: Vec<FactDto>,
    /// The latest sessions, by date.
    recent: Vec<FactDto>,
    /// The DLC card: the store's list of what the player owns, and the
    /// buttons.
    dlc: DlcCardDto,
    /// The Wine prefix card's data, for a Windows game Zendo starts.
    prefix: Option<PrefixDto>,
    /// Update in the action bar.
    update: UpdateDto,
    /// The More row and the Steam buttons.
    manage: ManageDto,
    /// What the queue is doing with the game.
    job: JobLineDto,
}

#[derive(Debug, Serialize)]
struct FactDto {
    label: String,
    value: String,
    path: bool,
    /// The value's colour: 0 plain, 1 good, 2 bad, 3 dim.
    tone: i32,
}

#[derive(Debug, Serialize)]
struct TileDto {
    label: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct StatusDto {
    key: String,
    label: String,
    chosen: bool,
}

/// A running game's new state, for the page.
#[derive(Debug, Clone, Serialize)]
pub struct RunState {
    id: String,
    state: i32,
}

/// A line the page shows under the actions.
#[derive(Debug, Clone, Serialize)]
pub struct GameLine {
    id: String,
    line: String,
}

#[tauri::command]
pub fn game_get(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    Ok(dto(
        &state,
        &game,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

/// Starts the game on a thread of its own, then follows it: the page hears
/// each change of state and line.
#[tauri::command]
pub fn game_launch(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    {
        let mut running = state.running();
        if running.is_open(&id) {
            return Ok(dto(
                &state,
                &game,
                &app.state::<PrefixState>(),
                &app.state::<CloudState>(),
                &app,
            ));
        }
        running.begin(&id);
    }
    let handle = app.clone();
    running::follow(game.clone(), move |message| {
        handle_run(&handle, message);
        true
    });
    emit_run(&app, &id);
    Ok(dto(
        &state,
        &game,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

/// Asks the game to close; a second press kills it, as the Slint window does.
#[tauri::command]
pub fn game_stop(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    handle_run(&app, RunMessage::Stop(id.clone()));
    Ok(dto(
        &state,
        &game,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

#[tauri::command]
pub fn game_set_pinned(
    id: String,
    pinned: bool,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    if game.pinned != pinned {
        state.change(|browser| browser.toggle_pinned_of(&id));
    }
    Ok(dto(
        &state,
        &fresh(&state, &id)?,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

#[tauri::command]
pub fn game_set_hidden(
    id: String,
    hidden: bool,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    if game.hidden != hidden {
        state.change(|browser| browser.toggle_hidden_of(&id));
    }
    Ok(dto(
        &state,
        &fresh(&state, &id)?,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

/// Sets the play status; `none` clears it.
#[tauri::command]
pub fn game_set_status(
    id: String,
    status: Option<String>,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<GameDto, String> {
    let game = state
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    let wanted = match status.as_deref() {
        None | Some("none") => None,
        Some(word) => Some(Status::parse(word).ok_or_else(|| format!("{word:?} is not a status"))?),
    };
    if game.status != wanted {
        state.change(|browser| browser.set_status_of(&id, wanted));
    }
    Ok(dto(
        &state,
        &fresh(&state, &id)?,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        &app,
    ))
}

/// A game as it is now: the page asks again after a change, so its chips and
/// badges show what was written, not what was there before.
fn fresh(state: &LibraryState, id: &str) -> Result<Game, String> {
    state
        .game(id)
        .ok_or_else(|| format!("no game with the id {id:?}"))
}

/// What the page shows of a game now.
fn dto(
    state: &LibraryState,
    game: &Game,
    prefix: &PrefixState,
    cloud: &CloudState,
    app: &AppHandle,
) -> GameDto {
    let (run_state, running) = {
        let running = state.running();
        let state_number = match running.games.get(&game.id) {
            None => 0,
            Some(session) if !session.seen => 1,
            Some(session) if session.stopping => 3,
            Some(_) => 2,
        };
        (state_number, running.is_running(&game.id))
    };
    let (session_summary, session_recent) = sessions_of(&all_sessions(), &game.id);
    GameDto {
        id: game.id.clone(),
        title: game.title.clone(),
        source_line: zendo_shell::game::source_line(game),
        cover: game.cover.as_ref().map(|path| path.display().to_string()),
        hero: game.hero.as_ref().map(|path| path.display().to_string()),
        pinned: game.pinned,
        hidden: game.hidden,
        status: game.status.map(|status| status.name().to_owned()),
        run_state,
        line: zendo_shell::i18n::tr("game-ready", None),
        tiles: tiles_of(game, running::now())
            .into_iter()
            .map(|tile| TileDto {
                label: tile.label,
                value: tile.value,
            })
            .collect(),
        statuses: Status::ALL
            .iter()
            .map(|status| StatusDto {
                key: status.name().to_owned(),
                label: zendo_shell::format::play_status(*status),
                chosen: game.status == Some(*status),
            })
            .collect(),
        local: game.source == Source::Local,
        details: {
            let mut rows = facts(details(game));
            // What the last look for an update found.
            if game.store_managed() {
                let downloads = app.state::<crate::downloads::DownloadsState>();
                let (line, tone) = {
                    let queue = crate::downloads::lock(&downloads);
                    zendo_shell::queue::update_line(queue.updates.get(&game.id))
                };
                if !line.is_empty() {
                    rows.push(FactDto {
                        label: zendo_shell::t!("fact-updates"),
                        value: line,
                        path: false,
                        tone: match tone {
                            zendo_shell::queue::Tone::Good => 1,
                            zendo_shell::queue::Tone::Bad => 2,
                            zendo_shell::queue::Tone::Dim => 3,
                            zendo_shell::queue::Tone::Accent => 4,
                        },
                    });
                }
            }
            // The cloud saves' row closes the details, as in the Slint page.
            if let Some((value, tone)) = cloud::row(game, cloud) {
                rows.push(FactDto {
                    label: zendo_shell::t!("fact-cloud-saves"),
                    value,
                    path: false,
                    tone,
                });
            }
            rows
        },
        sessions: facts(session_summary),
        recent: facts(session_recent),
        dlc: jobs::dlc_card(app, game),
        prefix: prefix_of(game, running, prefix),
        update: jobs::update(app, game),
        manage: jobs::manage(app, game),
        job: jobs::job_line(app, game),
    }
}

/// A game's page as it is now, for a command that changed something.
pub(crate) fn page(app: &AppHandle, id: &str) -> Result<GameDto, String> {
    let state = app.state::<LibraryState>();
    let game = state
        .game(id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    Ok(dto(
        &state,
        &game,
        &app.state::<PrefixState>(),
        &app.state::<CloudState>(),
        app,
    ))
}

/// The facts as the page takes them.
fn facts(rows: Vec<zendo_shell::game::Fact>) -> Vec<FactDto> {
    rows.into_iter()
        .map(|row| FactDto {
            label: row.label,
            value: row.value,
            path: row.path,
            tone: 0,
        })
        .collect()
}

fn all_sessions() -> Vec<zendo_core::sessions::Session> {
    zendo_core::sessions::load().unwrap_or_default()
}

/// Applies what the game did and tells the page.
fn handle_run(app: &AppHandle, message: RunMessage) {
    let id = message_id(&message).to_owned();
    let effects = {
        let state = app.state::<LibraryState>();
        let mut running = state.running();
        let names = |id: &str| Names::of(id, state.game(id).as_ref());
        running.apply(message, &names)
    };
    let state = app.state::<LibraryState>();
    for effect in effects {
        match effect {
            Effect::Say(line) => {
                let _ = app.emit(
                    "game_line",
                    GameLine {
                        id: id.clone(),
                        line,
                    },
                );
            }
            Effect::Launched(changed) => {
                let _ = CoreState::update(|state| state.launched(&changed));
                state.change(|browser| browser.launched(&changed, running::now()));
                let _ = app.emit("library_changed", ());
            }
            Effect::Played(changed, minutes) => {
                let _ = CoreState::update(|state| state.add_minutes(&changed, minutes));
                state.change(|browser| browser.add_minutes(&changed, minutes));
                let _ = app.emit("library_changed", ());
            }
            Effect::Session {
                id: played,
                since,
                minutes,
            } => {
                let title = state
                    .game(&played)
                    .map_or_else(|| played.clone(), |game| game.title);
                let session = zendo_core::sessions::Session {
                    id: played,
                    title,
                    start: since,
                    minutes,
                    offset: zendo_core::sessions::local_offset(since),
                };
                let _ = zendo_core::sessions::record(&session);
            }
            Effect::CloudReload => emit_game_changed(app, &id),
            Effect::CloudSyncing(changed, syncing) => {
                app.state::<CloudState>().set_syncing(&changed, syncing);
                emit_game_changed(app, &changed);
            }
            Effect::Changed(changed) => emit_game_changed(app, &changed),
            Effect::HideOnLaunch => platform::hide_for_launch(app),
            Effect::ShowOnClose => platform::show_after_game(app),
            Effect::Presence => crate::presence::PresenceState::show(app),
            Effect::StopAsking { watch, kill, .. } => {
                std::thread::spawn(move || {
                    if kill {
                        launch::kill(&watch);
                    } else {
                        let _ = launch::stop(&watch);
                    }
                });
            }
            _ => {}
        }
    }
    emit_run(app, &id);
}

/// The game a message is about.
fn message_id(message: &RunMessage) -> &str {
    match message {
        RunMessage::Fetching(id, _)
        | RunMessage::FetchFailed(id, _)
        | RunMessage::SettingUp(id, _)
        | RunMessage::SetupFailed(id, _)
        | RunMessage::FixesFailed(id, _)
        | RunMessage::GalaxyFailed(id, _)
        | RunMessage::Started(id, _)
        | RunMessage::Seen(id)
        | RunMessage::Shown(id)
        | RunMessage::Missed(id)
        | RunMessage::Ended(id)
        | RunMessage::SavesSyncing(id)
        | RunMessage::AfterDone(id, _)
        | RunMessage::Stop(id)
        | RunMessage::StopAnswered(id, _) => id,
    }
}

/// Something about a game's data changed (its saves' state, a mark, a prefix
/// tool): the page asks for its game again.
pub(crate) fn emit_game_changed(app: &AppHandle, id: &str) {
    let _ = app.emit("game_changed", id.to_owned());
}

fn emit_run(app: &AppHandle, id: &str) {
    let state = app.state::<LibraryState>();
    let state_number = {
        let running = state.running();
        match running.games.get(id) {
            None => 0,
            Some(session) if !session.seen => 1,
            Some(session) if session.stopping => 3,
            Some(_) => 2,
        }
    };
    let _ = app.emit(
        "game_run",
        RunState {
            id: id.to_owned(),
            state: state_number,
        },
    );
}
