//! Mods: archives unpacked into a folder of Zendo's per game, then copied
//! into the game's folder on Apply and taken out again on Back to vanilla.
//!
//! Each game's are in `$XDG_DATA_HOME/zendo/mods/<game id>/`:
//!
//! - `mods.toml`: the mods in order (a later one wins a file both have),
//!   whether each is on and the Nexus file it came from, the game's mod
//!   folder when the user set one, and its name on Nexus when the user
//!   chose it;
//! - `downloads/`: files from Nexus while they download, deleted once
//!   unpacked;
//! - `files/<mod>/`: each mod as its archive held it;
//! - `applied.json`: every file and folder Apply placed, so Back to vanilla
//!   removes exactly those;
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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::nexus::{self, Files};
use crate::{Game, Source, launch, paths, protondb};

/// One change at a time to any game's mods: the window's, and a download
/// from Nexus finishing on its own thread.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A game whose mods Zendo knows where to put.
#[derive(Debug, PartialEq, Eq)]
pub struct Known {
    /// Its names in the stores.
    pub titles: &'static [&'static str],
    pub steam: Option<u32>,
    /// Its name on Nexus Mods, in links and the API.
    pub nexus: &'static str,
    /// Where mods go, from the game's folder; empty for the folder itself.
    pub folder: &'static str,
    /// A top folder some archives wrap their files in that is the mod
    /// folder itself (`Data` for Skyrim), whose contents are taken.
    pub strip: &'static [&'static str],
    /// The folders a mod packed right starts with; folders around them are
    /// removed until one shows.
    pub tops: &'static [&'static str],
}

/// The games Zendo knows, from where each one's mods go.
pub const KNOWN: &[Known] = &[
    Known {
        titles: &["Stardew Valley"],
        steam: Some(413_150),
        nexus: "stardewvalley",
        folder: "Mods",
        strip: &["Mods"],
        tops: &[],
    },
    Known {
        titles: &[
            "The Elder Scrolls V: Skyrim Special Edition",
            "Skyrim Special Edition",
            "The Elder Scrolls V: Skyrim Anniversary Edition",
        ],
        steam: Some(489_830),
        nexus: "skyrimspecialedition",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
    },
    Known {
        titles: &["The Elder Scrolls V: Skyrim"],
        steam: Some(72_850),
        nexus: "skyrim",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
    },
    Known {
        titles: &["Fallout 4"],
        steam: Some(377_160),
        nexus: "fallout4",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
    },
    Known {
        titles: &["Fallout: New Vegas", "Fallout New Vegas"],
        steam: Some(22_380),
        nexus: "newvegas",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
    },
    Known {
        titles: &["Starfield"],
        steam: Some(1_716_740),
        nexus: "starfield",
        folder: "Data",
        strip: &["Data"],
        tops: &[],
    },
    Known {
        titles: &["Cyberpunk 2077"],
        steam: Some(1_091_500),
        nexus: "cyberpunk2077",
        folder: "",
        strip: &[],
        tops: &["archive", "bin", "r6", "red4ext", "engine", "mods"],
    },
];

/// The game Zendo knows this one as: by Steam id, or by title.
#[must_use]
pub fn known(game: &Game) -> Option<&'static Known> {
    let steam = game
        .id
        .strip_prefix("steam:")
        .filter(|_| game.source == Source::Steam)
        .and_then(|app| app.parse::<u32>().ok());
    let title = protondb::plain(&game.title);
    KNOWN.iter().find(|known| match steam {
        Some(app) => known.steam == Some(app),
        None => known
            .titles
            .iter()
            .any(|name| protondb::plain(name) == title),
    })
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
    #[serde(default, rename = "mod")]
    mods: Vec<Entry>,
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
}

