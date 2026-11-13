//! The window: a top bar of pages (Library, Store, Downloads, and Settings
//! with Accounts and About), the library and the store as grids of covers with
//! chips, and a page for each game with its art, actions and details.
//! Keyboard and controller drive all of it; missing covers and wide images
//! download in the background, and store games install through a queue.

mod about;
mod accounts;
mod achievements;
mod anti_cheat;
mod artwork;
mod collections;
mod cloud_saves;
mod dlc;
mod downloads;
mod engines;
mod eos_overlay;
mod format;
mod game;
mod game_mode;
mod gog_changelog;
mod gog_playtime;
mod icons;
mod installer;
mod launch_options;
mod library;
mod hero;
mod local;
mod mods;
mod nexus;
mod nexus_search;
mod prefix;
mod protondb;
mod protons;
mod requirements;
mod running;
mod runtimes;
mod settings;
mod stats;
mod steam;
mod steam_facts;
mod steam_deck;
mod steam_installs;
mod store;
mod store_game;
mod style;
mod themes;
mod top_bar;

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use iced::futures::channel::{mpsc, oneshot};
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::keyboard::{self, Key, key::Named};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{Id, column, container, operation, row, space, text};
use iced::{Element, Event, Length, Size, Subscription, Task, event, window};
use zendo_core::config::Config;
use zendo_core::covers::{Artwork, Fetcher, KeyCheck, Kind};
use zendo_core::owned::Owned;
use zendo_core::{Category, Game, Library, SavedFilter, Sort, Source, State, Status};

use crate::gamepad::{self, Pad};
use crate::i18n::{self, Language, t};
use crate::presence::PresenceHandle;
use crate::tray::{self, Tray, TrayMessage};
use accounts::{AccountMessage, Accounts};
use artwork::{ArtMessage, Picker};
use collections::CollectionsMessage;
use downloads::{DownloadMessage, DownloadsState};
use engines::{EngineMessage, Engines};
use eos_overlay::{OverlayMessage, OverlayState};
use game::{GameView, GameViewMessage};
use hero::HeroMessage;
use game_mode::{GameMode, GameModeMessage};
use installer::{InstallerMessage, InstallerState};
use launch_options::{Sheet, SheetMessage};
use local::{LocalMessage, LocalState};
use mods::{ModsMessage, ModsState};
use nexus::{NexusMessage, NexusState};
use nexus_search::{Search, SearchMessage};
use protondb::RatingState;
use protons::{ProtonMessage, Protons};
use running::{RunMessage, Running};
use runtimes::{RuntimeMessage, RuntimesState};
use settings::Diagnostics;
use stats::{StatsMessage, StatsState};
use steam::{SteamMessage, SteamState};
use steam_installs::{SteamInstallMessage, SteamInstalls};
use store::{StoreMessage, StoreState};
use style::colors;
use themes::{Choice, Themes};
use zendo_core::theme::Theme;

const START_SIZE: Size = Size::new(1360.0, 860.0);

/// How often the file watch looks whether a launcher's files changed.
const WATCH: std::time::Duration = std::time::Duration::from_secs(10);

fn grid_id() -> Id {
    Id::new("grid")
}

fn game_scroll_id() -> Id {
    Id::new("game")
}

fn search_id() -> Id {
    Id::new("search")
}

fn store_search_id() -> Id {
    Id::new("store-search")
}

fn code_id() -> Id {
    Id::new("sign-in-code")
}

fn key_id() -> Id {
    Id::new("steamgriddb-key")
}

/// Zendo's windows' application id (Wayland) or class (X11): `zendo`.
pub const APP_ID: &str = "zendo";

