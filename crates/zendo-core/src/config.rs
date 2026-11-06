//! What the user sets, in `$XDG_CONFIG_HOME/zendo/config.toml`:
//!
//! ```toml
//! # A key from https://www.steamgriddb.com/profile/preferences, for covers
//! # of games whose store has none.
//! steamgriddb_key = "…"
//! # Set to false to never download covers.
//! download_covers = true
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

use crate::paths;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub steamgriddb_key: Option<String>,
    pub download_covers: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            steamgriddb_key: None,
            download_covers: true,
        }
    }
}

impl Config {
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::config_home().map(|dir| dir.join("zendo/config.toml"))
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

    fn parse(text: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(text)?;
        config.steamgriddb_key = config
            .steamgriddb_key
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty());
        Ok(config)
    }
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
}
