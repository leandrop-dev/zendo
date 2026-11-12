//! The Achievements card on a game's page: how many the player has
//! unlocked, with a bar, then each one with its icon, name, description
//! and when it was unlocked or how rare it is. Unlocked first, newest
//! first, then locked, most common first. A locked hidden one shows as
//! "Hidden achievement" until unlocked. Ten show at first; Show more adds
//! more. Asked once a session as the page opens, from the game's store
//! (GOG, Steam or Epic); a game with none has no card.

use iced::widget::{button, column, container, image, progress_bar, row, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::achievements::{self, Achievement, Achievements};
use zendo_core::steam;

use super::game::{GameViewMessage, card, failed_card};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking, format, now};
use crate::i18n::t;

/// How many show at first, and how many more Show more adds.
pub(super) const SHOWN: usize = 10;

/// The icons' size.
const ICON: f32 = 44.0;

/// A game's achievements, as far as Zendo knows them.
#[derive(Debug)]
pub enum AchievementsState {
    Asking,
    /// `None`: the store gives none for it.
    Got(Option<Achievements>),
    /// What went wrong; the card offers to ask again.
    Failed(String),
}

impl Zendo {
    /// Asks the game's store for its achievements, on a thread, once a
    /// session; nothing for a local game.
    pub(super) fn want_achievements(&mut self, id: &str) -> Task<Message> {
        let from_store = ["gog:", "steam:", "epic:"]
            .iter()
            .any(|prefix| id.starts_with(prefix));
        if !from_store
            || !self.persist
            || matches!(
                self.achievements.get(id),
                Some(AchievementsState::Asking | AchievementsState::Got(_))
            )
        {
            return Task::none();
        }
        self.achievements
            .insert(id.to_owned(), AchievementsState::Asking);
        let signed_in = self.steam_session().map(|session| session.steamid);
        let id = id.to_owned();
        Task::perform(
            {
                let id = id.clone();
                blocking(move || {
                    // The Steam sign-in's account, else the one Steam on
                    // this computer signed in last.
                    let steam_id =
                        signed_in.or_else(|| steam::user(&steam::roots()).map(|user| user.id));
                    achievements::for_game(&id, steam_id.as_deref())
                        .map_err(|err| format!("{err:#}"))
                })
            },
            move |result| Message::Achievements(id.clone(), result),
        )
    }

    /// Keeps the answer; a failure (offline, say) keeps its reason, and
    /// the card offers to ask again.
    pub(super) fn achievements_arrived(
        &mut self,
        id: String,
        result: Result<Option<Achievements>, String>,
    ) -> Task<Message> {
        // Kept for the library's progress line, when the unlocks are known.
        let seen = match &result {
            Ok(Some(found)) if found.unlocks_known => {
                let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
                Some((count(found.unlocked()), count(found.list.len())))
            }
            _ => None,
        };
        let state = match result {
            Ok(found) => AchievementsState::Got(found),
            Err(err) => AchievementsState::Failed(err),
        };
        if let Some((unlocked, total)) = seen {
            if let Some(game) = self.game_mut(&id) {
                game.achievements = Some((unlocked, total));
            }
            self.save(|state| state.set_achievements(&id, unlocked, total));
        }
        self.achievements.insert(id, state);
        Task::none()
    }

    /// The card, once the achievements have come.
    pub(super) fn achievements_card(&self, id: &str) -> Option<Element<'_, Message>> {
        if let AchievementsState::Failed(error) = self.achievements.get(id)? {
            return Some(failed_card(t!("achievements-title"), id, error));
        }
        let AchievementsState::Got(Some(found)) = self.achievements.get(id)? else {
            return None;
        };
        let total = found.list.len();
        let mut body = column![].spacing(12);
        if found.unlocks_known {
            let unlocked = found.unlocked();
            #[expect(
                clippy::cast_precision_loss,
                reason = "counts of achievements, far below f32's exact range"
            )]
            let share = unlocked as f32 / total.max(1) as f32;
            body = body.push(
                row![
                    text(t!(
                        "achievements-unlocked",
                        unlocked = unlocked,
                        total = total
                    ))
                    .size(14),
                    progress_bar(0.0..=1.0, share)
                        .girth(6)
                        .style(style::progress),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        } else {
            let why = if id.starts_with("steam:") {
                t!("achievements-unknown-steam", total = total)
            } else {
                t!("achievements-unknown", total = total)
            };
            body = body.push(text(why).size(14).color(colors().dim));
        }
        let shown = (SHOWN + self.game_view.achievements_more).min(total);
        let now = now();
        for achievement in &found.list[..shown] {
            body = body.push(line(achievement, now));
        }
        let left = total - shown;
        if left > 0 {
            body = body.push(
                button(text(t!("achievements-more", count = left)).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::GameView(GameViewMessage::MoreAchievements)),
            );
        }
        Some(card(t!("achievements-title"), body.into()))
    }
}