/// Opens the window; `link` is an `nxm://` link it was started for.
pub fn run(fullscreen: bool, link: Option<String>) -> anyhow::Result<()> {
    i18n::set_from_settings();
    let boot = move || {
        let (mut zendo, task) = Zendo::boot();
        let config = Config::load_file().ok();
        let opaque = config.as_ref().is_some_and(|config| config.opaque);
        let theme = config.and_then(|config| config.theme);
        zendo.themes = Themes::chosen(theme.as_deref(), Theme::dir().as_deref());
        if let Some(link) = link.clone() {
            zendo.nexus_pending(link);
        }
        if fullscreen {
            zendo.start_in_game_mode();
        }
        let (id, open) = window::open(window_settings(START_SIZE, fullscreen, opaque));
        zendo.window = Some(id);
        (zendo, Task::batch([task, open.map(Message::WindowReady)]))
    };
    // A daemon, not an application: hiding to the tray closes the window and
    // the tray's click opens a new one (a Wayland app cannot unminimise its
    // own window), so closing the last window must not end the program. Every
    // way out ends in `iced::exit()` instead.
    iced::daemon(boot, Zendo::update, Zendo::view)
        .title("Zendo")
        .font(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").as_slice())
        .font(include_bytes!("../fonts/JetBrainsMono-Bold.ttf").as_slice())
        .default_font(style::FONT)
        .theme(Zendo::theme)
        .style(Zendo::style)
        .subscription(Zendo::subscription)
        .run()?;
    Ok(())
}

/// A window's settings: the size and mode it opens with, and whether the
/// compositor may show the desktop through it (blurred, where it blurs).
/// Fixed at creation, so changing `opaque` opens a new window.
fn window_settings(size: Size, fullscreen: bool, opaque: bool) -> window::Settings {
    window::Settings {
        size,
        transparent: !opaque,
        blur: !opaque,
        min_size: Some(Size::new(library::min_width(), 560.0)),
        fullscreen,
        // A running download asks before the window goes.
        exit_on_close_request: false,
        // The class the compositor matches the window by.
        platform_specific: window::settings::PlatformSpecific {
            application_id: APP_ID.to_owned(),
            ..window::settings::PlatformSpecific::default()
        },
        ..window::Settings::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Page {
    Library,
    /// The game with this id.
    Game(String),
    Store,
    /// The owned game with this id.
    Owned(String),
    Downloads,
    /// The play sessions and what they add up to.
    Stats,
    Settings,
    Accounts,
    About,
}

/// The two pages with a virtualised grid, so a scroll event names its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grid {
    Library,
    Store,
}

#[derive(Debug, Clone, Copy)]
enum Move {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    First,
    Last,
}

/// The library read in a thread: the games, the saved sort, the user's
/// categories, the settings and the saved filters.
type Scanned = (
    Library,
    Sort,
    Vec<Category>,
    Result<Config, String>,
    Vec<SavedFilter>,
);

#[derive(Debug, Clone)]
enum Message {
    /// Boxed: the settings and the library are large.
    Loaded(Box<Scanned>),
    /// Play or details, from the game in Game Mode's hero.
    Hero(HeroMessage),
    Refresh,
    /// An image download finished: the file, none found, or what went wrong.
    Image(String, Kind, Result<Option<PathBuf>, String>),
    ImagesDone,
    Resized(window::Id, Size),
    /// The window iced made for Zendo, before any focus or resize event.
    WindowReady(window::Id),
    Focused(window::Id, bool),
    Scrolled(Grid, Viewport),
    Navigate(Page),
    Move(Move),
    /// The search field's new text.
    Search(String),
    /// Text typed while no field has focus: it goes to the search.
    Type(String),
    Backspace,
    /// Escape, B: clears the search, or goes back to the library.
    Back,
    /// Enter, A: opens the selected game, or plays the one on its page.
    Activate,
    Clicked(String),
    Play,
    SetSort(Sort),
    CycleSort,
    SetSource(Option<Source>),
    /// A category chip; the one already chosen is left.
    SetCategory(Option<String>),
    /// A status chip; the one already chosen is left.
    SetStatus(Option<Status>),
    /// Back to every game: no store, no category, no status.
    ClearFilters,
    /// Puts a game's play status on it, or clears it.
    SetGameStatus(String, Option<Status>),
    /// A saved filter's chip: everything it holds is put on the window.
    ApplyFilter(String),
    Stats(StatsMessage),
    /// Every marked game becomes a favorite, or stops being one.
    BulkPinned(bool),
    /// Every marked game is hidden, or brought back.
    BulkHidden(bool),
    /// Marks the card under the cursor, or leaves it.
    ToggleSelected,
    /// Marks the card at this place in the grid, or leaves it.
    ToggleSelectedAt(String),
    /// Asks again for a game page's cards that could not be asked.
    Retry(String),
    ClearSelection,
    /// Hides the current game, or brings it back when it is hidden.
    ToggleHidden,
    TogglePinned,
    /// A view chip: All, Favorites, Recent or Hidden.
    SetView(library::View),
    /// Hidden games, or back to All.
    ShowHidden,
    /// F11, Select: Game Mode on or off.
    ToggleGameMode,
    GameMode(GameModeMessage),
    FocusSearch,
    /// A folder or web page, opened with `xdg-open`.
    Open(String),
    KeyInput(String),
    ToggleKeyShown,
    SaveKey,
    CheckKey,
    KeyChecked(String, Result<KeyCheck, String>),
    SetDownloadCovers(bool),
    /// Settings' "Update games by themselves".
    SetAutoUpdate(bool),
    /// How many files the engines download at once; `None` leaves it to
    /// them.
    SetWorkers(Option<u32>),
    /// Settings' Window section: hide to the tray on close and on start.
    SetMinimizeToTray(bool),
    /// Settings' Discord section: rich presence on or off.
    SetDiscordPresence(bool),
    /// Settings' Theme section: no translucency behind the window.
    SetOpaque(bool),
    /// Settings: show another group of sections.
    SettingsGroup(settings::Group),
    /// The tray icon was clicked, or one of its menu items was chosen.
    Tray(TrayMessage),
    /// The window's close button was pressed: hide to the tray, or quit.
    CloseRequested,
    /// Time to look whether a launcher's files changed.
    WatchTick,
    /// The signature of the launchers' files, read just now.
    WatchChecked(Vec<(PathBuf, u64)>),
    /// A settings change written on a thread: the settings as they were
    /// before it, what the change was, and the file's settings now.
    ConfigSaved(Box<Config>, settings::Setting, Result<Box<Config>, String>),
    DownloadNow,
    /// Settings' Copy report: gathered on a thread, then copied.
    CopyReport,
    ReportReady(Result<String, String>),
    /// The window's language; `None` follows the system's.
    SetLanguage(Option<Language>),
    /// Settings' Theme section.
    SetTheme(Choice),
    /// Time to look whether the KDE colour scheme changed.
    LookAtScheme,
    Account(AccountMessage),
    Download(DownloadMessage),
    Run(RunMessage),
    Sheet(SheetMessage),
    Local(LocalMessage),
    Collections(CollectionsMessage),
    Art(ArtMessage),
    Mods(ModsMessage),
    Nexus(NexusMessage),
    /// The Nexus search sheet.
    NexusSearch(SearchMessage),
    Installer(InstallerMessage),
    Proton(ProtonMessage),
    Engine(EngineMessage),
    Runtime(RuntimeMessage),
    Overlay(OverlayMessage),
    Steam(SteamMessage),
    SteamInstall(SteamInstallMessage),
    /// What `ProtonDB` says of a game.
    Rating(String, Result<Option<zendo_core::protondb::Rating>, String>),
    /// A tool for a game's Wine prefix.
    Prefix(prefix::PrefixMessage),
    /// GOG's total for a game, in minutes.
    GogTotal(String, Result<Option<u64>, String>),
    /// A game's achievements and the player's unlocks.
    Achievements(
        String,
        Result<Option<zendo_core::achievements::Achievements>, String>,
    ),
    /// What a game asks of the computer.
    Requirements(
        String,
        Result<Option<zendo_core::requirements::Requirements>, String>,
    ),
    /// What Steam's store says about a game.
    Facts(
        String,
        Result<Option<zendo_core::steam_facts::Facts>, String>,
    ),
    /// Valve's Steam Deck review of a game.
    Deck(
        String,
        Result<Option<zendo_core::steam_deck::Report>, String>,
    ),
    /// What `AreWeAntiCheatYet` says of a game.
    AntiCheat(
        String,
        Result<Option<zendo_core::areweanticheatyet::Entry>, String>,
    ),
    /// GOG's changelog for a game.
    GogChangelog(
        String,
        Result<Vec<zendo_core::gog_changelog::Entry>, String>,
    ),
    Store(StoreMessage),
    GameView(GameViewMessage),
    /// Ctrl+V with no field focused: the clipboard goes to the field the
    /// page is for.
    PasteClipboard,
    Pasted(Option<String>),
    Pad(Pad),
}

/// The Settings page's key field and what was last said about it.
#[derive(Debug, Default)]
struct KeyForm {
    value: String,
    shown: bool,
    /// The key `SteamGridDB` is being asked about.
    checking: Option<String>,
    /// Keep the key once `SteamGridDB` has answered.
    save_after_check: bool,
    note: Option<(String, bool)>,
}

#[derive(Debug, Default)]
struct ImageDownloads {
    left: usize,
    failed: usize,
    /// More games came in while these downloaded; look again after.
    again: bool,
}

#[expect(clippy::struct_excessive_bools, reason = "independent window flags")]
struct Zendo {
    /// `None` while the first scan runs.
    library: Option<Library>,
    scanning: bool,
    page: Page,
    search: String,
    /// The store chip chosen; `None` is "All".
    source: Option<Source>,
    /// The category chip chosen; `None` is every category.
    category: Option<String>,
    /// The status chip chosen; `None` is every status.
    play_status: Option<Status>,
    /// Index into the games the filters let through.
    selected: usize,
    /// The games marked in the grid, for assigning all at once.
    selected_games: BTreeSet<String>,
    columns: usize,
    /// The window's width, for the game page's columns.
    window_width: f32,
    /// The window's last size, for the one opened again from the tray.
    window_size: Size,
    /// The open window; `None` while hidden to the tray.
    window: Option<window::Id>,
    focused: bool,
    fullscreen: bool,
    /// Game Mode, while it is on (the window is then full screen).
    game_mode: Option<GameMode>,
    /// Hidden to the tray (not minimised), waiting for a tray click.
    hidden: bool,
    scroll_top: f32,
    viewport_height: f32,
    status: Option<String>,
    sort: Sort,
    view: library::View,
    images: ImageDownloads,
    downloads: DownloadsState,
    running: Running,
    /// A game's launch options, open over its page.
    sheet: Option<Sheet>,
    /// Games added by hand: the Add game sheet and Remove.
    local: LocalState,
    /// The Collections sheet, open over a page.
    collections_sheet: Option<collections::Collections>,
    /// The user's categories as last read or written, with their games.
    categories: Vec<Category>,
    /// The Saved filters sheet, open over a page.
    /// The user's saved filters as last read or written.
    filters: Vec<SavedFilter>,
    /// Every play session kept, oldest first; the window is the only writer,
    /// so this stays the file's content.
    sessions: Vec<zendo_core::sessions::Session>,
    /// The Stats page: the period it shows.
    stats: StatsState,
    /// The art picker, open over a game's page.
    picker: Option<Picker>,
    /// The Nexus search, open over a game's page.
    mod_search: Option<Search>,
    /// Each game's mods, on its page.
    mods: ModsState,
    /// Nexus Mods: the key, links coming in and mods downloading.
    nexus: NexusState,
    /// A mod's FOMOD installer, open over the page.
    installer: InstallerState,
    /// The Proton section of Settings.
    protons: Protons,
    engines: Engines,
    runtimes: RuntimesState,
    overlay: OverlayState,
    cloud: cloud_saves::CloudState,
    steam: SteamState,
    steam_installs: SteamInstalls,
    /// `ProtonDB`'s rating of each game whose page was opened.
    ratings: std::collections::HashMap<String, RatingState>,
    /// `AreWeAntiCheatYet`'s word on each game whose page was opened.
    anti_cheat: std::collections::HashMap<String, anti_cheat::AntiCheatState>,
    /// Valve's Steam Deck review of each game whose page was opened.
    decks: std::collections::HashMap<String, steam_deck::DeckState>,
    /// The system requirements of each game whose page was opened.
    requirements: std::collections::HashMap<String, requirements::RequirementsState>,
    /// What Steam's store says about each game whose page was opened or that
    /// was in Game Mode's hero.
    facts: std::collections::HashMap<String, steam_facts::FactsState>,
    /// The achievements of each game whose page was opened.
    achievements: std::collections::HashMap<String, achievements::AchievementsState>,
    /// The tools run in the Windows games' prefixes.
    prefix: prefix::PrefixState,
    /// The GOG games whose total GOG was asked for, this session.
    gog_totals: std::collections::HashSet<String>,
    /// GOG's changelogs asked for this session, by game.
    changelogs: std::collections::HashMap<String, gog_changelog::Changelog>,
    config: Config,
    /// The window's colours and where they come from.
    themes: Themes,
    key: KeyForm,
    /// The group of sections Settings shows.
    settings_group: settings::Group,
    /// Settings' Diagnostics section.
    diagnostics: Diagnostics,
    accounts: Accounts,
    store: StoreState,
    /// What is open on the game page.
    game_view: GameView,
    /// The tray icon, when the Window setting is on and it was made.
    tray: Option<Tray>,
    /// Discord rich presence, when the Discord setting is on.
    presence: Option<PresenceHandle>,
    /// The launchers' files' signature as last seen, for the rescan watch.
    watch_signature: Option<Vec<(PathBuf, u64)>>,
    /// Off in tests, so they never touch the user's files or the network.
    persist: bool,
}

impl Zendo {
    fn boot() -> (Self, Task<Message>) {
        let zendo = Self {
            library: None,
            scanning: true,
            page: Page::Library,
            search: String::new(),
            source: None,
            category: None,
            play_status: None,
            selected: 0,
            selected_games: BTreeSet::new(),
            columns: library::columns_for(START_SIZE.width),
            window_width: START_SIZE.width,
            window_size: START_SIZE,
            window: None,
            focused: false,
            fullscreen: false,
            game_mode: None,
            hidden: false,
            scroll_top: 0.0,
            viewport_height: START_SIZE.height,
            status: None,
            sort: Sort::default(),
            view: library::View::default(),
            images: ImageDownloads::default(),
            downloads: DownloadsState::default(),
            running: Running::default(),
            sheet: None,
            local: LocalState::default(),
            collections_sheet: None,
            categories: Vec::new(),
            filters: Vec::new(),
            sessions: Vec::new(),
            stats: StatsState::default(),
            picker: None,
            mod_search: None,
            mods: ModsState::default(),
            nexus: NexusState::default(),
            installer: InstallerState::default(),
            protons: Protons::default(),
            engines: Engines::default(),
            runtimes: RuntimesState::default(),
            overlay: OverlayState::default(),
            cloud: cloud_saves::CloudState::load(),
            steam: SteamState::default(),
            steam_installs: SteamInstalls::default(),
            ratings: std::collections::HashMap::new(),
            anti_cheat: std::collections::HashMap::new(),
            decks: std::collections::HashMap::new(),
            requirements: std::collections::HashMap::new(),
            facts: std::collections::HashMap::new(),
            achievements: std::collections::HashMap::new(),
            prefix: prefix::PrefixState::default(),
            gog_totals: std::collections::HashSet::new(),
            changelogs: std::collections::HashMap::new(),
            config: Config::default(),
            themes: Themes::default(),
            key: KeyForm::default(),
            settings_group: settings::Group::default(),
            diagnostics: Diagnostics::default(),
            accounts: Accounts::default(),
            store: StoreState::default(),
            game_view: GameView::default(),
            tray: None,
            presence: None,
            watch_signature: None,
            persist: true,
        };
        let tasks = [
            scan(),
            accounts::check_all(),
            store::load_cached(),
            downloads::load(),
            stats::load(),
        ];
        (zendo, Task::batch(tasks))
    }

    /// How many cards the page's grid has.
    fn grid_len(&self) -> usize {
        match self.page {
            Page::Library => self.visible().len(),
            Page::Store => self.store_visible().len(),
            _ => 0,
        }
    }

    /// The page's cursor and how far its grid is scrolled.
    fn cursor(&mut self) -> (&mut usize, &mut f32) {
        if self.page == Page::Store {
            (&mut self.store.selected, &mut self.store.scroll_top)
        } else {
            (&mut self.selected, &mut self.scroll_top)
        }
    }

    /// The games the grid shows, in its order.
    fn visible(&self) -> Vec<&Game> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        let words: Vec<String> = self
            .search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        // The lowered title once per game, not twice per comparison.
        let now = now();
        let mut games: Vec<(&Game, String)> = library
            .games
            .iter()
            .filter(|game| self.view.shows(game, now))
            .filter(|game| self.source.is_none_or(|source| game.source == source))
            .filter(|game| {
                self.category
                    .as_ref()
                    .is_none_or(|name| game.categories.contains(name))
            })
            .filter(|game| {
                self.play_status
                    .is_none_or(|status| game.status == Some(status))
            })
            .map(|game| (game, game.title.to_lowercase()))
            .filter(|(_, title)| words.iter().all(|word| title.contains(word.as_str())))
            .collect();
        games.sort_by(|(a, at), (b, bt)| self.sort.compare_with_titles(a, b, at, bt));
        games.into_iter().map(|(game, _)| game).collect()
    }

    fn selected_id(&self) -> Option<String> {
        self.visible()
            .get(self.selected)
            .map(|game| game.id.clone())
    }

    /// The game the actions apply to: the page's, or the selected card's.
    fn current_id(&self) -> Option<String> {
        match &self.page {
            Page::Game(id) => Some(id.clone()),
            Page::Library => self.selected_id(),
            Page::Store
            | Page::Owned(_)
            | Page::Downloads
            | Page::Stats
            | Page::Accounts
            | Page::Settings
            | Page::About => None,
        }
    }

    fn game(&self, id: &str) -> Option<&Game> {
        self.library.as_ref()?.find(id)
    }

    fn game_mut(&mut self, id: &str) -> Option<&mut Game> {
        self.library
            .as_mut()?
            .games
            .iter_mut()
            .find(|game| game.id == id)
    }

    /// Keeps the cursor on the same game after the order or filters change.
    fn select(&mut self, id: Option<&str>) -> Task<Message> {
        let visible = self.visible();
        let index = id.and_then(|id| visible.iter().position(|game| game.id == id));
        self.selected = index.unwrap_or_else(|| self.selected.min(visible.len().saturating_sub(1)));
        self.reveal()
    }

    /// Writes a change to the state file, saying so when it fails.
    fn save(&mut self, change: impl FnOnce(&mut State)) {
        if !self.persist {
            return;
        }
        if let Err(err) = State::update(change) {
            self.status = Some(format!("{err:#}"));
        }
    }

    /// What an open sheet, form, picker or chooser does with a key press.
    /// Asked in the order `view` draws them, the topmost first, so the
    /// surface the user sees is the one that hears the key.
    fn overlay_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.installer_takes(message)
            .or_else(|| self.chooser_takes(message))
            .or_else(|| self.search_takes(message))
            .or_else(|| self.picker_takes(message))
            .or_else(|| self.collections_takes(message))
            .or_else(|| self.form_takes(message))
            .or_else(|| self.sheet_takes(message))
            .or_else(|| self.game_mode_takes(message))
    }

    fn resized(&mut self, id: window::Id, size: Size) -> Task<Message> {
        if self.hidden {
            return Task::none();
        }
        self.window = Some(id);
        self.columns = library::columns_for(size.width);
        self.window_width = size.width;
        if !self.fullscreen {
            self.window_size = size;
        }
        self.reveal()
    }

    /// Remembers whether the window has focus: the controller moves Zendo
    /// only while it does.
    fn focus_changed(&mut self, _id: window::Id, focused: bool) -> Task<Message> {
        self.focused = focused;
        Task::none()
    }

    /// Routes the message, then asks for the facts of the game that now
    /// has Game Mode's hero, whatever moved it.
    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.route(message);
        if self.game_mode.is_none() {
            return task;
        }
        Task::batch([task, self.want_hero_facts()])
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per message, which the router keeps together"
    )]
    fn route(&mut self, message: Message) -> Task<Message> {
        if let Some(task) = self.overlay_takes(&message) {
            return task;
        }
        match message {
            Message::Loaded(loaded) => self.loaded(*loaded),
            Message::Hero(message) => self.update_hero(message),
            Message::Refresh => self.refresh(),
            Message::Image(id, kind, result) => self.image_arrived(&id, kind, result),
            Message::ImagesDone => self.images_done(),
            Message::Resized(id, size) => self.resized(id, size),
            Message::WindowReady(id) => {
                self.window = Some(id);
                Task::none()
            }
            Message::Focused(id, focused) => self.focus_changed(id, focused),
            Message::Scrolled(grid, viewport) => self.scrolled(grid, viewport),
            Message::Navigate(page) => self.navigate(page),
            Message::Move(step) if matches!(self.page, Page::Library | Page::Store) => {
                self.step(step)
            }
            Message::Move(step) if self.page == Page::Stats => self.stats_step(step),
            Message::Stats(message) => self.update_stats(message),
            Message::Search(search) => {
                self.search = search;
                self.filter_changed()
            }
            Message::Type(typed) if self.page == Page::Settings => self.type_key(&typed),
            Message::Type(typed) if self.page == Page::Accounts => self.type_code(&typed),
            Message::Type(typed) => self.edit_search(|search| search.push_str(&typed)),
            Message::Backspace => self.edit_search(|search| {
                search.pop();
            }),
            Message::Back => self.back(),
            Message::Activate => self.activate(),
            Message::Store(message) => self.update_store(message),
            Message::GameView(message) => self.update_game_view(message),
            Message::Clicked(id) => self.clicked(id),
            Message::Retry(id) => self.retry_cards(&id),
            Message::Play => self.play(),
            Message::SetSort(sort) => self.set_sort(sort),
            Message::CycleSort if self.page == Page::Library => self.set_sort(self.sort.next()),
            Message::SetSource(source) => self.filter(|zendo| zendo.source = source),
            Message::SetCategory(category) => self.filter(|zendo| {
                zendo.category = if zendo.category == category {
                    None
                } else {
                    category
                };
            }),
            Message::SetStatus(status) => self.filter(|zendo| {
                zendo.play_status = if zendo.play_status == status {
                    None
                } else {
                    status
                };
            }),
            Message::ClearFilters => self.filter(|zendo| {
                zendo.source = None;
                zendo.category = None;
                zendo.play_status = None;
            }),
            Message::SetGameStatus(id, status) => self.set_game_status(&id, status),
            Message::ApplyFilter(name) => self.apply_filter(&name),
            Message::BulkPinned(pinned) => self.bulk_pinned(pinned),
            Message::BulkHidden(hidden) => self.bulk_hidden(hidden),
            Message::ToggleSelected | Message::ToggleSelectedAt(_) | Message::ClearSelection => {
                self.selecting(&message)
            }
            Message::ToggleHidden => self.toggle_hidden(),
            Message::TogglePinned => self.toggle_pinned(),
            Message::SetView(view) => self.filter(|zendo| zendo.view = view),
            // The key and the Start button: the hidden games, and back.
            Message::ShowHidden => self.filter(|zendo| {
                zendo.view = if zendo.view == library::View::Hidden {
                    library::View::All
                } else {
                    library::View::Hidden
                };
            }),
            Message::ToggleGameMode => self.toggle_game_mode(),
            Message::GameMode(message) => self.update_game_mode(message),
            Message::Tray(message) => self.tray_message(message),
            Message::CloseRequested => self.close_requested(),
            Message::WatchTick => self.watch_tick(),
            Message::WatchChecked(signature) => self.watch_checked(signature),
            // From another page the search is not on screen to focus yet;
            // typing reaches it there all the same.
            // The Store has a search field of its own: Ctrl+F there focuses
            // it rather than leaving the page.
            Message::FocusSearch if self.page == Page::Store => operation::focus(store_search_id()),
            Message::FocusSearch if self.page != Page::Library => self.navigate(Page::Library),
            Message::FocusSearch => operation::focus(search_id()),
            Message::Open(target) => self.open_target(&target),
            Message::KeyInput(_)
            | Message::ToggleKeyShown
            | Message::SaveKey
            | Message::CheckKey
            | Message::KeyChecked(..)
            | Message::SetDownloadCovers(_)
            | Message::SetAutoUpdate(_)
            | Message::SetWorkers(_)
            | Message::SetMinimizeToTray(_)
            | Message::SetDiscordPresence(_)
            | Message::SetOpaque(_)
            | Message::SettingsGroup(_)
            | Message::DownloadNow
            | Message::CopyReport
            | Message::ReportReady(_)
            | Message::ConfigSaved(..)
            | Message::SetLanguage(_) => self.update_settings(message),
            Message::SetTheme(choice) => self.set_theme(&choice),
            Message::LookAtScheme => self.look_at_scheme(),
            Message::Account(message) => self.update_account(message),
            Message::Download(message) => self.update_download(message),
            Message::Run(message) => self.update_running(message),
            Message::Sheet(message) => self.update_sheet(message),
            Message::Local(message) => self.update_local(message),
            Message::Collections(message) => self.update_collections(message),
            Message::Art(message) => self.update_art(message),
            Message::NexusSearch(message) => self.update_search(message),
            Message::Mods(message) => self.update_mods(message),
            Message::Nexus(message) => self.update_nexus(message),
            Message::Installer(message) => self.update_installer(message),
            Message::Proton(message) => self.update_protons(message),
            Message::Engine(message) => self.update_engines(message),
            Message::Runtime(message) => self.update_runtimes(message),
            Message::Overlay(message) => self.update_overlay(message),
            Message::Steam(message) => self.update_steam(message),
            Message::SteamInstall(message) => self.update_steam_installs(message),
            Message::Rating(id, result) => self.rating_arrived(id, result),
            Message::Prefix(message) => self.update_prefix(message),
            Message::GogTotal(id, result) => self.gog_total_arrived(&id, &result),
            Message::AntiCheat(id, result) => self.anti_cheat_arrived(id, result),
            Message::Deck(id, result) => self.deck_arrived(id, result),
            Message::Requirements(id, result) => self.requirements_arrived(id, result),
            Message::Facts(id, result) => self.facts_arrived(id, result),
            Message::Achievements(id, result) => self.achievements_arrived(id, result),
            Message::GogChangelog(id, result) => self.changelog_arrived(id, result),
            Message::PasteClipboard => iced::clipboard::read().map(Message::Pasted),
            Message::Pasted(Some(pasted)) => self.pasted(&pasted),
            Message::Pad(_) if !self.focused => Task::none(),
            Message::Pad(pad) => self.pad(pad),
            Message::Move(_) | Message::CycleSort | Message::Pasted(None) => Task::none(),
        }
    }

    /// Opens a link or a folder in the program the desktop chooses; tests
    /// open nothing.
    fn open_target(&mut self, target: &str) -> Task<Message> {
        if self.persist
            && let Err(err) = open(target)
        {
            self.status = Some(err);
        }
        Task::none()
    }

    /// Reads the library again, unless it is being read already.
    fn refresh(&mut self) -> Task<Message> {
        if self.scanning {
            return Task::none();
        }
        self.scanning = true;
        self.selected_games.clear();
        self.status = Some(t!("library-looking"));
        scan()
    }

    fn loaded(
        &mut self,
        (library, sort, categories, config, filters): Scanned,
    ) -> Task<Message> {
        let first = self.library.is_none();
        let mut id = self.selected_id();
        let after = self.local.after_scan.take();
        self.library = Some(library);
        self.categories = categories;
        self.filters = filters;
        if let Some(library) = &self.library {
            let present: HashSet<&str> =
                library.games.iter().map(|game| game.id.as_str()).collect();
            self.selected_games
                .retain(|id| present.contains(id.as_str()));
        }
        self.scanning = false;
        self.status = None;
        let mut nexus = Task::none();
        if first {
            self.sort = sort;
            match config {
                Ok(config) => {
                    i18n::set(Language::chosen(config.language.as_deref()));
                    if self.persist {
                        self.themes =
                            Themes::chosen(config.theme.as_deref(), Theme::dir().as_deref());
                    }
                    self.key.value = config.steamgriddb_key.clone().unwrap_or_default();
                    self.config = config;
                    if self.persist && self.config.minimize_to_tray {
                        self.tray = Tray::spawn();
                    }
                }
                Err(err) => self.status = Some(err),
            }
            nexus = Task::batch([self.nexus_loaded(), self.steam_loaded()]);
        }
        // A game just added or edited opens.
        if let Some((Some(open), _)) = &after
            && self.game(open).is_some()
        {
            self.page = Page::Game(open.clone());
            id = Some(open.clone());
        }
        // A game whose page is open may be gone after a rescan.
        if let Page::Game(id) = &self.page
            && self.game(id).is_none()
        {
            self.page = Page::Library;
        }
        let select = self.select(id.as_deref());
        if let Some((_, note)) = after {
            self.status = Some(note);
        }
        let info = match &self.page {
            Page::Game(id) => {
                let id = id.clone();
                Task::batch([
                    self.want_info(&id),
                    self.want_rating(&id),
                    self.want_gog_total(&id),
                    self.want_changelog(&id),
                    self.want_achievements(&id),
                    self.want_known_fixes(&id),
                    self.want_mods(&id),
                ])
            }
            _ => Task::none(),
        };
        Task::batch([
            select,
            self.download_images(),
            self.check_new_updates(),
            info,
            nexus,
        ])
    }

    /// The clipboard, pasted with no field focused: into the page's field.
    fn pasted(&mut self, pasted: &str) -> Task<Message> {
        let pasted = pasted.trim();
        match self.page {
            Page::Library | Page::Store => self.edit_search(|search| search.push_str(pasted)),
            Page::Settings if self.settings_group == settings::Group::Keys => {
                pasted.clone_into(&mut self.key.value);
                self.key.note = None;
                Task::none()
            }
            Page::Accounts => self.update_account(AccountMessage::Paste(pasted.to_owned())),
            Page::Settings
            | Page::Game(_)
            | Page::Owned(_)
            | Page::Downloads
            | Page::Stats
            | Page::About => Task::none(),
        }
    }

    /// Enter: what it means depends on the page.
    fn activate(&mut self) -> Task<Message> {
        match &self.page {
            Page::Library => self.open_selected(),
            Page::Game(_) => self.play(),
            Page::Store => self.open_owned(),
            Page::Owned(id) if self.is_installed(id) => self.navigate(Page::Game(id.clone())),
            Page::Owned(id) => match self.install_job(&id.clone()) {
                Some(job) => self.update_download(DownloadMessage::Queue(job)),
                // Steam installs its own, through its install dialog.
                None => match (id.strip_prefix("steam:"), self.store.owned(id)) {
                    (Some(app), Some(owned)) if !self.is_installed(id) => {
                        let message =
                            SteamInstallMessage::Install(app.to_owned(), owned.title.clone());
                        self.update_steam_installs(message)
                    }
                    _ => Task::none(),
                },
            },
            Page::Downloads if self.downloads.confirm_close => {
                self.update_download(DownloadMessage::CloseAnyway)
            }
            Page::Downloads | Page::Stats | Page::Accounts | Page::Settings | Page::About => {
                Task::none()
            }
        }
    }

    /// Changes the search of the page's grid; other pages have none.
    fn edit_search(&mut self, edit: impl FnOnce(&mut String)) -> Task<Message> {
        match self.page {
            Page::Library => {
                edit(&mut self.search);
                self.filter_changed()
            }
            Page::Store => {
                edit(&mut self.store.search);
                self.store_filter_changed()
            }
            _ => Task::none(),
        }
    }

    fn navigate(&mut self, page: Page) -> Task<Message> {
        if self.page == page {
            return Task::none();
        }
        if self.page == Page::Library {
            self.selected_games.clear();
        }
        self.page = page;
        self.status = None;
        self.downloads.confirm_uninstall = None;
        self.game_view = GameView::default();
        match &self.page {
            Page::Game(id) | Page::Owned(id) => {
                let id = id.clone();
                self.load_prefix_components(&id);
                Task::batch([
                    operation::scroll_to(game_scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                    self.want_info(&id),
                    self.want_rating(&id),
                    self.want_gog_total(&id),
                    self.want_changelog(&id),
                    self.want_achievements(&id),
                    self.want_known_fixes(&id),
                    self.want_mods(&id),
                ])
            }
            // The grid is built anew; put it back where the cursor is.
            Page::Library | Page::Store => {
                let y = *self.cursor().1;
                operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y })
            }
            Page::Settings => {
                if self.persist {
                    self.themes.reload(Theme::dir().as_deref());
                }
                self.want_overlay();
                self.settings_paths();
                Task::batch([
                    self.want_engines(),
                    self.want_protons(),
                    self.want_runtimes(),
                ])
            }
            Page::Accounts | Page::Downloads | Page::Stats | Page::About => Task::none(),
        }
    }

    fn back(&mut self) -> Task<Message> {
        match self.page {
            Page::Library if !self.selected_games.is_empty() => {
                self.selected_games.clear();
                Task::none()
            }
            Page::Library if self.search.is_empty() => Task::none(),
            Page::Library => {
                self.search.clear();
                self.filter_changed()
            }
            Page::Store if self.store.search.is_empty() => Task::none(),
            Page::Store => {
                self.store.search.clear();
                self.store_filter_changed()
            }
            Page::Owned(_) => self.navigate(Page::Store),
            Page::Downloads if self.downloads.confirm_close => {
                self.update_download(DownloadMessage::StayOpen)
            }
            _ => self.navigate(Page::Library),
        }
    }

    fn open_selected(&mut self) -> Task<Message> {
        match self.selected_id() {
            Some(id) => self.navigate(Page::Game(id)),
            None => Task::none(),
        }
    }

    fn set_sort(&mut self, sort: Sort) -> Task<Message> {
        self.save(|state| state.sort = sort);
        self.filter(|zendo| zendo.sort = sort)
    }

    /// Changes what the grid shows, keeping the cursor on its game.
    fn filter(&mut self, change: impl FnOnce(&mut Self)) -> Task<Message> {
        let id = self.selected_id();
        change(self);
        self.select(id.as_deref())
    }

    /// Opens the game of the card clicked, by its id: the list can change
    /// between the click and this message.
    fn clicked(&mut self, id: String) -> Task<Message> {
        if let Some(index) = self.visible().iter().position(|game| game.id == id) {
            self.selected = index;
        }
        self.navigate(Page::Game(id))
    }

    /// What the marks in the grid do: the cursor's card and a clicked one
    /// are marked or left, and anything else clears them all.
    fn selecting(&mut self, message: &Message) -> Task<Message> {
        match message {
            Message::ToggleSelected => {
                let id = self.selected_id();
                self.toggle_selected(id.as_deref());
            }
            Message::ToggleSelectedAt(id) => {
                self.toggle_selected(Some(id));
            }
            _ => self.selected_games.clear(),
        }
        Task::none()
    }

    /// Marks a game for assigning, or unmarks it. Only the library's grid
    /// marks.
    fn toggle_selected(&mut self, id: Option<&str>) {
        let Some(id) = id.filter(|_| self.page == Page::Library) else {
            return;
        };
        if !self.selected_games.remove(id) {
            self.selected_games.insert(id.to_owned());
        }
    }

    /// How far the page's grid is scrolled, and how tall its window is.
    fn scrolled(&mut self, grid: Grid, viewport: Viewport) -> Task<Message> {
        // A viewport event from the page being left says nothing about the
        // one being entered.
        if grid != self.grid_page() {
            return Task::none();
        }
        *self.cursor().1 = viewport.absolute_offset().y;
        self.viewport_height = viewport.bounds().height;
        Task::none()
    }

    /// Which grid the page shows, for its scroll events.
    fn grid_page(&self) -> Grid {
        if self.page == Page::Store {
            Grid::Store
        } else {
            Grid::Library
        }
    }

    /// The window's close button: with the tray on, hide to it; otherwise
    /// the usual close, which asks before a running download is lost.
    fn close_requested(&mut self) -> Task<Message> {
        if self.config.minimize_to_tray && self.tray.is_some() {
            self.hide_to_tray()
        } else {
            self.update_download(DownloadMessage::CloseRequested)
        }
    }

    /// A click or a menu item from the tray.
    fn tray_message(&mut self, message: TrayMessage) -> Task<Message> {
        match message {
            TrayMessage::Toggle => self.toggle_window(),
            TrayMessage::Open => self.show_from_tray(),
            TrayMessage::Quit if self.hidden && self.downloads.is_running() => {
                // The question about the download is in the window, so the
                // window comes back to ask it.
                Task::batch([
                    self.show_from_tray(),
                    self.update_download(DownloadMessage::CloseRequested),
                ])
            }
            TrayMessage::Quit => self.update_download(DownloadMessage::CloseRequested),
        }
    }

    /// Shows the window, or hides it to the tray, as a tray click says.
    fn toggle_window(&mut self) -> Task<Message> {
        if self.hidden {
            self.show_from_tray()
        } else {
            self.hide_to_tray()
        }
    }

    /// Hides the window to the tray by closing it, leaving the icon to
    /// bring a new one back. Not minimising, since a Wayland app cannot
    /// undo that, and not `Mode::Hidden`, which winit's Wayland backend
    /// ignores. Closing also takes it off the taskbar.
    fn hide_to_tray(&mut self) -> Task<Message> {
        let Some(id) = self.window.take() else {
            return Task::none();
        };
        self.hidden = true;
        self.focused = false;
        window::close(id)
    }

    /// Hides the window to the tray as a game starts, when the Window
    /// setting asks for it and the tray is there.
    fn hide_on_launch(&mut self) -> Task<Message> {
        if self.config.minimize_to_tray && self.tray.is_some() && !self.hidden {
            self.hide_to_tray()
        } else {
            Task::none()
        }
    }

    /// Reads the launchers' files' signature on a thread, to compare it
    /// with the last seen.
    fn watch_tick(&mut self) -> Task<Message> {
        if !self.persist || self.scanning {
            return Task::none();
        }
        Task::perform(
            blocking(zendo_core::watch::signature),
            Message::WatchChecked,
        )
    }

    /// The signature read: rescan when it differs from the last seen.
    fn watch_checked(&mut self, signature: Vec<(PathBuf, u64)>) -> Task<Message> {
        match &self.watch_signature {
            None => {
                self.watch_signature = Some(signature);
                Task::none()
            }
            Some(previous) if *previous == signature => Task::none(),
            Some(_) => {
                self.watch_signature = Some(signature);
                self.refresh()
            }
        }
    }

    /// Brings the window back from the tray: a new window, as big as the
    /// last one, since a Wayland app cannot unminimise its own. Checked on
    /// `KWin` to come up in front with focus.
    fn show_from_tray(&mut self) -> Task<Message> {
        self.hidden = false;
        if let Some(id) = self.window {
            return window::gain_focus(id);
        }
        let (id, open) = window::open(window_settings(
            self.window_size,
            self.fullscreen,
            self.config.opaque,
        ));
        self.window = Some(id);
        // The new grid's scrollable starts at the top; the next resize
        // scrolls the selected card back into view.
        self.scroll_top = 0.0;
        self.store.scroll_top = 0.0;
        open.map(Message::WindowReady)
    }

    /// A controller button, by page: on a game's page Y pins and X hides.
    fn pad(&mut self, pad: Pad) -> Task<Message> {
        let on_game = matches!(self.page, Page::Game(_));
        let in_game_mode = self.game_mode_screen();
        self.update(match pad {
            // On the Game Mode screen: Start opens the menu, and the
            // shoulder buttons change the shelf.
            Pad::ShowHidden if in_game_mode => Message::GameMode(GameModeMessage::OpenMenu),
            Pad::PrevShelf => Message::GameMode(GameModeMessage::CycleShelf(-1)),
            Pad::NextShelf => Message::GameMode(GameModeMessage::CycleShelf(1)),
            Pad::Left => Message::Move(Move::Left),
            Pad::Right => Message::Move(Move::Right),
            Pad::Up => Message::Move(Move::Up),
            Pad::Down => Message::Move(Move::Down),
            Pad::Activate => Message::Activate,
            Pad::Back => Message::Back,
            Pad::Sort if on_game => Message::TogglePinned,
            Pad::Sort => Message::CycleSort,
            Pad::Hide => Message::ToggleHidden,
            Pad::ShowHidden => Message::ShowHidden,
            Pad::ToggleGameMode => Message::ToggleGameMode,
        })
    }

    fn image_arrived(
        &mut self,
        id: &str,
        kind: Kind,
        result: Result<Option<PathBuf>, String>,
    ) -> Task<Message> {
        self.images.left = self.images.left.saturating_sub(1);
        match result {
            Ok(Some(path)) => self.set_image(id, kind, &path),
            Ok(None) => {}
            Err(_) => self.images.failed += 1,
        }
        Task::none()
    }

    /// Shows a game's new image, in the library and the store, which can
    /// both hold it.
    fn set_image(&mut self, id: &str, kind: Kind, path: &Path) {
        if let Some(game) = self.game_mut(id) {
            match kind {
                Kind::Cover => game.cover = Some(path.to_path_buf()),
                Kind::Hero => game.hero = Some(path.to_path_buf()),
            }
        }
        for owned in self.store.each_mut(id) {
            match kind {
                Kind::Cover => owned.cover = Some(path.to_path_buf()),
                Kind::Hero => owned.hero = Some(path.to_path_buf()),
            }
        }
    }

    fn images_done(&mut self) -> Task<Message> {
        self.images.left = 0;
        if self.images.failed > 0 {
            self.status = Some(t!("images-failed", count = self.images.failed));
        }
        if std::mem::take(&mut self.images.again) {
            self.download_images()
        } else {
            Task::none()
        }
    }

    fn toggle_hidden(&mut self) -> Task<Message> {
        let Some(id) = self.current_id() else {
            return Task::none();
        };
        let Some(game) = self.game_mut(&id) else {
            return Task::none();
        };
        game.hidden = !game.hidden;
        let (hidden, title) = (game.hidden, game.title.clone());
        self.save(|state| state.set_hidden(&id, hidden));
        self.status = Some(if hidden {
            t!("status-hid", title = title)
        } else {
            t!("status-unhid", title = title)
        });
        // A hidden game leaves the grid; the cursor stays in place.
        if self.page == Page::Library {
            self.select(Some(&id))
        } else {
            Task::none()
        }
    }

    fn toggle_pinned(&mut self) -> Task<Message> {
        let Some(id) = self.current_id() else {
            return Task::none();
        };
        let Some(game) = self.game_mut(&id) else {
            return Task::none();
        };
        game.pinned = !game.pinned;
        let (pinned, title) = (game.pinned, game.title.clone());
        self.save(|state| state.set_pinned(&id, pinned));
        self.status = Some(if pinned {
            t!("status-pinned", title = title)
        } else {
            t!("status-unpinned", title = title)
        });
        self.select(Some(&id))
    }

    fn set_game_status(&mut self, id: &str, status: Option<Status>) -> Task<Message> {
        let Some(game) = self.game_mut(id) else {
            return Task::none();
        };
        game.status = status;
        let title = game.title.clone();
        self.save(|state| state.set_status(id, status));
        self.status = Some(if let Some(status) = status {
            t!(
                "status-is",
                title = title,
                status = format::play_status(status)
            )
        } else {
            t!("status-cleared", title = title)
        });
        // A game that leaves the chosen status leaves the grid.
        if self.page == Page::Library {
            self.select(Some(id))
        } else {
            Task::none()
        }
    }

    /// Makes every marked game a favorite, or not one: the same for all, so
    /// a mixed selection ends alike. The marks stay.
    fn bulk_pinned(&mut self, pinned: bool) -> Task<Message> {
        let ids: Vec<String> = self.selected_games.iter().cloned().collect();
        self.filter(|zendo| {
            let mut changed = 0;
            for id in &ids {
                if let Some(game) = zendo.game_mut(id)
                    && game.pinned != pinned
                {
                    game.pinned = pinned;
                    changed += 1;
                }
            }
            if changed == 0 {
                return;
            }
            zendo.save(|state| {
                for id in &ids {
                    state.set_pinned(id, pinned);
                }
            });
            zendo.status = Some(if pinned {
                t!("status-bulk-pinned", count = changed)
            } else {
                t!("status-bulk-unpinned", count = changed)
            });
        })
    }

    /// Hides every marked game, or brings it back. They leave the grid, so
    /// the marks go too.
    fn bulk_hidden(&mut self, hidden: bool) -> Task<Message> {
        let ids: Vec<String> = self.selected_games.iter().cloned().collect();
        self.filter(|zendo| {
            let mut changed = 0;
            for id in &ids {
                if let Some(game) = zendo.game_mut(id)
                    && game.hidden != hidden
                {
                    game.hidden = hidden;
                    changed += 1;
                }
            }
            if changed == 0 {
                return;
            }
            zendo.save(|state| {
                for id in &ids {
                    state.set_hidden(id, hidden);
                }
            });
            zendo.selected_games.clear();
            zendo.status = Some(if hidden {
                t!("status-bulk-hid", count = changed)
            } else {
                t!("status-bulk-unhid", count = changed)
            });
        })
    }

    fn filter_changed(&mut self) -> Task<Message> {
        self.selected = 0;
        self.status = None;
        operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y: 0.0 })
    }

    fn step(&mut self, step: Move) -> Task<Message> {
        let count = self.grid_len();
        if count == 0 {
            return Task::none();
        }
        let last = count - 1;
        let page = self.columns * self.rows_on_screen();
        let columns = self.columns;
        let selected = self.cursor().0;
        let current = (*selected).min(last);
        *selected = match step {
            Move::Left => current.saturating_sub(1),
            Move::Right => (current + 1).min(last),
            Move::Up => current.checked_sub(columns).unwrap_or(current),
            Move::Down => {
                let below = current + columns;
                // From the row above the last, a short last row still counts.
                if below <= last || current / columns < last / columns {
                    below.min(last)
                } else {
                    current
                }
            }
            Move::PageUp => current.saturating_sub(page),
            Move::PageDown => (current + page).min(last),
            Move::First => 0,
            Move::Last => last,
        };
        self.reveal()
    }

    fn rows_on_screen(&self) -> usize {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number of rows; heights are never negative"
        )]
        let rows = ((self.viewport_height + library::SPACING)
            / (library::CARD_HEIGHT + library::SPACING)) as usize;
        rows.max(1)
    }

    /// Scrolls just enough to show the selected card.
    fn reveal(&mut self) -> Task<Message> {
        if !matches!(self.page, Page::Library | Page::Store) {
            return Task::none();
        }
        let columns = self.columns.max(1);
        let (selected, scroll_top) = self.cursor();
        let Some(y) = reveal_y(*selected, columns, *scroll_top, self.viewport_height) else {
            return Task::none();
        };
        operation::scroll_to(grid_id(), AbsoluteOffset { x: 0.0, y })
    }

    /// Starts the game on the page, or the selected one in the library.
    fn play(&mut self) -> Task<Message> {
        match self.current_id().and_then(|id| self.game(&id)).cloned() {
            Some(game) => self.start_game(game),
            None => Task::none(),
        }
    }

    /// Starts downloading the covers and wide images that are missing, one
    /// at a time, in a thread of their own.
    fn download_images(&mut self) -> Task<Message> {
        if !self.persist {
            return Task::none();
        }
        if self.images.left > 0 {
            self.images.again = true;
            return Task::none();
        }
        let Some(fetcher) = Fetcher::new(&self.config) else {
            return Task::none();
        };
        let games = self.library.iter().flat_map(|library| &library.games);
        let in_library: HashSet<&str> = games.clone().map(|game| game.id.as_str()).collect();
        // A store game that is installed already comes with the library's.
        let owned: Vec<&Owned> = self
            .store
            .all()
            .filter(|owned| !in_library.contains(owned.id.as_str()))
            .collect();
        // Every cover first: the grids show them.
        let mut wanted: Vec<(Job, Kind)> = Vec::new();
        for kind in [Kind::Cover, Kind::Hero] {
            wanted.extend(
                games
                    .clone()
                    .filter(|game| fetcher.wants(*game, kind))
                    .map(|game| (Box::new(game.clone()) as Job, kind)),
            );
            wanted.extend(
                owned
                    .iter()
                    .filter(|owned| fetcher.wants(**owned, kind))
                    .map(|owned| (Box::new((*owned).clone()) as Job, kind)),
            );
        }
        if wanted.is_empty() {
            return Task::none();
        }
        self.images = ImageDownloads {
            left: wanted.len(),
            ..ImageDownloads::default()
        };
        Task::run(fetch_images(fetcher, wanted), |message| message)
    }

    fn subscription(&self) -> Subscription<Message> {
        let scheme = if self.themes.watching() {
            Subscription::run(themes::ticks).map(|()| Message::LookAtScheme)
        } else {
            Subscription::none()
        };
        // Steam's manifests, read often while it installs something.
        let steam = if !self.persist {
            Subscription::none()
        } else if self.steam_installs.active() {
            Subscription::run(steam_installs::busy_ticks)
                .map(|()| Message::SteamInstall(SteamInstallMessage::Tick))
        } else {
            Subscription::run(steam_installs::idle_ticks)
                .map(|()| Message::SteamInstall(SteamInstallMessage::Tick))
        };
        // The tray's clicks and menu, listened to only while it is there.
        let tray = if self.tray.is_some() {
            Subscription::run(tray::events).map(Message::Tray)
        } else {
            Subscription::none()
        };
        // The launchers' files, looked at every so often for a change.
        let watch = if self.persist {
            Subscription::run(watch_ticks).map(|()| Message::WatchTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            scheme,
            steam,
            tray,
            watch,
            event::listen_with(on_event),
            Subscription::run(gamepad::events).map(Message::Pad),
            window::close_requests().map(|_| Message::CloseRequested),
            Subscription::run(crate::links::listen)
                .map(|link| Message::Nexus(NexusMessage::Link(link))),
        ])
    }

    /// Iced's theme from the colours the state holds, not a global.
    fn theme(&self, _window: window::Id) -> iced::Theme {
        style::theme_of(self.themes.palette())
    }

    /// The window's own background and text: the palette's background,
    /// translucent unless Settings asks for an opaque window.
    fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        style::window(self.config.opaque)
    }

    /// Opens a window with the current settings and closes the old one,
    /// as when `opaque` changes (a window's transparency is fixed when it
    /// is created). Nothing to do while the window is hidden in the tray.
    fn reopen_window(&mut self) -> Task<Message> {
        let Some(old) = self.window else {
            return Task::none();
        };
        let (id, open) = window::open(window_settings(
            self.window_size,
            self.fullscreen,
            self.config.opaque,
        ));
        self.window = Some(id);
        // The new grid's scrollable starts at the top, as after the tray.
        self.scroll_top = 0.0;
        self.store.scroll_top = 0.0;
        Task::batch([open.map(Message::WindowReady), window::close(old)])
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
        if self.game_mode_screen() {
            return self.game_mode_view();
        }
        let page = match &self.page {
            Page::Library => self.library_page(),
            Page::Game(id) => match self.game(id) {
                Some(game) => self.game_page(game),
                None => centered(text(t!("game-gone")).color(colors().dim)),
            },
            Page::Settings => self.settings_page(),
            Page::About => about::page(),
            Page::Accounts => self.accounts_page(),
            Page::Downloads => self.downloads_page(),
            Page::Stats => self.stats_page(),
            Page::Store => self.store_page(),
            Page::Owned(id) => match self.store.owned(id) {
                Some(game) => self.owned_page(game),
                None => centered(text(t!("owned-gone")).color(colors().dim)),
            },
        };
        let page = if self.in_settings() {
            column![self.settings_tabs(), page].into()
        } else {
            page
        };
        let page = self.with_sheet(
            column![self.top_bar(), page, self.footer()]
                .width(Length::Fill)
                .into(),
        );
        let page = self.with_collections(self.with_form(page));
        self.with_installer(self.with_chooser(self.with_search(self.with_picker(page))))
    }

    fn footer(&self) -> Element<'_, Message> {
        let problems = self
            .library
            .as_ref()
            .map_or(0, |library| library.problems.len());
        // A game's page shows the status beside its actions.
        let status = if matches!(self.page, Page::Game(_) | Page::Owned(_)) {
            &None
        } else {
            &self.status
        };
        let summary = self.downloads.summary();
        let left = match (status, problems, summary.as_deref()) {
            _ if self.images.left > 0 => text(t!("images-downloading", count = self.images.left)),
            (Some(status), ..) => text(status.clone()),
            (None, _, Some(summary)) => text(summary.to_owned()),
            (None, 0, None) => text(""),
            (None, n, None) => text(t!("footer-unreadable", count = n)),
        };
        let hints = match self.page {
            Page::Library if !self.selected_games.is_empty() => t!("hints-selection"),
            Page::Library => t!("hints-library"),
            Page::Game(_) if self.sheet.is_some() => t!("hints-sheet"),
            Page::Game(_) => t!("hints-game"),
            Page::Store => t!("hints-store"),
            Page::Owned(_) => t!("hints-owned"),
            Page::Downloads if self.downloads.confirm_close => t!("hints-closing"),
            Page::Stats => t!("hints-stats"),
            Page::Accounts | Page::Downloads | Page::Settings | Page::About => t!("hints-other"),
        };
        row![
            left.size(13).color(colors().dim),
            space::horizontal(),
            text(hints).size(13).color(colors().dim),
        ]
        .spacing(16)
        // 8 and 24: the window's other paddings, all on the 8-point scale.
        .padding([8, 24])
        .into()
    }
}

