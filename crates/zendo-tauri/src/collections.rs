//! The Collections sheet: a collection is either hand-picked (a category) or a
//! saved view (what the library shows, kept under a name). Both live in one
//! list; they are made, renamed and deleted here, and the games the sheet was
//! opened for are put into or taken out of the hand-picked ones.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use zendo_core::{Filter, State as CoreState};
use zendo_shell::collections::{Item, items, member, summary};
use zendo_shell::t;

use crate::library::LibraryState;
use crate::settings::RowDto;

/// What the sheet keeps while it is open.
#[derive(Default)]
struct Sheet {
    /// The games the ticks assign. Empty when the sheet only manages.
    targets: BTreeSet<String>,
    draft: String,
    renaming: Option<String>,
    rename_draft: String,
    /// The collection whose Delete was pressed once: pressed again, it goes.
    confirm_delete: Option<String>,
    note: Option<(String, bool)>,
}

#[derive(Default)]
pub struct CollectionsState {
    sheet: Mutex<Option<Sheet>>,
}

impl CollectionsState {
    fn lock(&self) -> MutexGuard<'_, Option<Sheet>> {
        self.sheet.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the row draws, as the Slint struct has them"
)]
pub struct CollRowDto {
    name: String,
    sub: String,
    count: String,
    view: bool,
    checked: bool,
    checkable: bool,
    renaming: bool,
    confirm: bool,
}

#[derive(Debug, Serialize)]
pub struct CollectionsDto {
    open: bool,
    count: String,
    empty: String,
    now: String,
    draft: String,
    rename_draft: String,
    note: Option<RowDto>,
    rows: Vec<CollRowDto>,
}

/// Opens the sheet for the marked games (`marked`), or to manage only.
pub(crate) fn open(app: &AppHandle, marked: bool) {
    if let Ok(state) = CoreState::load() {
        app.state::<LibraryState>()
            .set_collections(state.categories(), state.filters());
    }
    let targets = if marked {
        app.state::<LibraryState>().marked()
    } else {
        BTreeSet::new()
    };
    *app.state::<CollectionsState>().lock() = Some(Sheet {
        targets,
        ..Sheet::default()
    });
    let _ = app.emit("collections_changed", ());
}

#[tauri::command]
pub fn collections_open(marked: bool, app: AppHandle) -> CollectionsDto {
    open(&app, marked);
    get(&app)
}

#[tauri::command]
pub fn collections_close(app: AppHandle) -> CollectionsDto {
    *app.state::<CollectionsState>().lock() = None;
    let _ = app.emit("collections_changed", ());
    get(&app)
}

#[tauri::command]
pub fn collections_get(app: AppHandle) -> CollectionsDto {
    get(&app)
}

/// The sheet as it stands.
pub(crate) fn get(app: &AppHandle) -> CollectionsDto {
    let state = app.state::<CollectionsState>();
    let sheet = state.lock();
    let Some(sheet) = sheet.as_ref() else {
        return CollectionsDto {
            open: false,
            count: String::new(),
            empty: String::new(),
            now: String::new(),
            draft: String::new(),
            rename_draft: String::new(),
            note: None,
            rows: Vec::new(),
        };
    };
    let library = app.state::<LibraryState>();
    let (categories, saved) = library.collections();
    let rows = items(&categories, &saved)
        .into_iter()
        .map(|item| {
            let name = item.name().to_owned();
            let (view, sub, count, checked) = match item {
                Item::Picked(category) => (
                    false,
                    String::new(),
                    t!("count-games", count = category.games.len()),
                    member(&sheet.targets, category),
                ),
                Item::View(saved) => (true, summary(&saved.filter), String::new(), false),
            };
            CollRowDto {
                renaming: sheet.renaming.as_deref() == Some(name.as_str()),
                confirm: sheet.confirm_delete.as_deref() == Some(name.as_str()),
                checkable: !view && !sheet.targets.is_empty(),
                name,
                sub,
                count,
                view,
                checked,
            }
        })
        .collect();
    CollectionsDto {
        open: true,
        count: t!("collections-count", count = categories.len() + saved.len()),
        empty: if categories.is_empty() && saved.is_empty() {
            t!("collections-empty")
        } else {
            String::new()
        },
        now: t!(
            "collections-now",
            summary = summary(&library.current_filter())
        ),
        draft: sheet.draft.clone(),
        rename_draft: sheet.rename_draft.clone(),
        note: sheet.note.as_ref().map(|(note, good)| RowDto {
            label: String::new(),
            value: note.clone(),
            tone: if *good { 1 } else { 2 },
        }),
        rows,
    }
}

