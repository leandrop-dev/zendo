//! Searching Nexus Mods from a game's Mods card: a sheet over the game's
//! page with the search, its results a page at a time, and each mod's
//! files, downloaded into the game's mods with Premium, or else got on the
//! mod's page in Zendo's Nexus window ([`crate::sign_in_window`]): the
//! Mod manager download's link or the manual download's file is taken
//! into the game's mods.

use std::collections::HashSet;

use iced::widget::{
    Id, button, column, container, image, operation, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Element, Length, Task};
use zendo_core::nexus::{self, FileInfo, Files, Link};
use zendo_core::nexus_search::{self, Found, Hit, Order, Query};
use zendo_core::{Size, mods};

use super::artwork::thumbnails;
use super::launch_options::overlay;
use super::nexus::added_note;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;
use crate::sign_in_window::{self, FromSite, Handle};

fn input_id() -> Id {
    Id::new("nexus-search")
}

/// The sheet, open over a game's page.
pub struct Search {
    /// The game's id in Zendo, its title, and its name on Nexus.
    game: String,
    title: String,
    domain: String,
    words: String,
    order: Order,
    hits: Vec<Hit>,
    total: u64,
    /// Each search's number: a late answer to an earlier one is dropped.
    generation: u64,
    asking: bool,
    error: Option<String>,
    thumbs: std::collections::HashMap<u64, image::Handle>,
    /// The mod whose files are shown, and them once Nexus said.
    open: Option<(u64, FilesState)>,
    /// The thumbnails coming; dropped with the sheet, they stop.
    downloading: Vec<iced::task::Handle>,
    /// Nexus's window, while a mod's page is open in it.
    site: Option<Handle>,
}