/// Reads every source in a thread, with the saved sort, categories and
/// settings.
fn scan() -> Task<Message> {
    Task::perform(
        blocking(|| {
            let state = State::load().unwrap_or_default();
            let config = Config::load().map_err(|err| format!("{err:#}"));
            (
                Library::scan(),
                state.sort,
                state.categories(),
                config,
                state.filters(),
            )
        }),
        |loaded| Message::Loaded(Box::new(loaded)),
    )
}

/// A tick every [`WATCH`], for the launchers' file watch.
fn watch_ticks() -> impl Stream<Item = ()> {
    iced::stream::channel(1, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            while sender.unbounded_send(()).is_ok() {
                thread::sleep(WATCH);
            }
        });
        while let Some(()) = receiver.next().await {
            if output.send(()).await.is_err() {
                return;
            }
        }
    })
}

/// Runs blocking work (file reads, HTTP) on a thread of its own. When the
/// work panics (a bug), the window carries on with [`Fallback::panicked`]
/// in its place, and says what the panic said when it can.
async fn blocking<T: Fallback + Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = oneshot::channel();
    thread::spawn(move || {
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let _ = sender.send(answer.unwrap_or_else(|payload| T::panicked(Some(payload))));
    });
    receiver.await.unwrap_or_else(|_| T::panicked(None))
}

