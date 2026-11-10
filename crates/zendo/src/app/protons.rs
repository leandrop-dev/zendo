//! The Proton section of Settings: the default Proton, the Protons found,
//! GE-Proton releases to download (the latest few, older ones on asking),
//! one at a time with its progress, and Remove for the ones in the user's
//! own tools folder.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, pick_list, progress_bar, row, text};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::Size;
use zendo_core::config::Config;
use zendo_core::ge_proton::{self, Release};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::proton::{self, Proton};

use super::settings::section;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// How many releases the section offers at first.
const SHOWN: usize = 5;
/// How many more each "Show older versions" adds.
const OLDER: usize = 10;

#[derive(Debug)]
pub struct Protons {
    installed: Vec<Proton>,
    /// The releases read so far, newest first.
    releases: Vec<Release>,
    /// Pages of GitHub's list read; 0 until asked.
    pages: u32,
    /// GitHub has pages after those.
    more: bool,
    /// How many releases are on show.
    shown: usize,
    error: Option<String>,
    asking: bool,
    download: Option<Download>,
    /// The Proton whose Remove was pressed once.
    confirm_remove: Option<String>,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
}

impl Default for Protons {
    fn default() -> Self {
        Self {
            installed: Vec::new(),
            releases: Vec::new(),
            pages: 0,
            more: false,
            shown: SHOWN,
            error: None,
            asking: false,
            download: None,
            confirm_remove: None,
            note: None,
        }
    }
}

/// An entry of the default Proton list: a build by name, or `None` for
/// Automatic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultChoice {
    name: Option<String>,
    label: String,
}

impl fmt::Display for DefaultChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug)]
struct Download {
    name: String,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub enum ProtonMessage {
    /// A page of GitHub's list, and whether more follow.
    Releases(u32, Result<(Vec<Release>, bool), String>),
    Refresh,
    ShowOlder,
    SetDefault(DefaultChoice),
    Download(Release),
    Progress(String, u64, u64),
    Installed(String, Result<(), String>),
    Cancel,
    Remove(String),
    /// Remove finished, in a thread: the name, and what came of it.
    Removed(String, Result<(), String>),
}

impl Zendo {
    /// Looks at the Protons on the machine, and asks GitHub for releases
    /// once a session.
    pub(super) fn want_protons(&mut self) -> Task<Message> {
        self.protons.installed = proton::installed();
        if !self.persist || self.protons.pages > 0 || self.protons.asking {
            return Task::none();
        }
        self.ask_page(1)
    }

