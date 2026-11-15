//! Search Nexus: the sheet over a game's page that searches a game's mods on
//! Nexus, shows them a page at a time with their pictures, lists a mod's
//! files, and takes one into the game's mods: straight away with Premium,
//! else through the mod's page in Zendo's Nexus window. The state and the
//! words are `zendo_shell::nexus_search`'; each question to Nexus runs on a
//! thread. A part of the Library's glue (`library_ui`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Image, ModelRc, VecModel};
use zendo_core::mods;
use zendo_core::nexus::{self, Files, Link};
use zendo_core::nexus_search::{self, Found, Order};
use zendo_shell::artwork::WORKERS;
use zendo_shell::nexus_search::{self as shell, FilesState, FilesStep, Search};
use zendo_shell::sign_in_window::{self, FromSite, Handle};
use zendo_shell::t;

use super::mods::refill;
use super::settings::{Setting, chip};
use super::{Glue, with_glue};
use crate::covers::{self as slint_covers, Decoded};
use crate::{ChipData, MainWindow, ModsData, SearchData, SearchRow};

/// The widest a picture is kept: twice what the sheet draws.
const THUMB_WIDTH: u32 = 320;

/// What a row of the sheet stands for, by its index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Mod(u64),
    File(u64, u64),
    Note,
}

/// The sheet's state and the models its rows come from.
pub(super) struct SearchUiState {
    search: RefCell<Option<Search>>,
    keys: RefCell<Vec<RowKey>>,
    rows: Rc<VecModel<SearchRow>>,
    orders: Rc<VecModel<ChipData>>,
    /// The pictures decoded so far, by mod.
    pictures: RefCell<HashMap<u64, Image>>,
    /// Set to stop the pictures of a search on their way.
    stop: RefCell<Arc<AtomicBool>>,
    /// Nexus's window, while a mod's page is open in it.
    site: RefCell<Option<Handle>>,
    /// Counts up to put the keyboard in the field again.
    focus: std::cell::Cell<i32>,
}

impl SearchUiState {
    pub(super) fn new() -> Self {
        Self {
            search: RefCell::new(None),
            keys: RefCell::default(),
            rows: Rc::new(VecModel::default()),
            orders: Rc::new(VecModel::default()),
            pictures: RefCell::default(),
            stop: RefCell::new(Arc::new(AtomicBool::new(false))),
            site: RefCell::new(None),
            focus: std::cell::Cell::new(0),
        }
    }
}

/// An index the window sent, as one to read a list by.
fn at(index: i32) -> Option<usize> {
    usize::try_from(index).ok()
}

impl Glue {
    /// Hooks the sheet's callbacks up and gives it its models.
    pub(super) fn connect_search(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SearchData>();
        data.set_rows(ModelRc::from(Rc::clone(&self.search.rows)));
        data.set_orders(ModelRc::from(Rc::clone(&self.search.orders)));

        let glue = Rc::clone(self);
        window
            .global::<ModsData>()
            .on_search_nexus(move || glue.search_open());
        let glue = Rc::clone(self);
        data.on_close(move || {
            glue.search_back();
        });
        let glue = Rc::clone(self);
        data.on_search(move || glue.search_run(false));
        let glue = Rc::clone(self);
        data.on_words_edited(move |typed| {
            if let Some(search) = glue.search.search.borrow_mut().as_mut() {
                search.words = typed.to_string();
            }
        });
        let glue = Rc::clone(self);
        data.on_order(move |index| {
            let order = at(index).and_then(|index| Order::ALL.get(index).copied());
            if let (Some(order), Some(search)) = (order, glue.search.search.borrow_mut().as_mut()) {
                search.order = order;
            }
            glue.search_run(false);
        });
        let glue = Rc::clone(self);
        data.on_toggle_adult(move || {
            let adult = !glue.settings_config().nexus_adult;
            glue.save_setting(Setting::NexusAdult(adult));
            glue.search_run(false);
        });
        let glue = Rc::clone(self);
        data.on_show_more(move || glue.search_run(true));
        let glue = Rc::clone(self);
        data.on_files(move |row| {
            if let Some(RowKey::Mod(mod_id)) = glue.search_key(row) {
                glue.search_files(mod_id);
            }
        });
        let glue = Rc::clone(self);
        data.on_page(move |row| {
            if let Some(RowKey::Mod(mod_id)) = glue.search_key(row) {
                glue.search_site(mod_id, None);
            }
        });
        let glue = Rc::clone(self);
        data.on_download(move |row| {
            if let Some(RowKey::File(mod_id, file_id)) = glue.search_key(row) {
                glue.search_download(mod_id, file_id);
            }
        });
    }

