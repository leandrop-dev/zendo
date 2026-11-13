//! Play sessions: each run of a game that Zendo started and watched until it
//! ended, kept in `$XDG_STATE_HOME/zendo/sessions.jsonl` (one JSON object a
//! line, appended to), and what can be said of them: a game's history and a
//! summary of a year or of all of them.
//!
//! The launchers' own totals are not sessions: a game started outside Zendo,
//! by `zendo launch` (nobody watches that one) or before this file existed
//! has none. Not in `state.json`, which is rewritten whole at every change,
//! while this file grows for years.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{paths, time};

/// How many of a game's latest sessions [`of_game`] lists.
pub const RECENT: usize = 10;
/// How many games a summary's top list holds.
pub const TOP: usize = 10;

/// One run of a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The game's id, as the library has it.
    pub id: String,
    /// Its title then, so a game that left the library still reads in an
    /// old year.
    #[serde(default)]
    pub title: String,
    /// When it began, Unix seconds.
    pub start: u64,
    pub minutes: u64,
    /// The local time's offset from UTC, in seconds, when it ended: the
    /// day, month and weekday it counts for are the local ones.
    #[serde(default)]
    pub offset: i32,
}

impl Session {
    /// The local day it began on, counted from 1970-01-01. A session that
    /// crosses midnight is not split.
    fn day(&self) -> i64 {
        let start = i64::try_from(self.start).unwrap_or(i64::MAX / 2);
        (start + i64::from(self.offset)).div_euclid(86_400)
    }

    fn year(&self) -> i32 {
        i32::try_from(time::civil_from_days(self.day()).0).unwrap_or(i32::MAX)
    }
}

/// Where the sessions are kept.
#[must_use]
pub fn path() -> Option<PathBuf> {
    paths::state_home().map(|dir| dir.join("zendo/sessions.jsonl"))
}

/// Adds a session to the file.
///
/// # Errors
///
/// When there is no state folder, or the file cannot be locked or written.
pub fn record(session: &Session) -> anyhow::Result<()> {
    let path = path().context("no home folder to keep Zendo's sessions in")?;
    append_to(&path, session)
}

/// Every session kept, oldest first as they were written. A line that does
/// not read (a last one cut short by a crash, say) is skipped, not the
/// rest with it.
///
/// # Errors
///
/// When the file exists and cannot be read.
pub fn load() -> anyhow::Result<Vec<Session>> {
    match path() {
        Some(path) => read_from(&path),
        None => Ok(Vec::new()),
    }
}

fn append_to(path: &Path, session: &Session) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(session)?;
    line.push(b'\n');
    paths::locked(path, || {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)
            .with_context(|| format!("could not open {}", path.display()))?;
        // A file whose last line was cut short has no newline: starting a
        // new line keeps the new session from being glued to it.
        let len = file.metadata()?.len();
        if len > 0 {
            file.seek(SeekFrom::Start(len - 1))?;
            let mut last = [0_u8; 1];
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.insert(0, b'\n');
            }
        }
        file.write_all(&line)
            .and_then(|()| file.sync_data())
            .with_context(|| format!("could not write {}", path.display()))
    })
}

fn read_from(path: &Path) -> anyhow::Result<Vec<Session>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("could not read {}", path.display())),
    };
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

/// The local time's offset from UTC at a moment, in seconds, as `date`
/// says it. Zero (UTC) when `date` cannot be run.
#[must_use]
pub fn local_offset(at: u64) -> i32 {
    Command::new("date")
        .arg("-d")
        .arg(format!("@{at}"))
        .arg("+%z")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| parse_offset(String::from_utf8_lossy(&output.stdout).trim()))
        .unwrap_or(0)
}

/// `+0100` or `-0330` as seconds.
fn parse_offset(text: &str) -> Option<i32> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, text.get(1..)?),
        b'-' => (-1, text.get(1..)?),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits[..2].parse().ok()?;
    let minutes: i32 = digits[2..].parse().ok()?;
    (hours < 24 && minutes < 60).then_some(sign * (hours * 3600 + minutes * 60))
}

/// The year now, UTC, for the list of years.
#[must_use]
pub fn year_now() -> i32 {
    i32::try_from(time::year_of(i64::try_from(time::now()).unwrap_or(0))).unwrap_or(1970)
}

