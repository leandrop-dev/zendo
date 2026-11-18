//! The download queue: one job at a time, run by the store's engine as a
//! child of the window, with its progress read from the engine's log. There
//! is no background process: closing the window while something downloads
//! asks first, then pauses everything queued, to be resumed from the
//! Downloads page next time.
//!
//! Pause keeps what was downloaded; Cancel stops and, for an install,
//! deletes it. A job that fails stays in the queue, paused with the reason,
//! so Resume tries again.
//!
//! The state and its transitions are here, with no toolkit: each method
//! takes what happened and says what the window must do about it
//! ([`Effect`]), as [`crate::running`] does for games. The window turns the
//! effects into its own tasks, and calls [`Queue::next`] after each change
//! to start what is due.
//!
//! The queue also keeps whether each game Zendo installed has an update,
//! asked of its store once a session and after each job.

use std::collections::HashMap;
use std::thread;

use zendo_core::Game;
use zendo_core::downloads::{self, Action, Ended, Event, Job, Progress, Stop};
use zendo_core::info::{self, Update};
use zendo_core::stores::Store;

use crate::t;

/// What the window must do after the queue changed.
#[derive(Debug)]
pub enum Effect {
    /// The line at the foot of the window.
    Say(String),
    /// The queue changed: write it to `downloads.json`.
    Save,
    /// Run this job on a thread ([`run`]); it is now at the queue's front.
    Start(Job, Stop),
    /// A cancelled install: clear its files away, on a thread
    /// ([`downloads::discard`]), then say how it went ([`Queue::discarded`]).
    Discard(Job),
    /// A job ended well: the library has something new, read it again.
    Rescan,
    /// Nothing runs and the window was asked to close: close it.
    Exit,
    /// A job runs and the window was asked to close: show the question.
    ShowDownloads,
}

/// Whether the job changes the files of a game that may be running: all but
/// installing and importing one Zendo does not have yet.
#[must_use]
pub fn changes_files(action: Action) -> bool {
    !matches!(action, Action::Install | Action::Import)
}

/// The action's name, as its button says it: Install.
#[must_use]
pub fn action_label(action: Action) -> String {
    t!("action-label", action = action_tag(action))
}

/// The action as it goes on: Installing.
#[must_use]
pub fn action_doing(action: Action) -> String {
    t!("action-doing", action = action_tag(action))
}

/// The action as it goes on, said without the game's title after it, as on
/// the game's own page.
#[must_use]
pub fn action_doing_alone(action: Action) -> String {
    let tag = match action {
        Action::AddDlc => "add-dlc-alone",
        Action::RemoveDlc => "remove-dlc-alone",
        other => action_tag(other),
    };
    t!("action-doing", action = tag)
}

fn action_tag(action: Action) -> &'static str {
    match action {
        Action::Install => "install",
        Action::Update => "update",
        Action::Repair => "repair",
        Action::Uninstall => "uninstall",
        Action::AddDlc => "add-dlc",
        Action::RemoveDlc => "remove-dlc",
        Action::Move => "move",
        Action::Import => "import",
    }
}

#[derive(Debug, Default)]
pub struct Queue {
    /// What is left to do; the first runs while `running` is set.
    pub queue: Vec<Job>,
    pub running: Option<Running>,
    /// What ended since the window opened, the latest last.
    pub finished: Vec<(Job, Result<(), String>)>,
    /// Why a queued job failed, until it is resumed.
    pub errors: HashMap<String, String>,
    /// Closing was asked for while a job ran.
    pub confirm_close: bool,
    /// The user chose to close; the window goes once the engine stops.
    pub closing: bool,
    /// The game whose Uninstall was pressed once.
    pub confirm_uninstall: Option<String>,
    /// Whether each game Zendo installed has an update, by id.
    pub updates: HashMap<String, UpdateState>,
}

/// The job at the queue's front, while its engine runs.
#[derive(Debug)]
pub struct Running {
    pub stop: Stop,
    pub progress: Progress,
    pub step: Option<String>,
    /// What the user asked for, once the engine has stopped.
    pub then: Option<Then>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Pause,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Checking,
    UpToDate,
    Available(Option<String>),
    Failed(String),
}

/// How a line is to be coloured; each toolkit has its own colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Dim,
    Good,
    Accent,
    Bad,
}

