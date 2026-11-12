//! The System requirements card on a game's page: what the game asks of
//! the computer, minimum and recommended side by side, from its own store
//! or Steam's. For Windows, or Linux for a game installed as a Linux
//! build. Asked once a session, after `ProtonDB`'s answer gives the game's
//! Steam app id; a game no store lists them for has no card.

use iced::widget::{column, row, text};
use iced::{Element, Length, Task};
use zendo_core::requirements::{self, From, Requirements, System};

use super::game::{LABEL_WIDTH, card, failed_card};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// A game's requirements, as far as Zendo knows them.
#[derive(Debug)]
pub enum RequirementsState {
    Asking,
    /// `None`: no store lists them.
    Got(Option<Requirements>),
    /// What went wrong; the card offers to ask again.
    Failed(String),
}

impl Zendo {
    /// Asks the stores for the game's requirements, on a thread, once a
    /// session.
    pub(super) fn want_requirements(&mut self, id: &str, steam_app: Option<u32>) -> Task<Message> {
        if !self.persist
            || matches!(
                self.requirements.get(id),
                Some(RequirementsState::Asking | RequirementsState::Got(_))
            )
        {
            return Task::none();
        }
        let linux = self
            .game(id)
            .and_then(|game| game.details.platform.as_deref())
            .is_some_and(|platform| platform.eq_ignore_ascii_case("linux"));
        let system = if linux {
            System::Linux
        } else {
            System::Windows
        };
        self.requirements
            .insert(id.to_owned(), RequirementsState::Asking);
        let id = id.to_owned();
        Task::perform(
            {
                let id = id.clone();
                blocking(move || {
                    requirements::for_game(&id, steam_app, system).map_err(|err| format!("{err:#}"))
                })
            },
            move |result| Message::Requirements(id.clone(), result),
        )
    }

    /// Keeps the answer; a failure (offline, say) keeps its reason, and
    /// the card offers to ask again.
    pub(super) fn requirements_arrived(
        &mut self,
        id: String,
        result: Result<Option<Requirements>, String>,
    ) -> Task<Message> {
        let state = match result {
            Ok(found) => RequirementsState::Got(found),
            Err(err) => RequirementsState::Failed(err),
        };
        self.requirements.insert(id, state);
        Task::none()
    }

    /// The card, once the requirements have come.
    pub(super) fn requirements_card(&self, id: &str) -> Option<Element<'_, Message>> {
        match self.requirements.get(id)? {
            RequirementsState::Got(Some(found)) => Some(requirements_card(found)),
            RequirementsState::Failed(error) => {
                Some(failed_card(t!("requirements-title"), id, error))
            }
            RequirementsState::Got(None) | RequirementsState::Asking => None,
        }
    }
}

fn requirements_card(found: &Requirements) -> Element<'_, Message> {
    let system = match found.system {
        System::Windows => "Windows",
        System::Linux => "Linux",
    };
    let source = match found.from {
        From::Gog => t!("requirements-from-gog", system = system),
        From::Epic => t!("requirements-from-epic", system = system),
        From::Steam => t!("requirements-from-steam", system = system),
    };
    let both = found.has_recommended();
    let mut body = column![text(source).size(13).color(colors().dim)].spacing(style::SPACE_M);
    if both {
        body = body.push(line(
            String::new(),
            Some(t!("requirements-minimum")),
            Some(t!("requirements-recommended")),
            true,
        ));
    }
    for found_row in &found.rows {
        body = body.push(line(
            found_row.label.clone(),
            found_row.minimum.clone(),
            both.then(|| found_row.recommended.clone().unwrap_or_default()),
            false,
        ));
    }
    card(t!("requirements-title"), body.into())
}

/// A row: the label, then the minimum and, when the game has any, the
/// recommended value.
fn line<'a>(
    label: String,
    minimum: Option<String>,
    recommended: Option<String>,
    heading: bool,
) -> Element<'a, Message> {
    let cell = |value: Option<String>| {
        let value = text(value.unwrap_or_default())
            .size(14)
            .width(Length::FillPortion(1));
        if heading {
            value.font(BOLD).color(colors().dim)
        } else {
            value
        }
    };
    let mut cells = row![
        text(label).size(14).color(colors().dim).width(LABEL_WIDTH),
        cell(minimum),
    ]
    .spacing(style::SPACE_M);
    if let Some(recommended) = recommended {
        cells = cells.push(cell(Some(recommended)));
    }
    cells.into()
}

#[cfg(test)]
mod tests {
    use zendo_core::requirements::Row;

    use super::*;

    #[test]
    fn shows_a_card_only_when_a_store_lists_them() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        assert!(zendo.requirements_card("gog:1").is_none(), "not asked yet");
        let found = Requirements {
            system: System::Windows,
            from: From::Steam,
            rows: vec![Row {
                label: "Memory".into(),
                minimum: Some("8 GB RAM".into()),
                recommended: Some("16 GB RAM".into()),
            }],
        };
        let _ = zendo.update(Message::Requirements("gog:1".into(), Ok(Some(found))));
        assert!(zendo.requirements_card("gog:1").is_some());
        let _ = zendo.update(Message::Requirements("gog:2".into(), Ok(None)));
        assert!(zendo.requirements_card("gog:2").is_none());
        let _ = zendo.update(Message::Requirements("gog:1".into(), Err("offline".into())));
        assert!(
            matches!(
                zendo.requirements.get("gog:1"),
                Some(RequirementsState::Failed(error)) if error == "offline"
            ),
            "the reason stays, with a way to ask again"
        );
        assert!(zendo.requirements_card("gog:1").is_some());
    }
}
