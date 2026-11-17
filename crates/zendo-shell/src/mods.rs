//! The Mods card on a game's page, without a toolkit: the profile in use,
//! the mods Zendo keeps for the game in order, a Bethesda game's plugins,
//! the folder they go in, Apply and Back to vanilla. The window
//! draws it and runs its [`Job`]s on a thread through [`run`],
//! which calls `zendo_core::mods`.
//!
//! [`ModsState`] holds what the card remembers (what was read, the change in
//! flight, a press that asks first, the folder and the profile name being
//! typed). Its methods are the card's transitions: each returns the [`Job`]
//! to run, or nothing when the press only changed the state. `runnable` is
//! false when the window cannot do the work (no such game, or a test): the
//! state changes the same, but nothing is started.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zendo_core::Game;
use zendo_core::fomod::Selection;
use zendo_core::mods::{self, Added, Applied, ModInfo, Overview, Plugins, ProfileChange};
use zendo_core::nexus;
use zendo_core::plugins::{Plugin, Rank, Style};

use crate::t;

/// What the card knows of one game's mods.
#[derive(Debug)]
pub enum Shown {
    Reading,
    /// No folder to put mods in.
    Nowhere,
    Ready(Overview),
    Failed(String),
}

/// A press that asks first: pressed again, it goes ahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    Remove(String),
    ClearOverwrite,
    DeleteProfile(String),
}

/// A new profile's name, or a new name for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Naming {
    /// The profile being renamed; `None` for a new one.
    pub renaming: Option<String>,
    pub name: String,
}

/// One change to a game's mods, for a thread.
#[derive(Debug, Clone)]
pub enum Work {
    Overview,
    Toggle(String, bool),
    Move(String, bool),
    TogglePlugin(String, bool),
    MovePlugin(String, bool),
    Remove(String),
    SetFolder(Option<String>),
    ClearOverwrite,
    /// A change to the profiles; `apply` puts the new profile in the game.
    Profile(ProfileChange, bool),
    Apply,
    Restore,
    /// Run a mod's installer again.
    Reinstall(String),
    /// Build a mod from its installer with the options chosen.
    Install(Box<mods::Pending>, Selection),
    /// Add an archive; the Nexus key, if set, lets one Nexus named be
    /// followed for updates.
    Add(PathBuf, Option<String>),
    /// Add a file Nexus's window downloaded for a mod: the key, the game's
    /// name on Nexus and the mod.
    AddFromSite(PathBuf, Option<String>, String, u64),
}

/// A [`Work`] for a game.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub work: Work,
}

/// What a [`Work`] came to.
#[derive(Debug, Clone)]
pub enum Outcome {
    Read(Option<Overview>),
    Changed(Overview, String),
    Added(Added, String),
}

/// What the window does once a job ends, beyond showing the state.
#[derive(Debug)]
pub enum After {
    /// The game's mods are read: ask Nexus for their updates (once a session).
    CheckUpdates(String),
    /// The slot is free: an installer that waited for it may open.
    NextInstaller,
    /// An archive with an installer: its options are to be chosen.
    OpenInstaller(String, Box<mods::Pending>),
}

/// What a finished job says: a line for the footer, and what follows.
#[derive(Debug, Default)]
pub struct Done {
    pub status: Option<String>,
    pub after: Vec<After>,
}

/// What the card remembers.
#[derive(Debug, Default)]
pub struct ModsState {
    shown: HashMap<String, Shown>,
    /// The game whose mods are being changed, and what is happening.
    pub busy: Option<(String, String)>,
    confirm: Option<(String, Confirm)>,
    /// The mod folder as typed, before Save.
    folder: Option<(String, String)>,
    /// A profile's name being typed: the game, and the edit.
    naming: Option<(String, Naming)>,
}

impl ModsState {
    #[must_use]
    pub fn shown(&self, id: &str) -> Option<&Shown> {
        self.shown.get(id)
    }

    /// The game's mods, once read.
    #[must_use]
    pub fn overview(&self, id: &str) -> Option<&Overview> {
        match self.shown.get(id)? {
            Shown::Ready(overview) => Some(overview),
            _ => None,
        }
    }

    /// No change is in flight, for any game.
    #[must_use]
    pub fn free(&self) -> bool {
        self.busy.is_none()
    }

