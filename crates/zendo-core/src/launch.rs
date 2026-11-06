//! Starting a game and letting it run on its own.

use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::{Game, Launch, State};

/// Starts the game without waiting for it, and notes the launch in Zendo's
/// state. A game Zendo runs itself (a `.desktop` one) is also timed until it
/// exits, as long as this process lives; for the others `xdg-open` hands
/// the game to its launcher at once, and the launcher keeps the time.
///
/// # Errors
///
/// When the program, or `xdg-open` for a link, cannot be started. Failing
/// to note the launch is not an error: the game is already running.
pub fn launch(game: &Game) -> anyhow::Result<()> {
    let (mut command, timed) = match &game.launch {
        Launch::Uri { uri } => {
            let mut command = Command::new("xdg-open");
            command.arg(uri);
            (command, false)
        }
        Launch::Command { program, args, dir } => {
            let mut command = Command::new(program);
            command.args(args);
            if let Some(dir) = dir.as_ref().filter(|dir| dir.is_dir()) {
                command.current_dir(dir);
            }
            (command, true)
        }
        Launch::Engine { .. } => bail!(
            "Zendo cannot start the games it installs yet; that comes with launch options, \
             the next step"
        ),
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("could not start {}", game.title))?;
    let _ = State::update(|state| state.launched(&game.id));
    let id = game.id.clone();
    let started = Instant::now();
    // Reap it when it ends, so it does not linger as a zombie.
    thread::spawn(move || {
        let _ = child.wait();
        let minutes = started.elapsed().as_secs() / 60;
        if timed && minutes > 0 {
            let _ = State::update(|state| state.add_minutes(&id, minutes));
        }
    });
    Ok(())
}
