//! `zendo`'s command line: the commands to list and launch games from a
//! terminal or a keybind, the helper windows the window starts, and the
//! window itself, which each toolkit's binary passes in.

use std::process::ExitCode;

use anyhow::Context;

use crate::{file_dialog, links, sign_in_window};
use zendo_core::config::Config;
use zendo_core::covers::{Fetcher, Kind};
use zendo_core::{Game, Library, Size, State, launch, report};

const HELP: &str = "\
zendo: every game in one library

Usage:
  zendo                        open the library
  zendo --game-mode            open it in Game Mode (F11 toggles); --fullscreen is the same
  zendo list [--all] [--json]  print the games, in the window's order;
                               --all adds the hidden ones
  zendo launch <id>            start a game by the id `list` prints
  zendo hide <id>              leave a game out of the window
  zendo unhide <id>            bring it back
  zendo pin <id>               put a game first in every order
  zendo unpin <id>             stop
  zendo categorize <id> <category>
                               put a game in a category, made if new
  zendo uncategorize <id> <category>
                               take it out of one
  zendo status <id> <backlog|playing|completed|abandoned|none>
                               say where you are with a game
  zendo covers                 download the covers and wide images missing
  zendo logs                   list the games' last launch logs, latest first
  zendo logs <id>              print a game's last launch log
  zendo report                 print what Zendo found (tools, Protons,
                               settings without the keys, latest logs), to
                               paste into a bug report
  zendo nxm://…                download a mod from a Nexus Mods link, in the
                               open window or a new one
";

/// What opens the window: whether to start in Game Mode, and an `nxm://`
/// link it was started for.
pub type RunWindow = fn(bool, Option<String>) -> anyhow::Result<()>;

