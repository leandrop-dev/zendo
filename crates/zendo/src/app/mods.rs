//! The Mods card on a game's page: the profile in use, the mods Zendo
//! keeps for the game, in order, each on or off, Add mod, a Bethesda
//! game's plugins in load order, the folder they go in, Apply and Back to
//! vanilla, with downloads and updates from Nexus (`nexus.rs`). The state,
//! the jobs and the words are `zendo_shell::mods`'; the work is
//! `zendo_core::mods`'; this shows it and runs it on a thread.

use std::path::PathBuf;

use iced::widget::{button, checkbox, column, container, pick_list, row, space, text, text_input};
use iced::{Alignment, Element, Font, Length, Task};
use zendo_core::Game;
use zendo_core::mods::{Added, ModInfo, Overview, Plugins};
use zendo_core::nexus;
use zendo_core::plugins::{self, Plugin, Rank, Style};
use zendo_shell::mods::{self as shell, After, Confirm, Job, Outcome, Shown, Tone};

pub use zendo_shell::mods::ModsState;

use super::nexus::{NexusMessage, download_row};
use super::nexus_search::SearchMessage;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub enum ModsMessage {
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
    /// Switch to the profile with this name.
    Profile(String, String),
    NewProfile(String),
    RenameProfile(String),
    ProfileName(String, String),
    SaveProfile(String),
    CancelProfile,
    DeleteProfile(String),
    /// A job ended: a read, a change, or an archive added or an installer
    /// read (the mod in, or the installer's options to choose).
    Done(String, Box<Result<Outcome, String>>),
}

impl Zendo {
    /// Whether work on the game's mods can run: it is in the library, and
    /// this is not a test.
    fn mods_runnable(&self, id: &str) -> bool {
        self.persist && self.game(id).is_some()
    }

    /// Runs a job on a thread.
    fn run_job(&self, job: Option<Job>) -> Task<Message> {
        let Some(Job { id, work }) = job else {
            return Task::none();
        };
        let Some(game) = self.game(&id).cloned() else {
            return Task::none();
        };
        Task::perform(blocking(move || shell::run(work, &game)), move |result| {
            Message::Mods(ModsMessage::Done(id.clone(), Box::new(result)))
        })
    }

    /// Reads a game's mods when its page opens.
    pub(super) fn want_mods(&mut self, id: &str) -> Task<Message> {
        if !self.mods_runnable(id) {
            return Task::none();
        }
        let job = self.mods.open(id);
        self.run_job(Some(job))
    }

