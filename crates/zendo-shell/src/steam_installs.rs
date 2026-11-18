//! Steam's installs, followed from Zendo: Install with Steam asks Steam's
//! own dialog, then Steam's manifests say how far each download is. They
//! show on the Downloads page and the game's Store page, and the library is
//! read again when one finishes. Uninstall and Verify are asked of Steam
//! the same way. What Steam does by itself is followed too (its own
//! downloads, updates and uninstalls); pausing and cancelling stay in Steam.
//!
//! The state and its transitions are here, with no toolkit (the window is Slint).
//! Each method says what the window must do
//! about what happened: a line to say and whether the library is read again.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use zendo_core::Size;
use zendo_core::steam::{self, AppState, Progress, Request, VerifyEvent};

use crate::t;

/// How often Steam's manifests are read while something is happening.
pub const BUSY: Duration = Duration::from_secs(2);
/// And otherwise, to notice downloads started in Steam.
pub const IDLE: Duration = Duration::from_secs(15);
/// How long an install asked of Steam waits for Steam to start it. Steam
/// writes nothing when its dialog is cancelled, so the wait ends by itself,
/// or sooner with Stop waiting.
const GIVE_UP: Duration = Duration::from_mins(3);

/// A look whose thread died is taken over after this long: reading Steam's
/// manifests takes a moment, a broken thread never answers.
const LOOK_GIVE_UP: Duration = Duration::from_secs(30);

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
    /// When that look began: one whose thread died must not stop every
    /// later look for the rest of the session.
    looking_since: Option<Instant>,
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

/// What the page shows of a Steam game Zendo follows.
pub enum Following<'a> {
    /// Asked; Steam's dialog is open, or Steam is starting.
    Asked,
    Busy(&'a AppState),
}

/// What a look at Steam's manifests asks of the window.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Looked {
    /// A line to say.
    pub say: Option<String>,
    /// Whether something was installed, updated or uninstalled, so the
    /// library is read again, and the line said once more after.
    pub rescan: bool,
}

/// One line of the Steam card on the Downloads page.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub app: String,
    pub title: String,
    /// What is going on, in words.
    pub line: String,
    /// How far along, out of 100, for a download.
    pub percent: Option<f32>,
    /// Whether Stop waiting applies (an install asked, not yet started).
    pub forget: bool,
}

impl SteamInstalls {
    /// Whether anything is going on, for the faster look.
    #[must_use]
    pub fn active(&self) -> bool {
        !self.asked.is_empty()
            || !self.busy.is_empty()
            || !self.removing.is_empty()
            || !self.verifying.is_empty()
    }

    /// How often to look now.
    #[must_use]
    pub fn every(&self) -> Duration {
        if self.active() { BUSY } else { IDLE }
    }

    /// Whether a verify asked of Steam is under way.
    #[must_use]
    pub fn verifying(&self, app: &str) -> bool {
        self.verifying.contains_key(app)
    }

    /// Whether an uninstall asked of Steam is waiting on it.
    #[must_use]
    pub fn removing(&self, app: &str) -> bool {
        self.removing.contains_key(app)
    }

    /// What Steam is busy with, less the verifies, which show apart.
    fn busy(&self) -> impl Iterator<Item = &AppState> {
        self.busy
            .values()
            .filter(|state| !self.verifying.contains_key(&state.app))
    }

    /// How many installs the Downloads page shows.
    #[must_use]
    pub fn count(&self) -> usize {
        self.asked.len() + self.busy().count() + self.removing.len() + self.verifying.len()
    }

