//! The Categories sheet: the user's own categories, made, renamed and
//! deleted here, and the games it was opened for put in or taken out of
//! them.

use std::collections::BTreeSet;

use iced::widget::{button, checkbox, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::{Category, State};

use super::launch_options::overlay;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo};
use crate::i18n::t;

/// The Categories sheet: the user's categories, and the games it assigns
/// when it was opened from a game's page or a selection in the grid.
#[derive(Debug, Clone, Default)]
pub struct Categories {
    /// The games the checkboxes assign. Empty when the sheet only manages.
    pub(super) targets: BTreeSet<String>,
    /// The new category being typed.
    pub(super) draft: String,
    /// The category being renamed, and what is typed in its field.
    pub(super) renaming: Option<String>,
    pub(super) rename_draft: String,
    /// The category whose Delete was pressed once: pressed again, it goes.
    pub(super) confirm_delete: Option<String>,
    /// What the last action did, and whether it worked.
    pub(super) note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum CategoriesMessage {
    /// Open the sheet, assigning to these games (empty: manage only).
    Open(BTreeSet<String>),
    Close,
    Typed(String),
    Create,
    Rename(String),
    RenameTyped(String),
    RenameDone,
    /// Tick or untick the category for every game in `targets`.
    SetMembership {
        name: String,
        member: bool,
    },
    Delete(String),
}

impl Zendo {
    pub(super) fn update_categories(&mut self, message: CategoriesMessage) -> Task<Message> {
        // Any other press takes back a delete that was only asking.
        if !matches!(message, CategoriesMessage::Delete(_))
            && let Some(sheet) = self.categories_sheet.as_mut()
        {
            sheet.confirm_delete = None;
        }
        match message {
            CategoriesMessage::Open(targets) => self.open_categories(targets),
            CategoriesMessage::Close => self.categories_sheet = None,
            CategoriesMessage::Typed(text) => {
                if let Some(sheet) = self.categories_sheet.as_mut() {
                    sheet.draft = text;
                }
            }
            CategoriesMessage::RenameTyped(text) => {
                if let Some(sheet) = self.categories_sheet.as_mut() {
                    sheet.rename_draft = text;
                }
            }
            CategoriesMessage::Create => self.create_category(),
            CategoriesMessage::Rename(name) => {
                if let Some(sheet) = self.categories_sheet.as_mut() {
                    sheet.renaming = Some(name.clone());
                    sheet.rename_draft = name;
                    sheet.note = None;
                }
            }
            CategoriesMessage::RenameDone => self.rename_category(),
            CategoriesMessage::SetMembership { name, member } => {
                self.set_membership(&name, member);
            }
            CategoriesMessage::Delete(name) => self.delete_category(&name),
        }
        Task::none()
    }

    /// Opens the sheet, its list as the state has it now: another program
    /// (`zendo categorize`) may have changed it since the library loaded.
    fn open_categories(&mut self, targets: BTreeSet<String>) {
        if self.persist {
            match State::load() {
                Ok(state) => self.reload_categories(&state),
                Err(err) => self.status = Some(format!("{err:#}")),
            }
        }
        self.categories_sheet = Some(Categories {
            targets,
            ..Categories::default()
        });
    }

    /// Makes the category in the draft, and puts the sheet's games in it.
    fn create_category(&mut self) {
        let Some(sheet) = self.categories_sheet.as_ref() else {
            return;
        };
        let draft = sheet.draft.clone();
        let targets: Vec<String> = sheet.targets.iter().cloned().collect();
        if draft.trim().is_empty() {
            self.say(t!("categories-blank"), false);
            return;
        }
        let mut created = false;
        let made = self.change_state(|state| {
            created = state.add_category(&draft);
            if created {
                for id in &targets {
                    state.set_category(&draft, id, true);
                }
            }
        });
        if !made {
            return;
        }
        if let Some(sheet) = self.categories_sheet.as_mut() {
            sheet.note = if created {
                sheet.draft.clear();
                Some((t!("categories-created", name = draft.trim()), true))
            } else {
                Some((t!("categories-taken", name = draft.trim()), false))
            };
        }
    }

    fn rename_category(&mut self) {
        let Some(sheet) = self.categories_sheet.as_ref() else {
            return;
        };
        let Some(from) = sheet.renaming.clone() else {
            return;
        };
        let to = sheet.rename_draft.clone();
        if to.trim().is_empty() {
            self.say(t!("categories-blank"), false);
            return;
        }
        let mut renamed = false;
        if !self.change_state(|state| renamed = state.rename_category(&from, &to)) {
            return;
        }
        if let Some(sheet) = self.categories_sheet.as_mut() {
            sheet.note = if renamed {
                sheet.renaming = None;
                sheet.rename_draft.clear();
                Some((t!("categories-renamed", name = to.trim()), true))
            } else {
                Some((t!("categories-taken", name = to.trim()), false))
            };
        }
    }