/// What work on a thread gives when it panicked instead of finishing.
trait Fallback {
    fn panicked(payload: Option<Box<dyn std::any::Any + Send>>) -> Self;
}

/// Where the grid scrolls to show `selected`: just enough, or `None` when
/// it is already in view.
fn reveal_y(selected: usize, columns: usize, scroll_top: f32, viewport: f32) -> Option<f32> {
    #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
    let row = (selected / columns) as f32;
    let top = row * (library::CARD_HEIGHT + library::SPACING);
    let bottom = top + library::CARD_HEIGHT + 2.0 * library::GRID_PADDING;
    if top < scroll_top {
        Some(top)
    } else if bottom > scroll_top + viewport {
        Some(bottom - viewport)
    } else {
        None
    }
}

/// What the window says when work hit a bug: the panic's own words when it
/// carried any.
fn panicked_note(payload: Option<Box<dyn std::any::Any + Send>>) -> String {
    let said = payload.and_then(|payload| {
        payload.downcast_ref::<String>().cloned().or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
    });
    match said {
        Some(said) if !said.is_empty() => t!("panicked-with", error = said),
        _ => t!("panicked"),
    }
}

impl<T> Fallback for Result<T, String> {
    fn panicked(payload: Option<Box<dyn std::any::Any + Send>>) -> Self {
        Err(panicked_note(payload))
    }
}

