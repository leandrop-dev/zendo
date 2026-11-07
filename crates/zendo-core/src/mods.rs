//! Mods: archives unpacked into a folder of Zendo's per game, then copied
//! into the game's folder on Apply and taken out again on Back to vanilla.
//!
//! Each game's are in `$XDG_DATA_HOME/zendo/mods/<game id>/`:
//!
//! - `mods.toml`: the mods in order (a later one wins a file both have),
//!   whether each is on and the Nexus file it came from, the game's mod
//!   folder when the user set one, and its name on Nexus when the user
//!   chose it; a Bethesda game's plugin order ([`plugins`]); with the
//!   profile that order is, and the other profiles' orders and the mods
//!   and plugins on in each;
//! - `downloads/`: files from Nexus while they download, deleted once
//!   unpacked;
//! - `installers/<mod>/`: a mod's archive as unpacked when it has a FOMOD
//!   installer, to choose its options again;
//! - `files/<mod>/`: each mod as its archive held it;
//! - `applied.json`: every file and folder Apply placed, so Back to vanilla
//!   removes exactly those, and the plugin lists it wrote in the game's
//!   prefix and the plugin dates it set, to put back;
//! - `backup/`: the game's own files a mod replaced, put back after;
//! - `overwrite/`: what the game wrote while modded into folders Zendo
//!   made, or mod files it changed (configs, logs), applied again next time
//!   after every mod.
//!
//! Files are copied, not linked, so a game writing to one never changes the
//! mod kept here; on btrfs or XFS the kernel shares the copy's blocks until
//! either changes (`copy_file_range`).
//!
//! Where mods go comes from a few games Zendo knows ([`KNOWN`]): `Data` for
//! Bethesda's, `Mods` for Stardew Valley; any other game takes them in its
//! own folder, or wherever the user says.
//!
//! This file holds the types and the calls the app makes; the work is in
//! `known` (the games), `add` (taking mods in), `order` (plugin order),
//! `apply` (Apply and Back to vanilla) and `files` (folder helpers).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::fomod::{self, Installer, Present, Selection};
use crate::nexus::{self, Files};
use crate::plugins::{self, Plugin};
use crate::{Game, launch, paths};

mod add;
mod apply;
mod files;
mod known;
mod order;

pub use known::{KNOWN, Known, known};

use add::{find_installer, read_installer};
use files::{case_in, check_relative, entries, file_name, walk};
use order::order_of;

/// One change at a time to any game's mods: the window's, a download from
/// Nexus finishing on its own thread, and another Zendo window's.
static LOCK: Mutex<()> = Mutex::new(());

/// Held while mods change: this process's lock, and the lock file another
/// process's change waits on (released when dropped).
struct Locked {
    _thread: MutexGuard<'static, ()>,
    _process: Option<fs::File>,
}

fn lock() -> Locked {
    // A poisoned lock guards nothing in memory: a change that panicked
    // left at worst files on disk, which Apply and Back to vanilla check.
    let thread = LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    // Without a data folder there are no mods to change, so no lock file.
    let process = paths::data_home().and_then(|dir| {
        let path = dir.join("zendo/mods.lock");
        fs::create_dir_all(dir.join("zendo")).ok()?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .ok()?;
        file.lock().ok()?;
        Some(file)
    });
    Locked {
        _thread: thread,
        _process: process,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    /// The mod folder the user chose, from the game's folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    folder: Option<String>,
    /// The game's name on Nexus, as the user chose it for a link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nexus: Option<String>,
    /// The profile `mods` is in; empty for the first, [`DEFAULT_PROFILE`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    active_profile: String,
    #[serde(default, rename = "mod")]
    mods: Vec<Entry>,
    /// The plugin order, for Bethesda's games: every plugin seen, on or not.
    #[serde(default, rename = "plugin", skip_serializing_if = "Vec::is_empty")]
    plugins: Vec<PluginEntry>,
    /// The other profiles.
    #[serde(default, rename = "profile", skip_serializing_if = "Vec::is_empty")]
    profiles: Vec<Profile>,
}

/// The name of the profile a game's mods start in.
pub const DEFAULT_PROFILE: &str = "Default";

/// A profile not in use: its order of the mods and the ones on in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    name: String,
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    enabled: Vec<String>,
    #[serde(default, rename = "plugin", skip_serializing_if = "Vec::is_empty")]
    plugins: Vec<PluginEntry>,
}