    fn search_key(&self, row: i32) -> Option<RowKey> {
        self.search.keys.borrow().get(at(row)?).copied()
    }

    /// Whether the sheet is open, so it has the keyboard.
    pub(super) fn search_is_open(&self) -> bool {
        self.search.search.borrow().is_some()
    }

    /// Opens the sheet for the open game, which Nexus knows by a name.
    fn search_open(&self) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let Some(domain) = self
            .mods_overview(&id)
            .and_then(|overview| overview.nexus.clone())
        else {
            return;
        };
        let title = self
            .browser
            .borrow()
            .game(&id)
            .map_or_else(|| domain.clone(), |game| game.title.clone());
        *self.search.search.borrow_mut() = Some(Search::new(&id, title, domain));
        self.search_run(false);
        self.search.focus.set(self.search.focus.get() + 1);
    }

    /// Escape, and Close: closes the sheet. Says whether one was open.
    pub(super) fn search_back(&self) -> bool {
        if self.search.search.borrow_mut().take().is_none() {
            return false;
        }
        self.search_restart_pictures();
        if let Some(open) = self.search.site.borrow_mut().take() {
            open.close();
        }
        self.search.pictures.borrow_mut().clear();
        self.show_search();
        self.focus_page();
        true
    }

    /// A new search or a closed sheet: the pictures still coming stop.
    fn search_restart_pictures(&self) {
        let old = self.search.stop.replace(Arc::new(AtomicBool::new(false)));
        old.store(true, Ordering::Relaxed);
    }

    /// Asks Nexus for the search as it stands: from the start, or the next
    /// page after the results shown.
    fn search_run(&self, more: bool) {
        let adult = self.settings_config().nexus_adult;
        let ask = {
            let mut guard = self.search.search.borrow_mut();
            let Some(search) = guard.as_mut() else {
                return;
            };
            if more {
                search.more(adult)
            } else {
                self.search.pictures.borrow_mut().clear();
                Some(search.restart(adult))
            }
        };
        if !more {
            self.search_restart_pictures();
        }
        self.show_search();
        let Some(ask) = ask else {
            return;
        };
        if !self.persist {
            return;
        }
        let generation = ask.generation;
        let query = ask.query;
        self.ask(
            move || nexus_search::search(&query).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.search_found(generation, result),
        );
    }

    pub(super) fn search_found(&self, generation: u64, result: Result<Found, String>) {
        let wanted = self
            .search
            .search
            .borrow_mut()
            .as_mut()
            .and_then(|search| search.found(generation, result));
        self.show_search();
        let Some(wanted) = wanted.filter(|wanted| !wanted.is_empty() && self.persist) else {
            return;
        };
        let stop = Arc::clone(&self.search.stop.borrow());
        let queue = Arc::new(Mutex::new(wanted.into_iter()));
        for _ in 0..WORKERS {
            let (queue, stop, window) =
                (Arc::clone(&queue), Arc::clone(&stop), self.window.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let next = queue.lock().ok().and_then(|mut queue| queue.next());
                    let Some((id, url)) = next else {
                        return;
                    };
                    let pixels = zendo_core::steamgriddb::thumbnail(&url)
                        .ok()
                        .and_then(|bytes| slint_covers::decode_bytes(&bytes, THUMB_WIDTH));
                    let sent = window.upgrade_in_event_loop(move |_| {
                        with_glue(|glue| glue.search_thumb(generation, id, pixels));
                    });
                    if sent.is_err() {
                        return;
                    }
                }
            });
        }
    }

    /// A picture was downloaded and decoded; its row fills in.
    pub(super) fn search_thumb(&self, generation: u64, id: u64, pixels: Decoded) {
        let current = self
            .search
            .search
            .borrow()
            .as_ref()
            .is_some_and(|search| search.generation == generation);
        let Some(pixels) = pixels.filter(|_| current) else {
            return;
        };
        self.search
            .pictures
            .borrow_mut()
            .insert(id, Image::from_rgba8(pixels));
        self.show_search();
    }

    // The files of a mod.

    fn search_files(&self, mod_id: u64) {
        let key = self.settings_config().nexus_key;
        let askable = key.is_some() && self.persist;
        let (step, domain) = {
            let mut guard = self.search.search.borrow_mut();
            let Some(search) = guard.as_mut() else {
                return;
            };
            (search.toggle_files(mod_id, askable), search.domain.clone())
        };
        self.show_search();
        let (FilesStep::Ask(mod_id), Some(key)) = (step, key) else {
            return;
        };
        self.ask(
            move || nexus::files(&key, &domain, mod_id).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.search_got_files(mod_id, result),
        );
    }

    pub(super) fn search_got_files(&self, mod_id: u64, result: Result<Files, String>) {
        if let Some(search) = self.search.search.borrow_mut().as_mut() {
            search.got_files(mod_id, result);
        }
        self.show_search();
    }

    // Taking a file in.

    /// A file of a mod found: into the game's mods with Premium, else the
    /// file's Mod manager download in Nexus's window.
    fn search_download(&self, mod_id: u64, file_id: u64) {
        let premium = self.nexus_premium();
        if !premium {
            self.search_site(mod_id, Some(file_id));
            return;
        }
        let taken = self
            .search
            .search
            .borrow()
            .as_ref()
            .map(|search| (search.game.clone(), search.link(mod_id, file_id)));
        if let Some((game, link)) = taken {
            self.say(t!("search-downloading"));
            self.nexus_download(&game, link, false);
        }
    }

    /// Opens a mod's page, or one file's Mod manager download, in Nexus's
    /// window; one at a time.
    fn search_site(&self, mod_id: u64, file_id: Option<u64>) {
        let taken = self.search.search.borrow().as_ref().map(|search| {
            (
                search.game.clone(),
                search.domain.clone(),
                shell::site_url(&search.domain, mod_id, file_id),
            )
        });
        let Some((id, domain, url)) = taken else {
            return;
        };
        let Some(game) = self.browser.borrow().game(&id).cloned() else {
            return;
        };
        if let Some(open) = self.search.site.borrow_mut().take() {
            open.close();
        }
        let handle = Handle::default();
        *self.search.site.borrow_mut() = Some(handle.clone());
        self.say(t!("search-site-open"));
        if !self.persist {
            return;
        }
        self.ask(
            move || {
                let folder = mods::downloads_dir(&game).map_err(|err| format!("{err:#}"))?;
                sign_in_window::open_nexus(&url, &folder, &handle).map_err(|err| format!("{err:#}"))
            },
            move |glue, result| glue.search_site_gave(&id, &domain, mod_id, result),
        );
    }

    /// Takes what Nexus's window gave into the game's mods: a link as any
    /// Mod manager download, a file as one from that mod (found by its name
    /// among the mod's files), or as a mod of its own without a key.
    pub(super) fn search_site_gave(
        &self,
        game: &str,
        domain: &str,
        mod_id: u64,
        result: Result<Option<FromSite>, String>,
    ) {
        match result {
            Ok(None) => {
                self.browser.borrow_mut().say(None);
                self.sync_status();
            }
            Ok(Some(FromSite::Link(text))) => match Link::parse(&text) {
                Ok(link) => self.nexus_download(game, link, false),
                Err(err) => self.say(format!("{err:#}")),
            },
            Ok(Some(FromSite::File(archive))) => {
                self.mods_add_from_site(game, archive, domain, mod_id);
            }
            Err(err) => self.say(err),
        }
    }

    // Drawing.

    /// Writes the sheet from the state.
    pub(super) fn show_search(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<SearchData>();
        let guard = self.search.search.borrow();
        let Some(search) = guard.as_ref() else {
            data.set_open(false);
            self.search.rows.set_vec(Vec::new());
            self.search.keys.borrow_mut().clear();
            return;
        };
        let config = self.settings_config();
        let has_key = config.nexus_key.is_some();
        let premium = self.nexus_premium();
        data.set_title(t!("search-title", title = search.title.as_str()).into());
        data.set_adult(config.nexus_adult);
        data.set_limit(
            if !has_key {
                t!("search-needs-key")
            } else if !premium {
                t!("search-not-premium")
            } else {
                String::new()
            }
            .into(),
        );
        refill(
            &self.search.orders,
            Order::ALL
                .iter()
                .enumerate()
                .map(|(index, order)| {
                    chip(
                        i32::try_from(index).unwrap_or(i32::MAX),
                        shell::order_name(*order),
                        search.order == *order,
                    )
                })
                .collect(),
        );
        data.set_more(search.has_more());
        data.set_asking(search.asking);
        let (footer, bad) = search.footer();
        data.set_footer(footer.into());
        data.set_footer_bad(bad);
        data.set_focus_tick(self.search.focus.get());

        let (rows, keys) = self.search_rows(search, has_key, premium);
        debug_assert_eq!(rows.len(), keys.len());
        *self.search.keys.borrow_mut() = keys;
        refill(&self.search.rows, rows);
        data.set_open(true);
    }

    /// The sheet's rows, and what each stands for.
    fn search_rows(
        &self,
        search: &Search,
        has_key: bool,
        premium: bool,
    ) -> (Vec<SearchRow>, Vec<RowKey>) {
        let overview = self.mods_overview(&search.game);
        let (have, have_files) = shell::have(overview.as_ref(), &search.domain);
        let pictures = self.search.pictures.borrow();
        let mut rows = Vec::new();
        let mut keys = Vec::new();
        for hit in &search.hits {
            let picture = pictures.get(&hit.mod_id);
            let open = search
                .open
                .as_ref()
                .filter(|(id, _)| *id == hit.mod_id)
                .map(|(_, state)| state);
            rows.push(SearchRow {
                kind: 0,
                name: hit.name.as_str().into(),
                version: hit.version.as_str().into(),
                byline: shell::byline(hit).into(),
                summary: hit.summary.as_str().into(),
                numbers: shell::numbers(hit).into(),
                have: if have.contains(&hit.mod_id) {
                    t!("search-have").into()
                } else {
                    "".into()
                },
                adult: if hit.adult {
                    t!("search-adult-badge").into()
                } else {
                    "".into()
                },
                thumb: picture.cloned().unwrap_or_default(),
                thumb_ready: picture.is_some(),
                files_label: if !has_key {
                    "".into()
                } else if open.is_some() {
                    t!("search-hide-files").into()
                } else {
                    t!("search-files").into()
                },
                page_label: t!("search-page").into(),
                ..SearchRow::default()
            });
            keys.push(RowKey::Mod(hit.mod_id));
            let Some(state) = open else {
                continue;
            };
            match state {
                FilesState::Asking => {
                    rows.push(note(t!("search-asking-files"), false));
                    keys.push(RowKey::Note);
                }
                FilesState::Failed(err) => {
                    rows.push(note(err.clone(), true));
                    keys.push(RowKey::Note);
                }
                FilesState::Got(files) if files.is_empty() => {
                    rows.push(note(t!("search-no-files"), false));
                    keys.push(RowKey::Note);
                }
                FilesState::Got(files) => {
                    for file in files {
                        let have_it = have_files.contains(&(hit.mod_id, file.file_id));
                        rows.push(SearchRow {
                            kind: 1,
                            name: file.name.as_str().into(),
                            file_facts: shell::file_facts(file).into(),
                            file_action: if have_it {
                                "".into()
                            } else if premium {
                                t!("search-download").into()
                            } else {
                                t!("search-get-on-nexus").into()
                            },
                            file_have: if have_it {
                                t!("search-have-file").into()
                            } else {
                                "".into()
                            },
                            ..SearchRow::default()
                        });
                        keys.push(RowKey::File(hit.mod_id, file.file_id));
                    }
                }
            }
        }
        (rows, keys)
    }
}