/// Runs the command line `zendo` was started with. `run_window` opens the
/// window, for the commands that want one.
#[must_use]
pub fn main(run_window: RunWindow) -> ExitCode {
    // A path on Linux need not be UTF-8: say so rather than panic.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let Some(args) = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<&str>>>()
    else {
        eprintln!("zendo: an argument is not valid UTF-8");
        return ExitCode::from(2);
    };
    // Started by the window for a store's sign-in; not for people.
    if let ["sign-in-window", store, url] = args.as_slice() {
        return sign_in_window::main(store, url);
    }
    // Started by the window for a mod's page on Nexus Mods.
    if let ["nexus-window", url, folder] = args.as_slice() {
        return sign_in_window::nexus_main(url, std::path::Path::new(folder));
    }
    // Started by the window to choose a file.
    if let ["pick-file", kind, title] = args.as_slice()
        && let Some(kind) = file_dialog::Kind::from_name(kind)
    {
        return file_dialog::main(kind, title);
    }
    let result = match args.as_slice() {
        [] => run_window(false, None),
        ["--game-mode" | "--fullscreen"] => run_window(true, None),
        [link]
            if link
                .get(..6)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("nxm://")) =>
        {
            open_link(link, run_window)
        }
        ["list", flags @ ..] if flags.iter().all(|flag| ["--all", "--json"].contains(flag)) => {
            list(flags.contains(&"--all"), flags.contains(&"--json"))
        }
        ["launch", id] => find(id).and_then(|game| launch_game(&game)),
        ["hide", id] => set_hidden(id, true),
        ["unhide", id] => set_hidden(id, false),
        ["pin", id] => set_pinned(id, true),
        ["unpin", id] => set_pinned(id, false),
        ["categorize", id, name] => set_category(id, name, true),
        ["uncategorize", id, name] => set_category(id, name, false),
        ["status", id, word] => set_status(id, word),
        ["covers"] => covers(),
        ["logs"] => {
            list_logs();
            Ok(())
        }
        ["logs", id] => print_log(id),
        ["report"] => {
            report();
            Ok(())
        }
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
fn open_link(link: &str, run_window: RunWindow) -> anyhow::Result<()> {
    zendo_core::nexus::Link::parse(link)?;
    if links::send(link) {
        return Ok(());
    }
    run_window(false, Some(link.to_owned()))
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

/// Starts a game, after downloading the anti-cheat runtimes it needs; one
/// that cannot be downloaded is said and the game starts without it.
fn launch_game(game: &Game) -> anyhow::Result<()> {
    let fetched = launch::fetch_runtimes(game, &mut |runtime| {
        eprintln!("zendo: downloading the {} runtime…", runtime.label());
    });
    if let Err(err) = fetched {
        eprintln!("zendo: starting without its anti-cheat runtime: {err:#}");
    }
    zendo_core::launch(game)
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

/// Puts a game in a category, made when it is new, or takes it out of one.
fn set_category(id: &str, name: &str, member: bool) -> anyhow::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("a category needs a name");
    }
    let game = find(id)?;
    // The name as it is stored, and whether the game was in it.
    let mut category = name.to_owned();
    if !member {
        // Refused before the file is touched, not after a write that
        // changed nothing.
        let state = State::load()?;
        let Some(found) = state
            .categories()
            .into_iter()
            .find(|found| found.name.eq_ignore_ascii_case(name))
        else {
            anyhow::bail!("{id} is not in {name:?}");
        };
        category = found.name;
        if !found.games.contains(&game.id) {
            anyhow::bail!("{id} is not in {category:?}");
        }
    }
    let mut made = false;
    let mut had = false;
    let mut blocked = false;
    State::update(|state| {
        if let Some(found) = state
            .categories()
            .into_iter()
            .find(|found| found.name.eq_ignore_ascii_case(name))
        {
            category = found.name;
            had = found.games.contains(&game.id);
        }
        if member && !had {
            made = state.add_category(name);
            // Refused when a saved view has the name: the two kinds of
            // collection share their names.
            blocked = !state.set_category(name, &game.id, true);
        } else if !member {
            state.set_category(name, &game.id, false);
        }
    })?;
    if blocked {
        anyhow::bail!("{name:?} is the name of a saved view; pick another name for the category");
    }
    if member {
        if had {
            println!("already in {category:?}");
        } else if made {
            println!("added {:?} to {category:?} (new category)", game.title);
        } else {
            println!("added {:?} to {category:?}", game.title);
        }
    } else {
        println!("took {:?} out of {category:?}", game.title);
    }
    Ok(())
}

/// Sets a game's play status, or clears it with `none`.
fn set_status(id: &str, word: &str) -> anyhow::Result<()> {
    let status = if word.trim().eq_ignore_ascii_case("none") {
        None
    } else {
        Some(zendo_core::Status::parse(word).with_context(|| {
            format!("{word:?} is not a status: backlog, playing, completed, abandoned or none")
        })?)
    };
    let game = find(id)?;
    State::update(|state| state.set_status(&game.id, status))?;
    match status {
        Some(status) => println!("{:?} is now {}", game.title, status.name()),
        None => println!("{:?} has no status", game.title),
    }
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

fn list_logs() {
    let logs = report::logs();
    if logs.is_empty() {
        println!("No launches logged yet.");
        return;
    }
    let library = Library::scan();
    for log in logs {
        let (id, title) = log.game(&library).map_or_else(
            || (String::new(), log.path.display().to_string()),
            |game| (game.id.clone(), game.title.clone()),
        );
        println!("{}\t{}\t{id}\t{title}", log.when(), Size::Bytes(log.size));
    }
}

fn print_log(id: &str) -> anyhow::Result<()> {
    let path = launch::log_path(id).context("no home folder to keep logs in")?;
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("no launch of {id:?} logged; `zendo logs` lists the ones there are")
        }
        Err(err) => return Err(err).with_context(|| format!("could not read {}", path.display())),
    };
    std::io::copy(&mut file, &mut std::io::stdout().lock())?;
    Ok(())
}

fn report() {
    let config = Config::load_as_is().unwrap_or_else(|err| {
        eprintln!("zendo: {err:#}");
        Config::load_file().unwrap_or_default()
    });
    println!("{}", report::gather(&config, &Library::scan()));
}
