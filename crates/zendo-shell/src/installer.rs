//! A mod's FOMOD installer, without a toolkit: the steps shown one at a time,
//! each group's options as radios (one at most) or checkboxes, the option last
//! touched described with its picture, then Install builds the mod. Mods that
//! come while one is open, or while mods change, wait their turn.
//!
//! [`InstallerState`] holds the sheet and the queue and says, for each press,
//! what the window does next; [`InstallerState::view`] flattens the sheet for
//! drawing. The work
//! itself is `zendo_core::mods`' and `zendo_core::fomod`'s; building the mod
//! runs as `mods::Work::Install`.

use std::collections::HashMap;
use std::path::PathBuf;

use zendo_core::fomod::{GroupKind, OptionType, Selection};
use zendo_core::mods::Pending;

use crate::t;

/// The sheet and the mods that wait for it.
#[derive(Debug, Default)]
pub struct InstallerState {
    open: Option<Sheet>,
    /// Mods whose installer waits for the open one: the game, and the mod.
    waiting: Vec<(String, Box<Pending>)>,
}

#[derive(Debug)]
struct Sheet {
    game: String,
    pending: Box<Pending>,
    selection: Selection,
    /// The step shown, an index into the installer's steps.
    step: usize,
    /// The option last touched, described beside the list.
    focus: Option<(usize, usize)>,
}

/// A mod with its options chosen, to be built.
#[derive(Debug)]
pub struct Ready {
    pub game: String,
    pub pending: Box<Pending>,
    pub selection: Selection,
}

/// What opening an installer came to.
#[derive(Debug)]
pub enum Opening {
    /// Another installer is open, or mods are changing: it waits its turn.
    Waits,
    /// The sheet is open, and this is what the footer says.
    Shown(String),
    /// It has no step to show: it installs as it is.
    InstallNow(Ready),
}

/// A sheet closed without installing.
#[derive(Debug)]
pub struct Cancelled {
    pub status: String,
    /// The mod to drop: its unpacked files go.
    pub pending: Box<Pending>,
}

impl InstallerState {
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The game the open sheet is for.
    #[must_use]
    pub fn game(&self) -> Option<&str> {
        self.open.as_ref().map(|sheet| sheet.game.as_str())
    }

    /// How many installers wait.
    #[must_use]
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// Opens a mod's installer, or keeps it for after the one open. One with
    /// no step to show installs straight away. `mods_free` is false while
    /// mods are being changed, which must not overlap.
    pub fn open(&mut self, game: String, pending: Box<Pending>, mods_free: bool) -> Opening {
        if self.open.is_some() || !mods_free {
            self.waiting.push((game, pending));
            return Opening::Waits;
        }
        self.open_now(game, pending)
    }

    fn open_now(&mut self, game: String, pending: Box<Pending>) -> Opening {
        let selection = pending
            .installer
            .selection_from(&pending.chosen, &pending.present);
        let Some(&step) = pending
            .installer
            .visible_steps(&selection, &pending.present)
            .first()
        else {
            return Opening::InstallNow(Ready {
                game,
                pending,
                selection,
            });
        };
        let status = t!("installer-choose", name = pending.name.as_str());
        self.open = Some(Sheet {
            game,
            pending,
            selection,
            step,
            focus: None,
        });
        Opening::Shown(status)
    }

    /// The slot is free: the mod that waited longest opens. Nothing while a
    /// sheet is open or mods are changing.
    pub fn next_waiting(&mut self, mods_free: bool) -> Option<Opening> {
        if self.open.is_some() || !mods_free || self.waiting.is_empty() {
            return None;
        }
        let (game, pending) = self.waiting.remove(0);
        Some(self.open_now(game, pending))
    }