impl std::fmt::Debug for Search {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Search")
            .field("game", &self.game)
            .field("words", &self.words)
            .field("hits", &self.hits.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
enum FilesState {
    Asking,
    /// The files offered.
    Got(Vec<FileInfo>),
    Failed(String),
}

/// A mod's page asked of Nexus's window: for which game and mod.
#[derive(Debug, Clone)]
pub struct Asked {
    game: String,
    domain: String,
    mod_id: u64,
}

#[derive(Debug, Clone)]
pub enum SearchMessage {
    /// Opens the sheet for a game's mods on Nexus.
    Open(String),
    Words(String),
    Search,
    Order(Order),
    ToggleAdult,
    More,
    Found(u64, Result<Found, String>),
    Thumb(u64, u64, Option<Vec<u8>>),
    /// Shows or hides a mod's files.
    Files(u64),
    GotFiles(u64, Result<Files, String>),
    Download(u64, u64),
    /// Opens a mod's page in Nexus's window.
    Page(u64),
    /// What Nexus's window gave.
    FromSite(Asked, Result<Option<FromSite>, String>),
    Close,
}

impl Zendo {
    pub(super) fn update_search(&mut self, message: SearchMessage) -> Task<Message> {
        if let SearchMessage::Open(id) = message {
            return self.open_search(&id);
        }
        if let SearchMessage::ToggleAdult = message {
            let adult = !self.config.nexus_adult;
            let saved = self.save_setting(super::settings::Setting::NexusAdult(adult));
            return Task::batch([saved, self.run_search(false)]);
        }
        if let SearchMessage::Download(mod_id, file_id) = message {
            return self.download_found(mod_id, file_id);
        }
        if let SearchMessage::Page(mod_id) = message {
            return self.open_site(mod_id, None);
        }
        if let SearchMessage::FromSite(asked, result) = message {
            return self.site_gave(asked, result);
        }
        let Some(search) = self.mod_search.as_mut() else {
            return Task::none();
        };
        match message {
            SearchMessage::Words(words) => search.words = words,
            SearchMessage::Search => return self.run_search(false),
            SearchMessage::Order(order) => {
                search.order = order;
                return self.run_search(false);
            }
            SearchMessage::More if !search.asking && (search.hits.len() as u64) < search.total => {
                return self.run_search(true);
            }
            SearchMessage::Found(generation, result) if generation == search.generation => {
                search.asking = false;
                match result {
                    Ok(found) => {
                        search.total = found.total;
                        let wanted: Vec<(u64, String)> = found
                            .hits
                            .iter()
                            .filter_map(|hit| Some((hit.mod_id, hit.thumbnail.clone()?)))
                            .collect();
                        search.hits.extend(found.hits);
                        if wanted.is_empty() || !self.persist {
                            return Task::none();
                        }
                        let (task, handle) = Task::run(
                            thumbnails(wanted, move |id, bytes| {
                                Message::NexusSearch(SearchMessage::Thumb(generation, id, bytes))
                            }),
                            |message| message,
                        )
                        .abortable();
                        search.downloading.push(handle.abort_on_drop());
                        return task;
                    }
                    Err(err) => search.error = Some(err),
                }
            }
            SearchMessage::Thumb(generation, id, Some(bytes))
                if generation == search.generation =>
            {
                search.thumbs.insert(id, image::Handle::from_bytes(bytes));
            }
            SearchMessage::Files(mod_id) => return self.toggle_files(mod_id),
            SearchMessage::GotFiles(mod_id, result) => {
                if let Some((open, state)) = search.open.as_mut()
                    && *open == mod_id
                {
                    *state = match result {
                        Ok(files) => FilesState::Got(offered(files)),
                        Err(err) => FilesState::Failed(err),
                    };
                }
            }
            SearchMessage::Close => self.mod_search = None,
            _ => {}
        }
        Task::none()
    }

    /// Shows a mod's files, asking Nexus for them, or hides them.
    fn toggle_files(&mut self, mod_id: u64) -> Task<Message> {
        let key = self.config.nexus_key.clone();
        let persist = self.persist;
        let Some(search) = self.mod_search.as_mut() else {
            return Task::none();
        };
        if search
            .open
            .as_ref()
            .is_some_and(|(open, _)| *open == mod_id)
        {
            search.open = None;
            return Task::none();
        }
        search.open = Some((mod_id, FilesState::Asking));
        let (Some(key), true) = (key, persist) else {
            return Task::none();
        };
        let domain = search.domain.clone();
        Task::perform(
            blocking(move || nexus::files(&key, &domain, mod_id).map_err(|err| format!("{err:#}"))),
            move |result| Message::NexusSearch(SearchMessage::GotFiles(mod_id, result)),
        )
    }

    fn open_search(&mut self, id: &str) -> Task<Message> {
        let Some(domain) = self
            .mods_overview(id)
            .and_then(|overview| overview.nexus.clone())
        else {
            return Task::none();
        };
        let title = self
            .game(id)
            .map_or_else(|| domain.clone(), |game| game.title.clone());
        self.mod_search = Some(Search {
            game: id.to_owned(),
            title,
            domain,
            words: String::new(),
            order: Order::default(),
            hits: Vec::new(),
            total: 0,
            generation: 0,
            asking: false,
            error: None,
            thumbs: std::collections::HashMap::new(),
            open: None,
            downloading: Vec::new(),
            site: None,
        });
        Task::batch([self.run_search(false), operation::focus(input_id())])
    }

    /// Asks Nexus for the search as it stands: from the start, or the next
    /// page after the results shown.
    fn run_search(&mut self, more: bool) -> Task<Message> {
        let adult = self.config.nexus_adult;
        let persist = self.persist;
        let Some(search) = self.mod_search.as_mut() else {
            return Task::none();
        };
        if !more {
            search.generation += 1;
            search.hits.clear();
            search.thumbs.clear();
            search.downloading.clear();
            search.total = 0;
            search.open = None;
        }
        search.asking = true;
        search.error = None;
        let query = Query {
            domain: search.domain.clone(),
            words: search.words.clone(),
            order: search.order,
            adult,
            offset: u32::try_from(search.hits.len()).unwrap_or(u32::MAX),
        };
        if !persist {
            return Task::none();
        }
        let generation = search.generation;
        Task::perform(
            blocking(move || nexus_search::search(&query).map_err(|err| format!("{err:#}"))),
            move |result| Message::NexusSearch(SearchMessage::Found(generation, result)),
        )
    }

    /// A file of a mod found: into the game's mods with Premium, else the
    /// file's Mod manager download in Nexus's window.
    fn download_found(&mut self, mod_id: u64, file_id: u64) -> Task<Message> {
        let Some(search) = self.mod_search.as_ref() else {
            return Task::none();
        };
        if self.nexus_premium() == Some(true) {
            let link = Link {
                domain: search.domain.clone(),
                mod_id,
                file_id,
                key: None,
                expires: None,
            };
            let game = search.game.clone();
            self.status = Some(t!("search-downloading"));
            return self.download_nexus_file(&game, link);
        }
        self.open_site(mod_id, Some(file_id))
    }

    /// Opens a mod's page, or one file's Mod manager download (`nmm=1`),
    /// in Nexus's window; one at a time.
    fn open_site(&mut self, mod_id: u64, file_id: Option<u64>) -> Task<Message> {
        let persist = self.persist;
        let Some(search) = self.mod_search.as_mut() else {
            return Task::none();
        };
        let Some(game) = self.library.as_ref().and_then(|library| {
            library
                .games
                .iter()
                .find(|game| game.id == search.game)
                .cloned()
        }) else {
            return Task::none();
        };
        let mut url = nexus::mod_page_url(&search.domain, mod_id, file_id);
        if file_id.is_some() {
            url.push_str("&nmm=1");
        }
        if let Some(open) = search.site.take() {
            open.close();
        }
        let handle = Handle::default();
        search.site = Some(handle.clone());
        let asked = Asked {
            game: search.game.clone(),
            domain: search.domain.clone(),
            mod_id,
        };
        self.status = Some(t!("search-site-open"));
        if !persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || {
                let folder = mods::downloads_dir(&game)?;
                sign_in_window::open_nexus(&url, &folder, &handle)
            }),
            move |result| {
                Message::NexusSearch(SearchMessage::FromSite(
                    asked.clone(),
                    result.map_err(|err| format!("{err:#}")),
                ))
            },
        )
    }

