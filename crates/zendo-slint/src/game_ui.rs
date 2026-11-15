//! A game's page in the window: what `GameData` shows of the game that is
//! open, the Play and Stop of the games Zendo starts, and what comes of it
//! (the running badge on the cards, the launch and the minutes counted, the
//! sessions kept). It is a part of the Library's glue (`library_ui`), which
//! owns the browser and the covers.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::launch::{self, Started, Watch};
use zendo_core::{Game, Source, State, Status, sessions};
use zendo_shell::presence::PresenceHandle;
use zendo_shell::queue;
use zendo_shell::running::{self, Effect, Names, RunMessage, Running};
use zendo_shell::{format, open, t};

use super::{Glue, with_glue};
use crate::covers::Cover;
use crate::{ChipData, Fact, GameData, MainWindow, Tile};

/// What the Store's glue makes of a store game, for its page.
pub(super) struct OwnedPage {
    pub(super) title: String,
    pub(super) source: String,
    pub(super) installed: bool,
    /// What installing it would do, in words.
    pub(super) note: String,
    pub(super) cover: Option<PathBuf>,
    pub(super) hero: Option<PathBuf>,
    pub(super) tiles: Vec<Tile>,
    pub(super) details: Vec<Fact>,
    pub(super) technical: Vec<Fact>,
}

/// What the game page keeps between one redraw and the next.
pub(super) struct GameState {
    /// The games Zendo started and still runs.
    pub(super) running: RefCell<Running>,
    /// Every session watched, for the page's Sessions card.
    pub(super) sessions: RefCell<Vec<sessions::Session>>,
    /// Each hero asked for, and the smaller copy drawn instead of it once made.
    backdrops: RefCell<HashMap<PathBuf, Option<PathBuf>>>,
    /// The cover and backdrop the page shows, so a picture arriving that the
    /// page does not use leaves it alone.
    images: RefCell<Vec<PathBuf>>,
    pub(super) technical: Cell<bool>,
    statuses: Rc<VecModel<ChipData>>,
    /// The names of the collections the open game is in.
    collections: Rc<VecModel<slint::SharedString>>,
    tiles: Rc<VecModel<Tile>>,
    facts: [Rc<VecModel<Fact>>; 4],
    /// Off in tests: no game is started and nothing is written.
    persist: bool,
}

impl GameState {
    pub(super) fn new(persist: bool) -> Self {
        let sessions = if persist {
            sessions::load().unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            running: RefCell::new(Running::default()),
            sessions: RefCell::new(sessions),
            backdrops: RefCell::new(HashMap::new()),
            images: RefCell::new(Vec::new()),
            technical: Cell::new(false),
            statuses: Rc::new(VecModel::default()),
            collections: Rc::new(VecModel::default()),
            tiles: Rc::new(VecModel::default()),
            facts: std::array::from_fn(|_| Rc::new(VecModel::default())),
            persist,
        }
    }
}

/// Stores write platforms as `Windows`, `windows`, `linux`; shown alike.
fn platform_label(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "windows" => "Windows".to_owned(),
        "linux" => "Linux".to_owned(),
        "mac" | "osx" => "macOS".to_owned(),
        _ => platform.to_owned(),
    }
}

/// A game Steam installed, by its app id.
pub(super) fn steam_app(game: &Game) -> Option<&str> {
    (game.source == Source::Steam)
        .then(|| game.id.strip_prefix("steam:"))
        .flatten()
}

/// The page of the game on `PCGamingWiki`: the redirect for a Steam app id,
/// else a search for the title.
fn pcgamingwiki_url(app: Option<&str>, title: &str) -> String {
    if let Some(app) = app {
        return format!("https://www.pcgamingwiki.com/api/appid.php?appid={app}");
    }
    let search: String = title
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    format!("https://www.pcgamingwiki.com/w/index.php?search={search}")
}

fn fact(label: impl Into<String>, value: impl Into<String>) -> Fact {
    Fact {
        label: label.into().into(),
        value: value.into().into(),
        path: false,
        mono: false,
        url: "".into(),
        tone: 0,
    }
}

fn mono(label: impl Into<String>, value: impl Into<String>) -> Fact {
    Fact {
        mono: true,
        ..fact(label, value)
    }
}

fn path_fact(label: impl Into<String>, path: &Path) -> Fact {
    Fact {
        path: true,
        mono: true,
        ..fact(label, path.display().to_string())
    }
}

