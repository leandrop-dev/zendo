//! The Anti-cheat card of Settings: for Easy Anti-Cheat and `BattlEye`, the
//! copy games use (Steam's, else Zendo's own), the build Lutris lists, and
//! Download, Update or Remove for Zendo's copy. Games download the one they
//! need on their own before they start; this is for seeing and tidying. A
//! part of the Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::runtimes::{self, Found, Remote, Runtime};
use zendo_shell::t;

use super::Glue;
use super::settings::note_tone;
use crate::{MainWindow, SetRow, SettingsData};

#[derive(Default)]
pub(super) struct RuntimesState {
    /// The copy games use, for each runtime, in `Runtime::ALL`'s order.
    found: RefCell<Vec<(Runtime, Option<Found>)>>,
    /// Zendo's own copies, used or not, with their build.
    own: RefCell<Vec<(Runtime, Option<String>)>>,
    /// What Lutris lists; `None` until it answers.
    latest: RefCell<Option<Vec<Remote>>>,
    error: RefCell<Option<String>>,
    asking: Cell<bool>,
    /// The runtime being downloaded.
    downloading: Cell<Option<Runtime>>,
    /// The runtime whose Remove was pressed once.
    confirm_remove: Cell<Option<Runtime>>,
    /// The last thing that happened, and whether it went well.
    note: RefCell<Option<(String, bool)>>,
    model: Rc<VecModel<SetRow>>,
}

impl RuntimesState {
    fn remote(&self, runtime: Runtime) -> Option<Remote> {
        self.latest
            .borrow()
            .as_ref()?
            .iter()
            .find(|remote| remote.runtime == runtime)
            .cloned()
    }

    fn has_own(&self, runtime: Runtime) -> bool {
        self.own.borrow().iter().any(|(r, _)| *r == runtime)
    }

    /// The build of Zendo's copy, when it has one and knows it.
    fn own_version(&self, runtime: Runtime) -> Option<String> {
        self.own
            .borrow()
            .iter()
            .find(|(r, _)| *r == runtime)
            .and_then(|(_, version)| version.clone())
    }
}

/// The day of a Lutris build time: `2024-01-14` of
/// `2024-01-14T19:38:26.671443Z`.
fn day(version: &str) -> &str {
    version.get(..10).unwrap_or(version)
}

impl Glue {
    pub(super) fn connect_runtimes(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        data.set_runtimes(ModelRc::from(Rc::clone(&self.runtimes.model)));
        let glue = Rc::clone(self);
        data.on_runtime_act(move |index| {
            if let Some(runtime) = usize::try_from(index)
                .ok()
                .and_then(|index| Runtime::ALL.get(index).copied())
            {
                glue.download_runtime(runtime);
            }
        });
        let glue = Rc::clone(self);
        data.on_runtime_remove(move |index| {
            if let Some(runtime) = usize::try_from(index)
                .ok()
                .and_then(|index| Runtime::ALL.get(index).copied())
            {
                glue.remove_runtime(runtime);
            }
        });
    }

    /// Looks at the runtimes on the machine. Nothing is read in tests.
    fn refresh_runtimes(&self) {
        if !self.persist {
            return;
        }
        *self.runtimes.found.borrow_mut() = Runtime::ALL
            .into_iter()
            .map(|runtime| (runtime, runtimes::find(runtime)))
            .collect();
        *self.runtimes.own.borrow_mut() = Runtime::ALL
            .into_iter()
            .filter_map(|runtime| match runtimes::own(runtime)? {
                Found::Zendo { version, .. } => Some((runtime, version)),
                Found::Steam(_) => None,
            })
            .collect();
    }

    /// Looks at the runtimes as Settings opens, and asks Lutris for its list
    /// once a session.
    pub(super) fn want_runtimes(&self) {
        self.refresh_runtimes();
        let state = &self.runtimes;
        if !self.persist || state.latest.borrow().is_some() || state.asking.get() {
            return;
        }
        state.asking.set(true);
        *state.error.borrow_mut() = None;
        self.ask(
            || runtimes::latest().map_err(|err| format!("{err:#}")),
            Self::runtimes_latest,
        );
    }

    pub(super) fn runtimes_latest(&self, result: Result<Vec<Remote>, String>) {
        self.runtimes.asking.set(false);
        match result {
            Ok(latest) => *self.runtimes.latest.borrow_mut() = Some(latest),
            Err(err) => *self.runtimes.error.borrow_mut() = Some(err),
        }
        self.show_settings();
    }

    pub(super) fn download_runtime(&self, runtime: Runtime) {
        let state = &self.runtimes;
        if state.downloading.get().is_some() {
            return;
        }
        let Some(remote) = state.remote(runtime) else {
            return;
        };
        state.downloading.set(Some(runtime));
        *state.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        self.ask(
            move || {
                runtimes::install(&remote, &AtomicBool::new(false), &mut |_, _| {})
                    .map(|_| ())
                    .map_err(|err| format!("{err:#}"))
            },
            move |glue, result| glue.runtime_done(runtime, result),
        );
    }

