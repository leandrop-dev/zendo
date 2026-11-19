//! What GOG says of its own games, on the page: the Changelog card (GOG's
//! notes for each update, newest first) and the total time played, which
//! GOG keeps as Steam does. Both are asked on a thread, once a session, as
//! the page opens; the total again after Zendo sent GOG a session. A
//! failure (offline, say) is forgotten, so the page asks again next time.
//! A part of the Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::gog_changelog::{self, Entry};
use zendo_core::gog_playtime;
use zendo_shell::gog::{self, MORE};

use super::Glue;
use super::cards::Ask;
use crate::{GameData, Line, MainWindow};

/// GOG's answers, by game, and the lines the changelog card reads.
pub(super) struct GogState {
    changelogs: RefCell<HashMap<String, Ask<Vec<Entry>>>>,
    /// How many entries are shown beyond the newest.
    more: Cell<usize>,
    /// The games whose total was asked this session.
    totals: RefCell<HashSet<String>>,
    lines: Rc<VecModel<Line>>,
}

impl GogState {
    pub(super) fn new() -> Self {
        Self {
            changelogs: RefCell::default(),
            more: Cell::new(0),
            totals: RefCell::default(),
            lines: Rc::new(VecModel::default()),
        }
    }
}

impl Glue {
    /// Hooks the card's callback up and gives the page its model.
    pub(super) fn connect_gog(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameData>();
        data.set_changelog(ModelRc::from(Rc::clone(&self.gog.lines)));
        let glue = Rc::clone(self);
        data.on_more_changelog(move || {
            glue.gog.more.set(glue.gog.more.get() + MORE);
            glue.show_game();
        });
    }

    /// The page just opened: asks GOG for what it has not been asked this
    /// session. Nothing for a game not from GOG.
    pub(super) fn want_gog(&self, id: &str) {
        self.gog.more.set(0);
        self.want_changelog(id);
        self.want_gog_total(id);
    }

    fn want_changelog(&self, id: &str) {
        let Some(product) = id.strip_prefix("gog:") else {
            return;
        };
        if !self.persist || self.gog.changelogs.borrow().contains_key(id) {
            return;
        }
        self.gog
            .changelogs
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let (owned, product) = (id.to_owned(), product.to_owned());
        self.ask(
            move || gog_changelog::fetch(&product).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.changelog_arrived(owned, result),
        );
    }

    pub(super) fn changelog_arrived(&self, id: String, result: Result<Vec<Entry>, String>) {
        match result {
            Ok(entries) => {
                self.gog
                    .changelogs
                    .borrow_mut()
                    .insert(id, Ask::Got(entries));
            }
            Err(_) => {
                self.gog.changelogs.borrow_mut().remove(&id);
            }
        }
        self.show_game();
    }

    /// Asks GOG for the game's total, once a session.
    pub(super) fn want_gog_total(&self, id: &str) {
        if !self.persist
            || !id.starts_with("gog:")
            || !self.gog.totals.borrow_mut().insert(id.to_owned())
        {
            return;
        }
        let owned = id.to_owned();
        let asked = id.to_owned();
        self.ask(
            move || gog_playtime::total(&asked).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.gog_total_arrived(&owned, &result),
        );
    }

    /// Asks again, as a session was just sent.
    pub(super) fn refresh_gog_total(&self, id: &str) {
        self.gog.totals.borrow_mut().remove(id);
        self.want_gog_total(id);
    }

    /// Shows the total where it is more than the count known.
    pub(super) fn gog_total_arrived(&self, id: &str, result: &Result<Option<u64>, String>) {
        match *result {
            Ok(Some(minutes)) => {
                let outcome = self.browser.borrow_mut().raise_minutes(id, minutes);
                self.apply(outcome);
                self.show_game();
            }
            Ok(None) => {}
            Err(_) => {
                self.gog.totals.borrow_mut().remove(id);
            }
        }
    }

    /// Writes the Changelog card: the newest entry, and a few more each time
    /// Show earlier updates is pressed. Nothing for a game with no changelog.
    pub(super) fn show_changelog(&self, id: &str, data: &GameData<'_>) {
        data.set_changelog_more("".into());
        let mut lines = Vec::new();
        if let Some(Ask::Got(entries)) = self.gog.changelogs.borrow().get(id) {
            let shown = (1 + self.gog.more.get()).min(entries.len());
            if let Some(more) = gog::more_label(entries, shown) {
                data.set_changelog_more(more.into());
            }
            lines = gog::lines(entries, shown)
                .into_iter()
                .map(|line| Line {
                    kind: line.kind,
                    text: line.text.into(),
                })
                .collect();
        }
        self.gog.lines.set_vec(lines);
    }
}
