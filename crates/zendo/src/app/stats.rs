//! The Stats page: what the play sessions add up to, for a year or for all
//! of them. Sessions are the runs of a game Zendo started and watched
//! until it ended; the page says so when there are none.

use iced::widget::{column, container, row, scrollable, space, text};
use iced::{Alignment, Background, Element, Length, Task};
use zendo_core::Status;
use zendo_core::sessions::{self, Period, Session, Summary};

use super::game::{card, stat_tile};
use super::library::{GRID_PADDING, chip};
use super::style::{self, SPACE_L, SPACE_M, SPACE_S, SPACE_XL, colors};
use super::{Message, Move, Zendo, blocking, centered, format};
use crate::i18n::t;

/// The longest a horizontal bar gets.
const BAR_LENGTH: f32 = 260.0;
/// How tall the tallest bar of a column chart gets.
const CHART_HEIGHT: f32 = 90.0;
const COLUMN_WIDTH: f32 = 22.0;

/// The period the page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsState {
    pub period: Period,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            period: Period::Year(sessions::year_now()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatsMessage {
    /// The sessions kept, read on a thread.
    Loaded(Result<Vec<Session>, String>),
    SetPeriod(Period),
}

/// The sessions kept from before, read on a thread.
pub fn load() -> Task<Message> {
    Task::perform(
        blocking(|| sessions::load().map_err(|err| format!("{err:#}"))),
        |result| Message::Stats(StatsMessage::Loaded(result)),
    )
}

impl Zendo {
    pub(super) fn update_stats(&mut self, message: StatsMessage) -> Task<Message> {
        match message {
            StatsMessage::Loaded(Ok(sessions)) => self.sessions = sessions,
            StatsMessage::Loaded(Err(err)) => self.status = Some(err),
            StatsMessage::SetPeriod(period) => self.stats.period = period,
        }
        Task::none()
    }

    /// The periods the chips offer: the years with sessions, latest first,
    /// then all of them.
    fn periods(&self) -> Vec<Period> {
        sessions::years(&self.sessions, sessions::year_now())
            .into_iter()
            .map(Period::Year)
            .chain([Period::All])
            .collect()
    }

    /// Left and Right (and the pad's) move along the period chips.
    pub(super) fn stats_step(&mut self, step: Move) -> Task<Message> {
        let periods = self.periods();
        let at = periods
            .iter()
            .position(|period| *period == self.stats.period)
            .unwrap_or(0);
        let to = match step {
            Move::Left | Move::Up => at.saturating_sub(1),
            Move::Right | Move::Down => (at + 1).min(periods.len() - 1),
            Move::First | Move::PageUp => 0,
            Move::Last | Move::PageDown => periods.len() - 1,
        };
        self.stats.period = periods[to];
        Task::none()
    }

    pub(super) fn stats_page(&self) -> Element<'_, Message> {
        if self.sessions.is_empty() {
            return centered(
                column![
                    text(t!("stats-none")).size(20),
                    text(t!("stats-none-about"))
                        .color(colors().dim)
                        .width(520)
                        .center(),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            );
        }
        let chips: Vec<Element<'_, Message>> = self
            .periods()
            .into_iter()
            .map(|period| {
                chip(
                    period_label(period),
                    None,
                    self.stats.period == period,
                    Message::Stats(StatsMessage::SetPeriod(period)),
                )
            })
            .collect();
        let header = row(chips)
            .spacing(SPACE_S)
            .padding(iced::Padding {
                top: SPACE_XL,
                right: GRID_PADDING,
                bottom: SPACE_S,
                left: GRID_PADDING,
            })
            .wrap()
            .vertical_spacing(SPACE_S);
        let summary = Summary::of(&self.sessions, self.stats.period);
        let body: Element<'_, Message> = if summary.sessions == 0 {
            centered(text(t!("stats-none-period")).color(colors().dim))
        } else {
            scrollable(
                column![
                    tiles(&summary),
                    top_games(&summary),
                    chart(
                        t!("stats-by-month"),
                        &summary.by_month,
                        (1..=12).map(format::month_short).collect(),
                    ),
                    chart(
                        t!("stats-by-weekday"),
                        &summary.by_weekday,
                        (0..7).map(format::weekday_short).collect(),
                    ),
                    facts(&summary, self.stats.period),
                    self.by_status(),
                ]
                .spacing(SPACE_XL)
                .padding(GRID_PADDING),
            )
            .height(Length::Fill)
            .into()
        };
        column![header, body].into()
    }

    /// How many games the library has in each play status, whatever the
    /// period.
    fn by_status(&self) -> Element<'_, Message> {
        let count = |status: Status| {
            self.library
                .iter()
                .flat_map(|library| &library.games)
                .filter(|game| game.status == Some(status))
                .count()
        };
        column![
            style::label(&t!("stats-by-status")),
            row(Status::ALL
                .map(|status| stat_tile(format::play_status(status), count(status).to_string())))
            .spacing(SPACE_M),
        ]
        .spacing(SPACE_S)
        .into()
    }
}

fn period_label(period: Period) -> String {
    match period {
        Period::Year(year) => year.to_string(),
        Period::All => t!("stats-all-time"),
    }
}

/// Four numbers: the time, the sessions, the days and the games.
fn tiles(summary: &Summary) -> Element<'static, Message> {
    row![
        stat_tile(t!("fact-play-time"), format::play_time(summary.minutes)),
        stat_tile(t!("stats-sessions"), summary.sessions.to_string()),
        stat_tile(t!("stats-days"), summary.active_days.to_string()),
        stat_tile(t!("stats-games"), summary.games.to_string()),
    ]
    .spacing(SPACE_M)
    .into()
}

