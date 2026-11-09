//! The tools for the Wine prefix of a Windows game Zendo starts, as Heroic
//! has them: Wine's settings (winecfg), winetricks' own window, winetricks
//! components installed without questions, and a program run in the
//! prefix (an installer, a patch). Each runs through umu-run with the
//! game's Proton, prefix and environment ([`launch::prefix_command`]),
//! and logs to `$XDG_STATE_HOME/zendo/logs/<id>.prefix.log`.
//!
//! What umu does with each (umu-launcher 1.4.3's `umu_run.py`):
//! - a name that is no file (`winecfg`, `msiexec`) goes to Wine as it is,
//!   which finds it in the prefix;
//! - `winetricks <verbs>` runs the winetricks inside Proton
//!   (`protonfixes/winetricks`) with `-q`; umu runs none of the verbs when
//!   one is in the prefix's `winetricks.log` already ("winetricks verb
//!   '…' is already installed"), so Zendo gives it one at a time;
//! - winetricks' window is protonfixes' fix for the id `winetricks-gui`,
//!   run with no program (as Heroic does): GE-Proton and UMU-Proton have
//!   it, Valve's Proton has no protonfixes.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use crate::{Game, Launch, Source, launch};

/// What to run in the prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tool {
    /// Wine's settings.
    WineSettings,
    /// winetricks' own window.
    Winetricks,
    /// Winetricks verbs, installed without questions.
    Components(Vec<String>),
    /// A Windows program: an `.exe`, or an `.msi` through `msiexec`.
    Program(PathBuf),
}

/// How a run went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ran {
    /// The components installed.
    pub installed: Vec<String>,
    /// The components that were in the prefix already, so not installed
    /// again.
    pub already: Vec<String>,
}

/// Whether the game has a prefix of Zendo's: a Windows game it installed
/// or that was added by hand.
#[must_use]
pub fn applies(game: &Game) -> bool {
    let starts_itself = matches!(game.launch, Launch::Engine { .. })
        || (game.source == Source::Local && matches!(game.launch, Launch::Local { .. }));
    starts_itself && launch::needs_proton(game)
}

/// The components typed, split at spaces and commas.
///
/// # Errors
///
/// When none is typed, or one is not a winetricks verb (umu's pattern:
/// letters, digits, `_` and `-`, with `=value` for a setting).
pub fn components(typed: &str) -> anyhow::Result<Vec<String>> {
    let verbs: Vec<String> = typed
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|verb| !verb.is_empty())
        .map(str::to_owned)
        .collect();
    if verbs.is_empty() {
        bail!("type the winetricks components to install, such as vcrun2022");
    }
    if let Some(bad) = verbs.iter().find(|verb| !is_verb(verb)) {
        bail!("{bad} is not a winetricks component");
    }
    Ok(verbs)
}

