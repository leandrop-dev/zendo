//! Steam's card on the Accounts page: the account Steam signed in last on
//! this computer, and the user's Steam Web API key, with which the Store
//! lists the Steam games that account owns. Steam signs in by itself; Zendo
//! only reads which account it is.

use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Source;
use zendo_core::owned::{self, Shop, SteamAnswer, SteamUser};
use zendo_core::secrets::Key;

use super::nexus::KeyField;
use super::settings::kept_note;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// Where Steam makes Web API keys.
const KEY_PAGE: &str = "https://steamcommunity.com/dev/apikey";

#[derive(Debug, Default)]
pub struct SteamState {
    key: KeyField,
    /// The account Steam signed in last here, read when the settings are.
    user: Option<SteamUser>,
}

#[derive(Debug, Clone)]
pub enum SteamMessage {
    KeyInput(String),
    ToggleShown,
    SaveKey,
    CheckKey,
    KeyChecked(Result<SteamAnswer, String>),
}

impl Zendo {
    /// After the settings are read: the key in its field, the account, and
    /// Steam's list when there is a key and none kept.
    pub(super) fn steam_loaded(&mut self) -> Task<Message> {
        self.steam.key.value = self.config.steam_key.clone().unwrap_or_default();
        if !self.persist {
            return Task::none();
        }
        self.steam.user = owned::steam_user();
        if self.config.steam_key.is_some() && !self.store.has_list(Shop::Steam) {
            return self.fetch_owned(Shop::Steam);
        }
        Task::none()
    }

    /// Reads again which account Steam signed in last, as the Accounts
    /// page opens.
    pub(super) fn want_steam_user(&mut self) -> Task<Message> {
        if self.persist {
            self.steam.user = owned::steam_user();
        }
        Task::none()
    }

    pub(super) fn update_steam(&mut self, message: SteamMessage) -> Task<Message> {
        match message {
            SteamMessage::KeyInput(value) => {
                self.steam.key.value = value;
                self.steam.key.note = None;
                Task::none()
            }
            SteamMessage::ToggleShown => {
                self.steam.key.shown = !self.steam.key.shown;
                Task::none()
            }
            SteamMessage::SaveKey => {
                let key = self.steam.key.value.trim().to_owned();
                self.steam.key.value.clone_from(&key);
                if key.is_empty() {
                    if self.store_steam_key(None) {
                        self.store.forget(Shop::Steam);
                        if self.persist {
                            owned::forget(Shop::Steam);
                        }
                        self.steam.key.note = Some((t!("key-removed"), true));
                    }
                    return Task::none();
                }
                self.steam.key.save_after_check = true;
                self.check_steam_key(key)
            }
            SteamMessage::CheckKey => {
                self.steam.key.save_after_check = false;
                let key = self.steam.key.value.trim().to_owned();
                self.check_steam_key(key)
            }
            SteamMessage::KeyChecked(result) => self.steam_key_checked(result),
        }
    }

