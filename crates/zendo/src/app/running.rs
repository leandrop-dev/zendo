//! The games Zendo started and still runs: Play turns into Running, with a
//! Stop that ends the game, and a second press that kills it when it does
//! not end. A game handed to Steam or Heroic is not followed here.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::launch::{self, Started};
use zendo_core::{Game, downloads};

use super::style::{self, BOLD, DIM};
use super::{Message, Zendo};

/// How often Zendo looks whether a game still runs.
const POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub struct Running {
    games: HashMap<String, Session>,
}

#[derive(Debug)]
struct Session {
    /// `None` while it starts.
    group: Option<u32>,
    started: Instant,
    /// Stop was pressed: the next press kills it.
    stopping: bool,
}

#[derive(Debug, Clone)]
pub enum RunMessage {
    Started(String, Result<Started, String>),
    Ended(String),
    Stop(String),
}

impl Running {
    pub fn is_running(&self, id: &str) -> bool {
        self.games.contains_key(id)
    }
}

impl Zendo {
    /// Starts the game on a thread (its launch may look things up), then
    /// follows it until it ends.
    pub(super) fn start_game(&mut self, game: Game) -> Task<Message> {
        if self.running.is_running(&game.id) {
            return Task::none();
        }
        self.status = Some(format!("Starting {}…", game.title));
        // Tests start nothing: as if the game went to its launcher.
        if !self.persist {
            let started = Started {
                group: 0,
                log: None,
                tracked: false,
            };
            return self.update_running(RunMessage::Started(game.id, Ok(started)));
        }
        self.running.games.insert(
            game.id.clone(),
            Session {
                group: None,
                started: Instant::now(),
                stopping: false,
            },
        );
        Task::run(follow(game), Message::Run)
    }

    pub(super) fn update_running(&mut self, message: RunMessage) -> Task<Message> {
        match message {
            RunMessage::Started(id, Ok(started)) => {
                let title = self.title_of(&id);
                if !started.tracked {
                    self.running.games.remove(&id);
                    self.status = Some(format!("Handed {title} to its launcher"));
                } else if let Some(session) = self.running.games.get_mut(&id) {
                    session.group = Some(started.group);
                    session.started = Instant::now();
                    self.status = Some(format!("{title} is running"));
                }
                // Zendo's state now has the launch; show it in the order
                // without scanning again.
                if let Some(played) = self.game_mut(&id).map(|game| &mut game.played) {
                    played.launches += 1;
                    played.last = Some(super::now());
                }
                self.select(Some(&id))
            }
            RunMessage::Started(id, Err(err)) => {
                self.running.games.remove(&id);
                self.status = Some(err);
                Task::none()
            }
            RunMessage::Ended(id) => {
                let Some(session) = self.running.games.remove(&id) else {
                    return Task::none();
                };
                let minutes = session.started.elapsed().as_secs() / 60;
                if minutes > 0 {
                    self.save(|state| state.add_minutes(&id, minutes));
                    if let Some(game) = self.game_mut(&id) {
                        game.played.minutes += minutes;
                    }
                }
                let title = self.title_of(&id);
                self.status = Some(match minutes {
                    0 => format!("{title} closed"),
                    1 => format!("{title} closed after a minute"),
                    n => format!("{title} closed after {n} minutes"),
                });
                Task::none()
            }
            RunMessage::Stop(id) => {
                let Some(session) = self.running.games.get_mut(&id) else {
                    return Task::none();
                };
                let Some(group) = session.group else {
                    return Task::none();
                };
                if session.stopping {
                    launch::kill(group);
                } else {
                    session.stopping = true;
                    launch::stop(group);
                }
                Task::none()
            }
        }
    }

    fn title_of(&self, id: &str) -> String {
        self.game(id)
            .map_or_else(|| id.to_owned(), |game| game.title.clone())
    }