/// A bar `share` of its full length, on a track.
fn bar<'a>(share: f32) -> Element<'a, Message> {
    container(
        container(space::vertical().height(8))
            .width(BAR_LENGTH * share.clamp(0.02, 1.0))
            .style(|_| container::Style {
                background: Some(Background::Color(colors().accent)),
                ..container::Style::default()
            }),
    )
    .width(BAR_LENGTH)
    .style(|_| container::Style {
        background: Some(Background::Color(colors().raised)),
        ..container::Style::default()
    })
    .into()
}

/// The most played games, each with a bar and its time.
fn top_games(summary: &Summary) -> Element<'static, Message> {
    let most = summary.top.first().map_or(1, |game| game.minutes.max(1));
    let rows = summary.top.iter().map(|game| {
        #[expect(clippy::cast_precision_loss, reason = "minutes are far below 2^24")]
        let share = game.minutes as f32 / most as f32;
        row![
            text(game.title.clone())
                .size(14)
                .width(Length::FillPortion(3)),
            bar(share),
            text(format::play_time(game.minutes))
                .size(13)
                .color(colors().dim)
                .width(Length::FillPortion(1)),
        ]
        .spacing(SPACE_L)
        .align_y(Alignment::Center)
        .into()
    });
    card(t!("stats-top"), column(rows).spacing(SPACE_M).into())
}

/// A column per value, as tall as its share of the largest, the hours on
/// top and a label under.
fn chart(heading: String, values: &[u64], labels: Vec<String>) -> Element<'static, Message> {
    let most = values.iter().copied().max().unwrap_or(0).max(1);
    let columns = values.iter().zip(labels).map(|(&minutes, label)| {
        #[expect(clippy::cast_precision_loss, reason = "minutes are far below 2^24")]
        let height = CHART_HEIGHT * (minutes as f32 / most as f32);
        // "45m" under an hour, then whole hours.
        let value = match minutes {
            0 => String::new(),
            1..60 => format!("{minutes}m"),
            _ => format!("{}h", (minutes + 30) / 60),
        };
        // The bar sits at the foot of an area as tall for every column, so
        // the labels line up whatever the bars are.
        let bar = container(space::horizontal())
            .width(COLUMN_WIDTH)
            .height(if minutes == 0 { 0.0 } else { height.max(2.0) })
            .style(|_| container::Style {
                background: Some(Background::Color(colors().accent)),
                ..container::Style::default()
            });
        column![
            container(text(value).size(11).color(colors().dim)).height(14),
            container(bar).align_bottom(CHART_HEIGHT),
            text(label).size(11).color(colors().dim),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .width(Length::FillPortion(1))
        .into()
    });
    card(heading, row(columns).spacing(SPACE_S).into())
}

/// The longest streak, the games started and the longest session.
fn facts(summary: &Summary, period: Period) -> Element<'static, Message> {
    let line = |label: String, value: String| -> Element<'_, Message> {
        row![
            text(label)
                .size(13)
                .color(colors().dim)
                .width(Length::FillPortion(1)),
            text(value).size(14).width(Length::FillPortion(3)),
        ]
        .spacing(SPACE_L)
        .into()
    };
    let mut lines = column![
        line(
            match period {
                Period::Year(_) => t!("stats-new-games"),
                Period::All => t!("stats-games"),
            },
            summary.new_games.to_string(),
        ),
        line(
            t!("stats-streak"),
            t!("stats-days-count", count = summary.streak)
        ),
    ]
    .spacing(SPACE_S);
    if let Some(longest) = &summary.longest {
        lines = lines.push(line(
            t!("stats-longest"),
            format!(
                "{} · {} · {}",
                format::play_time(longest.minutes),
                longest.title,
                longest.date
            ),
        ));
    }
    card(t!("stats-facts"), lines.into())
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn left_and_right_move_along_the_periods() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        // Sessions in 2024 and 2025: the chips are this year, 2025, 2024, all.
        zendo.sessions = vec![
            session("a", 1_717_000_000, 30),
            session("a", 1_748_000_000, 30),
        ];
        let this = sessions::year_now();
        zendo.stats.period = Period::Year(this);
        let _ = zendo.stats_step(Move::Right);
        assert_eq!(zendo.stats.period, Period::Year(2025));
        let _ = zendo.stats_step(Move::Right);
        let _ = zendo.stats_step(Move::Right);
        assert_eq!(zendo.stats.period, Period::All);
        let _ = zendo.stats_step(Move::Right);
        assert_eq!(zendo.stats.period, Period::All, "no further than the last");
        let _ = zendo.stats_step(Move::Left);
        assert_eq!(zendo.stats.period, Period::Year(2024));
        let _ = zendo.stats_step(Move::First);
        assert_eq!(zendo.stats.period, Period::Year(this));
    }

    #[test]
    fn loaded_sessions_replace_the_list_and_a_failure_is_said() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update_stats(StatsMessage::Loaded(Ok(vec![session("a", 1, 5)])));
        assert_eq!(zendo.sessions.len(), 1);
        let _ = zendo.update_stats(StatsMessage::Loaded(Err("broken".into())));
        assert_eq!(zendo.status.as_deref(), Some("broken"));
        assert_eq!(zendo.sessions.len(), 1, "a failed read keeps what there is");
    }
}
