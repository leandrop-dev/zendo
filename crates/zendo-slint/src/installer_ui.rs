//! A mod's FOMOD installer sheet: its steps one at a time, the options of
//! each group as radios or boxes, the option last touched described with its
//! picture, then Install builds the mod. The state, the rules and the words
//! are `zendo_shell::installer`'s; the build is `zendo_shell::mods`'
//! `Work::Install`, run through `mods_ui.rs`. A part of the Library's glue
//! (`library_ui`).
//!
//! The sheet is over the whole window, whatever page is open: a download
//! that ends elsewhere can bring an installer up. It takes every key while it
//! is open, and only Cancel and Escape close it (closing drops the unpacked
//! mod, so a click outside does not).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use slint::{ComponentHandle, Image, Model, ModelRc, VecModel};
use zendo_core::mods::{self, Pending};
use zendo_shell::gamepad::Pad;
use zendo_shell::installer::{InstallerState, Opening, Ready, View};
use zendo_shell::t;

use super::mods::refill;
use super::{Glue, with_glue};
use crate::covers::{self as slint_covers, Decoded};
use crate::{InstallerData, InstallerRow, MainWindow};

/// The widest a picture is drawn: the column is under 400 px.
const PICTURE_WIDTH: u32 = 480;

/// The sheet's state and the model its rows come from.
pub(super) struct InstallerUiState {
    state: RefCell<InstallerState>,
    rows: Rc<VecModel<InstallerRow>>,
    /// The picture asked for, and the number of the ask, so that a late
    /// answer for an option no longer described is dropped.
    asked: RefCell<Option<PathBuf>>,
    generation: Cell<u64>,
    picture: RefCell<Option<Image>>,
}

impl InstallerUiState {
    pub(super) fn new() -> Self {
        Self {
            state: RefCell::default(),
            rows: Rc::new(VecModel::default()),
            asked: RefCell::default(),
            generation: Cell::new(0),
            picture: RefCell::default(),
        }
    }
}

/// An index the window sent, as one to read a list by.
fn at(index: i32) -> Option<usize> {
    usize::try_from(index).ok()
}

