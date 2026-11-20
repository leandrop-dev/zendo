//! The game page's store cards: `ProtonDB`'s rating, Steam's store page (the
//! about line, the screenshots and the facts only Steam has), the system
//! requirements and the achievements. Each is asked of the network on a
//! thread, once a session, when the page opens; the page hears `game_cards`
//! when an answer is in and asks for the cards again. A failure is kept, and
//! the page offers to ask again.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use zendo_core::achievements::{self, Achievements};
use zendo_core::areweanticheatyet::{self, Entry};
use zendo_core::config::Config;
use zendo_core::gog_changelog::{self, Entry as ChangelogEntry};
use zendo_core::protondb::{self, Rating};
use zendo_core::requirements::{self, Requirements, System};
use zendo_core::steam_deck::{self, Category, Report};
use zendo_core::steam_facts::{self, Facts};
use zendo_core::{Game, State as CoreState, steam, steam_web};
use zendo_shell::cards::{self, Pill, RequirementsView, SHOWN};
use zendo_shell::compat;
use zendo_shell::gog;
use zendo_shell::running::now;
use zendo_shell::t;

use crate::library::LibraryState;

/// A question put to the network and what is known of the answer.
#[derive(Debug)]
enum Ask<T> {
    Asking,
    Got(T),
    /// What went wrong; the page offers to ask again.
    Failed(String),
}

/// What has been asked about one game, and how much of it the page shows.
#[derive(Debug, Default)]
struct Answers {
    /// `None`: no Steam app was found for the game, so `ProtonDB` cannot know it.
    rating: Option<Ask<Option<Rating>>>,
    /// `None`: Steam's store has no page for the app.
    facts: Option<Ask<Option<Facts>>>,
    /// `None`: no store lists them.
    requirements: Option<Ask<Option<Requirements>>>,
    /// `None`: the store gives none.
    achievements: Option<Ask<Option<Achievements>>>,
    /// `None`: GOG has no changelog for it.
    changelog: Option<Ask<Vec<ChangelogEntry>>>,
    /// `None`: `AreWeAntiCheatYet` does not list the game.
    anti_cheat: Option<Ask<Option<Entry>>>,
    /// `None`: Valve has not reviewed it.
    deck: Option<Ask<Option<Report>>>,
    /// How many achievements are shown beyond the first ones.
    more: usize,
    /// How many of GOG's updates are shown beyond the newest.
    changelog_more: usize,
}

/// The cards' answers, by game, for this session.
#[derive(Debug, Default)]
pub struct CardsState {
    answers: Mutex<HashMap<String, Answers>>,
}

impl CardsState {
    fn answers(&self) -> MutexGuard<'_, HashMap<String, Answers>> {
        self.answers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Changes what is known of a game, when it is being asked about.
    fn keep(&self, id: &str, change: impl FnOnce(&mut Answers)) {
        let mut all = self.answers();
        if let Some(answers) = all.get_mut(id) {
            change(answers);
        }
    }
}

/// A badge on the header: `ProtonDB`'s rating.
#[derive(Debug, Serialize)]
pub struct PillDto {
    label: String,
    /// 0 takes `tint`; 1, 2 and 3 take the theme's good, bad and dim.
    tone: i32,
    /// The tier's own colour, as CSS takes it.
    tint: Option<String>,
    tip: String,
    url: String,
}

impl From<Pill> for PillDto {
    fn from(pill: Pill) -> Self {
        Self {
            label: pill.label,
            tone: pill.tone,
            tint: pill
                .tint
                .map(|(red, green, blue)| format!("#{red:02x}{green:02x}{blue:02x}")),
            tip: pill.tip,
            url: pill.url,
        }
    }
}

/// A line of the Anti-cheat card: what the list says, and the link it opens.
#[derive(Debug, Serialize)]
pub struct AntiCheatNoteDto {
    text: String,
    /// Empty when the line has no source to open.
    url: String,
}

/// The Anti-cheat card, from `AreWeAntiCheatYet`.
#[derive(Debug, Serialize)]
pub struct AntiCheatDto {
    label: String,
    about: String,
    /// The anti-cheats it ships, as the list names them.
    makers: String,
    /// 1 good, 2 bad, 3 dim.
    tone: i32,
    url: String,
    /// Drawn before About: online play will not work here, or is not known to.
    warns: bool,
    notes: Vec<AntiCheatNoteDto>,
}

