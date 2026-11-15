//! The art picker over a game's page: Change cover and Change backdrop. Its
//! rules, lists and effects are `zendo_shell::artwork`, shared with Iced;
//! this draws the sheet, runs the effects on threads and keeps the
//! thumbnails. A part of the Library's glue (`library_ui`), which owns the
//! open game and the covers.
//!
//! Every answer comes back through `art_*` with the generation it was asked
//! in, and the picker drops the ones it no longer wants.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};
use zendo_core::covers::{self, Kind};
use zendo_core::steamgriddb::{self, Found, Page};
use zendo_shell::artwork::{self as shared, Art, Effect, GAMES_SHOWN, WORKERS};
use zendo_shell::t;

use super::settings::chip;
use super::{Glue, with_glue};
use crate::covers::{self as slint_covers, Decoded};
use crate::{ArtData, ArtThumb, ChipData, GameData, MainWindow};

/// The widest a thumbnail is kept: twice the widest the sheet draws.
const THUMB_WIDTH: u32 = 576;

/// The picker, while it is open, and what the sheet shows of it.
pub(super) struct ArtState {
    art: RefCell<Art>,
    tabs: Rc<VecModel<ChipData>>,
    games: Rc<VecModel<ChipData>>,
    thumbs: Rc<VecModel<ArtThumb>>,
    /// The thumbnails decoded so far, by the image's id.
    pictures: RefCell<HashMap<u64, Image>>,
    /// Set to stop the thumbnails of the search or list on its way.
    stop: RefCell<Arc<AtomicBool>>,
}

impl ArtState {
    pub(super) fn new(persist: bool) -> Self {
        Self {
            art: RefCell::new(Art::new(persist)),
            tabs: Rc::new(VecModel::default()),
            games: Rc::new(VecModel::default()),
            thumbs: Rc::new(VecModel::default()),
            pictures: RefCell::new(HashMap::new()),
            stop: RefCell::new(Arc::new(AtomicBool::new(false))),
        }
    }
}

impl Glue {
    /// Hooks the sheet's callbacks and the game page's buttons up.
    pub(super) fn connect_art(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<ArtData>();
        data.set_tabs(ModelRc::from(Rc::clone(&self.art.tabs)));
        data.set_games(ModelRc::from(Rc::clone(&self.art.games)));
        data.set_thumbs(ModelRc::from(Rc::clone(&self.art.thumbs)));
        let glue = Rc::clone(self);
        data.on_chose_kind(move |kind| glue.art_kind(kind));
        let glue = Rc::clone(self);
        data.on_edited(move |text| glue.art.art.borrow_mut().set_query(text.to_string()));
        let glue = Rc::clone(self);
        data.on_search(move || glue.art_search());
        let glue = Rc::clone(self);
        data.on_chose_game(move |index| glue.art_game(index));
        let glue = Rc::clone(self);
        data.on_choose(move |index| glue.art_choose(index));
        let glue = Rc::clone(self);
        data.on_more_clicked(move || glue.art_more());
        let glue = Rc::clone(self);
        data.on_automatic(move || glue.art_automatic());
        let glue = Rc::clone(self);
        data.on_settings(move || glue.art_settings());
        let glue = Rc::clone(self);
        data.on_close(move || {
            glue.art_back();
        });
        let data = window.global::<GameData>();
        let glue = Rc::clone(self);
        data.on_change_art(move |kind| glue.art_open(kind));
    }

    /// Whether the picker is open: then the page's keys and the controller
    /// do nothing.
    pub(super) fn art_open_now(&self) -> bool {
        self.art.art.borrow().picker.is_some()
    }

    /// Enter searches; the field has every other key and the page none.
    pub(super) fn art_key(&self, text: &str) -> bool {
        if text == char::from(slint::platform::Key::Return).to_string() {
            self.art_search();
        }
        true
    }

    /// Escape, Close and a click outside. Says whether it was open.
    pub(super) fn art_back(&self) -> bool {
        if !self.art_open_now() {
            return false;
        }
        self.art_stop();
        self.art.art.borrow_mut().close();
        self.art_closed();
        true
    }

