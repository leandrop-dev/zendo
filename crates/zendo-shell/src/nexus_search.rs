//! Searching Nexus Mods from a game's Mods card, without a toolkit: the
//! sheet's state (the words, the order, the results a page at a time, each
//! mod's files), its transitions, and the words it shows. The window
//! draws it. What waits on the network runs on a thread
//! through [`nexus_search::search`] and [`zendo_core::nexus::files`]; the
//! pictures and Nexus's window are the toolkits' own business.
//!
//! The transitions touch no file and no network, so tests need none.

use std::collections::HashSet;

use zendo_core::Size;
use zendo_core::mods::Overview;
use zendo_core::nexus::{self, FileInfo, Files, Link};
use zendo_core::nexus_search::{Found, Hit, Order, Query};

use crate::t;

/// What a mod's file list is.
#[derive(Debug)]
pub enum FilesState {
    Asking,
    /// The files offered.
    Got(Vec<FileInfo>),
    Failed(String),
}

/// The sheet, open over a game's page.
#[derive(Debug)]
pub struct Search {
    /// The game's id in Zendo, its title, and its name on Nexus.
    pub game: String,
    pub title: String,
    pub domain: String,
    pub words: String,
    pub order: Order,
    pub hits: Vec<Hit>,
    pub total: u64,
    /// Each search's number: a late answer to an earlier one is dropped.
    pub generation: u64,
    pub asking: bool,
    pub error: Option<String>,
    /// The mod whose files are shown, and them once Nexus said.
    pub open: Option<(u64, FilesState)>,
}

/// What opening a mod's files leaves to do.
#[derive(Debug, PartialEq, Eq)]
pub enum FilesStep {
    /// Only the state changed (the files were hidden, or cannot be asked).
    Nothing,
    /// Nexus is to be asked for the mod's files, on a thread.
    Ask(u64),
}

/// A search to run on a thread: its number, and what to ask.
#[derive(Debug)]
pub struct Ask {
    pub generation: u64,
    pub query: Query,
}

impl Search {
    #[must_use]
    pub fn new(game: &str, title: String, domain: String) -> Self {
        Self {
            game: game.to_owned(),
            title,
            domain,
            words: String::new(),
            order: Order::default(),
            hits: Vec::new(),
            total: 0,
            generation: 0,
            asking: false,
            error: None,
            open: None,
        }
    }

    /// The search from the start: the results so far go.
    pub fn restart(&mut self, adult: bool) -> Ask {
        self.generation += 1;
        self.hits.clear();
        self.total = 0;
        self.open = None;
        self.ask(adult)
    }

    /// The next page, after the results shown; nothing while one is asked
    /// for or when every result is shown.
    pub fn more(&mut self, adult: bool) -> Option<Ask> {
        (!self.asking && self.has_more()).then(|| self.ask(adult))
    }

    /// Whether Nexus has results that are not shown yet.
    #[must_use]
    pub fn has_more(&self) -> bool {
        (self.hits.len() as u64) < self.total
    }

    fn ask(&mut self, adult: bool) -> Ask {
        self.asking = true;
        self.error = None;
        Ask {
            generation: self.generation,
            query: Query {
                domain: self.domain.clone(),
                words: self.words.clone(),
                order: self.order,
                adult,
                offset: u32::try_from(self.hits.len()).unwrap_or(u32::MAX),
            },
        }
    }

    /// Nexus answered a search. The thumbnails to fetch (mod and address), or
    /// `None` when the answer is to an earlier search or failed.
    pub fn found(
        &mut self,
        generation: u64,
        result: Result<Found, String>,
    ) -> Option<Vec<(u64, String)>> {
        if generation != self.generation {
            return None;
        }
        self.asking = false;
        match result {
            Ok(found) => {
                self.total = found.total;
                let wanted = found
                    .hits
                    .iter()
                    .filter_map(|hit| Some((hit.mod_id, hit.thumbnail.clone()?)))
                    .collect();
                self.hits.extend(found.hits);
                Some(wanted)
            }
            Err(err) => {
                self.error = Some(err);
                None
            }
        }
    }

    /// Shows a mod's files, or hides them. `askable` is whether Nexus can be
    /// asked (a key, and not a test).
    pub fn toggle_files(&mut self, mod_id: u64, askable: bool) -> FilesStep {
        if self.open.as_ref().is_some_and(|(open, _)| *open == mod_id) {
            self.open = None;
            return FilesStep::Nothing;
        }
        self.open = Some((mod_id, FilesState::Asking));
        if askable {
            FilesStep::Ask(mod_id)
        } else {
            FilesStep::Nothing
        }
    }

