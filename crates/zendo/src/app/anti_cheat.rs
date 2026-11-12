//! The Anti-cheat card on a game's page: whether its anti-cheat lets it
//! play online on Linux, from `AreWeAntiCheatYet`, with the anti-cheats it
//! ships, the newest change and the list's notes. Asked once a session,
//! after `ProtonDB`'s answer gives the game's Steam app id; a game not on
//! the list has no card. A game that will not play online comes first on
//! its page, before installing; Install and Play stay as they are.

use iced::widget::{button, column, row, text};
use iced::{Color, Element, Length, Task};
use zendo_core::areweanticheatyet::{self, Entry, Note, Status};

use super::game::{card, failed_card};
use super::style::{self, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// What `AreWeAntiCheatYet` says of a game, as far as Zendo knows it.
#[derive(Debug)]
pub enum AntiCheatState {
    Asking,
    /// `None`: the list does not have it.
    Got(Option<Entry>),
    /// What went wrong; the card offers to ask again.
    Failed(String),
}

fn status_color(status: Status) -> Color {
    match status {
        Status::Supported | Status::Running => colors().good,
        Status::Planned => colors().dim,
        Status::Broken | Status::Denied | Status::Unknown => colors().bad,
    }
}

fn status_label(status: Status) -> String {
    match status {
        Status::Supported => t!("anti-cheat-supported"),
        Status::Running => t!("anti-cheat-running"),
        Status::Planned => t!("anti-cheat-planned"),
        Status::Broken => t!("anti-cheat-broken"),
        Status::Denied => t!("anti-cheat-denied"),
        Status::Unknown => t!("anti-cheat-unknown"),
    }
}

/// The site's meaning of the status.
fn status_about(status: Status) -> String {
    match status {
        Status::Supported => t!("anti-cheat-supported-about"),
        Status::Running => t!("anti-cheat-running-about"),
        Status::Planned => t!("anti-cheat-planned-about"),
        Status::Broken => t!("anti-cheat-broken-about"),
        Status::Denied => t!("anti-cheat-denied-about"),
        Status::Unknown => t!("anti-cheat-unknown-about"),
    }
}

impl Zendo {
    /// Looks the game up in the list, on a thread, once a session.
    pub(super) fn want_anti_cheat(&mut self, id: &str, steam_app: Option<u32>) -> Task<Message> {
        if !self.persist
            || matches!(
                self.anti_cheat.get(id),
                Some(AntiCheatState::Asking | AntiCheatState::Got(_))
            )
        {
            return Task::none();
        }
        self.anti_cheat
            .insert(id.to_owned(), AntiCheatState::Asking);
        let id = id.to_owned();
        Task::perform(
            {
                let id = id.clone();
                blocking(move || {
                    areweanticheatyet::for_game(&id, steam_app).map_err(|err| format!("{err:#}"))
                })
            },
            move |result| Message::AntiCheat(id.clone(), result),
        )
    }

    /// Keeps the answer; a failure (offline with no list kept, say) keeps
    /// its reason, and the card offers to ask again.
    pub(super) fn anti_cheat_arrived(
        &mut self,
        id: String,
        result: Result<Option<Entry>, String>,
    ) -> Task<Message> {
        let state = match result {
            Ok(entry) => AntiCheatState::Got(entry),
            Err(err) => AntiCheatState::Failed(err),
        };
        self.anti_cheat.insert(id, state);
        Task::none()
    }

    /// The game's entry, once it has come.
    fn anti_cheat_entry(&self, id: &str) -> Option<&Entry> {
        match self.anti_cheat.get(id)? {
            AntiCheatState::Got(Some(entry)) => Some(entry),
            AntiCheatState::Got(None) | AntiCheatState::Asking | AntiCheatState::Failed(_) => None,
        }
    }

    /// Adds the card to a page's main column: first when the game will
    /// not play online, else after About.
    pub(super) fn add_anti_cheat_card<'a>(
        &'a self,
        id: &str,
        main: &mut Vec<Element<'a, Message>>,
    ) {
        if let Some(AntiCheatState::Failed(error)) = self.anti_cheat.get(id) {
            main.push(failed_card(t!("anti-cheat-title"), id, error));
            return;
        }
        let Some(entry) = self.anti_cheat_entry(id) else {
            return;
        };
        let card = anti_cheat_card(entry);
        if entry.status.warns() {
            main.insert(0, card);
        } else {
            main.push(card);
        }
    }
}

