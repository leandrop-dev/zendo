//! The Mods card on a game's page: the profile in use, the mods Zendo
//! keeps for the game, in order, each on or off, Add mod, a Bethesda
//! game's plugins in load order, the folder they go in, Apply and Back to
//! vanilla, with downloads and updates from Nexus (`nexus.rs`). The work is
//! `zendo_core::mods`'; this shows it and runs it on a thread.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, checkbox, column, container, pick_list, row, space, text, text_input};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::Game;
use zendo_core::mods::{self, Added, Applied, ModInfo, Overview, Plugins, ProfileChange};
use zendo_core::nexus;
use zendo_core::plugins::{self, Plugin, Rank, Style};

use super::nexus::{NexusMessage, added_note, download_row};
use super::nexus_search::SearchMessage;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct ModsState {
    /// Each game's, read when its page opens.
    shown: HashMap<String, Shown>,
    /// The game whose mods are being changed, and what is happening.
    pub(super) busy: Option<(String, String)>,
    /// A press that asks first: the game, and what it would do.
    confirm: Option<(String, Confirm)>,
    /// The mod folder as typed, before Save.
    folder: Option<(String, String)>,
    /// A profile's name being typed: the game, and the edit.
    naming: Option<(String, Naming)>,
}

/// A new profile's name, or a new name for one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Naming {
    /// The profile being renamed; `None` for a new one.
    renaming: Option<String>,
    name: String,
}