    /// Play, or Running with Stop while the game runs.
    pub(super) fn play_buttons(&self, game: &Game) -> Element<'_, Message> {
        let Some(session) = self.running.games.get(&game.id) else {
            return button(text("Play").size(16).font(BOLD))
                .padding([10, 32])
                .style(style::primary)
                .on_press(Message::Play)
                .into();
        };
        let state = if session.group.is_some() {
            "Running"
        } else {
            "Starting…"
        };
        let stop = if session.stopping {
            "Force stop"
        } else {
            "Stop"
        };
        row![
            button(text(state).size(16).font(BOLD))
                .padding([10, 32])
                .style(style::primary),
            button(text(stop).size(14).font(BOLD))
                .padding([10, 18])
                .style(style::danger)
                .on_press_maybe(
                    session
                        .group
                        .map(|_| Message::Run(RunMessage::Stop(game.id.clone())))
                ),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }
}

impl Zendo {
    /// Where a game Zendo installed keeps its Wine prefix and its last
    /// launch's log.
    pub(super) fn launch_card<'a>(&self, game: &'a Game) -> Element<'a, Message> {
        let mut rows = column![].spacing(12);
        if launch::needs_proton(game)
            && let Some(dir) = self.config.install_dir()
        {
            let prefix = dir
                .join("Prefixes")
                .join(downloads::folder_name(&game.title));
            rows = rows.push(path_row("Wine prefix", &prefix));
        }
        if let Some(log) = launch::log_path(&game.id) {
            rows = rows.push(path_row("Last launch's log", &log));
        }
        container(
            column![
                text("Launch").size(18).font(BOLD),
                text("Windows games run through umu-launcher with Proton, each in a prefix of its own.")
                    .size(13)
                    .color(DIM),
                rows,
            ]
            .spacing(10),
        )
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
    }
}

fn path_row<'a>(label: &'static str, path: &std::path::Path) -> Element<'a, Message> {
    let exists = path.exists();
    row![
        text(label).size(14).color(DIM).width(170),
        text(path.display().to_string())
            .size(13)
            .font(Font::MONOSPACE)
            .width(Length::Fill),
        button(text("Open").size(13))
            .padding([4, 10])
            .style(style::secondary)
            .on_press_maybe(exists.then(|| Message::Open(path.to_string_lossy().into_owned()))),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

/// Starts the game, says how, and then says when its process group is
/// gone.
fn follow(game: Game) -> impl Stream<Item = RunMessage> {
    iced::stream::channel(4, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            let id = game.id.clone();
            let result = launch::start(&game).map_err(|err| format!("{err:#}"));
            let group = result
                .as_ref()
                .ok()
                .filter(|started| started.tracked)
                .map(|started| started.group);
            let _ = sender.unbounded_send(RunMessage::Started(id.clone(), result));
            if let Some(group) = group {
                while launch::running(group) {
                    thread::sleep(POLL);
                }
                let _ = sender.unbounded_send(RunMessage::Ended(id));
            }
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
    use zendo_core::{Launch, Source};

    #[test]
    fn a_game_runs_until_its_group_is_gone() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        zendo.running.games.insert(
            "gog:1".into(),
            Session {
                group: None,
                started: Instant::now(),
                stopping: false,
            },
        );
        let started = Started {
            group: 4_000_000,
            log: None,
            tracked: true,
        };
        let _ = zendo.update(Message::Run(RunMessage::Started(
            "gog:1".into(),
            Ok(started),
        )));
        assert!(zendo.running.is_running("gog:1"));
        assert_eq!(zendo.status.as_deref(), Some("Sugar is running"));
        assert_eq!(
            zendo.game("gog:1").map(|game| game.played.launches),
            Some(1)
        );
        let _ = zendo.update(Message::Run(RunMessage::Ended("gog:1".into())));
        assert!(!zendo.running.is_running("gog:1"));
        assert_eq!(zendo.status.as_deref(), Some("Sugar closed"));
    }

    #[test]
    fn a_failed_start_leaves_play_as_it_was() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.running.games.insert(
            "gog:1".into(),
            Session {
                group: None,
                started: Instant::now(),
                stopping: false,
            },
        );
        let _ = zendo.update(Message::Run(RunMessage::Started(
            "gog:1".into(),
            Err("umu-launcher is needed".into()),
        )));
        assert!(!zendo.running.is_running("gog:1"));
        assert_eq!(zendo.status.as_deref(), Some("umu-launcher is needed"));
    }
}