/// What a summary covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Year(i32),
    All,
}

/// The years that have sessions, and `current`, latest first.
#[must_use]
pub fn years(sessions: &[Session], current: i32) -> Vec<i32> {
    let mut years: BTreeSet<i32> = sessions.iter().map(Session::year).collect();
    years.insert(current);
    years.into_iter().rev().collect()
}

/// A game in a summary's top list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopGame {
    pub id: String,
    /// Its title in its latest session.
    pub title: String,
    pub minutes: u64,
    pub sessions: usize,
}

/// A session that stood out for its length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Longest {
    pub minutes: u64,
    pub title: String,
    /// Its local day, `2026-11-13`.
    pub date: String,
}

/// What the sessions of a year, or of all, add up to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub minutes: u64,
    pub sessions: usize,
    /// Different games played.
    pub games: usize,
    /// Different days played on.
    pub active_days: usize,
    pub longest: Option<Longest>,
    /// The most played, up to [`TOP`].
    pub top: Vec<TopGame>,
    /// Minutes in each month of the year (all years added, for `All`).
    pub by_month: [u64; 12],
    /// Minutes on each weekday, Monday first.
    pub by_weekday: [u64; 7],
    /// Games whose first session ever fell in the period (every game, for
    /// `All`).
    pub new_games: usize,
    /// The most days played in a row.
    pub streak: usize,
}

