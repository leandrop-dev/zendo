//! The Collections sheet. A collection is either hand-picked (the games the
//! user ticked: a category) or a saved view (what the library shows, kept
//! under a name: a saved filter). Both live in one list, one set of names,
//! and are made, renamed and deleted here; the games the sheet was opened
//! for are put in or taken out of the hand-picked ones. A part of the
//! Library's glue (`library_ui`), which owns the browser.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::State;
use zendo_shell::collections::{Item, items, member, summary};
use zendo_shell::t;

use super::Glue;
use crate::library::Pick;
use crate::{CollRow, CollectionsData, MainWindow, Page};

/// What the sheet keeps while it is open.
#[derive(Default)]
struct Sheet {
    /// The games the ticks assign. Empty when the sheet only manages.
    targets: BTreeSet<String>,
    /// The name being typed, for a new collection or a saved view.
    draft: String,
    /// The collection being renamed, and what is typed in its field.
    renaming: Option<String>,
    rename_draft: String,
    /// The collection whose Delete was pressed once: pressed again, it goes.
    confirm_delete: Option<String>,
    /// What the last action did, and whether it worked.
    note: Option<(String, bool)>,
}

pub(super) struct CollectionsState {
    sheet: RefCell<Option<Sheet>>,
    rows: Rc<VecModel<CollRow>>,
    focus: Cell<i32>,
}

impl CollectionsState {
    pub(super) fn new() -> Self {
        Self {
            sheet: RefCell::new(None),
            rows: Rc::new(VecModel::default()),
            focus: Cell::new(0),
        }
    }
}

