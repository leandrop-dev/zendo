//! `zendo`: the library window, and commands to list and launch games from
//! a terminal or a keybind.

mod app;
mod gamepad;

use std::process::ExitCode;

use zendo_core::Library;

const HELP: &str = "\
zendo: every game in one library

Usage:
  zendo                  open the library
  zendo --fullscreen     open it full-screen (F11 toggles)
  zendo list [--json]    print the games found
  zendo launch <id>      start a game by the id `list` prints
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => app::run(false),
        ["--fullscreen"] => app::run(true),
        ["list"] => list(false),
        ["list", "--json"] => list(true),
        ["launch", id] => launch(id),
        ["-h" | "--help"] => {
            print!("{HELP}");
            Ok(())
        }
        ["-V" | "--version"] => {
            println!("zendo {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            eprint!("{HELP}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("zendo: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn list(json: bool) -> anyhow::Result<()> {
    let library = Library::scan();
    if json {
        println!("{}", serde_json::to_string_pretty(&library)?);
        return Ok(());
    }
    for game in &library.games {
        println!("{}\t{}\t{}", game.id, game.source.label(), game.title);
    }
    for problem in &library.problems {
        eprintln!("zendo: could not read {problem}");
    }
    Ok(())
}

fn launch(id: &str) -> anyhow::Result<()> {
    let library = Library::scan();
    let game = library
        .find(id)
        .ok_or_else(|| anyhow::anyhow!("no game with the id {id:?}; `zendo list` shows them"))?;
    zendo_core::launch(game)
}
