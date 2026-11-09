//! The games Zendo started and still runs: Play turns into Running, with a
//! Stop that ends the game, and a second press that kills it when it does
//! not end. A game handed to Steam or Heroic shows Starting until its
//! processes turn up, found by its install folder.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Task};
use zendo_core::cloud_saves;
use zendo_core::launch::{self, Started, Watch};
use zendo_core::{Game, Launch};

use super::style::{self, BOLD};
use super::{Message, Zendo};
use crate::i18n::t;

/// How often Zendo looks whether a game still runs.
const POLL: Duration = Duration::from_secs(2);

/// How long Steam or Heroic may take to start a game (an update first, a
/// launcher's window) before Zendo stops waiting for it.
const WAIT: Duration = Duration::from_mins(5);

/// How many looks in a row must find none of a game's processes before it
/// counts as closed: a game's own launcher may hand over to the game with a
/// moment between.
const GONE: u32 = 3;

#[derive(Debug, Default)]
pub struct Running {
    games: HashMap<String, Session>,
}

#[derive(Debug)]
struct Session {
    /// How it is followed; `None` while it starts.
    watch: Option<Watch>,
    /// Its processes were seen: at once for a game Zendo starts itself,
    /// later for one its launcher starts.
    seen: bool,
    started: Instant,
    /// Stop was pressed: the next press kills it.
    stopping: bool,
    /// An anti-cheat runtime it needs could not be downloaded.
    warning: Option<String>,
}

