//! The Launchers group of Settings: the programs that need a window of their
//! own. Nexus Launcher, brought as its `AppImage`, and Greenlight, which
//! streams an Xbox Series S's screen. Their sign-in and their games stay with
//! them; the Accounts page opens them too. A part of the Library's glue
//! (`library_ui`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::{greenlight, xodus};
use zendo_shell::t;

use super::settings::{bar_of, note_tone};
use super::{Glue, with_glue};
use crate::{MainWindow, SetRow, SettingsData};

/// How often a download's bar is redrawn.
const BAR_EVERY: Duration = Duration::from_millis(150);

/// Which launcher a download or a row is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Launcher {
    Nexus,
    Greenlight,
}

struct Download {
    which: Launcher,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
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

/// What the Xbox Series S's row shows: Greenlight found, and the last word.
#[derive(Default)]
struct Series {
    /// Greenlight's `AppImage`, Flatpak or program.
    program: Option<PathBuf>,
    note: Option<(String, bool)>,
}

pub(super) struct SourcesState {
    nexus: RefCell<Nexus>,
    series: RefCell<Series>,
    download: RefCell<Option<Download>>,
    model: Rc<VecModel<SetRow>>,
}

impl SourcesState {
    pub(super) fn new() -> Self {
        Self {
            nexus: RefCell::default(),
            series: RefCell::default(),
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

    /// Looks at the launchers again: the `AppImage` and games of Nexus
    /// Launcher's, and Greenlight's program.
    pub(super) fn want_sources(&self) {
        if !self.persist {
            return;
        }
        {
            let mut nexus = self.sources.nexus.borrow_mut();
            nexus.appimage = xodus::installed();
            nexus.games = xodus::count();
        }
        self.sources.series.borrow_mut().program = greenlight::installed();
        self.show_sources();
    }

    /// A row's one button: open a launcher, or bring one.
    fn launcher_act(self: &Rc<Self>, index: i32) {
        match index {
            0 => self.nexus_act(),
            1 => self.series_act(),
            _ => {}
        }
    }

    fn nexus_act(self: &Rc<Self>) {
        if self.sources.nexus.borrow().appimage.is_some() {
            self.open_nexus();
        } else {
            self.download(Launcher::Nexus);
        }
    }

    /// Greenlight opens, or its `AppImage` is brought first.
    fn series_act(self: &Rc<Self>) {
        if self.sources.series.borrow().program.is_none() {
            self.download(Launcher::Greenlight);
            return;
        }
        if !self.persist {
            return;
        }
        match greenlight::open() {
            Ok(mut child) => {
                self.sources.series.borrow_mut().note = Some((t!("series-opened"), true));
                self.show_settings();
                let weak = self.window.clone();
                std::thread::spawn(move || {
                    let _ = child.wait();
                    let _ = weak.upgrade_in_event_loop(|_| with_glue(Glue::series_done));
                });
            }
            Err(err) => {
                self.sources.series.borrow_mut().note = Some((format!("{err:#}"), false));
                self.show_settings();
            }
        }
    }

    /// Greenlight's window closed: look at it again, in case it was
    /// installed or removed meanwhile.
    fn series_done(self: &Rc<Self>) {
        self.sources.series.borrow_mut().program = greenlight::installed();
        self.show_sources();
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

    /// Brings a launcher: its `AppImage`.
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
                Launcher::Nexus => xodus::release()
                    .and_then(|release| xodus::install(&release, &stop, &mut report))
                    .map(drop),
                Launcher::Greenlight => greenlight::release()
                    .and_then(|release| greenlight::install(&release, &stop, &mut report))
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
                Launcher::Nexus => {
                    self.sources.nexus.borrow_mut().appimage = xodus::installed();
                    (t!("xbox-downloaded"), true)
                }
                Launcher::Greenlight => {
                    self.sources.series.borrow_mut().program = greenlight::installed();
                    (t!("series-downloaded"), true)
                }
            },
            Err(err) => (err, false),
        };
        match which {
            Launcher::Nexus => self.sources.nexus.borrow_mut().note = Some(note),
            Launcher::Greenlight => self.sources.series.borrow_mut().note = Some(note),
        }
        self.show_settings();
    }

    /// Writes the rows and the bar.
    pub(super) fn show_sources(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<SettingsData>();
        let downloading = self.sources.download.borrow().is_some();
        let (nexus, series) = (self.nexus_row(downloading), self.series_row(downloading));
        self.sources.model.set_vec(vec![nexus, series]);
        let bar =
            self.sources
                .download
                .borrow()
                .as_ref()
                .map_or_else(crate::Bar::default, |download| {
                    let what = match download.which {
                        Launcher::Nexus => t!("xbox-launcher"),
                        Launcher::Greenlight => t!("series-launcher"),
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
            (true, _) => (t!("launcher-installed"), 1),
            (false, _) => (t!("launcher-not-installed"), 0),
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

    /// The Xbox Series S's row: Greenlight, which streams the console's
    /// screen, and the way to it when it is not installed.
    fn series_row(&self, downloading: bool) -> SetRow {
        let state = self.sources.series.borrow();
        let installed = state.program.is_some();
        let (note, tone) = state.note.clone().unwrap_or_default();
        row(
            t!("series-title"),
            t!("series-about"),
            state
                .program
                .as_ref()
                .map_or_else(String::new, |path| path.display().to_string()),
            &note.into(),
            note_tone(tone),
            if installed {
                t!("series-open")
            } else {
                t!("series-download")
            },
            !downloading || installed,
            if installed {
                t!("launcher-installed")
            } else {
                t!("launcher-not-installed")
            },
            i32::from(installed),
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
    /// note of its own too, so a downloaded launcher's line was drawn three
    /// times.
    #[test]
    fn a_note_belongs_to_the_row_it_is_about() {
        let (window, glue) = page(1);
        glue.downloaded(Launcher::Nexus, Ok(()));
        let rows = window.global::<SettingsData>().get_launchers();
        assert_eq!(rows.row_count(), 2, "Nexus Launcher and the Series S");
        let row = rows.row_data(0).unwrap();
        assert_eq!(row.note.as_str(), t!("xbox-downloaded"));
        assert_eq!(
            rows.row_data(1).unwrap().note.as_str(),
            "",
            "the other row's line"
        );
    }
}
