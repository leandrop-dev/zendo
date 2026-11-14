//! Numbers and times as people read them, in the window's language.

use zendo_core::{Sort, Source, Status};

use crate::t;

/// A month's short name, 1 to 12.
#[must_use]
pub fn month_short(month: u32) -> String {
    match month {
        1 => t!("month-1"),
        2 => t!("month-2"),
        3 => t!("month-3"),
        4 => t!("month-4"),
        5 => t!("month-5"),
        6 => t!("month-6"),
        7 => t!("month-7"),
        8 => t!("month-8"),
        9 => t!("month-9"),
        10 => t!("month-10"),
        11 => t!("month-11"),
        _ => t!("month-12"),
    }
}

/// A weekday's short name, 0 (Monday) to 6 (Sunday).
#[must_use]
pub fn weekday_short(day: u32) -> String {
    match day {
        0 => t!("weekday-1"),
        1 => t!("weekday-2"),
        2 => t!("weekday-3"),
        3 => t!("weekday-4"),
        4 => t!("weekday-5"),
        5 => t!("weekday-6"),
        _ => t!("weekday-7"),
    }
}

/// A library view chip's name.
#[must_use]
pub fn view(view: zendo_core::View) -> String {
    match view {
        zendo_core::View::All => t!("library-all"),
        zendo_core::View::Favorites => t!("library-favorites"),
        zendo_core::View::Recent => t!("library-recent"),
        zendo_core::View::Hidden => t!("library-hidden"),
    }
}

/// A play status's name.
#[must_use]
pub fn play_status(status: Status) -> String {
    match status {
        Status::Backlog => t!("play-backlog"),
        Status::Playing => t!("play-playing"),
        Status::Completed => t!("play-completed"),
        Status::Abandoned => t!("play-abandoned"),
    }
}

/// Where a game came from, as the window names it: the stores by their
/// names, the rest in the window's language.
#[must_use]
pub fn source(source: Source) -> String {
    match source {
        Source::Desktop => t!("source-desktop"),
        Source::Local => t!("source-added"),
        other => other.label().to_owned(),
    }
}

/// A library order's name.
#[must_use]
pub fn sort(sort: Sort) -> String {
    match sort {
        Sort::Recent => t!("sort-recent"),
        Sort::MostPlayed => t!("sort-most-played"),
        Sort::Title => t!("sort-title"),
    }
}

/// "45 min", "3 h", "18 h 10 min".
#[must_use]
pub fn play_time(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, minutes) => t!("time-minutes", minutes = minutes),
        (hours, 0) => t!("time-hours", hours = hours),
        (hours, minutes) => t!("time-hours-minutes", hours = hours, minutes = minutes),
    }
}

/// How long before `now` a time was: "today", "3 days ago".
#[must_use]
pub fn ago(then: u64, now: u64) -> String {
    let days = now.saturating_sub(then) / 86_400;
    match days {
        0 => t!("ago-today"),
        1 => t!("ago-yesterday"),
        2..=13 => t!("ago-days", count = days),
        14..=59 => t!("ago-weeks", count = days / 7),
        60..=729 => t!("ago-months", count = days / 30),
        _ => t!("ago-years", count = days / 365),
    }
}

/// How long before `now` a time was, to the minute while it is recent:
/// "just now", "5 min ago", "3 h ago", then as [`ago`].
#[must_use]
pub fn since(then: u64, now: u64) -> String {
    match now.saturating_sub(then) {
        0..60 => t!("ago-now"),
        seconds @ 60..3_600 => t!("ago-minutes", count = seconds / 60),
        seconds @ 3_600..86_400 => t!("ago-hours", count = seconds / 3_600),
        _ => ago(then, now),
    }
}

/// The share of achievements unlocked, for a card's line: nothing for a
/// game with none.
#[must_use]
pub fn progress_of((unlocked, total): (u32, u32)) -> Option<f32> {
    #[expect(clippy::cast_precision_loss, reason = "counts of achievements")]
    (total > 0).then(|| unlocked.min(total) as f32 / total as f32)
}

/// The line under a card's title: how much it was played.
#[must_use]
pub fn played_line(game: &zendo_core::Game, now: u64) -> String {
    match (game.played.minutes, game.played.last) {
        (0, None) => t!("played-never"),
        (0, Some(last)) => t!("played-when", when = ago(last, now)),
        (minutes, _) => t!("played-time", time = play_time(minutes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_like_people_write() {
        assert_eq!(play_time(45), "45 min");
        assert_eq!(play_time(180), "3 h");
        assert_eq!(play_time(1090), "18 h 10 min");
        let day = 86_400;
        assert_eq!(ago(10 * day, 10 * day + 5), "today");
        assert_eq!(ago(0, day), "yesterday");
        assert_eq!(ago(0, 20 * day), "2 weeks ago");
        assert_eq!(ago(0, 90 * day), "3 months ago");
        assert_eq!(ago(0, 800 * day), "2 years ago");
        assert_eq!(since(100, 130), "just now");
        assert_eq!(since(0, 300), "5 min ago");
        assert_eq!(since(0, 3 * 3_600 + 5), "3 h ago");
        assert_eq!(since(0, 2 * day), "2 days ago");
    }

    #[test]
    fn a_card_says_how_much_was_played() {
        let mut game = zendo_core::Game::new(
            "desktop:x".into(),
            "X".into(),
            Source::Desktop,
            zendo_core::Launch::Uri { uri: String::new() },
        );
        assert_eq!(played_line(&game, 0), t!("played-never"));
        game.played.last = Some(0);
        assert_eq!(played_line(&game, 0), t!("played-when", when = ago(0, 0)));
        game.played.minutes = 90;
        assert_eq!(
            played_line(&game, 0),
            t!("played-time", time = play_time(90))
        );
    }
}
