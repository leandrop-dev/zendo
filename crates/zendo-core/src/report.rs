//! What Zendo found on this machine, as text to paste into a bug report:
//! the system, the tools and store engines with their versions, the
//! Protons and anti-cheat runtimes, the settings (never the keys), whether each store is signed in
//! (never the account), the library's counts and problems, the Wine
//! prefixes of the games Zendo starts, and the end of the latest launches'
//! logs.
//!
//! The home folder shows as `~`, and a key found anywhere in the text is
//! hidden.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use crate::config::{Config, Kept};
use crate::runtimes::{self, Found, Runtime};
use crate::secrets::Key;
use crate::stores::Store;
use crate::{Launch, Library, Size, Source, heroic, launch, paths, proton, steam, time};

/// How many of the latest launches' logs the report ends with.
const LOGS: usize = 3;
/// How many of each log's last lines it shows.
const LOG_LINES: usize = 40;
/// How much of a log's end is read for them.
const LOG_TAIL: u64 = 64 * 1024;
/// The longest a program is given to say its version.
const VERSION_WAIT: Duration = Duration::from_secs(5);

/// The programs Zendo uses besides the store engines, each with the
/// argument that makes it say its version, when it has a safe one.
const TOOLS: &[(&str, Option<&str>)] = &[
    ("umu-run", Some("--version")),
    ("gamescope", Some("--version")),
    ("mangohud", None),
    ("gamemoderun", None),
    ("7z", None),
    ("7zz", None),
    ("bsdtar", Some("--version")),
    ("xdg-mime", None),
    ("xdg-open", None),
];

/// A game's last launch, logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    pub path: PathBuf,
    /// Unix seconds.
    pub modified: Option<u64>,
    pub size: u64,
}

impl Log {
    /// When it was last written, as `2026-11-07 14:05 UTC`.
    #[must_use]
    pub fn when(&self) -> String {
        self.modified
            .map_or_else(|| "unknown".to_owned(), time::utc)
    }

    /// The game it is for, by the file name its id gives.
    #[must_use]
    pub fn game<'a>(&self, library: &'a Library) -> Option<&'a crate::Game> {
        library
            .games
            .iter()
            .find(|game| launch::log_path(&game.id).as_deref() == Some(self.path.as_path()))
    }
}

/// The launch logs, the latest first.
#[must_use]
pub fn logs() -> Vec<Log> {
    let Some(entries) = launch::logs_dir().and_then(|dir| fs::read_dir(dir).ok()) else {
        return Vec::new();
    };
    let mut logs: Vec<Log> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|since| since.as_secs());
            Some(Log {
                path: entry.path(),
                modified,
                size: meta.len(),
            })
        })
        .collect();
    logs.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    logs
}

/// The report. Runs each tool once to ask its version, and nile for
/// Amazon's sign-in, so it takes a moment: not for the window's thread.
#[must_use]
pub fn gather(config: &Config, library: &Library) -> String {
    let mut lines = vec!["# Zendo report".to_owned(), String::new()];
    system(&mut lines);
    tools(&mut lines);
    protons(&mut lines, config);
    anti_cheat(&mut lines);
    settings(&mut lines, config);
    accounts(&mut lines);
    games(&mut lines, library);
    prefixes(&mut lines, config, library);
    launches(&mut lines, library);
    let text = lines.join("\n");
    scrub(&text, paths::home().as_deref(), config)
}

fn heading(lines: &mut Vec<String>, title: &str) {
    lines.push(String::new());
    lines.push(format!("## {title}"));
    lines.push(String::new());
}

fn system(lines: &mut Vec<String>) {
    lines.push(format!(
        "Zendo {}, report made {}",
        env!("CARGO_PKG_VERSION"),
        time::utc(time::now())
    ));
    let os = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| os_name(&text))
        .unwrap_or_else(|| "unknown".to_owned());
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_or_else(|_| "unknown".to_owned(), |text| text.trim().to_owned());
    lines.push(format!("System: {os}, Linux {kernel}"));
    let var = |name| std::env::var(name).unwrap_or_else(|_| "unknown".to_owned());
    lines.push(format!(
        "Desktop: {} on {}",
        var("XDG_CURRENT_DESKTOP"),
        var("XDG_SESSION_TYPE")
    ));
    for gpu in gpus() {
        lines.push(format!("GPU: {gpu}"));
    }
}