impl Entry {
    /// What Apply put in, to tell when the mod's files changed since.
    fn stamp(&self) -> String {
        let file = self.nexus.as_ref().map_or(0, |nexus| nexus.file_id);
        format!("{}#{}#{file}", self.id, self.source)
    }
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
    pub mods: Vec<ModInfo>,
    /// Files in `overwrite/`.
    pub overwrite: usize,
    pub applied: Applied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub files: usize,
    pub nexus: Option<NexusSource>,
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
    Ok(Place {
        data,
        game_dir,
        known: known(game),
        any_case: launch::needs_proton(game),
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
pub fn add(game: &Game, archive: &Path) -> anyhow::Result<Overview> {
    let _lock = lock();
    let place = place(game)?;
    place.add(archive, None, None)?;
    place.overview()
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
) -> anyhow::Result<(Overview, bool)> {
    let _lock = lock();
    let place = place(game)?;
    let updated = place.add_fetched(fetched, domain, mod_id)?;
    Ok((place.overview()?, updated))
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
        Ok(())
    })?;
    let files = place.files().join(id);
    if !id.is_empty() && files.is_dir() {
        fs::remove_dir_all(&files)
            .with_context(|| format!("could not delete {}", files.display()))?;
    }
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
        let applied = match self.manifest()? {
            None => Applied::No,
            Some(manifest) => {
                let enabled: Vec<&Entry> = list.mods.iter().filter(|entry| entry.enabled).collect();
                let ids: Vec<&String> = enabled.iter().map(|entry| &entry.id).collect();
                let stamps: Vec<String> = enabled.iter().map(|entry| entry.stamp()).collect();
                let same_files = manifest.stamps.is_empty() || manifest.stamps == stamps;
                if manifest.mods.iter().collect::<Vec<_>>() == ids && same_files {
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
            mods,
            overwrite: walk(&self.overwrite()).map_or(0, |files| files.len()),
            applied,
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

    /// Unpacks an archive into `files/.adding-<id>`: that folder, and the
    /// mod's root inside it.
    fn unpack_new(&self, archive: &Path, id: &str) -> anyhow::Result<(PathBuf, String)> {
        let unpacking = self.files().join(format!(".adding-{id}"));
        if unpacking.exists() {
            fs::remove_dir_all(&unpacking)
                .with_context(|| format!("could not clear {}", unpacking.display()))?;
        }
        fs::create_dir_all(&unpacking)
            .with_context(|| format!("could not make {}", unpacking.display()))?;
        let root = unpack(archive, &unpacking).and_then(|()| {
            let found = walk(&unpacking).map_err(|err| {
                if err.kind() == ErrorKind::InvalidData {
                    anyhow::anyhow!("the archive holds links, which Zendo does not install")
                } else {
                    anyhow::Error::new(err).context("could not read what was unpacked")
                }
            })?;
            if found.is_empty() {
                bail!("the archive is empty");
            }
            Ok(self.find_root(&unpacking))
        });
        match root {
            Ok(root) => Ok((unpacking, root)),
            Err(err) => {
                let _ = fs::remove_dir_all(&unpacking);
                Err(err)
            }
        }
    }

    fn add(
        &self,
        archive: &Path,
        name: Option<String>,
        nexus: Option<NexusSource>,
    ) -> anyhow::Result<String> {
        let list = self.list()?;
        let name = name.unwrap_or_else(|| archive_name(archive));
        let id = new_id(&name, &list.mods);
        let (unpacked, root) = self.unpack_new(archive, &id)?;
        let kept = self.files().join(&id);
        fs::rename(&unpacked, &kept)
            .with_context(|| format!("could not keep the mod in {}", kept.display()))?;
        self.change(|list| {
            list.mods.push(Entry {
                id: id.clone(),
                name,
                enabled: true,
                root,
                source: file_name(archive),
                nexus,
            });
            Ok(())
        })?;
        Ok(id)
    }

    /// [`add_from_nexus`]'s work; says whether it was an update.
    fn add_fetched(
        &self,
        fetched: &nexus::Fetched,
        domain: &str,
        mod_id: u64,
    ) -> anyhow::Result<bool> {
        let source = NexusSource {
            domain: domain.to_owned(),
            mod_id,
            file_id: fetched.file.file_id,
            version: fetched.file.version.clone().unwrap_or_default(),
        };
        let list = self.list()?;
        let updates = updated_entry(&list, &source, &fetched.files).map(|entry| entry.id.clone());
        let result = match &updates {
            Some(id) => self.replace(id, &fetched.archive, source),
            None => self
                .add(
                    &fetched.archive,
                    Some(fetched.mod_name.clone()),
                    Some(source),
                )
                .map(|_| ()),
        };
        let _ = fs::remove_file(&fetched.archive);
        result.map(|()| updates.is_some())
    }

    /// Puts a newer archive's files in place of a mod's.
    fn replace(&self, id: &str, archive: &Path, nexus: NexusSource) -> anyhow::Result<()> {
        let (unpacked, root) = self.unpack_new(archive, id)?;
        let kept = self.files().join(id);
        let old = self.files().join(format!(".old-{id}"));
        if old.exists() {
            fs::remove_dir_all(&old)
                .with_context(|| format!("could not clear {}", old.display()))?;
        }
        if kept.exists() {
            fs::rename(&kept, &old)
                .with_context(|| format!("could not move {} aside", kept.display()))?;
        }
        if let Err(err) = fs::rename(&unpacked, &kept) {
            let _ = fs::rename(&old, &kept);
            let _ = fs::remove_dir_all(&unpacked);
            return Err(err)
                .with_context(|| format!("could not keep the mod in {}", kept.display()));
        }
        let _ = fs::remove_dir_all(&old);
        self.change(|list| {
            if let Some(entry) = list.mods.iter_mut().find(|entry| entry.id == id) {
                entry.root = root;
                entry.source = file_name(archive);
                entry.nexus = Some(nexus);
            }
            Ok(())
        })
    }

    /// The folder inside an unpacked archive whose contents are the mod.
    fn find_root(&self, unpacked: &Path) -> String {
        let Some(known) = self.known else {
            return String::new();
        };
        let mut root = PathBuf::new();
        if !known.tops.is_empty() {
            // Into the one folder there, while no folder a mod starts with shows.
            let mut at = PathBuf::new();
            loop {
                let (dirs, _) = entries(&unpacked.join(&at));
                if dirs
                    .iter()
                    .any(|dir| known.tops.iter().any(|top| dir.eq_ignore_ascii_case(top)))
                {
                    root = at;
                    break;
                }
                match dirs.as_slice() {
                    [only] => at.push(only),
                    _ => break,
                }
            }
        }
        let (dirs, _) = entries(&unpacked.join(&root));
        if let Some(dir) = dirs.iter().find(|dir| {
            known
                .strip
                .iter()
                .any(|strip| dir.eq_ignore_ascii_case(strip))
        }) {
            root.push(dir);
        }
        root.to_string_lossy().into_owned()
    }

    fn apply(&self) -> anyhow::Result<()> {
        self.restore()?;
        let list = self.list()?;
        let (folder, _) = self.folder(&list);
        check_relative(Path::new(&folder))?;

        // Each file from the last mod to give it; then what the game wrote.
        let mut winners: BTreeMap<String, (PathBuf, PathBuf)> = BTreeMap::new();
        let enabled: Vec<&Entry> = list.mods.iter().filter(|entry| entry.enabled).collect();
        let mut sources: Vec<PathBuf> = enabled.iter().map(|entry| self.mod_root(entry)).collect();
        sources.push(self.overwrite());
        for source in sources {
            let files =
                walk(&source).with_context(|| format!("could not read {}", source.display()))?;
            for file in files {
                let into = Path::new(&folder).join(&file);
                winners.insert(self.file_key(&into), (source.join(&file), into));
            }
        }

        let mut manifest = Manifest {
            game_dir: self.game_dir.clone(),
            mods: enabled.iter().map(|entry| entry.id.clone()).collect(),
            stamps: enabled.iter().map(|entry| entry.stamp()).collect(),
            ..Manifest::default()
        };
        if winners.is_empty() {
            // Nothing on: the game stays as it is.
            return Ok(());
        }
        let placed = winners
            .into_values()
            .try_for_each(|(from, into)| self.place_file(&from, &into, &mut manifest));
        if let Err(err) = placed.and_then(|()| self.save_manifest(&manifest)) {
            // Take out what went in.
            let _ = self.take_out(&manifest);
            let _ = fs::remove_file(self.manifest_path());
            return Err(err);
        }
        Ok(())
    }

    /// Copies one file in, backing up the game's own and making folders.
    fn place_file(&self, from: &Path, into: &Path, manifest: &mut Manifest) -> anyhow::Result<()> {
        let into = self.resolve_case(into);
        let mut made = PathBuf::new();
        if let Some(parent) = into.parent() {
            for part in parent.components() {
                made.push(part);
                let dir = self.game_dir.join(&made);
                match fs::symlink_metadata(&dir) {
                    Ok(meta) if meta.is_dir() => {}
                    Ok(_) => bail!("{} is a file where a mod needs a folder", dir.display()),
                    Err(err) if err.kind() == ErrorKind::NotFound => {
                        fs::create_dir(&dir)
                            .with_context(|| format!("could not make {}", dir.display()))?;
                        manifest.created.push(made.clone());
                    }
                    Err(err) => {
                        return Err(err)
                            .with_context(|| format!("could not read {}", dir.display()));
                    }
                }
            }
        }
        let target = self.game_dir.join(&into);
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.is_dir() => {
                bail!("{} is a folder where a mod has a file", target.display())
            }
            Ok(_) => {
                let backup = self.backup().join(&into);
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("could not make {}", parent.display()))?;
                }
                fs::rename(&target, &backup)
                    .with_context(|| format!("could not back up {}", target.display()))?;
                manifest.backups.push(into.clone());
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("could not read {}", target.display()));
            }
        }
        fs::copy(from, &target).with_context(|| format!("could not copy {}", from.display()))?;
        let meta = fs::metadata(&target)
            .with_context(|| format!("could not read {}", target.display()))?;
        manifest.files.push(Placed {
            path: into,
            size: meta.len(),
            modified: meta.modified().ok(),
        });
        Ok(())
    }

