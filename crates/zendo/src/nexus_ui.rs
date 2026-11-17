//! Nexus Mods in the window: the key and the link handler on the Settings
//! page, `nxm://` links (which game one is for, the sheet that asks when
//! it cannot tell, the download into that game's mods), the downloads and
//! Update on the Mods card, and the newer files Nexus has for a game's mods.
//! The state, the transitions and the words are `zendo_shell::nexus`'; the
//! work that waits on the network runs on a thread. A part of the Library's
//! glue (`library_ui`).
//! Search Nexus is `search_ui.rs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use zendo_core::nexus::{self, KeyCheck, Link};
use zendo_core::secrets::Key;
use zendo_shell::nexus::{
    self as shell, Arrived, Checked, Finished, KeyStep, Matched, Matching, NexusState, Update,
    UpdateList,
};
use zendo_shell::t;

use super::settings::{Setting, kept_text};
use super::{Glue, with_glue};
use crate::{DownloadRow, DownloadsData, MainWindow, ModsData, NexusData};

/// How often a download's progress reaches the window: more would only
/// redraw the card for nothing.
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

/// What the window remembers of Nexus, and the models its rows come from.
pub(super) struct NexusUiState {
    pub(super) state: RefCell<NexusState>,
    /// The settings were read and the library scanned: links can be taken.
    ready: Cell<bool>,
    downloads: Rc<VecModel<DownloadRow>>,
    /// Every download, for the Downloads page's card.
    all_downloads: Rc<VecModel<DownloadRow>>,
    choices: Rc<VecModel<SharedString>>,
}

impl NexusUiState {
    pub(super) fn new() -> Self {
        Self {
            state: RefCell::default(),
            ready: Cell::new(false),
            downloads: Rc::new(VecModel::default()),
            all_downloads: Rc::new(VecModel::default()),
            choices: Rc::new(VecModel::default()),
        }
    }
}

impl Glue {
    /// Hooks the Settings rows, the chooser and the card's downloads up.
    pub(super) fn connect_nexus(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<NexusData>();
        data.set_choices(ModelRc::from(Rc::clone(&self.nexus.choices)));
        window
            .global::<ModsData>()
            .set_downloads(ModelRc::from(Rc::clone(&self.nexus.downloads)));

        let glue = Rc::clone(self);
        data.on_key_edited(move |typed| {
            glue.nexus.state.borrow_mut().edit_key(typed.to_string());
            glue.show_nexus_settings();
        });
        let glue = Rc::clone(self);
        data.on_key_toggle_shown(move || {
            glue.nexus.state.borrow_mut().toggle_shown();
            glue.show_nexus_settings();
        });
        let glue = Rc::clone(self);
        data.on_key_check(move || {
            let step = glue.nexus.state.borrow_mut().check_key(glue.persist);
            glue.nexus_key_step(step);
        });
        let glue = Rc::clone(self);
        data.on_key_save(move || {
            let step = glue.nexus.state.borrow_mut().save_key(glue.persist);
            glue.nexus_key_step(step);
        });
        let glue = Rc::clone(self);
        data.on_key_get(move || glue.open_target(nexus::KEY_PAGE));
        let glue = Rc::clone(self);
        data.on_become_handler(move || glue.nexus_become_handler());
        let glue = Rc::clone(self);
        data.on_choose(move |index| glue.nexus_chosen(index));
        let glue = Rc::clone(self);
        data.on_cancel_choose(move || {
            glue.nexus_back();
        });

        let mods = window.global::<ModsData>();
        let glue = Rc::clone(self);
        mods.on_stop_download(move |id| glue.nexus_stop(id));
        let glue = Rc::clone(self);
        let downloads = window.global::<DownloadsData>();
        downloads.set_mods_rows(ModelRc::from(Rc::clone(&self.nexus.all_downloads)));
        downloads.on_stop_mod(move |id| glue.nexus_stop(id));
        let glue = Rc::clone(self);
        mods.on_update(move |index| glue.nexus_update(index));
    }

    /// Stop on a download, on the card or on the Downloads page.
    fn nexus_stop(&self, id: i32) {
        if let Ok(id) = u64::try_from(id) {
            self.nexus.state.borrow_mut().stop(id);
            self.show_nexus_downloads();
        }
    }

