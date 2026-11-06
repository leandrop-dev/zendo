//! Games added by hand ("Add game"): a program the user installed
//! themselves, a Windows `.exe` (run through umu and Proton, in a prefix of
//! its own) or a Linux program. Kept in `$XDG_CONFIG_HOME/zendo/local.toml`:
//!
//! ```toml
//! [[game]]
//! id = "local:cave-story"
//! title = "Cave Story"
//! program = "/home/me/Games/Cave Story/Doukutsu.exe"
//! arguments = "-fullscreen"
//! ```
//!
//! The id comes from the title it was added with and stays when the title
//! is changed, so play time, pins and the prefix stay with the game.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::{Details, Game, Launch, Scan, Source, paths};

const HEADER: &str = "\
# Games added by hand in Zendo, which rewrites this file.

";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalGame {
    pub id: String,
    pub title: String,
    pub program: PathBuf,
    /// Passed to the program, split as a shell would.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arguments: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default, rename = "game")]
    games: Vec<LocalGame>,
}

impl LocalGame {
    /// A Windows program, run through Proton.
    #[must_use]
    pub fn is_windows(&self) -> bool {
        is_windows(&self.program)
    }

    /// Whether it can be added as it is.
    ///
    /// # Errors
    ///
    /// Saying what is wrong: no title, no such file, a Linux file that is
    /// not a program, arguments with a quote left open.
    pub fn check(&self) -> anyhow::Result<()> {
        if self.title.trim().is_empty() {
            bail!("Give the game a title");
        }
        if !self.program.is_absolute() {
            bail!("Choose the game's program, by its full path");
        }
        let meta = fs::metadata(&self.program)
            .with_context(|| format!("{} is not there", self.program.display()))?;
        if !meta.is_file() {
            bail!("{} is not a file", self.program.display());
        }
        if !self.is_windows() && meta.permissions().mode() & 0o111 == 0 {
            bail!(
                "{} is not a program: a Windows game is an .exe, a Linux one must be executable",
                self.program.display()
            );
        }
        shlex::split(&self.arguments).context("the arguments have a quote left open")?;
        Ok(())
    }

    fn game(&self) -> Game {
        let arguments = shlex::split(&self.arguments).unwrap_or_default();
        let dir = self.program.parent().map(Path::to_path_buf);
        let mut game = Game::new(
            self.id.clone(),
            self.title.clone(),
            Source::Local,
            Launch::Local {
                program: self.program.clone(),
                args: arguments,
                dir: dir.clone(),
            },
        );
        game.details = Details {
            platform: Some(
                if self.is_windows() {
                    "windows"
                } else {
                    "linux"
                }
                .to_owned(),
            ),
            install_path: dir,
            ..Details::default()
        };
        game
    }
}

/// A Windows program: `.exe`, `.bat` or `.msi`, case aside.
#[must_use]
pub fn is_windows(program: &Path) -> bool {
    program
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ["exe", "bat", "msi"].contains(&ext.to_lowercase().as_str()))
}

fn path() -> Option<PathBuf> {
    paths::config_home().map(|dir| dir.join("zendo/local.toml"))
}

/// The games added by hand.
///
/// # Errors
///
/// When the file exists but cannot be read or is not valid.
pub fn load() -> anyhow::Result<Vec<LocalGame>> {
    let Some(path) = path() else {
        return Ok(Vec::new());
    };
    match fs::read_to_string(&path) {
        Ok(text) => Ok(toml::from_str::<File>(&text)
            .with_context(|| format!("{}", path.display()))?
            .games),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err).with_context(|| format!("could not read {}", path.display())),
    }
}

fn save(games: Vec<LocalGame>) -> anyhow::Result<()> {
    let path = path().context("no home folder to keep the games added by hand in")?;
    let text = format!("{HEADER}{}", toml::to_string(&File { games })?);
    paths::write_atomic(&path, text.as_bytes())
        .with_context(|| format!("could not write {}", path.display()))
}

/// An id for a new game, from its title, unlike any taken.
fn new_id(title: &str, taken: &[LocalGame]) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let base = format!("local:{}", if slug.is_empty() { "game" } else { slug });
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|game| game.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Adds a game, or changes the one with the same id; returns its id. A new
/// game's `id` may be left empty: one is made from its title.
///
/// # Errors
///
/// When it is not valid ([`LocalGame::check`]) or the file cannot be read
/// or written.
pub fn put(mut game: LocalGame) -> anyhow::Result<String> {
    game.title = game.title.trim().to_owned();
    game.check()?;
    let mut games = load()?;
    if let Some(kept) = games.iter_mut().find(|kept| kept.id == game.id) {
        *kept = game.clone();
    } else {
        game.id = new_id(&game.title, &games);
        games.push(game.clone());
    }
    save(games)?;
    Ok(game.id)
}

/// Takes a game out of the library; its files and prefix stay.
///
/// # Errors
///
/// When the file cannot be read or written.
pub fn remove(id: &str) -> anyhow::Result<()> {
    let mut games = load()?;
    games.retain(|game| game.id != id);
    save(games)
}

/// One game's record, for editing it.
///
/// # Errors
///
/// As [`load`].
pub fn find(id: &str) -> anyhow::Result<Option<LocalGame>> {
    Ok(load()?.into_iter().find(|game| game.id == id))
}

pub(crate) fn scan() -> Scan {
    match load() {
        Ok(games) => Scan {
            games: games.iter().map(LocalGame::game).collect(),
            problems: Vec::new(),
        },
        Err(err) => Scan {
            games: Vec::new(),
            problems: vec![format!("{err:#}")],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: &str) -> LocalGame {
        LocalGame {
            id: id.into(),
            title: "Cave Story".into(),
            program: "/games/Cave Story/Doukutsu.EXE".into(),
            arguments: String::new(),
        }
    }

    #[test]
    fn makes_an_id_from_the_title_unlike_any_taken() {
        assert_eq!(new_id("Cave Story+", &[]), "local:cave-story");
        assert_eq!(new_id("  ", &[]), "local:game");
        assert_eq!(
            new_id("Cave Story", &[game("local:cave-story")]),
            "local:cave-story-2"
        );
    }

    #[test]
    fn tells_windows_programs_by_their_extension() {
        assert!(game("x").is_windows());
        assert!(!is_windows(Path::new("/games/celeste/Celeste")));
        let local = game("local:cave-story").game();
        assert_eq!(local.details.platform.as_deref(), Some("windows"));
        assert_eq!(
            local.details.install_path.as_deref(),
            Some(Path::new("/games/Cave Story"))
        );
    }

    #[test]
    fn checks_before_adding() {
        let dir = std::env::temp_dir().join(format!("zendo-local-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("run.sh");
        fs::write(&script, "#!/bin/sh\n").unwrap();
        let mut local = LocalGame {
            id: String::new(),
            title: "Run".into(),
            program: script.clone(),
            arguments: String::new(),
        };
        assert!(local.check().is_err(), "not executable");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        local.check().unwrap();
        local.arguments = "\"open".into();
        assert!(local.check().is_err());
        local.arguments = String::new();
        local.title = " ".into();
        assert!(local.check().is_err());
        local.title = "Run".into();
        local.program = dir.join("missing.exe");
        assert!(local.check().is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writes_what_it_reads() {
        let file = File {
            games: vec![game("local:cave-story")],
        };
        let text = toml::to_string(&file).unwrap();
        assert!(text.contains("[[game]]"), "{text}");
        let back: File = toml::from_str(&text).unwrap();
        assert_eq!(back.games, file.games);
    }
}
