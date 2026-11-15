//! The FOMOD installer sheet: a mod's steps one at a time, each group's
//! options as radios (one at most) or checkboxes, the option last touched
//! described with its picture, then Install builds the mod. Mods that come
//! while one is open wait their turn. The state and the words are
//! `zendo_shell::installer`'s; the work is `zendo_core::mods`' and
//! `zendo_core::fomod`'s.

use iced::widget::{
    button, checkbox, column, container, image, radio, row, scrollable, space, text,
};
use iced::{Alignment, ContentFit, Element, Length, Task};
use zendo_core::mods::{self, Pending};
use zendo_shell::installer::{Opening, Ready, View};

pub use zendo_shell::installer::InstallerState;

use super::launch_options::overlay;
use super::style::{self, BOLD, colors};
use super::{Message, Move, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Clone)]
pub enum InstallerMessage {
    /// An option clicked: its group and place, and on or off.
    Pick(usize, usize, bool),
    Next,
    Back,
    Cancel,
    /// Moves the cursor by this many options in the step.
    Focus(isize),
    /// What came of clearing the cancelled mod's scratch folder.
    Cancelled(Result<(), String>),
}

impl Zendo {
    /// Opens a mod's installer, or keeps it for after the one open. One
    /// with no step to show installs straight away.
    pub(super) fn open_installer(&mut self, game: String, pending: Box<Pending>) -> Task<Message> {
        // One installer at a time, and no other mods change in flight: the
        // work waits for the slot rather than being dropped.
        let free = self.mods.free();
        let opening = self.installer.open(game, pending, free);
        self.opened(opening)
    }

    fn opened(&mut self, opening: Opening) -> Task<Message> {
        match opening {
            Opening::Waits => Task::none(),
            Opening::Shown(status) => {
                self.status = Some(status);
                Task::none()
            }
            Opening::InstallNow(ready) => self.install_ready(ready),
        }
    }

    /// Builds the mod with the options chosen, on a thread.
    fn install_ready(&mut self, ready: Ready) -> Task<Message> {
        let Ready {
            game,
            pending,
            selection,
        } = ready;
        let ok = self.mods_runnable(&game);
        let job = self.mods.install(&game, pending, selection, ok);
        self.run_job(job)
    }