    /// Takes what Nexus's window gave into the game's mods: a link as any
    /// Mod manager download, a file as one from that mod (found by its
    /// name among the mod's files), or as a mod of its own without a key.
    fn site_gave(
        &mut self,
        asked: Asked,
        result: Result<Option<FromSite>, String>,
    ) -> Task<Message> {
        match result {
            Ok(None) => {
                self.status = None;
                Task::none()
            }
            Ok(Some(FromSite::Link(text))) => match Link::parse(&text) {
                Ok(link) => self.download_nexus_file(&asked.game, link),
                Err(err) => {
                    self.status = Some(format!("{err:#}"));
                    Task::none()
                }
            },
            Ok(Some(FromSite::File(archive))) => {
                let key = self.config.nexus_key.clone();
                self.add_mods(asked.game, t!("mods-unpacking"), move |game| {
                    add_from_site(game, key.as_deref(), &asked.domain, asked.mod_id, &archive)
                })
            }
            Err(err) => {
                self.status = Some(err);
                Task::none()
            }
        }
    }

    /// The Escape and Enter of the sheet, while it is open.
    pub(super) fn search_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.mod_search.as_ref()?;
        match message {
            Message::Back => Some(self.update_search(SearchMessage::Close)),
            Message::Activate => Some(self.update_search(SearchMessage::Search)),
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

    pub(super) fn with_search<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        let Some(search) = &self.mod_search else {
            return page;
        };
        let mut have = HashSet::new();
        let mut have_files = HashSet::new();
        for info in self
            .mods_overview(&search.game)
            .map(|overview| overview.mods.as_slice())
            .unwrap_or_default()
        {
            match &info.nexus {
                Some(source) if source.domain == search.domain => {
                    have.insert(source.mod_id);
                    have_files.insert((source.mod_id, source.file_id));
                }
                Some(_) => {}
                // Added by hand from an archive Nexus named: its name says
                // the mod.
                None => have.extend(nexus::ids_in_name(&info.name).first()),
            }
        }
        let sheet = Sheet {
            search,
            have,
            have_files,
            adult: self.config.nexus_adult,
            has_key: self.config.nexus_key.is_some(),
            premium: self.nexus_premium() == Some(true),
        };
        overlay(page, sheet.card())
    }
}

