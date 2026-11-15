//! The art picker, without a toolkit: "Change cover" and "Change backdrop"
//! on a game's page open a sheet that searches `SteamGridDB` for the game's
//! title and lists the covers, or the wide images, of the game chosen; a
//! click makes one the game's own, before Steam's and the stores'. The window
//! draws it and runs its [`Effect`]s.
//!
//! Every answer carries the generation it was asked in, and the counter
//! lives in [`Art`] across opens, so a late answer for a picker that was
//! closed never matches the next one.

use std::path::PathBuf;

use zendo_core::covers::{self, Kind};
use zendo_core::steamgriddb::{Found, Image, Page};

use crate::t;

/// Thumbnails downloaded at once.
pub const WORKERS: usize = 4;
/// Games found shown to choose from.
pub const GAMES_SHOWN: usize = 8;

/// What the window has to do next, on a thread of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Search by the title typed; a Steam game's own entry comes first.
    Search {
        generation: u64,
        key: String,
        query: String,
        steam: Option<String>,
    },
    /// A page of the game's images; page 0 starts over, and with it the
    /// thumbnails on their way stop.
    List {
        generation: u64,
        key: String,
        game: u64,
        kind: Kind,
        page: u32,
    },
    /// Download these thumbnails, `WORKERS` at a time, each answered with
    /// its id and bytes.
    Thumbs {
        generation: u64,
        wanted: Vec<(u64, String)>,
    },
    /// Download the image and keep it as the game's own.
    Choose {
        id: String,
        kind: Kind,
        image: Image,
    },
}

/// The picker, open over a game's page.
#[derive(Debug)]
pub struct Picker {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub query: String,
    pub games: Vec<Found>,
    pub game: Option<u64>,
    pub images: Vec<Image>,
    pub page: u32,
    pub more: bool,
    pub busy: Option<String>,
    pub note: Option<String>,
    /// The note is the one that asks for a key: Settings is offered.
    pub needs_key: bool,
    /// Whether the game has an image of this kind chosen by hand.
    pub chosen: bool,
    /// The generation the answers now wanted belong to.
    pub generation: u64,
    key: Option<String>,
}

/// What "Use the automatic one" did, for the window to finish.
#[derive(Debug, PartialEq, Eq)]
pub struct Automatic {
    pub id: String,
    pub kind: Kind,
    pub title: String,
}

/// The picker and the counter of its generations.
#[derive(Debug)]
pub struct Art {
    pub picker: Option<Picker>,
    generation: u64,
    /// Off in tests: nothing is read from disk and no effect is asked for.
    pub persist: bool,
}

/// The kind, as the messages choose their words by: `cover` or `hero`.
#[must_use]
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Cover => "cover",
        Kind::Hero => "hero",
    }
}

/// A game found, with its year when `SteamGridDB` says.
#[must_use]
pub fn game_label(game: &Found) -> String {
    match game.year() {
        Some(year) => format!("{} ({year})", game.name),
        None => game.name.clone(),
    }
}

/// What a thumbnail says when pointed at: its size, style and author.
#[must_use]
pub fn thumb_about(found: &Image) -> String {
    let mut about = format!("{}x{}", found.width, found.height);
    if !found.style.is_empty() {
        about = format!("{about}, {}", found.style);
    }
    if let Some(author) = &found.author {
        about = t!("art-by", about = about, author = author.name.as_str());
    }
    about
}

impl Art {
    #[must_use]
    pub fn new(persist: bool) -> Self {
        Self {
            picker: None,
            generation: 0,
            persist,
        }
    }

    /// Whether an answer asked in this generation is still wanted.
    #[must_use]
    pub fn wants(&self, generation: u64) -> bool {
        self.picker
            .as_ref()
            .is_some_and(|picker| picker.generation == generation)
    }