    /// What is happening to this game's mods.
    #[must_use]
    pub fn busy_for(&self, id: &str) -> Option<&str> {
        self.busy
            .as_ref()
            .filter(|(game, _)| game == id)
            .map(|(_, busy)| busy.as_str())
    }

    /// Whether this press has been made once already and asks again.
    #[must_use]
    pub fn asking(&self, id: &str, what: &Confirm) -> bool {
        self.confirm
            .as_ref()
            .is_some_and(|(game, ask)| game == id && ask == what)
    }

    /// The name being typed for this game's profile.
    #[must_use]
    pub fn naming(&self, id: &str) -> Option<&Naming> {
        self.naming
            .as_ref()
            .filter(|(game, _)| game == id)
            .map(|(_, naming)| naming)
    }

    /// The folder as typed for this game, before Save.
    #[must_use]
    pub fn typed_folder(&self, id: &str) -> Option<&str> {
        self.folder
            .as_ref()
            .filter(|(game, _)| game == id)
            .map(|(_, folder)| folder.as_str())
    }

    /// The game's page opened: reads its mods unless they were read.
    pub fn open(&mut self, id: &str) -> Job {
        self.shown.entry(id.to_owned()).or_insert(Shown::Reading);
        Job {
            id: id.to_owned(),
            work: Work::Overview,
        }
    }

    /// The game's mods as they now stand.
    pub fn ready(&mut self, id: String, overview: Overview) {
        self.shown.insert(id, Shown::Ready(overview));
    }

    /// Starts a change: one at a time, and only when it can run.
    fn start(&mut self, id: &str, busy: String, work: Work, runnable: bool) -> Option<Job> {
        if self.busy.is_some() || !runnable {
            return None;
        }
        self.busy = Some((id.to_owned(), busy));
        Some(Job {
            id: id.to_owned(),
            work,
        })
    }

    pub fn toggle(&mut self, id: &str, mod_id: &str, on: bool, runnable: bool) -> Option<Job> {
        let work = Work::Toggle(mod_id.to_owned(), on);
        self.start(id, t!("saving"), work, runnable)
    }

    pub fn move_mod(&mut self, id: &str, mod_id: &str, up: bool, runnable: bool) -> Option<Job> {
        let work = Work::Move(mod_id.to_owned(), up);
        self.start(id, t!("saving"), work, runnable)
    }

    pub fn toggle_plugin(&mut self, id: &str, name: &str, on: bool, runnable: bool) -> Option<Job> {
        let work = Work::TogglePlugin(name.to_owned(), on);
        self.start(id, t!("saving"), work, runnable)
    }

    pub fn move_plugin(&mut self, id: &str, name: &str, up: bool, runnable: bool) -> Option<Job> {
        let work = Work::MovePlugin(name.to_owned(), up);
        self.start(id, t!("saving"), work, runnable)
    }

    /// Asks the first time, deletes the second.
    pub fn remove(&mut self, id: &str, mod_id: &str, runnable: bool) -> Option<Job> {
        if !self.second_press(id, Confirm::Remove(mod_id.to_owned())) {
            return None;
        }
        let work = Work::Remove(mod_id.to_owned());
        self.start(id, t!("deleting"), work, runnable)
    }

    pub fn clear_overwrite(&mut self, id: &str, runnable: bool) -> Option<Job> {
        if !self.second_press(id, Confirm::ClearOverwrite) {
            return None;
        }
        self.start(id, t!("deleting"), Work::ClearOverwrite, runnable)
    }

    /// `true` when this press is the second of the same; otherwise it is
    /// remembered.
    fn second_press(&mut self, id: &str, ask: Confirm) -> bool {
        if self.asking(id, &ask) {
            return true;
        }
        self.confirm = Some((id.to_owned(), ask));
        false
    }

    /// The folder as it is typed.
    pub fn type_folder(&mut self, id: &str, folder: String) {
        self.folder = Some((id.to_owned(), folder));
    }

    pub fn save_folder(&mut self, id: &str, runnable: bool) -> Option<Job> {
        let folder = match self.folder.take() {
            Some((game, folder)) if game == id => folder,
            other => {
                self.folder = other;
                return None;
            }
        };
        self.start(id, t!("saving"), Work::SetFolder(Some(folder)), runnable)
    }

