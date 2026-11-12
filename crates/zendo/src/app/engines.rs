//! The Engines section of Settings: legendary, gogdl, nile, comet and
//! umu-launcher, each with the copy found now and whose it is, Zendo's own
//! release if it has one, and the latest on GitHub (asked once a session,
//! and again on asking). Download, Update and Remove work on Zendo's own
//! folder, one download at a time. Where each is found is kept in state and
//! read again when the section opens and after a change, never in `view`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, row, text, toggler};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::engines::{self, Engine, Located, Origin, Release};

use super::settings::{Setting, download_bar, section};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// What is known of the latest release of an engine.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Latest {
    Asking,
    Known(Release),
    Failed(String),
}

/// An engine as the section shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    engine: Engine,
    /// The copy in use, whoever's it is.
    found: Option<Located>,
    /// The release of Zendo's own copy.
    installed: Option<String>,
}

#[derive(Debug)]
struct Download {
    engine: Engine,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
pub struct Engines {
    rows: Vec<Row>,
    latest: HashMap<Engine, Latest>,
    /// Asked this session.
    asked: bool,
    download: Option<Download>,
    /// The engine whose Remove was pressed once.
    confirm_remove: Option<Engine>,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
}

#[derive(Debug, Clone)]
pub enum EngineMessage {
    Latest(Engine, Result<Release, String>),
    CheckAgain,
    Download(Engine),
    Progress(Engine, u64, u64),
    Installed(Engine, Result<(), String>),
    Cancel,
    Remove(Engine),
    Removed(Engine, Result<(), String>),
    PathFirst(bool),
}

impl Engines {
    /// Looks again at where each engine is found.
    pub(super) fn refresh(&mut self) {
        self.rows = Engine::ALL
            .into_iter()
            .map(|engine| Row {
                engine,
                found: engines::find_with_origin(engine.program()),
                installed: engine.installed(),
            })
            .collect();
    }
}

impl Zendo {
    /// Looks at the engines on the machine as Settings opens, and asks
    /// GitHub for the latest releases once a session.
    pub(super) fn want_engines(&mut self) -> Task<Message> {
        self.engines.refresh();
        if !self.persist || self.engines.asked {
            return Task::none();
        }
        self.ask_latest()
    }

    fn ask_latest(&mut self) -> Task<Message> {
        self.engines.asked = true;
        let tasks = Engine::ALL.into_iter().map(|engine| {
            self.engines.latest.insert(engine, Latest::Asking);
            Task::perform(
                blocking(move || engines::latest(engine).map_err(|err| format!("{err:#}"))),
                move |result| Message::Engine(EngineMessage::Latest(engine, result)),
            )
        });
        Task::batch(tasks.collect::<Vec<_>>())
    }

    /// Whether replacing or deleting an engine could pull the program out
    /// from under a game (umu-run lives as long as one) or a store job.
    fn engines_busy(&self) -> bool {
        self.running.busy() || self.downloads.is_running()
    }

    pub(super) fn update_engines(&mut self, message: EngineMessage) -> Task<Message> {
        let state = &mut self.engines;
        match message {
            EngineMessage::Latest(engine, result) => {
                state.latest.insert(
                    engine,
                    match result {
                        Ok(release) => Latest::Known(release),
                        Err(err) => Latest::Failed(err),
                    },
                );
            }
            EngineMessage::CheckAgain => return self.ask_latest(),
            EngineMessage::Download(engine) => {
                if state.download.is_some() {
                    return Task::none();
                }
                let Some(Latest::Known(release)) = state.latest.get(&engine).cloned() else {
                    return Task::none();
                };
                if state
                    .rows
                    .iter()
                    .any(|r| r.engine == engine && r.installed.is_some())
                    && self.engines_busy()
                {
                    return Task::none();
                }
                let stop = Arc::new(AtomicBool::new(false));
                let state = &mut self.engines;
                state.download = Some(Download {
                    engine,
                    done: 0,
                    total: release.asset.size,
                    stop: Arc::clone(&stop),
                });
                state.note = None;
                if !self.persist {
                    return Task::none();
                }
                return Task::run(install(engine, release, stop), Message::Engine);
            }
            EngineMessage::Progress(engine, done, total) => {
                if let Some(download) = state.download.as_mut().filter(|d| d.engine == engine) {
                    download.done = done;
                    download.total = total;
                }
            }
            EngineMessage::Installed(engine, result) => {
                let stopped = state
                    .download
                    .take()
                    .is_some_and(|download| download.stop.load(Ordering::Relaxed));
                state.refresh();
                state.note = Some(match result {
                    Ok(()) => (
                        t!(
                            "engines-installed-note",
                            name = engine.label(),
                            tag = engine.installed().unwrap_or_default()
                        ),
                        true,
                    ),
                    Err(_) if stopped => (t!("protons-stopped", name = engine.label()), true),
                    Err(err) => (err, false),
                });
            }
            EngineMessage::Cancel => {
                if let Some(download) = &state.download {
                    download.stop.store(true, Ordering::Relaxed);
                }
            }
            EngineMessage::Remove(engine) => return self.remove_engine(engine),
            EngineMessage::Removed(engine, result) => {
                state.refresh();
                state.note = Some(match result {
                    Ok(()) => (t!("engines-removed-note", name = engine.label()), true),
                    Err(err) => (err, false),
                });
            }
            EngineMessage::PathFirst(on) => {
                return self.save_setting(Setting::EnginesPathFirst(on));
            }
        }
        Task::none()
    }

