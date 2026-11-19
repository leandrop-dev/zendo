//! The Stats page: what the play sessions add up to, for a year or for all of
//! them, and the library's statuses. Both windows say it the same.

use zendo_core::Game;
use zendo_core::Status;
use zendo_core::sessions::{self, Period, Session, Summary};

use crate::{format, t};

/// A chip: the period's name and whether it is the one shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodChip {
    pub label: String,
    pub chosen: bool,
}

/// A label and its value, as a tile or a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub label: String,
    pub value: String,
}

/// A bar: a game's play time as a share of the most played.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub label: String,
    pub value: String,
    pub share: f32,
}

/// A column of a chart: its name, its value and its share of the tallest.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub label: String,
    pub value: String,
    pub share: f32,
}

/// The page as it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct StatsView {
    /// The period shown: the one asked for, or the first there is.
    pub period: Period,
    pub periods: Vec<PeriodChip>,
    pub tiles: Vec<Stat>,
    pub top: Vec<Bar>,
    pub months: Vec<Column>,
    pub weekdays: Vec<Column>,
    pub facts: Vec<Stat>,
    pub statuses: Vec<Stat>,
    /// No session is kept at all.
    pub empty: bool,
    /// The period shown has no session.
    pub none_period: bool,
}

/// The periods the chips offer: the years with sessions, latest first, then
/// all of them.
#[must_use]
pub fn periods_of(sessions: &[Session]) -> Vec<Period> {
    sessions::years(sessions, sessions::year_now())
        .into_iter()
        .map(Period::Year)
        .chain([Period::All])
        .collect()
}

#[must_use]
pub fn period_label(period: Period) -> String {
    match period {
        Period::Year(year) => year.to_string(),
        Period::All => t!("stats-all-time"),
    }
}

/// The period of a place among the chips.
#[must_use]
pub fn period_at(sessions: &[Session], at: usize) -> Option<Period> {
    periods_of(sessions).get(at).copied()
}

/// A column's value: "45m" under an hour, then whole hours, nothing for none.
#[must_use]
pub fn column_value(minutes: u64) -> String {
    match minutes {
        0 => String::new(),
        1..60 => format!("{minutes}m"),
        _ => format!("{}h", (minutes + 30) / 60),
    }
}

/// `value` as a share of `most`, 0 to 1.
#[expect(clippy::cast_precision_loss, reason = "minutes are far below 2^24")]
fn share(value: u64, most: u64) -> f32 {
    value as f32 / most.max(1) as f32
}

fn columns(values: &[u64], labels: Vec<String>) -> Vec<Column> {
    let most = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .zip(labels)
        .map(|(&minutes, label)| Column {
            label,
            value: column_value(minutes),
            share: share(minutes, most),
        })
        .collect()
}

fn stat(label: String, value: String) -> Stat {
    Stat { label, value }
}

fn tiles_of(summary: &Summary) -> Vec<Stat> {
    vec![
        stat(t!("fact-play-time"), format::play_time(summary.minutes)),
        stat(t!("stats-sessions"), summary.sessions.to_string()),
        stat(t!("stats-days"), summary.active_days.to_string()),
        stat(t!("stats-games"), summary.games.to_string()),
    ]
}

fn top_of(summary: &Summary) -> Vec<Bar> {
    let most = summary.top.first().map_or(1, |game| game.minutes);
    summary
        .top
        .iter()
        .map(|game| Bar {
            label: game.title.clone(),
            value: format::play_time(game.minutes),
            share: share(game.minutes, most),
        })
        .collect()
}

fn facts_of(summary: &Summary, period: Period) -> Vec<Stat> {
    let mut facts = vec![
        stat(
            match period {
                Period::Year(_) => t!("stats-new-games"),
                Period::All => t!("stats-games"),
            },
            summary.new_games.to_string(),
        ),
        stat(
            t!("stats-streak"),
            t!("stats-days-count", count = summary.streak),
        ),
    ];
    if let Some(longest) = &summary.longest {
        facts.push(stat(
            t!("stats-longest"),
            format!(
                "{} · {} · {}",
                format::play_time(longest.minutes),
                longest.title,
                longest.date
            ),
        ));
    }
    facts
}

