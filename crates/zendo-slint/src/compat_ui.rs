//! What the game's page says of how it plays on Linux: the Anti-cheat card
//! (`AreWeAntiCheatYet`), Valve's Steam Deck pill, and the cloud saves' line
//! of the Details card. The first two are asked of the network on a thread,
//! once a session, after `ProtonDB`'s answer gives the Steam app id; the last
//! is read from the files the syncs leave. A part of the Library's glue
//! (`library_ui`), as `cards_ui` is.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use slint::{Color, ComponentHandle, ModelRc, VecModel};
use zendo_core::Game;
use zendo_core::areweanticheatyet::{self, Entry, Note, Status};
use zendo_core::cloud_saves::{self, State as SyncState, Status as SyncStatus};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::steam_deck::{self, Category, Kind, Note as DeckNote, Report};
use zendo_shell::running::now;
use zendo_shell::{format, t};

use super::Glue;
use super::cards::Ask;
use crate::{Fact, GameData, MainWindow, Pill};

/// The cloud saves' outcomes, which games turn them off and which are
/// syncing as they end.
#[derive(Default)]
struct Cloud {
    statuses: BTreeMap<String, SyncStatus>,
    off: BTreeSet<String>,
    syncing: HashSet<String>,
}

/// The answers, by game, and the model the anti-cheat card's notes read from.
pub(super) struct CompatState {
    /// `None`: the list does not have the game.
    anti_cheat: RefCell<HashMap<String, Ask<Option<Entry>>>>,
    /// `None`: Valve has not reviewed it.
    decks: RefCell<HashMap<String, Ask<Option<Report>>>>,
    cloud: RefCell<Cloud>,
    notes: Rc<VecModel<Fact>>,
}

impl CompatState {
    pub(super) fn new(persist: bool) -> Self {
        let state = Self {
            anti_cheat: RefCell::default(),
            decks: RefCell::default(),
            cloud: RefCell::default(),
            notes: Rc::new(VecModel::default()),
        };
        if persist {
            state.reload_cloud();
        }
        state
    }

    /// Reads the outcomes and the options again, after a sync or a save.
    fn reload_cloud(&self) {
        let mut cloud = self.cloud.borrow_mut();
        cloud.statuses = cloud_saves::statuses();
        cloud.off = LaunchOptions::load_all()
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, options)| !options.syncs_saves())
            .map(|(id, _)| id)
            .collect();
    }
}

fn status_tone(status: Status) -> i32 {
    match status {
        Status::Supported | Status::Running => 1,
        Status::Planned => 3,
        Status::Broken | Status::Denied | Status::Unknown => 2,
    }
}

fn status_label(status: Status) -> String {
    match status {
        Status::Supported => t!("anti-cheat-supported"),
        Status::Running => t!("anti-cheat-running"),
        Status::Planned => t!("anti-cheat-planned"),
        Status::Broken => t!("anti-cheat-broken"),
        Status::Denied => t!("anti-cheat-denied"),
        Status::Unknown => t!("anti-cheat-unknown"),
    }
}

/// The site's meaning of the status.
fn status_about(status: Status) -> String {
    match status {
        Status::Supported => t!("anti-cheat-supported-about"),
        Status::Running => t!("anti-cheat-running-about"),
        Status::Planned => t!("anti-cheat-planned-about"),
        Status::Broken => t!("anti-cheat-broken-about"),
        Status::Denied => t!("anti-cheat-denied-about"),
        Status::Unknown => t!("anti-cheat-unknown-about"),
    }
}

/// A line of the list's: its words, and the source it opens when it has one.
fn note_fact(line: String, note: &Note) -> Fact {
    Fact {
        label: line.into(),
        value: "".into(),
        path: false,
        mono: false,
        url: note.link.clone().unwrap_or_default().into(),
        tone: 0,
    }
}