fn anti_cheat_card(entry: &Entry) -> Element<'_, Message> {
    let color = status_color(entry.status);
    let mut head = row![style::badge(status_label(entry.status), color)]
        .spacing(style::SPACE_M)
        .align_y(iced::Alignment::Center);
    if !entry.anti_cheats.is_empty() {
        head = head.push(
            text(entry.anti_cheats.join(", "))
                .size(14)
                .color(colors().dim),
        );
    }
    let head = head.wrap();
    let mut body = column![
        head,
        text(status_about(entry.status)).size(14).line_height(1.5),
    ]
    .spacing(style::SPACE_M);
    if let Some((latest, date)) = &entry.latest {
        let line = if date.is_empty() {
            t!("anti-cheat-latest", change = latest.text.as_str())
        } else {
            t!(
                "anti-cheat-latest-dated",
                change = latest.text.as_str(),
                date = date.as_str()
            )
        };
        body = body.push(note_line(line, latest));
    }
    for note in &entry.notes {
        body = body.push(note_line(note.text.clone(), note));
    }
    body = body.push(
        button(style::button_text(t!("anti-cheat-open")))
            .padding(style::BUTTON_SMALL)
            .style(style::secondary)
            .on_press(Message::Open(entry.url())),
    );
    card(t!("anti-cheat-title"), body.into())
}

/// A line of the list's, opening its source when it has one.
fn note_line<'a>(line: String, note: &Note) -> Element<'a, Message> {
    let words = text(line).size(14).line_height(1.5).width(Length::Fill);
    let bullet = text("\u{2022}").size(14).color(colors().dim);
    match &note.link {
        Some(source) => row![
            bullet,
            button(words)
                .padding(0)
                .style(style::link)
                .on_press(Message::Open(source.clone())),
        ]
        .spacing(style::SPACE_M)
        .into(),
        None => row![bullet, words].spacing(style::SPACE_M).into(),
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::protondb::Rating;

    use super::*;

    fn entry(status: Status) -> Entry {
        Entry {
            name: "Rust".into(),
            status,
            anti_cheats: vec!["Easy Anti-Cheat".into()],
            notes: vec![Note {
                text: "A note".into(),
                link: None,
            }],
            latest: None,
            slug: "rust".into(),
        }
    }

    /// How many cards the page's main column has once the anti-cheat
    /// card is added to one holding About.
    fn cards(zendo: &Zendo, id: &str) -> usize {
        let mut main: Vec<Element<'_, Message>> = vec![text("About").into()];
        zendo.add_anti_cheat_card(id, &mut main);
        main.len()
    }

    #[test]
    fn shows_a_card_for_a_game_on_the_list() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        for (id, status) in [("steam:1", Status::Denied), ("steam:2", Status::Supported)] {
            let _ = zendo.update(Message::AntiCheat(id.into(), Ok(Some(entry(status)))));
            assert_eq!(cards(&zendo, id), 2, "{status:?}");
        }
    }

    #[test]
    fn looks_the_game_up_only_once_protondb_answered() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        assert_eq!(cards(&zendo, "steam:252490"), 1, "nothing asked yet");
        let _ = zendo.update(Message::Rating(
            "steam:252490".into(),
            Ok(Some(Rating {
                app: 252_490,
                tier: Some("borked".into()),
                reports: 3,
                native: false,
            })),
        ));
        assert!(
            !zendo.anti_cheat.contains_key("steam:252490"),
            "not asked when nothing may be kept"
        );
        let _ = zendo.update(Message::AntiCheat("steam:252490".into(), Ok(None)));
        assert_eq!(cards(&zendo, "steam:252490"), 1, "not on the list");
        let _ = zendo.update(Message::AntiCheat(
            "steam:252490".into(),
            Err("offline".into()),
        ));
        assert!(
            matches!(
                zendo.anti_cheat.get("steam:252490"),
                Some(AntiCheatState::Failed(error)) if error == "offline"
            ),
            "the reason stays, with a way to ask again"
        );
        assert_eq!(cards(&zendo, "steam:252490"), 2, "the failure card");
    }

    #[test]
    fn colours_what_will_not_play_online_as_bad() {
        assert_eq!(status_color(Status::Denied), colors().bad);
        assert_eq!(status_color(Status::Broken), colors().bad);
        assert_eq!(status_color(Status::Supported), colors().good);
        assert_eq!(status_label(Status::Denied), "DENIED");
    }
}
