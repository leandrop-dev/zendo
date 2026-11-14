//! The Downloads page and the queue behind it. The queue's state and rules
//! are `zendo_shell::queue`'s, shared with the Slint window: one job at a
//! time, run by the store's engine as a child of the window, with its
//! progress read from the engine's log. Closing the window while something
//! downloads asks first, then pauses everything queued, to be resumed from
//! this page next time.
//!
//! What is here is Iced's own: turning the queue's effects into tasks, the
//! folder dialog, and the views.

use std::path::PathBuf;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, text};
use iced::{Alignment, Color, Element, Length, Task};
use zendo_core::Game;
use zendo_core::downloads::{Action, Job, Stop};
use zendo_core::info::Update;
use zendo_shell::queue::{
    self, Effect, Report, Running, Then, Tone, action_doing, action_label, progress_line,
};
pub use zendo_shell::queue::{Queue, UpdateState, action_doing_alone};

use super::game::stat_tile;
use super::style::{
    self, BOLD, BUTTON, SPACE_L, SPACE_M, SPACE_S, SPACE_XL, SPACE_XS, button_text, colors,
};
use super::{Message, Page, Zendo, blocking, centered};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    Loaded(Result<Vec<Job>, String>),
    Queue(Job),
    Report(Report),
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
    /// Move pressed on a game's page, or Import on a store game's: the
    /// folder dialog opens.
    ChooseFolder(FolderFor, String),
    /// The folder chosen, if one was.
    FolderChosen(FolderFor, String, Result<Option<PathBuf>, String>),
}

/// What a folder is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderFor {
    /// The folder a game Zendo installed goes into.
    Move,
    /// The folder a store game already is in.
    Import,
}

/// The queue left from last time, read on a thread.
pub fn load() -> Task<Message> {
    Task::perform(
        blocking(|| zendo_core::downloads::load_queue().map_err(|err| format!("{err:#}"))),
        |result| Message::Download(DownloadMessage::Loaded(result)),
    )
}

/// The colour of a line's tone.
fn tone_color(tone: Tone) -> Color {
    match tone {
        Tone::Dim => colors().dim,
        Tone::Good => colors().good,
        Tone::Accent => colors().accent,
        Tone::Bad => colors().bad,
    }
}

/// What the last look for an update found, and its colour.
pub(super) fn update_line(update: Option<&UpdateState>) -> (String, Color) {
    let (line, tone) = queue::update_line(update);
    (line, tone_color(tone))
}

impl Zendo {
    pub(super) fn update_download(&mut self, message: DownloadMessage) -> Task<Message> {
        let effects = match message {
            DownloadMessage::Loaded(Ok(jobs)) => self.downloads.loaded(jobs),
            DownloadMessage::Loaded(Err(err)) => {
                self.status = Some(err);
                Vec::new()
            }
            DownloadMessage::Queue(job) => self.downloads.enqueue(job),
            DownloadMessage::Report(Report::Event(event)) => self.downloads.event(event),
            DownloadMessage::Report(Report::Ended(result)) => self.downloads.ended(result),
            DownloadMessage::Pause(id) => self.downloads.pause(&id),
            DownloadMessage::Resume(id) => self.downloads.resume(&id),
            DownloadMessage::Cancel(id) => self.downloads.cancel(&id),
            DownloadMessage::Discarded(title, result) => {
                vec![Queue::discarded(&title, &result)]
            }
            DownloadMessage::CloseRequested => self.downloads.close_requested(),
            DownloadMessage::CloseAnyway => self.downloads.close_anyway(),
            DownloadMessage::StayOpen => {
                self.downloads.stay_open();
                Vec::new()
            }
            DownloadMessage::ClearFinished => {
                self.downloads.clear_finished();
                Vec::new()
            }
            DownloadMessage::AskUninstall(id) => {
                self.downloads.confirm_uninstall = Some(id);
                Vec::new()
            }
            DownloadMessage::CheckUpdate(id) => return self.check_updates(&[id]),
            DownloadMessage::UpdatesChecked(results) => {
                let (auto, library) = (self.config.auto_update, &self.library);
                self.downloads.checked(results, auto, |id| {
                    library
                        .as_ref()
                        .and_then(|library| library.find(id))
                        .cloned()
                })
            }
            DownloadMessage::ChooseFolder(purpose, id) => return self.choose_folder(purpose, id),
            DownloadMessage::FolderChosen(purpose, id, result) => {
                return self.folder_chosen(purpose, &id, result);
            }
        };
        self.apply_queue(effects)
    }

