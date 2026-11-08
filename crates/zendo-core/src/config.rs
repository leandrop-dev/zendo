//! What the user sets, in `$XDG_CONFIG_HOME/zendo/config.toml`, by hand or
//! from Zendo's Settings page:
//!
//! ```toml
//! # A key from https://www.steamgriddb.com/profile/preferences, for covers
//! # of games whose store has none.
//! steamgriddb_key = "…"
//! # A personal key from Nexus Mods' API Access page, for downloading mods.
//! nexus_key = "…"
//! # A Steam Web API key from https://steamcommunity.com/dev/apikey, for
//! # the Steam games you own but have not installed.
//! steam_key = "…"
//! # Set to false to never download covers.
//! download_covers = true
//! # Where store games are installed, each in a folder of its own.
//! install_dir = "~/Games"
//! # The Proton Windows games run with unless their launch options choose
//! # one: a folder name. Left out, the newest GE-Proton found, then others.
//! default_proton = "GE-Proton11-12-x86_64"
//! # The window's language: "en" or "pt-PT". Left out, the system's.
//! language = "pt-PT"
//! # The window's colours: "zendo", Zendo's own; or, on a computer with
//! # hyprmonk, "hyprmonk" to follow its tone. Left out, "zendo".
//! theme = "hyprmonk"
//! ```
//!
//! The API keys are kept in the system keyring ([`secrets`]) when one
//! answers, and in this file only when none does: then Zendo writes it
//! readable by its owner only. A key found in the file is moved into the
//! keyring the next time Zendo reads its settings. A key written `""`
//! means none, without asking the keyring: what Zendo writes when a key
//! is removed while the keyring cannot be reached.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::secrets::{self, Key, Keyring};

const HEADER: &str = "\
# Zendo's settings. The Settings page rewrites this file; see
# https://github.com/leandrop-dev/zendo#settings-and-files

";

/// Zendo's settings. Its `Debug` leaves the keys out, so a log or a
/// report never shows them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steamgriddb_key: Option<String>,
    /// The user's personal Nexus Mods API key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nexus_key: Option<String>,
    /// The user's Steam Web API key, for the games owned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_key: Option<String>,
    pub download_covers: bool,
    /// Where store games are installed; `~/Games` when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<PathBuf>,
    /// The Proton games run with unless their launch options choose one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_proton: Option<String>,
    /// The window's language as a tag (`en`, `pt-PT`); the system's when
    /// unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The window's colours by name (`zendo`, `hyprmonk`); Zendo's own
    /// when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Where each key is kept, and its value there.
    #[serde(skip)]
    held: [Held; 3],
}

/// Where a key is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Kept {
    /// No key.
    #[default]
    Nowhere,
    Keyring,
    /// In `config.toml`: with why the keyring did not take it, or `None`
    /// when the keyring was not asked.
    File(Option<String>),
    /// Why the keyring could not be read: a key there is not known.
    Unread(String),
}

/// A key as last read or written.
#[derive(Clone, Default, PartialEq, Eq)]
struct Held {
    kept: Kept,
    saved: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hidden = |key: &Option<String>| key.as_ref().map(|_| "…");
        f.debug_struct("Config")
            .field("steamgriddb_key", &hidden(&self.steamgriddb_key))
            .field("nexus_key", &hidden(&self.nexus_key))
            .field("steam_key", &hidden(&self.steam_key))
            .field("download_covers", &self.download_covers)
            .field("install_dir", &self.install_dir)
            .field("default_proton", &self.default_proton)
            .field("language", &self.language)
            .field("theme", &self.theme)
            .field("steamgriddb_kept", self.kept(Key::SteamGridDb))
            .field("nexus_kept", self.kept(Key::Nexus))
            .field("steam_kept", self.kept(Key::Steam))
            .finish_non_exhaustive()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            steamgriddb_key: None,
            nexus_key: None,
            steam_key: None,
            download_covers: true,
            install_dir: None,
            default_proton: None,
            language: None,
            theme: None,
            held: Default::default(),
        }
    }
}

