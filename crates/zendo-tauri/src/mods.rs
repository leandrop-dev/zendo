//! The Mods card: the mods Zendo keeps for a game, the profile in use, a
//! Bethesda game's plugins, the folder they go in, and Apply and Back to
//! vanilla. The state and the words are [`zendo_shell::mods`]'s, shared with
//! the Slint window; each change runs as a [`Job`] on a thread through
//! `zendo_core::mods`, and its end says `game_changed`.
//!
//! Left for slice 7, with the sheets: Update from Nexus and the downloads'
//! lines on the card, Search Nexus, the FOMOD installer (an archive that has
//! one is not added, and the card says why), and the updates' check.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use zendo_core::config::Config;
use zendo_core::mods::{Overview, Plugins};
use zendo_core::nexus;
use zendo_core::plugins::{self, Rank};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::mods::{self as shell, After, Confirm, Job, ModsState, Naming, Shown, Tone};
use zendo_shell::t;

use crate::game::emit_game_changed;
use crate::library::LibraryState;

/// The card's state, which one game's job at a time changes, and the last
/// line a change left.
#[derive(Debug, Default)]
pub struct ModsHandle {
    state: Mutex<ModsState>,
    note: Mutex<Option<String>>,
}

impl ModsHandle {
    pub(crate) fn lock(&self) -> MutexGuard<'_, ModsState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn note(&self) -> Option<String> {
        self.note
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn say(&self, note: Option<String>) {
        if note.is_some() {
            *self.note.lock().unwrap_or_else(PoisonError::into_inner) = note;
        }
    }
}

/// A chip on the card: a profile.
#[derive(Debug, Serialize)]
pub struct ChipDto {
    label: String,
    chosen: bool,
}

/// A mod in the order Apply puts them in.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the card draws, as the Slint struct has them"
)]
pub struct ModRowDto {
    id: String,
    name: String,
    /// Where it came from, its files, what it wins and loses.
    facts: String,
    enabled: bool,
    up: bool,
    down: bool,
    /// "Delete", or "Really delete" once pressed.
    delete_label: String,
    asking: bool,
    /// "Update" or "Update to 2" when Nexus has a newer file; else empty.
    pub(crate) update: String,
}

/// A Bethesda game's plugin in load order.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the card draws, as the Slint struct has them"
)]
pub struct PluginRowDto {
    name: String,
    facts: String,
    /// What is wrong with it; empty when nothing is.
    problems: String,
    enabled: bool,
    /// The game always loads it: no box to untick.
    locked: bool,
    up: bool,
    down: bool,
}

#[derive(Debug, Serialize)]
pub struct PluginsDto {
    note: String,
    note_bad: bool,
    file: Option<String>,
    rows: Vec<PluginRowDto>,
}

/// A profile's name being typed.
#[derive(Debug, Serialize)]
pub struct NamingDto {
    placeholder: String,
    name: String,
    /// A change is not in flight and there is a name.
    ok: bool,
}

/// What the Mods card draws, at this moment.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the card draws, as the Slint struct has them"
)]
pub struct ModsDto {
    /// The card is drawn: the mods are here, or the read failed with why.
    show: bool,
    /// 1 the read failed, 2 the mods are here.
    body_kind: i32,
    message: String,
    /// What is happening to the game's mods now; empty when nothing is.
    busy: String,
    /// No change is in flight, so the controls take presses.
    free: bool,
    /// The game is on Nexus Mods.
    nexus: bool,
    note: Option<String>,
    profiles: Vec<ChipDto>,
    delete_profile_label: String,
    delete_profile_asking: bool,
    delete_profile_on: bool,
    naming: Option<NamingDto>,
    place: String,
    target: String,
    pub(crate) mods: Vec<ModRowDto>,
    plugins: Option<PluginsDto>,
    /// "N files in overwrite", when there are any.
    overwrite: Option<String>,
    overwrite_label: String,
    overwrite_asking: bool,
    folder: String,
    folder_editable: bool,
    folder_save: bool,
    folder_known: bool,
    state: String,
    state_tone: i32,
    can_apply: bool,
    can_restore: bool,
}

