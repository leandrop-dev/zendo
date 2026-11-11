//! Steam's card on the Accounts page: the Steam sign-in, on Steam's own
//! page in Zendo's sign-in window, with which the Store lists the games the
//! account owns and those its Steam family shares. The token Steam's page
//! gives is kept in the keyring and asked again of Steam's page, in a
//! window that never shows, when it runs out.

use iced::widget::{button, column, container, row, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Source;
use zendo_core::owned::{self, Owned, Shop};
use zendo_core::secrets::Key;
use zendo_core::steam_web::{self, Family, Session, WebAnswer};

use super::settings::kept_note;
use super::store::StoreMessage;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking, now};
use crate::i18n::t;
use crate::sign_in_window::{self, Handle};

#[derive(Debug, Default)]
pub struct SteamState {
    /// The sign-in window, while it is open.
    signing_in: Option<Handle>,
    /// What the last sign-in, sign-out or list said, and whether it went
    /// well.
    note: Option<(String, bool)>,
    /// The account's Steam family, as the last list said.
    family: Option<Family>,
    /// Why the family's games could not be listed, when they could not.
    family_note: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SteamMessage {
    SignIn,
    CancelSignIn,
    SignedIn(Result<Option<Session>, String>),
    SignOut,
    Fetched(Result<SteamFetch, String>),
}

/// Steam's list asked with the sign-in.
#[derive(Debug, Clone)]
pub enum SteamFetch {
    Games {
        games: Vec<Owned>,
        /// A new token, when the old one had run out.
        renewed: Option<Session>,
        family: Option<Family>,
        family_error: Option<String>,
    },
    /// Steam's page no longer knows the sign-in.
    SignedOut,
}

impl Zendo {
    /// The signed-in session, read from the settings.
    pub(super) fn steam_session(&self) -> Option<Session> {
        self.config
            .steam_session
            .as_deref()
            .and_then(Session::parse)
    }

    /// After the settings are read: Steam's list when signed in and none is
    /// kept.
    pub(super) fn steam_loaded(&mut self) -> Task<Message> {
        if self.persist && self.config.steam_session.is_some() && !self.store.has_list(Shop::Steam)
        {
            return self.fetch_owned(Shop::Steam);
        }
        Task::none()
    }

    pub(super) fn update_steam(&mut self, message: SteamMessage) -> Task<Message> {
        match message {
            SteamMessage::SignIn => {
                if self.steam.signing_in.is_some() {
                    return Task::none();
                }
                let handle = Handle::default();
                self.steam.signing_in = Some(handle.clone());
                self.steam.note = None;
                if !self.persist {
                    return Task::none();
                }
                Task::perform(
                    blocking(move || {
                        sign_in_window::open_steam(&handle).map_err(|err| format!("{err:#}"))
                    }),
                    |result| Message::Steam(SteamMessage::SignedIn(result)),
                )
            }
            SteamMessage::CancelSignIn => {
                if let Some(handle) = self.steam.signing_in.take() {
                    handle.close();
                }
                Task::none()
            }
            SteamMessage::SignedIn(result) => self.steam_signed_in(result),
            SteamMessage::SignOut => {
                if !self.store_steam_session(None) {
                    return Task::none();
                }
                self.store.forget(Shop::Steam);
                self.steam.family = None;
                self.steam.family_note = None;
                if self.persist {
                    owned::forget(Shop::Steam);
                    if let Err(err) = steam_web::forget_profile() {
                        self.steam.note = Some((format!("{err:#}"), false));
                        return Task::none();
                    }
                }
                self.steam.note = Some((t!("steam-signed-out"), true));
                Task::none()
            }
            SteamMessage::Fetched(result) => self.steam_fetched(result),
        }
    }

