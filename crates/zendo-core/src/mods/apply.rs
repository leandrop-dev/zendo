//! Apply and Back to vanilla: copying the mods on into the game, keeping
//! what it replaced and recording what went in, and taking it out again.

use super::{Entry, Manifest, Place, Placed, case_in, check_relative, walk};
use crate::paths;
use anyhow::{Context, bail};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

impl Place {
    pub(super) fn apply(&self) -> anyhow::Result<()> {
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
        let plugin_list = self.plugins(&list, None).filter(|_| self.appdata.is_some());
        if winners.is_empty() && plugin_list.is_none() {
            // Nothing on: the game stays as it is.
            return Ok(());
        }
        let mut placed = winners
            .into_values()
            .try_for_each(|(from, into)| self.place_file(&from, &into, &mut manifest));
        if let (Ok(()), Some(plugins)) = (&placed, &plugin_list) {
            placed = self.write_plugins(&folder, plugins, &mut manifest);
        }
        if let Err(err) = placed.and_then(|()| self.save_manifest(&manifest)) {
            // Take out what went in.
            let _ = self.take_out(&manifest);
            let _ = fs::remove_file(self.manifest_path());
            return Err(err);
        }
        Ok(())
    }

    /// Copies one file in, backing up the game's own and making folders.
    pub(super) fn place_file(
        &self,
        from: &Path,
        into: &Path,
        manifest: &mut Manifest,
    ) -> anyhow::Result<()> {
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
    pub(super) fn resolve_case(&self, path: &Path) -> PathBuf {
        if self.any_case {
            case_in(&self.game_dir, path)
        } else {
            path.to_path_buf()
        }
    }

    pub(super) fn save_manifest(&self, manifest: &Manifest) -> anyhow::Result<()> {
        let path = self.manifest_path();
        let json = serde_json::to_vec_pretty(manifest)?;
        paths::write_atomic(&path, &json)
            .with_context(|| format!("could not write {}", path.display()))
    }

    pub(super) fn restore(&self) -> anyhow::Result<()> {
        let Some(manifest) = self.manifest()? else {
            return Ok(());
        };
        self.take_out(&manifest)?;
        fs::remove_file(self.manifest_path()).context("could not delete applied.json")?;
        Ok(())
    }

    /// Removes what was placed, keeping what the game changed or wrote in
    /// `overwrite/`, and puts the game's own files back.
    pub(super) fn take_out(&self, manifest: &Manifest) -> anyhow::Result<()> {
        let game_dir = &manifest.game_dir;
        let overwrite = self.overwrite();
        for (path, time) in &manifest.times {
            let path = game_dir.join(path);
            if let Some(time) = time
                && let Ok(file) = fs::File::options().write(true).open(&path)
            {
                file.set_modified(*time)
                    .with_context(|| format!("could not date {}", path.display()))?;
            }
        }
        for backup in manifest.lists.iter().rev() {
            let path = &backup.path;
            match &backup.original {
                Some(bytes) => paths::write_atomic(path, bytes)
                    .with_context(|| format!("could not put back {}", path.display()))?,
                None => match fs::remove_file(path) {
                    Err(err) if err.kind() != ErrorKind::NotFound => {
                        return Err(err)
                            .with_context(|| format!("could not remove {}", path.display()));
                    }
                    _ => {}
                },
            }
        }
        for dir in manifest.made.iter().rev() {
            // Left when the game wrote there too.
            let _ = fs::remove_dir(dir);
        }
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
pub(super) fn keep(from: &Path, to: &Path) -> anyhow::Result<()> {
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
