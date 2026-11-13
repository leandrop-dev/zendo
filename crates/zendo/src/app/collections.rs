//! The Collections sheet. A collection is either hand-picked (the games the
//! user ticked: a category) or a saved view (what the library shows, kept
//! under a name: a saved filter). Both live in one list, one set of names,
//! and are made, renamed and deleted here; the games the sheet was opened
//! for are put in or taken out of the hand-picked ones.

use std::collections::BTreeSet;

use iced::widget::{button, checkbox, column, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::{Category, Filter, SavedFilter, State};

use super::launch_options::{frame, overlay};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, format};
use crate::i18n::t;

/// The Collections sheet: the user's collections, and the games it assigns
/// when it was opened from a game's page or a selection in the grid.
#[derive(Debug, Clone, Default)]
pub struct Collections {
    /// The games the checkboxes assign. Empty when the sheet only manages.
    pub(super) targets: BTreeSet<String>,
    /// The name being typed, for a new collection or a saved view.
    pub(super) draft: String,
    /// The collection being renamed, and what is typed in its field.
    pub(super) renaming: Option<String>,
    pub(super) rename_draft: String,
    /// The collection whose Delete was pressed once: pressed again, it goes.
    pub(super) confirm_delete: Option<String>,
    /// What the last action did, and whether it worked.
    pub(super) note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum CollectionsMessage {
    /// Open the sheet, assigning to these games (empty: manage only).
    Open(BTreeSet<String>),
    Close,
    Typed(String),
    /// Make an empty hand-picked collection with the typed name, and put the
    /// sheet's games in it.
    Create,
    /// Keep what the library shows now under the typed name.
    SaveView,
    Rename(String),
    RenameTyped(String),
    RenameDone,
    /// Tick or untick the collection for every game in `targets`.
    SetMembership {
        name: String,
        member: bool,
    },
    Delete(String),
}

impl Zendo {
    /// What the library shows now, as a saved view would hold it.
    pub(super) fn current_filter(&self) -> Filter {
        Filter {
            view: self.view,
            source: self.source,
            category: self.category.clone(),
            status: self.play_status,
            search: self.search.clone(),
            sort: self.sort,
        }
    }

    /// Puts a saved view on the library. A store with no games, or a
    /// collection that is gone, is left off rather than showing nothing.
    pub(super) fn apply_filter(&mut self, name: &str) -> Task<Message> {
        let Some(saved) = self.filters.iter().find(|saved| saved.name == name) else {
            return Task::none();
        };
        let mut filter = saved.filter.clone();
        if let Some(source) = filter.source
            && !self
                .library
                .iter()
                .flat_map(|library| &library.games)
                .any(|game| game.source == source)
        {
            filter.source = None;
        }
        if let Some(category) = &filter.category
            && !self.categories.iter().any(|known| &known.name == category)
        {
            filter.category = None;
        }
        self.collections_sheet = None;
        self.page = super::Page::Library;
        self.save(|state| state.sort = filter.sort);
        self.filter(|zendo| {
            zendo.view = filter.view;
            zendo.source = filter.source;
            zendo.category = filter.category;
            zendo.play_status = filter.status;
            zendo.search = filter.search;
            zendo.sort = filter.sort;
        })
    }

