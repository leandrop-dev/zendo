//! The Mods card on the page of a game with a folder to put mods in: the
//! profile in use, each mod in order with its box, ↑ ↓ and Delete, a
//! Bethesda game's plugins, the folder they go in, Apply and Back to
//! vanilla, Add mod through the file dialog and Find on Nexus. The state, the
//! jobs and the words are `zendo_shell::mods`'; each job runs on a thread
//! through `Glue::ask`. A part of the Library's glue (`library_ui`).
//!
//! The downloads and Update on the card come from `nexus_ui.rs` (item
//! 52c2a) and Search Nexus from `search_ui.rs` (52c2b). An archive with a
//! FOMOD installer, and Options… on a mod, open the sheet of
//! `installer_ui.rs` (52c3).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use zendo_core::Game;
use zendo_core::mods::Overview;
use zendo_core::nexus;
use zendo_core::plugins::{self, Rank};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::installer::Ready;
use zendo_shell::mods::{self as shell, After, Confirm, Job, ModsState, Outcome, Shown, Tone};
use zendo_shell::open;
use zendo_shell::t;

use super::settings::chip;
use super::{Glue, with_glue};
use crate::{ChipData, MainWindow, ModRow, ModsData, PluginRow};

/// The card's state and the models its rows come from.
pub(super) struct ModsUiState {
    pub(super) state: RefCell<ModsState>,
    rows: Rc<VecModel<ModRow>>,
    plugins: Rc<VecModel<PluginRow>>,
    profiles: Rc<VecModel<ChipData>>,
}

impl ModsUiState {
    pub(super) fn new() -> Self {
        Self {
            state: RefCell::default(),
            rows: Rc::new(VecModel::default()),
            plugins: Rc::new(VecModel::default()),
            profiles: Rc::new(VecModel::default()),
        }
    }
}

/// Puts `rows` in the model: row by row when the count is the same, so the
/// rows on the page stay where they are. A reset would empty the card for
/// a moment, and a page scrolled low is pulled up to the shorter height.
pub(super) fn refill<T: Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    if model.row_count() == rows.len() {
        for (index, row) in rows.into_iter().enumerate() {
            model.set_row_data(index, row);
        }
    } else {
        model.set_vec(rows);
    }
}

/// An index the window sent, as one to read a list by.
fn at(index: i32) -> Option<usize> {
    usize::try_from(index).ok()
}