    /// Does what the queue asked, then starts what is due. Tests start and
    /// write nothing.
    fn apply_queue(&mut self, effects: Vec<Effect>) -> Task<Message> {
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                Effect::Say(line) => self.status = Some(line),
                Effect::Save => self.save_queue(),
                Effect::Discard(job) if self.persist => {
                    let title = job.title.clone();
                    tasks.push(Task::perform(
                        blocking(move || queue::clear_away(&job)),
                        move |result| {
                            Message::Download(DownloadMessage::Discarded(title.clone(), result))
                        },
                    ));
                }
                // Started by `next_job` below, when the window is not a test.
                Effect::Start(..) | Effect::Discard(_) => {}
                Effect::Rescan => tasks.push(self.update(Message::Refresh)),
                Effect::Exit => tasks.push(iced::exit()),
                Effect::ShowDownloads => tasks.push(self.navigate(Page::Downloads)),
            }
        }
        tasks.push(self.next_job());
        Task::batch(tasks)
    }

    /// Opens the folder dialog to move a game (unless it is running) or to
    /// say where a store game already is.
    fn choose_folder(&mut self, purpose: FolderFor, id: String) -> Task<Message> {
        if !self.persist {
            return Task::none();
        }
        let title = match purpose {
            FolderFor::Move => {
                let Some(game) = self.game(&id) else {
                    return Task::none();
                };
                if self.running.is_open(&id) {
                    self.status = Some(t!("downloads-move-running", title = game.title.as_str()));
                    return Task::none();
                }
                t!("downloads-move-to", title = game.title.as_str())
            }
            FolderFor::Import => {
                let Some(game) = self.store.owned(&id) else {
                    return Task::none();
                };
                t!("downloads-import-where", title = game.title.as_str())
            }
        };
        Task::perform(
            blocking(move || {
                crate::file_dialog::pick(crate::file_dialog::Kind::Folder, &title)
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| {
                Message::Download(DownloadMessage::FolderChosen(purpose, id.clone(), result))
            },
        )
    }

    /// Queues the move or the import with the folder chosen.
    fn folder_chosen(
        &mut self,
        purpose: FolderFor,
        id: &str,
        chosen: Result<Option<PathBuf>, String>,
    ) -> Task<Message> {
        match chosen {
            Ok(Some(folder)) => {
                let job = match purpose {
                    FolderFor::Move => self.game(id).and_then(|game| Job::for_move(game, folder)),
                    FolderFor::Import => self
                        .store
                        .owned(id)
                        .and_then(|game| Job::import(game, folder)),
                };
                match job {
                    Some(job) => self.update(Message::Download(DownloadMessage::Queue(job))),
                    None => Task::none(),
                }
            }
            Ok(None) => Task::none(),
            Err(err) => {
                self.status = Some(err);
                Task::none()
            }
        }
    }

    /// Whether the game's files are being moved now, so it must not start.
    pub(super) fn moving(&self, id: &str) -> bool {
        self.downloads.moving(id)
    }

    fn save_queue(&mut self) {
        if !self.persist {
            return;
        }
        if let Err(err) = zendo_core::downloads::save_queue(&self.downloads.queue) {
            self.status = Some(format!("{err:#}"));
        }
    }

    /// Whether the job waits for its game to close: it would change the
    /// files of a game running.
    pub(super) fn held_by_game(&self, job: &Job) -> bool {
        queue::changes_files(job.action) && self.running.is_open(&job.id())
    }

    /// Starts the first job not paused, unless one runs; it moves to the
    /// front of the queue. A job that changes a game's files waits while
    /// the game runs, and goes when it closes.
    pub(super) fn next_job(&mut self) -> Task<Message> {
        if !self.persist {
            return Task::none();
        }
        let running = &self.running;
        let started = self
            .downloads
            .next(|job| queue::changes_files(job.action) && running.is_open(&job.id()));
        match started {
            Some(Effect::Start(job, stop)) => Task::run(run_job(job, stop), |message| message),
            _ => Task::none(),
        }
    }

    /// Queues the updates already found, as Settings' switch turns on.
    pub(super) fn update_by_themselves(&mut self) -> Task<Message> {
        let found = self.downloads.available();
        let (auto, library) = (self.config.auto_update, &self.library);
        let effects = self.downloads.update_by_themselves(&found, auto, |id| {
            library
                .as_ref()
                .and_then(|library| library.find(id))
                .cloned()
        });
        self.apply_queue(effects)
    }

    /// Asks the stores, on a thread, whether these games have updates.
    pub(super) fn check_updates(&mut self, ids: &[String]) -> Task<Message> {
        let games: Vec<Game> = {
            let library = self.library.as_ref();
            let wanted = ids
                .iter()
                .filter_map(|id| library.and_then(|library| library.find(id)));
            self.downloads.to_check(wanted)
        };
        if games.is_empty() || !self.persist {
            return Task::none();
        }
        Task::perform(blocking(move || queue::ask_updates(&games)), |results| {
            Message::Download(DownloadMessage::UpdatesChecked(results))
        })
    }

    /// The games Zendo installed whose updates were not asked about yet.
    pub(super) fn check_new_updates(&mut self) -> Task<Message> {
        let ids = self
            .downloads
            .unasked(self.library.iter().flat_map(|library| &library.games));
        self.check_updates(&ids)
    }

    pub(super) fn downloads_page(&self) -> Element<'_, Message> {
        let state = &self.downloads;
        let paused = state.paused();
        // The counts, as tiles; nothing to count is no tiles.
        let mut body = column![].spacing(SPACE_L).padding(SPACE_XL);
        if !state.queue.is_empty() || !state.finished.is_empty() {
            body = body.push(
                row![
                    stat_tile(
                        t!("downloads-up-next"),
                        state.queue.len().saturating_sub(paused).to_string()
                    ),
                    stat_tile(t!("downloads-paused"), paused.to_string()),
                    stat_tile(t!("downloads-finished"), state.finished.len().to_string()),
                ]
                .spacing(SPACE_M),
            );
        }
        if state.confirm_close {
            body = body.push(close_banner());
        }
        if let Some(steam) = self.steam_downloads_card() {
            body = body.push(steam);
        }
        if let Some(mods) = self.mod_downloads_card() {
            body = body.push(mods);
        }
        if state.queue.is_empty() && state.finished.is_empty() {
            if !self.nexus.downloads.is_empty() || self.steam_installs.active() {
                // A long Steam or mod card list still scrolls.
                return scrollable(body).height(Length::Fill).into();
            }
            return scrollable(column![
                body,
                centered(
                    column![
                        text(t!("downloads-empty")).size(18),
                        button(button_text(t!("downloads-go-store")))
                            .padding(BUTTON)
                            .style(style::secondary)
                            .on_press(Message::Navigate(Page::Store)),
                    ]
                    .spacing(SPACE_M)
                    .align_x(Alignment::Center),
                ),
            ])
            .height(Length::Fill)
            .into();
        }

        if let (Some(running), Some(job)) = (&state.running, state.queue.first()) {
            body = body.push(running_card(job, running));
        }
        let start = usize::from(state.running.is_some());
        let rest = || state.queue.iter().skip(start);
        if rest().any(|job| !job.paused) {
            body = body.push(style::label(&t!("downloads-up-next")));
            for job in rest().filter(|job| !job.paused) {
                let waits = self.held_by_game(job);
                body = body.push(queued_row(job, None, waits));
            }
        }
        if rest().any(|job| job.paused) {
            body = body.push(style::label(&t!("downloads-paused")));
            for job in rest().filter(|job| job.paused) {
                body = body.push(queued_row(job, state.errors.get(&job.id()), false));
            }
        }
        if !state.finished.is_empty() {
            body = body.push(
                row![
                    style::label(&t!("downloads-finished")),
                    space::horizontal(),
                    button(button_text(t!("downloads-clear")))
                        .padding(style::BUTTON_SMALL)
                        .style(style::secondary)
                        .on_press(Message::Download(DownloadMessage::ClearFinished)),
                ]
                .align_y(Alignment::Center),
            );
            for (job, outcome) in state.finished.iter().rev() {
                body = body.push(finished_row(job, outcome, self.game(&job.id()).is_some()));
            }
        }
        scrollable(body).height(Length::Fill).into()
    }
}