    pub(super) fn update_collections(&mut self, message: CollectionsMessage) -> Task<Message> {
        // Any other press takes back a delete that was only asking.
        if !matches!(message, CollectionsMessage::Delete(_))
            && let Some(sheet) = self.collections_sheet.as_mut()
        {
            sheet.confirm_delete = None;
        }
        match message {
            CollectionsMessage::Open(targets) => self.open_collections(targets),
            CollectionsMessage::Close => self.collections_sheet = None,
            CollectionsMessage::Typed(text) => {
                if let Some(sheet) = self.collections_sheet.as_mut() {
                    sheet.draft = text;
                }
            }
            CollectionsMessage::RenameTyped(text) => {
                if let Some(sheet) = self.collections_sheet.as_mut() {
                    sheet.rename_draft = text;
                }
            }
            CollectionsMessage::Create => self.create_collection(),
            CollectionsMessage::SaveView => self.save_view(),
            CollectionsMessage::Rename(name) => {
                if let Some(sheet) = self.collections_sheet.as_mut() {
                    sheet.renaming = Some(name.clone());
                    sheet.rename_draft = name;
                    sheet.note = None;
                }
            }
            CollectionsMessage::RenameDone => self.rename_collection(),
            CollectionsMessage::SetMembership { name, member } => {
                self.set_membership(&name, member);
            }
            CollectionsMessage::Delete(name) => self.delete_collection(&name),
        }
        Task::none()
    }

    /// Whether the name is a saved view, not a hand-picked collection.
    fn is_view(&self, name: &str) -> bool {
        self.filters.iter().any(|saved| saved.name == name)
    }

    /// Opens the sheet, its list as the state has it now: another program
    /// (`zendo categorize`) may have changed it since the library loaded.
    fn open_collections(&mut self, targets: BTreeSet<String>) {
        if self.persist {
            match State::load() {
                Ok(state) => self.reload_collections(&state),
                Err(err) => self.status = Some(format!("{err:#}")),
            }
        }
        self.collections_sheet = Some(Collections {
            targets,
            ..Collections::default()
        });
    }

    /// Makes the hand-picked collection in the draft, and puts the sheet's
    /// games in it.
    fn create_collection(&mut self) {
        let Some(sheet) = self.collections_sheet.as_ref() else {
            return;
        };
        let draft = sheet.draft.clone();
        let targets: Vec<String> = sheet.targets.iter().cloned().collect();
        if draft.trim().is_empty() {
            self.say(t!("collections-blank"), false);
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
        if let Some(sheet) = self.collections_sheet.as_mut() {
            sheet.note = if created {
                sheet.draft.clear();
                Some((t!("collections-created", name = draft.trim()), true))
            } else {
                Some((t!("collections-taken", name = draft.trim()), false))
            };
        }
    }

    /// Keeps what the library shows under the draft's name: a new saved
    /// view, or the update of the one with that name.
    fn save_view(&mut self) {
        let Some(sheet) = self.collections_sheet.as_ref() else {
            return;
        };
        let name = sheet.draft.trim().to_owned();
        if name.is_empty() {
            self.say(t!("collections-blank"), false);
            return;
        }
        let existed = self
            .filters
            .iter()
            .any(|saved| saved.name.to_lowercase() == name.to_lowercase());
        let filter = self.current_filter();
        let mut saved = false;
        if !self.change_state(|state| saved = state.save_filter(&name, filter)) {
            return;
        }
        if !saved {
            // Only a hand-picked collection with that name stops it.
            self.say(t!("collections-taken", name = name), false);
            return;
        }
        if let Some(sheet) = self.collections_sheet.as_mut() {
            sheet.draft.clear();
        }
        self.say(
            if existed {
                t!("collections-updated", name = name)
            } else {
                t!("collections-saved", name = name)
            },
            true,
        );
    }

    fn rename_collection(&mut self) {
        let Some(sheet) = self.collections_sheet.as_ref() else {
            return;
        };
        let Some(from) = sheet.renaming.clone() else {
            return;
        };
        let to = sheet.rename_draft.trim().to_owned();
        if to.is_empty() {
            self.say(t!("collections-blank"), false);
            return;
        }
        let view = self.is_view(&from);
        let mut renamed = false;
        if !self.change_state(|state| {
            renamed = if view {
                state.rename_filter(&from, &to)
            } else {
                state.rename_category(&from, &to)
            };
        }) {
            return;
        }
        if let Some(sheet) = self.collections_sheet.as_mut() {
            if renamed {
                sheet.renaming = None;
                sheet.rename_draft.clear();
            }
            sheet.note = Some(if renamed {
                (t!("collections-renamed", name = to), true)
            } else {
                (t!("collections-taken", name = to), false)
            });
        }
    }

    /// Ticks or unticks a hand-picked collection for every game the sheet
    /// was opened for.
    fn set_membership(&mut self, name: &str, member: bool) {
        let Some(sheet) = self.collections_sheet.as_ref() else {
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
        if let Some(sheet) = self.collections_sheet.as_mut() {
            sheet.note = Some(if member {
                (t!("collections-assigned", count = count), true)
            } else {
                (t!("collections-unassigned", count = count), true)
            });
        }
    }

    /// Says what the last action did, in the sheet.
    fn say(&mut self, note: String, worked: bool) {
        if let Some(sheet) = self.collections_sheet.as_mut() {
            sheet.note = Some((note, worked));
        }
    }

    /// Delete, pressed once to ask and again to drop the collection (a
    /// hand-picked one with its games, as removing a game added by hand
    /// does; a saved view with what it kept).
    fn delete_collection(&mut self, name: &str) {
        let Some(sheet) = self.collections_sheet.as_mut() else {
            return;
        };
        if sheet.confirm_delete.as_deref() != Some(name) {
            sheet.confirm_delete = Some(name.to_owned());
            sheet.note = None;
            return;
        }
        sheet.confirm_delete = None;
        let view = self.is_view(name);
        let mut deleted = false;
        if !self.change_state(|state| {
            deleted = if view {
                state.delete_filter(name)
            } else {
                state.delete_category(name)
            };
        }) {
            return;
        }
        let Some(sheet) = self.collections_sheet.as_mut() else {
            return;
        };
        if deleted {
            sheet.note = Some((t!("collections-deleted", name = name.trim()), true));
        }
        if sheet.renaming.as_deref() == Some(name) {
            sheet.renaming = None;
            sheet.rename_draft.clear();
        }
    }

    /// Changes the state, and the window with it: the sheet's list and each
    /// game's collections. False when the state could not be written.
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
            let mut state = self.collections_state();
            change(&mut state);
            state
        };
        self.reload_collections(&state);
        true
    }

