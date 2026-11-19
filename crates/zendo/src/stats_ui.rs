//! The Stats page in the window: what the play sessions add up to, for a year
//! or for all of them. The sessions are the list the game page keeps
//! (`game_ui`); the sums are `zendo_core::sessions::Summary`. A part of the
//! Library's glue (`library_ui`).

use std::cell::Cell;
use std::rc::Rc;

use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::browse::Move;
use zendo_core::sessions::{self, Period};

use super::Glue;
use crate::{ChipData, MainWindow, Page, StatBar, StatCol, StatLine, StatsData, Tile};

/// What the page keeps: the period shown and the rows drawn.
pub(super) struct StatsState {
    period: Cell<Period>,
    periods: Rc<VecModel<ChipData>>,
    tiles: Rc<VecModel<Tile>>,
    top: Rc<VecModel<StatBar>>,
    months: Rc<VecModel<StatCol>>,
    weekdays: Rc<VecModel<StatCol>>,
    facts: Rc<VecModel<StatLine>>,
    statuses: Rc<VecModel<Tile>>,
}

impl StatsState {
    pub(super) fn new() -> Self {
        Self {
            period: Cell::new(Period::Year(sessions::year_now())),
            periods: Rc::new(VecModel::default()),
            tiles: Rc::new(VecModel::default()),
            top: Rc::new(VecModel::default()),
            months: Rc::new(VecModel::default()),
            weekdays: Rc::new(VecModel::default()),
            facts: Rc::new(VecModel::default()),
            statuses: Rc::new(VecModel::default()),
        }
    }
}

/// The periods the chips offer, from `zendo_shell::stats`.
fn periods_of(sessions: &[sessions::Session]) -> Vec<Period> {
    zendo_shell::stats::periods_of(sessions)
}

