//! The Accounts page: Zendo's own sign-ins to Epic, GOG and Amazon. Sign in
//! opens the store's page in a window of Zendo's own, which hands back the
//! code when the store's sign-in ends; nothing to copy. When that window
//! cannot open, or the user prefers, the page opens in their browser and
//! they paste back the code or address it ends on.

use iced::widget::{
    button, column, container, operation, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::stores::{self, AmazonLogin, Store};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking, code_id, open};
use crate::i18n::t;
use crate::sign_in_window::{self, Handle};

/// What Zendo knows of one store's sign-in.
#[derive(Debug, Clone, Default)]
pub enum Account {
    #[default]
    Checking,
    SignedOut,
    SignedIn(String),
    Failed(String),
}

/// A sign-in under way.
#[derive(Debug)]
pub struct SignIn {
    store: Store,
    pasted: String,
    /// What nile made for this sign-in; `None` for the other stores, and
    /// for Amazon until nile answers.
    amazon: Option<AmazonLogin>,
    busy: bool,
    note: Option<String>,
    /// The sign-in window, while it is open.
    window: Option<Handle>,
    /// In the user's browser, finished by pasting.
    browser: bool,
}

#[derive(Debug, Default)]
pub struct Accounts {
    epic: Account,
    gog: Account,
    amazon: Account,
    signing_in: Option<SignIn>,
}

impl Accounts {
    pub fn get(&self, store: Store) -> &Account {
        match store {
            Store::Epic => &self.epic,
            Store::Gog => &self.gog,
            Store::Amazon => &self.amazon,
        }
    }

    pub(super) fn set(&mut self, store: Store, account: Account) {
        match store {
            Store::Epic => self.epic = account,
            Store::Gog => self.gog = account,
            Store::Amazon => self.amazon = account,
        }
    }

    pub fn signed_in(&self) -> usize {
        Store::ALL
            .iter()
            .filter(|store| matches!(self.get(**store), Account::SignedIn(_)))
            .count()
    }
}

#[derive(Debug, Clone)]
pub enum AccountMessage {
    Checked(Store, Result<Option<String>, String>),
    Start(Store),
    AmazonReady(Result<AmazonLogin, String>),
    /// The sign-in window ended: the code, or `None` when it was closed.
    WindowEnded(Store, Result<Option<String>, String>),
    /// Signs in in the browser instead, closing the window.
    UseBrowser,
    OpenPage,
    Paste(String),
    Finish,
    Finished(Store, Result<String, String>),
    Cancel,
    SignOut(Store),
    SignedOut(Store, Result<(), String>),
}

/// Asks each engine who is signed in, each on a thread.
pub fn check_all() -> Task<Message> {
    Task::batch(Store::ALL.map(|store| {
        Task::perform(
            blocking(move || store.account().map_err(|err| format!("{err:#}"))),
            move |result| Message::Account(AccountMessage::Checked(store, result)),
        )
    }))
}

