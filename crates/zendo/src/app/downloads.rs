//! The Downloads page and the queue behind it: one job at a time, run by
//! the store's engine as a child of the window, with its progress read from
//! the engine's log. There is no background process: closing the window
//! while something downloads asks first, then pauses everything queued, to
//! be resumed from this page next time.
//!
//! Pause keeps what was downloaded; Cancel stops and, for an install,
//! deletes it. A job that fails stays in the queue, paused with the
//! reason, so Resume tries again.
//!
//! The page also keeps whether each game Zendo installed has an update,
//! asked of its store once a session and after each job.

use std::collections::HashMap;
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::downloads::{self, Action, Ended, Event, Job, Progress, Stop};
use zendo_core::info::{self, Update};

use super::style::{self, ACCENT, BAD, BOLD, DIM, GOOD};
use super::{Message, Page, Zendo, blocking, centered};

#[derive(Debug, Default)]
pub struct DownloadsState {
    /// What is left to do; the first runs while `running` is set.
    pub queue: Vec<Job>,
    running: Option<Running>,
    /// What ended since the window opened, the latest last.
    finished: Vec<(Job, Result<(), String>)>,
    /// Why a queued job failed, until it is resumed.
    errors: HashMap<String, String>,
    /// Closing was asked for while a job ran.
    pub confirm_close: bool,
    /// The user chose to close; the window goes once the engine stops.
    closing: bool,
    /// The game whose Uninstall was pressed once.
    pub confirm_uninstall: Option<String>,
    /// Whether each game Zendo installed has an update, by id.
    pub updates: HashMap<String, UpdateState>,
}

#[derive(Debug)]
struct Running {
    stop: Stop,
    progress: Progress,
    step: Option<String>,
    /// What the user asked for, once the engine has stopped.
    then: Option<Then>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
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

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    Loaded(Result<Vec<Job>, String>),
    Queue(Job),
    Event(Event),
    Ended(Result<Ended, String>),
    /// Holds a game's job in the queue, stopping it when it runs.
    Pause(String),
    Resume(String),
    /// Takes a game's job out of the queue, clearing away a half-done
    /// install.
    Cancel(String),
    /// A cancelled install was cleared away, by title.
    Discarded(String, Result<(), String>),
    CloseRequested,
    CloseAnyway,
    StayOpen,
    ClearFinished,
    /// Uninstall pressed once: the button asks to be pressed again.
    AskUninstall(String),
    CheckUpdate(String),
    UpdatesChecked(Vec<(String, Result<Update, String>)>),
}

/// The queue left from last time, read on a thread.
pub fn load() -> Task<Message> {
    Task::perform(
        blocking(|| downloads::load_queue().map_err(|err| format!("{err:#}"))),
        |result| Message::Download(DownloadMessage::Loaded(result)),
    )
}

impl DownloadsState {
    /// The job for this game in the queue, and whether it is the one
    /// running.
    pub fn job_for(&self, id: &str) -> Option<(&Job, bool)> {
        self.queue
            .iter()
            .enumerate()
            .find(|(_, job)| job.id() == id)
            .map(|(index, job)| (job, index == 0 && self.running.is_some()))
    }

    /// The running job's progress, as a short line for the footer; or how
    /// many wait paused.
    pub fn summary(&self) -> Option<String> {
        let Some(running) = &self.running else {
            return match self.paused() {
                0 => None,
                1 => Some("1 download paused".to_owned()),
                n => Some(format!("{n} downloads paused")),
            };
        };
        let job = self.queue.first()?;
        Some(match (running.then, running.progress.percent) {
            (Some(Then::Pause), _) => format!("Pausing {}…", job.title),
            (Some(Then::Cancel), _) => format!("Cancelling {}…", job.title),
            (None, Some(percent)) => {
                format!("{} {}, {percent:.0}%", job.action.doing(), job.title)
            }
            (None, None) => format!("{} {}…", job.action.doing(), job.title),
        })
    }

    fn paused(&self) -> usize {
        self.queue.iter().filter(|job| job.paused).count()
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.queue.iter().position(|job| job.id() == id)
    }