    /// Nexus listed a mod's files.
    pub fn got_files(&mut self, mod_id: u64, result: Result<Files, String>) {
        if let Some((open, state)) = self.open.as_mut()
            && *open == mod_id
        {
            *state = match result {
                Ok(files) => FilesState::Got(offered(files)),
                Err(err) => FilesState::Failed(err),
            };
        }
    }

    /// The link of a file of one of the found mods.
    #[must_use]
    pub fn link(&self, mod_id: u64, file_id: u64) -> Link {
        Link {
            domain: self.domain.clone(),
            mod_id,
            file_id,
            key: None,
            expires: None,
        }
    }

    /// The footer's line, and whether it is a failure.
    #[must_use]
    pub fn footer(&self) -> (String, bool) {
        if self.asking {
            (t!("search-asking"), false)
        } else if let Some(err) = &self.error {
            (err.clone(), true)
        } else if self.total == 0 {
            (t!("search-nothing"), false)
        } else {
            (
                t!("search-count", shown = self.hits.len(), total = self.total),
                false,
            )
        }
    }
}

/// The URL of a mod's page on Nexus, or of one file's Mod manager download
/// (`nmm=1`), for Nexus's window.
#[must_use]
pub fn site_url(domain: &str, mod_id: u64, file_id: Option<u64>) -> String {
    let mut url = nexus::mod_page_url(domain, mod_id, file_id);
    if file_id.is_some() {
        url.push_str("&nmm=1");
    }
    url
}

/// The mods from Nexus already in the game's list, and their files.
#[must_use]
pub fn have(overview: Option<&Overview>, domain: &str) -> (HashSet<u64>, HashSet<(u64, u64)>) {
    let mut mods = HashSet::new();
    let mut files = HashSet::new();
    for info in overview.map_or_default(|overview| overview.mods.as_slice()) {
        match &info.nexus {
            Some(source) if source.domain == domain => {
                mods.insert(source.mod_id);
                files.insert((source.mod_id, source.file_id));
            }
            Some(_) => {}
            // Added by hand from an archive Nexus named: its name says the
            // mod.
            None => mods.extend(nexus::ids_in_name(&info.name).first()),
        }
    }
    (mods, files)
}

/// The files worth offering, newest first: those the author keeps current.
#[must_use]
pub fn offered(files: Files) -> Vec<FileInfo> {
    let mut files: Vec<FileInfo> = files
        .files
        .into_iter()
        .filter(|file| {
            !matches!(
                file.category_name.as_deref(),
                Some("OLD_VERSION" | "ARCHIVED" | "DELETED" | "REMOVED")
            )
        })
        .collect();
    files.sort_by_key(|file| std::cmp::Reverse(file.uploaded_timestamp));
    files
}

/// A file's facts on its line: version, kind and size.
#[must_use]
pub fn file_facts(file: &FileInfo) -> String {
    let mut facts = Vec::new();
    if let Some(version) = file.version.as_deref().filter(|v| !v.is_empty()) {
        facts.push(version.to_owned());
    }
    if let Some(category) = &file.category_name {
        facts.push(file_kind(category));
    }
    if let Some(kb) = file.size_kb {
        facts.push(Size::Bytes(kb * 1024).to_string());
    }
    facts.join(" · ")
}

/// A mod's byline: author, category and when it was updated.
#[must_use]
pub fn byline(hit: &Hit) -> String {
    let mut line = vec![t!("search-by", author = hit.author.as_str())];
    if let Some(category) = &hit.category {
        line.push(category.clone());
    }
    if !hit.updated.is_empty() {
        line.push(t!("search-updated", date = hit.updated.as_str()));
    }
    line.join(" · ")
}

/// A mod's endorsements, downloads and size.
#[must_use]
pub fn numbers(hit: &Hit) -> String {
    let mut line = vec![
        t!("search-endorsements", count = count(hit.endorsements)),
        t!("search-downloads", count = count(hit.downloads)),
    ];
    if let Some(bytes) = hit.size {
        line.push(Size::Bytes(bytes).to_string());
    }
    line.join(" · ")
}

#[must_use]
pub fn order_name(order: Order) -> String {
    match order {
        Order::Endorsed => t!("search-order-endorsed"),
        Order::Downloaded => t!("search-order-downloaded"),
        Order::Updated => t!("search-order-updated"),
    }
}