    /// An option clicked: its group and place, and on or off. A radio
    /// clicked again clears a group that may have none.
    pub fn pick(&mut self, group: usize, option: usize, on: bool) {
        let Some(sheet) = &mut self.open else {
            return;
        };
        let installer = &sheet.pending.installer;
        let Some(kind) = installer
            .steps
            .get(sheet.step)
            .and_then(|step| step.groups.get(group))
            .map(|group| group.kind)
        else {
            return;
        };
        let was = sheet.selection.is_on(sheet.step, group, option);
        let on = if kind == GroupKind::AtMostOne && was && on {
            false
        } else {
            on
        };
        installer.toggle(
            &mut sheet.selection,
            sheet.step,
            group,
            option,
            on,
            &sheet.pending.present,
        );
        sheet.focus = Some((group, option));
    }

    /// Back to the step before.
    pub fn back(&mut self) {
        if let Some(sheet) = &mut self.open
            && let Some(before) = sheet.neighbour(false)
        {
            sheet.step = before;
            sheet.focus = None;
        }
    }

    /// On to the next step; on the last one, the mod to build (the sheet
    /// closes).
    pub fn advance(&mut self) -> Option<Ready> {
        let sheet = self.open.as_mut()?;
        if let Some(after) = sheet.neighbour(true) {
            sheet.step = after;
            sheet.focus = None;
            return None;
        }
        let Sheet {
            game,
            pending,
            selection,
            ..
        } = self.open.take()?;
        Some(Ready {
            game,
            pending,
            selection,
        })
    }

    /// Moves the cursor by this many options in the step, round the ends.
    pub fn focus(&mut self, delta: isize) {
        let Some(sheet) = &mut self.open else {
            return;
        };
        let Some(step) = sheet.pending.installer.steps.get(sheet.step) else {
            return;
        };
        let positions: Vec<(usize, usize)> = step
            .groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| (0..group.options.len()).map(move |o| (g, o)))
            .collect();
        if positions.is_empty() {
            return;
        }
        let at = sheet
            .focus
            .and_then(|focus| positions.iter().position(|place| *place == focus))
            .unwrap_or(0);
        let len = isize::try_from(positions.len()).unwrap_or(1);
        let at = isize::try_from(at).unwrap_or(0);
        let next = usize::try_from((at + delta).rem_euclid(len)).unwrap_or(0);
        sheet.focus = Some(positions[next]);
    }

    /// Turns the focused option on or off, as a click would.
    pub fn pick_focused(&mut self) {
        let Some(sheet) = &self.open else {
            return;
        };
        let (g, o) = sheet.focus.unwrap_or((0, 0));
        let Some(group) = sheet
            .pending
            .installer
            .steps
            .get(sheet.step)
            .and_then(|step| step.groups.get(g))
        else {
            return;
        };
        if o >= group.options.len() {
            return;
        }
        let was = sheet.selection.is_on(sheet.step, g, o);
        let on = if group.kind == GroupKind::AtMostOne && was {
            false
        } else {
            !was
        };
        self.pick(g, o, on);
    }

    /// Enter: turns the focused option on or off, or goes on when none is
    /// focused.
    pub fn activate(&mut self) -> Option<Ready> {
        if self.open.as_ref()?.focus.is_some() {
            self.pick_focused();
            None
        } else {
            self.advance()
        }
    }

    /// Closes the sheet without installing.
    pub fn cancel(&mut self) -> Option<Cancelled> {
        let sheet = self.open.take()?;
        Some(Cancelled {
            status: t!("installer-cancelled", name = sheet.pending.name.as_str()),
            pending: sheet.pending,
        })
    }

    /// What the open sheet shows.
    #[must_use]
    pub fn view(&self) -> Option<View> {
        let sheet = self.open.as_ref()?;
        let installer = &sheet.pending.installer;
        let present = &sheet.pending.present;
        let shown = installer.visible_steps(&sheet.selection, present);
        let at = shown
            .iter()
            .position(|step| *step == sheet.step)
            .unwrap_or(0);
        let step = installer.steps.get(sheet.step)?;
        let types = installer.types(&sheet.selection, sheet.step, present);

        let groups: Vec<GroupView> = step
            .groups
            .iter()
            .enumerate()
            .map(|(g, group)| {
                let options = group
                    .options
                    .iter()
                    .enumerate()
                    .map(|(o, option)| {
                        let kind = types
                            .get(g)
                            .and_then(|types| types.get(o))
                            .copied()
                            .unwrap_or(OptionType::Optional);
                        let single = group.kind.single();
                        let enabled = if single {
                            kind != OptionType::NotUsable
                        } else {
                            !(matches!(kind, OptionType::Required | OptionType::NotUsable)
                                || group.kind == GroupKind::All)
                        };
                        OptionView {
                            label: label(&option.name, kind),
                            single,
                            on: sheet.selection.is_on(sheet.step, g, o),
                            enabled,
                            dim: single && kind == OptionType::NotUsable,
                            focused: sheet.focus == Some((g, o)),
                        }
                    })
                    .collect();
                GroupView {
                    title: format!("{} · {}", group.name, hint(group.kind)),
                    options,
                }
            })
            .collect();

        let focused = sheet
            .focus
            .or_else(|| (!step.groups.is_empty()).then_some((0, 0)))
            .and_then(|(g, o)| step.groups.get(g)?.options.get(o));
        let picture = focused
            .and_then(|option| option.image.as_deref())
            .or(installer.image.as_deref())
            .and_then(|path| sheet.pending.image(path));
        let about = About {
            picture,
            name: focused
                .map(|option| option.name.clone())
                .unwrap_or_default(),
            text: focused.map_or_else(String::new, |option| {
                if option.description.is_empty() {
                    t!("installer-no-description")
                } else {
                    option.description.clone()
                }
            }),
        };

        Some(View {
            title: if installer.name.is_empty() {
                sheet.pending.name.clone()
            } else {
                installer.name.clone()
            },
            step_line: t!(
                "installer-step",
                at = at + 1,
                steps = shown.len(),
                name = step.name.as_str()
            ),
            needs_warning: installer
                .needs
                .as_ref()
                .is_some_and(|needs| !needs.holds(&HashMap::new(), present)),
            groups,
            about,
            back_enabled: at > 0,
            final_step: at + 1 >= shown.len(),
        })
    }
}

