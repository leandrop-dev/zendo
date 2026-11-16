//! What the stores and sites say about the open game, as cards on its page:
//! `ProtonDB`'s rating, Steam's store page (description, facts, screenshots),
//! the system requirements, the achievements and the DLC installed, and the
//! Steam app id that ties a game to those. Each is asked of the network on
//! a thread, once a session, when the page opens; a failure is shown on the
//! page with the way to ask again. A part of the Library's glue
//! (`library_ui`), which owns the browser and the covers.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};
use zendo_core::achievements::{self, Achievement, Achievements};
use zendo_core::owned::{Owned, StoreDetails};
use zendo_core::protondb::{self, Rating};
use zendo_core::requirements::{self, From, Requirements, System};
use zendo_core::steam_facts::{self, Facts};
use zendo_core::{Game, Source, State, steam};
use zendo_shell::running::now;
use zendo_shell::{format, t};

use super::store::DlcView;
use super::{Glue, with_glue};
use crate::covers::Cover;
use crate::{Ach, DlcRow, Fact, GameData, MainWindow, Pill, Req, Shot};

/// How many achievements show at first, and how many more Show more adds.
const SHOWN: usize = 10;

/// How many of Steam's features the details card names before "+N more".
const FEATURES_SHOWN: usize = 6;

/// A question put to the network and what is known of the answer.
#[derive(Debug)]
pub(super) enum Ask<T> {
    Asking,
    Got(T),
    /// What went wrong; the card offers to ask again.
    Failed(String),
}

/// The cards' answers, by game, and the models the page reads them from.
pub(super) struct CardsState {
    /// `None`: no Steam app was found for the game, so `ProtonDB` cannot know it.
    ratings: RefCell<HashMap<String, Ask<Option<Rating>>>>,
    /// `None`: Steam's store has no page for the app.
    facts: RefCell<HashMap<String, Ask<Option<Box<Facts>>>>>,
    /// `None`: no store lists them.
    requirements: RefCell<HashMap<String, Ask<Option<Requirements>>>>,
    /// `None`: the store gives none.
    achievements: RefCell<HashMap<String, Ask<Option<Achievements>>>>,
    /// How many achievements are shown beyond the first ones.
    achievements_more: Cell<usize>,
    pills: Rc<VecModel<Pill>>,
    shots: Rc<VecModel<Shot>>,
    dlc: Rc<VecModel<DlcRow>>,
    achievements_model: Rc<VecModel<Ach>>,
    requirements_model: Rc<VecModel<Req>>,
    failures: Rc<VecModel<Fact>>,
}

impl CardsState {
    pub(super) fn new() -> Self {
        Self {
            ratings: RefCell::default(),
            facts: RefCell::default(),
            requirements: RefCell::default(),
            achievements: RefCell::default(),
            achievements_more: Cell::new(0),
            pills: Rc::new(VecModel::default()),
            shots: Rc::new(VecModel::default()),
            dlc: Rc::new(VecModel::default()),
            achievements_model: Rc::new(VecModel::default()),
            requirements_model: Rc::new(VecModel::default()),
            failures: Rc::new(VecModel::default()),
        }
    }
}

