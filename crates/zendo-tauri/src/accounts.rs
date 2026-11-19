//! The Accounts page: Zendo's own sign-ins to Epic, GOG and Amazon, Steam's
//! card, and Nexus Launcher's. A sign-in opens the store's page in a window of
//! Zendo's own, which hands back the code when it ends; when that window cannot
//! open, or the user prefers, the page opens in their browser and they paste
//! back the code. The window is `zendo_shell::sign_in_window`'s, run as a
//! subprocess of this one. What a sign-in or sign-out means for the Store's
//! lists is slice 5's.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use zendo_core::owned;
use zendo_core::secrets::Key;
use zendo_core::steam_web::{self, Session};
use zendo_core::stores::{self, AmazonLogin, Store};
use zendo_core::xodus;
use zendo_shell::sign_in_window::{self, Handle};
use zendo_shell::t;

use crate::settings::{self, Setting, SettingsState, kept_text};

/// What Zendo knows of one store's sign-in.
#[derive(Debug, Clone, Default)]
enum Account {
    #[default]
    Checking,
    SignedOut,
    SignedIn(String),
    Failed(String),
}

/// A sign-in under way.
struct SignIn {
    store: Store,
    pasted: String,
    /// What nile made for this sign-in; `None` for the other stores.
    amazon: Option<AmazonLogin>,
    busy: bool,
    note: Option<String>,
    /// The sign-in window, while it is open.
    window: Option<Handle>,
    /// In the user's browser, finished by pasting.
    browser: bool,
}

/// Steam's sign-in, and what the last one said.
#[derive(Default)]
struct Steam {
    signing_in: Option<Handle>,
    note: Option<(String, bool)>,
    family: Option<String>,
    family_note: Option<String>,
}

/// Where Nexus Launcher stands.
#[derive(Default)]
struct Xbox {
    installed: bool,
    games: usize,
    path: Option<PathBuf>,
    note: Option<(String, bool)>,
}

pub struct AccountsState {
    accounts: Mutex<[Account; 3]>,
    engines: Mutex<[Option<PathBuf>; 3]>,
    signing_in: Mutex<Option<SignIn>>,
    steam: Mutex<Steam>,
    xbox: Mutex<Xbox>,
    /// How many stores are being asked who is signed in.
    asking: Mutex<u32>,
    ready: AtomicBool,
}

impl Default for AccountsState {
    fn default() -> Self {
        Self {
            accounts: Mutex::default(),
            engines: Mutex::default(),
            signing_in: Mutex::default(),
            steam: Mutex::default(),
            xbox: Mutex::default(),
            asking: Mutex::default(),
            ready: AtomicBool::new(false),
        }
    }
}

fn index(store: Store) -> usize {
    match store {
        Store::Epic => 0,
        Store::Gog => 1,
        Store::Amazon => 2,
    }
}

// What the page draws.

#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one thing the card draws, as the Slint struct has them"
)]
pub struct AccountCardDto {
    badge: String,
    title: String,
    status: String,
    status_tone: i32,
    engine: String,
    engine_ok: bool,
    action: String,
    /// 0 Sign in, 1 Sign out.
    action_kind: i32,
    action_on: bool,
    gog: bool,
    /// 0 none, 1 in Zendo's window, 2 in the browser.
    flow: i32,
    line: String,
    note: String,
    retry: bool,
    browser_on: bool,
    step_one: String,
    step_two: String,
    step_three: String,
    open_label: String,
    open_on: bool,
    finish_label: String,
    finish_on: bool,
}

#[derive(Debug, Serialize)]
pub struct SteamCardDto {
    /// 0 signed out, 1 signing in, 2 signed in.
    state: i32,
    status: String,
    about: String,
    family: String,
    note: String,
    note_good: bool,
    family_note: String,
    kept: String,
    kept_bad: bool,
}

#[derive(Debug, Serialize)]
pub struct LauncherCardDto {
    badge: String,
    title: String,
    about: String,
    status: String,
    status_tone: i32,
    path: String,
    line: String,
    action: String,
    action_on: bool,
    note: String,
    note_tone: i32,
}