/// A plugin in the order kept, and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginEntry {
    name: String,
    enabled: bool,
}

impl List {
    fn active(&self) -> &str {
        if self.active_profile.is_empty() {
            DEFAULT_PROFILE
        } else {
            &self.active_profile
        }
    }

    /// Every profile's name, the one in use among them, sorted.
    fn profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .profiles
            .iter()
            .map(|profile| profile.name.clone())
            .chain([self.active().to_owned()])
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        names
    }

    /// The list as it stands, as a profile under the name in use.
    fn current(&self) -> Profile {
        Profile {
            name: self.active().to_owned(),
            order: self.mods.iter().map(|entry| entry.id.clone()).collect(),
            enabled: self
                .mods
                .iter()
                .filter(|entry| entry.enabled)
                .map(|entry| entry.id.clone())
                .collect(),
            plugins: self.plugins.clone(),
        }
    }

    fn stored_plugins(&self) -> Vec<(String, bool)> {
        self.plugins
            .iter()
            .map(|entry| (entry.name.clone(), entry.enabled))
            .collect()
    }

    /// Keeps this plugin order; plugins kept before that are not there now
    /// (their mod off) stay after it, for when they come back.
    fn keep_plugins(&mut self, order: &[(String, bool)]) {
        let mut kept: Vec<PluginEntry> = order
            .iter()
            .map(|(name, enabled)| PluginEntry {
                name: name.clone(),
                enabled: *enabled,
            })
            .collect();
        for entry in &self.plugins {
            if !kept
                .iter()
                .any(|other| other.name.eq_ignore_ascii_case(&entry.name))
            {
                kept.push(entry.clone());
            }
        }
        self.plugins = kept;
    }

    /// A name for a new profile or a rename: trimmed, short, and not
    /// another profile's (whatever the case); `keep` is the name being
    /// renamed.
    fn check_name(&self, name: &str, keep: Option<&str>) -> anyhow::Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Give the profile a name");
        }
        if name.chars().count() > 40 {
            bail!("Profile names are 40 characters at most");
        }
        if name.chars().any(char::is_control) {
            bail!("Profile names are one line of text");
        }
        let taken = self.profile_names().iter().any(|other| {
            other.to_lowercase() == name.to_lowercase()
                && keep.is_none_or(|keep| other.to_lowercase() != keep.to_lowercase())
        });
        if taken {
            bail!("There is a profile named {name} already");
        }
        Ok(name.to_owned())
    }

    /// Starts a profile as a copy of the one in use and switches to it.
    fn new_profile(&mut self, name: &str) -> anyhow::Result<()> {
        let name = self.check_name(name, None)?;
        let current = self.current();
        self.profiles.push(current);
        self.active_profile = name;
        Ok(())
    }

    /// Puts another profile's order and mods on in the list; the one in
    /// use is kept as it stands. Mods added since that profile was left
    /// come last, off.
    fn switch_profile(&mut self, name: &str) -> anyhow::Result<()> {
        if name == self.active() {
            return Ok(());
        }
        let at = self
            .profiles
            .iter()
            .position(|profile| profile.name == name)
            .with_context(|| format!("there is no profile named {name}"))?;
        let target = self.profiles.remove(at);
        let current = self.current();
        self.profiles.insert(at, current);
        let mut left: Vec<Entry> = std::mem::take(&mut self.mods);
        for id in &target.order {
            if let Some(found) = left.iter().position(|entry| &entry.id == id) {
                let mut entry = left.remove(found);
                entry.enabled = target.enabled.contains(id);
                self.mods.push(entry);
            }
        }
        for mut entry in left {
            entry.enabled = false;
            self.mods.push(entry);
        }
        self.plugins = target.plugins;
        self.active_profile = target.name;
        Ok(())
    }

    fn rename_profile(&mut self, old: &str, new: &str) -> anyhow::Result<()> {
        let new = self.check_name(new, Some(old))?;
        if old == self.active() {
            self.active_profile = new;
            return Ok(());
        }
        let profile = self
            .profiles
            .iter_mut()
            .find(|profile| profile.name == old)
            .with_context(|| format!("there is no profile named {old}"))?;
        profile.name = new;
        Ok(())
    }

    /// Deletes a profile; the one in use after switching to the next.
    fn delete_profile(&mut self, name: &str) -> anyhow::Result<()> {
        if name == self.active() {
            let names = self.profile_names();
            let at = names.iter().position(|other| other == name).unwrap_or(0);
            let Some(next) = names
                .iter()
                .cycle()
                .skip(at + 1)
                .take(names.len())
                .find(|other| *other != name)
                .cloned()
            else {
                bail!("A game keeps at least one profile");
            };
            self.switch_profile(&next)?;
        }
        let before = self.profiles.len();
        self.profiles.retain(|profile| profile.name != name);
        if self.profiles.len() == before {
            bail!("there is no profile named {name}");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    name: String,
    enabled: bool,
    /// The folder inside the archive whose contents are the mod, when it
    /// wrapped them in others.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    root: String,
    /// The archive it came from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    /// The file on Nexus it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nexus: Option<NexusSource>,
    /// Built by its FOMOD installer, kept in `installers/<id>/`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    installer: bool,
    /// The installer's options chosen, as `step/group/option`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chosen: Vec<String>,
}

impl Entry {
    /// What Apply put in, to tell when the mod's files changed since.
    fn stamp(&self) -> String {
        let file = self.nexus.as_ref().map_or(0, |nexus| nexus.file_id);
        let mut stamp = format!("{}#{}#{file}", self.id, self.source);
        if !self.chosen.is_empty() {
            stamp.push('#');
            stamp.push_str(&self.chosen.join("|"));
        }
        stamp
    }
}

/// A mod whose archive has a FOMOD installer, waiting for the options the
/// user chooses.
#[derive(Debug, Clone)]
pub struct Pending {
    /// The mod's name.
    pub name: String,
    pub installer: Installer,
    /// What file conditions see.
    pub present: Present,
    /// The options chosen last time, for an update or Options….
    pub chosen: Vec<String>,
    /// The folder holding `fomod/`, where the installer's paths start.
    root: PathBuf,
    what: PendingKind,
}

#[derive(Debug, Clone)]
enum PendingKind {
    /// A new mod, unpacked in `raw` (`files/.adding-<id>`).
    New {
        id: String,
        raw: PathBuf,
        source: String,
        nexus: Option<NexusSource>,
    },
    /// A newer archive for a mod, unpacked in `raw`.
    Update {
        id: String,
        raw: PathBuf,
        source: String,
        nexus: Option<NexusSource>,
    },
    /// The mod's kept installer, run again.
    Options { id: String },
}

impl Pending {
    /// A picture the installer names, found whatever its case.
    #[must_use]
    pub fn image(&self, path: &str) -> Option<PathBuf> {
        let path = fomod::normalise(path);
        if path.is_empty() || !fomod::safe(&path) {
            return None;
        }
        let found = self.root.join(case_in(&self.root, Path::new(&path)));
        found.is_file().then_some(found)
    }

    /// Whether it replaces a mod (an update or Options…) rather than adds.
    #[must_use]
    pub fn replaces(&self) -> bool {
        !matches!(self.what, PendingKind::New { .. })
    }

    fn id(&self) -> &str {
        match &self.what {
            PendingKind::New { id, .. }
            | PendingKind::Update { id, .. }
            | PendingKind::Options { id } => id,
        }
    }
}

/// A mod waiting for an installer, with nothing on disk, for the window's
/// tests.
#[doc(hidden)]
#[must_use]
pub fn test_pending(name: &str, installer: Installer) -> Pending {
    Pending {
        name: name.to_owned(),
        installer,
        present: Present::default(),
        chosen: Vec::new(),
        root: PathBuf::new(),
        what: PendingKind::Options { id: String::new() },
    }
}

/// What adding an archive came to: the mod in, or its installer to run.
#[derive(Debug, Clone)]
pub enum Added {
    Done(Box<Overview>),
    Choose(Box<Pending>),
}

/// The file on Nexus Mods a mod came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusSource {
    /// The game's name on Nexus.
    pub domain: String,
    pub mod_id: u64,
    pub file_id: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
}

