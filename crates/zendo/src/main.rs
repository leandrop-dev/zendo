//! `zendo`: the library window, and commands to list and launch games from
//! a terminal or a keybind.

mod app;
mod file_dialog;
mod gamepad;
mod links;
mod sign_in_window;

use std::process::ExitCode;

use anyhow::Context;
use zendo_core::config::Config;
use zendo_core::covers::{Fetcher, Kind};
use zendo_core::{Game, Library, State};

const HELP: &str = "\
zendo: every game in one library

Usage:
  zendo                        open the library
  zendo --fullscreen           open it full-screen (F11 toggles)
  zendo list [--all] [--json]  print the games, in the window's order;
                               --all adds the hidden ones
  zendo launch <id>            start a game by the id `list` prints
  zendo hide <id>              leave a game out of the window
  zendo unhide <id>            bring it back
  zendo pin <id>               put a game first in every order
  zendo unpin <id>             stop
  zendo covers                 download the covers and wide images missing
  zendo nxm://…                download a mod from a Nexus Mods link, in the
                               open window or a new one
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // Started by the window for a store's sign-in; not for people.
    if let ["sign-in-window", store, url] = args.as_slice() {
        return sign_in_window::main(store, url);
    }
    // Started by the window to choose a file.
    if let ["pick-file", kind, title] = args.as_slice()
        && let Some(kind) = file_dialog::Kind::from_name(kind)
    {
        return file_dialog::main(kind, title);
    }
    let result = match args.as_slice() {
        [] => app::run(false, None),
        ["--fullscreen"] => app::run(true, None),
        [link]
            if link
                .get(..6)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("nxm://")) =>
        {
            open_link(link)
        }
        ["list", flags @ ..] if flags.iter().all(|flag| ["--all", "--json"].contains(flag)) => {
            list(flags.contains(&"--all"), flags.contains(&"--json"))
        }
        ["launch", id] => find(id).and_then(|game| zendo_core::launch(&game)),
        ["hide", id] => set_hidden(id, true),
        ["unhide", id] => set_hidden(id, false),
        ["pin", id] => set_pinned(id, true),
        ["unpin", id] => set_pinned(id, false),
        ["covers"] => covers(),
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

/// Hands a Nexus link to the open window, or opens one for it.
fn open_link(link: &str) -> anyhow::Result<()> {
    zendo_core::nexus::Link::parse(link)?;
    if links::send(link) {
        return Ok(());
    }
    app::run(false, Some(link.to_owned()))
}

fn list(all: bool, json: bool) -> anyhow::Result<()> {
    let library = Library::scan();
    let sort = State::load().map(|state| state.sort).unwrap_or_default();
    let mut games: Vec<&Game> = library
        .games
        .iter()
        .filter(|game| all || !game.hidden)
        .collect();
    games.sort_by(|a, b| sort.compare(a, b));
    if json {
        let json = serde_json::json!({ "games": games, "problems": library.problems });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }
    for game in games {
        println!("{}\t{}\t{}", game.id, game.source.label(), game.title);
    }
    for problem in &library.problems {
        eprintln!("zendo: could not read {problem}");
    }
    Ok(())
}

fn find(id: &str) -> anyhow::Result<Game> {
    Library::scan()
        .find(id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no game with the id {id:?}; `zendo list --all` shows them"))
}

fn set_hidden(id: &str, hidden: bool) -> anyhow::Result<()> {
    let game = find(id)?;
    State::update(|state| state.set_hidden(&game.id, hidden))?;
    Ok(())
}

fn set_pinned(id: &str, pinned: bool) -> anyhow::Result<()> {
    let game = find(id)?;
    State::update(|state| state.set_pinned(&game.id, pinned))?;
    Ok(())
}

fn covers() -> anyhow::Result<()> {
    let config = Config::load()?;
    let mut fetcher = Fetcher::new(&config)
        .context("covers are turned off (download_covers = false) or there is no cache folder")?;
    let library = Library::scan();
    let wanted: Vec<(&Game, Kind)> = library
        .games
        .iter()
        .flat_map(|game| [(game, Kind::Cover), (game, Kind::Hero)])
        .filter(|(game, kind)| fetcher.wants(*game, *kind))
        .collect();
    if wanted.is_empty() {
        println!("No images to download.");
        if config.steamgriddb_key.is_none() {
            println!(
                "Games whose store has no image need a SteamGridDB key, set in Zendo's \
                 Settings or in {}.",
                Config::path()
                    .map_or_else(|| "config.toml".into(), |path| path.display().to_string())
            );
        }
        return Ok(());
    }
    let mut failed = 0;
    for (game, kind) in wanted {
        let what = match kind {
            Kind::Cover => "cover",
            Kind::Hero => "hero",
        };
        match fetcher.fetch(game, kind) {
            Ok(Some(path)) => println!("{}\t{what}\t{}", game.title, path.display()),
            Ok(None) => println!("{}\t{what}\tnone found", game.title),
            Err(err) => {
                failed += 1;
                eprintln!("zendo: {} ({what}): {err:#}", game.title);
            }
        }
    }
    if failed > 0 {
        anyhow::bail!("{failed} image(s) could not be downloaded");
    }
    Ok(())
}
