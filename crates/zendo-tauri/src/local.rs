//! Games added by hand: the form the Library opens to add one, a game's page
//! to edit it, and Remove. The records are `zendo_core::local`'s; the file
//! dialog is a process of its own, as it is for the Slint window.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use zendo_core::local::{self, LocalGame};
use zendo_shell::file_dialog::{self, Kind};

use crate::library::LibraryState;

/// What the form holds: the game being edited, or a new one with no id.
#[derive(Debug, Serialize, Deserialize)]
pub struct LocalDraft {
    id: Option<String>,
    title: String,
    program: String,
    arguments: String,
}

fn draft(game: LocalGame) -> LocalDraft {
    LocalDraft {
        id: Some(game.id),
        title: game.title,
        program: game.program.display().to_string(),
        arguments: game.arguments,
    }
}

/// The game's record, for the page's Edit.
#[tauri::command]
pub fn local_get(id: String) -> Result<LocalDraft, String> {
    let game = local::find(&id)
        .map_err(|err| format!("{err:#}"))?
        .ok_or_else(|| zendo_shell::t!("local-no-record"))?;
    Ok(draft(game))
}

/// What the form says under the program field: whether it is a Windows game.
#[tauri::command]
pub fn local_about(program: String) -> String {
    let program = program.trim();
    if program.is_empty() {
        zendo_shell::t!("local-about-empty")
    } else if local::is_windows(Path::new(program)) {
        zendo_shell::t!("local-about-windows")
    } else {
        zendo_shell::t!("local-about-linux")
    }
}

/// The Choose button: the file dialog runs as a process, on a blocker
/// thread, and the answer comes back here.
#[tauri::command]
pub async fn local_pick_program() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        file_dialog::pick(Kind::Program, &zendo_shell::t!("local-choose-program"))
            .map(|path| path.map(|path| path.display().to_string()))
            .map_err(|err| format!("{err:#}"))
    })
    .await
    .map_err(|err| format!("{err}"))?
}

/// Adds the game, or saves the changes to it, then reads the library again.
/// The id is the one the page opens.
#[tauri::command]
pub fn local_save(
    draft: LocalDraft,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<String, String> {
    let game = LocalGame {
        id: draft.id.unwrap_or_default(),
        title: draft.title,
        program: PathBuf::from(draft.program),
        arguments: draft.arguments,
    };
    game.check().map_err(|err| format!("{err:#}"))?;
    let id = local::put(game).map_err(|err| format!("{err:#}"))?;
    state.reload();
    let _ = app.emit("library_loaded", ());
    Ok(id)
}

/// Takes the game out of the library; its files stay where they are.
#[tauri::command]
pub fn local_remove(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<(), String> {
    local::remove(&id).map_err(|err| format!("{err:#}"))?;
    state.reload();
    let _ = app.emit("library_loaded", ());
    Ok(())
}
