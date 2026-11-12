//! Taking mods in: archives unpacked, the folder inside that is the mod,
//! and FOMOD installers built into a mod from the options chosen.

use super::{
    Entry, List, NexusSource, Pending, PendingKind, Place, case_in, entries, file_name,
    updated_entry, walk,
};
use crate::fomod;
use crate::fomod::{Installer, Present, Selection};
use crate::launch;
use crate::nexus;
use anyhow::{Context, bail};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

impl Place {
    /// Unpacks an archive into `files/.adding-<id>`: that folder, and the
    /// mod's root inside it.
    pub(super) fn unpack_new(&self, archive: &Path, id: &str) -> anyhow::Result<(PathBuf, String)> {
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

    /// Unpacks an archive as a new mod, or in place of `updates`. With a
    /// FOMOD installer it waits for the options, unpacked; otherwise it is in.
    pub(super) fn take_in(
        &self,
        archive: &Path,
        name: Option<String>,
        nexus: Option<NexusSource>,
        updates: Option<&Entry>,
    ) -> anyhow::Result<Option<Pending>> {
        let list = self.list()?;
        let name = name.unwrap_or_else(|| archive_name(archive));
        let id = match updates {
            Some(entry) => entry.id.clone(),
            None => self.free_id(&name, &list),
        };
        let source = file_name(archive);
        let (unpacked, root) = self.unpack_new(archive, &id)?;
        if let Some(installer_root) = find_installer(&unpacked) {
            let installer = match read_installer(&installer_root) {
                Ok(installer) => installer,
                Err(err) => {
                    let _ = fs::remove_dir_all(&unpacked);
                    return Err(err);
                }
            };
            let what = match updates {
                Some(_) => PendingKind::Update {
                    id,
                    raw: unpacked,
                    source,
                    nexus,
                },
                None => PendingKind::New {
                    id,
                    raw: unpacked,
                    source,
                    nexus,
                },
            };
            return Ok(Some(Pending {
                name: updates.map_or(name, |entry| entry.name.clone()),
                installer,
                present: self.present(&list),
                chosen: updates
                    .map(|entry| entry.chosen.clone())
                    .unwrap_or_default(),
                root: installer_root,
                what,
            }));
        }
        let old = self.put_in(&id, &unpacked)?;
        let installer_old = aside(&self.installers().join(&id))?;
        let written = self.change(|list| {
            match list.mods.iter_mut().find(|entry| entry.id == id) {
                Some(entry) => {
                    entry.root = root;
                    entry.source = source;
                    entry.nexus = nexus;
                    entry.installer = false;
                    entry.chosen.clear();
                }
                None => list.mods.push(Entry {
                    id: id.clone(),
                    name,
                    enabled: true,
                    root,
                    source,
                    nexus,
                    installer: false,
                    chosen: Vec::new(),
                }),
            }
            Ok(())
        });
        if let Err(err) = written {
            restore(&self.files().join(&id), old);
            restore(&self.installers().join(&id), installer_old);
            return Err(err);
        }
        for old in [old, installer_old].into_iter().flatten() {
            let _ = fs::remove_dir_all(old);
        }
        Ok(None)
    }

    /// An id for a new mod, not in the list nor waiting in `files/`.
    pub(super) fn free_id(&self, name: &str, list: &List) -> String {
        let mut taken = list.mods.clone();
        loop {
            let id = new_id(name, &taken);
            let files = self.files();
            if !files.join(&id).exists() && !files.join(format!(".adding-{id}")).exists() {
                return id;
            }
            taken.push(Entry {
                id,
                name: String::new(),
                enabled: false,
                root: String::new(),
                source: String::new(),
                nexus: None,
                installer: false,
                chosen: Vec::new(),
            });
        }
    }

    /// Puts a folder in as `files/<id>`, the mod's previous files moved
    /// aside rather than deleted: the list that records the new ones has
    /// not been written yet.
    pub(super) fn put_in(&self, id: &str, folder: &Path) -> anyhow::Result<Option<PathBuf>> {
        let kept = self.files().join(id);
        let old = aside(&kept)?;
        if let Err(err) = fs::rename(folder, &kept) {
            if let Some(old) = &old {
                let _ = fs::rename(old, &kept);
            }
            let _ = fs::remove_dir_all(folder);
            return Err(err)
                .with_context(|| format!("could not keep the mod in {}", kept.display()));
        }
        Ok(old)
    }

    /// [`add_from_nexus`]'s work: the installer to run, if any, and whether
    /// it updates a mod.
    pub(super) fn add_fetched(
        &self,
        fetched: &nexus::Fetched,
        domain: &str,
        mod_id: u64,
    ) -> anyhow::Result<(Option<Pending>, bool)> {
        let source = NexusSource {
            domain: domain.to_owned(),
            mod_id,
            file_id: fetched.file.file_id,
            version: fetched.file.version.clone().unwrap_or_default(),
        };
        let list = self.list()?;
        let updates = updated_entry(&list, &source, &fetched.files).cloned();
        let result = self.take_in(
            &fetched.archive,
            Some(fetched.mod_name.clone()),
            Some(source),
            updates.as_ref(),
        );
        let _ = fs::remove_file(&fetched.archive);
        Ok((result?, updates.is_some()))
    }

    /// What file conditions see: the mod folder's files (a few levels deep)
    /// and those of the mods that are on.
    pub(super) fn present(&self, list: &List) -> Present {
        let (folder, _) = self.folder(list);
        let mut files = HashSet::new();
        listed(&self.game_dir.join(&folder), Path::new(""), 4, &mut files);
        for entry in list.mods.iter().filter(|entry| entry.enabled) {
            // Best effort here: a broken mod's files are missing from the
            // conditions, and `overview` is where its error is said.
            for file in walk(&self.mod_root(entry)).unwrap_or_default() {
                files.insert(fomod::normalise(&file.to_string_lossy()).to_lowercase());
            }
        }
        let manifest = self.manifest().ok().flatten();
        let inactive = self
            .plugins(list, manifest.as_ref())
            .unwrap_or_default()
            .into_iter()
            .filter(|plugin| !plugin.enabled)
            .map(|plugin| plugin.name.to_lowercase())
            .collect();
        Present { files, inactive }
    }

    /// [`install`]'s work.
    pub(super) fn install(&self, pending: &Pending, selection: &Selection) -> anyhow::Result<()> {
        let id = pending.id().to_owned();
        let installs = pending.installer.files(selection, &pending.present);
        let chosen = pending.installer.chosen(selection, &pending.present);
        let built = self.files().join(format!(".built-{id}"));
        if built.exists() {
            fs::remove_dir_all(&built)
                .with_context(|| format!("could not clear {}", built.display()))?;
        }
        if let Err(err) = self.build(&pending.root, &installs, &built) {
            let _ = fs::remove_dir_all(&built);
            return Err(err);
        }
        let files_old = self.put_in(&id, &built)?;
        let (source, nexus, installer_old) = match &pending.what {
            PendingKind::New {
                raw, source, nexus, ..
            }
            | PendingKind::Update {
                raw, source, nexus, ..
            } => {
                let kept = self.installers().join(&id);
                let old = aside(&kept)?;
                fs::create_dir_all(self.installers())?;
                fs::rename(raw, &kept).with_context(|| {
                    format!("could not keep the installer in {}", kept.display())
                })?;
                (Some(source.clone()), Some(nexus.clone()), old)
            }
            PendingKind::Options { .. } => (None, None, None),
        };
        let written = self.change(|list| {
            match list.mods.iter_mut().find(|entry| entry.id == id) {
                Some(entry) => {
                    entry.root = String::new();
                    entry.installer = true;
                    entry.chosen = chosen;
                    if let Some(source) = source {
                        entry.source = source;
                    }
                    if let Some(nexus) = nexus {
                        entry.nexus = nexus;
                    }
                }
                None => list.mods.push(Entry {
                    id: id.clone(),
                    name: pending.name.clone(),
                    enabled: true,
                    root: String::new(),
                    source: source.unwrap_or_default(),
                    nexus: nexus.flatten(),
                    installer: true,
                    chosen,
                }),
            }
            Ok(())
        });
        if let Err(err) = written {
            restore(&self.files().join(&id), files_old);
            restore(&self.installers().join(&id), installer_old);
            return Err(err);
        }
        for old in [files_old, installer_old].into_iter().flatten() {
            let _ = fs::remove_dir_all(old);
        }
        Ok(())
    }

    /// Copies what an installer's options install from `root` into `into`,
    /// lowest priority first. Sources are found whatever their case.
    pub(super) fn build(
        &self,
        root: &Path,
        installs: &[fomod::Install],
        into: &Path,
    ) -> anyhow::Result<()> {
        let all = walk(root).context("could not read the unpacked archive")?;
        let index: Vec<(String, &PathBuf)> = all
            .iter()
            .map(|path| {
                (
                    fomod::normalise(&path.to_string_lossy()).to_lowercase(),
                    path,
                )
            })
            .collect();
        fs::create_dir_all(into).with_context(|| format!("could not make {}", into.display()))?;
        let mut placed = 0;
        let mut missing = Vec::new();
        for install in installs {
            let Some(target) = install.target() else {
                // Name the value that left the mod: the source when it is
                // itself unsafe, else the destination the installer gave.
                let bad = if fomod::safe(&install.source) {
                    install.destination.as_deref().unwrap_or(&install.source)
                } else {
                    install.source.as_str()
                };
                bail!("the installer puts {bad} outside the mod, which Zendo does not allow");
            };
            let source = install.source.to_lowercase();
            let copies: Vec<(PathBuf, PathBuf)> = if install.folder {
                let depth = if source.is_empty() {
                    0
                } else {
                    source.split('/').count()
                };
                let prefix = if source.is_empty() {
                    String::new()
                } else {
                    format!("{source}/")
                };
                index
                    .iter()
                    .filter(|(lower, _)| {
                        lower.starts_with(&prefix)
                            && !(source.is_empty() && lower.starts_with("fomod/"))
                    })
                    .map(|(_, path)| {
                        let rest: PathBuf = path.components().skip(depth).collect();
                        (root.join(path), Path::new(&target).join(rest))
                    })
                    .collect()
            } else {
                index
                    .iter()
                    .filter(|(lower, _)| *lower == source)
                    .map(|(_, path)| (root.join(path), PathBuf::from(&target)))
                    .take(1)
                    .collect()
            };
            if copies.is_empty() {
                missing.push(install.source.clone());
            }
            for (from, to) in copies {
                let to = if self.any_case {
                    into.join(case_in(into, &to))
                } else {
                    into.join(to)
                };
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("could not make {}", parent.display()))?;
                }
                if to.is_dir() {
                    bail!(
                        "the installer puts a file where a folder is: {}",
                        to.display()
                    );
                }
                if to.exists() {
                    fs::remove_file(&to)?;
                }
                fs::copy(&from, &to)
                    .with_context(|| format!("could not copy {}", from.display()))?;
                placed += 1;
            }
        }
        if placed == 0 {
            if missing.is_empty() {
                bail!("the options chosen install no files");
            }
            bail!(
                "the installer names files its archive does not have: {}",
                missing.join(", ")
            );
        }
        Ok(())
    }

    /// The folder inside an unpacked archive whose contents are the mod.
    pub(super) fn find_root(&self, unpacked: &Path) -> String {
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
}

