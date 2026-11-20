//! How a game plays on Linux, in the window's words: the status the
//! anti-cheat list gives it, and Valve's Steam Deck review with its notes.
//! The reading itself is `zendo_core::areweanticheatyet`'s and
//! `zendo_core::steam_deck`'s; this is what the page says of it.

use zendo_core::areweanticheatyet::{Entry, Status};
use zendo_core::steam_deck::{Category, Kind, Note, Report};

use crate::t;

/// The list's name for a status, what it means, and the tone it is drawn in:
/// 1 good, 2 bad, 3 dim.
#[must_use]
pub fn status_words(status: Status) -> (String, String, i32) {
    match status {
        Status::Supported => (
            t!("anti-cheat-supported"),
            t!("anti-cheat-supported-about"),
            1,
        ),
        Status::Running => (t!("anti-cheat-running"), t!("anti-cheat-running-about"), 1),
        Status::Planned => (t!("anti-cheat-planned"), t!("anti-cheat-planned-about"), 3),
        Status::Broken => (t!("anti-cheat-broken"), t!("anti-cheat-broken-about"), 2),
        Status::Denied => (t!("anti-cheat-denied"), t!("anti-cheat-denied-about"), 2),
        Status::Unknown => (t!("anti-cheat-unknown"), t!("anti-cheat-unknown-about"), 2),
    }
}

/// Valve's verdict, and what it means.
#[must_use]
pub fn deck_words(category: Category) -> (String, String) {
    match category {
        Category::Verified => (t!("deck-verified"), t!("deck-verified-about")),
        Category::Playable => (t!("deck-playable"), t!("deck-playable-about")),
        Category::Unsupported => (t!("deck-unsupported"), t!("deck-unsupported-about")),
    }
}

/// The newest change to a game's status, with its date when the list has one.
#[must_use]
pub fn latest_line(entry: &Entry) -> Option<String> {
    let (latest, date) = entry.latest.as_ref()?;
    Some(if date.is_empty() {
        t!("anti-cheat-latest", change = latest.text.as_str())
    } else {
        t!(
            "anti-cheat-latest-dated",
            change = latest.text.as_str(),
            date = date.as_str()
        )
    })
}

/// Valve's review as the pill's words, with the tests that did not pass under
/// it and the way to the store's page.
#[must_use]
pub fn deck_tip(report: &Report) -> String {
    let mut tip = deck_words(report.category).1;
    for note in report.notes.iter().filter(|note| note.kind != Kind::Passed) {
        tip.push_str("\n\u{2022} ");
        tip.push_str(&note_text(note));
    }
    tip.push('\n');
    tip.push_str(&t!("deck-opens-steam"));
    tip
}

/// Zendo's words for one of Valve's tests.
#[must_use]
pub fn note_text(note: &Note) -> String {
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

/// A test's name as words, for one Zendo has no words for: `ReadsLikeThis_VR`
/// as "Reads like this VR".
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
    use super::*;

    #[test]
    fn the_list_s_words_come_with_a_tone() {
        let (label, about, tone) = status_words(Status::Broken);
        assert_eq!(label, "BROKEN");
        assert!(about.contains("Online play will not work on Linux"));
        assert_eq!(tone, 2);
        assert_eq!(status_words(Status::Supported).2, 1);
        assert_eq!(status_words(Status::Planned).2, 3);
    }

    #[test]
    fn valve_s_notes_are_named_and_unknown_ones_are_spaced_out() {
        let known = Note {
            kind: Kind::Warning,
            test: "InterfaceTextIsNotLegible".into(),
        };
        assert_eq!(
            note_text(&known),
            "Some text in the game is small and may be hard to read"
        );
        let unknown = Note {
            kind: Kind::Info,
            test: "HDRMustBeManuallyEnabled".into(),
        };
        assert_eq!(
            note_text(&unknown),
            "HDR works, but has to be switched on in the game's settings"
        );
        let other = Note {
            kind: Kind::Info,
            test: "ReadsLikeThis_VR".into(),
        };
        assert_eq!(note_text(&other), "Reads like this VR");
    }

    #[test]
    fn the_pill_names_the_verdict_and_the_tests_that_did_not_pass() {
        let report = Report {
            app: 620,
            category: Category::Playable,
            notes: vec![
                Note {
                    kind: Kind::Passed,
                    test: "Whatever".into(),
                },
                Note {
                    kind: Kind::Warning,
                    test: "GamepadNotEnabledByDefault".into(),
                },
            ],
        };
        let tip = deck_tip(&report);
        assert!(tip.starts_with("Valve checked it on the Steam Deck"));
        assert!(
            tip.contains(
                "\u{2022} Controller support has to be switched on in the game's settings"
            )
        );
        assert!(
            !tip.contains("Whatever"),
            "the tests that passed are not named"
        );
        assert!(tip.ends_with("Opens the game's Steam page"));
        assert_eq!(deck_words(Category::Verified).0, "DECK VERIFIED");
    }

    #[test]
    fn the_latest_change_reads_with_its_date_or_without() {
        let note = |date: &str| Entry {
            name: "Portal".into(),
            status: Status::Broken,
            anti_cheats: vec!["BattlEye".into()],
            notes: Vec::new(),
            latest: Some((
                zendo_core::areweanticheatyet::Note {
                    text: "Anti-cheat enabled".into(),
                    link: None,
                },
                date.to_owned(),
            )),
            slug: "portal".into(),
        };
        assert_eq!(
            latest_line(&note("2024-05-01")),
            Some("Latest: Anti-cheat enabled · 2024-05-01".to_owned())
        );
        assert_eq!(
            latest_line(&note("")),
            Some("Latest: Anti-cheat enabled".to_owned())
        );
        let mut without = note("");
        without.latest = None;
        assert_eq!(latest_line(&without), None);
    }
}