    /// Opens the picker for a game, on covers or on wide images. With no
    /// key it only says so.
    pub fn open(
        &mut self,
        id: &str,
        title: &str,
        kind: Kind,
        key: Option<String>,
    ) -> Option<Effect> {
        self.generation += 1;
        let chosen = self.persist && covers::chosen(id, kind).is_some();
        let steam = id.strip_prefix("steam:").map(str::to_owned);
        self.picker = Some(Picker {
            id: id.to_owned(),
            title: title.to_owned(),
            kind,
            query: title.to_owned(),
            games: Vec::new(),
            game: None,
            images: Vec::new(),
            page: 0,
            more: false,
            busy: None,
            note: None,
            needs_key: false,
            chosen,
            generation: self.generation,
            key,
        });
        if let Some(picker) = self.picker.as_mut()
            && picker.key.is_none()
        {
            picker.note = Some(t!("art-needs-key"));
            picker.needs_key = true;
            return None;
        }
        self.search(steam)
    }

    /// Closes the picker, which makes every answer on its way stale.
    pub fn close(&mut self) {
        self.picker = None;
    }

    /// Closes the picker when it is this game's, and gives it back.
    pub fn take_for(&mut self, id: &str) -> Option<Picker> {
        self.picker.take_if(|picker| picker.id == id)
    }

    /// The tab: covers or wide images of the game chosen.
    pub fn set_kind(&mut self, kind: Kind) -> Option<Effect> {
        let persist = self.persist;
        let picker = self.picker.as_mut()?;
        if kind == picker.kind {
            return None;
        }
        picker.kind = kind;
        picker.chosen = persist && covers::chosen(&picker.id, kind).is_some();
        let game = picker.game?;
        self.list(game, 0)
    }

    pub fn set_query(&mut self, query: String) {
        if let Some(picker) = self.picker.as_mut() {
            picker.query = query;
        }
    }

    /// Searches by the title typed; a Steam game's own entry comes first.
    pub fn search(&mut self, steam: Option<String>) -> Option<Effect> {
        let persist = self.persist;
        let generation = self.next();
        let picker = self.picker.as_mut()?;
        let key = picker.key.clone()?;
        picker.generation = generation;
        picker.games.clear();
        picker.game = None;
        picker.images.clear();
        picker.note = None;
        picker.needs_key = false;
        picker.busy = Some(t!("art-searching"));
        persist.then(|| Effect::Search {
            generation,
            key,
            query: picker.query.clone(),
            steam,
        })
    }

    /// Searches again for the title typed, as Enter and the button do. A
    /// Steam game's own entry is not asked for again.
    pub fn search_typed(&mut self) -> Option<Effect> {
        self.search(None)
    }

    /// The games found. The first one's images are listed.
    pub fn found(&mut self, generation: u64, result: Result<Vec<Found>, String>) -> Option<Effect> {
        if !self.wants(generation) {
            return None;
        }
        let picker = self.picker.as_mut()?;
        picker.busy = None;
        match result {
            Ok(games) if games.is_empty() => {
                picker.games.clear();
                picker.note = Some(t!("art-no-game", query = picker.query.trim()));
                None
            }
            Ok(games) => {
                let first = games[0].id;
                picker.games = games;
                self.list(first, 0)
            }
            Err(err) => {
                picker.note = Some(err);
                None
            }
        }
    }

    /// A game of those found was clicked.
    pub fn pick_game(&mut self, game: u64) -> Option<Effect> {
        if self.picker.as_ref()?.game == Some(game) {
            return None;
        }
        self.list(game, 0)
    }

    /// Show more: the next page joins the images already there.
    pub fn more(&mut self) -> Option<Effect> {
        let picker = self.picker.as_ref()?;
        if !picker.more || picker.busy.is_some() {
            return None;
        }
        let (game, page) = (picker.game?, picker.page + 1);
        self.list(game, page)
    }

    /// A page of the game's images of the picker's kind; page 0 starts
    /// over.
    fn list(&mut self, game: u64, page: u32) -> Option<Effect> {
        let persist = self.persist;
        // Only a fresh list makes the answers and thumbnails already on
        // their way stale; a later page joins them.
        let generation = if page == 0 {
            self.next()
        } else {
            self.generation
        };
        let picker = self.picker.as_mut()?;
        let key = picker.key.clone()?;
        picker.generation = generation;
        if page == 0 {
            picker.images.clear();
        }
        picker.game = Some(game);
        picker.page = page;
        picker.more = false;
        picker.note = None;
        picker.busy = Some(t!("art-looking"));
        persist.then_some(Effect::List {
            generation,
            key,
            game,
            kind: picker.kind,
            page,
        })
    }