fn close_banner<'a>() -> Element<'a, Message> {
    container(
        column![
            text(t!("downloads-running-title")).size(15).font(BOLD),
            text(t!("downloads-running-about"))
                .size(13)
                .color(colors().dim),
            row![
                button(button_text(t!("downloads-close-anyway")).font(BOLD))
                    .padding(BUTTON)
                    .style(style::primary)
                    .on_press(Message::Download(DownloadMessage::CloseAnyway)),
                button(button_text(t!("downloads-keep")))
                    .padding(BUTTON)
                    .style(style::secondary)
                    .on_press(Message::Download(DownloadMessage::StayOpen)),
            ]
            .spacing(SPACE_M),
        ]
        .spacing(SPACE_S),
    )
    .padding(SPACE_L)
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
    let mut content = column![
        row![
            column![
                text(&job.title).size(18).font(BOLD),
                text(format!(
                    "{} · {}",
                    action_doing(job.action),
                    job.store().label()
                ))
                .size(13)
                .color(colors().accent),
            ]
            .spacing(SPACE_XS),
            space::horizontal(),
            button(button_text(if running.then == Some(Then::Pause) {
                t!("pausing")
            } else {
                t!("pause")
            }))
            .padding(BUTTON)
            .style(style::secondary)
            .on_press_maybe(
                running
                    .then
                    .is_none()
                    .then(|| Message::Download(DownloadMessage::Pause(job.id())))
            ),
            button(button_text(if running.then == Some(Then::Cancel) {
                t!("cancelling")
            } else {
                t!("cancel")
            }))
            .padding(BUTTON)
            .style(style::secondary)
            .on_press_maybe(
                (running.then != Some(Then::Cancel))
                    .then(|| Message::Download(DownloadMessage::Cancel(job.id())))
            ),
        ]
        .spacing(SPACE_M)
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_M);
    // A bar at zero says "stalled"; without a percentage the line below
    // already says the engine is starting.
    if let Some(percent) = progress.percent {
        content = content.push(
            progress_bar(0.0..=100.0, percent)
                .girth(8)
                .style(style::progress),
        );
    }
    content = content.push(text(line).size(13).color(colors().dim));
    container(content)
        .padding(SPACE_L)
        .width(Length::Fill)
        .style(style::card)
        .into()
}

