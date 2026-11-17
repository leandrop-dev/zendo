//! The Launchers group of Settings: the programs that need a window of their
//! own. Battle.net, which Zendo brings as Blizzard's installer run in a
//! prefix under the games folder, and Nexus Launcher, brought as its
//! `AppImage`. Their sign-in and their games stay with them; the Accounts page
//! opens them too. A part of the Library's glue (`library_ui`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::{battlenet, xodus};
use zendo_shell::t;

use super::settings::{bar_of, note_tone};
use super::{Glue, with_glue};
use crate::{MainWindow, SetRow, SettingsData};

/// How often a download's bar is redrawn.
const BAR_EVERY: Duration = Duration::from_millis(150);

/// Which launcher a download or a row is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Launcher {
    BattleNet,
    Nexus,
}

struct Download {
    which: Launcher,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

/// What Battle.net's row shows, looked at as the page opens.
#[derive(Default)]
struct Battlenet {
    /// The prefix and the client found.
    installed: Option<(PathBuf, PathBuf)>,
    games: usize,
    /// The installer is downloaded and ready to run.
    ready: bool,
    /// The installer's window is open.
    installing: bool,
    /// What the last action on this row said.
    note: Option<(String, bool)>,
}

/// What Nexus Launcher's row shows.
#[derive(Default)]
struct Nexus {
    /// The `AppImage` Zendo downloaded.
    appimage: Option<PathBuf>,
    games: usize,
    /// The launcher's window is open.
    running: bool,
    /// What the last action on this row said.
    note: Option<(String, bool)>,
}

pub(super) struct SourcesState {
    battlenet: RefCell<Battlenet>,
    nexus: RefCell<Nexus>,
    download: RefCell<Option<Download>>,
    model: Rc<VecModel<SetRow>>,
}

impl SourcesState {
    pub(super) fn new() -> Self {
        Self {
            battlenet: RefCell::default(),
            nexus: RefCell::default(),
            download: RefCell::default(),
            model: Rc::new(VecModel::default()),
        }
    }
}

impl Glue {
    pub(super) fn connect_sources(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        data.set_launchers(ModelRc::from(Rc::clone(&self.sources.model)));
        let glue = Rc::clone(self);
        data.on_launcher_act(move |index| glue.launcher_act(index));
        let glue = Rc::clone(self);
        data.on_launchers_stop(move || {
            if let Some(download) = glue.sources.download.borrow().as_ref() {
                download.stop.store(true, Ordering::Relaxed);
            }
            glue.show_settings();
        });
    }

    /// Looks at both launchers again: Battle.net's prefix, client, games and
    /// installer, and the `AppImage` and games of Nexus Launcher's.
    pub(super) fn want_sources(&self) {
        if !self.persist {
            return;
        }
        {
            let mut battlenet = self.sources.battlenet.borrow_mut();
            battlenet.installed = battlenet::installed();
            battlenet.games = battlenet::count();
            battlenet.ready = battlenet::installer_path().is_some_and(|path| path.is_file());
        }
        {
            let mut nexus = self.sources.nexus.borrow_mut();
            nexus.appimage = xodus::installed();
            nexus.games = xodus::count();
        }
        self.show_sources();
    }

    /// A row's one button: open a client, run an installer, or bring one.
    fn launcher_act(self: &Rc<Self>, index: i32) {
        match index {
            0 => self.battlenet_act(),
            1 => self.nexus_act(),
            _ => {}
        }
    }

    fn battlenet_act(self: &Rc<Self>) {
        if self.sources.battlenet.borrow().installed.is_some() {
            self.open_battlenet();
        } else if self.sources.battlenet.borrow().ready {
            self.run_battlenet_installer();
        } else {
            self.download(Launcher::BattleNet);
        }
    }

    fn nexus_act(self: &Rc<Self>) {
        if self.sources.nexus.borrow().appimage.is_some() {
            self.open_nexus();
        } else {
            self.download(Launcher::Nexus);
        }
    }

    fn open_battlenet(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        self.sources.battlenet.borrow_mut().note =
            Some(match battlenet::open_client() {
                Ok(()) => (t!("battlenet-opened"), true),
                Err(err) => (format!("{err:#}"), false),
            });
        self.show_settings();
    }