    /// A page of images arrived: the thumbnails to download.
    pub fn listed(&mut self, generation: u64, result: Result<Page, String>) -> Option<Effect> {
        if !self.wants(generation) {
            return None;
        }
        let persist = self.persist;
        let picker = self.picker.as_mut()?;
        picker.busy = None;
        match result {
            Ok(page) => {
                picker.more = page.more;
                let wanted: Vec<(u64, String)> = page
                    .images
                    .iter()
                    .map(|image| (image.id, image.thumb.clone()))
                    .collect();
                picker.images.extend(page.images);
                if picker.images.is_empty() {
                    picker.note = Some(t!("art-no-images", kind = kind_name(picker.kind)));
                }
                (persist && !wanted.is_empty()).then_some(Effect::Thumbs { generation, wanted })
            }
            Err(err) => {
                picker.note = Some(err);
                None
            }
        }
    }

    /// An image was clicked: it is downloaded and kept.
    pub fn choose(&mut self, image: u64) -> Option<Effect> {
        let persist = self.persist;
        let picker = self.picker.as_mut()?;
        if picker.busy.is_some() {
            return None;
        }
        let image = picker
            .images
            .iter()
            .find(|found| found.id == image)?
            .clone();
        picker.busy = Some(t!("saving"));
        picker.note = None;
        persist.then(|| Effect::Choose {
            id: picker.id.clone(),
            kind: picker.kind,
            image,
        })
    }

    /// Saving the chosen image failed, for a picker still on this game.
    pub fn choose_failed(&mut self, id: &str, err: String) {
        if let Some(picker) = self.picker.as_mut().filter(|picker| picker.id == id) {
            picker.busy = None;
            picker.note = Some(err);
        }
    }

    /// "Use the automatic one": forgets the image chosen by hand and closes
    /// the picker. `None` when the choice could not be deleted, which the
    /// picker then says.
    pub fn automatic(&mut self) -> Option<Automatic> {
        let persist = self.persist;
        let picker = self.picker.as_mut()?;
        let (id, kind) = (picker.id.clone(), picker.kind);
        if persist && let Err(err) = covers::forget_choice(&id, kind) {
            picker.note = Some(format!("{err:#}"));
            return None;
        }
        let picker = self.picker.take()?;
        Some(Automatic {
            id,
            kind,
            title: picker.title,
        })
    }

    /// The footer's words for a finished choice.
    #[must_use]
    pub fn new_note(kind: Kind, title: &str) -> String {
        t!("art-new", kind = kind_name(kind), title = title)
    }

    /// The footer's words for the automatic image back.
    #[must_use]
    pub fn automatic_note(kind: Kind, title: &str) -> String {
        t!(
            "art-back-to-automatic",
            kind = kind_name(kind),
            title = title
        )
    }

    /// The label of the Use automatic button.
    #[must_use]
    pub fn automatic_label(kind: Kind) -> String {
        t!("art-use-automatic", kind = kind_name(kind))
    }

    fn next(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }
}

/// Where a chosen image was saved, for the window to draw.
pub type Chosen = Result<PathBuf, String>;

#[cfg(test)]
mod tests {
    use super::*;

    fn found(id: u64, name: &str) -> Found {
        Found {
            id,
            name: name.into(),
            release_date: None,
        }
    }

    fn image(id: u64) -> Image {
        Image {
            id,
            url: format!("https://x/{id}"),
            thumb: format!("https://x/t{id}"),
            width: 600,
            height: 900,
            style: String::new(),
            author: None,
        }
    }

    fn art() -> Art {
        Art::new(false)
    }

