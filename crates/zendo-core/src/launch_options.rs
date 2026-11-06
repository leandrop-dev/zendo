//! How each game Zendo starts itself should run, set on its page's launch
//! options sheet and kept in `$XDG_CONFIG_HOME/zendo/games.toml`, a table
//! per game:
//!
//! ```toml
//! ["gog:1299228602"]
//! proton = "GE-Proton10-4"
//! mangohud = true
//! gamescope = true
//! gamescope_fullscreen = true
//! xbox_controllers = true
//! arguments = "-windowed"
//! environment = "DXVK_ASYNC=1"
//! ```
//!
//! Games left as they come are not written. Steam and Heroic games keep
//! their launcher's own settings.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

const HEADER: &str = "\
# Each game's launch options, as set on its page in Zendo, which rewrites
# this file.

";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[expect(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct LaunchOptions {
    /// The Proton build's folder name; the default one when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proton: Option<String>,
    /// `MangoHud`'s overlay: frame rate, frame times, temperatures.
    #[serde(skip_serializing_if = "is_false")]
    pub mangohud: bool,
    /// Feral's `GameMode`, which asks the system for performance while the
    /// game runs.
    #[serde(skip_serializing_if = "is_false")]
    pub gamemode: bool,
    /// Valve's gamescope, a compositor of the game's own.
    #[serde(skip_serializing_if = "is_false")]
    pub gamescope: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub gamescope_fullscreen: bool,
    /// The size gamescope shows the game at; the game's own when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_height: Option<u32>,
    /// Proton's `PROTON_PREFER_SDL`: every controller reaches the game as
    /// an Xbox pad, for games that read nothing else (a `PlayStation`
    /// controller's touchpad, motion and triggers are then not passed on).
    #[serde(skip_serializing_if = "is_false")]
    pub xbox_controllers: bool,
    /// Passed to the game, split as a shell would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub arguments: String,
    /// `NAME=value` pairs for the game's environment, split as a shell
    /// would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub environment: String,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
fn is_false(value: &bool) -> bool {
    !value
}