/// What Apply placed, so Back to vanilla can take it out.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    game_dir: PathBuf,
    /// The mods applied, in order.
    mods: Vec<String>,
    /// Each one's [`Entry::stamp`]; missing from manifests before them.
    #[serde(default)]
    stamps: Vec<String>,
    /// Files placed, from the game's folder.
    files: Vec<Placed>,
    /// The game's own files a mod replaced, kept in `backup/`.
    backups: Vec<PathBuf>,
    /// Folders made, from the game's folder, outermost first.
    created: Vec<PathBuf>,
    /// The plugin order written, for Bethesda's games.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plugins: Option<Vec<(String, bool)>>,
    /// The game's plugin lists in the prefix, as they were.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lists: Vec<ListBackup>,
    /// Folders made in the prefix for them, outermost first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    made: Vec<PathBuf>,
    /// The game's own plugins' modified times before Apply set them (the
    /// order Fallout 3 and New Vegas load in), from the game's folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    times: Vec<(PathBuf, Option<SystemTime>)>,
}

/// A plugin list file in the prefix, and what it held; `None` when there
/// was none.
#[derive(Debug, Serialize, Deserialize)]
struct ListBackup {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Placed {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

/// One game's mods, as the window shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    pub game_dir: PathBuf,
    /// Where mods go.
    pub target: PathBuf,
    /// That folder from the game's, as set or known ("" for the game's own).
    pub folder: String,
    /// Whether the user set it.
    pub custom_folder: bool,
    pub known: Option<&'static Known>,
    /// The game's name on Nexus, when known.
    pub nexus: Option<String>,
    /// The profile in use, and every profile's name, sorted.
    pub profile: String,
    pub profiles: Vec<String>,
    pub mods: Vec<ModInfo>,
    /// Files in `overwrite/`.
    pub overwrite: usize,
    pub applied: Applied,
    /// The plugins, for Bethesda's games.
    pub plugins: Option<Box<Plugins>>,
}

/// A Bethesda game's plugins, in the order they load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugins {
    pub list: Vec<Plugin>,
    /// The `plugins.txt` Apply writes; `None` until the game's prefix is
    /// there (started once).
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub files: usize,
    pub nexus: Option<NexusSource>,
    /// Built by a FOMOD installer, whose options can be chosen again.
    pub installer: bool,
    /// Mods before it whose files it replaces, and how many.
    pub wins: Vec<(String, usize)>,
    /// Mods after it that replace its files, and how many.
    pub loses: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The game is as it was.
    No,
    /// Applied as the list stands, with this many mods.
    Current(usize),
    /// Applied, and the list changed since.
    Changed,
}

