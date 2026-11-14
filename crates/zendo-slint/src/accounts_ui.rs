//! The Accounts page: Zendo's own sign-ins to Epic, GOG and Amazon. Sign in
//! opens the store's page in a window of Zendo's own, which hands back the
//! code when the store's sign-in ends; nothing to copy. When that window
//! cannot open, or the user prefers, the page opens in their browser and
//! they paste back the code or address it ends on. A part of the Library's
//! glue (`library_ui`), which owns the settings.
//!
//! Left for item 51, which has the Store's lists: asking a store for its
//! games once it is signed in, forgetting them on sign-out, and Steam's card.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::owned::{self, Shop};
use zendo_core::stores::{self, AmazonLogin, Store};
use zendo_shell::sign_in_window::{self, Handle};
use zendo_shell::t;

use super::settings::Setting;
use super::{Glue, with_glue};
use crate::{AccountCard, AccountsData, MainWindow};

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
#[derive(Debug)]
struct SignIn {
    store: Store,
    pasted: String,
    /// What nile made for this sign-in; `None` for the other stores, and for
    /// Amazon until nile answers.
    amazon: Option<AmazonLogin>,
    busy: bool,
    note: Option<String>,
    /// The sign-in window, while it is open.
    window: Option<Handle>,
    /// In the user's browser, finished by pasting.
    browser: bool,
}

pub(super) struct AccountsState {
    accounts: RefCell<[Account; 3]>,
    /// Each store's engine, found when the accounts are checked: not when
    /// the page is drawn, which would walk `PATH` each time.
    engines: RefCell<[Option<PathBuf>; 3]>,
    signing_in: RefCell<Option<SignIn>>,
    cards: Rc<VecModel<AccountCard>>,
    focus: Cell<i32>,
    paste: Cell<i32>,
}

impl AccountsState {
    pub(super) fn new() -> Self {
        Self {
            accounts: RefCell::default(),
            engines: RefCell::default(),
            signing_in: RefCell::new(None),
            cards: Rc::new(VecModel::default()),
            focus: Cell::new(0),
            paste: Cell::new(0),
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

/// One store's card, with every text it shows.
fn card_of(
    store: Store,
    account: &Account,
    engine: Option<&Path>,
    sign_in: Option<&SignIn>,
) -> AccountCard {
    let (status, status_tone) = match account {
        Account::Checking => (t!("key-checking"), 0),
        Account::SignedOut => (t!("accounts-signed-out"), 0),
        Account::SignedIn(name) => (t!("accounts-signed-in-as", name = name.as_str()), 1),
        Account::Failed(err) => (err.clone(), 2),
    };
    // The slot stays where the button will be, disabled until the engines
    // have answered.
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
        // Not yet looked for: nothing to complain about.
        None if matches!(account, Account::Checking) => (String::new(), true),
        None => (
            t!("accounts-engine-missing", engine = store.engine_name()),
            false,
        ),
    };
    let mut card = AccountCard {
        badge: store.source().label().to_uppercase().into(),
        title: store.label().to_uppercase().into(),
        status: status.into(),
        status_tone,
        engine: engine_line.into(),
        engine_ok,
        action: action.into(),
        action_kind,
        action_on,
        gog: store == Store::Gog && matches!(account, Account::SignedIn(_)),
        ..AccountCard::default()
    };
    let Some(sign_in) = sign_in else {
        return card;
    };
    card.note = sign_in.note.clone().unwrap_or_default().into();
    if sign_in.browser {
        let ready = store != Store::Amazon || sign_in.amazon.is_some();
        card.flow = 2;
        card.step_one = t!("accounts-step-one").into();
        card.step_two = match store {
            Store::Epic => t!("accounts-step-two-epic"),
            Store::Gog => t!("accounts-step-two-gog"),
            Store::Amazon => t!("accounts-step-two-amazon"),
        }
        .into();
        card.step_three = t!("accounts-step-three").into();
        card.open_label = if ready {
            t!("accounts-open-again")
        } else {
            t!("accounts-preparing")
        }
        .into();
        card.open_on = ready;
        card.finish_label = if sign_in.busy && ready {
            t!("accounts-signing-in")
        } else {
            t!("accounts-finish")
        }
        .into();
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
        card.line = line.into();
        card.retry = retry;
        card.browser_on = !sign_in.busy;
    }
    card
}

impl Glue {
    /// Hooks the page's callbacks up.
    pub(super) fn connect_accounts(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<AccountsData>();
        data.set_cards(ModelRc::from(Rc::clone(&self.accounts.cards)));
        let store_of = |index: i32| {
            usize::try_from(index)
                .ok()
                .and_then(|index| Store::ALL.get(index).copied())
        };
        let glue = Rc::clone(self);
        data.on_start(move |at| {
            if let Some(store) = store_of(at) {
                glue.start_sign_in(store);
            }
        });
        let glue = Rc::clone(self);
        data.on_sign_out(move |at| {
            if let Some(store) = store_of(at) {
                glue.sign_out(store);
            }
        });
        let glue = Rc::clone(self);
        data.on_cancel(move || glue.cancel_sign_in());
        let glue = Rc::clone(self);
        data.on_use_browser(move || glue.use_browser());
        let glue = Rc::clone(self);
        data.on_open_page(move || glue.open_page());
        let glue = Rc::clone(self);
        data.on_code_edited(move |text| {
            if let Some(sign_in) = glue.accounts.signing_in.borrow_mut().as_mut() {
                sign_in.pasted = text.into();
                sign_in.note = None;
            }
            glue.show_accounts();
        });
        let glue = Rc::clone(self);
        data.on_finish(move || glue.finish_sign_in());
        let glue = Rc::clone(self);
        data.on_set_gog_playtime(move |on| glue.save_setting(Setting::GogPlaytime(on)));
        let glue = Rc::clone(self);
        data.on_set_gog_galaxy(move |on| glue.save_setting(Setting::GogGalaxy(on)));
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_accounts());
    }

    /// Whether Accounts is the page on screen.
    fn on_accounts(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|window| window.get_page() == crate::Page::Accounts)
    }