    fn check_steam_key(&mut self, key: String) -> Task<Message> {
        if key.is_empty() {
            self.steam.key.note = Some((t!("key-paste-first"), false));
            return Task::none();
        }
        let Some(user) = self.steam.user.clone() else {
            self.steam.key.note = Some((t!("steam-no-account"), false));
            return Task::none();
        };
        self.steam.key.checking = Some(key.clone());
        self.steam.key.note = None;
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || owned::ask_steam(&key, &user).map_err(|err| format!("{err:#}"))),
            |result| Message::Steam(SteamMessage::KeyChecked(result)),
        )
    }

    fn steam_key_checked(&mut self, result: Result<SteamAnswer, String>) -> Task<Message> {
        let Some(key) = self.steam.key.checking.take() else {
            return Task::none();
        };
        let save = std::mem::take(&mut self.steam.key.save_after_check);
        let name = self
            .steam
            .user
            .as_ref()
            .map(|user| user.name.clone())
            .unwrap_or_default();
        let (note, keep) = match result {
            Ok(SteamAnswer::Games(games)) => {
                let count = games.len();
                let note = if save {
                    t!("steam-key-saved", count = count, name = name.as_str())
                } else {
                    t!("steam-key-accepted", count = count, name = name.as_str())
                };
                ((note, true), save)
            }
            // A good key: kept, with what to change in Steam.
            Ok(SteamAnswer::Nothing) => {
                ((t!("steam-key-nothing", name = name.as_str()), false), save)
            }
            Ok(SteamAnswer::Rejected) => {
                let note = if save {
                    t!("steam-key-rejected-unsaved")
                } else {
                    t!("steam-key-rejected")
                };
                ((note, false), false)
            }
            // Offline is no reason to lose what the user pasted.
            Err(err) if save => ((t!("key-saved-unchecked", error = err), false), true),
            Err(err) => ((err, false), false),
        };
        if keep && !self.store_steam_key(Some(key)) {
            return Task::none();
        }
        self.steam.key.note = Some(note);
        if keep {
            self.store.forget(Shop::Steam);
            return self.fetch_owned(Shop::Steam);
        }
        Task::none()
    }

    /// Sets the key and writes the settings; on failure the old key stays.
    fn store_steam_key(&mut self, key: Option<String>) -> bool {
        let previous = std::mem::replace(&mut self.config.steam_key, key);
        if !self.persist {
            return true;
        }
        match self.config.save() {
            Ok(()) => true,
            Err(err) => {
                self.config.steam_key = previous;
                self.steam.key.note = Some((format!("{err:#}"), false));
                false
            }
        }
    }

    pub(super) fn steam_card(&self) -> Element<'_, Message> {
        let key = &self.steam.key;
        let saved = self.config.steam_key.as_deref().unwrap_or("");
        let changed = key.value.trim() != saved;
        let field = row![
            text_input(&t!("steam-paste"), &key.value)
                .secure(!key.shown)
                .on_input(|value| Message::Steam(SteamMessage::KeyInput(value)))
                .on_submit(Message::Steam(SteamMessage::SaveKey))
                .padding([8, 12])
                .size(14)
                .style(style::input),
            button(
                text(if key.shown {
                    t!("key-hide")
                } else {
                    t!("key-show")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::Steam(SteamMessage::ToggleShown)),
            button(
                text(if key.checking.is_some() {
                    t!("key-checking")
                } else {
                    t!("key-check")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(
                (key.checking.is_none() && !key.value.trim().is_empty())
                    .then_some(Message::Steam(SteamMessage::CheckKey))
            ),
            button(text(t!("key-save")).size(14).font(BOLD))
                .padding([8, 18])
                .style(style::primary)
                .on_press_maybe(changed.then_some(Message::Steam(SteamMessage::SaveKey))),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let title = row![
            style::badge(Source::Steam.label(), style::source_color(Source::Steam)),
            text("Steam").size(18).font(BOLD),
            space::horizontal(),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let account = match &self.steam.user {
            Some(user) => text(t!("steam-account", name = user.name.as_str()))
                .size(14)
                .color(colors().good),
            None => text(t!("steam-no-account")).size(14).color(colors().bad),
        };
        let mut body = column![
            title,
            account,
            text(t!("steam-about")).size(13).color(colors().dim),
            field,
        ]
        .spacing(8);
        match &key.note {
            Some((note, good)) => {
                body = body.push(text(note).size(13).color(if *good {
                    colors().good
                } else {
                    colors().bad
                }));
            }
            None if saved.is_empty() => {
                body = body.push(text(t!("key-none")).size(13).color(colors().dim));
            }
            None => {}
        }
        body = body.push(kept_note(self.config.kept(Key::Steam)));
        body = body.push(
            row![
                button(text(t!("steam-get-key")).size(13))
                    .padding(0)
                    .style(style::link)
                    .on_press(Message::Open(KEY_PAGE.to_owned())),
                text(t!("steam-key-domain")).size(13).color(colors().dim),
            ]
            .spacing(8),
        );
        container(body)
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_user(zendo: &mut Zendo) {
        zendo.steam.user = Some(SteamUser {
            id: "76561197960265730".into(),
            name: "Player".into(),
        });
    }

    #[test]
    fn saves_a_key_steam_takes() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        with_user(&mut zendo);
        let _ = zendo.update(Message::Steam(SteamMessage::KeyInput(" abc ".into())));
        let _ = zendo.update(Message::Steam(SteamMessage::SaveKey));
        assert_eq!(zendo.steam.key.checking.as_deref(), Some("abc"));
        let games = vec![owned::Owned::new(Shop::Steam, "620", "Portal 2".into())];
        let _ = zendo.update(Message::Steam(SteamMessage::KeyChecked(Ok(
            SteamAnswer::Games(games),
        ))));
        assert_eq!(zendo.config.steam_key.as_deref(), Some("abc"));
        assert_eq!(
            zendo.steam.key.note,
            Some(("Saved. Steam lists 1 game for Player".into(), true))
        );
    }

    #[test]
    fn keeps_out_a_key_steam_refuses() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        with_user(&mut zendo);
        let _ = zendo.update(Message::Steam(SteamMessage::KeyInput("bad".into())));
        let _ = zendo.update(Message::Steam(SteamMessage::SaveKey));
        let _ = zendo.update(Message::Steam(SteamMessage::KeyChecked(Ok(
            SteamAnswer::Rejected,
        ))));
        assert_eq!(zendo.config.steam_key, None);
        assert!(zendo.steam.key.note.as_ref().is_some_and(|(_, good)| !good));
    }

    #[test]
    fn needs_a_steam_account_to_check() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.steam.user = None;
        let _ = zendo.update(Message::Steam(SteamMessage::KeyInput("abc".into())));
        let _ = zendo.update(Message::Steam(SteamMessage::CheckKey));
        assert_eq!(zendo.steam.key.checking, None);
        assert!(zendo.steam.key.note.as_ref().is_some_and(|(_, good)| !good));
    }
}
