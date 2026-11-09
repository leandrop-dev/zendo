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
//! fsr = true
//! fsr_strength = 3
//! frame_cap = 60
//! arguments = "-windowed"
//! environment = "DXVK_ASYNC=1"
//! executable = "bin/x64/Cyberpunk2077.exe"
//! before = "notify-send 'Starting'"
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
use crate::steam_runtime::Runtime;

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
    /// A window without its frame (`-b`), when not full screen.
    #[serde(skip_serializing_if = "is_false")]
    pub gamescope_borderless: bool,
    /// The size the game draws at (`-w`, `-h`), which gamescope scales
    /// to its own; the same when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_game_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_game_height: Option<u32>,
    /// How the game's frames fit gamescope's (`-S`); gamescope's own
    /// choice when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_scaler: Option<Scaler>,
    /// What scales them (`-F`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_filter: Option<Filter>,
    /// The filter's sharpening, 0 (the most) to 20 (`--sharpness`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_sharpness: Option<u8>,
    /// The game's frames a second (`-r`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamescope_fps: Option<u32>,
    /// More of gamescope's options, split as a shell would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub gamescope_more: String,
    /// Proton's `PROTON_PREFER_SDL`: every controller reaches the game as
    /// an Xbox pad, for games that read nothing else (a `PlayStation`
    /// controller's touchpad, motion and triggers are then not passed on).
    #[serde(skip_serializing_if = "is_false")]
    pub xbox_controllers: bool,
    /// Proton's esync and fsync off (`PROTON_NO_ESYNC`, `PROTON_NO_FSYNC`),
    /// for games that hang or crash with them.
    #[serde(skip_serializing_if = "is_false")]
    pub no_sync: bool,
    /// Wine's Wayland driver instead of X11 (`PROTON_ENABLE_WAYLAND`).
    #[serde(skip_serializing_if = "is_false")]
    pub wayland: bool,
    /// HDR: `PROTON_ENABLE_HDR` and `DXVK_HDR` (which DXVK reads itself),
    /// and gamescope's `--hdr-enabled` under gamescope.
    #[serde(skip_serializing_if = "is_false")]
    pub hdr: bool,
    /// Wine's FSR 1 upscaling (`WINE_FULLSCREEN_FSR`), for a game run full
    /// screen below the screen's size, through the X11 driver.
    #[serde(skip_serializing_if = "is_false")]
    pub fsr: bool,
    /// FSR's sharpening, 0 (the most) to 5 (`WINE_FULLSCREEN_FSR_STRENGTH`);
    /// Wine's 2 when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fsr_strength: Option<u8>,
    /// Wine's `WoW64` (`PROTON_USE_WOW64`): 32-bit Windows code run without
    /// the system's 32-bit libraries.
    #[serde(skip_serializing_if = "is_false")]
    pub wow64: bool,
    /// DXVK's frame rate counter (`DXVK_HUD=fps`), for Direct3D 8 to 11.
    #[serde(skip_serializing_if = "is_false")]
    pub fps_counter: bool,
    /// The most frames a second DXVK and VKD3D-Proton draw
    /// (`DXVK_FRAME_RATE`, `VKD3D_FRAME_RATE`); none when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_cap: Option<u32>,
    /// Draws on the NVIDIA card of a computer with two (PRIME render
    /// offload).
    #[serde(skip_serializing_if = "is_false")]
    pub nvidia_offload: bool,
    /// Proton run by Zendo in Steam Linux Runtime 4.0, not by umu.
    #[serde(skip_serializing_if = "is_false")]
    pub no_umu: bool,
    /// The Steam Linux Runtime a native game runs in; none when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_runtime: Option<Runtime>,
    /// The game's language for an Epic game (legendary's `--language`):
    /// `fr`, `pt-BR`; the system's when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The program run instead of the store's (Epic and GOG games): a
    /// path in the game's folder, relative to it or whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// Another way to start the game its store lists, by its key
    /// ([`crate::store_tasks`]); the store's own when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// A command run through `sh -c` before the game starts; the game
    /// waits for it, and does not start when it fails.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub before: String,
    /// A command run through `sh -c` once the game has ended, when
    /// Zendo's window followed it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub after: String,
    /// Passed to the game, split as a shell would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub arguments: String,
    /// `NAME=value` pairs for the game's environment, split as a shell
    /// would.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub environment: String,
}

/// How gamescope fits the game's frames to its own (`-S`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scaler {
    Auto,
    /// Whole multiples only, for pixel art.
    Integer,
    Fit,
    Fill,
    Stretch,
}

impl Scaler {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Integer,
        Self::Fit,
        Self::Fill,
        Self::Stretch,
    ];

    /// gamescope's word for it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Integer => "integer",
            Self::Fit => "fit",
            Self::Fill => "fill",
            Self::Stretch => "stretch",
        }
    }
}

/// What gamescope scales with (`-F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    Linear,
    Nearest,
    /// AMD's FSR 1.
    Fsr,
    /// NVIDIA Image Scaling.
    Nis,
    Pixel,
    /// Snapdragon Game Super Resolution.
    Sgsr,
}