impl Config {
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::config_home().map(|dir| dir.join("zendo/config.toml"))
    }

    /// Where store games go: `install_dir` with a leading `~` expanded, or
    /// `~/Games`.
    #[must_use]
    pub fn install_dir(&self) -> Option<PathBuf> {
        let home = paths::home();
        match &self.install_dir {
            Some(dir) => match dir.strip_prefix("~") {
                Ok(rest) => home.map(|home| home.join(rest)),
                Err(_) => Some(dir.clone()),
            },
            None => home.map(|home| home.join("Games")),
        }
    }

    /// The key, when there is one.
    #[must_use]
    pub fn key(&self, key: Key) -> Option<&str> {
        match key {
            Key::SteamGridDb => self.steamgriddb_key.as_deref(),
            Key::Nexus => self.nexus_key.as_deref(),
            Key::Steam => self.steam_key.as_deref(),
        }
    }

    fn key_mut(&mut self, key: Key) -> &mut Option<String> {
        match key {
            Key::SteamGridDb => &mut self.steamgriddb_key,
            Key::Nexus => &mut self.nexus_key,
            Key::Steam => &mut self.steam_key,
        }
    }

    /// Where the key is kept, as last read or written.
    #[must_use]
    pub fn kept(&self, key: Key) -> &Kept {
        &self.held[key as usize].kept
    }

    /// The user's settings with the keys, or the defaults when there is no
    /// file. Asks the keyring, which may show its unlock prompt and wait for
    /// it, so not for the window's thread.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or is not valid, or a key
    /// moved into the keyring cannot be taken out of the file.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_with(Self::path().as_deref(), &secrets::System, true)
    }

    /// As [`Config::load`], but a key found in the file stays there: for a
    /// report, which changes nothing.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or is not valid.
    pub fn load_as_is() -> anyhow::Result<Self> {
        Self::load_with(Self::path().as_deref(), &secrets::System, false)
    }

    /// The settings file alone, without asking the keyring: for work that
    /// needs no key (launching, installing).
    ///
    /// # Errors
    ///
    /// As [`Config::load`].
    pub fn load_file() -> anyhow::Result<Self> {
        Self::path()
            .as_deref()
            .map_or_else(|| Ok(Self::default()), Self::read)
    }

    /// Writes the settings back, readable by the user only, and a changed
    /// key to the keyring, or to the file when the keyring cannot take it.
    /// Comments added by hand are not kept.
    ///
    /// # Errors
    ///
    /// When there is no config folder or the file cannot be written.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let path = Self::path().context("no home folder to keep Zendo's settings in")?;
        self.save_to(&path, &secrets::System)
    }

    #[cfg(test)]
    fn load_from(path: Option<&Path>, keyring: &dyn Keyring) -> anyhow::Result<Self> {
        Self::load_with(path, keyring, true)
    }

    /// The settings and the keys; `move_keys` moves those in the file into
    /// the keyring.
    fn load_with(
        path: Option<&Path>,
        keyring: &dyn Keyring,
        move_keys: bool,
    ) -> anyhow::Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let mut config = Self::read(path)?;
        let mut moved = false;
        for key in Key::ALL {
            let value = config.key(key).map(str::to_owned);
            let held = &mut config.held[key as usize];
            if let Kept::File(_) = held.kept {
                if !move_keys {
                    held.saved = value;
                    continue;
                }
                // Out of the file and into the keyring, when it takes it.
                match keyring.set(key, value.as_deref()) {
                    Ok(()) => {
                        held.kept = kept_in_keyring(value.as_ref());
                        moved = true;
                    }
                    Err(err) => held.kept = Kept::File(Some(format!("{err:#}"))),
                }
                held.saved = value;
                continue;
            }
            match keyring.get(key) {
                Ok(found) => {
                    let found = normalise_key(found);
                    held.kept = kept_in_keyring(found.as_ref());
                    held.saved.clone_from(&found);
                    *config.key_mut(key) = found;
                }
                Err(err) => held.kept = Kept::Unread(format!("{err:#}")),
            }
        }
        if moved {
            config
                .write(path)
                .context("the keys moved into the keyring, but not out of the file")?;
        }
        Ok(config)
    }

    fn save_to(&mut self, path: &Path, keyring: &dyn Keyring) -> anyhow::Result<()> {
        for key in Key::ALL {
            let value = normalise_key(self.key_mut(key).take());
            self.key_mut(key).clone_from(&value);
            let held = &mut self.held[key as usize];
            // A key already in the keyring is not written again; one in
            // the file is offered to the keyring again.
            let kept = value == held.saved && !matches!(held.kept, Kept::File(_));
            if !kept {
                held.kept = match keyring.set(key, value.as_deref()) {
                    Ok(()) => kept_in_keyring(value.as_ref()),
                    Err(err) => Kept::File(Some(format!("{err:#}"))),
                };
            }
            held.saved = value;
        }
        self.write(path)
    }

    /// The file: every setting, and the keys the keyring did not take.
    fn write(&self, path: &Path) -> anyhow::Result<()> {
        let mut file = self.clone();
        for key in Key::ALL {
            let in_file = matches!(self.kept(key), Kept::File(_));
            let value = file.key_mut(key);
            *value = in_file.then(|| value.take().unwrap_or_default());
        }
        let text = format!("{HEADER}{}", toml::to_string(&file)?);
        paths::write_private(path, text.as_bytes())
            .with_context(|| format!("could not write {}", path.display()))
    }

    fn read(path: &Path) -> anyhow::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).with_context(|| format!("{}", path.display())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// The file's settings; a key written in it, even `""`, is kept there.
    fn parse(text: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        for key in Key::ALL {
            let written = config.key_mut(key).take();
            if written.is_some() {
                config.held[key as usize].kept = Kept::File(None);
            }
            *config.key_mut(key) = normalise_key(written);
        }
        Ok(config)
    }
}

