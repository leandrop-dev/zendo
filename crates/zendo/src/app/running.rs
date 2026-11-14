//! The games Zendo started and still runs, in the Iced window: the state and
//! its transitions are `zendo_shell::running`'s, shared with the Slint window;
//! this turns what it says to do into Iced tasks, and draws Play and Stop.

use iced::futures::channel::mpsc;
use iced::futures::{Stream, StreamExt};
use iced::widget::{button, row};
use iced::{Alignment, Element, Task};
use zendo_core::Game;
use zendo_core::launch::{Started, Watch};
use zendo_core::sessions;
use zendo_shell::running::{self, Effect, Names};
pub use zendo_shell::running::{RunMessage, Running};

use super::style::{self, BOLD};
use super::{Message, Zendo};
use crate::i18n::t;

impl Zendo {
    /// Starts the game on a thread (its launch may look things up), then
    /// follows it until it ends.
    pub(super) fn start_game(&mut self, game: Game) -> Task<Message> {
        if self.running.is_open(&game.id) {
            return Task::none();
        }
        if self.moving(&game.id) {
            self.status = Some(t!("downloads-moving-wait", title = game.title.as_str()));
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
        self.running.begin(&game.id);
        Task::run(follow(game), Message::Run)
    }

    pub(super) fn update_running(&mut self, message: RunMessage) -> Task<Message> {
        let names = |id: &str| Names::of(id, self.library.as_ref().and_then(|l| l.find(id)));
        let effects = self.running.apply(message, &names);
        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                Effect::Say(line) => self.status = Some(line),
                Effect::CloudReload => self.cloud.reload(),
                Effect::CloudSyncing(id, true) => {
                    self.cloud.syncing.insert(id);
                }
                Effect::CloudSyncing(id, false) => {
                    self.cloud.syncing.remove(&id);
                }
                Effect::Launched(id) => {
                    if let Some(played) = self.game_mut(&id).map(|game| &mut game.played) {
                        played.launches += 1;
                        played.last = Some(super::now());
                    }
                    tasks.push(self.select(Some(&id)));
                }
                Effect::Played(id, minutes) => {
                    self.save(|state| state.add_minutes(&id, minutes));
                    if let Some(game) = self.game_mut(&id) {
                        game.played.minutes += minutes;
                    }
                }
                Effect::Session { id, since, minutes } => self.record_session(&id, since, minutes),
                Effect::Presence => self.update_presence(),
                Effect::HideOnLaunch => tasks.push(self.hide_on_launch()),
                Effect::NextJob => tasks.push(self.next_job()),
                Effect::RefreshGogTotal(id) => tasks.push(self.refresh_gog_total(&id)),
                Effect::Changed(_) => {}
            }
        }
        Task::batch(tasks)
    }

    /// Keeps a session in the window's list and, outside tests, in the
    /// sessions file; a failure is said in the status line.
    fn record_session(&mut self, id: &str, start: u64, minutes: u64) {
        let session = sessions::Session {
            id: id.to_owned(),
            title: self.title_of(id),
            start,
            minutes,
            offset: if self.persist {
                sessions::local_offset(start)
            } else {
                0
            },
        };
        if self.persist
            && let Err(err) = sessions::record(&session)
        {
            self.status = Some(format!("{err:#}"));
        }
        self.sessions.push(session);
    }

    fn title_of(&self, id: &str) -> String {
        self.game(id)
            .map_or_else(|| id.to_owned(), |game| game.title.clone())
    }

    /// Shows the most recently started running game on Discord, or nothing
    /// when none runs; does nothing while the Discord setting is off.
    pub(super) fn update_presence(&mut self) {
        let Some(presence) = &self.presence else {
            return;
        };
        presence.show(self.running.presence(|id| self.title_of(id)));
    }

    /// Play, or Running with Stop while the game runs.
    pub(super) fn play_buttons(&self, game: &Game) -> Element<'_, Message> {
        let Some(session) = self.running.games.get(&game.id) else {
            return button(style::button_text(t!("run-play")).size(16).font(BOLD))
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
            button(style::button_text(state).size(16).font(BOLD))
                .padding([10, 32])
                .style(style::primary),
            button(style::button_text(stop).font(BOLD))
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
        running::follow(game, move |message| sender.unbounded_send(message).is_ok());
        while let Some(message) = receiver.next().await {
            if iced::futures::SinkExt::send(&mut output, message)
                .await
                .is_err()
            {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use zendo_core::downloads::{Action, Job};
    use zendo_core::launch;
    use zendo_core::{Launch, Source};
    use zendo_shell::running::Session;

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
        assert!(zendo.sessions.is_empty(), "under a minute is no session");
    }

    fn sugar() -> Zendo {
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
    }

    #[test]
    fn a_watched_game_that_ran_a_minute_is_a_session() {
        let mut zendo = sugar();
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
        // As if it had run for two minutes.
        let session = zendo.running.games.get_mut("gog:1").unwrap();
        session.started = Instant::now()
            .checked_sub(Duration::from_secs(125))
            .unwrap();
        let since = session.since;
        let _ = zendo.update(Message::Run(RunMessage::Ended("gog:1".into())));
        assert_eq!(zendo.sessions.len(), 1);
        let kept = &zendo.sessions[0];
        assert_eq!(
            (kept.id.as_str(), kept.title.as_str(), kept.minutes),
            ("gog:1", "Sugar", 2)
        );
        assert_eq!(kept.start, since);
    }

    #[test]
    fn a_launcher_run_game_is_a_session_but_adds_no_minutes_of_its_own() {
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
        let _ = zendo.update(Message::Run(RunMessage::Seen("steam:620".into())));
        // As if its processes had been found for ten minutes.
        zendo.running.games.get_mut("steam:620").unwrap().started = Instant::now()
            .checked_sub(Duration::from_secs(605))
            .unwrap();
        let _ = zendo.update(Message::Run(RunMessage::Ended("steam:620".into())));
        assert_eq!(zendo.sessions.len(), 1);
        assert_eq!(zendo.sessions[0].id, "steam:620");
        assert_eq!(zendo.sessions[0].minutes, 10);
        // Steam counts its own time: Zendo's total does not move.
        assert_eq!(
            zendo.game("steam:620").map(|game| game.played.minutes),
            Some(0)
        );
    }

    #[test]
    fn a_game_never_seen_running_is_no_session() {
        let mut zendo = sugar();
        let mut waiting = Session::starting();
        waiting.started = Instant::now()
            .checked_sub(Duration::from_secs(600))
            .unwrap();
        zendo.running.games.insert("gog:1".into(), waiting);
        let _ = zendo.update(Message::Run(RunMessage::Ended("gog:1".into())));
        assert_eq!(zendo.sessions.len(), 0);
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

    #[test]
    fn a_job_changing_a_running_games_files_waits_for_it() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.install_path = Some("/home/me/Games/Sugar".into());
        let update = Job::for_game(&game, Action::Update).unwrap();
        let mut import = update.clone();
        import.action = Action::Import;
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        zendo
            .running
            .games
            .insert("gog:1".into(), Session::starting());
        assert!(zendo.held_by_game(&update));
        assert!(!zendo.held_by_game(&import));
        let _ = zendo.update(Message::Run(RunMessage::Ended("gog:1".into())));
        assert!(!zendo.held_by_game(&update));
    }
}
