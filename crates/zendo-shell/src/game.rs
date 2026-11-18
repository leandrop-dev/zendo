//! What a game's page says without a toolkit: the line under its title and
//! its four tiles. Both windows use these; the cards and the rest of the
//! page stay with each window.

use zendo_core::Game;

use crate::{format, t};

/// A label and its value in the page's row of tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub label: String,
    pub value: String,
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