    pub(super) fn update_mods(&mut self, message: ModsMessage) -> Task<Message> {
        match message {
            ModsMessage::Pick(id) => self.pick_mod(id),
            ModsMessage::Picked(id, result) => self.mod_picked(&id, result),
            ModsMessage::Toggle(id, mod_id, on) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.toggle(&id, &mod_id, on, ok);
                self.run_job(job)
            }
            ModsMessage::Move(id, mod_id, up) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.move_mod(&id, &mod_id, up, ok);
                self.run_job(job)
            }
            ModsMessage::TogglePlugin(id, name, on) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.toggle_plugin(&id, &name, on, ok);
                self.run_job(job)
            }
            ModsMessage::MovePlugin(id, name, up) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.move_plugin(&id, &name, up, ok);
                self.run_job(job)
            }
            ModsMessage::Remove(id, mod_id) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.remove(&id, &mod_id, ok);
                self.run_job(job)
            }
            ModsMessage::Folder(id, folder) => {
                self.mods.type_folder(&id, folder);
                Task::none()
            }
            ModsMessage::SaveFolder(id) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.save_folder(&id, ok);
                self.run_job(job)
            }
            ModsMessage::KnownFolder(id) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.known_folder(&id, ok);
                self.run_job(job)
            }
            ModsMessage::ClearOverwrite(id) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.clear_overwrite(&id, ok);
                self.run_job(job)
            }
            ModsMessage::Profile(..)
            | ModsMessage::NewProfile(_)
            | ModsMessage::RenameProfile(_)
            | ModsMessage::ProfileName(..)
            | ModsMessage::SaveProfile(_)
            | ModsMessage::CancelProfile
            | ModsMessage::DeleteProfile(_) => self.update_profiles(message),
            ModsMessage::Options(id, mod_id) => {
                let ok = self.mods_runnable(&id);
                let job = self.mods.reinstall(&id, &mod_id, ok);
                self.run_job(job)
            }
            ModsMessage::Apply(id) => self.change_game_files(&id, true),
            ModsMessage::Restore(id) => self.change_game_files(&id, false),
            ModsMessage::Done(id, result) => self.mods_done(&id, *result),
        }
    }

    /// The file dialog for a mod's archive, on a thread.
    fn pick_mod(&mut self, id: String) -> Task<Message> {
        let ok = self.mods_runnable(&id);
        if !self.mods.choosing(&id, ok) {
            return Task::none();
        }
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
        self.mods.ready(id, overview);
        check
    }

    pub(super) fn mods_overview(&self, id: &str) -> Option<&Overview> {
        self.mods.overview(id)
    }

    /// Unpacks the archive chosen in the dialog.
    fn mod_picked(&mut self, id: &str, result: Result<Option<PathBuf>, String>) -> Task<Message> {
        self.mods.chosen();
        match result {
            Ok(Some(archive)) => {
                let ok = self.mods_runnable(id);
                let key = self.config.nexus_key.clone();
                let job = self.mods.add(id, archive, key, ok);
                self.run_job(job)
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
                let running = self.running.is_running(&id);
                let ok = self.mods_runnable(&id);
                let job = self.mods.switch_profile(&id, &name, running, ok);
                self.run_job(job)
            }
            ModsMessage::NewProfile(id) => {
                self.mods.start_naming(&id, false);
                Task::none()
            }
            ModsMessage::RenameProfile(id) => {
                self.mods.start_naming(&id, true);
                Task::none()
            }
            ModsMessage::ProfileName(id, name) => {
                self.mods.name_typed(&id, name);
                Task::none()
            }
            ModsMessage::SaveProfile(id) => {
                let running = self.running.is_running(&id);
                let ok = self.mods_runnable(&id);
                let job = self.mods.save_profile(&id, running, ok);
                self.run_job(job)
            }
            ModsMessage::CancelProfile => {
                self.mods.cancel_profile();
                Task::none()
            }
            ModsMessage::DeleteProfile(id) => {
                let running = self.running.is_running(&id);
                let ok = self.mods_runnable(&id);
                let job = self.mods.delete_profile(&id, running, ok);
                self.run_job(job)
            }
            _ => Task::none(),
        }
    }

    /// Apply or Back to vanilla, never while the game runs.
    fn change_game_files(&mut self, id: &str, apply: bool) -> Task<Message> {
        let running = self.running.is_running(id);
        let ok = self.mods_runnable(id);
        match self.mods.apply(id, apply, running, ok) {
            Ok(job) => self.run_job(job),
            Err(note) => {
                self.status = Some(note);
                Task::none()
            }
        }
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
            blocking(move || {
                work(&game)
                    .map(|(added, note)| Outcome::Added(added, note))
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::Mods(ModsMessage::Done(id.clone(), Box::new(result))),
        )
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
            blocking(move || {
                work(&game)
                    .map(|(overview, note)| Outcome::Changed(overview, note))
                    .map_err(|err| format!("{err:#}"))
            }),
            move |result| Message::Mods(ModsMessage::Done(id.clone(), Box::new(result))),
        )
    }

    /// A job done: the state it leaves, what it says, and what follows (the
    /// slot is free, so an installer that waited for it may run).
    fn mods_done(&mut self, id: &str, result: Result<Outcome, String>) -> Task<Message> {
        let done = self.mods.done(id, result);
        if let Some(status) = done.status {
            self.status = Some(status);
        }
        let tasks: Vec<Task<Message>> = done
            .after
            .into_iter()
            .map(|after| match after {
                After::CheckUpdates(id) => match self.mods.overview(&id).cloned() {
                    Some(overview) => self.check_mod_updates(&id, &overview),
                    None => Task::none(),
                },
                After::NextInstaller => self.next_installer(),
                After::OpenInstaller(id, pending) => self.open_installer(id, pending),
            })
            .collect();
        Task::batch(tasks)
    }

    /// The Mods card, for a game with a folder to put them in.
    pub(super) fn mods_card<'a>(&'a self, game: &'a Game) -> Option<Element<'a, Message>> {
        let id = &game.id;
        let busy = self.mods.busy_for(id);
        let free = self.mods.free();
        let body: Element<'_, Message> = match self.mods.shown(id)? {
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
        let mut body = column![
            self.profile_row(id, overview, free),
            text(shell::place_line(overview))
                .size(13)
                .color(colors().dim),
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
            body = body.push(plugins_section(
                id,
                plugins,
                shell::plugins_style(overview),
                free,
            ));
        }
        if overview.overwrite > 0 {
            let asking = self.mods.asking(id, &Confirm::ClearOverwrite);
            body = body.push(
                row![
                    text(t!("mods-overwrite", count = overview.overwrite))
                        .size(13)
                        .color(colors().dim)
                        .width(Length::Fill),
                    button(style::button_text(shell::delete_label(asking)))
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
        let asking = self.mods.asking(id, &Confirm::Remove(info.id.clone()));
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
            button(
                text(zendo_shell::nexus::update_label(file))
                    .size(13)
                    .color(colors().good),
            )
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
                text(shell::mod_facts(info)).size(12).color(colors().dim),
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
                button(style::button_text(shell::delete_label(asking)))
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
        if let Some(naming) = self.mods.naming(id) {
            return row![
                label,
                text_input(
                    &shell::name_placeholder(naming.renaming.is_some()),
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
        let asking = self
            .mods
            .asking(id, &Confirm::DeleteProfile(overview.profile.clone()));
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
            button(style::button_text(shell::delete_label(asking)))
                .padding(style::BUTTON_SMALL)
                .style(if asking {
                    style::danger
                } else {
                    style::secondary
                })
                .on_press_maybe(
                    shell::profile_delete_enabled(free, overview)
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
        let typed = self.mods.typed_folder(id);
        let value = typed.unwrap_or(&overview.folder);
        let editable = shell::folder_editable(overview);
        let mut folder = row![
            text(t!("mods-folder"))
                .size(14)
                .color(colors().dim)
                .width(110),
            text_input(&t!("mods-folder-placeholder"), value)
                .on_input_maybe(editable.then(|| {
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
        if shell::folder_save_shown(typed, overview) {
            folder = folder.push(
                button(style::button_text(t!("key-save")))
                    .padding(style::BUTTON_SMALL)
                    .style(style::secondary)
                    .on_press_maybe(
                        free.then(|| Message::Mods(ModsMessage::SaveFolder(id.to_owned()))),
                    ),
            );
        }
        if shell::known_folder_shown(overview) {
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
        let (state, tone) = shell::apply_state(overview, running);
        let color = match tone {
            Tone::Dim => colors().dim,
            Tone::Good => colors().good,
            Tone::Bad => colors().bad,
        };
        row![
            text(state).size(13).color(color).width(Length::Fill),
            button(style::button_text(t!("mods-back-to-vanilla")))
                .padding(style::BUTTON)
                .style(style::secondary)
                .on_press_maybe(
                    shell::can_restore(free, running, overview)
                        .then(|| Message::Mods(ModsMessage::Restore(id.to_owned()))),
                ),
            button(style::button_text(t!("mods-apply")).font(BOLD))
                .padding(style::BUTTON_LARGE)
                .style(style::primary)
                .on_press_maybe(
                    shell::can_apply(free, running, overview)
                        .then(|| Message::Mods(ModsMessage::Apply(id.to_owned()))),
                ),
        ]
        .spacing(style::SPACE_M)
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
    let (note, bad) = shell::plugins_note(plugins, style);
    let mut section = column![
        style::subheading(t!("plugins-title")),
        text(note)
            .size(13)
            .color(if bad { colors().bad } else { colors().dim }),
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
    let problems = shell::plugin_problems(plugin);
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
        text(shell::plugin_facts(plugin))
            .size(12)
            .color(colors().dim),
    ]
    .spacing(style::SPACE_XS)
    .width(Length::Fill);
    if !problems.is_empty() {
        about = about.push(text(problems).size(12).color(colors().bad));
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

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::mods::{self, Applied};

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

    /// The window's messages reach the shared state; its own tests are
    /// `zendo_shell::mods`'.
    #[test]
    fn the_messages_drive_the_shared_state() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let id = "epic:cat".to_owned();
        let mut two = overview(Applied::No);
        two.profiles.push("Light".into());
        let _ = zendo.update(Message::Mods(ModsMessage::Done(
            id.clone(),
            Box::new(Ok(Outcome::Read(Some(two)))),
        )));
        assert!(matches!(zendo.mods.shown(&id), Some(Shown::Ready(_))));

        let mods = |message| Message::Mods(message);
        let _ = zendo.update(mods(ModsMessage::NewProfile(id.clone())));
        let _ = zendo.update(mods(ModsMessage::ProfileName(id.clone(), "Run 2".into())));
        assert_eq!(zendo.mods.naming(&id).unwrap().name, "Run 2");
        let _ = zendo.update(mods(ModsMessage::CancelProfile));
        assert_eq!(zendo.mods.naming(&id), None);

        let _ = zendo.update(mods(ModsMessage::Remove(id.clone(), "bepinex".into())));
        assert!(zendo.mods.asking(&id, &Confirm::Remove("bepinex".into())));

        let done = overview(Applied::Current(1));
        let note = shell::applied_note(&done, "Cat Quest");
        let _ = zendo.update(mods(ModsMessage::Done(
            id.clone(),
            Box::new(Ok(Outcome::Changed(done, note))),
        )));
        assert_eq!(zendo.status.as_deref(), Some("Applied 1 mod to Cat Quest"));
        assert!(!zendo.mods.asking(&id, &Confirm::Remove("bepinex".into())));
        let _ = zendo.update(mods(ModsMessage::Done(
            id.clone(),
            Box::new(Ok(Outcome::Read(None))),
        )));
        assert!(matches!(zendo.mods.shown(&id), Some(Shown::Nowhere)));
    }
}