/// Where one game's mods are kept and go.
#[derive(Debug, Clone)]
struct Place {
    data: PathBuf,
    game_dir: PathBuf,
    known: Option<&'static Known>,
    /// A Windows game, which sees `Data` and `data` as one folder: a mod's
    /// paths take the case of the folders and files already there.
    any_case: bool,
    /// The folder the game's plugin list goes in: its folder in the
    /// prefix's `AppData/Local` (or the game's own, as Oblivion's `.ini`
    /// may say); `None` for games without one, or no prefix yet.
    appdata: Option<PathBuf>,
    /// The game's folder in the prefix's `Documents/My Games`, for those
    /// that keep files there Zendo reads (Starfield's Creation Club list).
    my_games: Option<PathBuf>,
}

fn place(game: &Game) -> anyhow::Result<Place> {
    let game_dir = game
        .details
        .install_path
        .clone()
        .filter(|dir| dir.is_dir())
        .context("Zendo does not know where this game is installed")?;
    let data = paths::data_home()
        .context("no home folder to keep mods in")?
        .join("zendo/mods")
        .join(paths::safe_name(&game.id));
    let known = known(game);
    let rules = known.and_then(|known| known.plugins);
    let local = rules.and_then(|_| plugins::appdata_local(game, &game_dir));
    let appdata = rules.and_then(|rules| {
        if rules.list_in_game(&game_dir) {
            Some(game_dir.clone())
        } else {
            local
                .as_ref()
                .map(|local| local.join(rules.appdata_for(&game_dir)))
        }
    });
    // `<user>/AppData/Local` to `<user>/Documents/My Games/<game>`.
    let my_games = rules.and_then(|rules| rules.my_games).and_then(|name| {
        let user = local.as_ref()?.parent()?.parent()?;
        Some(user.join("Documents/My Games").join(name))
    });
    Ok(Place {
        data,
        game_dir,
        known,
        any_case: launch::needs_proton(game),
        appdata,
        my_games,
    })
}