    /// Opens Nexus Launcher, which writes its profile the first time and
    /// needs its runtime and a sign-in done in its own window.
    fn open_nexus(self: &Rc<Self>) {
        if !self.persist || self.sources.nexus.borrow().running {
            return;
        }
        match xodus::open() {
            Ok(mut child) => {
                {
                    let mut nexus = self.sources.nexus.borrow_mut();
                    nexus.running = true;
                    nexus.note = Some((t!("xbox-running"), true));
                }
                self.show_settings();
                let weak = self.window.clone();
                std::thread::spawn(move || {
                    let _ = child.wait();
                    let _ = weak.upgrade_in_event_loop(|_| with_glue(Glue::nexus_done));
                });
            }
            Err(err) => {
                self.sources.nexus.borrow_mut().note = Some((format!("{err:#}"), false));
                self.show_settings();
            }
        }
    }

    /// The launcher's window closed: look at its profile and the library
    /// again, which may have gained games.
    fn nexus_done(self: &Rc<Self>) {
        {
            let mut nexus = self.sources.nexus.borrow_mut();
            nexus.running = false;
            nexus.note = None;
        }
        self.want_sources();
        self.rescan();
    }

    /// Brings a launcher: Blizzard's installer, or the Nexus `AppImage`.
    fn download(self: &Rc<Self>, which: Launcher) {
        if self.sources.download.borrow().is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *self.sources.download.borrow_mut() = Some(Download {
            which,
            done: 0,
            total: 0,
            stop: Arc::clone(&stop),
        });
        self.show_settings();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let progress = weak.clone();
            let mut last: Option<Instant> = None;
            let mut report = |done: u64, total: u64| {
                if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                    return;
                }
                last = Some(Instant::now());
                let _ = progress.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.progress(done, total));
                });
            };
            let result = match which {
                Launcher::BattleNet => battlenet::download_installer(&stop, &mut |done, total| {
                    report(done, total.unwrap_or(done));
                })
                .map(drop),
                Launcher::Nexus => xodus::release()
                    .and_then(|release| xodus::install(&release, &stop, &mut report))
                    .map(drop),
            }
            .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.downloaded(which, result));
            });
        });
    }

    fn progress(&self, done: u64, total: u64) {
        if let Some(download) = self.sources.download.borrow_mut().as_mut() {
            download.done = done;
            download.total = total;
        }
        self.show_sources();
    }

    fn downloaded(self: &Rc<Self>, which: Launcher, result: Result<(), String>) {
        *self.sources.download.borrow_mut() = None;
        let note = match result {
            Ok(()) => match which {
                Launcher::BattleNet => {
                    self.sources.battlenet.borrow_mut().ready = true;
                    (t!("battlenet-downloaded"), true)
                }
                Launcher::Nexus => {
                    self.sources.nexus.borrow_mut().appimage = xodus::installed();
                    (t!("xbox-downloaded"), true)
                }
            },
            Err(err) => (err, false),
        };
        match which {
            Launcher::BattleNet => self.sources.battlenet.borrow_mut().note = Some(note),
            Launcher::Nexus => self.sources.nexus.borrow_mut().note = Some(note),
        }
        self.show_settings();
    }

    fn run_battlenet_installer(self: &Rc<Self>) {
        if self.sources.battlenet.borrow().installing || !self.persist {
            return;
        }
        match battlenet::run_installer() {
            Ok(mut child) => {
                {
                    let mut battlenet = self.sources.battlenet.borrow_mut();
                    battlenet.installing = true;
                    battlenet.note = Some((t!("battlenet-installer-open"), true));
                }
                self.show_settings();
                let weak = self.window.clone();
                std::thread::spawn(move || {
                    let _ = child.wait();
                    let _ =
                        weak.upgrade_in_event_loop(|_| with_glue(Glue::battlenet_installer_done));
                });
            }
            Err(err) => {
                self.sources.battlenet.borrow_mut().note = Some((format!("{err:#}"), false));
                self.show_settings();
            }
        }
    }

    /// The installer's window closed: look at the prefix again, and at the
    /// library, which the client may have given games.
    fn battlenet_installer_done(self: &Rc<Self>) {
        {
            let mut battlenet = self.sources.battlenet.borrow_mut();
            battlenet.installing = false;
            battlenet.note = None;
        }
        self.want_sources();
        self.rescan();
    }

    /// Writes the rows and the bar.
    pub(super) fn show_sources(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<SettingsData>();
        let downloading = self.sources.download.borrow().is_some();
        let (battlenet, nexus) = (
            self.battlenet_row(downloading),
            self.nexus_row(downloading),
        );
        self.sources.model.set_vec(vec![battlenet, nexus]);
        let bar =
            self.sources
                .download
                .borrow()
                .as_ref()
                .map_or_else(crate::Bar::default, |download| {
                    let what = match download.which {
                        Launcher::BattleNet => t!("battlenet-installer"),
                        Launcher::Nexus => t!("xbox-launcher"),
                    };
                    bar_of(
                        &what,
                        download.done,
                        download.total,
                        download.stop.load(Ordering::Relaxed),
                    )
                });
        data.set_launchers_bar(bar);
    }

    /// Battle.net's row.
    fn battlenet_row(&self, downloading: bool) -> SetRow {
        let state = self.sources.battlenet.borrow();
        let games = state.games;
        let installed = state.installed.is_some();
        let (action, action_on) = if installed {
            (t!("battlenet-open"), true)
        } else if state.installing {
            (t!("battlenet-installing"), false)
        } else if state.ready {
            (t!("battlenet-run-installer"), !downloading)
        } else {
            (t!("battlenet-install"), !downloading)
        };
        let (status, status_tone) = match (installed, games) {
            (true, 1..) => (t!("count-games", count = games), 1),
            (true, _) => (t!("battlenet-installed"), 1),
            (false, _) => (t!("battlenet-not-installed"), 0),
        };
        let path = state
            .installed
            .as_ref()
            .map_or_else(String::new, |(prefix, _)| prefix.display().to_string());
        let (note, tone) = state.note.clone().unwrap_or_default();
        row(
            t!("battlenet-title"),
            t!("battlenet-about"),
            path,
            &note.into(),
            note_tone(tone),
            action,
            action_on,
            status,
            status_tone,
        )
    }

    /// Nexus Launcher's row.
    fn nexus_row(&self, downloading: bool) -> SetRow {
        let state = self.sources.nexus.borrow();
        let games = state.games;
        let installed = state.appimage.is_some();
        let (action, action_on) = if installed {
            (t!("xbox-open"), !downloading && !state.running)
        } else {
            (t!("xbox-install"), !downloading)
        };
        let (status, status_tone) = match (installed, games) {
            (true, 1..) => (t!("count-games", count = games), 1),
            (true, _) => (t!("xbox-installed"), 1),
            (false, _) => (t!("xbox-not-installed"), 0),
        };
        let path = state
            .appimage
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string());
        let (note, tone) = state.note.clone().unwrap_or_default();
        row(
            t!("xbox-title"),
            t!("xbox-about"),
            path,
            &note.into(),
            note_tone(tone),
            action,
            action_on,
            status,
            status_tone,
        )
    }
}

