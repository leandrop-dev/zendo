//! Games with a `.desktop` file: every application entry in the `Game`
//! category, except the launchers themselves and the shortcuts Steam and
//! Heroic write for games their own scanners already list.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{Game, Launch, Scan, Source, paths};

/// Desktop file ids of launchers, stores and gaming tools that file
/// themselves under `Game` alone, so no category tells them apart.
const LAUNCHERS: [&str; 9] = [
    "steam",
    "com.valvesoftware.Steam",
    "com.heroicgameslauncher.hgl",
    "heroic",
    "net.lutris.Lutris",
    "lutris",
    "io.github.Faugus.faugus-launcher",
    "io.github.benjamimgois.goverlay",
    "zendo",
];

/// Links whose games come from another scanner.
const OWNED_SCHEMES: [&str; 2] = ["steam://rungameid/", "heroic://launch"];

/// The `applications` folders, the user's first, so their entries win.
pub fn dirs() -> Vec<PathBuf> {
    paths::data_home()
        .into_iter()
        .chain(paths::data_dirs())
        .map(|dir| dir.join("applications"))
        .collect()
}

pub fn scan(dirs: &[PathBuf]) -> Scan {
    let mut scan = Scan::default();
    let mut seen = HashSet::new();
    for dir in dirs {
        let mut files = Vec::new();
        collect(dir, &mut files);
        files.sort();
        for file in files {
            let Some(id) = desktop_id(dir, &file) else {
                continue;
            };
            // The first folder that has an id decides it, even when that
            // entry is hidden: that is how users hide system entries.
            if !seen.insert(id.clone()) {
                continue;
            }
            match fs::read_to_string(&file) {
                Ok(text) => {
                    if let Some(game) = parse(&id, &text) {
                        scan.games.push(game);
                    }
                }
                Err(err) => scan.problems.push(format!("{}: {err}", file.display())),
            }
        }
    }
    scan
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "desktop") {
            files.push(path);
        }
    }
}

/// The desktop file id: the path under `applications` with `/` as `-` and
/// without `.desktop`.
fn desktop_id(dir: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(dir).ok()?.to_str()?;
    Some(relative.strip_suffix(".desktop")?.replace('/', "-"))
}

/// The game an entry describes, or `None` when it is not a visible game.
fn parse(id: &str, text: &str) -> Option<Game> {
    let entry = Entry::parse(text);
    let shown = entry.get("Type") == Some("Application")
        && entry.get("NoDisplay") != Some("true")
        && entry.get("Hidden") != Some("true")
        && entry
            .get("Categories")
            .is_some_and(|categories| categories.split(';').any(|category| category == "Game"));
    if !shown || LAUNCHERS.contains(&id) {
        return None;
    }
    let exec = entry.get("Exec")?;
    if OWNED_SCHEMES.iter().any(|scheme| exec.contains(scheme)) {
        return None;
    }
    let mut words = split_exec(&unescape(exec))?.into_iter();
    let program = words.next()?;
    Some(Game {
        id: format!("desktop:{id}"),
        title: entry.get("Name").map_or_else(|| id.to_owned(), unescape),
        source: Source::Desktop,
        launch: Launch::Command {
            program,
            args: words.collect(),
            dir: entry
                .get("Path")
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        },
        cover: None,
    })
}

/// The keys of the `[Desktop Entry]` group, without the translated ones.
struct Entry<'a> {
    pairs: Vec<(&'a str, &'a str)>,
}

impl<'a> Entry<'a> {
    fn parse(text: &'a str) -> Self {
        let mut pairs = Vec::new();
        let mut in_entry = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_entry = line == "[Desktop Entry]";
            } else if in_entry
                && !line.starts_with('#')
                && let Some((key, value)) = line.split_once('=')
                && !key.contains('[')
            {
                pairs.push((key.trim(), value.trim()));
            }
        }
        Self { pairs }
    }

    fn get(&self, key: &str) -> Option<&'a str> {
        self.pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    }
}

/// The string escapes every value may hold: `\s`, `\n`, `\t`, `\r`, `\\`.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// Splits `Exec` into words as the Desktop Entry spec says: double quotes
/// group, a backslash inside them escapes `"`, `` ` ``, `$` and `\`. Field
/// codes (`%f`, `%U`, …) are dropped, since nothing is opened with the
/// game, and `%%` is a literal `%`. `None` for an unclosed quote.
fn split_exec(exec: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            '\\' if quoted => {
                if let Some(&next) = chars.peek().filter(|next| "\"`$\\".contains(**next)) {
                    word.push(next);
                    chars.next();
                } else {
                    word.push('\\');
                }
            }
            '%' if !quoted => {
                if chars.next() == Some('%') {
                    word.push('%');
                    in_word = true;
                }
            }
            c if c.is_whitespace() && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if in_word && !word.is_empty() {
        words.push(word);
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/applications")
    }

    #[test]
    fn finds_games_only() {
        let scan = scan(&[fixture()]);
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        let ids: Vec<&str> = scan.games.iter().map(|game| game.id.as_str()).collect();
        assert_eq!(ids, ["desktop:games-supertux", "desktop:xonotic"]);
        assert_eq!(
            scan.games[1].launch,
            Launch::Command {
                program: "xonotic-sdl".into(),
                args: vec!["-basedir".into(), "/opt/xonotic data".into()],
                dir: Some(PathBuf::from("/opt/xonotic")),
            }
        );
    }

    #[test]
    fn a_user_entry_hides_the_system_one() {
        let user = fixture().join("../applications-user");
        let scan = scan(&[user, fixture()]);
        let ids: Vec<&str> = scan.games.iter().map(|game| game.id.as_str()).collect();
        assert_eq!(ids, ["desktop:games-supertux"]);
    }

    #[test]
    fn splits_exec_like_the_spec() {
        assert_eq!(
            split_exec(r#"env "A=b c" game %U --x=100%% "say \"hi\"""#).unwrap(),
            ["env", "A=b c", "game", "--x=100%", r#"say "hi""#]
        );
        assert!(split_exec(r#"game "unclosed"#).is_none());
    }
}