#[derive(Debug)]
enum Shown {
    Reading,
    /// No folder to put mods in.
    Nowhere,
    Ready(Overview),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Confirm {
    Remove(String),
    ClearOverwrite,
    DeleteProfile(String),
}

#[derive(Debug, Clone)]
pub enum ModsMessage {
    Read(String, Result<Option<Overview>, String>),
    Pick(String),
    Picked(String, Result<Option<PathBuf>, String>),
    Toggle(String, String, bool),
    Move(String, String, bool),
    /// Turn a plugin on or off: the game, the plugin's name.
    TogglePlugin(String, String, bool),
    MovePlugin(String, String, bool),
    Remove(String, String),
    Folder(String, String),
    SaveFolder(String),
    KnownFolder(String),
    ClearOverwrite(String),
    Apply(String),
    Restore(String),
    /// Run a mod's installer again.
    Options(String, String),
    /// An archive added or an installer read: the mod in, or the
    /// installer's options to choose.
    Added(String, Result<(Added, String), String>),
    /// Switch to the profile with this name.
    Profile(String, String),
    NewProfile(String),
    RenameProfile(String),
    ProfileName(String, String),
    SaveProfile(String),
    CancelProfile,
    DeleteProfile(String),
    Done(String, Result<(Overview, String), String>),
}

/// What a change to the profiles says.
fn profile_note(change: &ProfileChange, overview: &Overview) -> String {
    let now = overview.profile.as_str();
    let state = match overview.applied {
        Applied::Current(_) => "applied",
        Applied::Changed => "changed",
        Applied::No => "no",
    };
    match change {
        ProfileChange::New(_) => t!("profile-made", now = now),
        ProfileChange::Switch(_) => t!("profile-switched", now = now, state = state),
        ProfileChange::Rename { new, .. } => t!("profile-renamed", name = new.trim()),
        ProfileChange::Delete(name) => t!(
            "profile-deleted",
            name = name.as_str(),
            now = now,
            state = state
        ),
    }
}

/// What a finished change says.
fn applied_note(overview: &Overview, title: &str) -> String {
    match overview.applied {
        Applied::Current(count) => t!("mods-applied-count", count = count, title = title),
        Applied::No => t!("mods-vanilla", title = title),
        Applied::Changed => t!("mods-applied", title = title),
    }
}

impl Zendo {
    /// Reads a game's mods when its page opens.
    pub(super) fn want_mods(&mut self, id: &str) -> Task<Message> {
        let Some(game) = self.game(id).cloned() else {
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        if !self.mods.shown.contains_key(id) {
            self.mods.shown.insert(id.to_owned(), Shown::Reading);
        }
        let id = id.to_owned();
        Task::perform(
            blocking(move || mods::overview(&game).map_err(|err| format!("{err:#}"))),
            move |result| Message::Mods(ModsMessage::Read(id.clone(), result)),
        )
    }

    pub(super) fn update_mods(&mut self, message: ModsMessage) -> Task<Message> {
        match message {
            ModsMessage::Read(id, result) => match result {
                Ok(Some(overview)) => self.mods_ready(id, overview),
                Ok(None) => {
                    self.mods.shown.insert(id, Shown::Nowhere);
                    Task::none()
                }
                Err(err) => {
                    self.mods.shown.insert(id, Shown::Failed(err));
                    Task::none()
                }
            },
            ModsMessage::Pick(id) => self.pick_mod(id),
            ModsMessage::Picked(id, result) => self.mod_picked(id, result),
            ModsMessage::Toggle(id, mod_id, on) => {
                self.change_mods(id, t!("saving"), move |game| {
                    mods::set_enabled(game, &mod_id, on).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::Move(id, mod_id, up) => self.change_mods(id, t!("saving"), move |game| {
                mods::move_mod(game, &mod_id, up).map(|overview| (overview, String::new()))
            }),
            ModsMessage::TogglePlugin(id, name, on) => {
                self.change_mods(id, t!("saving"), move |game| {
                    mods::set_plugin(game, &name, on).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::MovePlugin(id, name, up) => {
                self.change_mods(id, t!("saving"), move |game| {
                    mods::move_plugin(game, &name, up).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::Remove(id, mod_id) => {
                let ask = Confirm::Remove(mod_id.clone());
                if self.mods.confirm.as_ref() != Some(&(id.clone(), ask.clone())) {
                    self.mods.confirm = Some((id, ask));
                    return Task::none();
                }
                let note = t!("mods-deleted");
                self.change_mods(id, t!("deleting"), move |game| {
                    mods::remove(game, &mod_id).map(|overview| (overview, note))
                })
            }
            ModsMessage::Folder(id, folder) => {
                self.mods.folder = Some((id, folder));
                Task::none()
            }
            ModsMessage::SaveFolder(id) => {
                let folder = match self.mods.folder.take() {
                    Some((game, folder)) if game == id => folder,
                    _ => return Task::none(),
                };
                let note = t!("mods-new-folder");
                self.change_mods(id, t!("saving"), move |game| {
                    mods::set_folder(game, Some(&folder)).map(|overview| (overview, note))
                })
            }
            ModsMessage::KnownFolder(id) => {
                self.mods.folder = None;
                self.change_mods(id, t!("saving"), |game| {
                    mods::set_folder(game, None).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::ClearOverwrite(id) => {
                if self.mods.confirm.as_ref() != Some(&(id.clone(), Confirm::ClearOverwrite)) {
                    self.mods.confirm = Some((id, Confirm::ClearOverwrite));
                    return Task::none();
                }
                let note = t!("mods-overwrite-deleted");
                self.change_mods(id, t!("deleting"), |game| {
                    mods::clear_overwrite(game).map(|overview| (overview, note))
                })
            }
            ModsMessage::Profile(..)
            | ModsMessage::NewProfile(_)
            | ModsMessage::RenameProfile(_)
            | ModsMessage::ProfileName(..)
            | ModsMessage::SaveProfile(_)
            | ModsMessage::CancelProfile
            | ModsMessage::DeleteProfile(_) => self.update_profiles(message),
            ModsMessage::Options(id, mod_id) => {
                self.add_mods(id, t!("mods-reading-installer"), move |game| {
                    let pending = mods::reinstall(game, &mod_id)?;
                    Ok((Added::Choose(Box::new(pending)), String::new()))
                })
            }
            ModsMessage::Added(id, result) => self.mods_added(id, result),
            ModsMessage::Apply(id) => self.change_game_files(id, t!("mods-applying"), true),
            ModsMessage::Restore(id) => self.change_game_files(id, t!("mods-restoring"), false),
            ModsMessage::Done(id, result) => self.mods_done(id, result),
        }
    }

    /// The file dialog for a mod's archive, on a thread.
    fn pick_mod(&mut self, id: String) -> Task<Message> {
        if self.mods.busy.is_some() || !self.persist {
            return Task::none();
        }
        self.mods.busy = Some((id.clone(), t!("mods-choosing")));
        let title = t!("mods-choose");
        Task::perform(
            blocking(move || {
                crate::file_dialog::pick(crate::file_dialog::Kind::Mod, &title)
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::Mods(ModsMessage::Picked(id.clone(), result)),
        )
    }

    /// A game's mods as they now stand; their updates are asked of Nexus
    /// once a session.
    pub(super) fn mods_ready(&mut self, id: String, overview: Overview) -> Task<Message> {
        let check = self.check_mod_updates(&id, &overview);
        self.mods.shown.insert(id, Shown::Ready(overview));
        check
    }

    pub(super) fn mods_overview(&self, id: &str) -> Option<&Overview> {
        match self.mods.shown.get(id)? {
            Shown::Ready(overview) => Some(overview),
            _ => None,
        }
    }

    /// Unpacks the archive chosen in the dialog.
    fn mod_picked(&mut self, id: String, result: Result<Option<PathBuf>, String>) -> Task<Message> {
        self.mods.busy = None;
        match result {
            Ok(Some(archive)) => {
                let name = archive
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let note = t!("mods-added", name = name);
                let key = self.config.nexus_key.clone();
                self.add_mods(id, t!("mods-unpacking"), move |game| {
                    if let Some(added) = add_as_from_nexus(game, key.as_deref(), &archive) {
                        return Ok(added);
                    }
                    mods::add(game, &archive).map(|added| (added, note))
                })
            }
            Ok(None) => Task::none(),
            Err(err) => {
                self.status = Some(err);
                Task::none()
            }
        }
    }

    fn update_profiles(&mut self, message: ModsMessage) -> Task<Message> {
        match message {
            ModsMessage::Profile(id, name) => {
                self.change_profile(id, ProfileChange::Switch(name), t!("mods-switching"))
            }
            ModsMessage::NewProfile(id) => self.start_naming(id, false),
            ModsMessage::RenameProfile(id) => self.start_naming(id, true),
            ModsMessage::ProfileName(id, name) => {
                if let Some((game, naming)) = &mut self.mods.naming
                    && *game == id
                {
                    naming.name = name;
                }
                Task::none()
            }
            ModsMessage::SaveProfile(id) => match self.mods.naming.take() {
                Some((game, naming)) if game == id => {
                    let change = match naming.renaming {
                        Some(old) => ProfileChange::Rename {
                            old,
                            new: naming.name,
                        },
                        None => ProfileChange::New(naming.name),
                    };
                    self.change_profile(id, change, t!("saving"))
                }
                other => {
                    self.mods.naming = other;
                    Task::none()
                }
            },
            ModsMessage::CancelProfile => {
                self.mods.naming = None;
                Task::none()
            }
            ModsMessage::DeleteProfile(id) => {
                let Some(name) = self
                    .mods_overview(&id)
                    .map(|overview| overview.profile.clone())
                else {
                    return Task::none();
                };
                let ask = Confirm::DeleteProfile(name.clone());
                if self.mods.confirm.as_ref() != Some(&(id.clone(), ask.clone())) {
                    self.mods.confirm = Some((id, ask));
                    return Task::none();
                }
                self.change_profile(id, ProfileChange::Delete(name), t!("deleting"))
            }
            _ => Task::none(),
        }
    }

    /// Opens the name field for a new profile, or to rename the one in use.
    fn start_naming(&mut self, id: String, renaming: bool) -> Task<Message> {
        let current = self
            .mods_overview(&id)
            .map(|overview| overview.profile.clone())
            .unwrap_or_default();
        let naming = Naming {
            name: if renaming {
                current.clone()
            } else {
                String::new()
            },
            renaming: renaming.then_some(current),
        };
        self.mods.naming = Some((id, naming));
        Task::none()
    }

    /// A change to the game's profiles; a switch while mods are applied
    /// applies the new profile, unless the game runs.
    fn change_profile(&mut self, id: String, change: ProfileChange, busy: String) -> Task<Message> {
        let apply = !self.running.is_running(&id);
        self.change_mods(id, busy, move |game| {
            let overview = mods::change_profiles(game, &change, apply)?;
            let note = profile_note(&change, &overview);
            Ok((overview, note))
        })
    }

    /// Apply or Back to vanilla, never while the game runs.
    fn change_game_files(&mut self, id: String, busy: String, apply: bool) -> Task<Message> {
        if self.running.is_running(&id) {
            self.status = Some(t!("mods-close-game-first"));
            return Task::none();
        }
        let title = self
            .game(&id)
            .map(|game| game.title.clone())
            .unwrap_or_default();
        self.change_mods(id, busy, move |game| {
            let overview = if apply {
                mods::apply(game)?
            } else {
                mods::restore(game)?
            };
            let note = applied_note(&overview, &title);
            Ok((overview, note))
        })
    }

    /// Adds a mod, or reads an installer, on a thread; one change at a time.
    pub(super) fn add_mods(
        &mut self,
        id: String,
        busy: String,
        work: impl FnOnce(&Game) -> anyhow::Result<(Added, String)> + Send + 'static,
    ) -> Task<Message> {
        if self.mods.busy.is_some() {
            return Task::none();
        }
        let Some(game) = self.game(&id).cloned() else {
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        self.mods.busy = Some((id.clone(), busy));
        Task::perform(
            blocking(move || work(&game).map_err(|err| format!("{err:#}"))),
            move |result| Message::Mods(ModsMessage::Added(id.clone(), result)),
        )
    }

    /// A change done: the mods as they now stand, and what it says.
    fn mods_done(
        &mut self,
        id: String,
        result: Result<(Overview, String), String>,
    ) -> Task<Message> {
        self.mods.busy = None;
        self.mods.confirm = None;
        // The slot is free now: an installer that waited for it may run.
        match result {
            Ok((overview, note)) => {
                if !note.is_empty() {
                    self.status = Some(note);
                }
                Task::batch([self.mods_ready(id, overview), self.next_installer()])
            }
            Err(err) => {
                self.status = Some(err);
                self.next_installer()
            }
        }
    }

    /// A mod in, or its installer to open.
    fn mods_added(&mut self, id: String, result: Result<(Added, String), String>) -> Task<Message> {
        self.mods.busy = None;
        self.mods.confirm = None;
        match result {
            Ok((Added::Done(overview), note)) => {
                self.status = Some(note);
                self.mods_ready(id, *overview)
            }
            Ok((Added::Choose(pending), _)) => self.open_installer(id, pending),
            Err(err) => {
                self.status = Some(err);
                Task::none()
            }
        }
    }

    /// Runs a change on a thread, one at a time.
    pub(super) fn change_mods(
        &mut self,
        id: String,
        busy: String,
        work: impl FnOnce(&Game) -> anyhow::Result<(Overview, String)> + Send + 'static,
    ) -> Task<Message> {
        if self.mods.busy.is_some() {
            return Task::none();
        }
        let Some(game) = self.game(&id).cloned() else {
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        self.mods.busy = Some((id.clone(), busy));
        Task::perform(
            blocking(move || work(&game).map_err(|err| format!("{err:#}"))),
            move |result| Message::Mods(ModsMessage::Done(id.clone(), result)),
        )
    }

    /// The Mods card, for a game with a folder to put them in.
    pub(super) fn mods_card<'a>(&'a self, game: &'a Game) -> Option<Element<'a, Message>> {
        let id = &game.id;
        let busy = self
            .mods
            .busy
            .as_ref()
            .filter(|(game, _)| game == id)
            .map(|(_, busy)| busy.as_str());
        let free = self.mods.busy.is_none();
        let body: Element<'_, Message> = match self.mods.shown.get(id)? {
            Shown::Nowhere => return None,
            Shown::Reading => text(t!("mods-reading")).size(13).color(colors().dim).into(),
            Shown::Failed(err) => text(err).size(13).color(colors().bad).into(),
            Shown::Ready(overview) => self.mods_body(id, overview, free),
        };
        let add = button(style::button_text(t!("mods-add")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press_maybe(free.then(|| Message::Mods(ModsMessage::Pick(id.clone()))));
        let mut header = row![
            text(t!("mods-title").to_uppercase()).size(14).font(BOLD),
            space::horizontal()
        ]
        .spacing(style::SPACE_S)
        .align_y(Alignment::Center);
        if let Some(domain) = self
            .mods_overview(id)
            .and_then(|overview| overview.nexus.as_ref())
        {
            header = header
                .push(
                    button(style::button_text(t!("mods-search-nexus")))
                        .padding(style::BUTTON)
                        .style(style::secondary)
                        .on_press(Message::NexusSearch(SearchMessage::Open(id.clone()))),
                )
                .push(
                    button(style::button_text(t!("mods-find-on-nexus")))
                        .padding(style::BUTTON)
                        .style(style::secondary)
                        .on_press(Message::Open(nexus::game_url(domain))),
                );
        }
        let mut card = column![header.push(add)].spacing(style::SPACE_M);
        for download in self.nexus_downloads_for(id) {
            card = card.push(download_row(download));
        }
        card = card.push(body);
        if let Some(busy) = busy {
            card = card.push(text(busy).size(13).color(colors().dim));
        }
        Some(
            container(card)
                .padding(style::SPACE_XL)
                .width(Length::Fill)
                .style(style::card)
                .into(),
        )
    }

    fn mods_body<'a>(
        &'a self,
        id: &'a str,
        overview: &'a Overview,
        free: bool,
    ) -> Element<'a, Message> {
        let place = match overview.known {
            Some(_) if !overview.custom_folder && overview.folder.is_empty() => {
                t!("mods-place-known-root")
            }
            Some(_) if !overview.custom_folder => {
                t!("mods-place-known", folder = overview.folder.as_str())
            }
            _ if overview.custom_folder => {
                t!("mods-place-custom", folder = overview.folder.as_str())
            }
            _ => t!("mods-place-root"),
        };
        let mut body = column![
            self.profile_row(id, overview, free),
            text(place).size(13).color(colors().dim),
            text(overview.target.display().to_string())
                .size(13)
                .font(Font::MONOSPACE),
        ]
        .spacing(style::SPACE_M);

        if overview.mods.is_empty() {
            body = body.push(text(t!("mods-none")).size(14).color(colors().dim));
        }
        let last = overview.mods.len().saturating_sub(1);
        for (at, info) in overview.mods.iter().enumerate() {
            body = body.push(self.mod_row(id, info, at == 0, at == last, free));
        }
        if let Some(plugins) = &overview.plugins {
            let style = overview
                .known
                .and_then(|known| known.plugins)
                .map(|rules| rules.style);
            body = body.push(plugins_section(id, plugins, style, free));
        }
        if overview.overwrite > 0 {
            let asking =
                self.mods.confirm.as_ref() == Some(&(id.to_owned(), Confirm::ClearOverwrite));
            body = body.push(
                row![
                    text(t!("mods-overwrite", count = overview.overwrite))
                        .size(13)
                        .color(colors().dim)
                        .width(Length::Fill),
                    button(style::button_text(delete_label(asking)))
                        .padding(style::BUTTON_SMALL)
                        .style(if asking {
                            style::danger
                        } else {
                            style::secondary
                        })
                        .on_press_maybe(
                            free.then(|| Message::Mods(ModsMessage::ClearOverwrite(id.to_owned()))),
                        ),
                ]
                .spacing(style::SPACE_M)
                .align_y(Alignment::Center),
            );
        }
        body = body.push(self.folder_row(id, overview, free));
        body.push(self.apply_row(id, overview, free)).into()
    }

    fn mod_row<'a>(
        &self,
        id: &'a str,
        info: &'a ModInfo,
        first: bool,
        last: bool,
        free: bool,
    ) -> Element<'a, Message> {
        let mut facts = Vec::new();
        if let Some(version) = info
            .nexus
            .as_ref()
            .map(|nexus| nexus.version.as_str())
            .filter(|version| !version.is_empty())
        {
            facts.push(t!("mods-from-nexus-version", version = version));
        } else if info.nexus.is_some() {
            facts.push(t!("mods-from-nexus"));
        }
        facts.push(t!("count-files", count = info.files));
        for (other, count) in &info.wins {
            facts.push(t!("mods-wins", count = *count, other = other.as_str()));
        }
        for (other, count) in &info.loses {
            facts.push(t!("mods-loses", count = *count, other = other.as_str()));
        }
        let asking =
            self.mods.confirm.as_ref() == Some(&(id.to_owned(), Confirm::Remove(info.id.clone())));
        let (game, mod_id) = (id.to_owned(), info.id.clone());
        let toggle = {
            let (game, mod_id) = (game.clone(), mod_id.clone());
            move |on| Message::Mods(ModsMessage::Toggle(game.clone(), mod_id.clone(), on))
        };
        let step =
            |label: &'static str, up: bool, can: bool| {
                button(style::button_text(label))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press_maybe((free && can).then(|| {
                        Message::Mods(ModsMessage::Move(game.clone(), mod_id.clone(), up))
                    }))
            };
        let options = info.installer.then(|| {
            button(style::button_text(t!("mods-options")))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press_maybe(
                    free.then(|| Message::Mods(ModsMessage::Options(game.clone(), mod_id.clone()))),
                )
        });
        let update = self.mod_update(id, &info.id).map(|file| {
            let label = match file
                .version
                .as_deref()
                .filter(|version| !version.is_empty())
            {
                Some(version) => t!("mods-update-to", version = version),
                None => t!("mods-update"),
            };
            button(text(label).size(13).color(colors().good))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press(Message::Nexus(NexusMessage::Update(
                    game.clone(),
                    mod_id.clone(),
                )))
        });
        row![
            checkbox(info.enabled)
                .on_toggle_maybe(free.then_some(toggle))
                .size(16)
                .style(style::checkbox),
            column![
                text(&info.name).size(14),
                text(facts.join(" · ")).size(12).color(colors().dim),
            ]
            .spacing(style::SPACE_XS)
            .width(Length::Fill),
        ]
        .push(update)
        .push(options)
        .push(
            row![
                step("↑", true, !first),
                step("↓", false, !last),
                button(style::button_text(delete_label(asking)))
                    .padding(style::BUTTON_SMALL)
                    .style(if asking {
                        style::danger
                    } else {
                        style::secondary
                    })
                    .on_press_maybe(
                        free.then(|| Message::Mods(ModsMessage::Remove(
                            game.clone(),
                            mod_id.clone()
                        ))),
                    ),
            ]
            .spacing(style::SPACE_M)
            .align_y(Alignment::Center),
        )
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
    }

    /// The profile in use, to switch, and New, Rename and Delete; or the
    /// name being typed for New or Rename.
    fn profile_row<'a>(
        &'a self,
        id: &'a str,
        overview: &'a Overview,
        free: bool,
    ) -> Element<'a, Message> {
        let label = text(t!("mods-profile"))
            .size(14)
            .color(colors().dim)
            .width(110);
        let small = |label: String| {
            button(style::button_text(label))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
        };
        if let Some((_, naming)) = self.mods.naming.as_ref().filter(|(game, _)| game == id) {
            return row![
                label,
                text_input(
                    &if naming.renaming.is_some() {
                        t!("mods-profile-rename")
                    } else {
                        t!("mods-profile-new-name")
                    },
                    &naming.name,
                )
                .on_input(|name| Message::Mods(ModsMessage::ProfileName(id.to_owned(), name)))
                .on_submit(Message::Mods(ModsMessage::SaveProfile(id.to_owned())))
                .size(13)
                .padding(style::BUTTON_SMALL)
                .style(style::input),
                small(t!("key-save")).on_press_maybe(
                    (free && !naming.name.trim().is_empty())
                        .then(|| Message::Mods(ModsMessage::SaveProfile(id.to_owned()))),
                ),
                small(t!("cancel")).on_press(Message::Mods(ModsMessage::CancelProfile)),
            ]
            .spacing(style::SPACE_M)
            .align_y(Alignment::Center)
            .into();
        }
        let asking = self.mods.confirm.as_ref()
            == Some(&(
                id.to_owned(),
                Confirm::DeleteProfile(overview.profile.clone()),
            ));
        let choose = pick_list(
            overview.profiles.as_slice(),
            Some(&overview.profile),
            move |name| Message::Mods(ModsMessage::Profile(id.to_owned(), name)),
        )
        .text_size(13)
        .padding([style::SPACE_S, style::SPACE_M])
        .style(style::pick_list)
        .menu_style(style::menu)
        .width(Length::Fill);
        row![
            label,
            choose,
            small(t!("mods-profile-new")).on_press_maybe(
                free.then(|| Message::Mods(ModsMessage::NewProfile(id.to_owned())))
            ),
            small(t!("mods-profile-rename-button")).on_press_maybe(
                free.then(|| Message::Mods(ModsMessage::RenameProfile(id.to_owned())))
            ),
            button(style::button_text(delete_label(asking)))
                .padding(style::BUTTON_SMALL)
                .style(if asking {
                    style::danger
                } else {
                    style::secondary
                })
                .on_press_maybe(
                    (free && overview.profiles.len() > 1)
                        .then(|| Message::Mods(ModsMessage::DeleteProfile(id.to_owned()))),
                ),
        ]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
    }

    fn folder_row<'a>(
        &'a self,
        id: &'a str,
        overview: &'a Overview,
        free: bool,
    ) -> Element<'a, Message> {
        let typed = self
            .mods
            .folder
            .as_ref()
            .filter(|(game, _)| game == id)
            .map(|(_, folder)| folder.as_str());
        let value = typed.unwrap_or(&overview.folder);
        let applied = overview.applied != Applied::No;
        let mut folder = row![
            text(t!("mods-folder"))
                .size(14)
                .color(colors().dim)
                .width(110),
            text_input(&t!("mods-folder-placeholder"), value)
                .on_input_maybe((!applied).then(|| {
                    let id = id.to_owned();
                    move |folder| Message::Mods(ModsMessage::Folder(id.clone(), folder))
                }))
                .on_submit(Message::Mods(ModsMessage::SaveFolder(id.to_owned())))
                .size(13)
                .padding(style::BUTTON_SMALL)
                .style(style::input),
        ]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center);
        if typed.is_some_and(|typed| typed != overview.folder) {
            folder = folder.push(
                button(style::button_text(t!("key-save")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press_maybe(
                        free.then(|| Message::Mods(ModsMessage::SaveFolder(id.to_owned()))),
                    ),
            );
        }
        if overview.custom_folder && !applied {
            folder = folder.push(
                button(style::button_text(t!("mods-folder-known")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press_maybe(
                        free.then(|| Message::Mods(ModsMessage::KnownFolder(id.to_owned()))),
                    ),
            );
        }
        folder.into()
    }

    fn apply_row<'a>(
        &self,
        id: &'a str,
        overview: &'a Overview,
        free: bool,
    ) -> Element<'a, Message> {
        let running = self.running.is_running(id);
        let writes_plugins = overview
            .plugins
            .as_ref()
            .is_some_and(|plugins| plugins.file.is_some());
        let (state, color) = match overview.applied {
            Applied::No => (t!("mods-state-no"), colors().dim),
            Applied::Current(count) if writes_plugins => (
                t!("mods-state-applied-plugins", count = count),
                colors().good,
            ),
            Applied::Current(count) => (t!("mods-state-applied", count = count), colors().good),
            Applied::Changed => (t!("mods-state-changed"), colors().bad),
        };
        let state = if running {
            t!("mods-state-running")
        } else {
            state
        };
        let can = free && !running;
        let any_on = overview.mods.iter().any(|info| info.enabled)
            || overview.overwrite > 0
            || writes_plugins;
        row![
            text(state).size(13).color(color).width(Length::Fill),
            button(style::button_text(t!("mods-back-to-vanilla")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press_maybe(
                    (can && overview.applied != Applied::No)
                        .then(|| Message::Mods(ModsMessage::Restore(id.to_owned()))),
                ),
            button(style::button_text(t!("mods-apply")).font(BOLD))
                .padding(style::BUTTON_LARGE)
                .style(style::primary)
                .on_press_maybe(
                    (can && (any_on || overview.applied != Applied::No))
                        .then(|| Message::Mods(ModsMessage::Apply(id.to_owned()))),
                ),
        ]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
    }
}

/// Delete, or Really delete once pressed.
fn delete_label(asking: bool) -> String {
    if asking {
        t!("really-delete")
    } else {
        t!("delete")
    }
}

/// A Bethesda game's plugins in load order, each on or off, and where
/// Apply writes the order.
fn plugins_section<'a>(
    id: &'a str,
    plugins: &'a Plugins,
    style: Option<Style>,
    free: bool,
) -> Element<'a, Message> {
    let note = match (&plugins.file, style) {
        (Some(_), Some(Style::Timestamps)) => t!("plugins-note-dates"),
        (Some(_), _) => t!("plugins-note"),
        (None, _) => t!("plugins-note-no-prefix"),
    };
    let mut section = column![
        style::subheading(t!("plugins-title")),
        text(note).size(13).color(if plugins.file.is_some() {
            colors().dim
        } else {
            colors().bad
        }),
    ]
    .spacing(style::SPACE_S);
    if let Some(file) = &plugins.file {
        section = section.push(
            text(file.display().to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .color(colors().dim),
        );
    }
    if plugins.list.is_empty() {
        section = section.push(text(t!("plugins-none")).size(13).color(colors().dim));
    }
    for (at, plugin) in plugins.list.iter().enumerate() {
        section = section.push(plugin_row(id, &plugins.list, at, plugin, free));
    }
    section.into()
}

fn plugin_row<'a>(
    id: &'a str,
    list: &[Plugin],
    at: usize,
    plugin: &'a Plugin,
    free: bool,
) -> Element<'a, Message> {
    let mut facts = vec![match &plugin.from {
        Some(name) => t!("plugins-from", name = name.as_str()),
        None => t!("plugins-games-own"),
    }];
    if plugin.master {
        facts.push(t!("plugins-master"));
    }
    if plugin.rank == Rank::Implicit {
        facts.push(t!("plugins-always"));
    }
    let mut problems = Vec::new();
    if !plugin.missing.is_empty() {
        problems.push(t!("plugins-needs", names = plugin.missing.join(", ")));
    }
    if !plugin.after.is_empty() {
        problems.push(t!("plugins-before", names = plugin.after.join(", ")));
    }
    if plugin.unreadable {
        problems.push(t!("plugins-unreadable"));
    }
    let name = plugin.name.clone();
    let toggle = {
        let (game, name) = (id.to_owned(), name.clone());
        move |on| Message::Mods(ModsMessage::TogglePlugin(game.clone(), name.clone(), on))
    };
    let step =
        |label: &'static str, up: bool| {
            button(style::button_text(label))
                .padding(style::BUTTON_SMALL)
                .style(style::secondary)
                .on_press_maybe((free && plugins::can_move(list, at, up)).then(|| {
                    Message::Mods(ModsMessage::MovePlugin(id.to_owned(), name.clone(), up))
                }))
        };
    let mut about = column![
        text(&plugin.name).size(14),
        text(facts.join(" · ")).size(12).color(colors().dim),
    ]
    .spacing(style::SPACE_XS)
    .width(Length::Fill);
    if !problems.is_empty() {
        about = about.push(text(problems.join(" · ")).size(12).color(colors().bad));
    }
    row![
        checkbox(plugin.enabled)
            .on_toggle_maybe((free && plugin.rank != Rank::Implicit).then_some(toggle))
            .size(16)
            .style(style::checkbox),
        about,
        step("↑", true),
        step("↓", false),
    ]
    .spacing(style::SPACE_M)
    .align_y(Alignment::Center)
    .into()
}

/// An archive Nexus named (`<name>-<mod id>-<version>-<time>`), added by
/// hand: as the file of its mod, so its updates are followed, when the
/// game is on Nexus and the key finds it. The archive chosen stays where it
/// is; a copy is taken in. `None` adds it as any other.
fn add_as_from_nexus(
    game: &Game,
    key: Option<&str>,
    archive: &std::path::Path,
) -> Option<(mods::Added, String)> {
    let key = key?;
    let name = archive.file_name()?.to_string_lossy().into_owned();
    if nexus::ids_in_name(&name).is_empty() {
        return None;
    }
    let domain = mods::nexus_domain(game)?;
    // Best effort, unlike a file the user asked Nexus for: a lookup that
    // fails adds the archive as any other mod, which is what `None` says.
    let (mod_id, mut fetched) = nexus::identify(key, &domain, archive, None).ok()??;
    let folder = mods::downloads_dir(game).ok()?;
    std::fs::create_dir_all(&folder).ok()?;
    let copy = folder.join(&name);
    std::fs::copy(archive, &copy).ok()?;
    fetched.archive = copy;
    if let Ok((added, updated)) = mods::add_from_nexus(game, &fetched, &domain, mod_id) {
        return Some((added, added_note(updated, &fetched, &game.title)));
    }
    let _ = std::fs::remove_file(&fetched.archive);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overview(applied: Applied) -> Overview {
        Overview {
            game_dir: "/games/Cat Quest".into(),
            target: "/games/Cat Quest".into(),
            folder: String::new(),
            custom_folder: false,
            known: None,
            nexus: None,
            profile: mods::DEFAULT_PROFILE.into(),
            profiles: vec![mods::DEFAULT_PROFILE.into()],
            mods: vec![ModInfo {
                id: "bepinex".into(),
                name: "BepInEx".into(),
                enabled: true,
                files: 22,
                nexus: None,
                installer: false,
                wins: Vec::new(),
                loses: Vec::new(),
            }],
            overwrite: 0,
            applied,
            plugins: None,
        }
    }

    #[test]
    fn a_profile_is_named_in_place_and_deleting_it_asks_first() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let id = "epic:cat".to_owned();
        let mut two = overview(Applied::No);
        two.profiles.push("Light".into());
        let _ = zendo.update(Message::Mods(ModsMessage::Read(id.clone(), Ok(Some(two)))));

        let mods = |message| Message::Mods(message);
        let _ = zendo.update(mods(ModsMessage::RenameProfile(id.clone())));
        let (_, naming) = zendo.mods.naming.clone().unwrap();
        assert_eq!(naming.renaming.as_deref(), Some("Default"));
        assert_eq!(naming.name, "Default");
        let _ = zendo.update(mods(ModsMessage::CancelProfile));
        assert_eq!(zendo.mods.naming, None);

        let _ = zendo.update(mods(ModsMessage::NewProfile(id.clone())));
        let _ = zendo.update(mods(ModsMessage::ProfileName(id.clone(), "Run 2".into())));
        let (_, naming) = zendo.mods.naming.clone().unwrap();
        assert_eq!((naming.renaming, naming.name.as_str()), (None, "Run 2"));
        let _ = zendo.update(mods(ModsMessage::SaveProfile(id.clone())));
        assert_eq!(zendo.mods.naming, None);

        let _ = zendo.update(mods(ModsMessage::DeleteProfile(id.clone())));
        assert_eq!(
            zendo.mods.confirm,
            Some((id.clone(), Confirm::DeleteProfile("Default".into())))
        );

        let mut light = overview(Applied::Current(1));
        light.profile = "Light".into();
        assert_eq!(
            profile_note(&ProfileChange::Switch("Light".into()), &light),
            "Switched to Light; Light is applied"
        );
        light.applied = Applied::Changed;
        assert_eq!(
            profile_note(&ProfileChange::Delete("Default".into()), &light),
            "Deleted Default; now in Light; Apply puts it in the game"
        );
    }

    #[test]
    fn deleting_asks_first_and_a_change_shows_when_done() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let id = "epic:cat".to_owned();
        let _ = zendo.update(Message::Mods(ModsMessage::Read(
            id.clone(),
            Ok(Some(overview(Applied::No))),
        )));
        assert!(matches!(zendo.mods.shown.get(&id), Some(Shown::Ready(_))));

        let remove = || Message::Mods(ModsMessage::Remove(id.clone(), "bepinex".into()));
        let _ = zendo.update(remove());
        assert_eq!(
            zendo.mods.confirm,
            Some((id.clone(), Confirm::Remove("bepinex".into())))
        );

        let done = overview(Applied::Current(1));
        let note = applied_note(&done, "Cat Quest");
        assert_eq!(note, "Applied 1 mod to Cat Quest");
        let _ = zendo.update(Message::Mods(ModsMessage::Done(
            id.clone(),
            Ok((done, note)),
        )));
        assert_eq!(zendo.mods.confirm, None);
        assert_eq!(zendo.status.as_deref(), Some("Applied 1 mod to Cat Quest"));
        assert!(matches!(
            zendo.mods.shown.get(&id),
            Some(Shown::Ready(Overview {
                applied: Applied::Current(1),
                ..
            }))
        ));
        let _ = zendo.update(Message::Mods(ModsMessage::Read(id.clone(), Ok(None))));
        assert!(matches!(zendo.mods.shown.get(&id), Some(Shown::Nowhere)));
    }
}
