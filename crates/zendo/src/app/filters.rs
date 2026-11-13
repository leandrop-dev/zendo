//! The Saved filters sheet: what the library shows (view, store, category,
//! status, search words and order) kept under a name, applied from a chip
//! in the Organize row or from here.

use iced::widget::{button, column, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::{Filter, SavedFilter, State};

use super::launch_options::{frame, overlay};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, format};
use crate::i18n::t;

/// The Saved filters sheet.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// The name being typed for the filter on screen.
    pub(super) draft: String,
    /// The filter being renamed, and what is typed in its field.
    pub(super) renaming: Option<String>,
    pub(super) rename_draft: String,
    /// The filter whose Delete was pressed once: pressed again, it goes.
    pub(super) confirm_delete: Option<String>,
    /// What the last action did, and whether it worked.
    pub(super) note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum FiltersMessage {
    Open,
    Close,
    Typed(String),
    /// Keep what the library shows now under the typed name.
    Save,
    Rename(String),
    RenameTyped(String),
    RenameDone,
    Delete(String),
}

impl Zendo {
    /// What the library shows now, as a saved filter would hold it.
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

    /// Puts a saved filter on the library. A store with no games, or a
    /// category that is gone, is left off rather than showing nothing.
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
        self.filters_sheet = None;
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

    pub(super) fn update_filters(&mut self, message: FiltersMessage) -> Task<Message> {
        // Any other press takes back a delete that was only asking.
        if !matches!(message, FiltersMessage::Delete(_))
            && let Some(sheet) = self.filters_sheet.as_mut()
        {
            sheet.confirm_delete = None;
        }
        match message {
            FiltersMessage::Open => self.open_filters(),
            FiltersMessage::Close => self.filters_sheet = None,
            FiltersMessage::Typed(text) => {
                if let Some(sheet) = self.filters_sheet.as_mut() {
                    sheet.draft = text;
                }
            }
            FiltersMessage::RenameTyped(text) => {
                if let Some(sheet) = self.filters_sheet.as_mut() {
                    sheet.rename_draft = text;
                }
            }
            FiltersMessage::Save => self.save_filter(),
            FiltersMessage::Rename(name) => {
                if let Some(sheet) = self.filters_sheet.as_mut() {
                    sheet.renaming = Some(name.clone());
                    sheet.rename_draft = name;
                    sheet.note = None;
                }
            }
            FiltersMessage::RenameDone => self.rename_filter(),
            FiltersMessage::Delete(name) => self.delete_filter(&name),
        }
        Task::none()
    }

    /// Opens the sheet, its list as the state has it now.
    fn open_filters(&mut self) {
        if self.persist {
            match State::load() {
                Ok(state) => self.filters = state.filters(),
                Err(err) => self.status = Some(format!("{err:#}")),
            }
        }
        self.filters_sheet = Some(Filters::default());
    }

    fn save_filter(&mut self) {
        let Some(sheet) = self.filters_sheet.as_ref() else {
            return;
        };
        let name = sheet.draft.trim().to_owned();
        if name.is_empty() {
            self.say_filters(t!("filters-blank"), false);
            return;
        }
        let existed = self
            .filters
            .iter()
            .any(|saved| saved.name.to_lowercase() == name.to_lowercase());
        let filter = self.current_filter();
        if !self.change_filters(|state| {
            state.save_filter(&name, filter);
        }) {
            return;
        }
        if let Some(sheet) = self.filters_sheet.as_mut() {
            sheet.draft.clear();
        }
        self.say_filters(
            if existed {
                t!("filters-updated", name = name)
            } else {
                t!("filters-saved", name = name)
            },
            true,
        );
    }

    fn rename_filter(&mut self) {
        let Some(sheet) = self.filters_sheet.as_ref() else {
            return;
        };
        let Some(from) = sheet.renaming.clone() else {
            return;
        };
        let to = sheet.rename_draft.trim().to_owned();
        if to.is_empty() {
            self.say_filters(t!("filters-blank"), false);
            return;
        }
        let mut renamed = false;
        if !self.change_filters(|state| renamed = state.rename_filter(&from, &to)) {
            return;
        }
        if renamed {
            if let Some(sheet) = self.filters_sheet.as_mut() {
                sheet.renaming = None;
                sheet.rename_draft.clear();
            }
            self.say_filters(t!("filters-renamed", name = to), true);
        } else {
            self.say_filters(t!("filters-taken", name = to), false);
        }
    }

    /// Delete, pressed once to ask and again to drop the filter.
    fn delete_filter(&mut self, name: &str) {
        let Some(sheet) = self.filters_sheet.as_mut() else {
            return;
        };
        if sheet.confirm_delete.as_deref() != Some(name) {
            sheet.confirm_delete = Some(name.to_owned());
            sheet.note = None;
            return;
        }
        sheet.confirm_delete = None;
        if sheet.renaming.as_deref() == Some(name) {
            sheet.renaming = None;
            sheet.rename_draft.clear();
        }
        if self.change_filters(|state| {
            state.delete_filter(name);
        }) {
            self.say_filters(t!("filters-deleted", name = name), true);
        }
    }

