//! Steam's installs, followed from Zendo: Install with Steam asks Steam's
//! own dialog, then Steam's manifests say how far each download is. They
//! show on the Downloads page and the game's Store page, and the library is
//! read again when one finishes. Uninstall and Verify are asked of Steam
//! the same way. What Steam does by itself is followed too (its own
//! downloads, updates and uninstalls); pausing and cancelling stay in Steam.

use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Size;
use zendo_core::steam::{self, AppState, Progress, Request, VerifyEvent};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// How often Steam's manifests are read while something is happening.
const BUSY: Duration = Duration::from_secs(2);
/// And otherwise, to notice downloads started in Steam.
const IDLE: Duration = Duration::from_secs(15);
/// How long an install asked of Steam waits for Steam to start it. Steam
/// writes nothing when its dialog is cancelled, so the wait ends by itself,
/// or sooner with Stop waiting.
const GIVE_UP: Duration = Duration::from_mins(3);

#[derive(Debug, Default)]
pub struct SteamInstalls {
    /// Installs asked of Steam that it has not started yet, by app.
    asked: HashMap<String, Asked>,
    /// Uninstalls asked of Steam that it has not done yet, by app.
    removing: HashMap<String, Asked>,
    /// Verifies asked of Steam, by app, followed in Steam's content log.
    verifying: HashMap<String, Verifying>,
    /// What Steam is downloading, installing or updating, by app.
    busy: HashMap<String, AppState>,
    /// The apps Steam had at the last look, by app: one installed then
    /// and gone since was uninstalled, one installed since was installed.
    seen: Option<HashMap<String, Seen>>,
    /// Installed apps Steam has been busy with since: updates, which end
    /// installed again.
    updating: HashSet<String>,
    /// Whether Steam's manifests are being read now.
    looking: bool,
}

#[derive(Debug)]
struct Asked {
    title: String,
    since: Instant,
}

#[derive(Debug)]
struct Seen {
    name: String,
    installed: bool,
}

#[derive(Debug)]
struct Verifying {
    title: String,
    since: Instant,
    /// Where Steam's content log ended when it was asked.
    from: u64,
    /// Whether the log says Steam started it.
    started: bool,
}

#[derive(Debug, Clone)]
pub enum SteamInstallMessage {
    /// Install with Steam pressed, for an app and its title.
    Install(String, String),
    /// Stop waiting for Steam to start an app's install (its dialog was
    /// cancelled).
    Forget(String),
    /// Uninstall with Steam pressed, for an app and its title.
    Uninstall(String, String),
    /// Verify with Steam pressed, for an app and its title.
    Verify(String, String),
    Tick,
    /// Steam's manifests read, and the verifies its log tells of.
    States(Vec<AppState>, Vec<VerifyEvent>),
}

/// What the page shows of a Steam game Zendo follows.
pub(super) enum Following<'a> {
    /// Asked; Steam's dialog is open, or Steam is starting.
    Asked,
    Busy(&'a AppState),
}

impl SteamInstalls {
    /// Whether anything is going on, for the faster look.
    pub(super) fn active(&self) -> bool {
        !self.asked.is_empty()
            || !self.busy.is_empty()
            || !self.removing.is_empty()
            || !self.verifying.is_empty()
    }

    /// Whether a verify asked of Steam is under way.
    pub(super) fn verifying(&self, app: &str) -> bool {
        self.verifying.contains_key(app)
    }

    /// What Steam is busy with, less the verifies, which show apart.
    fn busy(&self) -> impl Iterator<Item = &AppState> {
        self.busy
            .values()
            .filter(|state| !self.verifying.contains_key(&state.app))
    }

    /// Whether an uninstall asked of Steam is waiting on it.
    pub(super) fn removing(&self, app: &str) -> bool {
        self.removing.contains_key(app)
    }

    /// How many installs the Downloads page shows.
    pub(super) fn count(&self) -> usize {
        self.asked.len() + self.busy().count() + self.removing.len() + self.verifying.len()
    }