/// A line under a mod about its files.
fn note(text: String, bad: bool) -> SearchRow {
    SearchRow {
        kind: 2,
        name: text.into(),
        bad,
        ..SearchRow::default()
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::config::Config;
    use zendo_core::mods::{Applied, ModInfo, NexusSource, Overview};
    use zendo_core::nexus::{FileInfo, User};
    use zendo_core::nexus_search::Hit;
    use zendo_shell::mods::Outcome;

    use super::super::tests::page;
    use super::*;
    use crate::LibraryData;

    fn overview(domain: Option<&str>) -> Overview {
        Overview {
            game_dir: "/games/Cat Quest".into(),
            target: "/games/Cat Quest".into(),
            folder: String::new(),
            custom_folder: false,
            known: None,
            nexus: domain.map(str::to_owned),
            profile: mods::DEFAULT_PROFILE.into(),
            profiles: vec![mods::DEFAULT_PROFILE.into()],
            mods: vec![ModInfo {
                id: "one".into(),
                name: "one".into(),
                enabled: true,
                files: 1,
                nexus: Some(NexusSource {
                    domain: "catquest".into(),
                    mod_id: 10,
                    file_id: 100,
                    version: "1".into(),
                }),
                installer: false,
                wins: Vec::new(),
                loses: Vec::new(),
            }],
            overwrite: 0,
            applied: Applied::No,
            plugins: None,
        }
    }

    fn hit(mod_id: u64) -> Hit {
        Hit {
            mod_id,
            name: format!("Mod {mod_id}"),
            summary: "Does things".into(),
            author: "Someone".into(),
            version: "1.0".into(),
            endorsements: 1500,
            downloads: 20,
            size: None,
            updated: String::new(),
            category: None,
            adult: mod_id == 12,
            thumbnail: None,
        }
    }

    fn file(id: u64, at: i64) -> FileInfo {
        FileInfo {
            file_id: id,
            name: format!("File {id}"),
            version: Some("1.0".into()),
            file_name: "a.zip".into(),
            size_kb: None,
            category_name: Some("MAIN".into()),
            uploaded_timestamp: at,
        }
    }

    /// A game page with its mods read, and the sheet open on it.
    fn opened(domain: Option<&str>) -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(1);
        let mut config = Config::default();
        config.nexus_key = Some("k".into());
        *glue.config.borrow_mut() = Some(config);
        window
            .global::<LibraryData>()
            .invoke_card_clicked("desktop:0".into());
        glue.mods_arrived("desktop:0", Ok(Outcome::Read(Some(overview(domain)))));
        window.global::<ModsData>().invoke_search_nexus();
        (window, glue)
    }

    fn found(glue: &Glue, mods: &[u64], total: u64) {
        let generation = glue.search.search.borrow().as_ref().unwrap().generation;
        glue.search_found(
            generation,
            Ok(Found {
                hits: mods.iter().map(|id| hit(*id)).collect(),
                total,
            }),
        );
    }

    #[test]
    fn the_sheet_does_not_open_for_a_game_unknown_to_nexus() {
        let (window, _glue) = opened(None);
        assert!(!window.global::<SearchData>().get_open());
    }

    #[test]
    fn the_sheet_opens_for_a_game_known_on_nexus() {
        let (window, _glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        assert!(data.get_open());
        assert_eq!(data.get_orders().row_count(), 3);
        assert!(data.get_asking(), "the first search is under way");
        assert!(
            data.get_limit().contains("Premium"),
            "no Premium account known"
        );
    }

    #[test]
    fn results_come_a_page_at_a_time_and_a_late_answer_is_dropped() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        let generation = glue.search.search.borrow().as_ref().unwrap().generation;
        glue.search_found(
            generation + 1,
            Ok(Found {
                hits: vec![hit(1)],
                total: 1,
            }),
        );
        assert_eq!(
            data.get_rows().row_count(),
            0,
            "an answer to another search"
        );

        found(&glue, &[10, 11], 3);
        assert_eq!(data.get_rows().row_count(), 2);
        assert!(data.get_more());
        let row = data.get_rows().row_data(0).unwrap();
        assert_eq!(row.have, "In your mods", "mod 10 is in the game's list");
        assert_eq!(data.get_rows().row_data(1).unwrap().have, "");
        assert!(
            row.numbers.contains("1k") || row.numbers.contains("1.5"),
            "{}",
            row.numbers
        );

        data.invoke_show_more();
        found(&glue, &[12], 3);
        assert_eq!(data.get_rows().row_count(), 3);
        assert!(!data.get_more());
        assert_eq!(data.get_rows().row_data(2).unwrap().adult, "Adult");
    }

    #[test]
    fn a_mods_files_open_under_it_and_close_again() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        found(&glue, &[10, 11], 2);
        data.invoke_files(1);
        // Asking, in a line under the mod.
        assert_eq!(data.get_rows().row_count(), 3);
        assert_eq!(data.get_rows().row_data(2).unwrap().kind, 2);

        glue.search_got_files(
            11,
            Ok(Files {
                files: vec![file(100, 5), file(101, 9)],
                file_updates: Vec::new(),
            }),
        );
        assert_eq!(data.get_rows().row_count(), 4);
        // Newest first; a free account is sent to the site.
        let first = data.get_rows().row_data(2).unwrap();
        assert_eq!((first.kind, first.name.as_str()), (1, "File 101"));
        assert_eq!(first.file_action, "Get on Nexus");

        data.invoke_files(1);
        assert_eq!(data.get_rows().row_count(), 2, "hidden again");
    }

    #[test]
    fn a_file_already_in_the_mods_has_no_button() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        found(&glue, &[10], 1);
        data.invoke_files(0);
        glue.search_got_files(
            10,
            Ok(Files {
                files: vec![file(100, 5), file(101, 9)],
                file_updates: Vec::new(),
            }),
        );
        let have = data.get_rows().row_data(2).unwrap();
        assert_eq!(have.name, "File 100");
        assert_eq!(have.file_action, "");
        assert!(!have.file_have.is_empty());
        assert_ne!(data.get_rows().row_data(1).unwrap().file_action, "");
    }

    #[test]
    fn a_failure_to_list_files_says_so() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        found(&glue, &[10], 1);
        data.invoke_files(0);
        glue.search_got_files(10, Err("offline".into()));
        let note = data.get_rows().row_data(1).unwrap();
        assert!(note.bad && note.name == "offline");
    }

    #[test]
    fn a_free_account_goes_to_the_site_and_premium_downloads() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        let mods = window.global::<ModsData>();
        found(&glue, &[11], 1);
        data.invoke_files(0);
        glue.search_got_files(
            11,
            Ok(Files {
                files: vec![file(101, 9)],
                file_updates: Vec::new(),
            }),
        );
        data.invoke_download(1);
        assert_eq!(
            mods.get_downloads().row_count(),
            0,
            "no download of its own"
        );
        assert!(
            window
                .global::<LibraryData>()
                .get_status()
                .contains("Nexus")
        );

        glue.nexus.state.borrow_mut().user = Some(User {
            name: "someone".into(),
            is_premium: true,
        });
        glue.show_search();
        assert_eq!(data.get_rows().row_data(1).unwrap().file_action, "Download");
        data.invoke_download(1);
        assert_eq!(mods.get_downloads().row_count(), 1);
    }

    #[test]
    fn the_adult_chip_is_kept_in_the_settings_and_searches_again() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        found(&glue, &[10], 1);
        assert!(!data.get_adult());
        data.invoke_toggle_adult();
        assert!(glue.settings_config().nexus_adult);
        assert!(data.get_adult());
        assert_eq!(data.get_rows().row_count(), 0, "a new search starts empty");
        assert!(data.get_asking());
    }

    #[test]
    fn escape_and_close_shut_the_sheet_and_leave_nothing() {
        let (window, glue) = opened(Some("catquest"));
        let data = window.global::<SearchData>();
        found(&glue, &[10], 1);
        window.global::<LibraryData>().invoke_back();
        assert!(!data.get_open());
        assert_eq!(data.get_rows().row_count(), 0);
        assert!(!glue.search_is_open());
        // Closed with the button as well.
        window.global::<ModsData>().invoke_search_nexus();
        assert!(data.get_open());
        data.invoke_close();
        assert!(!data.get_open());
    }
}