impl<T> Fallback for anyhow::Result<T> {
    fn panicked(payload: Option<Box<dyn std::any::Any + Send>>) -> Self {
        Err(anyhow::anyhow!(panicked_note(payload)))
    }
}

/// Work that found nothing when it hit a bug.
impl<T> Fallback for Option<T> {
    fn panicked(_: Option<Box<dyn std::any::Any + Send>>) -> Self {
        None
    }
}

/// A list nothing came of.
impl<T> Fallback for Vec<T> {
    fn panicked(_: Option<Box<dyn std::any::Any + Send>>) -> Self {
        Vec::new()
    }
}

/// Two pieces of work, each with its own fallback.
impl<A: Fallback, B: Fallback> Fallback for (A, B) {
    fn panicked(_: Option<Box<dyn std::any::Any + Send>>) -> Self {
        (A::panicked(None), B::panicked(None))
    }
}

/// The library read at start: none, saying why, and the settings unread.
impl Fallback for Scanned {
    fn panicked(payload: Option<Box<dyn std::any::Any + Send>>) -> Self {
        let note = panicked_note(payload);
        let library = Library {
            problems: vec![note.clone()],
            ..Library::default()
        };
        (
            library,
            Sort::default(),
            Vec::new(),
            Err(note),
            Vec::new(),
        )
    }
}

/// Not known to open `nxm://` links.
impl Fallback for zendo_core::nexus::Handler {
    fn panicked(_: Option<Box<dyn std::any::Any + Send>>) -> Self {
        Self::Nothing
    }
}

/// A game from the library or a store, whose images are wanted.
type Job = Box<dyn Artwork + Send>;

/// Fetches each image on a thread (the requests block) and reports each one
/// as it lands, then `ImagesDone`.
fn fetch_images(mut fetcher: Fetcher, wanted: Vec<(Job, Kind)>) -> impl Stream<Item = Message> {
    iced::stream::channel(8, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            for (game, kind) in wanted {
                let result = fetcher
                    .fetch(game.as_ref(), kind)
                    .map_err(|err| format!("{err:#}"));
                if sender
                    .unbounded_send(Message::Image(game.id().to_owned(), kind, result))
                    .is_err()
                {
                    return;
                }
            }
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
        let _ = output.send(Message::ImagesDone).await;
    })
}

/// Opens a folder or a web page in the user's program for it.
fn open(target: &str) -> Result<(), String> {
    let mut child = Command::new("xdg-open")
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not open {target}: {err}"))?;
    thread::spawn(move || child.wait());
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center(Length::Fill).into()
}

fn on_event(event: Event, status: event::Status, id: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Resized(size)) => Some(Message::Resized(id, size)),
        Event::Window(window::Event::Focused) => Some(Message::Focused(id, true)),
        Event::Window(window::Event::Unfocused) => Some(Message::Focused(id, false)),
        // Escape closes what is open before a focused field can keep it.
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        }) => Some(Message::Back),
        // A key a text field used (typing, its cursor) is its own.
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            text,
            ..
        }) if status == event::Status::Ignored => key_message(&key, modifiers, text.as_deref()),
        _ => None,
    }
}

