//! The Wine prefix card on the page of a Windows game Zendo starts: Wine's
//! settings, winetricks' window, components installed through winetricks,
//! a Windows program run in the prefix (an installer, a patch), and its
//! folder. One tool at a time per game, and none while the game runs; each
//! runs on a thread through `zendo_core::prefix_tools`.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, text, text_input};
use iced::{Alignment, Element, Length, Task};
use zendo_core::Game;
use zendo_core::gog_setup;
use zendo_core::known_fixes::{self, Fixes};
use zendo_core::launch;
use zendo_core::prefix_tools::{self, Ran, Tool};

use super::game::card;
use super::style::{self, colors};
use super::{Message, Zendo, blocking};
use crate::i18n::t;

#[derive(Debug, Default)]
pub struct PrefixState {
    /// What runs in each game's prefix now, by id.
    busy: HashMap<String, Tool>,
    /// The components typed to install.
    components: String,
    /// How each game's last tool went: the words, and whether it went well.
    notes: HashMap<String, (String, bool)>,
    /// Each game's known fixes, read as its page opens; `None` when the
    /// list has none for it.
    fixes: HashMap<String, Option<Fixes>>,
}

#[derive(Debug, Clone)]
pub enum PrefixMessage {
    Run(String, Tool),
    /// Run a program: choose it first.
    Pick(String),
    Picked(String, Result<Option<PathBuf>, String>),
    Components(String),
    Install(String),
    Done(String, Result<Ran, String>),
    /// The game's known fixes, read from the list kept.
    Fixes(String, Option<Fixes>),
}

/// What the card says runs.
fn label(tool: &Tool) -> String {
    match tool {
        Tool::WineSettings => t!("prefix-wine-settings"),
        Tool::Winetricks => "winetricks".to_owned(),
        Tool::Components(verbs) => verbs.join(" "),
        Tool::GogSetup => t!("prefix-gog-setup"),
        Tool::KnownFixes => t!("prefix-known-fixes"),
        Tool::Program(program) => program
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
    }
}

/// What a components run did: what it installed, and what was there.
fn installed(ran: &Ran) -> String {
    match (ran.installed.is_empty(), ran.already.is_empty()) {
        (_, true) => t!("prefix-installed", components = ran.installed.join(" ")),
        (true, false) => t!("prefix-all-there", components = ran.already.join(" ")),
        (false, false) => t!(
            "prefix-installed-some",
            components = ran.installed.join(" "),
            already = ran.already.join(" ")
        ),
    }
}

impl Zendo {
    pub(super) fn update_prefix(&mut self, message: PrefixMessage) -> Task<Message> {
        match message {
            PrefixMessage::Run(id, tool) => self.run_tool(id, tool),
            PrefixMessage::Pick(id) => {
                if !self.persist || !self.tools_free(&id) {
                    return Task::none();
                }
                let title = t!("prefix-choose-program");
                Task::perform(
                    blocking(move || {
                        crate::file_dialog::pick(crate::file_dialog::Kind::Installer, &title)
                            .map_err(|err| format!("{err:#}"))
                    }),
                    move |result| Message::Prefix(PrefixMessage::Picked(id.clone(), result)),
                )
            }
            PrefixMessage::Picked(id, Ok(Some(program))) => {
                self.run_tool(id, Tool::Program(program))
            }
            PrefixMessage::Picked(_, Ok(None)) => Task::none(),
            PrefixMessage::Picked(id, Err(err)) => {
                self.prefix.notes.insert(id, (err, false));
                Task::none()
            }
            PrefixMessage::Components(typed) => {
                self.prefix.components = typed;
                Task::none()
            }
            PrefixMessage::Install(id) => match prefix_tools::components(&self.prefix.components) {
                Ok(verbs) => self.run_tool(id, Tool::Components(verbs)),
                Err(err) => {
                    self.prefix.notes.insert(id, (format!("{err:#}"), false));
                    Task::none()
                }
            },
            PrefixMessage::Fixes(id, fixes) => {
                self.prefix.fixes.insert(id, fixes);
                Task::none()
            }
            PrefixMessage::Done(id, result) => {
                let Some(tool) = self.prefix.busy.remove(&id) else {
                    return Task::none();
                };
                let note = match (&tool, result) {
                    (Tool::Components(_), Ok(ran)) => {
                        self.prefix.components.clear();
                        (installed(&ran), true)
                    }
                    (tool, Ok(_)) => (t!("prefix-done", what = label(tool)), true),
                    (_, Err(err)) => (err, false),
                };
                self.prefix.notes.insert(id, note);
                Task::none()
            }
        }
    }