    /// Whether the job at `index` is the one the engine runs.
    fn runs(&self, index: usize) -> bool {
        index == 0 && self.running.is_some()
    }
}

impl Zendo {
    pub(super) fn update_download(&mut self, message: DownloadMessage) -> Task<Message> {
        match message {
            DownloadMessage::Loaded(Ok(jobs)) => {
                // Anything queued meanwhile goes after what was left.
                let added = std::mem::replace(&mut self.downloads.queue, jobs);
                for job in added {
                    self.queue_job(job);
                }
                self.next_job()
            }
            DownloadMessage::Loaded(Err(err)) => {
                self.status = Some(err);
                Task::none()
            }
            DownloadMessage::Queue(job) => {
                self.downloads.confirm_uninstall = None;
                self.queue_job(job);
                self.save_queue();
                self.next_job()
            }
            DownloadMessage::Event(event) => {
                self.job_event(event);
                Task::none()
            }
            DownloadMessage::Ended(result) => self.job_ended(result),
            DownloadMessage::Pause(id) => {
                self.stop_or(&id, Then::Pause, |state, index| {
                    state.queue[index].paused = true;
                });
                self.save_queue();
                Task::none()
            }
            DownloadMessage::Resume(id) => {
                if let Some(index) = self.downloads.index_of(&id) {
                    self.downloads.queue[index].paused = false;
                    self.downloads.errors.remove(&id);
                    self.save_queue();
                }
                self.next_job()
            }
            DownloadMessage::Cancel(id) => {
                let mut removed = None;
                self.stop_or(&id, Then::Cancel, |state, index| {
                    removed = Some(state.queue.remove(index));
                });
                self.save_queue();
                match removed {
                    Some(job) => self.cancelled(job),
                    None => Task::none(),
                }
            }
            DownloadMessage::Discarded(title, result) => {
                self.status = Some(match result {
                    Ok(()) => format!("Cancelled {title}; its files are gone"),
                    Err(err) => format!("Cancelled {title}, but {err}"),
                });
                Task::none()
            }
            DownloadMessage::CloseRequested if self.downloads.running.is_some() => {
                self.downloads.confirm_close = true;
                self.navigate(Page::Downloads)
            }
            DownloadMessage::CloseRequested => iced::exit(),
            DownloadMessage::CloseAnyway => {
                self.downloads.confirm_close = false;
                match &self.downloads.running {
                    Some(running) => {
                        self.downloads.closing = true;
                        running.stop.stop();
                        self.status = Some("Pausing the download, then closing…".to_owned());
                        Task::none()
                    }
                    None => iced::exit(),
                }
            }
            DownloadMessage::StayOpen => {
                self.downloads.confirm_close = false;
                Task::none()
            }
            DownloadMessage::ClearFinished => {
                self.downloads.finished.clear();
                Task::none()
            }
            DownloadMessage::AskUninstall(id) => {
                self.downloads.confirm_uninstall = Some(id);
                Task::none()
            }
            DownloadMessage::CheckUpdate(id) => self.check_updates(&[id]),
            DownloadMessage::UpdatesChecked(results) => {
                for (id, result) in results {
                    let state = match result {
                        Ok(Update::UpToDate) => UpdateState::UpToDate,
                        Ok(Update::Available { version }) => UpdateState::Available(version),
                        Err(err) => UpdateState::Failed(err),
                    };
                    self.downloads.updates.insert(id, state);
                }
                Task::none()
            }
        }
    }

    /// Stops the game's job when it runs, to do `then` after; otherwise
    /// does `queued` to it where it waits.
    fn stop_or(&mut self, id: &str, then: Then, queued: impl FnOnce(&mut DownloadsState, usize)) {
        let Some(index) = self.downloads.index_of(id) else {
            return;
        };
        if self.downloads.runs(index) {
            if let Some(running) = &mut self.downloads.running {
                running.then = Some(then);
                running.stop.stop();
            }
        } else {
            queued(&mut self.downloads, index);
        }
    }

