//! The Downloads page and the queue behind it: one job at a time, run by
//! the store's engine as a child of the window, with its progress read from
//! the engine's log. There is no background process: closing the window
//! while something downloads asks first, stops the engine so it keeps what
//! it has, and the queue picks up on the next start.

use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, scrollable, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::downloads::{self, Action, Ended, Event, Job, Progress, Stop};

use super::style::{self, ACCENT, BAD, BOLD, DIM, GOOD};
use super::{Message, Page, Zendo, blocking, centered};

#[derive(Debug, Default)]
pub struct DownloadsState {
    /// What is left to do; the first runs while `running` is set.
    pub queue: Vec<Job>,
    running: Option<Running>,
    /// What ended since the window opened, the latest last.
    finished: Vec<(Job, Result<(), String>)>,
    /// Closing was asked for while a job ran.
    pub confirm_close: bool,
    /// The user chose to close; the window goes once the engine stops.
    closing: bool,
    /// The game whose Uninstall was pressed once.
    pub confirm_uninstall: Option<String>,
}

#[derive(Debug)]
struct Running {
    stop: Stop,
    progress: Progress,
    step: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DownloadMessage {
    Loaded(Result<Vec<Job>, String>),
    Queue(Job),
    Event(Event),
    Ended(Result<Ended, String>),
    /// Takes a job out of the queue, stopping it when it runs.
    Remove(usize),
    CloseRequested,
    CloseAnyway,
    StayOpen,
    ClearFinished,
    /// Uninstall pressed once: the button asks to be pressed again.
    AskUninstall(String),
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

    /// The running job's progress, as a short line for the footer.
    pub fn summary(&self) -> Option<String> {
        let running = self.running.as_ref()?;
        let job = self.queue.first()?;
        Some(match running.progress.percent {
            Some(percent) => format!("{} {}, {percent:.0}%", job.action.doing(), job.title),
            None => format!("{} {}…", job.action.doing(), job.title),
        })
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
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
                if let Some(running) = &mut self.downloads.running {
                    match event {
                        Event::Progress(progress) => {
                            running.progress = progress;
                            running.step = None;
                        }
                        Event::Step(step) => running.step = Some(step),
                    }
                }
                Task::none()
            }
            DownloadMessage::Ended(result) => self.job_ended(result),
            DownloadMessage::Remove(index) => {
                if index == 0 && self.downloads.running.is_some() {
                    if let Some(running) = &self.downloads.running {
                        running.stop.stop();
                    }
                } else if index < self.downloads.queue.len() {
                    self.downloads.queue.remove(index);
                    self.save_queue();
                }
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
                        self.status = Some("Stopping the download, then closing…".to_owned());
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
        }
    }