/// A job waiting its turn, or paused, with why it failed if it did.
fn queued_row<'a>(job: &'a Job, error: Option<&'a String>, waits: bool) -> Element<'a, Message> {
    let id = job.id();
    let mut about = column![
        text(&job.title).size(15),
        text(format!(
            "{} · {}",
            action_label(job.action),
            job.store().label()
        ))
        .size(12)
        .color(colors().dim),
    ]
    .spacing(SPACE_XS);
    if waits {
        about = about.push(
            text(t!("downloads-waits-running"))
                .size(12)
                .color(colors().dim),
        );
    }
    if let Some(error) = error {
        about = about.push(
            text(t!("downloads-failed", error = error.as_str()))
                .size(12)
                .color(colors().bad),
        );
    }
    let (first, message) = if job.paused {
        (
            if error.is_some() {
                t!("try-again")
            } else {
                t!("resume")
            },
            DownloadMessage::Resume(id.clone()),
        )
    } else {
        (t!("pause"), DownloadMessage::Pause(id.clone()))
    };
    container(
        row![
            about.width(Length::Fill),
            button(button_text(first))
                .padding(style::BUTTON_SMALL)
                .style(if job.paused {
                    style::primary
                } else {
                    style::secondary
                })
                .on_press(Message::Download(message)),
            button(button_text(t!("cancel")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::Download(DownloadMessage::Cancel(id))),
        ]
        .spacing(SPACE_M)
        .align_y(Alignment::Center),
    )
    .padding([SPACE_S, SPACE_L])
    .width(Length::Fill)
    .style(style::card)
    .into()
}

/// `in_library` is whether the scan already knows the game: right after an
/// install it does not, and the game's page would say it has gone.
fn finished_row<'a>(
    job: &'a Job,
    outcome: &'a Result<(), String>,
    in_library: bool,
) -> Element<'a, Message> {
    let (verdict, color) = match outcome {
        Ok(()) => (t!("downloads-done"), colors().good),
        Err(err) => (err.clone(), colors().bad),
    };
    let open = (outcome.is_ok() && job.action != Action::Uninstall).then(|| {
        let page = if in_library {
            Page::Game(job.id())
        } else {
            Page::Owned(job.id())
        };
        button(button_text(t!("open")))
            .padding(style::BUTTON_SMALL)
            .style(style::secondary)
            .on_press(Message::Navigate(page))
    });
    container(
        row![
            column![
                text(&job.title).size(15),
                text(format!(
                    "{} · {}",
                    action_label(job.action),
                    job.store().label()
                ))
                .size(12)
                .color(colors().dim),
            ]
            .spacing(SPACE_XS)
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
    .padding([SPACE_S, SPACE_L])
    .width(Length::Fill)
    .style(style::card)
    .into()
}

/// Runs a job on a thread (the engine blocks) and reports each step, then
/// how it ended.
fn run_job(job: Job, stop: Stop) -> impl Stream<Item = Message> {
    iced::stream::channel(32, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        queue::run(job, stop, move |report| {
            sender
                .unbounded_send(Message::Download(DownloadMessage::Report(report)))
                .is_ok()
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
    use zendo_core::downloads::{Ended, Progress};
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
            dlc: Vec::new(),
            to: None,
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

    fn ended(result: Result<Ended, String>) -> Message {
        Message::Download(DownloadMessage::Report(Report::Ended(result)))
    }

    #[test]
    fn queues_each_game_once() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("b"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        assert_eq!(apps(&zendo), ["a", "b"]);
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
        let _ = zendo.update(ended(Ok(Ended::Stopped)));
        assert_eq!(apps(&zendo), ["a", "b"]);
        assert!(zendo.downloads.queue.iter().all(|job| job.paused));
        assert_eq!(zendo.downloads.finished.len(), 0);
    }

    #[test]
    fn pause_keeps_the_job_and_cancel_drops_it() {
        let mut zendo = running(vec![job("a"), job("b")]);
        let _ = zendo.update(Message::Download(DownloadMessage::Pause("epic:a".into())));
        assert!(zendo.downloads.running.as_ref().unwrap().stop.asked());
        let _ = zendo.update(ended(Ok(Ended::Stopped)));
        assert_eq!(apps(&zendo), ["a", "b"]);
        assert!(zendo.downloads.queue[0].paused);
        assert_eq!(
            zendo.downloads.summary().as_deref(),
            Some("1 download paused")
        );

        let mut zendo = running(vec![job("a"), job("b")]);
        let _ = zendo.update(Message::Download(DownloadMessage::Cancel("epic:a".into())));
        assert_eq!(apps(&zendo), ["a", "b"], "it leaves once the engine stops");
        let _ = zendo.update(ended(Ok(Ended::Stopped)));
        assert_eq!(apps(&zendo), ["b"]);
        assert_eq!(zendo.downloads.finished[0].1, Err("Cancelled".to_owned()));
    }

    #[test]
    fn a_failed_job_waits_to_be_tried_again() {
        let mut zendo = running(vec![job("a")]);
        let _ = zendo.update(ended(Err("no space".into())));
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
    fn updates_found_are_queued_by_themselves_when_set() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let mut game = Game::new(
            "epic:Sugar".into(),
            "Sugar".into(),
            zendo_core::Source::Epic,
            zendo_core::Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        game.details.install_path = Some("/home/me/Games/Sugar".into());
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        let found = || {
            Message::Download(DownloadMessage::UpdatesChecked(vec![(
                "epic:Sugar".into(),
                Ok(Update::Available {
                    version: Some("2".into()),
                }),
            )]))
        };
        zendo.config.auto_update = false;
        let _ = zendo.update(found());
        assert_eq!(zendo.downloads.queue, []);
        zendo.config.auto_update = true;
        let _ = zendo.update(found());
        let _ = zendo.update(found());
        let actions: Vec<Action> = zendo.downloads.queue.iter().map(|job| job.action).collect();
        assert_eq!(actions, [Action::Update]);
    }
}