    #[must_use]
    pub fn following(&self, app: &str) -> Option<Following<'_>> {
        if let Some(state) = self.busy.get(app) {
            return Some(Following::Busy(state));
        }
        self.asked.contains_key(app).then_some(Following::Asked)
    }

    /// The lines of the Downloads page's Steam card: installs asked, then
    /// uninstalls, verifies and what Steam is busy with, each by title.
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut asked: Vec<_> = self.asked.iter().collect();
        asked.sort_by(|a, b| a.1.title.cmp(&b.1.title));
        rows.extend(asked.into_iter().map(|(app, asked)| Row {
            app: app.clone(),
            title: asked.title.clone(),
            line: t!("steam-waiting-dialog"),
            percent: None,
            forget: true,
        }));
        let mut removing: Vec<_> = self.removing.iter().collect();
        removing.sort_by(|a, b| a.1.title.cmp(&b.1.title));
        rows.extend(removing.into_iter().map(|(app, asked)| Row {
            app: app.clone(),
            title: asked.title.clone(),
            line: t!("steam-waiting-uninstall"),
            percent: None,
            forget: false,
        }));
        let mut verifying: Vec<_> = self.verifying.iter().collect();
        verifying.sort_by(|a, b| a.1.title.cmp(&b.1.title));
        rows.extend(verifying.into_iter().map(|(app, verifying)| Row {
            app: app.clone(),
            title: verifying.title.clone(),
            line: if verifying.started {
                t!("steam-verifying")
            } else {
                t!("steam-waiting-verify")
            },
            percent: None,
            forget: false,
        }));
        let mut busy: Vec<&AppState> = self.busy().collect();
        busy.sort_by(|a, b| a.name.cmp(&b.name));
        rows.extend(busy.into_iter().map(|state| Row {
            app: state.app.clone(),
            title: state.name.clone(),
            line: progress_line(state.progress),
            percent: Some(percent(state.progress)),
            forget: false,
        }));
        rows
    }

    /// Install with Steam pressed: Steam's dialog opens, and the install is
    /// followed. Asked again while waiting, the dialog opens again (it may
    /// have been closed). Returns the line to say, or `None` when Steam is
    /// already on it. `persist` is false in tests, which ask nothing.
    pub fn install(&mut self, app: &str, title: &str, persist: bool) -> Option<String> {
        if matches!(self.following(app), Some(Following::Busy(_))) {
            return None;
        }
        if persist && let Err(err) = steam::ask(Request::Install, app) {
            return Some(format!("{err:#}"));
        }
        self.asked.insert(
            app.to_owned(),
            Asked {
                title: title.to_owned(),
                since: Instant::now(),
            },
        );
        Some(t!("steam-asked", title = title))
    }

    /// Stop waiting for Steam to start an app's install.
    pub fn forget(&mut self, app: &str) {
        self.asked.remove(app);
    }

    /// Uninstall with Steam pressed: Steam asks its own question.
    pub fn uninstall(&mut self, app: &str, title: &str, persist: bool) -> String {
        if persist && let Err(err) = steam::ask(Request::Uninstall, app) {
            return format!("{err:#}");
        }
        self.removing.insert(
            app.to_owned(),
            Asked {
                title: title.to_owned(),
                since: Instant::now(),
            },
        );
        t!("steam-asked-uninstall", title = title)
    }

    /// Verify with Steam pressed. `None` when one is under way already.
    pub fn verify(&mut self, app: &str, title: &str, persist: bool) -> Option<String> {
        if self.verifying(app) {
            return None;
        }
        // Where the log ends now: the verify's lines come after.
        let from = if persist { steam::log_end() } else { 0 };
        if persist && let Err(err) = steam::ask(Request::Verify, app) {
            return Some(format!("{err:#}"));
        }
        self.verifying.insert(
            app.to_owned(),
            Verifying {
                title: title.to_owned(),
                since: Instant::now(),
                from,
                started: false,
            },
        );
        Some(t!("steam-asked-verify", title = title))
    }

    /// A look is due: `None` while one is under way, else where the verifies
    /// to read begin (`Some(None)` when none is followed). Hand it to
    /// [`look`] on a thread, then the answer to [`Self::looked`].
    pub fn begin_look(&mut self) -> Option<Option<u64>> {
        if self.looking
            && self
                .looking_since
                .is_some_and(|at| at.elapsed() < LOOK_GIVE_UP)
        {
            return None;
        }
        self.looking = true;
        self.looking_since = Some(Instant::now());
        Some(self.verifying.values().map(|v| v.from).min())
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

    /// Steam's manifests read: the downloads under way, and the installs
    /// and uninstalls since the last look, which bring a new read of the
    /// library; and the verifies Steam's log tells of.
    pub fn looked(&mut self, states: Vec<AppState>, log: &[VerifyEvent]) -> Looked {
        self.looking = false;
        self.looking_since = None;
        // A game Steam verifies is busy a while, not newly installed.
        let checked: HashSet<String> = self.verifying.keys().cloned().collect();
        let verified = self.verifies_done(log);
        let known: HashSet<&str> = states.iter().map(|state| state.app.as_str()).collect();
        let installed: HashMap<String, String> = states
            .iter()
            .filter(|state| state.progress == Progress::Installed)
            .map(|state| (state.app.clone(), state.name.clone()))
            .collect();
        let before = self.seen.take();
        let was_installed = |app: &str| {
            before
                .as_ref()
                .and_then(|before| before.get(app))
                .is_some_and(|seen| seen.installed)
        };
        for state in &states {
            if state.progress != Progress::Installed && was_installed(&state.app) {
                self.updating.insert(state.app.clone());
            }
        }
        let mut finished = Changes::default();
        let mut updated = Changes::default();
        for (app, name) in &installed {
            let was_updating = self.updating.remove(app);
            if checked.contains(app) || was_installed(app) {
                continue;
            }
            if was_updating {
                updated.add(app, name);
            } else if before.is_some() {
                finished.add(app, name);
            }
        }
        self.asked.retain(|app, asked| {
            if installed.contains_key(app) {
                finished.add(app, &asked.title);
                return false;
            }
            // Started: followed by its manifest from now on.
            !known.contains(app.as_str()) && asked.since.elapsed() < GIVE_UP
        });
        let mut removed = Changes::default();
        for (app, seen) in before.iter().flatten() {
            // A download cancelled in Steam goes too: not an uninstall.
            let had = seen.installed || self.updating.contains(app);
            if had && !known.contains(app.as_str()) {
                removed.add(app, &seen.name);
            }
        }
        self.updating.retain(|app| known.contains(app.as_str()));
        self.removing.retain(|app, asked| {
            if known.contains(app.as_str()) {
                return asked.since.elapsed() < GIVE_UP;
            }
            removed.add(app, &asked.title);
            false
        });
        self.seen = Some(
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
        self.busy = states
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
            return Looked {
                say: verified,
                rescan: false,
            };
        }
        let note = notes
            .into_iter()
            .chain(verified)
            .collect::<Vec<_>>()
            .join(" · ");
        Looked {
            say: Some(note),
            rescan: true,
        }
    }
}