/// `PRETTY_NAME` from `/etc/os-release`, else `NAME`.
fn os_name(text: &str) -> Option<String> {
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .map(|value| value.trim().trim_matches('"').to_owned())
    };
    value("PRETTY_NAME").or_else(|| value("NAME"))
}

/// Each graphics card's kernel driver and PCI ids.
fn gpus() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut cards: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `card0`, not its outputs (`card0-DP-1`).
            let number = name.strip_prefix("card")?;
            if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let device = entry.path().join("device");
            let read = |file: &str| {
                fs::read_to_string(device.join(file))
                    .map(|text| text.trim().trim_start_matches("0x").to_owned())
                    .unwrap_or_default()
            };
            let driver = fs::read_link(device.join("driver"))
                .ok()
                .and_then(|link| {
                    link.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "no driver".to_owned());
            Some((
                name,
                format!("{driver} ({}:{})", read("vendor"), read("device")),
            ))
        })
        .collect();
    cards.sort();
    cards.into_iter().map(|(_, card)| card).collect()
}

fn tools(lines: &mut Vec<String>) {
    heading(lines, "Tools");
    let found_in = |roots: Vec<PathBuf>| {
        roots
            .into_iter()
            .find(|root| root.is_dir())
            .map_or_else(|| "not found".to_owned(), |root| root.display().to_string())
    };
    lines.push(format!("{:<12} {}", "Steam", found_in(steam::roots())));
    lines.push(format!("{:<12} {}", "Heroic", found_in(heroic::roots())));
    for store in Store::ALL {
        lines.push(tool_line(
            store.engine_name(),
            store.engine(),
            Some("--version"),
        ));
    }
    for (name, version) in TOOLS {
        lines.push(tool_line(name, launch::which(name), *version));
    }
}

fn tool_line(name: &str, path: Option<PathBuf>, version_arg: Option<&str>) -> String {
    let Some(path) = path else {
        return format!("{name:<12} not found");
    };
    let version = version_arg
        .and_then(|arg| version(&path, arg))
        .map(|version| format!(", {version}"))
        .unwrap_or_default();
    format!("{name:<12} {}{version}", path.display())
}

/// The first line a program prints when asked its version, waiting for it
/// no longer than [`VERSION_WAIT`].
fn version(program: &Path, arg: &str) -> Option<String> {
    let mut child = Command::new(program)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + VERSION_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    let text = plain(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    let lines = || text.lines().map(str::trim).filter(|line| !line.is_empty());
    let line = lines()
        .find(|line| line.to_lowercase().contains("version"))
        .or_else(|| lines().next())?;
    // gamescope starts its lines with tags: `[gamescope] [Info]  console:`.
    let mut line = line;
    while let Some(rest) = line.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        line = rest.1.trim_start();
    }
    Some(line.chars().take(120).collect())
}

/// Text without the escape sequences terminals colour it with.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // `ESC [ … letter`, or a lone escape and the character after.
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn protons(lines: &mut Vec<String>, config: &Config) {
    heading(lines, "Protons");
    let installed = proton::installed();
    if installed.is_empty() {
        lines.push("None found.".to_owned());
    }
    for proton in &installed {
        lines.push(format!("{:<28} {}", proton.name, proton.path.display()));
    }
    let chosen = config.default_proton.as_deref();
    let default = match (proton::default(), chosen) {
        (Some(proton), Some(name)) if proton.name == name => {
            format!("{name}, chosen in Settings")
        }
        (Some(proton), Some(name)) => {
            format!(
                "{}, as {name}, chosen in Settings, is not installed",
                proton.name
            )
        }
        (Some(proton), None) => format!("{}, the newest found", proton.name),
        (None, _) => "none: umu-launcher downloads UMU-Proton".to_owned(),
    };
    lines.push(format!("Default: {default}"));
}

