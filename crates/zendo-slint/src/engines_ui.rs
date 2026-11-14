//! The Engines card of Settings: legendary, gogdl, nile, comet and
//! umu-launcher, each with the copy found now and whose it is, Zendo's own
//! release if it has one, and the latest on GitHub (asked once a session, and
//! again on asking). Download, Update and Remove work on Zendo's own folder,
//! one download at a time. Where each is found is kept in state and read
//! again when the page opens and after a change, never as the page is drawn.
//! A part of the Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::config::Config;
use zendo_core::engines::{self, Engine, Located, Origin, Release};
use zendo_shell::t;

use super::settings::{bar_of, note_tone};
use super::{Glue, with_glue};
use crate::{MainWindow, SetRow, SettingsData};

/// How often a download's bar is redrawn: a bar redrawn on every chunk
/// would bury the window in events.
const BAR_EVERY: Duration = Duration::from_millis(100);

/// What is known of the latest release of an engine.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Latest {
    Asking,
    Known(Release),
    Failed(String),
}

/// An engine as the card shows it.
struct Row {
    engine: Engine,
    /// The copy in use, whoever's it is.
    found: Option<Located>,
    /// The release of Zendo's own copy.
    installed: Option<String>,
}

struct Download {
    engine: Engine,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct EnginesState {
    rows: RefCell<Vec<Row>>,
    latest: RefCell<HashMap<Engine, Latest>>,
    /// Asked this session.
    asked: Cell<bool>,
    download: RefCell<Option<Download>>,
    /// The engine whose Remove was pressed once.
    confirm_remove: Cell<Option<Engine>>,
    /// The last thing that happened, and whether it went well.
    note: RefCell<Option<(String, bool)>>,
    model: Rc<VecModel<SetRow>>,
}

impl Glue {
    pub(super) fn connect_engines(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        data.set_engines(ModelRc::from(Rc::clone(&self.engines.model)));
        let glue = Rc::clone(self);
        data.on_engines_check(move || glue.ask_latest());
        let glue = Rc::clone(self);
        data.on_engine_act(move |index| {
            let engine = usize::try_from(index)
                .ok()
                .and_then(|index| glue.engines.rows.borrow().get(index).map(|row| row.engine));
            if let Some(engine) = engine {
                glue.download_engine(engine);
            }
        });
        let glue = Rc::clone(self);
        data.on_engine_remove(move |index| {
            let engine = usize::try_from(index)
                .ok()
                .and_then(|index| glue.engines.rows.borrow().get(index).map(|row| row.engine));
            if let Some(engine) = engine {
                glue.remove_engine(engine);
            }
        });
        let glue = Rc::clone(self);
        data.on_engines_stop(move || {
            if let Some(download) = glue.engines.download.borrow().as_ref() {
                download.stop.store(true, Ordering::Relaxed);
            }
            glue.show_settings();
        });
    }

    /// Looks again at where each engine is found. Nothing is read in tests.
    pub(super) fn refresh_engines(&self) {
        if !self.persist {
            return;
        }
        *self.engines.rows.borrow_mut() = Engine::ALL
            .into_iter()
            .map(|engine| Row {
                engine,
                found: engines::find_with_origin(engine.program()),
                installed: engine.installed(),
            })
            .collect();
        // Whether legendary is there decides the Epic overlay card's buttons.
        self.want_overlay();
    }

    /// Looks at the engines on the machine as Settings opens, and asks
    /// GitHub for the latest releases once a session.
    pub(super) fn want_engines(&self) {
        self.refresh_engines();
        if self.persist && !self.engines.asked.get() {
            self.ask_latest();
        }
    }

    fn ask_latest(&self) {
        self.engines.asked.set(true);
        for engine in Engine::ALL {
            self.engines
                .latest
                .borrow_mut()
                .insert(engine, Latest::Asking);
            if self.persist {
                self.ask(
                    move || engines::latest(engine).map_err(|err| format!("{err:#}")),
                    move |glue, result| glue.engine_latest(engine, result),
                );
            }
        }
        self.show_settings();
    }

    pub(super) fn engine_latest(&self, engine: Engine, result: Result<Release, String>) {
        self.engines.latest.borrow_mut().insert(
            engine,
            match result {
                Ok(release) => Latest::Known(release),
                Err(err) => Latest::Failed(err),
            },
        );
        self.show_settings();
    }

    /// Whether replacing or deleting an engine could pull the program out
    /// from under a game (umu-run lives as long as one). A store job counts
    /// too once the Downloads page is here (item 51b).
    fn engines_busy(&self) -> bool {
        self.game.running.borrow().busy()
    }

