//! The Downloads page and the queue behind it, in the window. The queue's
//! state and rules are `zendo_shell::queue`'s;
//! what is here carries its effects out (the job's thread, the files it
//! keeps, the lines it says) and writes what the page shows. A part of the
//! Library's glue (`library_ui`), which owns the games and the stores.
//!
//! The store game's Install and Import are here too: the page's buttons
//! follow what the queue holds of the game.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use zendo_core::Game;
use zendo_core::Size;
use zendo_core::config::Config;
use zendo_core::downloads::{self, Action, Event, Job, Stop};
use zendo_core::info::{Dlc, Update};
use zendo_core::owned::{Owned, Shop};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::queue::{self, Effect, Queue, Report, Then};
use zendo_shell::store_facts::InfoState;
use zendo_shell::t;

use super::{Glue, with_glue};
use crate::{DownloadsData, GameData, JobRow, MainWindow, Page};

/// How long the page waits, after a job's progress, to draw it: the engine
/// writes its log many times a second.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// What a folder from the dialog is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FolderFor {
    /// Where a store game already is.
    Import,
    /// Where an installed game's folder goes.
    Move,
}

/// What the page keeps of the queue.
pub(super) struct DownloadsState {
    pub(super) queue: RefCell<Queue>,
    /// A redraw of the progress is already waiting for more to come.
    progress_due: Cell<bool>,
    /// The store games whose "Also install its DLC" was unticked.
    skip_dlc: RefCell<HashSet<String>>,
    up_next: Rc<VecModel<JobRow>>,
    paused: Rc<VecModel<JobRow>>,
    finished: Rc<VecModel<JobRow>>,
}

impl DownloadsState {
    pub(super) fn new() -> Self {
        Self {
            queue: RefCell::new(Queue::default()),
            progress_due: Cell::new(false),
            skip_dlc: RefCell::new(HashSet::new()),
            up_next: Rc::new(VecModel::default()),
            paused: Rc::new(VecModel::default()),
            finished: Rc::new(VecModel::default()),
        }
    }
}

/// Hands a job's report to the window's thread.
fn post(weak: &Weak<MainWindow>, report: Report) -> bool {
    weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.job_report(report)))
        .is_ok()
}

fn row(job: &Job) -> JobRow {
    JobRow {
        id: job.id().into(),
        title: job.title.clone().into(),
        line: format!("{} · {}", queue::action_label(job.action), job.shop.label()).into(),
        note: "".into(),
        error: "".into(),
        verdict: "".into(),
        verdict_bad: false,
        first: "".into(),
        first_primary: false,
        cancel: "".into(),
        open: "".into(),
    }
}

impl Glue {
    /// Hooks the page's and the store game's callbacks up.
    pub(super) fn connect_downloads(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<DownloadsData>();
        data.set_up_next(ModelRc::from(Rc::clone(&self.downloads.up_next)));
        data.set_paused(ModelRc::from(Rc::clone(&self.downloads.paused)));
        data.set_finished(ModelRc::from(Rc::clone(&self.downloads.finished)));
        let glue = Rc::clone(self);
        data.on_pause(move |id| {
            let id = glue.job_id(&id);
            let effects = glue.downloads.queue.borrow_mut().pause(&id);
            glue.queue_apply(effects);
        });
        let glue = Rc::clone(self);
        data.on_resume(move |id| {
            let effects = glue.downloads.queue.borrow_mut().resume(&id);
            glue.queue_apply(effects);
        });
        let glue = Rc::clone(self);
        data.on_cancel(move |id| {
            let id = glue.job_id(&id);
            let effects = glue.downloads.queue.borrow_mut().cancel(&id);
            glue.queue_apply(effects);
        });
        let glue = Rc::clone(self);
        data.on_clear(move || {
            glue.downloads.queue.borrow_mut().clear_finished();
            glue.queue_changed();
        });
        let glue = Rc::clone(self);
        data.on_open(move |id| glue.open_finished(&id));
        let glue = Rc::clone(self);
        data.on_go_store(move || glue.navigate(Page::Store));
        let glue = Rc::clone(self);
        data.on_close_anyway(move || glue.close_anyway());
        let glue = Rc::clone(self);
        data.on_stay_open(move || {
            glue.downloads.queue.borrow_mut().stay_open();
            glue.queue_changed();
        });
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_downloads());