impl LaunchOptions {
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        paths::config_home().map(|dir| dir.join("zendo/games.toml"))
    }

    /// Every game's options, by id.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or is not valid.
    pub fn load_all() -> anyhow::Result<BTreeMap<String, Self>> {
        let Some(path) = Self::path() else {
            return Ok(BTreeMap::new());
        };
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("{}", path.display())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
        }
    }

    /// One game's options, the defaults when it has none.
    ///
    /// # Errors
    ///
    /// As [`load_all`](Self::load_all).
    pub fn load(id: &str) -> anyhow::Result<Self> {
        Ok(Self::load_all()?.remove(id).unwrap_or_default())
    }

    /// Keeps one game's options, the others as they were. The file can
    /// hold tokens in an environment, so it is readable by the user only.
    ///
    /// # Errors
    ///
    /// When the options are not valid ([`check`](Self::check)), or the file
    /// cannot be read or written.
    pub fn save(&self, id: &str) -> anyhow::Result<()> {
        self.check()?;
        let path = Self::path().context("no home folder to keep the launch options in")?;
        let mut all = Self::load_all()?;
        if *self == Self::default() {
            all.remove(id);
        } else {
            all.insert(id.to_owned(), self.clone());
        }
        let text = format!("{HEADER}{}", toml::to_string(&all)?);
        paths::write_private(&path, text.as_bytes())
            .with_context(|| format!("could not write {}", path.display()))
    }

    /// Whether the arguments and environment can be read.
    ///
    /// # Errors
    ///
    /// Saying which one is wrong and how.
    pub fn check(&self) -> anyhow::Result<()> {
        self.arguments()?;
        self.environment()?;
        Ok(())
    }

    /// The arguments for the game.
    ///
    /// # Errors
    ///
    /// When a quote is left open.
    pub fn arguments(&self) -> anyhow::Result<Vec<String>> {
        shlex::split(&self.arguments).context("the arguments have a quote left open")
    }

    /// The environment for the game, as name and value.
    ///
    /// # Errors
    ///
    /// When a quote is left open or a word is not `NAME=value`.
    pub fn environment(&self) -> anyhow::Result<Vec<(String, String)>> {
        let words =
            shlex::split(&self.environment).context("the environment has a quote left open")?;
        words
            .into_iter()
            .map(|word| {
                let Some((name, value)) = word.split_once('=') else {
                    bail!("\"{word}\" in the environment is not NAME=value");
                };
                let valid = !name.is_empty()
                    && !name.starts_with(|c: char| c.is_ascii_digit())
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if !valid {
                    bail!("\"{name}\" is not a name for the environment");
                }
                Ok((name.to_owned(), value.to_owned()))
            })
            .collect()
    }

    /// The wrappers the game runs inside, outermost first, before umu-run
    /// for a Windows game: `gamemoderun`, then gamescope (with
    /// `--mangoapp` for the overlay, as `MangoHud` does not draw inside
    /// it), then `mangohud`.
    #[must_use]
    pub fn wrappers(&self) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        if self.gamemode {
            words.push("gamemoderun".into());
        }
        if self.gamescope {
            words.push("gamescope".into());
            if self.gamescope_fullscreen {
                words.push("-f".into());
            }
            if let (Some(width), Some(height)) = (self.gamescope_width, self.gamescope_height) {
                for (flag, value) in [("-W", width), ("-H", height)] {
                    words.push(flag.into());
                    words.push(value.to_string());
                }
            }
            if self.mangohud {
                words.push("--mangoapp".into());
            }
            words.push("--".into());
        } else if self.mangohud {
            words.push("mangohud".into());
        }
        words
    }

    /// The programs [`wrappers`](Self::wrappers) needs, with the names
    /// people know them by.
    #[must_use]
    pub fn tools(&self) -> Vec<(&'static str, &'static str)> {
        let mut tools = Vec::new();
        if self.gamemode {
            tools.push(("gamemoderun", "GameMode"));
        }
        if self.gamescope {
            tools.push(("gamescope", "gamescope"));
        }
        if self.mangohud {
            tools.push(if self.gamescope {
                ("mangoapp", "MangoHud")
            } else {
                ("mangohud", "MangoHud")
            });
        }
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_out_what_is_left_as_it_comes() {
        let options = LaunchOptions {
            mangohud: true,
            ..LaunchOptions::default()
        };
        let text = toml::to_string(&options).unwrap();
        assert_eq!(text.trim(), "mangohud = true");
        let mut all = BTreeMap::new();
        all.insert("gog:1".to_owned(), options.clone());
        let text = toml::to_string(&all).unwrap();
        let back: BTreeMap<String, LaunchOptions> = toml::from_str(&text).unwrap();
        assert_eq!(back["gog:1"], options);
    }

    #[test]
    fn splits_arguments_and_environment_as_a_shell_would() {
        let options = LaunchOptions {
            arguments: "-windowed --name \"Lea P\"".into(),
            environment: "DXVK_HUD=fps WINEDLLOVERRIDES='d3d9=n,b'".into(),
            ..LaunchOptions::default()
        };
        assert_eq!(
            options.arguments().unwrap(),
            ["-windowed", "--name", "Lea P"]
        );
        assert_eq!(
            options.environment().unwrap(),
            [
                ("DXVK_HUD".to_owned(), "fps".to_owned()),
                ("WINEDLLOVERRIDES".to_owned(), "d3d9=n,b".to_owned()),
            ]
        );
        let open = LaunchOptions {
            arguments: "\"open".into(),
            ..LaunchOptions::default()
        };
        assert!(open.check().is_err());
        for bad in ["DXVK_HUD", "=fps", "1A=b", "A-B=c"] {
            let options = LaunchOptions {
                environment: bad.into(),
                ..LaunchOptions::default()
            };
            assert!(options.check().is_err(), "{bad}");
        }
    }

    #[test]
    fn puts_mangohud_inside_gamescope() {
        let options = LaunchOptions {
            mangohud: true,
            gamemode: true,
            gamescope: true,
            gamescope_fullscreen: true,
            gamescope_width: Some(1920),
            gamescope_height: Some(1080),
            ..LaunchOptions::default()
        };
        assert_eq!(
            options.wrappers(),
            [
                "gamemoderun",
                "gamescope",
                "-f",
                "-W",
                "1920",
                "-H",
                "1080",
                "--mangoapp",
                "--"
            ]
        );
        let alone = LaunchOptions {
            mangohud: true,
            ..LaunchOptions::default()
        };
        assert_eq!(alone.wrappers(), ["mangohud"]);
        assert_eq!(alone.tools(), [("mangohud", "MangoHud")]);
    }
}