impl Summary {
    /// The summary of the sessions in `period`.
    #[must_use]
    pub fn of(sessions: &[Session], period: Period) -> Self {
        // The first day each game was ever played, over every session.
        let mut first: BTreeMap<&str, i64> = BTreeMap::new();
        for session in sessions {
            let day = session.day();
            first
                .entry(session.id.as_str())
                .and_modify(|first| *first = (*first).min(day))
                .or_insert(day);
        }
        let in_period = |session: &&Session| match period {
            Period::All => true,
            Period::Year(year) => session.year() == year,
        };
        let chosen: Vec<&Session> = sessions.iter().filter(in_period).collect();

        let mut summary = Self {
            sessions: chosen.len(),
            ..Self::default()
        };
        let mut days = BTreeSet::new();
        let mut per_game: BTreeMap<&str, (u64, usize, u64, &str)> = BTreeMap::new();
        let mut longest: Option<&Session> = None;
        for session in &chosen {
            summary.minutes += session.minutes;
            let day = session.day();
            days.insert(day);
            let (_, month, _) = time::civil_from_days(day);
            summary.by_month[usize::try_from(month - 1).unwrap_or(0).min(11)] += session.minutes;
            summary.by_weekday[time::weekday_of(day)] += session.minutes;
            // The title of the latest session names the game.
            let entry =
                per_game
                    .entry(session.id.as_str())
                    .or_insert((0, 0, 0, session.title.as_str()));
            entry.0 += session.minutes;
            entry.1 += 1;
            if session.start >= entry.2 {
                entry.2 = session.start;
                entry.3 = session.title.as_str();
            }
            if longest.is_none_or(|best| session.minutes > best.minutes) {
                longest = Some(session);
            }
        }
        summary.games = per_game.len();
        summary.active_days = days.len();
        summary.streak = longest_run(&days);
        summary.longest = longest.map(|session| {
            let (year, month, day) = time::civil_from_days(session.day());
            Longest {
                minutes: session.minutes,
                title: session.title.clone(),
                date: format!("{year:04}-{month:02}-{day:02}"),
            }
        });
        summary.new_games = match period {
            Period::All => per_game.len(),
            Period::Year(year) => per_game
                .keys()
                .filter(|id| {
                    first.get(*id).is_some_and(|&day| {
                        i32::try_from(time::civil_from_days(day).0).ok() == Some(year)
                    })
                })
                .count(),
        };
        let mut top: Vec<TopGame> = per_game
            .into_iter()
            .map(|(id, (minutes, sessions, _, title))| TopGame {
                id: id.to_owned(),
                title: title.to_owned(),
                minutes,
                sessions,
            })
            .collect();
        top.sort_by(|a, b| {
            b.minutes
                .cmp(&a.minutes)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        top.truncate(TOP);
        summary.top = top;
        summary
    }
}

/// The longest run of consecutive days in a sorted set.
fn longest_run(days: &BTreeSet<i64>) -> usize {
    let (mut best, mut run, mut previous) = (0, 0, None);
    for &day in days {
        run = if previous == Some(day - 1) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        previous = Some(day);
    }
    best
}

/// One game's sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSessions {
    pub count: usize,
    pub minutes: u64,
    pub average: u64,
    pub longest: u64,
    /// The latest, [`RECENT`] at most, latest first.
    pub recent: Vec<Session>,
}

/// A game's sessions, or `None` when it has none.
#[must_use]
pub fn of_game(sessions: &[Session], id: &str) -> Option<GameSessions> {
    let mut own: Vec<&Session> = sessions.iter().filter(|s| s.id == id).collect();
    if own.is_empty() {
        return None;
    }
    let minutes: u64 = own.iter().map(|session| session.minutes).sum();
    let count = own.len();
    let longest = own.iter().map(|session| session.minutes).max().unwrap_or(0);
    own.sort_by_key(|session| std::cmp::Reverse(session.start));
    Some(GameSessions {
        count,
        minutes,
        average: minutes / u64::try_from(count).unwrap_or(1).max(1),
        longest,
        recent: own.into_iter().take(RECENT).cloned().collect(),
    })
}

/// A session's local date as `2026-11-13`, for lists.
#[must_use]
pub fn date_of(session: &Session) -> String {
    let (year, month, day) = time::civil_from_days(session.day());
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A session beginning at this UTC moment.
    fn at(id: &str, year: i64, month: i64, day: i64, hour: u64, minutes: u64) -> Session {
        let days = u64::try_from(time::days_from_civil(year, month, day)).unwrap();
        Session {
            id: id.to_owned(),
            title: id.to_uppercase(),
            start: days * 86_400 + hour * 3600,
            minutes,
            offset: 0,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zendo-sessions-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn keeps_sessions_through_a_round_trip() {
        let dir = scratch("round");
        let file = dir.join("zendo/sessions.jsonl");
        assert_eq!(read_from(&file).unwrap(), []);
        let one = at("a", 2026, 11, 13, 20, 90);
        let two = at("b", 2026, 11, 14, 9, 5);
        append_to(&file, &one).unwrap();
        append_to(&file, &two).unwrap();
        assert_eq!(read_from(&file).unwrap(), [one, two]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cut_short_line_is_skipped_and_the_next_session_kept() {
        let dir = scratch("cut");
        let file = dir.join("sessions.jsonl");
        fs::create_dir_all(&dir).unwrap();
        let one = at("a", 2026, 1, 2, 3, 4);
        let good = serde_json::to_string(&one).unwrap();
        // A good line, a bad one, and one cut off with no newline.
        fs::write(&file, format!("{good}\nnot json\n{{\"id\":\"x\",\"sta")).unwrap();
        let two = at("b", 2026, 1, 3, 3, 4);
        append_to(&file, &two).unwrap();
        assert_eq!(read_from(&file).unwrap(), [one, two]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_an_offset() {
        assert_eq!(parse_offset("+0100"), Some(3600));
        assert_eq!(parse_offset("-0330"), Some(-(3 * 3600 + 30 * 60)));
        assert_eq!(parse_offset("+0000"), Some(0));
        assert_eq!(parse_offset("0100"), None);
        assert_eq!(parse_offset("+01"), None);
        assert_eq!(parse_offset("+2500"), None);
        assert_eq!(parse_offset("+01:00"), None);
        assert_eq!(parse_offset(""), None);
    }

    #[test]
    fn sums_a_year_and_all_of_them() {
        let sessions = [
            at("a", 2025, 12, 31, 20, 60),
            at("a", 2026, 1, 1, 20, 30),
            at("b", 2026, 1, 1, 22, 15),
            at("b", 2026, 3, 5, 10, 45),
        ];
        let year = Summary::of(&sessions, Period::Year(2026));
        assert_eq!((year.minutes, year.sessions, year.games), (90, 3, 2));
        assert_eq!(year.active_days, 2);
        assert_eq!(year.by_month[0], 45);
        assert_eq!(year.by_month[2], 45);
        assert_eq!(year.longest.as_ref().unwrap().minutes, 45);
        assert_eq!(year.longest.as_ref().unwrap().date, "2026-03-05");
        // `a` was first played in 2025, `b` in 2026.
        assert_eq!(year.new_games, 1);

        let all = Summary::of(&sessions, Period::All);
        assert_eq!((all.minutes, all.sessions, all.games), (150, 4, 2));
        assert_eq!(all.new_games, 2);
        // 2025-12-31 and 2026-01-01 are consecutive days.
        assert_eq!(all.streak, 2);
        assert_eq!(
            Summary::of(&sessions, Period::Year(2024)),
            Summary::default()
        );
    }

    #[test]
    fn counts_a_session_on_the_local_day() {
        // 23:30 UTC on 2026-10-31 is 01:30 on Sunday 2026-11-01 at +0200.
        let mut late = at("a", 2026, 10, 31, 23, 40);
        late.start += 1800;
        late.offset = 7200;
        let month = Summary::of(std::slice::from_ref(&late), Period::Year(2026));
        assert_eq!(month.by_month[9], 0);
        assert_eq!(month.by_month[10], 40);
        assert_eq!(month.by_weekday[6], 40, "Sunday");
        assert_eq!(month.longest.unwrap().date, "2026-11-01");
        // Near New Year the local year is the one that counts.
        let mut new_year = at("a", 2025, 12, 31, 23, 10);
        new_year.start += 1800;
        new_year.offset = 3600;
        assert_eq!(Summary::of(&[new_year], Period::Year(2026)).sessions, 1);
    }

    #[test]
    fn finds_the_longest_streak_across_a_month_end() {
        let sessions = [
            at("a", 2026, 1, 30, 9, 5),
            at("a", 2026, 1, 31, 9, 5),
            at("a", 2026, 2, 1, 9, 5),
            at("a", 2026, 2, 3, 9, 5),
            at("a", 2026, 2, 3, 20, 5),
        ];
        let summary = Summary::of(&sessions, Period::All);
        assert_eq!(summary.streak, 3);
        assert_eq!(summary.active_days, 4);
        assert_eq!(Summary::of(&[], Period::All).streak, 0);
    }

    #[test]
    fn ranks_the_top_games_and_caps_the_list() {
        let mut sessions = vec![
            at("slow", 2026, 5, 1, 9, 10),
            at("long", 2026, 5, 2, 9, 120),
            at("tied", 2026, 5, 3, 9, 10),
        ];
        sessions.push(at("slow", 2026, 5, 4, 9, 50));
        let top = Summary::of(&sessions, Period::All).top;
        let ids: Vec<&str> = top.iter().map(|game| game.id.as_str()).collect();
        assert_eq!(ids, ["long", "slow", "tied"]);
        assert_eq!((top[1].minutes, top[1].sessions), (60, 2));

        let many: Vec<Session> = (0..15)
            .map(|n| at(&format!("g{n:02}"), 2026, 5, 1, 9, 10 + n))
            .collect();
        let top = Summary::of(&many, Period::All).top;
        assert_eq!(top.len(), TOP);
        assert_eq!(top[0].id, "g14");
    }

    #[test]
    fn lists_the_years_with_sessions_and_this_one() {
        let sessions = [at("a", 2023, 1, 1, 9, 5), at("a", 2025, 6, 1, 9, 5)];
        assert_eq!(years(&sessions, 2026), [2026, 2025, 2023]);
        assert_eq!(years(&sessions, 2025), [2025, 2023]);
        assert_eq!(years(&[], 2026), [2026]);
    }

    #[test]
    fn says_what_a_game_has_played() {
        let mut sessions: Vec<Session> = (0..12)
            .map(|n: i64| at("a", 2026, 3, 1 + n, 9, 10 + n.unsigned_abs()))
            .collect();
        sessions.push(at("b", 2026, 3, 1, 9, 999));
        assert_eq!(of_game(&sessions, "none"), None);
        let game = of_game(&sessions, "a").unwrap();
        assert_eq!(game.count, 12);
        assert_eq!(game.minutes, 12 * 10 + 66);
        assert_eq!(game.average, (12 * 10 + 66) / 12);
        assert_eq!(game.longest, 21);
        assert_eq!(game.recent.len(), RECENT);
        assert_eq!(game.recent[0].minutes, 21, "latest first");
    }
}
