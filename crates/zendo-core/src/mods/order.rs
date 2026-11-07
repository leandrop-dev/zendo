//! A Bethesda game's plugin order: the plugins found, arranged with
//! [`plugins::arrange`], and the lists Apply writes in the game's prefix.

use super::{List, ListBackup, Manifest, Place, case_in, entries, file_name};
use crate::plugins::Plugin;
use crate::{paths, plugins};
use anyhow::Context;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

impl Place {
    pub(super) fn rules(&self) -> Option<&'static plugins::Rules> {
        self.known.and_then(|known| known.plugins)
    }

    /// The game's plugins in the order they load: its own in the mod folder
    /// and those of the mods on (a mod's replacing the game's of the same
    /// name). `None` for a game without plugins.
    pub(super) fn plugins(&self, list: &List, manifest: Option<&Manifest>) -> Option<Vec<Plugin>> {
        let rules = self.rules()?;
        let (folder, _) = self.folder(list);
        let data = self.game_dir.join(self.resolve_case(Path::new(&folder)));
        // Names straight in the mod folder, of paths from the game's.
        let top = |paths: &mut dyn Iterator<Item = &PathBuf>| -> HashSet<String> {
            paths
                .filter(|path| {
                    path.parent().is_some_and(|parent| {
                        parent.to_string_lossy().eq_ignore_ascii_case(&folder)
                    })
                })
                .map(|path| file_name(path).to_lowercase())
                .collect()
        };
        let placed = manifest.map_or_else(HashSet::new, |manifest| {
            top(&mut manifest.files.iter().map(|placed| &placed.path))
        });
        let replaced =
            manifest.map_or_else(HashSet::new, |manifest| top(&mut manifest.backups.iter()));

        let mut found: Vec<plugins::Found> = Vec::new();
        for name in entries(&data).1 {
            let lower = name.to_lowercase();
            if !plugins::is_plugin(&name) || (placed.contains(&lower) && !replaced.contains(&lower))
            {
                continue;
            }
            found.push(plugins::Found {
                header: plugins::read_header(&data.join(&name), rules).ok(),
                name,
                from: None,
            });
        }
        for entry in list.mods.iter().filter(|entry| entry.enabled) {
            let root = self.mod_root(entry);
            for name in entries(&root).1 {
                if !plugins::is_plugin(&name) {
                    continue;
                }
                let header = plugins::read_header(&root.join(&name), rules).ok();
                let from = Some(entry.name.clone());
                match found
                    .iter_mut()
                    .find(|other| other.name.eq_ignore_ascii_case(&name))
                {
                    Some(other) => {
                        other.from = from;
                        other.header = header;
                    }
                    None => found.push(plugins::Found { name, from, header }),
                }
            }
        }
        // The game's own are on as its list had them before Zendo, or all
        // when it had none; a mod's are on.
        let original = self.original_list(manifest);
        let also = rules.also_implicit(&data, &self.game_dir, self.my_games.as_deref());
        Some(plugins::arrange(
            rules,
            found,
            &also,
            &list.stored_plugins(),
            |found| {
                found.from.is_some()
                    || original.as_ref().is_none_or(|original| {
                        original
                            .iter()
                            .any(|(name, on)| *on && name.eq_ignore_ascii_case(&found.name))
                    })
            },
        ))
    }

    /// One of the game's list files, spelt as the one already there is.
    pub(super) fn list_file(&self, name: &str) -> Option<PathBuf> {
        let dir = self.appdata.as_ref()?;
        Some(dir.join(case_in(dir, Path::new(name))))
    }

    /// The game's `Plugins.txt` as it was before Zendo wrote it.
    pub(super) fn original_list(&self, manifest: Option<&Manifest>) -> Option<Vec<(String, bool)>> {
        let rules = self.rules()?;
        let path = self.list_file(plugins::PLUGINS_TXT)?;
        let kept =
            manifest.and_then(|manifest| manifest.lists.iter().find(|backup| backup.path == path));
        let bytes = match kept {
            Some(backup) => backup.original.clone()?,
            None => fs::read(&path).ok()?,
        };
        Some(plugins::read_list(
            &String::from_utf8_lossy(&bytes),
            rules.style,
        ))
    }

    /// The plugin order Apply writes; `None` when it writes none.
    pub(super) fn wanted(&self, plugins: Option<&[Plugin]>) -> Option<Vec<(String, bool)>> {
        self.appdata.as_ref().and(plugins).map(order_of)
    }

    /// Changes the plugin order kept: `edit` gives the new one, or `None`
    /// to leave it.
    pub(super) fn change_plugins(
        &self,
        edit: impl FnOnce(&[Plugin]) -> Option<Vec<(String, bool)>>,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest()?;
        self.change(|list| {
            let plugins = self
                .plugins(list, manifest.as_ref())
                .context("Zendo keeps no plugin order for this game")?;
            if let Some(order) = edit(&plugins) {
                list.keep_plugins(&order);
            }
            Ok(())
        })
    }

    /// Writes the plugin order into the game's lists in its prefix, keeping
    /// what they held; for Fallout 3 and New Vegas, also dates the plugins
    /// a minute apart in that order.
    pub(super) fn write_plugins(
        &self,
        folder: &str,
        plugins: &[Plugin],
        manifest: &mut Manifest,
    ) -> anyhow::Result<()> {
        let (Some(rules), Some(appdata)) = (self.rules(), &self.appdata) else {
            return Ok(());
        };
        let order = order_of(plugins);
        let implicit: Vec<String> = plugins
            .iter()
            .filter(|plugin| plugin.rank == plugins::Rank::Implicit)
            .map(|plugin| plugin.name.clone())
            .collect();
        let mut missing: Vec<&Path> = appdata
            .ancestors()
            .take_while(|dir| !dir.exists())
            .collect();
        missing.reverse();
        for dir in missing {
            fs::create_dir(dir).with_context(|| format!("could not make {}", dir.display()))?;
            manifest.made.push(dir.to_path_buf());
        }
        for (name, text) in plugins::files_for(rules.style, &order, &implicit) {
            let path = appdata.join(case_in(appdata, Path::new(name)));
            let original = match fs::read(&path) {
                Ok(bytes) => Some(bytes),
                Err(err) if err.kind() == ErrorKind::NotFound => None,
                Err(err) => {
                    return Err(err).with_context(|| format!("could not read {}", path.display()));
                }
            };
            manifest.lists.push(ListBackup {
                path: path.clone(),
                original,
            });
            paths::write_atomic(&path, text.as_bytes())
                .with_context(|| format!("could not write {}", path.display()))?;
        }
        if rules.style == plugins::Style::Timestamps {
            self.date_plugins(folder, &order, manifest)?;
        }
        manifest.plugins = Some(order);
        Ok(())
    }

    /// Sets the plugins' modified times a minute apart from a fixed day,
    /// in order, keeping the game's own files' times to put back.
    pub(super) fn date_plugins(
        &self,
        folder: &str,
        order: &[(String, bool)],
        manifest: &mut Manifest,
    ) -> anyhow::Result<()> {
        // 2008-01-10, before any of these games' files.
        #[expect(
            clippy::duration_suboptimal_units,
            reason = "a Unix time, which is counted in seconds"
        )]
        let mut time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_200_000_000);
        for (name, _) in order {
            let into = self.resolve_case(&Path::new(folder).join(name));
            let path = self.game_dir.join(&into);
            let key = self.file_key(&into);
            let placed = manifest
                .files
                .iter()
                .position(|placed| self.file_key(&placed.path) == key);
            let file = fs::File::options()
                .write(true)
                .open(&path)
                .with_context(|| format!("could not open {}", path.display()))?;
            if placed.is_none() {
                let before = file.metadata().ok().and_then(|meta| meta.modified().ok());
                manifest.times.push((into.clone(), before));
            }
            file.set_modified(time)
                .with_context(|| format!("could not date {}", path.display()))?;
            if let Some(at) = placed {
                manifest.files[at].modified = fs::metadata(&path)
                    .ok()
                    .and_then(|meta| meta.modified().ok());
            }
            time += Duration::from_secs(60);
        }
        Ok(())
    }
}

pub(super) fn order_of(plugins: &[Plugin]) -> Vec<(String, bool)> {
    plugins
        .iter()
        .map(|plugin| (plugin.name.clone(), plugin.enabled))
        .collect()
}