    /// The window's collections as a state of their own, so that a change
    /// can be made without reading the user's file.
    fn collections_state(&self) -> State {
        let mut state = State::default();
        for category in &self.categories {
            state.add_category(&category.name);
            for id in &category.games {
                state.set_category(&category.name, id, true);
            }
        }
        // A rename or delete of a category reaches the views naming it.
        for saved in &self.filters {
            state.save_filter(&saved.name, saved.filter.clone());
        }
        state
    }

    /// Puts a state's collections on the sheet's list and on every game.
    fn reload_collections(&mut self, state: &State) {
        self.categories = state.categories();
        self.filters = state.filters();
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
    /// makes or renames a collection, and the page's own keys and the
    /// controller do nothing.
    pub(super) fn collections_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.collections_sheet.as_ref()?;
        match message {
            Message::Back => {
                if let Some(sheet) = self.collections_sheet.as_mut()
                    && sheet.renaming.take().is_some()
                {
                    sheet.rename_draft.clear();
                    return Some(Task::none());
                }
                Some(self.update_collections(CollectionsMessage::Close))
            }
            Message::Activate => {
                let renaming = self
                    .collections_sheet
                    .as_ref()
                    .is_some_and(|sheet| sheet.renaming.is_some());
                Some(self.update_collections(if renaming {
                    CollectionsMessage::RenameDone
                } else {
                    CollectionsMessage::Create
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
    pub(super) fn with_collections<'a>(
        &'a self,
        page: Element<'a, Message>,
    ) -> Element<'a, Message> {
        match &self.collections_sheet {
            Some(sheet) => overlay(
                page,
                collections_card(
                    sheet,
                    &self.categories,
                    &self.filters,
                    &self.current_filter(),
                ),
            ),
            None => page,
        }
    }
}

/// One collection of either kind, for the list.
enum Item<'a> {
    Picked(&'a Category),
    View(&'a SavedFilter),
}

impl Item<'_> {
    fn name(&self) -> &str {
        match self {
            Self::Picked(category) => &category.name,
            Self::View(saved) => &saved.name,
        }
    }
}

/// Every collection, hand-picked and saved views together, by name.
fn items<'a>(categories: &'a [Category], filters: &'a [SavedFilter]) -> Vec<Item<'a>> {
    let mut items: Vec<Item<'a>> = categories
        .iter()
        .map(Item::Picked)
        .chain(filters.iter().map(Item::View))
        .collect();
    items.sort_by_cached_key(|item| (item.name().to_lowercase(), item.name().to_owned()));
    items
}

/// Whether every game the sheet was opened for is in the collection, which
/// is what its checkbox shows.
fn member(sheet: &Collections, category: &Category) -> bool {
    !sheet.targets.is_empty() && sheet.targets.iter().all(|id| category.games.contains(id))
}

/// A saved view in a few words: `FAVORITES · STEAM · BACKLOG · "doom"`.
fn summary(filter: &Filter) -> String {
    let mut parts = vec![format::view(filter.view).to_uppercase()];
    if let Some(source) = filter.source {
        parts.push(format::source(source).to_uppercase());
    }
    if let Some(category) = &filter.category {
        parts.push(category.to_uppercase());
    }
    if let Some(status) = filter.status {
        parts.push(format::play_status(status).to_uppercase());
    }
    if !filter.search.trim().is_empty() {
        parts.push(format!("\u{201c}{}\u{201d}", filter.search.trim()));
    }
    parts.push(format::sort(filter.sort).to_uppercase());
    parts.join(" · ")
}

fn collections_card<'a>(
    sheet: &'a Collections,
    categories: &'a [Category],
    filters: &'a [SavedFilter],
    now: &Filter,
) -> Element<'a, Message> {
    let mut body = column![
        text(t!(
            "collections-count",
            count = categories.len() + filters.len()
        ))
        .size(13)
        .color(colors().dim),
    ]
    .spacing(style::SPACE_L);
    if categories.is_empty() && filters.is_empty() {
        body = body.push(text(t!("collections-empty")).size(13).color(colors().dim));
    }
    for item in items(categories, filters) {
        body = body.push(match item {
            Item::Picked(category) => picked_row(sheet, category),
            Item::View(saved) => view_row(sheet, saved),
        });
    }
    body = body.push(
        column![
            text(t!("collections-now", summary = summary(now)))
                .size(12)
                .color(colors().dim),
            row![
                text_input(&t!("collections-new-placeholder"), &sheet.draft)
                    .on_input(|text| Message::Collections(CollectionsMessage::Typed(text)))
                    .on_submit(Message::Collections(CollectionsMessage::Create))
                    .size(14)
                    .padding([style::SPACE_S, style::SPACE_M])
                    .style(style::input),
                button(style::button_text(t!("collections-new")).font(BOLD))
                    .padding(style::BUTTON_LARGE)
                    .style(style::primary)
                    .on_press(Message::Collections(CollectionsMessage::Create)),
                button(style::button_text(t!("collections-save-view")))
                    .padding(style::BUTTON_LARGE)
                    .style(style::secondary)
                    .on_press(Message::Collections(CollectionsMessage::SaveView)),
            ]
            .spacing(style::SPACE_M)
            .align_y(Alignment::Center),
        ]
        .spacing(style::SPACE_S),
    );
    if let Some((note, worked)) = &sheet.note {
        body = body.push(text(note).size(13).color(if *worked {
            colors().good
        } else {
            colors().accent
        }));
    }
    let footer = row![
        space::horizontal(),
        button(style::button_text(t!("collections-cancel")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Collections(CollectionsMessage::Close)),
    ];
    frame(&t!("collections-title"), body, footer, 620.0, false)
}

/// The rename field and its OK, in place of a collection's name.
fn rename_items<'a>(sheet: &'a Collections, name: &str) -> Vec<Element<'a, Message>> {
    vec![
        text_input(name, &sheet.rename_draft)
            .on_input(|text| Message::Collections(CollectionsMessage::RenameTyped(text)))
            .on_submit(Message::Collections(CollectionsMessage::RenameDone))
            .size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::input)
            .width(Length::Fill)
            .into(),
        button(style::button_text(t!("collections-ok")))
            .padding([6, 14])
            .style(style::primary)
            .on_press(Message::Collections(CollectionsMessage::RenameDone))
            .into(),
    ]
}

/// The Rename button of a collection.
fn rename_button<'a>(name: &str) -> Element<'a, Message> {
    button(style::button_text(t!("collections-rename")))
        .padding([6, 14])
        .style(style::secondary)
        .on_press(Message::Collections(CollectionsMessage::Rename(
            name.to_owned(),
        )))
        .into()
}

/// The Delete button of a collection: it asks first.
fn delete_button<'a>(sheet: &Collections, name: &str) -> Element<'a, Message> {
    let confirming = sheet.confirm_delete.as_deref() == Some(name);
    button(style::button_text(if confirming {
        t!("collections-really-delete")
    } else {
        t!("collections-delete")
    }))
    .padding([6, 14])
    .style(if confirming {
        style::danger
    } else {
        style::secondary
    })
    .on_press(Message::Collections(CollectionsMessage::Delete(
        name.to_owned(),
    )))
    .into()
}

/// One hand-picked collection in the sheet: its name, its games, and what
/// may be done to it, or its rename in place of the name.
fn picked_row<'a>(sheet: &'a Collections, category: &'a Category) -> Element<'a, Message> {
    let name = category.name.clone();
    let mut items: Vec<Element<'a, Message>> = Vec::new();
    if !sheet.targets.is_empty() {
        let member = member(sheet, category);
        let ticked = name.clone();
        items.push(
            checkbox(member)
                .size(16)
                .style(style::checkbox)
                .on_toggle(move |member| {
                    Message::Collections(CollectionsMessage::SetMembership {
                        name: ticked.clone(),
                        member,
                    })
                })
                .into(),
        );
    }
    if sheet.renaming.as_deref() == Some(name.as_str()) {
        items.extend(rename_items(sheet, &name));
    } else {
        items.push(text(&category.name).size(15).width(Length::Fill).into());
        items.push(
            text(t!("count-games", count = category.games.len()))
                .size(12)
                .color(colors().dim)
                .into(),
        );
        items.push(rename_button(&name));
    }
    items.push(delete_button(sheet, &name));
    row(items)
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
}

/// One saved view in the sheet: its name and what it holds, or its rename in
/// place of them, and what may be done to it.
fn view_row<'a>(sheet: &'a Collections, saved: &'a SavedFilter) -> Element<'a, Message> {
    let name = saved.name.clone();
    let mut items: Vec<Element<'a, Message>> = Vec::new();
    if sheet.renaming.as_deref() == Some(name.as_str()) {
        items.extend(rename_items(sheet, &name));
    } else {
        items.push(
            column![
                text(&saved.name).size(15),
                text(summary(&saved.filter)).size(11).color(colors().dim),
            ]
            .width(Length::Fill)
            .into(),
        );
        items.push(
            button(style::button_text(t!("collections-apply")))
                .padding([6, 14])
                .style(style::primary)
                .on_press(Message::ApplyFilter(name.clone()))
                .into(),
        );
        items.push(rename_button(&name));
    }
    items.push(delete_button(sheet, &name));
    row(items)
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
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

    fn view(name: &str) -> SavedFilter {
        SavedFilter {
            name: name.to_owned(),
            filter: Filter::default(),
        }
    }

    fn sheet(targets: &[&str]) -> Collections {
        Collections {
            targets: targets.iter().map(|id| (*id).to_owned()).collect(),
            ..Collections::default()
        }
    }

    #[test]
    fn deletes_a_collection_only_on_the_second_press() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.categories = vec![category("Co-op", &["a"])];
        let _ = zendo.update_collections(CollectionsMessage::Open(BTreeSet::new()));
        let _ = zendo.update_collections(CollectionsMessage::Delete("Co-op".into()));
        assert_eq!(zendo.categories.len(), 1, "the first press only asks");
        let _ = zendo.update_collections(CollectionsMessage::Delete("Co-op".into()));
        assert_eq!(zendo.categories.len(), 0);
    }

