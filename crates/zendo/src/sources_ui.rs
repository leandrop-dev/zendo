//! The Launchers group of Settings: the programs that need a window of their
//! own. Battle.net today: Zendo downloads Blizzard's installer and runs it in
//! a prefix under the games folder; the sign-in and the games stay with the
//! client, which the Accounts page opens too. A part of the Library's glue
//! (`library_ui`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::battlenet;
use zendo_shell::t;

use super::settings::{bar_of, note_tone};
use super::{Glue, with_glue};
use crate::{MainWindow, SetRow, SettingsData};

/// How often a download's bar is redrawn.
const BAR_EVERY: Duration = Duration::from_millis(150);

struct Download {
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

pub(super) struct SourcesState {
    /// The prefix and client found, looked at as the page opens.
    installed: RefCell<Option<(PathBuf, PathBuf)>>,
    games: RefCell<usize>,
    /// The installer is downloaded and ready to run.
    ready: RefCell<bool>,
    download: RefCell<Option<Download>>,
    /// The installer's window is open.
    installing: RefCell<bool>,
    note: RefCell<Option<(String, bool)>>,
    model: Rc<VecModel<SetRow>>,
}

impl SourcesState {
    pub(super) fn new() -> Self {
        Self {
            installed: RefCell::default(),
            games: RefCell::default(),
            ready: RefCell::default(),
            download: RefCell::default(),
            installing: RefCell::default(),
            note: RefCell::default(),
            model: Rc::new(VecModel::default()),
        }
    }
}

impl Glue {
    pub(super) fn connect_sources(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        data.set_launchers(ModelRc::from(Rc::clone(&self.sources.model)));
        let glue = Rc::clone(self);
        data.on_launcher_act(move |_| glue.launcher_act());
        let glue = Rc::clone(self);
        data.on_launchers_stop(move || {
            if let Some(download) = glue.sources.download.borrow().as_ref() {
                download.stop.store(true, Ordering::Relaxed);
            }
            glue.show_settings();
        });
    }

    /// Looks at Battle.net again: the prefix, the client, the games and the
    /// installer kept.
    pub(super) fn want_sources(&self) {
        if !self.persist {
            return;
        }
        *self.sources.installed.borrow_mut() = battlenet::installed();
        *self.sources.games.borrow_mut() = battlenet::count();
        *self.sources.ready.borrow_mut() =
            battlenet::installer_path().is_some_and(|path| path.is_file());
        self.show_sources();
    }

    /// The row's one button: open the client, run the installer, or bring
    /// the installer.
    fn launcher_act(self: &Rc<Self>) {
        if self.sources.installed.borrow().is_some() {
            self.open_battlenet();
        } else if *self.sources.ready.borrow() {
            self.run_battlenet_installer();
        } else {
            self.download_battlenet();
        }
    }

    fn open_battlenet(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        *self.sources.note.borrow_mut() = Some(match battlenet::open_client() {
            Ok(()) => (t!("battlenet-opened"), true),
            Err(err) => (format!("{err:#}"), false),
        });
        self.show_settings();
    }

    fn download_battlenet(self: &Rc<Self>) {
        if self.sources.download.borrow().is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *self.sources.download.borrow_mut() = Some(Download {
            done: 0,
            total: 0,
            stop: Arc::clone(&stop),
        });
        *self.sources.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let progress = weak.clone();
            let mut last: Option<Instant> = None;
            let result = battlenet::download_installer(&stop, &mut |done, total| {
                if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                    return;
                }
                last = Some(Instant::now());
                let _ = progress.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.battlenet_progress(done, total.unwrap_or(done)));
                });
            })
            .map(drop)
            .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.battlenet_downloaded(result));
            });
        });
    }

    fn battlenet_progress(&self, done: u64, total: u64) {
        if let Some(download) = self.sources.download.borrow_mut().as_mut() {
            download.done = done;
            download.total = total;
        }
        self.show_sources();
    }

    fn battlenet_downloaded(&self, result: Result<(), String>) {
        *self.sources.download.borrow_mut() = None;
        *self.sources.note.borrow_mut() = Some(match result {
            Ok(()) => {
                *self.sources.ready.borrow_mut() = true;
                (t!("battlenet-downloaded"), true)
            }
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    fn run_battlenet_installer(self: &Rc<Self>) {
        if *self.sources.installing.borrow() || !self.persist {
            return;
        }
        match battlenet::run_installer() {
            Ok(mut child) => {
                *self.sources.installing.borrow_mut() = true;
                *self.sources.note.borrow_mut() = Some((t!("battlenet-installer-open"), true));
                self.show_settings();
                let weak = self.window.clone();
                std::thread::spawn(move || {
                    let _ = child.wait();
                    let _ =
                        weak.upgrade_in_event_loop(|_| with_glue(Glue::battlenet_installer_done));
                });
            }
            Err(err) => {
                *self.sources.note.borrow_mut() = Some((format!("{err:#}"), false));
                self.show_settings();
            }
        }
    }

    /// The installer's window closed: look at the prefix again, and at the
    /// library, which the client may have given games.
    fn battlenet_installer_done(self: &Rc<Self>) {
        *self.sources.installing.borrow_mut() = false;
        self.want_sources();
        self.rescan();
    }

    /// Writes the row and the bar.
    pub(super) fn show_sources(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<SettingsData>();
        let installed = self.sources.installed.borrow().clone();
        let games = *self.sources.games.borrow();
        let ready = *self.sources.ready.borrow();
        let installing = *self.sources.installing.borrow();
        let downloading = self.sources.download.borrow().is_some();
        let (action, action_on) = if installed.is_some() {
            (t!("battlenet-open"), true)
        } else if installing {
            (t!("battlenet-installing"), false)
        } else if ready {
            (t!("battlenet-run-installer"), !downloading)
        } else {
            (t!("battlenet-install"), !downloading)
        };
        let (status, status_tone) = match &installed {
            Some(_) if games > 0 => (t!("count-games", count = games), 1),
            Some(_) => (t!("battlenet-installed"), 1),
            None => (t!("battlenet-not-installed"), 0),
        };
        let path = installed
            .as_ref()
            .map_or_else(String::new, |(prefix, _)| prefix.display().to_string());
        let note = self.sources.note.borrow().clone();
        let row = SetRow {
            title: t!("battlenet-title").into(),
            sub: t!("battlenet-about").into(),
            path: path.into(),
            note: note
                .as_ref()
                .map_or_else(String::new, |(text, _)| text.clone())
                .into(),
            note_tone: note.as_ref().map_or(0, |(_, good)| note_tone(*good)),
            action: action.into(),
            action_on,
            remove: "".into(),
            remove_on: false,
            remove_danger: false,
            status: status.into(),
            status_tone,
        };
        self.sources.model.set_vec(vec![row]);
        let bar =
            self.sources
                .download
                .borrow()
                .as_ref()
                .map_or_else(crate::Bar::default, |download| {
                    bar_of(
                        &t!("battlenet-installer"),
                        download.done,
                        download.total,
                        download.stop.load(Ordering::Relaxed),
                    )
                });
        data.set_launchers_bar(bar);
        let (text, good) = note.unwrap_or_default();
        data.set_launchers_note(text.into());
        data.set_launchers_note_tone(note_tone(good));
    }
}