/// The files under a folder, lowercase, `depth` levels down at most, links
/// left out: for file conditions, so a game's own folder may be large.
pub(super) fn listed(root: &Path, at: &Path, depth: usize, found: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(root.join(at)) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = at.join(entry.file_name());
        if kind.is_dir() && depth > 0 {
            listed(root, &path, depth - 1, found);
        } else if kind.is_file() {
            found.insert(fomod::normalise(&path.to_string_lossy()).to_lowercase());
        }
    }
}

/// The folder holding `fomod/ModuleConfig.xml`, any case, at most three
/// folders down.
pub(super) fn find_installer(unpacked: &Path) -> Option<PathBuf> {
    fn look(dir: &Path, depth: usize) -> Option<PathBuf> {
        let (dirs, _) = entries(dir);
        if let Some(fomod) = dirs.iter().find(|name| name.eq_ignore_ascii_case("fomod")) {
            let (_, files) = entries(&dir.join(fomod));
            if files
                .iter()
                .any(|name| name.eq_ignore_ascii_case("ModuleConfig.xml"))
            {
                return Some(dir.to_path_buf());
            }
        }
        if depth == 0 {
            return None;
        }
        dirs.iter()
            .find_map(|name| look(&dir.join(name), depth - 1))
    }
    look(unpacked, 3)
}

