//! GOG's total played time on a GOG game's page, as Steam's games show
//! Steam's: asked once a session as the page opens, and again after Zendo
//! sent GOG a session of the game. The larger of it and the count Zendo
//! had is shown, on the page and the library's card alike.

use iced::Task;
use zendo_core::gog_playtime;

use super::{Message, Zendo, blocking};

impl Zendo {
    /// Asks GOG for the game's total, on a thread; nothing for a game
    /// not from GOG, or one asked already.
    pub(super) fn want_gog_total(&mut self, id: &str) -> Task<Message> {
        if !self.persist || !id.starts_with("gog:") || !self.gog_totals.insert(id.to_owned()) {
            return Task::none();
        }
        let id = id.to_owned();
        Task::perform(
            {
                let id = id.clone();
                blocking(move || gog_playtime::total(&id).map_err(|err| format!("{err:#}")))
            },
            move |result| Message::GogTotal(id.clone(), result),
        )
    }

    /// Asks again, as a session was just sent.
    pub(super) fn refresh_gog_total(&mut self, id: &str) -> Task<Message> {
        self.gog_totals.remove(id);
        self.want_gog_total(id)
    }

    /// Shows the total where it is more than the count known; a failure
    /// (offline, say) is forgotten, so the page asks again next time.
    pub(super) fn gog_total_arrived(
        &mut self,
        id: &str,
        result: &Result<Option<u64>, String>,
    ) -> Task<Message> {
        match *result {
            Ok(Some(minutes)) => {
                if let Some(game) = self.game_mut(id) {
                    game.played.minutes = game.played.minutes.max(minutes);
                }
                self.store.raise_minutes(id, minutes);
            }
            Ok(None) => {}
            Err(_) => {
                self.gog_totals.remove(id);
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Game, Launch, Source};

    use super::*;

    #[test]
    fn gogs_total_shows_where_it_is_more() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.played.minutes = 90;
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        let minutes = |zendo: &Zendo| zendo.game("gog:1").map(|game| game.played.minutes);
        let _ = zendo.update(Message::GogTotal("gog:1".into(), Ok(Some(60))));
        assert_eq!(minutes(&zendo), Some(90), "Zendo's own count is more");
        let _ = zendo.update(Message::GogTotal("gog:1".into(), Ok(Some(300))));
        assert_eq!(minutes(&zendo), Some(300));
        zendo.gog_totals.insert("gog:1".into());
        let _ = zendo.update(Message::GogTotal("gog:1".into(), Err("offline".into())));
        assert!(!zendo.gog_totals.contains("gog:1"), "asked again next time");
    }
}