#[derive(Debug, Serialize)]
pub struct AccountsDto {
    steam: SteamCardDto,
    cards: Vec<AccountCardDto>,
    xbox: LauncherCardDto,
    gog_playtime: bool,
    gog_galaxy: bool,
    code: String,
}

/// The page as it stands.
#[tauri::command]
pub fn accounts_get(app: AppHandle) -> AccountsDto {
    ready(&app);
    dto(&app)
}

/// Asks each store who is signed in, once as the page first opens.
pub(crate) fn ready(app: &AppHandle) {
    let state = app.state::<AccountsState>();
    if state.ready.swap(true, Ordering::Relaxed) {
        return;
    }
    check_accounts(app);
}

/// Whether the store is known to be signed in (the Store lists its games).
pub(crate) fn signed_in(app: &AppHandle, store: Store) -> bool {
    let state = app.state::<AccountsState>();
    matches!(
        state
            .accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)[index(store)],
        Account::SignedIn(_)
    )
}

/// Whether the store is known to be signed out (its list waits for a sign-in).
pub(crate) fn signed_out(app: &AppHandle, store: Store) -> bool {
    let state = app.state::<AccountsState>();
    matches!(
        state
            .accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)[index(store)],
        Account::SignedOut
    )
}

/// Zendo's Steam sign-in, from the settings.
pub(crate) fn steam_session(app: &AppHandle) -> Option<Session> {
    app.state::<SettingsState>()
        .config()
        .steam_session
        .as_deref()
        .and_then(Session::parse)
}

fn dto(app: &AppHandle) -> AccountsDto {
    let state = app.state::<AccountsState>();
    let settings = app.state::<SettingsState>();
    let config = settings.config();
    let accounts = state
        .accounts
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let engines = state.engines.lock().unwrap_or_else(PoisonError::into_inner);
    let signing_in = state
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let cards = Store::ALL
        .into_iter()
        .map(|store| {
            card_of(
                store,
                &accounts[index(store)],
                engines[index(store)].as_deref(),
                signing_in.as_ref().filter(|sign_in| sign_in.store == store),
            )
        })
        .collect();
    let code = signing_in
        .as_ref()
        .map_or_else(String::new, |sign_in| sign_in.pasted.clone());
    AccountsDto {
        steam: steam_card(app, &config),
        cards,
        xbox: xbox_card(app),
        gog_playtime: config.gog_playtime,
        gog_galaxy: config.gog_galaxy,
        code,
    }
}

fn card_of(
    store: Store,
    account: &Account,
    engine: Option<&std::path::Path>,
    sign_in: Option<&SignIn>,
) -> AccountCardDto {
    let (status, status_tone) = match account {
        Account::Checking => (t!("key-checking"), 0),
        Account::SignedOut => (t!("accounts-signed-out"), 0),
        Account::SignedIn(name) => (t!("accounts-signed-in-as", name = name.as_str()), 1),
        Account::Failed(err) => (err.clone(), 2),
    };
    let (action, action_kind, action_on) = match account {
        Account::SignedIn(_) => (t!("accounts-sign-out"), 1, true),
        Account::Checking => (t!("accounts-sign-in"), 0, false),
        Account::SignedOut | Account::Failed(_) => (
            t!("accounts-sign-in"),
            0,
            engine.is_some() && sign_in.is_none(),
        ),
    };
    let (engine_line, engine_ok) = match engine {
        Some(path) => (
            t!("accounts-through", path = path.display().to_string()),
            true,
        ),
        None if matches!(account, Account::Checking) => (String::new(), true),
        None => (
            t!("accounts-engine-missing", engine = store.engine_name()),
            false,
        ),
    };
    let mut card = AccountCardDto {
        badge: store.source().label().to_uppercase(),
        title: store.label().to_uppercase(),
        status,
        status_tone,
        engine: engine_line,
        engine_ok,
        action,
        action_kind,
        action_on,
        gog: store == Store::Gog && matches!(account, Account::SignedIn(_)),
        flow: 0,
        line: String::new(),
        note: String::new(),
        retry: false,
        browser_on: false,
        step_one: String::new(),
        step_two: String::new(),
        step_three: String::new(),
        open_label: String::new(),
        open_on: false,
        finish_label: String::new(),
        finish_on: false,
    };
    let Some(sign_in) = sign_in else {
        return card;
    };
    card.note = sign_in.note.clone().unwrap_or_default();
    if sign_in.browser {
        let ready = store != Store::Amazon || sign_in.amazon.is_some();
        card.flow = 2;
        card.step_one = t!("accounts-step-one");
        card.step_two = match store {
            Store::Epic => t!("accounts-step-two-epic"),
            Store::Gog => t!("accounts-step-two-gog"),
            Store::Amazon => t!("accounts-step-two-amazon"),
        };
        card.step_three = t!("accounts-step-three");
        card.open_label = if ready {
            t!("accounts-open-again")
        } else {
            t!("accounts-preparing")
        };
        card.open_on = ready;
        card.finish_label = if sign_in.busy && ready {
            t!("accounts-signing-in")
        } else {
            t!("accounts-finish")
        };
        card.finish_on = !sign_in.busy && ready && !sign_in.pasted.trim().is_empty();
    } else {
        card.flow = 1;
        let (line, retry) = match (&sign_in.window, sign_in.busy, &sign_in.note) {
            (_, true, _) if sign_in.amazon.is_none() && store == Store::Amazon => {
                (t!("accounts-preparing-amazon"), false)
            }
            (_, true, _) => (t!("accounts-signing-in"), false),
            (Some(_), false, _) => (t!("accounts-in-window", store = store.label()), false),
            (None, false, Some(_)) => (String::new(), true),
            (None, false, None) => (t!("accounts-opening-window"), false),
        };
        card.line = line;
        card.retry = retry;
        card.browser_on = !sign_in.busy;
    }
    card
}