/// The game's mods, or `None` when there is no folder to put them in.
///
/// # Errors
///
/// When Zendo's files for the game cannot be read.
pub fn overview(game: &Game) -> anyhow::Result<Option<Overview>> {
    let _lock = lock();
    let Ok(place) = place(game) else {
        return Ok(None);
    };
    let mut overview = place.overview()?;
    if overview.nexus.is_none() {
        overview.nexus = nexus::domain_for(&game.title, &nexus::cached_games());
    }
    Ok(Some(overview))
}

/// Adds a mod from an archive (zip, 7z, rar, tar…), on and last.
///
/// # Errors
///
/// When it cannot be unpacked, is empty, holds links, or cannot be kept.
pub fn add(game: &Game, archive: &Path) -> anyhow::Result<Added> {
    let _lock = lock();
    let place = place(game)?;
    match place.take_in(archive, None, None, None)? {
        Some(pending) => Ok(Added::Choose(Box::new(pending))),
        None => Ok(Added::Done(Box::new(place.overview()?))),
    }
}

/// Builds a mod from its installer with the options chosen.
///
/// # Errors
///
/// When the options install nothing, a path leaves the mod, or the files
/// cannot be copied or kept.
pub fn install(game: &Game, pending: &Pending, selection: &Selection) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.install(pending, selection)?;
    place.overview()
}

/// Drops a mod waiting for its installer's options.
pub fn cancel_install(pending: &Pending) {
    match &pending.what {
        PendingKind::New { raw, .. } | PendingKind::Update { raw, .. } => {
            let _ = fs::remove_dir_all(raw);
        }
        PendingKind::Options { .. } => {}
    }
}

/// A mod's installer again, to choose other options.
///
/// # Errors
///
/// When the mod has no installer kept or it cannot be read.
pub fn reinstall(game: &Game, id: &str) -> anyhow::Result<Pending> {
    let _lock = lock();
    let place = place(game)?;
    let list = place.list()?;
    let entry = list
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .context("the mod is no longer in the list")?;
    let raw = place.installers().join(id);
    let root = find_installer(&raw).context("Zendo no longer has this mod's installer")?;
    Ok(Pending {
        name: entry.name.clone(),
        installer: read_installer(&root)?,
        present: place.present(&list),
        chosen: entry.chosen.clone(),
        root,
        what: PendingKind::Options { id: id.to_owned() },
    })
}

/// Adds a file downloaded from Nexus: in place of the mod it updates (kept
/// where it was in the order, on or off), or as a new mod, on and last.
/// The archive is deleted once unpacked. Says whether it was an update.
///
/// # Errors
///
/// As [`add`].
pub fn add_from_nexus(
    game: &Game,
    fetched: &nexus::Fetched,
    domain: &str,
    mod_id: u64,
) -> anyhow::Result<(Added, bool)> {
    let _lock = lock();
    let place = place(game)?;
    let (pending, updated) = place.add_fetched(fetched, domain, mod_id)?;
    let added = match pending {
        Some(pending) => Added::Choose(Box::new(pending)),
        None => Added::Done(Box::new(place.overview()?)),
    };
    Ok((added, updated))
}