    /// Remove, pressed once to ask and again to delete.
    fn remove_engine(&mut self, engine: Engine) -> Task<Message> {
        if self.engines_busy() {
            return Task::none();
        }
        let state = &mut self.engines;
        if state.confirm_remove != Some(engine) {
            state.confirm_remove = Some(engine);
            return Task::none();
        }
        state.confirm_remove = None;
        if !self.persist {
            return self.update_engines(EngineMessage::Removed(engine, Ok(())));
        }
        Task::perform(
            blocking(move || engines::remove(engine).map_err(|err| format!("{err:#}"))),
            move |result| Message::Engine(EngineMessage::Removed(engine, result)),
        )
    }

    pub(super) fn engines_section(&self) -> Element<'_, Message> {
        let state = &self.engines;
        let busy = self.engines_busy();
        let mut body = column![
            toggler(self.config.engines_on_path_first)
                .label(t!("engines-path-first"))
                .text_size(14)
                .on_toggle(|on| Message::Engine(EngineMessage::PathFirst(on))),
            button(text(t!("protons-check-again")).size(13))
                .padding([4, 10])
                .style(style::secondary)
                .on_press(Message::Engine(EngineMessage::CheckAgain)),
        ]
        .spacing(10);
        for row in &state.rows {
            body = body.push(self.engine_row(row, busy));
        }
        if let Some(download) = &state.download {
            body = body.push(download_bar(
                download.engine.label(),
                download.done,
                download.total,
                download.stop.load(Ordering::Relaxed),
                Message::Engine(EngineMessage::Cancel),
            ));
        }
        if let Some((note, good)) = &state.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        section(t!("engines-title"), t!("engines-about"), body)
    }