/// What the last look for an update found, and its tone.
#[must_use]
pub fn update_line(update: Option<&UpdateState>) -> (String, Tone) {
    match update {
        None => (String::new(), Tone::Dim),
        Some(UpdateState::Checking) => (t!("downloads-checking-updates"), Tone::Dim),
        Some(UpdateState::UpToDate) => (t!("downloads-up-to-date"), Tone::Good),
        Some(UpdateState::Available(Some(version))) => (
            t!("downloads-update-version", version = version.as_str()),
            Tone::Accent,
        ),
        Some(UpdateState::Available(None)) => (t!("downloads-update-available"), Tone::Accent),
        Some(UpdateState::Failed(err)) => (
            t!("downloads-update-failed", error = err.as_str()),
            Tone::Bad,
        ),
    }
}

impl Queue {
    /// The job for this game in the queue, and whether it is the one
    /// running.
    #[must_use]
    pub fn job_for(&self, id: &str) -> Option<(&Job, bool)> {
        self.queue
            .iter()
            .enumerate()
            .find(|(_, job)| job.id() == id)
            .map(|(index, job)| (job, index == 0 && self.running.is_some()))
    }

    /// Whether a download is running now.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Whether the game's files are being moved now, so it must not start.
    #[must_use]
    pub fn moving(&self, id: &str) -> bool {
        self.job_for(id)
            .is_some_and(|(job, running)| running && job.action == Action::Move)
    }

    /// How many jobs wait, the running one included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// How many jobs are held until resumed.
    #[must_use]
    pub fn paused(&self) -> usize {
        self.queue.iter().filter(|job| job.paused).count()
    }

    /// The running job's progress, as a short line for the footer; or how
    /// many wait paused.
    #[must_use]
    pub fn summary(&self) -> Option<String> {
        let Some(running) = &self.running else {
            return match self.paused() {
                0 => None,
                count => Some(t!("downloads-paused-count", count = count)),
            };
        };
        let job = self.queue.first()?;
        Some(match (running.then, running.progress.percent) {
            (Some(Then::Pause), _) => t!("downloads-pausing", title = job.title.as_str()),
            (Some(Then::Cancel), _) => t!("downloads-cancelling", title = job.title.as_str()),
            (None, Some(percent)) => t!(
                "downloads-doing-percent",
                doing = action_doing(job.action),
                title = job.title.as_str(),
                percent = format!("{percent:.0}")
            ),
            (None, None) => t!(
                "downloads-doing-dots",
                doing = action_doing(job.action),
                title = job.title.as_str()
            ),
        })
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.queue.iter().position(|job| job.id() == id)
    }

    /// Whether the job at `index` is the one the engine runs.
    fn runs(&self, index: usize) -> bool {
        index == 0 && self.running.is_some()
    }

    // What the window does to the queue.

    /// The queue left from last time came in. What runs stays at the front;
    /// anything queued meanwhile goes after what was left.
    pub fn loaded(&mut self, jobs: Vec<Job>) -> Vec<Effect> {
        if self.running.is_some() {
            let known: std::collections::HashSet<String> = self.queue.iter().map(Job::id).collect();
            self.queue
                .extend(jobs.into_iter().filter(|job| !known.contains(&job.id())));
            return vec![Effect::Save];
        }
        let added = std::mem::replace(&mut self.queue, jobs);
        added.into_iter().map(|job| self.add(job)).collect()
    }

    /// Queues a job, unless its game is queued already.
    pub fn enqueue(&mut self, job: Job) -> Vec<Effect> {
        self.confirm_uninstall = None;
        vec![self.add(job), Effect::Save]
    }

    /// Adds the job and says what happens to it.
    fn add(&mut self, job: Job) -> Effect {
        let id = job.id();
        if self.queue.iter().any(|queued| queued.id() == id) {
            return Effect::Say(t!("downloads-already-queued", title = job.title.as_str()));
        }
        let ahead = self.queue.iter().filter(|queued| !queued.paused).count();
        let line = match ahead {
            0 => t!(
                "downloads-doing",
                doing = action_doing(job.action),
                title = job.title.as_str()
            ),
            count => t!(
                "downloads-ahead",
                action = action_label(job.action),
                title = job.title.as_str(),
                count = count
            ),
        };
        self.queue.push(job);
        Effect::Say(line)
    }