    #[test]
    fn asks_for_a_key_and_drops_late_answers() {
        let mut art = art();
        assert_eq!(art.open("steam:620", "Portal 2", Kind::Cover, None), None);
        let picker = art.picker.as_ref().unwrap();
        assert!(picker.needs_key);
        assert!(
            picker
                .note
                .as_deref()
                .is_some_and(|n| n.contains("Settings"))
        );
        art.close();

        art.open("steam:620", "Portal 2", Kind::Cover, Some("key".into()));
        let picker = art.picker.as_ref().unwrap();
        assert_eq!(picker.busy.as_deref(), Some("Searching…"));
        let generation = picker.generation;
        // An answer to an earlier search is dropped.
        art.found(generation - 1, Ok(vec![found(1, "Portal")]));
        assert_eq!(art.picker.as_ref().unwrap().games, []);
        art.found(generation, Ok(vec![found(17830, "Portal 2")]));
        let picker = art.picker.as_ref().unwrap();
        assert_eq!(picker.game, Some(17830));
        assert_eq!(picker.busy.as_deref(), Some("Looking for images…"));
    }

    #[test]
    fn a_late_answer_for_a_closed_picker_never_matches_the_next() {
        let mut art = art();
        art.open("a", "A", Kind::Cover, Some("k".into()));
        let old = art.picker.as_ref().unwrap().generation;
        art.close();
        art.open("b", "B", Kind::Cover, Some("k".into()));
        assert_ne!(art.picker.as_ref().unwrap().generation, old);
        art.found(old, Ok(vec![found(1, "A")]));
        assert_eq!(art.picker.as_ref().unwrap().games, []);
    }

    #[test]
    fn a_later_page_joins_and_a_new_list_starts_over() {
        let mut art = art();
        art.open("local:x", "X", Kind::Cover, Some("k".into()));
        let first = art.picker.as_ref().unwrap().generation;
        art.found(first, Ok(vec![found(5, "X"), found(6, "Y")]));
        let listing = art.picker.as_ref().unwrap().generation;
        art.listed(
            listing,
            Ok(Page {
                images: vec![image(1), image(2)],
                more: true,
            }),
        );
        let picker = art.picker.as_ref().unwrap();
        assert_eq!((picker.images.len(), picker.more), (2, true));
        art.more();
        assert_eq!(
            art.picker.as_ref().unwrap().generation,
            listing,
            "a later page keeps the generation"
        );
        art.listed(
            listing,
            Ok(Page {
                images: vec![image(3)],
                more: false,
            }),
        );
        assert_eq!(art.picker.as_ref().unwrap().images.len(), 3);
        art.pick_game(6);
        let picker = art.picker.as_ref().unwrap();
        assert_eq!(picker.images, []);
        assert_ne!(picker.generation, listing);
    }

    #[test]
    fn the_tab_lists_the_other_kind_of_the_game_chosen() {
        let mut art = art();
        art.open("local:x", "X", Kind::Cover, Some("k".into()));
        let generation = art.picker.as_ref().unwrap().generation;
        art.found(generation, Ok(vec![found(5, "X")]));
        let listing = art.picker.as_ref().unwrap().generation;
        art.listed(
            listing,
            Ok(Page {
                images: vec![image(1)],
                more: false,
            }),
        );
        art.set_kind(Kind::Hero);
        let picker = art.picker.as_ref().unwrap();
        assert_eq!(picker.kind, Kind::Hero);
        assert_eq!(picker.images, []);
        assert_eq!(picker.busy.as_deref(), Some("Looking for images…"));
    }

    #[test]
    fn choosing_waits_while_busy_and_a_failure_is_for_its_game_only() {
        let mut art = art();
        art.open("local:x", "X", Kind::Cover, Some("k".into()));
        let generation = art.picker.as_ref().unwrap().generation;
        art.found(generation, Ok(vec![found(5, "X")]));
        let listing = art.picker.as_ref().unwrap().generation;
        art.listed(
            listing,
            Ok(Page {
                images: vec![image(1)],
                more: false,
            }),
        );
        art.choose(1);
        assert_eq!(
            art.picker.as_ref().unwrap().busy.as_deref(),
            Some("Saving…")
        );
        art.choose_failed("local:other", "no".into());
        assert!(art.picker.as_ref().unwrap().busy.is_some());
        art.choose_failed("local:x", "no".into());
        let picker = art.picker.as_ref().unwrap();
        assert_eq!(
            (picker.busy.as_deref(), picker.note.as_deref()),
            (None, Some("no"))
        );
        assert!(art.take_for("local:other").is_none());
        assert!(art.take_for("local:x").is_some());
    }
}
