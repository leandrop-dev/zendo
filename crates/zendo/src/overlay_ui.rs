//! The Epic overlay card of Settings: whether Epic's EOS overlay is
//! installed in Zendo's folder, and Install (Update once installed) and
//! Remove, through legendary. Each Epic game turns it on in its launch
//! options. A part of the Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::ComponentHandle;
use zendo_core::eos_overlay;
use zendo_core::stores::Store;
use zendo_shell::t;

use super::Glue;
use super::settings::note_tone;
use crate::{MainWindow, SettingsData};

/// Where Install and Remove are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Step {
    #[default]
    Idle,
    /// Remove was pressed once.
    ConfirmRemove,
    /// Installing, updating or removing.
    Working,
}

#[derive(Default)]
pub(super) struct OverlayState {
    installed: Cell<bool>,
    /// legendary is there to install it.
    engine: Cell<bool>,
    step: Cell<Step>,
    /// The last thing that happened, and whether it went well.
    note: RefCell<Option<(String, bool)>>,
}

impl Glue {
    pub(super) fn connect_overlay(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        let glue = Rc::clone(self);
        data.on_overlay_do_install(move || glue.install_overlay());
        let glue = Rc::clone(self);
        data.on_overlay_do_remove(move || glue.remove_overlay());
    }

    /// Looks again whether it is installed, as Settings opens. Nothing is
    /// read in tests.
    pub(super) fn want_overlay(&self) {
        if !self.persist {
            return;
        }
        self.overlay.installed.set(eos_overlay::installed());
        self.overlay.engine.set(Store::Epic.engine().is_some());
    }

    fn install_overlay(&self) {
        let state = &self.overlay;
        if state.step.get() == Step::Working {
            return;
        }
        state.step.set(Step::Working);
        *state.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        let done = if state.installed.get() {
            t!("overlay-updated")
        } else {
            t!("overlay-installed")
        };
        self.ask(
            || eos_overlay::install().map_err(|err| format!("{err:#}")),
            move |glue, result| glue.overlay_done(done, result),
        );
    }

    fn remove_overlay(&self) {
        let state = &self.overlay;
        match state.step.get() {
            Step::Working => return,
            Step::Idle => {
                state.step.set(Step::ConfirmRemove);
                self.show_settings();
                return;
            }
            Step::ConfirmRemove => state.step.set(Step::Working),
        }
        *state.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        let done = t!("overlay-removed");
        self.ask(
            || eos_overlay::remove().map_err(|err| format!("{err:#}")),
            move |glue, result| glue.overlay_done(done, result),
        );
    }

    /// Done: what was done (installed or updated, removed), and how it went.
    pub(super) fn overlay_done(&self, done: String, result: Result<(), String>) {
        let state = &self.overlay;
        state.step.set(Step::Idle);
        if self.persist {
            state.installed.set(eos_overlay::installed());
        }
        *state.note.borrow_mut() = Some(match result {
            Ok(()) => (done, true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    /// Writes the card.
    pub(super) fn show_overlay(&self, data: &SettingsData<'_>) {
        let state = &self.overlay;
        let (installed, engine) = (state.installed.get(), state.engine.get());
        let step = state.step.get();
        let working = step == Step::Working;
        let status = if working {
            t!("overlay-working")
        } else if installed {
            t!(
                "overlay-is-installed",
                path = eos_overlay::folder().map_or_default(|folder| folder.display().to_string())
            )
        } else if engine {
            t!("overlay-not-installed")
        } else {
            t!("overlay-no-legendary")
        };
        let ready = engine && !working;
        data.set_overlay_status(status.into());
        data.set_overlay_install(
            if installed {
                t!("overlay-update")
            } else {
                t!("overlay-install")
            }
            .into(),
        );
        data.set_overlay_install_on(ready);
        data.set_overlay_remove(
            if !installed {
                String::new()
            } else if step == Step::ConfirmRemove {
                t!("really-remove")
            } else {
                t!("remove")
            }
            .into(),
        );
        data.set_overlay_remove_on(ready);
        let (note, good) = state.note.borrow().clone().unwrap_or_default();
        data.set_overlay_note(note.into());
        data.set_overlay_note_tone(note_tone(good));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Themes;

    #[test]
    fn asks_once_more_before_removing() {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, Themes::plain());
        glue.connect(&window);
        glue.overlay.installed.set(true);
        glue.overlay.engine.set(true);
        glue.remove_overlay();
        assert_eq!(glue.overlay.step.get(), Step::ConfirmRemove);
        glue.remove_overlay();
        assert_eq!(glue.overlay.step.get(), Step::Working, "removing");
        glue.overlay_done("Removed".into(), Err("legendary failed".into()));
        assert_eq!(glue.overlay.step.get(), Step::Idle);
        assert_eq!(
            *glue.overlay.note.borrow(),
            Some(("legendary failed".to_owned(), false))
        );
    }
}