    /// What the running job reported.
    pub fn event(&mut self, event: Event) -> Vec<Effect> {
        match event {
            // Kept with the job at once, so a crash still finds it.
            Event::Folder(folder) => {
                if let Some(job) = self.queue.first_mut() {
                    job.folder = Some(folder);
                }
                vec![Effect::Save]
            }
            Event::Progress(progress) => {
                if let Some(running) = &mut self.running {
                    running.progress = progress;
                    running.step = None;
                }
                Vec::new()
            }
            Event::Step(step) => {
                if let Some(running) = &mut self.running {
                    running.step = Some(step);
                }
                Vec::new()
            }
        }
    }

    /// Stops the game's job when it runs, to do `then` after; otherwise does
    /// `queued` to it where it waits.
    fn stop_or(&mut self, id: &str, then: Then, queued: impl FnOnce(&mut Self, usize)) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        if self.runs(index) {
            if let Some(running) = &mut self.running {
                running.then = Some(then);
                running.stop.stop();
            }
        } else {
            queued(self, index);
        }
    }

    /// Holds a game's job in the queue, stopping it when it runs.
    pub fn pause(&mut self, id: &str) -> Vec<Effect> {
        self.stop_or(id, Then::Pause, |state, index| {
            state.queue[index].paused = true;
        });
        vec![Effect::Save]
    }

    pub fn resume(&mut self, id: &str) -> Vec<Effect> {
        let Some(index) = self.index_of(id) else {
            return Vec::new();
        };
        self.queue[index].paused = false;
        self.errors.remove(id);
        vec![Effect::Save]
    }

    /// Takes a game's job out of the queue, clearing away a half-done
    /// install; a running one goes once its engine has stopped.
    pub fn cancel(&mut self, id: &str) -> Vec<Effect> {
        let mut removed = None;
        self.stop_or(id, Then::Cancel, |state, index| {
            removed = Some(state.queue.remove(index));
        });
        let mut effects = vec![Effect::Save];
        if let Some(job) = removed {
            effects.extend(self.cancelled(job));
        }
        effects
    }

    /// A cancelled job leaves the queue; an install's files are cleared
    /// away.
    fn cancelled(&mut self, job: Job) -> Vec<Effect> {
        self.errors.remove(&job.id());
        let say = Effect::Say(t!(
            "downloads-cancelled-job",
            doing = action_doing(job.action),
            title = job.title.as_str()
        ));
        self.finished
            .push((job.clone(), Err(t!("downloads-cancelled"))));
        if job.action == Action::Install {
            vec![say, Effect::Discard(job)]
        } else {
            vec![say]
        }
    }

    /// A cancelled install was cleared away.
    #[must_use]
    pub fn discarded(title: &str, result: &Result<(), String>) -> Effect {
        Effect::Say(match result {
            Ok(()) => t!("downloads-cancelled-gone", title = title),
            Err(err) => t!(
                "downloads-cancelled-but",
                title = title,
                error = err.as_str()
            ),
        })
    }

    /// The window was asked to close: with a job running the question is
    /// asked first.
    pub fn close_requested(&mut self) -> Vec<Effect> {
        if self.running.is_some() {
            self.confirm_close = true;
            vec![Effect::ShowDownloads]
        } else {
            vec![Effect::Exit]
        }
    }

    /// Close anyway: the engine stops, and the window goes after it.
    pub fn close_anyway(&mut self) -> Vec<Effect> {
        self.confirm_close = false;
        match &self.running {
            Some(running) => {
                self.closing = true;
                running.stop.stop();
                vec![Effect::Say(t!("downloads-pausing-closing"))]
            }
            None => vec![Effect::Exit],
        }
    }

    pub fn stay_open(&mut self) {
        self.confirm_close = false;
    }

    pub fn clear_finished(&mut self) {
        self.finished.clear();
    }

    /// Starts the first job not paused, unless one runs; it moves to the
    /// front of the queue. `held` says which jobs wait: one that changes a
    /// game's files waits while the game runs, and goes when it closes.
    pub fn next(&mut self, held: impl Fn(&Job) -> bool) -> Option<Effect> {
        if self.running.is_some() || self.closing {
            return None;
        }
        let index = self
            .queue
            .iter()
            .position(|job| !job.paused && !held(job))?;
        let job = self.queue.remove(index);
        self.queue.insert(0, job.clone());
        let stop = Stop::default();
        self.running = Some(Running {
            stop: stop.clone(),
            progress: Progress::default(),
            step: None,
            then: None,
        });
        Some(Effect::Start(job, stop))
    }

    /// The running job's engine ended.
    pub fn ended(&mut self, result: Result<Ended, String>) -> Vec<Effect> {
        let then = self.running.take().and_then(|running| running.then);
        // Closing pauses the whole queue, to be resumed by hand next time.
        if self.closing {
            for job in &mut self.queue {
                job.paused = true;
            }
            return vec![Effect::Save, Effect::Exit];
        }
        if self.queue.is_empty() {
            return Vec::new();
        }
        match (result, then) {
            (Ok(Ended::Stopped), Some(Then::Cancel)) => {
                let job = self.queue.remove(0);
                let mut effects = vec![Effect::Save];
                effects.extend(self.cancelled(job));
                effects
            }
            (Ok(Ended::Stopped), _) => {
                let job = &mut self.queue[0];
                job.paused = true;
                let say = t!(
                    "downloads-paused-job",
                    doing = action_doing(job.action),
                    title = job.title.as_str()
                );
                vec![Effect::Save, Effect::Say(say)]
            }
            (Ok(Ended::Done), _) => {
                let job = self.queue.remove(0);
                let say = t!(
                    "downloads-done-job",
                    action = action_label(job.action),
                    title = job.title.as_str()
                );
                // Asked again once the library has it back.
                self.updates.remove(&job.id());
                self.finished.push((job, Ok(())));
                vec![Effect::Save, Effect::Say(say), Effect::Rescan]
            }
            (Err(err), _) => {
                let first = &self.queue[0];
                let say = t!(
                    "downloads-failed-job",
                    action = action_label(first.action),
                    title = first.title.as_str(),
                    error = err.as_str()
                );
                // An uninstall has nothing to carry on; the rest wait to be
                // tried again.
                if first.action == Action::Uninstall {
                    let job = self.queue.remove(0);
                    self.finished.push((job, Err(err)));
                } else {
                    let job = &mut self.queue[0];
                    job.paused = true;
                    self.errors.insert(job.id(), err);
                }
                vec![Effect::Save, Effect::Say(say)]
            }
        }
    }

    // Updates.

    /// The games, of these, whose updates are to be asked about: the ones
    /// Zendo installed. They show as checking until [`Self::checked`].
    pub fn to_check<'a>(&mut self, games: impl Iterator<Item = &'a Game>) -> Vec<Game> {
        let games: Vec<Game> = games.filter(|game| game.store_managed()).cloned().collect();
        for game in &games {
            self.updates.insert(game.id.clone(), UpdateState::Checking);
        }
        games
    }

    /// The ids of the games Zendo installed whose updates were not asked
    /// about yet.
    #[must_use]
    pub fn unasked<'a>(&self, games: impl Iterator<Item = &'a Game>) -> Vec<String> {
        games
            .filter(|game| game.store_managed() && !self.updates.contains_key(&game.id))
            .map(|game| game.id.clone())
            .collect()
    }

    /// What the stores said of each game's updates. With `auto_update` on,
    /// the updates found are queued by themselves, a game already in the
    /// queue keeping its job; `game` finds a game by its id.
    pub fn checked(
        &mut self,
        results: Vec<(String, Result<Update, String>)>,
        auto_update: bool,
        game: impl Fn(&str) -> Option<Game>,
    ) -> Vec<Effect> {
        let found: Vec<String> = results
            .iter()
            .filter(|(_, result)| matches!(result, Ok(Update::Available { .. })))
            .map(|(id, _)| id.clone())
            .collect();
        for (id, result) in results {
            let state = match result {
                Ok(Update::UpToDate) => UpdateState::UpToDate,
                Ok(Update::Available { version }) => UpdateState::Available(version),
                Err(err) => UpdateState::Failed(err),
            };
            self.updates.insert(id, state);
        }
        self.update_by_themselves(&found, auto_update, game)
    }

    /// The ids of the games with an update waiting, by what was found.
    #[must_use]
    pub fn available(&self) -> Vec<String> {
        self.updates
            .iter()
            .filter(|(_, state)| matches!(state, UpdateState::Available(_)))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Queues the updates just found, unless Settings says games update
    /// only when asked; a game already in the queue keeps its job.
    pub fn update_by_themselves(
        &mut self,
        found: &[String],
        auto_update: bool,
        game: impl Fn(&str) -> Option<Game>,
    ) -> Vec<Effect> {
        if !auto_update {
            return Vec::new();
        }
        let mut jobs: Vec<Job> = found
            .iter()
            .filter(|id| self.job_for(id).is_none())
            .filter_map(|id| game(id))
            .filter_map(|game| Job::for_game(&game, Action::Update))
            .collect();
        if jobs.is_empty() {
            return Vec::new();
        }
        jobs.sort_by(|a, b| a.title.cmp(&b.title));
        let mut effects: Vec<Effect> = jobs.into_iter().map(|job| self.add(job)).collect();
        effects.push(Effect::Save);
        effects
    }
}