/// Nexus's file categories, in words.
#[must_use]
pub fn file_kind(category: &str) -> String {
    match category {
        "MAIN" => t!("search-file-main"),
        "OPTIONAL" => t!("search-file-optional"),
        "UPDATE" => t!("search-file-update"),
        "MISCELLANEOUS" => t!("search-file-misc"),
        other => other.to_lowercase(),
    }
}

/// "950", "12k", "4.5M".
#[must_use]
pub fn count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", n / 1_000),
        _ => format!("{}.{}M", n / 1_000_000, n % 1_000_000 / 100_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(mod_id: u64) -> Hit {
        Hit {
            mod_id,
            name: "SMAPI".into(),
            summary: String::new(),
            author: "Pathoschild".into(),
            version: "4.0".into(),
            endorsements: 1,
            downloads: 1,
            size: None,
            updated: String::new(),
            category: None,
            adult: false,
            thumbnail: Some("https://x/t.jpg".into()),
        }
    }

    fn search() -> Search {
        Search::new(
            "steam:413150",
            "Stardew Valley".into(),
            "stardewvalley".into(),
        )
    }

    #[test]
    fn counts_read_short() {
        assert_eq!(count(950), "950");
        assert_eq!(count(12_345), "12k");
        assert_eq!(count(4_560_000), "4.5M");
    }

    #[test]
    fn offers_the_current_files_newest_first() {
        let file = |id: u64, category: &str, at: i64| FileInfo {
            file_id: id,
            name: format!("file {id}"),
            version: None,
            file_name: String::new(),
            size_kb: None,
            category_name: Some(category.into()),
            uploaded_timestamp: at,
        };
        let files = Files {
            files: vec![
                file(1, "MAIN", 10),
                file(2, "OLD_VERSION", 30),
                file(3, "OPTIONAL", 20),
            ],
            file_updates: Vec::new(),
        };
        let ids: Vec<u64> = offered(files).iter().map(|file| file.file_id).collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn an_answer_to_an_earlier_search_is_dropped() {
        let mut search = search();
        let first = search.restart(false);
        assert!(search.asking);
        let found = || Found {
            hits: vec![hit(2400)],
            total: 30,
        };
        assert!(search.found(first.generation + 1, Ok(found())).is_none());
        assert!(search.hits.is_empty() && search.asking, "still waiting");

        let wanted = search.found(first.generation, Ok(found())).unwrap();
        assert_eq!(wanted, [(2400, "https://x/t.jpg".to_owned())]);
        assert_eq!(
            (search.hits.len(), search.total, search.asking),
            (1, 30, false)
        );

        // Changing the order starts again, with a new number.
        search.order = Order::Updated;
        let again = search.restart(true);
        assert_eq!(again.generation, first.generation + 1);
        assert!(again.query.adult);
        assert_eq!((search.hits.len(), again.query.offset), (0, 0));
    }

    #[test]
    fn more_asks_for_the_page_after_the_results_and_only_once() {
        let mut search = search();
        let ask = search.restart(false);
        let _ = search.found(
            ask.generation,
            Ok(Found {
                hits: vec![hit(1), hit(2)],
                total: 5,
            }),
        );
        let next = search.more(false).expect("more to show");
        assert_eq!(next.query.offset, 2);
        assert!(search.more(false).is_none(), "one at a time");
        let _ = search.found(
            next.generation,
            Ok(Found {
                hits: vec![hit(3), hit(4), hit(5)],
                total: 5,
            }),
        );
        assert!(search.more(false).is_none(), "everything is shown");
    }

    #[test]
    fn a_mods_files_open_once_and_close_on_a_second_press() {
        let mut search = search();
        assert_eq!(search.toggle_files(7, true), FilesStep::Ask(7));
        assert!(matches!(search.open, Some((7, FilesState::Asking))));
        // An answer for another mod is let go.
        search.got_files(8, Err("x".into()));
        assert!(matches!(search.open, Some((7, FilesState::Asking))));
        search.got_files(7, Err("offline".into()));
        assert!(matches!(search.open, Some((7, FilesState::Failed(_)))));
        assert_eq!(search.toggle_files(7, true), FilesStep::Nothing);
        assert!(search.open.is_none());
        assert_eq!(search.toggle_files(9, false), FilesStep::Nothing);
        assert!(search.open.is_some(), "shown, though not asked");
    }

    #[test]
    fn a_file_download_needs_the_nmm_flag_but_a_page_does_not() {
        assert!(site_url("stardewvalley", 1, Some(2)).ends_with("&nmm=1"));
        assert!(!site_url("stardewvalley", 1, None).contains("nmm"));
    }
}
