//! Steam's installs in the Iced window: the messages, the Downloads card and
//! the ticks that read Steam's manifests. What is followed, and how, is
//! `zendo_shell::steam_installs`, shared with the Slint window.

use std::thread;
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, progress_bar, row, space, text};
use iced::{Alignment, Element, Length, Task};
use zendo_core::steam::{AppState, VerifyEvent};
use zendo_shell::steam_installs::{BUSY, IDLE, Looked, Row, look};

pub use zendo_shell::steam_installs::{Following, SteamInstalls, percent, progress_line};

use super::style::{self, BOLD, button_text, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub enum SteamInstallMessage {
    /// Install with Steam pressed, for an app and its title.
    Install(String, String),
    /// Stop waiting for Steam to start an app's install (its dialog was
    /// cancelled).
    Forget(String),
    /// Uninstall with Steam pressed, for an app and its title.
    Uninstall(String, String),
    /// Verify with Steam pressed, for an app and its title.
    Verify(String, String),
    Tick,
    /// Steam's manifests read, and the verifies its log tells of.
    States(Vec<AppState>, Vec<VerifyEvent>),
}

impl Zendo {
    pub(super) fn update_steam_installs(&mut self, message: SteamInstallMessage) -> Task<Message> {
        match message {
            SteamInstallMessage::Install(app, title) => {
                if let Some(line) = self.steam_installs.install(&app, &title, self.persist) {
                    self.status = Some(line);
                }
                Task::none()
            }
            SteamInstallMessage::Forget(app) => {
                self.steam_installs.forget(&app);
                Task::none()
            }
            SteamInstallMessage::Uninstall(app, title) => {
                self.status = Some(self.steam_installs.uninstall(&app, &title, self.persist));
                Task::none()
            }
            SteamInstallMessage::Verify(app, title) => {
                if let Some(line) = self.steam_installs.verify(&app, &title, self.persist) {
                    self.status = Some(line);
                }
                Task::none()
            }
            SteamInstallMessage::Tick => {
                if !self.persist {
                    return Task::none();
                }
                let Some(from) = self.steam_installs.begin_look() else {
                    return Task::none();
                };
                Task::perform(blocking(move || look(from)), |(states, verifies)| {
                    Message::SteamInstall(SteamInstallMessage::States(states, verifies))
                })
            }
            SteamInstallMessage::States(states, verifies) => {
                let Looked { say, rescan } = self.steam_installs.looked(states, &verifies);
                let Some(line) = say else {
                    return Task::none();
                };
                self.status = Some(line.clone());
                if !rescan {
                    return Task::none();
                }
                // Said again once the library is read, which clears the line.
                self.local.after_scan = Some((None, line));
                self.refresh()
            }
        }
    }

    /// The Downloads page's card for Steam, when it has something going.
    pub(super) fn steam_downloads_card(&self) -> Option<Element<'_, Message>> {
        let installs = &self.steam_installs;
        if !installs.active() {
            return None;
        }
        let mut card = column![
            row![
                text(t!("steam-downloads-title").to_uppercase())
                    .size(14)
                    .font(BOLD),
                space::horizontal(),
                button(button_text(t!("steam-open-downloads")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press(Message::Open("steam://open/downloads".to_owned())),
            ]
            .align_y(Alignment::Center),
            text(t!("steam-downloads-about"))
                .size(13)
                .color(colors().dim),
        ]
        .spacing(12);
        for line in installs.rows() {
            card = card.push(steam_row(line));
        }
        Some(
            container(card)
                .padding(20)
                .width(Length::Fill)
                .style(style::card)
                .into(),
        )
    }
}

fn steam_row<'a>(line: Row) -> Element<'a, Message> {
    let mut head = row![
        text(line.title).size(15).width(Length::Fill),
        text(line.line).size(13).color(colors().dim),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if line.forget {
        head = head.push(
            button(button_text(t!("steam-stop-waiting")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::SteamInstall(SteamInstallMessage::Forget(line.app))),
        );
    }
    match line.percent {
        Some(percent) => column![
            head,
            progress_bar(0.0..=100.0, percent)
                .girth(8)
                .style(style::progress),
        ]
        .spacing(8)
        .into(),
        None => head.into(),
    }
}

/// A tick every [`BUSY`], for as long as it is listened to.
pub fn busy_ticks() -> impl Stream<Item = ()> {
    ticks(BUSY)
}

/// A tick every [`IDLE`].
pub fn idle_ticks() -> impl Stream<Item = ()> {
    ticks(IDLE)
}

fn ticks(every: Duration) -> impl Stream<Item = ()> {
    iced::stream::channel(1, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            while sender.unbounded_send(()).is_ok() {
                thread::sleep(every);
            }
        });
        while let Some(()) = receiver.next().await {
            if output.send(()).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Iced half: the messages reach the shared state, and what Steam
    /// finishing says is kept to be said again after the library is read.
    #[test]
    fn a_finished_install_is_said_again_after_the_scan() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::Install(
            "620".into(),
            "Portal 2".into(),
        )));
        assert!(matches!(
            zendo.steam_installs.following("620"),
            Some(Following::Asked)
        ));
        let state = |progress| AppState {
            app: "620".into(),
            name: "Game 620".into(),
            progress,
        };
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
            vec![state(zendo_core::steam::Progress::Waiting)],
            Vec::new(),
        )));
        let _ = zendo.update(Message::SteamInstall(SteamInstallMessage::States(
            vec![state(zendo_core::steam::Progress::Installed)],
            Vec::new(),
        )));
        assert!(!zendo.steam_installs.active());
        assert_eq!(
            zendo.local.after_scan,
            Some((None, "Game 620 is installed".into()))
        );
    }
}