impl Glue {
    pub(super) fn connect_installer(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<InstallerData>();
        data.set_rows(ModelRc::from(Rc::clone(&self.installer.rows)));
        let glue = Rc::clone(self);
        data.on_pick(move |index| glue.installer_pick(index));
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_installer());
        let glue = Rc::clone(self);
        data.on_next(move || glue.installer_next());
        let glue = Rc::clone(self);
        data.on_back(move || {
            glue.installer.state.borrow_mut().back();
            glue.show_installer();
        });
        let glue = Rc::clone(self);
        data.on_cancel(move || {
            glue.installer_back();
        });
    }

    /// Whether the sheet is open.
    pub(super) fn installer_is_open(&self) -> bool {
        self.installer.state.borrow().is_open()
    }

    /// A mod's installer is to be run: its sheet opens, or it waits for the
    /// slot, or (with no step to show) it installs as it is.
    pub(super) fn installer_open(&self, game: String, pending: Box<Pending>) {
        let free = self.mods_free();
        let opening = self.installer.state.borrow_mut().open(game, pending, free);
        self.installer_opened(opening);
    }

    fn installer_opened(&self, opening: Opening) {
        match opening {
            Opening::Waits => {}
            Opening::Shown(status) => {
                self.say(status);
                self.show_installer();
                self.focus_page();
            }
            Opening::InstallNow(ready) => self.installer_build(ready),
        }
    }

    /// The slot is free: the installer that waited longest opens.
    pub(super) fn installer_next_waiting(&self) {
        let free = self.mods_free();
        let opening = self.installer.state.borrow_mut().next_waiting(free);
        if let Some(opening) = opening {
            self.installer_opened(opening);
        }
    }

    /// Builds the mod with the options chosen, on a thread.
    fn installer_build(&self, ready: Ready) {
        self.mods_install(ready);
        self.show_game();
    }

    /// The last step's Install, if it was one. The sheet closes; the next
    /// installer opens when mods are free again.
    fn installer_finish(&self, ready: Option<Ready>) {
        if let Some(ready) = ready {
            self.installer_build(ready);
            self.installer_next_waiting();
            self.focus_page();
        }
        self.show_installer();
    }

    fn installer_next(&self) {
        let ready = self.installer.state.borrow_mut().advance();
        self.installer_finish(ready);
    }

    fn installer_pick(&self, index: i32) {
        let target = at(index).and_then(|index| self.installer.rows.row_data(index));
        let Some(row) = target else {
            return;
        };
        let (Some(group), Some(option)) = (at(row.group), at(row.option)) else {
            return;
        };
        // A radio asks to be on; a box flips.
        match row.kind {
            1 if row.enabled => self.installer.state.borrow_mut().pick(group, option, true),
            2 if row.enabled => self
                .installer
                .state
                .borrow_mut()
                .pick(group, option, !row.on),
            _ => return,
        }
        self.show_installer();
    }

    /// Escape, Cancel and the pad's Back: the mod is dropped. Says whether
    /// the sheet was open.
    pub(super) fn installer_back(&self) -> bool {
        let cancelled = self.installer.state.borrow_mut().cancel();
        let Some(cancelled) = cancelled else {
            return false;
        };
        self.say(cancelled.status);
        if self.persist {
            let pending = cancelled.pending;
            std::thread::spawn(move || mods::cancel_install(&pending));
        }
        self.show_installer();
        self.focus_page();
        self.installer_next_waiting();
        true
    }

    /// A key while the sheet is open: it takes all of them. Says whether it
    /// was taken (always).
    pub(super) fn installer_key(&self, text: &str) -> bool {
        use slint::platform::Key;
        let mut chars = text.chars();
        let (Some(key), None) = (chars.next(), chars.next()) else {
            return true;
        };
        if key == char::from(Key::UpArrow) {
            self.installer.state.borrow_mut().focus(-1);
        } else if key == char::from(Key::DownArrow) {
            self.installer.state.borrow_mut().focus(1);
        } else if key == char::from(Key::RightArrow) {
            return self.installer_pad(Pad::Right);
        } else if key == char::from(Key::LeftArrow) {
            self.installer.state.borrow_mut().back();
        } else if key == char::from(Key::Return) {
            return self.installer_pad(Pad::Activate);
        }
        self.show_installer();
        true
    }

    /// The pad's presses while the sheet is open.
    pub(super) fn installer_pad(&self, pad: Pad) -> bool {
        match pad {
            Pad::Back => {
                self.installer_back();
            }
            Pad::Up => self.installer.state.borrow_mut().focus(-1),
            Pad::Down => self.installer.state.borrow_mut().focus(1),
            Pad::Left => self.installer.state.borrow_mut().back(),
            Pad::Right => {
                self.installer_next();
                return true;
            }
            Pad::Activate => {
                let ready = self.installer.state.borrow_mut().activate();
                self.installer_finish(ready);
                return true;
            }
            _ => {}
        }
        self.show_installer();
        true
    }

    /// Writes the sheet, or closes it.
    pub(super) fn show_installer(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<InstallerData>();
        let view = self.installer.state.borrow().view();
        let Some(view) = view else {
            data.set_open(false);
            self.installer.rows.set_vec(Vec::new());
            self.installer_picture(&data, None);
            return;
        };
        data.set_open(true);
        data.set_title(view.title.as_str().into());
        data.set_step_line(view.step_line.as_str().into());
        data.set_needs(view.needs_warning);
        data.set_about_name(view.about.name.as_str().into());
        data.set_about_text(view.about.text.as_str().into());
        data.set_back_enabled(view.back_enabled);
        data.set_primary_label(
            if view.final_step {
                t!("installer-install")
            } else {
                t!("installer-next")
            }
            .into(),
        );
        refill(&self.installer.rows, flat_rows(&view));
        self.installer_picture(&data, view.about.picture);
    }

    /// The picture of the option described: kept while it is the same file,
    /// else read and decoded on a thread.
    fn installer_picture(&self, data: &InstallerData<'_>, wanted: Option<PathBuf>) {
        if *self.installer.asked.borrow() != wanted {
            self.installer.asked.borrow_mut().clone_from(&wanted);
            self.installer
                .generation
                .set(self.installer.generation.get() + 1);
            *self.installer.picture.borrow_mut() = None;
            if let Some(path) = wanted.filter(|_| self.persist) {
                let generation = self.installer.generation.get();
                let weak = self.window.clone();
                std::thread::spawn(move || {
                    let pixels = std::fs::read(&path)
                        .ok()
                        .and_then(|bytes| slint_covers::decode_bytes(&bytes, PICTURE_WIDTH));
                    let _ = weak.upgrade_in_event_loop(move |_| {
                        with_glue(|glue| glue.installer_picture_ready(generation, pixels));
                    });
                });
            }
        }
        match &*self.installer.picture.borrow() {
            Some(image) => {
                data.set_picture(image.clone());
                data.set_has_picture(true);
            }
            None => data.set_has_picture(false),
        }
    }

    /// A picture was read and decoded; it shows when it is still the one
    /// asked for.
    pub(super) fn installer_picture_ready(&self, generation: u64, pixels: Decoded) {
        if generation != self.installer.generation.get() {
            return;
        }
        let Some(pixels) = pixels else {
            return;
        };
        *self.installer.picture.borrow_mut() = Some(Image::from_rgba8(pixels));
        self.show_installer();
    }
}