/// The colour of a `ProtonDB` tier, or `None` for the theme's own.
fn tier_color(tier: &str) -> Option<Color> {
    match tier {
        "platinum" => Some(Color::from_rgb_u8(0xb4, 0xc7, 0xdc)),
        "gold" => Some(Color::from_rgb_u8(0xcf, 0xb5, 0x3b)),
        "silver" => Some(Color::from_rgb_u8(0xa6, 0xa6, 0xa6)),
        "bronze" => Some(Color::from_rgb_u8(0xcd, 0x7f, 0x32)),
        _ => None,
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// What the reports say, for the pill's words.
fn reports(rating: &Rating) -> String {
    match (&rating.tier, rating.reports) {
        (None, _) => t!("protondb-no-reports-yet"),
        (Some(tier), count) => t!("protondb-reports", tier = title_case(tier), count = count),
    }
}

/// The pill for the game's page: Native for a game with a Linux build, else
/// the reports' tier. The tone says which of the theme's colours it takes
/// (1 good, 2 bad, 3 dim), or 0 for the tier's own.
fn pill_of(rating: &Rating) -> Pill {
    let (label, tone, tint, about) = match &rating.tier {
        _ if rating.native => (
            t!("protondb-native"),
            1,
            None,
            t!("protondb-native-about", reports = reports(rating)),
        ),
        Some(tier) => {
            let own = tier_color(tier);
            let tone = if tier == "borked" { 2 } else { 3 };
            (
                format!("PROTONDB {}", tier.to_uppercase()),
                if own.is_some() { 0 } else { tone },
                own,
                t!("protondb-tier-about", reports = reports(rating)),
            )
        }
        None => (t!("protondb-none"), 3, None, t!("protondb-none-about")),
    };
    Pill {
        label: label.into(),
        tint: tint.unwrap_or_default(),
        tone,
        tip: about.into(),
        url: rating.url().into(),
    }
}

/// Steam's features of the game, with its controller support when the
/// features do not already say it.
fn features_line(details: &StoreDetails) -> String {
    let mut features = details.features.clone();
    let controller = match details.controller.as_deref() {
        Some("full") => Some(t!("fact-controller-full")),
        Some("partial") => Some(t!("fact-controller-partial")),
        _ => None,
    };
    if let Some(controller) = controller
        && !features
            .iter()
            .any(|feature| feature.eq_ignore_ascii_case(&controller))
    {
        features.push(controller);
    }
    // Steam lists up to thirty; the first few say what the game is.
    let more = features.len().saturating_sub(FEATURES_SHOWN);
    features.truncate(FEATURES_SHOWN);
    let mut line = features.join(", ");
    if more > 0 {
        line.push_str(&t!("fact-features-more", count = more.to_string()));
    }
    line
}

/// A share of players: one decimal under 10 %, whole numbers above.
fn rarity_text(percent: f32) -> String {
    if percent < 10.0 {
        format!("{percent:.1}")
    } else {
        format!("{percent:.0}")
    }
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

impl Glue {
    /// Hooks the cards' callbacks up and gives the page their models.
    pub(super) fn connect_cards(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameData>();
        data.set_pills(ModelRc::from(Rc::clone(&self.cards.pills)));
        data.set_shots(ModelRc::from(Rc::clone(&self.cards.shots)));
        data.set_dlc(ModelRc::from(Rc::clone(&self.cards.dlc)));
        data.set_ach(ModelRc::from(Rc::clone(&self.cards.achievements_model)));
        data.set_req(ModelRc::from(Rc::clone(&self.cards.requirements_model)));
        data.set_failures(ModelRc::from(Rc::clone(&self.cards.failures)));
        let glue = Rc::clone(self);
        data.on_more_achievements(move || {
            glue.cards
                .achievements_more
                .set(glue.cards.achievements_more.get() + SHOWN);
            glue.show_game();
        });
        let glue = Rc::clone(self);
        data.on_retry(move || {
            // A library game's page, or a store game's.
            let id = glue.open.borrow().clone().or_else(|| glue.store_open_id());
            if let Some(id) = id {
                glue.retry_cards(&id);
            }
        });
        // The screenshot lightbox: open one, close it, or step along them.
        let glue = Rc::clone(self);
        data.on_open_shot(move |index| {
            if let Some(window) = glue.window.upgrade() {
                window.global::<GameData>().set_shot(index);
            }
        });
        let glue = Rc::clone(self);
        data.on_close_shot(move || {
            if let Some(window) = glue.window.upgrade() {
                window.global::<GameData>().set_shot(-1);
            }
        });
        let glue = Rc::clone(self);
        data.on_step_shot(move |step| {
            if let Some(window) = glue.window.upgrade() {
                let data = window.global::<GameData>();
                let count = data.get_shots().row_count();
                if count > 0 {
                    let next = (data.get_shot() + step)
                        .rem_euclid(i32::try_from(count).unwrap_or(i32::MAX));
                    data.set_shot(next);
                }
            }
        });
        let glue = Rc::clone(self);
        data.on_show_steam_app(move || {
            glue.game.technical.set(true);
            glue.show_game();
            if let Some(window) = glue.window.upgrade() {
                // The Technical card is the page's last: show it.
                let data = window.global::<GameData>();
                data.set_scroll_end(data.get_scroll_end() + 1);
            }
        });
        let glue = Rc::clone(self);
        data.on_steam_app_save(move |typed| glue.save_steam_app(&typed));
        let glue = Rc::clone(self);
        data.on_steam_app_clear(move || {
            if let Some(id) = glue.open.borrow().clone() {
                glue.give_steam_app(&id, None);
            }
        });
    }

    /// Whether the screenshot lightbox is open.
    pub(super) fn shot_open(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|window| window.global::<GameData>().get_shot() >= 0)
    }

    /// Escape closes the screenshot lightbox.
    pub(super) fn shots_back(&self) -> bool {
        let Some(window) = self.window.upgrade() else {
            return false;
        };
        let data = window.global::<GameData>();
        if data.get_shot() < 0 {
            return false;
        }
        data.set_shot(-1);
        true
    }

    /// Runs `work` on a thread and hands its answer to `arrived` on the
    /// window's thread. Nothing is asked in tests.
    pub(super) fn ask<R: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<R, String> + Send + 'static,
        arrived: impl FnOnce(&Self, Result<R, String>) + Send + 'static,
    ) {
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|payload| Err(super::panicked_note(&*payload)));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| arrived(glue, result));
            });
        });
    }

    /// The page just opened: asks what it has not asked yet this session.
    pub(super) fn want_cards(&self, id: &str) {
        self.cards.achievements_more.set(0);
        self.want_prefix(id);
        self.mods_want(id);
        self.want_gog(id);
        self.want_rating(id);
        self.want_achievements(id);
    }

    /// A store game's page just opened: its rating and achievements, which
    /// need no installed game. The prefix and GOG's extras do.
    pub(super) fn want_owned_cards(&self, id: &str) {
        self.cards.achievements_more.set(0);
        self.want_rating(id);
        self.want_achievements(id);
    }

    /// Asks `ProtonDB` about the game, once a session. Steam's store is asked
    /// for the game's facts on the way when its app id is known already.
    fn want_rating(&self, id: &str) {
        self.want_known_facts(id);
        if !self.persist || self.cards.ratings.borrow().contains_key(id) {
            return;
        }
        let Some((title, chosen)) = self.card_game(id).map(|game| (game.title, game.steam_app))
        else {
            return;
        };
        self.cards
            .ratings
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let owned = id.to_owned();
        let asked = id.to_owned();
        self.ask(
            move || protondb::rating(&asked, &title, chosen).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.rating_arrived(owned, result),
        );
    }

    /// Keeps the answer, then asks for the requirements and Steam's page by
    /// the Steam app id found; a failure (offline, say) is forgotten, so the
    /// page asks again the next time it opens.
    pub(super) fn rating_arrived(&self, id: String, result: Result<Option<Rating>, String>) {
        let Ok(rating) = result else {
            self.cards.ratings.borrow_mut().remove(&id);
            return;
        };
        let app = rating.as_ref().map(|rating| rating.app);
        self.want_compat(&id, app);
        self.want_requirements(&id, app);
        if let Some(app) = app {
            self.want_facts(&id, app);
        }
        self.cards.ratings.borrow_mut().insert(id, Ask::Got(rating));
        self.show_game();
    }

    /// Asks Steam's store when the game's Steam app id is known without
    /// asking the network: the user's, a Steam game's own, or the one a
    /// `ProtonDB` lookup found earlier.
    pub(super) fn want_known_facts(&self, id: &str) {
        if !self.persist {
            return;
        }
        let chosen = self.card_game(id).and_then(|game| game.steam_app);
        if let Some(app) = steam_facts::app_for(id, chosen) {
            self.want_facts(id, app);
        }
    }

    fn want_facts(&self, id: &str, app: u32) {
        if !self.persist
            || matches!(
                self.cards.facts.borrow().get(id),
                Some(Ask::Asking | Ask::Got(_))
            )
        {
            return;
        }
        self.cards
            .facts
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let id = id.to_owned();
        self.ask(
            move || steam_facts::for_app(app).map_err(|err| format!("{err:#}")),
            move |glue, result| glue.facts_arrived(id, result),
        );
    }

    pub(super) fn facts_arrived(&self, id: String, result: Result<Option<Facts>, String>) {
        let state = match result {
            Ok(found) => Ask::Got(found.map(Box::new)),
            Err(err) => Ask::Failed(err),
        };
        self.cards.facts.borrow_mut().insert(id, state);
        self.show_game();
    }

    fn want_requirements(&self, id: &str, steam_app: Option<u32>) {
        if !self.persist
            || matches!(
                self.cards.requirements.borrow().get(id),
                Some(Ask::Asking | Ask::Got(_))
            )
        {
            return;
        }
        let linux = self
            .card_game(id)
            .and_then(|game| game.details.platform)
            .is_some_and(|platform| platform.eq_ignore_ascii_case("linux"));
        let system = if linux {
            System::Linux
        } else {
            System::Windows
        };
        self.cards
            .requirements
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let (owned, asked) = (id.to_owned(), id.to_owned());
        self.ask(
            move || {
                requirements::for_game(&asked, steam_app, system).map_err(|err| format!("{err:#}"))
            },
            move |glue, result| glue.requirements_arrived(owned, result),
        );
    }

    pub(super) fn requirements_arrived(
        &self,
        id: String,
        result: Result<Option<Requirements>, String>,
    ) {
        let state = match result {
            Ok(found) => Ask::Got(found),
            Err(err) => Ask::Failed(err),
        };
        self.cards.requirements.borrow_mut().insert(id, state);
        self.show_game();
    }

    /// Asks the game's store for its achievements, once a session; nothing
    /// for a game with none to ask.
    fn want_achievements(&self, id: &str) {
        let from_store = ["gog:", "steam:", "epic:"]
            .iter()
            .any(|prefix| id.starts_with(prefix));
        if !from_store
            || !self.persist
            || matches!(
                self.cards.achievements.borrow().get(id),
                Some(Ask::Asking | Ask::Got(_))
            )
        {
            return;
        }
        self.cards
            .achievements
            .borrow_mut()
            .insert(id.to_owned(), Ask::Asking);
        let (owned, asked) = (id.to_owned(), id.to_owned());
        let signed_in = self.steam_session().map(|session| session.steamid);
        self.ask(
            move || {
                // The Steam sign-in's account, else the one Steam on this
                // computer signed in last.
                let steam_id =
                    signed_in.or_else(|| steam::user(&steam::roots()).map(|user| user.id));
                achievements::for_game(&asked, steam_id.as_deref())
                    .map_err(|err| format!("{err:#}"))
            },
            move |glue, result| glue.achievements_arrived(owned, result),
        );
    }

    pub(super) fn achievements_arrived(
        &self,
        id: String,
        result: Result<Option<Achievements>, String>,
    ) {
        // Kept for the library's progress line, when the unlocks are known.
        let seen = match &result {
            Ok(Some(found)) if found.unlocks_known => {
                let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
                Some((count(found.unlocked()), count(found.list.len())))
            }
            _ => None,
        };
        let state = match result {
            Ok(found) => Ask::Got(found),
            Err(err) => Ask::Failed(err),
        };
        if let Some((unlocked, total)) = seen {
            let outcome = self
                .browser
                .borrow_mut()
                .set_achievements(&id, unlocked, total);
            self.apply(outcome);
            if self.persist
                && let Err(err) =
                    State::update(|state| state.set_achievements(&id, unlocked, total))
            {
                self.say(format!("{err:#}"));
            }
        }
        self.cards.achievements.borrow_mut().insert(id, state);
        self.show_game();
    }

    /// Asks again for the cards that failed, with the Steam app id the
    /// rating found.
    fn retry_cards(&self, id: &str) {
        let app = match self.cards.ratings.borrow().get(id) {
            Some(Ask::Got(Some(rating))) => Some(rating.app),
            _ => None,
        };
        self.want_compat(id, app);
        self.want_requirements(id, app);
        self.want_achievements(id);
        if let Some(app) = app {
            self.want_facts(id, app);
        }
        self.show_game();
    }

    // The Steam app id.

    /// Saves what was typed as the game's Steam app id.
    fn save_steam_app(&self, typed: &str) {
        let Some(id) = self.open.borrow().clone() else {
            return;
        };
        match typed.trim().parse::<u32>() {
            Ok(app) if app > 0 => self.give_steam_app(&id, Some(app)),
            _ => self.say(t!("steam-app-invalid")),
        }
    }

    /// Gives the game a Steam app id, or takes it back with `None`; what
    /// was found for the old one is forgotten and asked again.
    pub(super) fn give_steam_app(&self, id: &str, app: Option<u32>) {
        let Some(title) = self.browser.borrow_mut().set_steam_app(id, app) else {
            return;
        };
        if self.persist
            && let Err(err) = State::update(|state| {
                state.set_steam_app(id, app);
            })
        {
            self.say(format!("{err:#}"));
        }
        self.cards.ratings.borrow_mut().remove(id);
        self.cards.facts.borrow_mut().remove(id);
        self.cards.requirements.borrow_mut().remove(id);
        self.forget_compat(id);
        self.say(if let Some(app) = app {
            t!("steam-app-saved", title = title, app = app.to_string())
        } else {
            t!("steam-app-cleared", title = title)
        });
        self.show_steam_input(id);
        self.want_rating(id);
        self.show_game();
    }

    /// Puts the Steam app id the game has in the field, for when the page
    /// opens or it was changed.
    pub(super) fn show_steam_input(&self, id: &str) {
        let given = self
            .browser
            .borrow()
            .game(id)
            .and_then(|game| game.steam_app)
            .map(|app| app.to_string())
            .unwrap_or_default();
        if let Some(window) = self.window.upgrade() {
            window
                .global::<GameData>()
                .set_steam_app_input(given.into());
        }
    }

    // What the page shows of them.

    /// The facts of the game, once they have come and the store has a page.
    pub(super) fn facts_of(&self, id: &str) -> Option<Facts> {
        match self.cards.facts.borrow().get(id)? {
            Ask::Got(Some(facts)) => Some((**facts).clone()),
            _ => None,
        }
    }

    /// Writes the cards of the open game. Returns the pictures they draw, so
    /// one arriving later draws the page again.
    pub(super) fn show_cards(&self, game: &Game, data: &GameData<'_>) -> Vec<PathBuf> {
        let mut images = Vec::new();
        let id = game.id.as_str();

        // The pills.
        let mut pills = Vec::new();
        match self.cards.ratings.borrow().get(id) {
            Some(Ask::Asking) => pills.push(Pill {
                label: "PROTONDB …".into(),
                tint: Color::default(),
                tone: 3,
                tip: "".into(),
                url: "".into(),
            }),
            Some(Ask::Got(Some(rating))) => pills.push(pill_of(rating)),
            _ => {}
        }
        pills.extend(self.deck_pills(id));
        self.cards.pills.set_vec(pills);

        // About, or the way to give the game a Steam app id.
        let facts = self.facts_of(id);
        let about = facts
            .as_ref()
            .and_then(|facts| facts.details.summary.clone())
            .unwrap_or_default();
        data.set_about(about.clone().into());
        let none_found = matches!(self.cards.ratings.borrow().get(id), Some(Ask::Got(None)));
        let no_match =
            about.is_empty() && none_found && !id.starts_with("steam:") && game.steam_app.is_none();
        data.set_no_match(if no_match {
            t!("steam-app-no-match").into()
        } else {
            "".into()
        });

        // Screenshots: their small images beside the card, and their full ones
        // for the lightbox.
        let mut shots = Vec::new();
        if let Some(facts) = &facts {
            for shot in &facts.shots {
                images.push(shot.thumb.clone());
                images.push(shot.full.clone());
                let picture = match self.covers.borrow_mut().get(&shot.thumb) {
                    Cover::Ready(image) => Some(image),
                    _ => None,
                };
                let full = match self.covers.borrow_mut().get_wide(&shot.full) {
                    Cover::Ready(image) => Some(image),
                    _ => None,
                };
                shots.push(Shot {
                    ready: picture.is_some(),
                    thumb: picture.unwrap_or_default(),
                    full_ready: full.is_some(),
                    full: full.unwrap_or_default(),
                });
            }
        }
        // The open picture stays inside the list that replaced it.
        let count = i32::try_from(shots.len()).unwrap_or(i32::MAX);
        if data.get_shot() >= count {
            data.set_shot(count - 1);
        }
        self.cards.shots.set_vec(shots);

        self.show_anti_cheat(id, data);
        self.show_dlc(game, data);
        self.show_mods(game);
        self.show_prefix(game, data);
        self.show_changelog(id, data);
        images.extend(self.show_achievements(id, data));
        self.show_requirements(id, data);
        self.show_failures(id);
        self.show_steam_row(game, data);
        images
    }

    /// The DLC of a game of Epic or GOG: those the store says the user owns
    /// and those installed, with Install and Uninstall for a game Zendo
    /// installed (`game_jobs_ui.rs` does what they ask).
    pub(super) fn show_dlc(&self, game: &Game, data: &GameData<'_>) {
        let view = if matches!(game.source, Source::Epic | Source::Gog) {
            self.dlc_rows(game)
        } else {
            DlcView::default()
        };
        data.set_dlc_count(view.count.into());
        data.set_dlc_install_all(view.all.into());
        data.set_dlc_install_all_on(view.all_on);
        data.set_dlc_note(view.note.into());
        self.cards.dlc.set_vec(view.rows);
    }

    fn show_achievements(&self, id: &str, data: &GameData<'_>) -> Vec<PathBuf> {
        let mut images = Vec::new();
        let mut rows = Vec::new();
        data.set_ach_share(-1.0);
        data.set_ach_more("".into());
        data.set_ach_summary("".into());
        if let Some(Ask::Got(Some(found))) = self.cards.achievements.borrow().get(id) {
            let total = found.list.len();
            if found.unlocks_known {
                let unlocked = found.unlocked();
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "counts of achievements, far below f32's exact range"
                )]
                let share = unlocked as f32 / total.max(1) as f32;
                data.set_ach_share(share);
                data.set_ach_summary(
                    t!("achievements-unlocked", unlocked = unlocked, total = total).into(),
                );
            } else {
                let why = if id.starts_with("steam:") {
                    t!("achievements-unknown-steam", total = total)
                } else {
                    t!("achievements-unknown", total = total)
                };
                data.set_ach_summary(why.into());
            }
            let shown = (SHOWN + self.cards.achievements_more.get()).min(total);
            let at = now();
            for achievement in &found.list[..shown] {
                let icon = achievement.icon.as_ref().and_then(|path| {
                    images.push(path.clone());
                    match self.covers.borrow_mut().get(path) {
                        Cover::Ready(image) => Some(image),
                        _ => None,
                    }
                });
                rows.push(achievement_row(achievement, icon, at));
            }
            let left = total - shown;
            if left > 0 {
                data.set_ach_more(t!("achievements-more", count = left).into());
            }
        }
        self.cards.achievements_model.set_vec(rows);
        images
    }

    fn show_requirements(&self, id: &str, data: &GameData<'_>) {
        let mut rows = Vec::new();
        data.set_req_both(false);
        data.set_req_source("".into());
        if let Some(Ask::Got(Some(found))) = self.cards.requirements.borrow().get(id) {
            let system = match found.system {
                System::Windows => "Windows",
                System::Linux => "Linux",
            };
            let source = match found.from {
                From::Gog => t!("requirements-from-gog", system = system),
                From::Epic => t!("requirements-from-epic", system = system),
                From::Steam => t!("requirements-from-steam", system = system),
            };
            let both = found.has_recommended();
            data.set_req_both(both);
            data.set_req_source(source.into());
            for row in &found.rows {
                rows.push(Req {
                    label: row.label.clone().into(),
                    minimum: row.minimum.clone().unwrap_or_default().into(),
                    recommended: row.recommended.clone().unwrap_or_default().into(),
                });
            }
        }
        self.cards.requirements_model.set_vec(rows);
    }

    /// The cards that could not be asked, with why.
    fn show_failures(&self, id: &str) {
        let mut failures = self.compat_failures(id);
        if let Some(Ask::Failed(error)) = self.cards.requirements.borrow().get(id) {
            failures.push(fact(
                t!("requirements-title"),
                t!("card-failed", error = error.as_str()),
            ));
        }
        if let Some(Ask::Failed(error)) = self.cards.achievements.borrow().get(id) {
            failures.push(fact(
                t!("achievements-title"),
                t!("card-failed", error = error.as_str()),
            ));
        }
        self.cards.failures.set_vec(failures);
    }

    /// The Technical card's row for the game's Steam app id: the one in use
    /// and where it came from. Not for a Steam game, which has its own.
    fn show_steam_row(&self, game: &Game, data: &GameData<'_>) {
        data.set_steam_app_row(!game.id.starts_with("steam:"));
        let found = match self.cards.ratings.borrow().get(&game.id) {
            Some(Ask::Got(Some(rating))) => Some(Some(rating.app)),
            Some(Ask::Got(None)) => Some(None),
            _ => None,
        };
        let line = match (game.steam_app, found) {
            (Some(app), _) => t!("steam-app-by-you", app = app.to_string()),
            (None, Some(Some(app))) => t!("steam-app-found", app = app.to_string()),
            (None, Some(None)) => t!("steam-app-none"),
            (None, None) => t!("fact-asking"),
        };
        data.set_steam_app_line(line.into());
        data.set_steam_app_clearable(game.steam_app.is_some());
    }

    /// Rows for the details card from Steam's store: the developer,
    /// publisher, release date and genres, then the features and the
    /// Metacritic score, which only Steam gives. (What a game's own store
    /// already said comes first, in `store_ui`'s rows.)
    pub(super) fn steam_fact_rows(&self, game: &Game, owned: Option<&Owned>) -> Vec<Fact> {
        if let Some(Ask::Failed(error)) = self.cards.facts.borrow().get(&game.id) {
            return vec![fact(
                t!("fact-store-details"),
                t!("fact-store-failed", error = error.as_str()),
            )];
        }
        let Some(facts) = self.facts_of(&game.id) else {
            return Vec::new();
        };
        let details = &facts.details;
        // What the game's own store already said is not said twice.
        let said = |own: Option<&Option<String>>| own.is_some_and(Option::is_some);
        let mut rows = Vec::new();
        if let Some(developer) = &details.developer
            && game.details.developer.is_none()
            && !said(owned.map(|owned| &owned.developer))
        {
            rows.push(fact(t!("fact-developer"), developer.clone()));
        }
        if let Some(publisher) = &details.publisher
            && !said(owned.map(|owned| &owned.publisher))
        {
            rows.push(fact(t!("fact-publisher"), publisher.clone()));
        }
        if let Some(day) = &details.released
            && !said(owned.map(|owned| &owned.released))
        {
            rows.push(fact(t!("fact-released"), day.clone()));
        }
        if !details.genres.is_empty() && owned.is_none_or(|owned| owned.genres.is_empty()) {
            rows.push(fact(t!("fact-genres"), details.genres.join(", ")));
        }
        let features = features_line(details);
        if !features.is_empty() {
            rows.push(fact(t!("fact-features"), features));
        }
        if let Some(found) = &details.metacritic {
            rows.push(Fact {
                url: found.url.clone().into(),
                ..fact(t!("fact-metacritic"), format!("{} / 100", found.score))
            });
        }
        rows
    }
}