#[tauri::command]
pub fn collections_draft(text: String, app: AppHandle) -> CollectionsDto {
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.draft = text;
    }
    get(&app)
}

#[tauri::command]
pub fn collections_rename_draft(text: String, app: AppHandle) -> CollectionsDto {
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.rename_draft = text;
    }
    get(&app)
}

/// Makes the hand-picked collection in the draft, and puts the sheet's games
/// in it.
#[tauri::command]
pub fn collections_create(app: AppHandle) -> CollectionsDto {
    untake_delete(&app);
    let (draft, targets) = {
        let state = app.state::<CollectionsState>();
        let guard = state.lock();
        let Some(sheet) = guard.as_ref() else {
            return get(&app);
        };
        (
            sheet.draft.clone(),
            sheet.targets.iter().cloned().collect::<Vec<_>>(),
        )
    };
    if draft.trim().is_empty() {
        note(&app, t!("collections-blank"), false);
        return get(&app);
    }
    let mut created = false;
    if let Ok(state) = CoreState::update(|state| {
        created = state.add_category(&draft);
        if created {
            for id in &targets {
                state.set_category(&draft, id, true);
            }
        }
    }) {
        app.state::<LibraryState>()
            .set_collections(state.categories(), state.filters());
    }
    if created {
        if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
            sheet.draft.clear();
        }
        note(&app, t!("collections-created", name = draft.trim()), true);
    } else {
        note(&app, t!("collections-taken", name = draft.trim()), false);
    }
    get(&app)
}

/// Keeps what the library shows under the draft's name.
#[tauri::command]
pub fn collections_save_view(app: AppHandle) -> CollectionsDto {
    untake_delete(&app);
    let draft = {
        let state = app.state::<CollectionsState>();
        let guard = state.lock();
        let Some(sheet) = guard.as_ref() else {
            return get(&app);
        };
        sheet.draft.clone()
    };
    let name = draft.trim().to_owned();
    if name.is_empty() {
        note(&app, t!("collections-blank"), false);
        return get(&app);
    }
    let library = app.state::<LibraryState>();
    let existed = library
        .collections()
        .1
        .iter()
        .any(|saved| saved.name.to_lowercase() == name.to_lowercase());
    let filter: Filter = library.current_filter();
    let mut saved = false;
    if let Ok(state) = CoreState::update(|state| saved = state.save_filter(&name, filter)) {
        library.set_collections(state.categories(), state.filters());
    }
    if !saved {
        note(&app, t!("collections-taken", name = name.as_str()), false);
        return get(&app);
    }
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.draft.clear();
    }
    note(
        &app,
        if existed {
            t!("collections-updated", name = name.as_str())
        } else {
            t!("collections-saved", name = name.as_str())
        },
        true,
    );
    get(&app)
}

#[tauri::command]
pub fn collections_rename(name: String, app: AppHandle) -> CollectionsDto {
    untake_delete(&app);
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.renaming = Some(name.clone());
        sheet.rename_draft = name;
        sheet.note = None;
    }
    get(&app)
}

