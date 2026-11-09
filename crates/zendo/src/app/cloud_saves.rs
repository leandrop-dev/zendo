//! The cloud saves' line on a game's page: whether they are in sync, when
//! they last were, or what went wrong, for the Windows games installed from
//! Epic and GOG. The syncs themselves run in `zendo_core::cloud_saves`,
//! before and after the game and from its launch options sheet; each keeps
//! its outcome, read here again as one ends.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use iced::widget::text;
use iced::{Color, Element};
use zendo_core::Game;
use zendo_core::cloud_saves::{self, State, Status};
use zendo_core::launch_options::LaunchOptions;

use super::style::colors;
use super::{Message, Zendo, format, now};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct CloudState {
    /// Each game's last sync, by id.
    statuses: BTreeMap<String, Status>,
    /// The games whose launch options turn the sync off.
    off: BTreeSet<String>,
    /// The games whose saves are syncing as they end.
    pub syncing: HashSet<String>,
}

impl CloudState {
    pub fn load() -> Self {
        let mut state = Self::default();
        state.reload();
        state
    }

    /// Reads the outcomes and the options again, after a sync or a save.
    pub fn reload(&mut self) {
        self.statuses = cloud_saves::statuses();
        self.off = LaunchOptions::load_all()
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, options)| !options.syncs_saves())
            .map(|(id, _)| id)
            .collect();
    }
}

impl Zendo {
    /// The game's cloud saves, as its Details card says them; none for a
    /// game Zendo does not sync.
    pub(super) fn cloud_line(&self, game: &Game) -> Option<Element<'_, Message>> {
        let (line, color) = self.cloud_text(game)?;
        Some(text(line).size(14).color(color).into())
    }

    fn cloud_text(&self, game: &Game) -> Option<(String, Color)> {
        if !cloud_saves::applies(game) {
            return None;
        }
        let cloud = &self.cloud;
        let store = format::source(game.source);
        let (line, color) = if cloud.off.contains(&game.id) {
            (t!("cloud-off"), colors().dim)
        } else if cloud.syncing.contains(&game.id) {
            (t!("saves-syncing"), colors().dim)
        } else {
            match cloud.statuses.get(&game.id) {
                None if cloud_saves::ready(game) => (t!("cloud-not-yet"), colors().dim),
                None => (t!("cloud-after-first-start"), colors().dim),
                Some(status) => {
                    let when = format::since(status.at, now());
                    match &status.state {
                        State::Synced => (t!("cloud-synced", when = when), colors().good),
                        State::NoCloudSaves => (t!("cloud-none", store = store), colors().dim),
                        State::Failed(error) => (
                            t!("cloud-failed", when = when, error = error.as_str()),
                            colors().bad,
                        ),
                    }
                }
            }
        };
        Some((line, color))
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    #[test]
    fn says_how_the_last_sync_went() {
        let (mut zendo, _) = Zendo::boot();
        zendo.cloud = CloudState::default();
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
        let line = |zendo: &Zendo, game: &Game| zendo.cloud_text(game).map(|(line, _)| line);
        let at = now() - 300;
        zendo.cloud.statuses.insert(
            game.id.clone(),
            Status {
                at,
                state: State::Failed("conflict".into()),
            },
        );
        assert_eq!(
            line(&zendo, &game).as_deref(),
            Some("Failed 5 min ago: conflict")
        );
        zendo.cloud.statuses.insert(
            game.id.clone(),
            Status {
                at,
                state: State::NoCloudSaves,
            },
        );
        assert_eq!(
            line(&zendo, &game).as_deref(),
            Some("GOG keeps no cloud saves for this game")
        );
        zendo.cloud.syncing.insert(game.id.clone());
        assert_eq!(line(&zendo, &game).as_deref(), Some("Syncing the saves…"));
        zendo.cloud.off.insert(game.id.clone());
        assert_eq!(
            line(&zendo, &game).as_deref(),
            Some("Off (turn them on in Launch options)")
        );
        game.details.platform = Some("linux".into());
        assert_eq!(line(&zendo, &game), None, "only the Windows games");
    }
}