    #[test]
    fn ticks_only_when_every_target_is_in_the_collection() {
        let co_op = category("Co-op", &["a"]);
        assert!(!member(&sheet(&[]), &co_op), "no targets, nothing to tick");
        assert!(!member(&sheet(&["a", "b"]), &co_op), "one of the two");
        assert!(member(&sheet(&["a"]), &co_op));
    }

    #[test]
    fn lists_both_kinds_together_by_name() {
        let categories = [category("beta", &[]), category("Delta", &[])];
        let views = [view("Alpha"), view("charlie")];
        let names: Vec<String> = items(&categories, &views)
            .iter()
            .map(|item| item.name().to_owned())
            .collect();
        assert_eq!(names, ["Alpha", "beta", "charlie", "Delta"]);
    }

    #[test]
    fn a_saved_view_cannot_take_a_hand_picked_name_and_the_reverse() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update_collections(CollectionsMessage::Open(BTreeSet::new()));
        let typed = |zendo: &mut Zendo, text: &str| {
            let _ = zendo.update_collections(CollectionsMessage::Typed(text.into()));
        };
        typed(&mut zendo, "Co-op");
        let _ = zendo.update_collections(CollectionsMessage::Create);
        typed(&mut zendo, "co-op");
        let _ = zendo.update_collections(CollectionsMessage::SaveView);
        assert!(zendo.filters.is_empty(), "the name is a collection's");
        assert_eq!(
            zendo
                .collections_sheet
                .as_ref()
                .and_then(|s| s.note.clone()),
            Some((t!("collections-taken", name = "co-op"), false))
        );
        typed(&mut zendo, "Evening");
        let _ = zendo.update_collections(CollectionsMessage::SaveView);
        assert_eq!(zendo.filters.len(), 1);
        typed(&mut zendo, "evening");
        let _ = zendo.update_collections(CollectionsMessage::Create);
        assert_eq!(zendo.categories.len(), 1, "the name is a view's");
    }

    #[test]
    fn a_saved_view_is_renamed_updated_and_deleted_from_the_same_sheet() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update_collections(CollectionsMessage::Open(BTreeSet::new()));
        let _ = zendo.update_collections(CollectionsMessage::SaveView);
        assert!(zendo.filters.is_empty(), "a view needs a name");
        let _ = zendo.update_collections(CollectionsMessage::Typed("Evening".into()));
        let _ = zendo.update_collections(CollectionsMessage::SaveView);
        assert_eq!(zendo.filters[0].name, "Evening");
        zendo.search = "doom".into();
        let _ = zendo.update_collections(CollectionsMessage::Typed("evening".into()));
        let _ = zendo.update_collections(CollectionsMessage::SaveView);
        assert_eq!(zendo.filters.len(), 1, "the same name updates");
        assert_eq!(zendo.filters[0].filter.search, "doom");
        let _ = zendo.update_collections(CollectionsMessage::Rename("Evening".into()));
        let _ = zendo.update_collections(CollectionsMessage::RenameTyped("Night".into()));
        let _ = zendo.update_collections(CollectionsMessage::RenameDone);
        assert_eq!(zendo.filters[0].name, "Night");
        let _ = zendo.update_collections(CollectionsMessage::Delete("Night".into()));
        assert_eq!(zendo.filters.len(), 1, "the first press only asks");
        let _ = zendo.update_collections(CollectionsMessage::Delete("Night".into()));
        assert_eq!(zendo.filters.len(), 0);
    }
}