    /// The store's sign-in, when it is the one under way.
    fn signing_in_to<R>(&self, store: Store, then: impl FnOnce(&mut SignIn) -> R) -> Option<R> {
        self.accounts
            .signing_in
            .borrow_mut()
            .as_mut()
            .filter(|sign_in| sign_in.store == store)
            .map(then)
    }

    fn set_account(&self, store: Store, account: Account) {
        self.accounts.accounts.borrow_mut()[index(store)] = account;
    }

    /// Asks each engine who is signed in, each on a thread.
    pub(super) fn check_accounts(&self) {
        if !self.persist {
            return;
        }
        for store in Store::ALL {
            let weak = self.window.clone();
            std::thread::spawn(move || {
                let account = store.account().map_err(|err| format!("{err:#}"));
                let engine = store.engine();
                let _ = weak.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.account_checked(store, account, engine));
                });
            });
        }
    }

    pub(super) fn account_checked(
        &self,
        store: Store,
        result: Result<Option<String>, String>,
        engine: Option<PathBuf>,
    ) {
        self.accounts.engines.borrow_mut()[index(store)] = engine;
        self.set_account(
            store,
            match result {
                Ok(Some(name)) => Account::SignedIn(name),
                Ok(None) => Account::SignedOut,
                Err(err) => Account::Failed(err),
            },
        );
        // 51: a store signed in whose list is not kept is asked for its games.
        self.show_accounts();
    }

    pub(super) fn start_sign_in(self: &Rc<Self>, store: Store) {
        // One sign-in at a time: another store's window closes with it, so no
        // window is left on screen that Zendo no longer tracks.
        if let Some(window) = self
            .accounts
            .signing_in
            .borrow_mut()
            .take()
            .and_then(|previous| previous.window)
        {
            window.close();
        }
        *self.accounts.signing_in.borrow_mut() = Some(SignIn {
            store,
            pasted: String::new(),
            amazon: None,
            busy: store == Store::Amazon,
            note: None,
            window: None,
            browser: false,
        });
        self.show_accounts();
        if !self.persist {
            return;
        }
        if let Some(page) = store.login_page() {
            self.open_window(store, page.to_owned());
            return;
        }
        // Amazon's page is made for each sign-in, by nile.
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = Store::start_amazon().map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.amazon_ready(result));
            });
        });
    }

    /// Opens the store's page in the sign-in window, and waits on a thread for
    /// the code.
    fn open_window(self: &Rc<Self>, store: Store, url: String) {
        let handle = Handle::default();
        let held = handle.clone();
        if self
            .signing_in_to(store, |sign_in| sign_in.window = Some(held))
            .is_none()
        {
            return;
        }
        self.show_accounts();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result =
                sign_in_window::open(store, &url, &handle).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.window_ended(store, result));
            });
        });
    }

    fn amazon_ready(self: &Rc<Self>, result: Result<AmazonLogin, String>) {
        enum Next {
            Nothing,
            Browser(String),
            Window(String),
        }
        let next = self.signing_in_to(Store::Amazon, |sign_in| {
            sign_in.busy = false;
            match result {
                Ok(login) => {
                    let url = login.url.clone();
                    sign_in.amazon = Some(login);
                    if sign_in.browser {
                        Next::Browser(url)
                    } else {
                        Next::Window(url)
                    }
                }
                Err(err) => {
                    sign_in.note = Some(err);
                    Next::Nothing
                }
            }
        });
        match next {
            Some(Next::Browser(url)) => {
                if self.persist
                    && let Err(err) = zendo_shell::open::target(&url)
                {
                    self.say(err);
                }
            }
            Some(Next::Window(url)) => self.open_window(Store::Amazon, url),
            Some(Next::Nothing) | None => {}
        }
        self.show_accounts();
    }

    /// The sign-in window ended: the code, or `None` when it was closed.
    pub(super) fn window_ended(
        self: &Rc<Self>,
        store: Store,
        result: Result<Option<String>, String>,
    ) {
        // Gone to the browser meanwhile: the window was closed for it.
        let live = self.signing_in_to(store, |sign_in| {
            let had_window = sign_in.window.take().is_some();
            had_window && !sign_in.browser
        });
        if live != Some(true) {
            self.show_accounts();
            return;
        }
        match result {
            Ok(Some(code)) => self.sign_in_with(code),
            Ok(None) => {
                *self.accounts.signing_in.borrow_mut() = None;
                self.say(t!("accounts-closed", store = store.label()));
                self.show_accounts();
            }
            // Without a web view, the browser still works.
            Err(err) => {
                self.signing_in_to(store, |sign_in| {
                    sign_in.note = Some(t!("accounts-no-web-view", error = err));
                    sign_in.browser = true;
                });
                self.open_page();
            }
        }
    }

    /// Signs in in the browser instead, closing the window.
    pub(super) fn use_browser(self: &Rc<Self>) {
        if let Some(sign_in) = self.accounts.signing_in.borrow_mut().as_mut() {
            sign_in.browser = true;
            sign_in.note = None;
            if let Some(window) = sign_in.window.take() {
                window.close();
            }
        }
        self.open_page();
    }

    /// Opens the store's sign-in page in the user's browser.
    fn open_page(&self) {
        let page = self
            .accounts
            .signing_in
            .borrow()
            .as_ref()
            .and_then(|sign_in| {
                sign_in
                    .store
                    .login_page()
                    .map(str::to_owned)
                    .or_else(|| sign_in.amazon.as_ref().map(|login| login.url.clone()))
            });
        if let Some(page) = page.filter(|_| self.persist)
            && let Err(err) = zendo_shell::open::target(&page)
        {
            self.say(err);
        }
        self.show_accounts();
    }

    fn cancel_sign_in(&self) {
        if let Some(window) = self
            .accounts
            .signing_in
            .borrow_mut()
            .take()
            .and_then(|sign_in| sign_in.window)
        {
            window.close();
        }
        self.show_accounts();
        // The code field had the keyboard, and is gone.
        self.focus_page();
    }

    fn finish_sign_in(self: &Rc<Self>) {
        let persist = self.persist;
        let code = {
            let mut guard = self.accounts.signing_in.borrow_mut();
            let Some(sign_in) = guard.as_mut() else {
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
                drop(guard);
                self.show_accounts();
                return;
            };
            if !persist {
                sign_in.busy = true;
                drop(guard);
                self.show_accounts();
                return;
            }
            code
        };
        self.sign_in_with(code);
    }

    /// Hands the store's code to its engine, on a thread.
    fn sign_in_with(self: &Rc<Self>, code: String) {
        let (store, amazon) = {
            let mut guard = self.accounts.signing_in.borrow_mut();
            let Some(sign_in) = guard.as_mut() else {
                return;
            };
            sign_in.busy = true;
            sign_in.note = None;
            (sign_in.store, sign_in.amazon.clone())
        };
        self.show_accounts();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = store
                .sign_in(&code, amazon.as_ref())
                .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.sign_in_finished(store, result));
            });
        });
    }

    /// The engine's answer to a sign-in, whether or not the page that started
    /// it still shows it.
    pub(super) fn sign_in_finished(&self, store: Store, result: Result<String, String>) {
        let still = self.signing_in_to(store, |sign_in| sign_in.busy = false);
        if still.is_none() {
            // Cancel does not stop the engine, which writes its own
            // credentials: an answer that arrives after it still says how the
            // account stands.
            if let Ok(name) = result {
                self.say(t!(
                    "accounts-signed-in-to",
                    store = store.label(),
                    name = name.as_str()
                ));
                self.set_account(store, Account::SignedIn(name));
                // 51: the store's games are asked for.
            }
            self.show_accounts();
            return;
        }
        match result {
            Ok(name) => {
                *self.accounts.signing_in.borrow_mut() = None;
                self.say(t!(
                    "accounts-signed-in-to",
                    store = store.label(),
                    name = name.as_str()
                ));
                self.set_account(store, Account::SignedIn(name));
                // 51: the store's games are asked for.
                self.focus_page();
            }
            Err(err) => {
                // A code works once; Amazon's sign-in starts over.
                self.signing_in_to(store, |sign_in| {
                    sign_in.pasted.clear();
                    sign_in.note = Some(err);
                });
            }
        }
        self.show_accounts();
    }

    fn sign_out(&self, store: Store) {
        self.set_account(store, Account::Checking);
        if !self.persist {
            self.set_account(store, Account::SignedOut);
            self.show_accounts();
            return;
        }
        self.show_accounts();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = store.sign_out().map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.signed_out(store, result));
            });
        });
    }

    pub(super) fn signed_out(&self, store: Store, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.set_account(store, Account::SignedOut);
                // 51: the Store page forgets this store's list as well.
                if self.persist {
                    owned::forget(Shop::from(store));
                }
                self.say(t!("accounts-signed-out-of", store = store.label()));
            }
            Err(err) => self.set_account(store, Account::Failed(err)),
        }
        self.show_accounts();
    }

    /// Text typed with no field focused: it starts the code, as typing in the
    /// library starts a search. Says whether it was taken.
    pub(super) fn type_code(&self, typed: &str) -> bool {
        if !self.on_accounts() {
            return false;
        }
        {
            let mut guard = self.accounts.signing_in.borrow_mut();
            match guard.as_mut() {
                Some(sign_in) if sign_in.browser && sign_in.pasted.is_empty() => {
                    sign_in.pasted.push_str(typed);
                    sign_in.note = None;
                }
                _ => return false,
            }
        }
        self.accounts.focus.set(self.accounts.focus.get() + 1);
        self.show_accounts();
        true
    }

    /// Ctrl+V on the page: the clipboard goes in the code field. Says whether
    /// there was one.
    pub(super) fn paste_code(&self) -> bool {
        let browser = self
            .accounts
            .signing_in
            .borrow()
            .as_ref()
            .is_some_and(|sign_in| sign_in.browser);
        if !self.on_accounts() || !browser {
            return false;
        }
        self.accounts.paste.set(self.accounts.paste.get() + 1);
        self.show_accounts();
        true
    }

    /// Writes everything the page shows, and the count beside its tab.
    pub(super) fn show_accounts(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<AccountsData>();
        let config = self.settings_config();
        let accounts = self.accounts.accounts.borrow();
        let engines = self.accounts.engines.borrow();
        let signing_in = self.accounts.signing_in.borrow();
        let cards: Vec<AccountCard> = Store::ALL
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
        self.accounts.cards.set_vec(cards);
        data.set_gog_playtime(config.gog_playtime);
        data.set_gog_galaxy(config.gog_galaxy);
        let pasted = signing_in
            .as_ref()
            .map(|sign_in| sign_in.pasted.clone())
            .unwrap_or_default();
        if data.get_code() != pasted {
            data.set_code(pasted.into());
        }
        data.set_focus_code(self.accounts.focus.get());
        data.set_paste_code(self.accounts.paste.get());
        // The stores signed in, and Steam once it has a session.
        let connected = accounts
            .iter()
            .filter(|account| matches!(account, Account::SignedIn(_)))
            .count()
            + usize::from(config.steam_session.is_some());
        window.set_accounts_count(
            if connected == 0 {
                String::new()
            } else {
                t!(
                    "top-bar-signed-in",
                    count = connected,
                    stores = Shop::ALL.len()
                )
            }
            .into(),
        );
    }
}

#[cfg(test)]
impl Glue {
    /// The sign-in under way, for the tests: whether it is busy, has its
    /// window, is in the browser, and its note.
    pub(super) fn sign_in_state(&self) -> Option<(bool, bool, bool, Option<String>)> {
        self.accounts.signing_in.borrow().as_ref().map(|sign_in| {
            (
                sign_in.busy,
                sign_in.window.is_some(),
                sign_in.browser,
                sign_in.note.clone(),
            )
        })
    }

    /// Gives the sign-in the window `open_window` would have opened.
    pub(super) fn give_sign_in_a_window(&self) {
        if let Some(sign_in) = self.accounts.signing_in.borrow_mut().as_mut() {
            sign_in.window = Some(Handle::default());
        }
    }

    pub(super) fn account_is_signed_in(&self, store: Store) -> bool {
        matches!(
            self.accounts.accounts.borrow()[index(store)],
            Account::SignedIn(_)
        )
    }
}