    /// Back to where the game is known to take its mods.
    pub fn known_folder(&mut self, id: &str, runnable: bool) -> Option<Job> {
        self.folder = None;
        self.start(id, t!("saving"), Work::SetFolder(None), runnable)
    }

    pub fn switch_profile(
        &mut self,
        id: &str,
        name: &str,
        running: bool,
        runnable: bool,
    ) -> Option<Job> {
        let change = ProfileChange::Switch(name.to_owned());
        self.start(
            id,
            t!("mods-switching"),
            Work::Profile(change, !running),
            runnable,
        )
    }

    /// Opens the name field for a new profile, or to rename the one in use.
    pub fn start_naming(&mut self, id: &str, renaming: bool) {
        let current = self
            .overview(id)
            .map_or_default(|overview| overview.profile.clone());
        let naming = Naming {
            name: if renaming {
                current.clone()
            } else {
                String::new()
            },
            renaming: renaming.then_some(current),
        };
        self.naming = Some((id.to_owned(), naming));
    }

    pub fn name_typed(&mut self, id: &str, name: String) {
        if let Some((game, naming)) = &mut self.naming
            && game == id
        {
            naming.name = name;
        }
    }

    pub fn save_profile(&mut self, id: &str, running: bool, runnable: bool) -> Option<Job> {
        match self.naming.take() {
            Some((game, naming)) if game == id => {
                let change = match naming.renaming {
                    Some(old) => ProfileChange::Rename {
                        old,
                        new: naming.name,
                    },
                    None => ProfileChange::New(naming.name),
                };
                self.start(id, t!("saving"), Work::Profile(change, !running), runnable)
            }
            other => {
                self.naming = other;
                None
            }
        }
    }

    pub fn cancel_profile(&mut self) {
        self.naming = None;
    }

    /// Asks the first time, deletes the second.
    pub fn delete_profile(&mut self, id: &str, running: bool, runnable: bool) -> Option<Job> {
        let name = self.overview(id)?.profile.clone();
        if !self.second_press(id, Confirm::DeleteProfile(name.clone())) {
            return None;
        }
        let change = ProfileChange::Delete(name);
        self.start(
            id,
            t!("deleting"),
            Work::Profile(change, !running),
            runnable,
        )
    }

    /// Apply, or Back to vanilla, never while the game runs.
    ///
    /// # Errors
    ///
    /// The line to show when the game runs.
    pub fn apply(
        &mut self,
        id: &str,
        apply: bool,
        running: bool,
        runnable: bool,
    ) -> Result<Option<Job>, String> {
        if running {
            return Err(t!("mods-close-game-first"));
        }
        let (busy, work) = if apply {
            (t!("mods-applying"), Work::Apply)
        } else {
            (t!("mods-restoring"), Work::Restore)
        };
        Ok(self.start(id, busy, work, runnable))
    }

    /// Run a mod's installer again.
    pub fn reinstall(&mut self, id: &str, mod_id: &str, runnable: bool) -> Option<Job> {
        let work = Work::Reinstall(mod_id.to_owned());
        self.start(id, t!("mods-reading-installer"), work, runnable)
    }

    /// Builds a mod from its installer with the options chosen.
    pub fn install(
        &mut self,
        id: &str,
        pending: Box<mods::Pending>,
        selection: Selection,
        runnable: bool,
    ) -> Option<Job> {
        let work = Work::Install(pending, selection);
        self.start(id, t!("installer-installing"), work, runnable)
    }

    /// The file dialog for an archive is open: nothing else changes.
    pub fn choosing(&mut self, id: &str, runnable: bool) -> bool {
        if self.busy.is_some() || !runnable {
            return false;
        }
        self.busy = Some((id.to_owned(), t!("mods-choosing")));
        true
    }

    /// The file dialog closed.
    pub fn chosen(&mut self) {
        self.busy = None;
    }

    /// Adds the archive chosen.
    pub fn add(
        &mut self,
        id: &str,
        archive: PathBuf,
        key: Option<String>,
        runnable: bool,
    ) -> Option<Job> {
        let work = Work::Add(archive, key);
        self.start(id, t!("mods-unpacking"), work, runnable)
    }

    /// Adds the file Nexus's window downloaded for a mod.
    pub fn add_from_site(
        &mut self,
        id: &str,
        archive: PathBuf,
        key: Option<String>,
        domain: String,
        mod_id: u64,
        runnable: bool,
    ) -> Option<Job> {
        let work = Work::AddFromSite(archive, key, domain, mod_id);
        self.start(id, t!("mods-unpacking"), work, runnable)
    }