/// A file Nexus's window downloaded: the mod's file it is, by its name,
/// with the key; else a mod of its own. The download goes either way.
fn add_from_site(
    game: &zendo_core::Game,
    key: Option<&str>,
    domain: &str,
    mod_id: u64,
    archive: &std::path::Path,
) -> anyhow::Result<(mods::Added, String)> {
    // A failed lookup is not "the name carries no ids": the caller says
    // what went wrong instead of adding the mod without its identity.
    let identified = match key {
        Some(key) => nexus::identify(key, domain, archive, Some(mod_id))?,
        None => None,
    };
    if let Some((mod_id, fetched)) = identified {
        let (added, updated) = mods::add_from_nexus(game, &fetched, domain, mod_id)?;
        return Ok((added, added_note(updated, &fetched, &game.title)));
    }
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let added = mods::add(game, archive);
    let _ = std::fs::remove_file(archive);
    Ok((added?, t!("mods-added", name = name)))
}

/// The files worth offering, newest first: those the author keeps current.
fn offered(files: Files) -> Vec<FileInfo> {
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

/// What the sheet shows, with what it needs from the rest of the window.
struct Sheet<'a> {
    search: &'a Search,
    /// The mods from Nexus already in the game's list, and their files.
    have: HashSet<u64>,
    have_files: HashSet<(u64, u64)>,
    adult: bool,
    has_key: bool,
    premium: bool,
}

impl<'a> Sheet<'a> {
    fn card(&self) -> Element<'a, Message> {
        let search = self.search;
        let input = row![
            text_input(&t!("search-placeholder"), &search.words)
                .id(input_id())
                .on_input(|words| Message::NexusSearch(SearchMessage::Words(words)))
                .on_submit(Message::NexusSearch(SearchMessage::Search))
                .size(14)
                .padding([style::SPACE_S, style::SPACE_M])
                .style(style::input),
            button(style::button_text(t!("art-search")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::NexusSearch(SearchMessage::Search)),
        ]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center);
        let mut chips = row![].spacing(style::SPACE_S).align_y(Alignment::Center);
        for order in Order::ALL {
            chips = chips.push(
                button(style::button_text(order_name(order)))
                    .padding(style::BUTTON_SMALL)
                    .style(style::chip(search.order == order))
                    .on_press(Message::NexusSearch(SearchMessage::Order(order))),
            );
        }
        chips = chips.push(space::horizontal().width(16)).push(
            button(style::button_text(t!("search-adult")))
                .padding(style::BUTTON_SMALL)
                .style(style::chip(self.adult))
                .on_press(Message::NexusSearch(SearchMessage::ToggleAdult)),
        );

        let mut body = column![
            row![
                style::sheet_title(t!("search-title", title = search.title.as_str())),
                space::horizontal(),
            ]
            .align_y(Alignment::Center),
            input,
            chips.wrap(),
        ]
        .spacing(style::SPACE_L);
        if !self.has_key {
            body = body.push(text(t!("search-needs-key")).size(13).color(colors().dim));
        } else if !self.premium {
            body = body.push(text(t!("search-not-premium")).size(13).color(colors().dim));
        }
        for hit in &search.hits {
            body = body.push(self.hit_row(hit));
        }
        if (search.hits.len() as u64) < search.total {
            body = body.push(
                button(style::button_text(t!("art-show-more")))
                    .padding(style::BUTTON)
                    .style(style::secondary)
                    .on_press_maybe(
                        (!search.asking).then_some(Message::NexusSearch(SearchMessage::More)),
                    ),
            );
        }