    /// Change cover (0) or Change backdrop (1) on the open game's page.
    fn art_open(&self, kind: i32) {
        if self.art_open_now() {
            return;
        }
        let kind = if kind == 0 { Kind::Cover } else { Kind::Hero };
        let found = match self.store_open_id() {
            Some(id) => {
                let title = self
                    .store
                    .browser
                    .borrow()
                    .owned(&id)
                    .map(|owned| owned.title.clone());
                title.map(|title| (id, title))
            }
            None => self.open_game_record().map(|game| (game.id, game.title)),
        };
        let Some((id, title)) = found else {
            return;
        };
        let key = self
            .config
            .borrow()
            .as_ref()
            .and_then(|config| config.steamgriddb_key.clone());
        self.art.pictures.borrow_mut().clear();
        let effect = self.art.art.borrow_mut().open(&id, &title, kind, key);
        self.art_run(effect);
        self.show_picker();
    }

    fn art_kind(&self, kind: i32) {
        let kind = if kind == 0 { Kind::Cover } else { Kind::Hero };
        let effect = self.art.art.borrow_mut().set_kind(kind);
        self.art_run(effect);
        self.show_picker();
    }

    fn art_search(&self) {
        let effect = self.art.art.borrow_mut().search_typed();
        self.art_run(effect);
        self.show_picker();
    }

    fn art_game(&self, index: i32) {
        let found = self
            .art
            .art
            .borrow()
            .picker
            .as_ref()
            .and_then(|picker| picker.games.get(usize::try_from(index).ok()?).map(|g| g.id));
        let Some(game) = found else {
            return;
        };
        let effect = self.art.art.borrow_mut().pick_game(game);
        self.art_run(effect);
        self.show_picker();
    }

    fn art_more(&self) {
        let effect = self.art.art.borrow_mut().more();
        self.art_run(effect);
        self.show_picker();
    }

    fn art_choose(&self, index: i32) {
        let found = self.art.art.borrow().picker.as_ref().and_then(|picker| {
            picker
                .images
                .get(usize::try_from(index).ok()?)
                .map(|image| image.id)
        });
        let Some(image) = found else {
            return;
        };
        let effect = self.art.art.borrow_mut().choose(image);
        self.art_run(effect);
        self.show_picker();
    }

    /// Settings, on the Keys group, which is what the picker is asking for.
    fn art_settings(self: &Rc<Self>) {
        self.art_back();
        self.show_keys_group();
        self.navigate(crate::Page::Settings);
    }

    /// "Use the automatic one".
    fn art_automatic(self: &Rc<Self>) {
        let done = self.art.art.borrow_mut().automatic();
        let Some(done) = done else {
            self.show_picker();
            return;
        };
        self.art_stop();
        let (id, kind) = (done.id, done.kind);
        if self.persist {
            // The store's copies get the one found by itself back, or none.
            let image = covers::cached(&id, kind);
            let outcome = self
                .store
                .browser
                .borrow_mut()
                .put_image(&id, kind, image.as_deref());
            self.apply_store(outcome);
        }
        let note = Art::automatic_note(kind, &done.title);
        self.art_closed();
        let in_library = self.browser.borrow().game(&id).is_some();
        if in_library && self.persist {
            // The scan puts the library's copy back, or downloads it; the
            // note is said once it is read.
            *self.local.after_scan.borrow_mut() = Some((None, note));
            self.rescan();
        } else {
            self.say(note);
            self.show_game();
        }
    }