fn steam_card(app: &AppHandle, config: &zendo_core::config::Config) -> SteamCardDto {
    let state = app.state::<AccountsState>();
    let steam = state.steam.lock().unwrap_or_else(PoisonError::into_inner);
    let session = config.steam_session.as_deref().and_then(Session::parse);
    let signing_in = steam.signing_in.is_some();
    let (state_number, status, about) = match (&session, signing_in) {
        (Some(session), _) => (
            2,
            t!("accounts-signed-in-as", name = steam_name(&session.steamid)),
            t!("steam-signed-in-about"),
        ),
        (None, true) => (1, t!("steam-signing-in"), t!("steam-sign-in-about")),
        (None, false) => (0, String::new(), t!("steam-sign-in-about")),
    };
    let (note, note_good) = steam.note.clone().unwrap_or_default();
    let (kept, kept_bad) = kept_text(config.kept(Key::SteamSession));
    SteamCardDto {
        state: state_number,
        status,
        about,
        family: steam.family.clone().unwrap_or_default(),
        note,
        note_good,
        family_note: steam.family_note.clone().map_or_else(String::new, |error| {
            t!("steam-family-failed", error = error.as_str())
        }),
        kept,
        kept_bad,
    }
}

/// The signed-in account's name: Steam's own when this computer knows it.
fn steam_name(steamid: &str) -> String {
    owned::steam_user_named(steamid).unwrap_or_else(|| steamid.to_owned())
}

fn xbox_card(app: &AppHandle) -> LauncherCardDto {
    let state = app.state::<AccountsState>();
    let card = state.xbox.lock().unwrap_or_else(PoisonError::into_inner);
    let status = if card.installed {
        if card.games > 0 {
            t!("count-games", count = card.games)
        } else {
            t!("launcher-installed")
        }
    } else {
        t!("launcher-not-installed")
    };
    let (note, note_tone) = card.note.clone().map_or_else(
        || (String::new(), 0),
        |(note, good)| (note, if good { 1 } else { 2 }),
    );
    LauncherCardDto {
        badge: "XBOX".to_owned(),
        title: t!("xbox-title"),
        about: t!("xbox-about"),
        status,
        status_tone: i32::from(card.installed),
        path: card
            .path
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string()),
        line: t!("xbox-sign-in"),
        action: t!("xbox-open"),
        action_on: card.installed,
        note,
        note_tone,
    }
}

// The commands.

#[tauri::command]
pub fn accounts_check(app: AppHandle) -> AccountsDto {
    check_accounts(&app);
    dto(&app)
}