/// One row of the group, with all its texts.
#[expect(
    clippy::too_many_arguments,
    reason = "one argument per field of the row"
)]
fn row(
    title: String,
    sub: String,
    path: String,
    note: &slint::SharedString,
    tone: i32,
    action: String,
    action_on: bool,
    status: String,
    status_tone: i32,
) -> SetRow {
    SetRow {
        title: title.into(),
        sub: sub.into(),
        path: path.into(),
        note: note.clone(),
        note_tone: tone,
        action: action.into(),
        action_on,
        remove: "".into(),
        remove_on: false,
        remove_danger: false,
        status: status.into(),
        status_tone,
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;

    use super::super::tests::page;
    use super::*;

    /// A row's note is about that row, and says so once: the section had a
    /// note of its own too, so a downloaded installer's line was drawn three
    /// times.
    #[test]
    fn a_note_belongs_to_the_row_it_is_about() {
        let (window, glue) = page(1);
        glue.downloaded(Launcher::BattleNet, Ok(()));
        let rows = window.global::<SettingsData>().get_launchers();
        assert_eq!(rows.row_count(), 2, "Battle.net and Nexus Launcher");
        let row = rows.row_data(0).unwrap();
        assert_eq!(row.note.as_str(), t!("battlenet-downloaded"));
        assert_eq!(row.action.as_str(), t!("battlenet-run-installer"));
        let other = rows.row_data(1).unwrap();
        assert_eq!(other.note.as_str(), "", "another row's line");
        assert_eq!(other.action.as_str(), t!("xbox-install"));
    }
}