    /// Starts what the picker asked for, on threads that answer through the
    /// window's loop.
    fn art_run(&self, effect: Option<Effect>) {
        let Some(effect) = effect else {
            return;
        };
        let window = self.window.clone();
        match effect {
            Effect::Search {
                generation,
                key,
                query,
                steam,
            } => {
                self.art_restart();
                std::thread::spawn(move || {
                    let result = (|| -> anyhow::Result<Vec<Found>> {
                        let mut found = Vec::new();
                        if let Some(app) = steam
                            && let Some(game) = steamgriddb::steam_game(&key, &app)?
                        {
                            found.push(game);
                        }
                        for game in steamgriddb::search(&key, &query)? {
                            if !found.iter().any(|kept: &Found| kept.id == game.id) {
                                found.push(game);
                            }
                        }
                        Ok(found)
                    })()
                    .map_err(|err| format!("{err:#}"));
                    let _ = window.upgrade_in_event_loop(move |_| {
                        with_glue(|glue| glue.art_found(generation, result));
                    });
                });
            }
            Effect::List {
                generation,
                key,
                game,
                kind,
                page,
            } => {
                if page == 0 {
                    self.art_restart();
                    self.art.pictures.borrow_mut().clear();
                }
                std::thread::spawn(move || {
                    let result = steamgriddb::images(&key, game, kind, page)
                        .map_err(|err| format!("{err:#}"));
                    let _ = window.upgrade_in_event_loop(move |_| {
                        with_glue(|glue| glue.art_listed(generation, result));
                    });
                });
            }
            Effect::Thumbs { generation, wanted } => {
                let stop = Arc::clone(&self.art.stop.borrow());
                let queue = Arc::new(Mutex::new(wanted.into_iter()));
                for _ in 0..WORKERS {
                    let (queue, stop, window) =
                        (Arc::clone(&queue), Arc::clone(&stop), window.clone());
                    std::thread::spawn(move || {
                        while !stop.load(Ordering::Relaxed) {
                            let next = queue.lock().ok().and_then(|mut queue| queue.next());
                            let Some((id, url)) = next else {
                                return;
                            };
                            let pixels = steamgriddb::thumbnail(&url)
                                .ok()
                                .and_then(|bytes| slint_covers::decode_bytes(&bytes, THUMB_WIDTH));
                            let sent = window.upgrade_in_event_loop(move |_| {
                                with_glue(|glue| glue.art_thumb(generation, id, pixels));
                            });
                            if sent.is_err() {
                                return;
                            }
                        }
                    });
                }
            }
            Effect::Choose { id, kind, image } => {
                std::thread::spawn(move || {
                    let result =
                        covers::choose(&id, kind, &image).map_err(|err| format!("{err:#}"));
                    let _ = window.upgrade_in_event_loop(move |_| {
                        with_glue(|glue| glue.art_chosen(&id, kind, result));
                    });
                });
            }
        }
    }

    /// A new search or list: the thumbnails still coming stop.
    fn art_restart(&self) {
        let old = self.art.stop.replace(Arc::new(AtomicBool::new(false)));
        old.store(true, Ordering::Relaxed);
    }

    fn art_stop(&self) {
        self.art.stop.borrow().store(true, Ordering::Relaxed);
    }

    // The answers.

    pub(super) fn art_found(&self, generation: u64, result: Result<Vec<Found>, String>) {
        let effect = self.art.art.borrow_mut().found(generation, result);
        self.art_run(effect);
        self.show_picker();
    }

    pub(super) fn art_listed(&self, generation: u64, result: Result<Page, String>) {
        let effect = self.art.art.borrow_mut().listed(generation, result);
        self.art_run(effect);
        self.show_picker();
    }

    /// A thumbnail was downloaded and decoded; its row fills in.
    pub(super) fn art_thumb(&self, generation: u64, id: u64, pixels: Decoded) {
        let Some(pixels) = pixels else {
            return;
        };
        let row = {
            let art = self.art.art.borrow();
            if !art.wants(generation) {
                return;
            }
            art.picker
                .as_ref()
                .and_then(|picker| picker.images.iter().position(|image| image.id == id))
        };
        self.art
            .pictures
            .borrow_mut()
            .insert(id, Image::from_rgba8(pixels));
        if let Some(row) = row
            && let Some(mut thumb) = self.art.thumbs.row_data(row)
        {
            thumb.ready = true;
            thumb.image = self.art.pictures.borrow()[&id].clone();
            self.art.thumbs.set_row_data(row, thumb);
        }
    }

    /// The chosen image was saved, or could not be.
    pub(super) fn art_chosen(
        &self,
        id: &str,
        kind: Kind,
        result: Result<std::path::PathBuf, String>,
    ) {
        match result {
            Ok(path) => {
                self.store_image(id, kind, &path);
                let outcome = self.browser.borrow_mut().set_image(id, kind, path);
                self.apply(outcome);
                self.show_game();
                // The picker may have been closed and opened for another
                // game while the file was written.
                let taken = self.art.art.borrow_mut().take_for(id);
                let title = taken
                    .as_ref()
                    .map_or_else(|| id.to_owned(), |p| p.title.clone());
                if taken.is_some() {
                    self.art_stop();
                    self.art_closed();
                }
                self.say(Art::new_note(kind, &title));
            }
            Err(err) => {
                self.art.art.borrow_mut().choose_failed(id, err);
                self.show_picker();
            }
        }
    }