    pub(super) fn download_engine(self: &Rc<Self>, engine: Engine) {
        if self.engines.download.borrow().is_some() {
            return;
        }
        let Some(Latest::Known(release)) = self.engines.latest.borrow().get(&engine).cloned()
        else {
            return;
        };
        let own = self
            .engines
            .rows
            .borrow()
            .iter()
            .any(|row| row.engine == engine && row.installed.is_some());
        if own && self.engines_busy() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *self.engines.download.borrow_mut() = Some(Download {
            engine,
            done: 0,
            total: release.asset.size,
            stop: Arc::clone(&stop),
        });
        *self.engines.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let progress = weak.clone();
            let mut last: Option<Instant> = None;
            let result = engines::install(engine, &release, &stop, &mut |done, total| {
                if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                    return;
                }
                last = Some(Instant::now());
                let _ = progress.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.engine_progress(engine, done, total));
                });
            });
            let result = result.map(|_| ()).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.engine_installed(engine, result));
            });
        });
    }

    /// How far a download got: only the bar is written, not the page.
    pub(super) fn engine_progress(&self, engine: Engine, done: u64, total: u64) {
        let bar = {
            let mut download = self.engines.download.borrow_mut();
            let Some(download) = download.as_mut().filter(|d| d.engine == engine) else {
                return;
            };
            download.done = done;
            download.total = total;
            bar_of(
                engine.label(),
                done,
                total,
                download.stop.load(Ordering::Relaxed),
            )
        };
        if let Some(window) = self.window.upgrade() {
            window.global::<SettingsData>().set_engines_bar(bar);
        }
    }

    pub(super) fn engine_installed(&self, engine: Engine, result: Result<(), String>) {
        let stopped = self
            .engines
            .download
            .borrow_mut()
            .take()
            .is_some_and(|download| download.stop.load(Ordering::Relaxed));
        self.refresh_engines();
        *self.engines.note.borrow_mut() = Some(match result {
            Ok(()) => (
                t!(
                    "engines-installed-note",
                    name = engine.label(),
                    tag = engine.installed().unwrap_or_default()
                ),
                true,
            ),
            Err(_) if stopped => (t!("protons-stopped", name = engine.label()), true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    /// Remove, pressed once to ask and again to delete.
    pub(super) fn remove_engine(self: &Rc<Self>, engine: Engine) {
        if self.engines_busy() {
            return;
        }
        if self.engines.confirm_remove.get() != Some(engine) {
            self.engines.confirm_remove.set(Some(engine));
            self.show_settings();
            return;
        }
        self.engines.confirm_remove.set(None);
        if !self.persist {
            self.engine_removed(engine, Ok(()));
            return;
        }
        self.ask(
            move || engines::remove(engine).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.engine_removed(engine, result),
        );
        self.show_settings();
    }

    pub(super) fn engine_removed(&self, engine: Engine, result: Result<(), String>) {
        self.refresh_engines();
        *self.engines.note.borrow_mut() = Some(match result {
            Ok(()) => (t!("engines-removed-note", name = engine.label()), true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    /// The card's rows, with every text and which buttons are there.
    fn engine_rows(&self) -> Vec<SetRow> {
        let busy = self.engines_busy();
        let downloading = self.engines.download.borrow().is_some();
        let latest = self.engines.latest.borrow();
        let confirm = self.engines.confirm_remove.get();
        self.engines
            .rows
            .borrow()
            .iter()
            .map(|row| {
                let engine = row.engine;
                let whose = match row.found.as_ref().map(|found| found.origin) {
                    Some(Origin::Zendo) => t!("engines-found-zendo"),
                    Some(Origin::Path) => t!("engines-found-path"),
                    Some(Origin::Heroic) => t!("engines-found-heroic"),
                    None => t!("engines-missing"),
                };
                let (mut note, mut bad) = match latest.get(&engine) {
                    Some(Latest::Asking) => (t!("protons-asking"), false),
                    Some(Latest::Known(release)) => {
                        (t!("engines-latest", tag = release.tag.as_str()), false)
                    }
                    Some(Latest::Failed(err)) => (err.clone(), true),
                    None => (String::new(), false),
                };
                if busy && row.installed.is_some() {
                    if !note.is_empty() {
                        note.push('\n');
                    }
                    note.push_str(&t!("engines-busy"));
                    bad = false;
                }
                let known = match latest.get(&engine) {
                    Some(Latest::Known(release)) => Some(release),
                    _ => None,
                };
                let (mut action, mut action_on, mut status) = (String::new(), false, String::new());
                match (&row.installed, known) {
                    (Some(tag), Some(release)) if &release.tag != tag => {
                        action = t!("engines-update", tag = release.tag.as_str());
                        action_on = !downloading && !busy;
                    }
                    (Some(_), Some(_)) => status = t!("engines-up-to-date"),
                    (None, Some(release)) => {
                        action = t!("engines-download-tag", tag = release.tag.as_str());
                        action_on = !downloading;
                    }
                    _ => {}
                }
                let confirming = confirm == Some(engine);
                SetRow {
                    title: engine.label().into(),
                    sub: format!("{} · {whose}", engine.purpose()).into(),
                    path: row
                        .found
                        .as_ref()
                        .map(|found| found.path.display().to_string())
                        .unwrap_or_default()
                        .into(),
                    note: note.into(),
                    note_tone: if bad { 2 } else { 0 },
                    action: action.into(),
                    action_on,
                    remove: if row.installed.is_none() {
                        "".into()
                    } else if confirming {
                        t!("really-remove").into()
                    } else {
                        t!("remove").into()
                    },
                    remove_on: !busy,
                    remove_danger: confirming,
                    status: status.into(),
                    status_tone: 1,
                }
            })
            .collect()
    }

    /// Writes the card.
    pub(super) fn show_engines(&self, data: &SettingsData<'_>, config: &Config) {
        data.set_path_first(config.engines_on_path_first);
        self.engines.model.set_vec(self.engine_rows());
        let bar = self
            .engines
            .download
            .borrow()
            .as_ref()
            .map(|download| {
                bar_of(
                    download.engine.label(),
                    download.done,
                    download.total,
                    download.stop.load(Ordering::Relaxed),
                )
            })
            .unwrap_or_default();
        data.set_engines_bar(bar);
        let (note, good) = self.engines.note.borrow().clone().unwrap_or_default();
        data.set_engines_note(note.into());
        data.set_engines_note_tone(note_tone(good));
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::engines::Asset;

    use super::*;
    use crate::theme::Themes;

    fn page() -> (MainWindow, Rc<Glue>) {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, Themes::plain());
        glue.connect(&window);
        (window, glue)
    }

    fn release(tag: &str) -> Release {
        Release {
            tag: tag.into(),
            asset: Asset {
                name: "legendary_linux_x64".into(),
                url: String::new(),
                size: 1000,
                sha256: "0".repeat(64),
            },
            service: None,
        }
    }

    fn known(glue: &Glue, engine: Engine) {
        glue.engine_latest(engine, Ok(release("0.21.1")));
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (_window, glue) = page();
        // Nothing is downloaded before the latest release is known.
        glue.download_engine(Engine::Legendary);
        assert!(glue.engines.download.borrow().is_none());
        known(&glue, Engine::Legendary);
        known(&glue, Engine::Nile);
        glue.download_engine(Engine::Legendary);
        glue.download_engine(Engine::Nile);
        assert_eq!(
            glue.engines.download.borrow().as_ref().map(|d| d.engine),
            Some(Engine::Legendary)
        );
        glue.engine_progress(Engine::Legendary, 500, 1000);
        assert_eq!(
            glue.engines.download.borrow().as_ref().map(|d| d.done),
            Some(500)
        );
        if let Some(download) = glue.engines.download.borrow().as_ref() {
            download.stop.store(true, Ordering::Relaxed);
        }
        glue.engine_installed(Engine::Legendary, Err("stopped".into()));
        assert!(glue.engines.download.borrow().is_none());
        assert_eq!(
            *glue.engines.note.borrow(),
            Some(("Stopped downloading legendary".into(), true))
        );
        glue.download_engine(Engine::Nile);
        glue.engine_installed(
            Engine::Nile,
            Err("the download of nile does not match its checksum".into()),
        );
        assert_eq!(
            *glue.engines.note.borrow(),
            Some((
                "the download of nile does not match its checksum".into(),
                false
            ))
        );
    }

    #[test]
    fn remove_asks_once_first() {
        let (_window, glue) = page();
        glue.remove_engine(Engine::Comet);
        assert_eq!(glue.engines.confirm_remove.get(), Some(Engine::Comet));
        assert!(glue.engines.note.borrow().is_none());
        glue.remove_engine(Engine::Comet);
        assert_eq!(glue.engines.confirm_remove.get(), None);
        assert_eq!(
            *glue.engines.note.borrow(),
            Some(("Removed Zendo's copy of comet".into(), true))
        );
    }

    #[test]
    fn keeps_what_it_learns_of_each_engines_latest_release() {
        let (_window, glue) = page();
        known(&glue, Engine::Legendary);
        glue.engine_latest(Engine::Gogdl, Err("could not reach GitHub".into()));
        let latest = glue.engines.latest.borrow();
        assert!(matches!(
            latest.get(&Engine::Legendary),
            Some(Latest::Known(release)) if release.tag == "0.21.1"
        ));
        assert!(matches!(
            latest.get(&Engine::Gogdl),
            Some(Latest::Failed(_))
        ));
    }

    #[test]
    fn a_row_offers_what_the_latest_release_allows() {
        let (_window, glue) = page();
        *glue.engines.rows.borrow_mut() = vec![Row {
            engine: Engine::Legendary,
            found: None,
            installed: Some("0.20.0".into()),
        }];
        known(&glue, Engine::Legendary);
        let rows = glue.engine_rows();
        assert_eq!(rows[0].action, "Update to 0.21.1");
        assert!(rows[0].action_on);
        assert_eq!(rows[0].remove, "Remove");
    }
}