/// One achievement: its icon, its name and description, and when it was
/// unlocked or how rare it is.
fn achievement_row(achievement: &Achievement, icon: Option<slint::Image>, at: u64) -> Ach {
    let kept_back = achievement.hidden && achievement.unlocked.is_none();
    let (name, description) = if kept_back {
        (t!("achievements-hidden"), t!("achievements-hidden-about"))
    } else {
        (achievement.name.clone(), achievement.description.clone())
    };
    let side = match (achievement.unlocked, achievement.rarity) {
        (Some(when), _) if when > 0 => {
            t!("achievements-unlocked-when", when = format::ago(when, at))
        }
        (Some(_), _) => t!("achievements-unlocked-plain"),
        (None, Some(rarity)) => t!("achievements-rarity", percent = rarity_text(rarity)),
        (None, None) => String::new(),
    };
    Ach {
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
        name: name.into(),
        description: description.into(),
        side: side.into(),
        unlocked: achievement.unlocked.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::owned::Metacritic;

    use super::*;

    fn details() -> StoreDetails {
        StoreDetails {
            features: vec!["Single-player".into(), "Co-op".into()],
            controller: Some("full".into()),
            metacritic: Some(Metacritic {
                score: 95,
                url: "https://www.metacritic.com/game/pc/portal-2".into(),
            }),
            ..StoreDetails::default()
        }
    }

    #[test]
    fn names_the_controller_once_and_counts_the_rest() {
        let mut details = details();
        assert_eq!(
            features_line(&details),
            format!("Single-player, Co-op, {}", t!("fact-controller-full"))
        );
        details.features.push(t!("fact-controller-full"));
        assert_eq!(features_line(&details).matches("ontroller").count(), 1);
        details.controller = None;
        details.features = (1..=9).map(|n| format!("F{n}")).collect();
        let line = features_line(&details);
        assert!(line.starts_with("F1, F2, F3, F4, F5, F6"));
        assert!(!line.contains("F7"));
        assert!(line.ends_with(&t!("fact-features-more", count = "3".to_owned())));
    }

    #[test]
    fn a_pill_says_native_or_the_tier() {
        let rating = Rating {
            app: 435_790,
            tier: Some("gold".into()),
            reports: 37,
            native: false,
        };
        let pill = pill_of(&rating);
        assert_eq!(pill.label, "PROTONDB GOLD");
        assert_eq!(pill.tone, 0);
        assert_eq!(reports(&rating), "Proton: Gold from 37 reports");
        let native = Rating {
            native: true,
            ..rating
        };
        assert_eq!(pill_of(&native).tone, 1);
        let borked = Rating {
            app: 1,
            tier: Some("borked".into()),
            reports: 2,
            native: false,
        };
        assert_eq!(pill_of(&borked).tone, 2);
    }

    #[test]
    fn rarity_has_a_decimal_under_ten_percent() {
        assert_eq!(rarity_text(3.24), "3.2");
        assert_eq!(rarity_text(42.4), "42");
    }
}