/// A line of a card, and the link its value opens.
#[derive(Debug, Serialize)]
pub struct RowDto {
    label: String,
    value: String,
    url: Option<String>,
}

/// A screenshot: the small picture on the page and the full-size one.
#[derive(Debug, Serialize)]
pub struct ShotDto {
    thumb: String,
    full: String,
}

#[derive(Debug, Serialize)]
pub struct RequirementDto {
    label: String,
    minimum: String,
    recommended: String,
}

#[derive(Debug, Serialize, Default)]
pub struct RequirementsDto {
    /// Where the answer came from, with the system it is for.
    source: String,
    /// Both levels are given, so the card draws two columns.
    both: bool,
    rows: Vec<RequirementDto>,
}

#[derive(Debug, Serialize)]
pub struct AchievementDto {
    /// Its picture in the cache; the page turns it into an asset URL.
    icon: Option<String>,
    name: String,
    description: String,
    side: String,
    unlocked: bool,
}

/// A line of GOG's changelog: 0 an update's title, 1 a heading, 2 a
/// paragraph, 3 a list item, 4 a gap between updates.
#[derive(Debug, Serialize)]
pub struct LineDto {
    kind: i32,
    text: String,
}

#[derive(Debug, Serialize, Default)]
pub struct ChangelogDto {
    lines: Vec<LineDto>,
    /// What Show earlier updates says, when there are older updates.
    more: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct AchievementsDto {
    /// "3 of 10 unlocked", or why that cannot be said.
    summary: String,
    /// The share of them unlocked, 0 to 1; `None` when it cannot be said.
    share: Option<f32>,
    rows: Vec<AchievementDto>,
    /// What Show more says, when there are more.
    more: Option<String>,
}

/// The Technical card's Steam app id row.
#[derive(Debug, Serialize)]
pub struct SteamAppDto {
    /// What is in use and where it came from; empty for a Steam game.
    line: String,
    /// What the field holds: the id the user gave, if any.
    input: String,
    /// The game has an id of its own, so it can be cleared.
    clearable: bool,
}

/// Everything the page's cards draw, at this moment.
#[derive(Debug, Serialize)]
pub struct CardsDto {
    /// The `ProtonDB` pill, while it is asked and once it is known.
    pill: Option<PillDto>,
    /// Valve's review of it on the Steam Deck, once asked.
    deck: Option<PillDto>,
    /// `AreWeAntiCheatYet`'s say on its anti-cheat.
    anti_cheat: Option<AntiCheatDto>,
    /// Steam's about line; empty when the store has no page.
    about: String,
    /// No Steam game was found for this one: the page offers its app id.
    no_match: Option<String>,
    shots: Vec<ShotDto>,
    /// What only Steam says: the developer, publisher, release date, genres,
    /// features and Metacritic score. A failure to ask is a row here too.
    steam: Vec<RowDto>,
    requirements: RequirementsDto,
    achievements: AchievementsDto,
    /// GOG's notes for each update, newest first.
    changelog: ChangelogDto,
    /// The cards that could not be asked, with why.
    failures: Vec<RowDto>,
    steam_app: SteamAppDto,
}

/// The page's cards for a game, asking anything not asked yet this session.
#[tauri::command]
pub fn cards_get(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<CardsDto, String> {
    cards_for(&app, &state, &id)
}

/// Asks again for the cards that could not be asked.
#[tauri::command]
pub fn cards_retry(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<CardsDto, String> {
    app.state::<CardsState>().keep(&id, |answers| {
        // A rating that came is kept; one that failed is asked again, and so
        // is a card that failed.
        if matches!(answers.rating, Some(Ask::Failed(_))) {
            answers.rating = None;
        }
        if matches!(answers.facts, Some(Ask::Failed(_))) {
            answers.facts = None;
        }
        if matches!(answers.requirements, Some(Ask::Failed(_))) {
            answers.requirements = None;
        }
        if matches!(answers.achievements, Some(Ask::Failed(_))) {
            answers.achievements = None;
        }
        if matches!(answers.anti_cheat, Some(Ask::Failed(_))) {
            answers.anti_cheat = None;
        }
        if matches!(answers.deck, Some(Ask::Failed(_))) {
            answers.deck = None;
        }
        if matches!(answers.changelog, Some(Ask::Failed(_))) {
            answers.changelog = None;
        }
    });
    cards_for(&app, &state, &id)
}

/// Shows more of the achievements, the next `SHOWN` of them.
#[tauri::command]
pub fn cards_more_achievements(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<CardsDto, String> {
    app.state::<CardsState>().keep(&id, |answers| {
        if let Some(Ask::Got(Some(found))) = &answers.achievements {
            answers.more = (answers.more + SHOWN).min(found.list.len());
        }
    });
    cards_for(&app, &state, &id)
}

/// Shows more of GOG's updates, the next `MORE` of them.
#[tauri::command]
pub fn cards_more_changelog(
    id: String,
    app: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<CardsDto, String> {
    app.state::<CardsState>().keep(&id, |answers| {
        if let Some(Ask::Got(entries)) = &answers.changelog
            && !entries.is_empty()
        {
            answers.changelog_more = (answers.changelog_more + gog::MORE).min(entries.len() - 1);
        }
    });
    cards_for(&app, &state, &id)
}

/// Gives the game a Steam app id, or takes it back with an empty one. What
/// was found for the old one is forgotten and asked again.
#[tauri::command]
pub fn steam_app_set(
    id: String,
    app: Option<String>,
    handle: AppHandle,
    state: State<'_, LibraryState>,
) -> Result<String, String> {
    let parsed = match app
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        None => None,
        Some(text) => match text.parse::<u32>() {
            Ok(app) if app > 0 => Some(app),
            _ => return Err(t!("steam-app-invalid")),
        },
    };
    let Some(title) = state.set_steam_app(&id, parsed) else {
        return Err(format!("no game with the id {id:?}"));
    };
    handle.state::<CardsState>().keep(&id, |answers| {
        answers.rating = None;
        answers.facts = None;
        answers.requirements = None;
        answers.anti_cheat = None;
        answers.deck = None;
    });
    let _ = handle.emit("library_changed", ());
    cards_for(&handle, &state, &id)?;
    Ok(if let Some(app) = parsed {
        t!("steam-app-saved", title = title, app = app.to_string())
    } else {
        t!("steam-app-cleared", title = title)
    })
}

/// The cards as they stand, starting the asks that have not been made.
fn cards_for(app: &AppHandle, state: &LibraryState, id: &str) -> Result<CardsDto, String> {
    let game = state
        .game(id)
        .ok_or_else(|| format!("no game with the id {id:?}"))?;
    let cards = app.state::<CardsState>();
    let mut all = cards.answers();
    let answers = all.entry(id.to_owned()).or_default();
    start_asks(app, &game, answers);
    Ok(dto(&game, answers, now()))
}

/// Starts what has not been asked yet: the rating, the achievements, and
/// Steam's store when its app id is known without asking the network.
fn start_asks(app: &AppHandle, game: &Game, answers: &mut Answers) {
    if answers.rating.is_none() {
        answers.rating = Some(Ask::Asking);
        ask_rating(app, game);
    }
    let known = steam_facts::app_for(&game.id, game.steam_app);
    if let Some(app_id) = known
        && answers.facts.is_none()
    {
        answers.facts = Some(Ask::Asking);
        ask_facts(app, &game.id, app_id);
    }
    if answers.requirements.is_none() {
        answers.requirements = Some(Ask::Asking);
        let id = game.id.clone();
        ask_requirements(app, id, known, system_of(game));
    }
    if answers.achievements.is_none() {
        want_achievements(app, game, answers);
    }
    start_compat(app, game, answers);
    if answers.changelog.is_none()
        && let Some(product) = game.id.strip_prefix("gog:")
    {
        answers.changelog = Some(Ask::Asking);
        ask_changelog(app, &game.id, product.to_owned());
    }
}

/// Asks GOG for its notes on each update, which need no token.
fn ask_changelog(app: &AppHandle, id: &str, product: String) {
    let handle = app.clone();
    let kept = id.to_owned();
    std::thread::spawn(move || {
        let result =
            guarded(move || gog_changelog::fetch(&product).map_err(|err| format!("{err:#}")));
        handle.state::<CardsState>().keep(&kept, |answers| {
            answers.changelog = Some(match result {
                Ok(entries) => Ask::Got(entries),
                Err(err) => Ask::Failed(err),
            });
        });
        finish(&handle, &kept);
    });
}

/// The achievements: a store's own, or for a Game Pass game the launcher's
/// backend, which needs an installed game.
fn want_achievements(app: &AppHandle, game: &Game, answers: &mut Answers) {
    let id = game.id.clone();
    if id.starts_with("xbox:") {
        let Some(install) = game.details.install_path.clone() else {
            return;
        };
        answers.achievements = Some(Ask::Asking);
        ask_xbox_achievements(app, id, install);
        return;
    }
    let from_store = ["gog:", "steam:", "epic:"]
        .iter()
        .any(|prefix| id.starts_with(prefix));
    if !from_store {
        return;
    }
    answers.achievements = Some(Ask::Asking);
    ask_achievements(app, id, steam_id());
}

/// Asks `ProtonDB` about the game; once it names the Steam app, Steam's
/// store is asked on the way when it was not known before.
fn ask_rating(app: &AppHandle, game: &Game) {
    let handle = app.clone();
    let (title, chosen) = (game.title.clone(), game.steam_app);
    let asked = game.id.clone();
    let kept = asked.clone();
    std::thread::spawn(move || {
        let result = guarded(move || {
            protondb::rating(&asked, &title, chosen).map_err(|err| format!("{err:#}"))
        });
        let found = match &result {
            Ok(Some(rating)) => Some(rating.app),
            _ => None,
        };
        let cards = handle.state::<CardsState>();
        cards.keep(&kept, |answers| {
            answers.rating = Some(match result {
                Ok(found) => Ask::Got(found),
                Err(err) => Ask::Failed(err),
            });
        });
        if let Some(app_id) = found
            && cards
                .answers()
                .get(&kept)
                .is_some_and(|answers| answers.facts.is_none())
        {
            cards.keep(&kept, |answers| {
                answers.facts = Some(Ask::Asking);
            });
            ask_facts(&handle, &kept, app_id);
        }
        // The rating named the Steam app: the anti-cheat list knows it by
        // that, and Valve may have reviewed the game.
        want_compat(&handle, &kept, found);
        finish(&handle, &kept);
    });
}

/// Asks `AreWeAntiCheatYet` about the game, and Valve about its Deck review
/// when the Steam app is known, for the asks not made yet. Once a session.
fn start_compat(app: &AppHandle, game: &Game, answers: &mut Answers) {
    let app_id = steam_facts::app_for(&game.id, game.steam_app);
    if answers.anti_cheat.is_none() {
        answers.anti_cheat = Some(Ask::Asking);
        ask_anti_cheat(app, &game.id, app_id);
    }
    if let Some(app_id) = app_id
        && answers.deck.is_none()
    {
        answers.deck = Some(Ask::Asking);
        ask_deck(app, &game.id, app_id);
    }
}

/// Asks `AreWeAntiCheatYet` about the game, and Valve about its Deck review
/// when the Steam app is known. Once a session; a failure says why and Try
/// again asks again.
fn want_compat(app: &AppHandle, id: &str, app_id: Option<u32>) {
    let cards = app.state::<CardsState>();
    let asked = cards.answers().get(id).map(|answers| {
        (
            matches!(answers.anti_cheat, Some(Ask::Asking | Ask::Got(_))),
            matches!(answers.deck, Some(Ask::Asking | Ask::Got(_))),
        )
    });
    let Some((anti_cheat_asked, deck_asked)) = asked else {
        return;
    };
    if !anti_cheat_asked {
        cards.keep(id, |answers| answers.anti_cheat = Some(Ask::Asking));
        ask_anti_cheat(app, id, app_id);
    }
    if let Some(app_id) = app_id
        && !deck_asked
    {
        cards.keep(id, |answers| answers.deck = Some(Ask::Asking));
        ask_deck(app, id, app_id);
    }
}

fn ask_anti_cheat(app: &AppHandle, id: &str, app_id: Option<u32>) {
    let handle = app.clone();
    let (asked, kept) = (id.to_owned(), id.to_owned());
    std::thread::spawn(move || {
        let result = guarded(move || {
            areweanticheatyet::for_game(&asked, app_id).map_err(|err| format!("{err:#}"))
        });
        handle.state::<CardsState>().keep(&kept, |answers| {
            answers.anti_cheat = Some(match result {
                Ok(found) => Ask::Got(found),
                Err(err) => Ask::Failed(err),
            });
        });
        finish(&handle, &kept);
    });
}

fn ask_deck(app: &AppHandle, id: &str, app_id: u32) {
    let handle = app.clone();
    let kept = id.to_owned();
    std::thread::spawn(move || {
        let result = guarded(move || steam_deck::report(app_id).map_err(|err| format!("{err:#}")));
        handle.state::<CardsState>().keep(&kept, |answers| {
            answers.deck = Some(match result {
                Ok(found) => Ask::Got(found),
                Err(err) => Ask::Failed(err),
            });
        });
        finish(&handle, &kept);
    });
}

fn ask_facts(app: &AppHandle, id: &str, app_id: u32) {
    let handle = app.clone();
    let kept = id.to_owned();
    std::thread::spawn(move || {
        let result =
            guarded(move || steam_facts::for_app(app_id).map_err(|err| format!("{err:#}")));
        handle.state::<CardsState>().keep(&kept, |answers| {
            answers.facts = Some(match result {
                Ok(found) => Ask::Got(found),
                Err(err) => Ask::Failed(err),
            });
        });
        finish(&handle, &kept);
    });
}

fn ask_requirements(app: &AppHandle, id: String, steam_app: Option<u32>, system: System) {
    let handle = app.clone();
    let (asked, kept) = (id.clone(), id.clone());
    std::thread::spawn(move || {
        let result = guarded(move || {
            requirements::for_game(&asked, steam_app, system).map_err(|err| format!("{err:#}"))
        });
        handle.state::<CardsState>().keep(&kept, |answers| {
            answers.requirements = Some(match result {
                Ok(found) => Ask::Got(found),
                Err(err) => Ask::Failed(err),
            });
        });
        finish(&handle, &kept);
    });
}

fn ask_achievements(app: &AppHandle, id: String, steam_id: Option<String>) {
    let handle = app.clone();
    let (asked, kept) = (id.clone(), id.clone());
    std::thread::spawn(move || {
        let result = guarded(move || {
            achievements::for_game(&asked, steam_id.as_deref()).map_err(|err| format!("{err:#}"))
        });
        arrived(&handle, &kept, result);
    });
}

/// A Game Pass game's achievements: the backend asks Xbox Live by the
/// game's title id, which is in the `MicrosoftGame.config` beside its
/// executable.
fn ask_xbox_achievements(app: &AppHandle, id: String, install: std::path::PathBuf) {
    let handle = app.clone();
    let kept = id.clone();
    std::thread::spawn(move || {
        let result = guarded(move || {
            let title = zendo_core::xodus::title_id(&install)
                .ok_or("its MicrosoftGame.config has no title id")?;
            let list = zendo_core::xodus::achievements(title).map_err(|err| format!("{err:#}"))?;
            Ok(Some(Achievements {
                list,
                unlocks_known: true,
            }))
        });
        arrived(&handle, &kept, result);
    });
}

/// Keeps the achievements, and records their progress for the game's tiles
/// and the Library's cards when the store said which ones are unlocked.
fn arrived(app: &AppHandle, id: &str, result: Result<Option<Achievements>, String>) {
    let seen = match &result {
        Ok(Some(found)) if found.unlocks_known => {
            let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
            Some((count(found.unlocked()), count(found.list.len())))
        }
        _ => None,
    };
    app.state::<CardsState>().keep(id, |answers| {
        answers.achievements = Some(match result {
            Ok(found) => Ask::Got(found),
            Err(err) => Ask::Failed(err),
        });
    });
    if let Some((unlocked, total)) = seen {
        let state = app.state::<LibraryState>();
        state.change(|browser| browser.set_achievements(id, unlocked, total));
        let _ = CoreState::update(|state| state.set_achievements(id, unlocked, total));
        let _ = app.emit("library_changed", ());
    }
    finish(app, id);
}

/// Runs a question, turning a panic into the failure it is.
fn guarded<T>(work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
        .unwrap_or_else(|payload| Err(panicked(&*payload)))
}

fn panicked(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .map_or_else(|| t!("panicked"), |text| t!("panicked-with", error = text))
}

/// Tells the page that the cards changed; it asks for them again.
fn finish(app: &AppHandle, id: &str) {
    let _ = app.emit("game_cards", id.to_owned());
}

/// The system the requirements are asked for.
fn system_of(game: &Game) -> System {
    if game
        .details
        .platform
        .as_deref()
        .is_some_and(|platform| platform.eq_ignore_ascii_case("linux"))
    {
        System::Linux
    } else {
        System::Windows
    }
}

/// The player's Steam id: the account the Steam sign-in used, else the one
/// Steam on this computer signed in last.
fn steam_id() -> Option<String> {
    let signed_in = Config::load_file()
        .ok()
        .and_then(|config| config.steam_session)
        .and_then(|text| steam_web::Session::parse(&text))
        .map(|session| session.steamid);
    signed_in.or_else(|| steam::user(&steam::roots()).map(|user| user.id))
}

/// What the page draws, from what is known now.
fn dto(game: &Game, answers: &Answers, at: u64) -> CardsDto {
    let pill = match &answers.rating {
        Some(Ask::Asking) => Some(PillDto::from(cards::asking_pill())),
        Some(Ask::Got(Some(rating))) => Some(PillDto::from(cards::rating_pill(rating))),
        _ => None,
    };
    let deck = match &answers.deck {
        Some(Ask::Got(Some(report))) => Some(PillDto::from(deck_pill(report))),
        _ => None,
    };
    let anti_cheat = anti_cheat_of(answers);
    let facts = match &answers.facts {
        Some(Ask::Got(Some(facts))) => Some(facts),
        _ => None,
    };
    let about = facts
        .and_then(|facts| facts.details.summary.clone())
        .unwrap_or_default();
    let no_match = (about.is_empty()
        && matches!(&answers.rating, Some(Ask::Got(None)))
        && !game.id.starts_with("steam:")
        && game.steam_app.is_none())
    .then(|| t!("steam-app-no-match"));
    let shots = facts.map_or_else(Vec::new, |facts| {
        facts
            .shots
            .iter()
            .map(|shot| ShotDto {
                thumb: shot.thumb.display().to_string(),
                full: shot.full.display().to_string(),
            })
            .collect()
    });
    CardsDto {
        pill,
        deck,
        anti_cheat,
        about,
        no_match,
        shots,
        steam: steam_rows(game, answers),
        requirements: requirements_of(answers),
        achievements: achievements_of(game, answers, at),
        changelog: changelog_of(answers),
        failures: failures_of(answers),
        steam_app: SteamAppDto {
            line: steam_app_line(game, answers),
            input: game.steam_app.map_or_default(|app| app.to_string()),
            clearable: game.steam_app.is_some(),
        },
    }
}

/// Valve's review as the header's pill: its verdict, the tests that did not
/// pass and the way to the store's page.
fn deck_pill(report: &Report) -> Pill {
    let (label, _) = compat::deck_words(report.category);
    let (tone, tint) = match report.category {
        Category::Verified => (1, None),
        // As the ProtonDB pill's Gold.
        Category::Playable => (0, Some((0xcf, 0xb5, 0x3b))),
        Category::Unsupported => (3, None),
    };
    Pill {
        label,
        tone,
        tint,
        tip: compat::deck_tip(report),
        url: report.url(),
    }
}

/// The Anti-cheat card, from what the list said of this game.
fn anti_cheat_of(answers: &Answers) -> Option<AntiCheatDto> {
    let Some(Ask::Got(Some(entry))) = &answers.anti_cheat else {
        return None;
    };
    let (label, about, tone) = compat::status_words(entry.status);
    let mut notes = Vec::new();
    if let Some(text) = compat::latest_line(entry) {
        let (note, _) = entry.latest.as_ref()?;
        notes.push(AntiCheatNoteDto {
            text,
            url: note.link.clone().unwrap_or_default(),
        });
    }
    notes.extend(entry.notes.iter().map(|note| AntiCheatNoteDto {
        text: note.text.clone(),
        url: note.link.clone().unwrap_or_default(),
    }));
    Some(AntiCheatDto {
        label,
        about,
        makers: entry.anti_cheats.join(", "),
        tone,
        url: entry.url(),
        warns: entry.status.warns(),
        notes,
    })
}

/// Steam's rows for the details card, or why they could not be asked.
fn steam_rows(game: &Game, answers: &Answers) -> Vec<RowDto> {
    match &answers.facts {
        Some(Ask::Got(Some(facts))) => cards::steam_rows(facts, game, None)
            .into_iter()
            .map(|row| RowDto {
                label: row.label,
                value: row.value,
                url: row.url,
            })
            .collect(),
        Some(Ask::Failed(error)) => vec![RowDto {
            label: t!("fact-store-details"),
            value: t!("fact-store-failed", error = error.as_str()),
            url: None,
        }],
        _ => Vec::new(),
    }
}

fn requirements_of(answers: &Answers) -> RequirementsDto {
    let Some(Ask::Got(Some(found))) = &answers.requirements else {
        return RequirementsDto::default();
    };
    let RequirementsView { source, both, rows } = cards::requirements_view(found);
    RequirementsDto {
        source,
        both,
        rows: rows
            .into_iter()
            .map(|row| RequirementDto {
                label: row.label,
                minimum: row.minimum,
                recommended: row.recommended,
            })
            .collect(),
    }
}

fn achievements_of(game: &Game, answers: &Answers, at: u64) -> AchievementsDto {
    let Some(Ask::Got(Some(found))) = &answers.achievements else {
        return AchievementsDto::default();
    };
    let (summary, share) = cards::achievements_summary(found, &game.id);
    let shown = (SHOWN + answers.more).min(found.list.len());
    AchievementsDto {
        summary,
        share,
        rows: cards::achievement_rows(found, shown, at)
            .into_iter()
            .map(|row| AchievementDto {
                icon: row.icon.map(|path| path.display().to_string()),
                name: row.name,
                description: row.description,
                side: row.side,
                unlocked: row.unlocked,
            })
            .collect(),
        more: cards::more_label(found, shown),
    }
}

/// GOG's changelog as the card draws it: the newest update, and `MORE` more
/// each time Show earlier updates is pressed.
fn changelog_of(answers: &Answers) -> ChangelogDto {
    let Some(Ask::Got(entries)) = &answers.changelog else {
        return ChangelogDto::default();
    };
    let shown = (1 + answers.changelog_more).min(entries.len());
    ChangelogDto {
        lines: gog::lines(entries, shown)
            .into_iter()
            .map(|line| LineDto {
                kind: line.kind,
                text: line.text,
            })
            .collect(),
        more: gog::more_label(entries, shown),
    }
}

/// The cards that could not be asked, with why.
fn failures_of(answers: &Answers) -> Vec<RowDto> {
    let mut failures = Vec::new();
    if let Some(Ask::Failed(error)) = &answers.requirements {
        failures.push(RowDto {
            label: t!("requirements-title"),
            value: t!("card-failed", error = error.as_str()),
            url: None,
        });
    }
    if let Some(Ask::Failed(error)) = &answers.achievements {
        failures.push(RowDto {
            label: t!("achievements-title"),
            value: t!("card-failed", error = error.as_str()),
            url: None,
        });
    }
    if let Some(Ask::Failed(error)) = &answers.anti_cheat {
        failures.push(RowDto {
            label: t!("anti-cheat-title"),
            value: t!("card-failed", error = error.as_str()),
            url: None,
        });
    }
    if let Some(Ask::Failed(error)) = &answers.deck {
        failures.push(RowDto {
            label: t!("deck-title"),
            value: t!("card-failed", error = error.as_str()),
            url: None,
        });
    }
    failures
}

/// The Steam app id in use and where it came from.
fn steam_app_line(game: &Game, answers: &Answers) -> String {
    if game.id.starts_with("steam:") {
        return String::new();
    }
    let found = match &answers.rating {
        Some(Ask::Got(Some(rating))) => Some(Some(rating.app)),
        Some(Ask::Got(None)) => Some(None),
        _ => None,
    };
    match (game.steam_app, found) {
        (Some(app), _) => t!("steam-app-by-you", app = app.to_string()),
        (None, Some(Some(app))) => t!("steam-app-found", app = app.to_string()),
        (None, Some(None)) => t!("steam-app-none"),
        (None, None) => t!("fact-asking"),
    }
}
