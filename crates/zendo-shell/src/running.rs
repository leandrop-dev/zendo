//! The games Zendo started and still runs: Play turns into Running, with a
//! Stop that ends the game, and a second press that kills it when it does
//! not end. A game handed to Steam or Heroic shows Starting until its
//! processes turn up, found by its install folder.
//!
//! The state and its transitions are here, with no toolkit: [`Running::apply`]
//! takes what happened ([`RunMessage`]) and says what the window must do
//! about it ([`Effect`]). [`follow`] starts the game on a thread and reports
//! to a closure, which each toolkit turns into its own kind of event.

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use zendo_core::launch::{self, Started, Watch};
use zendo_core::{Game, Launch};
use zendo_core::{cloud_saves, galaxy, gog_playtime, gog_setup, known_fixes};

use crate::presence::Presence;
use crate::t;

/// How often Zendo looks whether a game still runs.
const POLL: Duration = Duration::from_secs(2);

/// How long Steam or Heroic may take to start a game (an update first, a
/// launcher's window) before Zendo stops waiting for it.
const WAIT: Duration = Duration::from_mins(5);

/// How many looks in a row must find none of a game's processes before it
/// counts as closed: a game's own launcher may hand over to the game with a
/// moment between.
const GONE: u32 = 3;

/// How long Zendo waits for the game's window before hiding itself anyway:
/// a native Wayland game has no X11 window Zendo can see.
const WINDOW_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct Running {
    pub games: HashMap<String, Session>,
}

#[derive(Debug)]
pub struct Session {
    /// How it is followed; `None` while it starts.
    pub watch: Option<Watch>,
    /// Its processes were seen: at once for a game Zendo starts itself,
    /// later for one its launcher starts.
    pub seen: bool,
    pub started: Instant,
    /// When its processes were seen, Unix seconds: Discord's start time.
    pub since: u64,
    /// Stop was pressed: the next press kills it.
    pub stopping: bool,
    /// An anti-cheat runtime it needs could not be downloaded.
    pub warning: Option<String>,
}