/// The card as it stands; nothing is started.
#[tauri::command]
pub fn mods_get(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    let game = library
        .game(&id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    let mods = app.state::<ModsHandle>();
    let running = library.running().is_running(&id);
    let mut card = dto(&mods, &game, running);
    crate::nexus::write_updates(&app, &id, &mut card);
    Ok(card)
}

/// The page just opened: reads the game's mods again, on a thread.
#[tauri::command]
pub fn mods_begin(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    let job = app.state::<ModsHandle>().lock().open(&id);
    run(&app, &library, Some(job));
    mods_get(id, app, library)
}

/// A mod's box: on puts its files in, off takes them out.
#[tauri::command]
pub fn mods_toggle_mod(
    id: String,
    mod_id: String,
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.toggle(id, &mod_id, on, ok)
    })?;
    mods_get(id, app, library)
}

/// A mod moved up or down the order.
#[tauri::command]
pub fn mods_move_mod(
    id: String,
    mod_id: String,
    up: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.move_mod(id, &mod_id, up, ok)
    })?;
    mods_get(id, app, library)
}

/// A plugin's box; the game's own plugins have none to untick.
#[tauri::command]
pub fn mods_toggle_plugin(
    id: String,
    name: String,
    on: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.toggle_plugin(id, &name, on, ok)
    })?;
    mods_get(id, app, library)
}

#[tauri::command]
pub fn mods_move_plugin(
    id: String,
    name: String,
    up: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.move_plugin(id, &name, up, ok)
    })?;
    mods_get(id, app, library)
}

/// Delete asks first and deletes the second time.
#[tauri::command]
pub fn mods_remove(
    id: String,
    mod_id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.remove(id, &mod_id, ok)
    })?;
    mods_get(id, app, library)
}

#[tauri::command]
pub fn mods_clear_overwrite(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.clear_overwrite(id, ok)
    })?;
    mods_get(id, app, library)
}

/// The folder typed and saved.
#[tauri::command]
pub fn mods_folder_save(
    id: String,
    text: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.type_folder(id, text.clone());
        state.save_folder(id, ok)
    })?;
    mods_get(id, app, library)
}

/// Back to where the game is known to take its mods.
#[tauri::command]
pub fn mods_folder_known(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, _| {
        state.known_folder(id, ok)
    })?;
    mods_get(id, app, library)
}

/// The profile a chip names.
#[tauri::command]
pub fn mods_profile_switch(
    id: String,
    name: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, running| {
        if state.overview(id)?.profile == name {
            return None;
        }
        state.switch_profile(id, &name, running, ok)
    })?;
    mods_get(id, app, library)
}

/// New… or Rename…: the name field opens.
#[tauri::command]
pub fn mods_profile_naming(
    id: String,
    renaming: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    app.state::<ModsHandle>().lock().start_naming(&id, renaming);
    mods_get(id, app, library)
}

/// The name typed and saved: a new profile, or a renaming.
#[tauri::command]
pub fn mods_profile_save(
    id: String,
    name: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, running| {
        state.name_typed(id, name.clone());
        state.save_profile(id, running, ok)
    })?;
    mods_get(id, app, library)
}

#[tauri::command]
pub fn mods_profile_cancel(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    app.state::<ModsHandle>().lock().cancel_profile();
    mods_get(id, app, library)
}

/// Delete asks first and deletes the second time.
#[tauri::command]
pub fn mods_profile_delete(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    act(&app, &library, &id, |state, id, ok, running| {
        state.delete_profile(id, running, ok)
    })?;
    mods_get(id, app, library)
}