    pub(super) fn update_installer(&mut self, message: InstallerMessage) -> Task<Message> {
        match message {
            InstallerMessage::Pick(group, option, on) => {
                self.installer.pick(group, option, on);
                Task::none()
            }
            InstallerMessage::Back => {
                self.installer.back();
                Task::none()
            }
            InstallerMessage::Next => {
                let ready = self.installer.advance();
                self.installed(ready)
            }
            InstallerMessage::Focus(delta) => {
                self.installer.focus(delta);
                Task::none()
            }
            InstallerMessage::Cancel => {
                let mut task = Task::none();
                if let Some(cancelled) = self.installer.cancel() {
                    self.status = Some(cancelled.status);
                    if self.persist {
                        let pending = cancelled.pending;
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

    /// The last step's Install, if it was one: builds the mod, then lets
    /// the next waiting installer open once mods are free.
    fn installed(&mut self, ready: Option<Ready>) -> Task<Message> {
        let Some(ready) = ready else {
            return Task::none();
        };
        let task = self.install_ready(ready);
        Task::batch([task, self.next_installer()])
    }

    pub(super) fn next_installer(&mut self) -> Task<Message> {
        let free = self.mods.free();
        match self.installer.next_waiting(free) {
            Some(opening) => self.opened(opening),
            None => Task::none(),
        }
    }

    /// Escape cancels, arrows move the cursor, Enter picks and Right goes
    /// on, while the sheet is open.
    pub(super) fn installer_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        if !self.installer.is_open() {
            return None;
        }
        match message {
            Message::Back => Some(self.update_installer(InstallerMessage::Cancel)),
            Message::Activate => {
                let ready = self.installer.activate();
                Some(self.installed(ready))
            }
            Message::Move(Move::Up) => Some(self.update_installer(InstallerMessage::Focus(-1))),
            Message::Move(Move::Down) => Some(self.update_installer(InstallerMessage::Focus(1))),
            Message::Move(Move::Right) => Some(self.update_installer(InstallerMessage::Next)),
            Message::Move(Move::Left) => Some(self.update_installer(InstallerMessage::Back)),
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
        match self.installer.view() {
            Some(view) => overlay(page, sheet_card(&view)),
            None => page,
        }
    }
}

fn sheet_card<'a>(view: &View) -> Element<'a, Message> {
    let mut header = column![
        style::sheet_title(view.title.clone()),
        text(view.step_line.clone()).size(13).color(colors().dim),
    ]
    .spacing(style::SPACE_XS);
    if view.needs_warning {
        header = header.push(text(t!("installer-needs")).size(13).color(colors().bad));
    }

    let footer = row![
        button(style::button_text(t!("cancel")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press(Message::Installer(InstallerMessage::Cancel)),
        space::horizontal(),
        button(style::button_text(t!("installer-back")))
            .padding(style::BUTTON)
            .style(style::secondary)
            .on_press_maybe(
                view.back_enabled
                    .then_some(Message::Installer(InstallerMessage::Back))
            ),
        button(
            style::button_text(if view.final_step {
                t!("installer-install")
            } else {
                t!("installer-next")
            })
            .font(BOLD)
        )
        .padding([8, 22])
        .style(style::primary)
        .on_press(Message::Installer(InstallerMessage::Next)),
    ]
    .spacing(style::SPACE_M)
    .align_y(Alignment::Center);

    let list = option_list(view);
    let about = about_panel(view);
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
            .spacing(style::SPACE_XL)
            .height(Length::Fill),
            footer,
        ]
        .spacing(style::SPACE_L),
    )
    .padding(style::SPACE_XL)
    .max_width(920)
    .height(Length::Fill)
    .style(style::card)
    .into()
}

/// The step's groups and their options.
fn option_list<'a>(view: &View) -> Element<'a, Message> {
    let mut list = column![].spacing(style::SPACE_L);
    for (g, group) in view.groups.iter().enumerate() {
        let chosen = group.options.iter().position(|option| option.on);
        let mut options =
            column![text(group.title.clone()).size(14).font(BOLD)].spacing(style::SPACE_S);
        for (o, option) in group.options.iter().enumerate() {
            let name = option.label.clone();
            let option_row: Element<'a, Message> =
                if option.single {
                    if option.dim {
                        text(name).size(14).color(colors().dim).into()
                    } else {
                        radio(name, o, chosen, move |o| {
                            Message::Installer(InstallerMessage::Pick(g, o, true))
                        })
                        .size(16)
                        .text_size(14)
                        .into()
                    }
                } else {
                    checkbox(option.on)
                        .label(name)
                        .size(16)
                        .text_size(14)
                        .style(style::checkbox)
                        .on_toggle_maybe(option.enabled.then_some(move |on| {
                            Message::Installer(InstallerMessage::Pick(g, o, on))
                        }))
                        .into()
                };
            let focused = option.focused;
            options = options.push(container(option_row).padding([2, 4]).style(move |theme| {
                if focused {
                    style::card(theme)
                } else {
                    iced::widget::container::Style::default()
                }
            }));
        }
        list = list.push(options);
    }

    list.into()
}

/// The option last touched, or the first of the step, or the mod: its
/// picture and description.
fn about_panel<'a>(view: &View) -> Element<'a, Message> {
    let mut about = column![].spacing(style::SPACE_M);
    if let Some(picture) = &view.about.picture {
        about = about.push(
            image(image::Handle::from_path(picture))
                .height(220)
                .content_fit(ContentFit::Contain),
        );
    }
    if !view.about.name.is_empty() {
        about = about.push(text(view.about.name.clone()).size(15).font(BOLD));
        about = about.push(text(view.about.text.clone()).size(13).color(colors().dim));
    }

    about.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zendo_core::fomod::Installer;

    fn installer() -> Installer {
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

    /// The keys and buttons reach the shared sheet, and a second installer
    /// waits and opens when the first is cancelled.
    #[test]
    fn the_messages_reach_the_sheet_and_a_waiting_one_opens_after_cancel() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let pending = mods::test_pending("Test", installer());
        let _ = zendo.open_installer("epic:cat".into(), Box::new(pending.clone()));
        let _ = zendo.open_installer("epic:dog".into(), Box::new(pending));
        assert!(zendo.installer.is_open());

        let _ = zendo.update(Message::Installer(InstallerMessage::Pick(0, 0, true)));
        assert!(zendo.installer.view().unwrap().groups[0].options[0].on);
        let _ = zendo.update(Message::Move(Move::Right));
        assert_eq!(
            zendo.installer.view().unwrap().step_line,
            "Step 2 of 2 · Two"
        );
        let _ = zendo.update(Message::Move(Move::Left));
        assert!(!zendo.installer.view().unwrap().back_enabled);

        // Escape cancels; the waiting one opens.
        let _ = zendo.update(Message::Back);
        assert_eq!(zendo.installer.game(), Some("epic:dog"));
        assert_eq!(zendo.installer.waiting(), 0);
    }
}