impl Glue {
    /// Hooks the page's callbacks up.
    pub(super) fn connect_stats(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<StatsData>();
        data.set_periods(ModelRc::from(Rc::clone(&self.stats.periods)));
        data.set_tiles(ModelRc::from(Rc::clone(&self.stats.tiles)));
        data.set_top(ModelRc::from(Rc::clone(&self.stats.top)));
        data.set_months(ModelRc::from(Rc::clone(&self.stats.months)));
        data.set_weekdays(ModelRc::from(Rc::clone(&self.stats.weekdays)));
        data.set_facts(ModelRc::from(Rc::clone(&self.stats.facts)));
        data.set_statuses(ModelRc::from(Rc::clone(&self.stats.statuses)));
        let glue = Rc::clone(self);
        data.on_period(move |at| {
            let periods = periods_of(&glue.game.sessions.borrow());
            if let Some(period) = usize::try_from(at).ok().and_then(|at| periods.get(at)) {
                glue.stats.period.set(*period);
                glue.show_stats();
            }
        });
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_stats());
        self.show_stats();
    }

    /// Writes the page from the sessions kept and the library's statuses. The
    /// words and sums are `zendo_shell::stats`'s, shared with Tauri.
    pub(super) fn show_stats(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let all = self.game.sessions.borrow();
        let browser = self.browser.borrow();
        let view = zendo_shell::stats::view(&all, self.stats.period.get(), browser.games());
        drop(browser);
        drop(all);
        // A period that has no chip any more (a year with no session left)
        // falls back to the first.
        self.stats.period.set(view.period);
        let data = window.global::<StatsData>();
        data.set_empty(view.empty);
        data.set_none_period(view.none_period);
        self.stats.periods.set_vec(
            view.periods
                .iter()
                .enumerate()
                .map(|(at, chip)| ChipData {
                    id: i32::try_from(at).unwrap_or(0),
                    label: chip.label.clone().into(),
                    count: "".into(),
                    chosen: chip.chosen,
                })
                .collect::<Vec<_>>(),
        );
        self.stats.tiles.set_vec(
            view.tiles
                .iter()
                .map(|tile| Tile {
                    label: tile.label.clone().into(),
                    value: tile.value.clone().into(),
                })
                .collect::<Vec<_>>(),
        );
        self.stats.top.set_vec(
            view.top
                .iter()
                .map(|bar| StatBar {
                    title: bar.label.clone().into(),
                    value: bar.value.clone().into(),
                    share: bar.share,
                })
                .collect::<Vec<_>>(),
        );
        let cols = |cols: &[zendo_shell::stats::Column]| -> Vec<StatCol> {
            cols.iter()
                .map(|col| StatCol {
                    label: col.label.clone().into(),
                    value: col.value.clone().into(),
                    share: col.share,
                })
                .collect()
        };
        self.stats.months.set_vec(cols(&view.months));
        self.stats.weekdays.set_vec(cols(&view.weekdays));
        let lines = |lines: &[zendo_shell::stats::Stat]| -> Vec<StatLine> {
            lines
                .iter()
                .map(|line| StatLine {
                    label: line.label.clone().into(),
                    value: line.value.clone().into(),
                })
                .collect()
        };
        self.stats.facts.set_vec(lines(&view.facts));
        let statuses: Vec<Tile> = view
            .statuses
            .iter()
            .map(|status| Tile {
                label: status.label.clone().into(),
                value: status.value.clone().into(),
            })
            .collect();
        self.stats.statuses.set_vec(statuses);
    }

    /// Whether the Stats page is the one on screen.
    pub(super) fn on_stats(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|window| window.get_page() == Page::Stats)
    }

    /// The arrows, Home and End change the period on the Stats page.
    pub(super) fn stats_key(&self, key: char) -> bool {
        let step = if key == char::from(Key::LeftArrow) {
            Move::Left
        } else if key == char::from(Key::RightArrow) {
            Move::Right
        } else if key == char::from(Key::Home) {
            Move::First
        } else if key == char::from(Key::End) {
            Move::Last
        } else {
            return false;
        };
        self.stats_step(step)
    }

    /// Left and Right (and the pad's) move along the period chips.
    pub(super) fn stats_step(&self, step: Move) -> bool {
        let periods = periods_of(&self.game.sessions.borrow());
        let at = periods
            .iter()
            .position(|period| *period == self.stats.period.get())
            .unwrap_or(0);
        let to = match step {
            Move::Left | Move::Up => at.saturating_sub(1),
            Move::Right | Move::Down => (at + 1).min(periods.len() - 1),
            Move::First | Move::PageUp => 0,
            Move::Last | Move::PageDown => periods.len() - 1,
        };
        self.stats.period.set(periods[to]);
        self.show_stats();
        true
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;

    use super::super::tests::page;
    use super::*;

    fn session(id: &str, start: u64, minutes: u64) -> sessions::Session {
        sessions::Session {
            id: id.to_owned(),
            title: id.to_owned(),
            start,
            minutes,
            offset: 0,
        }
    }

    #[test]
    fn a_page_with_no_sessions_is_empty_and_one_with_sessions_is_not() {
        let (window, glue) = page(1);
        let data = window.global::<StatsData>();
        glue.show_stats();
        assert!(data.get_empty());

        // Sessions in 2024 and 2025: the chips are this year, 2025, 2024, all.
        glue.game.sessions.borrow_mut().extend([
            session("a", 1_717_000_000, 30),
            session("b", 1_748_000_000, 90),
        ]);
        glue.stats.period.set(Period::All);
        glue.show_stats();
        assert!(!data.get_empty());
        assert!(!data.get_none_period());
        assert_eq!(data.get_periods().row_count(), 4);
        assert_eq!(data.get_tiles().row_data(1).unwrap().value, "2");
        assert_eq!(data.get_months().row_count(), 12);
        assert_eq!(data.get_weekdays().row_count(), 7);
        assert_eq!(data.get_top().row_count(), 2);
        assert_eq!(data.get_statuses().row_count(), 4);

        // This year has none.
        glue.stats.period.set(Period::Year(sessions::year_now()));
        glue.show_stats();
        assert!(data.get_none_period());
    }

    #[test]
    fn the_arrows_change_the_period_on_the_stats_page_only() {
        let (window, glue) = page(3);
        glue.game
            .sessions
            .borrow_mut()
            .push(session("a", 1_748_000_000, 30));
        let this = sessions::year_now();
        glue.stats.period.set(Period::Year(this));
        let right = String::from(char::from(Key::RightArrow));
        // On the Library the arrows move the cursor, not the period.
        glue.key(&right, false, false, false);
        assert_eq!(glue.stats.period.get(), Period::Year(this));

        window.invoke_page_requested(Page::Stats);
        assert!(glue.key(&right, false, false, false));
        assert_eq!(glue.stats.period.get(), Period::Year(2025));
        assert!(
            !glue.key(&String::from('x'), false, false, false),
            "other keys are not taken"
        );
    }

    #[test]
    fn left_and_right_move_along_the_periods_and_a_chip_chooses_one() {
        let (window, glue) = page(1);
        glue.game.sessions.borrow_mut().extend([
            session("a", 1_717_000_000, 30),
            session("a", 1_748_000_000, 30),
        ]);
        let this = sessions::year_now();
        glue.stats.period.set(Period::Year(this));
        glue.stats_step(Move::Right);
        assert_eq!(glue.stats.period.get(), Period::Year(2025));
        glue.stats_step(Move::Right);
        glue.stats_step(Move::Right);
        assert_eq!(glue.stats.period.get(), Period::All);
        glue.stats_step(Move::Right);
        assert_eq!(
            glue.stats.period.get(),
            Period::All,
            "no further than the last"
        );
        glue.stats_step(Move::Left);
        assert_eq!(glue.stats.period.get(), Period::Year(2024));
        glue.stats_step(Move::First);
        assert_eq!(glue.stats.period.get(), Period::Year(this));

        window.global::<StatsData>().invoke_period(1);
        assert_eq!(glue.stats.period.get(), Period::Year(2025));
        let chosen: Vec<bool> = window
            .global::<StatsData>()
            .get_periods()
            .iter()
            .map(|chip| chip.chosen)
            .collect();
        assert_eq!(chosen, [false, true, false, false]);
    }
}
