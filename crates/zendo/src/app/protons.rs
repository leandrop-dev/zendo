//! The Proton section of Settings: the default Proton, the Protons found,
//! GE-Proton or Proton-CachyOS releases to download (the latest few, older
//! ones on asking), one at a time with its progress, and Remove for the
//! ones in the user's own tools folder.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, pick_list, row, text};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::Size;
use zendo_core::ge_proton::{self, Build, Release};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::proton::{self, Proton};

use super::settings::{download_bar, section};
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

/// How many releases the section offers at first.
const SHOWN: usize = 5;
/// How many more each "Show older versions" adds.
const OLDER: usize = 10;

/// What is known of one build's releases.
#[derive(Debug)]
struct List {
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
}

impl Default for List {
    fn default() -> Self {
        Self {
            releases: Vec::new(),
            pages: 0,
            more: false,
            shown: SHOWN,
            error: None,
            asking: false,
        }
    }
}

fn slot(build: Build) -> usize {
    match build {
        Build::Ge => 0,
        Build::CachyOs => 1,
    }
}

#[derive(Debug, Default)]
pub struct Protons {
    installed: Vec<Proton>,
    /// The build whose releases are on show.
    build: Build,
    /// Each build's releases, by [`slot`].
    lists: [List; 2],
    download: Option<Download>,
    /// The Proton whose Remove was pressed once, and the games using it,
    /// counted then rather than in `view`.
    confirm_remove: Option<(String, usize)>,
    /// The last thing that happened, and whether it went well.
    note: Option<(String, bool)>,
}

impl Protons {
    fn list(&self) -> &List {
        &self.lists[slot(self.build)]
    }

    fn list_mut(&mut self, build: Build) -> &mut List {
        &mut self.lists[slot(build)]
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
    /// A page of a build's GitHub list, and whether more follow.
    Releases(Build, u32, Result<(Vec<Release>, bool), String>),
    Refresh,
    ShowOlder,
    /// Which build's releases to show.
    SetBuild(Build),
    SetDefault(DefaultChoice),
    Download(Release),
    Progress(String, u64, u64),
    Installed(String, Result<(), String>),
    Cancel,
    Remove(String),
    /// Remove finished, in a thread: the name, and what came of it.
    Removed(String, Result<(), String>),
}

impl Protons {
    /// Says which Proton games without one of their own use now, once the
    /// choice is in `config.toml`.
    pub(super) fn default_set(&mut self) {
        let shown = proton::default().map_or_else(|| t!("protons-umu"), |proton| proton.name);
        self.note = Some((t!("protons-default-set", name = shown), true));
    }
}

impl Zendo {
    /// Looks at the Protons on the machine, and asks GitHub for releases
    /// once a session.
    pub(super) fn want_protons(&mut self) -> Task<Message> {
        self.protons.installed = proton::installed();
        let list = self.protons.list();
        if !self.persist || list.pages > 0 || list.asking {
            return Task::none();
        }
        self.ask_page(self.protons.build, 1)
    }