    /// For a Windows game, the path as the folders and files already there
    /// spell it, case aside.
    fn resolve_case(&self, path: &Path) -> PathBuf {
        if !self.any_case {
            return path.to_path_buf();
        }
        let mut resolved = PathBuf::new();
        for part in path.components() {
            let name = part.as_os_str();
            let found = fs::read_dir(self.game_dir.join(&resolved))
                .ok()
                .and_then(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.file_name())
                        .find(|entry| {
                            entry
                                .to_string_lossy()
                                .eq_ignore_ascii_case(&name.to_string_lossy())
                        })
                });
            resolved.push(found.as_deref().unwrap_or(name));
        }
        resolved
    }

    fn save_manifest(&self, manifest: &Manifest) -> anyhow::Result<()> {
        let path = self.manifest_path();
        let json = serde_json::to_vec_pretty(manifest)?;
        paths::write_atomic(&path, &json)
            .with_context(|| format!("could not write {}", path.display()))
    }

    fn restore(&self) -> anyhow::Result<()> {
        let Some(manifest) = self.manifest()? else {
            return Ok(());
        };
        self.take_out(&manifest)?;
        fs::remove_file(self.manifest_path()).context("could not delete applied.json")?;
        Ok(())
    }

    /// Removes what was placed, keeping what the game changed or wrote in
    /// `overwrite/`, and puts the game's own files back.
    fn take_out(&self, manifest: &Manifest) -> anyhow::Result<()> {
        let game_dir = &manifest.game_dir;
        let overwrite = self.overwrite();
        for placed in &manifest.files {
            let path = game_dir.join(&placed.path);
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            let unchanged = meta.is_file()
                && meta.len() == placed.size
                && meta.modified().ok() == placed.modified;
            if unchanged {
                fs::remove_file(&path)
                    .with_context(|| format!("could not remove {}", path.display()))?;
            } else {
                keep(&path, &overwrite.join(&placed.path))?;
            }
        }
        // Folders Zendo made, innermost first: what the game wrote there is
        // kept, then the folder goes.
        let made: HashSet<&PathBuf> = manifest.created.iter().collect();
        for dir in manifest.created.iter().rev() {
            let path = game_dir.join(dir);
            let Ok(entries) = fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                let inner = dir.join(entry.file_name());
                if made.contains(&inner) {
                    continue;
                }
                keep(&entry.path(), &overwrite.join(&inner))?;
            }
            fs::remove_dir(&path)
                .with_context(|| format!("could not remove {}", path.display()))?;
        }
        for original in &manifest.backups {
            let from = self.backup().join(original);
            let to = game_dir.join(original);
            if from.exists() {
                fs::rename(&from, &to)
                    .with_context(|| format!("could not put back {}", to.display()))?;
            }
        }
        let backup = self.backup();
        if backup.exists() {
            fs::remove_dir_all(&backup)
                .with_context(|| format!("could not delete {}", backup.display()))?;
        }
        Ok(())
    }
}

