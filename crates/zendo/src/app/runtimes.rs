//! The anti-cheat section of Settings: for Easy Anti-Cheat and `BattlEye`,
//! the copy games use (Steam's, else Zendo's own), the build Lutris lists,
//! and Download, Update or Remove for Zendo's copy. Games download the one
//! they need on their own before they start; this is for seeing and
//! tidying.

use std::sync::atomic::AtomicBool;

use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::runtimes::{self, Found, Remote, Runtime};

use super::settings::section;
use super::style::{self, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct RuntimesState {
    /// The copy games use, for each runtime, in `Runtime::ALL`'s order.
    found: Vec<(Runtime, Option<Found>)>,
    /// Zendo's own copies, used or not, with their build.
    own: Vec<(Runtime, Option<String>)>,
    /// What Lutris lists; `None` until it answers.
    latest: Option<Vec<Remote>>,
    error: Option<String>,
    asking: bool,
    /// The runtime being downloaded.
    downloading: Option<Runtime>,
    /// The runtime whose Remove was pressed once.
    confirm_remove: Option<Runtime>,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum RuntimeMessage {
    Latest(Result<Vec<Remote>, String>),
    Download(Runtime),
    Done(Runtime, Result<(), String>),
    Remove(Runtime),
}

impl RuntimesState {
    fn refresh(&mut self) {
        self.found = Runtime::ALL
            .into_iter()
            .map(|runtime| (runtime, runtimes::find(runtime)))
            .collect();
        self.own = Runtime::ALL
            .into_iter()
            .filter_map(|runtime| match runtimes::own(runtime)? {
                Found::Zendo { version, .. } => Some((runtime, version)),
                Found::Steam(_) => None,
            })
            .collect();
    }

    fn remote(&self, runtime: Runtime) -> Option<&Remote> {
        self.latest
            .as_ref()?
            .iter()
            .find(|remote| remote.runtime == runtime)
    }

    fn has_own(&self, runtime: Runtime) -> bool {
        self.own.iter().any(|(r, _)| *r == runtime)
    }

    /// The build of Zendo's copy, when it has one and knows it.
    fn own_version(&self, runtime: Runtime) -> Option<&str> {
        self.own
            .iter()
            .find(|(r, _)| *r == runtime)
            .and_then(|(_, version)| version.as_deref())
    }
}

impl Zendo {
    /// Looks at the runtimes on the machine, and asks Lutris for its list
    /// once a session.
    pub(super) fn want_runtimes(&mut self) -> Task<Message> {
        let state = &mut self.runtimes;
        state.refresh();
        if !self.persist || state.latest.is_some() || state.asking {
            return Task::none();
        }
        state.asking = true;
        state.error = None;
        Task::perform(
            blocking(|| runtimes::latest().map_err(|err| format!("{err:#}"))),
            |result| Message::Runtime(RuntimeMessage::Latest(result)),
        )
    }

    pub(super) fn update_runtimes(&mut self, message: RuntimeMessage) -> Task<Message> {
        let state = &mut self.runtimes;
        match message {
            RuntimeMessage::Latest(result) => {
                state.asking = false;
                match result {
                    Ok(latest) => state.latest = Some(latest),
                    Err(err) => state.error = Some(err),
                }
            }
            RuntimeMessage::Download(runtime) => {
                if state.downloading.is_some() {
                    return Task::none();
                }
                let Some(remote) = state.remote(runtime).cloned() else {
                    return Task::none();
                };
                state.downloading = Some(runtime);
                state.note = None;
                if !self.persist {
                    return Task::none();
                }
                return Task::perform(
                    blocking(move || {
                        runtimes::install(&remote, &AtomicBool::new(false), &mut |_, _| {})
                            .map(|_| ())
                            .map_err(|err| format!("{err:#}"))
                    }),
                    move |result| Message::Runtime(RuntimeMessage::Done(runtime, result)),
                );
            }
            RuntimeMessage::Done(runtime, result) => {
                state.downloading = None;
                state.refresh();
                state.note = Some(match result {
                    Ok(()) => (t!("runtimes-downloaded", runtime = runtime.label()), true),
                    Err(err) => (err, false),
                });
            }
            RuntimeMessage::Remove(runtime) => {
                if state.confirm_remove != Some(runtime) {
                    state.confirm_remove = Some(runtime);
                    return Task::none();
                }
                state.confirm_remove = None;
                let result = if self.persist {
                    runtimes::remove(runtime)
                } else {
                    Ok(())
                };
                state.refresh();
                state.note = Some(match result {
                    Ok(()) => (t!("runtimes-removed", runtime = runtime.label()), true),
                    Err(err) => (format!("{err:#}"), false),
                });
            }
        }
        Task::none()
    }

    pub(super) fn runtimes_section(&self) -> Element<'_, Message> {
        let state = &self.runtimes;
        let mut body = column![].spacing(12);
        for (runtime, found) in &state.found {
            body = body.push(self.runtime_row(*runtime, found.as_ref()));
        }
        if state.asking {
            body = body.push(text(t!("runtimes-asking")).size(13).color(colors().dim));
        } else if let Some(err) = &state.error {
            body = body.push(text(err).size(13).color(colors().bad));
        }
        if let Some((note, good)) = &state.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        section(t!("runtimes-title"), t!("runtimes-about"), body)
    }

    fn runtime_row<'a>(
        &'a self,
        runtime: Runtime,
        found: Option<&'a Found>,
    ) -> Element<'a, Message> {
        let state = &self.runtimes;
        let own = state.has_own(runtime);
        let remote = state.remote(runtime);
        let status = match found {
            Some(Found::Steam(_)) => t!("runtimes-from-steam"),
            Some(Found::Zendo { version, .. }) => match version {
                Some(version) => t!("runtimes-downloaded-on", date = day(version)),
                None => t!("runtimes-downloaded-undated"),
            },
            None => t!("runtimes-not-here"),
        };
        let mut label = column![
            text(runtime.label()).size(14),
            text(status).size(13).color(colors().dim),
        ]
        .spacing(2);
        if let Some(found) = found {
            label = label.push(
                text(found.path().display().to_string())
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(colors().dim),
            );
        }
        let mut line = row![label.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);

        // Download when Zendo has no copy, Update when Lutris has a newer
        // build than Zendo's; Steam's copy is Steam's to keep current.
        let offer = match remote {
            Some(_) if !own && !matches!(found, Some(Found::Steam(_))) => Some(t!("download")),
            Some(remote) if own && state.own_version(runtime) != Some(remote.version.as_str()) => {
                Some(t!("runtimes-update", date = day(&remote.version)))
            }
            _ => None,
        };
        if state.downloading == Some(runtime) {
            line = line.push(
                text(t!("runtimes-downloading"))
                    .size(13)
                    .color(colors().dim),
            );
        } else if let Some(offer) = offer {
            line = line.push(
                button(text(offer).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press_maybe(
                        state
                            .downloading
                            .is_none()
                            .then_some(Message::Runtime(RuntimeMessage::Download(runtime))),
                    ),
            );
        }
        if own && state.downloading != Some(runtime) {
            let confirming = state.confirm_remove == Some(runtime);
            line = line.push(
                button(
                    text(if confirming {
                        t!("really-remove")
                    } else {
                        t!("remove")
                    })
                    .size(13),
                )
                .padding([6, 12])
                .style(if confirming {
                    style::danger
                } else {
                    style::secondary
                })
                .on_press(Message::Runtime(RuntimeMessage::Remove(runtime))),
            );
        }
        line.into()
    }
}

