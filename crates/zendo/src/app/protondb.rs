//! The `ProtonDB` pill on a game's page: how well it runs through Proton,
//! from Platinum to Borked, opening the game's `ProtonDB` page when clicked.

use iced::widget::{button, container, text, tooltip};
use iced::{Color, Element, Task};
use zendo_core::protondb::{self, Rating};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub enum RatingState {
    Asking,
    /// `None`: no Steam app found for the game, so `ProtonDB` cannot know it.
    Got(Option<Rating>),
}

fn tier_color(tier: &str) -> Color {
    match tier {
        "platinum" => Color::from_rgb8(0xb4, 0xc7, 0xdc),
        "gold" => Color::from_rgb8(0xcf, 0xb5, 0x3b),
        "silver" => Color::from_rgb8(0xa6, 0xa6, 0xa6),
        "bronze" => Color::from_rgb8(0xcd, 0x7f, 0x32),
        "borked" => colors().bad,
        _ => colors().dim,
    }
}

impl Zendo {
    /// Asks `ProtonDB` about the game, on a thread, once a session.
    pub(super) fn want_rating(&mut self, id: &str) -> Task<Message> {
        if !self.persist || self.ratings.contains_key(id) {
            return Task::none();
        }
        let title = match (self.game(id), self.store.owned(id)) {
            (Some(game), _) => game.title.clone(),
            (None, Some(owned)) => owned.title.clone(),
            (None, None) => return Task::none(),
        };
        self.ratings.insert(id.to_owned(), RatingState::Asking);
        let id = id.to_owned();
        Task::perform(
            {
                let id = id.clone();
                blocking(move || protondb::rating(&id, &title).map_err(|err| format!("{err:#}")))
            },
            move |result| Message::Rating(id.clone(), result),
        )
    }

    /// Keeps the answer; a failure (offline, say) is forgotten, so the
    /// page asks again next time it opens.
    pub(super) fn rating_arrived(
        &mut self,
        id: String,
        result: Result<Option<Rating>, String>,
    ) -> Task<Message> {
        match result {
            Ok(rating) => {
                self.ratings.insert(id, RatingState::Got(rating));
            }
            Err(_) => {
                self.ratings.remove(&id);
            }
        }
        Task::none()
    }

    /// The pill for the game's page: nothing when `ProtonDB` cannot know the
    /// game, or could not be asked.
    pub(super) fn rating_pill(&self, id: &str) -> Option<Element<'_, Message>> {
        match self.ratings.get(id)? {
            RatingState::Asking => Some(style::badge("PROTONDB …", colors().dim)),
            RatingState::Got(Some(rating)) => Some(pill(rating)),
            RatingState::Got(None) => None,
        }
    }
}

/// What the reports say, for the tooltip.
fn reports(rating: &Rating) -> String {
    match (&rating.tier, rating.reports) {
        (None, _) => t!("protondb-no-reports-yet"),
        (Some(tier), count) => t!("protondb-reports", tier = title_case(tier), count = count),
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Native, as `ProtonDB`'s pages say, for a game with a Linux build; else
/// the reports' tier.
fn pill(rating: &Rating) -> Element<'_, Message> {
    let (label, color, about) = match &rating.tier {
        _ if rating.native => (
            t!("protondb-native"),
            colors().good,
            t!("protondb-native-about", reports = reports(rating)),
        ),
        Some(tier) => (
            format!("PROTONDB {}", tier.to_uppercase()),
            tier_color(tier),
            t!("protondb-tier-about", reports = reports(rating)),
        ),
        None => (t!("protondb-none"), colors().dim, t!("protondb-none-about")),
    };
    let link = button(text(label).size(11).font(BOLD))
        .padding([2, 7])
        .style(style::pill(color))
        .on_press(Message::Open(rating.url()));
    tooltip(
        link,
        container(text(about).size(12))
            .padding([4, 8])
            .style(style::card),
        tooltip::Position::Bottom,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_a_pill_only_for_games_protondb_knows() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        assert!(zendo.rating_pill("gog:1").is_none(), "not asked yet");
        let _ = zendo.update(Message::Rating(
            "gog:1".into(),
            Ok(Some(Rating {
                app: 435_790,
                tier: Some("platinum".into()),
                reports: 37,
                native: false,
            })),
        ));
        assert!(zendo.rating_pill("gog:1").is_some());
        let _ = zendo.update(Message::Rating("gog:2".into(), Ok(None)));
        assert!(zendo.rating_pill("gog:2").is_none());
        let _ = zendo.update(Message::Rating("gog:3".into(), Err("offline".into())));
        assert!(zendo.rating_pill("gog:3").is_none());
        assert_eq!(tier_color("gold"), Color::from_rgb8(0xcf, 0xb5, 0x3b));
        let native = Rating {
            app: 785_890,
            tier: Some("platinum".into()),
            reports: 9,
            native: true,
        };
        assert_eq!(reports(&native), "Proton: Platinum from 9 reports");
    }
}