/// Reads Steam's manifests and, from where the verifies begin, its log.
/// Blocks: run it on a thread.
#[must_use]
pub fn look(from: Option<u64>) -> (Vec<AppState>, Vec<VerifyEvent>) {
    (steam::states(), from.map_or_default(steam::verifies))
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
#[must_use]
pub fn progress_line(progress: Progress) -> String {
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
#[must_use]
pub fn percent(progress: Progress) -> f32 {
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
        let mut installs = SteamInstalls::default();
        let said = installs.install("620", "Portal 2", false);
        assert_eq!(
            said.as_deref(),
            Some("Asked Steam to install Portal 2: choose where in Steam's dialog")
        );
        assert!(matches!(installs.following("620"), Some(Following::Asked)));
        // Steam's dialog cancelled: Stop waiting, then asked again.
        installs.forget("620");
        assert!(!installs.active());
        let _ = installs.install("620", "Portal 2", false);
        // Steam has started: its manifest says how far.
        let downloading = Progress::Downloading {
            done: 10,
            total: 100,
        };
        let looked = installs.looked(
            vec![state("620", downloading), state("730", Progress::Installed)],
            &[],
        );
        assert_eq!(looked, Looked::default());
        assert!(installs.asked.is_empty());
        assert!(matches!(
            installs.following("620"),
            Some(Following::Busy(found)) if found.progress == downloading
        ));
        // Asked again while busy: Steam is on it.
        assert_eq!(installs.install("620", "Portal 2", false), None);
        let looked = installs.looked(
            vec![
                state("620", Progress::Installed),
                state("730", Progress::Installed),
            ],
            &[],
        );
        assert!(!installs.active());
        assert_eq!(
            looked,
            Looked {
                say: Some("Game 620 is installed".into()),
                rescan: true,
            }
        );
    }

    #[test]
    fn notices_installs_and_uninstalls_done_in_steam() {
        let mut installs = SteamInstalls::default();
        let look =
            |installs: &mut SteamInstalls, list: Vec<AppState>| installs.looked(list, &[]).say;
        // The first look only learns what is installed.
        assert_eq!(
            look(&mut installs, vec![state("620", Progress::Installed)]),
            None
        );
        // Installed in Steam between two looks.
        assert_eq!(
            look(
                &mut installs,
                vec![
                    state("620", Progress::Installed),
                    state("730", Progress::Installed),
                ],
            )
            .as_deref(),
            Some("Game 730 is installed")
        );
        // Steam updates one: busy a while, then installed again.
        let downloading = Progress::Downloading { done: 1, total: 2 };
        let _ = look(
            &mut installs,
            vec![state("620", Progress::Installed), state("730", downloading)],
        );
        assert_eq!(
            look(
                &mut installs,
                vec![
                    state("620", Progress::Installed),
                    state("730", Progress::Installed),
                ],
            )
            .as_deref(),
            Some("Game 730 is updated")
        );
        // Uninstall asked from Zendo; Steam removes its manifest.
        let _ = installs.uninstall("620", "Portal 2", false);
        assert!(installs.removing("620"));
        assert_eq!(
            look(&mut installs, vec![state("730", Progress::Installed)]).as_deref(),
            Some("Game 620 is uninstalled")
        );
        assert!(!installs.active());
    }

    #[test]
    fn follows_a_verify_through_steams_log() {
        let mut installs = SteamInstalls::default();
        let event = |at: u64, finished: bool| VerifyEvent {
            at,
            app: "620".into(),
            finished,
        };
        let _ = installs.looked(vec![state("620", Progress::Installed)], &[]);
        let _ = installs.verify("620", "Portal 2", false);
        assert!(installs.verifying("620"));
        // Steam verifying: busy, but shown as a verify, not a download.
        let _ = installs.looked(vec![state("620", Progress::Waiting)], &[event(10, false)]);
        assert_eq!(installs.count(), 1);
        let looked = installs.looked(
            vec![state("620", Progress::Installed)],
            &[event(10, false), event(20, true)],
        );
        assert!(!installs.active());
        assert_eq!(
            looked,
            Looked {
                say: Some("Steam verified the files of Portal 2".into()),
                rescan: false,
            },
            "not read again as an install"
        );
    }

    #[test]
    fn the_card_lists_what_is_going_on_in_order() {
        let mut installs = SteamInstalls::default();
        let _ = installs.install("1", "Beta", false);
        let _ = installs.uninstall("2", "Alpha", false);
        // Steam still has the manifest of the one being uninstalled.
        let _ = installs.looked(
            vec![
                state("2", Progress::Installed),
                state(
                    "3",
                    Progress::Downloading {
                        done: 50,
                        total: 100,
                    },
                ),
            ],
            &[],
        );
        let rows = installs.rows();
        let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles, ["Beta", "Alpha", "Game 3"]);
        assert!(rows[0].forget && !rows[1].forget);
        assert_eq!(rows[2].percent, Some(50.0));
        assert_eq!(installs.count(), 3);
    }

    #[test]
    fn only_one_look_at_a_time() {
        let mut installs = SteamInstalls::default();
        assert_eq!(installs.begin_look(), Some(None));
        assert_eq!(installs.begin_look(), None);
        let _ = installs.looked(Vec::new(), &[]);
        assert_eq!(installs.begin_look(), Some(None));
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
