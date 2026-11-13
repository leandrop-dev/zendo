//! The wide image behind a game page's header, drawn from a smaller copy.
//!
//! The copy is made on a thread (`zendo_core::thumbs`), so the page opens
//! at once and the backdrop fades in when it is ready. Nothing big is ever
//! decoded on the thread that draws: that was 12 ms to 150 ms a page in a
//! release build and half a second or more in a debug one without optimised
//! dependencies (checked 2026-11-13).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use iced::Task;
use zendo_core::thumbs;

use super::{Message, Page, Zendo, blocking};

/// The copies ready to draw, and the ones being made.
#[derive(Debug, Default)]
pub struct Backdrops {
    /// A hero's path, and the file to draw instead.
    ready: HashMap<PathBuf, PathBuf>,
    /// Heroes whose copy is being made.
    pending: HashSet<PathBuf>,
}

impl Backdrops {
    /// The file to draw for this hero, when its copy is ready.
    pub fn get(&self, hero: &Path) -> Option<&Path> {
        self.ready.get(hero).map(PathBuf::as_path)
    }

    /// What a finished copy changes: it is ready, or it failed and the page
    /// stays without a backdrop.
    pub fn made(&mut self, hero: PathBuf, copy: Option<PathBuf>) {
        self.pending.remove(&hero);
        if let Some(copy) = copy {
            self.ready.insert(hero, copy);
        }
    }

    /// Whether this hero still has to be asked for: not ready, not being
    /// made. It is marked as being made.
    fn ask(&mut self, hero: &Path) -> bool {
        if self.ready.contains_key(hero) {
            return false;
        }
        if let Some(copy) = thumbs::backdrop(hero) {
            self.ready.insert(hero.to_path_buf(), copy);
            return false;
        }
        self.pending.insert(hero.to_path_buf())
    }
}

impl Zendo {
    /// The file to draw behind a header for this hero.
    pub(super) fn backdrop_of(&self, hero: Option<&Path>) -> Option<&Path> {
        self.backdrops.get(hero?)
    }

    /// Asks for the copy of the hero of the game page on screen, when there
    /// is one to ask for.
    pub(super) fn want_backdrop(&mut self) -> Task<Message> {
        let (Page::Game(id) | Page::Owned(id)) = &self.page else {
            return Task::none();
        };
        let hero = self
            .game(id)
            .and_then(|game| game.hero.clone())
            .or_else(|| self.store.owned(id).and_then(|owned| owned.hero.clone()));
        let Some(hero) = hero else {
            return Task::none();
        };
        if !self.backdrops.ask(&hero) {
            return Task::none();
        }
        let source = hero.clone();
        Task::perform(
            blocking(move || thumbs::make_backdrop(&source).ok()),
            move |copy| Message::Backdrop(hero.clone(), copy),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_copy_is_ready_and_a_failed_one_is_not_asked_for_twice_at_once() {
        let mut backdrops = Backdrops::default();
        let hero = PathBuf::from("/nowhere/hero.jpg");
        // Gone from disk: no copy exists, so it is asked for, once.
        assert!(backdrops.ask(&hero));
        assert!(!backdrops.ask(&hero), "being made");
        backdrops.made(hero.clone(), None);
        assert_eq!(backdrops.get(&hero), None);
        assert!(backdrops.ask(&hero), "a failure may be tried again");
        backdrops.made(hero.clone(), Some(PathBuf::from("/nowhere/copy.jpg")));
        assert_eq!(backdrops.get(&hero), Some(Path::new("/nowhere/copy.jpg")));
        assert!(!backdrops.ask(&hero), "ready");
    }
}