    /// The sheet is gone: its lists are emptied and the page has the
    /// keyboard again.
    fn art_closed(&self) {
        self.art.pictures.borrow_mut().clear();
        self.show_picker();
        self.focus_page();
    }

    /// Writes the sheet from the picker.
    pub(super) fn show_picker(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<ArtData>();
        let art = self.art.art.borrow();
        let Some(picker) = art.picker.as_ref() else {
            data.set_open(false);
            data.set_hover_about("".into());
            self.art.games.set_vec(Vec::new());
            self.art.thumbs.set_vec(Vec::new());
            return;
        };
        data.set_open(true);
        data.set_title(picker.title.clone().into());
        data.set_kind(i32::from(picker.kind == Kind::Hero));
        data.set_query(picker.query.clone().into());
        self.art.tabs.set_vec(vec![
            chip(0, t!("art-cover"), picker.kind == Kind::Cover),
            chip(1, t!("art-hero"), picker.kind == Kind::Hero),
        ]);
        self.art.games.set_vec(
            picker
                .games
                .iter()
                .take(GAMES_SHOWN)
                .enumerate()
                .map(|(index, game)| {
                    chip(
                        i32::try_from(index).unwrap_or(0),
                        shared::game_label(game),
                        picker.game == Some(game.id),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let pictures = self.art.pictures.borrow();
        self.art.thumbs.set_vec(
            picker
                .images
                .iter()
                .map(|found| {
                    let image = pictures.get(&found.id);
                    ArtThumb {
                        ready: image.is_some(),
                        image: image.cloned().unwrap_or_default(),
                        about: shared::thumb_about(found).into(),
                    }
                })
                .collect::<Vec<_>>(),
        );
        data.set_more(picker.more);
        data.set_busy(picker.busy.is_some());
        data.set_status(picker.busy.clone().unwrap_or_default().into());
        data.set_note(picker.note.clone().unwrap_or_default().into());
        data.set_needs_key(picker.needs_key);
        data.set_automatic_label(if picker.chosen {
            Art::automatic_label(picker.kind).into()
        } else {
            "".into()
        });
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use slint::{Model, Rgba8Pixel, SharedPixelBuffer};
    use zendo_core::steamgriddb::Image as Picture;

    use super::super::tests::page;
    use super::*;
    use crate::LibraryData;

    fn found(id: u64, name: &str) -> Found {
        Found {
            id,
            name: name.into(),
            release_date: Some(1_303_084_800),
        }
    }

    fn picture(id: u64) -> Picture {
        Picture {
            id,
            url: format!("https://x/{id}"),
            thumb: format!("https://x/t{id}"),
            width: 600,
            height: 900,
            style: "alternate".into(),
            author: None,
        }
    }

    /// A window with the first game's page open and a key in the settings.
    fn opened(key: Option<&str>) -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(2);
        let mut config = zendo_core::config::Config::default();
        config.steamgriddb_key = key.map(str::to_owned);
        *glue.config.borrow_mut() = Some(config);
        glue.open_game("desktop:0");
        window.global::<GameData>().invoke_change_art(0);
        (window, glue)
    }

    fn generation(glue: &Glue) -> u64 {
        glue.art.art.borrow().picker.as_ref().unwrap().generation
    }

    #[test]
    fn with_no_key_the_sheet_asks_for_one_and_escape_closes_it() {
        let (window, glue) = opened(None);
        let sheet = window.global::<ArtData>();
        assert!(sheet.get_open());
        assert!(sheet.get_needs_key());
        assert!(sheet.get_note().contains("Settings"));
        assert_eq!(sheet.get_title(), "Game 00");
        // The page's keys do nothing under it, and the pad only closes it.
        assert!(glue.key("x", false, false, false));
        window.global::<LibraryData>().invoke_back();
        assert!(!sheet.get_open());
        assert!(window.global::<GameData>().get_open(), "the page stays");
        assert_eq!(
            sheet.get_games().row_count() + sheet.get_thumbs().row_count(),
            0
        );
    }

    #[test]
    fn the_answers_fill_the_sheet_and_late_ones_are_dropped() {
        let (window, glue) = opened(Some("key"));
        let sheet = window.global::<ArtData>();
        assert!(sheet.get_busy());
        let first = generation(&glue);
        glue.art_found(first - 1, Ok(vec![found(1, "Late")]));
        assert_eq!(sheet.get_games().row_count(), 0);
        glue.art_found(first, Ok(vec![found(17830, "Portal 2"), found(2, "Other")]));
        assert_eq!(sheet.get_games().row_count(), 2);
        assert_eq!(
            sheet.get_games().row_data(0).unwrap().label,
            "Portal 2 (2011)"
        );
        assert!(sheet.get_games().row_data(0).unwrap().chosen);

        let listing = generation(&glue);
        glue.art_listed(
            listing,
            Ok(Page {
                images: vec![picture(10), picture(11)],
                more: true,
            }),
        );
        assert_eq!(sheet.get_thumbs().row_count(), 2);
        assert!(!sheet.get_thumbs().row_data(0).unwrap().ready);
        assert!(sheet.get_more());
        glue.art_thumb(
            listing - 1,
            10,
            Some(SharedPixelBuffer::<Rgba8Pixel>::new(2, 2)),
        );
        assert!(!sheet.get_thumbs().row_data(0).unwrap().ready, "stale");
        glue.art_thumb(
            listing,
            11,
            Some(SharedPixelBuffer::<Rgba8Pixel>::new(2, 2)),
        );
        assert!(sheet.get_thumbs().row_data(1).unwrap().ready);
        assert!(!sheet.get_thumbs().row_data(0).unwrap().ready);

        // The other game's chip lists its images from the start.
        sheet.invoke_chose_game(1);
        assert_eq!(sheet.get_thumbs().row_count(), 0);
        assert!(sheet.get_games().row_data(1).unwrap().chosen);
        // The wide images tab asks again for the game chosen.
        sheet.invoke_chose_kind(1);
        assert_eq!(sheet.get_kind(), 1);
        assert!(sheet.get_busy());
    }

    #[test]
    fn a_chosen_image_becomes_the_games_and_closes_the_sheet() {
        let (window, glue) = opened(Some("key"));
        let sheet = window.global::<ArtData>();
        glue.art_found(generation(&glue), Ok(vec![found(5, "Game 00")]));
        glue.art_listed(
            generation(&glue),
            Ok(Page {
                images: vec![picture(10)],
                more: false,
            }),
        );
        sheet.invoke_choose(0);
        assert_eq!(sheet.get_status(), "Saving…");
        // Another game's picture arriving leaves the sheet alone.
        glue.art_chosen("desktop:1", Kind::Cover, Ok(PathBuf::from("/c/other.jpg")));
        assert!(sheet.get_open());
        assert_eq!(
            glue.browser
                .borrow()
                .game("desktop:1")
                .and_then(|g| g.cover.clone()),
            Some(PathBuf::from("/c/other.jpg"))
        );
        glue.art_chosen("desktop:0", Kind::Cover, Ok(PathBuf::from("/c/mine.jpg")));
        assert!(!sheet.get_open());
        assert!(window.global::<GameData>().get_open());
        assert_eq!(
            glue.browser
                .borrow()
                .game("desktop:0")
                .and_then(|g| g.cover.clone()),
            Some(PathBuf::from("/c/mine.jpg"))
        );
        assert_eq!(sheet.get_thumbs().row_count(), 0);
    }

    #[test]
    fn a_failed_save_stays_in_the_sheet() {
        let (window, glue) = opened(Some("key"));
        let sheet = window.global::<ArtData>();
        glue.art_found(generation(&glue), Ok(vec![found(5, "Game 00")]));
        glue.art_listed(
            generation(&glue),
            Ok(Page {
                images: vec![picture(10)],
                more: false,
            }),
        );
        sheet.invoke_choose(0);
        glue.art_chosen("desktop:0", Kind::Cover, Err("disk full".into()));
        assert!(sheet.get_open());
        assert!(!sheet.get_busy());
        assert_eq!(sheet.get_note(), "disk full");
    }

    #[test]
    fn open_settings_closes_the_sheet() {
        let (window, _glue) = opened(None);
        let sheet = window.global::<ArtData>();
        sheet.invoke_settings();
        assert!(!sheet.get_open());
        assert_eq!(window.get_page(), crate::Page::Settings);
    }
}