/// Apply, or Back to vanilla; never while the game runs.
#[tauri::command]
pub fn mods_apply(
    id: String,
    apply: bool,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<ModsDto, String> {
    let (ok, running) = how(&library, &id);
    let applied = app
        .state::<ModsHandle>()
        .lock()
        .apply(&id, apply, running, ok);
    match applied {
        Ok(job) => {
            run(&app, &library, job);
            mods_get(id, app, library)
        }
        // A game that runs says why instead of changing anything.
        Err(note) => Err(note),
    }
}

/// Add mod…: an archive from the disk, chosen in Zendo's own file dialog.
#[tauri::command]
pub async fn mods_add(id: String, app: AppHandle) -> Result<(), String> {
    wait_for_slot(&app, &id)?;
    let picked = tauri::async_runtime::spawn_blocking(|| {
        let picked =
            file_dialog::pick(Kind::Mod, &t!("mods-choose")).map_err(|err| format!("{err:#}"))?;
        // The key lets a Nexus-named mod be followed for updates.
        let key = Config::load()
            .ok()
            .and_then(|config| config.nexus_key.clone());
        Ok::<_, String>((picked, key))
    })
    .await
    .map_err(|err| format!("{err}"))??;
    let (picked, key) = picked;
    {
        let mods = app.state::<ModsHandle>();
        mods.lock().chosen();
        let Some(archive) = picked else {
            return Ok(());
        };
        let library = app.state::<LibraryState>();
        let ok = library.game(&id).is_some();
        let job = mods.lock().add(&id, archive, key, ok);
        run(&app, &library, job);
    }
    emit_game_changed(&app, &id);
    Ok(())
}

/// Opens the game's page on Nexus Mods in the browser.
#[tauri::command]
pub fn mods_find_nexus(
    id: String,
    app: AppHandle,
    library: State<'_, LibraryState>,
) -> Result<(), String> {
    let domain = app
        .state::<ModsHandle>()
        .lock()
        .overview(&id)
        .and_then(|overview| overview.nexus.clone())
        .ok_or_else(|| "the game is not on Nexus Mods".to_owned())?;
    let _ = library;
    zendo_shell::open::target(&nexus::game_url(&domain))
}

/// Whether work can run, and whether the game is on screen: a change cannot
/// touch a game's files while it runs.
fn how(library: &LibraryState, id: &str) -> (bool, bool) {
    (library.game(id).is_some(), library.running().is_running(id))
}

/// A press on the card: the state moves, and the job it makes runs.
fn act(
    app: &AppHandle,
    library: &LibraryState,
    id: &str,
    change: impl FnOnce(&mut ModsState, &str, bool, bool) -> Option<Job>,
) -> Result<(), String> {
    if library.game(id).is_none() {
        return Err(format!("no game with the id {id:?}"));
    }
    let (ok, running) = how(library, id);
    let job = change(&mut app.state::<ModsHandle>().lock(), id, ok, running);
    run(app, library, job);
    Ok(())
}

/// Runs a job on a thread of its own; its end says `game_changed`.
fn run(app: &AppHandle, library: &LibraryState, job: Option<Job>) {
    if let Some(job) = job {
        run_job(app, library, job);
    }
}

fn run_job(app: &AppHandle, library: &LibraryState, job: Job) {
    let Some(game) = library.game(&job.id) else {
        return;
    };
    let handle = app.clone();
    let Job { id, work } = job;
    std::thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| shell::run(work, &game)))
            .unwrap_or_else(|_| Err("the change panicked".to_owned()));
        let mods = handle.state::<ModsHandle>();
        let done = mods.lock().done(&id, result);
        mods.say(done.status);
        for after in done.after {
            match after {
                // Nexus is asked once a session for newer files of the mods.
                After::CheckUpdates(game) => crate::nexus::want_updates(&handle, &game),
                // The installer sheets arrive with the next slice.
                After::NextInstaller => {}
                After::OpenInstaller(_, _) => mods.say(Some(t!("mods-installer-later"))),
            }
        }
        emit_game_changed(&handle, &id);
    });
}

/// The slot must be free before the file dialog opens.
fn wait_for_slot(app: &AppHandle, id: &str) -> Result<(), String> {
    let mods = app.state::<ModsHandle>();
    let mut state = mods.lock();
    let busy = !state.free();
    if busy || !state.choosing(id, true) {
        return Err(t!("mods-close-game-first"));
    }
    drop(state);
    emit_game_changed(app, id);
    Ok(())
}

