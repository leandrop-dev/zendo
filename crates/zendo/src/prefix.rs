//! The Wine prefix card: the folder a Windows game Zendo starts runs in, the
//! tools that act on it (Wine's settings, winetricks, a program run in the
//! prefix, GOG's setup, the known fixes), the components box and the
//! community list's fixes. One tool runs per game at a time, and none while
//! the game does; each runs on a thread through `zendo_core::prefix_tools`.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use zendo_core::config::Config;
use zendo_core::launch_options::LaunchOptions;
use zendo_core::prefix_tools::{self, Ran, Tool};
use zendo_core::{Game, gog_setup, known_fixes, launch};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::prefix as words;
use zendo_shell::t;

use crate::library::LibraryState;

/// What is known of the prefix tools, by game.
#[derive(Debug, Default)]
pub struct PrefixState {
    /// What runs in each game's prefix now.
    busy: Mutex<HashMap<String, Tool>>,
    /// The components typed to install, kept in the game's launch options so
    /// the box holds them the next time Zendo starts.
    components: Mutex<HashMap<String, String>>,
    /// How each game's last tool went: the words, and whether it went well.
    notes: Mutex<HashMap<String, (String, bool)>>,
}

impl PrefixState {
    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn busy(&self, id: &str) -> Option<Tool> {
        Self::lock(&self.busy).get(id).cloned()
    }

    fn is_busy(&self, id: &str) -> bool {
        Self::lock(&self.busy).contains_key(id)
    }

    /// The components typed for a game, read from its launch options the
    /// first time its page opens.
    fn components(&self, id: &str) -> String {
        let mut all = Self::lock(&self.components);
        all.entry(id.to_owned())
            .or_insert_with(|| {
                LaunchOptions::load(id)
                    .ok()
                    .and_then(|options| options.winetricks_components)
                    .unwrap_or_default()
            })
            .clone()
    }

    fn set_components(&self, id: &str, typed: String) {
        Self::lock(&self.components).insert(id.to_owned(), typed);
    }

    fn clear_components(&self, id: &str) {
        if let Some(text) = Self::lock(&self.components).get_mut(id) {
            text.clear();
        }
    }

    fn note(&self, id: &str) -> Option<(String, bool)> {
        Self::lock(&self.notes).get(id).cloned()
    }

    fn put_note(&self, id: String, note: (String, bool)) {
        Self::lock(&self.notes).insert(id, note);
    }

    fn forget_note(&self, id: &str) {
        Self::lock(&self.notes).remove(id);
    }
}

/// A line the card shows under the tools, and what it means: 1 went well,
/// 2 went wrong, 3 is running.
#[derive(Debug, Serialize)]
pub struct StatusDto {
    text: String,
    tone: i32,
}

/// What the page's prefix card shows, when it has one.
#[derive(Debug, Serialize)]
pub struct PrefixDto {
    /// The prefix's folder, when it is there yet.
    pub folder: Option<String>,
    /// The fixes' lines, verb or path with its note.
    pub fixes: Vec<String>,
    /// GOG's setup can run in the prefix.
    pub gog: bool,
    /// No tool runs in the prefix and the game does not.
    pub free: bool,
    /// The components typed last, for the box.
    pub components: String,
    /// What runs, or how the last tool went.
    pub status: Option<StatusDto>,
}

/// The card's data for a game, or `None` when Zendo does not start it in a
/// prefix.
pub(crate) fn prefix_of(game: &Game, running: bool, prefix: &PrefixState) -> Option<PrefixDto> {
    if !prefix_tools::applies(game) {
        return None;
    }
    let config = Config::load_file().unwrap_or_default();
    let folder = config
        .install_dir()
        .map(|dir| launch::prefix_in(&dir, game))
        .filter(|prefix| prefix.is_dir())
        .map(|prefix| prefix.to_string_lossy().into_owned());
    let fixes = known_fixes::for_game(game).map_or_else(Vec::new, |fixes| {
        fixes
            .winetricks
            .iter()
            .chain(fixes.run_in_prefix.iter())
            .map(|fix| match fixes.notes.get(fix) {
                Some(note) => format!("{fix}: {note}"),
                None => fix.clone(),
            })
            .collect()
    });
    let busy = prefix.busy(&game.id);
    Some(PrefixDto {
        folder,
        fixes,
        gog: gog_setup::applies(game),
        free: busy.is_none() && !running,
        components: prefix.components(&game.id),
        status: status_of(busy.as_ref(), running, prefix.note(&game.id)),
    })
}

/// What the card says about the tools: one running, the game holding them,
/// or how the last one went.
fn status_of(
    busy: Option<&Tool>,
    running: bool,
    note: Option<(String, bool)>,
) -> Option<StatusDto> {
    if let Some(tool) = busy {
        return Some(StatusDto {
            text: t!("prefix-running", what = words::label(tool)),
            tone: 3,
        });
    }
    if running {
        return Some(StatusDto {
            text: t!("prefix-game-running"),
            tone: 3,
        });
    }
    note.map(|(text, good)| StatusDto {
        text,
        tone: if good { 1 } else { 2 },
    })
}