    fn steam_signed_in(&mut self, result: Result<Option<Session>, String>) -> Task<Message> {
        // Cancelled from here: whatever the window said is dropped.
        if self.steam.signing_in.take().is_none() {
            return Task::none();
        }
        match result {
            Ok(Some(session)) => {
                if !self.store_steam_session(Some(&session)) {
                    return Task::none();
                }
                self.steam.note = Some((t!("steam-signed-in"), true));
                self.steam.family = None;
                self.steam.family_note = None;
                self.store.forget(Shop::Steam);
                self.fetch_owned(Shop::Steam)
            }
            Ok(None) => Task::none(),
            Err(err) => {
                self.steam.note = Some((err, false));
                Task::none()
            }
        }
    }

    fn steam_fetched(&mut self, result: Result<SteamFetch, String>) -> Task<Message> {
        let result = match result {
            Ok(SteamFetch::Games {
                games,
                renewed,
                family,
                family_error,
            }) => {
                if let Some(session) = renewed {
                    // Kept for the next start; the list stands either way.
                    self.store_steam_session(Some(&session));
                }
                self.steam.family = family;
                self.steam.family_note = family_error;
                Ok(games)
            }
            Ok(SteamFetch::SignedOut) => {
                self.store_steam_session(None);
                self.steam.note = Some((t!("steam-sign-in-again"), false));
                Err(t!("steam-sign-in-again"))
            }
            Err(err) => Err(err),
        };
        self.update_store(StoreMessage::Fetched(Shop::Steam, result, Vec::new()))
    }

    /// Sets the session and writes the settings; on failure the old one
    /// stays.
    fn store_steam_session(&mut self, session: Option<&Session>) -> bool {
        let previous = std::mem::replace(
            &mut self.config.steam_session,
            session.map(Session::to_text),
        );
        if !self.persist {
            return true;
        }
        match self.config.save() {
            Ok(()) => true,
            Err(err) => {
                self.config.steam_session = previous;
                self.steam.note = Some((format!("{err:#}"), false));
                false
            }
        }
    }

    /// The signed-in account's name: Steam's own on this computer when it
    /// knows it, else the Steam ID.
    fn steam_name(session: &Session) -> String {
        owned::steam_user_named(&session.steamid).unwrap_or_else(|| session.steamid.clone())
    }

    pub(super) fn steam_card(&self) -> Element<'_, Message> {
        let title = row![
            style::badge(Source::Steam.label(), style::source_color(Source::Steam)),
            text("Steam").size(18).font(BOLD),
            space::horizontal(),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let mut body = column![title].spacing(8);
        let session = self.steam_session();
        match (&session, &self.steam.signing_in) {
            (Some(session), _) => {
                let name = Self::steam_name(session);
                body = body.push(
                    row![
                        text(t!("accounts-signed-in-as", name = name.as_str()))
                            .size(14)
                            .color(colors().good),
                        space::horizontal(),
                        button(text(t!("accounts-sign-out")).size(14))
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press(Message::Steam(SteamMessage::SignOut)),
                    ]
                    .align_y(Alignment::Center),
                );
                body = body.push(
                    text(t!("steam-signed-in-about"))
                        .size(13)
                        .color(colors().dim),
                );
                if let Some(family) = &self.steam.family {
                    body = body.push(
                        text(t!(
                            "steam-family",
                            name = family.name.as_str(),
                            count = family.members
                        ))
                        .size(13),
                    );
                }
            }
            (None, Some(_)) => {
                body = body.push(text(t!("steam-sign-in-about")).size(13).color(colors().dim));
                body = body.push(
                    row![
                        text(t!("steam-signing-in")).size(14),
                        space::horizontal(),
                        button(text(t!("cancel")).size(14))
                            .padding([8, 14])
                            .style(style::secondary)
                            .on_press(Message::Steam(SteamMessage::CancelSignIn)),
                    ]
                    .align_y(Alignment::Center),
                );
            }
            (None, None) => {
                body = body.push(text(t!("steam-sign-in-about")).size(13).color(colors().dim));
                body = body.push(
                    button(text(t!("steam-sign-in")).size(14).font(BOLD))
                        .padding([8, 18])
                        .style(style::primary)
                        .on_press(Message::Steam(SteamMessage::SignIn)),
                );
            }
        }
        if let Some((note, good)) = &self.steam.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        if let Some(error) = &self.steam.family_note {
            body = body.push(
                text(t!("steam-family-failed", error = error.as_str()))
                    .size(13)
                    .color(colors().bad),
            );
        }
        body = body.push(kept_note(self.config.kept(Key::SteamSession)));
        container(body)
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }
}

/// Steam's list with the sign-in, on a thread: a new token first when
/// the old one has run out or Steam refuses it.
pub(super) fn fetch_steam_signed_in(session: Session) -> Task<Message> {
    Task::perform(
        blocking(move || fetch_signed_in(session).map_err(|err| format!("{err:#}"))),
        |result| Message::Steam(SteamMessage::Fetched(result)),
    )
}

/// Steam's list with the sign-in: a new token from Steam's page first when
/// the old one has run out, and again once if Steam refuses it.
fn fetch_signed_in(session: Session) -> anyhow::Result<SteamFetch> {
    let mut session = session;
    let mut renewed = None;
    if !session.fresh(now()) {
        match sign_in_window::renew_steam()? {
            Some(new) => {
                session = new.clone();
                renewed = Some(new);
            }
            None => return Ok(SteamFetch::SignedOut),
        }
    }
    let mut answer = steam_web::ask(&session)?;
    if matches!(answer, WebAnswer::Rejected) && renewed.is_none() {
        match sign_in_window::renew_steam()? {
            Some(new) => {
                answer = steam_web::ask(&new)?;
                renewed = Some(new);
            }
            None => return Ok(SteamFetch::SignedOut),
        }
    }
    match answer {
        WebAnswer::Games {
            games,
            family,
            family_error,
        } => Ok(SteamFetch::Games {
            games: steam_web::keep(games)?,
            renewed,
            family,
            family_error,
        }),
        WebAnswer::Rejected => Ok(SteamFetch::SignedOut),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            steamid: "76561197960265730".into(),
            token: "token".into(),
        }
    }

