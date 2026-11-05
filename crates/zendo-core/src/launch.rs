//! Starting a game and letting it run on its own.

use std::process::{Command, Stdio};
use std::thread;

use anyhow::Context;

use crate::{Game, Launch};

/// Starts the game without waiting for it. Its output goes nowhere; errors
/// starting it (a missing program) are returned.
///
/// # Errors
///
/// When the program, or `xdg-open` for a link, cannot be started.
pub fn launch(game: &Game) -> anyhow::Result<()> {
    let mut command = match &game.launch {
        Launch::Uri { uri } => {
            let mut command = Command::new("xdg-open");
            command.arg(uri);
            command
        }
        Launch::Command { program, args, dir } => {
            let mut command = Command::new(program);
            command.args(args);
            if let Some(dir) = dir.as_ref().filter(|dir| dir.is_dir()) {
                command.current_dir(dir);
            }
            command
        }
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("could not start {}", game.title))?;
    // Reap it when it ends, so it does not linger as a zombie.
    thread::spawn(move || child.wait());
    Ok(())
}