impl Zendo {
    pub(super) fn update_account(&mut self, message: AccountMessage) -> Task<Message> {
        match message {
            AccountMessage::Checked(store, result) => {
                let signed_in = matches!(result, Ok(Some(_)));
                self.accounts.set(
                    store,
                    match result {
                        Ok(Some(name)) => Account::SignedIn(name),
                        Ok(None) => Account::SignedOut,
                        Err(err) => Account::Failed(err),
                    },
                );
                // A list kept from last time waits for Refresh.
                if signed_in && !self.store.has_list(store) {
                    self.fetch_owned(store)
                } else {
                    Task::none()
                }
            }
            AccountMessage::Start(store) => self.start_sign_in(store),
            AccountMessage::AmazonReady(result) => self.amazon_ready(result),
            AccountMessage::WindowEnded(store, result) => self.window_ended(store, result),
            AccountMessage::UseBrowser => self.use_browser(),
            AccountMessage::OpenPage => {
                if let Some(sign_in) = &self.accounts.signing_in {
                    let page = sign_in
                        .store
                        .login_page()
                        .map(str::to_owned)
                        .or_else(|| sign_in.amazon.as_ref().map(|login| login.url.clone()));
                    if let Some(page) = page.filter(|_| self.persist) {
                        open(&page);
                    }
                }
                Task::none()
            }
            AccountMessage::Paste(pasted) => {
                if let Some(sign_in) = &mut self.accounts.signing_in {
                    sign_in.pasted = pasted;
                    sign_in.note = None;
                }
                Task::none()
            }
            AccountMessage::Finish => self.finish_sign_in(),
            AccountMessage::Finished(store, result) => {
                let Some(sign_in) = self.signing_in_to(store) else {
                    return Task::none();
                };
                sign_in.busy = false;
                match result {
                    Ok(name) => {
                        self.accounts.signing_in = None;
                        self.status = Some(t!(
                            "accounts-signed-in-to",
                            store = store.label(),
                            name = name.as_str()
                        ));
                        self.accounts.set(store, Account::SignedIn(name));
                        self.fetch_owned(store)
                    }
                    Err(err) => {
                        // A code works once; Amazon's sign-in starts over.
                        sign_in.pasted.clear();
                        sign_in.note = Some(err);
                        Task::none()
                    }
                }
            }
            AccountMessage::Cancel => {
                if let Some(window) = self
                    .accounts
                    .signing_in
                    .take()
                    .and_then(|sign_in| sign_in.window)
                {
                    window.close();
                }
                Task::none()
            }
            AccountMessage::SignOut(store) => {
                self.accounts.set(store, Account::Checking);
                if !self.persist {
                    self.accounts.set(store, Account::SignedOut);
                    return Task::none();
                }
                Task::perform(
                    blocking(move || store.sign_out().map_err(|err| format!("{err:#}"))),
                    move |result| Message::Account(AccountMessage::SignedOut(store, result)),
                )
            }
            AccountMessage::SignedOut(store, result) => self.signed_out(store, result),
        }
    }

    fn amazon_ready(&mut self, result: Result<AmazonLogin, String>) -> Task<Message> {
        let Some(sign_in) = self.signing_in_to(Store::Amazon) else {
            return Task::none();
        };
        sign_in.busy = false;
        match result {
            Ok(login) => {
                let url = login.url.clone();
                sign_in.amazon = Some(login);
                if sign_in.browser {
                    open(&url);
                    Task::none()
                } else {
                    self.open_window(Store::Amazon, url)
                }
            }
            Err(err) => {
                sign_in.note = Some(err);
                Task::none()
            }
        }
    }

    fn use_browser(&mut self) -> Task<Message> {
        if let Some(sign_in) = &mut self.accounts.signing_in {
            sign_in.browser = true;
            sign_in.note = None;
            if let Some(window) = sign_in.window.take() {
                window.close();
            }
        }
        self.update_account(AccountMessage::OpenPage)
    }