    /// Whether a tool may run in the game's prefix: none runs there, and
    /// the game does not.
    fn tools_free(&self, id: &str) -> bool {
        !self.prefix.busy.contains_key(id) && !self.running.is_running(id)
    }

    /// Reads the game's known fixes, on a thread, as its page opens (Heroic's
    /// list asked again once a day).
    pub(super) fn want_known_fixes(&mut self, id: &str) -> Task<Message> {
        let Some(game) = self
            .game(id)
            .filter(|game| known_fixes::applies(game))
            .cloned()
        else {
            return Task::none();
        };
        if !self.persist {
            return Task::none();
        }
        let id = id.to_owned();
        Task::perform(
            blocking(move || {
                let _ = known_fixes::refresh();
                known_fixes::for_game(&game)
            }),
            move |fixes| Message::Prefix(PrefixMessage::Fixes(id.clone(), fixes)),
        )
    }

    fn run_tool(&mut self, id: String, tool: Tool) -> Task<Message> {
        if !self.tools_free(&id) {
            return Task::none();
        }
        let Some(game) = self.game(&id).cloned() else {
            return Task::none();
        };
        self.prefix.busy.insert(id.clone(), tool.clone());
        self.prefix.notes.remove(&id);
        if !self.persist {
            return Task::none();
        }
        Task::perform(
            blocking(move || prefix_tools::run(&game, &tool).map_err(|err| format!("{err:#}"))),
            move |result| Message::Prefix(PrefixMessage::Done(id.clone(), result)),
        )
    }

