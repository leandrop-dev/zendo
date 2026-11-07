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

use super::nexus::{NexusMessage, download_row};
use super::style::{self, BAD, BOLD, DIM, GOOD};
use super::{Message, Zendo, blocking};

#[derive(Debug, Default)]
pub struct ModsState {
    /// Each game's, read when its page opens.
    shown: HashMap<String, Shown>,
    /// The game whose mods are being changed, and what is happening.
    busy: Option<(String, &'static str)>,
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
    let now = &overview.profile;
    let state = match overview.applied {
        Applied::Current(_) => format!("; {now} is applied"),
        Applied::Changed => "; Apply puts it in the game".to_owned(),
        Applied::No => String::new(),
    };
    match change {
        ProfileChange::New(_) => format!("Made {now}, a copy of the profile before"),
        ProfileChange::Switch(_) => format!("Switched to {now}{state}"),
        ProfileChange::Rename { new, .. } => format!("Renamed the profile {}", new.trim()),
        ProfileChange::Delete(name) => format!("Deleted {name}; now in {now}{state}"),
    }
}

/// What a finished change says.
fn applied_note(overview: &Overview, title: &str) -> String {
    match overview.applied {
        Applied::Current(count) => format!(
            "Applied {} to {title}",
            super::format::count(count, "mod", "mods")
        ),
        Applied::No => format!("{title} is back to vanilla"),
        Applied::Changed => format!("Applied to {title}"),
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
            ModsMessage::Pick(id) => {
                if self.mods.busy.is_some() || !self.persist {
                    return Task::none();
                }
                self.mods.busy = Some((id.clone(), "Choosing a mod…"));
                Task::perform(
                    blocking(|| {
                        crate::file_dialog::pick(crate::file_dialog::Kind::Mod, "Choose a mod")
                            .map_err(|err| format!("{err:#}"))
                    }),
                    move |result| Message::Mods(ModsMessage::Picked(id.clone(), result)),
                )
            }
            ModsMessage::Picked(id, result) => self.mod_picked(id, result),
            ModsMessage::Toggle(id, mod_id, on) => self.change_mods(id, "Saving…", move |game| {
                mods::set_enabled(game, &mod_id, on).map(|overview| (overview, String::new()))
            }),
            ModsMessage::Move(id, mod_id, up) => self.change_mods(id, "Saving…", move |game| {
                mods::move_mod(game, &mod_id, up).map(|overview| (overview, String::new()))
            }),
            ModsMessage::TogglePlugin(id, name, on) => {
                self.change_mods(id, "Saving…", move |game| {
                    mods::set_plugin(game, &name, on).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::MovePlugin(id, name, up) => {
                self.change_mods(id, "Saving…", move |game| {
                    mods::move_plugin(game, &name, up).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::Remove(id, mod_id) => {
                let ask = Confirm::Remove(mod_id.clone());
                if self.mods.confirm.as_ref() != Some(&(id.clone(), ask.clone())) {
                    self.mods.confirm = Some((id, ask));
                    return Task::none();
                }
                self.change_mods(id, "Deleting…", move |game| {
                    mods::remove(game, &mod_id).map(|overview| (overview, "Mod deleted".to_owned()))
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
                self.change_mods(id, "Saving…", move |game| {
                    mods::set_folder(game, Some(&folder))
                        .map(|overview| (overview, "Mods go to the new folder".to_owned()))
                })
            }
            ModsMessage::KnownFolder(id) => {
                self.mods.folder = None;
                self.change_mods(id, "Saving…", |game| {
                    mods::set_folder(game, None).map(|overview| (overview, String::new()))
                })
            }
            ModsMessage::ClearOverwrite(id) => {
                if self.mods.confirm.as_ref() != Some(&(id.clone(), Confirm::ClearOverwrite)) {
                    self.mods.confirm = Some((id, Confirm::ClearOverwrite));
                    return Task::none();
                }
                self.change_mods(id, "Deleting…", |game| {
                    mods::clear_overwrite(game)
                        .map(|overview| (overview, "Deleted what the game wrote".to_owned()))
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
                self.add_mods(id, "Reading the installer…", move |game| {
                    let pending = mods::reinstall(game, &mod_id)?;
                    Ok((Added::Choose(Box::new(pending)), String::new()))
                })
            }
            ModsMessage::Added(id, result) => self.mods_added(id, result),
            ModsMessage::Apply(id) => self.change_game_files(id, "Applying…", true),
            ModsMessage::Restore(id) => self.change_game_files(id, "Going back to vanilla…", false),
            ModsMessage::Done(id, result) => self.mods_done(id, result),
        }
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
                self.add_mods(id, "Unpacking…", move |game| {
                    mods::add(game, &archive).map(|added| (added, format!("Added {name}")))
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
                self.change_profile(id, ProfileChange::Switch(name), "Switching…")
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
                    self.change_profile(id, change, "Saving…")
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
                self.change_profile(id, ProfileChange::Delete(name), "Deleting…")
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
    fn change_profile(
        &mut self,
        id: String,
        change: ProfileChange,
        busy: &'static str,
    ) -> Task<Message> {
        let apply = !self.running.is_running(&id);
        self.change_mods(id, busy, move |game| {
            let overview = mods::change_profiles(game, &change, apply)?;
            let note = profile_note(&change, &overview);
            Ok((overview, note))
        })
    }

    /// Apply or Back to vanilla, never while the game runs.
    fn change_game_files(&mut self, id: String, busy: &'static str, apply: bool) -> Task<Message> {
        if self.running.is_running(&id) {
            self.status = Some("Close the game first: its files are in use".to_owned());
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
    fn add_mods(
        &mut self,
        id: String,
        busy: &'static str,
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
        match result {
            Ok((overview, note)) => {
                if !note.is_empty() {
                    self.status = Some(note);
                }
                self.mods_ready(id, overview)
            }
            Err(err) => {
                self.status = Some(err);
                Task::none()
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
        busy: &'static str,
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
            .map(|(_, busy)| *busy);
        let free = self.mods.busy.is_none();
        let body: Element<'_, Message> = match self.mods.shown.get(id)? {
            Shown::Nowhere => return None,
            Shown::Reading => text("Reading…").size(13).color(DIM).into(),
            Shown::Failed(err) => text(err).size(13).color(BAD).into(),
            Shown::Ready(overview) => self.mods_body(id, overview, free),
        };
        let add = button(text("Add mod…").size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(free.then(|| Message::Mods(ModsMessage::Pick(id.clone()))));
        let mut header = row![text("Mods").size(18).font(BOLD), space::horizontal()]
            .spacing(8)
            .align_y(Alignment::Center);
        if let Some(domain) = self
            .mods_overview(id)
            .and_then(|overview| overview.nexus.as_ref())
        {
            header = header.push(
                button(text("Find mods on Nexus").size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press(Message::Open(nexus::game_url(domain))),
            );
        }
        let mut card = column![header.push(add)].spacing(12);
        for download in self.nexus_downloads_for(id) {
            card = card.push(download_row(download));
        }
        card = card.push(body);
        if let Some(busy) = busy {
            card = card.push(text(busy).size(13).color(DIM));
        }
        Some(
            container(card)
                .padding(20)
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
                "Zendo knows this game: mods go in its own folder.".to_owned()
            }
            Some(_) if !overview.custom_folder => {
                format!("Zendo knows this game: mods go in {}.", overview.folder)
            }
            _ if overview.custom_folder => format!("Mods go in {}, as you set.", overview.folder),
            _ => "Mods go in the game's own folder; set another below if they belong elsewhere."
                .to_owned(),
        };
        let mut body = column![
            self.profile_row(id, overview, free),
            text(place).size(13).color(DIM),
            text(overview.target.display().to_string())
                .size(13)
                .font(Font::MONOSPACE),
        ]
        .spacing(10);

        if overview.mods.is_empty() {
            body = body.push(
                text("No mods yet. Add mod… takes an archive (zip, 7z, rar) you downloaded.")
                    .size(14)
                    .color(DIM),
            );
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
                    text(format!(
                        "Files the game wrote while modded: {}, applied after every mod",
                        overview.overwrite
                    ))
                    .size(13)
                    .color(DIM)
                    .width(Length::Fill),
                    button(text(if asking { "Really delete" } else { "Delete" }).size(13))
                        .padding([4, 10])
                        .style(if asking {
                            style::danger
                        } else {
                            style::secondary
                        })
                        .on_press_maybe(
                            free.then(|| Message::Mods(ModsMessage::ClearOverwrite(id.to_owned()))),
                        ),
                ]
                .spacing(10)
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
            facts.push(format!("version {version} from Nexus"));
        } else if info.nexus.is_some() {
            facts.push("from Nexus".to_owned());
        }
        facts.push(super::format::count(info.files, "file", "files"));
        for (other, count) in &info.wins {
            facts.push(format!("replaces {count} of {other}"));
        }
        for (other, count) in &info.loses {
            facts.push(format!("{count} replaced by {other}"));
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
                button(text(label).size(13))
                    .padding([4, 10])
                    .style(style::secondary)
                    .on_press_maybe((free && can).then(|| {
                        Message::Mods(ModsMessage::Move(game.clone(), mod_id.clone(), up))
                    }))
            };
        let options = info.installer.then(|| {
            button(text("Options…").size(13))
                .padding([4, 10])
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
                Some(version) => format!("Update to {version}"),
                None => "Update".to_owned(),
            };
            button(text(label).size(13).color(GOOD))
                .padding([4, 10])
                .style(style::secondary)
                .on_press(Message::Nexus(NexusMessage::Update(
                    game.clone(),
                    mod_id.clone(),
                )))
        });
        row![
            checkbox(info.enabled)
                .on_toggle_maybe(free.then_some(toggle))
                .size(16),
            column![
                text(&info.name).size(14),
                text(facts.join(" · ")).size(12).color(DIM),
            ]
            .spacing(2)
            .width(Length::Fill),
        ]
        .push(update)
        .push(options)
        .push(
            row![
                step("↑", true, !first),
                step("↓", false, !last),
                button(text(if asking { "Really delete" } else { "Delete" }).size(13))
                    .padding([4, 10])
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
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .spacing(10)
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
        let label = text("Profile").size(14).color(DIM).width(110);
        let small = |label: &'a str| {
            button(text(label).size(13))
                .padding([6, 12])
                .style(style::secondary)
        };
        if let Some((_, naming)) = self.mods.naming.as_ref().filter(|(game, _)| game == id) {
            return row![
                label,
                text_input(
                    if naming.renaming.is_some() {
                        "A new name for this profile"
                    } else {
                        "A name for the new profile, a copy of this one"
                    },
                    &naming.name,
                )
                .on_input(|name| Message::Mods(ModsMessage::ProfileName(id.to_owned(), name)))
                .on_submit(Message::Mods(ModsMessage::SaveProfile(id.to_owned())))
                .size(13)
                .padding([6, 10])
                .style(style::input),
                small("Save").on_press_maybe(
                    (free && !naming.name.trim().is_empty())
                        .then(|| Message::Mods(ModsMessage::SaveProfile(id.to_owned()))),
                ),
                small("Cancel").on_press(Message::Mods(ModsMessage::CancelProfile)),
            ]
            .spacing(10)
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
        .padding([6, 10])
        .width(Length::Fill);
        row![
            label,
            choose,
            small("New…").on_press_maybe(
                free.then(|| Message::Mods(ModsMessage::NewProfile(id.to_owned())))
            ),
            small("Rename…").on_press_maybe(
                free.then(|| Message::Mods(ModsMessage::RenameProfile(id.to_owned())))
            ),
            button(text(if asking { "Really delete" } else { "Delete" }).size(13))
                .padding([6, 12])
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
        .spacing(10)
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
            text("Mod folder").size(14).color(DIM).width(110),
            text_input("the game's own folder", value)
                .on_input_maybe((!applied).then(|| {
                    let id = id.to_owned();
                    move |folder| Message::Mods(ModsMessage::Folder(id.clone(), folder))
                }))
                .on_submit(Message::Mods(ModsMessage::SaveFolder(id.to_owned())))
                .size(13)
                .padding([6, 10])
                .style(style::input),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if typed.is_some_and(|typed| typed != overview.folder) {
            folder = folder.push(
                button(text("Save").size(13))
                    .padding([6, 12])
                    .style(style::secondary)
                    .on_press_maybe(
                        free.then(|| Message::Mods(ModsMessage::SaveFolder(id.to_owned()))),
                    ),
            );
        }
        if overview.custom_folder && !applied {
            folder = folder.push(
                button(text("As Zendo knows").size(13))
                    .padding([6, 12])
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
            Applied::No => ("Not applied: the game is as it was".to_owned(), DIM),
            Applied::Current(count) => (
                format!(
                    "Applied: {}{} in the game",
                    super::format::count(count, "mod", "mods"),
                    if writes_plugins {
                        " and the plugin order"
                    } else {
                        ""
                    }
                ),
                GOOD,
            ),
            Applied::Changed => (
                "Changed since applied: Apply to put the list in".to_owned(),
                BAD,
            ),
        };
        let state = if running {
            "The game is running: close it to apply or go back to vanilla".to_owned()
        } else {
            state
        };
        let can = free && !running;
        let any_on = overview.mods.iter().any(|info| info.enabled)
            || overview.overwrite > 0
            || writes_plugins;
        row![
            text(state).size(13).color(color).width(Length::Fill),
            button(text("Back to vanilla").size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press_maybe(
                    (can && overview.applied != Applied::No)
                        .then(|| Message::Mods(ModsMessage::Restore(id.to_owned()))),
                ),
            button(text("Apply").size(14).font(BOLD))
                .padding([10, 24])
                .style(style::primary)
                .on_press_maybe(
                    (can && (any_on || overview.applied != Applied::No))
                        .then(|| Message::Mods(ModsMessage::Apply(id.to_owned()))),
                ),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
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
        (Some(_), Some(Style::Timestamps)) => {
            "Apply writes this order to plugins.txt in the game's prefix, and dates the \
             plugins in it, which is how this game reads it."
        }
        (Some(_), _) => "Apply writes this order to the game's plugin list in its prefix.",
        (None, _) => {
            "Start the game once, so its prefix is made; Apply writes this order there after."
        }
    };
    let mut section = column![
        text("Plugins").size(15).font(BOLD),
        text(note)
            .size(13)
            .color(if plugins.file.is_some() { DIM } else { BAD }),
    ]
    .spacing(8);
    if let Some(file) = &plugins.file {
        section = section.push(
            text(file.display().to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .color(DIM),
        );
    }
    if plugins.list.is_empty() {
        section = section.push(
            text("No plugins in the game's Data folder or the mods on.")
                .size(13)
                .color(DIM),
        );
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
        Some(name) => format!("from {name}"),
        None => "the game's".to_owned(),
    }];
    if plugin.master {
        facts.push("master".to_owned());
    }
    if plugin.rank == Rank::Implicit {
        facts.push("always loads".to_owned());
    }
    let mut problems = Vec::new();
    if !plugin.missing.is_empty() {
        problems.push(format!("needs {}, not on", plugin.missing.join(", ")));
    }
    if !plugin.after.is_empty() {
        problems.push(format!(
            "loads before {}, which it needs",
            plugin.after.join(", ")
        ));
    }
    if plugin.unreadable {
        problems.push("its header cannot be read".to_owned());
    }
    let name = plugin.name.clone();
    let toggle = {
        let (game, name) = (id.to_owned(), name.clone());
        move |on| Message::Mods(ModsMessage::TogglePlugin(game.clone(), name.clone(), on))
    };
    let step =
        |label: &'static str, up: bool| {
            button(text(label).size(13))
                .padding([4, 10])
                .style(style::secondary)
                .on_press_maybe((free && plugins::can_move(list, at, up)).then(|| {
                    Message::Mods(ModsMessage::MovePlugin(id.to_owned(), name.clone(), up))
                }))
        };
    let mut about = column![
        text(&plugin.name).size(14),
        text(facts.join(" · ")).size(12).color(DIM),
    ]
    .spacing(2)
    .width(Length::Fill);
    if !problems.is_empty() {
        about = about.push(text(problems.join(" · ")).size(12).color(BAD));
    }
    row![
        checkbox(plugin.enabled)
            .on_toggle_maybe((free && plugin.rank != Rank::Implicit).then_some(toggle))
            .size(16),
        about,
        step("↑", true),
        step("↓", false),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
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