/// Asks the stores whether these games have updates. It blocks, so the
/// window runs it on a thread.
#[must_use]
pub fn ask_updates(games: &[Game]) -> Vec<(String, Result<Update, String>)> {
    games
        .iter()
        .map(|game| {
            let result = info::check_update(game).map_err(|err| format!("{err:#}"));
            (game.id.clone(), result)
        })
        .collect()
}

/// What a job's thread tells the window.
#[derive(Debug, Clone)]
pub enum Report {
    Event(Event),
    Ended(Result<Ended, String>),
}

/// Runs a job on a thread of its own (the engine blocks) and reports each
/// step, then how it ended. `send` returns `false` once nothing wants the
/// reports any more.
pub fn run(job: Job, stop: Stop, send: impl Fn(Report) -> bool + Send + 'static) {
    thread::spawn(move || {
        // Nothing wants the reports any more, so nothing wants the job: it
        // is interrupted rather than left to download for a window that
        // closed.
        let mut wanted = true;
        let result = downloads::run(&job, &stop, &mut |event| {
            if wanted && !send(Report::Event(event)) {
                wanted = false;
                stop.stop();
            }
        })
        .map_err(|err| format!("{err:#}"));
        if wanted {
            send(Report::Ended(result));
        }
    });
}

/// Clears a cancelled install's files away. It blocks.
///
/// # Errors
///
/// When the files cannot be removed.
pub fn clear_away(job: &Job) -> Result<(), String> {
    downloads::discard(job).map_err(|err| format!("{err:#}"))
}