fn anti_cheat(lines: &mut Vec<String>) {
    heading(lines, "Anti-cheat runtimes");
    for runtime in Runtime::ALL {
        let found = match runtimes::find(runtime) {
            Some(Found::Steam(path)) => format!("Steam's, {}", path.display()),
            Some(Found::Zendo { path, version }) => format!(
                "Zendo's, built {}, {}",
                version.as_deref().unwrap_or("on an unknown day"),
                path.display()
            ),
            None => "none: downloaded when a game needs it".to_owned(),
        };
        lines.push(format!("{:<28} {found}", runtime.label()));
    }
}

fn settings(lines: &mut Vec<String>, config: &Config) {
    heading(lines, "Settings");
    lines.push(format!(
        "Download covers: {}",
        if config.download_covers { "yes" } else { "no" }
    ));
    lines.push(format!(
        "Update games by themselves: {}",
        if config.auto_update { "yes" } else { "no" }
    ));
    lines.push(format!(
        "Download workers: {}",
        config
            .download_workers
            .filter(|&workers| workers > 0)
            .map_or_else(|| "the engine's choice".to_owned(), |n| n.to_string())
    ));
    lines.push(format!(
        "Games folder: {}",
        config
            .install_dir()
            .map_or_else(|| "none".to_owned(), |dir| dir.display().to_string())
    ));
    for (key, name) in [
        (Key::SteamGridDb, "SteamGridDB key"),
        (Key::Nexus, "Nexus Mods key"),
        (Key::SteamSession, "Steam sign-in"),
    ] {
        let set = if config.key(key).is_some() {
            "set"
        } else {
            "not set"
        };
        let kept = match config.kept(key) {
            Kept::Nowhere => String::new(),
            Kept::Keyring => ", in the keyring".to_owned(),
            Kept::File(None) => ", in config.toml".to_owned(),
            Kept::File(Some(why)) => format!(", in config.toml ({why})"),
            Kept::Unread(why) => format!(", keyring not read ({why})"),
        };
        lines.push(format!("{name}: {set}{kept}"));
    }
}

fn accounts(lines: &mut Vec<String>) {
    heading(lines, "Store accounts");
    for store in Store::ALL {
        let state = match store.signed_in() {
            Ok(true) => "signed in".to_owned(),
            Ok(false) => "not signed in".to_owned(),
            Err(err) => format!("unknown: {err:#}"),
        };
        lines.push(format!("{:<8} {state}", store.source().label()));
    }
    let steam = if steam::user(&steam::roots()).is_some() {
        "an account has signed in"
    } else {
        "no account has signed in"
    };
    lines.push(format!("{:<8} {steam}", "Steam"));
}

fn games(lines: &mut Vec<String>, library: &Library) {
    heading(lines, "Library");
    let sources = [
        Source::Steam,
        Source::Epic,
        Source::Gog,
        Source::Amazon,
        Source::Heroic,
        Source::Desktop,
        Source::Local,
    ];
    let counts: Vec<String> = sources
        .into_iter()
        .filter_map(|source| {
            let count = library
                .games
                .iter()
                .filter(|game| game.source == source)
                .count();
            (count > 0).then(|| format!("{} {count}", source.label()))
        })
        .collect();
    let hidden = library.games.iter().filter(|game| game.hidden).count();
    lines.push(format!(
        "{} games ({}), {hidden} hidden",
        library.games.len(),
        counts.join(", ")
    ));
    if !library.problems.is_empty() {
        lines.push("Problems:".to_owned());
        lines.extend(
            library
                .problems
                .iter()
                .map(|problem| format!("- {problem}")),
        );
    }
}

