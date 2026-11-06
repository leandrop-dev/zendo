//! What the user sets, in `$XDG_CONFIG_HOME/zendo/config.toml`, by hand or
//! from Zendo's Settings page:
//!
//! ```toml
//! # A key from https://www.steamgriddb.com/profile/preferences, for covers
//! # of games whose store has none.
//! steamgriddb_key = "…"
//! # Set to false to never download covers.
//! download_covers = true
//! # Where store games are installed, each in a folder of its own.
//! install_dir = "~/Games"
//! ```
//!
//! The file can hold a key, so Zendo writes it readable by its owner only.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::paths;

const HEADER: &str = "\
# Zendo's settings. The Settings page rewrites this file; see
# https://github.com/leandrop-dev/zendo#settings-and-files

";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steamgriddb_key: Option<String>,
    pub download_covers: bool,
    /// Where store games are installed; `~/Games` when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            steamgriddb_key: None,
            download_covers: true,
            install_dir: None,
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

    /// The user's settings, or the defaults when there is no file.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or is not valid.
    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).with_context(|| format!("{}", path.display())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// Writes the settings back, readable by the user only. Comments added
    /// by hand are not kept.
    ///
    /// # Errors
    ///
    /// When there is no config folder or the file cannot be written.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path().context("no home folder to keep Zendo's settings in")?;
        let mut config = self.clone();
        config.steamgriddb_key = normalise_key(config.steamgriddb_key);
        let text = format!("{HEADER}{}", toml::to_string(&config)?);
        paths::write_private(&path, text.as_bytes())
            .with_context(|| format!("could not write {}", path.display()))
    }

    fn parse(text: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        config.steamgriddb_key = normalise_key(config.steamgriddb_key);
        Ok(config)
    }
}

fn normalise_key(key: Option<String>) -> Option<String> {
    key.map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            steamgriddb_key: Some("abc".into()),
            download_covers: false,
            install_dir: Some("/mnt/games".into()),
        };
        let text = format!("{HEADER}{}", toml::to_string(&config).unwrap());
        assert_eq!(Config::parse(&text).unwrap(), config);
        let text = toml::to_string(&Config::default()).unwrap();
        assert!(!text.contains("steamgriddb_key"), "{text}");
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
