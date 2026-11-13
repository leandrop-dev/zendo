//! What Steam's store says about a game, on any game's page and in Game
//! Mode's hero: its description, genres and features, the Metacritic score
//! and screenshots, for any game a Steam app id is known for. The id is the
//! one the user gave the game (the Technical card's field), a Steam game's
//! own, or the one `ProtonDB`'s lookup found; with none, the page says so
//! and offers the field. Asked once a session, from the cache when a week
//! old at most.

use iced::widget::{button, column, image, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::owned::Owned;
use zendo_core::steam_facts::{self, Facts};

use super::game::{GameViewMessage, card, label_row};
use super::protondb::RatingState;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// How many of Steam's features the details card names before "+N more".
const FEATURES_SHOWN: usize = 6;
/// How tall a screenshot's small image is drawn.
const SHOT_HEIGHT: f32 = 150.0;

/// A game's facts, as far as Zendo knows them.
#[derive(Debug, Clone)]
pub enum FactsState {
    Asking,
    /// `None`: Steam's store has no page for the app.
    Got(Option<Box<Facts>>),
    /// What went wrong; asked again the next time the page opens.
    Failed(String),
}

impl Zendo {
    /// Asks Steam's store about the app, on a thread, once a session; a
    /// failed ask is asked again.
    pub(super) fn want_facts(&mut self, id: &str, app: u32) -> Task<Message> {
        if !self.persist
            || matches!(
                self.facts.get(id),
                Some(FactsState::Asking | FactsState::Got(_))
            )
        {
            return Task::none();
        }
        self.facts.insert(id.to_owned(), FactsState::Asking);
        let id = id.to_owned();
        Task::perform(
            blocking(move || steam_facts::for_app(app).map_err(|err| format!("{err:#}"))),
            move |result| Message::Facts(id.clone(), result),
        )
    }

    /// Asks for the game's facts when its Steam app id is known without
    /// asking the network: the user's, a Steam game's own, or the one a
    /// `ProtonDB` lookup found earlier.
    pub(super) fn want_known_facts(&mut self, id: &str) -> Task<Message> {
        if !self.persist {
            return Task::none();
        }
        let chosen = self.game(id).and_then(|game| game.steam_app);
        match steam_facts::app_for(id, chosen) {
            Some(app) => self.want_facts(id, app),
            None => Task::none(),
        }
    }

    /// Game Mode's hero: asks for the facts of the game that just took it,
    /// once for each game that does. A game whose Steam app id is not known
    /// yet gets them after its page is opened once.
    pub(super) fn want_hero_facts(&mut self) -> Task<Message> {
        let Some(id) = self.hero_game().map(|game| game.id.clone()) else {
            return Task::none();
        };
        let Some(mode) = &mut self.game_mode else {
            return Task::none();
        };
        if mode.facts_for.as_deref() == Some(id.as_str()) {
            return Task::none();
        }
        mode.facts_for = Some(id.clone());
        self.want_known_facts(&id)
    }

    /// Keeps the answer; a failure (offline, say) keeps its reason.
    pub(super) fn facts_arrived(
        &mut self,
        id: String,
        result: Result<Option<Facts>, String>,
    ) -> Task<Message> {
        let state = match result {
            Ok(found) => FactsState::Got(found.map(Box::new)),
            Err(err) => FactsState::Failed(err),
        };
        self.facts.insert(id, state);
        Task::none()
    }

    /// The game's facts, once they have come and the store has a page.
    pub(super) fn facts_of(&self, id: &str) -> Option<&Facts> {
        match self.facts.get(id)? {
            FactsState::Got(Some(facts)) => Some(&**facts),
            _ => None,
        }
    }

    /// The description to show: the game's own store's, else Steam's.
    pub(super) fn description_of<'a>(
        &'a self,
        id: &str,
        owned: Option<&'a Owned>,
    ) -> Option<&'a str> {
        owned
            .and_then(|owned| owned.summary.as_deref())
            .or_else(|| {
                self.facts_of(id)
                    .and_then(|facts| facts.details.summary.as_deref())
            })
    }

    /// Rows for the details card from Steam's store: what the game's own
    /// store did not say (developer, publisher, release date, genres), then
    /// the features and the Metacritic score, which only Steam gives.
    pub(super) fn steam_fact_rows<'a>(
        &'a self,
        game: &Game,
        owned: Option<&Owned>,
    ) -> Vec<(String, Element<'a, Message>)> {
        if let Some(FactsState::Failed(error)) = self.facts.get(&game.id) {
            return vec![(
                t!("fact-store-details"),
                text(t!("fact-store-failed", error = error.as_str()))
                    .size(14)
                    .into(),
            )];
        }
        let Some(facts) = self.facts_of(&game.id) else {
            return Vec::new();
        };
        let details = &facts.details;
        let said = |own: Option<&Option<String>>| own.is_some_and(Option::is_some);
        let mut rows: Vec<(String, Element<'a, Message>)> = Vec::new();
        let mut add = |label: String, value: String| rows.push((label, text(value).size(14).into()));
        if let Some(developer) = &details.developer
            && game.details.developer.is_none()
            && !said(owned.map(|owned| &owned.developer))
        {
            add(t!("fact-developer"), developer.clone());
        }
        if let Some(publisher) = &details.publisher
            && !said(owned.map(|owned| &owned.publisher))
        {
            add(t!("fact-publisher"), publisher.clone());
        }
        if let Some(day) = &details.released
            && !said(owned.map(|owned| &owned.released))
        {
            add(t!("fact-released"), day.clone());
        }
        if !details.genres.is_empty() && owned.is_none_or(|owned| owned.genres.is_empty()) {
            add(t!("fact-genres"), details.genres.join(", "));
        }
        let features = features_line(details);
        if !features.is_empty() {
            add(t!("fact-features"), features);
        }
        if let Some(found) = &details.metacritic {
            rows.push((
                t!("fact-metacritic"),
                button(text(format!("{} / 100", found.score)).size(14))
                    .padding(0)
                    .style(style::link)
                    .on_press(Message::Open(found.url.clone()))
                    .into(),
            ));
        }
        rows
    }

    /// The screenshots, a row to scroll sideways; each opens its full size.
    pub(super) fn screenshots_card(&self, id: &str) -> Option<Element<'_, Message>> {
        let facts = self.facts_of(id)?;
        if facts.shots.is_empty() {
            return None;
        }
        let shots = facts.shots.iter().map(|shot| {
            button(image(image::Handle::from_path(&shot.thumb)).height(SHOT_HEIGHT))
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(shot.full.clone()))
                .into()
        });
        Some(card(
            t!("screenshots-title"),
            scrollable(row(shots).spacing(style::SPACE_M))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .width(Length::Fill)
                .into(),
        ))
    }

    /// Where the page's description would be, for a game no Steam app could
    /// be found for: the way to give it one.
    pub(super) fn no_match_card(&self, game: &Game) -> Option<Element<'_, Message>> {
        let none_found = matches!(self.ratings.get(&game.id), Some(RatingState::Got(None)));
        if game.id.starts_with("steam:") || game.steam_app.is_some() || !none_found {
            return None;
        }
        Some(card(
            t!("game-about"),
            column![
                text(t!("steam-app-no-match")).size(13).color(colors().dim),
                button(style::button_text(t!("steam-app-set")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press(Message::GameView(GameViewMessage::ShowSteamApp)),
            ]
            .spacing(style::SPACE_M)
            .into(),
        ))
    }

    /// The Technical card's row for the game's Steam app id: the one in use
    /// and where it came from, and a field to give or clear one. Not for a
    /// Steam game, which has its own.
    pub(super) fn steam_app_row<'a>(&'a self, game: &'a Game) -> Option<Element<'a, Message>> {
        if game.id.starts_with("steam:") {
            return None;
        }
        let in_use = match (game.steam_app, self.ratings.get(&game.id)) {
            (Some(app), _) => t!("steam-app-by-you", app = app.to_string()),
            (None, Some(RatingState::Got(Some(rating)))) => {
                t!("steam-app-found", app = rating.app.to_string())
            }
            (None, Some(RatingState::Got(None))) => t!("steam-app-none"),
            (None, _) => t!("fact-asking"),
        };
        let typed = self
            .game_view
            .steam_app_input
            .clone()
            .unwrap_or_else(|| game.steam_app.map(|app| app.to_string()).unwrap_or_default());
        let id = game.id.clone();
        let mut controls = row![
            text_input(&t!("steam-app-placeholder"), &typed)
                .on_input(|text| Message::GameView(GameViewMessage::SteamAppInput(text)))
                .on_submit(Message::GameView(GameViewMessage::SteamAppSave(id.clone())))
                .size(13)
                .padding([style::SPACE_XS, style::SPACE_S])
                .style(style::input)
                .width(120),
            button(style::button_text(t!("steam-app-save")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::GameView(GameViewMessage::SteamAppSave(id.clone()))),
        ]
        .spacing(style::SPACE_S)
        .align_y(Alignment::Center);
        if game.steam_app.is_some() {
            controls = controls.push(
                button(style::button_text(t!("steam-app-clear")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press(Message::GameView(GameViewMessage::SteamAppClear(id))),
            );
        }
        Some(label_row(
            t!("steam-app-label"),
            column![text(in_use).size(13).font(BOLD), controls]
                .spacing(style::SPACE_S)
                .into(),
        ))
    }

    /// Saves what was typed as the game's Steam app id.
    pub(super) fn save_steam_app(&mut self, id: &str) -> Task<Message> {
        let typed = self.game_view.steam_app_input.clone().unwrap_or_default();
        match typed.trim().parse::<u32>() {
            Ok(app) if app > 0 => self.give_steam_app(id, Some(app)),
            _ => {
                self.status = Some(t!("steam-app-invalid"));
                Task::none()
            }
        }
    }

    /// Gives the game a Steam app id, or takes it back with `None`; what
    /// was found for the old one is forgotten and asked again.
    pub(super) fn give_steam_app(&mut self, id: &str, app: Option<u32>) -> Task<Message> {
        let Some(game) = self.game_mut(id) else {
            return Task::none();
        };
        game.steam_app = app;
        let title = game.title.clone();
        self.save(|state| {
            state.set_steam_app(id, app);
        });
        self.ratings.remove(id);
        self.facts.remove(id);
        self.requirements.remove(id);
        self.anti_cheat.remove(id);
        self.decks.remove(id);
        self.game_view.steam_app_input = None;
        self.status = Some(if let Some(app) = app {
            t!("steam-app-saved", title = title, app = app.to_string())
        } else {
            t!("steam-app-cleared", title = title)
        });
        self.want_rating(id)
    }
}

/// Steam's features of the game, with its controller support when the
/// features do not already say it.
fn features_line(details: &zendo_core::owned::StoreDetails) -> String {
    let mut features = details.features.clone();
    let controller = match details.controller.as_deref() {
        Some("full") => Some(t!("fact-controller-full")),
        Some("partial") => Some(t!("fact-controller-partial")),
        _ => None,
    };
    if let Some(controller) = controller
        && !features
            .iter()
            .any(|feature| feature.eq_ignore_ascii_case(&controller))
    {
        features.push(controller);
    }
    // Steam lists up to thirty; the first few say what the game is.
    let more = features.len().saturating_sub(FEATURES_SHOWN);
    features.truncate(FEATURES_SHOWN);
    let mut line = features.join(", ");
    if more > 0 {
        line.push_str(&t!("fact-features-more", count = more.to_string()));
    }
    line
}

#[cfg(test)]
mod tests {
    use zendo_core::owned::{Metacritic, Screenshot, StoreDetails};
    use zendo_core::steam_facts::Shot;

    use super::*;

    fn facts() -> Facts {
        Facts {
            details: StoreDetails {
                summary: Some("A game about portals.".into()),
                developer: Some("Valve".into()),
                publisher: Some("Valve".into()),
                released: Some("18 Apr, 2011".into()),
                genres: vec!["Action".into()],
                features: vec!["Single-player".into(), "Co-op".into()],
                controller: Some("full".into()),
                metacritic: Some(Metacritic {
                    score: 95,
                    url: "https://www.metacritic.com/game/pc/portal-2".into(),
                }),
                screenshots: vec![Screenshot {
                    thumb: "https://x/t.jpg".into(),
                    full: "https://x/f.jpg".into(),
                }],
                ..StoreDetails::default()
            },
            shots: vec![Shot {
                thumb: "/nowhere/t.jpg".into(),
                full: "https://x/f.jpg".into(),
            }],
        }
    }

    fn labels(rows: &[(String, Element<'_, Message>)]) -> Vec<String> {
        rows.iter().map(|(label, _)| label.clone()).collect()
    }

    #[test]
    fn keeps_the_answer_and_its_failure() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        assert!(zendo.facts_of("local:a").is_none(), "not asked yet");
        let _ = zendo.update(Message::Facts("local:a".into(), Ok(Some(facts()))));
        assert!(zendo.facts_of("local:a").is_some());
        assert!(zendo.screenshots_card("local:a").is_some());
        let _ = zendo.update(Message::Facts("local:b".into(), Ok(None)));
        assert!(zendo.facts_of("local:b").is_none());
        assert!(zendo.screenshots_card("local:b").is_none());
        let _ = zendo.update(Message::Facts("local:a".into(), Err("offline".into())));
        assert!(matches!(
            zendo.facts.get("local:a"),
            Some(FactsState::Failed(error)) if error == "offline"
        ));
        assert!(zendo.facts_of("local:a").is_none());
    }

    #[test]
    fn describes_a_game_by_its_own_store_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Facts("local:a".into(), Ok(Some(facts()))));
        assert_eq!(
            zendo.description_of("local:a", None),
            Some("A game about portals.")
        );
        let mut owned = Owned::new(zendo_core::owned::Shop::Steam, "620", "Portal 2".into());
        owned.summary = Some("Its own.".into());
        assert_eq!(zendo.description_of("local:a", Some(&owned)), Some("Its own."));
        assert_eq!(zendo.description_of("local:z", None), None);
    }

    #[test]
    fn fills_only_what_the_games_store_left_out() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Facts("local:a".into(), Ok(Some(facts()))));
        let game = Game::new(
            "local:a".into(),
            "Portal".into(),
            zendo_core::Source::Local,
            zendo_core::Launch::Local {
                program: "/bin/true".into(),
                args: Vec::new(),
                dir: None,
            },
        );
        let rows = zendo.steam_fact_rows(&game, None);
        assert_eq!(
            labels(&rows),
            [
                t!("fact-developer"),
                t!("fact-publisher"),
                t!("fact-released"),
                t!("fact-genres"),
                t!("fact-features"),
                t!("fact-metacritic"),
            ]
        );
        let mut owned = Owned::new(zendo_core::owned::Shop::Steam, "620", "Portal 2".into());
        owned.publisher = Some("Own publisher".into());
        owned.genres = vec!["Puzzle".into()];
        let rows = zendo.steam_fact_rows(&game, Some(&owned));
        let labels = labels(&rows);
        assert!(!labels.contains(&t!("fact-publisher")));
        assert!(!labels.contains(&t!("fact-genres")));
        assert!(labels.contains(&t!("fact-features")));
    }

    #[test]
    fn names_the_controller_once() {
        let mut details = facts().details;
        assert_eq!(
            features_line(&details),
            format!("Single-player, Co-op, {}", t!("fact-controller-full"))
        );
        details.features.push(t!("fact-controller-full"));
        assert_eq!(features_line(&details).matches("ontroller").count(), 1);
        details.controller = None;
        details.features.clear();
        assert_eq!(features_line(&details), "");
    }

    #[test]
    fn names_the_first_features_and_counts_the_rest() {
        let details = StoreDetails {
            features: (1..=9).map(|n| format!("F{n}")).collect(),
            ..StoreDetails::default()
        };
        let line = features_line(&details);
        assert!(line.starts_with("F1, F2, F3, F4, F5, F6"));
        assert!(!line.contains("F7"));
        assert!(line.ends_with(&t!("fact-features-more", count = "3".to_owned())));
    }
}