    fn queue_job(&mut self, job: Job) {
        let id = job.id();
        if self.downloads.queue.iter().any(|queued| queued.id() == id) {
            self.status = Some(format!("{} is in the queue already", job.title));
            return;
        }
        self.status = Some(match self.downloads.queue.len() {
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

    /// Starts the first job unless one runs.
    fn next_job(&mut self) -> Task<Message> {
        if !self.persist || self.downloads.running.is_some() || self.downloads.closing {
            return Task::none();
        }
        let Some(job) = self.downloads.queue.first().cloned() else {
            return Task::none();
        };
        let stop = Stop::default();
        self.downloads.running = Some(Running {
            stop: stop.clone(),
            progress: Progress::default(),
            step: None,
        });
        Task::run(run_job(job, stop), |message| message)
    }

    fn job_ended(&mut self, result: Result<Ended, String>) -> Task<Message> {
        self.downloads.running = None;
        // Closing keeps the stopped job queued, to pick up next time.
        if self.downloads.closing {
            self.save_queue();
            return iced::exit();
        }
        if self.downloads.queue.is_empty() {
            return Task::none();
        }
        let job = self.downloads.queue.remove(0);
        self.save_queue();
        let outcome = match result {
            Ok(Ended::Done) => {
                self.status = Some(format!("{} {}: done", job.action.label(), job.title));
                Ok(())
            }
            Ok(Ended::Stopped) => {
                self.status = Some(format!("Stopped: {} {}", job.action.doing(), job.title));
                Err("Stopped".to_owned())
            }
            Err(err) => {
                self.status = Some(format!(
                    "{} {} failed: {err}",
                    job.action.label(),
                    job.title
                ));
                Err(err)
            }
        };
        let changed = outcome.is_ok();
        self.downloads.finished.push((job, outcome));
        let rescan = if changed {
            self.update(Message::Refresh)
        } else {
            Task::none()
        };
        Task::batch([rescan, self.next_job()])
    }

    /// Update, Repair and Uninstall, for a game Zendo installed.
    pub(super) fn store_actions<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let queued = self.downloads.job_for(&game.id);
        let mut actions = row![].spacing(10).align_y(Alignment::Center);
        if let Some((job, running)) = queued {
            let state = if running { "now" } else { "queued" };
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
            actions = actions
                .push(
                    button(text("Update").size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(queue(Action::Update)),
                )
                .push(
                    button(text("Repair").size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(queue(Action::Repair)),
                )
                .push(uninstall.padding([8, 14]));
        }
        container(
            column![
                text("Installed by Zendo").size(18).font(BOLD),
                text(format!(
                    "Through {}. Starting it from Zendo comes with launch options, the next step.",
                    game.launcher()
                ))
                .size(13)
                .color(DIM),
                actions,
            ]
            .spacing(10),
        )
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
    }

    pub(super) fn downloads_page(&self) -> Element<'_, Message> {
        let state = &self.downloads;
        let subtitle = match state.queue.len() {
            0 => "Nothing to download".to_owned(),
            1 => "1 in the queue".to_owned(),
            n => format!("{n} in the queue"),
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
        if state.queue.is_empty() && state.finished.is_empty() {
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

        for (index, job) in state.queue.iter().enumerate() {
            let running = (index == 0).then_some(state.running.as_ref()).flatten();
            if index == 1 || (index == 0 && running.is_none()) {
                body = body.push(text("UP NEXT").size(11).font(BOLD).color(DIM));
            }
            body = body.push(match running {
                Some(running) => running_card(job, running),
                None => queued_row(job, index),
            });
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
            text("Closing stops it; it picks up where it left off next time")
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
                button(text("Stop").size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press(Message::Download(DownloadMessage::Remove(0))),
            ]
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

fn queued_row(job: &Job, index: usize) -> Element<'_, Message> {
    container(
        row![
            column![
                text(&job.title).size(15),
                text(format!("{} · {}", job.action.label(), job.store().label()))
                    .size(12)
                    .color(DIM),
            ]
            .spacing(2),
            space::horizontal(),
            button(text("Remove").size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press(Message::Download(DownloadMessage::Remove(index))),
        ]
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
        }
    }

    #[test]
    fn queues_each_game_once() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("b"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("a"))));
        let apps: Vec<&str> = zendo
            .downloads
            .queue
            .iter()
            .map(|job| job.app.as_str())
            .collect();
        assert_eq!(apps, ["a", "b"]);
        assert_eq!(
            zendo
                .downloads
                .job_for("epic:b")
                .map(|(_, running)| running),
            Some(false)
        );
        let _ = zendo.update(Message::Download(DownloadMessage::Remove(1)));
        assert_eq!(zendo.downloads.queue.len(), 1);
    }

    #[test]
    fn what_was_left_goes_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Queue(job("new"))));
        let _ = zendo.update(Message::Download(DownloadMessage::Loaded(Ok(vec![job(
            "left",
        )]))));
        let apps: Vec<&str> = zendo
            .downloads
            .queue
            .iter()
            .map(|job| job.app.as_str())
            .collect();
        assert_eq!(apps, ["left", "new"]);
    }

    #[test]
    fn closing_keeps_the_stopped_job_for_next_time() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.downloads.queue = vec![job("a"), job("b")];
        zendo.downloads.closing = true;
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Ok(
            Ended::Stopped,
        ))));
        assert_eq!(zendo.downloads.queue.len(), 2);
        assert_eq!(zendo.downloads.finished.len(), 0);
        // Stopped by hand, it leaves the queue.
        zendo.downloads.closing = false;
        let _ = zendo.update(Message::Download(DownloadMessage::Ended(Ok(
            Ended::Stopped,
        ))));
        assert_eq!(zendo.downloads.queue.len(), 1);
        assert_eq!(zendo.downloads.finished.len(), 1);
    }

    #[test]
    fn writes_sizes_for_people() {
        assert_eq!(mebibytes(12.34), "12.3 MiB");
        assert_eq!(mebibytes(2048.0), "2.00 GiB");
    }
}