/// The day of a Lutris build time: `2024-01-14` of
/// `2024-01-14T19:38:26.671443Z`.
fn day(version: &str) -> &str {
    version.get(..10).unwrap_or(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(runtime: Runtime) -> Remote {
        Remote {
            runtime,
            url: "https://example.com/eac_runtime.tar.xz".into(),
            version: "2024-01-14T19:38:26.671443Z".into(),
        }
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Latest(Ok(vec![
            remote(Runtime::Eac),
            remote(Runtime::BattlEye),
        ]))));
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Download(Runtime::Eac)));
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Download(
            Runtime::BattlEye,
        )));
        assert_eq!(zendo.runtimes.downloading, Some(Runtime::Eac));
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Done(
            Runtime::Eac,
            Err("could not reach github.com".into()),
        )));
        assert_eq!(zendo.runtimes.downloading, None);
        assert_eq!(
            zendo.runtimes.note,
            Some(("could not reach github.com".into(), false))
        );
    }

    #[test]
    fn needs_lutris_list_to_download() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Download(Runtime::Eac)));
        assert_eq!(zendo.runtimes.downloading, None);
    }

    #[test]
    fn remove_asks_once_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Remove(Runtime::BattlEye)));
        assert_eq!(zendo.runtimes.confirm_remove, Some(Runtime::BattlEye));
        assert!(zendo.runtimes.note.is_none());
        let _ = zendo.update(Message::Runtime(RuntimeMessage::Remove(Runtime::BattlEye)));
        assert_eq!(zendo.runtimes.confirm_remove, None);
        assert_eq!(
            zendo.runtimes.note,
            Some(("Removed Zendo's copy of the BattlEye runtime".into(), true))
        );
    }

    #[test]
    fn shows_the_day_of_a_build() {
        assert_eq!(day("2024-01-14T19:38:26.671443Z"), "2024-01-14");
        assert_eq!(day("soon"), "soon");
    }
}