/// The four numbers worth seeing first: the time played, when it was last
/// played, the achievements unlocked and the share of them.
fn tiles_of(game: &Game, now: u64) -> Vec<Tile> {
    let dash = || "—".to_owned();
    let (unlocked, share) = game.achievements.map_or_else(
        || (dash(), dash()),
        |(unlocked, total)| {
            (
                format!("{unlocked} / {total}"),
                format::progress_of((unlocked, total))
                    .map_or_else(dash, |share| format!("{:.0}%", share * 100.0)),
            )
        },
    );
    let tile = |label: String, value: String| Tile {
        label: label.into(),
        value: value.into(),
    };
    vec![
        tile(
            t!("fact-play-time"),
            if game.played.minutes > 0 {
                format::play_time(game.played.minutes)
            } else {
                dash()
            },
        ),
        tile(
            t!("fact-last-played"),
            game.played
                .last
                .map_or_else(dash, |last| format::ago(last, now)),
        ),
        tile(t!("achievements-title"), unlocked),
        tile(t!("stat-completion"), share),
    ]
}

impl Glue {
    /// Hooks the page's callbacks up.
    pub(super) fn connect_game(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameData>();
        data.set_statuses(ModelRc::from(Rc::clone(&self.game.statuses)));
        data.set_collections(ModelRc::from(Rc::clone(&self.game.collections)));
        let glue = Rc::clone(self);
        data.on_edit_collections(move || {
            if let Some(id) = glue.open.borrow().clone() {
                glue.open_collections(std::collections::BTreeSet::from([id]));
            }
        });
        data.set_tiles(ModelRc::from(Rc::clone(&self.game.tiles)));
        data.set_sessions(ModelRc::from(Rc::clone(&self.game.facts[0])));
        data.set_recent(ModelRc::from(Rc::clone(&self.game.facts[1])));
        data.set_details(ModelRc::from(Rc::clone(&self.game.facts[2])));
        data.set_technical(ModelRc::from(Rc::clone(&self.game.facts[3])));
        let glue = Rc::clone(self);
        data.on_back(move || {
            glue.close_game();
        });
        let glue = Rc::clone(self);
        data.on_play(move || glue.play());
        let glue = Rc::clone(self);
        data.on_stop(move || {
            if let Some(id) = glue.open.borrow().clone() {
                glue.run_event(RunMessage::Stop(id));
            }
        });
        let glue = Rc::clone(self);
        data.on_toggle_pinned(move || glue.toggle_pinned());
        let glue = Rc::clone(self);
        data.on_toggle_hidden(move || glue.toggle_hidden());
        let glue = Rc::clone(self);
        data.on_manage_steam(move || {
            let app = glue
                .open_game_record()
                .and_then(|game| steam_app(&game).map(str::to_owned));
            if let Some(app) = app {
                glue.open_target(&format!("steam://nav/games/details/{app}"));
            }
        });
        let glue = Rc::clone(self);
        data.on_wiki(move || {
            if let Some(game) = glue.open_game_record() {
                glue.open_target(&pcgamingwiki_url(steam_app(&game), &game.title));
            }
        });
        let glue = Rc::clone(self);
        data.on_status(move |index| glue.set_status(index));
        let glue = Rc::clone(self);
        data.on_open_path(move |path| glue.open_target(&path));
        let glue = Rc::clone(self);
        data.on_toggle_technical(move || {
            glue.game.technical.set(!glue.game.technical.get());
            glue.show_game();
        });
    }

    /// The open game, as the browser has it now.
    pub(super) fn open_game_record(&self) -> Option<Game> {
        let id = self.open.borrow().clone()?;
        self.browser.borrow().game(&id).cloned()
    }

    /// Opens a folder or a link in the desktop's program for it; tests open
    /// nothing.
    pub(super) fn open_target(&self, target: &str) {
        if self.game.persist
            && let Err(err) = open::target(target)
        {
            self.say(err);
        }
    }

    /// Says something in the footer.
    pub(super) fn say(&self, line: String) {
        self.browser.borrow_mut().say(Some(line));
        self.sync_status();
    }