/// The mod this file updates: the same mod on Nexus, from a file this one
/// replaced (or this very file, downloaded again).
fn updated_entry<'a>(list: &'a List, source: &NexusSource, files: &Files) -> Option<&'a Entry> {
    list.mods.iter().find(|entry| {
        entry.nexus.as_ref().is_some_and(|old| {
            old.domain == source.domain
                && old.mod_id == source.mod_id
                && (old.file_id == source.file_id || files.replaces(source.file_id, old.file_id))
        })
    })
}

/// Where a game's downloads from Nexus go while they download.
///
/// # Errors
///
/// When the game has no folder to put mods in.
pub fn downloads_dir(game: &Game) -> anyhow::Result<PathBuf> {
    Ok(place(game)?.data.join("downloads"))
}

/// The game's name on Nexus: the one Zendo knows, the one the user chose,
/// or the game with its title in Nexus's list.
#[must_use]
pub fn nexus_domain(game: &Game) -> Option<String> {
    let _lock = lock();
    let chosen = place(game)
        .ok()
        .and_then(|place| place.list().ok())
        .and_then(|list| list.nexus);
    chosen
        .or_else(|| known(game).map(|known| known.nexus.to_owned()))
        .or_else(|| nexus::domain_for(&game.title, &nexus::cached_games()))
}

/// Remembers the game's name on Nexus, as the user chose it for a link.
///
/// # Errors
///
/// When the list cannot be written.
pub fn set_nexus(game: &Game, domain: &str) -> anyhow::Result<()> {
    let _lock = lock();
    place(game)?.change(|list| {
        list.nexus = Some(domain.to_owned());
        Ok(())
    })
}

/// Turns a mod on or off; Apply makes it so.
///
/// # Errors
///
/// When the list cannot be read or written.
pub fn set_enabled(game: &Game, id: &str, enabled: bool) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change(|list| {
        if let Some(entry) = list.mods.iter_mut().find(|entry| entry.id == id) {
            entry.enabled = enabled;
        }
        Ok(())
    })?;
    place.overview()
}

/// Moves a mod one place earlier (`up`) or later in the order.
///
/// # Errors
///
/// When the list cannot be read or written.
pub fn move_mod(game: &Game, id: &str, up: bool) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change(|list| {
        if let Some(at) = list.mods.iter().position(|entry| entry.id == id) {
            let to = if up { at.checked_sub(1) } else { Some(at + 1) };
            if let Some(to) = to.filter(|&to| to < list.mods.len()) {
                list.mods.swap(at, to);
            }
        }
        Ok(())
    })?;
    place.overview()
}

/// Deletes a mod Zendo keeps. Applied, its files stay in the game until
/// Back to vanilla or the next Apply.
///
/// # Errors
///
/// When its files or the list cannot be changed.
pub fn remove(game: &Game, id: &str) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change(|list| {
        list.mods.retain(|entry| entry.id != id);
        for profile in &mut list.profiles {
            profile.order.retain(|other| other != id);
            profile.enabled.retain(|other| other != id);
        }
        Ok(())
    })?;
    for kept in [place.files().join(id), place.installers().join(id)] {
        if !id.is_empty() && kept.is_dir() {
            fs::remove_dir_all(&kept)
                .with_context(|| format!("could not delete {}", kept.display()))?;
        }
    }
    place.overview()
}

/// Turns a plugin on or off; Apply makes it so. The game's main masters
/// stay on.
///
/// # Errors
///
/// When the game has no plugins, or the list cannot be read or written.
pub fn set_plugin(game: &Game, name: &str, enabled: bool) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change_plugins(|plugins| {
        let mut order = order_of(plugins);
        let at = plugins
            .iter()
            .position(|plugin| plugin.name == name && plugin.rank != plugins::Rank::Implicit)?;
        order[at].1 = enabled;
        Some(order)
    })?;
    place.overview()
}