    /// Says what the last action did, in the sheet.
    fn say_filters(&mut self, note: String, worked: bool) {
        if let Some(sheet) = self.filters_sheet.as_mut() {
            sheet.note = Some((note, worked));
        }
    }

    /// Changes the state, and the window's list with it. False when the
    /// state could not be written.
    fn change_filters(&mut self, change: impl FnOnce(&mut State)) -> bool {
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
            let mut state = State::default();
            for saved in &self.filters {
                state.save_filter(&saved.name, saved.filter.clone());
            }
            change(&mut state);
            state
        };
        self.filters = state.filters();
        true
    }

    /// While the sheet is open, Escape closes it (or leaves a rename),
    /// Enter saves or renames, and the page's own keys do nothing.
    pub(super) fn filters_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.filters_sheet.as_ref()?;
        match message {
            Message::Back => {
                if let Some(sheet) = self.filters_sheet.as_mut()
                    && sheet.renaming.take().is_some()
                {
                    sheet.rename_draft.clear();
                    return Some(Task::none());
                }
                Some(self.update_filters(FiltersMessage::Close))
            }
            Message::Activate => {
                let renaming = self
                    .filters_sheet
                    .as_ref()
                    .is_some_and(|sheet| sheet.renaming.is_some());
                Some(self.update_filters(if renaming {
                    FiltersMessage::RenameDone
                } else {
                    FiltersMessage::Save
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
    pub(super) fn with_filters<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.filters_sheet {
            Some(sheet) => overlay(
                page,
                filters_card(sheet, &self.filters, &self.current_filter()),
            ),
            None => page,
        }
    }
}

/// A filter in a few words: `FAVORITES · STEAM · BACKLOG · "doom"`.
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

fn filters_card<'a>(
    sheet: &'a Filters,
    filters: &'a [SavedFilter],
    now: &Filter,
) -> Element<'a, Message> {
    let mut body = column![
        text(t!("filters-count", count = filters.len()))
            .size(13)
            .color(colors().dim),
    ]
    .spacing(style::SPACE_L);
    if filters.is_empty() {
        body = body.push(text(t!("filters-empty")).size(13).color(colors().dim));
    }
    for saved in filters {
        body = body.push(filter_row(sheet, saved));
    }
    body = body.push(
        column![
            text(t!("filters-now", summary = summary(now)))
                .size(12)
                .color(colors().dim),
            row![
                text_input(&t!("filters-new-placeholder"), &sheet.draft)
                    .on_input(|text| Message::Filters(FiltersMessage::Typed(text)))
                    .on_submit(Message::Filters(FiltersMessage::Save))
                    .size(14)
                    .padding([style::SPACE_S, style::SPACE_M])
                    .style(style::input),
                button(style::button_text(t!("filters-save")).font(BOLD))
                    .padding(style::BUTTON_LARGE)
                    .style(style::primary)
                    .on_press(Message::Filters(FiltersMessage::Save)),
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
        button(style::button_text(t!("categories-cancel")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Filters(FiltersMessage::Close)),
    ];
    frame(&t!("filters-title"), body, footer, 620.0, false)
}

/// One saved filter in the sheet: its name and what it holds, or its
/// rename in place of them, and what may be done to it.
fn filter_row<'a>(sheet: &'a Filters, saved: &'a SavedFilter) -> Element<'a, Message> {
    let name = saved.name.clone();
    let renaming = sheet.renaming.as_deref() == Some(name.as_str());
    let mut items: Vec<Element<'a, Message>> = Vec::new();
    if renaming {
        items.push(
            text_input(&name, &sheet.rename_draft)
                .on_input(|text| Message::Filters(FiltersMessage::RenameTyped(text)))
                .on_submit(Message::Filters(FiltersMessage::RenameDone))
                .size(14)
                .padding([style::SPACE_S, style::SPACE_M])
                .style(style::input)
                .width(Length::Fill)
                .into(),
        );
        items.push(
            button(style::button_text(t!("categories-ok")))
                .padding([6, 14])
                .style(style::primary)
                .on_press(Message::Filters(FiltersMessage::RenameDone))
                .into(),
        );
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
            button(style::button_text(t!("filters-apply")))
                .padding([6, 14])
                .style(style::primary)
                .on_press(Message::ApplyFilter(name.clone()))
                .into(),
        );
        items.push(
            button(style::button_text(t!("categories-rename")))
                .padding([6, 14])
                .style(style::secondary)
                .on_press(Message::Filters(FiltersMessage::Rename(name.clone())))
                .into(),
        );
    }
    let confirming = sheet.confirm_delete.as_deref() == Some(name.as_str());
    items.push(
        button(style::button_text(if confirming {
            t!("categories-really-delete")
        } else {
            t!("categories-delete")
        }))
        .padding([6, 14])
        .style(if confirming {
            style::danger
        } else {
            style::secondary
        })
        .on_press(Message::Filters(FiltersMessage::Delete(name)))
        .into(),
    );
    row(items)
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
}
