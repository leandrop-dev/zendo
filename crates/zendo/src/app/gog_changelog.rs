//! The Changelog card on a GOG game's page: GOG's notes for each update,
//! newest first, as Heroic shows them. Asked once a session as the page
//! opens; the card shows the newest entry, and Show earlier updates adds a
//! few more each time. A game with no changelog (about half) has no card;
//! a failure (offline, say) is forgotten, so the page asks again next
//! time.

use iced::widget::{button, column, row, text};
use iced::{Element, Length, Task};
use zendo_core::Game;
use zendo_core::gog_changelog::{self, Block, Entry};

use super::game::{GameViewMessage, card};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// How many more entries Show earlier updates shows each time.
pub(super) const MORE: usize = 3;

/// GOG's changelog for a game, as far as Zendo knows it.
#[derive(Debug)]
pub enum Changelog {
    Asking,
    Got(Vec<Entry>),
}

impl Zendo {
    /// Asks GOG for the game's changelog, on a thread; nothing for a game
    /// not from GOG, or one asked already.
    pub(super) fn want_changelog(&mut self, id: &str) -> Task<Message> {
        let Some(product) = id.strip_prefix("gog:") else {
            return Task::none();
        };
        if !self.persist || self.changelogs.contains_key(id) {
            return Task::none();
        }
        self.changelogs.insert(id.to_owned(), Changelog::Asking);
        let id = id.to_owned();
        let product = product.to_owned();
        Task::perform(
            blocking(move || gog_changelog::fetch(&product).map_err(|err| format!("{err:#}"))),
            move |result| Message::GogChangelog(id.clone(), result),
        )
    }

    pub(super) fn changelog_arrived(
        &mut self,
        id: String,
        result: Result<Vec<Entry>, String>,
    ) -> Task<Message> {
        match result {
            Ok(entries) => {
                self.changelogs.insert(id, Changelog::Got(entries));
            }
            Err(_) => {
                self.changelogs.remove(&id);
            }
        }
        Task::none()
    }

    /// The card, for a GOG game whose changelog has come and says
    /// something.
    pub(super) fn changelog_card<'a>(&'a self, game: &Game) -> Option<Element<'a, Message>> {
        let Some(Changelog::Got(entries)) = self.changelogs.get(&game.id) else {
            return None;
        };
        if entries.is_empty() {
            return None;
        }
        let shown = (1 + self.game_view.changelog_more).min(entries.len());
        let mut body = column(entries[..shown].iter().map(entry)).spacing(28);
        let left = entries.len() - shown;
        if left > 0 {
            body = body.push(
                button(text(t!("changelog-earlier", count = left)).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::GameView(GameViewMessage::MoreChangelog)),
            );
        }
        Some(card(t!("changelog-title"), body.into()))
    }
}

/// One update: its title, then its parts.
fn entry(entry: &Entry) -> Element<'_, Message> {
    let mut lines = column![].spacing(8);
    if let Some(title) = &entry.title {
        lines = lines.push(text(title).size(15).font(BOLD));
    }
    for block in &entry.blocks {
        lines = lines.push(match block {
            Block::Heading(heading) => Element::from(
                column![text(heading).size(14).font(BOLD).color(colors().dim)].padding([6, 0]),
            ),
            Block::Text(paragraph) => text(paragraph).size(14).line_height(1.5).into(),
            Block::Item(item) => row![
                text("\u{2022}").size(14).color(colors().dim),
                text(item).size(14).line_height(1.5).width(Length::Fill),
            ]
            .spacing(10)
            .into(),
        });
    }
    lines.into()
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn titled(title: &str) -> Entry {
        Entry {
            title: Some(title.to_owned()),
            blocks: vec![Block::Item("Fixed".into())],
        }
    }

    #[test]
    fn shows_the_newest_entry_and_more_on_asking() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        assert!(zendo.changelog_card(&game).is_none(), "nothing asked yet");
        let _ = zendo.update(Message::GogChangelog("gog:1".into(), Ok(Vec::new())));
        assert!(zendo.changelog_card(&game).is_none(), "GOG has none");

        let entries: Vec<Entry> = (1..=5).map(|n| titled(&format!("Patch {n}"))).collect();
        let _ = zendo.update(Message::GogChangelog("gog:1".into(), Ok(entries)));
        assert!(zendo.changelog_card(&game).is_some());
        let _ = zendo.update(Message::GameView(GameViewMessage::MoreChangelog));
        assert_eq!(zendo.game_view.changelog_more, MORE);
        let _ = zendo.update(Message::GogChangelog("gog:1".into(), Err("offline".into())));
        assert!(
            !zendo.changelogs.contains_key("gog:1"),
            "asked again next time"
        );
    }

    #[test]
    fn only_gog_games_ask() {
        let (mut zendo, _) = Zendo::boot();
        let _ = zendo.want_changelog("epic:abc");
        assert!(zendo.changelogs.is_empty());
    }
}