    #[expect(clippy::too_many_lines, reason = "one row of buttons and notes")]
    fn engine_row<'a>(&'a self, row_state: &'a Row, busy: bool) -> Element<'a, Message> {
        let state = &self.engines;
        let engine = row_state.engine;
        let whose = match row_state.found.as_ref().map(|found| found.origin) {
            Some(Origin::Zendo) => t!("engines-found-zendo"),
            Some(Origin::Path) => t!("engines-found-path"),
            Some(Origin::Heroic) => t!("engines-found-heroic"),
            None => t!("engines-missing"),
        };
        let mut info = column![
            row![
                text(engine.label()).size(14).font(BOLD),
                text(engine.purpose()).size(13).color(colors().dim),
                text(whose).size(13).color(colors().dim),
            ]
            .spacing(12)
            .wrap(),
        ]
        .spacing(2);
        if let Some(found) = &row_state.found {
            info = info.push(
                text(found.path.display().to_string())
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(colors().dim),
            );
        }
        let latest = state.latest.get(&engine);
        let latest_line = match latest {
            Some(Latest::Asking) => Some((t!("protons-asking"), false)),
            Some(Latest::Known(release)) => {
                Some((t!("engines-latest", tag = release.tag.as_str()), false))
            }
            Some(Latest::Failed(err)) => Some((err.clone(), true)),
            None => None,
        };
        if let Some((line, bad)) = latest_line {
            info =
                info.push(
                    text(line)
                        .size(12)
                        .color(if bad { colors().bad } else { colors().dim }),
                );
        }
        let downloading = state.download.is_some();
        let mut line = row![info.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);
        let known = match latest {
            Some(Latest::Known(release)) => Some(release),
            _ => None,
        };
        match (&row_state.installed, known) {
            (Some(tag), Some(release)) if &release.tag != tag => {
                line = line.push(
                    button(text(t!("engines-update", tag = release.tag.as_str())).size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press_maybe(
                            (!downloading && !busy)
                                .then(|| Message::Engine(EngineMessage::Download(engine))),
                        ),
                );
            }
            (Some(_), Some(_)) => {
                line = line.push(text(t!("engines-up-to-date")).size(13).color(colors().good));
            }
            (None, Some(release)) => {
                line = line.push(
                    button(text(t!("engines-download-tag", tag = release.tag.as_str())).size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press_maybe(
                            (!downloading)
                                .then(|| Message::Engine(EngineMessage::Download(engine))),
                        ),
                );
            }
            _ => {}
        }
        if row_state.installed.is_some() {
            let confirming = state.confirm_remove == Some(engine);
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
                .on_press_maybe((!busy).then(|| Message::Engine(EngineMessage::Remove(engine)))),
            );
        }
        if busy && row_state.installed.is_some() {
            return column![line, text(t!("engines-busy")).size(12).color(colors().dim)]
                .spacing(4)
                .into();
        }
        line.into()
    }
}

/// Downloads and installs a release on a thread, saying how far it got.
fn install(
    engine: Engine,
    release: Release,
    stop: Arc<AtomicBool>,
) -> impl Stream<Item = EngineMessage> {
    iced::stream::channel(16, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            let progress = sender.clone();
            let result = engines::install(engine, &release, &stop, &mut |done, total| {
                let _ = progress.unbounded_send(EngineMessage::Progress(engine, done, total));
            });
            let _ = sender.unbounded_send(EngineMessage::Installed(
                engine,
                result.map(|_| ()).map_err(|err| format!("{err:#}")),
            ));
        });
        while let Some(message) = receiver.next().await {
            if output.send(message).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::engines::Asset;

    fn release(tag: &str) -> Release {
        Release {
            tag: tag.into(),
            asset: Asset {
                name: "legendary_linux_x64".into(),
                url: String::new(),
                size: 1000,
                sha256: "0".repeat(64),
            },
            service: None,
        }
    }

    fn known(zendo: &mut Zendo, engine: Engine) {
        let _ = zendo.update(Message::Engine(EngineMessage::Latest(
            engine,
            Ok(release("0.21.1")),
        )));
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        // Nothing is downloaded before the latest release is known.
        let _ = zendo.update(Message::Engine(EngineMessage::Download(Engine::Legendary)));
        assert!(zendo.engines.download.is_none());
        known(&mut zendo, Engine::Legendary);
        known(&mut zendo, Engine::Nile);
        let _ = zendo.update(Message::Engine(EngineMessage::Download(Engine::Legendary)));
        let _ = zendo.update(Message::Engine(EngineMessage::Download(Engine::Nile)));
        assert_eq!(
            zendo.engines.download.as_ref().map(|d| d.engine),
            Some(Engine::Legendary)
        );
        let _ = zendo.update(Message::Engine(EngineMessage::Progress(
            Engine::Legendary,
            500,
            1000,
        )));
        assert_eq!(zendo.engines.download.as_ref().map(|d| d.done), Some(500));
        let _ = zendo.update(Message::Engine(EngineMessage::Cancel));
        let _ = zendo.update(Message::Engine(EngineMessage::Installed(
            Engine::Legendary,
            Err("stopped".into()),
        )));
        assert!(zendo.engines.download.is_none());
        assert_eq!(
            zendo.engines.note,
            Some(("Stopped downloading legendary".into(), true))
        );
        let _ = zendo.update(Message::Engine(EngineMessage::Download(Engine::Nile)));
        let _ = zendo.update(Message::Engine(EngineMessage::Installed(
            Engine::Nile,
            Err("the download of nile does not match its checksum".into()),
        )));
        assert_eq!(
            zendo.engines.note,
            Some((
                "the download of nile does not match its checksum".into(),
                false
            ))
        );
    }

    #[test]
    fn remove_asks_once_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Engine(EngineMessage::Remove(Engine::Comet)));
        assert_eq!(zendo.engines.confirm_remove, Some(Engine::Comet));
        assert!(zendo.engines.note.is_none());
        let _ = zendo.update(Message::Engine(EngineMessage::Remove(Engine::Comet)));
        assert_eq!(zendo.engines.confirm_remove, None);
        assert_eq!(
            zendo.engines.note,
            Some(("Removed Zendo's copy of comet".into(), true))
        );
    }

    #[test]
    fn keeps_what_it_learns_of_each_engines_latest_release() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        known(&mut zendo, Engine::Legendary);
        let _ = zendo.update(Message::Engine(EngineMessage::Latest(
            Engine::Gogdl,
            Err("could not reach GitHub".into()),
        )));
        assert!(matches!(
            zendo.engines.latest.get(&Engine::Legendary),
            Some(Latest::Known(release)) if release.tag == "0.21.1"
        ));
        assert!(matches!(
            zendo.engines.latest.get(&Engine::Gogdl),
            Some(Latest::Failed(_))
        ));
    }
}