        let data = window.global::<GameData>();
        let glue = Rc::clone(self);
        data.on_owned_install(move || {
            // A Steam game is installed by Steam's own dialog.
            if glue
                .store_open_id()
                .is_some_and(|id| glue.steam_install(&id))
            {
                return;
            }
            let job = glue.store_open_id().and_then(|id| glue.install_job(&id));
            if let Some(job) = job {
                let effects = glue.downloads.queue.borrow_mut().enqueue(job);
                glue.queue_apply(effects);
            }
        });
        let glue = Rc::clone(self);
        data.on_owned_import(move || glue.owned_import());
        let glue = Rc::clone(self);
        data.on_owned_resume(move || {
            if let Some(id) = glue.store_open_id() {
                let effects = glue.downloads.queue.borrow_mut().resume(&id);
                glue.queue_apply(effects);
            }
        });
        let glue = Rc::clone(self);
        data.on_owned_downloads(move || glue.navigate(Page::Downloads));
        let glue = Rc::clone(self);
        data.on_owned_toggle_dlc(move || {
            if let Some(id) = glue.store_open_id() {
                let mut skipped = glue.downloads.skip_dlc.borrow_mut();
                if !skipped.remove(&id) {
                    skipped.insert(id);
                }
                drop(skipped);
                glue.show_owned();
            }
        });
    }

    /// The id a button on the page means: the running card says "running".
    fn job_id(&self, id: &str) -> String {
        if id == "running" {
            self.downloads
                .queue
                .borrow()
                .queue
                .first()
                .map(Job::id)
                .unwrap_or_default()
        } else {
            id.to_owned()
        }
    }

    // Reading and keeping the queue.

    /// Reads what was left from last time, on a thread.
    pub(super) fn load_queue(self: &Rc<Self>) {
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = downloads::load_queue().map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.queue_loaded(result));
            });
        });
    }

    pub(super) fn queue_loaded(self: &Rc<Self>, result: Result<Vec<Job>, String>) {
        match result {
            Ok(jobs) => {
                let effects = self.downloads.queue.borrow_mut().loaded(jobs);
                self.queue_apply(effects);
            }
            Err(err) => self.say(err),
        }
    }

    fn save_queue(&self) {
        if !self.persist {
            return;
        }
        let saved = downloads::save_queue(&self.downloads.queue.borrow().queue);
        if let Err(err) = saved {
            self.say(format!("{err:#}"));
        }
    }

    /// Does what the queue asked, starts what is due and writes the page.
    pub(super) fn queue_apply(self: &Rc<Self>, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Say(line) => self.say(line),
                Effect::Save => self.save_queue(),
                Effect::Discard(job) if self.persist => {
                    let (title, weak) = (job.title.clone(), self.window.clone());
                    std::thread::spawn(move || {
                        let result = queue::clear_away(&job);
                        let _ = weak.upgrade_in_event_loop(move |_| {
                            with_glue(|glue| glue.discarded(&title, &result));
                        });
                    });
                }
                Effect::Rescan if self.persist => self.rescan(),
                Effect::Exit if self.persist => {
                    let _ = slint::quit_event_loop();
                }
                Effect::ShowDownloads => self.show_asking(),
                // Started by `next_job` below; the rest do nothing in tests.
                Effect::Start(..) | Effect::Discard(_) | Effect::Rescan | Effect::Exit => {}
            }
        }
        self.next_job();
        self.queue_changed();
    }

    fn discarded(&self, title: &str, result: &Result<(), String>) {
        if let Effect::Say(line) = Queue::discarded(title, result) {
            self.say(line);
        }
    }

    /// Starts the first job not paused, unless one runs. A job that changes a
    /// game's files waits while the game runs, and goes when it closes.
    pub(super) fn next_job(&self) {
        if !self.persist {
            return;
        }
        let started = {
            let running = self.game.running.borrow();
            self.downloads
                .queue
                .borrow_mut()
                .next(|job| queue::changes_files(job.action) && running.is_open(&job.id()))
        };
        if let Some(Effect::Start(job, stop)) = started {
            self.run_job(job, stop);
            self.queue_changed();
        }
    }

    /// Runs the job on a thread and reports every event to the window.
    fn run_job(&self, job: Job, stop: Stop) {
        let weak = self.window.clone();
        queue::run(job, stop, move |report| post(&weak, report));
    }

    /// What a job's thread said. The engine writes its log many times a
    /// second: each progress goes into the queue at once, so the page is
    /// never behind what was said, and only the redraw waits, for a moment
    /// that takes in all that came meanwhile (as the covers' do).
    pub(super) fn job_report(self: &Rc<Self>, report: Report) {
        match report {
            Report::Event(Event::Progress(progress)) => {
                self.downloads
                    .queue
                    .borrow_mut()
                    .event(Event::Progress(progress));
                if !self.downloads.progress_due.replace(true) {
                    slint::Timer::single_shot(PROGRESS_EVERY, || {
                        with_glue(|glue| glue.progress_shown());
                    });
                }
            }
            Report::Event(event) => {
                let effects = self.downloads.queue.borrow_mut().event(event);
                self.queue_apply(effects);
            }
            Report::Ended(result) => {
                let effects = self.downloads.queue.borrow_mut().ended(result);
                self.queue_apply(effects);
            }
        }
    }

    /// The moment after the first progress of a burst: writes the latest.
    pub(super) fn progress_shown(&self) {
        self.downloads.progress_due.set(false);
        self.show_progress();
        self.sync_status();
    }

    /// The running job's progress, as a short line for the footer; or how
    /// many wait paused.
    pub(super) fn queue_summary(&self) -> Option<String> {
        self.downloads.queue.borrow().summary()
    }

    /// Whether the game's files are being moved now, so it must not start.
    pub(super) fn moving(&self, id: &str) -> bool {
        self.downloads.queue.borrow().moving(id)
    }

    /// Whether a download runs, so an engine must not be replaced.
    pub(super) fn download_running(&self) -> bool {
        self.downloads.queue.borrow().is_running()
    }

    // The updates.

    /// Asks the stores, on a thread, about the games Zendo installed that
    /// were not asked about yet.
    pub(super) fn check_new_updates(&self) {
        let ids = self
            .downloads
            .queue
            .borrow()
            .unasked(self.browser.borrow().games().iter());
        self.check_updates(&ids);
    }

    /// Asks the stores, on a thread, whether these games have updates (Check
    /// for updates asks about one).
    pub(super) fn check_updates(&self, ids: &[String]) {
        let games: Vec<Game> = {
            let browser = self.browser.borrow();
            self.downloads
                .queue
                .borrow_mut()
                .to_check(ids.iter().filter_map(|id| browser.game(id)))
        };
        if games.is_empty() {
            return;
        }
        // The pages say the games are being asked about; tests ask nothing.
        self.queue_changed();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let results = queue::ask_updates(&games);
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.updates_checked(results));
            });
        });
    }

    pub(super) fn updates_checked(self: &Rc<Self>, results: Vec<(String, Result<Update, String>)>) {
        let auto = self.auto_update();
        let effects = {
            let browser = self.browser.borrow();
            self.downloads
                .queue
                .borrow_mut()
                .checked(results, auto, |id| browser.game(id).cloned())
        };
        self.queue_apply(effects);
    }

    /// Queues the updates already found, as Settings' switch turns on.
    pub(super) fn update_by_themselves(self: &Rc<Self>) {
        let auto = self.auto_update();
        let effects = {
            let browser = self.browser.borrow();
            let mut queue = self.downloads.queue.borrow_mut();
            let found = queue.available();
            queue.update_by_themselves(&found, auto, |id| browser.game(id).cloned())
        };
        self.queue_apply(effects);
    }

    fn auto_update(&self) -> bool {
        self.config
            .borrow()
            .as_ref()
            .is_some_and(|config| config.auto_update)
    }

    // Closing.

    /// The window is to close for good: says whether it may. While a job
    /// runs it may not: the page asks first.
    pub(super) fn quit_requested(self: &Rc<Self>) -> bool {
        let effects = self.downloads.queue.borrow_mut().close_requested();
        if effects.iter().any(|effect| matches!(effect, Effect::Exit)) {
            return true;
        }
        self.show_asking();
        false
    }

    /// Shows the question the queue asked.
    fn show_asking(&self) {
        let on_page = self
            .window
            .upgrade()
            .is_some_and(|window| window.get_page() == Page::Downloads);
        if !on_page {
            self.navigate(Page::Downloads);
        }
        self.queue_changed();
    }

    /// Whether the page is asking to close, so Enter and A answer it.
    pub(super) fn asking(&self) -> bool {
        self.downloads.queue.borrow().confirm_close
            && self
                .window
                .upgrade()
                .is_some_and(|window| window.get_page() == Page::Downloads)
    }

    /// Close anyway: the engine stops, and the window goes after it.
    pub(super) fn close_anyway(self: &Rc<Self>) {
        let effects = self.downloads.queue.borrow_mut().close_anyway();
        self.queue_apply(effects);
    }

    /// Keep downloading, from Escape or the pad's B. Says whether the
    /// question was up.
    pub(super) fn stay_open(&self) -> bool {
        if !self.asking() {
            return false;
        }
        self.downloads.queue.borrow_mut().stay_open();
        self.queue_changed();
        true
    }

    // Writing the page.

    /// The queue changed: the page, the footer, the tab's count and the open
    /// game's page (a store game's, or an installed one with its buttons)
    /// say so.
    pub(super) fn queue_changed(&self) {
        self.show_downloads();
        self.sync_status();
        self.show_game();
    }

    /// Writes the page, the tab's count and the footer's line.
    pub(super) fn show_downloads(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let queue = self.downloads.queue.borrow();
        let data = window.global::<DownloadsData>();
        let count = queue.len() + self.steam_installs.count();
        window.set_downloads_count(
            if count == 0 {
                String::new()
            } else {
                count.to_string()
            }
            .into(),
        );

        let start = usize::from(queue.running.is_some());
        let rest = || queue.queue.iter().skip(start);
        let running = self.game.running.borrow();
        let up_next: Vec<JobRow> = rest()
            .filter(|job| !job.paused)
            .map(|job| JobRow {
                note: if queue::changes_files(job.action) && running.is_open(&job.id()) {
                    t!("downloads-waits-running").into()
                } else {
                    "".into()
                },
                first: t!("pause").into(),
                cancel: t!("cancel").into(),
                ..row(job)
            })
            .collect();
        let paused: Vec<JobRow> = rest()
            .filter(|job| job.paused)
            .map(|job| {
                let error = queue.errors.get(&job.id());
                JobRow {
                    error: error
                        .map(|error| t!("downloads-failed", error = error.as_str()))
                        .unwrap_or_default()
                        .into(),
                    first: if error.is_some() {
                        t!("try-again")
                    } else {
                        t!("resume")
                    }
                    .into(),
                    first_primary: true,
                    cancel: t!("cancel").into(),
                    ..row(job)
                }
            })
            .collect();
        let finished: Vec<JobRow> = queue
            .finished
            .iter()
            .rev()
            .map(|(job, outcome)| JobRow {
                verdict: match outcome {
                    Ok(()) => t!("downloads-done"),
                    Err(err) => err.clone(),
                }
                .into(),
                verdict_bad: outcome.is_err(),
                open: if outcome.is_ok() && job.action != Action::Uninstall {
                    t!("open").into()
                } else {
                    "".into()
                },
                ..row(job)
            })
            .collect();
        data.set_empty(queue.queue.is_empty() && queue.finished.is_empty());
        data.set_invite(
            queue.queue.is_empty()
                && queue.finished.is_empty()
                && !self.steam_active()
                && !self.nexus_downloading(),
        );
        self.show_steam_card(&data);
        // As in Iced, the running job counts as waiting its turn's end.
        data.set_up_next_count(queue.len().saturating_sub(paused.len()).to_string().into());
        data.set_paused_count(paused.len().to_string().into());
        data.set_finished_count(finished.len().to_string().into());
        data.set_asking(queue.confirm_close);
        self.downloads.up_next.set_vec(up_next);
        self.downloads.paused.set_vec(paused);
        self.downloads.finished.set_vec(finished);
        drop(running);
        Self::show_running(&data, &queue);
    }

    /// Writes the running job's card.
    fn show_running(data: &DownloadsData<'_>, queue: &Queue) {
        let (Some(running), Some(job)) = (&queue.running, queue.queue.first()) else {
            data.set_running(false);
            return;
        };
        data.set_running(true);
        data.set_current_title(job.title.clone().into());
        data.set_current_line(
            format!("{} · {}", queue::action_doing(job.action), job.shop.label()).into(),
        );
        data.set_percent(running.progress.percent.unwrap_or(-1.0));
        data.set_progress_line(
            running
                .step
                .clone()
                .unwrap_or_else(|| queue::progress_line(job, &running.progress))
                .into(),
        );
        data.set_pause_label(
            if running.then == Some(Then::Pause) {
                t!("pausing")
            } else {
                t!("pause")
            }
            .into(),
        );
        data.set_pause_on(running.then.is_none());
        data.set_cancel_label(
            if running.then == Some(Then::Cancel) {
                t!("cancelling")
            } else {
                t!("cancel")
            }
            .into(),
        );
        data.set_cancel_on(running.then != Some(Then::Cancel));
    }

    /// Only the running card's bar and line: the engine's log is quick.
    fn show_progress(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let queue = self.downloads.queue.borrow();
        let data = window.global::<DownloadsData>();
        Self::show_running(&data, &queue);
    }

    /// Open on a finished job: the game's page, in the library when the scan
    /// has it, otherwise in the Store.
    fn open_finished(&self, id: &str) {
        if self.browser.borrow().game(id).is_some() {
            self.navigate(Page::Library);
            self.open_game(id);
        } else if self.store.browser.borrow().owned(id).is_some() {
            self.navigate(Page::Store);
            self.store.browser.borrow_mut().click(id);
            self.open_owned(id);
        }
    }

    // A store game's Install and Import.

    /// The DLC the store says the user owns and can install for a game.
    pub(super) fn installable_dlc(&self, id: &str) -> Vec<Dlc> {
        match self.store.browser.borrow().info.get(id) {
            Some(InfoState::Got(info)) => info
                .dlc
                .iter()
                .filter(|dlc| dlc.installable)
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Installing this store game where Settings says, unless it is queued,
    /// with its DLC unless "Also install its DLC" was unticked.
    pub(super) fn install_job(&self, id: &str) -> Option<Job> {
        if self.downloads.queue.borrow().job_for(id).is_some() {
            return None;
        }
        let dlc = if self.downloads.skip_dlc.borrow().contains(id) {
            Vec::new()
        } else {
            self.installable_dlc(id)
                .into_iter()
                .map(|dlc| dlc.id)
                .collect()
        };
        let dir = self
            .config
            .borrow()
            .as_ref()
            .and_then(Config::install_dir)?;
        let browser = self.store.browser.borrow();
        let owned = browser.owned(id)?;
        if owned.shop == Shop::Xbox && (!zendo_core::xodus::ready() || zendo_core::xodus::running())
        {
            return None;
        }
        Job::install(owned, dir, dlc)
    }

    /// Writes the buttons under a store game's title: Install and Import, or
    /// what the queue is doing with it.
    pub(super) fn show_owned_install(&self, data: &GameData<'_>, owned: &Owned, installed: bool) {
        let queue = self.downloads.queue.borrow();
        let mut label = String::new();
        let mut progress = -1.0;
        let state = if installed {
            0
        } else if let Some(app) = owned.id.strip_prefix("steam:") {
            // Steam's own dialog does it; the page follows Steam's manifest.
            let (state, words, bar) = self.steam_install_state(app);
            label = words;
            progress = bar;
            state
        } else if let Some((job, running)) = queue.job_for(&owned.id) {
            match (running, job.paused) {
                (true, _) => {
                    label = queue::action_doing_alone(job.action);
                    3
                }
                (false, true) => 4,
                (false, false) => 2,
            }
        } else {
            i32::from(self.install_job(&owned.id).is_some())
        };
        data.set_owned_job(state);
        data.set_owned_job_label(label.into());
        data.set_owned_progress(progress);
        let dlc = if state == 1 {
            self.installable_dlc(&owned.id)
        } else {
            Vec::new()
        };
        let bytes: u64 = dlc.iter().filter_map(|dlc| dlc.install_size).sum();
        let words = if dlc.is_empty() {
            String::new()
        } else if bytes > 0 {
            t!(
                "store-with-dlc-size",
                count = dlc.len(),
                size = Size::Bytes(bytes).to_string()
            )
        } else {
            t!("store-with-dlc", count = dlc.len())
        };
        data.set_owned_dlc(words.into());
        data.set_owned_dlc_on(!self.downloads.skip_dlc.borrow().contains(&owned.id));
    }

    /// Enter or A on a store game's page: an installed game opens in the
    /// library, any other is queued to install; a Steam game's is asked of
    /// Steam, unless Steam is on it.
    pub(super) fn owned_activate(self: &Rc<Self>) {
        let Some(id) = self.store_open_id() else {
            return;
        };
        if self.store.browser.borrow().is_installed(&id) {
            self.open_in_library();
        } else if id.starts_with("steam:") {
            self.steam_install(&id);
        } else if let Some(job) = self.install_job(&id) {
            let effects = self.downloads.queue.borrow_mut().enqueue(job);
            self.queue_apply(effects);
        }
    }

    /// Import: the folder dialog opens, and the folder chosen is where the
    /// store game already is.
    fn owned_import(&self) {
        if !self.persist {
            return;
        }
        let Some(id) = self.store_open_id() else {
            return;
        };
        let Some(title) = self
            .store
            .browser
            .borrow()
            .owned(&id)
            .map(|owned| owned.title.clone())
        else {
            return;
        };
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = file_dialog::pick(
                Kind::Folder,
                &t!("downloads-import-where", title = title.as_str()),
            )
            .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.folder_chosen(FolderFor::Import, &id, result));
            });
        });
    }

    /// The folder the dialog gave: queues the import or the move with it.
    pub(super) fn folder_chosen(
        self: &Rc<Self>,
        purpose: FolderFor,
        id: &str,
        chosen: Result<Option<PathBuf>, String>,
    ) {
        match chosen {
            Ok(Some(folder)) => {
                let job = match purpose {
                    FolderFor::Import => self
                        .store
                        .browser
                        .borrow()
                        .owned(id)
                        .and_then(|owned| Job::import(owned, folder)),
                    FolderFor::Move => self
                        .browser
                        .borrow()
                        .game(id)
                        .and_then(|game| Job::for_move(game, folder)),
                };
                if let Some(job) = job {
                    let effects = self.downloads.queue.borrow_mut().enqueue(job);
                    self.queue_apply(effects);
                }
            }
            Ok(None) => {}
            Err(err) => self.say(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use slint::platform::Key;
    use zendo_core::downloads::{Ended, Progress};
    use zendo_core::info::GameInfo;
    use zendo_core::owned::Shop;
    use zendo_core::stores::Store;
    use zendo_core::{Launch, Library, Sort, Source};
    use zendo_shell::queue::Running;

    use super::super::tests::page;
    use super::*;
    use crate::LibraryData;

    /// A window on the Store with Epic signed in and one game, Beta Racer,
    /// and a folder to install into.
    fn store_page() -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(0);
        glue.store_cached(vec![(
            Shop::Epic,
            vec![Owned::new(Store::Epic, "b", "Beta Racer".into())],
        )]);
        glue.account_checked(Store::Epic, Ok(Some("me".into())), None);
        glue.store_scanned();
        *glue.config.borrow_mut() = Some(config(true));
        window.invoke_page_requested(Page::Store);
        (window, glue)
    }

    /// Settings with a folder to install into and the updates by themselves
    /// on or off.
    fn config(auto_update: bool) -> Config {
        let mut config = Config::default();
        config.install_dir = Some("/home/me/Games".into());
        config.auto_update = auto_update;
        config
    }

    /// The queue's first job is running, as `next_job` leaves it outside
    /// tests.
    fn run_first(glue: &Rc<Glue>) {
        glue.downloads.queue.borrow_mut().running = Some(Running {
            stop: Stop::default(),
            progress: Progress::default(),
            step: None,
            then: None,
        });
        glue.queue_changed();
    }

    fn ids(model: &ModelRc<JobRow>) -> Vec<String> {
        model.iter().map(|job| job.id.to_string()).collect()
    }

    #[test]
    fn install_queues_a_store_game_once_and_its_buttons_follow_the_queue() {
        let (window, glue) = store_page();
        let game = window.global::<GameData>();
        let data = window.global::<DownloadsData>();
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        assert_eq!(game.get_owned_job(), 1, "it can be installed");
        assert!(game.get_owned_note().contains("/home/me/Games"));

        game.invoke_owned_install();
        game.invoke_owned_install();
        assert_eq!(glue.downloads.queue.borrow().len(), 1, "once");
        assert_eq!(game.get_owned_job(), 2, "queued");
        assert_eq!(ids(&data.get_up_next()), ["epic:b"]);
        assert_eq!(window.get_downloads_count(), "1");
        assert!(!data.get_empty());

        run_first(&glue);
        assert_eq!(game.get_owned_job(), 3, "running");
        assert_eq!(
            game.get_owned_job_label(),
            t!("action-doing", action = "install")
        );
        assert!(data.get_running());
        assert_eq!(data.get_current_title(), "Beta Racer");
        assert_eq!(data.get_up_next_count(), "1", "the running job counts");
        assert_eq!(data.get_percent(), -1.0, "no bar before a percentage");
        assert!(data.get_pause_on() && data.get_cancel_on());

        // The engine says how far it is; the footer shows it once no line
        // of Zendo's own stands there (the status wins, as in Iced).
        glue.browser.borrow_mut().say(None);
        glue.job_report(Report::Event(Event::Progress(Progress {
            percent: Some(42.0),
            ..Progress::default()
        })));
        // The queue has it at once; the page draws it a moment later, once
        // for all that came meanwhile.
        assert_eq!(data.get_percent(), -1.0);
        assert!(glue.downloads.progress_due.get());
        glue.job_report(Report::Event(Event::Progress(Progress {
            percent: Some(43.0),
            downloaded_mib: Some(10.0),
            ..Progress::default()
        })));
        glue.progress_shown();
        assert!(!glue.downloads.progress_due.get());
        assert_eq!(data.get_percent(), 43.0, "the latest, not the first");
        assert!(data.get_progress_line().contains("10.0 MiB"));
        assert!(
            window.global::<LibraryData>().get_status().contains("43"),
            "the footer says it too"
        );

        // Pause: the engine is asked to stop, and the job waits paused.
        data.invoke_pause("running".into());
        assert_eq!(data.get_pause_label(), t!("pausing"));
        assert!(!data.get_pause_on());
        glue.job_report(Report::Ended(Ok(Ended::Stopped)));
        assert_eq!(game.get_owned_job(), 4, "paused");
        assert_eq!(ids(&data.get_paused()), ["epic:b"]);
        assert_eq!(data.get_paused().row_data(0).unwrap().first, t!("resume"));
        game.invoke_owned_resume();
        assert_eq!(game.get_owned_job(), 2);
        assert_eq!(data.get_paused().row_count(), 0);
    }

    #[test]
    fn enter_on_a_store_games_page_queues_the_install_once() {
        let (window, glue) = store_page();
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        assert!(glue.key(&String::from(char::from(Key::Return)), false, false, false));
        assert_eq!(glue.downloads.queue.borrow().len(), 1);
        assert!(glue.key(&String::from(char::from(Key::Return)), false, false, false));
        assert_eq!(glue.downloads.queue.borrow().len(), 1, "once");
        assert_eq!(window.global::<GameData>().get_owned_job(), 2);
    }

    #[test]
    fn a_failed_job_says_why_and_try_again_asks_once_more() {
        let (window, glue) = store_page();
        let data = window.global::<DownloadsData>();
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        window.global::<GameData>().invoke_owned_install();
        run_first(&glue);
        glue.job_report(Report::Ended(Err("no space".into())));
        let row = data.get_paused().row_data(0).expect("the job waits");
        assert_eq!(row.error, t!("downloads-failed", error = "no space"));
        assert_eq!(row.first, t!("try-again"));
        data.invoke_resume("epic:b".into());
        let row = data.get_up_next().row_data(0).expect("it is next again");
        assert_eq!(row.error, "");
        // Cancel takes it out, and the page keeps what became of it.
        data.invoke_cancel("epic:b".into());
        assert_eq!(glue.downloads.queue.borrow().len(), 0);
        let row = data.get_finished().row_data(0).expect("it is finished");
        assert_eq!(
            (row.verdict.to_string(), row.verdict_bad),
            (t!("downloads-cancelled"), true)
        );
        assert_eq!(row.open, "", "nothing to open");
        data.invoke_clear();
        assert!(data.get_empty());
    }

    #[test]
    fn the_dlc_box_decides_what_comes_with_the_game() {
        let (window, glue) = store_page();
        let game = window.global::<GameData>();
        let info = GameInfo {
            dlc: vec![zendo_core::info::Dlc {
                id: "d2".into(),
                title: "Soundtrack".into(),
                installable: true,
                download_size: None,
                install_size: Some(1_000_000_000),
            }],
            ..GameInfo::default()
        };
        glue.store
            .browser
            .borrow_mut()
            .info
            .insert("epic:b".into(), InfoState::Got(info));
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        assert_ne!(game.get_owned_dlc(), "", "it has DLC to give");
        assert!(game.get_owned_dlc_on());
        assert_eq!(glue.install_job("epic:b").unwrap().dlc, ["d2"]);
        game.invoke_owned_toggle_dlc();
        assert!(!game.get_owned_dlc_on());
        assert_eq!(
            glue.install_job("epic:b").unwrap().dlc,
            Vec::<String>::new()
        );
        game.invoke_owned_toggle_dlc();
        game.invoke_owned_install();
        assert_eq!(glue.downloads.queue.borrow().queue[0].dlc, ["d2"]);
        assert_eq!(game.get_owned_dlc(), "", "no box once it is queued");
    }

    #[test]
    fn closing_while_a_job_runs_asks_first_and_the_keys_answer() {
        let (window, glue) = store_page();
        let data = window.global::<DownloadsData>();
        assert!(glue.quit_requested(), "nothing runs: it may close");
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        window.global::<GameData>().invoke_owned_install();
        run_first(&glue);

        assert!(!glue.quit_requested(), "a download runs: it asks");
        assert_eq!(window.get_page(), Page::Downloads);
        assert!(data.get_asking() && glue.asking());
        // Escape keeps downloading.
        glue.back();
        assert!(!data.get_asking());
        assert!(glue.downloads.queue.borrow().is_running());

        // Enter is Close anyway: the engine is asked to stop, and the queue
        // is held for next time once it has.
        assert!(!glue.quit_requested());
        assert!(glue.key(&String::from(char::from(Key::Return)), false, false, false));
        assert!(glue.downloads.queue.borrow().closing);
        glue.job_report(Report::Ended(Ok(Ended::Stopped)));
        assert!(
            glue.downloads
                .queue
                .borrow()
                .queue
                .iter()
                .all(|job| job.paused)
        );
    }

    #[test]
    fn a_running_download_keeps_the_engines_from_being_replaced() {
        let (window, glue) = store_page();
        window
            .global::<crate::StoreData>()
            .invoke_card_clicked("epic:b".into());
        window.global::<GameData>().invoke_owned_install();
        assert!(!glue.engines_busy(), "queued is not running");
        run_first(&glue);
        assert!(glue.engines_busy());
    }

    #[test]
    fn updates_found_after_a_scan_are_queued_by_themselves_when_set() {
        let (_window, glue) = page(0);
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
        let library = Library {
            games: vec![sugar],
            problems: Vec::new(),
        };
        let outcome =
            glue.browser
                .borrow_mut()
                .loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        glue.apply(outcome);
        let found = || {
            vec![(
                "epic:Sugar".to_owned(),
                Ok(Update::Available {
                    version: Some("2".into()),
                }),
            )]
        };

        // A scan asks about the games Zendo installed, once.
        glue.check_new_updates();
        assert!(
            glue.downloads
                .queue
                .borrow()
                .updates
                .contains_key("epic:Sugar")
        );
        let unasked = glue
            .downloads
            .queue
            .borrow()
            .unasked(glue.browser.borrow().games().iter());
        assert_eq!(unasked, Vec::<String>::new());

        // Off: the answer waits, and turning the switch on queues it.
        *glue.config.borrow_mut() = Some(config(false));
        glue.updates_checked(found());
        assert_eq!(glue.downloads.queue.borrow().len(), 0);
        *glue.config.borrow_mut() = Some(config(true));
        glue.update_by_themselves();
        let actions: Vec<Action> = glue
            .downloads
            .queue
            .borrow()
            .queue
            .iter()
            .map(|job| job.action)
            .collect();
        assert_eq!(actions, [Action::Update]);
        // The same answer again leaves the one job.
        glue.updates_checked(found());
        assert_eq!(glue.downloads.queue.borrow().len(), 1);
    }
}