    /// Ticks or unticks a category for every game the sheet was opened for.
    fn set_membership(&mut self, name: &str, member: bool) {
        let Some(sheet) = self.categories_sheet.as_ref() else {
            return;
        };
        let targets: Vec<String> = sheet.targets.iter().cloned().collect();
        let count = targets.len();
        if count == 0 {
            return;
        }
        if !self.change_state(|state| {
            for id in &targets {
                state.set_category(name, id, member);
            }
        }) {
            return;
        }
        if let Some(sheet) = self.categories_sheet.as_mut() {
            sheet.note = Some(if member {
                (t!("categories-assigned", count = count), true)
            } else {
                (t!("categories-unassigned", count = count), true)
            });
        }
    }

    /// Says what the last action did, in the sheet.
    fn say(&mut self, note: String, worked: bool) {
        if let Some(sheet) = self.categories_sheet.as_mut() {
            sheet.note = Some((note, worked));
        }
    }

    /// Delete, pressed once to ask and again to drop the category with its
    /// games, as removing a game added by hand does.
    fn delete_category(&mut self, name: &str) {
        let Some(sheet) = self.categories_sheet.as_mut() else {
            return;
        };
        if sheet.confirm_delete.as_deref() != Some(name) {
            sheet.confirm_delete = Some(name.to_owned());
            sheet.note = None;
            return;
        }
        sheet.confirm_delete = None;
        let mut deleted = false;
        if !self.change_state(|state| deleted = state.delete_category(name)) {
            return;
        }
        let Some(sheet) = self.categories_sheet.as_mut() else {
            return;
        };
        if deleted {
            sheet.note = Some((t!("categories-deleted", name = name.trim()), true));
        }
        if sheet.renaming.as_deref() == Some(name) {
            sheet.renaming = None;
            sheet.rename_draft.clear();
        }
    }

    /// Changes the state, and the window with it: the sheet's list and each
    /// game's categories. False when the state could not be written.
    fn change_state(&mut self, change: impl FnOnce(&mut State)) -> bool {
        let state = if self.persist {
            match State::update(change) {
                Ok(state) => state,
                Err(err) => {
                    self.status = Some(format!("{err:#}"));
                    return false;
                }
            }
        } else {
            // Tests keep the change in memory: the state file is the user's.
            let mut state = self.categories_state();
            change(&mut state);
            state
        };
        self.reload_categories(&state);
        true
    }

    /// The window's categories as a state of their own, so that a change
    /// can be made without reading the user's file.
    fn categories_state(&self) -> State {
        let mut state = State::default();
        for category in &self.categories {
            state.add_category(&category.name);
            for id in &category.games {
                state.set_category(&category.name, id, true);
            }
        }
        state
    }

    /// Puts a state's categories on the sheet's list and on every game.
    fn reload_categories(&mut self, state: &State) {
        self.categories = state.categories();
        let ids: Vec<String> = self
            .library
            .iter()
            .flat_map(|library| &library.games)
            .map(|game| game.id.clone())
            .collect();
        for id in ids {
            let names = state.categories_of(&id);
            if let Some(game) = self.game_mut(&id) {
                game.categories = names;
            }
        }
    }

    /// While the sheet is open, Escape closes it (or leaves a rename), Enter
    /// makes or renames a category, and the page's own keys and the
    /// controller do nothing.
    pub(super) fn categories_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.categories_sheet.as_ref()?;
        match message {
            Message::Back => {
                if let Some(sheet) = self.categories_sheet.as_mut()
                    && sheet.renaming.take().is_some()
                {
                    sheet.rename_draft.clear();
                    return Some(Task::none());
                }
                Some(self.update_categories(CategoriesMessage::Close))
            }
            Message::Activate => {
                let renaming = self
                    .categories_sheet
                    .as_ref()
                    .is_some_and(|sheet| sheet.renaming.is_some());
                Some(self.update_categories(if renaming {
                    CategoriesMessage::RenameDone
                } else {
                    CategoriesMessage::Create
                }))
            }
            Message::Move(_)
            | Message::Type(_)
            | Message::Backspace
            | Message::Play
            | Message::Navigate(_)
            | Message::Clicked(_)
            | Message::ToggleHidden
            | Message::TogglePinned
            | Message::ShowHidden
            | Message::CycleSort
            | Message::FocusSearch
            | Message::PasteClipboard
            | Message::Pad(_) => Some(Task::none()),
            _ => None,
        }
    }

    /// The sheet over the page, when it is open.
    pub(super) fn with_categories<'a>(
        &'a self,
        page: Element<'a, Message>,
    ) -> Element<'a, Message> {
        match &self.categories_sheet {
            Some(sheet) => overlay(page, categories_card(sheet, &self.categories)),
            None => page,
        }
    }
}

/// Whether every game the sheet was opened for is in the category, which is
/// what its checkbox shows.
fn member(sheet: &Categories, category: &Category) -> bool {
    !sheet.targets.is_empty() && sheet.targets.iter().all(|id| category.games.contains(id))
}