impl Sheet {
    /// The step shown after (or before) this one.
    fn neighbour(&self, after: bool) -> Option<usize> {
        let shown = self
            .pending
            .installer
            .visible_steps(&self.selection, &self.pending.present);
        let at = shown.iter().position(|step| *step == self.step)?;
        if after {
            shown.get(at + 1).copied()
        } else {
            at.checked_sub(1)
                .and_then(|before| shown.get(before).copied())
        }
    }
}

/// What a sheet shows, ready to draw.
#[derive(Debug, Clone)]
pub struct View {
    pub title: String,
    /// "Step 1 of 2 · Name".
    pub step_line: String,
    /// The mod says it needs files that were not found.
    pub needs_warning: bool,
    pub groups: Vec<GroupView>,
    pub about: About,
    pub back_enabled: bool,
    /// Install, not Next.
    pub final_step: bool,
}

#[derive(Debug, Clone)]
pub struct GroupView {
    /// The group's name and its rule: "Name · choose one".
    pub title: String,
    pub options: Vec<OptionView>,
}

#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one independent flag per way the option is drawn"
)]
pub struct OptionView {
    pub label: String,
    /// A radio, else a checkbox.
    pub single: bool,
    pub on: bool,
    /// A click changes it.
    pub enabled: bool,
    /// Drawn as plain dim text: the mod says it cannot be used.
    pub dim: bool,
    /// The option last touched, or under the cursor.
    pub focused: bool,
}