    /// Runs `work` on a thread and hands its answer to `arrived` on the
    /// window's thread, with the glue itself (a change of setting needs it).
    fn ask_rc<R: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<R, String> + Send + 'static,
        arrived: impl FnOnce(&Rc<Self>, Result<R, String>) + Send + 'static,
    ) {
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|payload| Err(super::panicked_note(&*payload)));
            let _ = weak.upgrade_in_event_loop(move |_| with_glue(|glue| arrived(glue, result)));
        });
    }

    pub(super) fn nexus_key(&self) -> Option<String> {
        self.config
            .borrow()
            .as_ref()
            .and_then(|config| config.nexus_key.clone())
    }

    /// The first scan is in: the field shows the stored key, Nexus is asked
    /// whose it is and what opens links, and the links that waited are taken.
    pub(super) fn nexus_scanned(self: &Rc<Self>) {
        if self.nexus.ready.replace(true) {
            return;
        }
        let key = self.nexus_key();
        self.nexus.state.borrow_mut().loaded(key.as_deref());
        if self.persist {
            self.ask_rc(
                || Ok(nexus::handler()),
                |glue, handler| {
                    if let Ok(handler) = handler {
                        glue.nexus.state.borrow_mut().handler_known(handler);
                        glue.show_nexus_settings();
                    }
                },
            );
            if let Some(key) = key {
                let asked = key.clone();
                self.ask_rc(
                    move || shell::check_key(&key, true),
                    move |glue, result| glue.nexus_key_checked(&asked, result),
                );
            }
        }
        self.show_nexus_settings();
        let pending = std::mem::take(&mut self.nexus.state.borrow_mut().pending);
        for link in pending {
            self.nexus_link(&link);
        }
    }

    // The Settings page's rows.

    /// Writes the Settings page's Nexus rows from the state.
    pub(super) fn show_nexus_settings(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<NexusData>();
        let state = self.nexus.state.borrow();
        let config = self.settings_config();
        let saved = config.nexus_key.as_deref().unwrap_or("");
        let key = &state.key;
        if data.get_key_value() != key.value.as_str() {
            data.set_key_value(key.value.as_str().into());
        }
        data.set_key_shown(key.shown);
        data.set_key_checking(key.checking.is_some());
        data.set_key_changed(key.value.trim() != saved);
        let (note, tone) = match (&key.note, &state.user) {
            (Some((note, good)), _) => (note.clone(), if *good { 1 } else { 2 }),
            (None, _) if saved.is_empty() => (t!("key-none"), 3),
            (None, Some(user)) => (shell::signed_in(user), 1),
            (None, None) => (String::new(), 0),
        };
        data.set_key_note(note.into());
        data.set_key_tone(tone);
        let (kept, bad) = kept_text(config.kept(Key::Nexus));
        data.set_key_kept(kept.into());
        data.set_key_kept_bad(bad);
        let (line, good, can) = state.handler_row();
        data.set_handler_line(line.into());
        data.set_handler_good(good);
        data.set_handler_can(can);
    }

    /// What a press on Check or Save leaves to do.
    fn nexus_key_step(self: &Rc<Self>, step: KeyStep) {
        match step {
            KeyStep::Nothing => {}
            KeyStep::Remove => self.save_setting(Setting::NexusKey {
                key: None,
                then: shell::Then::Removed,
            }),
            KeyStep::Check(key) => {
                let asked = key.clone();
                self.ask_rc(
                    move || shell::check_key(&key, false),
                    move |glue, result| glue.nexus_key_checked(&asked, result),
                );
            }
        }
        self.show_nexus_settings();
    }

    /// Nexus answered about a key.
    fn nexus_key_checked(self: &Rc<Self>, key: &str, result: Result<KeyCheck, String>) {
        let stored = self.nexus_key();
        let checked = self
            .nexus
            .state
            .borrow_mut()
            .key_checked(key, result, stored.as_deref());
        if let Checked::Save(key, then) = checked {
            self.save_setting(Setting::NexusKey {
                key: Some(key),
                then,
            });
        }
        self.show_nexus_settings();
    }

    /// The key is in the file: the page says what became of it.
    pub(super) fn nexus_key_stored(&self, then: &shell::Then) {
        self.nexus.state.borrow_mut().key_stored(then.clone());
        self.show_nexus_settings();
    }

    fn nexus_become_handler(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        self.ask_rc(shell::become_handler, |glue, result| {
            let note = glue.nexus.state.borrow_mut().became_handler(result);
            glue.say(note);
            glue.show_nexus_settings();
        });
    }

    // A link from the browser.

    /// A link: which game it is for is asked, then its file comes down.
    pub(super) fn nexus_link(&self, text: &str) {
        let key = self.nexus_key();
        let ready = self.nexus.ready.get();
        let arrived = self
            .nexus
            .state
            .borrow_mut()
            .link_arrived(text, ready, key.is_some());
        let (link, note) = match arrived {
            Arrived::Say(note) => {
                self.say(note);
                return;
            }
            Arrived::Wait => return,
            Arrived::Ask(link, note) => (link, note),
        };
        let Some(key) = key else {
            return;
        };
        if !self.persist {
            return;
        }
        let games = shell::with_folders(self.browser.borrow().games());
        self.say(note);
        self.ask(
            move || Ok(shell::match_games(&key, &link, &games)),
            Self::nexus_matched,
        );
    }

    fn nexus_matched(&self, found: Result<Matched, String>) {
        let found = match found {
            Ok(found) => found,
            Err(err) => {
                self.say(err);
                return;
            }
        };
        let matching = self.nexus.state.borrow_mut().matched(found);
        match matching {
            Matching::Download(id, link) => self.nexus_download(&id, link, false),
            Matching::Say(note) => self.say(note),
            Matching::Choose => self.show_chooser(),
        }
    }

    // The chooser.

    pub(super) fn nexus_choosing(&self) -> bool {
        self.nexus.state.borrow().choosing.is_some()
    }

    /// Escape, and Cancel: closes the chooser. Says whether one was open.
    pub(super) fn nexus_back(&self) -> bool {
        if !self.nexus_choosing() {
            return false;
        }
        self.nexus.state.borrow_mut().cancel_choose();
        self.show_chooser();
        self.focus_page();
        true
    }

    fn nexus_chosen(&self, index: i32) {
        let id = {
            let state = self.nexus.state.borrow();
            usize::try_from(index)
                .ok()
                .and_then(|index| state.choosing.as_ref()?.games.get(index))
                .map(|(id, _)| id.clone())
        };
        let Some(id) = id else {
            return;
        };
        let chosen = self.nexus.state.borrow_mut().choose(&id);
        self.show_chooser();
        self.focus_page();
        if let Some((id, link, remember)) = chosen {
            self.nexus_download(&id, link, remember);
        }
    }

    fn show_chooser(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<NexusData>();
        let state = self.nexus.state.borrow();
        let Some(choosing) = state.choosing.as_ref() else {
            data.set_choosing(false);
            self.nexus.choices.set_vec(Vec::new());
            return;
        };
        let about = if choosing.all {
            t!("nexus-choose-none", name = choosing.name.as_str())
        } else {
            t!("nexus-choose-several", name = choosing.name.as_str())
        };
        data.set_choose_about(about.into());
        self.nexus.choices.set_vec(
            choosing
                .games
                .iter()
                .map(|(_, title)| title.as_str().into())
                .collect::<Vec<SharedString>>(),
        );
        data.set_choosing(true);
    }

    // Downloads.

    /// Downloads the link's file into a game's mods, on a thread that says
    /// how far it got now and then.
    pub(super) fn nexus_download(&self, id: &str, link: Link, remember: bool) {
        let (Some(game), Some(key)) = (self.browser.borrow().game(id).cloned(), self.nexus_key())
        else {
            return;
        };
        let started = self.nexus.state.borrow_mut().begin(&game, &link);
        let Some(started) = started else {
            return;
        };
        self.say(started.note);
        self.show_nexus_downloads();
        self.show_game();
        if !self.persist {
            return;
        }
        let (number, stop) = (started.id, started.stop);
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let progress_weak = weak.clone();
            let mut last: Option<Instant> = None;
            let mut progress = move |done: u64, total: Option<u64>| {
                if last.is_some_and(|last| last.elapsed() < PROGRESS_EVERY) {
                    return;
                }
                last = Some(Instant::now());
                let _ = progress_weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.nexus_progress(number, done, total));
                });
            };
            let game_id = game.id.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                shell::fetch(&game, &key, &link, remember, &stop, &mut progress)
            }))
            .unwrap_or_else(|payload| Err(super::panicked_note(&*payload)));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.nexus_finished(number, &game_id, result));
            });
        });
    }

    fn nexus_progress(&self, id: u64, done: u64, total: Option<u64>) {
        self.nexus.state.borrow_mut().progress(id, done, total);
        self.show_nexus_downloads();
    }

    fn nexus_finished(
        &self,
        id: u64,
        game: &str,
        result: Result<(zendo_core::mods::Added, String), String>,
    ) {
        let finished = self.nexus.state.borrow_mut().finished(id, game, result);
        match finished {
            Finished::Choose(pending) => self.installer_open(game.to_owned(), pending),
            Finished::Done(overview, note) => {
                self.say(note);
                self.mods_ready(game, *overview);
                self.nexus_want_updates(game);
            }
            Finished::Say(note) => self.say(note),
        }
        self.show_nexus_downloads();
        self.show_game();
        // The sheet marks the mods that are in now.
        self.show_search();
    }

    /// Writes the open game's downloads for the card: row by row when the
    /// count is the same, so the card does not move.
    pub(super) fn show_nexus_downloads(&self) {
        self.show_mods_downloads_card();
        let Some(id) = self.open.borrow().clone() else {
            self.nexus.downloads.set_vec(Vec::new());
            return;
        };
        let state = self.nexus.state.borrow();
        let rows: Vec<DownloadRow> = state.downloads_for(&id).map(download_row).collect();
        drop(state);
        super::mods::refill(&self.nexus.downloads, rows);
    }

    /// The Downloads page's card: every mod coming in, for any game. When
    /// the first one starts or the last one ends the page is drawn again, for
    /// its invitation to the Store.
    fn show_mods_downloads_card(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let rows: Vec<DownloadRow> = self
            .nexus
            .state
            .borrow()
            .downloads
            .iter()
            .map(download_row)
            .collect();
        let show = !rows.is_empty();
        super::mods::refill(&self.nexus.all_downloads, rows);
        let data = window.global::<DownloadsData>();
        if data.get_mods_show() != show {
            data.set_mods_show(show);
            self.show_downloads();
        }
    }

    /// Whether a mod is coming in from Nexus, for any game.
    pub(super) fn nexus_downloading(&self) -> bool {
        !self.nexus.state.borrow().downloads.is_empty()
    }

    // Updates.

    /// Asks Nexus, once a session, for the newer files of a game's mods.
    pub(super) fn nexus_want_updates(&self, game: &str) {
        let Some(overview) = self.mods_overview(game) else {
            return;
        };
        let key = self.nexus_key();
        let job = self.nexus.state.borrow_mut().want_updates(
            game,
            &overview,
            key.is_some(),
            self.persist && self.browser.borrow().game(game).is_some(),
        );
        let (Some(job), Some(key)) = (job, key) else {
            return;
        };
        let game = job.game;
        self.ask(
            move || Ok(shell::ask_updates(&key, job.sources)),
            move |glue, result| glue.nexus_updates(&game, result),
        );
    }

    fn nexus_updates(&self, game: &str, result: Result<UpdateList, String>) {
        let note = self.nexus.state.borrow_mut().updates_found(game, result);
        if let Some(note) = note {
            self.say(note);
        }
        self.show_game();
    }

    /// The label of Update on a mod; empty when Nexus has no newer file.
    pub(super) fn nexus_update_label(&self, game: &str, mod_id: &str) -> String {
        self.nexus
            .state
            .borrow()
            .update_for(game, mod_id)
            .map_or_default(shell::update_label)
    }

    /// Update on a mod of the open game: downloaded straight away with
    /// Premium; otherwise the file's page opens, for the site's button to
    /// send it back.
    fn nexus_update(&self, index: i32) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let Some(overview) = self.mods_overview(&id) else {
            return;
        };
        let Some(mod_id) = usize::try_from(index)
            .ok()
            .and_then(|index| overview.mods.get(index))
            .map(|info| info.id.clone())
        else {
            return;
        };
        let update = self.nexus.state.borrow().update(&id, &mod_id, &overview);
        match update {
            Some(Update::Download(link)) => self.nexus_download(&id, link, false),
            Some(Update::Page(url, note)) => {
                self.say(note);
                self.open_target(&url);
            }
            None => {}
        }
    }

    /// Whether the key's account has Premium (false until Nexus said).
    pub(super) fn nexus_premium(&self) -> bool {
        self.nexus.state.borrow().premium() == Some(true)
    }

    /// The page closed: the card's rows go with it.
    pub(super) fn nexus_clear(&self) {
        self.nexus.downloads.set_vec(Vec::new());
    }
}