fn prefixes(lines: &mut Vec<String>, config: &Config, library: &Library) {
    heading(lines, "Wine prefixes");
    let Some(dir) = config.install_dir() else {
        lines.push("No games folder.".to_owned());
        return;
    };
    let mut any = false;
    for game in &library.games {
        if !matches!(game.launch, Launch::Engine { .. } | Launch::Local { .. })
            || !launch::needs_proton(game)
        {
            continue;
        }
        any = true;
        let prefix = launch::prefix_in(&dir, game);
        let made = if prefix.is_dir() {
            ""
        } else {
            " (not made yet)"
        };
        lines.push(format!("{}: {}{made}", game.title, prefix.display()));
    }
    if !any {
        lines.push("No Windows games started by Zendo.".to_owned());
    }
}

fn launches(lines: &mut Vec<String>, library: &Library) {
    heading(lines, "Latest launches");
    let logs = logs();
    if logs.is_empty() {
        lines.push("None logged.".to_owned());
    }
    for log in logs.iter().take(LOGS) {
        let name = log.game(library).map_or_else(
            || log.path.display().to_string(),
            |game| format!("{} ({})", game.title, game.id),
        );
        lines.push(format!(
            "### {name}, {}, {}",
            log.when(),
            Size::Bytes(log.size)
        ));
        lines.push(String::new());
        lines.push("```".to_owned());
        match tail(&log.path, LOG_LINES) {
            Ok(text) => lines.push(text),
            Err(err) => lines.push(format!("could not read it: {err}")),
        }
        lines.push("```".to_owned());
        lines.push(String::new());
    }
}

/// The last `count` lines of a file, from its last [`LOG_TAIL`] bytes.
fn tail(path: &Path, count: usize) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(LOG_TAIL)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = plain(&String::from_utf8_lossy(&bytes));
    let lines: Vec<&str> = text.lines().collect();
    Ok(lines[lines.len().saturating_sub(count)..].join("\n"))
}

/// The home folder as `~`, and the keys, should one have been logged,
/// hidden.
fn scrub(text: &str, home: Option<&Path>, config: &Config) -> String {
    let mut text = text.to_owned();
    for key in Key::ALL {
        if let Some(value) = config.key(key).filter(|value| value.len() >= 4) {
            text = text.replace(value, "…");
        }
    }
    if let Some(home) = home.and_then(Path::to_str).filter(|home| home.len() > 1) {
        text = text.replace(home, "~");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_the_home_folder_and_the_keys() {
        let mut config = Config::default();
        config.nexus_key = Some("nexus-secret".into());
        let text = "/home/me/Games and /home/me/x?key=nexus-secret";
        assert_eq!(
            scrub(text, Some(Path::new("/home/me")), &config),
            "~/Games and ~/x?key=…"
        );
    }

    #[test]
    fn takes_the_colours_out() {
        assert_eq!(
            plain("[gamescope] [\u{1b}[0;34mInfo\u{1b}[0m]  ok"),
            "[gamescope] [Info]  ok"
        );
        assert_eq!(plain("plain"), "plain");
    }

    #[test]
    fn names_the_system() {
        let text = "NAME=\"CachyOS Linux\"\nPRETTY_NAME=\"CachyOS\"\nID=cachyos\n";
        assert_eq!(os_name(text).as_deref(), Some("CachyOS"));
        assert_eq!(os_name("NAME=Arch\n").as_deref(), Some("Arch"));
        assert_eq!(os_name(""), None);
    }

    #[test]
    fn keeps_a_logs_last_lines() {
        let dir = std::env::temp_dir().join(format!("zendo-report-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("game.log");
        let text: Vec<String> = (1..=100).map(|n| format!("line {n}")).collect();
        fs::write(&path, text.join("\n")).unwrap();
        let tail = tail(&path, 3).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(tail, "line 98\nline 99\nline 100");
    }

    #[test]
    fn asks_a_programs_version_and_gives_up_on_none() {
        assert_eq!(version(Path::new("/no/such/program"), "--version"), None);
        let echo = launch::which("echo").unwrap();
        assert_eq!(version(&echo, "1.2.3").as_deref(), Some("1.2.3"));
    }
}