    #[test]
    fn keeps_the_sign_in_and_asks_for_the_list() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Steam(SteamMessage::SignIn));
        assert!(zendo.steam.signing_in.is_some());
        let _ = zendo.update(Message::Steam(SteamMessage::SignedIn(Ok(Some(session())))));
        assert!(zendo.steam.signing_in.is_none());
        assert_eq!(zendo.steam_session(), Some(session()));
        assert_eq!(zendo.steam.note, Some((t!("steam-signed-in"), true)));
    }

    #[test]
    fn a_cancelled_sign_in_keeps_nothing() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Steam(SteamMessage::SignIn));
        let _ = zendo.update(Message::Steam(SteamMessage::CancelSignIn));
        let _ = zendo.update(Message::Steam(SteamMessage::SignedIn(Ok(Some(session())))));
        assert_eq!(zendo.config.steam_session, None);
    }

    #[test]
    fn a_renewed_token_is_kept_and_a_lost_sign_in_forgotten() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.config.steam_session = Some(session().to_text());
        let renewed = Session {
            token: "new".into(),
            ..session()
        };
        let games = vec![Owned::new(Shop::Steam, "620", "Portal 2".into())];
        let _ = zendo.update(Message::Steam(SteamMessage::Fetched(Ok(
            SteamFetch::Games {
                games,
                renewed: Some(renewed.clone()),
                family: None,
                family_error: Some("Steam's family answered 401".into()),
            },
        ))));
        assert_eq!(zendo.steam_session(), Some(renewed));
        assert!(zendo.store.has_list(Shop::Steam));
        assert!(zendo.steam.family_note.is_some());
        let _ = zendo.update(Message::Steam(SteamMessage::Fetched(Ok(
            SteamFetch::SignedOut,
        ))));
        assert_eq!(zendo.config.steam_session, None);
        assert_eq!(zendo.steam.note, Some((t!("steam-sign-in-again"), false)));
    }

    #[test]
    fn signing_out_forgets_the_sign_in_and_the_list() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.config.steam_session = Some(session().to_text());
        let _ = zendo.update(Message::Steam(SteamMessage::SignOut));
        assert_eq!(zendo.config.steam_session, None);
        assert!(!zendo.store.has_list(Shop::Steam));
    }
}