        // Window-tall from the start, so it stays put as results come.
        container(
            column![
                scrollable(body.padding(style::SPACE_XL)).height(Length::Fill),
                container(self.footer()).padding([16, 24]),
            ]
            .spacing(0),
        )
        .max_width(980)
        .height(Length::Fill)
        .style(style::card)
        .into()
    }

    /// A mod found: its picture, name and facts, and its files when open.
    fn hit_row(&self, hit: &'a Hit) -> Element<'a, Message> {
        let picture: Element<'a, Message> = match self.search.thumbs.get(&hit.mod_id) {
            Some(handle) => image(handle.clone())
                .width(160)
                .height(90)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(style::RADIUS_PANEL)
                .into(),
            None => container(space::horizontal())
                .width(160)
                .height(90)
                .style(style::card)
                .into(),
        };
        let mut name = row![text(&hit.name).size(16).font(BOLD)]
            .spacing(style::SPACE_S)
            .align_y(Alignment::Center);
        if !hit.version.is_empty() {
            name = name.push(text(hit.version.clone()).size(13).color(colors().dim));
        }
        if self.have.contains(&hit.mod_id) {
            name = name.push(style::badge(t!("search-have"), colors().good));
        }
        if hit.adult {
            name = name.push(style::badge(t!("search-adult-badge"), colors().bad));
        }
        let mut byline = vec![t!("search-by", author = hit.author.as_str())];
        if let Some(category) = &hit.category {
            byline.push(category.clone());
        }
        if !hit.updated.is_empty() {
            byline.push(t!("search-updated", date = hit.updated.as_str()));
        }
        let mut numbers = vec![
            t!("search-endorsements", count = count(hit.endorsements)),
            t!("search-downloads", count = count(hit.downloads)),
        ];
        if let Some(bytes) = hit.size {
            numbers.push(Size::Bytes(bytes).to_string());
        }
        let open = self
            .search
            .open
            .as_ref()
            .filter(|(id, _)| *id == hit.mod_id)
            .map(|(_, state)| state);
        let mut actions = column![].spacing(style::SPACE_S).align_x(Alignment::End);
        if self.has_key {
            actions = actions.push(
                button(style::button_text(if open.is_some() {
                    t!("search-hide-files")
                } else {
                    t!("search-files")
                }))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::NexusSearch(SearchMessage::Files(hit.mod_id))),
            );
        }
        actions = actions.push(
            button(style::button_text(t!("search-page")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::NexusSearch(SearchMessage::Page(hit.mod_id))),
        );
        let facts = column![
            name.wrap(),
            text(byline.join(" · ")).size(12).color(colors().dim),
            text(&hit.summary).size(13),
            text(numbers.join(" · ")).size(12).color(colors().dim),
        ]
        .spacing(style::SPACE_XS)
        .width(Length::Fill);
        let mut block = column![
            row![picture, facts, actions]
                .spacing(style::SPACE_L)
                .align_y(Alignment::Start)
        ]
        .spacing(style::SPACE_M);
        if let Some(state) = open {
            block = block.push(self.files(hit.mod_id, state));
        }
        container(block)
            .padding(style::SPACE_M)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }

    fn files(&self, mod_id: u64, state: &'a FilesState) -> Element<'a, Message> {
        let list: Element<'a, Message> = match state {
            FilesState::Asking => text(t!("search-asking-files"))
                .size(13)
                .color(colors().dim)
                .into(),
            FilesState::Failed(err) => text(err).size(13).color(colors().bad).into(),
            FilesState::Got(files) if files.is_empty() => text(t!("search-no-files"))
                .size(13)
                .color(colors().dim)
                .into(),
            FilesState::Got(files) => {
                let mut list = column![].spacing(style::SPACE_S);
                for file in files {
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
                    let label = if self.premium {
                        t!("search-download")
                    } else {
                        t!("search-get-on-nexus")
                    };
                    let action: Element<'a, Message> =
                        if self.have_files.contains(&(mod_id, file.file_id)) {
                            text(t!("search-have-file"))
                                .size(13)
                                .color(colors().good)
                                .into()
                        } else {
                            button(style::button_text(label.clone()))
                                .padding(style::BUTTON_SMALL)
                                .style(style::primary)
                                .on_press(Message::NexusSearch(SearchMessage::Download(
                                    mod_id,
                                    file.file_id,
                                )))
                                .into()
                        };
                    list = list.push(
                        row![
                            text(&file.name).size(14).width(Length::Fill),
                            text(facts.join(" · ")).size(12).color(colors().dim),
                            action,
                        ]
                        .spacing(style::SPACE_M)
                        .align_y(Alignment::Center),
                    );
                }
                list.into()
            }
        };
        container(list).padding([0, 176]).into()
    }

    fn footer(&self) -> Element<'a, Message> {
        let search = self.search;
        let mut footer = row![].spacing(style::SPACE_M).align_y(Alignment::Center);
        let line = if search.asking {
            Some((t!("search-asking"), colors().dim))
        } else if let Some(err) = &search.error {
            Some((err.clone(), colors().bad))
        } else if search.total == 0 {
            Some((t!("search-nothing"), colors().dim))
        } else {
            Some((
                t!(
                    "search-count",
                    shown = search.hits.len(),
                    total = search.total
                ),
                colors().dim,
            ))
        };
        if let Some((line, color)) = line {
            footer = footer.push(text(line).size(13).color(color));
        }
        footer = footer.push(space::horizontal()).push(
            button(style::button_text(t!("close")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press(Message::NexusSearch(SearchMessage::Close)),
        );
        footer.into()
    }
}

