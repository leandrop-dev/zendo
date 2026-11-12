//! The Epic overlay section of Settings: whether Epic's EOS overlay is
//! installed in Zendo's folder, and Install (Update once installed) and
//! Remove, through legendary. Each Epic game turns it on in its launch
//! options.

use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Task};
use zendo_core::eos_overlay;
use zendo_core::stores::Store;

use super::settings::section;
use super::style::{self, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct OverlayState {
    installed: bool,
    /// legendary is there to install it.
    engine: bool,
    step: Step,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
}

/// Where Install and Remove are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Step {
    #[default]
    Idle,
    /// Remove was pressed once.
    ConfirmRemove,
    /// Installing, updating or removing.
    Working,
}

#[derive(Debug, Clone)]
pub enum OverlayMessage {
    Install,
    Remove,
    /// Done: what was done (installed or updated, removed), and how it went.
    Done(String, Result<(), String>),
}

impl Zendo {
    /// Looks again whether it is installed, as Settings opens.
    pub(super) fn want_overlay(&mut self) {
        self.overlay.installed = eos_overlay::installed();
        self.overlay.engine = Store::Epic.engine().is_some();
    }

    pub(super) fn update_overlay(&mut self, message: OverlayMessage) -> Task<Message> {
        let state = &mut self.overlay;
        match message {
            OverlayMessage::Install => {
                if state.step == Step::Working {
                    return Task::none();
                }
                state.step = Step::Working;
                state.note = None;
                if !self.persist {
                    return Task::none();
                }
                let done = if state.installed {
                    t!("overlay-updated")
                } else {
                    t!("overlay-installed")
                };
                return Task::perform(
                    blocking(|| eos_overlay::install().map_err(|err| format!("{err:#}"))),
                    move |result| Message::Overlay(OverlayMessage::Done(done.clone(), result)),
                );
            }
            OverlayMessage::Remove => {
                match state.step {
                    Step::Working => return Task::none(),
                    Step::Idle => {
                        state.step = Step::ConfirmRemove;
                        return Task::none();
                    }
                    Step::ConfirmRemove => state.step = Step::Working,
                }
                state.note = None;
                if !self.persist {
                    return Task::none();
                }
                return Task::perform(
                    blocking(|| eos_overlay::remove().map_err(|err| format!("{err:#}"))),
                    |result| Message::Overlay(OverlayMessage::Done(t!("overlay-removed"), result)),
                );
            }
            OverlayMessage::Done(done, result) => {
                state.step = Step::Idle;
                state.installed = eos_overlay::installed();
                state.note = Some(match result {
                    Ok(()) => (done, true),
                    Err(err) => (err, false),
                });
            }
        }
        Task::none()
    }

    pub(super) fn overlay_section(&self) -> Element<'_, Message> {
        let state = &self.overlay;
        let working = state.step == Step::Working;
        let status = if working {
            t!("overlay-working")
        } else if state.installed {
            t!(
                "overlay-is-installed",
                path = eos_overlay::folder()
                    .map(|folder| folder.display().to_string())
                    .unwrap_or_default()
            )
        } else if state.engine {
            t!("overlay-not-installed")
        } else {
            t!("overlay-no-legendary")
        };
        let ready = state.engine && !working;
        let install = button(style::button_text(if state.installed {
            t!("overlay-update")
        } else {
            t!("overlay-install")
        }))
        .padding(style::BUTTON)
        .style(style::secondary)
        .on_press_maybe(ready.then_some(Message::Overlay(OverlayMessage::Install)));
        let mut buttons = row![install]
            .spacing(style::SPACE_M)
            .align_y(Alignment::Center);
        if state.installed {
            buttons = buttons.push(
                button(style::button_text(if state.step == Step::ConfirmRemove {
                    t!("really-remove")
                } else {
                    t!("remove")
                }))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press_maybe(ready.then_some(Message::Overlay(OverlayMessage::Remove))),
            );
        }
        let mut body =
            column![text(status).size(13).color(colors().dim), buttons].spacing(style::SPACE_M);
        if let Some((note, good)) = &state.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        section(t!("overlay-title"), t!("overlay-about"), body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_once_more_before_removing() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.overlay.installed = true;
        zendo.overlay.engine = true;
        let _ = zendo.update(Message::Overlay(OverlayMessage::Remove));
        assert_eq!(zendo.overlay.step, Step::ConfirmRemove);
        let _ = zendo.update(Message::Overlay(OverlayMessage::Remove));
        assert_eq!(zendo.overlay.step, Step::Working, "removing");
        let _ = zendo.update(Message::Overlay(OverlayMessage::Done(
            "Removed".into(),
            Err("legendary failed".into()),
        )));
        assert_eq!(zendo.overlay.step, Step::Idle);
        assert_eq!(
            zendo.overlay.note,
            Some(("legendary failed".to_owned(), false))
        );
    }
}