/// Runs a tool in the game's prefix: `wine-settings`, `winetricks`,
/// `gog-setup` or `known-fixes`.
#[tauri::command]
pub fn prefix_run(
    id: String,
    tool: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<(), String> {
    let tool = match tool.as_str() {
        "wine-settings" => Tool::WineSettings,
        "winetricks" => Tool::Winetricks,
        "gog-setup" => Tool::GogSetup,
        "known-fixes" => Tool::KnownFixes,
        other => return Err(format!("{other:?} is not a prefix tool")),
    };
    start(&app, &state, id, tool)
}

/// Installs the winetricks components typed, after checking them, and keeps
/// what was typed in the game's launch options.
#[tauri::command]
pub fn prefix_install(
    id: String,
    typed: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<(), String> {
    let verbs = prefix_tools::components(&typed).map_err(|err| format!("{err:#}"))?;
    save_components(&id, &typed);
    app.state::<PrefixState>()
        .set_components(&id, typed.trim().to_owned());
    start(&app, &state, id, Tool::Components(verbs))
}

/// The Run a program… button: the file dialog runs as a process of this
/// binary, on a blocker thread, then the program runs in the prefix.
#[tauri::command]
pub async fn prefix_pick_program(id: String, app: AppHandle) -> Result<(), String> {
    let picked = tauri::async_runtime::spawn_blocking(|| {
        file_dialog::pick(Kind::Installer, &t!("prefix-choose-program"))
            .map_err(|err| format!("{err:#}"))
    })
    .await
    .map_err(|err| format!("{err}"))??;
    let Some(program) = picked else {
        return Ok(());
    };
    let state = app.state::<LibraryState>();
    start(&app, &state, id, Tool::Program(program))
}

/// Asks Heroic's list for updates once a day when
/// a page opens, then says the card may be read again.
#[tauri::command]
pub async fn prefix_want(id: String, app: AppHandle) -> Result<(), String> {
    let applies = app
        .state::<LibraryState>()
        .game(&id)
        .is_some_and(|game| known_fixes::applies(&game));
    if !applies {
        return Ok(());
    }
    let _ = tauri::async_runtime::spawn_blocking(known_fixes::refresh).await;
    crate::game::emit_game_changed(&app, &id);
    Ok(())
}

/// Starts a tool on a thread of its own, when none runs there and the game
/// does not; its end says `game_changed`.
fn start(app: &AppHandle, state: &LibraryState, id: String, tool: Tool) -> Result<(), String> {
    let Some(game) = state.game(&id) else {
        return Err(format!("no game with the id {id:?}"));
    };
    let running = state.running().is_running(&id);
    let prefix = app.state::<PrefixState>();
    if prefix.is_busy(&id) || running {
        return Err(t!("prefix-game-running"));
    }
    if !prefix_tools::applies(&game) {
        return Err(format!("Zendo keeps no Wine prefix for {}", game.title));
    }
    {
        prefix
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone(), tool.clone());
    }
    prefix.forget_note(&id);
    let handle = app.clone();
    let asked = id.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prefix_tools::run(&game, &tool).map_err(|err| format!("{err:#}"))
        }))
        .unwrap_or_else(|_| Err("the tool panicked".to_owned()));
        finished(&handle, &asked, &tool, result);
    });
    Ok(())
}

/// A tool ended: says how it went and tells the page.
fn finished(app: &AppHandle, id: &str, tool: &Tool, result: Result<Ran, String>) {
    let prefix = app.state::<PrefixState>();
    {
        prefix
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }
    if matches!(tool, Tool::Components(_)) && result.is_ok() {
        prefix.clear_components(id);
    }
    prefix.put_note(id.to_owned(), words::done(tool, result));
    crate::game::emit_game_changed(app, id);
}

/// Keeps the components typed in the game's launch options, so the box
/// holds them the next time Zendo starts.
fn save_components(id: &str, typed: &str) {
    let Ok(mut options) = LaunchOptions::load(id) else {
        return;
    };
    options.winetricks_components = Some(typed.trim())
        .filter(|typed| !typed.is_empty())
        .map(str::to_owned);
    let _ = options.save(id);
}

/// Opens the prefix's folder, with the desktop's own file manager.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its argument by value"
)]
pub fn prefix_open(path: String) -> Result<(), String> {
    zendo_shell::open::target(&path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn game() -> Game {
        Game::new(
            "local:prefix".into(),
            "Prefix".into(),
            zendo_core::Source::Local,
            zendo_core::Launch::Local {
                program: PathBuf::from("/nowhere/game.exe"),
                args: Vec::new(),
                dir: None,
            },
        )
    }

    #[test]
    fn a_tool_that_runs_holds_the_card() {
        let prefix = PrefixState::default();
        prefix.put_note("local:prefix".into(), ("It ended".into(), true));
        let running = status_of(Some(&Tool::Winetricks), false, prefix.note("local:prefix"));
        assert_eq!(running.expect("a line").tone, 3);
        let held = status_of(None, true, None).expect("a line");
        assert_eq!(held.text, t!("prefix-game-running"));
        let said = status_of(None, false, prefix.note("local:prefix")).expect("a line");
        assert_eq!((said.text.as_str(), said.tone), ("It ended", 1));
        assert!(status_of(None, false, None).is_none());
    }

    #[test]
    fn the_typed_components_are_kept_and_cleared() {
        let prefix = PrefixState::default();
        prefix.set_components("local:prefix", "vcrun2022".to_owned());
        assert_eq!(prefix.components("local:prefix"), "vcrun2022");
        prefix.clear_components("local:prefix");
        assert_eq!(prefix.components("local:prefix"), "");
    }

    #[test]
    fn a_game_outside_a_prefix_has_no_card() {
        let prefix = PrefixState::default();
        let steam = Game::new(
            "steam:620".into(),
            "Portal 2".into(),
            zendo_core::Source::Steam,
            zendo_core::Launch::Uri {
                uri: "steam://rungameid/620".into(),
            },
        );
        assert!(prefix_of(&steam, false, &prefix).is_none());
        assert!(prefix_of(&game(), false, &prefix).is_some());
    }
}