/// The card as it stands.
fn dto(mods: &ModsHandle, game: &zendo_core::Game, running: bool) -> ModsDto {
    let state = mods.lock();
    let id = game.id.as_str();
    let note = mods.note();
    let Some(shown) = state.shown(id) else {
        return hidden(state.free(), state.busy_for(id), note);
    };
    match shown {
        Shown::Reading | Shown::Nowhere => hidden(state.free(), state.busy_for(id), note),
        Shown::Failed(err) => ModsDto {
            show: true,
            body_kind: 1,
            message: err.clone(),
            ..hidden(state.free(), state.busy_for(id), note)
        },
        Shown::Ready(overview) => ready(&state, mods, id, overview, running),
    }
}

/// The card while there is nothing to draw.
fn hidden(free: bool, busy: Option<&str>, note: Option<String>) -> ModsDto {
    ModsDto {
        show: false,
        body_kind: 0,
        message: String::new(),
        busy: busy.map_or_default(str::to_owned),
        free,
        nexus: false,
        note,
        profiles: Vec::new(),
        delete_profile_label: String::new(),
        delete_profile_asking: false,
        delete_profile_on: false,
        naming: None,
        place: String::new(),
        target: String::new(),
        mods: Vec::new(),
        plugins: None,
        overwrite: None,
        overwrite_label: String::new(),
        overwrite_asking: false,
        folder: String::new(),
        folder_editable: false,
        folder_save: false,
        folder_known: false,
        state: String::new(),
        state_tone: 0,
        can_apply: false,
        can_restore: false,
    }
}

/// The card with the game's mods in it.
fn ready(
    state: &ModsState,
    mods: &ModsHandle,
    id: &str,
    overview: &Overview,
    running: bool,
) -> ModsDto {
    let free = state.free();
    let last = overview.mods.len().saturating_sub(1);
    let asking_delete = state.asking(id, &Confirm::DeleteProfile(overview.profile.clone()));
    let (words, tone) = shell::apply_state(overview, running);
    let typed = state.typed_folder(id);
    let overwrite_asking = state.asking(id, &Confirm::ClearOverwrite);
    ModsDto {
        show: true,
        body_kind: 2,
        message: String::new(),
        busy: state.busy_for(id).unwrap_or_default().to_owned(),
        free,
        nexus: overview.nexus.is_some(),
        note: mods.note(),
        profiles: overview
            .profiles
            .iter()
            .map(|name| ChipDto {
                label: name.clone(),
                chosen: *name == overview.profile,
            })
            .collect(),
        delete_profile_label: shell::delete_label(asking_delete),
        delete_profile_asking: asking_delete,
        delete_profile_on: shell::profile_delete_enabled(free, overview),
        naming: state.naming(id).map(|naming| naming_dto(naming, free)),
        place: shell::place_line(overview),
        target: overview.target.display().to_string(),
        mods: overview
            .mods
            .iter()
            .enumerate()
            .map(|(index, info)| ModRowDto {
                id: info.id.clone(),
                name: info.name.clone(),
                facts: shell::mod_facts(info),
                enabled: info.enabled,
                up: index > 0,
                down: index < last,
                delete_label: shell::delete_label(
                    state.asking(id, &Confirm::Remove(info.id.clone())),
                ),
                asking: state.asking(id, &Confirm::Remove(info.id.clone())),
                update: String::new(),
            })
            .collect(),
        plugins: overview
            .plugins
            .as_ref()
            .map(|found| plugins_dto(found, overview)),
        overwrite: (overview.overwrite > 0)
            .then(|| t!("mods-overwrite", count = overview.overwrite)),
        overwrite_label: shell::delete_label(overwrite_asking),
        overwrite_asking,
        folder: typed.unwrap_or(&overview.folder).to_owned(),
        folder_editable: shell::folder_editable(overview),
        folder_save: shell::folder_save_shown(typed, overview),
        folder_known: shell::known_folder_shown(overview),
        state: words,
        state_tone: match tone {
            Tone::Dim => 0,
            Tone::Good => 1,
            Tone::Bad => 2,
        },
        can_apply: shell::can_apply(free, running, overview),
        can_restore: shell::can_restore(free, running, overview),
    }
}