/// The pill for Valve's review: its words and what Valve noted on hover.
fn deck_pill(report: &Report) -> Pill {
    let (label, about) = match report.category {
        Category::Verified => (t!("deck-verified"), t!("deck-verified-about")),
        Category::Playable => (t!("deck-playable"), t!("deck-playable-about")),
        Category::Unsupported => (t!("deck-unsupported"), t!("deck-unsupported-about")),
    };
    let mut tip = about;
    for note in report.notes.iter().filter(|note| note.kind != Kind::Passed) {
        tip.push_str("\n\u{2022} ");
        tip.push_str(&note_text(note));
    }
    tip.push('\n');
    tip.push_str(&t!("deck-opens-steam"));
    let (tone, tint) = match report.category {
        Category::Verified => (1, Color::default()),
        // As the ProtonDB pill's Gold.
        Category::Playable => (0, Color::from_rgb_u8(0xcf, 0xb5, 0x3b)),
        Category::Unsupported => (3, Color::default()),
    };
    Pill {
        label: label.into(),
        tint,
        tone,
        tip: tip.into(),
        url: report.url().into(),
    }
}

/// Zendo's words for one of Valve's tests.
fn note_text(note: &DeckNote) -> String {
    match note.test.as_str() {
        "DefaultControllerConfigNotFullyFunctional" => t!("deck-note-controls-partial"),
        "ControllerGlyphsDoNotMatchDeckDevice" => t!("deck-note-glyphs"),
        "DefaultConfigurationIsNotPerformant" => t!("deck-note-graphics-settings"),
        "TextInputDoesNotAutomaticallyInvokesKeyboard" => t!("deck-note-keyboard"),
        "InterfaceTextIsNotLegible" => t!("deck-note-small-text"),
        "NativeResolutionNotSupported" => t!("deck-note-resolution-unsupported"),
        "NativeResolutionNotDefault" => t!("deck-note-resolution-not-default"),
        "DisplayOutputHasNonblockingIssues" => t!("deck-note-display-issues"),
        "AudioOutputHasNonblockingIssues" => t!("deck-note-audio-issues"),
        "DeviceCompatibilityWarningsShown" => t!("deck-note-device-warnings"),
        "LauncherInteractionIssues" => t!("deck-note-launcher"),
        "VideoPlaybackHasNonblockingIssues" => t!("deck-note-video"),
        "GameOrLauncherDoesntExitCleanly" => t!("deck-note-exit"),
        "AuxFunctionalityNotAccessible_MapEditor" => t!("deck-note-map-editor"),
        "MultiWindowAppAutomaticallySetsFocus" => t!("deck-note-windows"),
        "DisplayOutputNotCorrectlyScaled" => t!("deck-note-scaling"),
        "ResumeFromSleepNotFunctional" => t!("deck-note-sleep"),
        "GamepadNotEnabledByDefault" => t!("deck-note-gamepad-off"),
        "CloudSavesNotEnabledByDefault" => t!("deck-note-cloud-off"),
        "NotFullyFunctionalWithoutExternalKeyboard" => t!("deck-note-external-keyboard"),
        "NotFullyFunctionalWithoutExternalWebcam" => t!("deck-note-webcam"),
        "NotFullyFunctionalWithoutExternalUSBGuitar" => t!("deck-note-guitar"),
        "SteamOSDoesNotSupport" => t!("deck-note-not-yet"),
        "SteamOSDoesNotSupport_VR" => t!("deck-note-vr"),
        "SteamOSDoesNotSupport_Software" => t!("deck-note-software"),
        "SteamOSDoesNotSupport_Retired" => t!("deck-note-retired"),
        "SteamOSDoesNotSupport_OperatingSystem" => t!("deck-note-operating-system"),
        "UnsupportedAntiCheat_Other" => t!("deck-note-anti-cheat-other"),
        "UnsupportedAntiCheatConfiguration" => t!("deck-note-anti-cheat"),
        "UnsupportedGraphicsPerformance" => t!("deck-note-too-heavy"),
        "FirstTimeSetupRequiresActiveInternetConnection" => t!("deck-note-setup-online"),
        "SingleplayerGameplayRequiresActiveInternetConnection" => t!("deck-note-always-online"),
        "CrossPlatformCloudSavesNotSupported" => t!("deck-note-cross-saves"),
        "ExternalControllersNotSupportedPrimaryPlayer" => t!("deck-note-external-controller"),
        "ExternalControllersNotSupportedLocalMultiplayer" => {
            t!("deck-note-external-controllers-local")
        }
        "SimultaneousInputGyroTrackpadFriendly" => t!("deck-note-gyro"),
        "HDRMustBeManuallyEnabled" => t!("deck-note-hdr"),
        other => spaced(other),
    }
}