/// The sheet's groups as the flat rows the window draws.
fn flat_rows(view: &View) -> Vec<InstallerRow> {
    let index = |at: usize| i32::try_from(at).unwrap_or(i32::MAX);
    let mut rows = Vec::new();
    for (g, group) in view.groups.iter().enumerate() {
        rows.push(InstallerRow {
            kind: 0,
            group: index(g),
            option: -1,
            label: group.title.as_str().into(),
            on: false,
            enabled: false,
            dim: false,
            focused: false,
        });
        for (o, option) in group.options.iter().enumerate() {
            rows.push(InstallerRow {
                kind: if option.single { 1 } else { 2 },
                group: index(g),
                option: index(o),
                label: option.label.as_str().into(),
                on: option.on,
                enabled: option.enabled,
                dim: option.dim,
                focused: option.focused,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use slint::{Model, platform::Key};
    use zendo_core::fomod::Installer;
    use zendo_shell::mods::Outcome;

    use super::super::tests::page;
    use super::*;
    use crate::{GameData, LibraryData, Page};

    fn installer() -> Installer {
        Installer::parse(
            br#"<config><moduleName>Fancy</moduleName><installSteps order="Explicit">
              <installStep name="One"><optionalFileGroups>
                <group name="Maybe" type="SelectAtMostOne"><plugins>
                  <plugin name="A"><description>About A</description>
                    <files><file source="a"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                  <plugin name="B"><files><file source="b"/></files>
                    <typeDescriptor><type name="NotUsable"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
              <installStep name="Two"><optionalFileGroups>
                <group name="Any" type="SelectAny"><plugins>
                  <plugin name="C"><files><file source="c"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
            </installSteps></config>"#,
        )
        .unwrap()
    }

    /// An archive with an installer has just been read for the game.
    fn arrives(glue: &Glue, game: &str) {
        let pending = mods::test_pending("Fancy", installer());
        glue.mods_arrived(
            game,
            Ok(Outcome::Added(
                mods::Added::Choose(Box::new(pending)),
                String::new(),
            )),
        );
    }

    fn rows(window: &MainWindow) -> Vec<InstallerRow> {
        window.global::<InstallerData>().get_rows().iter().collect()
    }

    fn status(glue: &Glue) -> String {
        glue.browser
            .borrow()
            .status()
            .map(str::to_owned)
            .unwrap_or_default()
    }

    #[test]
    fn the_sheet_opens_over_the_page_and_shows_the_first_step() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        assert!(!data.get_open());
        arrives(&glue, "desktop:0");
        assert!(data.get_open());
        assert_eq!(data.get_title(), "Fancy");
        assert_eq!(data.get_step_line(), "Step 1 of 2 · One");
        assert_eq!(data.get_primary_label(), "Next");
        assert!(!data.get_back_enabled());
        assert!(status(&glue).contains("Choose the options of Fancy"));
        // The group's name, then its two options: a radio and a dim one.
        let rows = rows(&window);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, 0);
        assert_eq!(rows[0].label, "Maybe · choose one or none");
        assert_eq!((rows[1].kind, rows[1].label.as_str()), (1, "A"));
        assert!(rows[1].enabled && !rows[1].on);
        assert!(!rows[2].enabled && rows[2].dim, "not usable");
        // The first option is described until another is touched.
        assert_eq!(data.get_about_name(), "A");
        assert_eq!(data.get_about_text(), "About A");
    }

    #[test]
    fn a_click_picks_a_radio_twice_clears_it_and_never_changes_a_row_that_cannot() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");

        data.invoke_pick(1);
        assert!(rows(&window)[1].on && rows(&window)[1].focused);
        data.invoke_pick(1);
        assert!(
            !rows(&window)[1].on,
            "a radio of a group that may have none"
        );
        data.invoke_pick(1);
        data.invoke_pick(2);
        data.invoke_pick(0);
        assert!(
            rows(&window)[1].on,
            "the not usable and the title do nothing"
        );
        assert!(!rows(&window)[2].on);
        data.invoke_pick(99);
    }

    #[test]
    fn next_goes_on_back_returns_and_the_last_step_says_install() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");

        data.invoke_next();
        assert_eq!(data.get_step_line(), "Step 2 of 2 · Two");
        assert_eq!(data.get_primary_label(), "Install");
        assert!(data.get_back_enabled());
        assert_eq!(rows(&window)[1].kind, 2, "a box");
        data.invoke_back();
        assert_eq!(data.get_step_line(), "Step 1 of 2 · One");

        // Install: the sheet closes and the build is asked for (nothing
        // runs in a test, so mods stay free).
        data.invoke_next();
        data.invoke_next();
        assert!(!data.get_open());
        assert_eq!(rows(&window).len(), 0);
        assert!(glue.mods_free());
    }

    #[test]
    fn cancel_and_escape_drop_the_mod_and_say_so() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");
        data.invoke_cancel();
        assert!(!data.get_open());
        assert_eq!(status(&glue), "Fancy was not installed");

        arrives(&glue, "desktop:0");
        window.global::<LibraryData>().invoke_back();
        assert!(!data.get_open());
    }

    #[test]
    fn an_installer_that_comes_while_one_is_open_waits_and_opens_after_it() {
        let (window, glue) = page(2);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");
        arrives(&glue, "desktop:1");
        assert_eq!(glue.installer.state.borrow().game(), Some("desktop:0"));
        assert_eq!(glue.installer.state.borrow().waiting(), 1);

        data.invoke_cancel();
        assert!(data.get_open(), "the waiting one is up");
        assert_eq!(glue.installer.state.borrow().game(), Some("desktop:1"));
        assert_eq!(glue.installer.state.borrow().waiting(), 0);
        data.invoke_cancel();
        assert!(!data.get_open());
    }

    #[test]
    fn an_installer_that_comes_while_mods_change_waits_for_the_slot() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        // A change is in flight for the game.
        glue.mods.state.borrow_mut().busy = Some(("desktop:0".into(), "Applying…".into()));
        let pending = mods::test_pending("Fancy", installer());
        glue.installer_open("desktop:0".into(), Box::new(pending));
        assert!(!data.get_open(), "it waits");
        assert_eq!(glue.installer.state.borrow().waiting(), 1);

        // The change ends: the slot is free and it opens.
        glue.mods_arrived("desktop:0", Err("it failed".into()));
        assert!(data.get_open());
    }

    #[test]
    fn escape_closes_the_sheet_before_the_page_and_the_sheet_takes_every_key() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        let library = window.global::<LibraryData>();
        library.invoke_card_clicked("desktop:0".into());
        assert!(window.global::<GameData>().get_open());
        arrives(&glue, "desktop:0");

        // Letters would start the search, Ctrl+3 would change the page.
        assert!(library.invoke_key("a".into(), false, false, false, false));
        assert_eq!(library.get_search(), "");
        assert!(library.invoke_key("3".into(), true, false, false, false));
        assert_eq!(window.get_page(), Page::Library);
        assert!(window.global::<GameData>().get_open());

        // The arrows move the cursor, Enter picks, Right goes on, Left back.
        let key = |key: Key| {
            library.invoke_key(
                String::from(char::from(key)).into(),
                false,
                false,
                false,
                false,
            )
        };
        // With nothing touched the cursor counts as on the first option, so
        // two presses of Down come round to it again (Iced does the same).
        assert!(key(Key::DownArrow));
        assert!(rows(&window)[2].focused);
        assert!(key(Key::DownArrow));
        assert!(rows(&window)[1].focused);
        assert!(key(Key::Return));
        assert!(
            rows(&window)[1].on,
            "Enter picked the option under the cursor"
        );
        assert!(key(Key::RightArrow));
        assert_eq!(data.get_step_line(), "Step 2 of 2 · Two");
        assert!(key(Key::LeftArrow));
        assert_eq!(data.get_step_line(), "Step 1 of 2 · One");

        library.invoke_back();
        assert!(!data.get_open());
        assert!(
            window.global::<GameData>().get_open(),
            "the page is still there"
        );
        library.invoke_back();
        assert!(!window.global::<GameData>().get_open());
    }

    #[test]
    fn the_pad_moves_picks_goes_on_and_closes() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");
        glue.installer_pad(Pad::Down);
        glue.installer_pad(Pad::Down);
        assert!(rows(&window)[1].focused);
        glue.installer_pad(Pad::Activate);
        assert!(rows(&window)[1].on);
        glue.installer_pad(Pad::Right);
        assert_eq!(data.get_step_line(), "Step 2 of 2 · Two");
        glue.installer_pad(Pad::Left);
        glue.installer_pad(Pad::Back);
        assert!(!data.get_open());
    }

    #[test]
    fn a_picture_that_comes_late_for_another_option_is_dropped() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");
        assert!(!data.get_has_picture());
        // The fifth ask is the one in force; an answer for the fourth is
        // ignored.
        glue.installer.generation.set(5);
        let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(2, 2);
        glue.installer_picture_ready(4, Some(pixels.clone()));
        assert!(!data.get_has_picture());
        glue.installer_picture_ready(5, Some(pixels));
        assert!(data.get_has_picture());
    }

    #[test]
    fn the_sheet_is_asked_for_again_when_the_language_changes() {
        let (window, glue) = page(1);
        let data = window.global::<InstallerData>();
        arrives(&glue, "desktop:0");
        data.set_step_line("stale".into());
        data.invoke_relabel();
        assert_eq!(data.get_step_line(), "Step 1 of 2 · One");
        drop(glue);
    }
}