fn check_accounts(app: &AppHandle) {
    let state = app.state::<AccountsState>();
    {
        let mut asking = state.asking.lock().unwrap_or_else(PoisonError::into_inner);
        if *asking > 0 {
            return;
        }
        *asking = u32::try_from(Store::ALL.len()).unwrap_or(0);
    }
    for store in Store::ALL {
        let handle = app.clone();
        std::thread::spawn(move || {
            let account = store.account().map_err(|err| format!("{err:#}"));
            let engine = store.engine();
            let state = handle.state::<AccountsState>();
            {
                let mut asking = state.asking.lock().unwrap_or_else(PoisonError::into_inner);
                *asking = asking.saturating_sub(1);
            }
            state.engines.lock().unwrap_or_else(PoisonError::into_inner)[index(store)] = engine;
            state
                .accounts
                .lock()
                .unwrap_or_else(PoisonError::into_inner)[index(store)] = match account {
                Ok(Some(name)) => Account::SignedIn(name),
                Ok(None) => Account::SignedOut,
                Err(err) => Account::Failed(err),
            };
            let _ = handle.emit("accounts_changed", ());
        });
    }
    let appimage = xodus::installed();
    {
        let mut xbox = state.xbox.lock().unwrap_or_else(PoisonError::into_inner);
        xbox.installed = appimage.is_some();
        xbox.path = appimage;
        xbox.games = xodus::count();
    }
    let _ = app.emit("accounts_changed", ());
}