/// The page for `period`: the one asked for when there is a chip for it, else
/// the first; and the library's games, whose statuses the page counts.
#[must_use]
pub fn view(sessions: &[Session], period: Period, games: &[Game]) -> StatsView {
    let periods = periods_of(sessions);
    let period = if periods.contains(&period) {
        period
    } else {
        periods
            .first()
            .copied()
            .unwrap_or(Period::Year(sessions::year_now()))
    };
    let summary = Summary::of(sessions, period);
    let count = |status: Status| {
        games
            .iter()
            .filter(|game| game.status == Some(status))
            .count()
    };
    StatsView {
        period,
        periods: periods
            .iter()
            .map(|&chip| PeriodChip {
                label: period_label(chip),
                chosen: chip == period,
            })
            .collect(),
        tiles: tiles_of(&summary),
        top: top_of(&summary),
        months: columns(
            &summary.by_month,
            (1..=12).map(format::month_short).collect(),
        ),
        weekdays: columns(
            &summary.by_weekday,
            (0..7).map(format::weekday_short).collect(),
        ),
        facts: facts_of(&summary, period),
        statuses: Status::ALL
            .iter()
            .map(|status| stat(format::play_status(*status), count(*status).to_string()))
            .collect(),
        empty: sessions.is_empty(),
        none_period: summary.sessions == 0,
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn session(id: &str, start: u64, minutes: u64) -> Session {
        Session {
            id: id.to_owned(),
            title: id.to_owned(),
            start,
            minutes,
            offset: 0,
        }
    }

    fn game(id: &str, status: Option<Status>) -> Game {
        let mut game = Game::new(
            id.into(),
            id.to_uppercase(),
            Source::Desktop,
            Launch::Uri { uri: String::new() },
        );
        game.status = status;
        game
    }

    #[test]
    fn column_values_read_as_minutes_then_hours() {
        assert_eq!(column_value(0), "");
        assert_eq!(column_value(55), "55m");
        assert_eq!(column_value(60), "1h");
        assert_eq!(column_value(185), "3h");
    }

    #[test]
    fn a_page_with_no_sessions_is_empty_and_one_with_sessions_is_not() {
        let none = view(&[], Period::All, &[]);
        assert!(none.empty);
        assert_eq!(none.periods.len(), 2, "this year, then all of them");

        let kept = [
            session("a", 1_717_000_000, 30),
            session("b", 1_748_000_000, 90),
        ];
        let games = [game("a", Some(Status::Playing)), game("b", None)];
        let page = view(&kept, Period::All, &games);
        assert!(!page.empty && !page.none_period);
        assert_eq!(page.periods.len(), 4, "this year, 2025, 2024, all");
        assert_eq!(page.tiles[1].value, "2", "two sessions");
        assert_eq!(page.months.len(), 12);
        assert_eq!(page.weekdays.len(), 7);
        assert_eq!(page.top.len(), 2);
        assert_eq!(page.statuses.len(), 4);
        assert_eq!(page.statuses[1].value, "1", "one playing");
        assert!(
            page.periods
                .iter()
                .any(|chip| chip.chosen && chip.label == t!("stats-all-time"))
        );

        // This year has none: the page says so, and keeps the first chip.
        let this = view(&kept, Period::Year(sessions::year_now()), &games);
        assert!(this.none_period);
    }

    #[test]
    fn a_period_without_a_chip_falls_back_to_the_first() {
        let kept = [session("a", 1_748_000_000, 30)];
        let page = view(&kept, Period::Year(1999), &[]);
        assert_eq!(page.period, Period::Year(sessions::year_now()));
        assert!(page.periods[0].chosen);
    }
}