impl Session {
    fn starting() -> Self {
        Self {
            watch: None,
            seen: false,
            started: Instant::now(),
            stopping: false,
            warning: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RunMessage {
    /// An anti-cheat runtime the game needs is downloading, before it starts.
    Fetching(String, &'static str),
    /// It could not be; the game starts without it.
    FetchFailed(String, String),
    Started(String, Result<Started, String>),
    /// The launcher started the game: its processes turned up.
    Seen(String),
    /// They did not, in time.
    Missed(String),
    Ended(String),
    /// Its saves are syncing, after it ended.
    SavesSyncing(String),
    /// What follows the game (its saves' sync, its script) is done: how it
    /// went.
    AfterDone(String, Result<(), String>),
    Stop(String),
}

impl Running {
    /// Whether the game is seen running (not while it starts).
    pub fn is_running(&self, id: &str) -> bool {
        self.games.get(id).is_some_and(|session| session.seen)
    }
}

/// The program a link hands the game to.
fn launcher(game: Option<&Game>) -> String {
    match game.map(|game| &game.launch) {
        Some(Launch::Uri { uri }) if uri.starts_with("steam:") => "Steam".to_owned(),
        Some(Launch::Uri { uri }) if uri.starts_with("heroic:") => "Heroic".to_owned(),
        _ => t!("run-its-launcher"),
    }
}

impl Zendo {
    /// Starts the game on a thread (its launch may look things up), then
    /// follows it until it ends.
    pub(super) fn start_game(&mut self, game: Game) -> Task<Message> {
        if self.running.games.contains_key(&game.id) {
            return Task::none();
        }
        self.status = Some(t!("run-starting-game", title = game.title.as_str()));
        // Tests start nothing: as if the game went to its launcher.
        if !self.persist {
            let started = Started {
                watch: Watch::Nothing,
                log: None,
                warning: None,
            };
            return self.update_running(RunMessage::Started(game.id, Ok(started)));
        }
        self.running
            .games
            .insert(game.id.clone(), Session::starting());
        Task::run(follow(game), Message::Run)
    }

    pub(super) fn update_running(&mut self, message: RunMessage) -> Task<Message> {
        match message {
            RunMessage::Fetching(id, runtime) => {
                self.status = Some(t!(
                    "run-fetching-runtime",
                    runtime = runtime,
                    title = self.title_of(&id)
                ));
                Task::none()
            }
            RunMessage::FetchFailed(id, err) => {
                let warning = t!(
                    "run-runtime-failed",
                    title = self.title_of(&id),
                    error = err.as_str()
                );
                if let Some(session) = self.running.games.get_mut(&id) {
                    session.warning = Some(warning);
                }
                Task::none()
            }
            RunMessage::Started(id, Ok(started)) => {
                // Its saves were fetched as it started.
                self.cloud.reload();
                self.started(&id, started)
            }
            RunMessage::Started(id, Err(err)) => {
                self.cloud.reload();
                self.running.games.remove(&id);
                self.status = Some(err);
                Task::none()
            }
            RunMessage::Seen(id) => {
                if let Some(session) = self.running.games.get_mut(&id) {
                    session.seen = true;
                    session.started = Instant::now();
                    self.status = Some(t!("run-running", title = self.title_of(&id)));
                }
                Task::none()
            }
            RunMessage::Missed(id) => {
                if self.running.games.remove(&id).is_some() {
                    let title = self.title_of(&id);
                    let launcher = launcher(self.game(&id));
                    self.status = Some(t!("run-missed", title = title, launcher = launcher));
                }
                Task::none()
            }
            RunMessage::Ended(id) => {
                let Some(session) = self.running.games.remove(&id) else {
                    return Task::none();
                };
                let minutes = session.started.elapsed().as_secs() / 60;
                // Steam and Heroic count the time of the games they run.
                let own = matches!(session.watch, Some(Watch::Group(_)));
                if own && minutes > 0 {
                    self.save(|state| state.add_minutes(&id, minutes));
                    if let Some(game) = self.game_mut(&id) {
                        game.played.minutes += minutes;
                    }
                }
                let title = self.title_of(&id);
                self.status = Some(t!("run-closed", title = title, minutes = minutes));
                Task::none()
            }
            RunMessage::SavesSyncing(id) => {
                self.cloud.syncing.insert(id);
                Task::none()
            }
            RunMessage::AfterDone(id, result) => {
                self.cloud.syncing.remove(&id);
                self.cloud.reload();
                if let Err(err) = result {
                    self.status = Some(err);
                }
                Task::none()
            }
            RunMessage::Stop(id) => {
                let Some(session) = self.running.games.get_mut(&id) else {
                    return Task::none();
                };
                let Some(watch) = session.watch.as_ref().filter(|_| session.seen) else {
                    return Task::none();
                };
                if session.stopping {
                    launch::kill(watch);
                    return Task::none();
                }
                session.stopping = true;
                let asked = launch::stop(watch);
                let title = self.title_of(&id);
                self.status = Some(if asked {
                    t!("run-asked-close", title = title)
                } else {
                    t!("run-asked-end", title = title)
                });
                Task::none()
            }
        }
    }

    /// A game's launcher is up: watch it, count the launch.
    fn started(&mut self, id: &str, started: Started) -> Task<Message> {
        let title = self.title_of(id);
        let launcher = launcher(self.game(id));
        let start_warning = started.warning;
        match started.watch {
            Watch::Nothing => {
                self.running.games.remove(id);
                self.status = Some(t!("run-handed", title = title, launcher = launcher));
            }
            watch => {
                if let Some(session) = self.running.games.get_mut(id) {
                    session.seen = matches!(watch, Watch::Group(_));
                    session.started = Instant::now();
                    let warning = session.warning.take().or(start_warning);
                    self.status = Some(if let Some(warning) = warning {
                        warning
                    } else if session.seen {
                        t!("run-running", title = title)
                    } else {
                        t!("run-waiting", title = title, launcher = launcher)
                    });
                    session.watch = Some(watch);
                }
            }
        }
        // Zendo's state now has the launch; show it in the order
        // without scanning again.
        if let Some(played) = self.game_mut(id).map(|game| &mut game.played) {
            played.launches += 1;
            played.last = Some(super::now());
        }
        self.select(Some(id))
    }

    fn title_of(&self, id: &str) -> String {
        self.game(id)
            .map_or_else(|| id.to_owned(), |game| game.title.clone())
    }

    /// Play, or Running with Stop while the game runs.
    pub(super) fn play_buttons(&self, game: &Game) -> Element<'_, Message> {
        let Some(session) = self.running.games.get(&game.id) else {
            return button(text(t!("run-play")).size(16).font(BOLD))
                .padding([10, 32])
                .style(style::primary)
                .on_press(Message::Play)
                .into();
        };
        let state = if session.seen {
            t!("run-state-running")
        } else {
            t!("run-state-starting")
        };
        let stop = if session.stopping {
            t!("run-force-stop")
        } else {
            t!("run-stop")
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
                        .seen
                        .then(|| Message::Run(RunMessage::Stop(game.id.clone())))
                ),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }
}
/// Starts the game, says how, and then says when its process group is
/// gone.
fn follow(game: Game) -> impl Stream<Item = RunMessage> {
    iced::stream::channel(4, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            let id = game.id.clone();
            let fetched = launch::fetch_runtimes(&game, &mut |runtime| {
                let _ = sender.unbounded_send(RunMessage::Fetching(id.clone(), runtime.label()));
            });
            if let Err(err) = fetched {
                let _ =
                    sender.unbounded_send(RunMessage::FetchFailed(id.clone(), format!("{err:#}")));
            }
            let result = launch::start(&game).map_err(|err| format!("{err:#}"));
            let watch = result.as_ref().ok().map(|started| started.watch.clone());
            let _ = sender.unbounded_send(RunMessage::Started(id.clone(), result));
            let send = |message| sender.unbounded_send(message).is_ok();
            let own = matches!(watch, Some(Watch::Group(_)));
            match watch {
                Some(watch @ Watch::Group(_)) => {
                    while launch::running(&watch) {
                        thread::sleep(POLL);
                    }
                }
                Some(watch @ Watch::Folder(_)) => {
                    let asked = Instant::now();
                    while !launch::running(&watch) {
                        if asked.elapsed() > WAIT {
                            send(RunMessage::Missed(id));
                            return;
                        }
                        thread::sleep(POLL);
                    }
                    if !send(RunMessage::Seen(id.clone())) {
                        return;
                    }
                    let mut gone = 0;
                    while gone < GONE {
                        thread::sleep(POLL);
                        gone = if launch::running(&watch) { 0 } else { gone + 1 };
                    }
                }
                _ => return,
            }
            if !send(RunMessage::Ended(id.clone())) {
                return;
            }
            // Only for a game Zendo started (its group): the others'
            // launchers have their own.
            if own {
                if cloud_saves::wanted_for(&game) {
                    send(RunMessage::SavesSyncing(id.clone()));
                }
                let result = launch::after(&game).map_err(|err| format!("{err:#}"));
                send(RunMessage::AfterDone(id, result));
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
    use zendo_core::Source;

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
        zendo
            .running
            .games
            .insert("gog:1".into(), Session::starting());
        let started = Started {
            watch: Watch::Group(launch::Group::new(4_000_000, None)),
            log: None,
            warning: None,
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
        zendo
            .running
            .games
            .insert("gog:1".into(), Session::starting());
        let _ = zendo.update(Message::Run(RunMessage::Started(
            "gog:1".into(),
            Err("umu-launcher is needed".into()),
        )));
        assert!(!zendo.running.is_running("gog:1"));
        assert_eq!(zendo.status.as_deref(), Some("umu-launcher is needed"));
    }

    #[test]
    fn a_steam_game_starts_once_its_processes_turn_up() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let game = Game::new(
            "steam:620".into(),
            "Portal 2".into(),
            Source::Steam,
            Launch::Uri {
                uri: "steam://rungameid/620".into(),
            },
        );
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        zendo
            .running
            .games
            .insert("steam:620".into(), Session::starting());
        let started = Started {
            watch: Watch::Folder("/games/Portal 2".into()),
            log: None,
            warning: None,
        };
        let _ = zendo.update(Message::Run(RunMessage::Started(
            "steam:620".into(),
            Ok(started),
        )));
        assert!(!zendo.running.is_running("steam:620"));
        assert_eq!(
            zendo.status.as_deref(),
            Some("Waiting for Steam to start Portal 2…")
        );
        // Stop does nothing before there is something to stop.
        let _ = zendo.update(Message::Run(RunMessage::Stop("steam:620".into())));
        assert!(!zendo.running.games["steam:620"].stopping);
        let _ = zendo.update(Message::Run(RunMessage::Seen("steam:620".into())));
        assert!(zendo.running.is_running("steam:620"));
        let _ = zendo.update(Message::Run(RunMessage::Ended("steam:620".into())));
        assert!(!zendo.running.is_running("steam:620"));
        assert_eq!(zendo.status.as_deref(), Some("Portal 2 closed"));
        assert_eq!(
            zendo.game("steam:620").map(|game| game.played.minutes),
            Some(0)
        );
    }
}
