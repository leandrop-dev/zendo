//! The Steam Deck pill on a game's page: Valve's verdict, Verified,
//! Playable or Unsupported, beside the `ProtonDB` pill, with what Valve
//! noted in its tooltip and the game's Steam page when clicked. Asked once
//! a session, after `ProtonDB`'s answer gives the game's Steam app id; a
//! game Valve has not reviewed has no pill. The notes are Zendo's own
//! words for Valve's tests; a test Zendo does not know shows its name.

use iced::widget::{button, column, container, text, tooltip};
use iced::{Color, Element, Task};
use zendo_core::steam_deck::{self, Category, Kind, Note, Report};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// Valve's review of a game, as far as Zendo knows it.
#[derive(Debug)]
pub enum DeckState {
    Asking,
    /// `None`: Valve has not reviewed it.
    Got(Option<Report>),
    /// What went wrong; the pill offers to ask again.
    Failed(String),
}

impl Zendo {
    /// Asks Steam about the game, on a thread, once a session.
    pub(super) fn want_deck(&mut self, id: &str, app: u32) -> Task<Message> {
        if !self.persist
            || matches!(
                self.decks.get(id),
                Some(DeckState::Asking | DeckState::Got(_))
            )
        {
            return Task::none();
        }
        self.decks.insert(id.to_owned(), DeckState::Asking);
        let id = id.to_owned();
        Task::perform(
            blocking(move || steam_deck::report(app).map_err(|err| format!("{err:#}"))),
            move |result| Message::Deck(id.clone(), result),
        )
    }

    /// Keeps the answer; a failure (offline, say) keeps its reason, and
    /// the pill offers to ask again.
    pub(super) fn deck_arrived(
        &mut self,
        id: String,
        result: Result<Option<Report>, String>,
    ) -> Task<Message> {
        let state = match result {
            Ok(report) => DeckState::Got(report),
            Err(err) => DeckState::Failed(err),
        };
        self.decks.insert(id, state);
        Task::none()
    }

    /// The pill, once Valve's review has come.
    pub(super) fn deck_pill(&self, id: &str) -> Option<Element<'_, Message>> {
        match self.decks.get(id)? {
            DeckState::Got(Some(report)) => Some(pill(report)),
            DeckState::Failed(error) => Some(failed_pill(id, error)),
            DeckState::Got(None) | DeckState::Asking => None,
        }
    }
}

fn category_color(category: Category) -> Color {
    match category {
        Category::Verified => colors().good,
        // As the ProtonDB pill's Gold.
        Category::Playable => Color::from_rgb8(0xcf, 0xb5, 0x3b),
        Category::Unsupported => colors().dim,
    }
}

/// The pill when Valve could not be asked: a way to ask again, with the
/// reason in its tooltip.
fn failed_pill<'a>(id: &str, error: &'a str) -> Element<'a, Message> {
    tooltip(
        button(text(t!("try-again")).size(11).font(BOLD))
            .padding([2, 7])
            .style(style::pill(colors().dim))
            .on_press(Message::Retry(id.to_owned())),
        container(text(error).size(12))
            .padding([4, 8])
            .max_width(420)
            .style(style::card),
        tooltip::Position::Bottom,
    )
    .into()
}

fn pill(report: &Report) -> Element<'_, Message> {
    let (label, about) = match report.category {
        Category::Verified => (t!("deck-verified"), t!("deck-verified-about")),
        Category::Playable => (t!("deck-playable"), t!("deck-playable-about")),
        Category::Unsupported => (t!("deck-unsupported"), t!("deck-unsupported-about")),
    };
    let mut lines = column![text(about).size(12)].spacing(4);
    for note in report.notes.iter().filter(|note| note.kind != Kind::Passed) {
        lines = lines.push(text(format!("\u{2022} {}", note_text(note))).size(12));
    }
    lines = lines.push(text(t!("deck-opens-steam")).size(12).color(colors().dim));
    let link = button(text(label).size(11).font(BOLD))
        .padding([2, 7])
        .style(style::pill(category_color(report.category)))
        .on_press(Message::Open(report.url()));
    tooltip(
        link,
        container(lines)
            .padding([4, 8])
            .max_width(420)
            .style(style::card),
        tooltip::Position::Bottom,
    )
    .into()
}

/// Zendo's words for one of Valve's tests.
fn note_text(note: &Note) -> String {
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

#[cfg(test)]
mod tests {
    use zendo_core::protondb::Rating;

    use super::*;

    #[test]
    fn spells_out_a_test_zendo_does_not_know() {
        assert_eq!(spaced("ReadsLikeThis_VR"), "Reads like this VR");
        assert_eq!(spaced("NeedsUSBMouse"), "Needs USB mouse");
        assert_eq!(
            note_text(&Note {
                kind: Kind::Warning,
                test: "InterfaceTextIsNotLegible".into()
            }),
            "Some text in the game is small and may be hard to read"
        );
    }

    #[test]
    fn shows_a_pill_only_for_a_game_valve_reviewed() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        assert!(zendo.deck_pill("steam:1").is_none(), "not asked yet");
        let _ = zendo.update(Message::Rating(
            "steam:1".into(),
            Ok(Some(Rating {
                app: 1,
                tier: None,
                reports: 0,
                native: false,
            })),
        ));
        assert!(!zendo.decks.contains_key("steam:1"), "nothing may be kept");
        let _ = zendo.update(Message::Deck(
            "steam:1".into(),
            Ok(Some(Report {
                app: 1,
                category: Category::Playable,
                notes: vec![Note {
                    kind: Kind::Warning,
                    test: "Unknown_Test".into(),
                }],
            })),
        ));
        assert!(zendo.deck_pill("steam:1").is_some());
        let _ = zendo.update(Message::Deck("steam:2".into(), Ok(None)));
        assert!(zendo.deck_pill("steam:2").is_none());
        let _ = zendo.update(Message::Deck("steam:1".into(), Err("offline".into())));
        assert!(
            matches!(
                zendo.decks.get("steam:1"),
                Some(DeckState::Failed(error)) if error == "offline"
            ),
            "the reason stays, with a way to ask again"
        );
        assert!(zendo.deck_pill("steam:1").is_some());
    }
}