fn categories_card<'a>(sheet: &'a Categories, categories: &'a [Category]) -> Element<'a, Message> {
    let mut body = column![
        text(t!("categories-title")).size(20).font(BOLD),
        text(t!("categories-count", count = categories.len()))
            .size(13)
            .color(colors().dim),
    ]
    .spacing(14);
    if categories.is_empty() {
        body = body.push(text(t!("categories-empty")).size(13).color(colors().dim));
    }
    for category in categories {
        body = body.push(category_row(sheet, category));
    }
    body = body.push(
        row![
            text_input(&t!("categories-new-placeholder"), &sheet.draft)
                .on_input(|text| Message::Categories(CategoriesMessage::Typed(text)))
                .on_submit(Message::Categories(CategoriesMessage::Create))
                .size(14)
                .padding([8, 12])
                .style(style::input),
            button(text(t!("categories-new")).size(14).font(BOLD))
                .padding([10, 20])
                .style(style::primary)
                .on_press(Message::Categories(CategoriesMessage::Create)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    );
    if let Some((note, worked)) = &sheet.note {
        body = body.push(text(note).size(13).color(if *worked {
            colors().good
        } else {
            colors().accent
        }));
    }
    body = body.push(
        row![
            space::horizontal(),
            button(text(t!("categories-cancel")).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Categories(CategoriesMessage::Close)),
        ]
        .spacing(10),
    );
    container(scrollable(body.padding(24)))
        .max_width(620)
        .style(style::card)
        .into()
}

/// One category in the sheet: its name, its games, and what may be done to
/// it, or its rename in place of the name.
fn category_row<'a>(sheet: &'a Categories, category: &'a Category) -> Element<'a, Message> {
    let name = category.name.clone();
    let renaming = sheet.renaming.as_deref() == Some(name.as_str());
    let mut items: Vec<Element<'a, Message>> = Vec::new();
    if !sheet.targets.is_empty() {
        let member = member(sheet, category);
        let ticked = name.clone();
        items.push(
            checkbox(member)
                .size(16)
                .style(style::checkbox)
                .on_toggle(move |member| {
                    Message::Categories(CategoriesMessage::SetMembership {
                        name: ticked.clone(),
                        member,
                    })
                })
                .into(),
        );
    }
    if renaming {
        items.push(
            text_input(&name, &sheet.rename_draft)
                .on_input(|text| Message::Categories(CategoriesMessage::RenameTyped(text)))
                .on_submit(Message::Categories(CategoriesMessage::RenameDone))
                .size(14)
                .padding([8, 12])
                .style(style::input)
                .width(Length::Fill)
                .into(),
        );
        items.push(
            button(text(t!("categories-ok")).size(13))
                .padding([6, 14])
                .style(style::primary)
                .on_press(Message::Categories(CategoriesMessage::RenameDone))
                .into(),
        );
    } else {
        items.push(text(&category.name).size(15).width(Length::Fill).into());
        items.push(
            text(t!("count-games", count = category.games.len()))
                .size(12)
                .color(colors().dim)
                .into(),
        );
        items.push(
            button(text(t!("categories-rename")).size(13))
                .padding([6, 14])
                .style(style::secondary)
                .on_press(Message::Categories(CategoriesMessage::Rename(name.clone())))
                .into(),
        );
    }
    let confirming = sheet.confirm_delete.as_deref() == Some(name.as_str());
    items.push(
        button(
            text(if confirming {
                t!("categories-really-delete")
            } else {
                t!("categories-delete")
            })
            .size(13),
        )
        .padding([6, 14])
        .style(if confirming {
            style::danger
        } else {
            style::secondary
        })
        .on_press(Message::Categories(CategoriesMessage::Delete(name)))
        .into(),
    );
    row(items).spacing(10).align_y(Alignment::Center).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(name: &str, games: &[&str]) -> Category {
        Category {
            name: name.to_owned(),
            games: games.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    fn sheet(targets: &[&str]) -> Categories {
        Categories {
            targets: targets.iter().map(|id| (*id).to_owned()).collect(),
            ..Categories::default()
        }
    }

    #[test]
    fn deletes_a_category_only_on_the_second_press() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.categories = vec![category("Co-op", &["a"])];
        let _ = zendo.update_categories(CategoriesMessage::Open(BTreeSet::new()));
        let _ = zendo.update_categories(CategoriesMessage::Delete("Co-op".into()));
        assert_eq!(zendo.categories.len(), 1, "the first press only asks");
        let _ = zendo.update_categories(CategoriesMessage::Delete("Co-op".into()));
        assert_eq!(zendo.categories.len(), 0);
    }

    #[test]
    fn ticks_only_when_every_target_is_in_the_category() {
        let co_op = category("Co-op", &["a"]);
        assert!(!member(&sheet(&[]), &co_op), "no targets, nothing to tick");
        assert!(!member(&sheet(&["a", "b"]), &co_op), "one of the two");
        assert!(member(&sheet(&["a"]), &co_op));
    }
}
