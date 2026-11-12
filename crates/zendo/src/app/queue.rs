//! The up-next queue and the lists Game Mode reads: the games played lately
//! and the queued ones. The queue is the user's own, kept in `state.json`.

use iced::Task;
use zendo_core::Game;
use zendo_core::state::UP_NEXT_MAX;

use super::{Message, Page, Zendo, format};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub(super) enum QueueMessage {
    Play(String),
    Details(String),
    /// Puts a game in the up-next queue, or takes it out when it is there.
    ToggleQueued(String),
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

/// The queued games that are in the library, in the queue's order.
pub(super) fn queued_games<'a>(games: &'a [Game], queue: &[String]) -> Vec<&'a Game> {
    queue
        .iter()
        .filter_map(|id| games.iter().find(|game| &game.id == id))
        .collect()
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
    pub(super) fn update_queue(&mut self, message: QueueMessage) -> Task<Message> {
        match message {
            QueueMessage::Play(id) => match self.game(&id).cloned() {
                Some(game) => self.start_game(game),
                None => Task::none(),
            },
            QueueMessage::Details(id) => self.navigate(Page::Game(id)),
            QueueMessage::ToggleQueued(id) => {
                self.toggle_queued(&id);
                Task::none()
            }
        }
    }

    /// Queues a game, or takes it out of the queue, in the window and in the
    /// state file.
    pub(super) fn toggle_queued(&mut self, id: &str) {
        if self.up_next.iter().any(|queued| queued == id) {
            self.up_next.retain(|queued| queued != id);
            self.save(|state| state.unqueue(id));
        } else if self.up_next.len() < UP_NEXT_MAX {
            self.up_next.push(id.to_owned());
            self.save(|state| {
                state.queue_next(id);
            });
        } else {
            self.status = Some(t!("queue-full", count = UP_NEXT_MAX));
        }
    }

    pub(super) fn is_queued(&self, id: &str) -> bool {
        self.up_next.iter().any(|queued| queued == id)
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
    fn the_queue_keeps_its_order_and_skips_games_gone_from_the_library() {
        let games = [game("a"), game("b"), game("c")];
        let queue = ["c".to_owned(), "gone".to_owned(), "a".to_owned()];
        let ids: Vec<&str> = queued_games(&games, &queue)
            .iter()
            .map(|game| game.id.as_str())
            .collect();
        assert_eq!(ids, ["c", "a"]);
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