/// A test's name as words: `ReadsLikeThis_VR` as "Reads like this VR".
fn spaced(name: &str) -> String {
    let mut words = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        if c == '_' {
            words.push(' ');
            continue;
        }
        let previous = index.checked_sub(1).map(|before| chars[before]);
        let next = chars.get(index + 1);
        let starts_word = c.is_uppercase()
            && previous.is_some_and(|p| {
                p.is_lowercase() || (p.is_uppercase() && next.is_some_and(|n| n.is_lowercase()))
            });
        if starts_word {
            words.push(' ');
        }
        words.push(c);
    }
    let mut out = String::new();
    for (index, word) in words.split_whitespace().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let acronym = word.chars().all(|c| c.is_uppercase() || c.is_ascii_digit());
        if index == 0 || acronym {
            out.push_str(word);
        } else {
            out.push_str(&word.to_lowercase());
        }
    }
    out
}

impl Glue {
    /// Hands the page the anti-cheat card's notes.
    pub(super) fn connect_compat(self: &Rc<Self>, window: &MainWindow) {
        window
            .global::<GameData>()
            .set_ac_notes(ModelRc::from(Rc::clone(&self.compat.notes)));
    }

    /// `ProtonDB` answered (or the user asks again): looks the game up on
    /// `AreWeAntiCheatYet`, and asks Valve for the review when the Steam app
    /// is known. Once a session; a failure is asked again by Try again.
    pub(super) fn want_compat(&self, id: &str, app: Option<u32>) {
        if !self.persist {
            return;
        }
        if !matches!(
            self.compat.anti_cheat.borrow().get(id),
            Some(Ask::Asking | Ask::Got(_))
        ) {
            self.compat
                .anti_cheat
                .borrow_mut()
                .insert(id.to_owned(), Ask::Asking);
            let (owned, looked) = (id.to_owned(), id.to_owned());
            self.ask(
                move || areweanticheatyet::for_game(&looked, app).map_err(|err| format!("{err:#}")),
                move |glue, result| glue.anti_cheat_arrived(owned, result),
            );
        }
        let Some(app) = app else {
            return;
        };
        if matches!(
            self.compat.decks.borrow().get(id),
            Some(Ask::Asking | Ask::Got(_))
        ) {
            return;
        }
        self.compat
            .decks
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let owned = id.to_owned();
        self.ask(
            move || steam_deck::report(app).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.deck_arrived(owned, result),
        );
    }

    /// The game has another Steam app id: what was found for the old one is
    /// forgotten, to be asked again.
    pub(super) fn forget_compat(&self, id: &str) {
        self.compat.anti_cheat.borrow_mut().remove(id);
        self.compat.decks.borrow_mut().remove(id);
    }

    /// Keeps the answer; a failure (offline with no list kept, say) keeps its
    /// reason, and the page offers to ask again.
    pub(super) fn anti_cheat_arrived(&self, id: String, result: Result<Option<Entry>, String>) {
        let state = match result {
            Ok(entry) => Ask::Got(entry),
            Err(err) => Ask::Failed(err),
        };
        self.compat.anti_cheat.borrow_mut().insert(id, state);
        self.show_game();
    }