    /// The verifies Steam's log says are done, no longer followed; and
    /// those Steam never started (its window closed, or no log) dropped.
    fn verifies_done(&mut self, log: &[VerifyEvent]) -> Changes {
        let mut done = Changes::default();
        self.verifying.retain(|app, verifying| {
            for event in log
                .iter()
                .filter(|event| event.app == *app && event.at >= verifying.from)
            {
                if !event.finished {
                    verifying.started = true;
                } else if verifying.started {
                    done.add(app, &verifying.title);
                    return false;
                }
            }
            verifying.started || verifying.since.elapsed() < GIVE_UP
        });
        done
    }

    pub(super) fn following(&self, app: &str) -> Option<Following<'_>> {
        if let Some(state) = self.busy.get(app) {
            return Some(Following::Busy(state));
        }
        self.asked.contains_key(app).then_some(Following::Asked)
    }
}

impl Zendo {
    pub(super) fn update_steam_installs(&mut self, message: SteamInstallMessage) -> Task<Message> {
        match message {
            SteamInstallMessage::Install(app, title) => {
                // Asked again while waiting: Steam's dialog may have been
                // closed, so it opens again.
                if matches!(
                    self.steam_installs.following(&app),
                    Some(Following::Busy(_))
                ) {
                    return Task::none();
                }
                if self.persist
                    && let Err(err) = steam::ask(Request::Install, &app)
                {
                    self.status = Some(format!("{err:#}"));
                    return Task::none();
                }
                self.status = Some(t!("steam-asked", title = title.as_str()));
                self.steam_installs.asked.insert(
                    app,
                    Asked {
                        title,
                        since: Instant::now(),
                    },
                );
                Task::none()
            }
            SteamInstallMessage::Forget(app) => {
                self.steam_installs.asked.remove(&app);
                Task::none()
            }
            SteamInstallMessage::Uninstall(app, title) => {
                if self.persist
                    && let Err(err) = steam::ask(Request::Uninstall, &app)
                {
                    self.status = Some(format!("{err:#}"));
                    return Task::none();
                }
                self.status = Some(t!("steam-asked-uninstall", title = title.as_str()));
                self.steam_installs.removing.insert(
                    app,
                    Asked {
                        title,
                        since: Instant::now(),
                    },
                );
                Task::none()
            }
            SteamInstallMessage::Verify(app, title) => {
                if self.steam_installs.verifying(&app) {
                    return Task::none();
                }
                // Where the log ends now: the verify's lines come after.
                let from = if self.persist { steam::log_end() } else { 0 };
                if self.persist
                    && let Err(err) = steam::ask(Request::Verify, &app)
                {
                    self.status = Some(format!("{err:#}"));
                    return Task::none();
                }
                self.status = Some(t!("steam-asked-verify", title = title.as_str()));
                self.steam_installs.verifying.insert(
                    app,
                    Verifying {
                        title,
                        since: Instant::now(),
                        from,
                        started: false,
                    },
                );
                Task::none()
            }
            SteamInstallMessage::Tick => {
                if self.steam_installs.looking || !self.persist {
                    return Task::none();
                }
                self.steam_installs.looking = true;
                let from = self.steam_installs.verifying.values().map(|v| v.from).min();
                Task::perform(
                    blocking(move || {
                        (
                            steam::states(),
                            from.map(steam::verifies).unwrap_or_default(),
                        )
                    }),
                    |(states, verifies)| {
                        Message::SteamInstall(SteamInstallMessage::States(states, verifies))
                    },
                )
            }
            SteamInstallMessage::States(states, verifies) => self.steam_states(states, &verifies),
        }
    }