#[tauri::command]
pub fn collections_rename_done(app: AppHandle) -> CollectionsDto {
    untake_delete(&app);
    let (from, to) = {
        let state = app.state::<CollectionsState>();
        let guard = state.lock();
        let Some(sheet) = guard.as_ref() else {
            return get(&app);
        };
        let Some(from) = sheet.renaming.clone() else {
            return get(&app);
        };
        (from, sheet.rename_draft.trim().to_owned())
    };
    if to.is_empty() {
        note(&app, t!("collections-blank"), false);
        return get(&app);
    }
    let view = app
        .state::<LibraryState>()
        .collections()
        .1
        .iter()
        .any(|saved| saved.name == from);
    let mut renamed = false;
    if let Ok(state) = CoreState::update(|state| {
        renamed = if view {
            state.rename_filter(&from, &to)
        } else {
            state.rename_category(&from, &to)
        };
    }) {
        app.state::<LibraryState>()
            .set_collections(state.categories(), state.filters());
    }
    if renamed {
        if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
            sheet.renaming = None;
            sheet.rename_draft.clear();
        }
        note(&app, t!("collections-renamed", name = to.as_str()), true);
    } else {
        note(&app, t!("collections-taken", name = to.as_str()), false);
    }
    get(&app)
}

/// Ticks or unticks a hand-picked collection for every game the sheet was
/// opened for.
#[tauri::command]
pub fn collections_set_member(name: String, member: bool, app: AppHandle) -> CollectionsDto {
    untake_delete(&app);
    let targets = {
        let state = app.state::<CollectionsState>();
        let guard = state.lock();
        let Some(sheet) = guard.as_ref() else {
            return get(&app);
        };
        sheet.targets.iter().cloned().collect::<Vec<_>>()
    };
    let count = targets.len();
    if count == 0 {
        return get(&app);
    }
    if let Ok(state) = CoreState::update(|state| {
        for id in &targets {
            state.set_category(&name, id, member);
        }
    }) {
        app.state::<LibraryState>()
            .set_collections(state.categories(), state.filters());
    }
    note(
        &app,
        if member {
            t!("collections-assigned", count = count)
        } else {
            t!("collections-unassigned", count = count)
        },
        true,
    );
    get(&app)
}

#[tauri::command]
pub fn collections_delete(name: String, app: AppHandle) -> CollectionsDto {
    let asking = {
        let state = app.state::<CollectionsState>();
        let mut guard = state.lock();
        let Some(sheet) = guard.as_mut() else {
            return get(&app);
        };
        let asking = sheet.confirm_delete.as_deref() != Some(name.as_str());
        sheet.confirm_delete = asking.then(|| name.clone());
        if asking {
            sheet.note = None;
        }
        asking
    };
    if asking {
        return get(&app);
    }
    let view = app
        .state::<LibraryState>()
        .collections()
        .1
        .iter()
        .any(|saved| saved.name == name);
    let mut deleted = false;
    if let Ok(state) = CoreState::update(|state| {
        deleted = if view {
            state.delete_filter(&name)
        } else {
            state.delete_category(&name)
        };
    }) {
        app.state::<LibraryState>()
            .set_collections(state.categories(), state.filters());
    }
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut()
        && sheet.renaming.as_deref() == Some(name.as_str())
    {
        sheet.renaming = None;
        sheet.rename_draft.clear();
    }
    if deleted {
        note(&app, t!("collections-deleted", name = name.trim()), true);
    }
    get(&app)
}

/// Closes the sheet; the frontend then puts the saved view on the library.
#[tauri::command]
pub fn collections_apply(_name: String, app: AppHandle) -> CollectionsDto {
    *app.state::<CollectionsState>().lock() = None;
    let _ = app.emit("collections_changed", ());
    get(&app)
}

fn note(app: &AppHandle, text: String, worked: bool) {
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.note = Some((text, worked));
    }
}

fn untake_delete(app: &AppHandle) {
    if let Some(sheet) = app.state::<CollectionsState>().lock().as_mut() {
        sheet.confirm_delete = None;
    }
}