    pub(super) fn deck_arrived(&self, id: String, result: Result<Option<Report>, String>) {
        let state = match result {
            Ok(report) => Ask::Got(report),
            Err(err) => Ask::Failed(err),
        };
        self.compat.decks.borrow_mut().insert(id, state);
        self.show_game();
    }

    /// The Steam Deck pill, once Valve's review has come.
    pub(super) fn deck_pills(&self, id: &str) -> Vec<Pill> {
        match self.compat.decks.borrow().get(id) {
            Some(Ask::Got(Some(report))) => vec![deck_pill(report)],
            _ => Vec::new(),
        }
    }

    /// The cards that could not be asked, with why.
    pub(super) fn compat_failures(&self, id: &str) -> Vec<Fact> {
        let mut failures = Vec::new();
        if let Some(Ask::Failed(error)) = self.compat.anti_cheat.borrow().get(id) {
            failures.push(plain(
                t!("anti-cheat-title"),
                t!("card-failed", error = error.as_str()),
            ));
        }
        if let Some(Ask::Failed(error)) = self.compat.decks.borrow().get(id) {
            failures.push(plain(
                t!("deck-title"),
                t!("card-failed", error = error.as_str()),
            ));
        }
        failures
    }

    /// Writes the Anti-cheat card: shown first when the game will not play
    /// online, else after About.
    pub(super) fn show_anti_cheat(&self, id: &str, data: &GameData<'_>) {
        let found = match self.compat.anti_cheat.borrow().get(id) {
            Some(Ask::Got(Some(entry))) => Some(entry.clone()),
            _ => None,
        };
        let Some(entry) = found else {
            data.set_ac_show(false);
            self.compat.notes.set_vec(Vec::new());
            return;
        };
        let mut notes = Vec::new();
        if let Some((latest, date)) = &entry.latest {
            let line = if date.is_empty() {
                t!("anti-cheat-latest", change = latest.text.as_str())
            } else {
                t!(
                    "anti-cheat-latest-dated",
                    change = latest.text.as_str(),
                    date = date.as_str()
                )
            };
            notes.push(note_fact(line, latest));
        }
        notes.extend(
            entry
                .notes
                .iter()
                .map(|note| note_fact(note.text.clone(), note)),
        );
        self.compat.notes.set_vec(notes);
        data.set_ac_show(true);
        data.set_ac_first(entry.status.warns());
        data.set_ac_label(status_label(entry.status).into());
        data.set_ac_tone(status_tone(entry.status));
        data.set_ac_makers(entry.anti_cheats.join(", ").into());
        data.set_ac_about(status_about(entry.status).into());
        data.set_ac_url(entry.url().into());
    }

    // The cloud saves.

    /// Reads the sync outcomes again, as one ends.
    pub(super) fn cloud_reload(&self) {
        if self.persist {
            self.compat.reload_cloud();
        }
    }

    /// The game's saves are syncing as it ends, or are done.
    pub(super) fn cloud_syncing(&self, id: String, syncing: bool) {
        let mut cloud = self.compat.cloud.borrow_mut();
        if syncing {
            cloud.syncing.insert(id);
        } else {
            cloud.syncing.remove(&id);
        }
    }

    /// The game's cloud saves, as its Details card says them, with the tone
    /// (0 plain, 1 good, 2 bad, 3 dim); none for a game Zendo does not sync.
    pub(super) fn cloud_row(&self, game: &Game) -> Option<Fact> {
        let (line, tone) = self.cloud_text(game)?;
        Some(Fact {
            tone,
            ..plain(t!("fact-cloud-saves"), line)
        })
    }