/// Moves a file or folder into `overwrite/`, over what is there.
fn keep(from: &Path, to: &Path) -> anyhow::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not make {}", parent.display()))?;
    }
    match fs::symlink_metadata(to) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(to),
        Ok(_) => fs::remove_file(to),
        Err(_) => Ok(()),
    }
    .with_context(|| format!("could not replace {}", to.display()))?;
    fs::rename(from, to).with_context(|| format!("could not keep {}", from.display()))
}

/// A folder's files, from it, in order; an `InvalidData` error for a link.
fn walk(root: &Path) -> io::Result<Vec<PathBuf>> {
    fn visit(root: &Path, at: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(root.join(at))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = at.join(entry.file_name());
            if kind.is_symlink() {
                return Err(io::Error::new(ErrorKind::InvalidData, "a link"));
            } else if kind.is_dir() {
                visit(root, &path, found)?;
            } else {
                found.push(path);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    match visit(root, Path::new(""), &mut found) {
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        result => result?,
    }
    found.sort();
    Ok(found)
}

/// A folder's subfolders and files, by name.
fn entries(dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    dirs.sort();
    files.sort();
    (dirs, files)
}

/// A folder inside the game's: relative, never above it.
fn check_relative(folder: &Path) -> anyhow::Result<()> {
    if folder
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        Ok(())
    } else {
        bail!("the mod folder must be inside the game's folder, as Data or BepInEx/plugins")
    }
}

/// A mod's name from its archive: `SkyUI_5_2_SE.7z` is `SkyUI_5_2_SE`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn archive_name(archive: &Path) -> String {
    let mut name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    for ext in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz"] {
        if name.to_lowercase().ends_with(ext) {
            name.truncate(name.len() - ext.len());
            return name;
        }
    }
    match Path::new(&name).file_stem().map(OsStr::to_string_lossy) {
        Some(stem) if !stem.is_empty() => stem.into_owned(),
        _ => name,
    }
}