pub(super) fn read_installer(root: &Path) -> anyhow::Result<Installer> {
    let (dirs, _) = entries(root);
    let fomod = dirs
        .iter()
        .find(|name| name.eq_ignore_ascii_case("fomod"))
        .context("the installer's folder is gone")?;
    let (_, files) = entries(&root.join(fomod));
    let config = files
        .iter()
        .find(|name| name.eq_ignore_ascii_case("ModuleConfig.xml"))
        .context("the installer's ModuleConfig.xml is gone")?;
    let bytes = fs::read(root.join(fomod).join(config))
        .context("could not read the installer's ModuleConfig.xml")?;
    Installer::parse(&bytes).context("its FOMOD installer could not be read")
}

pub(super) fn archive_name(archive: &Path) -> String {
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
pub(super) fn new_id(name: &str, taken: &[Entry]) -> String {
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
pub(crate) fn unpack(archive: &Path, into: &Path) -> anyhow::Result<()> {
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
        bail!("unpacking needs 7-Zip (7zip) or bsdtar (libarchive)");
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

/// Moves a folder to `<name>.old` beside it, so a change whose list write
/// fails can put it back.
fn aside(path: &Path) -> anyhow::Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut name = path.as_os_str().to_owned();
    name.push(".old");
    let old = PathBuf::from(name);
    if old.exists() {
        fs::remove_dir_all(&old).with_context(|| format!("could not clear {}", old.display()))?;
    }
    fs::rename(path, &old).with_context(|| format!("could not move {} aside", path.display()))?;
    Ok(Some(old))
}

/// Puts back what [`aside`] moved, if anything.
fn restore(path: &Path, old: Option<PathBuf>) {
    let _ = fs::remove_dir_all(path);
    if let Some(old) = old {
        let _ = fs::rename(old, path);
    }
}
