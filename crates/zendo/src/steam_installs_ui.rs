//! Steam's installs in the window: Install with Steam on a Steam store game's
//! page, what Steam is doing (read from its manifests every few seconds), the
//! Steam card on Downloads, and Verify and Uninstall on an installed Steam
//! game's page. What is followed, and how, is `zendo_shell::steam_installs`,
//! the toolkit-free part; this part carries it to the Slint models and
//! asks Steam on the window's behalf. A part of the Library's glue
//! (`library_ui`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::steam::{AppState, VerifyEvent};
use zendo_shell::steam_installs::{self, Following, SteamInstalls};
use zendo_shell::t;

use super::{Glue, with_glue};
use crate::{DownloadsData, GameData, MainWindow, SteamRow};

/// What the window keeps of Steam's installs.
pub(super) struct SteamInstallState {
    installs: RefCell<SteamInstalls>,
    rows: Rc<VecModel<SteamRow>>,
    /// Looks at Steam's manifests; faster while something is going on.
    timer: slint::Timer,
    every: Cell<Duration>,
}

impl SteamInstallState {
    pub(super) fn new() -> Self {
        Self {
            installs: RefCell::new(SteamInstalls::default()),
            rows: Rc::new(VecModel::default()),
            timer: slint::Timer::default(),
            every: Cell::new(steam_installs::IDLE),
        }
    }

    /// How many installs the Downloads page shows, for the tab's count.
    pub(super) fn count(&self) -> usize {
        self.installs.borrow().count()
    }
}

impl Glue {
    pub(super) fn connect_steam_installs(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<DownloadsData>();
        data.set_steam_rows(ModelRc::from(Rc::clone(&self.steam_installs.rows)));
        let glue = Rc::clone(self);
        data.on_steam_forget(move |app| {
            glue.steam_installs.installs.borrow_mut().forget(&app);
            glue.steam_changed();
        });
        let glue = Rc::clone(self);
        data.on_steam_open(move || glue.open_target("steam://open/downloads"));

        let data = window.global::<GameData>();
        let glue = Rc::clone(self);
        data.on_owned_forget(move || {
            if let Some(app) = glue.open_steam_store_app() {
                glue.steam_installs.installs.borrow_mut().forget(&app);
                glue.steam_changed();
            }
        });
        let glue = Rc::clone(self);
        data.on_steam_verify(move || glue.steam_verify_open());
        let glue = Rc::clone(self);
        data.on_steam_uninstall(move || glue.steam_uninstall_open());
    }

    /// Starts reading Steam's manifests: every 15 seconds, and every 2 while
    /// something is going on.
    pub(super) fn start_steam_installs(self: &Rc<Self>) {
        let state = &self.steam_installs;
        let glue = Rc::downgrade(self);
        state
            .timer
            .start(slint::TimerMode::Repeated, state.every.get(), move || {
                if let Some(glue) = glue.upgrade() {
                    glue.steam_tick();
                }
            });
    }

    /// Looks faster or slower, as Steam is busy or not.
    fn steam_retime(&self) {
        let state = &self.steam_installs;
        let every = state.installs.borrow().every();
        if state.every.replace(every) != every && state.timer.running() {
            state.timer.set_interval(every);
        }
    }