fn kept_in_keyring(value: Option<&String>) -> Kept {
    if value.is_some() {
        Kept::Keyring
    } else {
        Kept::Nowhere
    }
}

fn normalise_key(key: Option<String>) -> Option<String> {
    key.map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::tests::Memory;

    #[test]
    fn debug_never_shows_a_key() {
        let config = Config {
            steamgriddb_key: Some("sgdb-secret".into()),
            nexus_key: Some("nexus-secret".into()),
            steam_key: Some("steam-secret".into()),
            ..Config::default()
        };
        let shown = format!("{config:?}");
        assert!(!shown.contains("secret"), "{shown}");
        assert!(shown.contains("nexus_key: Some(\"…\")"), "{shown}");
        assert!(format!("{:?}", Config::default()).contains("nexus_key: None"));
    }

    #[test]
    fn reads_the_key_and_defaults_the_rest() {
        let config = Config::parse("steamgriddb_key = \" abc \"\n").unwrap();
        assert_eq!(config.steamgriddb_key.as_deref(), Some("abc"));
        assert!(config.download_covers);
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert!(Config::parse("steamgrid_key = \"typo\"").is_err());
    }

    #[test]
    fn writes_what_it_reads() {
        let config = Config {
            download_covers: false,
            install_dir: Some("/mnt/games".into()),
            default_proton: Some("GE-Proton11-12".into()),
            ..Config::default()
        };
        let text = format!("{HEADER}{}", toml::to_string(&config).unwrap());
        assert_eq!(Config::parse(&text).unwrap(), config);
        let text = toml::to_string(&Config::default()).unwrap();
        assert!(!text.contains("steamgriddb_key"), "{text}");
        assert!(!text.contains("nexus_key"), "{text}");
    }

    /// A settings file of its own, removed after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("zendo-config-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir.join("config.toml"))
        }

        fn text(&self) -> String {
            fs::read_to_string(&self.0).unwrap()
        }

        fn load(&self, keyring: &Memory) -> Config {
            Config::load_from(Some(&self.0), keyring).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = fs::remove_dir_all(dir);
            }
        }
    }

    fn with_keys() -> Config {
        Config {
            steamgriddb_key: Some(" sgdb ".into()),
            nexus_key: Some("nx".into()),
            ..Config::default()
        }
    }

    #[test]
    fn keys_go_to_the_keyring_not_the_file() {
        let scratch = Scratch::new("keyring");
        let keyring = Memory::default();
        let mut config = with_keys();
        config.save_to(&scratch.0, &keyring).unwrap();
        assert!(!scratch.text().contains("_key"), "{}", scratch.text());
        assert_eq!(keyring.keys.borrow()["steamgriddb"], "sgdb");
        assert_eq!(config.kept(Key::Nexus), &Kept::Keyring);
        let read = scratch.load(&keyring);
        assert_eq!(read.key(Key::SteamGridDb), Some("sgdb"));
        assert_eq!(read.key(Key::Nexus), Some("nx"));
        assert_eq!(read.kept(Key::SteamGridDb), &Kept::Keyring);
    }

    #[test]
    fn a_key_in_the_file_moves_into_the_keyring() {
        let scratch = Scratch::new("moves");
        fs::write(&scratch.0, "nexus_key = \"nx\"\ndownload_covers = false\n").unwrap();
        let keyring = Memory::default();
        let config = scratch.load(&keyring);
        assert_eq!(config.key(Key::Nexus), Some("nx"));
        assert_eq!(config.kept(Key::Nexus), &Kept::Keyring);
        assert_eq!(keyring.keys.borrow()["nexus"], "nx");
        assert!(!scratch.text().contains("nexus_key"), "{}", scratch.text());
        assert!(scratch.text().contains("download_covers = false"));
    }

    #[test]
    fn reading_as_is_moves_nothing() {
        let scratch = Scratch::new("as-is");
        fs::write(&scratch.0, "nexus_key = \"nx\"\n").unwrap();
        let keyring = Memory::default();
        keyring
            .keys
            .borrow_mut()
            .insert("steamgriddb", "sgdb".into());
        let config = Config::load_with(Some(&scratch.0), &keyring, false).unwrap();
        assert_eq!(config.key(Key::Nexus), Some("nx"));
        assert_eq!(config.kept(Key::Nexus), &Kept::File(None));
        assert_eq!(config.key(Key::SteamGridDb), Some("sgdb"));
        assert!(!keyring.keys.borrow().contains_key("nexus"));
        assert_eq!(scratch.text(), "nexus_key = \"nx\"\n");
    }

    #[test]
    fn without_a_keyring_the_keys_stay_in_the_file() {
        let scratch = Scratch::new("file");
        let keyring = Memory::default();
        keyring.broken.set(Some("no keyring answered"));
        let mut config = with_keys();
        config.save_to(&scratch.0, &keyring).unwrap();
        let why = Kept::File(Some("no keyring answered".into()));
        assert_eq!(config.kept(Key::Nexus), &why);
        assert!(scratch.text().contains("nexus_key = \"nx\""));
        let read = scratch.load(&keyring);
        assert_eq!(read.key(Key::SteamGridDb), Some("sgdb"));
        assert_eq!(read.kept(Key::SteamGridDb), &why);
        // Once a keyring answers, the next read moves them there.
        keyring.broken.set(None);
        let read = scratch.load(&keyring);
        assert_eq!(read.kept(Key::Nexus), &Kept::Keyring);
        assert!(!scratch.text().contains("_key"), "{}", scratch.text());
    }

    #[test]
    fn a_key_removed_while_the_keyring_is_away_stays_removed() {
        let scratch = Scratch::new("removed");
        let keyring = Memory::default();
        keyring.keys.borrow_mut().insert("nexus", "old".into());
        let mut config = scratch.load(&keyring);
        assert_eq!(config.key(Key::Nexus), Some("old"));
        keyring.broken.set(Some("the keyring is locked"));
        config.nexus_key = None;
        config.save_to(&scratch.0, &keyring).unwrap();
        assert!(
            scratch.text().contains("nexus_key = \"\""),
            "{}",
            scratch.text()
        );
        assert_eq!(scratch.load(&keyring).key(Key::Nexus), None);
        // Unlocked again, the old key is removed, not brought back.
        keyring.broken.set(None);
        let read = scratch.load(&keyring);
        assert_eq!(read.key(Key::Nexus), None);
        assert_eq!(read.kept(Key::Nexus), &Kept::Nowhere);
        assert!(keyring.keys.borrow().is_empty());
        assert!(!scratch.text().contains("nexus_key"), "{}", scratch.text());
    }

    #[test]
    fn a_key_the_keyring_holds_is_left_alone_by_other_changes() {
        let scratch = Scratch::new("alone");
        let keyring = Memory::default();
        keyring.keys.borrow_mut().insert("nexus", "nx".into());
        let mut config = scratch.load(&keyring);
        keyring.broken.set(Some("the keyring is locked"));
        config.default_proton = Some("GE-Proton11-12".into());
        config.save_to(&scratch.0, &keyring).unwrap();
        assert_eq!(config.kept(Key::Nexus), &Kept::Keyring);
        assert!(!scratch.text().contains("nexus_key"), "{}", scratch.text());
    }

    #[test]
    fn a_keyring_that_cannot_be_read_keeps_its_key() {
        let scratch = Scratch::new("unread");
        let keyring = Memory::default();
        keyring.keys.borrow_mut().insert("nexus", "nx".into());
        keyring.broken.set(Some("the keyring was not unlocked"));
        let mut config = scratch.load(&keyring);
        assert_eq!(config.key(Key::Nexus), None);
        let why = Kept::Unread("the keyring was not unlocked".into());
        assert_eq!(config.kept(Key::Nexus), &why);
        config.download_covers = false;
        config.save_to(&scratch.0, &keyring).unwrap();
        keyring.broken.set(None);
        assert_eq!(scratch.load(&keyring).key(Key::Nexus), Some("nx"));
    }

    #[test]
    fn puts_games_in_the_folder_asked_for() {
        let home = paths::home().unwrap();
        assert_eq!(Config::default().install_dir(), Some(home.join("Games")));
        let config = Config::parse("install_dir = \"~/Spiele\"").unwrap();
        assert_eq!(config.install_dir(), Some(home.join("Spiele")));
        let config = Config::parse("install_dir = \"/mnt/games\"").unwrap();
        assert_eq!(config.install_dir(), Some(PathBuf::from("/mnt/games")));
    }
}