    /// Opens a game's page.
    pub(super) fn open_game(&self, id: &str) {
        let found = {
            let mut browser = self.browser.borrow_mut();
            browser.click(id);
            browser.game(id).is_some()
        };
        self.sync_cursor();
        if found {
            *self.open.borrow_mut() = Some(id.to_owned());
            self.game.technical.set(false);
            *self.local.confirm_remove.borrow_mut() = None;
            self.reset_game_jobs();
            self.show_steam_input(id);
            self.want_cards(id);
            self.want_info(id);
            if let Some(window) = self.window.upgrade() {
                window.global::<GameData>().set_open(true);
            }
            self.show_game();
        }
    }

    /// Closes the open page, a library game's or a store game's.
    pub(super) fn close_game(&self) -> bool {
        let was_library = self.open.borrow_mut().take().is_some();
        self.reset_game_jobs();
        let was_store = self.close_owned();
        self.mods_clear();
        self.nexus_clear();
        let was_open = was_library || was_store;
        if was_open && let Some(window) = self.window.upgrade() {
            let data = window.global::<GameData>();
            data.set_open(false);
            data.set_owned(false);
        }
        was_open
    }

    /// The games were read again: the open one may have changed, or gone.
    pub(super) fn refresh_game(&self) {
        let gone = self
            .open
            .borrow()
            .as_deref()
            .is_some_and(|id| self.browser.borrow().game(id).is_none());
        if gone {
            self.close_game();
        } else {
            self.show_game();
        }
    }

    /// Whether the page draws this picture.
    pub(super) fn page_shows(&self, path: &Path) -> bool {
        (self.open.borrow().is_some() || self.store_page_open())
            && self.game.images.borrow().iter().any(|own| own == path)
    }