    fn job_event(&mut self, event: Event) {
        match event {
            // Kept with the job at once, so a crash still finds it.
            Event::Folder(folder) => {
                if let Some(job) = self.downloads.queue.first_mut() {
                    job.folder = Some(folder);
                }
                self.save_queue();
            }
            Event::Progress(progress) => {
                if let Some(running) = &mut self.downloads.running {
                    running.progress = progress;
                    running.step = None;
                }
            }
            Event::Step(step) => {
                if let Some(running) = &mut self.downloads.running {
                    running.step = Some(step);
                }
            }
        }
    }

    /// A cancelled job leaves the queue; an install's files are cleared away
    /// on a thread.
    fn cancelled(&mut self, job: Job) -> Task<Message> {
        self.downloads.errors.remove(&job.id());
        self.status = Some(format!("Cancelled: {} {}", job.action.doing(), job.title));
        self.downloads
            .finished
            .push((job.clone(), Err("Cancelled".to_owned())));
        if job.action != Action::Install || !self.persist {
            return Task::none();
        }
        let title = job.title.clone();
        Task::perform(
            blocking(move || downloads::discard(&job).map_err(|err| format!("{err:#}"))),
            move |result| Message::Download(DownloadMessage::Discarded(title.clone(), result)),
        )
    }

    fn queue_job(&mut self, job: Job) {
        let id = job.id();
        if self.downloads.queue.iter().any(|queued| queued.id() == id) {
            self.status = Some(format!("{} is in the queue already", job.title));
            return;
        }
        let ahead = self
            .downloads
            .queue
            .iter()
            .filter(|queued| !queued.paused)
            .count();
        self.status = Some(match ahead {
            0 => format!("{} {}", job.action.doing(), job.title),
            n => format!("{} {}: {} ahead of it", job.action.label(), job.title, n),
        });
        self.downloads.queue.push(job);
    }

    fn save_queue(&mut self) {
        if !self.persist {
            return;
        }
        if let Err(err) = downloads::save_queue(&self.downloads.queue) {
            self.status = Some(format!("{err:#}"));
        }
    }

    /// Starts the first job not paused, unless one runs; it moves to the
    /// front of the queue.
    fn next_job(&mut self) -> Task<Message> {
        if !self.persist || self.downloads.running.is_some() || self.downloads.closing {
            return Task::none();
        }
        let Some(index) = self.downloads.queue.iter().position(|job| !job.paused) else {
            return Task::none();
        };
        let job = self.downloads.queue.remove(index);
        self.downloads.queue.insert(0, job.clone());
        let stop = Stop::default();
        self.downloads.running = Some(Running {
            stop: stop.clone(),
            progress: Progress::default(),
            step: None,
            then: None,
        });
        Task::run(run_job(job, stop), |message| message)
    }

    fn job_ended(&mut self, result: Result<Ended, String>) -> Task<Message> {
        let then = self
            .downloads
            .running
            .take()
            .and_then(|running| running.then);
        // Closing pauses the whole queue, to be resumed by hand next time.
        if self.downloads.closing {
            for job in &mut self.downloads.queue {
                job.paused = true;
            }
            self.save_queue();
            return iced::exit();
        }
        if self.downloads.queue.is_empty() {
            return Task::none();
        }
        match (result, then) {
            (Ok(Ended::Stopped), Some(Then::Cancel)) => {
                let job = self.downloads.queue.remove(0);
                self.save_queue();
                let cancelled = self.cancelled(job);
                Task::batch([cancelled, self.next_job()])
            }
            (Ok(Ended::Stopped), _) => {
                let job = &mut self.downloads.queue[0];
                job.paused = true;
                self.status = Some(format!("Paused: {} {}", job.action.doing(), job.title));
                self.save_queue();
                self.next_job()
            }
            (Ok(Ended::Done), _) => {
                let job = self.downloads.queue.remove(0);
                self.save_queue();
                self.status = Some(format!("{} {}: done", job.action.label(), job.title));
                // Asked again once the library has it back.
                self.downloads.updates.remove(&job.id());
                self.downloads.finished.push((job, Ok(())));
                let rescan = self.update(Message::Refresh);
                Task::batch([rescan, self.next_job()])
            }
            (Err(err), _) => {
                self.status = Some(format!(
                    "{} {} failed: {err}",
                    self.downloads.queue[0].action.label(),
                    self.downloads.queue[0].title
                ));
                // An uninstall has nothing to carry on; the rest wait to be
                // tried again.
                if self.downloads.queue[0].action == Action::Uninstall {
                    let job = self.downloads.queue.remove(0);
                    self.downloads.finished.push((job, Err(err)));
                } else {
                    let job = &mut self.downloads.queue[0];
                    job.paused = true;
                    self.downloads.errors.insert(job.id(), err);
                }
                self.save_queue();
                self.next_job()
            }
        }
    }

