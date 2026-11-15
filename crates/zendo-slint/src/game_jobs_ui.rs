//! What an installed game's page does with the queue: Update in the action
//! bar, the line saying which job is queued for the game, and the More row
//! (Check for updates, Repair, Move and Uninstall for a game Zendo installed,
//! with Uninstall pressed twice), and the DLC card's Install, Install all and
//! Uninstall. The jobs are the queue's (`downloads_ui`); a Steam game's Verify
//! and Uninstall are asked of Steam (`steam_installs_ui`). A part of the
//! Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use slint::ComponentHandle;
use zendo_core::Game;
use zendo_core::downloads::{Action, Job};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::queue::{self, UpdateState};
use zendo_shell::t;

use super::downloads::FolderFor;
use super::{Glue, with_glue};
use crate::{GameData, MainWindow};

/// What the page keeps while it is open on a game; folded on each game.
pub(super) struct JobsState {
    /// The More row is open.
    more: Cell<bool>,
    /// The DLC whose Uninstall was pressed once.
    pub(super) confirm_dlc: RefCell<Option<String>>,
}

impl JobsState {
    pub(super) fn new() -> Self {
        Self {
            more: Cell::new(false),
            confirm_dlc: RefCell::new(None),
        }
    }
}

impl Glue {
    pub(super) fn connect_game_jobs(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameData>();
        let glue = Rc::clone(self);
        data.on_update(move || glue.game_job(Action::Update));
        let glue = Rc::clone(self);
        data.on_repair(move || glue.game_job(Action::Repair));
        let glue = Rc::clone(self);
        data.on_uninstall(move || glue.uninstall_pressed());
        let glue = Rc::clone(self);
        data.on_move_game(move || glue.move_open());
        let glue = Rc::clone(self);
        data.on_check_updates(move || {
            if let Some(game) = glue.open_game_record() {
                glue.check_updates(&[game.id]);
            }
        });
        let glue = Rc::clone(self);
        data.on_toggle_more(move || {
            glue.jobs.more.set(!glue.jobs.more.get());
            glue.show_game();
        });
        let glue = Rc::clone(self);
        data.on_see_downloads(move || glue.navigate(crate::Page::Downloads));
        let glue = Rc::clone(self);
        data.on_dlc_action(move |id| glue.dlc_pressed(&id));
        let glue = Rc::clone(self);
        data.on_dlc_all(move || glue.dlc_install_all());
    }

    /// The page opens on another game, or closes: everything starts folded
    /// and unasked.
    pub(super) fn reset_game_jobs(&self) {
        self.jobs.more.set(false);
        *self.jobs.confirm_dlc.borrow_mut() = None;
        self.downloads.queue.borrow_mut().confirm_uninstall = None;
    }

    /// Queues a job, unless the game has one already (its buttons are off
    /// then, so this only guards a key or a double press).
    fn queue_for_game(self: &Rc<Self>, game: &Game, job: Option<Job>) {
        let Some(job) = job else {
            return;
        };
        if self.downloads.queue.borrow().job_for(&game.id).is_some() {
            return;
        }
        *self.jobs.confirm_dlc.borrow_mut() = None;
        let effects = self.downloads.queue.borrow_mut().enqueue(job);
        self.queue_apply(effects);
    }

    /// Update or Repair on the open game.
    fn game_job(self: &Rc<Self>, action: Action) {
        let Some(game) = self.open_game_record() else {
            return;
        };
        let job = Job::for_game(&game, action);
        self.queue_for_game(&game, job);
    }

    /// Uninstall: asks once more first, then queues it.
    fn uninstall_pressed(self: &Rc<Self>) {
        let Some(game) = self.open_game_record() else {
            return;
        };
        if self.downloads.queue.borrow().job_for(&game.id).is_some() {
            return;
        }
        let confirming =
            self.downloads.queue.borrow().confirm_uninstall.as_deref() == Some(game.id.as_str());
        if confirming {
            let job = Job::for_game(&game, Action::Uninstall);
            self.queue_for_game(&game, job);
        } else {
            self.downloads.queue.borrow_mut().confirm_uninstall = Some(game.id);
            self.show_game();
        }
    }