impl Glue {
    /// Hooks the sheet's callbacks up.
    pub(super) fn connect_collections(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<CollectionsData>();
        data.set_rows(ModelRc::from(Rc::clone(&self.collections.rows)));
        let glue = Rc::clone(self);
        data.on_close(move || {
            glue.close_collections();
        });
        let glue = Rc::clone(self);
        data.on_draft_edited(move |text| {
            glue.edit_sheet(|sheet| sheet.draft = text.into());
        });
        let glue = Rc::clone(self);
        data.on_rename_edited(move |text| {
            glue.edit_sheet(|sheet| sheet.rename_draft = text.into());
        });
        let glue = Rc::clone(self);
        data.on_create(move || {
            glue.untake_delete();
            glue.create_collection();
        });
        let glue = Rc::clone(self);
        data.on_save_view(move || {
            glue.untake_delete();
            glue.save_view();
        });
        let glue = Rc::clone(self);
        data.on_rename(move |name| {
            glue.untake_delete();
            glue.edit_sheet(|sheet| {
                sheet.renaming = Some(name.to_string());
                sheet.rename_draft = name.to_string();
                sheet.note = None;
            });
            glue.show_collections();
        });
        let glue = Rc::clone(self);
        data.on_rename_done(move || {
            glue.untake_delete();
            glue.rename_collection();
        });
        let glue = Rc::clone(self);
        data.on_set_member(move |name, member| {
            glue.untake_delete();
            glue.set_membership(&name, member);
        });
        let glue = Rc::clone(self);
        data.on_delete(move |name| glue.delete_collection(&name));
        let glue = Rc::clone(self);
        data.on_apply(move |name| glue.apply_view(&name));
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_collections());
    }

    /// Whether the sheet is open: then the page's keys and the controller do
    /// nothing.
    pub(super) fn collections_open(&self) -> bool {
        self.collections.sheet.borrow().is_some()
    }

    /// Opens the sheet for these games (none: only to manage), its list as
    /// the state has it now: another program (`zendo categorize`) may have
    /// changed it since the library loaded.
    pub(super) fn open_collections(&self, targets: BTreeSet<String>) {
        if self.persist {
            match State::load() {
                Ok(state) => self.reload_collections(&state),
                Err(err) => self.say(format!("{err:#}")),
            }
        }
        *self.collections.sheet.borrow_mut() = Some(Sheet {
            targets,
            ..Sheet::default()
        });
        self.show_collections();
    }

    /// Closes the sheet. Says whether it was open.
    pub(super) fn close_collections(&self) -> bool {
        let was_open = self.collections.sheet.borrow_mut().take().is_some();
        if was_open {
            self.show_collections();
            // The sheet's field had the keyboard and is gone with it.
            self.focus_page();
        }
        was_open
    }

    /// Puts the keyboard in the name field, for when the field that had it (a
    /// rename's) is gone.
    fn focus_draft(&self) {
        let next = self.collections.focus.get() + 1;
        self.collections.focus.set(next);
        if let Some(window) = self.window.upgrade() {
            window.global::<CollectionsData>().set_focus_draft(next);
        }
    }

    /// Escape: a rename ends first, then the sheet closes. Says whether the
    /// sheet was open.
    pub(super) fn collections_back(&self) -> bool {
        let renamed = {
            let mut sheet = self.collections.sheet.borrow_mut();
            let Some(sheet) = sheet.as_mut() else {
                return false;
            };
            let renaming = sheet.renaming.take().is_some();
            if renaming {
                sheet.rename_draft.clear();
            }
            renaming
        };
        if renamed {
            self.show_collections();
            self.focus_draft();
        } else {
            self.close_collections();
        }
        true
    }

    fn edit_sheet(&self, change: impl FnOnce(&mut Sheet)) {
        if let Some(sheet) = self.collections.sheet.borrow_mut().as_mut() {
            change(sheet);
        }
    }

    /// Any other press takes back a delete that was only asking.
    fn untake_delete(&self) {
        self.edit_sheet(|sheet| sheet.confirm_delete = None);
    }

    /// Says what the last action did, in the sheet.
    fn note(&self, note: String, worked: bool) {
        self.edit_sheet(|sheet| sheet.note = Some((note, worked)));
        self.show_collections();
    }

    /// Whether the name is a saved view, not a hand-picked collection.
    fn is_view(&self, name: &str) -> bool {
        self.browser
            .borrow()
            .saved()
            .iter()
            .any(|saved| saved.name == name)
    }

    /// Makes the hand-picked collection in the draft, and puts the sheet's
    /// games in it.
    fn create_collection(&self) {
        let Some((draft, targets)) = self.draft_and_targets() else {
            return;
        };
        if draft.trim().is_empty() {
            self.note(t!("collections-blank"), false);
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
        if created {
            self.edit_sheet(|sheet| sheet.draft.clear());
            self.note(t!("collections-created", name = draft.trim()), true);
        } else {
            self.note(t!("collections-taken", name = draft.trim()), false);
        }
    }

    fn draft_and_targets(&self) -> Option<(String, Vec<String>)> {
        let sheet = self.collections.sheet.borrow();
        let sheet = sheet.as_ref()?;
        Some((sheet.draft.clone(), sheet.targets.iter().cloned().collect()))
    }

    /// Keeps what the library shows under the draft's name: a new saved
    /// view, or the update of the one with that name.
    fn save_view(&self) {
        let Some((draft, _)) = self.draft_and_targets() else {
            return;
        };
        let name = draft.trim().to_owned();
        if name.is_empty() {
            self.note(t!("collections-blank"), false);
            return;
        }
        let (existed, filter) = {
            let browser = self.browser.borrow();
            (
                browser
                    .saved()
                    .iter()
                    .any(|saved| saved.name.to_lowercase() == name.to_lowercase()),
                browser.current().clone(),
            )
        };
        let mut saved = false;
        if !self.change_state(|state| saved = state.save_filter(&name, filter)) {
            return;
        }
        if !saved {
            // Only a hand-picked collection with that name stops it.
            self.note(t!("collections-taken", name = name.as_str()), false);
            return;
        }
        self.edit_sheet(|sheet| sheet.draft.clear());
        self.note(
            if existed {
                t!("collections-updated", name = name.as_str())
            } else {
                t!("collections-saved", name = name.as_str())
            },
            true,
        );
    }

    fn rename_collection(&self) {
        let (from, to) = {
            let sheet = self.collections.sheet.borrow();
            let Some(sheet) = sheet.as_ref() else {
                return;
            };
            let Some(from) = sheet.renaming.clone() else {
                return;
            };
            (from, sheet.rename_draft.trim().to_owned())
        };
        if to.is_empty() {
            self.note(t!("collections-blank"), false);
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
        if renamed {
            self.edit_sheet(|sheet| {
                sheet.renaming = None;
                sheet.rename_draft.clear();
            });
            self.note(t!("collections-renamed", name = to.as_str()), true);
            self.focus_draft();
        } else {
            self.note(t!("collections-taken", name = to.as_str()), false);
        }
    }

    /// Ticks or unticks a hand-picked collection for every game the sheet
    /// was opened for.
    fn set_membership(&self, name: &str, member: bool) {
        let Some((_, targets)) = self.draft_and_targets() else {
            return;
        };
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
        self.note(
            if member {
                t!("collections-assigned", count = count)
            } else {
                t!("collections-unassigned", count = count)
            },
            true,
        );
    }

    /// Delete, pressed once to ask and again to drop the collection (a
    /// hand-picked one with its games, as removing a game added by hand
    /// does; a saved view with what it kept).
    fn delete_collection(&self, name: &str) {
        let asking = {
            let mut sheet = self.collections.sheet.borrow_mut();
            let Some(sheet) = sheet.as_mut() else {
                return;
            };
            let asking = sheet.confirm_delete.as_deref() != Some(name);
            sheet.confirm_delete = asking.then(|| name.to_owned());
            if asking {
                sheet.note = None;
            }
            asking
        };
        if asking {
            self.show_collections();
            return;
        }
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
        self.edit_sheet(|sheet| {
            if sheet.renaming.as_deref() == Some(name) {
                sheet.renaming = None;
                sheet.rename_draft.clear();
            }
        });
        if deleted {
            self.note(t!("collections-deleted", name = name.trim()), true);
            // A rename of this row had the keyboard, and is gone.
            self.focus_draft();
        } else {
            self.show_collections();
        }
    }

    /// Puts a saved view on the library, and closes the sheet.
    fn apply_view(&self, name: &str) {
        self.close_collections();
        self.navigate(Page::Library);
        let outcome = self
            .browser
            .borrow_mut()
            .pick(&Pick::Saved(name.to_owned()));
        self.apply(outcome);
    }

    /// Changes the state, and the window with it: the sheet's list and each
    /// game's collections. False when the state could not be written.
    fn change_state(&self, change: impl FnOnce(&mut State)) -> bool {
        let state = if self.persist {
            match State::update(change) {
                Ok(state) => state,
                Err(err) => {
                    self.say(format!("{err:#}"));
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
        let browser = self.browser.borrow();
        let mut state = State::default();
        for category in browser.categories() {
            state.add_category(&category.name);
            for id in &category.games {
                state.set_category(&category.name, id, true);
            }
        }
        // A rename or delete of a category reaches the views naming it.
        for saved in browser.saved() {
            state.save_filter(&saved.name, saved.filter.clone());
        }
        state
    }

    /// Puts a state's collections on the page and on every game.
    fn reload_collections(&self, state: &State) {
        let outcome = self
            .browser
            .borrow_mut()
            .set_collections(state.categories(), state.filters());
        self.apply(outcome);
        self.show_game_collections();
        self.show_collections();
    }

    /// Writes the whole sheet.
    pub(super) fn show_collections(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<CollectionsData>();
        let sheet = self.collections.sheet.borrow();
        data.set_open(sheet.is_some());
        let Some(sheet) = sheet.as_ref() else {
            return;
        };
        let browser = self.browser.borrow();
        let (categories, saved) = (browser.categories(), browser.saved());
        data.set_count(t!("collections-count", count = categories.len() + saved.len()).into());
        data.set_empty(
            if categories.is_empty() && saved.is_empty() {
                t!("collections-empty")
            } else {
                String::new()
            }
            .into(),
        );
        data.set_now(t!("collections-now", summary = summary(browser.current())).into());
        data.set_draft(sheet.draft.clone().into());
        data.set_rename_draft(sheet.rename_draft.clone().into());
        let (note, good) = sheet.note.clone().unwrap_or_default();
        data.set_note(note.into());
        data.set_note_good(good);
        let rows: Vec<CollRow> = items(categories, saved)
            .into_iter()
            .map(|item| {
                let name = item.name().to_owned();
                let (view, sub, count, checked) = match item {
                    Item::Picked(category) => (
                        false,
                        String::new(),
                        t!("count-games", count = category.games.len()),
                        member(&sheet.targets, category),
                    ),
                    Item::View(saved) => (true, summary(&saved.filter), String::new(), false),
                };
                CollRow {
                    renaming: sheet.renaming.as_deref() == Some(name.as_str()),
                    confirm: sheet.confirm_delete.as_deref() == Some(name.as_str()),
                    checkable: !view && !sheet.targets.is_empty(),
                    name: name.into(),
                    sub: sub.into(),
                    count: count.into(),
                    view,
                    checked,
                }
            })
            .collect();
        self.collections.rows.set_vec(rows);
    }

    /// The marked games' ids, for the sheet the bulk chip opens.
    pub(super) fn marked_games(&self) -> BTreeSet<String> {
        self.browser.borrow().marked().clone()
    }
}