    /// A look is due: Steam's manifests are read on a thread. Tests look at
    /// nothing; they hand `steam_looked` what Steam would have said.
    fn steam_tick(&self) {
        if !self.persist {
            return;
        }
        let Some(from) = self.steam_installs.installs.borrow_mut().begin_look() else {
            return;
        };
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let (states, verifies) = steam_installs::look(from);
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.steam_looked(states, &verifies));
            });
        });
    }

    /// Steam's manifests read: the downloads under way, and what finished.
    pub(super) fn steam_looked(self: &Rc<Self>, states: Vec<AppState>, verifies: &[VerifyEvent]) {
        let looked = self
            .steam_installs
            .installs
            .borrow_mut()
            .looked(states, verifies);
        if let Some(line) = looked.say {
            self.say(line.clone());
            if looked.rescan {
                // Said again once the library is read, which clears the line.
                *self.local.after_scan.borrow_mut() = Some((None, line));
                // Tests read nothing.
                if self.persist {
                    self.rescan();
                }
            }
        }
        self.steam_changed();
    }

    /// Something changed in what Steam is doing: the pages say so.
    fn steam_changed(&self) {
        self.steam_retime();
        self.queue_changed();
    }

    // The Steam game on a store game's page.

    /// The Steam app of the store game whose page is open.
    fn open_steam_store_app(&self) -> Option<String> {
        self.store_open_id()?
            .strip_prefix("steam:")
            .map(str::to_owned)
    }

    /// Install with Steam on a store game: asks Steam's dialog. Says whether
    /// the game is Steam's (anything else is the engines' to install).
    pub(super) fn steam_install(&self, id: &str) -> bool {
        let Some(app) = id.strip_prefix("steam:") else {
            return false;
        };
        let title = self
            .store
            .browser
            .borrow()
            .owned(id)
            .map_or_default(|owned| owned.title.clone());
        let line = self
            .steam_installs
            .installs
            .borrow_mut()
            .install(app, &title, self.persist);
        if let Some(line) = line {
            self.say(line);
        }
        self.steam_changed();
        true
    }

    /// What the page says of a Steam game being installed, in words.
    pub(super) fn steam_install_note(&self, app: &str) -> Option<String> {
        match self.steam_installs.installs.borrow().following(app)? {
            Following::Asked => Some(t!("steam-waiting-dialog")),
            Following::Busy(state) => Some(steam_installs::progress_line(state.progress)),
        }
    }

    /// The main button of a Steam store game that is not installed: its state
    /// (the page's `owned-job` 5 to 7), the words on the button and how far
    /// Steam is (-1 for no bar).
    pub(super) fn steam_install_state(&self, app: &str) -> (i32, String, f32) {
        let installs = self.steam_installs.installs.borrow();
        match installs.following(app) {
            None => (5, t!("store-install-with-steam"), -1.0),
            // Steam writes nothing when its dialog is cancelled: the button
            // asks again, and Stop waiting ends the wait.
            Some(Following::Asked) => (6, t!("steam-ask-again"), -1.0),
            Some(Following::Busy(state)) => (
                7,
                if matches!(state.progress, zendo_core::steam::Progress::Paused { .. }) {
                    t!("steam-paused")
                } else {
                    t!("installing")
                },
                steam_installs::percent(state.progress),
            ),
        }
    }

    // The installed Steam game's page.

    /// Verify with Steam on the open game: Steam shows its own progress.
    fn steam_verify_open(&self) {
        let Some((app, title)) = self.open_steam_game() else {
            return;
        };
        let line = self
            .steam_installs
            .installs
            .borrow_mut()
            .verify(&app, &title, self.persist);
        if let Some(line) = line {
            self.say(line);
        }
        self.steam_changed();
    }

    /// Uninstall with Steam on the open game: Steam asks its own question.
    fn steam_uninstall_open(&self) {
        let Some((app, title)) = self.open_steam_game() else {
            return;
        };
        let line = self
            .steam_installs
            .installs
            .borrow_mut()
            .uninstall(&app, &title, self.persist);
        self.say(line);
        self.steam_changed();
    }

    /// The Steam app and the title of the open game of the library.
    fn open_steam_game(&self) -> Option<(String, String)> {
        let game = self.open_game_record()?;
        let app = super::game::steam_app(&game)?.to_owned();
        Some((app, game.title))
    }

    /// The words and states of Verify and Uninstall on an installed Steam
    /// game's page; empty labels for a game that is not Steam's.
    pub(super) fn show_steam_buttons(&self, app: Option<&str>, data: &GameData<'_>) {
        let (verify, uninstall) = match app {
            Some(app) => {
                let installs = self.steam_installs.installs.borrow();
                (
                    if installs.verifying(app) {
                        t!("steam-verifying")
                    } else {
                        t!("steam-verify")
                    },
                    if installs.removing(app) {
                        t!("steam-ask-again-uninstall")
                    } else {
                        t!("steam-uninstall")
                    },
                )
            }
            None => (String::new(), String::new()),
        };
        let verifying = app.is_some_and(|app| self.steam_installs.installs.borrow().verifying(app));
        data.set_steam_verify_label(verify.into());
        data.set_steam_verify_on(!verifying);
        data.set_steam_uninstall_label(uninstall.into());
    }

    // The Downloads page's Steam card.

    /// Writes the card's rows, and whether the card shows.
    pub(super) fn show_steam_card(&self, data: &DownloadsData<'_>) {
        let installs = self.steam_installs.installs.borrow();
        data.set_steam_show(installs.active());
        let rows: Vec<SteamRow> = installs
            .rows()
            .into_iter()
            .map(|row| SteamRow {
                app: row.app.into(),
                title: row.title.into(),
                line: row.line.into(),
                percent: row.percent.unwrap_or(-1.0),
                forget: if row.forget {
                    t!("steam-stop-waiting")
                } else {
                    String::new()
                }
                .into(),
            })
            .collect();
        self.steam_installs.rows.set_vec(rows);
    }

    /// Whether Steam has something going, so Downloads has more to show.
    pub(super) fn steam_active(&self) -> bool {
        self.steam_installs.installs.borrow().active()
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::owned::{Owned, Shop};
    use zendo_core::steam::Progress;

    use super::super::tests::page;
    use super::*;

    /// A window on Portal 2's store page, as Steam's list gives it.
    fn steam_page() -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(0);
        glue.store_cached(vec![(
            Shop::Steam,
            vec![Owned::new(Shop::Steam, "620", "Portal 2".into())],
        )]);
        glue.open_owned("steam:620");
        (window, glue)
    }

    fn state(progress: Progress) -> AppState {
        AppState {
            app: "620".into(),
            name: "Portal 2".into(),
            progress,
        }
    }

    #[test]
    fn install_with_steam_asks_follows_and_finishes() {
        let (window, glue) = steam_page();
        let game = window.global::<GameData>();
        let downloads = window.global::<DownloadsData>();
        assert_eq!(game.get_owned_job(), 5);
        assert_eq!(game.get_owned_job_label(), t!("store-install-with-steam"));
        assert!(game.get_owned_progress() < 0.0);
        glue.show_downloads();
        assert!(!downloads.get_steam_show());
        assert!(downloads.get_invite());

        // Asked: Steam's dialog is open, and the page waits on it.
        game.invoke_owned_install();
        assert_eq!(game.get_owned_job(), 6);
        assert_eq!(game.get_owned_job_label(), t!("steam-ask-again"));
        assert_eq!(game.get_owned_note(), t!("steam-waiting-dialog"));
        assert_eq!(
            glue.browser.borrow().status(),
            Some(t!("steam-asked", title = "Portal 2").as_str())
        );
        assert!(downloads.get_steam_show());
        assert!(!downloads.get_invite(), "Steam has something going");
        assert_eq!(window.get_downloads_count(), "1");
        let row = downloads.get_steam_rows().row_data(0).expect("a row");
        assert_eq!(row.title, "Portal 2");
        assert_eq!(row.forget, t!("steam-stop-waiting"));
        // Asked again, the dialog opens again and it is still one install.
        game.invoke_owned_install();
        assert_eq!(downloads.get_steam_rows().row_count(), 1);

        // Stop waiting, on the page.
        game.invoke_owned_forget();
        assert_eq!(game.get_owned_job(), 5);
        assert!(!downloads.get_steam_show());
        assert_eq!(window.get_downloads_count(), "");

        // Enter asks too, and once.
        glue.owned_activate();
        assert_eq!(game.get_owned_job(), 6);
        // Stop waiting, on the card.
        downloads.invoke_steam_forget("620".into());
        assert_eq!(game.get_owned_job(), 5);
        assert_eq!(downloads.get_steam_rows().row_count(), 0);

        // Steam starts: its manifest says how far.
        glue.owned_activate();
        let downloading = Progress::Downloading {
            done: 10,
            total: 100,
        };
        glue.steam_looked(vec![state(downloading)], &[]);
        assert_eq!(game.get_owned_job(), 7);
        assert_eq!(game.get_owned_job_label(), t!("installing"));
        assert!((game.get_owned_progress() - 10.0).abs() < 0.01);
        assert_eq!(
            game.get_owned_note(),
            zendo_shell::steam_installs::progress_line(downloading)
        );
        let row = downloads.get_steam_rows().row_data(0).expect("a row");
        assert!((row.percent - 10.0).abs() < 0.01);
        assert_eq!(
            row.forget, "",
            "Stop waiting is for a dialog, not a download"
        );
        assert_eq!(window.get_downloads_count(), "1");
        // Steam is on it: the button asks nothing.
        game.invoke_owned_install();
        assert_eq!(game.get_owned_job(), 7);
        let paused = Progress::Paused {
            done: 20,
            total: 100,
        };
        glue.steam_looked(vec![state(paused)], &[]);
        assert_eq!(game.get_owned_job_label(), t!("steam-paused"));

        // Installed: the line says so and the library is to be read again,
        // which says it once more (tests read nothing).
        glue.steam_looked(vec![state(Progress::Installed)], &[]);
        assert!(!downloads.get_steam_show());
        assert_eq!(window.get_downloads_count(), "");
        assert_eq!(
            glue.browser.borrow().status(),
            Some(t!("steam-installed", title = "Portal 2").as_str())
        );
        assert_eq!(
            *glue.local.after_scan.borrow(),
            Some((None, t!("steam-installed", title = "Portal 2")))
        );
    }

    #[test]
    fn what_steam_does_by_itself_shows_on_downloads() {
        let (window, glue) = page(0);
        let downloads = window.global::<DownloadsData>();
        // The first look only learns what is there; a download Steam started
        // shows with its bar, and no Stop waiting.
        glue.steam_looked(Vec::new(), &[]);
        glue.steam_looked(
            vec![state(Progress::Downloading {
                done: 50,
                total: 100,
            })],
            &[],
        );
        assert!(downloads.get_steam_show());
        assert!(!downloads.get_invite());
        let row = downloads.get_steam_rows().row_data(0).expect("a row");
        assert_eq!(row.app, "620");
        assert!((row.percent - 50.0).abs() < 0.01);
        assert_eq!(glue.steam_installs.every.get(), steam_installs::BUSY);
        // Steam cancelled it: nothing was uninstalled, nothing is shown.
        glue.steam_looked(Vec::new(), &[]);
        assert!(!downloads.get_steam_show());
        assert_eq!(glue.steam_installs.every.get(), steam_installs::IDLE);
    }
}