    pub(super) fn runtime_done(&self, runtime: Runtime, result: Result<(), String>) {
        self.runtimes.downloading.set(None);
        self.refresh_runtimes();
        *self.runtimes.note.borrow_mut() = Some(match result {
            Ok(()) => (t!("runtimes-downloaded", runtime = runtime.label()), true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    /// Remove, pressed once to ask and again to delete.
    pub(super) fn remove_runtime(&self, runtime: Runtime) {
        let state = &self.runtimes;
        if state.confirm_remove.get() != Some(runtime) {
            state.confirm_remove.set(Some(runtime));
            self.show_settings();
            return;
        }
        state.confirm_remove.set(None);
        if !self.persist {
            self.runtime_removed(runtime, Ok(()));
            return;
        }
        // The folder can be large, so it goes on a thread.
        self.ask(
            move || runtimes::remove(runtime).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.runtime_removed(runtime, result),
        );
        self.show_settings();
    }

    pub(super) fn runtime_removed(&self, runtime: Runtime, result: Result<(), String>) {
        self.refresh_runtimes();
        *self.runtimes.note.borrow_mut() = Some(match result {
            Ok(()) => (t!("runtimes-removed", runtime = runtime.label()), true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    fn runtime_rows(&self) -> Vec<SetRow> {
        let state = &self.runtimes;
        let confirm = state.confirm_remove.get();
        let downloading = state.downloading.get();
        state
            .found
            .borrow()
            .iter()
            .map(|(runtime, found)| {
                let runtime = *runtime;
                let own = state.has_own(runtime);
                let remote = state.remote(runtime);
                let status = match found {
                    Some(Found::Steam(_)) => t!("runtimes-from-steam"),
                    Some(Found::Zendo { version, .. }) => match version {
                        Some(version) => t!("runtimes-downloaded-on", date = day(version)),
                        None => t!("runtimes-downloaded-undated"),
                    },
                    None => t!("runtimes-not-here"),
                };
                // Download when Zendo has no copy, Update when Lutris has a
                // newer build than Zendo's; Steam's copy is Steam's to keep
                // current.
                let offer = match &remote {
                    Some(_) if !own && !matches!(found, Some(Found::Steam(_))) => {
                        Some(t!("download"))
                    }
                    Some(remote)
                        if own
                            && state.own_version(runtime).as_deref()
                                != Some(remote.version.as_str()) =>
                    {
                        Some(t!("runtimes-update", date = day(&remote.version)))
                    }
                    _ => None,
                };
                let this_downloading = downloading == Some(runtime);
                let confirming = confirm == Some(runtime);
                SetRow {
                    title: runtime.label().into(),
                    sub: status.into(),
                    path: found
                        .as_ref()
                        .map(|found| found.path().display().to_string())
                        .unwrap_or_default()
                        .into(),
                    action: if this_downloading {
                        "".into()
                    } else {
                        offer.unwrap_or_default().into()
                    },
                    action_on: downloading.is_none(),
                    status: if this_downloading {
                        t!("runtimes-downloading").into()
                    } else {
                        "".into()
                    },
                    status_tone: 0,
                    remove: if own && !this_downloading {
                        if confirming {
                            t!("really-remove").into()
                        } else {
                            t!("remove").into()
                        }
                    } else {
                        "".into()
                    },
                    remove_on: true,
                    remove_danger: confirming,
                    ..SetRow::default()
                }
            })
            .collect()
    }

    /// Writes the card.
    pub(super) fn show_runtimes(&self, data: &SettingsData<'_>) {
        let state = &self.runtimes;
        state.model.set_vec(self.runtime_rows());
        let (note, tone) = if state.asking.get() {
            (t!("runtimes-asking"), 3)
        } else if let Some(err) = state.error.borrow().as_ref() {
            (err.clone(), 2)
        } else if let Some((note, good)) = state.note.borrow().as_ref() {
            (note.clone(), note_tone(*good))
        } else {
            (String::new(), 0)
        };
        data.set_runtimes_note(note.into());
        data.set_runtimes_note_tone(tone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Themes;

    fn page() -> (MainWindow, Rc<Glue>) {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, Themes::plain());
        glue.connect(&window);
        (window, glue)
    }

    fn remote(runtime: Runtime) -> Remote {
        Remote {
            runtime,
            url: "https://example.com/eac_runtime.tar.xz".into(),
            version: "2024-01-14T19:38:26.671443Z".into(),
        }
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (_window, glue) = page();
        glue.runtimes_latest(Ok(vec![remote(Runtime::Eac), remote(Runtime::BattlEye)]));
        glue.download_runtime(Runtime::Eac);
        glue.download_runtime(Runtime::BattlEye);
        assert_eq!(glue.runtimes.downloading.get(), Some(Runtime::Eac));
        glue.runtime_done(Runtime::Eac, Err("could not reach github.com".into()));
        assert_eq!(glue.runtimes.downloading.get(), None);
        assert_eq!(
            *glue.runtimes.note.borrow(),
            Some(("could not reach github.com".into(), false))
        );
    }

    #[test]
    fn needs_lutris_list_to_download() {
        let (_window, glue) = page();
        glue.download_runtime(Runtime::Eac);
        assert_eq!(glue.runtimes.downloading.get(), None);
    }

    #[test]
    fn remove_asks_once_first() {
        let (_window, glue) = page();
        glue.remove_runtime(Runtime::BattlEye);
        assert_eq!(glue.runtimes.confirm_remove.get(), Some(Runtime::BattlEye));
        assert!(glue.runtimes.note.borrow().is_none());
        glue.remove_runtime(Runtime::BattlEye);
        assert_eq!(glue.runtimes.confirm_remove.get(), None);
        assert_eq!(
            *glue.runtimes.note.borrow(),
            Some(("Removed Zendo's copy of the BattlEye runtime".into(), true))
        );
    }

    #[test]
    fn shows_the_day_of_a_build() {
        assert_eq!(day("2024-01-14T19:38:26.671443Z"), "2024-01-14");
        assert_eq!(day("soon"), "soon");
    }
}