    fn signed_out(&mut self, store: Store, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(()) => {
                self.accounts.set(store, Account::SignedOut);
                self.store.forget(store);
                if self.persist {
                    zendo_core::owned::forget(store);
                }
                self.status = Some(t!("accounts-signed-out-of", store = store.label()));
            }
            Err(err) => self.accounts.set(store, Account::Failed(err)),
        }
        Task::none()
    }

    fn start_sign_in(&mut self, store: Store) -> Task<Message> {
        self.accounts.signing_in = Some(SignIn {
            store,
            pasted: String::new(),
            amazon: None,
            busy: store == Store::Amazon,
            note: None,
            window: None,
            browser: false,
        });
        if !self.persist {
            return Task::none();
        }
        match store.login_page() {
            Some(page) => self.open_window(store, page.to_owned()),
            None => Task::perform(
                blocking(|| Store::start_amazon().map_err(|err| format!("{err:#}"))),
                |result| Message::Account(AccountMessage::AmazonReady(result)),
            ),
        }
    }

    /// Opens the store's page in the sign-in window, and waits on a thread
    /// for the code.
    fn open_window(&mut self, store: Store, url: String) -> Task<Message> {
        let Some(sign_in) = self.signing_in_to(store) else {
            return Task::none();
        };
        let handle = Handle::default();
        sign_in.window = Some(handle.clone());
        Task::perform(
            blocking(move || {
                sign_in_window::open(store, &url, &handle).map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::Account(AccountMessage::WindowEnded(store, result)),
        )
    }

    fn window_ended(
        &mut self,
        store: Store,
        result: Result<Option<String>, String>,
    ) -> Task<Message> {
        let Some(sign_in) = self.signing_in_to(store) else {
            return Task::none();
        };
        // Gone to the browser meanwhile: the window was closed for it.
        if sign_in.window.take().is_none() || sign_in.browser {
            return Task::none();
        }
        match result {
            Ok(Some(code)) => self.sign_in_with(code),
            Ok(None) => {
                self.accounts.signing_in = None;
                self.status = Some(t!("accounts-closed", store = store.label()));
                Task::none()
            }
            // Without a web view, the browser still works.
            Err(err) => {
                sign_in.note = Some(t!("accounts-no-web-view", error = err));
                sign_in.browser = true;
                self.update_account(AccountMessage::OpenPage)
            }
        }
    }

    fn signing_in_to(&mut self, store: Store) -> Option<&mut SignIn> {
        self.accounts
            .signing_in
            .as_mut()
            .filter(|sign_in| sign_in.store == store)
    }

    fn finish_sign_in(&mut self) -> Task<Message> {
        let persist = self.persist;
        let Some(sign_in) = &mut self.accounts.signing_in else {
            return Task::none();
        };
        if sign_in.busy {
            return Task::none();
        }
        let Some(code) = stores::code_from(sign_in.store, &sign_in.pasted) else {
            sign_in.note = Some(match sign_in.store {
                Store::Epic => t!("accounts-no-code-epic"),
                Store::Gog | Store::Amazon => t!("accounts-no-code-address"),
            });
            return Task::none();
        };
        if !persist {
            sign_in.busy = true;
            return Task::none();
        }
        self.sign_in_with(code)
    }

    /// Hands the store's code to its engine, on a thread.
    fn sign_in_with(&mut self, code: String) -> Task<Message> {
        let Some(sign_in) = &mut self.accounts.signing_in else {
            return Task::none();
        };
        sign_in.busy = true;
        sign_in.note = None;
        let store = sign_in.store;
        let amazon = sign_in.amazon.clone();
        Task::perform(
            blocking(move || {
                store
                    .sign_in(&code, amazon.as_ref())
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::Account(AccountMessage::Finished(store, result)),
        )
    }

    /// Text typed with no field focused: it starts the code, as typing in
    /// the library starts a search.
    pub(super) fn type_code(&mut self, typed: &str) -> Task<Message> {
        match &mut self.accounts.signing_in {
            Some(sign_in) if sign_in.pasted.is_empty() => {
                sign_in.pasted.push_str(typed);
                Task::batch([
                    operation::focus(code_id()),
                    operation::move_cursor_to_end(code_id()),
                ])
            }
            _ => Task::none(),
        }
    }

    pub(super) fn accounts_page(&self) -> Element<'_, Message> {
        let header = column![
            text(t!("page-accounts")).size(28).font(BOLD),
            text(t!("accounts-about")).size(13).color(colors().dim),
        ]
        .spacing(2);
        scrollable(
            column![header]
                .extend(Store::ALL.map(|store| self.account_card(store)))
                .spacing(20)
                .padding(24)
                .max_width(960),
        )
        .height(Length::Fill)
        .into()
    }

    fn account_card(&self, store: Store) -> Element<'_, Message> {
        let engine = store.engine();
        let signing_in = self
            .accounts
            .signing_in
            .as_ref()
            .filter(|sign_in| sign_in.store == store);
        let (status, color) = match self.accounts.get(store) {
            Account::Checking => (t!("key-checking"), colors().dim),
            Account::SignedOut => (t!("accounts-signed-out"), colors().dim),
            Account::SignedIn(name) => (
                t!("accounts-signed-in-as", name = name.as_str()),
                colors().good,
            ),
            Account::Failed(err) => (err.clone(), colors().bad),
        };
        let action: Element<'_, Message> = match self.accounts.get(store) {
            Account::SignedIn(_) => button(text(t!("accounts-sign-out")).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::Account(AccountMessage::SignOut(store)))
                .into(),
            Account::Checking => space::horizontal().width(0).into(),
            Account::SignedOut | Account::Failed(_) => {
                button(text(t!("accounts-sign-in")).size(14).font(BOLD))
                    .padding([8, 18])
                    .style(style::primary)
                    .on_press_maybe(
                        (engine.is_some() && signing_in.is_none())
                            .then_some(Message::Account(AccountMessage::Start(store))),
                    )
                    .into()
            }
        };

        let title = row![
            style::badge(store.source().label(), style::source_color(store.source())),
            text(store.label()).size(18).font(BOLD),
            space::horizontal(),
            action,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let engine_line = match &engine {
            Some(path) => text(t!("accounts-through", path = path.display().to_string()))
                .size(12)
                .font(Font::MONOSPACE)
                .color(colors().dim),
            None => text(t!("accounts-engine-missing", engine = store.engine_name()))
                .size(13)
                .color(colors().bad),
        };

        let mut body = column![title, text(status).size(14).color(color), engine_line].spacing(8);
        if let Some(sign_in) = signing_in {
            body = body.push(if sign_in.browser {
                sign_in_steps(sign_in)
            } else {
                window_steps(sign_in)
            });
        }
        container(body)
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }
}

/// The sign-in in Zendo's window: what it waits on, and a way out to the
/// browser.
fn window_steps(sign_in: &SignIn) -> Element<'_, Message> {
    let store = sign_in.store;
    let (line, retry) = match (&sign_in.window, sign_in.busy, &sign_in.note) {
        (_, true, _) if sign_in.amazon.is_none() && store == Store::Amazon => {
            (t!("accounts-preparing-amazon"), false)
        }
        (_, true, _) => (t!("accounts-signing-in"), false),
        (Some(_), false, _) => (t!("accounts-in-window", store = store.label()), false),
        (None, false, Some(_)) => (String::new(), true),
        (None, false, None) => (t!("accounts-opening-window"), false),
    };
    let mut buttons = row![].spacing(8).align_y(Alignment::Center);
    if retry {
        buttons = buttons.push(
            button(text(t!("try-again")).size(14).font(BOLD))
                .padding([8, 18])
                .style(style::primary)
                .on_press(Message::Account(AccountMessage::Start(store))),
        );
    }
    buttons = buttons
        .push(
            button(text(t!("accounts-use-browser")).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press_maybe(
                    (!sign_in.busy).then_some(Message::Account(AccountMessage::UseBrowser)),
                ),
        )
        .push(
            button(text(t!("cancel")).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(Message::Account(AccountMessage::Cancel)),
        );
    let mut steps = column![].spacing(10);
    if !line.is_empty() {
        steps = steps.push(text(line).size(14));
    }
    if let Some(note) = &sign_in.note {
        steps = steps.push(text(note).size(13).color(colors().bad));
    }
    container(steps.push(buttons)).padding([12, 0]).into()
}

/// The sign-in in the user's browser, finished by pasting.
fn sign_in_steps(sign_in: &SignIn) -> Element<'_, Message> {
    let store = sign_in.store;
    let ready = store != Store::Amazon || sign_in.amazon.is_some();
    let second = match store {
        Store::Epic => t!("accounts-step-two-epic"),
        Store::Gog => t!("accounts-step-two-gog"),
        Store::Amazon => t!("accounts-step-two-amazon"),
    };
    let can_finish = !sign_in.busy && ready && !sign_in.pasted.trim().is_empty();
    let field = row![
        text_input(&t!("accounts-paste-here"), &sign_in.pasted)
            .id(code_id())
            .on_input(|pasted| Message::Account(AccountMessage::Paste(pasted)))
            .on_submit(Message::Account(AccountMessage::Finish))
            .padding([8, 12])
            .size(14)
            .style(style::input),
        button(
            text(if sign_in.busy && ready {
                t!("accounts-signing-in")
            } else {
                t!("accounts-finish")
            })
            .size(14)
            .font(BOLD)
        )
        .padding([8, 18])
        .style(style::primary)
        .on_press_maybe(can_finish.then_some(Message::Account(AccountMessage::Finish))),
        button(text(t!("cancel")).size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Account(AccountMessage::Cancel)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut steps = column![
        row![
            text(t!("accounts-step-one")).size(14),
            button(
                text(if ready {
                    t!("accounts-open-again")
                } else {
                    t!("accounts-preparing")
                })
                .size(13)
            )
            .padding([4, 10])
            .style(style::secondary)
            .on_press_maybe(ready.then_some(Message::Account(AccountMessage::OpenPage))),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        text(second).size(14),
        text(t!("accounts-step-three")).size(14),
        field,
    ]
    .spacing(10);
    if let Some(note) = &sign_in.note {
        steps = steps.push(text(note).size(13).color(colors().bad));
    }
    container(steps).padding([12, 0]).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_stores_signed_in() {
        let mut accounts = Accounts::default();
        accounts.set(Store::Gog, Account::SignedIn("me".into()));
        accounts.set(Store::Epic, Account::SignedOut);
        assert_eq!(accounts.signed_in(), 1);
    }

    /// A GOG sign-in with its window open, as `open_window` leaves it.
    fn with_window() -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Account(AccountMessage::Start(Store::Gog)));
        if let Some(sign_in) = &mut zendo.accounts.signing_in {
            sign_in.window = Some(Handle::default());
        }
        zendo
    }

    fn ended(zendo: &mut Zendo, result: Result<Option<String>, String>) {
        let _ = zendo.update(Message::Account(AccountMessage::WindowEnded(
            Store::Gog,
            result,
        )));
    }

    #[test]
    fn closing_the_window_ends_the_sign_in() {
        let mut zendo = with_window();
        ended(&mut zendo, Ok(None));
        assert!(zendo.accounts.signing_in.is_none());
    }

    #[test]
    fn the_windows_code_signs_in_without_pasting() {
        let mut zendo = with_window();
        ended(&mut zendo, Ok(Some("code".into())));
        let sign_in = zendo.accounts.signing_in.as_ref().unwrap();
        assert!(sign_in.busy, "the code went to gogdl");
        assert!(sign_in.window.is_none() && !sign_in.browser);
    }

    #[test]
    fn without_a_window_the_browser_takes_over() {
        let mut zendo = with_window();
        ended(
            &mut zendo,
            Err("the sign-in window failed: no WebKitGTK".into()),
        );
        let sign_in = zendo.accounts.signing_in.as_ref().unwrap();
        assert!(sign_in.browser);
        assert!(sign_in.note.as_deref().unwrap().contains("browser"));
    }

    #[test]
    fn going_to_the_browser_closes_the_window() {
        let mut zendo = with_window();
        let _ = zendo.update(Message::Account(AccountMessage::UseBrowser));
        // The window, closed for it, ends; the sign-in carries on by paste.
        ended(&mut zendo, Ok(None));
        let sign_in = zendo.accounts.signing_in.as_ref().unwrap();
        assert!(sign_in.browser && sign_in.window.is_none());
    }
}