    /// Asks the stores, on a thread, whether these games have updates.
    pub(super) fn check_updates(&mut self, ids: &[String]) -> Task<Message> {
        let games: Vec<Game> = ids
            .iter()
            .filter_map(|id| self.game(id))
            .filter(|game| game.installed_by_zendo())
            .cloned()
            .collect();
        if games.is_empty() || !self.persist {
            return Task::none();
        }
        for game in &games {
            self.downloads
                .updates
                .insert(game.id.clone(), UpdateState::Checking);
        }
        Task::perform(
            blocking(move || {
                games
                    .iter()
                    .map(|game| {
                        let result = info::check_update(game).map_err(|err| format!("{err:#}"));
                        (game.id.clone(), result)
                    })
                    .collect()
            }),
            |results| Message::Download(DownloadMessage::UpdatesChecked(results)),
        )
    }

    /// The games Zendo installed whose updates were not asked about yet.
    pub(super) fn check_new_updates(&mut self) -> Task<Message> {
        let ids: Vec<String> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .filter(|game| {
                game.installed_by_zendo() && !self.downloads.updates.contains_key(&game.id)
            })
            .map(|game| game.id.clone())
            .collect();
        self.check_updates(&ids)
    }

    /// Update, Repair and Uninstall, for a game Zendo installed, with
    /// whether an update waits.
    pub(super) fn store_actions<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let queued = self.downloads.job_for(&game.id);
        let update = self.downloads.updates.get(&game.id);
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        if let Some((job, running)) = queued {
            let state = match (running, job.paused) {
                (true, _) => "now",
                (false, true) => "paused",
                (false, false) => "queued",
            };
            actions = actions
                .push(text(format!("{}: {state}", job.action.doing())).size(14))
                .push(
                    button(text("See downloads").size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press(Message::Navigate(Page::Downloads)),
                );
        } else {
            let queue = |action| {
                Job::for_game(game, action)
                    .map(|job| Message::Download(DownloadMessage::Queue(job)))
            };
            let confirming = self.downloads.confirm_uninstall.as_deref() == Some(game.id.as_str());
            let uninstall = if confirming {
                button(text("Press again to delete its files").size(14).font(BOLD))
                    .style(style::danger)
                    .on_press_maybe(queue(Action::Uninstall))
            } else {
                button(text("Uninstall").size(14))
                    .style(style::secondary)
                    .on_press(Message::Download(DownloadMessage::AskUninstall(
                        game.id.clone(),
                    )))
            };
            let waiting = matches!(update, Some(UpdateState::Available(_)));
            actions = actions
                .push(
                    button(text("Update").size(14).font(if waiting {
                        BOLD
                    } else {
                        iced::Font::default()
                    }))
                    .padding([8, 14])
                    .style(if waiting {
                        style::primary
                    } else {
                        style::secondary
                    })
                    .on_press_maybe(queue(Action::Update)),
                )
                .push(
                    button(text("Repair").size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(queue(Action::Repair)),
                )
                .push(uninstall.padding([8, 14]))
                .push(space::horizontal())
                .push(
                    button(text("Check for updates").size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press_maybe((update != Some(&UpdateState::Checking)).then(|| {
                            Message::Download(DownloadMessage::CheckUpdate(game.id.clone()))
                        })),
                );
        }
        let (line, color) = match update {
            None => (String::new(), DIM),
            Some(UpdateState::Checking) => ("Checking for updates…".to_owned(), DIM),
            Some(UpdateState::UpToDate) => ("Up to date".to_owned(), GOOD),
            Some(UpdateState::Available(Some(version))) => {
                (format!("Update available: {version}"), ACCENT)
            }
            Some(UpdateState::Available(None)) => ("Update available".to_owned(), ACCENT),
            Some(UpdateState::Failed(err)) => (format!("Could not check for updates: {err}"), BAD),
        };
        let mut card = column![
            text("Installed by Zendo").size(18).font(BOLD),
            text(format!(
                "Through {}, which Zendo starts it with too.",
                game.launcher()
            ))
            .size(13)
            .color(DIM),
        ]
        .spacing(10);
        if !line.is_empty() {
            card = card.push(text(line).size(14).color(color));
        }
        container(card.push(actions))
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }

    pub(super) fn downloads_page(&self) -> Element<'_, Message> {
        let state = &self.downloads;
        let paused = state.paused();
        let subtitle = match (state.queue.len(), paused) {
            (0, _) => "Nothing to download".to_owned(),
            (n, 0) => format!("{n} in the queue"),
            (n, p) => format!("{n} in the queue, {p} paused"),
        };
        let header = column![
            text("Downloads").size(28).font(BOLD),
            text(subtitle).size(13).color(DIM),
        ]
        .spacing(2);

        let mut body = column![header].spacing(16).padding(24);
        if state.confirm_close {
            body = body.push(close_banner());
        }
        if let Some(mods) = self.mod_downloads_card() {
            body = body.push(mods);
        }
        if state.queue.is_empty() && state.finished.is_empty() {
            if !self.nexus.downloads.is_empty() {
                return body.into();
            }
            return column![
                body,
                centered(
                    column![
                        text("Install games from the Store and they show up here").size(18),
                        button(text("Go to the Store").size(14))
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press(Message::Navigate(Page::Store)),
                    ]
                    .spacing(12)
                    .align_x(Alignment::Center),
                ),
            ]
            .into();
        }

        if let (Some(running), Some(job)) = (&state.running, state.queue.first()) {
            body = body.push(running_card(job, running));
        }
        let start = usize::from(state.running.is_some());
        let rest = || state.queue.iter().skip(start);
        if rest().any(|job| !job.paused) {
            body = body.push(text("UP NEXT").size(11).font(BOLD).color(DIM));
            for job in rest().filter(|job| !job.paused) {
                body = body.push(queued_row(job, None));
            }
        }
        if rest().any(|job| job.paused) {
            body = body.push(text("PAUSED").size(11).font(BOLD).color(DIM));
            for job in rest().filter(|job| job.paused) {
                body = body.push(queued_row(job, state.errors.get(&job.id())));
            }
        }
        if !state.finished.is_empty() {
            body = body.push(
                row![
                    text("FINISHED").size(11).font(BOLD).color(DIM),
                    space::horizontal(),
                    button(text("Clear").size(13))
                        .padding([4, 10])
                        .style(style::secondary)
                        .on_press(Message::Download(DownloadMessage::ClearFinished)),
                ]
                .align_y(Alignment::Center),
            );
            for (job, outcome) in state.finished.iter().rev() {
                body = body.push(finished_row(job, outcome));
            }
        }
        scrollable(body).height(Length::Fill).into()
    }
}

fn close_banner<'a>() -> Element<'a, Message> {
    container(
        column![
            text("A download is running").size(15).font(BOLD),
            text("Closing pauses it, with what it has so far; resume it from here next time")
                .size(13)
                .color(DIM),
            row![
                button(text("Close anyway").size(14).font(BOLD))
                    .padding([8, 14])
                    .style(style::primary)
                    .on_press(Message::Download(DownloadMessage::CloseAnyway)),
                button(text("Keep downloading").size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press(Message::Download(DownloadMessage::StayOpen)),
            ]
            .spacing(10),
        ]
        .spacing(6),
    )
    .padding(16)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn running_card<'a>(job: &'a Job, running: &'a Running) -> Element<'a, Message> {
    let progress = &running.progress;
    let line = running
        .step
        .clone()
        .unwrap_or_else(|| progress_line(job, progress));
    container(
        column![
            row![
                column![
                    text(&job.title).size(18).font(BOLD),
                    text(format!("{} · {}", job.action.doing(), job.store().label()))
                        .size(13)
                        .color(ACCENT),
                ]
                .spacing(2),
                space::horizontal(),
                button(
                    text(if running.then == Some(Then::Pause) {
                        "Pausing…"
                    } else {
                        "Pause"
                    })
                    .size(14)
                )
                .padding([8, 14])
                .style(style::secondary)
                .on_press_maybe(
                    running
                        .then
                        .is_none()
                        .then(|| Message::Download(DownloadMessage::Pause(job.id())))
                ),
                button(
                    text(if running.then == Some(Then::Cancel) {
                        "Cancelling…"
                    } else {
                        "Cancel"
                    })
                    .size(14)
                )
                .padding([8, 14])
                .style(style::secondary)
                .on_press_maybe(
                    (running.then != Some(Then::Cancel))
                        .then(|| Message::Download(DownloadMessage::Cancel(job.id())))
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            progress_bar(0.0..=100.0, progress.percent.unwrap_or(0.0))
                .girth(8)
                .style(style::progress),
            text(line).size(13).color(DIM),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

/// Percentage, size, speed and time left, as far as the engine said.
fn progress_line(job: &Job, progress: &Progress) -> String {
    let mut parts = Vec::new();
    if let Some(percent) = progress.percent {
        parts.push(format!("{percent:.1}%"));
    }
    if let Some(mib) = progress.downloaded_mib {
        parts.push(format!("{} downloaded", mebibytes(mib)));
    }
    if let Some(speed) = progress.speed_mib {
        parts.push(format!("{}/s", mebibytes(speed)));
    }
    if let Some(eta) = &progress.eta {
        parts.push(format!("{eta} left"));
    }
    if parts.is_empty() {
        format!("Starting {}…", job.store().engine_name())
    } else {
        parts.join(" · ")
    }
}

/// A job waiting its turn, or paused, with why it failed if it did.
fn queued_row<'a>(job: &'a Job, error: Option<&'a String>) -> Element<'a, Message> {
    let id = job.id();
    let mut about = column![
        text(&job.title).size(15),
        text(format!("{} · {}", job.action.label(), job.store().label()))
            .size(12)
            .color(DIM),
    ]
    .spacing(2);
    if let Some(error) = error {
        about = about.push(text(format!("Failed: {error}")).size(12).color(BAD));
    }
    let (first, message) = if job.paused {
        (
            if error.is_some() {
                "Try again"
            } else {
                "Resume"
            },
            DownloadMessage::Resume(id.clone()),
        )
    } else {
        ("Pause", DownloadMessage::Pause(id.clone()))
    };
    container(
        row![
            about.width(Length::Fill),
            button(text(first).size(13))
                .padding([6, 12])
                .style(if job.paused {
                    style::primary
                } else {
                    style::secondary
                })
                .on_press(Message::Download(message)),
            button(text("Cancel").size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press(Message::Download(DownloadMessage::Cancel(id))),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn finished_row<'a>(job: &'a Job, outcome: &'a Result<(), String>) -> Element<'a, Message> {
    let (verdict, color) = match outcome {
        Ok(()) => ("Done".to_owned(), GOOD),
        Err(err) => (err.clone(), BAD),
    };
    let open = (outcome.is_ok() && job.action != Action::Uninstall).then(|| {
        button(text("Open").size(13))
            .padding([6, 12])
            .style(style::secondary)
            .on_press(Message::Navigate(Page::Game(job.id())))
    });
    container(
        row![
            column![
                text(&job.title).size(15),
                text(format!("{} · {}", job.action.label(), job.store().label()))
                    .size(12)
                    .color(DIM),
            ]
            .spacing(2)
            .width(Length::FillPortion(2)),
            text(verdict)
                .size(13)
                .color(color)
                .width(Length::FillPortion(3)),
        ]
        .push(open)
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn mebibytes(mib: f32) -> String {
    if mib >= 1024.0 {
        format!("{:.2} GiB", mib / 1024.0)
    } else {
        format!("{mib:.1} MiB")
    }
}

/// Runs a job on a thread (the engine blocks) and reports each step, then
/// how it ended.
fn run_job(job: Job, stop: Stop) -> impl Stream<Item = Message> {
    iced::stream::channel(32, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            let progress = sender.clone();
            let result = downloads::run(&job, &stop, &mut |event| {
                let _ = progress.unbounded_send(Message::Download(DownloadMessage::Event(event)));
            })
            .map_err(|err| format!("{err:#}"));
            let _ = sender.unbounded_send(Message::Download(DownloadMessage::Ended(result)));
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::owned::StoreKey;

    fn job(app: &str) -> Job {
        Job {
            store: StoreKey::Epic,
            app: app.to_owned(),
            title: app.to_owned(),
            action: Action::Install,
            platform: "Windows".to_owned(),
            path: "/home/me/Games".into(),
            folder: None,
            paused: false,
        }
    }

    fn apps(zendo: &Zendo) -> Vec<&str> {
        zendo
            .downloads
            .queue
            .iter()
            .map(|job| job.app.as_str())
            .collect()
    }

    /// A Zendo with a job running, as `next_job` leaves it outside tests.
    fn running(jobs: Vec<Job>) -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.downloads.queue = jobs;
        zendo.downloads.running = Some(Running {
            stop: Stop::default(),
            progress: Progress::default(),
            step: None,
            then: None,
        });
        zendo
    }

    #[test]
    fn queues_each_game_once() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("b"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        assert_eq!(apps(&zendo), ["a", "b"]);
        assert_eq!(
            zendo
                .downloads
                .job_for("epic:b")
                .map(|(_, running)| running),
            Some(false)
        );
        let _ = zendo.update(Message::Download(DownloadMessage::Cancel("epic:b".into())));
        assert_eq!(apps(&zendo), ["a"]);
        assert_eq!(zendo.downloads.finished.len(), 1);
    }

    #[test]
    fn what_was_left_goes_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("new"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Loaded(Ok(vec![job(
            "left",
        )]))));
        assert_eq!(apps(&zendo), ["left", "new"]);
    }

    #[test]
    fn closing_pauses_the_queue_for_next_time() {
        let mut zendo = running(vec![job("a"), job("b")]);
        zendo.downloads.closing = true;
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Ok(
            Ended::Stopped,
        ))));
        assert_eq!(apps(&zendo), ["a", "b"]);
        assert!(zendo.downloads.queue.iter().all(|job| job.paused));
        assert_eq!(zendo.downloads.finished.len(), 0);
    }

    #[test]
    fn pause_keeps_the_job_and_cancel_drops_it() {
        let mut zendo = running(vec![job("a"), job("b")]);
        let _ = zendo.update(Message::Download(DownloadMessage::Pause("epic:a".into())));
        assert!(zendo.downloads.running.as_ref().unwrap().stop.asked());
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Ok(
            Ended::Stopped,
        ))));
        assert_eq!(apps(&zendo), ["a", "b"]);
        assert!(zendo.downloads.queue[0].paused);
        assert_eq!(
            zendo.downloads.summary().as_deref(),
            Some("1 download paused")
        );

        let mut zendo = running(vec![job("a"), job("b")]);
        let _ = zendo.update(Message::Download(DownloadMessage::Cancel("epic:a".into())));
        assert_eq!(apps(&zendo), ["a", "b"], "it leaves once the engine stops");
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Ok(
            Ended::Stopped,
        ))));
        assert_eq!(apps(&zendo), ["b"]);
        assert_eq!(zendo.downloads.finished[0].1, Err("Cancelled".to_owned()));
    }

    #[test]
    fn a_failed_job_waits_to_be_tried_again() {
        let mut zendo = running(vec![job("a")]);
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Err(
            "no space".into()
        ))));
        assert_eq!(apps(&zendo), ["a"]);
        assert!(zendo.downloads.queue[0].paused);
        assert_eq!(
            zendo.downloads.errors.get("epic:a").map(String::as_str),
            Some("no space")
        );
        let _ = zendo.update(Message::Download(DownloadMessage::Resume("epic:a".into())));
        assert!(!zendo.downloads.queue[0].paused);
        assert_eq!(zendo.downloads.errors.len(), 0);
    }

    #[test]
    fn writes_sizes_for_people() {
        assert_eq!(mebibytes(12.34), "12.3 MiB");
        assert_eq!(mebibytes(2048.0), "2.00 GiB");
    }
}
