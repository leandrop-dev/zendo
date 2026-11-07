//! The DLC card on a game's page: the DLC the user owns for it, which are
//! installed, and Install or Uninstall for the Epic and GOG games Zendo
//! installed. Heroic's games show theirs; Heroic installs those.

use std::collections::HashSet;

use iced::widget::{button, column, row, space, text};
use iced::{Alignment, Element, Length};
use zendo_core::downloads::{Action, Job};
use zendo_core::stores::Store;
use zendo_core::{Game, Size, Source};

use super::accounts::Account;
use super::downloads::DownloadMessage;
use super::game::{GameViewMessage, card};
use super::store::InfoState;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo};
use crate::i18n::t;

/// One DLC as the card shows it.
struct Line<'a> {
    id: &'a str,
    title: &'a str,
    installed: bool,
    /// False for an Epic DLC that comes with the game.
    installable: bool,
    /// Bytes on disk, when the store says.
    size: Option<u64>,
}

impl Zendo {
    /// The card, for an Epic or GOG game with DLC owned or installed.
    pub(super) fn dlc_card<'a>(&'a self, game: &'a Game) -> Option<Element<'a, Message>> {
        let store = match game.source {
            Source::Epic => Store::Epic,
            Source::Gog => Store::Gog,
            _ => return None,
        };
        let installed: HashSet<&str> = game.details.dlc.iter().map(|dlc| dlc.id.as_str()).collect();
        let info = self.store.info.get(&game.id);
        let owned = match info {
            Some(InfoState::Got(info)) => info.dlc.as_slice(),
            _ => &[],
        };
        let mut lines: Vec<Line<'a>> = owned
            .iter()
            .map(|dlc| Line {
                id: &dlc.id,
                title: &dlc.title,
                installed: installed.contains(dlc.id.as_str()),
                installable: dlc.installable,
                size: dlc.install_size,
            })
            .collect();
        // Installed, but not in the store's list (or the list is not here).
        for dlc in &game.details.dlc {
            if !owned.iter().any(|item| item.id == dlc.id) {
                lines.push(Line {
                    id: &dlc.id,
                    title: dlc.title.as_deref().unwrap_or(&dlc.id),
                    installed: true,
                    installable: true,
                    size: None,
                });
            }
        }
        if lines.is_empty() {
            return None;
        }

        let manage = game.installed_by_zendo();
        let free = manage && self.downloads.job_for(&game.id).is_none();
        let to_install: Vec<String> = lines
            .iter()
            .filter(|line| line.installable && !line.installed)
            .map(|line| line.id.to_owned())
            .collect();
        let installable = lines.iter().filter(|line| line.installable).count();
        let installed_count = lines
            .iter()
            .filter(|line| line.installable && line.installed)
            .count();

        let mut top = row![
            text(t!(
                "dlc-count",
                installed = installed_count,
                total = installable
            ))
            .size(13)
            .color(colors().dim),
            space::horizontal(),
        ]
        .align_y(Alignment::Center);
        if manage && to_install.len() > 1 {
            top = top.push(
                button(text(t!("dlc-install-all")).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press_maybe(
                        free.then(|| dlc_job(game, Action::AddDlc, to_install))
                            .flatten(),
                    ),
            );
        }
        let mut body = column![top].spacing(12);
        for line in lines {
            body = body.push(self.dlc_line(game, &line, manage, free));
        }
        let note = if manage {
            None
        } else if info.is_none() && !matches!(self.accounts.get(store), Account::SignedIn(_)) {
            Some(t!("dlc-sign-in", store = store.label()))
        } else {
            Some(t!("dlc-in-heroic"))
        };
        if let Some(note) = note {
            body = body.push(text(note).size(12).color(colors().dim));
        }
        if matches!(info, Some(InfoState::Asking)) {
            body = body.push(text(t!("dlc-asking")).size(12).color(colors().dim));
        }
        Some(card(t!("dlc-title"), body.into()))
    }

    fn dlc_line<'a>(
        &self,
        game: &Game,
        line: &Line<'a>,
        manage: bool,
        free: bool,
    ) -> Element<'a, Message> {
        let about = if line.installable {
            line.size.map(|bytes| Size::Bytes(bytes).to_string())
        } else {
            Some(t!("dlc-comes-with"))
        };
        let mut words = column![text(line.title.to_owned()).size(14)].spacing(2);
        if let Some(about) = about {
            words = words.push(text(about).size(12).color(colors().dim));
        }
        let action: Element<'a, Message> = match (line.installable, line.installed, manage) {
            (false, _, _) => space::horizontal().width(0).into(),
            (true, true, true) => {
                let confirming = self.game_view.confirm_dlc.as_deref() == Some(line.id);
                if confirming {
                    button(text(t!("downloads-press-again")).size(13).font(BOLD))
                        .padding([6, 12])
                        .style(style::danger)
                        .on_press_maybe(
                            free.then(|| {
                                dlc_job(game, Action::RemoveDlc, vec![line.id.to_owned()])
                            })
                            .flatten(),
                        )
                        .into()
                } else {
                    button(text(t!("action-label", action = "uninstall")).size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press_maybe(free.then(|| {
                            Message::GameView(GameViewMessage::AskRemoveDlc(line.id.to_owned()))
                        }))
                        .into()
                }
            }
            (true, false, true) => button(text(t!("install")).size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press_maybe(
                    free.then(|| dlc_job(game, Action::AddDlc, vec![line.id.to_owned()]))
                        .flatten(),
                )
                .into(),
            (true, true, false) => text(t!("installed")).size(13).color(colors().good).into(),
            (true, false, false) => text(t!("dlc-not-installed"))
                .size(13)
                .color(colors().dim)
                .into(),
        };
        row![words.width(Length::Fill), action]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }
}

fn dlc_job(game: &Game, action: Action, dlc: Vec<String>) -> Option<Message> {
    Job::for_dlc(game, action, dlc).map(|job| Message::Download(DownloadMessage::Queue(job)))
}