/// An id from a name, unlike any taken.
fn new_id(name: &str, taken: &[Entry]) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let base = if slug.is_empty() { "mod" } else { slug }.to_owned();
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|entry| entry.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Unpacks an archive with 7-Zip, or bsdtar when 7-Zip is missing.
fn unpack(archive: &Path, into: &Path) -> anyhow::Result<()> {
    let seven = ["7z", "7zz", "7za"]
        .into_iter()
        .find(|program| launch::which(program).is_some());
    let mut command = if let Some(seven) = seven {
        let mut command = Command::new(seven);
        command
            .args(["x", "-y", "-bso0", "-bsp0"])
            .arg(format!("-o{}", into.display()))
            .arg("--")
            .arg(archive);
        command
    } else if launch::which("bsdtar").is_some() {
        let mut command = Command::new("bsdtar");
        command.arg("-xf").arg(archive).arg("-C").arg(into);
        command
    } else {
        bail!("unpacking mods needs 7-Zip (7zip) or bsdtar (libarchive)");
    };
    let output = command
        .stdin(Stdio::null())
        .output()
        .context("could not start 7-Zip")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .unwrap_or("it failed");
        bail!("could not unpack {}: {reason}", archive.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("zendo-mods-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("game")).unwrap();
            Self(dir)
        }
        fn place(&self, known: Option<&'static Known>, any_case: bool) -> Place {
            Place {
                data: self.0.join("data"),
                game_dir: self.0.join("game"),
                known,
                any_case,
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A mod as if unpacked, listed last and on.
    fn add_unpacked(place: &Place, id: &str, files: &[(&str, &str)]) {
        for (path, text) in files {
            write(&place.files().join(id).join(path), text);
        }
        place
            .change(|list| {
                list.mods.push(Entry {
                    id: id.into(),
                    name: id.into(),
                    enabled: true,
                    root: String::new(),
                    source: String::new(),
                    nexus: None,
                });
                Ok(())
            })
            .unwrap();
    }

    fn read(path: &Path) -> Option<String> {
        fs::read_to_string(path).ok()
    }

    #[test]
    fn applies_and_goes_back_to_vanilla_exactly() {
        let scratch = Scratch::new("apply");
        let place = scratch.place(None, true);
        let game = &place.game_dir;
        write(&game.join("Game.exe"), "game");
        write(&game.join("Data/textures/sky.dds"), "vanilla sky");
        add_unpacked(
            &place,
            "loader",
            &[
                ("winhttp.dll", "loader"),
                ("BepInEx/core/BepInEx.dll", "core"),
            ],
        );
        add_unpacked(&place, "sky", &[("data/Textures/sky.dds", "first sky")]);
        add_unpacked(
            &place,
            "better-sky",
            &[("Data/textures/SKY.dds", "better sky")],
        );

        let overview = place.overview().unwrap();
        assert_eq!(overview.applied, Applied::No);
        assert_eq!(overview.mods[2].wins, vec![("sky".to_owned(), 1)]);
        assert_eq!(overview.mods[1].loses, vec![("better-sky".to_owned(), 1)]);

        place.apply().unwrap();
        assert_eq!(read(&game.join("winhttp.dll")).as_deref(), Some("loader"));
        // The game's own spelling, and the last mod's file.
        assert_eq!(
            read(&game.join("Data/textures/sky.dds")).as_deref(),
            Some("better sky")
        );
        assert!(!game.join("data").exists());
        assert_eq!(place.overview().unwrap().applied, Applied::Current(3));

        // The game writes a log and changes a mod's file while modded.
        write(&game.join("BepInEx/LogOutput.log"), "loaded");
        write(&game.join("winhttp.dll"), "changed by the game");

        place.restore().unwrap();
        assert_eq!(
            read(&game.join("Data/textures/sky.dds")).as_deref(),
            Some("vanilla sky")
        );
        assert!(!game.join("winhttp.dll").exists());
        assert!(!game.join("BepInEx").exists());
        assert_eq!(read(&game.join("Game.exe")).as_deref(), Some("game"));
        let overwrite = place.overwrite();
        assert_eq!(
            read(&overwrite.join("BepInEx/LogOutput.log")).as_deref(),
            Some("loaded")
        );
        assert_eq!(
            read(&overwrite.join("winhttp.dll")).as_deref(),
            Some("changed by the game")
        );
        assert_eq!(place.overview().unwrap().applied, Applied::No);
        assert_eq!(place.overview().unwrap().overwrite, 2);

        // Applied again, what the game wrote comes back after every mod.
        place.apply().unwrap();
        assert_eq!(
            read(&game.join("winhttp.dll")).as_deref(),
            Some("changed by the game")
        );
        assert_eq!(
            read(&game.join("BepInEx/LogOutput.log")).as_deref(),
            Some("loaded")
        );
        place.restore().unwrap();
        assert!(!game.join("BepInEx").exists());
    }

    #[test]
    fn a_linux_game_keeps_case_apart_and_a_change_shows() {
        let scratch = Scratch::new("case");
        let place = scratch.place(None, false);
        write(&place.game_dir.join("Mods/readme.txt"), "game's");
        add_unpacked(&place, "a", &[("mods/A/manifest.json", "{}")]);
        place.apply().unwrap();
        assert!(place.game_dir.join("mods/A/manifest.json").is_file());
        assert!(place.game_dir.join("Mods/readme.txt").is_file());
        place
            .change(|list| {
                list.mods[0].enabled = false;
                Ok(())
            })
            .unwrap();
        assert_eq!(place.overview().unwrap().applied, Applied::Changed);
        place.apply().unwrap();
        assert!(!place.game_dir.join("mods").exists());
        assert_eq!(place.overview().unwrap().applied, Applied::No);
    }

    #[test]
    fn a_failed_apply_takes_out_what_went_in() {
        let scratch = Scratch::new("fail");
        let place = scratch.place(None, false);
        // A file where the mod needs a folder.
        write(&place.game_dir.join("plugins"), "a file");
        add_unpacked(&place, "a", &[("aaa.txt", "a"), ("plugins/x.dll", "x")]);
        let err = place.apply().unwrap_err();
        assert!(format!("{err:#}").contains("needs a folder"), "{err:#}");
        assert!(!place.game_dir.join("aaa.txt").exists());
        assert!(place.manifest().unwrap().is_none());
    }

    #[test]
    fn finds_the_mod_inside_its_wrappers() {
        let scratch = Scratch::new("root");
        let unpacked = scratch.0.join("unpacked");
        let cyberpunk = KNOWN.iter().find(|known| known.nexus == "cyberpunk2077");
        write(
            &unpacked.join("Cool Mod 1.2/Cool Mod/archive/pc/mod/cool.archive"),
            "x",
        );
        write(&unpacked.join("Cool Mod 1.2/readme.txt"), "x");
        let place = scratch.place(cyberpunk, true);
        assert_eq!(place.find_root(&unpacked), "Cool Mod 1.2/Cool Mod");

        let skyrim = KNOWN
            .iter()
            .find(|known| known.nexus == "skyrimspecialedition");
        let unpacked = scratch.0.join("skyrim");
        write(&unpacked.join("data/meshes/a.nif"), "x");
        write(&unpacked.join("fomod/info.xml"), "x");
        assert_eq!(scratch.place(skyrim, true).find_root(&unpacked), "data");
        // Stardew mods are folders of their own, kept.
        let stardew = KNOWN.iter().find(|known| known.nexus == "stardewvalley");
        let unpacked = scratch.0.join("stardew");
        write(&unpacked.join("ContentPatcher/manifest.json"), "{}");
        assert_eq!(scratch.place(stardew, false).find_root(&unpacked), "");
        assert_eq!(scratch.place(None, false).find_root(&unpacked), "");
    }

    #[test]
    fn unpacks_an_archive_and_refuses_links() {
        if launch::which("7z").is_none() {
            return;
        }
        let scratch = Scratch::new("unpack");
        let place = scratch.place(None, true);
        let source = scratch.0.join("source");
        write(&source.join("BepInEx/core/BepInEx.dll"), "core");
        write(&source.join("winhttp.dll"), "loader");
        let archive = scratch.0.join("BepInEx_win_x64_5.4.23.2.zip");
        let status = Command::new("7z")
            .args(["a", "-bso0", "-bsp0"])
            .arg(&archive)
            .arg(".")
            .current_dir(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let id = place.add(&archive, None, None).unwrap();
        assert_eq!(id, "bepinex-win-x64-5-4-23-2");
        let overview = place.overview().unwrap();
        assert_eq!(overview.mods[0].name, "BepInEx_win_x64_5.4.23.2");
        assert_eq!(overview.mods[0].files, 2);

        std::os::unix::fs::symlink("/etc/passwd", source.join("passwd")).unwrap();
        let linked = scratch.0.join("linked.tar");
        let status = Command::new("bsdtar")
            .arg("-cf")
            .arg(&linked)
            .arg("-C")
            .arg(&source)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());
        let err = place.add(&linked, None, None).unwrap_err();
        assert!(format!("{err:#}").contains("links"), "{err:#}");
        assert_eq!(place.overview().unwrap().mods.len(), 1);
        assert_eq!(entries(&place.files()).0, vec![id]);
    }

    #[test]
    fn a_nexus_update_takes_the_mods_place_and_shows_as_a_change() {
        if launch::which("bsdtar").is_none() {
            return;
        }
        let scratch = Scratch::new("nexus");
        let place = scratch.place(None, false);
        add_unpacked(&place, "first", &[("first.txt", "first")]);
        let files: Files = serde_json::from_str(
            r#"{"files":[{"file_id":10,"version":"1.0","file_name":"m.tar"},
                         {"file_id":11,"version":"1.1","file_name":"m.tar"},
                         {"file_id":20,"version":"1.0","file_name":"extra.tar"}],
                "file_updates":[{"old_file_id":10,"new_file_id":11}]}"#,
        )
        .unwrap();
        let fetched = |file_id: u64, text: &str| {
            let source = scratch.0.join(format!("source-{file_id}"));
            write(&source.join("mod.txt"), text);
            let archive = scratch.0.join(format!("m-{file_id}.tar"));
            let status = Command::new("bsdtar")
                .arg("-cf")
                .arg(&archive)
                .arg("-C")
                .arg(&source)
                .arg(".")
                .status()
                .unwrap();
            assert!(status.success());
            nexus::Fetched {
                archive,
                mod_name: "Cool Mod".into(),
                file: files.get(file_id).unwrap().clone(),
                files: files.clone(),
            }
        };

        let old = fetched(10, "old");
        assert!(!place.add_fetched(&old, "catquest", 7).unwrap());
        assert!(
            !old.archive.exists(),
            "the download is deleted once unpacked"
        );
        add_unpacked(&place, "last", &[("last.txt", "last")]);
        place.apply().unwrap();
        let game = place.game_dir.clone();
        assert_eq!(read(&game.join("mod.txt")).as_deref(), Some("old"));
        assert_eq!(place.overview().unwrap().applied, Applied::Current(3));

        assert!(
            place
                .add_fetched(&fetched(11, "new"), "catquest", 7)
                .unwrap()
        );
        let overview = place.overview().unwrap();
        let names: Vec<&str> = overview
            .mods
            .iter()
            .map(|info| info.name.as_str())
            .collect();
        assert_eq!(names, ["first", "Cool Mod", "last"], "kept in its place");
        let nexus = overview.mods[1].nexus.as_ref().unwrap();
        assert_eq!((nexus.file_id, nexus.version.as_str()), (11, "1.1"));
        assert_eq!(overview.applied, Applied::Changed);
        assert_eq!(entries(&place.files()).0.len(), 3, "nothing left aside");
        place.apply().unwrap();
        assert_eq!(read(&game.join("mod.txt")).as_deref(), Some("new"));

        // Another file of the same mod is a mod of its own.
        assert!(
            !place
                .add_fetched(&fetched(20, "extra"), "catquest", 7)
                .unwrap()
        );
        assert_eq!(place.overview().unwrap().mods.len(), 4);
        place.restore().unwrap();
        assert_eq!(entries(&game), (Vec::new(), Vec::new()));
    }

    /// A real mod archive, added, applied to a game folder made for the
    /// test and taken out again, as in `ZENDO_TEST_ARCHIVE=<archive> cargo
    /// test -p zendo-core mods -- --ignored`.
    #[test]
    #[ignore = "needs a real mod archive in ZENDO_TEST_ARCHIVE"]
    fn a_real_archive_goes_in_and_out() {
        let Some(archive) = std::env::var_os("ZENDO_TEST_ARCHIVE") else {
            return;
        };
        let scratch = Scratch::new("real");
        let place = scratch.place(None, true);
        write(&place.game_dir.join("Game.exe"), "game");
        write(&place.game_dir.join("Game_Data/level0"), "level");
        place.add(Path::new(&archive), None, None).unwrap();
        let overview = place.overview().unwrap();
        assert!(overview.mods[0].files > 0);
        place.apply().unwrap();
        let placed = place.manifest().unwrap().unwrap();
        assert_eq!(placed.files.len(), overview.mods[0].files);
        place.restore().unwrap();
        let mut left = walk(&place.game_dir).unwrap();
        left.sort();
        assert_eq!(
            left,
            vec![PathBuf::from("Game.exe"), PathBuf::from("Game_Data/level0")]
        );
    }

    #[test]
    fn knows_games_by_steam_id_or_title() {
        let game = |id: &str, title: &str, source| {
            Game::new(
                id.into(),
                title.into(),
                source,
                crate::Launch::Uri { uri: String::new() },
            )
        };
        let stardew = game("steam:413150", "Stardew Valley", Source::Steam);
        assert_eq!(known(&stardew).map(|known| known.folder), Some("Mods"));
        let gog = game("gog:1", "Cyberpunk 2077", Source::Gog);
        assert_eq!(known(&gog).map(|known| known.nexus), Some("cyberpunk2077"));
        let unknown = game("steam:1", "Stardew Valley", Source::Steam);
        assert_eq!(known(&unknown), None);
    }

    #[test]
    fn names_mods_from_their_archives() {
        assert_eq!(
            archive_name(Path::new("/d/SkyUI_5_2_SE-12604-5-2SE.7z")),
            "SkyUI_5_2_SE-12604-5-2SE"
        );
        assert_eq!(archive_name(Path::new("/d/mod.tar.gz")), "mod");
        assert!(check_relative(Path::new("BepInEx/plugins")).is_ok());
        assert!(check_relative(Path::new("../x")).is_err());
        assert!(check_relative(Path::new("/x")).is_err());
    }
}