    /// Move…: the folder dialog opens, unless the game is running.
    fn move_open(&self) {
        let Some(game) = self.open_game_record() else {
            return;
        };
        if self.game.running.borrow().is_open(&game.id) {
            self.say(t!("downloads-move-running", title = game.title.as_str()));
            return;
        }
        if !self.persist || self.downloads.queue.borrow().job_for(&game.id).is_some() {
            return;
        }
        let weak = self.window.clone();
        let title = t!("downloads-move-to", title = game.title.as_str());
        let id = game.id;
        std::thread::spawn(move || {
            let result = file_dialog::pick(Kind::Folder, &title).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.folder_chosen(FolderFor::Move, &id, result));
            });
        });
    }

    /// The DLC's button: Install, or Uninstall, which asks once more first.
    fn dlc_pressed(self: &Rc<Self>, id: &str) {
        let Some(game) = self.open_game_record() else {
            return;
        };
        let installed = game.details.dlc.iter().any(|dlc| dlc.id == id);
        if !installed {
            let job = Job::for_dlc(&game, Action::AddDlc, vec![id.to_owned()]);
            self.queue_for_game(&game, job);
            return;
        }
        if self.jobs.confirm_dlc.borrow().as_deref() == Some(id) {
            let job = Job::for_dlc(&game, Action::RemoveDlc, vec![id.to_owned()]);
            self.queue_for_game(&game, job);
        } else {
            *self.jobs.confirm_dlc.borrow_mut() = Some(id.to_owned());
            self.show_game();
        }
    }

    /// Install all: every DLC the store says can be installed and is not.
    fn dlc_install_all(self: &Rc<Self>) {
        let Some(game) = self.open_game_record() else {
            return;
        };
        let have: HashSet<&str> = game.details.dlc.iter().map(|dlc| dlc.id.as_str()).collect();
        let wanted: Vec<String> = self
            .installable_dlc(&game.id)
            .into_iter()
            .map(|dlc| dlc.id)
            .filter(|id| !have.contains(id.as_str()))
            .collect();
        let job = Job::for_dlc(&game, Action::AddDlc, wanted);
        self.queue_for_game(&game, job);
    }

    /// Writes the buttons that act on the queue: Update, More and what is in
    /// it, and the job queued for the game in the line under the bar.
    pub(super) fn show_game_jobs(&self, game: &Game, data: &GameData<'_>) {
        let queue = self.downloads.queue.borrow();
        let managed = game.installed_by_zendo();
        let job = queue.job_for(&game.id);
        let update =
            managed && matches!(queue.updates.get(&game.id), Some(UpdateState::Available(_)));
        data.set_update_label(
            if update {
                t!("action-label", action = "update")
            } else {
                String::new()
            }
            .into(),
        );
        data.set_update_on(job.is_none());
        let steam = super::game::steam_app(game);
        data.set_has_more(managed || steam.is_some());
        data.set_more_open(self.jobs.more.get());
        data.set_managed(managed);
        data.set_job_free(job.is_none());
        data.set_check_on(queue.updates.get(&game.id) != Some(&UpdateState::Checking));
        data.set_move_on(job.is_none() && !self.game.running.borrow().is_open(&game.id));
        data.set_uninstall_again(queue.confirm_uninstall.as_deref() == Some(game.id.as_str()));
        data.set_job_shown(job.is_some());
        if let Some((job, running)) = job {
            let state = match (running, job.paused) {
                (true, _) => "now",
                (false, true) => "paused",
                (false, false) => "queued",
            };
            data.set_line(
                t!(
                    "downloads-job-state",
                    doing = queue::action_doing_alone(job.action),
                    state = state
                )
                .into(),
            );
        }
        drop(queue);
        self.show_steam_buttons(steam, data);
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use zendo_core::info::{Dlc, GameInfo, Update};
    use zendo_core::{InstalledDlc, Launch, Library, Sort, Source};
    use zendo_shell::store_facts::InfoState;

    use super::super::tests::page;
    use super::*;

    /// A window on the page of Sugar, an Epic game Zendo installed, with one
    /// DLC installed and three more the store says the user owns.
    fn sugar_page() -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(0);
        let mut sugar = Game::new(
            "epic:Sugar".into(),
            "Sugar".into(),
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        sugar.details.install_path = Some("/home/me/Games/Sugar".into());
        sugar.details.dlc = vec![InstalledDlc {
            id: "d1".into(),
            title: Some("Skins".into()),
        }];
        let library = Library {
            games: vec![sugar],
            problems: Vec::new(),
        };
        let outcome =
            glue.browser
                .borrow_mut()
                .loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        glue.apply(outcome);
        let dlc = |id: &str, installable: bool| Dlc {
            id: id.into(),
            title: format!("DLC {id}"),
            installable,
            download_size: None,
            install_size: Some(1_000_000),
        };
        let info = GameInfo {
            dlc: vec![
                dlc("d1", true),
                dlc("d2", true),
                dlc("d3", true),
                dlc("d4", false),
            ],
            ..GameInfo::default()
        };
        glue.store
            .browser
            .borrow_mut()
            .info
            .insert("epic:Sugar".into(), InfoState::Got(info));
        glue.open_game("epic:Sugar");
        (window, glue)
    }

    fn actions(glue: &Rc<Glue>) -> Vec<Action> {
        glue.downloads
            .queue
            .borrow()
            .queue
            .iter()
            .map(|job| job.action)
            .collect()
    }

    /// The job done: the queue is empty again and the page says so.
    fn clear_queue(glue: &Rc<Glue>) {
        glue.downloads.queue.borrow_mut().queue.clear();
        glue.queue_changed();
    }

    #[test]
    fn update_and_the_more_row_queue_the_right_jobs() {
        let (window, glue) = sugar_page();
        let data = window.global::<GameData>();
        assert!(data.get_has_more());
        assert!(!data.get_more_open());
        assert_eq!(data.get_update_label(), "", "no update found yet");
        assert!(!data.get_job_shown());
        data.invoke_toggle_more();
        assert!(data.get_more_open());
        assert!(data.get_managed());
        assert!(data.get_job_free() && data.get_move_on() && data.get_check_on());
        assert_eq!(data.get_steam_verify_label(), "", "not a Steam game");

        // An update found: the bar offers it, and the details say so.
        glue.updates_checked(vec![(
            "epic:Sugar".into(),
            Ok(Update::Available {
                version: Some("2".into()),
            }),
        )]);
        assert_eq!(
            data.get_update_label(),
            t!("action-label", action = "update")
        );
        let updates = data
            .get_details()
            .iter()
            .find(|fact| fact.label == t!("fact-updates"))
            .expect("the Updates row");
        assert_eq!(updates.tone, 4);
        data.invoke_update();
        assert_eq!(actions(&glue), [Action::Update]);
        // The game has a job: its line says which, with See downloads, and
        // the buttons that would queue another are off.
        assert!(data.get_job_shown());
        assert!(!data.get_update_on() && !data.get_job_free() && !data.get_move_on());
        assert_eq!(
            data.get_line(),
            t!(
                "downloads-job-state",
                doing = queue::action_doing_alone(Action::Update),
                state = "queued"
            )
        );
        data.invoke_repair();
        data.invoke_uninstall();
        assert_eq!(actions(&glue), [Action::Update], "one job per game");
        assert!(!data.get_uninstall_again());

        // Uninstall asks once more first.
        clear_queue(&glue);
        data.invoke_uninstall();
        assert!(data.get_uninstall_again());
        assert_eq!(actions(&glue), []);
        data.invoke_uninstall();
        assert_eq!(actions(&glue), [Action::Uninstall]);
        assert!(!data.get_uninstall_again());

        clear_queue(&glue);
        data.invoke_repair();
        assert_eq!(actions(&glue), [Action::Repair]);

        // Check for updates marks the game as being asked.
        clear_queue(&glue);
        data.invoke_check_updates();
        assert!(!data.get_check_on());

        // Move queues once the folder is chosen.
        clear_queue(&glue);
        glue.folder_chosen(FolderFor::Move, "epic:Sugar", Ok(Some("/mnt/games".into())));
        assert_eq!(actions(&glue), [Action::Move]);
        assert_eq!(
            glue.downloads.queue.borrow().queue[0].moved_to(),
            Some("/mnt/games/Sugar".into())
        );
    }

    #[test]
    fn the_page_opens_folded_and_unasked() {
        let (window, glue) = sugar_page();
        let data = window.global::<GameData>();
        data.invoke_toggle_more();
        data.invoke_uninstall();
        data.invoke_dlc_action("d1".into());
        assert!(data.get_more_open() && data.get_uninstall_again());
        assert!(glue.jobs.confirm_dlc.borrow().is_some());
        // Leaving and coming back starts over.
        glue.close_game();
        glue.open_game("epic:Sugar");
        assert!(!data.get_more_open() && !data.get_uninstall_again());
        assert!(glue.jobs.confirm_dlc.borrow().is_none());
        assert!(
            glue.downloads.queue.borrow().confirm_uninstall.is_none(),
            "{:?}",
            glue.downloads.queue.borrow().confirm_uninstall
        );
    }

    #[test]
    fn the_dlc_card_installs_and_uninstalls_through_the_queue() {
        let (window, glue) = sugar_page();
        let data = window.global::<GameData>();
        assert_eq!(
            data.get_dlc_count(),
            t!("dlc-count", installed = 1, total = 3)
        );
        assert_eq!(data.get_dlc_note(), "", "Zendo manages these");
        let labels: Vec<String> = data
            .get_dlc()
            .iter()
            .map(|row| row.label.to_string())
            .collect();
        assert_eq!(
            labels,
            [
                t!("action-label", action = "uninstall"),
                t!("install"),
                t!("install"),
                String::new(),
            ]
        );
        let last = data.get_dlc().row_data(3).expect("the fourth");
        assert_eq!(last.about, t!("dlc-comes-with"));
        assert_eq!(data.get_dlc_install_all(), t!("dlc-install-all"));

        data.invoke_dlc_action("d2".into());
        let job = glue.downloads.queue.borrow().queue[0].clone();
        assert_eq!(
            (job.action, job.dlc.clone()),
            (Action::AddDlc, vec!["d2".to_owned()])
        );
        // The card is off while the game has a job.
        assert!(data.get_dlc().iter().all(|row| !row.enabled));
        assert!(!data.get_dlc_install_all_on());

        clear_queue(&glue);
        data.invoke_dlc_all();
        let job = glue.downloads.queue.borrow().queue[0].clone();
        assert_eq!(job.dlc, ["d2", "d3"]);

        // Uninstall asks once more first.
        clear_queue(&glue);
        data.invoke_dlc_action("d1".into());
        assert_eq!(actions(&glue), []);
        let row = data.get_dlc().row_data(0).expect("the first");
        assert!(row.danger);
        assert_eq!(row.label, t!("downloads-press-again"));
        data.invoke_dlc_action("d1".into());
        let job = glue.downloads.queue.borrow().queue[0].clone();
        assert_eq!(
            (job.action, job.dlc.clone()),
            (Action::RemoveDlc, vec!["d1".to_owned()])
        );
    }
}