/// A download as the card draws it.
fn download_row(download: &zendo_shell::nexus::Download) -> DownloadRow {
    let (fraction, words) = download.progress();
    DownloadRow {
        id: i32::try_from(download.id).unwrap_or(i32::MAX),
        name: download.name.as_str().into(),
        words: words.into(),
        fraction,
        stopping: download.stopping(),
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::config::Config;
    use zendo_core::mods::{self, Applied, ModInfo, NexusSource, Overview};
    use zendo_core::nexus::{FileInfo, User};
    use zendo_shell::mods::Outcome;

    use super::super::tests::page;
    use super::*;
    use crate::LibraryData;

    fn overview() -> Overview {
        Overview {
            game_dir: "/games/Cat Quest".into(),
            target: "/games/Cat Quest".into(),
            folder: String::new(),
            custom_folder: false,
            known: None,
            nexus: Some("catquest".into()),
            profile: mods::DEFAULT_PROFILE.into(),
            profiles: vec![mods::DEFAULT_PROFILE.into()],
            mods: [("one", Some(5)), ("two", None)]
                .iter()
                .map(|(name, file)| ModInfo {
                    id: (*name).into(),
                    name: (*name).into(),
                    enabled: true,
                    files: 1,
                    nexus: file.map(|file_id| NexusSource {
                        domain: "catquest".into(),
                        mod_id: 9,
                        file_id,
                        version: "1".into(),
                    }),
                    installer: false,
                    wins: Vec::new(),
                    loses: Vec::new(),
                })
                .collect(),
            overwrite: 0,
            applied: Applied::No,
            plugins: None,
        }
    }

    fn newer(file_id: u64) -> FileInfo {
        FileInfo {
            file_id,
            name: "New".into(),
            version: Some("2".into()),
            file_name: "a.zip".into(),
            size_kb: None,
            category_name: None,
            uploaded_timestamp: 0,
        }
    }

    fn user() -> User {
        User {
            name: "someone".into(),
            is_premium: false,
        }
    }

    fn with_key(glue: &Glue, key: &str) {
        let mut config = Config::default();
        config.nexus_key = Some(key.into());
        *glue.config.borrow_mut() = Some(config);
    }

    #[test]
    fn a_key_is_asked_about_first_and_written_only_when_accepted() {
        let (window, glue) = page(1);
        let data = window.global::<NexusData>();

        data.set_key_value("good".into());
        data.invoke_key_edited("good".into());
        assert!(data.get_key_changed(), "a key typed and not saved");
        data.invoke_key_save();
        assert!(
            data.get_key_checking(),
            "Nexus is asked before it is written"
        );
        assert_eq!(glue.nexus_key(), None);

        glue.nexus_key_checked("good", Ok(KeyCheck::Rejected));
        assert!(!data.get_key_checking());
        assert_eq!(data.get_key_tone(), 2);
        assert_eq!(glue.nexus_key(), None, "a rejected key is not kept");

        data.invoke_key_save();
        glue.nexus_key_checked("good", Ok(KeyCheck::Accepted(user())));
        assert_eq!(glue.nexus_key().as_deref(), Some("good"));
        assert_eq!(data.get_key_tone(), 1);
        assert!(data.get_key_note().contains("someone"));
        assert!(!data.get_key_changed());

        // An empty field removes it.
        data.invoke_key_edited("".into());
        data.invoke_key_save();
        assert_eq!(glue.nexus_key(), None);
    }

    #[test]
    fn the_chooser_opens_for_a_link_no_game_matched_and_escape_closes_it() {
        let (window, glue) = page(2);
        let data = window.global::<NexusData>();
        let link = Link::parse("nxm://catquest/mods/1/files/2").unwrap();
        glue.nexus_matched(Ok(Matched {
            link,
            matched: Vec::new(),
            installed: vec![
                ("desktop:0".into(), "Game 0".into()),
                ("desktop:1".into(), "Game 1".into()),
            ],
            name: "Cat Quest".into(),
        }));
        assert!(data.get_choosing());
        assert_eq!(data.get_choices().row_count(), 2);
        assert!(glue.nexus_choosing());

        window.global::<LibraryData>().invoke_back();
        assert!(!data.get_choosing());
        assert_eq!(data.get_choices().row_count(), 0);
    }

    #[test]
    fn choosing_a_game_starts_its_download_on_the_card_and_stop_marks_it() {
        let (window, glue) = page(2);
        with_key(&glue, "k");
        let data = window.global::<NexusData>();
        let mods = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        let link = Link::parse("nxm://catquest/mods/1/files/2").unwrap();
        glue.nexus_matched(Ok(Matched {
            link,
            matched: Vec::new(),
            installed: vec![("desktop:0".into(), "Game 0".into())],
            name: "Cat Quest".into(),
        }));
        data.invoke_choose(0);
        assert!(!data.get_choosing());
        assert_eq!(mods.get_downloads().row_count(), 1);
        let row = mods.get_downloads().row_data(0).unwrap();
        assert!(!row.stopping);

        mods.invoke_stop_download(row.id);
        assert!(mods.get_downloads().row_data(0).unwrap().stopping);

        // The same file again is not started twice.
        glue.nexus_download(
            "desktop:0",
            Link::parse("nxm://catquest/mods/1/files/2").unwrap(),
            false,
        );
        assert_eq!(mods.get_downloads().row_count(), 1);

        // The page closes: nothing of it stays.
        window.global::<LibraryData>().invoke_back();
        assert_eq!(mods.get_downloads().row_count(), 0);
    }

    /// A download of file 2 into the first game, started as a choice would.
    fn download_started(glue: &Rc<Glue>, window: &MainWindow) -> i32 {
        with_key(glue, "k");
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        glue.nexus_download(
            "desktop:0",
            Link::parse("nxm://catquest/mods/1/files/2").unwrap(),
            false,
        );
        window
            .global::<ModsData>()
            .get_downloads()
            .row_data(0)
            .expect("a row")
            .id
    }

    #[test]
    fn the_downloads_page_lists_every_mod_coming_in_and_stop_works_there() {
        let (window, glue) = page(2);
        let downloads = window.global::<DownloadsData>();
        assert!(!downloads.get_mods_show());
        glue.show_downloads();
        assert!(downloads.get_invite(), "nothing going on: the Store invite");

        let id = download_started(&glue, &window);
        glue.nexus_download(
            "desktop:1",
            Link::parse("nxm://catquest/mods/3/files/4").unwrap(),
            false,
        );
        assert!(downloads.get_mods_show());
        assert_eq!(downloads.get_mods_rows().row_count(), 2, "both games'");
        assert_eq!(
            window.global::<ModsData>().get_downloads().row_count(),
            1,
            "the card on a game's page keeps to that game"
        );
        assert!(!downloads.get_invite(), "something is coming in");
        assert!(glue.nexus_downloading());

        downloads.invoke_stop_mod(id);
        assert!(
            downloads
                .get_mods_rows()
                .iter()
                .any(|row| row.id == id && row.stopping)
        );

        let id = u64::try_from(id).unwrap();
        glue.nexus_finished(id, "desktop:0", Err("cut".into()));
        assert_eq!(downloads.get_mods_rows().row_count(), 1);
        let other = u64::try_from(downloads.get_mods_rows().row_data(0).unwrap().id).unwrap();
        glue.nexus_finished(other, "desktop:1", Err("cut".into()));
        assert!(!downloads.get_mods_show(), "the card goes with the last");
        assert!(downloads.get_invite());
    }

    #[test]
    fn progress_moves_the_bar_and_a_finished_download_leaves_the_mods_read() {
        let (window, glue) = page(1);
        let mods = window.global::<ModsData>();
        let id = download_started(&glue, &window);
        assert_eq!(mods.get_downloads().row_data(0).unwrap().fraction, 0.0);

        let mib = 1024 * 1024;
        glue.nexus_progress(u64::try_from(id).unwrap(), 5 * mib, Some(10 * mib));
        let row = mods.get_downloads().row_data(0).unwrap();
        assert!((row.fraction - 0.5).abs() < 0.001);
        assert!(row.words.contains("5.0"), "{}", row.words);
        assert_eq!(mods.get_downloads().row_count(), 1, "the same row, updated");

        glue.nexus_finished(
            u64::try_from(id).unwrap(),
            "desktop:0",
            Ok((
                mods::Added::Done(Box::new(overview())),
                "Added it".to_owned(),
            )),
        );
        assert_eq!(mods.get_downloads().row_count(), 0);
        assert_eq!(
            mods.get_mods().row_count(),
            2,
            "the card shows the new mods"
        );
        assert_eq!(window.global::<LibraryData>().get_status(), "Added it");
    }

    #[test]
    fn a_failed_or_stopped_download_says_so_and_leaves_no_row() {
        let (window, glue) = page(1);
        let mods = window.global::<ModsData>();
        let id = u64::try_from(download_started(&glue, &window)).unwrap();
        glue.nexus_finished(id, "desktop:0", Err("offline".into()));
        assert_eq!(mods.get_downloads().row_count(), 0);
        assert!(
            window
                .global::<LibraryData>()
                .get_status()
                .contains("offline")
        );

        let id = u64::try_from(download_started(&glue, &window)).unwrap();
        mods.invoke_stop_download(i32::try_from(id).unwrap());
        glue.nexus_finished(id, "desktop:0", Err("cut".into()));
        assert!(
            window
                .global::<LibraryData>()
                .get_status()
                .contains("stopped"),
            "a stop is not a failure"
        );
    }

    #[test]
    fn update_downloads_with_premium_and_otherwise_sends_to_the_site() {
        let (window, glue) = page(1);
        with_key(&glue, "k");
        let mods = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        glue.mods_arrived("desktop:0", Ok(Outcome::Read(Some(overview()))));
        let _ = glue.nexus.state.borrow_mut().updates_found(
            "desktop:0",
            Ok((vec![("one".to_owned(), newer(6))], Vec::new())),
        );
        glue.show_game();

        // A free account is sent to the file's page; nothing is downloaded.
        mods.invoke_update(0);
        assert_eq!(mods.get_downloads().row_count(), 0);
        assert!(
            window
                .global::<LibraryData>()
                .get_status()
                .contains("Mod manager"),
            "{}",
            window.global::<LibraryData>().get_status()
        );

        // Premium downloads the newer file straight away.
        glue.nexus.state.borrow_mut().user = Some(User {
            name: "someone".into(),
            is_premium: true,
        });
        mods.invoke_update(0);
        assert_eq!(mods.get_downloads().row_count(), 1);
        // A mod that Nexus has nothing newer for does nothing.
        mods.invoke_update(1);
        assert_eq!(mods.get_downloads().row_count(), 1);
    }

    #[test]
    fn a_mod_with_a_newer_file_gets_an_update_button() {
        let (window, glue) = page(1);
        let mods = window.global::<ModsData>();
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        glue.mods_arrived("desktop:0", Ok(Outcome::Read(Some(overview()))));
        assert_eq!(mods.get_mods().row_data(0).unwrap().update, "");

        let said = glue.nexus.state.borrow_mut().updates_found(
            "desktop:0",
            Ok((vec![("one".to_owned(), newer(6))], Vec::new())),
        );
        assert!(said.is_none());
        glue.show_game();
        assert_eq!(mods.get_mods().row_data(0).unwrap().update, "Update to 2");
        assert_eq!(
            mods.get_mods().row_data(1).unwrap().update,
            "",
            "no newer file"
        );
    }
}