    fn cloud_text(&self, game: &Game) -> Option<(String, i32)> {
        if !cloud_saves::applies(game) {
            return None;
        }
        let cloud = self.compat.cloud.borrow();
        let store = format::source(game.source);
        Some(if cloud.off.contains(&game.id) {
            (t!("cloud-off"), 3)
        } else if cloud.syncing.contains(&game.id) {
            (t!("saves-syncing"), 3)
        } else {
            match cloud.statuses.get(&game.id) {
                None if cloud_saves::ready(game) => (t!("cloud-not-yet"), 3),
                None => (t!("cloud-after-first-start"), 3),
                Some(status) => {
                    let when = format::since(status.at, now());
                    match &status.state {
                        SyncState::Synced => (t!("cloud-synced", when = when), 1),
                        SyncState::NoCloudSaves => (t!("cloud-none", store = store), 3),
                        SyncState::Failed(error) => {
                            (t!("cloud-failed", when = when, error = error.as_str()), 2)
                        }
                    }
                }
            }
        })
    }
}

fn plain(label: impl Into<String>, value: impl Into<String>) -> Fact {
    Fact {
        label: label.into().into(),
        value: value.into().into(),
        path: false,
        mono: false,
        url: "".into(),
        tone: 0,
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    #[test]
    fn spells_out_a_test_zendo_does_not_know() {
        assert_eq!(spaced("ReadsLikeThis_VR"), "Reads like this VR");
        assert_eq!(spaced("NeedsUSBMouse"), "Needs USB mouse");
        assert_eq!(
            note_text(&DeckNote {
                kind: Kind::Warning,
                test: "InterfaceTextIsNotLegible".into()
            }),
            "Some text in the game is small and may be hard to read"
        );
    }

    #[test]
    fn colours_what_will_not_play_online_as_bad() {
        assert_eq!(status_tone(Status::Denied), 2);
        assert_eq!(status_tone(Status::Broken), 2);
        assert_eq!(status_tone(Status::Supported), 1);
        assert_eq!(status_label(Status::Denied), "DENIED");
    }

    #[test]
    fn the_deck_pill_names_the_notes_and_opens_steam() {
        let pill = deck_pill(&Report {
            app: 7,
            category: Category::Playable,
            notes: vec![
                DeckNote {
                    kind: Kind::Passed,
                    test: "Fine".into(),
                },
                DeckNote {
                    kind: Kind::Warning,
                    test: "Unknown_Test".into(),
                },
            ],
        });
        assert_eq!(pill.url, "https://store.steampowered.com/app/7");
        assert!(pill.tip.contains("\u{2022} Unknown test"));
        assert!(!pill.tip.contains("Fine"));
        assert_eq!(pill.tone, 0);
    }

    #[test]
    fn says_how_the_last_sync_went() {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false);
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.platform = Some("windows".into());
        let line = |glue: &Rc<Glue>, game: &Game| glue.cloud_text(game);
        let at = now() - 300;
        glue.compat.cloud.borrow_mut().statuses.insert(
            game.id.clone(),
            SyncStatus {
                at,
                state: SyncState::Failed("conflict".into()),
            },
        );
        assert_eq!(
            line(&glue, &game),
            Some(("Failed 5 min ago: conflict".to_owned(), 2))
        );
        glue.compat.cloud.borrow_mut().statuses.insert(
            game.id.clone(),
            SyncStatus {
                at,
                state: SyncState::NoCloudSaves,
            },
        );
        assert_eq!(
            line(&glue, &game).map(|(line, _)| line).as_deref(),
            Some("GOG keeps no cloud saves for this game")
        );
        glue.cloud_syncing(game.id.clone(), true);
        assert_eq!(
            line(&glue, &game).map(|(line, _)| line).as_deref(),
            Some("Syncing the saves…")
        );
        glue.compat.cloud.borrow_mut().off.insert(game.id.clone());
        assert_eq!(
            line(&glue, &game).map(|(line, _)| line).as_deref(),
            Some("Off (turn them on in Launch options)")
        );
        game.details.platform = Some("linux".into());
        assert_eq!(line(&glue, &game), None, "only the Windows games");
    }
}