    fn ask_page(&mut self, build: Build, page: u32) -> Task<Message> {
        let list = self.protons.list_mut(build);
        list.asking = true;
        list.error = None;
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || ge_proton::releases(build, page).map_err(|err| format!("{err:#}"))),
            move |result| Message::Proton(ProtonMessage::Releases(build, page, result)),
        )
    }

    pub(super) fn update_protons(&mut self, message: ProtonMessage) -> Task<Message> {
        let protons = &mut self.protons;
        match message {
            ProtonMessage::Releases(build, page, result) => {
                let list = protons.list_mut(build);
                list.asking = false;
                match result {
                    Ok((releases, more)) => {
                        if page == 1 {
                            list.releases.clear();
                        }
                        list.releases.extend(releases);
                        list.pages = page;
                        list.more = more;
                    }
                    Err(err) => list.error = Some(err),
                }
            }
            ProtonMessage::Refresh => {
                protons.installed = proton::installed();
                let build = protons.build;
                let list = protons.list_mut(build);
                list.shown = SHOWN;
                if !list.asking {
                    return self.ask_page(build, 1);
                }
            }
            ProtonMessage::ShowOlder => {
                let build = protons.build;
                let list = protons.list_mut(build);
                list.shown += OLDER;
                if list.shown > list.releases.len() && list.more && !list.asking {
                    let next = list.pages + 1;
                    return self.ask_page(build, next);
                }
            }
            ProtonMessage::SetBuild(build) => {
                protons.build = build;
                let list = protons.list();
                if list.pages == 0 && !list.asking {
                    return self.ask_page(build, 1);
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
        if protons
            .confirm_remove
            .as_ref()
            .map(|(chosen, _)| chosen.as_str())
            != Some(name.as_str())
        {
            // Counted once, here: the view must not read launch options.
            let users = games_using(&name);
            protons.confirm_remove = Some((name, users));
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
        // Games that named it go back to Automatic, and the default too,
        // or they would launch with a Proton that is gone.
        let reset = reset_games_using(name);
        if self.config.default_proton.as_deref() == Some(name) {
            let _ = self.set_default_proton(None);
            self.protons.note = Some((t!("protons-removed-default", name = name), true));
        } else if reset > 0 {
            self.protons.note = Some((t!("protons-reset", name = name, count = reset), true));
        } else {
            self.protons.note = Some((t!("protons-removed", name = name), true));
        }
        Task::none()
    }

    /// Makes a Proton the default for games without one of their own, or
    /// goes back to Automatic with `None`, in `config.toml`.
    fn set_default_proton(&mut self, name: Option<String>) -> Task<Message> {
        self.save_setting(super::settings::Setting::DefaultProton(name))
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

        let chips = Build::ALL
            .into_iter()
            .fold(row![].spacing(6), |chips, build| {
                chips.push(
                    button(text(build.label()).size(13))
                        .padding([4, 10])
                        .style(if build == protons.build {
                            style::primary
                        } else {
                            style::secondary
                        })
                        .on_press(Message::Proton(ProtonMessage::SetBuild(build))),
                )
            });
        let list = protons.list();
        body = body.push(
            row![
                text(t!("protons-releases", build = protons.build.label()))
                    .size(15)
                    .font(BOLD),
                chips,
                button(text(t!("protons-check-again")).size(13))
                    .padding([4, 10])
                    .style(style::secondary)
                    .on_press_maybe(
                        (!list.asking).then_some(Message::Proton(ProtonMessage::Refresh))
                    ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
        for release in list.releases.iter().take(list.shown) {
            body = body.push(self.release_row(release));
        }
        if list.asking {
            body = body.push(text(t!("protons-asking")).size(14).color(colors().dim));
        } else if let Some(err) = &list.error {
            body = body.push(text(err).size(13).color(colors().bad));
        } else if list.releases.len() > list.shown || list.more {
            body = body.push(
                button(text(t!("protons-show-older")).size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press(Message::Proton(ProtonMessage::ShowOlder)),
            );
        }
        if let Some(download) = &protons.download {
            body = body.push(download_bar(
                &download.name,
                download.done,
                download.total,
                download.stop.load(Ordering::Relaxed),
                Message::Proton(ProtonMessage::Cancel),
            ));
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
        let (confirming, users) = match &self.protons.confirm_remove {
            Some((chosen, users)) if chosen == &found.name => (true, *users),
            _ => (false, 0),
        };
        let mut label = column![text(&found.name).size(14)].spacing(2);
        label = label.push(
            text(found.path.display().to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .color(colors().dim),
        );
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
        // Natural widths: a `Fill` cell and `wrap` put the button on a
        // line of its own even when there is room.
        row![
            text(&release.name).size(14),
            text(&release.published).size(13).color(colors().dim),
            text(Size::Bytes(release.size).to_string())
                .size(13)
                .color(colors().dim),
            action,
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .wrap()
        .into()
    }
}

/// How many games' launch options chose this Proton.
/// Puts every game that chose this Proton back to Automatic, and says how
/// many.
fn reset_games_using(name: &str) -> usize {
    let Ok(all) = LaunchOptions::load_all() else {
        return 0;
    };
    let mut reset = 0;
    for (id, mut options) in all {
        if options.proton.as_deref() == Some(name) {
            options.proton = None;
            if options.save(&id).is_ok() {
                reset += 1;
            }
        }
    }
    reset
}

fn games_using(name: &str) -> usize {
    LaunchOptions::load_all().map_or(0, |all| {
        all.values()
            .filter(|options| options.proton.as_deref() == Some(name))
            .count()
    })
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
            build: Build::Ge,
            name: name.into(),
            published: "2026-11-03".into(),
            file: format!("{name}.tar.gz"),
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
            zendo
                .protons
                .confirm_remove
                .as_ref()
                .map(|(name, _)| name.as_str()),
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
            Build::Ge,
            1,
            Ok((page(0, 12), true)),
        )));
        assert_eq!(zendo.protons.list().shown, SHOWN);
        let _ = zendo.update(Message::Proton(ProtonMessage::ShowOlder));
        assert_eq!(zendo.protons.list().shown, SHOWN + OLDER);
        assert!(
            zendo.protons.list().asking,
            "past what was read, the next page is asked"
        );
        let _ = zendo.update(Message::Proton(ProtonMessage::Releases(
            Build::Ge,
            2,
            Ok((page(12, 20), false)),
        )));
        assert_eq!(zendo.protons.list().releases.len(), 20);
        assert_eq!(zendo.protons.list().pages, 2);
        assert!(!zendo.protons.list().more);
    }

    #[test]
    fn each_build_keeps_its_own_releases() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Proton(ProtonMessage::Releases(
            Build::Ge,
            1,
            Ok((vec![release("GE-Proton11-12")], false)),
        )));
        let _ = zendo.update(Message::Proton(ProtonMessage::SetBuild(Build::CachyOs)));
        assert_eq!(zendo.protons.build, Build::CachyOs);
        assert_eq!(zendo.protons.list().releases, []);
        assert!(zendo.protons.list().asking, "a build not read yet is asked");
        let _ = zendo.update(Message::Proton(ProtonMessage::Releases(
            Build::CachyOs,
            1,
            Ok((
                vec![release("proton-cachyos-11.0-20261015-slr-x86_64")],
                false,
            )),
        )));
        let _ = zendo.update(Message::Proton(ProtonMessage::SetBuild(Build::Ge)));
        assert_eq!(zendo.protons.list().releases.len(), 1);
        assert_eq!(zendo.protons.list().releases[0].name, "GE-Proton11-12");
        assert!(
            !zendo.protons.list().asking,
            "already read, not asked again"
        );
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