/// Moves a plugin one place earlier (`up`) or later, among its kind.
///
/// # Errors
///
/// When the game has no plugins, or the list cannot be read or written.
pub fn move_plugin(game: &Game, name: &str, up: bool) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change_plugins(|plugins| plugins::moved(plugins, name, up))?;
    place.overview()
}

/// What to do with a game's profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileChange {
    /// A copy of the profile in use, switched to.
    New(String),
    Switch(String),
    Rename {
        old: String,
        new: String,
    },
    Delete(String),
}

/// Changes a game's profiles. Deleting the profile in use switches to the
/// next first. A switch while mods are applied applies the new profile
/// when `apply` is set (the game is not running).
///
/// # Errors
///
/// When the name is taken or empty, the profile is missing or the last
/// one (for Delete), or the list cannot be written or applied.
pub fn change_profiles(
    game: &Game,
    change: &ProfileChange,
    apply: bool,
) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.change_profiles(change, apply)?;
    place.overview()
}

/// Sets where the game's mods go, from its folder; `None` for the one
/// Zendo knows, or the folder itself. Not while mods are applied.
///
/// # Errors
///
/// When mods are applied, or the folder is not inside the game's.
pub fn set_folder(game: &Game, folder: Option<&str>) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    if place.manifest()?.is_some() {
        bail!("Go back to vanilla before changing where mods go");
    }
    let folder = folder.map(str::trim).filter(|folder| !folder.is_empty());
    if let Some(folder) = folder {
        check_relative(Path::new(folder))?;
    }
    place.change(|list| {
        list.folder = folder.map(|folder| folder.trim_matches('/').to_owned());
        Ok(())
    })?;
    place.overview()
}

/// Deletes what the game wrote while modded.
///
/// # Errors
///
/// When it cannot be deleted.
pub fn clear_overwrite(game: &Game) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    let overwrite = place.overwrite();
    if overwrite.is_dir() {
        fs::remove_dir_all(&overwrite)
            .with_context(|| format!("could not delete {}", overwrite.display()))?;
    }
    place.overview()
}

/// Puts the mods that are on into the game, after taking out any applied
/// before.
///
/// # Errors
///
/// When a file cannot be copied or backed up; what was placed is taken
/// out again.
pub fn apply(game: &Game) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.apply()?;
    place.overview()
}

/// Takes the mods out: the game as it was.
///
/// # Errors
///
/// When a file cannot be removed or put back.
pub fn restore(game: &Game) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.restore()?;
    place.overview()
}

impl Place {
    fn list_path(&self) -> PathBuf {
        self.data.join("mods.toml")
    }
    fn manifest_path(&self) -> PathBuf {
        self.data.join("applied.json")
    }
    fn files(&self) -> PathBuf {
        self.data.join("files")
    }
    fn backup(&self) -> PathBuf {
        self.data.join("backup")
    }
    fn installers(&self) -> PathBuf {
        self.data.join("installers")
    }
    fn overwrite(&self) -> PathBuf {
        self.data.join("overwrite")
    }