    fn ask_page(&mut self, page: u32) -> Task<Message> {
        self.protons.asking = true;
        self.protons.error = None;
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || ge_proton::releases(page).map_err(|err| format!("{err:#}"))),
            move |result| Message::Proton(ProtonMessage::Releases(page, result)),
        )
    }

    pub(super) fn update_protons(&mut self, message: ProtonMessage) -> Task<Message> {
        let protons = &mut self.protons;
        match message {
            ProtonMessage::Releases(page, result) => {
                protons.asking = false;
                match result {
                    Ok((releases, more)) => {
                        if page == 1 {
                            protons.releases.clear();
                        }
                        protons.releases.extend(releases);
                        protons.pages = page;
                        protons.more = more;
                    }
                    Err(err) => protons.error = Some(err),
                }
            }
            ProtonMessage::Refresh => {
                protons.installed = proton::installed();
                protons.shown = SHOWN;
                if !protons.asking {
                    return self.ask_page(1);
                }
            }
            ProtonMessage::ShowOlder => {
                protons.shown += OLDER;
                if protons.shown > protons.releases.len() && protons.more && !protons.asking {
                    let next = protons.pages + 1;
                    return self.ask_page(next);
                }
            }
            ProtonMessage::SetDefault(choice) => return self.set_default_proton(choice.name),
            ProtonMessage::Download(release) => {
                if protons.download.is_some() {
                    return Task::none();
                }
                let stop = Arc::new(AtomicBool::new(false));
                protons.download = Some(Download {
                    name: release.name.clone(),
                    done: 0,
                    total: release.size,
                    stop: Arc::clone(&stop),
                });
                protons.note = None;
                if !self.persist {
                    return Task::none();
                }
                return Task::run(install(release, stop), Message::Proton);
            }
            ProtonMessage::Progress(name, done, total) => {
                if let Some(download) = protons.download.as_mut().filter(|d| d.name == name) {
                    download.done = done;
                    download.total = total;
                }
            }
            ProtonMessage::Installed(name, result) => {
                let stopped = protons
                    .download
                    .take()
                    .is_some_and(|download| download.stop.load(Ordering::Relaxed));
                protons.installed = proton::installed();
                protons.note = Some(match result {
                    Ok(()) => (t!("protons-installed-note", name = name.as_str()), true),
                    Err(_) if stopped => (t!("protons-stopped", name = name.as_str()), true),
                    Err(err) => (err, false),
                });
            }
            ProtonMessage::Cancel => {
                if let Some(download) = &protons.download {
                    download.stop.store(true, Ordering::Relaxed);
                }
            }
            ProtonMessage::Remove(name) => return self.remove_proton(name),
            ProtonMessage::Removed(name, result) => return self.proton_removed(&name, result),
        }
        Task::none()
    }

    /// Remove, pressed once to ask and again to delete.
    fn remove_proton(&mut self, name: String) -> Task<Message> {
        let protons = &mut self.protons;
        if protons.confirm_remove.as_deref() != Some(name.as_str()) {
            protons.confirm_remove = Some(name);
            return Task::none();
        }
        protons.confirm_remove = None;
        let Some(found) = protons.installed.iter().find(|p| p.name == name) else {
            return Task::none();
        };
        if !self.persist {
            return self.proton_removed(&name, Ok(()));
        }
        // A Proton tree is hundreds of megabytes, so it goes on a thread.
        let found = found.clone();
        Task::perform(
            blocking(move || ge_proton::remove(&found).map_err(|err| format!("{err:#}"))),
            move |result| Message::Proton(ProtonMessage::Removed(name.clone(), result)),
        )
    }

    /// What came of a Remove: the list reads again, and a default that is
    /// gone goes back to Automatic.
    fn proton_removed(&mut self, name: &str, result: Result<(), String>) -> Task<Message> {
        self.protons.installed = proton::installed();
        if let Err(err) = result {
            self.protons.note = Some((err, false));
            return Task::none();
        }
        // A default that is gone goes back to Automatic.
        if self.config.default_proton.as_deref() == Some(name) {
            let _ = self.set_default_proton(None);
            if self.protons.note.as_ref().is_some_and(|(_, good)| *good) {
                self.protons.note = Some((t!("protons-removed-default", name = name), true));
            }
        } else {
            self.protons.note = Some((t!("protons-removed", name = name), true));
        }
        Task::none()
    }

    /// Makes a Proton the default for games without one of their own, or
    /// goes back to Automatic with `None`, in `config.toml`.
    fn set_default_proton(&mut self, name: Option<String>) -> Task<Message> {
        self.config.default_proton = name;
        let setting = self.config.default_proton.clone();
        let saved = if self.persist {
            Config::update(|config| config.default_proton = setting).map(|_| ())
        } else {
            Ok(())
        };
        let shown = proton::default().map_or_else(|| t!("protons-umu"), |proton| proton.name);
        self.protons.note = Some(match saved {
            Ok(()) => (t!("protons-default-set", name = shown), true),
            Err(err) => (format!("{err:#}"), false),
        });
        Task::none()
    }

    /// Automatic, each Proton found, and the one chosen even when gone.
    fn default_choices(&self) -> Vec<DefaultChoice> {
        let automatic = match proton::automatic() {
            Some(first) => t!("protons-automatic", name = first.name.as_str()),
            None => t!("protons-automatic-umu"),
        };
        let mut choices = vec![DefaultChoice {
            name: None,
            label: automatic,
        }];
        choices.extend(self.protons.installed.iter().map(|found| DefaultChoice {
            name: Some(found.name.clone()),
            label: found.name.clone(),
        }));
        if let Some(chosen) = &self.config.default_proton
            && !choices
                .iter()
                .any(|choice| choice.name.as_ref() == Some(chosen))
        {
            choices.push(DefaultChoice {
                name: Some(chosen.clone()),
                label: t!("protons-missing", name = chosen.as_str()),
            });
        }
        choices
    }

    pub(super) fn protons_section(&self) -> Element<'_, Message> {
        let protons = &self.protons;
        let choices = self.default_choices();
        let selected = choices
            .iter()
            .find(|choice| choice.name == self.config.default_proton)
            .cloned();
        let mut body = column![
            text(t!("protons-default")).size(15).font(BOLD),
            text(t!("protons-default-about"))
                .size(13)
                .color(colors().dim),
            pick_list(choices, selected, |choice| {
                Message::Proton(ProtonMessage::SetDefault(choice))
            })
            .text_size(14)
            .padding([8, 12])
            .width(Length::Fill),
            text(t!("protons-here")).size(15).font(BOLD),
        ]
        .spacing(10);
        if protons.installed.is_empty() {
            body = body.push(text(t!("protons-none")).size(14).color(colors().dim));
        }
        for found in &protons.installed {
            body = body.push(self.installed_row(found));
        }

        body = body.push(
            row![
                text(t!("protons-releases")).size(15).font(BOLD),
                button(text(t!("protons-check-again")).size(13))
                    .padding([4, 10])
                    .style(style::secondary)
                    .on_press_maybe(
                        (!protons.asking).then_some(Message::Proton(ProtonMessage::Refresh))
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
        for release in protons.releases.iter().take(protons.shown) {
            body = body.push(self.release_row(release));
        }
        if protons.asking {
            body = body.push(text(t!("protons-asking")).size(14).color(colors().dim));
        } else if let Some(err) = &protons.error {
            body = body.push(text(err).size(13).color(colors().bad));
        } else if protons.releases.len() > protons.shown || protons.more {
            body = body.push(
                button(text(t!("protons-show-older")).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::Proton(ProtonMessage::ShowOlder)),
            );
        }
        if let Some(download) = &protons.download {
            body = body.push(download_bar(download));
        }
        if let Some((note, good)) = &protons.note {
            body = body.push(text(note).size(13).color(if *good {
                colors().good
            } else {
                colors().bad
            }));
        }
        section("Proton", t!("protons-about"), body)
    }

    fn installed_row<'a>(&'a self, found: &'a Proton) -> Element<'a, Message> {
        let removable = ge_proton::removable(found);
        let confirming = self.protons.confirm_remove.as_deref() == Some(found.name.as_str());
        let mut label = column![text(&found.name).size(14)].spacing(2);
        label = label.push(
            text(found.path.display().to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .color(colors().dim),
        );
        let users = if confirming {
            games_using(&found.name)
        } else {
            0
        };
        if users > 0 {
            label = label.push(
                text(t!("protons-chosen-by", count = users))
                    .size(12)
                    .color(colors().bad),
            );
        }
        let mut line = row![label.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);
        if removable {
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
                .on_press(Message::Proton(ProtonMessage::Remove(found.name.clone()))),
            );
        }
        line.into()
    }

    fn release_row<'a>(&'a self, release: &'a Release) -> Element<'a, Message> {
        let installed = self
            .protons
            .installed
            .iter()
            .any(|found| ge_proton::is_release(found, release));
        let action: Element<'a, Message> = if installed {
            text(t!("installed")).size(13).color(colors().good).into()
        } else {
            button(text(t!("download")).size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press_maybe(
                    self.protons
                        .download
                        .is_none()
                        .then(|| Message::Proton(ProtonMessage::Download(release.clone()))),
                )
                .into()
        };
        row![
            text(&release.name).size(14).width(200),
            text(&release.published)
                .size(13)
                .color(colors().dim)
                .width(110),
            text(Size::Bytes(release.size).to_string())
                .size(13)
                .color(colors().dim)
                .width(Length::Fill),
            action,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    }
}

/// How many games' launch options chose this Proton.
fn games_using(name: &str) -> usize {
    LaunchOptions::load_all().map_or(0, |all| {
        all.values()
            .filter(|options| options.proton.as_deref() == Some(name))
            .count()
    })
}

fn download_bar(download: &Download) -> Element<'_, Message> {
    #[expect(clippy::cast_precision_loss, reason = "a bar's width")]
    let percent = if download.total == 0 {
        0.0
    } else {
        download.done as f32 * 100.0 / download.total as f32
    };
    let stopping = download.stop.load(Ordering::Relaxed);
    column![
        row![
            text(t!("protons-downloading", name = download.name.as_str()))
                .size(14)
                .width(Length::Fill),
            text(t!(
                "size-of",
                done = Size::Bytes(download.done).to_string(),
                total = Size::Bytes(download.total).to_string()
            ))
            .size(13)
            .color(colors().dim),
            button(text(if stopping { t!("stopping") } else { t!("stop") }).size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press_maybe((!stopping).then_some(Message::Proton(ProtonMessage::Cancel))),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        progress_bar(0.0..=100.0, percent)
            .girth(8)
            .style(style::progress),
    ]
    .spacing(8)
    .into()
}

/// Downloads and unpacks a release on a thread, saying how far it got.
fn install(release: Release, stop: Arc<AtomicBool>) -> impl Stream<Item = ProtonMessage> {
    iced::stream::channel(16, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || {
            let name = release.name.clone();
            let progress = sender.clone();
            let result = ge_proton::install(&release, &stop, &mut |done, total| {
                let _ = progress.unbounded_send(ProtonMessage::Progress(name.clone(), done, total));
            });
            let _ = sender.unbounded_send(ProtonMessage::Installed(
                name,
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

    fn release(name: &str) -> Release {
        Release {
            name: name.into(),
            published: "2026-11-03".into(),
            tarball: String::new(),
            size: 1000,
            checksum: None,
        }
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Proton(ProtonMessage::Download(release(
            "GE-Proton11-12",
        ))));
        let _ = zendo.update(Message::Proton(ProtonMessage::Download(release(
            "GE-Proton11-11",
        ))));
        assert_eq!(
            zendo.protons.download.as_ref().map(|d| d.name.as_str()),
            Some("GE-Proton11-12")
        );
        let _ = zendo.update(Message::Proton(ProtonMessage::Progress(
            "GE-Proton11-12".into(),
            500,
            1000,
        )));
        assert_eq!(zendo.protons.download.as_ref().map(|d| d.done), Some(500));
        let _ = zendo.update(Message::Proton(ProtonMessage::Cancel));
        let _ = zendo.update(Message::Proton(ProtonMessage::Installed(
            "GE-Proton11-12".into(),
            Err("stopped".into()),
        )));
        assert!(zendo.protons.download.is_none());
        assert_eq!(
            zendo.protons.note,
            Some(("Stopped downloading GE-Proton11-12".into(), true))
        );
        let _ = zendo.update(Message::Proton(ProtonMessage::Download(release(
            "GE-Proton11-11",
        ))));
        let _ = zendo.update(Message::Proton(ProtonMessage::Installed(
            "GE-Proton11-11".into(),
            Err("the download of GE-Proton11-11 does not match its checksum".into()),
        )));
        assert_eq!(
            zendo.protons.note,
            Some((
                "the download of GE-Proton11-11 does not match its checksum".into(),
                false
            ))
        );
    }

    #[test]
    fn remove_asks_once_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        zendo.protons.installed = vec![Proton {
            name: "GE-Proton11-12".into(),
            path: "/nowhere/GE-Proton11-12".into(),
        }];
        let _ = zendo.update(Message::Proton(ProtonMessage::Remove(
            "GE-Proton11-12".into(),
        )));
        assert_eq!(
            zendo.protons.confirm_remove.as_deref(),
            Some("GE-Proton11-12")
        );
        assert!(zendo.protons.note.is_none());
    }

    #[test]
    fn shows_older_versions_a_few_at_a_time() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let page = |from: usize, to: usize| {
            (from..to)
                .map(|n| release(&format!("GE-Proton11-{n}")))
                .collect::<Vec<_>>()
        };
        let _ = zendo.update(Message::Proton(ProtonMessage::Releases(
            1,
            Ok((page(0, 12), true)),
        )));
        assert_eq!(zendo.protons.shown, SHOWN);
        let _ = zendo.update(Message::Proton(ProtonMessage::ShowOlder));
        assert_eq!(zendo.protons.shown, SHOWN + OLDER);
        assert!(
            zendo.protons.asking,
            "past what was read, the next page is asked"
        );
        let _ = zendo.update(Message::Proton(ProtonMessage::Releases(
            2,
            Ok((page(12, 20), false)),
        )));
        assert_eq!(zendo.protons.releases.len(), 20);
        assert_eq!(zendo.protons.pages, 2);
        assert!(!zendo.protons.more);
    }

    #[test]
    fn sets_the_default_proton() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let choice = DefaultChoice {
            name: Some("GE-Proton11-12".into()),
            label: "GE-Proton11-12".into(),
        };
        let _ = zendo.update(Message::Proton(ProtonMessage::SetDefault(choice)));
        assert_eq!(
            zendo.config.default_proton.as_deref(),
            Some("GE-Proton11-12")
        );
        assert!(zendo.protons.note.as_ref().is_some_and(|(_, good)| *good));
        let choices = zendo.default_choices();
        assert_eq!(choices[0].name, None, "Automatic comes first");
    }
}