    /// A job ended: the state it leaves, and what the window does next.
    pub fn done(&mut self, id: &str, result: Result<Outcome, String>) -> Done {
        let mut done = Done::default();
        match result {
            Ok(Outcome::Read(Some(overview))) => {
                self.shown.insert(id.to_owned(), Shown::Ready(overview));
                done.after.push(After::CheckUpdates(id.to_owned()));
            }
            Ok(Outcome::Read(None)) => {
                self.shown.insert(id.to_owned(), Shown::Nowhere);
            }
            Err(err) if self.is_reading(id) => {
                self.shown.insert(id.to_owned(), Shown::Failed(err));
            }
            Ok(Outcome::Changed(overview, note)) => {
                self.finished();
                if !note.is_empty() {
                    done.status = Some(note);
                }
                self.shown.insert(id.to_owned(), Shown::Ready(overview));
                done.after.push(After::CheckUpdates(id.to_owned()));
                done.after.push(After::NextInstaller);
            }
            Ok(Outcome::Added(Added::Done(overview), note)) => {
                self.finished();
                done.status = Some(note);
                self.shown.insert(id.to_owned(), Shown::Ready(*overview));
                done.after.push(After::CheckUpdates(id.to_owned()));
                // An installer that came while this was adding waited for it.
                done.after.push(After::NextInstaller);
            }
            Ok(Outcome::Added(Added::Choose(pending), _)) => {
                self.finished();
                done.after
                    .push(After::OpenInstaller(id.to_owned(), pending));
            }
            Err(err) => {
                self.finished();
                done.status = Some(err);
                done.after.push(After::NextInstaller);
            }
        }
        done
    }

    /// A read is the only job that fails while the card still says Reading
    /// and no change to this game is in flight.
    fn is_reading(&self, id: &str) -> bool {
        matches!(self.shown.get(id), Some(Shown::Reading)) && self.busy_for(id).is_none()
    }

    fn finished(&mut self) {
        self.busy = None;
        self.confirm = None;
    }
}

/// Does the work, on a thread; the words say what it came to.
///
/// # Errors
///
/// What went wrong, in words, for the footer.
pub fn run(work: Work, game: &Game) -> Result<Outcome, String> {
    run_inner(work, game).map_err(|err| format!("{err:#}"))
}

fn changed(overview: Overview) -> (Overview, String) {
    (overview, String::new())
}

fn run_inner(work: Work, game: &Game) -> anyhow::Result<Outcome> {
    let (overview, note) = match work {
        Work::Overview => return Ok(Outcome::Read(mods::overview(game)?)),
        Work::Toggle(mod_id, on) => changed(mods::set_enabled(game, &mod_id, on)?),
        Work::Move(mod_id, up) => changed(mods::move_mod(game, &mod_id, up)?),
        Work::TogglePlugin(name, on) => changed(mods::set_plugin(game, &name, on)?),
        Work::MovePlugin(name, up) => changed(mods::move_plugin(game, &name, up)?),
        Work::Remove(mod_id) => (mods::remove(game, &mod_id)?, t!("mods-deleted")),
        Work::SetFolder(Some(folder)) => (
            mods::set_folder(game, Some(&folder))?,
            t!("mods-new-folder"),
        ),
        Work::SetFolder(None) => changed(mods::set_folder(game, None)?),
        Work::ClearOverwrite => (mods::clear_overwrite(game)?, t!("mods-overwrite-deleted")),
        Work::Profile(change, apply) => {
            let overview = mods::change_profiles(game, &change, apply)?;
            let note = profile_note(&change, &overview);
            (overview, note)
        }
        Work::Apply => {
            let overview = mods::apply(game)?;
            let note = applied_note(&overview, &game.title);
            (overview, note)
        }
        Work::Restore => {
            let overview = mods::restore(game)?;
            let note = applied_note(&overview, &game.title);
            (overview, note)
        }
        Work::Install(pending, selection) => {
            let overview = mods::install(game, &pending, &selection)?;
            (
                overview,
                t!("installer-installed", name = pending.name.as_str()),
            )
        }
        Work::Reinstall(mod_id) => {
            let pending = mods::reinstall(game, &mod_id)?;
            return Ok(Outcome::Added(
                Added::Choose(Box::new(pending)),
                String::new(),
            ));
        }
        Work::Add(archive, key) => {
            let (added, note) =
                if let Some(added) = add_as_from_nexus(game, key.as_deref(), &archive) {
                    added
                } else {
                    let name = archive
                        .file_name()
                        .map_or_default(|name| name.to_string_lossy().into_owned());
                    (mods::add(game, &archive)?, t!("mods-added", name = name))
                };
            return Ok(Outcome::Added(added, note));
        }
        Work::AddFromSite(archive, key, domain, mod_id) => {
            let (added, note) = add_from_site(game, key.as_deref(), &domain, mod_id, &archive)?;
            return Ok(Outcome::Added(added, note));
        }
    };
    Ok(Outcome::Changed(overview, note))
}