/// The option last touched (or the step's first), or the mod: its picture
/// and description.
#[derive(Debug, Clone)]
pub struct About {
    pub picture: Option<PathBuf>,
    pub name: String,
    pub text: String,
}

fn hint(kind: GroupKind) -> String {
    match kind {
        GroupKind::ExactlyOne => t!("installer-exactly-one"),
        GroupKind::AtMostOne => t!("installer-at-most-one"),
        GroupKind::AtLeastOne => t!("installer-at-least-one"),
        GroupKind::All => t!("installer-all"),
        GroupKind::Any => t!("installer-any"),
    }
}

fn label(name: &str, kind: OptionType) -> String {
    match kind {
        OptionType::Required => t!("installer-required", name = name),
        OptionType::Recommended => t!("installer-recommended", name = name),
        OptionType::NotUsable => t!("installer-not-usable", name = name),
        OptionType::CouldBeUsable => t!("installer-may-not-work", name = name),
        OptionType::Optional => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::fomod::Installer;
    use zendo_core::mods;

    fn two_steps() -> Installer {
        Installer::parse(
            br#"<config><moduleName>Test</moduleName><installSteps order="Explicit">
              <installStep name="One"><optionalFileGroups>
                <group name="Maybe" type="SelectAtMostOne"><plugins>
                  <plugin name="A"><files><file source="a"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
              <installStep name="Two"><optionalFileGroups>
                <group name="Any" type="SelectAny"><plugins>
                  <plugin name="B"><files><file source="b"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
            </installSteps></config>"#,
        )
        .unwrap()
    }

    fn two_options() -> Installer {
        Installer::parse(
            br#"<config><moduleName>Test</moduleName><installSteps order="Explicit">
              <installStep name="One"><optionalFileGroups>
                <group name="Any" type="SelectAny"><plugins>
                  <plugin name="A"><files><file source="a"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                  <plugin name="B"><files><file source="b"/></files>
                    <typeDescriptor><type name="NotUsable"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
            </installSteps></config>"#,
        )
        .unwrap()
    }

    fn pending(installer: Installer) -> Box<Pending> {
        Box::new(mods::test_pending("Test", installer))
    }

    fn on(state: &InstallerState, step: usize, g: usize, o: usize) -> bool {
        state.open.as_ref().unwrap().selection.is_on(step, g, o)
    }

    #[test]
    fn steps_go_on_and_back_and_a_radio_clears_when_it_may() {
        let mut state = InstallerState::default();
        let first = state.open("epic:cat".into(), pending(two_steps()), true);
        assert!(matches!(first, Opening::Shown(_)));
        let second = state.open("epic:dog".into(), pending(two_steps()), true);
        assert!(matches!(second, Opening::Waits));
        assert_eq!(state.waiting(), 1, "the second waits");

        state.pick(0, 0, true);
        assert!(on(&state, 0, 0, 0));
        state.pick(0, 0, true);
        assert!(!on(&state, 0, 0, 0), "clicked again, it clears");

        assert!(state.advance().is_none());
        assert_eq!(state.open.as_ref().unwrap().step, 1);
        state.back();
        assert_eq!(state.open.as_ref().unwrap().step, 0);

        // Cancelled, the waiting one opens.
        let cancelled = state.cancel().unwrap();
        assert!(cancelled.status.contains("Test"));
        let Some(Opening::Shown(_)) = state.next_waiting(true) else {
            panic!("the waiting one opens");
        };
        assert_eq!(state.game(), Some("epic:dog"));
        assert_eq!(state.waiting(), 0);
    }

    #[test]
    fn the_last_step_hands_the_mod_over_and_closes() {
        let mut state = InstallerState::default();
        state.open("epic:cat".into(), pending(two_steps()), true);
        assert!(state.advance().is_none());
        let ready = state.advance().expect("the last step builds the mod");
        assert_eq!(ready.game, "epic:cat");
        assert!(!state.is_open());
    }

    #[test]
    fn the_arrows_focus_and_enter_picks() {
        let mut state = InstallerState::default();
        state.open("epic:cat".into(), pending(two_options()), true);
        assert_eq!(state.open.as_ref().unwrap().focus, None);
        state.focus(1);
        assert_eq!(state.open.as_ref().unwrap().focus, Some((0, 1)));
        state.focus(1);
        assert_eq!(state.open.as_ref().unwrap().focus, Some((0, 0)), "round");
        assert!(state.activate().is_none());
        assert!(on(&state, 0, 0, 0), "Enter picked the focused option");
        state.focus(-1);
        assert_eq!(state.open.as_ref().unwrap().focus, Some((0, 1)));
    }

    #[test]
    fn enter_with_nothing_focused_goes_on() {
        let mut state = InstallerState::default();
        state.open("epic:cat".into(), pending(two_steps()), true);
        assert!(state.activate().is_none());
        assert_eq!(state.open.as_ref().unwrap().step, 1);
    }

    #[test]
    fn an_installer_with_no_step_installs_at_once() {
        let empty = Installer::parse(
            br#"<config><moduleName>Test</moduleName><installSteps order="Explicit">
            </installSteps></config>"#,
        )
        .unwrap();
        let mut state = InstallerState::default();
        let opening = state.open("epic:cat".into(), pending(empty), true);
        assert!(matches!(opening, Opening::InstallNow(_)));
        assert!(!state.is_open());
    }

    #[test]
    fn a_mod_waits_while_mods_are_changing() {
        let mut state = InstallerState::default();
        let opening = state.open("epic:cat".into(), pending(two_steps()), false);
        assert!(matches!(opening, Opening::Waits));
        assert!(!state.is_open());
        assert!(state.next_waiting(false).is_none(), "still changing");
        assert!(matches!(state.next_waiting(true), Some(Opening::Shown(_))));
    }

    #[test]
    fn the_waiting_keep_their_order() {
        let mut state = InstallerState::default();
        state.open("one".into(), pending(two_steps()), true);
        state.open("two".into(), pending(two_steps()), true);
        state.open("three".into(), pending(two_steps()), true);
        state.cancel();
        // Mods are changing: nothing opens, and nothing is lost or moved.
        assert!(state.next_waiting(false).is_none());
        state.next_waiting(true);
        assert_eq!(state.game(), Some("two"));
        state.cancel();
        state.next_waiting(true);
        assert_eq!(state.game(), Some("three"));
    }

    #[test]
    fn the_view_names_each_option_and_what_a_click_may_change() {
        let mut state = InstallerState::default();
        assert!(state.view().is_none());
        state.open("epic:cat".into(), pending(two_options()), true);
        let view = state.view().unwrap();
        assert_eq!(view.title, "Test");
        assert_eq!(view.step_line, "Step 1 of 1 · One");
        assert!(view.final_step);
        assert!(!view.back_enabled);
        let options = &view.groups[0].options;
        assert_eq!(view.groups[0].title, "Any · choose any");
        assert_eq!(options[0].label, "A");
        assert!(options[0].enabled && !options[0].dim && !options[0].single);
        assert_eq!(options[1].label, "B (not usable)");
        assert!(!options[1].enabled, "a click cannot turn it on");
        // Nothing touched: the first option is described.
        assert_eq!(view.about.name, "A");
        assert_eq!(view.about.text, "No description.");
        state.focus(1);
        assert_eq!(state.view().unwrap().about.name, "B");
        assert!(state.view().unwrap().groups[0].options[1].focused);
    }

    #[test]
    fn a_step_line_counts_the_steps_shown() {
        let mut state = InstallerState::default();
        state.open("epic:cat".into(), pending(two_steps()), true);
        let view = state.view().unwrap();
        assert_eq!(view.step_line, "Step 1 of 2 · One");
        assert!(!view.final_step);
        assert!(view.groups[0].options[0].single);
    }
}
