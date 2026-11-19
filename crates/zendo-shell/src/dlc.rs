//! The DLC card of a game's page: what the store says the player owns, what
//! is installed with the game, and what each row's button says. Both windows
//! say it the same.

use std::collections::HashSet;

use zendo_core::Game;
use zendo_core::Size;
use zendo_core::info::Dlc;
use zendo_core::stores::Store;

use crate::store_facts::InfoState;
use crate::t;

/// One DLC: its name, what its line says under it, its button or the words
/// that stand in for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlcRow {
    pub id: String,
    pub title: String,
    /// Its size, or that it comes with the game.
    pub about: String,
    /// The button's words; empty where there is no button.
    pub label: String,
    /// Uninstall pressed once: pressed again, it goes.
    pub danger: bool,
    pub enabled: bool,
    /// "Installed" or "Not installed", where there is no button.
    pub status: String,
    /// 1 good, 3 dim.
    pub status_tone: i32,
}

/// The card: the count, the rows, Install all and the note under them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DlcView {
    pub count: String,
    pub rows: Vec<DlcRow>,
    pub all: String,
    pub all_on: bool,
    pub note: String,
}

/// What the card is drawn from.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the card is drawn by"
)]
#[derive(Clone, Copy)]
pub struct Ask<'a> {
    pub game: &'a Game,
    /// What the store said of the game, or why it could not.
    pub info: Option<&'a InfoState>,
    /// The store is signed in, so a missing ask is a sign-in away.
    pub signed_in: bool,
    /// Zendo installs and removes this game's DLC: a game it installed, with
    /// its own page open.
    pub manage: bool,
    /// The card is on a page that cannot manage them (a store game's).
    pub unmanaged: bool,
    /// The DLC whose Uninstall was pressed once.
    pub confirm: Option<&'a str>,
    /// The game has no job in the queue.
    pub free: bool,
    /// The game is in the library, so the count is worth saying.
    pub listed: bool,
}

/// A DLC as the card lists it.
struct Line {
    id: String,
    title: String,
    installed: bool,
    installable: bool,
    size: Option<u64>,
}

/// What one DLC's button says: its words, whether it is the danger colour,
/// the words that stand in for a button, and their tone.
fn button(installable: bool, installed: bool, manage: Option<bool>) -> (String, bool, String, i32) {
    let none = String::new;
    match (installable, installed, manage) {
        (false, ..) => (none(), false, none(), 0),
        (true, true, Some(true)) => (t!("downloads-press-again"), true, none(), 0),
        (true, true, Some(false)) => (t!("action-label", action = "uninstall"), false, none(), 0),
        (true, false, Some(_)) => (t!("install"), false, none(), 0),
        (true, true, None) => (none(), false, t!("installed"), 1),
        (true, false, None) => (none(), false, t!("dlc-not-installed"), 3),
    }
}

/// The card as these facts make it.
#[must_use]
pub fn view(ask: &Ask<'_>) -> DlcView {
    let game = ask.game;
    let owned: Vec<Dlc> = match ask.info {
        Some(InfoState::Got(info)) => info.dlc.clone(),
        _ => Vec::new(),
    };
    let installed: HashSet<&str> = game.details.dlc.iter().map(|dlc| dlc.id.as_str()).collect();
    let mut lines: Vec<Line> = owned
        .iter()
        .map(|dlc| Line {
            id: dlc.id.clone(),
            title: dlc.title.clone(),
            installed: installed.contains(dlc.id.as_str()),
            installable: dlc.installable,
            size: dlc.install_size,
        })
        .collect();
    // Installed, but not in the store's list (or the list is not here).
    for dlc in &game.details.dlc {
        if !owned.iter().any(|item| item.id == dlc.id) {
            lines.push(Line {
                id: dlc.id.clone(),
                title: dlc.title.clone().unwrap_or_else(|| dlc.id.clone()),
                installed: true,
                installable: true,
                size: None,
            });
        }
    }
    let count = if ask.listed {
        let total = lines.iter().filter(|line| line.installable).count();
        let have = lines
            .iter()
            .filter(|line| line.installable && line.installed)
            .count();
        t!("dlc-count", installed = have, total = total)
    } else {
        String::new()
    };
    let to_install = lines
        .iter()
        .filter(|line| line.installable && !line.installed)
        .count();
    let rows: Vec<DlcRow> = lines
        .into_iter()
        .map(|line| {
            let about = if line.installable {
                line.size
                    .map_or_else(String::new, |bytes| Size::Bytes(bytes).to_string())
            } else {
                t!("dlc-comes-with")
            };
            let asked = ask.confirm == Some(line.id.as_str());
            let (label, danger, status, status_tone) = button(
                line.installable,
                line.installed,
                ask.manage.then_some(asked),
            );
            DlcRow {
                id: line.id,
                title: line.title,
                about,
                label,
                danger,
                enabled: ask.free,
                status,
                status_tone,
            }
        })
        .collect();
    DlcView {
        count,
        rows,
        all: if ask.manage && to_install > 1 {
            t!("dlc-install-all")
        } else {
            String::new()
        },
        all_on: ask.free,
        note: note(ask.unmanaged, ask.info, ask.signed_in, game),
    }
}

