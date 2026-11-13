//! The lists Game Mode reads (the games played lately), the line over a
//! hero's title, and what the hero's Play and Details buttons do.

use iced::Task;
use zendo_core::Game;

use super::{Message, Page, Zendo, format};

#[derive(Debug, Clone)]
pub(super) enum HeroMessage {
    Play(String),
    Details(String),
}

/// The games played, the latest first, hidden ones left out.
pub(super) fn played_games(games: &[Game]) -> Vec<&Game> {
    let mut played: Vec<&Game> = games
        .iter()
        .filter(|game| !game.hidden && game.played.last.is_some())
        .collect();
    played.sort_by_key(|game| std::cmp::Reverse(game.played.last));
    played
}

/// "STEAM · 12 h · today": the line over a game's title.
pub(super) fn meta_line(game: &Game, now: u64) -> String {
    let mut parts = vec![format::source(game.source)];
    if game.played.minutes > 0 {
        parts.push(format::play_time(game.played.minutes));
    }
    if let Some(last) = game.played.last {
        parts.push(format::ago(last, now));
    }
    parts.join(" · ").to_uppercase()
}

impl Zendo {
    pub(super) fn update_hero(&mut self, message: HeroMessage) -> Task<Message> {
        match message {
            HeroMessage::Play(id) => match self.game(&id).cloned() {
                Some(game) => self.start_game(game),
                None => Task::none(),
            },
            HeroMessage::Details(id) => self.navigate(Page::Game(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::{Launch, Source};

    fn game(id: &str) -> Game {
        Game::new(
            id.into(),
            id.to_uppercase(),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        )
    }

    #[test]
    fn played_games_come_latest_first_and_leave_hidden_ones_out() {
        let mut older = game("a");
        let mut newest = game("b");
        let mut hidden = game("c");
        let never = game("d");
        older.played.last = Some(100);
        newest.played.last = Some(300);
        hidden.played.last = Some(200);
        hidden.hidden = true;
        let games = [older, newest, hidden, never];
        let ids: Vec<&str> = played_games(&games)
            .iter()
            .map(|game| game.id.as_str())
            .collect();
        assert_eq!(ids, ["b", "a"]);
    }

    #[test]
    fn the_meta_line_names_the_store_time_and_when() {
        let mut game = game("a");
        assert_eq!(
            meta_line(&game, 0),
            format::source(Source::Desktop).to_uppercase()
        );
        game.played.minutes = 90;
        game.played.last = Some(0);
        let line = meta_line(&game, 0);
        assert!(
            line.contains(&format::play_time(90).to_uppercase()),
            "{line}"
        );
        assert!(line.contains(&format::ago(0, 0).to_uppercase()), "{line}");
    }
}
