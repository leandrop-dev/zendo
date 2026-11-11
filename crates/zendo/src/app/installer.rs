//! The FOMOD installer sheet: a mod's steps one at a time, each group's
//! options as radios (one at most) or checkboxes, the option last touched
//! described with its picture, then Install builds the mod. Mods that come
//! while one is open wait their turn. The work is `zendo_core::mods`' and
//! `zendo_core::fomod`'s.

use iced::widget::{
    button, checkbox, column, container, image, radio, row, scrollable, space, text,
};
use iced::{Alignment, ContentFit, Element, Length, Task};
use zendo_core::fomod::{GroupKind, OptionType, Selection};
use zendo_core::mods::{self, Pending};

use super::launch_options::overlay;
use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

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

#[derive(Debug, Clone)]
pub enum InstallerMessage {
    /// An option clicked: its group and place, and on or off.
    Pick(usize, usize, bool),
    Next,
    Back,
    Cancel,
    /// What came of clearing the cancelled mod's scratch folder.
    Cancelled(Result<(), String>),
}

impl Zendo {
    /// Opens a mod's installer, or keeps it for after the one open. One
    /// with no step to show installs straight away.
    pub(super) fn open_installer(&mut self, game: String, pending: Box<Pending>) -> Task<Message> {
        // One installer at a time, and no other mods change in flight: the
        // work waits for the slot rather than being dropped.
        if self.installer.open.is_some() || self.mods.busy.is_some() {
            self.installer.waiting.push((game, pending));
            return Task::none();
        }
        let selection = pending
            .installer
            .selection_from(&pending.chosen, &pending.present);
        let Some(&step) = pending
            .installer
            .visible_steps(&selection, &pending.present)
            .first()
        else {
            let note = t!("installer-installed", name = pending.name.as_str());
            return self.change_mods(game, t!("installer-installing"), move |game| {
                let overview = mods::install(game, &pending, &selection)?;
                Ok((overview, note))
            });
        };
        self.status = Some(t!("installer-choose", name = pending.name.as_str()));
        self.installer.open = Some(Sheet {
            game,
            pending,
            selection,
            step,
            focus: None,
        });
        Task::none()
    }

    pub(super) fn update_installer(&mut self, message: InstallerMessage) -> Task<Message> {
        let Some(sheet) = &mut self.installer.open else {
            return Task::none();
        };
        match message {
            InstallerMessage::Pick(group, option, on) => {
                let installer = &sheet.pending.installer;
                let present = &sheet.pending.present;
                let kind = installer.steps[sheet.step].groups[group].kind;
                let was = sheet.selection.is_on(sheet.step, group, option);
                // A radio clicked again clears a group that may have none.
                let on = if kind == GroupKind::AtMostOne && was && on {
                    false
                } else {
                    on
                };
                installer.toggle(&mut sheet.selection, sheet.step, group, option, on, present);
                sheet.focus = Some((group, option));
                Task::none()
            }
            InstallerMessage::Back => {
                if let Some(before) = sheet.neighbour(false) {
                    sheet.step = before;
                    sheet.focus = None;
                }
                Task::none()
            }
            InstallerMessage::Next => {
                if let Some(after) = sheet.neighbour(true) {
                    sheet.step = after;
                    sheet.focus = None;
                    return Task::none();
                }
                let Some(sheet) = self.installer.open.take() else {
                    return Task::none();
                };
                let Sheet {
                    game,
                    pending,
                    selection,
                    ..
                } = sheet;
                let note = t!("installer-installed", name = pending.name.as_str());
                let task =
                    self.change_mods(game.clone(), t!("installer-installing"), move |game| {
                        let overview = mods::install(game, &pending, &selection)?;
                        Ok((overview, note))
                    });
                Task::batch([task, self.next_installer()])
            }
            InstallerMessage::Cancel => {
                let mut task = Task::none();
                if let Some(sheet) = self.installer.open.take() {
                    self.status = Some(t!(
                        "installer-cancelled",
                        name = sheet.pending.name.as_str()
                    ));
                    if self.persist {
                        let pending = sheet.pending;
                        // Through `blocking`, as every other piece of work:
                        // a panic becomes a status line.
                        task = Task::perform(
                            blocking(move || {
                                mods::cancel_install(&pending);
                                Ok::<(), String>(())
                            }),
                            |result| Message::Installer(InstallerMessage::Cancelled(result)),
                        );
                    }
                }
                Task::batch([task, self.next_installer()])
            }
            InstallerMessage::Cancelled(result) => {
                if let Err(err) = result {
                    self.status = Some(err);
                }
                Task::none()
            }
        }
    }