fn order_name(order: Order) -> String {
    match order {
        Order::Endorsed => t!("search-order-endorsed"),
        Order::Downloaded => t!("search-order-downloaded"),
        Order::Updated => t!("search-order-updated"),
    }
}

/// Nexus's file categories, in words.
fn file_kind(category: &str) -> String {
    match category {
        "MAIN" => t!("search-file-main"),
        "OPTIONAL" => t!("search-file-optional"),
        "UPDATE" => t!("search-file-update"),
        "MISCELLANEOUS" => t!("search-file-misc"),
        other => other.to_lowercase(),
    }
}

/// "950", "12k", "4.5M".
fn count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", n / 1_000),
        _ => format!("{}.{}M", n / 1_000_000, n % 1_000_000 / 100_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn opens_for_a_game_on_nexus_and_closes() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        // No game with mods known on Nexus: nothing opens.
        let _ = zendo.update(Message::NexusSearch(SearchMessage::Open(
            "steam:413150".into(),
        )));
        assert!(zendo.mod_search.is_none());
        zendo.mod_search = Some(Search {
            game: "steam:413150".into(),
            title: "Stardew Valley".into(),
            domain: "stardewvalley".into(),
            words: String::new(),
            order: Order::Endorsed,
            hits: Vec::new(),
            total: 0,
            generation: 1,
            asking: true,
            error: None,
            thumbs: std::collections::HashMap::new(),
            open: None,
            downloading: Vec::new(),
            site: None,
        });
        let found = Found {
            hits: vec![Hit {
                mod_id: 2400,
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
                thumbnail: None,
            }],
            total: 30,
        };
        // An answer to an earlier search is dropped.
        let _ = zendo.update(Message::NexusSearch(SearchMessage::Found(
            0,
            Ok(found.clone()),
        )));
        assert_eq!(zendo.mod_search.as_ref().unwrap().hits.len(), 0);
        let _ = zendo.update(Message::NexusSearch(SearchMessage::Found(1, Ok(found))));
        let search = zendo.mod_search.as_ref().unwrap();
        assert_eq!(
            (search.hits.len(), search.total, search.asking),
            (1, 30, false)
        );
        let _ = zendo.update(Message::NexusSearch(SearchMessage::Order(Order::Updated)));
        let search = zendo.mod_search.as_ref().unwrap();
        assert_eq!((search.generation, search.hits.len()), (2, 0));
        let _ = zendo.update(Message::Back);
        assert!(zendo.mod_search.is_none());
    }
}
