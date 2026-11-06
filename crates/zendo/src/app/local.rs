//! Games added by hand: the "Add game" sheet over the library, the same
//! sheet to edit one from its page, and Remove there, which takes it out
//! of the library and leaves its files and prefix.

use std::path::{Path, PathBuf};

use iced::widget::{button, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::local::{self, LocalGame};

use super::launch_options::overlay;
use super::style::{self, BAD, BOLD, DIM};
use super::{Message, Page, Zendo, blocking};

#[derive(Debug, Default)]
pub struct LocalState {
    form: Option<Form>,
    /// The game whose Remove was pressed once.
    confirm_remove: Option<String>,
    /// Once the library is read again: the game to open, if any, and what
    /// to say.
    pub(super) after_scan: Option<(Option<String>, String)>,
}

#[derive(Debug, Default)]
struct Form {
    /// The game being edited; `None` for a new one.
    id: Option<String>,
    title: String,
    program: String,
    arguments: String,
    picking: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LocalMessage {
    Add,
    Edit(String),
    Title(String),
    Program(String),
    Arguments(String),
    Pick,
    Picked(Result<Option<PathBuf>, String>),
    Save,
    Close,
    Remove(String),
}

/// A title from the program's folder (`Cave Story/Doukutsu.exe` is "Cave
/// Story"), or its own name when the folder says nothing.
fn guess_title(program: &Path) -> String {
    const PLAIN: [&str; 8] = [
        "bin", "bin64", "x64", "x86", "win64", "binaries", "game", "games",
    ];
    let folder = program
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !PLAIN.contains(&name.to_lowercase().as_str()));
    folder
        .or_else(|| {
            program
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

impl Zendo {
    pub(super) fn update_local(&mut self, message: LocalMessage) -> Task<Message> {
        match message {
            LocalMessage::Add => {
                self.local.form = Some(Form::default());
                Task::none()
            }
            LocalMessage::Edit(id) => {
                let found = if self.persist {
                    local::find(&id)
                } else {
                    Ok(None)
                };
                match found {
                    Ok(Some(game)) => {
                        self.local.form = Some(Form {
                            id: Some(game.id),
                            title: game.title,
                            program: game.program.display().to_string(),
                            arguments: game.arguments,
                            ..Form::default()
                        });
                    }
                    Ok(None) => self.status = Some("Zendo has no record of this game".into()),
                    Err(err) => self.status = Some(format!("{err:#}")),
                }
                Task::none()
            }
            LocalMessage::Remove(id) => self.remove_local(id),
            message => self.update_form(message),
        }
    }

    fn update_form(&mut self, message: LocalMessage) -> Task<Message> {
        let Some(form) = self.local.form.as_mut() else {
            return Task::none();
        };
        match message {
            LocalMessage::Title(title) => form.title = title,
            LocalMessage::Program(program) => form.program = program,
            LocalMessage::Arguments(arguments) => form.arguments = arguments,
            LocalMessage::Pick => {
                if form.picking || !self.persist {
                    return Task::none();
                }
                form.picking = true;
                return Task::perform(
                    blocking(|| {
                        crate::file_dialog::pick("Choose the game's program")
                            .map_err(|err| format!("{err:#}"))
                    }),
                    |result| Message::Local(LocalMessage::Picked(result)),
                );
            }
            LocalMessage::Picked(result) => {
                form.picking = false;
                match result {
                    Ok(Some(path)) => {
                        if form.title.trim().is_empty() {
                            form.title = guess_title(&path);
                        }
                        form.program = path.display().to_string();
                        form.error = None;
                    }
                    Ok(None) => {}
                    Err(err) => form.error = Some(err),
                }
            }
            LocalMessage::Save => return self.save_form(),
            LocalMessage::Close => self.local.form = None,
            LocalMessage::Add | LocalMessage::Edit(_) | LocalMessage::Remove(_) => {}
        }
        Task::none()
    }

    fn save_form(&mut self) -> Task<Message> {
        let Some(form) = self.local.form.as_mut() else {
            return Task::none();
        };
        let game = LocalGame {
            id: form.id.clone().unwrap_or_default(),
            title: form.title.clone(),
            program: PathBuf::from(form.program.trim()),
            arguments: form.arguments.trim().to_owned(),
        };
        let saved = if self.persist {
            local::put(game)
        } else {
            game.check().map(|()| "local:test".to_owned())
        };
        match saved {
            Ok(id) => {
                let added = form.id.is_none();
                let title = form.title.trim().to_owned();
                self.local.form = None;
                let note = if added {
                    format!("Added {title}")
                } else {
                    format!("Saved {title}")
                };
                self.local.after_scan = Some((Some(id), note));
                self.refresh()
            }
            Err(err) => {
                form.error = Some(format!("{err:#}"));
                Task::none()
            }
        }
    }

    /// Remove, pressed once to ask and again to take the game out.
    fn remove_local(&mut self, id: String) -> Task<Message> {
        if self.local.confirm_remove.as_deref() != Some(id.as_str()) {
            self.local.confirm_remove = Some(id);
            return Task::none();
        }
        self.local.confirm_remove = None;
        let removed = if self.persist {
            local::remove(&id)
        } else {
            Ok(())
        };
        match removed {
            Ok(()) => {
                let title = self
                    .game(&id)
                    .map_or_else(|| id.clone(), |game| game.title.clone());
                let note =
                    format!("Removed {title} from the library; its files are where they were");
                self.local.after_scan = Some((None, note.clone()));
                let back = self.navigate(Page::Library);
                self.status = Some(note);
                Task::batch([back, self.refresh()])
            }
            Err(err) => {
                self.status = Some(format!("{err:#}"));
                Task::none()
            }
        }
    }

    /// While the sheet is open, Escape closes it, Enter saves, and the
    /// page's own keys and the controller do nothing.
    pub(super) fn form_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.local.form.as_ref()?;
        match message {
            Message::Back => Some(self.update_local(LocalMessage::Close)),
            Message::Activate => Some(self.update_local(LocalMessage::Save)),
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

    pub(super) fn with_form<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.local.form {
            Some(form) => overlay(page, form_card(form)),
            None => page,
        }
    }

    /// Edit and Remove, on the page of a game added by hand.
    pub(super) fn local_buttons<'a>(&self, game: &'a zendo_core::Game) -> Element<'a, Message> {
        if game.source != zendo_core::Source::Local {
            return space::horizontal().width(0).into();
        }
        let confirming = self.local.confirm_remove.as_deref() == Some(game.id.as_str());
        row![
            button(text("Edit").size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Local(LocalMessage::Edit(game.id.clone()))),
            button(
                text(if confirming {
                    "Really remove"
                } else {
                    "Remove"
                })
                .size(14)
            )
            .padding([10, 18])
            .style(if confirming {
                style::danger
            } else {
                style::secondary
            })
            .on_press(Message::Local(LocalMessage::Remove(game.id.clone()))),
        ]
        .spacing(10)
        .into()
    }
}

fn form_card(form: &Form) -> Element<'_, Message> {
    let heading = if form.id.is_some() {
        "Edit game"
    } else {
        "Add a game"
    };
    let windows = local::is_windows(Path::new(form.program.trim()));
    let about = if form.program.trim().is_empty() {
        "A game you installed yourself: its .exe for a Windows game, which runs through \
         Proton in a prefix of its own, or the program for a Linux one."
    } else if windows {
        "A Windows game: it runs through umu-launcher and Proton, in a prefix of its own."
    } else {
        "A Linux program: it runs as it is."
    };
    let mut body = column![
        text(heading).size(20).font(BOLD),
        text(about).size(13).color(DIM),
        field(
            "Program",
            row![
                text_input("/home/you/Games/Some Game/Game.exe", &form.program)
                    .on_input(|value| Message::Local(LocalMessage::Program(value)))
                    .on_submit(Message::Local(LocalMessage::Save))
                    .size(14)
                    .padding([8, 12])
                    .style(style::input),
                button(text(if form.picking { "Choosing…" } else { "Choose…" }).size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe((!form.picking).then_some(Message::Local(LocalMessage::Pick))),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ),
        field(
            "Title",
            text_input("Some Game", &form.title)
                .on_input(|value| Message::Local(LocalMessage::Title(value)))
                .on_submit(Message::Local(LocalMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
        field(
            "Arguments",
            text_input("Passed to the game, as a shell splits them", &form.arguments)
                .on_input(|value| Message::Local(LocalMessage::Arguments(value)))
                .on_submit(Message::Local(LocalMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
        text("Its cover comes from SteamGridDB by its title when Settings has a key, and Change art on its page picks another. Proton, MangoHud and the rest are in its launch options.")
            .size(13)
            .color(DIM),
    ]
    .spacing(18);
    if let Some(error) = &form.error {
        body = body.push(text(error).size(13).color(BAD));
    }
    body = body.push(
        row![
            space::horizontal(),
            button(text("Cancel").size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Local(LocalMessage::Close)),
            button(
                text(if form.id.is_some() { "Save" } else { "Add" })
                    .size(14)
                    .font(BOLD)
            )
            .padding([10, 24])
            .style(style::primary)
            .on_press(Message::Local(LocalMessage::Save)),
        ]
        .spacing(10),
    );
    container(scrollable(body.padding(24)))
        .max_width(620)
        .style(style::card)
        .into()
}

fn field<'a>(label: &'a str, input: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![text(label).size(15).font(BOLD), input.into()]
        .spacing(6)
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_a_title_from_the_folder() {
        assert_eq!(
            guess_title(Path::new("/g/Cave Story/Doukutsu.exe")),
            "Cave Story"
        );
        assert_eq!(
            guess_title(Path::new("/g/Celeste/bin/Celeste.exe")),
            "Celeste"
        );
    }

    #[test]
    fn adds_checks_and_removes_with_a_second_press() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::Local(LocalMessage::Add));
        assert!(zendo.local.form.is_some());
        let _ = zendo.update(Message::Play);
        let _ = zendo.update(Message::Local(LocalMessage::Program(
            "/nowhere/Game.exe".into(),
        )));
        let _ = zendo.update(Message::Activate);
        let error = zendo
            .local
            .form
            .as_ref()
            .and_then(|form| form.error.clone());
        assert!(error.is_some_and(|error| error.contains("Give the game a title")));
        let _ = zendo.update(Message::Back);
        assert!(zendo.local.form.is_none());

        let _ = zendo.update(Message::Local(LocalMessage::Remove("local:x".into())));
        assert_eq!(zendo.local.confirm_remove.as_deref(), Some("local:x"));
        let _ = zendo.update(Message::Local(LocalMessage::Remove("local:x".into())));
        assert!(zendo.local.confirm_remove.is_none());
        assert_eq!(zendo.page, Page::Library);
    }
}