    pub(super) fn next_installer(&mut self) -> Task<Message> {
        if self.installer.waiting.is_empty() {
            return Task::none();
        }
        let (game, pending) = self.installer.waiting.remove(0);
        self.open_installer(game, pending)
    }

    /// Escape cancels and Enter goes on, while the sheet is open.
    pub(super) fn installer_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.installer.open.as_ref()?;
        match message {
            Message::Back => Some(self.update_installer(InstallerMessage::Cancel)),
            Message::Activate => Some(self.update_installer(InstallerMessage::Next)),
            Message::Move(_)
            | Message::Type(_)
            | Message::Backspace
            | Message::Play
            | Message::Navigate(_)
            | Message::Clicked(_)
            | Message::ToggleHidden
            | Message::TogglePinned
            | Message::ShowHidden
            | Message::CycleSort
            | Message::FocusSearch
            | Message::PasteClipboard
            | Message::Pad(_) => Some(Task::none()),
            _ => None,
        }
    }

    pub(super) fn with_installer<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.installer.open {
            Some(sheet) => overlay(page, sheet_card(sheet)),
            None => page,
        }
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

fn sheet_card(sheet: &Sheet) -> Element<'_, Message> {
    let installer = &sheet.pending.installer;
    let present = &sheet.pending.present;
    let shown = installer.visible_steps(&sheet.selection, present);
    let at = shown
        .iter()
        .position(|step| *step == sheet.step)
        .unwrap_or(0);
    let final_step = at + 1 >= shown.len();
    let step = &installer.steps[sheet.step];

    let title = if installer.name.is_empty() {
        sheet.pending.name.clone()
    } else {
        installer.name.clone()
    };
    let mut header = column![
        text(title).size(20).font(BOLD),
        text(t!(
            "installer-step",
            at = at + 1,
            steps = shown.len(),
            name = step.name.as_str()
        ))
        .size(13)
        .color(colors().dim),
    ]
    .spacing(2);
    if installer
        .needs
        .as_ref()
        .is_some_and(|needs| !needs.holds(&std::collections::HashMap::new(), present))
    {
        header = header.push(text(t!("installer-needs")).size(13).color(colors().bad));
    }

    let list = option_list(sheet);
    let about = about_panel(sheet);
    let footer = row![
        button(text(t!("cancel")).size(14))
            .padding([8, 16])
            .style(style::secondary)
            .on_press(Message::Installer(InstallerMessage::Cancel)),
        space::horizontal(),
        button(text(t!("installer-back")).size(14))
            .padding([8, 16])
            .style(style::secondary)
            .on_press_maybe((at > 0).then_some(Message::Installer(InstallerMessage::Back))),
        button(
            text(if final_step {
                t!("installer-install")
            } else {
                t!("installer-next")
            })
            .size(14)
            .font(BOLD)
        )
        .padding([8, 22])
        .style(style::primary)
        .on_press(Message::Installer(InstallerMessage::Next)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(
        column![
            header,
            row![
                scrollable(list)
                    .height(Length::Fill)
                    .width(Length::FillPortion(3)),
                scrollable(about)
                    .height(Length::Fill)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(20)
            .height(Length::Fill),
            footer,
        ]
        .spacing(16),
    )
    .padding(24)
    .max_width(920)
    .max_height(640)
    .style(style::card)
    .into()
}

/// The step's groups and their options.
fn option_list(sheet: &Sheet) -> Element<'_, Message> {
    let installer = &sheet.pending.installer;
    let step = &installer.steps[sheet.step];
    let types = installer.types(&sheet.selection, sheet.step, &sheet.pending.present);
    let mut list = column![].spacing(16);
    for (g, group) in step.groups.iter().enumerate() {
        let picks = sheet.selection.group(sheet.step, g);
        let mut options = column![
            text(format!("{} · {}", group.name, hint(group.kind)))
                .size(14)
                .font(BOLD)
        ]
        .spacing(6);
        for (o, option) in group.options.iter().enumerate() {
            let kind = types[g][o];
            let name = label(&option.name, kind);
            let option_row: Element<'_, Message> =
                if group.kind.single() {
                    if kind == OptionType::NotUsable {
                        text(name).size(14).color(colors().dim).into()
                    } else {
                        let chosen = picks.iter().position(|pick| *pick);
                        radio(name, o, chosen, move |o| {
                            Message::Installer(InstallerMessage::Pick(g, o, true))
                        })
                        .size(16)
                        .text_size(14)
                        .into()
                    }
                } else {
                    let fixed = matches!(kind, OptionType::Required | OptionType::NotUsable)
                        || group.kind == GroupKind::All;
                    checkbox(picks[o])
                        .label(name)
                        .size(16)
                        .text_size(14)
                        .style(style::checkbox)
                        .on_toggle_maybe((!fixed).then_some(move |on| {
                            Message::Installer(InstallerMessage::Pick(g, o, on))
                        }))
                        .into()
                };
            options = options.push(option_row);
        }
        list = list.push(options);
    }

    list.into()
}

/// The option last touched, or the first of the step, or the mod: its
/// picture and description.
fn about_panel(sheet: &Sheet) -> Element<'_, Message> {
    let installer = &sheet.pending.installer;
    let step = &installer.steps[sheet.step];
    let focused = sheet
        .focus
        .or_else(|| (!step.groups.is_empty()).then_some((0, 0)))
        .and_then(|(g, o)| step.groups.get(g)?.options.get(o));
    let picture = focused
        .and_then(|option| option.image.as_deref())
        .or(installer.image.as_deref())
        .and_then(|path| sheet.pending.image(path));
    let mut about = column![].spacing(10);
    if let Some(picture) = picture {
        about = about.push(
            image(image::Handle::from_path(picture))
                .height(220)
                .content_fit(ContentFit::Contain),
        );
    }
    if let Some(option) = focused {
        about = about.push(text(&option.name).size(15).font(BOLD));
        about = about.push(
            text(if option.description.is_empty() {
                t!("installer-no-description")
            } else {
                option.description.clone()
            })
            .size(13)
            .color(colors().dim),
        );
    }

    about.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::fomod::Installer;

    #[test]
    fn steps_go_on_and_back_and_a_radio_clears_when_it_may() {
        let installer = Installer::parse(
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
        .unwrap();
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let pending = mods::test_pending("Test", installer);
        let _ = zendo.open_installer("epic:cat".into(), Box::new(pending.clone()));
        let _ = zendo.open_installer("epic:dog".into(), Box::new(pending));
        assert_eq!(zendo.installer.waiting.len(), 1, "the second waits");

        let pick = |on| Message::Installer(InstallerMessage::Pick(0, 0, on));
        let _ = zendo.update(pick(true));
        let picks = |zendo: &Zendo| {
            zendo
                .installer
                .open
                .as_ref()
                .unwrap()
                .selection
                .is_on(0, 0, 0)
        };
        assert!(picks(&zendo));
        let _ = zendo.update(pick(true));
        assert!(!picks(&zendo), "clicked again, it clears");

        let _ = zendo.update(Message::Activate);
        assert_eq!(zendo.installer.open.as_ref().unwrap().step, 1);
        let _ = zendo.update(Message::Installer(InstallerMessage::Back));
        assert_eq!(zendo.installer.open.as_ref().unwrap().step, 0);

        // Escape cancels; the waiting one opens.
        let _ = zendo.update(Message::Back);
        let open = zendo.installer.open.as_ref().unwrap();
        assert_eq!(open.game, "epic:dog");
        assert!(zendo.installer.waiting.is_empty());
    }
}
