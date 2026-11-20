//! The Stats page: what the play sessions add up to, for a year or for all of
//! them, from `zendo_shell::stats`.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use zendo_core::sessions::{self, Period};
use zendo_shell::stats as shared;

use crate::library::LibraryState;

/// The period the page is on.
pub struct StatsState {
    period: Mutex<Period>,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            period: Mutex::new(Period::Year(sessions::year_now())),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChipDto {
    label: String,
    chosen: bool,
}

#[derive(Debug, Serialize)]
pub struct StatDto {
    label: String,
    value: String,
}

#[derive(Debug, Serialize)]
pub struct BarDto {
    label: String,
    value: String,
    share: f32,
}

/// The page as it stands.
#[derive(Debug, Default, Serialize)]
pub struct StatsPageDto {
    /// No session is kept at all.
    empty: bool,
    /// The period shown has no session.
    none_period: bool,
    periods: Vec<ChipDto>,
    /// The place of the chosen period among the chips.
    chosen: usize,
    tiles: Vec<StatDto>,
    top: Vec<BarDto>,
    months: Vec<BarDto>,
    weekdays: Vec<BarDto>,
    facts: Vec<StatDto>,
    statuses: Vec<StatDto>,
}

fn dto(app: &AppHandle) -> StatsPageDto {
    let kept = sessions::load().unwrap_or_default();
    let period = {
        let state = app.state::<StatsState>();
        *state.period.lock().unwrap_or_else(PoisonError::into_inner)
    };
    let games = app.state::<LibraryState>().games();
    let view = shared::view(&kept, period, &games);
    {
        let state = app.state::<StatsState>();
        *state.period.lock().unwrap_or_else(PoisonError::into_inner) = view.period;
    }
    let bars = |bars: &[shared::Column]| -> Vec<BarDto> {
        bars.iter()
            .map(|bar| BarDto {
                label: bar.label.clone(),
                value: bar.value.clone(),
                share: bar.share,
            })
            .collect()
    };
    let stats = |stats: &[shared::Stat]| -> Vec<StatDto> {
        stats
            .iter()
            .map(|stat| StatDto {
                label: stat.label.clone(),
                value: stat.value.clone(),
            })
            .collect()
    };
    StatsPageDto {
        empty: view.empty,
        none_period: view.none_period,
        chosen: view
            .periods
            .iter()
            .position(|chip| chip.chosen)
            .unwrap_or(0),
        periods: view
            .periods
            .into_iter()
            .map(|chip| ChipDto {
                label: chip.label,
                chosen: chip.chosen,
            })
            .collect(),
        tiles: stats(&view.tiles),
        top: view
            .top
            .iter()
            .map(|bar| BarDto {
                label: bar.label.clone(),
                value: bar.value.clone(),
                share: bar.share,
            })
            .collect(),
        months: bars(&view.months),
        weekdays: bars(&view.weekdays),
        facts: stats(&view.facts),
        statuses: stats(&view.statuses),
    }
}

#[tauri::command]
pub fn stats_get(app: AppHandle) -> StatsPageDto {
    dto(&app)
}

/// A period chip, or a move along them: the place among them.
#[tauri::command]
pub fn stats_period(at: usize, app: AppHandle) -> StatsPageDto {
    let kept = sessions::load().unwrap_or_default();
    if let Some(period) = shared::period_at(&kept, at) {
        let state = app.state::<StatsState>();
        *state.period.lock().unwrap_or_else(PoisonError::into_inner) = period;
    }
    dto(&app)
}
