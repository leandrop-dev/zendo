//! The Proton section of Settings: the Protons found, the latest GE-Proton
//! releases to download, one at a time with its progress, and Remove for
//! the ones in the user's own tools folder.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, progress_bar, row, text};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::Size;
use zendo_core::ge_proton::{self, Release};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::proton::{self, Proton};

use super::settings::section;
use super::style::{self, BAD, BOLD, DIM, GOOD};
use super::{Message, Zendo, blocking};

/// How many releases the section offers.
const SHOWN: usize = 5;

#[derive(Debug, Default)]
pub struct Protons {
    installed: Vec<Proton>,
    /// `None` until asked.
    releases: Option<Result<Vec<Release>, String>>,
    asking: bool,
    download: Option<Download>,
    /// The Proton whose Remove was pressed once.
    confirm_remove: Option<String>,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
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
    Releases(Result<Vec<Release>, String>),
    Refresh,
    Download(Release),
    Progress(String, u64, u64),
    Installed(String, Result<(), String>),
    Cancel,
    Remove(String),
}

impl Zendo {
    /// Looks at the Protons on the machine, and asks GitHub for releases
    /// once a session.
    pub(super) fn want_protons(&mut self) -> Task<Message> {
        self.protons.installed = proton::installed();
        if !self.persist || self.protons.releases.is_some() || self.protons.asking {
            return Task::none();
        }
        self.ask_releases()
    }

    fn ask_releases(&mut self) -> Task<Message> {
        self.protons.asking = true;
        Task::perform(
            blocking(|| ge_proton::releases().map_err(|err| format!("{err:#}"))),
            |result| Message::Proton(ProtonMessage::Releases(result)),
        )
    }

    pub(super) fn update_protons(&mut self, message: ProtonMessage) -> Task<Message> {
        let protons = &mut self.protons;
        match message {
            ProtonMessage::Releases(result) => {
                protons.asking = false;
                protons.releases = Some(result);
            }
            ProtonMessage::Refresh => {
                protons.installed = proton::installed();
                if !protons.asking && self.persist {
                    return self.ask_releases();
                }
            }
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
                    Ok(()) => (
                        format!("{name} is installed; launch options can choose it"),
                        true,
                    ),
                    Err(_) if stopped => (format!("Stopped downloading {name}"), true),
                    Err(err) => (err, false),
                });
            }
            ProtonMessage::Cancel => {
                if let Some(download) = &protons.download {
                    download.stop.store(true, Ordering::Relaxed);
                }
            }
            ProtonMessage::Remove(name) => {
                if protons.confirm_remove.as_deref() != Some(name.as_str()) {
                    protons.confirm_remove = Some(name);
                    return Task::none();
                }
                protons.confirm_remove = None;
                let Some(found) = protons.installed.iter().find(|p| p.name == name) else {
                    return Task::none();
                };
                let result = if self.persist {
                    ge_proton::remove(found)
                } else {
                    Ok(())
                };
                protons.note = Some(match result {
                    Ok(()) => (format!("Removed {name}"), true),
                    Err(err) => (format!("{err:#}"), false),
                });
                protons.installed = proton::installed();
            }
        }
        Task::none()
    }

    pub(super) fn protons_section(&self) -> Element<'_, Message> {
        let protons = &self.protons;
        let mut body = column![text("On this computer").size(15).font(BOLD)].spacing(10);
        if protons.installed.is_empty() {
            body = body.push(
                text("None yet: umu downloads UMU-Proton the first time a Windows game starts.")
                    .size(14)
                    .color(DIM),
            );
        }
        for found in &protons.installed {
            body = body.push(self.installed_row(found));
        }

        body = body.push(
            row![
                text("GE-Proton releases").size(15).font(BOLD),
                button(text("Check again").size(13))
                    .padding([4, 10])
                    .style(style::secondary)
                    .on_press_maybe(
                        (!protons.asking).then_some(Message::Proton(ProtonMessage::Refresh))
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
        match &protons.releases {
            None => body = body.push(text("Asking GitHub…").size(14).color(DIM)),
            Some(Err(err)) => body = body.push(text(err).size(13).color(BAD)),
            Some(Ok(releases)) => {
                for release in releases.iter().take(SHOWN) {
                    body = body.push(self.release_row(release));
                }
            }
        }
        if let Some(download) = &protons.download {
            body = body.push(download_bar(download));
        }
        if let Some((note, good)) = &protons.note {
            body = body.push(text(note).size(13).color(if *good { GOOD } else { BAD }));
        }
        section(
            "Proton",
            "Runs the Windows games Zendo installed; each game's launch options choose one. \
             GE-Proton carries the most fixes, and goes into Steam's compatibility tools \
             folder, where Steam and Heroic find it too.",
            body,
        )
    }

    fn installed_row<'a>(&'a self, found: &'a Proton) -> Element<'a, Message> {
        let removable = ge_proton::removable(found);
        let confirming = self.protons.confirm_remove.as_deref() == Some(found.name.as_str());
        let mut label = column![text(&found.name).size(14)].spacing(2);
        label = label.push(
            text(found.path.display().to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .color(DIM),
        );
        let users = if confirming {
            games_using(&found.name)
        } else {
            0
        };
        if users > 0 {
            label = label.push(
                text(format!(
                    "{} chose it; they will run with the default",
                    if users == 1 {
                        "1 game".to_owned()
                    } else {
                        format!("{users} games")
                    }
                ))
                .size(12)
                .color(BAD),
            );
        }
        let mut line = row![label.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);
        if removable {
            line = line.push(
                button(
                    text(if confirming {
                        "Really remove"
                    } else {
                        "Remove"
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
            text("Installed").size(13).color(GOOD).into()
        } else {
            button(text("Download").size(13))
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
            text(&release.published).size(13).color(DIM).width(110),
            text(Size::Bytes(release.size).to_string())
                .size(13)
                .color(DIM)
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
            text(format!("Downloading {}", download.name))
                .size(14)
                .width(Length::Fill),
            text(format!(
                "{} of {}",
                Size::Bytes(download.done),
                Size::Bytes(download.total)
            ))
            .size(13)
            .color(DIM),
            button(text(if stopping { "Stopping…" } else { "Stop" }).size(13))
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
}