    /// Steam's manifests read: the downloads under way, and the installs
    /// and uninstalls since the last look, which bring a new read of the
    /// library; and the verifies Steam's log tells of.
    fn steam_states(&mut self, states: Vec<AppState>, log: &[VerifyEvent]) -> Task<Message> {
        let installs = &mut self.steam_installs;
        installs.looking = false;
        // A game Steam verifies is busy a while, not newly installed.
        let checked: HashSet<String> = installs.verifying.keys().cloned().collect();
        let verified = installs.verifies_done(log);
        let known: HashSet<&str> = states.iter().map(|state| state.app.as_str()).collect();
        let installed: HashMap<String, String> = states
            .iter()
            .filter(|state| state.progress == Progress::Installed)
            .map(|state| (state.app.clone(), state.name.clone()))
            .collect();
        let before = installs.seen.as_ref();
        let was_installed = |app: &str| {
            before
                .and_then(|before| before.get(app))
                .is_some_and(|seen| seen.installed)
        };
        for state in &states {
            if state.progress != Progress::Installed && was_installed(&state.app) {
                installs.updating.insert(state.app.clone());
            }
        }
        let mut finished = Changes::default();
        let mut updated = Changes::default();
        for (app, name) in &installed {
            let was_updating = installs.updating.remove(app);
            if checked.contains(app) || was_installed(app) {
                continue;
            }
            if was_updating {
                updated.add(app, name);
            } else if before.is_some() {
                finished.add(app, name);
            }
        }
        installs.asked.retain(|app, asked| {
            if installed.contains_key(app) {
                finished.add(app, &asked.title);
                return false;
            }
            // Started: followed by its manifest from now on.
            !known.contains(app.as_str()) && asked.since.elapsed() < GIVE_UP
        });
        let mut removed = Changes::default();
        for (app, seen) in before.into_iter().flatten() {
            // A download cancelled in Steam goes too: not an uninstall.
            let had = seen.installed || installs.updating.contains(app);
            if had && !known.contains(app.as_str()) {
                removed.add(app, &seen.name);
            }
        }
        installs.updating.retain(|app| known.contains(app.as_str()));
        installs.removing.retain(|app, asked| {
            if known.contains(app.as_str()) {
                return asked.since.elapsed() < GIVE_UP;
            }
            removed.add(app, &asked.title);
            false
        });
        installs.seen = Some(
            states
                .iter()
                .map(|state| {
                    let seen = Seen {
                        name: state.name.clone(),
                        installed: state.progress == Progress::Installed,
                    };
                    (state.app.clone(), seen)
                })
                .collect(),
        );
        installs.busy = states
            .into_iter()
            .filter(|state| state.progress != Progress::Installed)
            .map(|state| (state.app.clone(), state))
            .collect();
        let verified = verified.line(|titles| t!("steam-verified", title = titles));
        let notes: Vec<String> = [
            finished.line(|titles| t!("steam-installed", title = titles)),
            updated.line(|titles| t!("steam-updated", title = titles)),
            removed.line(|titles| t!("steam-uninstalled", title = titles)),
        ]
        .into_iter()
        .flatten()
        .collect();
        if notes.is_empty() {
            if verified.is_some() {
                self.status = verified;
            }
            return Task::none();
        }
        let note = notes
            .into_iter()
            .chain(verified)
            .collect::<Vec<_>>()
            .join(" · ");
        self.status = Some(note.clone());
        // Said again once the library is read, which clears the line.
        self.local.after_scan = Some((None, note));
        self.refresh()
    }