fn naming_dto(naming: &Naming, free: bool) -> NamingDto {
    NamingDto {
        placeholder: shell::name_placeholder(naming.renaming.is_some()),
        name: naming.name.clone(),
        ok: free && !naming.name.trim().is_empty(),
    }
}

/// A Bethesda game's plugins, and what is wrong with them.
fn plugins_dto(found: &Plugins, overview: &Overview) -> PluginsDto {
    let (note, note_bad) = shell::plugins_note(found, shell::plugins_style(overview));
    PluginsDto {
        note,
        note_bad,
        file: found.file.as_ref().map(|file| file.display().to_string()),
        rows: found
            .list
            .iter()
            .enumerate()
            .map(|(index, plugin)| PluginRowDto {
                name: plugin.name.clone(),
                facts: shell::plugin_facts(plugin),
                problems: shell::plugin_problems(plugin),
                enabled: plugin.enabled,
                locked: plugin.rank == Rank::Implicit,
                up: plugins::can_move(&found.list, index, true),
                down: plugins::can_move(&found.list, index, false),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use zendo_core::mods::{self, Applied, ModInfo};
    use zendo_core::{Game, Launch, Source};

    use super::*;

    fn game() -> Game {
        Game::new(
            "local:mods".into(),
            "Mods".into(),
            Source::Local,
            Launch::Local {
                program: PathBuf::from("/nowhere/game"),
                args: Vec::new(),
                dir: None,
            },
        )
    }

    fn overview() -> Overview {
        Overview {
            game_dir: "/games/Mine".into(),
            target: "/games/Mine".into(),
            folder: "/games/Mine/Data".into(),
            custom_folder: false,
            known: None,
            nexus: Some("mine".into()),
            profile: mods::DEFAULT_PROFILE.into(),
            profiles: vec![mods::DEFAULT_PROFILE.into(), "Light".into()],
            mods: ["one", "two"]
                .iter()
                .map(|name| ModInfo {
                    id: (*name).into(),
                    name: (*name).into(),
                    enabled: true,
                    files: 2,
                    nexus: None,
                    installer: false,
                    wins: Vec::new(),
                    loses: Vec::new(),
                })
                .collect(),
            overwrite: 2,
            applied: Applied::No,
            plugins: None,
        }
    }

    #[test]
    fn a_card_not_read_yet_is_not_drawn() {
        let mods = ModsHandle::default();
        let dto = dto(&mods, &game(), false);
        assert!(!dto.show);
        assert!(dto.free, "nothing runs");
    }

    #[test]
    fn the_mods_show_with_their_order_and_the_profile_in_use() {
        let mods = ModsHandle::default();
        mods.lock().ready("local:mods".into(), overview());
        let dto = dto(&mods, &game(), false);
        assert!(dto.show);
        assert_eq!(dto.body_kind, 2);
        assert_eq!(dto.mods.len(), 2);
        assert!(!dto.mods[0].up, "the first cannot go up");
        assert!(dto.mods[0].down);
        assert!(!dto.mods[1].down, "the last cannot go down");
        assert_eq!(dto.profiles.len(), 2);
        assert!(dto.profiles[0].chosen);
        assert!(dto.overwrite.is_some());
        assert_eq!(dto.target, "/games/Mine");
        assert!(dto.nexus);
    }

    #[test]
    fn a_failed_read_says_why() {
        let mods = ModsHandle::default();
        let _job = mods.lock().open("local:mods");
        let _ = mods
            .lock()
            .done("local:mods", Err("could not read the mods".into()));
        let dto = dto(&mods, &game(), false);
        assert!(dto.show);
        assert_eq!(dto.body_kind, 1);
        assert_eq!(dto.message, "could not read the mods");
    }
}
