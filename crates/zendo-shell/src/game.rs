//! What a game's page says without a toolkit: the line under its title and
//! its four tiles. Both windows use these; the cards and the rest of the
//! page stay with each window.

use zendo_core::Game;
use zendo_core::sessions::{self, Session};

use crate::{format, t};

/// A label and its value in the page's row of tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub label: String,
    pub value: String,
}

/// A label and its value in a card: a fact, or a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub label: String,
    pub value: String,
    /// The value is a folder or a file that can be opened.
    pub path: bool,
}

/// The line under the title: the store, the developer and the platform.
#[must_use]
pub fn source_line(game: &Game) -> String {
    let mut parts = vec![format::source(game.source)];
    parts.extend(game.details.developer.clone());
    parts.extend(game.details.platform.as_deref().map(platform_label));
    parts.join(" · ")
}

/// The four tiles: play time, when it was last played, achievements and
/// completion. An em dash stands where Zendo knows nothing.
#[must_use]
pub fn tiles_of(game: &Game, now: u64) -> Vec<Tile> {
    let dash = || "—".to_owned();
    let (unlocked, share) = game.achievements.map_or_else(
        || (dash(), dash()),
        |(unlocked, total)| {
            (
                format!("{unlocked} / {total}"),
                format::progress_of((unlocked, total))
                    .map_or_else(dash, |share| format!("{:.0}%", share * 100.0)),
            )
        },
    );
    let tile = |label: String, value: String| Tile { label, value };
    vec![
        tile(
            t!("fact-play-time"),
            if game.played.minutes > 0 {
                format::play_time(game.played.minutes)
            } else {
                dash()
            },
        ),
        tile(
            t!("fact-last-played"),
            game.played
                .last
                .map_or_else(dash, |last| format::ago(last, now)),
        ),
        tile(t!("achievements-title"), unlocked),
        tile(t!("stat-completion"), share),
    ]
}

/// The words about the game itself: its store, how it starts, where it is,
/// and how often Zendo started it. The store's own facts and the queue's
/// line come with their cards.
#[must_use]
pub fn details(game: &Game) -> Vec<Fact> {
    let mut rows = vec![
        fact(t!("fact-store"), format::source(game.source)),
        fact(t!("fact-started-by"), game.launcher()),
    ];
    if let Some(platform) = &game.details.platform {
        rows.push(fact(t!("fact-platform"), platform_label(platform)));
    }
    if let Some(version) = &game.details.version {
        rows.push(fact(t!("fact-version"), version.clone()));
    }
    if let Some(size) = &game.details.size {
        rows.push(fact(t!("fact-install-size"), size.to_string()));
    }
    if let Some(path) = &game.details.install_path {
        rows.push(Fact {
            label: t!("fact-install-path"),
            value: path.display().to_string(),
            path: true,
        });
    }
    rows.push(fact(
        t!("fact-started-from-zendo"),
        t!("count-times", count = game.played.launches),
    ));
    rows
}

/// A game's sessions: how many, their average and their longest, and the
/// latest ones, as date and length. Empty until it has one.
#[must_use]
pub fn sessions_of(sessions: &[Session], id: &str) -> (Vec<Fact>, Vec<Fact>) {
    let Some(own) = sessions::of_game(sessions, id) else {
        return (Vec::new(), Vec::new());
    };
    let summary = vec![
        fact(t!("stats-sessions"), own.count.to_string()),
        fact(t!("stats-average"), format::play_time(own.average)),
        fact(t!("stats-longest"), format::play_time(own.longest)),
    ];
    let recent = own
        .recent
        .iter()
        .map(|session| {
            fact(
                sessions::date_of(session),
                format::play_time(session.minutes),
            )
        })
        .collect();
    (summary, recent)
}

fn fact(label: String, value: String) -> Fact {
    Fact {
        label,
        value,
        path: false,
    }
}

/// The platform as people write it.
#[must_use]
pub fn platform_label(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "windows" => "Windows".to_owned(),
        "linux" => "Linux".to_owned(),
        "mac" | "osx" => "macOS".to_owned(),
        _ => platform.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn game() -> Game {
        Game::new(
            "desktop:x".into(),
            "X".into(),
            Source::Desktop,
            Launch::Command {
                program: "x".into(),
                args: Vec::new(),
                dir: None,
            },
        )
    }

    #[test]
    fn the_tiles_name_what_they_show() {
        let tiles = tiles_of(&game(), 0);
        assert_eq!(tiles.len(), 4);
        assert_eq!(tiles[0].label, "Play time");
        assert_eq!(tiles[0].value, "—");
        assert_eq!(tiles[3].label, "Completion");
    }

    #[test]
    fn the_source_line_is_the_store_alone_for_a_desktop_game() {
        assert_eq!(source_line(&game()), "Desktop");
    }
}