    /// The Downloads page's card for Steam, when it has something going.
    pub(super) fn steam_downloads_card(&self) -> Option<Element<'_, Message>> {
        let installs = &self.steam_installs;
        if !installs.active() {
            return None;
        }
        let mut card = column![
            row![
                text(t!("steam-downloads-title")).size(18).font(BOLD),
                space::horizontal(),
                button(text(t!("steam-open-downloads")).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::Open("steam://open/downloads".to_owned())),
            ]
            .align_y(Alignment::Center),
            text(t!("steam-downloads-about"))
                .size(13)
                .color(colors().dim),
        ]
        .spacing(12);
        let mut asked: Vec<(&String, &Asked)> = installs.asked.iter().collect();
        asked.sort_by(|a, b| a.1.title.cmp(&b.1.title));
        for (app, asked) in asked {
            card = card.push(
                row![
                    text(&asked.title).size(15).width(Length::Fill),
                    text(t!("steam-waiting-dialog"))
                        .size(13)
                        .color(colors().dim),
                    button(text(t!("steam-stop-waiting")).size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press(Message::SteamInstall(SteamInstallMessage::Forget(
                            app.clone()
                        ))),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        }
        let mut removing: Vec<&Asked> = installs.removing.values().collect();
        removing.sort_by(|a, b| a.title.cmp(&b.title));
        for asked in removing {
            card = card.push(
                row![
                    text(&asked.title).size(15).width(Length::Fill),
                    text(t!("steam-waiting-uninstall"))
                        .size(13)
                        .color(colors().dim),
                ]
                .align_y(Alignment::Center),
            );
        }
        let mut verifying: Vec<&Verifying> = installs.verifying.values().collect();
        verifying.sort_by(|a, b| a.title.cmp(&b.title));
        for verifying in verifying {
            card = card.push(
                row![
                    text(&verifying.title).size(15).width(Length::Fill),
                    text(if verifying.started {
                        t!("steam-verifying")
                    } else {
                        t!("steam-waiting-verify")
                    })
                    .size(13)
                    .color(colors().dim),
                ]
                .align_y(Alignment::Center),
            );
        }
        let mut busy: Vec<&AppState> = installs.busy().collect();
        busy.sort_by(|a, b| a.name.cmp(&b.name));
        for state in busy {
            card = card.push(busy_row(state));
        }
        Some(
            container(card)
                .padding(20)
                .width(Length::Fill)
                .style(style::card)
                .into(),
        )
    }
}

/// The apps installed or uninstalled at a look, each once, by title.
#[derive(Default)]
struct Changes {
    apps: HashSet<String>,
    titles: Vec<String>,
}

impl Changes {
    fn add(&mut self, app: &str, title: &str) {
        if self.apps.insert(app.to_owned()) {
            self.titles.push(title.to_owned());
        }
    }

    /// The line saying what changed, when anything did.
    fn line(mut self, say: impl Fn(String) -> String) -> Option<String> {
        self.titles.sort();
        (!self.titles.is_empty()).then(|| say(self.titles.join(", ")))
    }
}

/// What Steam is doing with an app, and how far, in words.
pub(super) fn progress_line(progress: Progress) -> String {
    let of = |done: u64, total: u64| {
        t!(
            "size-of",
            done = Size::Bytes(done).to_string(),
            total = Size::Bytes(total).to_string()
        )
    };
    match progress {
        Progress::Waiting => t!("steam-state-waiting"),
        Progress::Downloading { done, total } => {
            t!("steam-state-downloading", progress = of(done, total))
        }
        Progress::Installing { done, total } => {
            t!("steam-state-installing", progress = of(done, total))
        }
        Progress::Paused { done, total } => t!("steam-state-paused", progress = of(done, total)),
        Progress::Installed => t!("installed"),
    }
}

/// How far along, out of 100.
pub(super) fn percent(progress: Progress) -> f32 {
    match progress {
        Progress::Downloading { done, total }
        | Progress::Installing { done, total }
        | Progress::Paused { done, total }
            if total > 0 =>
        {
            #[expect(clippy::cast_precision_loss, reason = "a bar's width")]
            let percent = done as f32 * 100.0 / total as f32;
            percent
        }
        Progress::Installed => 100.0,
        _ => 0.0,
    }
}

fn busy_row(state: &AppState) -> Element<'_, Message> {
    column![
        row![
            text(&state.name).size(15).width(Length::Fill),
            text(progress_line(state.progress))
                .size(13)
                .color(colors().dim),
        ]
        .align_y(Alignment::Center),
        progress_bar(0.0..=100.0, percent(state.progress))
            .girth(8)
            .style(style::progress),
    ]
    .spacing(8)
    .into()
}

/// A tick every [`BUSY`], for as long as it is listened to.
pub fn busy_ticks() -> impl Stream<Item = ()> {
    ticks(BUSY)
}

/// A tick every [`IDLE`].
pub fn idle_ticks() -> impl Stream<Item = ()> {
    ticks(IDLE)
}

fn ticks(every: Duration) -> impl Stream<Item = ()> {
    iced::stream::channel(1, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            while sender.unbounded_send(()).is_ok() {
                thread::sleep(every);
            }
        });
        while let Some(()) = receiver.next().await {
            if output.send(()).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(app: &str, progress: Progress) -> AppState {
        AppState {
            app: app.into(),
            name: format!("Game {app}"),
            progress,
        }
    }

    #[test]
    fn follows_an_install_until_steam_finishes_it() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Install(
            "620".into(),
            "Portal 2".into(),
        )));
        assert!(matches!(
            zendo.steam_installs.following("620"),
            Some(Following::Asked)
        ));
        // Steam's dialog cancelled: Stop waiting, then asked again.
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Forget(
            "620".into(),
        )));
        assert!(!zendo.steam_installs.active());
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Install(
            "620".into(),
            "Portal 2".into(),
        )));
        // Steam has started: its manifest says how far.
        let downloading = Progress::Downloading {
            done: 10,
            total: 100,
        };
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
            vec![state("620", downloading), state("730", Progress::Installed)],
            Vec::new(),
        )));
        assert!(zendo.steam_installs.asked.is_empty());
        assert!(matches!(
            zendo.steam_installs.following("620"),
            Some(Following::Busy(found)) if found.progress == downloading
        ));
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
            vec![
                state("620", Progress::Installed),
                state("730", Progress::Installed),
            ],
            Vec::new(),
        )));
        assert!(!zendo.steam_installs.active());
        assert_eq!(
            zendo.local.after_scan,
            Some((None, "Game 620 is installed".into()))
        );
    }

    #[test]
    fn notices_installs_and_uninstalls_done_in_steam() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let states = |zendo: &mut Zendo, list: Vec<AppState>| {
            let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
                list,
                Vec::new(),
            )));
        };
        // The first look only learns what is installed.
        states(&mut zendo, vec![state("620", Progress::Installed)]);
        assert_eq!(zendo.local.after_scan, None);
        // Installed in Steam between two looks.
        states(
            &mut zendo,
            vec![
                state("620", Progress::Installed),
                state("730", Progress::Installed),
            ],
        );
        assert_eq!(
            zendo.local.after_scan,
            Some((None, "Game 730 is installed".into()))
        );
        // Steam updates one: busy a while, then installed again.
        let downloading = Progress::Downloading { done: 1, total: 2 };
        states(
            &mut zendo,
            vec![state("620", Progress::Installed), state("730", downloading)],
        );
        states(
            &mut zendo,
            vec![
                state("620", Progress::Installed),
                state("730", Progress::Installed),
            ],
        );
        assert_eq!(
            zendo.local.after_scan,
            Some((None, "Game 730 is updated".into()))
        );
        // Uninstall asked from Zendo; Steam removes its manifest.
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Uninstall(
            "620".into(),
            "Portal 2".into(),
        )));
        assert!(zendo.steam_installs.removing("620"));
        states(&mut zendo, vec![state("730", Progress::Installed)]);
        assert!(!zendo.steam_installs.active());
        assert_eq!(
            zendo.local.after_scan,
            Some((None, "Game 620 is uninstalled".into()))
        );
    }

    #[test]
    fn follows_a_verify_through_steams_log() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let look = |zendo: &mut Zendo, progress: Progress, verifies: Vec<VerifyEvent>| {
            let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
                vec![state("620", progress)],
                verifies,
            )));
        };
        let event = |at: u64, finished: bool| VerifyEvent {
            at,
            app: "620".into(),
            finished,
        };
        look(&mut zendo, Progress::Installed, Vec::new());
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Verify(
            "620".into(),
            "Portal 2".into(),
        )));
        assert!(zendo.steam_installs.verifying("620"));
        // Steam verifying: busy, but shown as a verify, not a download.
        look(&mut zendo, Progress::Waiting, vec![event(10, false)]);
        assert_eq!(zendo.steam_installs.count(), 1);
        look(
            &mut zendo,
            Progress::Installed,
            vec![event(10, false), event(20, true)],
        );
        assert!(!zendo.steam_installs.active());
        assert_eq!(zendo.local.after_scan, None, "not read again as an install");
        assert_eq!(
            zendo.status.as_deref(),
            Some("Steam verified the files of Portal 2")
        );
    }

    #[test]
    fn says_how_far_in_words_and_percent() {
        let paused = Progress::Paused {
            done: 512,
            total: 1024,
        };
        assert!((percent(paused) - 50.0).abs() < f32::EPSILON);
        assert!(progress_line(paused).starts_with("Paused in Steam"));
        assert!(percent(Progress::Waiting).abs() < f32::EPSILON);
    }
}
