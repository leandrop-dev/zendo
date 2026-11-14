//! Games added by hand: the Add game sheet over the library, the same sheet
//! to edit one from its page, and Remove there, which takes it out of the
//! library and leaves its files and prefix. A part of the Library's glue
//! (`library_ui`), which owns the browser and the rescan.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::ComponentHandle;
use zendo_core::local::{self, LocalGame};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::t;

use super::{Glue, with_glue};
use crate::{LocalData, MainWindow};

/// What the sheet and the page's Remove keep between presses.
#[derive(Default)]
pub(super) struct LocalState {
    form: RefCell<Option<Form>>,
    /// The game whose Remove was pressed once.
    pub(super) confirm_remove: RefCell<Option<String>>,
    /// Once the library is read again: the game to open, if any, and what
    /// to say.
    after_scan: RefCell<Option<(Option<String>, String)>>,
}

#[derive(Default)]
struct Form {
    /// The game being edited; `None` for a new one.
    id: Option<String>,
    title: String,
    program: String,
    arguments: String,
    picking: bool,
    error: Option<String>,
}

/// What the sheet says under its heading about the program chosen.
fn about_of(program: &str) -> String {
    if program.trim().is_empty() {
        t!("local-about-empty")
    } else if local::is_windows(Path::new(program.trim())) {
        t!("local-about-windows")
    } else {
        t!("local-about-linux")
    }
}

impl Glue {
    /// Hooks the sheet's callbacks up.
    pub(super) fn connect_local(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<LocalData>();
        let glue = Rc::clone(self);
        data.on_add(move || glue.open_form(Form::default()));
        let glue = Rc::clone(self);
        data.on_program_edited(move |text| glue.edit_form(|form| form.program = text.into()));
        let glue = Rc::clone(self);
        data.on_title_edited(move |text| glue.edit_form(|form| form.title = text.into()));
        let glue = Rc::clone(self);
        data.on_arguments_edited(move |text| glue.edit_form(|form| form.arguments = text.into()));
        let glue = Rc::clone(self);
        data.on_pick(move || glue.pick());
        let glue = Rc::clone(self);
        data.on_save(move || glue.save_form());
        let glue = Rc::clone(self);
        data.on_close(move || {
            glue.close_form();
        });
        let data = window.global::<crate::GameData>();
        let glue = Rc::clone(self);
        data.on_edit(move || glue.edit_local());
        let glue = Rc::clone(self);
        data.on_remove(move || glue.remove_local());
    }

    /// Whether the sheet is open: then the page's keys and the controller do
    /// nothing.
    pub(super) fn form_open(&self) -> bool {
        self.local.form.borrow().is_some()
    }

    fn open_form(&self, form: Form) {
        *self.local.form.borrow_mut() = Some(form);
        self.show_form();
    }

    /// Closes the sheet. Says whether it was open.
    pub(super) fn close_form(&self) -> bool {
        let was_open = self.local.form.borrow_mut().take().is_some();
        if was_open {
            self.show_form();
        }
        was_open
    }

    /// A field was typed in: keeps it, and writes what depends on it.
    fn edit_form(&self, change: impl FnOnce(&mut Form)) {
        if let Some(form) = self.local.form.borrow_mut().as_mut() {
            change(form);
        }
        self.show_form_status();
    }

    /// Writes the whole sheet: the fields too, which is for when it opens or
    /// a program was chosen, not while the user types.
    fn show_form(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<LocalData>();
        {
            let form = self.local.form.borrow();
            data.set_open(form.is_some());
            if let Some(form) = form.as_ref() {
                data.set_editing(form.id.is_some());
                data.set_program(form.program.clone().into());
                data.set_title(form.title.clone().into());
                data.set_arguments(form.arguments.clone().into());
            }
        }
        self.show_form_status();
    }

    /// Writes what the sheet says about the fields: the kind of program, an
    /// error, and whether the file dialog is open.
    fn show_form_status(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<LocalData>();
        let form = self.local.form.borrow();
        let Some(form) = form.as_ref() else {
            return;
        };
        data.set_about(about_of(&form.program).into());
        data.set_error(form.error.clone().unwrap_or_default().into());
        data.set_picking(form.picking);
    }