/// Percentage, size, speed and time left, as far as the engine said.
#[must_use]
pub fn progress_line(job: &Job, progress: &Progress) -> String {
    let mut parts = Vec::new();
    if let Some(percent) = progress.percent {
        parts.push(format!("{percent:.1}%"));
    }
    if let Some(mib) = progress.downloaded_mib {
        parts.push(t!("downloads-downloaded", size = mebibytes(mib)));
    }
    if job.action == Action::Move
        && let Some((done, total)) = progress.written
    {
        #[expect(clippy::cast_precision_loss, reason = "sizes shown to two decimals")]
        let mib = |bytes: u64| mebibytes(bytes as f32 / 1_048_576.0);
        parts.push(t!("downloads-copied", done = mib(done), total = mib(total)));
    }
    if let Some(speed) = progress.speed_mib {
        parts.push(format!("{}/s", mebibytes(speed)));
    }
    if let Some(eta) = &progress.eta {
        parts.push(t!("downloads-left", time = eta.as_str()));
    }
    if parts.is_empty() {
        t!(
            "downloads-starting",
            engine = job.store().map_or("", Store::engine_name)
        )
    } else {
        parts.join(" · ")
    }
}

#[must_use]
pub fn mebibytes(mib: f32) -> String {
    if mib >= 1024.0 {
        format!("{:.2} GiB", mib / 1024.0)
    } else {
        format!("{mib:.1} MiB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::owned::Shop;

    fn job(app: &str) -> Job {
        Job {
            shop: Shop::Epic,
            app: app.to_owned(),
            title: app.to_owned(),
            action: Action::Install,
            platform: "Windows".to_owned(),
            path: "/home/me/Games".into(),
            folder: None,
            paused: false,
            dlc: Vec::new(),
            to: None,
        }
    }

    fn apps(queue: &Queue) -> Vec<&str> {
        queue.queue.iter().map(|job| job.app.as_str()).collect()
    }

    /// A queue with a job running, as [`Queue::next`] leaves it.
    fn running(jobs: Vec<Job>) -> Queue {
        Queue {
            queue: jobs,
            running: Some(Running {
                stop: Stop::default(),
                progress: Progress::default(),
                step: None,
                then: None,
            }),
            ..Queue::default()
        }
    }

    #[test]
    fn queues_each_game_once() {
        let mut queue = Queue::default();
        let _ = queue.enqueue(job("a"));
        let _ = queue.enqueue(job("b"));
        let _ = queue.enqueue(job("a"));
        assert_eq!(apps(&queue), ["a", "b"]);
        assert_eq!(
            queue.job_for("epic:b").map(|(_, running)| running),
            Some(false)
        );
        let _ = queue.cancel("epic:b");
        assert_eq!(apps(&queue), ["a"]);
        assert_eq!(queue.finished.len(), 1);
    }

    #[test]
    fn says_where_a_job_stands_in_the_line() {
        let mut queue = Queue::default();
        let say = |effects: Vec<Effect>| match effects.into_iter().next() {
            Some(Effect::Say(line)) => line,
            other => panic!("expected a line, got {other:?}"),
        };
        assert_eq!(say(queue.enqueue(job("a"))), "Installing a");
        assert_eq!(say(queue.enqueue(job("b"))), "Install b: 1 ahead of it");
        assert_eq!(say(queue.enqueue(job("a"))), "a is in the queue already");
    }

    #[test]
    fn what_was_left_goes_first() {
        let mut queue = Queue::default();
        let _ = queue.enqueue(job("new"));
        let _ = queue.loaded(vec![job("left")]);
        assert_eq!(apps(&queue), ["left", "new"]);
    }

    #[test]
    fn closing_pauses_the_queue_for_next_time() {
        let mut queue = running(vec![job("a"), job("b")]);
        queue.closing = true;
        let effects = queue.ended(Ok(Ended::Stopped));
        assert!(matches!(effects.last(), Some(Effect::Exit)));
        assert_eq!(apps(&queue), ["a", "b"]);
        assert!(queue.queue.iter().all(|job| job.paused));
        assert_eq!(queue.finished.len(), 0);
    }

    #[test]
    fn pause_keeps_the_job_and_cancel_drops_it() {
        let mut queue = running(vec![job("a"), job("b")]);
        let _ = queue.pause("epic:a");
        assert!(queue.running.as_ref().unwrap().stop.asked());
        let _ = queue.ended(Ok(Ended::Stopped));
        assert_eq!(apps(&queue), ["a", "b"]);
        assert!(queue.queue[0].paused);
        assert_eq!(queue.summary().as_deref(), Some("1 download paused"));

        let mut queue = running(vec![job("a"), job("b")]);
        let _ = queue.cancel("epic:a");
        assert_eq!(apps(&queue), ["a", "b"], "it leaves once the engine stops");
        let effects = queue.ended(Ok(Ended::Stopped));
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Discard(_)))
        );
        assert_eq!(apps(&queue), ["b"]);
        assert_eq!(queue.finished[0].1, Err("Cancelled".to_owned()));
    }

    #[test]
    fn a_failed_job_waits_to_be_tried_again() {
        let mut queue = running(vec![job("a")]);
        let _ = queue.ended(Err("no space".into()));
        assert_eq!(apps(&queue), ["a"]);
        assert!(queue.queue[0].paused);
        assert_eq!(
            queue.errors.get("epic:a").map(String::as_str),
            Some("no space")
        );
        let _ = queue.resume("epic:a");
        assert!(!queue.queue[0].paused);
        assert_eq!(queue.errors.len(), 0);
    }

    #[test]
    fn a_job_that_ends_well_asks_for_a_rescan_and_the_next_goes() {
        let mut queue = running(vec![job("a"), job("b")]);
        let effects = queue.ended(Ok(Ended::Done));
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Rescan))
        );
        assert_eq!(apps(&queue), ["b"]);
        assert_eq!(queue.finished.len(), 1);
        assert!(matches!(queue.next(|_| false), Some(Effect::Start(..))));
        assert!(queue.is_running());
        assert!(queue.next(|_| false).is_none(), "one at a time");
    }

    #[test]
    fn a_job_that_changes_a_running_games_files_waits() {
        let mut queue = Queue::default();
        let mut update = job("a");
        update.action = Action::Update;
        let _ = queue.enqueue(update);
        let _ = queue.enqueue(job("b"));
        let held = |job: &Job| changes_files(job.action);
        assert!(matches!(queue.next(held), Some(Effect::Start(started, _)) if started.app == "b"));
    }

    #[test]
    fn closing_asks_only_while_a_job_runs() {
        let mut queue = Queue::default();
        assert!(matches!(queue.close_requested().as_slice(), [Effect::Exit]));
        let mut queue = running(vec![job("a")]);
        assert!(matches!(
            queue.close_requested().as_slice(),
            [Effect::ShowDownloads]
        ));
        assert!(queue.confirm_close);
        queue.stay_open();
        assert!(!queue.confirm_close);
        let _ = queue.close_anyway();
        assert!(queue.closing);
        assert!(queue.running.as_ref().unwrap().stop.asked());
        assert!(
            queue.next(|_| false).is_none(),
            "nothing starts while closing"
        );
    }

    #[test]
    fn writes_sizes_for_people() {
        assert_eq!(mebibytes(12.34), "12.3 MiB");
        assert_eq!(mebibytes(2048.0), "2.00 GiB");
    }

    #[test]
    fn updates_found_are_queued_by_themselves_when_set() {
        let mut sugar = Game::new(
            "epic:Sugar".into(),
            "Sugar".into(),
            zendo_core::Source::Epic,
            zendo_core::Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        sugar.details.install_path = Some("/home/me/Games/Sugar".into());
        let find = |id: &str| (id == "epic:Sugar").then(|| sugar.clone());
        let found = || {
            vec![(
                "epic:Sugar".to_owned(),
                Ok(Update::Available {
                    version: Some("2".into()),
                }),
            )]
        };
        let mut queue = Queue::default();
        let _ = queue.checked(found(), false, find);
        assert_eq!(queue.queue, []);
        assert_eq!(queue.available(), ["epic:Sugar"]);
        let _ = queue.checked(found(), true, find);
        let _ = queue.checked(found(), true, find);
        let actions: Vec<Action> = queue.queue.iter().map(|job| job.action).collect();
        assert_eq!(actions, [Action::Update]);
    }

    #[test]
    fn only_games_zendo_installed_are_asked_about() {
        let mut theirs = Game::new(
            "epic:Sugar".into(),
            "Sugar".into(),
            zendo_core::Source::Epic,
            zendo_core::Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        theirs.details.install_path = Some("/home/me/Games/Sugar".into());
        let steam = Game::new(
            "steam:1".into(),
            "Steam".into(),
            zendo_core::Source::Steam,
            zendo_core::Launch::Uri {
                uri: "steam://rungameid/1".into(),
            },
        );
        let games = [theirs, steam];
        let mut queue = Queue::default();
        assert_eq!(queue.unasked(games.iter()), ["epic:Sugar"]);
        let asked = queue.to_check(games.iter());
        assert_eq!(asked.len(), 1);
        assert_eq!(
            queue.updates.get("epic:Sugar"),
            Some(&UpdateState::Checking)
        );
        assert_eq!(queue.unasked(games.iter()), Vec::<String>::new());
    }

    #[test]
    fn a_game_pass_install_is_not_asked_of_a_store_for_updates() {
        let pass = Game::new(
            "xbox:9N98Z825TNFW".into(),
            "Sugar Rush".into(),
            zendo_core::Source::Xbox,
            zendo_core::Launch::Command {
                program: "launch-game".into(),
                args: vec!["ed9ca0cf65c9665d".into()],
                dir: None,
            },
        );
        let games = [pass];
        let mut queue = Queue::default();
        assert_eq!(
            queue.to_check(games.iter()).len(),
            0,
            "no store engine knows its updates"
        );
        assert_eq!(queue.unasked(games.iter()).len(), 0);
    }
}