fn is_verb(verb: &str) -> bool {
    let (name, value) = verb.split_once('=').unwrap_or((verb, ""));
    // A leading dash would be one of winetricks' options.
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Runs the tool in the game's prefix and waits for it to end.
///
/// # Errors
///
/// When it cannot start, or ends with an error (its log says more).
pub fn run(game: &Game, tool: &Tool) -> anyhow::Result<Ran> {
    if !applies(game) {
        bail!("Zendo keeps no Wine prefix for {}", game.title);
    }
    let log = log_path(&game.id).context("no folder for the log")?;
    match tool {
        Tool::Components(verbs) => {
            let mut ran = Ran::default();
            for verb in verbs {
                let mut command = launch::prefix_command(game, Some("umu-0"))?;
                command.args(["winetricks", verb]);
                match wait(command, &log) {
                    Ok(()) => ran.installed.push(verb.clone()),
                    // umu's own refusal, as the verb is there already.
                    Err(_) if already_installed(&log) => ran.already.push(verb.clone()),
                    Err(err) => return Err(err.context(format!("winetricks {verb}"))),
                }
            }
            Ok(ran)
        }
        Tool::WineSettings => {
            let mut command = launch::prefix_command(game, None)?;
            command.arg("winecfg");
            wait(command, &log).context("Wine's settings")?;
            Ok(Ran::default())
        }
        Tool::Winetricks => {
            let mut command = launch::prefix_command(game, Some("winetricks-gui"))?;
            // Without PROTONPATH umu brings UMU-Proton, which has it.
            if let Some(proton) = proton_path(&command)
                && !proton
                    .join("protonfixes/gamefixes-umu/winetricks-gui.py")
                    .is_file()
            {
                bail!(
                    "winetricks' window comes with GE-Proton and UMU-Proton, not {}: choose one \
                     in the game's launch options",
                    proton.file_name().unwrap_or_default().to_string_lossy()
                );
            }
            command.arg("");
            wait(command, &log).context("winetricks")?;
            Ok(Ran::default())
        }
        Tool::Program(program) => {
            let mut command = launch::prefix_command(game, None)?;
            command.args(program_words(program));
            // Installers look for their files beside them.
            if let Some(dir) = program.parent().filter(|dir| dir.is_dir()) {
                command.current_dir(dir);
            }
            let name = program
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            wait(command, &log).with_context(|| name)?;
            Ok(Ran::default())
        }
    }
}

/// Where the prefix's tools last logged, for the game with id `id`.
#[must_use]
pub fn log_path(id: &str) -> Option<PathBuf> {
    launch::log_path(id).map(|log| log.with_extension("prefix.log"))
}

/// The Proton the command gives umu, when it names one.
fn proton_path(command: &Command) -> Option<PathBuf> {
    command
        .get_envs()
        .find(|(key, _)| *key == "PROTONPATH")
        .and_then(|(_, value)| value)
        .map(PathBuf::from)
}

/// What umu is given for a program: an `.msi` goes through `msiexec`.
fn program_words(program: &Path) -> Vec<String> {
    let path = program.to_string_lossy().into_owned();
    let msi = program
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"));
    if msi {
        vec!["msiexec".into(), "/i".into(), path]
    } else {
        vec![path]
    }
}

/// Runs the command into the log and waits for it.
fn wait(mut command: Command, log: &Path) -> anyhow::Result<()> {
    let (stdout, stderr) = launch::log_to(log, &command)?;
    let status = command
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .context("could not start umu-run")?;
    if !status.success() {
        bail!("it ended with {status}; see {}", log.display());
    }
    Ok(())
}

/// Whether umu refused a verb in the log as one installed already.
fn already_installed(log: &Path) -> bool {
    std::fs::read_to_string(log).is_ok_and(|text| refused_as_installed(&text))
}

fn refused_as_installed(text: &str) -> bool {
    text.lines()
        .any(|line| line.contains("winetricks verb '") && line.contains("' is already installed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_winetricks_verbs_only() {
        assert_eq!(
            components("vcrun2022, d3dx9  fontsmooth=rgb").unwrap(),
            ["vcrun2022", "d3dx9", "fontsmooth=rgb"]
        );
        assert!(components("  ").is_err());
        assert!(components("vcrun2022 -q").is_err(), "an option, not a verb");
        assert!(components("d3dcompiler_47 dotnet-4.8").is_err());
        assert!(components("d3dcompiler_47 win-10").is_ok());
        assert!(components("../evil.verb").is_err());
        assert!(components("a=b c").is_ok());
        assert!(components("=b").is_err());
        assert!(components("a=b/c").is_err());
    }

    #[test]
    fn runs_an_msi_through_msiexec() {
        assert_eq!(
            program_words(Path::new("/dl/Setup.MSI")),
            ["msiexec", "/i", "/dl/Setup.MSI"]
        );
        assert_eq!(
            program_words(Path::new("/dl/patch 1.2.exe")),
            ["/dl/patch 1.2.exe"]
        );
    }

    #[test]
    fn hears_umus_refusal_of_an_installed_verb() {
        // As umu-launcher 1.4.3 logs it.
        let log = "ERROR: winetricks verb 'vcrun2022' is already installed in '/home/me/Games/Prefixes/Sugar'";
        assert!(refused_as_installed(log));
        assert!(!refused_as_installed("wine: could not load kernel32.dll"));
    }

    #[test]
    fn only_windows_games_zendo_starts_have_a_prefix() {
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        assert!(applies(&game));
        game.details.platform = Some("linux".into());
        assert!(!applies(&game));
        let steam = Game::new(
            "steam:620".into(),
            "Portal 2".into(),
            Source::Steam,
            Launch::Uri {
                uri: "steam://rungameid/620".into(),
            },
        );
        assert!(!applies(&steam), "Steam keeps its own");
        assert_eq!(
            log_path("gog:1").map(|log| log.file_name().unwrap().to_owned()),
            Some("gog_1.prefix.log".into())
        );
    }

    /// Through the real umu and Proton, in a prefix of the games folder that
    /// `XDG_CONFIG_HOME`'s `config.toml` names: point both at a folder to
    /// throw away. `win10` downloads nothing.
    #[test]
    #[ignore = "runs umu and Proton; needs ZENDO_PREFIX_TEST and a throwaway XDG_CONFIG_HOME"]
    fn installs_through_umu_and_hears_its_refusal() {
        if std::env::var_os("ZENDO_PREFIX_TEST").is_none() {
            return;
        }
        let game = Game::new(
            "local:prefix-test".into(),
            "Prefix test".into(),
            Source::Local,
            Launch::Local {
                program: "/nowhere/game.exe".into(),
                args: Vec::new(),
                dir: None,
            },
        );
        let ran = run(&game, &Tool::Components(vec!["win10".into()])).unwrap();
        assert_eq!(ran.installed, ["win10"]);
        let prefix = launch::prefix(&game).unwrap();
        let log = prefix.join("winetricks.log");
        let logged = std::fs::read_to_string(&log).unwrap();
        assert!(logged.lines().any(|line| line == "win10"), "{logged}");
        std::fs::write(&log, format!("{logged}vcrun2022\n")).unwrap();
        let ran = run(&game, &Tool::Components(vec!["vcrun2022".into()])).unwrap();
        assert_eq!(ran.already, ["vcrun2022"]);
        assert_eq!(ran.installed, Vec::<String>::new());
    }
}