impl Glue {
    /// Hooks the card's callbacks up and gives it its models.
    pub(super) fn connect_mods(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<ModsData>();
        data.set_mods(ModelRc::from(Rc::clone(&self.mods.rows)));
        data.set_plugins(ModelRc::from(Rc::clone(&self.mods.plugins)));
        data.set_profiles(ModelRc::from(Rc::clone(&self.mods.profiles)));

        let glue = Rc::clone(self);
        data.on_add(move || glue.mods_add());
        let glue = Rc::clone(self);
        data.on_find_nexus(move || glue.mods_find_on_nexus());
        let glue = Rc::clone(self);
        data.on_profile(move |index| {
            glue.mods_act(|state, id, ok, running| {
                let name = state.overview(id)?.profiles.get(at(index)?)?.clone();
                if state.overview(id)?.profile == name {
                    return None;
                }
                state.switch_profile(id, &name, running, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_new_profile(move || {
            glue.mods_act(|state, id, _, _| {
                state.start_naming(id, false);
                None
            });
        });
        let glue = Rc::clone(self);
        data.on_rename_profile(move || {
            glue.mods_act(|state, id, _, _| {
                state.start_naming(id, true);
                None
            });
        });
        let glue = Rc::clone(self);
        data.on_delete_profile(move || {
            glue.mods_act(|state, id, ok, running| state.delete_profile(id, running, ok));
        });
        let glue = Rc::clone(self);
        data.on_name_typed(move |typed| {
            if let Some(id) = glue.open.borrow().clone() {
                let mut state = glue.mods.state.borrow_mut();
                state.name_typed(&id, typed.to_string());
                // Save wakes up as soon as there is a name, with no redraw of
                // the whole page for each letter.
                let ok = state.free() && !typed.trim().is_empty();
                if let Some(window) = glue.window.upgrade() {
                    window.global::<ModsData>().set_name_ok(ok);
                }
            }
        });
        let glue = Rc::clone(self);
        data.on_save_name(move || {
            glue.mods_act(|state, id, ok, running| state.save_profile(id, running, ok));
            glue.focus_page();
        });
        let glue = Rc::clone(self);
        data.on_cancel_name(move || {
            glue.mods_act(|state, _, _, _| {
                state.cancel_profile();
                None
            });
            glue.focus_page();
        });
        self.connect_mods_rows(&data);
    }

    /// The callbacks of the mods, the plugins, the folder and Apply.
    fn connect_mods_rows(self: &Rc<Self>, data: &ModsData<'_>) {
        let glue = Rc::clone(self);
        data.on_toggle(move |index, on| {
            glue.mods_act(|state, id, ok, _| {
                let mod_id = state.overview(id)?.mods.get(at(index)?)?.id.clone();
                state.toggle(id, &mod_id, on, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_options(move |index| {
            glue.mods_act(|state, id, ok, _| {
                let mod_id = state.overview(id)?.mods.get(at(index)?)?.id.clone();
                state.reinstall(id, &mod_id, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_step(move |index, up| {
            glue.mods_act(|state, id, ok, _| {
                let mod_id = state.overview(id)?.mods.get(at(index)?)?.id.clone();
                state.move_mod(id, &mod_id, up, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_remove(move |index| {
            glue.mods_act(|state, id, ok, _| {
                let mod_id = state.overview(id)?.mods.get(at(index)?)?.id.clone();
                state.remove(id, &mod_id, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_plugin_toggle(move |index, on| {
            glue.mods_act(|state, id, ok, _| {
                let list = &state.overview(id)?.plugins.as_ref()?.list;
                let name = list.get(at(index)?)?.name.clone();
                state.toggle_plugin(id, &name, on, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_plugin_step(move |index, up| {
            glue.mods_act(|state, id, ok, _| {
                let list = &state.overview(id)?.plugins.as_ref()?.list;
                let name = list.get(at(index)?)?.name.clone();
                state.move_plugin(id, &name, up, ok)
            });
        });
        let glue = Rc::clone(self);
        data.on_clear_overwrite(move || {
            glue.mods_act(|state, id, ok, _| state.clear_overwrite(id, ok));
        });
        let glue = Rc::clone(self);
        data.on_folder_typed(move |typed| {
            glue.mods_act(|state, id, _, _| {
                state.type_folder(id, typed.to_string());
                None
            });
        });
        let glue = Rc::clone(self);
        data.on_save_folder(move || {
            glue.mods_act(|state, id, ok, _| state.save_folder(id, ok));
        });
        let glue = Rc::clone(self);
        data.on_known_folder(move || {
            glue.mods_act(|state, id, ok, _| state.known_folder(id, ok));
        });
        let glue = Rc::clone(self);
        data.on_apply(move || glue.mods_apply(true));
        let glue = Rc::clone(self);
        data.on_restore(move || glue.mods_apply(false));
    }

    /// Whether work on the game's mods can run: it is in the library, and
    /// this is not a test.
    pub(super) fn mods_runnable(&self, id: &str) -> bool {
        self.persist && self.browser.borrow().game(id).is_some()
    }

    /// A press on the open game's card: the state moves, the job it makes
    /// runs, and the card is drawn again.
    fn mods_act(&self, act: impl FnOnce(&mut ModsState, &str, bool, bool) -> Option<Job>) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let ok = self.mods_runnable(&id);
        let running = self.game.running.borrow().is_running(&id);
        let job = act(&mut self.mods.state.borrow_mut(), &id, ok, running);
        self.mods_run(job);
        self.show_game();
    }

    /// Apply, or Back to vanilla, never while the game runs.
    fn mods_apply(&self, apply: bool) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let ok = self.mods_runnable(&id);
        let running = self.game.running.borrow().is_running(&id);
        let result = self.mods.state.borrow_mut().apply(&id, apply, running, ok);
        match result {
            Ok(job) => self.mods_run(job),
            Err(note) => self.say(note),
        }
        self.show_game();
    }

    /// Runs a job on a thread.
    pub(super) fn mods_run(&self, job: Option<Job>) {
        let Some(Job { id, work }) = job else {
            return;
        };
        let Some(game) = self.browser.borrow().game(&id).cloned() else {
            return;
        };
        if !self.persist {
            return;
        }
        self.ask(
            move || shell::run(work, &game),
            move |glue, result| glue.mods_arrived(&id, result),
        );
    }

    /// A game's mods as the card holds them.
    pub(super) fn mods_overview(&self, id: &str) -> Option<Overview> {
        self.mods.state.borrow().overview(id).cloned()
    }

    /// A game's mods as they now stand, after a download put one in.
    pub(super) fn mods_ready(&self, id: &str, overview: Overview) {
        self.mods.state.borrow_mut().ready(id.to_owned(), overview);
    }

    /// The page just opened: reads the game's mods again.
    pub(super) fn mods_want(&self, id: &str) {
        if !self.mods_runnable(id) {
            return;
        }
        let job = self.mods.state.borrow_mut().open(id);
        self.mods_run(Some(job));
    }

    /// A job ended: the state it leaves, what it says, and what follows.
    pub(super) fn mods_arrived(&self, id: &str, result: Result<Outcome, String>) {
        let done = self.mods.state.borrow_mut().done(id, result);
        if let Some(status) = done.status {
            self.say(status);
        }
        for after in done.after {
            match after {
                // Nexus is asked once a session for newer files of the mods.
                After::CheckUpdates(game) => self.nexus_want_updates(&game),
                // An installer that waited for the slot may open.
                After::NextInstaller => self.installer_next_waiting(),
                // An archive with a FOMOD installer: its options are chosen.
                After::OpenInstaller(game, pending) => self.installer_open(game, pending),
            }
        }
        self.show_game();
        self.show_search();
    }

    /// No change to any game's mods is in flight.
    pub(super) fn mods_free(&self) -> bool {
        self.mods.state.borrow().free()
    }

    /// Builds a mod from its installer with the options chosen.
    pub(super) fn mods_install(&self, ready: Ready) {
        let Ready {
            game,
            pending,
            selection,
        } = ready;
        let ok = self.mods_runnable(&game);
        let job = self
            .mods
            .state
            .borrow_mut()
            .install(&game, pending, selection, ok);
        self.mods_run(job);
    }

    /// Asks which archive to add, then adds it.
    fn mods_add(&self) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let ok = self.mods_runnable(&id);
        if !self.mods.state.borrow_mut().choosing(&id, ok) {
            return;
        }
        self.show_game();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result =
                file_dialog::pick(Kind::Mod, &t!("mods-choose")).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.mods_picked(&id, result));
            });
        });
    }

    pub(super) fn mods_picked(&self, id: &str, result: Result<Option<PathBuf>, String>) {
        self.mods.state.borrow_mut().chosen();
        match result {
            Ok(Some(archive)) => {
                let key = self
                    .config
                    .borrow()
                    .as_ref()
                    .and_then(|config| config.nexus_key.clone());
                let ok = self.mods_runnable(id);
                let job = self.mods.state.borrow_mut().add(id, archive, key, ok);
                self.mods_run(job);
            }
            Ok(None) => {}
            Err(err) => self.say(err),
        }
        self.show_game();
    }

    /// Adds the file Nexus's window downloaded for a mod of the game.
    pub(super) fn mods_add_from_site(&self, id: &str, archive: PathBuf, domain: &str, mod_id: u64) {
        let key = self.nexus_key();
        let ok = self.mods_runnable(id);
        let job = self.mods.state.borrow_mut().add_from_site(
            id,
            archive,
            key,
            domain.to_owned(),
            mod_id,
            ok,
        );
        self.mods_run(job);
        self.show_game();
    }

    /// Opens the game's page on Nexus Mods in the browser.
    fn mods_find_on_nexus(&self) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let domain = self
            .mods
            .state
            .borrow()
            .overview(&id)
            .and_then(|overview| overview.nexus.clone());
        if let Some(domain) = domain
            && self.persist
            && let Err(err) = open::target(&nexus::game_url(&domain))
        {
            self.say(err);
        }
    }

    /// The page closed: nothing of the card stays for the next one.
    pub(super) fn mods_clear(&self) {
        self.mods.rows.set_vec(Vec::new());
        self.mods.plugins.set_vec(Vec::new());
        self.mods.profiles.set_vec(Vec::new());
        if let Some(window) = self.window.upgrade() {
            window.global::<ModsData>().set_show(false);
        }
    }

    /// Writes the card of the open game: none for a game with no folder to
    /// put mods in, or whose mods have not been read.
    pub(super) fn show_mods(&self, game: &Game) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<ModsData>();
        let state = self.mods.state.borrow();
        let id = game.id.as_str();
        self.show_nexus_downloads();
        data.set_busy(state.busy_for(id).unwrap_or_default().into());
        data.set_free(state.free());
        let (kind, message) = match state.shown(id) {
            // While the mods are read there is no card: it comes whole, as
            // every other card on the page does. Growing a card that is
            // already there left the page's height short of it.
            None | Some(Shown::Nowhere | Shown::Reading) => {
                self.mods_clear();
                return;
            }
            Some(Shown::Failed(err)) => (1, err.clone()),
            Some(Shown::Ready(overview)) => {
                data.set_show(true);
                data.set_body_kind(2);
                data.set_message("".into());
                self.mods_fill(&data, &state, id, overview);
                return;
            }
        };
        data.set_show(true);
        data.set_body_kind(kind);
        data.set_message(message.into());
        self.mods.rows.set_vec(Vec::new());
        self.mods.plugins.set_vec(Vec::new());
        self.mods.profiles.set_vec(Vec::new());
    }

    /// The card's body, from the mods as they stand.
    fn mods_fill(&self, data: &ModsData<'_>, state: &ModsState, id: &str, overview: &Overview) {
        let free = state.free();
        let running = self.game.running.borrow().is_running(id);
        data.set_nexus(overview.nexus.is_some());

        let profiles: Vec<ChipData> = overview
            .profiles
            .iter()
            .enumerate()
            .map(|(index, name)| {
                chip(
                    i32::try_from(index).unwrap_or(i32::MAX),
                    name.clone(),
                    *name == overview.profile,
                )
            })
            .collect();
        refill(&self.mods.profiles, profiles);
        let asking = state.asking(id, &Confirm::DeleteProfile(overview.profile.clone()));
        data.set_delete_profile_label(shell::delete_label(asking).into());
        data.set_delete_profile_asking(asking);
        data.set_delete_profile_on(shell::profile_delete_enabled(free, overview));
        if let Some(naming) = state.naming(id) {
            data.set_naming(true);
            data.set_name_placeholder(shell::name_placeholder(naming.renaming.is_some()).into());
            data.set_name(naming.name.clone().into());
            data.set_name_ok(free && !naming.name.trim().is_empty());
        } else {
            data.set_naming(false);
            data.set_name("".into());
        }

        data.set_place(shell::place_line(overview).into());
        data.set_target(overview.target.display().to_string().into());
        let last = overview.mods.len().saturating_sub(1);
        let rows: Vec<ModRow> = overview
            .mods
            .iter()
            .enumerate()
            .map(|(index, info)| {
                let asking = state.asking(id, &Confirm::Remove(info.id.clone()));
                ModRow {
                    name: info.name.clone().into(),
                    facts: shell::mod_facts(info).into(),
                    enabled: info.enabled,
                    up: index > 0,
                    down: index < last,
                    delete_label: shell::delete_label(asking).into(),
                    asking,
                    update: self.nexus_update_label(id, &info.id).into(),
                    options: if info.installer {
                        t!("mods-options").into()
                    } else {
                        "".into()
                    },
                }
            })
            .collect();
        refill(&self.mods.rows, rows);

        self.mods_fill_plugins(data, overview);

        let asking = state.asking(id, &Confirm::ClearOverwrite);
        data.set_overwrite(if overview.overwrite > 0 {
            t!("mods-overwrite", count = overview.overwrite).into()
        } else {
            "".into()
        });
        data.set_overwrite_label(shell::delete_label(asking).into());
        data.set_overwrite_asking(asking);

        let typed = state.typed_folder(id);
        data.set_folder(typed.unwrap_or(&overview.folder).into());
        data.set_folder_editable(shell::folder_editable(overview));
        data.set_folder_save(shell::folder_save_shown(typed, overview));
        data.set_folder_known(shell::known_folder_shown(overview));

        let (words, tone) = shell::apply_state(overview, running);
        data.set_state(words.into());
        data.set_state_tone(match tone {
            Tone::Dim => 0,
            Tone::Good => 1,
            Tone::Bad => 2,
        });
        data.set_can_apply(shell::can_apply(free, running, overview));
        data.set_can_restore(shell::can_restore(free, running, overview));
    }

    fn mods_fill_plugins(&self, data: &ModsData<'_>, overview: &Overview) {
        let Some(found) = &overview.plugins else {
            data.set_plugins_show(false);
            self.mods.plugins.set_vec(Vec::new());
            return;
        };
        let (note, bad) = shell::plugins_note(found, shell::plugins_style(overview));
        data.set_plugins_show(true);
        data.set_plugins_note(note.into());
        data.set_plugins_note_bad(bad);
        data.set_plugins_file(
            found
                .file
                .as_ref()
                .map(|file| file.display().to_string())
                .unwrap_or_default()
                .into(),
        );
        let rows: Vec<PluginRow> = found
            .list
            .iter()
            .enumerate()
            .map(|(index, plugin)| PluginRow {
                name: plugin.name.clone().into(),
                facts: shell::plugin_facts(plugin).into(),
                problems: shell::plugin_problems(plugin).into(),
                enabled: plugin.enabled,
                locked: plugin.rank == Rank::Implicit,
                up: plugins::can_move(&found.list, index, true),
                down: plugins::can_move(&found.list, index, false),
            })
            .collect();
        refill(&self.mods.plugins, rows);
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::mods::{self, Applied, ModInfo};
    use zendo_core::plugins::Rank;

    use super::super::tests::page;
    use super::*;
    use crate::{GameData, LibraryData};

    fn overview(applied: Applied) -> Overview {
        Overview {
            game_dir: "/games/Cat Quest".into(),
            target: "/games/Cat Quest".into(),
            folder: String::new(),
            custom_folder: false,
            known: None,
            nexus: Some("catquest".into()),
            profile: mods::DEFAULT_PROFILE.into(),
            profiles: vec![mods::DEFAULT_PROFILE.into(), "Light".into()],
            mods: ["one", "two"]
                .iter()
                .map(|name| ModInfo {
                    id: (*name).into(),
                    name: (*name).into(),
                    enabled: true,
                    files: 3,
                    nexus: None,
                    installer: false,
                    wins: Vec::new(),
                    loses: Vec::new(),
                })
                .collect(),
            overwrite: 2,
            applied,
            plugins: None,
        }
    }

    fn read(glue: &Glue, id: &str, overview: Option<Overview>) {
        glue.mods_arrived(id, Ok(Outcome::Read(overview)));
    }

    #[test]
    fn the_card_shows_what_was_read_and_hides_with_the_page() {
        let (window, glue) = page(2);
        let data = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        assert!(!data.get_show(), "nothing read, no card");

        read(&glue, "desktop:0", Some(overview(Applied::No)));
        assert!(data.get_show());
        assert_eq!(data.get_body_kind(), 2);
        assert!(data.get_nexus());
        assert_eq!(data.get_mods().row_count(), 2);
        assert_eq!(data.get_profiles().row_count(), 2);
        assert!(data.get_overwrite().contains("wrote while modded: 2"));
        assert!(data.get_can_apply() && !data.get_can_restore());
        assert!(data.get_folder_editable());

        // A game with no place for mods has no card.
        read(&glue, "desktop:0", None);
        assert!(!data.get_show());
        assert_eq!(data.get_mods().row_count(), 0);

        read(&glue, "desktop:0", Some(overview(Applied::No)));
        assert!(window.global::<GameData>().get_open());
        window.global::<LibraryData>().invoke_back();
        assert!(!data.get_show());
        assert_eq!(data.get_mods().row_count(), 0, "the next page starts empty");
        assert_eq!(data.get_profiles().row_count(), 0);
    }

    #[test]
    fn a_failed_read_says_why() {
        let (window, glue) = page(1);
        let data = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        glue.mods.state.borrow_mut().open("desktop:0");
        glue.mods_arrived("desktop:0", Err("no disk".into()));
        assert!(data.get_show());
        assert_eq!(data.get_body_kind(), 1);
        assert_eq!(data.get_message(), "no disk");
    }

    #[test]
    fn deleting_asks_first_and_naming_keeps_what_is_typed() {
        let (window, glue) = page(1);
        let data = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        read(&glue, "desktop:0", Some(overview(Applied::No)));

        assert_eq!(data.get_mods().row_data(0).unwrap().delete_label, "Delete");
        data.invoke_remove(0);
        let row = data.get_mods().row_data(0).unwrap();
        assert!(row.asking);
        assert_eq!(row.delete_label, "Really delete");
        assert_eq!(data.get_mods().row_data(1).unwrap().delete_label, "Delete");

        data.invoke_delete_profile();
        assert!(data.get_delete_profile_asking());
        data.invoke_clear_overwrite();
        assert!(data.get_overwrite_asking());

        data.invoke_new_profile();
        assert!(data.get_naming());
        assert!(!data.get_name_ok());
        data.invoke_name_typed("Run 2".into());
        glue.show_game();
        assert_eq!(data.get_name(), "Run 2");
        assert!(data.get_name_ok());
        data.invoke_cancel_name();
        assert!(!data.get_naming());

        data.invoke_rename_profile();
        assert_eq!(data.get_name(), "Default");
        assert_eq!(
            data.get_name_placeholder(),
            shell::name_placeholder(true).as_str()
        );
    }

    #[test]
    fn a_finished_change_says_so_and_a_game_that_runs_cannot_apply() {
        let (window, glue) = page(1);
        let data = window.global::<ModsData>();
        let library = window.global::<LibraryData>();
        library.invoke_card_clicked("desktop:0".into());
        read(&glue, "desktop:0", Some(overview(Applied::No)));
        data.invoke_apply();
        assert!(!data.get_state().is_empty());

        let done = overview(Applied::Current(2));
        let note = shell::applied_note(&done, "Game 00");
        glue.mods_arrived("desktop:0", Ok(Outcome::Changed(done, note)));
        assert_eq!(data.get_state(), "Applied: 2 mods in the game");
        assert_eq!(data.get_state_tone(), 1);
        assert!(data.get_can_restore());
        assert!(!data.get_folder_editable(), "nothing to type while applied");
        assert_eq!(
            glue.browser.borrow().status(),
            Some("Applied 2 mods to Game 00")
        );
    }

    #[test]
    fn plugins_show_with_a_note_and_the_ones_the_game_loads_are_locked() {
        let (window, glue) = page(1);
        let data = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        let mut with = overview(Applied::No);
        let mut list = Vec::new();
        for (name, rank) in [("Skyrim.esm", Rank::Implicit), ("Mod.esp", Rank::Free)] {
            list.push(zendo_core::plugins::Plugin {
                name: name.into(),
                enabled: true,
                from: None,
                master: false,
                rank,
                missing: Vec::new(),
                after: Vec::new(),
                unreadable: false,
            });
        }
        with.plugins = Some(Box::new(mods::Plugins { list, file: None }));
        read(&glue, "desktop:0", Some(with));
        assert!(data.get_plugins_show() && data.get_plugins_note_bad());
        let rows = data.get_plugins();
        assert_eq!(rows.row_count(), 2);
        assert!(rows.row_data(0).unwrap().locked);
        assert!(!rows.row_data(1).unwrap().locked);
    }
}
