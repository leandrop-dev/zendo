//! Numbers and times as people read them, in the window's language.

use zendo_core::{Sort, Source};

use crate::i18n::t;

/// Where a game came from, as the window names it: the stores by their
/// names, the rest in the window's language.
pub fn source(source: Source) -> String {
    match source {
        Source::Desktop => t!("source-desktop"),
        Source::Local => t!("source-added"),
        other => other.label().to_owned(),
    }
}

/// A library order's name.
pub fn sort(sort: Sort) -> String {
    match sort {
        Sort::Recent => t!("sort-recent"),
        Sort::MostPlayed => t!("sort-most-played"),
        Sort::Title => t!("sort-title"),
    }
}

/// "45 min", "3 h", "18 h 10 min".
pub fn play_time(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, minutes) => t!("time-minutes", minutes = minutes),
        (hours, 0) => t!("time-hours", hours = hours),
        (hours, minutes) => t!("time-hours-minutes", hours = hours, minutes = minutes),
    }
}

/// How long before `now` a time was: "today", "3 days ago".
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
pub fn since(then: u64, now: u64) -> String {
    match now.saturating_sub(then) {
        0..60 => t!("ago-now"),
        seconds @ 60..3_600 => t!("ago-minutes", count = seconds / 60),
        seconds @ 3_600..86_400 => t!("ago-hours", count = seconds / 3_600),
        _ => ago(then, now),
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
}