#[tauri::command]
pub fn accounts_start(index: usize, app: AppHandle) -> AccountsDto {
    if let Some(store) = Store::ALL.get(index).copied() {
        start_sign_in(&app, store);
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_sign_out(index: usize, app: AppHandle) -> AccountsDto {
    if let Some(store) = Store::ALL.get(index).copied() {
        sign_out(&app, store);
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_cancel(app: AppHandle) -> AccountsDto {
    if let Some(window) = app
        .state::<AccountsState>()
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .and_then(|sign_in| sign_in.window)
    {
        window.close();
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_use_browser(app: AppHandle) -> AccountsDto {
    if let Some(sign_in) = app
        .state::<AccountsState>()
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        sign_in.browser = true;
        sign_in.note = None;
        if let Some(window) = sign_in.window.take() {
            window.close();
        }
    }
    open_page(&app);
    dto(&app)
}

#[tauri::command]
pub fn accounts_code_edited(text: String, app: AppHandle) -> AccountsDto {
    if let Some(sign_in) = app
        .state::<AccountsState>()
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        sign_in.pasted = text;
        sign_in.note = None;
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_finish(app: AppHandle) -> AccountsDto {
    finish_sign_in(&app);
    dto(&app)
}

#[tauri::command]
pub fn accounts_steam_sign_in(app: AppHandle) -> AccountsDto {
    steam_sign_in(&app);
    dto(&app)
}

#[tauri::command]
pub fn accounts_steam_cancel(app: AppHandle) -> AccountsDto {
    let state = app.state::<AccountsState>();
    if let Some(handle) = state
        .steam
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .signing_in
        .take()
    {
        handle.close();
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_steam_sign_out(app: AppHandle) -> AccountsDto {
    let state = app.state::<AccountsState>();
    {
        let mut steam = state.steam.lock().unwrap_or_else(PoisonError::into_inner);
        steam.family = None;
        steam.family_note = None;
    }
    settings::save(&app, Setting::SteamSession(None));
    match steam_web::forget_profile() {
        Ok(()) => {
            state
                .steam
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .note = Some((t!("steam-signed-out"), true));
        }
        Err(err) => {
            state
                .steam
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .note = Some((format!("{err:#}"), false));
        }
    }
    dto(&app)
}

#[tauri::command]
pub fn accounts_xbox_open(app: AppHandle) -> AccountsDto {
    let result = xodus::open().map(|mut child| {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    });
    let note = match result {
        Ok(()) => (t!("xbox-opened"), true),
        Err(err) => (format!("{err:#}"), false),
    };
    app.state::<AccountsState>()
        .xbox
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .note = Some(note);
    dto(&app)
}

#[tauri::command]
pub fn accounts_set_gog_playtime(on: bool, app: AppHandle) -> AccountsDto {
    settings::save(&app, Setting::GogPlaytime(on));
    dto(&app)
}

#[tauri::command]
pub fn accounts_set_gog_galaxy(on: bool, app: AppHandle) -> AccountsDto {
    settings::save(&app, Setting::GogGalaxy(on));
    dto(&app)
}

// The flows.

fn start_sign_in(app: &AppHandle, store: Store) {
    let state = app.state::<AccountsState>();
    if let Some(window) = state
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .and_then(|previous| previous.window)
    {
        window.close();
    }
    *state
        .signing_in
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(SignIn {
        store,
        pasted: String::new(),
        amazon: None,
        busy: store == Store::Amazon,
        note: None,
        window: None,
        browser: false,
    });
    let _ = app.emit("accounts_changed", ());
    if let Some(page) = store.login_page() {
        open_window(app, store, page.to_owned());
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = Store::start_amazon().map_err(|err| format!("{err:#}"));
        amazon_ready(&handle, result);
    });
}

fn open_window(app: &AppHandle, store: Store, url: String) {
    let handle = Handle::default();
    {
        let state = app.state::<AccountsState>();
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(sign_in) = signing_in.as_mut().filter(|sign_in| sign_in.store == store) else {
            return;
        };
        sign_in.window = Some(handle.clone());
    }
    let _ = app.emit("accounts_changed", ());
    let app = app.clone();
    std::thread::spawn(move || {
        let result = sign_in_window::open(store, &url, &handle).map_err(|err| format!("{err:#}"));
        window_ended(&app, store, result);
    });
}

fn amazon_ready(app: &AppHandle, result: Result<AmazonLogin, String>) {
    let next = {
        let state = app.state::<AccountsState>();
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        signing_in
            .as_mut()
            .filter(|sign_in| sign_in.store == Store::Amazon)
            .and_then(|sign_in| {
                sign_in.busy = false;
                match result {
                    Ok(login) => {
                        let url = login.url.clone();
                        sign_in.amazon = Some(login);
                        if sign_in.browser { Some(url) } else { None }
                    }
                    Err(err) => {
                        sign_in.note = Some(err);
                        None
                    }
                }
            })
    };
    if let Some(url) = next {
        let _ = zendo_shell::open::target(&url);
    }
    let _ = app.emit("accounts_changed", ());
}

fn window_ended(app: &AppHandle, store: Store, result: Result<Option<String>, String>) {
    let live = {
        let state = app.state::<AccountsState>();
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        signing_in
            .as_mut()
            .filter(|sign_in| sign_in.store == store)
            .map(|sign_in| {
                let had_window = sign_in.window.take().is_some();
                had_window && !sign_in.browser
            })
    };
    if live != Some(true) {
        let _ = app.emit("accounts_changed", ());
        return;
    }
    match result {
        Ok(Some(code)) => sign_in_with(app, code),
        Ok(None) => {
            app.state::<AccountsState>()
                .signing_in
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            let _ = app.emit("accounts_changed", ());
        }
        Err(err) => {
            {
                let state = app.state::<AccountsState>();
                let mut signing_in = state
                    .signing_in
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if let Some(sign_in) = signing_in.as_mut().filter(|sign_in| sign_in.store == store)
                {
                    sign_in.note = Some(t!("accounts-no-web-view", error = err));
                    sign_in.browser = true;
                }
            }
            open_page(app);
        }
    }
}

fn open_page(app: &AppHandle) {
    let page = {
        let state = app.state::<AccountsState>();
        let signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        signing_in.as_ref().and_then(|sign_in| {
            sign_in
                .store
                .login_page()
                .map(str::to_owned)
                .or_else(|| sign_in.amazon.as_ref().map(|login| login.url.clone()))
        })
    };
    if let Some(page) = page
        && let Err(err) = zendo_shell::open::target(&page)
    {
        let _ = err;
    }
}

fn finish_sign_in(app: &AppHandle) {
    let code = {
        let state = app.state::<AccountsState>();
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(sign_in) = signing_in.as_mut() else {
            return;
        };
        if sign_in.busy {
            return;
        }
        let Some(code) = stores::code_from(sign_in.store, &sign_in.pasted) else {
            sign_in.note = Some(match sign_in.store {
                Store::Epic => t!("accounts-no-code-epic"),
                Store::Gog | Store::Amazon => t!("accounts-no-code-address"),
            });
            drop(signing_in);
            let _ = app.emit("accounts_changed", ());
            return;
        };
        sign_in.busy = true;
        code
    };
    let _ = app.emit("accounts_changed", ());
    sign_in_with(app, code);
}

fn sign_in_with(app: &AppHandle, code: String) {
    let (store, amazon) = {
        let state = app.state::<AccountsState>();
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(sign_in) = signing_in.as_mut() else {
            return;
        };
        sign_in.busy = true;
        sign_in.note = None;
        (sign_in.store, sign_in.amazon.clone())
    };
    let _ = app.emit("accounts_changed", ());
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = store
            .sign_in(&code, amazon.as_ref())
            .map_err(|err| format!("{err:#}"));
        sign_in_finished(&handle, store, result);
    });
}

fn sign_in_finished(app: &AppHandle, store: Store, result: Result<String, String>) {
    let state = app.state::<AccountsState>();
    let still = {
        let mut signing_in = state
            .signing_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        signing_in
            .as_mut()
            .filter(|sign_in| sign_in.store == store)
            .map(|sign_in| sign_in.busy = false)
    };
    if still.is_none() {
        if let Ok(name) = result {
            state
                .accounts
                .lock()
                .unwrap_or_else(PoisonError::into_inner)[index(store)] = Account::SignedIn(name);
        }
        let _ = app.emit("accounts_changed", ());
        return;
    }
    match result {
        Ok(name) => {
            state
                .signing_in
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            state
                .accounts
                .lock()
                .unwrap_or_else(PoisonError::into_inner)[index(store)] = Account::SignedIn(name);
        }
        Err(err) => {
            let mut signing_in = state
                .signing_in
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(sign_in) = signing_in.as_mut().filter(|sign_in| sign_in.store == store) {
                sign_in.pasted.clear();
                sign_in.note = Some(err);
            }
        }
    }
    let _ = app.emit("accounts_changed", ());
}

fn sign_out(app: &AppHandle, store: Store) {
    let state = app.state::<AccountsState>();
    state
        .accounts
        .lock()
        .unwrap_or_else(PoisonError::into_inner)[index(store)] = Account::Checking;
    let _ = app.emit("accounts_changed", ());
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = store.sign_out().map_err(|err| format!("{err:#}"));
        let state = handle.state::<AccountsState>();
        state
            .accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)[index(store)] = match result {
            Ok(()) => Account::SignedOut,
            Err(err) => Account::Failed(err),
        };
        let _ = handle.emit("accounts_changed", ());
    });
}

fn steam_sign_in(app: &AppHandle) {
    let state = app.state::<AccountsState>();
    let handle = {
        let mut steam = state.steam.lock().unwrap_or_else(PoisonError::into_inner);
        if steam.signing_in.is_some() {
            return;
        }
        let handle = Handle::default();
        steam.signing_in = Some(handle.clone());
        steam.note = None;
        handle
    };
    let _ = app.emit("accounts_changed", ());
    let app = app.clone();
    std::thread::spawn(move || {
        let result = sign_in_window::open_steam(&handle).map_err(|err| format!("{err:#}"));
        steam_signed_in(&app, result);
    });
}

fn steam_signed_in(app: &AppHandle, result: Result<Option<Session>, String>) {
    let state = app.state::<AccountsState>();
    {
        let mut steam = state.steam.lock().unwrap_or_else(PoisonError::into_inner);
        if steam.signing_in.take().is_none() {
            return;
        }
        match &result {
            Ok(Some(_)) => steam.note = Some((t!("steam-signed-in"), true)),
            Ok(None) => {}
            Err(err) => steam.note = Some((err.clone(), false)),
        }
    }
    if let Ok(Some(session)) = result {
        settings::save(app, Setting::SteamSession(Some(session.to_text())));
    }
    let _ = app.emit("accounts_changed", ());
}