impl Session {
    #[must_use]
    pub fn starting() -> Self {
        Self {
            watch: None,
            seen: false,
            started: Instant::now(),
            since: 0,
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
    /// GOG's setup runs in the game's prefix before its first start: the
    /// step under way.
    SettingUp(String, String),
    /// Some of it failed; the game starts all the same.
    SetupFailed(String, String),
    /// Some known fixes failed; the game starts all the same.
    FixesFailed(String, String),
    /// GOG Galaxy could not be set up or started; the game starts without.
    GalaxyFailed(String, String),
    Started(String, Result<Started, String>),
    /// The launcher started the game: its processes turned up.
    Seen(String),
    /// Its window is up, or Zendo waited long enough for one: hide now.
    Shown(String),
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

/// What a message changed that the window must carry out, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The line at the foot of the window.
    Say(String),
    /// The saves on disk changed: read them again.
    CloudReload,
    /// The game's saves are syncing, or are done.
    CloudSyncing(String, bool),
    /// Count a launch of the game now, and keep the cursor on it.
    Launched(String),
    /// Zendo timed the game: add the minutes to its total.
    Played(String, u64),
    /// A session was watched to its end: keep it.
    Session {
        id: String,
        since: u64,
        minutes: u64,
    },
    /// Who is playing may have changed: tell Discord.
    Presence,
    /// A game is up: hide the window when the setting asks for it.
    HideOnLaunch,
    /// A game ended: show the window again when it was hidden for the launch.
    ShowOnClose,
    /// The game ended: a job held while it ran can go now.
    NextJob,
    /// GOG's total now holds the session just sent.
    RefreshGogTotal(String),
    /// The game's Play, badge or card changed.
    Changed(String),
}

/// What the words around a game need to know about it.
#[derive(Debug, Clone)]
pub struct Names {
    pub title: String,
    /// The program a link hands the game to.
    pub launcher: String,
}

impl Names {
    #[must_use]
    pub fn of(id: &str, game: Option<&Game>) -> Self {
        Self {
            title: game.map_or_else(|| id.to_owned(), |game| game.title.clone()),
            launcher: launcher(game),
        }
    }
}

/// Unix seconds now.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The program a link hands the game to.
fn launcher(game: Option<&Game>) -> String {
    match game.map(|game| &game.launch) {
        Some(Launch::Uri { uri }) if uri.starts_with("steam:") => "Steam".to_owned(),
        Some(Launch::Uri { uri }) if uri.starts_with("heroic:") => "Heroic".to_owned(),
        _ => t!("run-its-launcher"),
    }
}

impl Running {
    /// Whether the game is seen running (not while it starts).
    #[must_use]
    pub fn is_running(&self, id: &str) -> bool {
        self.games.get(id).is_some_and(|session| session.seen)
    }

    /// Whether any game is starting or running.
    #[must_use]
    pub fn busy(&self) -> bool {
        !self.games.is_empty()
    }

    /// Whether the game is starting (its prefix being set up, say) or
    /// running: its files are in use either way.
    #[must_use]
    pub fn is_open(&self, id: &str) -> bool {
        self.games.contains_key(id)
    }

    /// Marks the game as starting. `false` when it already is.
    pub fn begin(&mut self, id: &str) -> bool {
        if self.games.contains_key(id) {
            return false;
        }
        self.games.insert(id.to_owned(), Session::starting());
        true
    }

    /// The running game that was seen last, for Discord.
    #[must_use]
    pub fn current(&self) -> Option<(&str, &Session)> {
        self.games
            .iter()
            .filter(|(_, session)| session.seen)
            .max_by_key(|(_, session)| session.started)
            .map(|(id, session)| (id.as_str(), session))
    }

    /// What Discord should show now.
    #[must_use]
    pub fn presence(&self, title_of: impl Fn(&str) -> String) -> Presence {
        match self.current() {
            Some((id, session)) => Presence::Playing {
                title: title_of(id),
                since: session.since,
            },
            None => Presence::Idle,
        }
    }

    /// Takes in what happened to a game, and says what the window must do.
    pub fn apply(&mut self, message: RunMessage, names: &dyn Fn(&str) -> Names) -> Vec<Effect> {
        match message {
            RunMessage::Fetching(id, runtime) => {
                let title = names(&id).title;
                vec![Effect::Say(t!(
                    "run-fetching-runtime",
                    runtime = runtime,
                    title = title
                ))]
            }
            RunMessage::SettingUp(id, step) => {
                let title = names(&id).title;
                vec![Effect::Say(t!(
                    "run-gog-setup",
                    title = title,
                    step = step.as_str()
                ))]
            }
            RunMessage::FetchFailed(id, err) => {
                let title = names(&id).title;
                self.warn(
                    &id,
                    t!("run-runtime-failed", title = title, error = err.as_str()),
                )
            }
            RunMessage::SetupFailed(id, err) => {
                let title = names(&id).title;
                self.warn(
                    &id,
                    t!("run-gog-setup-failed", title = title, error = err.as_str()),
                )
            }
            RunMessage::FixesFailed(id, err) => {
                let title = names(&id).title;
                self.warn(
                    &id,
                    t!(
                        "run-known-fixes-failed",
                        title = title,
                        error = err.as_str()
                    ),
                )
            }
            RunMessage::GalaxyFailed(id, err) => {
                let title = names(&id).title;
                self.warn(
                    &id,
                    t!("run-galaxy-failed", title = title, error = err.as_str()),
                )
            }
            RunMessage::Started(id, Ok(started)) => {
                // Its saves were fetched as it started.
                let mut effects = vec![Effect::CloudReload];
                effects.extend(self.started(&id, started, &names(&id)));
                effects
            }
            RunMessage::Started(id, Err(err)) => {
                self.games.remove(&id);
                vec![Effect::CloudReload, Effect::Say(err), Effect::Changed(id)]
            }
            RunMessage::Seen(id) => {
                let mut effects = Vec::new();
                if let Some(session) = self.games.get_mut(&id) {
                    session.seen = true;
                    session.started = Instant::now();
                    session.since = now();
                    effects.push(Effect::Say(t!("run-running", title = names(&id).title)));
                }
                effects.extend([Effect::Presence, Effect::Changed(id)]);
                effects
            }
            RunMessage::Shown(id) => {
                if self.games.contains_key(&id) {
                    vec![Effect::HideOnLaunch]
                } else {
                    Vec::new()
                }
            }
            RunMessage::Missed(id) => {
                if self.games.remove(&id).is_none() {
                    return Vec::new();
                }
                let Names { title, launcher } = names(&id);
                vec![
                    Effect::Say(t!("run-missed", title = title, launcher = launcher)),
                    Effect::Changed(id),
                ]
            }
            RunMessage::Ended(id) => self.ended(id, names),
            RunMessage::SavesSyncing(id) => vec![Effect::CloudSyncing(id, true)],
            RunMessage::AfterDone(id, result) => {
                let mut effects =
                    vec![Effect::CloudSyncing(id.clone(), false), Effect::CloudReload];
                if let Err(err) = result {
                    effects.push(Effect::Say(err));
                }
                effects.push(Effect::RefreshGogTotal(id));
                effects
            }
            RunMessage::Stop(id) => self.stop(id, names),
        }
    }

    /// The game's process group is gone.
    fn ended(&mut self, id: String, names: &dyn Fn(&str) -> Names) -> Vec<Effect> {
        let Some(session) = self.games.remove(&id) else {
            return Vec::new();
        };
        let minutes = session.started.elapsed().as_secs() / 60;
        let mut effects = Vec::new();
        // A game seen running and watched to its end is a session, whoever
        // times it for the totals.
        if session.seen && minutes > 0 {
            effects.push(Effect::Session {
                id: id.clone(),
                since: session.since,
                minutes,
            });
        }
        // Steam and Heroic count the time of the games they run.
        let own = matches!(session.watch, Some(Watch::Group(_)));
        if own && minutes > 0 {
            effects.push(Effect::Played(id.clone(), minutes));
        }
        let title = names(&id).title;
        effects.extend([
            Effect::Say(t!("run-closed", title = title, minutes = minutes)),
            Effect::ShowOnClose,
            Effect::Presence,
            Effect::NextJob,
            Effect::Changed(id),
        ]);
        effects
    }

    /// Stop was pressed: asks the game to close, and on the second press
    /// kills it. Nothing before there is something to stop.
    fn stop(&mut self, id: String, names: &dyn Fn(&str) -> Names) -> Vec<Effect> {
        let Some(session) = self.games.get_mut(&id) else {
            return Vec::new();
        };
        let Some(watch) = session.watch.as_ref().filter(|_| session.seen) else {
            return Vec::new();
        };
        if session.stopping {
            launch::kill(watch);
            return Vec::new();
        }
        session.stopping = true;
        let asked = launch::stop(watch);
        let title = names(&id).title;
        vec![
            Effect::Say(if asked {
                t!("run-asked-close", title = title)
            } else {
                t!("run-asked-end", title = title)
            }),
            Effect::Changed(id),
        ]
    }

    /// A failure before the game starts is kept, to say once the game runs.
    fn warn(&mut self, id: &str, warning: String) -> Vec<Effect> {
        if let Some(session) = self.games.get_mut(id) {
            session.warning = Some(warning);
        }
        Vec::new()
    }

    /// A game's launcher is up: watch it, count the launch.
    fn started(&mut self, id: &str, started: Started, names: &Names) -> Vec<Effect> {
        let Names { title, launcher } = names;
        let mut effects = Vec::new();
        let mut running_now = false;
        match started.watch {
            Watch::Nothing => {
                self.games.remove(id);
                effects.push(Effect::Say(t!(
                    "run-handed",
                    title = title.as_str(),
                    launcher = launcher.as_str()
                )));
            }
            watch => {
                if let Some(session) = self.games.get_mut(id) {
                    running_now = matches!(watch, Watch::Group(_));
                    session.seen = running_now;
                    session.started = Instant::now();
                    session.since = now();
                    let warning = session.warning.take().or(started.warning);
                    effects.push(Effect::Say(if let Some(warning) = warning {
                        warning
                    } else if session.seen {
                        t!("run-running", title = title.as_str())
                    } else {
                        t!(
                            "run-waiting",
                            title = title.as_str(),
                            launcher = launcher.as_str()
                        )
                    }));
                    session.watch = Some(watch);
                }
            }
        }
        // Zendo's state now has the launch; show it in the order without
        // scanning again.
        effects.push(Effect::Launched(id.to_owned()));
        if running_now {
            effects.push(Effect::Presence);
        }
        effects.push(Effect::Changed(id.to_owned()));
        effects
    }
}

/// What a game's prefix gets before the game starts, the first time or
/// when there is more: GOG's setup, then the known fixes not tried yet
/// (with Heroic's list asked again once a day; offline, the one kept),
/// then GOG Galaxy's stand-in service.
/// Each step and failure goes to `send`; none stops the game.
fn prepare(game: &Game, send: &mut dyn FnMut(RunMessage)) {
    let id = game.id.clone();
    if gog_setup::needed(game) {
        let result = gog_setup::run(game, &mut |step| {
            send(RunMessage::SettingUp(id.clone(), step.to_owned()));
        });
        let failed = match result {
            Ok(setup) => setup.failed.join("; "),
            Err(err) => format!("{err:#}"),
        };
        if !failed.is_empty() {
            send(RunMessage::SetupFailed(id.clone(), failed));
        }
    }
    apply_known_fixes(game, send);
    if galaxy::service_needed(game) {
        send(RunMessage::SettingUp(id.clone(), t!("run-galaxy-service")));
        if let Err(err) = galaxy::install_service(game) {
            send(RunMessage::GalaxyFailed(id, format!("{err:#}")));
        }
    }
}

/// The known fixes not tried yet in the game's prefix.
fn apply_known_fixes(game: &Game, send: &mut dyn FnMut(RunMessage)) {
    let id = game.id.clone();
    if !known_fixes::applies(game) {
        return;
    }
    let _ = known_fixes::refresh();
    let Some(fixes) = known_fixes::pending(game) else {
        return;
    };
    let result = known_fixes::apply(game, &fixes, &mut |step| {
        send(RunMessage::SettingUp(
            id.clone(),
            t!("run-known-fix", fix = step),
        ));
    });
    let failed = match result {
        Ok(applied) => applied.failed.join("; "),
        Err(err) => format!("{err:#}"),
    };
    if !failed.is_empty() {
        send(RunMessage::FixesFailed(id, failed));
    }
}

/// Starts the game on a thread of its own (its launch may look things up),
/// says how, and then says when its process group is gone. `send` returns
/// `false` once nothing wants the messages any more.
pub fn follow(game: Game, send: impl Fn(RunMessage) -> bool + Send + 'static) {
    thread::spawn(move || {
        let id = game.id.clone();
        prepare(&game, &mut |message| {
            send(message);
        });
        let fetched = launch::fetch_runtimes(&game, &mut |runtime| {
            send(RunMessage::Fetching(id.clone(), runtime.label()));
        });
        if let Err(err) = fetched {
            send(RunMessage::FetchFailed(id.clone(), format!("{err:#}")));
        }
        // GOG Galaxy beside the game, stopped once the game ends.
        let comet = galaxy::start(&game).unwrap_or_else(|err| {
            send(RunMessage::GalaxyFailed(id.clone(), format!("{err:#}")));
            None
        });
        let result = launch::start(&game).map_err(|err| format!("{err:#}"));
        // When the session began, for GOG.
        let began = now();
        let watch = result.as_ref().ok().map(|started| started.watch.clone());
        send(RunMessage::Started(id.clone(), result));
        let own = matches!(watch, Some(Watch::Group(_)));
        match watch {
            Some(watch @ Watch::Group(_)) => {
                let mut shown = false;
                let since = Instant::now();
                while launch::running(&watch) {
                    if !shown
                        && (launch::any_window_mapped(&watch) || since.elapsed() > WINDOW_WAIT)
                    {
                        if !send(RunMessage::Shown(id.clone())) {
                            return;
                        }
                        shown = true;
                    }
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
                let mut shown = false;
                let since = Instant::now();
                let mut gone = 0;
                while gone < GONE {
                    if !shown
                        && (launch::any_window_mapped(&watch) || since.elapsed() > WINDOW_WAIT)
                    {
                        if !send(RunMessage::Shown(id.clone())) {
                            return;
                        }
                        shown = true;
                    }
                    thread::sleep(POLL);
                    gone = if launch::running(&watch) { 0 } else { gone + 1 };
                }
            }
            _ => return,
        }
        drop(comet);
        if !send(RunMessage::Ended(id.clone())) {
            return;
        }
        // Only for a game Zendo started (its group): the others' launchers
        // have their own.
        if own {
            if cloud_saves::wanted_for(&game) {
                send(RunMessage::SavesSyncing(id.clone()));
            }
            // The saves and the script, then the session for GOG: one not
            // sent is kept and goes with the next.
            let after = launch::after(&game);
            let playtime = gog_playtime::send(&game, began, now());
            let result = after.and(playtime).map_err(|err| format!("{err:#}"));
            send(RunMessage::AfterDone(id, result));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::Source;

    fn sugar() -> Game {
        Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        )
    }

    fn names(id: &str) -> Names {
        Names::of(id, Some(&sugar()))
    }

    fn said(effects: &[Effect]) -> Option<&str> {
        effects.iter().rev().find_map(|effect| match effect {
            Effect::Say(line) => Some(line.as_str()),
            _ => None,
        })
    }

    #[test]
    fn a_game_runs_until_its_group_is_gone() {
        let mut running = Running::default();
        assert!(running.begin("gog:1"));
        assert!(!running.begin("gog:1"), "already starting");
        let started = Started {
            watch: Watch::Group(launch::Group::new(4_000_000, None)),
            log: None,
            warning: None,
        };
        let effects = running.apply(RunMessage::Started("gog:1".into(), Ok(started)), &names);
        assert!(running.is_running("gog:1"));
        assert_eq!(said(&effects), Some("Sugar is running"));
        assert!(effects.contains(&Effect::Launched("gog:1".into())));
        assert!(
            !effects.contains(&Effect::HideOnLaunch),
            "the game's window is not up yet"
        );
        let effects = running.apply(RunMessage::Shown("gog:1".into()), &names);
        assert!(effects.contains(&Effect::HideOnLaunch));
        let effects = running.apply(RunMessage::Ended("gog:1".into()), &names);
        assert!(!running.is_running("gog:1"));
        assert_eq!(said(&effects), Some("Sugar closed"));
        assert!(effects.contains(&Effect::ShowOnClose));
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::Session { .. })),
            "under a minute is no session"
        );
    }

    #[test]
    fn a_watched_game_that_ran_a_minute_is_a_session_and_adds_its_minutes() {
        let mut running = Running::default();
        running.begin("gog:1");
        let started = Started {
            watch: Watch::Group(launch::Group::new(4_000_000, None)),
            log: None,
            warning: None,
        };
        running.apply(RunMessage::Started("gog:1".into(), Ok(started)), &names);
        let session = running.games.get_mut("gog:1").unwrap();
        session.started = Instant::now()
            .checked_sub(Duration::from_secs(125))
            .unwrap();
        let since = session.since;
        let effects = running.apply(RunMessage::Ended("gog:1".into()), &names);
        assert!(effects.contains(&Effect::Session {
            id: "gog:1".into(),
            since,
            minutes: 2
        }));
        assert!(effects.contains(&Effect::Played("gog:1".into(), 2)));
    }

    #[test]
    fn a_launcher_run_game_is_a_session_but_adds_no_minutes_of_its_own() {
        let mut running = Running::default();
        running.begin("steam:620");
        let started = Started {
            watch: Watch::Folder("/games/Portal 2".into()),
            log: None,
            warning: None,
        };
        running.apply(RunMessage::Started("steam:620".into(), Ok(started)), &names);
        assert!(!running.is_running("steam:620"));
        running.apply(RunMessage::Seen("steam:620".into()), &names);
        assert!(running.is_running("steam:620"));
        running.games.get_mut("steam:620").unwrap().started = Instant::now()
            .checked_sub(Duration::from_secs(605))
            .unwrap();
        let effects = running.apply(RunMessage::Ended("steam:620".into()), &names);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Session { minutes: 10, .. }))
        );
        assert!(!effects.iter().any(|e| matches!(e, Effect::Played(..))));
    }

    #[test]
    fn a_game_never_seen_running_is_no_session() {
        let mut running = Running::default();
        let mut waiting = Session::starting();
        waiting.started = Instant::now()
            .checked_sub(Duration::from_secs(600))
            .unwrap();
        running.games.insert("gog:1".into(), waiting);
        let effects = running.apply(RunMessage::Ended("gog:1".into()), &names);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Session { .. })));
    }

    #[test]
    fn a_failed_start_leaves_play_as_it_was() {
        let mut running = Running::default();
        running.begin("gog:1");
        let effects = running.apply(
            RunMessage::Started("gog:1".into(), Err("umu-launcher is needed".into())),
            &names,
        );
        assert!(!running.is_open("gog:1"));
        assert_eq!(said(&effects), Some("umu-launcher is needed"));
    }

    #[test]
    fn stop_waits_for_something_to_stop() {
        let mut running = Running::default();
        running.begin("steam:620");
        let started = Started {
            watch: Watch::Folder("/games/Portal 2".into()),
            log: None,
            warning: None,
        };
        running.apply(RunMessage::Started("steam:620".into(), Ok(started)), &names);
        running.apply(RunMessage::Stop("steam:620".into()), &names);
        assert!(!running.games["steam:620"].stopping);
    }
}