    /// The Choose button: the file dialog runs as a process of its own, on a
    /// thread, and the answer comes back to the window.
    fn pick(&self) {
        {
            let mut form = self.local.form.borrow_mut();
            let Some(form) = form.as_mut() else {
                return;
            };
            if form.picking || !self.persist {
                return;
            }
            form.picking = true;
        }
        self.show_form_status();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = file_dialog::pick(Kind::Program, &t!("local-choose-program"))
                .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.picked(result)));
        });
    }

    fn picked(&self, result: Result<Option<PathBuf>, String>) {
        {
            let mut form = self.local.form.borrow_mut();
            let Some(form) = form.as_mut() else {
                return;
            };
            form.picking = false;
            match result {
                Ok(Some(path)) => {
                    if form.title.trim().is_empty() {
                        form.title = local::guess_title(&path);
                    }
                    form.program = path.display().to_string();
                    form.error = None;
                }
                Ok(None) => {}
                Err(err) => form.error = Some(err),
            }
        }
        self.show_form();
    }

    /// Adds the game, or saves the changes to it, and reads the library
    /// again to open it.
    fn save_form(&self) {
        let game = {
            let form = self.local.form.borrow();
            let Some(form) = form.as_ref() else {
                return;
            };
            LocalGame {
                id: form.id.clone().unwrap_or_default(),
                title: form.title.clone(),
                program: PathBuf::from(form.program.trim()),
                arguments: form.arguments.trim().to_owned(),
            }
        };
        let added = game.id.is_empty();
        let title = game.title.trim().to_owned();
        let saved = if self.persist {
            local::put(game)
        } else {
            game.check().map(|()| "local:test".to_owned())
        };
        match saved {
            Ok(id) => {
                self.close_form();
                let note = if added {
                    t!("local-added", title = title)
                } else {
                    t!("local-saved", title = title)
                };
                *self.local.after_scan.borrow_mut() = Some((Some(id), note));
                self.read_again();
            }
            Err(err) => {
                if let Some(form) = self.local.form.borrow_mut().as_mut() {
                    form.error = Some(format!("{err:#}"));
                }
                self.show_form_status();
            }
        }
    }

    /// The Edit button on a game's page: the sheet, filled from its record.
    fn edit_local(&self) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        let found = if self.persist {
            local::find(&id)
        } else {
            Ok(None)
        };
        match found {
            Ok(Some(game)) => self.open_form(Form {
                id: Some(game.id),
                title: game.title,
                program: game.program.display().to_string(),
                arguments: game.arguments,
                ..Form::default()
            }),
            Ok(None) => self.say(t!("local-no-record")),
            Err(err) => self.say(format!("{err:#}")),
        }
    }

    /// The Remove button, pressed once to ask and again to take the game out
    /// of the library; its files and prefix stay where they are.
    fn remove_local(&self) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        if self.local.confirm_remove.borrow().as_deref() != Some(id.as_str()) {
            *self.local.confirm_remove.borrow_mut() = Some(id);
            self.show_game();
            return;
        }
        *self.local.confirm_remove.borrow_mut() = None;
        let removed = if self.persist {
            local::remove(&id)
        } else {
            Ok(())
        };
        match removed {
            Ok(()) => {
                let title = self
                    .browser
                    .borrow()
                    .game(&id)
                    .map_or_else(|| id.clone(), |game| game.title.clone());
                let note = t!("local-removed", title = title);
                self.close_game();
                *self.local.after_scan.borrow_mut() = Some((None, note.clone()));
                self.say(note);
                self.read_again();
            }
            Err(err) => self.say(format!("{err:#}")),
        }
    }

    /// Reads the games again after a change, unless this is a test, which
    /// must not read the user's files.
    fn read_again(&self) {
        if self.persist {
            with_glue(Glue::rescan);
        }
    }

    /// The games were read again: opens the game that was added or saved,
    /// and says what happened.
    pub(super) fn after_scan(&self) {
        let pending = self.local.after_scan.borrow_mut().take();
        if let Some((open, note)) = pending {
            if let Some(id) = open {
                self.open_game(&id);
            }
            self.say(note);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_says_what_kind_of_program_it_got() {
        assert_eq!(about_of(" "), t!("local-about-empty"));
        assert_eq!(about_of("/g/Game.EXE"), t!("local-about-windows"));
        assert_eq!(about_of("/g/game.sh"), t!("local-about-linux"));
    }
}