    /// The faint wide picture behind the header: the hero's smaller copy,
    /// made on a thread the first time.
    fn backdrop_of(&self, hero: &Path) -> Option<PathBuf> {
        if let Some(known) = self.game.backdrops.borrow().get(hero) {
            return known.clone();
        }
        let copy = zendo_core::thumbs::backdrop(hero);
        self.game
            .backdrops
            .borrow_mut()
            .insert(hero.to_path_buf(), copy.clone());
        if copy.is_none() && self.game.persist {
            let (weak, source) = (self.window.clone(), hero.to_path_buf());
            std::thread::spawn(move || {
                let made = zendo_core::thumbs::make_backdrop(&source).ok();
                let _ = weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| {
                        glue.game.backdrops.borrow_mut().insert(source, made);
                        glue.show_game();
                    });
                });
            });
        }
        copy
    }

    /// Writes the collections the open game is in, after a change to them.
    pub(super) fn show_game_collections(&self) {
        if let Some(game) = self.open_game_record() {
            let names: Vec<slint::SharedString> = game
                .categories
                .iter()
                .map(|name| slint::SharedString::from(name.as_str()))
                .collect();
            self.game.collections.set_vec(names);
        }
    }

    /// Writes the cover and the wide art, and says which pictures they are,
    /// so one arriving later draws the page again.
    fn show_art(
        &self,
        data: &GameData<'_>,
        cover: Option<&Path>,
        hero: Option<&Path>,
    ) -> Vec<PathBuf> {
        let mut images = Vec::new();
        let cover = cover.map(|path| (path, self.covers.borrow_mut().get(path)));
        if let Some((path, _)) = &cover {
            images.push(path.to_path_buf());
        }
        let cover = match cover {
            Some((_, Cover::Ready(image))) => Some(image),
            _ => None,
        };
        data.set_has_cover(cover.is_some());
        data.set_cover(cover.unwrap_or_default());
        let backdrop = hero.and_then(|hero| self.backdrop_of(hero)).map(|copy| {
            let shown = self.covers.borrow_mut().get_wide(&copy);
            images.push(copy);
            shown
        });
        let backdrop = match backdrop {
            Some(Cover::Ready(image)) => Some(image),
            _ => None,
        };
        data.set_has_backdrop(backdrop.is_some());
        data.set_backdrop(backdrop.unwrap_or_default());
        images
    }

    /// Writes a store game's page: the header and the details card are the
    /// game page's, filled from what the store says. `parts` is what the
    /// Store's glue made of the game.
    pub(super) fn show_owned_page(
        &self,
        parts: OwnedPage,
        cards: impl FnOnce(&GameData<'_>) -> Vec<PathBuf>,
    ) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<GameData>();
        data.set_owned(true);
        data.set_owned_installed(parts.installed);
        data.set_owned_note(parts.note.into());
        data.set_title(parts.title.into());
        data.set_source_line(parts.source.into());
        data.set_pinned(false);
        data.set_hidden(false);
        data.set_steam(false);
        data.set_local(false);
        data.set_launch_options_shown(false);
        data.set_confirm_remove(false);
        data.set_run_state(0);
        let mut images = self.show_art(&data, parts.cover.as_deref(), parts.hero.as_deref());
        images.extend(cards(&data));
        *self.game.images.borrow_mut() = images;
        self.game.tiles.set_vec(parts.tiles);
        self.game.facts[0].set_vec(Vec::new());
        self.game.facts[1].set_vec(Vec::new());
        self.game.facts[2].set_vec(parts.details);
        data.set_technical_open(self.game.technical.get());
        self.game.facts[3].set_vec(parts.technical);
    }

    /// Writes everything the open game's page shows.
    pub(super) fn show_game(&self) {
        // Game Mode's hero shows what a page learns (a summary, a cover).
        self.show_game_mode();
        // A store game's page is the open one, when there is one.
        if self.store_page_open() {
            self.show_owned();
            return;
        }
        let Some(game) = self.open_game_record() else {
            return;
        };
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<GameData>();
        data.set_owned(false);
        let now = running::now();

        let mut source = vec![format::source(game.source)];
        source.extend(game.details.developer.clone());
        source.extend(game.details.platform.as_deref().map(platform_label));
        data.set_title(game.title.clone().into());
        data.set_source_line(source.join(" · ").into());
        data.set_favorite_badge(t!("badge-favorite").into());
        data.set_hidden_badge(t!("badge-hidden").into());
        data.set_pinned(game.pinned);
        data.set_hidden(game.hidden);
        data.set_steam(steam_app(&game).is_some());
        data.set_local(game.source == Source::Local);
        data.set_launch_options_shown(zendo_shell::launch_options::applies(&game));
        data.set_confirm_remove(
            self.local.confirm_remove.borrow().as_deref() == Some(game.id.as_str()),
        );

        // The pictures: the cover the grid has too, and the wide art.
        let mut images = self.show_art(&data, game.cover.as_deref(), game.hero.as_deref());
        images.extend(self.show_cards(&game, &data));
        *self.game.images.borrow_mut() = images;

        // Play, Running with Stop.
        let state = match self.game.running.borrow().games.get(&game.id) {
            None => 0,
            Some(session) if !session.seen => 1,
            Some(session) if session.stopping => 3,
            Some(_) => 2,
        };
        data.set_run_state(state);
        let line = self
            .browser
            .borrow()
            .status()
            .map_or_else(|| t!("game-ready"), str::to_owned);
        data.set_line(line.into());
        self.show_game_jobs(&game, &data);

        let statuses: Vec<ChipData> = Status::ALL
            .iter()
            .enumerate()
            .map(|(index, status)| ChipData {
                id: i32::try_from(index).unwrap_or(0),
                label: format::play_status(*status).into(),
                count: "".into(),
                chosen: game.status == Some(*status),
            })
            .collect();
        self.game.statuses.set_vec(statuses);
        self.show_game_collections();
        self.game.tiles.set_vec(tiles_of(&game, now));

        let (summary, recent) = self.sessions_of(&game.id);
        self.game.facts[0].set_vec(summary);
        self.game.facts[1].set_vec(recent);
        self.game.facts[2].set_vec(self.details_of(&game));
        data.set_technical_open(self.game.technical.get());
        self.game.facts[3].set_vec(self.technical_of(&game));
    }

    /// How many sessions, the average and the longest, and the latest ones;
    /// nothing until the game has one.
    fn sessions_of(&self, id: &str) -> (Vec<Fact>, Vec<Fact>) {
        let all = self.game.sessions.borrow();
        let Some(own) = sessions::of_game(&all, id) else {
            return (Vec::new(), Vec::new());
        };
        let summary = vec![
            fact(t!("stats-sessions"), own.count.to_string()),
            fact(t!("stats-average"), format::play_time(own.average)),
            fact(t!("stats-longest"), format::play_time(own.longest)),
        ];
        let recent = own
            .recent
            .iter()
            .map(|session| {
                fact(
                    sessions::date_of(session),
                    format::play_time(session.minutes),
                )
            })
            .collect();
        (summary, recent)
    }

    /// The details worth a look: the store and install, and what Zendo has
    /// counted. (The store's own facts come with item 49b.)
    fn details_of(&self, game: &Game) -> Vec<Fact> {
        let mut rows = vec![
            fact(t!("fact-store"), format::source(game.source)),
            fact(t!("fact-started-by"), game.launcher()),
        ];
        if let Some(platform) = &game.details.platform {
            rows.push(fact(t!("fact-platform"), platform_label(platform)));
        }
        if let Some(version) = &game.details.version {
            rows.push(fact(t!("fact-version"), version.clone()));
        }
        if let Some(size) = &game.details.size {
            rows.push(fact(t!("fact-install-size"), size.to_string()));
        }
        if game.installed_by_zendo() {
            // What the last look for an update found.
            let (line, tone) =
                queue::update_line(self.downloads.queue.borrow().updates.get(&game.id));
            if !line.is_empty() {
                rows.push(Fact {
                    tone: match tone {
                        queue::Tone::Good => 1,
                        queue::Tone::Bad => 2,
                        queue::Tone::Dim => 3,
                        queue::Tone::Accent => 4,
                    },
                    ..fact(t!("fact-updates"), line)
                });
            }
        }
        if let Some(path) = &game.details.install_path {
            rows.push(path_fact(t!("fact-install-path"), path));
        }
        rows.extend(self.cloud_row(game));
        rows.extend(self.store_fact_rows(&game.id, game.details.size.is_some()));
        rows.extend(self.steam_fact_rows(game, self.owned_of(&game.id).as_ref()));
        rows.push(fact(
            t!("fact-started-from-zendo"),
            t!("count-times", count = game.played.launches),
        ));
        rows
    }

    /// The launch command and Zendo's id; for a game Zendo starts, its Wine
    /// prefix and last log.
    fn technical_of(&self, game: &Game) -> Vec<Fact> {
        let mut rows = vec![
            mono(t!("fact-launch-command"), game.command_line()),
            mono(t!("fact-zendo-id"), game.id.clone()),
        ];
        let starts_itself = game.installed_by_zendo() || game.source == Source::Local;
        if starts_itself
            && launch::needs_proton(game)
            && let Some(dir) = self
                .config
                .borrow()
                .as_ref()
                .and_then(zendo_core::config::Config::install_dir)
        {
            rows.push(path_fact(
                t!("run-wine-prefix"),
                &launch::prefix_in(&dir, game),
            ));
        }
        if starts_itself && let Some(log) = launch::log_path(&game.id) {
            rows.push(path_fact(t!("run-last-log"), &log));
        }
        rows
    }

    // The buttons.

    pub(super) fn toggle_pinned(&self) {
        if let Some(id) = self.open.borrow().clone() {
            let outcome = self.browser.borrow_mut().toggle_pinned_of(&id);
            self.apply(outcome);
            self.show_game();
        }
    }

    pub(super) fn toggle_hidden(&self) {
        if let Some(id) = self.open.borrow().clone() {
            let outcome = self.browser.borrow_mut().toggle_hidden_of(&id);
            self.apply(outcome);
            self.show_game();
        }
    }

    /// A status chip: the chosen one is pressed again to clear it.
    fn set_status(&self, index: i32) {
        let (Some(id), Some(choice)) = (
            self.open.borrow().clone(),
            usize::try_from(index)
                .ok()
                .and_then(|index| Status::ALL.get(index).copied()),
        ) else {
            return;
        };
        let chosen = self.browser.borrow().game(&id).and_then(|game| game.status) == Some(choice);
        let outcome = self
            .browser
            .borrow_mut()
            .set_status_of(&id, if chosen { None } else { Some(choice) });
        self.apply(outcome);
        self.show_game();
    }

    /// A key on a game's page: Enter plays, Delete hides.
    pub(super) fn game_key(&self, key: char) -> bool {
        use slint::platform::Key;
        if key == char::from(Key::Return) {
            self.play();
            true
        } else if key == char::from(Key::Delete) {
            self.toggle_hidden();
            true
        } else {
            false
        }
    }

    // Playing.

    /// Starts the open game, on a thread: its launch may look things up.
    pub(super) fn play(&self) {
        if let Some(game) = self.open_game_record() {
            self.play_record(game);
        }
    }

    /// Starts a game, whichever page asked: the open game's Play, or Game
    /// Mode's hero.
    pub(super) fn play_record(&self, game: Game) {
        if self.game.running.borrow().is_open(&game.id) {
            return;
        }
        if self.moving(&game.id) {
            self.say(t!("downloads-moving-wait", title = game.title.as_str()));
            return;
        }
        self.say(t!("run-starting-game", title = game.title.as_str()));
        // Tests start nothing: as if the game went to its launcher.
        if !self.game.persist {
            let started = Started {
                watch: Watch::Nothing,
                log: None,
                warning: None,
            };
            self.run_event(RunMessage::Started(game.id, Ok(started)));
            return;
        }
        self.game.running.borrow_mut().begin(&game.id);
        let weak = self.window.clone();
        running::follow(game, move |message| {
            weak.upgrade_in_event_loop(move |_| with_glue(|glue| glue.run_event(message)))
                .is_ok()
        });
        self.show_game();
    }

    /// What happened to a game Zendo started, and what the window does
    /// about it: Discord's presence, the saves' sync, the GOG total, and the
    /// queue's jobs that were held while the game ran (`Effect::NextJob`).
    pub(super) fn run_event(&self, message: RunMessage) {
        let effects = {
            let browser = self.browser.borrow();
            let names = |id: &str| Names::of(id, browser.game(id));
            self.game.running.borrow_mut().apply(message, &names)
        };
        for effect in effects {
            match effect {
                Effect::Say(line) => self.say(line),
                Effect::Launched(id) => {
                    let outcome = self.browser.borrow_mut().launched(&id, running::now());
                    self.apply(outcome);
                }
                Effect::Played(id, minutes) => {
                    if self.game.persist
                        && let Err(err) = State::update(|state| state.add_minutes(&id, minutes))
                    {
                        self.say(format!("{err:#}"));
                    }
                    let outcome = self.browser.borrow_mut().add_minutes(&id, minutes);
                    self.apply(outcome);
                }
                Effect::Session { id, since, minutes } => self.record_session(&id, since, minutes),
                Effect::HideOnLaunch => {
                    if let Some(window) = self.window.upgrade() {
                        crate::window::hide_for_launch(&window);
                    }
                }
                Effect::Changed(id) => {
                    let running = self.game.running.borrow().is_running(&id);
                    let outcome = self.browser.borrow_mut().set_running(&id, running);
                    self.apply(outcome);
                }
                Effect::CloudReload => self.cloud_reload(),
                Effect::CloudSyncing(id, syncing) => self.cloud_syncing(id, syncing),
                Effect::RefreshGogTotal(id) => self.refresh_gog_total(&id),
                Effect::Presence => self.update_presence(),
                Effect::NextJob => self.next_job(),
            }
        }
        self.show_game();
    }

    /// Tells Discord who is playing, when the Discord setting is on.
    pub(super) fn update_presence(&self) {
        let presence = self.presence.borrow();
        let Some(presence) = presence.as_ref() else {
            return;
        };
        let browser = self.browser.borrow();
        let shown = self
            .game
            .running
            .borrow()
            .presence(|id| Names::of(id, browser.game(id)).title);
        presence.show(shown);
    }

    /// Starts the presence at launch when the setting asks for it.
    pub(super) fn start_presence(&self) {
        if self.persist
            && zendo_core::config::Config::load_file().is_ok_and(|config| config.discord_presence)
        {
            *self.presence.borrow_mut() = Some(PresenceHandle::spawn());
        }
    }

    /// Keeps a session in the page's list and, outside tests, in the
    /// sessions file; a failure is said in the footer.
    fn record_session(&self, id: &str, start: u64, minutes: u64) {
        let title = self
            .browser
            .borrow()
            .game(id)
            .map_or_else(|| id.to_owned(), |game| game.title.clone());
        let session = sessions::Session {
            id: id.to_owned(),
            title,
            start,
            minutes,
            offset: if self.game.persist {
                sessions::local_offset(start)
            } else {
                0
            },
        };
        if self.game.persist
            && let Err(err) = sessions::record(&session)
        {
            self.say(format!("{err:#}"));
        }
        self.game.sessions.borrow_mut().push(session);
        if self.on_stats() {
            self.show_stats();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_to_pcgamingwiki_by_app_id_or_by_title() {
        assert_eq!(
            pcgamingwiki_url(Some("620"), "Portal 2"),
            "https://www.pcgamingwiki.com/api/appid.php?appid=620"
        );
        assert_eq!(
            pcgamingwiki_url(None, "Baldur's Gate 3 – GOTY"),
            "https://www.pcgamingwiki.com/w/index.php?search=Baldur%27s%20Gate%203%20%E2%80%93%20GOTY"
        );
    }
}
