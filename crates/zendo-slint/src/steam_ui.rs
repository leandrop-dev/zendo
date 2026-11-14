//! Steam's card on the Accounts page: the Steam sign-in, on Steam's own page
//! in Zendo's sign-in window, with which the Store lists the games the
//! account owns and those its Steam family shares. The token Steam's page
//! gives is kept in the keyring (through the settings, `Setting::SteamSession`)
//! and asked again of Steam's page, in a window that never shows, when it
//! runs out. A part of the Library's glue (`library_ui`).

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;
use zendo_core::owned::{self, Shop};
use zendo_core::secrets::Key;
use zendo_core::steam_web::{self, Family, Session};
use zendo_shell::sign_in_window::{self, Handle};
use zendo_shell::steam_list::SteamFetch;
use zendo_shell::t;

use super::settings::{Setting, kept_text};
use super::{Glue, with_glue};
use crate::{AccountsData, MainWindow, SteamCardData};

#[derive(Default)]
pub(super) struct SteamState {
    /// The sign-in window, while it is open.
    signing_in: RefCell<Option<Handle>>,
    /// What the last sign-in, sign-out or list said, and whether it went
    /// well.
    note: RefCell<Option<(String, bool)>>,
    /// The account's Steam family, as the last list said.
    family: RefCell<Option<Family>>,
    /// Why the family's games could not be listed, when they could not.
    family_note: RefCell<Option<String>>,
    /// The signed-in account's name, looked up once for its Steam ID.
    name: RefCell<Option<(String, String)>>,
}

impl Glue {
    /// The signed-in session, read from the settings. One that does not parse
    /// counts as none, for the card, the Store and the count alike.
    pub(super) fn steam_session(&self) -> Option<Session> {
        self.config
            .borrow()
            .as_ref()
            .and_then(|config| config.steam_session.as_deref())
            .and_then(Session::parse)
    }

    /// Hooks the card's callbacks up.
    pub(super) fn connect_steam(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<AccountsData>();
        let glue = Rc::clone(self);
        data.on_steam_sign_in(move || glue.steam_sign_in());
        let glue = Rc::clone(self);
        data.on_steam_cancel(move || glue.steam_cancel());
        let glue = Rc::clone(self);
        data.on_steam_sign_out(move || glue.steam_sign_out());
    }

    fn steam_note(&self, note: Option<(String, bool)>) {
        *self.steam.note.borrow_mut() = note;
    }

    fn steam_sign_in(self: &Rc<Self>) {
        if self.steam.signing_in.borrow().is_some() {
            return;
        }
        let handle = Handle::default();
        *self.steam.signing_in.borrow_mut() = Some(handle.clone());
        self.steam_note(None);
        self.show_accounts();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = sign_in_window::open_steam(&handle).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.steam_signed_in(result));
            });
        });
    }

    fn steam_cancel(&self) {
        if let Some(handle) = self.steam.signing_in.borrow_mut().take() {
            handle.close();
        }
        self.show_accounts();
    }

    /// Steam's page handed back a session, or the window was closed.
    pub(super) fn steam_signed_in(self: &Rc<Self>, result: Result<Option<Session>, String>) {
        // Cancelled from here: whatever the window said is dropped.
        if self.steam.signing_in.borrow_mut().take().is_none() {
            return;
        }
        match result {
            Ok(Some(session)) => {
                self.steam_note(Some((t!("steam-signed-in"), true)));
                *self.steam.family.borrow_mut() = None;
                *self.steam.family_note.borrow_mut() = None;
                self.save_setting(Setting::SteamSession(Some(session.to_text())));
                self.store_forget(Shop::Steam);
                self.fetch_owned(Shop::Steam);
            }
            Ok(None) => {}
            Err(err) => self.steam_note(Some((err, false))),
        }
        self.show_accounts();
    }

    fn steam_sign_out(self: &Rc<Self>) {
        if self.steam_session().is_none() {
            return;
        }
        self.save_setting(Setting::SteamSession(None));
        self.store_forget(Shop::Steam);
        *self.steam.family.borrow_mut() = None;
        *self.steam.family_note.borrow_mut() = None;
        if self.persist {
            owned::forget(Shop::Steam);
            if let Err(err) = steam_web::forget_profile() {
                self.steam_note(Some((format!("{err:#}"), false)));
                self.show_accounts();
                return;
            }
        }
        self.steam_note(Some((t!("steam-signed-out"), true)));
        self.show_accounts();
    }

    /// Steam's list came in, with the sign-in.
    pub(super) fn steam_fetched(self: &Rc<Self>, result: Result<SteamFetch, String>) {
        let result = match result {
            Ok(SteamFetch::Games {
                games,
                renewed,
                family,
                family_error,
            }) => {
                if let Some(session) = renewed {
                    // Kept for the next start; the list stands either way.
                    self.save_setting(Setting::SteamSession(Some(session.to_text())));
                }
                *self.steam.family.borrow_mut() = family;
                *self.steam.family_note.borrow_mut() = family_error;
                Ok(games)
            }
            Ok(SteamFetch::SignedOut) => {
                self.save_setting(Setting::SteamSession(None));
                self.steam_note(Some((t!("steam-sign-in-again"), false)));
                Err(t!("steam-sign-in-again"))
            }
            Err(err) => Err(err),
        };
        self.store_fetched(Shop::Steam, result, &[]);
        self.show_accounts();
    }

    /// The signed-in account's name: Steam's own on this computer when it
    /// knows it, else the Steam ID. Looked up once for each account.
    fn steam_name(&self, session: &Session) -> String {
        if let Some((id, name)) = &*self.steam.name.borrow()
            && *id == session.steamid
        {
            return name.clone();
        }
        // Reads Steam's files: not in tests.
        let name = if self.persist {
            owned::steam_user_named(&session.steamid)
        } else {
            None
        }
        .unwrap_or_else(|| session.steamid.clone());
        *self.steam.name.borrow_mut() = Some((session.steamid.clone(), name.clone()));
        name
    }

    /// What the card says now.
    pub(super) fn steam_card(&self) -> SteamCardData {
        let session = self.steam_session();
        let signing_in = self.steam.signing_in.borrow().is_some();
        let (state, status, about) = match (&session, signing_in) {
            (Some(session), _) => (
                2,
                t!("accounts-signed-in-as", name = self.steam_name(session)),
                t!("steam-signed-in-about"),
            ),
            (None, true) => (1, t!("steam-signing-in"), t!("steam-sign-in-about")),
            (None, false) => (0, String::new(), t!("steam-sign-in-about")),
        };
        let family = self
            .steam
            .family
            .borrow()
            .as_ref()
            .filter(|_| session.is_some())
            .map(|family| {
                t!(
                    "steam-family",
                    name = family.name.as_str(),
                    count = family.members
                )
            })
            .unwrap_or_default();
        let (note, note_good) = self.steam.note.borrow().clone().unwrap_or_default();
        let family_note = self
            .steam
            .family_note
            .borrow()
            .as_ref()
            .map(|error| t!("steam-family-failed", error = error.as_str()))
            .unwrap_or_default();
        let config = self.settings_config();
        let (kept, kept_bad) = kept_text(config.kept(Key::SteamSession));
        SteamCardData {
            state,
            status: status.into(),
            about: about.into(),
            family: family.into(),
            note: note.into(),
            note_good,
            family_note: family_note.into(),
            kept: kept.into(),
            kept_bad,
        }
    }
}