    /// The card, for a Windows game Zendo starts.
    pub(super) fn prefix_card<'a>(&'a self, game: &'a Game) -> Option<Element<'a, Message>> {
        if !prefix_tools::applies(game) {
            return None;
        }
        let id = &game.id;
        let free = self.tools_free(id);
        let message = |message: PrefixMessage| free.then_some(Message::Prefix(message));
        let tool = |label: String, on: Option<Message>| {
            button(text(label).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press_maybe(on)
        };
        let folder = launch::prefix(game).filter(|prefix| prefix.is_dir());
        let mut tools = row![
            tool(
                t!("prefix-wine-settings"),
                message(PrefixMessage::Run(id.clone(), Tool::WineSettings))
            ),
            tool(
                "winetricks".to_owned(),
                message(PrefixMessage::Run(id.clone(), Tool::Winetricks))
            ),
            tool(
                t!("prefix-run-program"),
                message(PrefixMessage::Pick(id.clone()))
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if gog_setup::applies(game) {
            tools = tools.push(tool(
                t!("prefix-gog-setup"),
                message(PrefixMessage::Run(id.clone(), Tool::GogSetup)),
            ));
        }
        if let Some(folder) = folder {
            tools = tools.push(tool(
                t!("files-open-folder"),
                Some(Message::Open(folder.to_string_lossy().into_owned())),
            ));
        }
        let install = row![
            text_input(
                &t!("prefix-components-placeholder"),
                &self.prefix.components
            )
            .on_input(|typed| Message::Prefix(PrefixMessage::Components(typed)))
            .on_submit_maybe(message(PrefixMessage::Install(id.clone())))
            .padding([8, 12])
            .size(14)
            .style(style::input)
            .width(Length::Fill),
            tool(
                t!("prefix-install"),
                message(PrefixMessage::Install(id.clone()))
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let mut body = column![
            text(t!("prefix-about")).size(13).color(colors().dim),
            tools.wrap(),
            install,
        ]
        .spacing(12);
        if let Some(Some(fixes)) = self.prefix.fixes.get(id) {
            body = body.push(fixes_list(
                fixes,
                message(PrefixMessage::Run(id.clone(), Tool::KnownFixes)),
            ));
        }
        if let Some(tool) = self.prefix.busy.get(id) {
            body = body.push(
                text(t!("prefix-running", what = label(tool)))
                    .size(13)
                    .color(colors().dim),
            );
        } else if self.running.is_running(id) {
            body = body.push(text(t!("prefix-game-running")).size(13).color(colors().dim));
        } else if let Some((note, good)) = self.prefix.notes.get(id) {
            let color = if *good { colors().good } else { colors().bad };
            body = body.push(text(note).size(13).color(color));
        }
        Some(card(t!("prefix-title"), body.into()))
    }
}

/// The game's known fixes with why each is there, and a button to apply
/// them again.
fn fixes_list(fixes: &Fixes, apply: Option<Message>) -> Element<'_, Message> {
    let mut list = column![text(t!("prefix-known-fixes-title")).size(14)].spacing(4);
    let paths = fixes.run_in_prefix.iter();
    for fix in fixes.winetricks.iter().chain(paths) {
        let line = match fixes.notes.get(fix) {
            Some(note) => format!("{fix}: {note}"),
            None => fix.clone(),
        };
        list = list.push(text(line).size(13).color(colors().dim));
    }
    row![
        list.width(Length::Fill),
        button(text(t!("prefix-known-fixes")).size(14))
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe(apply),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use zendo_core::{Launch, Source};

    use super::*;

    fn with_game() -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        zendo
    }

    fn send(zendo: &mut Zendo, message: PrefixMessage) {
        let _ = zendo.update(Message::Prefix(message));
    }

    #[test]
    fn one_tool_at_a_time_and_says_how_it_went() {
        let mut zendo = with_game();
        send(
            &mut zendo,
            PrefixMessage::Run("gog:1".into(), Tool::WineSettings),
        );
        assert!(!zendo.tools_free("gog:1"));
        send(
            &mut zendo,
            PrefixMessage::Run("gog:1".into(), Tool::Winetricks),
        );
        assert_eq!(
            zendo.prefix.busy.get("gog:1"),
            Some(&Tool::WineSettings),
            "the second waits for the first"
        );
        send(
            &mut zendo,
            PrefixMessage::Done("gog:1".into(), Ok(Ran::default())),
        );
        assert!(zendo.tools_free("gog:1"));
        assert_eq!(
            zendo.prefix.notes.get("gog:1"),
            Some(&("Wine settings ended".to_owned(), true))
        );
    }

    #[test]
    fn installs_the_components_typed() {
        let mut zendo = with_game();
        send(&mut zendo, PrefixMessage::Components("vcrun2022 -q".into()));
        send(&mut zendo, PrefixMessage::Install("gog:1".into()));
        assert!(zendo.tools_free("gog:1"), "an option is refused");
        assert!(matches!(zendo.prefix.notes.get("gog:1"), Some((_, false))));
        send(
            &mut zendo,
            PrefixMessage::Components("vcrun2022 d3dx9".into()),
        );
        send(&mut zendo, PrefixMessage::Install("gog:1".into()));
        assert_eq!(
            zendo.prefix.busy.get("gog:1"),
            Some(&Tool::Components(vec!["vcrun2022".into(), "d3dx9".into()]))
        );
        let ran = Ran {
            installed: vec!["vcrun2022".into()],
            already: vec!["d3dx9".into()],
        };
        send(&mut zendo, PrefixMessage::Done("gog:1".into(), Ok(ran)));
        assert_eq!(
            zendo.prefix.notes.get("gog:1"),
            Some(&(
                "Installed vcrun2022; d3dx9 was there already".to_owned(),
                true
            ))
        );
        assert_eq!(zendo.prefix.components, "");
    }

    #[test]
    fn keeps_the_fixes_read_and_applies_them_as_a_tool() {
        let mut zendo = with_game();
        let fixes = Fixes {
            winetricks: vec!["vcrun2019".into()],
            ..Fixes::default()
        };
        send(
            &mut zendo,
            PrefixMessage::Fixes("gog:1".into(), Some(fixes.clone())),
        );
        assert_eq!(zendo.prefix.fixes.get("gog:1"), Some(&Some(fixes)));
        send(
            &mut zendo,
            PrefixMessage::Run("gog:1".into(), Tool::KnownFixes),
        );
        assert_eq!(zendo.prefix.busy.get("gog:1"), Some(&Tool::KnownFixes));
        send(
            &mut zendo,
            PrefixMessage::Done("gog:1".into(), Err("vcrun2019 failed".into())),
        );
        assert_eq!(
            zendo.prefix.notes.get("gog:1"),
            Some(&("vcrun2019 failed".to_owned(), false))
        );
    }
}
