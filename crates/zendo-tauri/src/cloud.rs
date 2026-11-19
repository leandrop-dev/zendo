//! The cloud saves row on a game's page: whether Zendo syncs the game's
//! saves, how the last sync went and when, or why it waits. Whether one is
//! running now is kept here, set by the launch's effects.

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use zendo_core::Game;
use zendo_core::cloud_saves::{self, State as SyncState};
use zendo_core::launch_options::LaunchOptions;
use zendo_shell::running::now;
use zendo_shell::{format, t};

/// The games whose saves are syncing right now.
#[derive(Debug, Default)]
pub struct CloudState {
    syncing: Mutex<HashSet<String>>,
}

impl CloudState {
    /// The game's saves began or stopped syncing.
    pub(crate) fn set_syncing(&self, id: &str, syncing: bool) {
        let mut all = self.syncing.lock().unwrap_or_else(PoisonError::into_inner);
        if syncing {
            all.insert(id.to_owned());
        } else {
            all.remove(id);
        }
    }

    fn is_syncing(&self, id: &str) -> bool {
        self.syncing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(id)
    }
}

/// The row's line and its tone (0 plain, 1 good, 2 bad, 3 dim); `None` for a
/// game whose saves Zendo does not sync.
pub(crate) fn row(game: &Game, cloud: &CloudState) -> Option<(String, i32)> {
    if !cloud_saves::applies(game) {
        return None;
    }
    let id = game.id.as_str();
    let off = LaunchOptions::load(id).is_ok_and(|options| !options.syncs_saves());
    if off {
        return Some((t!("cloud-off"), 3));
    }
    if cloud.is_syncing(id) {
        return Some((t!("saves-syncing"), 3));
    }
    let statuses = cloud_saves::statuses();
    let Some(status) = statuses.get(id) else {
        return Some(if cloud_saves::ready(game) {
            (t!("cloud-not-yet"), 3)
        } else {
            (t!("cloud-after-first-start"), 3)
        });
    };
    let when = format::since(status.at, now());
    Some(match &status.state {
        SyncState::Synced => (t!("cloud-synced", when = when), 1),
        SyncState::NoCloudSaves => (t!("cloud-none", store = format::source(game.source)), 3),
        SyncState::Failed(error) => (t!("cloud-failed", when = when, error = error.as_str()), 2),
    })
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn game(source: Source, launch: Launch) -> Game {
        Game::new("epic:Firewatch".into(), "Firewatch".into(), source, launch)
    }

    #[test]
    fn only_a_store_that_keeps_cloud_saves_has_a_row() {
        let steam = game(
            Source::Steam,
            Launch::Uri {
                uri: "steam://rungameid/1".into(),
            },
        );
        assert!(row(&steam, &CloudState::default()).is_none());
    }

    #[test]
    fn a_sync_that_runs_says_so() {
        let epic = game(
            Source::Epic,
            Launch::Engine {
                runner: "legendary".into(),
                app: "Firewatch".into(),
            },
        );
        let cloud = CloudState::default();
        cloud.set_syncing("epic:Firewatch", true);
        let (line, tone) = row(&epic, &cloud).expect("a row");
        assert_eq!((line, tone), (t!("saves-syncing"), 3));
        cloud.set_syncing("epic:Firewatch", false);
        assert!(!cloud.is_syncing("epic:Firewatch"));
    }
}