impl Filter {
    pub const ALL: [Self; 6] = [
        Self::Linear,
        Self::Nearest,
        Self::Fsr,
        Self::Nis,
        Self::Pixel,
        Self::Sgsr,
    ];

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Nearest => "nearest",
            Self::Fsr => "fsr",
            Self::Nis => "nis",
            Self::Pixel => "pixel",
            Self::Sgsr => "sgsr",
        }
    }

    /// Whether it sharpens, so `--sharpness` applies.
    #[must_use]
    pub fn sharpens(self) -> bool {
        matches!(self, Self::Fsr | Self::Nis | Self::Sgsr)
    }
}

/// gamescope's least sharpening.
pub const GAMESCOPE_SOFTEST: u8 = 20;

/// FSR's least sharpening.
pub const MOST_SOFT: u8 = 5;
/// The sharpening Wine gives FSR when none is set.
pub const WINE_SHARPENING: u8 = 2;

/// Two or three ASCII letters, as in `fr` or `BR`.
fn is_code(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphabetic())
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
        paths::locked(&path, || {
            let mut all = Self::load_all()?;
            if *self == Self::default() {
                all.remove(id);
            } else {
                all.insert(id.to_owned(), self.clone());
            }
            let text = format!("{HEADER}{}", toml::to_string(&all)?);
            paths::write_private(&path, text.as_bytes())
                .with_context(|| format!("could not write {}", path.display()))
        })
    }

    /// Whether the arguments and environment can be read.
    ///
    /// # Errors
    ///
    /// Saying which one is wrong and how.
    pub fn check(&self) -> anyhow::Result<()> {
        self.arguments()?;
        self.environment()?;
        if self
            .fsr_strength
            .is_some_and(|strength| strength > MOST_SOFT)
        {
            bail!("FSR's sharpening goes from 0 to {MOST_SOFT}");
        }
        if self.frame_cap == Some(0) {
            bail!("a frame cap of 0 would stop the game");
        }
        if self
            .gamescope_sharpness
            .is_some_and(|sharpness| sharpness > GAMESCOPE_SOFTEST)
        {
            bail!("gamescope's sharpening goes from 0 to {GAMESCOPE_SOFTEST}");
        }
        self.gamescope_more()?;
        if let Some(language) = &self.language {
            let valid = match language.split_once('-') {
                Some((code, region)) => is_code(code) && is_code(region),
                None => is_code(language),
            };
            if !valid {
                bail!("\"{language}\" is not a language code such as fr or pt-BR");
            }
        }
        Ok(())
    }

    /// What the switches set in the game's environment, before the game's
    /// own environment (which can change any of it); the Proton ones only
    /// for a game Proton runs (`windows`).
    #[must_use]
    pub fn switches(&self, windows: bool) -> Vec<(&'static str, String)> {
        let mut env: Vec<(&'static str, String)> = Vec::new();
        let on = || "1".to_owned();
        if self.nvidia_offload {
            env.push(("__NV_PRIME_RENDER_OFFLOAD", on()));
            env.push(("__VK_LAYER_NV_optimus", "NVIDIA_only".into()));
            env.push(("__GLX_VENDOR_LIBRARY_NAME", "nvidia".into()));
        }
        if !windows {
            return env;
        }
        if self.xbox_controllers {
            env.push(("PROTON_PREFER_SDL", on()));
        }
        if self.no_sync {
            env.push(("PROTON_NO_ESYNC", on()));
            env.push(("PROTON_NO_FSYNC", on()));
        }
        if self.wayland {
            env.push(("PROTON_ENABLE_WAYLAND", on()));
        }
        if self.hdr {
            env.push(("PROTON_ENABLE_HDR", on()));
            env.push(("DXVK_HDR", on()));
        }
        if self.fsr {
            env.push(("WINE_FULLSCREEN_FSR", on()));
            if let Some(strength) = self.fsr_strength {
                env.push(("WINE_FULLSCREEN_FSR_STRENGTH", strength.to_string()));
            }
        }
        if self.wow64 {
            env.push(("PROTON_USE_WOW64", on()));
        }
        if self.fps_counter {
            env.push(("DXVK_HUD", "fps".into()));
        }
        if let Some(cap) = self.frame_cap {
            env.push(("DXVK_FRAME_RATE", cap.to_string()));
            env.push(("VKD3D_FRAME_RATE", cap.to_string()));
        }
        env
    }

    /// The switches' environment and then the game's own.
    ///
    /// # Errors
    ///
    /// As [`environment`](Self::environment).
    pub fn environment_for(&self, windows: bool) -> anyhow::Result<Vec<(String, String)>> {
        let mut env: Vec<(String, String)> = self
            .switches(windows)
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect();
        env.extend(self.environment()?);
        Ok(env)
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

    /// gamescope's other options.
    ///
    /// # Errors
    ///
    /// When a quote is left open.
    pub fn gamescope_more(&self) -> anyhow::Result<Vec<String>> {
        shlex::split(&self.gamescope_more).context("gamescope's options have a quote left open")
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
            } else if self.gamescope_borderless {
                words.push("-b".into());
            }
            if self.hdr {
                words.push("--hdr-enabled".into());
            }
            if let (Some(width), Some(height)) = (self.gamescope_width, self.gamescope_height) {
                for (flag, value) in [("-W", width), ("-H", height)] {
                    words.push(flag.into());
                    words.push(value.to_string());
                }
            }
            if let (Some(width), Some(height)) =
                (self.gamescope_game_width, self.gamescope_game_height)
            {
                for (flag, value) in [("-w", width), ("-h", height)] {
                    words.push(flag.into());
                    words.push(value.to_string());
                }
            }
            if let Some(scaler) = self.gamescope_scaler {
                words.extend(["-S".into(), scaler.word().into()]);
            }
            if let Some(filter) = self.gamescope_filter {
                words.extend(["-F".into(), filter.word().into()]);
                if let Some(sharpness) = self.gamescope_sharpness.filter(|_| filter.sharpens()) {
                    words.extend(["--sharpness".into(), sharpness.to_string()]);
                }
            }
            if let Some(fps) = self.gamescope_fps {
                words.extend(["-r".into(), fps.to_string()]);
            }
            // Checked when saved; a file changed by hand since gives none.
            words.extend(self.gamescope_more().unwrap_or_default());
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
    fn sets_the_switches_for_proton_and_leaves_the_rest_to_the_game() {
        let options = LaunchOptions {
            xbox_controllers: true,
            no_sync: true,
            wayland: true,
            fsr: true,
            fsr_strength: Some(4),
            frame_cap: Some(60),
            nvidia_offload: true,
            environment: "DXVK_FRAME_RATE=30".into(),
            ..LaunchOptions::default()
        };
        let names = |env: Vec<(&'static str, String)>| {
            env.into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(options.switches(true)),
            [
                "__NV_PRIME_RENDER_OFFLOAD=1",
                "__VK_LAYER_NV_optimus=NVIDIA_only",
                "__GLX_VENDOR_LIBRARY_NAME=nvidia",
                "PROTON_PREFER_SDL=1",
                "PROTON_NO_ESYNC=1",
                "PROTON_NO_FSYNC=1",
                "PROTON_ENABLE_WAYLAND=1",
                "WINE_FULLSCREEN_FSR=1",
                "WINE_FULLSCREEN_FSR_STRENGTH=4",
                "DXVK_FRAME_RATE=60",
                "VKD3D_FRAME_RATE=60",
            ]
        );
        // A native game: only what is not Proton's.
        assert_eq!(names(options.switches(false)).len(), 3);
        // The game's own environment comes last, so it wins.
        let env = options.environment_for(true).unwrap();
        assert_eq!(
            env.last(),
            Some(&("DXVK_FRAME_RATE".to_owned(), "30".to_owned()))
        );
        for bad in [
            LaunchOptions {
                fsr_strength: Some(6),
                ..LaunchOptions::default()
            },
            LaunchOptions {
                frame_cap: Some(0),
                ..LaunchOptions::default()
            },
        ] {
            assert!(bad.check().is_err(), "{bad:?}");
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
            gamescope_borderless: true,
            gamescope_game_width: Some(1280),
            gamescope_game_height: Some(720),
            gamescope_scaler: Some(Scaler::Fit),
            gamescope_filter: Some(Filter::Fsr),
            gamescope_sharpness: Some(5),
            gamescope_fps: Some(60),
            gamescope_more: "--adaptive-sync".into(),
            hdr: true,
            ..LaunchOptions::default()
        };
        assert_eq!(
            options.wrappers(),
            [
                "gamemoderun",
                "gamescope",
                "-f",
                "--hdr-enabled",
                "-W",
                "1920",
                "-H",
                "1080",
                "-w",
                "1280",
                "-h",
                "720",
                "-S",
                "fit",
                "-F",
                "fsr",
                "--sharpness",
                "5",
                "-r",
                "60",
                "--adaptive-sync",
                "--mangoapp",
                "--"
            ]
        );
        // Borderless only when not full screen; no sharpening for a
        // filter without it.
        let window = LaunchOptions {
            gamescope: true,
            gamescope_borderless: true,
            gamescope_filter: Some(Filter::Nearest),
            gamescope_sharpness: Some(5),
            ..LaunchOptions::default()
        };
        assert_eq!(
            window.wrappers(),
            ["gamescope", "-b", "-F", "nearest", "--"]
        );
        for (language, valid) in [
            ("fr", true),
            ("pt-BR", true),
            ("french", false),
            ("p", false),
        ] {
            let options = LaunchOptions {
                language: Some(language.into()),
                ..LaunchOptions::default()
            };
            assert_eq!(options.check().is_ok(), valid, "{language}");
        }
        let alone = LaunchOptions {
            mangohud: true,
            ..LaunchOptions::default()
        };
        assert_eq!(alone.wrappers(), ["mangohud"]);
        assert_eq!(alone.tools(), [("mangohud", "MangoHud")]);
    }
}