/// Why Zendo cannot manage the DLC, on a page where it cannot: sign in to see
/// all of them, or Heroic does; and that the store is being asked.
fn note(unmanaged: bool, info: Option<&InfoState>, signed_in: bool, game: &Game) -> String {
    let store = match game.source {
        zendo_core::Source::Gog => Store::Gog,
        _ => Store::Epic,
    };
    let mut notes = Vec::new();
    if unmanaged {
        notes.push(if info.is_none() && !signed_in {
            t!("dlc-sign-in", store = store.label())
        } else {
            t!("dlc-in-heroic")
        });
    }
    if matches!(info, Some(InfoState::Asking)) {
        notes.push(t!("dlc-asking"));
    }
    notes.join("\n")
}

#[cfg(test)]
mod tests {
    use zendo_core::info::GameInfo;
    use zendo_core::{InstalledDlc, Launch, Source};

    use super::*;

    fn game() -> Game {
        let mut game = Game::new(
            "epic:Sugar".into(),
            "Sugar".into(),
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "Sugar".into(),
            },
        );
        game.details.install_path = Some("/home/me/Games/Sugar".into());
        game.details.dlc = vec![InstalledDlc {
            id: "d1".into(),
            title: Some("Skins".into()),
        }];
        game
    }

    fn info() -> InfoState {
        let dlc = |id: &str, installable: bool| Dlc {
            id: id.into(),
            title: format!("DLC {id}"),
            installable,
            download_size: None,
            install_size: Some(1_000_000),
        };
        InfoState::Got(GameInfo {
            dlc: vec![
                dlc("d1", true),
                dlc("d2", true),
                dlc("d3", true),
                dlc("d4", false),
            ],
            ..GameInfo::default()
        })
    }

    fn ask<'a>(game: &'a Game, info: &'a InfoState, manage: bool) -> Ask<'a> {
        Ask {
            game,
            info: Some(info),
            signed_in: true,
            manage,
            unmanaged: false,
            confirm: None,
            free: true,
            listed: true,
        }
    }

    #[test]
    fn a_managed_game_installs_and_uninstalls_its_dlc() {
        let game = game();
        let info = info();
        let card = view(&ask(&game, &info, true));
        assert_eq!(card.count, t!("dlc-count", installed = 1, total = 3));
        assert_eq!(card.note, "");
        let labels: Vec<&str> = card.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                t!("action-label", action = "uninstall"),
                t!("install"),
                t!("install"),
                String::new(),
            ]
        );
        assert_eq!(card.rows[3].about, t!("dlc-comes-with"));
        assert_eq!(card.rows[3].status, "", "the button says it instead");
        assert_eq!(card.all, t!("dlc-install-all"));
        assert!(card.rows.iter().all(|row| row.enabled) && card.all_on);
    }

    #[test]
    fn an_uninstall_asks_once_more_and_a_job_turns_the_card_off() {
        let game = game();
        let info = info();
        let mut asked = ask(&game, &info, true);
        asked.confirm = Some("d1");
        let card = view(&asked);
        assert!(card.rows[0].danger);
        assert_eq!(card.rows[0].label, t!("downloads-press-again"));

        let mut busy = ask(&game, &info, true);
        busy.free = false;
        let card = view(&busy);
        assert!(card.rows.iter().all(|row| !row.enabled));
        assert!(!card.all_on);
    }

    #[test]
    fn a_page_that_cannot_manage_them_says_why() {
        let game = game();
        let info = info();
        let mut unmanaged = ask(&game, &info, false);
        unmanaged.unmanaged = true;
        let card = view(&unmanaged);
        assert_eq!(card.all, "", "no Install all there");
        assert_eq!(card.note, t!("dlc-in-heroic"));
        assert_eq!(card.rows[0].status, t!("installed"));
        assert_eq!(card.rows[0].status_tone, 1);
        assert_eq!(card.rows[1].status, t!("dlc-not-installed"));

        let asking = InfoState::Asking;
        let mut still_asking = ask(&game, &asking, false);
        still_asking.unmanaged = true;
        let note = view(&still_asking).note;
        assert!(note.contains(&t!("dlc-in-heroic")));
        assert!(note.contains(&t!("dlc-asking")));
    }

    #[test]
    fn a_disconnected_store_offers_the_sign_in() {
        let game = game();
        let away = Ask {
            game: &game,
            info: None,
            signed_in: false,
            manage: false,
            unmanaged: true,
            confirm: None,
            free: true,
            listed: true,
        };
        assert_eq!(
            view(&away).note,
            t!("dlc-sign-in", store = Store::Epic.label())
        );
        let here = Ask {
            game: &game,
            info: None,
            signed_in: true,
            manage: false,
            unmanaged: true,
            confirm: None,
            free: true,
            listed: true,
        };
        assert_eq!(view(&here).note, t!("dlc-in-heroic"));
    }
}