    fn list(&self) -> anyhow::Result<List> {
        let path = self.list_path();
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("{}", path.display())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(List::default()),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    fn change(&self, edit: impl FnOnce(&mut List) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let mut list = self.list()?;
        edit(&mut list)?;
        let text = format!(
            "# This game's mods in Zendo, which rewrites this file. Later ones win.\n\n{}",
            toml::to_string(&list)?
        );
        let path = self.list_path();
        paths::write_atomic(&path, text.as_bytes())
            .with_context(|| format!("could not write {}", path.display()))
    }

    fn manifest(&self) -> anyhow::Result<Option<Manifest>> {
        let path = self.manifest_path();
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).with_context(|| format!("{}", path.display()))?,
            )),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// The mod folder from the game's, as set or known.
    fn folder(&self, list: &List) -> (String, bool) {
        match &list.folder {
            Some(folder) => (folder.clone(), true),
            None => (
                self.known
                    .map(|known| known.folder.to_owned())
                    .unwrap_or_default(),
                false,
            ),
        }
    }

    fn overview(&self) -> anyhow::Result<Overview> {
        let list = self.list()?;
        let (folder, custom_folder) = self.folder(&list);
        let mut mods: Vec<ModInfo> = Vec::new();
        // Which mod gives each file, as Apply will place them.
        let mut owner: HashMap<String, usize> = HashMap::new();
        for (index, entry) in list.mods.iter().enumerate() {
            let files = walk(&self.mod_root(entry)).unwrap_or_default();
            mods.push(ModInfo {
                id: entry.id.clone(),
                name: entry.name.clone(),
                enabled: entry.enabled,
                files: files.len(),
                nexus: entry.nexus.clone(),
                installer: entry.installer,
                wins: Vec::new(),
                loses: Vec::new(),
            });
            if !entry.enabled {
                continue;
            }
            let mut wins: BTreeMap<usize, usize> = BTreeMap::new();
            for file in files {
                if let Some(before) = owner.insert(self.file_key(&file), index) {
                    *wins.entry(before).or_default() += 1;
                }
            }
            for (before, count) in wins {
                let loser = mods[before].name.clone();
                let winner = mods[index].name.clone();
                mods[index].wins.push((loser, count));
                mods[before].loses.push((winner, count));
            }
        }
        let manifest = self.manifest()?;
        let plugin_list = self.plugins(&list, manifest.as_ref());
        let wanted = self.wanted(plugin_list.as_deref());
        let applied = match manifest {
            None => Applied::No,
            Some(manifest) => {
                let enabled: Vec<&Entry> = list.mods.iter().filter(|entry| entry.enabled).collect();
                let ids: Vec<&String> = enabled.iter().map(|entry| &entry.id).collect();
                let stamps: Vec<String> = enabled.iter().map(|entry| entry.stamp()).collect();
                let same_files = manifest.stamps.is_empty() || manifest.stamps == stamps;
                if manifest.mods.iter().collect::<Vec<_>>() == ids
                    && same_files
                    && manifest.plugins == wanted
                {
                    Applied::Current(enabled.len())
                } else {
                    Applied::Changed
                }
            }
        };
        Ok(Overview {
            target: self.game_dir.join(&folder),
            game_dir: self.game_dir.clone(),
            folder,
            custom_folder,
            known: self.known,
            nexus: list
                .nexus
                .clone()
                .or_else(|| self.known.map(|known| known.nexus.to_owned())),
            profile: list.active().to_owned(),
            profiles: list.profile_names(),
            mods,
            overwrite: walk(&self.overwrite()).map_or(0, |files| files.len()),
            applied,
            plugins: plugin_list.map(|list| {
                Box::new(Plugins {
                    list,
                    file: self.list_file(plugins::PLUGINS_TXT),
                })
            }),
        })
    }

    fn file_key(&self, path: &Path) -> String {
        let key = path.to_string_lossy();
        if self.any_case {
            key.to_lowercase()
        } else {
            key.into_owned()
        }
    }

    fn mod_root(&self, entry: &Entry) -> PathBuf {
        self.files().join(&entry.id).join(&entry.root)
    }

    fn change_profiles(&self, change: &ProfileChange, apply: bool) -> anyhow::Result<()> {
        let before = self.list()?.active().to_owned();
        self.change(|list| match change {
            ProfileChange::New(name) => list.new_profile(name),
            ProfileChange::Switch(name) => list.switch_profile(name),
            ProfileChange::Rename { old, new } => list.rename_profile(old, new),
            ProfileChange::Delete(name) => list.delete_profile(name),
        })?;
        let switched = match change {
            ProfileChange::Switch(_) | ProfileChange::Delete(_) => self.list()?.active() != before,
            ProfileChange::New(_) | ProfileChange::Rename { .. } => false,
        };
        if switched && apply && self.manifest()?.is_some() {
            self.apply()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