fn key_message(key: &Key, modifiers: keyboard::Modifiers, text: Option<&str>) -> Option<Message> {
    if let Key::Named(named) = key {
        let message = match named {
            Named::ArrowLeft => Message::Move(Move::Left),
            Named::ArrowRight => Message::Move(Move::Right),
            Named::ArrowUp => Message::Move(Move::Up),
            Named::ArrowDown => Message::Move(Move::Down),
            Named::PageUp => Message::Move(Move::PageUp),
            Named::PageDown => Message::Move(Move::PageDown),
            Named::Home => Message::Move(Move::First),
            Named::End => Message::Move(Move::Last),
            Named::Enter => Message::Activate,
            Named::Escape => Message::Back,
            Named::Backspace => Message::Backspace,
            Named::F1 => Message::Navigate(Page::About),
            Named::F5 => Message::Refresh,
            Named::F11 => Message::ToggleGameMode,
            Named::Tab => Message::CycleSort,
            Named::Delete => Message::ToggleHidden,
            Named::Space if modifiers.control() => Message::ToggleSelected,
            Named::Space => Message::Type(" ".to_owned()),
            _ => return None,
        };
        return Some(message);
    }
    if modifiers.control() {
        let Key::Character(c) = key else {
            return None;
        };
        return match c.to_lowercase().as_str() {
            "h" => Some(Message::ShowHidden),
            "p" => Some(Message::TogglePinned),
            "f" => Some(Message::FocusSearch),
            "r" => Some(Message::Refresh),
            "v" => Some(Message::PasteClipboard),
            "1" => Some(Message::Navigate(Page::Library)),
            "2" => Some(Message::Navigate(Page::Store)),
            "3" => Some(Message::Navigate(Page::Downloads)),
            "4" => Some(Message::Navigate(Page::Accounts)),
            "5" | "," => Some(Message::Navigate(Page::Settings)),
            "6" => Some(Message::Navigate(Page::About)),
            "7" => Some(Message::Navigate(Page::Stats)),
            _ => None,
        };
    }
    if modifiers.alt() || modifiers.logo() {
        return None;
    }
    text.filter(|text| !text.chars().any(char::is_control))
        .map(|text| Message::Type(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn a_report_says_it_was_gathered_or_why_not() {
        let mut zendo = zendo_with(1, 1);
        let _ = zendo.update(Message::ReportReady(Ok("# Zendo report\n\nline".into())));
        assert_eq!(
            zendo.diagnostics.note(),
            Some(&(t!("report-gathered", count = 3), true))
        );
        let _ = zendo.update(Message::ReportReady(Err("broke".into())));
        assert_eq!(zendo.diagnostics.note(), Some(&("broke".to_owned(), false)));
    }

    #[test]
    fn work_that_panics_gives_its_fallback_not_a_crash() {
        let done: Result<u32, String> = iced::futures::executor::block_on(blocking(|| Ok(7)));
        assert_eq!(done, Ok(7));
        let failed: Result<u32, String> =
            iced::futures::executor::block_on(blocking(|| panic!("a bug in the work")));
        assert_eq!(
            failed,
            Err(t!("panicked-with", error = "a bug in the work"))
        );
        let none: Vec<u8> = iced::futures::executor::block_on(blocking(|| panic!("again")));
        assert_eq!(none, Vec::<u8>::new());
    }

    fn zendo_with(count: usize, columns: usize) -> Zendo {
        let games = (0..count)
            .map(|n| {
                Game::new(
                    format!("desktop:{n}"),
                    format!("Game {n:02}"),
                    if n % 2 == 0 {
                        Source::Desktop
                    } else {
                        Source::Steam
                    },
                    zendo_core::Launch::Uri { uri: String::new() },
                )
            })
            .collect();
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        // No focus event arrives in a test: the window is the one in use.
        zendo.focused = true;
        zendo.scanning = false;
        zendo.sort = Sort::Title;
        zendo.library = Some(Library {
            games,
            problems: Vec::new(),
        });
        zendo.columns = columns;
        zendo
    }

    fn titles(zendo: &Zendo) -> Vec<&str> {
        zendo
            .visible()
            .iter()
            .map(|game| game.title.as_str())
            .collect()
    }

    /// Puts games in a category, on the window and on the games themselves.
    fn in_category(zendo: &mut Zendo, name: &str, ids: &[&str]) {
        zendo.categories = vec![Category {
            name: name.to_owned(),
            games: ids.iter().map(|id| (*id).to_owned()).collect(),
        }];
        for id in ids {
            if let Some(game) = zendo.game_mut(id) {
                game.categories = vec![name.to_owned()];
            }
        }
    }

    #[test]
    fn a_category_narrows_with_the_store_and_the_search() {
        let mut zendo = zendo_with(3, 3);
        in_category(&mut zendo, "Co-op", &["desktop:0", "desktop:1"]);
        let titles = |zendo: &Zendo| {
            zendo
                .visible()
                .iter()
                .map(|g| g.title.clone())
                .collect::<Vec<_>>()
        };

        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        assert_eq!(titles(&zendo), ["Game 00", "Game 01"]);

        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        assert_eq!(titles(&zendo), ["Game 01"], "the store narrows it further");

        let _ = zendo.update(Message::ClearFilters);
        assert_eq!(titles(&zendo), ["Game 00", "Game 01", "Game 02"]);
        assert_eq!((zendo.source, zendo.category.clone()), (None, None));

        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        let _ = zendo.update(Message::Type("02".into()));
        assert_eq!(titles(&zendo), [] as [String; 0], "no match inside it");
        let _ = zendo.update(Message::Back);
        assert_eq!(titles(&zendo).len(), 2);
    }

    #[test]
    fn a_chosen_category_chip_is_left_when_pressed_again() {
        let mut zendo = zendo_with(2, 2);
        in_category(&mut zendo, "Co-op", &["desktop:0"]);
        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        assert_eq!(zendo.category.as_deref(), Some("Co-op"));
        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        assert_eq!(zendo.category, None);
    }

    #[test]
    fn the_cursor_stays_on_its_game_when_a_category_is_chosen() {
        let mut zendo = zendo_with(4, 4);
        in_category(&mut zendo, "Co-op", &["desktop:1", "desktop:3"]);
        zendo.selected = 3;
        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        assert_eq!(zendo.selected_id().as_deref(), Some("desktop:3"));
        assert_eq!(zendo.selected, 1);
    }

    #[test]
    fn ctrl_space_marks_the_focused_card() {
        let mut zendo = zendo_with(3, 3);
        let _ = zendo.update(Message::ToggleSelected);
        assert_eq!(
            zendo.selected_games.iter().collect::<Vec<_>>(),
            ["desktop:0"]
        );
        let _ = zendo.update(Message::ToggleSelected);
        assert!(zendo.selected_games.is_empty(), "pressed again, it is left");
        let _ = zendo.update(Message::ToggleSelectedAt("desktop:2".into()));
        assert_eq!(
            zendo.selected_games.iter().collect::<Vec<_>>(),
            ["desktop:2"]
        );
        let _ = zendo.update(Message::Move(Move::Right));
        let _ = zendo.update(Message::ToggleSelected);
        assert_eq!(zendo.selected_games.len(), 2);
        assert!(zendo.selected_games.contains("desktop:2"));
        let _ = zendo.update(Message::ClearSelection);
        assert!(zendo.selected_games.is_empty());
    }

    #[test]
    fn escape_clears_the_marks_before_the_search() {
        let mut zendo = zendo_with(3, 3);
        let _ = zendo.update(Message::Type("Game".into()));
        let _ = zendo.update(Message::ToggleSelected);
        assert_eq!(zendo.selected_games.len(), 1);
        let _ = zendo.update(Message::Back);
        assert!(zendo.selected_games.is_empty(), "the marks go first");
        assert_eq!(zendo.search, "Game");
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.search, "");
    }

    #[test]
    fn a_rescan_drops_marks_of_games_that_left() {
        let mut zendo = zendo_with(3, 3);
        let _ = zendo.update(Message::ToggleSelectedAt("desktop:0".into()));
        let _ = zendo.update(Message::ToggleSelectedAt("desktop:1".into()));
        let left = Game::new(
            "desktop:1".into(),
            "Game 01".into(),
            Source::Steam,
            zendo_core::Launch::Uri { uri: String::new() },
        );
        let library = Library {
            games: vec![left],
            problems: Vec::new(),
        };
        let _ = zendo.update(Message::Loaded(Box::new((
            library,
            Sort::default(),
            Vec::new(),
            Ok(Config::default()),
            Vec::new(),
        ))));
        assert_eq!(
            zendo.selected_games.iter().collect::<Vec<_>>(),
            ["desktop:1"]
        );
    }

    #[test]
    fn assigning_reaches_every_marked_game() {
        let mut zendo = zendo_with(2, 3);
        let _ = zendo.update(Message::ToggleSelectedAt("desktop:0".into()));
        let _ = zendo.update(Message::ToggleSelectedAt("desktop:1".into()));
        let marked = zendo.selected_games.clone();
        press(&mut zendo, CollectionsMessage::Open(marked));
        press(&mut zendo, CollectionsMessage::Typed("Book".into()));
        press(&mut zendo, CollectionsMessage::Create);
        for id in ["desktop:0", "desktop:1"] {
            assert_eq!(zendo.game(id).unwrap().categories, ["Book"], "{id}");
        }
    }

    #[test]
    fn the_categories_button_targets_its_own_game() {
        let zendo = zendo_with(2, 3);
        let game = zendo.game("desktop:1").unwrap();
        match game::change_categories(game) {
            Message::Collections(CollectionsMessage::Open(targets)) => {
                assert_eq!(targets.iter().collect::<Vec<_>>(), ["desktop:1"]);
            }
            other => panic!("not the categories sheet: {other:?}"),
        }
    }

    #[test]
    fn the_game_page_opens_the_categories_of_that_game() {
        let mut zendo = zendo_with(2, 3);
        press(
            &mut zendo,
            CollectionsMessage::Open(BTreeSet::from(["desktop:1".to_owned()])),
        );
        assert_eq!(
            zendo
                .collections_sheet
                .as_ref()
                .unwrap()
                .targets
                .iter()
                .collect::<Vec<_>>(),
            ["desktop:1"]
        );
        press(&mut zendo, CollectionsMessage::Typed("Book".into()));
        press(&mut zendo, CollectionsMessage::Create);
        assert_eq!(zendo.game("desktop:1").unwrap().categories, ["Book"]);
        assert_eq!(
            zendo.game("desktop:0").unwrap().categories,
            [] as [String; 0],
            "the other game was not the target"
        );
    }

    fn press(zendo: &mut Zendo, message: CollectionsMessage) {
        let _ = zendo.update(Message::Collections(message));
    }

    fn category_names(zendo: &Zendo) -> Vec<&str> {
        zendo
            .categories
            .iter()
            .map(|category| category.name.as_str())
            .collect()
    }

    fn note(zendo: &Zendo) -> Option<(String, bool)> {
        zendo
            .collections_sheet
            .as_ref()
            .and_then(|sheet| sheet.note.clone())
    }

    #[test]
    fn the_sheet_creates_renames_and_deletes_a_category() {
        let mut zendo = zendo_with(1, 3);
        press(&mut zendo, CollectionsMessage::Open(BTreeSet::new()));
        press(&mut zendo, CollectionsMessage::Create);
        assert_eq!(category_names(&zendo), [] as [&str; 0], "nothing typed");
        assert_eq!(
            note(&zendo),
            Some(("A collection needs a name".to_owned(), false))
        );
        press(&mut zendo, CollectionsMessage::Typed("Co-op".into()));
        press(&mut zendo, CollectionsMessage::Create);
        assert_eq!(category_names(&zendo), ["Co-op"]);
        assert_eq!(note(&zendo), Some(("Made Co-op".to_owned(), true)));

        press(&mut zendo, CollectionsMessage::Rename("Co-op".into()));
        press(
            &mut zendo,
            CollectionsMessage::RenameTyped("Co-op nights".into()),
        );
        press(&mut zendo, CollectionsMessage::RenameDone);
        assert_eq!(category_names(&zendo), ["Co-op nights"]);
        assert_eq!(
            note(&zendo),
            Some(("Renamed to Co-op nights".to_owned(), true))
        );

        press(&mut zendo, CollectionsMessage::Delete("Co-op nights".into()));
        assert_eq!(
            category_names(&zendo),
            ["Co-op nights"],
            "a delete asks first"
        );
        assert_eq!(note(&zendo), None);
        press(&mut zendo, CollectionsMessage::Delete("Co-op nights".into()));
        assert_eq!(category_names(&zendo), [] as [&str; 0]);
        assert_eq!(
            note(&zendo),
            Some(("Deleted Co-op nights".to_owned(), true))
        );
    }

    #[test]
    fn a_delete_asks_first_and_another_press_takes_it_back() {
        let mut zendo = zendo_with(1, 3);
        press(&mut zendo, CollectionsMessage::Open(BTreeSet::new()));
        press(&mut zendo, CollectionsMessage::Typed("Book".into()));
        press(&mut zendo, CollectionsMessage::Create);
        press(&mut zendo, CollectionsMessage::Delete("Book".into()));
        assert_eq!(category_names(&zendo), ["Book"], "it only asked");
        press(&mut zendo, CollectionsMessage::Typed("Other".into()));
        assert_eq!(
            zendo.collections_sheet.as_ref().unwrap().confirm_delete,
            None,
            "typing elsewhere takes the ask back"
        );
        press(&mut zendo, CollectionsMessage::Delete("Book".into()));
        assert_eq!(category_names(&zendo), ["Book"], "it asks again");
    }

    #[test]
    fn the_sheet_refuses_a_name_taken_in_another_case() {
        let mut zendo = zendo_with(1, 3);
        press(&mut zendo, CollectionsMessage::Open(BTreeSet::new()));
        press(&mut zendo, CollectionsMessage::Create);
        assert_eq!(category_names(&zendo), [] as [&str; 0], "nothing typed");
        assert_eq!(
            note(&zendo),
            Some(("A collection needs a name".to_owned(), false))
        );
        press(&mut zendo, CollectionsMessage::Typed("Co-op".into()));
        press(&mut zendo, CollectionsMessage::Create);
        press(&mut zendo, CollectionsMessage::Typed("co-op".into()));
        press(&mut zendo, CollectionsMessage::Create);
        assert_eq!(category_names(&zendo), ["Co-op"], "one category, not two");
        assert_eq!(
            note(&zendo),
            Some(("co-op is already a collection".to_owned(), false))
        );
    }

    #[test]
    fn the_sheet_assigns_every_target() {
        let mut zendo = zendo_with(2, 3);
        press(&mut zendo, CollectionsMessage::Open(BTreeSet::new()));
        press(&mut zendo, CollectionsMessage::Typed("Co-op".into()));
        press(&mut zendo, CollectionsMessage::Create);
        press(&mut zendo, CollectionsMessage::Close);
        let both = ["desktop:0".to_owned(), "desktop:1".to_owned()]
            .into_iter()
            .collect();
        press(&mut zendo, CollectionsMessage::Open(both));

        press(
            &mut zendo,
            CollectionsMessage::SetMembership {
                name: "Co-op".into(),
                member: true,
            },
        );
        for id in ["desktop:0", "desktop:1"] {
            assert_eq!(zendo.game(id).unwrap().categories, ["Co-op"], "{id}");
        }
        assert_eq!(note(&zendo), Some(("Added to 2 games".to_owned(), true)));

        press(
            &mut zendo,
            CollectionsMessage::SetMembership {
                name: "Co-op".into(),
                member: false,
            },
        );
        for id in ["desktop:0", "desktop:1"] {
            assert!(zendo.game(id).unwrap().categories.is_empty(), "{id}");
        }
        assert_eq!(
            note(&zendo),
            Some(("Taken out of 2 games".to_owned(), true))
        );
    }

    #[test]
    fn hiding_leaves_a_game_out_until_hidden_games_are_shown() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 1;
        let _ = zendo.update(Message::ToggleHidden);
        assert_eq!(titles(&zendo), ["Game 00", "Game 02"]);
        assert_eq!(zendo.selected, 1, "the cursor stays in place");
        // The Hidden view holds only the hidden game.
        let _ = zendo.update(Message::ShowHidden);
        assert_eq!(titles(&zendo), ["Game 01"]);
        // Bringing it back leaves the Hidden view empty; All has all three.
        zendo.selected = 0;
        let _ = zendo.update(Message::ToggleHidden);
        assert_eq!(titles(&zendo), [] as [&str; 0]);
        let _ = zendo.update(Message::ShowHidden);
        assert_eq!(zendo.view, library::View::All);
        assert_eq!(titles(&zendo), ["Game 00", "Game 01", "Game 02"]);
    }

    #[test]
    fn achievements_that_arrive_mark_the_games_card() {
        use zendo_core::achievements::{Achievement, Achievements};
        let mut zendo = zendo_with(1, 1);
        let one = |unlocked: Option<u64>| Achievement {
            name: String::new(),
            description: String::new(),
            hidden: false,
            unlocked,
            rarity: None,
            icon: None,
        };
        let found = |known: bool| Achievements {
            list: vec![one(Some(1)), one(None), one(None), one(None)],
            unlocks_known: known,
        };
        let _ = zendo.update(Message::Achievements(
            "desktop:0".into(),
            Ok(Some(found(false))),
        ));
        assert_eq!(zendo.game("desktop:0").unwrap().achievements, None);
        let _ = zendo.update(Message::Achievements(
            "desktop:0".into(),
            Ok(Some(found(true))),
        ));
        let game = zendo.game("desktop:0").unwrap();
        assert_eq!(game.achievements, Some((1, 4)));
        assert_eq!(library::progress_of((1, 4)), Some(0.25));
        assert_eq!(library::progress_of((0, 0)), None);
    }

    #[test]
    fn each_view_lets_through_its_games() {
        let mut zendo = zendo_with(4, 4);
        let now = now();
        if let Some(library) = zendo.library.as_mut() {
            library.games[0].pinned = true;
            library.games[1].played.last = Some(now - 86_400);
            library.games[2].played.last = Some(now - 90 * 86_400);
            library.games[3].hidden = true;
            library.games[3].pinned = true;
            library.games[3].played.last = Some(now);
        }
        let _ = zendo.update(Message::SetView(library::View::All));
        assert_eq!(titles(&zendo), ["Game 00", "Game 01", "Game 02"]);
        let _ = zendo.update(Message::SetView(library::View::Favorites));
        assert_eq!(titles(&zendo), ["Game 00"]);
        let _ = zendo.update(Message::SetView(library::View::Recent));
        assert_eq!(titles(&zendo), ["Game 01"]);
        let _ = zendo.update(Message::SetView(library::View::Hidden));
        assert_eq!(titles(&zendo), ["Game 03"]);
    }

    #[test]
    fn the_cursor_follows_its_game_when_the_sort_changes() {
        let mut zendo = zendo_with(3, 3);
        if let Some(library) = zendo.library.as_mut() {
            library.games[2].played.last = Some(100);
        }
        zendo.selected = 2;
        // A–Z, then Recent: Game 02 was played, so it comes first.
        let _ = zendo.update(Message::CycleSort);
        assert_eq!(zendo.sort, Sort::Recent);
        assert_eq!(titles(&zendo)[0], "Game 02");
        assert_eq!(zendo.selected, 0);
    }

    #[test]
    fn a_pinned_game_leads_and_keeps_the_cursor() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 2;
        let _ = zendo.update(Message::TogglePinned);
        assert_eq!(titles(&zendo), ["Game 02", "Game 00", "Game 01"]);
        assert_eq!(zendo.selected, 0);
    }

    #[test]
    fn store_chips_filter_the_grid() {
        let mut zendo = zendo_with(4, 3);
        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        assert_eq!(titles(&zendo), ["Game 01", "Game 03"]);
        let _ = zendo.update(Message::SetSource(None));
        assert_eq!(zendo.visible().len(), 4);
    }

    #[test]
    fn enter_opens_a_game_and_escape_comes_back() {
        let mut zendo = zendo_with(3, 3);
        zendo.selected = 1;
        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.page, Page::Game("desktop:1".into()));
        // Arrows and typing do nothing on a game's page.
        let _ = zendo.update(Message::Move(Move::Right));
        let _ = zendo.update(Message::Type("x".into()));
        assert_eq!((zendo.selected, zendo.search.as_str()), (1, ""));
        // Enter there plays it.
        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.game("desktop:1").map(|g| g.played.launches), Some(1));
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.page, Page::Library);
        assert_eq!(zendo.selected, 1);
    }

    #[test]
    fn the_controller_pins_on_a_game_page_and_sorts_in_the_library() {
        let mut zendo = zendo_with(3, 3);
        let _ = zendo.update(Message::Pad(Pad::Sort));
        assert_eq!(zendo.sort, Sort::Recent);
        let _ = zendo.update(Message::Pad(Pad::Activate));
        let _ = zendo.update(Message::Pad(Pad::Sort));
        assert_eq!(zendo.game("desktop:0").map(|g| g.pinned), Some(true));
    }

    fn after(zendo: &mut Zendo, from: usize, step: Move) -> usize {
        zendo.selected = from;
        let _ = zendo.step(step);
        zendo.selected
    }

    #[test]
    fn arrows_move_through_the_grid() {
        // Seven games in three columns: rows 0-2, 3-5 and a short 6.
        let mut zendo = zendo_with(7, 3);
        assert_eq!(after(&mut zendo, 1, Move::Up), 1);
        assert_eq!(after(&mut zendo, 1, Move::Down), 4);
        assert_eq!(after(&mut zendo, 5, Move::Down), 6);
        assert_eq!(after(&mut zendo, 6, Move::Down), 6);
        assert_eq!(after(&mut zendo, 6, Move::Right), 6);
        assert_eq!(after(&mut zendo, 0, Move::Left), 0);
        assert_eq!(after(&mut zendo, 2, Move::Last), 6);
    }

    #[test]
    fn reveals_the_selected_row_just_enough() {
        let pitch = library::CARD_HEIGHT + library::SPACING;
        let card = library::CARD_HEIGHT + 2.0 * library::GRID_PADDING;
        // Row 0 is above a scroll of one row: back to the top.
        assert_eq!(reveal_y(0, 3, pitch + 1.0, 800.0), Some(0.0));
        // Already in view: nothing moves.
        assert_eq!(reveal_y(0, 3, 0.0, card + 1.0), None);
        // Row 1 is below a viewport that shows only row 0.
        assert_eq!(reveal_y(3, 3, 0.0, pitch), Some(card));
    }

    #[test]
    fn typing_filters_by_every_word() {
        let mut zendo = zendo_with(12, 3);
        let _ = zendo.update(Message::Type("game 1".into()));
        assert_eq!(titles(&zendo), ["Game 01", "Game 10", "Game 11"]);
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.visible().len(), 12);
    }

    #[test]
    fn a_key_is_kept_only_once_steamgriddb_takes_it() {
        let mut zendo = zendo_with(1, 3);
        let _ = zendo.update(Message::Navigate(Page::Settings));
        let _ = zendo.update(Message::Type("bad".into()));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked("bad".into(), Ok(KeyCheck::Rejected)));
        assert_eq!(zendo.config.steamgriddb_key, None);

        let _ = zendo.update(Message::KeyInput(" good ".into()));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked("good".into(), Ok(KeyCheck::Accepted)));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("good"));

        // Offline, Save still keeps it; Check alone never does.
        let _ = zendo.update(Message::KeyInput("other".into()));
        let _ = zendo.update(Message::CheckKey);
        let _ = zendo.update(Message::KeyChecked("other".into(), Err("offline".into())));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("good"));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyChecked("other".into(), Err("offline".into())));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("other"));

        let _ = zendo.update(Message::KeyInput(String::new()));
        let _ = zendo.update(Message::SaveKey);
        assert_eq!(zendo.config.steamgriddb_key, None);
    }

    #[test]
    fn a_stale_key_check_decides_nothing() {
        let mut zendo = zendo_with(1, 3);
        let _ = zendo.update(Message::KeyInput("old".into()));
        let _ = zendo.update(Message::SaveKey);
        let _ = zendo.update(Message::KeyInput("new".into()));
        let _ = zendo.update(Message::SaveKey);
        // The answer about "old" arrives after "new" was asked about.
        let _ = zendo.update(Message::KeyChecked("old".into(), Ok(KeyCheck::Accepted)));
        assert_eq!(zendo.config.steamgriddb_key, None, "nothing was stored");
        let _ = zendo.update(Message::KeyChecked("new".into(), Ok(KeyCheck::Accepted)));
        assert_eq!(zendo.config.steamgriddb_key.as_deref(), Some("new"));
    }

    #[test]
    fn a_sign_in_takes_the_code_from_what_is_pasted() {
        let mut zendo = zendo_with(1, 3);
        let _ = zendo.update(Message::Navigate(Page::Accounts));
        let _ = zendo.update(Message::Account(AccountMessage::Start(
            zendo_core::stores::Store::Gog,
        )));
        // Typing with no field focused starts the code; pasting replaces it.
        let _ = zendo.update(Message::Type("x".into()));
        let _ = zendo.update(Message::Pasted(Some(
            " https://embed.gog.com/on_login_success?origin=client&code=abc ".into(),
        )));
        let _ = zendo.update(Message::Account(AccountMessage::Finish));
        let _ = zendo.update(Message::Account(AccountMessage::Finished(
            zendo_core::stores::Store::Gog,
            Ok("me".into()),
        )));
        assert_eq!(zendo.accounts.signed_in(), 1);
        assert_eq!(zendo.status.as_deref(), Some("Signed in to GOG as me"));
    }
    #[test]
    fn keys_a_field_used_are_left_to_it() {
        let enter = Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Enter),
            modified_key: Key::Named(Named::Enter),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        let id = window::Id::unique();
        assert!(on_event(enter.clone(), event::Status::Captured, id).is_none());
        assert!(matches!(
            on_event(enter, event::Status::Ignored, id),
            Some(Message::Activate)
        ));
    }

    #[test]
    fn zendo_starts_on_the_library() {
        let (zendo, _) = Zendo::boot();
        assert_eq!(zendo.page, Page::Library);
        assert!(zendo.game_mode.is_none());
    }

    /// A library of four games, played in the order 1, 3, 0 (2 never).
    fn played_zendo() -> Zendo {
        let mut zendo = zendo_with(4, 4);
        zendo.window = Some(window::Id::unique());
        let library = zendo.library.as_mut().expect("a library");
        for (id, last) in [("desktop:1", 300), ("desktop:3", 200), ("desktop:0", 100)] {
            let game = library
                .games
                .iter_mut()
                .find(|game| game.id == id)
                .expect("a game");
            game.played.last = Some(last);
        }
        zendo
    }

    #[test]
    fn game_mode_takes_over_the_library_and_hands_the_page_back() {
        let mut zendo = played_zendo();
        zendo.page = Page::Store;
        let _ = zendo.update(Message::ToggleGameMode);
        assert!(zendo.game_mode.is_some() && zendo.fullscreen);
        assert_eq!(zendo.page, Page::Library);
        assert!(zendo.game_mode_screen());

        // Another page keeps its own look; the bar's chip comes back to the
        // screen.
        let _ = zendo.update(Message::Navigate(Page::Downloads));
        assert!(!zendo.game_mode_screen());
        let _ = zendo.update(Message::GameMode(GameModeMessage::Show));
        assert!(zendo.game_mode_screen());

        let _ = zendo.update(Message::ToggleGameMode);
        assert!(zendo.game_mode.is_none() && !zendo.fullscreen);
        assert_eq!(zendo.page, Page::Store, "the page it was left from");
    }

    #[test]
    fn the_menus_library_choice_shows_the_grid_until_the_screen_is_asked_for() {
        let mut zendo = played_zendo();
        let _ = zendo.update(Message::ToggleGameMode);
        let _ = zendo.update(Message::GameMode(GameModeMessage::OpenMenu));
        let _ = zendo.update(Message::GameMode(GameModeMessage::MenuChoose(
            game_mode::MenuChoice::Library,
        )));
        assert!(!zendo.game_mode_screen(), "the grid shows");
        assert!(zendo.game_mode.is_some(), "still in Game Mode");
        let _ = zendo.update(Message::GameMode(GameModeMessage::Show));
        assert!(zendo.game_mode_screen());
    }

    #[test]
    fn game_mode_moves_one_game_along_a_shelf_and_stops_at_the_ends() {
        let mut zendo = played_zendo();
        let _ = zendo.update(Message::ToggleGameMode);
        // Continue playing: Game 01, Game 03, Game 00.
        assert_eq!(
            zendo.hero_game().map(|game| game.id.as_str()),
            Some("desktop:1")
        );
        let _ = zendo.update(Message::Move(Move::Right));
        assert_eq!(
            zendo.hero_game().map(|game| game.id.as_str()),
            Some("desktop:3")
        );
        for _ in 0..5 {
            let _ = zendo.update(Message::Move(Move::Right));
        }
        assert_eq!(
            zendo.hero_game().map(|game| game.id.as_str()),
            Some("desktop:0")
        );
        let _ = zendo.update(Message::Move(Move::Up));
        assert_eq!(
            zendo.hero_game().map(|game| game.id.as_str()),
            Some("desktop:0"),
            "up does nothing"
        );
    }

    #[test]
    fn the_shelves_cycle_and_skip_an_empty_one() {
        let mut zendo = played_zendo();
        let _ = zendo.update(Message::ToggleGameMode);
        assert_eq!(zendo.shown_shelf(), game_mode::Shelf::Continue);
        let _ = zendo.update(Message::GameMode(GameModeMessage::CycleShelf(1)));
        assert_eq!(zendo.shown_shelf(), game_mode::Shelf::All);
        let _ = zendo.update(Message::GameMode(GameModeMessage::CycleShelf(1)));
        assert_eq!(zendo.shown_shelf(), game_mode::Shelf::Continue);
    }

    #[test]
    fn with_nothing_played_game_mode_shows_every_game() {
        let mut zendo = zendo_with(3, 3);
        zendo.window = Some(window::Id::unique());
        let _ = zendo.update(Message::ToggleGameMode);
        assert_eq!(zendo.shown_shelf(), game_mode::Shelf::All);
        assert!(zendo.hero_game().is_some());
    }

    #[test]
    fn the_controller_means_new_things_only_in_game_mode() {
        let mut zendo = played_zendo();
        let _ = zendo.update(Message::ToggleGameMode);
        // Start opens the menu; B closes it.
        let _ = zendo.pad(Pad::ShowHidden);
        assert_eq!(zendo.game_mode.as_ref().and_then(|mode| mode.menu), Some(0));
        let _ = zendo.pad(Pad::Back);
        assert!(
            zendo
                .game_mode
                .as_ref()
                .is_some_and(|mode| mode.menu.is_none())
        );
        // The menu's last choice leaves Game Mode.
        let _ = zendo.pad(Pad::Back);
        let _ = zendo.pad(Pad::Down);
        let _ = zendo.pad(Pad::Down);
        let _ = zendo.pad(Pad::Activate);
        assert!(zendo.game_mode.is_none());

        // Outside Game Mode, Start still shows the hidden games.
        let _ = zendo.pad(Pad::ShowHidden);
        assert_eq!(zendo.view, library::View::Hidden);
    }
    /// Sets a play status on a game of the test library.
    fn with_status(zendo: &mut Zendo, id: &str, status: Status) {
        let _ = zendo.update(Message::SetGameStatus(id.to_owned(), Some(status)));
    }

    #[test]
    fn a_status_narrows_with_the_store_and_the_search() {
        let mut zendo = zendo_with(4, 4);
        with_status(&mut zendo, "desktop:0", Status::Backlog);
        with_status(&mut zendo, "desktop:1", Status::Backlog);
        with_status(&mut zendo, "desktop:2", Status::Playing);

        let _ = zendo.update(Message::SetStatus(Some(Status::Backlog)));
        assert_eq!(titles(&zendo), ["Game 00", "Game 01"]);
        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        assert_eq!(titles(&zendo), ["Game 01"]);
        let _ = zendo.update(Message::SetSource(None));
        let _ = zendo.update(Message::Search("00".into()));
        assert_eq!(titles(&zendo), ["Game 00"]);
        let _ = zendo.update(Message::Search(String::new()));

        let _ = zendo.update(Message::SetStatus(Some(Status::Backlog)));
        assert_eq!(zendo.play_status, None, "pressed twice, it is left");
        let _ = zendo.update(Message::SetStatus(Some(Status::Playing)));
        let _ = zendo.update(Message::ClearFilters);
        assert_eq!(zendo.play_status, None);
        assert_eq!(titles(&zendo).len(), 4);
    }

    #[test]
    fn a_game_gets_a_status_and_loses_it() {
        let mut zendo = zendo_with(2, 2);
        with_status(&mut zendo, "desktop:0", Status::Completed);
        assert_eq!(
            zendo.game("desktop:0").unwrap().status,
            Some(Status::Completed)
        );
        let _ = zendo.update(Message::SetGameStatus("desktop:0".into(), None));
        assert_eq!(zendo.game("desktop:0").unwrap().status, None);
        let _ = zendo.update(Message::SetGameStatus("nope".into(), Some(Status::Playing)));
    }

    #[test]
    fn a_saved_filter_puts_everything_back_and_reads_as_chosen() {
        let mut zendo = zendo_with(4, 4);
        in_category(&mut zendo, "Co-op", &["desktop:1", "desktop:3"]);
        with_status(&mut zendo, "desktop:1", Status::Playing);
        let _ = zendo.update(Message::SetView(library::View::Favorites));
        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        let _ = zendo.update(Message::SetCategory(Some("Co-op".into())));
        let _ = zendo.update(Message::SetStatus(Some(Status::Playing)));
        let _ = zendo.update(Message::Search("game".into()));
        let _ = zendo.update(Message::SetSort(Sort::MostPlayed));
        let saved = zendo.current_filter();
        zendo.filters = vec![SavedFilter {
            name: "Evening".into(),
            filter: saved.clone(),
        }];

        let _ = zendo.update(Message::ClearFilters);
        let _ = zendo.update(Message::SetView(library::View::All));
        let _ = zendo.update(Message::Search(String::new()));
        let _ = zendo.update(Message::SetSort(Sort::Title));
        assert_ne!(zendo.current_filter(), saved);

        let _ = zendo.update(Message::ApplyFilter("Evening".into()));
        assert_eq!(zendo.current_filter(), saved);
    }

    #[test]
    fn a_saved_filter_leaves_off_a_category_that_is_gone() {
        let mut zendo = zendo_with(2, 2);
        zendo.filters = vec![SavedFilter {
            name: "Old".into(),
            filter: zendo_core::Filter {
                category: Some("Deleted".into()),
                status: Some(Status::Backlog),
                ..zendo_core::Filter::default()
            },
        }];
        let _ = zendo.update(Message::ApplyFilter("Old".into()));
        assert_eq!(zendo.category, None);
        assert_eq!(zendo.play_status, Some(Status::Backlog));
    }

    #[test]
    fn the_collections_sheet_saves_updates_renames_and_deletes_a_view() {
        let mut zendo = zendo_with(2, 2);
        let _ = zendo.update(Message::SetSort(Sort::MostPlayed));
        let _ = zendo.update(Message::Collections(CollectionsMessage::Open(BTreeSet::new())));
        let _ = zendo.update(Message::Collections(CollectionsMessage::SaveView));
        assert_eq!(zendo.filters.len(), 0, "a blank name saves nothing");
        let _ = zendo.update(Message::Collections(CollectionsMessage::Typed("Evening".into())));
        let _ = zendo.update(Message::Collections(CollectionsMessage::SaveView));
        assert_eq!(zendo.filters.len(), 1);
        assert_eq!(zendo.filters[0].filter.sort, Sort::MostPlayed);

        let _ = zendo.update(Message::SetSource(Some(Source::Steam)));
        let _ = zendo.update(Message::Collections(CollectionsMessage::Typed("evening".into())));
        let _ = zendo.update(Message::Collections(CollectionsMessage::SaveView));
        assert_eq!(zendo.filters.len(), 1, "the same name is updated");
        assert_eq!(zendo.filters[0].filter.source, Some(Source::Steam));

        let _ = zendo.update(Message::Collections(CollectionsMessage::Rename("Evening".into())));
        let _ = zendo.update(Message::Collections(CollectionsMessage::RenameTyped(
            "Night".into(),
        )));
        let _ = zendo.update(Message::Collections(CollectionsMessage::RenameDone));
        assert_eq!(zendo.filters[0].name, "Night");

        let _ = zendo.update(Message::Collections(CollectionsMessage::Delete("Night".into())));
        assert_eq!(zendo.filters.len(), 1, "the first press only asks");
        let _ = zendo.update(Message::Collections(CollectionsMessage::Delete("Night".into())));
        assert_eq!(zendo.filters.len(), 0);
    }

    #[test]
    fn a_bulk_favorite_and_hide_set_every_marked_game_alike() {
        let mut zendo = zendo_with(4, 4);
        if let Some(game) = zendo.game_mut("desktop:1") {
            game.pinned = true;
        }
        for id in ["desktop:0", "desktop:1", "desktop:2"] {
            zendo.selected_games.insert(id.to_owned());
        }
        let _ = zendo.update(Message::BulkPinned(true));
        for id in ["desktop:0", "desktop:1", "desktop:2"] {
            assert!(zendo.game(id).unwrap().pinned, "{id}");
        }
        assert!(!zendo.game("desktop:3").unwrap().pinned);
        assert_eq!(zendo.selected_games.len(), 3, "a favorite keeps the marks");
        let _ = zendo.update(Message::BulkPinned(false));
        assert!(!zendo.game("desktop:1").unwrap().pinned);

        let _ = zendo.update(Message::BulkHidden(true));
        assert_eq!(titles(&zendo), ["Game 03"]);
        assert!(
            zendo.selected_games.is_empty(),
            "hidden games leave the marks"
        );
        let _ = zendo.update(Message::SetView(library::View::Hidden));
        for id in ["desktop:0", "desktop:1", "desktop:2"] {
            zendo.selected_games.insert(id.to_owned());
        }
        let _ = zendo.update(Message::BulkHidden(false));
        let _ = zendo.update(Message::SetView(library::View::All));
        assert_eq!(titles(&zendo).len(), 4);
    }

    #[test]
    fn ctrl_7_opens_stats_and_the_arrows_change_its_period_there_only() {
        let message = key_message(
            &Key::Character("7".into()),
            keyboard::Modifiers::CTRL,
            Some("7"),
        );
        assert!(matches!(message, Some(Message::Navigate(Page::Stats))));

        let mut zendo = zendo_with(2, 2);
        zendo.sessions = vec![zendo_core::sessions::Session {
            id: "desktop:0".into(),
            title: "Game 00".into(),
            start: 1_717_000_000,
            minutes: 30,
            offset: 0,
        }];
        let _ = zendo.update(Message::Navigate(Page::Stats));
        assert_eq!(zendo.page, Page::Stats);
        let before = zendo.stats.period;
        let _ = zendo.update(Message::Move(Move::Right));
        assert_ne!(zendo.stats.period, before);
        // On the Library the same key moves the cursor and leaves the period.
        let _ = zendo.update(Message::Navigate(Page::Library));
        let period = zendo.stats.period;
        let _ = zendo.update(Message::Move(Move::Right));
        assert_eq!(zendo.stats.period, period);
        assert_eq!(zendo.selected, 1);
    }
}