/// One achievement: its icon, its name and description, and when it was
/// unlocked or how rare it is.
fn line(achievement: &Achievement, now: u64) -> Element<'_, Message> {
    let kept_back = achievement.hidden && achievement.unlocked.is_none();
    let icon: Element<'_, Message> = match &achievement.icon {
        Some(path) => image(image::Handle::from_path(path))
            .width(ICON)
            .height(ICON)
            .border_radius(6)
            .into(),
        None => container(text("?").size(18).color(colors().dim))
            .center(Length::Fill)
            .width(ICON)
            .height(ICON)
            .style(style::card)
            .into(),
    };
    let (name, description) = if kept_back {
        (t!("achievements-hidden"), t!("achievements-hidden-about"))
    } else {
        (achievement.name.clone(), achievement.description.clone())
    };
    let mut words = column![text(name).size(14).font(BOLD)].spacing(2);
    if !description.is_empty() {
        words = words.push(text(description).size(13).color(colors().dim));
    }
    let side = match (achievement.unlocked, achievement.rarity) {
        (Some(when), _) if when > 0 => text(t!(
            "achievements-unlocked-when",
            when = format::ago(when, now)
        ))
        .size(13)
        .color(colors().good),
        (Some(_), _) => text(t!("achievements-unlocked-plain"))
            .size(13)
            .color(colors().good),
        (None, Some(rarity)) => text(t!("achievements-rarity", percent = rarity_text(rarity)))
            .size(13)
            .color(colors().dim),
        (None, None) => text(""),
    };
    row![
        icon,
        words.width(Length::Fill),
        space::horizontal().width(8),
        side
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

/// A share of players: one decimal under 10 %, whole numbers above.
fn rarity_text(percent: f32) -> String {
    if percent < 10.0 {
        format!("{percent:.1}")
    } else {
        format!("{percent:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn achievement(unlocked: Option<u64>) -> Achievement {
        Achievement {
            name: "Sugar".into(),
            description: "Eat it".into(),
            hidden: false,
            unlocked,
            rarity: Some(3.24),
            icon: None,
        }
    }

    #[test]
    fn shows_a_card_for_a_game_with_achievements() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.want_achievements("local:1");
        assert!(!zendo.achievements.contains_key("local:1"));
        assert!(zendo.achievements_card("gog:1").is_none(), "not asked yet");
        let found = Achievements {
            list: (0..15)
                .map(|n| achievement((n < 4).then_some(100 + n)))
                .collect(),
            unlocks_known: true,
        };
        let _ = zendo.update(Message::Achievements("gog:1".into(), Ok(Some(found))));
        assert!(zendo.achievements_card("gog:1").is_some());
        let _ = zendo.update(Message::GameView(GameViewMessage::MoreAchievements));
        assert_eq!(zendo.game_view.achievements_more, SHOWN);
        assert!(
            zendo.achievements_card("gog:1").is_some(),
            "all fifteen shown"
        );
        let _ = zendo.update(Message::Achievements("gog:2".into(), Ok(None)));
        assert!(zendo.achievements_card("gog:2").is_none());
        let _ = zendo.update(Message::Achievements("gog:1".into(), Err("offline".into())));
        assert!(
            matches!(
                zendo.achievements.get("gog:1"),
                Some(AchievementsState::Failed(error)) if error == "offline"
            ),
            "the reason stays, with a way to ask again"
        );
        assert!(zendo.achievements_card("gog:1").is_some());
    }

    #[test]
    fn writes_rarity_to_one_decimal_when_rare() {
        assert_eq!(rarity_text(3.24), "3.2");
        assert_eq!(rarity_text(34.8), "35");
    }
}