/// A file Nexus's window downloaded: the mod's file it is, by its name,
/// with the key; else a mod of its own. The download goes either way.
///
/// # Errors
///
/// When Nexus could not be asked, or the mod could not be added.
pub fn add_from_site(
    game: &Game,
    key: Option<&str>,
    domain: &str,
    mod_id: u64,
    archive: &Path,
) -> anyhow::Result<(Added, String)> {
    // A failed lookup is not "the name carries no ids": the caller says
    // what went wrong instead of adding the mod without its identity.
    let identified = match key {
        Some(key) => nexus::identify(key, domain, archive, Some(mod_id))?,
        None => None,
    };
    if let Some((mod_id, fetched)) = identified {
        let (added, updated) = mods::add_from_nexus(game, &fetched, domain, mod_id)?;
        return Ok((added, added_note(updated, &fetched, &game.title)));
    }
    let name = archive
        .file_name()
        .map_or_default(|name| name.to_string_lossy().into_owned());
    let added = mods::add(game, archive);
    let _ = std::fs::remove_file(archive);
    Ok((added?, t!("mods-added", name = name)))
}

/// What a change to the profiles says.
#[must_use]
pub fn profile_note(change: &ProfileChange, overview: &Overview) -> String {
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

/// What a finished Apply or Back to vanilla says.
#[must_use]
pub fn applied_note(overview: &Overview, title: &str) -> String {
    match overview.applied {
        Applied::Current(count) => t!("mods-applied-count", count = count, title = title),
        Applied::No => t!("mods-vanilla", title = title),
        Applied::Changed => t!("mods-applied", title = title),
    }
}

/// What adding a file from Nexus says.
#[must_use]
pub fn added_note(updated: bool, fetched: &nexus::Fetched, title: &str) -> String {
    let version = fetched
        .file
        .version
        .as_deref()
        .filter(|version| !version.is_empty());
    let name = fetched.mod_name.as_str();
    match (updated, version) {
        (true, Some(version)) => t!("nexus-updated-to", name = name, version = version),
        (true, None) => t!("nexus-updated", name = name),
        (false, Some(version)) => t!(
            "nexus-added-version",
            name = name,
            version = version,
            title = title
        ),
        (false, None) => t!("nexus-added", name = name, title = title),
    }
}

/// An archive Nexus named (`<name>-<mod id>-<version>-<time>`), added by
/// hand: as the file of its mod, so its updates are followed, when the
/// game is on Nexus and the key finds it. The archive chosen stays where it
/// is; a copy is taken in. `None` adds it as any other.
fn add_as_from_nexus(game: &Game, key: Option<&str>, archive: &Path) -> Option<(Added, String)> {
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

/// How a line of the card is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Dim,
    Good,
    Bad,
}

/// Delete, or Really delete once pressed.
#[must_use]
pub fn delete_label(asking: bool) -> String {
    if asking {
        t!("really-delete")
    } else {
        t!("delete")
    }
}

/// Where the mods go, in words.
#[must_use]
pub fn place_line(overview: &Overview) -> String {
    match overview.known {
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
    }
}

/// A mod's second line: where it came from, its files and what it wins and
/// loses against.
#[must_use]
pub fn mod_facts(info: &ModInfo) -> String {
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
    facts.join(" · ")
}

/// The plugins' note, and whether it is a warning.
#[must_use]
pub fn plugins_note(plugins: &Plugins, style: Option<Style>) -> (String, bool) {
    let note = match (&plugins.file, style) {
        (Some(_), Some(Style::Timestamps)) => t!("plugins-note-dates"),
        (Some(_), _) => t!("plugins-note"),
        (None, _) => t!("plugins-note-no-prefix"),
    };
    (note, plugins.file.is_none())
}

/// The load-order style of the game's plugins, when it is known.
#[must_use]
pub fn plugins_style(overview: &Overview) -> Option<Style> {
    overview
        .known
        .and_then(|known| known.plugins)
        .map(|rules| rules.style)
}

/// A plugin's second line.
#[must_use]
pub fn plugin_facts(plugin: &Plugin) -> String {
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
    facts.join(" · ")
}

/// What is wrong with a plugin; empty when nothing is.
#[must_use]
pub fn plugin_problems(plugin: &Plugin) -> String {
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
    problems.join(" · ")
}

/// Whether Apply writes a plugin order.
#[must_use]
pub fn writes_plugins(overview: &Overview) -> bool {
    overview
        .plugins
        .as_ref()
        .is_some_and(|plugins| plugins.file.is_some())
}

/// The line beside Apply, and its colour.
#[must_use]
pub fn apply_state(overview: &Overview, running: bool) -> (String, Tone) {
    if running {
        return (t!("mods-state-running"), Tone::Dim);
    }
    match overview.applied {
        Applied::No => (t!("mods-state-no"), Tone::Dim),
        Applied::Current(count) if writes_plugins(overview) => {
            (t!("mods-state-applied-plugins", count = count), Tone::Good)
        }
        Applied::Current(count) => (t!("mods-state-applied", count = count), Tone::Good),
        Applied::Changed => (t!("mods-state-changed"), Tone::Bad),
    }
}

/// Whether Back to vanilla may be pressed.
#[must_use]
pub fn can_restore(free: bool, running: bool, overview: &Overview) -> bool {
    free && !running && overview.applied != Applied::No
}

/// Whether Apply may be pressed.
#[must_use]
pub fn can_apply(free: bool, running: bool, overview: &Overview) -> bool {
    let any_on = overview.mods.iter().any(|info| info.enabled)
        || overview.overwrite > 0
        || writes_plugins(overview);
    free && !running && (any_on || overview.applied != Applied::No)
}

/// The folder field takes typing only while nothing is applied.
#[must_use]
pub fn folder_editable(overview: &Overview) -> bool {
    overview.applied == Applied::No
}

/// Save shows when the typed folder differs from the one in use.
#[must_use]
pub fn folder_save_shown(typed: Option<&str>, overview: &Overview) -> bool {
    typed.is_some_and(|typed| typed != overview.folder)
}

/// "Use the known folder" shows for a folder the user set, while nothing is
/// applied.
#[must_use]
pub fn known_folder_shown(overview: &Overview) -> bool {
    overview.custom_folder && folder_editable(overview)
}

/// The last profile cannot be deleted.
#[must_use]
pub fn profile_delete_enabled(free: bool, overview: &Overview) -> bool {
    free && overview.profiles.len() > 1
}

/// The hint in the profile's name field.
#[must_use]
pub fn name_placeholder(renaming: bool) -> String {
    if renaming {
        t!("mods-profile-rename")
    } else {
        t!("mods-profile-new-name")
    }
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

    fn read(state: &mut ModsState, id: &str, overview: Overview) {
        let _ = state.open(id);
        let _ = state.done(id, Ok(Outcome::Read(Some(overview))));
    }

    #[test]
    fn a_profile_is_named_in_place_and_deleting_it_asks_first() {
        let mut state = ModsState::default();
        let id = "epic:cat";
        let mut two = overview(Applied::No);
        two.profiles.push("Light".into());
        read(&mut state, id, two);

        state.start_naming(id, true);
        let naming = state.naming(id).unwrap().clone();
        assert_eq!(naming.renaming.as_deref(), Some("Default"));
        assert_eq!(naming.name, "Default");
        state.cancel_profile();
        assert_eq!(state.naming(id), None);

        state.start_naming(id, false);
        state.name_typed(id, "Run 2".into());
        let naming = state.naming(id).unwrap().clone();
        assert_eq!((naming.renaming, naming.name.as_str()), (None, "Run 2"));
        assert!(state.save_profile(id, false, false).is_none());
        assert_eq!(state.naming(id), None);

        assert!(state.delete_profile(id, false, true).is_none());
        assert!(state.asking(id, &Confirm::DeleteProfile("Default".into())));
        let job = state.delete_profile(id, false, true).unwrap();
        assert!(matches!(
            job.work,
            Work::Profile(ProfileChange::Delete(_), true)
        ));
        assert!(state.busy.is_some());

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
    fn installing_is_one_change_and_needs_a_game_to_run_on() {
        use zendo_core::fomod::Installer;
        let installer = Installer::parse(
            br#"<config><moduleName>T</moduleName><installSteps order="Explicit">
            </installSteps></config>"#,
        )
        .unwrap();
        let pending = Box::new(mods::test_pending("T", installer));
        let selection = pending
            .installer
            .selection_from(&pending.chosen, &pending.present);
        let mut state = ModsState::default();
        assert!(
            state
                .install("g", pending.clone(), selection.clone(), false)
                .is_none(),
            "not runnable: no job and nothing busy"
        );
        assert!(state.free());
        let job = state.install("g", pending.clone(), selection.clone(), true);
        assert!(matches!(
            job,
            Some(Job {
                work: Work::Install(..),
                ..
            })
        ));
        assert_eq!(state.busy_for("g"), Some("Installing…"));
        assert!(state.install("g", pending, selection, true).is_none());
    }

    #[test]
    fn deleting_asks_first_and_a_change_shows_when_done() {
        let mut state = ModsState::default();
        let id = "epic:cat";
        read(&mut state, id, overview(Applied::No));
        assert!(matches!(state.shown(id), Some(Shown::Ready(_))));

        assert!(state.remove(id, "bepinex", true).is_none());
        assert!(state.asking(id, &Confirm::Remove("bepinex".into())));
        assert!(state.remove(id, "bepinex", true).is_some());
        // One change at a time.
        assert!(state.toggle(id, "bepinex", false, true).is_none());

        let now = overview(Applied::Current(1));
        let note = applied_note(&now, "Cat Quest");
        assert_eq!(note, "Applied 1 mod to Cat Quest");
        let done = state.done(id, Ok(Outcome::Changed(now, note)));
        assert_eq!(done.status.as_deref(), Some("Applied 1 mod to Cat Quest"));
        assert!(matches!(done.after[1], After::NextInstaller));
        assert!(state.free());
        assert!(!state.asking(id, &Confirm::Remove("bepinex".into())));
        assert_eq!(state.overview(id).unwrap().applied, Applied::Current(1));

        let _ = state.done(id, Ok(Outcome::Read(None)));
        assert!(matches!(state.shown(id), Some(Shown::Nowhere)));
    }

    #[test]
    fn a_failed_read_shows_and_a_failed_change_keeps_the_mods() {
        let mut state = ModsState::default();
        let _ = state.open("a");
        let done = state.done("a", Err("no".into()));
        assert!(done.status.is_none());
        assert!(matches!(state.shown("a"), Some(Shown::Failed(_))));

        read(&mut state, "b", overview(Applied::No));
        let job = state.toggle("b", "bepinex", false, true);
        assert!(job.is_some());
        let done = state.done("b", Err("disk full".into()));
        assert_eq!(done.status.as_deref(), Some("disk full"));
        assert!(state.free());
        assert!(state.overview("b").is_some());
    }

    #[test]
    fn apply_is_refused_while_the_game_runs() {
        let mut state = ModsState::default();
        read(&mut state, "a", overview(Applied::No));
        assert!(state.apply("a", true, true, true).is_err());
        assert!(state.free());
        assert!(state.apply("a", true, false, true).unwrap().is_some());
    }

    #[test]
    fn the_buttons_follow_the_state() {
        let mut o = overview(Applied::No);
        assert!(can_apply(true, false, &o));
        assert!(!can_restore(true, false, &o));
        assert!(!can_apply(true, true, &o));
        o.mods[0].enabled = false;
        assert!(!can_apply(true, false, &o));
        o.applied = Applied::Changed;
        assert!(can_apply(true, false, &o) && can_restore(true, false, &o));
        assert!(!folder_editable(&o));
        assert_eq!(apply_state(&o, false).1, Tone::Bad);
        assert_eq!(apply_state(&o, true).1, Tone::Dim);
        assert!(!profile_delete_enabled(true, &o));
    }
}
