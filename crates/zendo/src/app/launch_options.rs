//! A game's launch options sheet, over its page: the Proton it runs with,
//! `MangoHud`, `GameMode`, gamescope, controllers as Xbox pads, and the
//! game's arguments and environment. Only for the games Zendo starts
//! itself; Steam and Heroic keep their own settings.

use std::fmt;

use iced::widget::{
    button, center, column, container, opaque, pick_list, row, scrollable, space, text, text_input,
    toggler,
};
use iced::{Alignment, Background, Color, Element, Length, Task};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::{Game, Launch, launch, proton};

use super::style::{self, BAD, BOLD, DIM};
use super::{Message, Zendo};

#[derive(Debug)]
pub struct Sheet {
    id: String,
    title: String,
    /// A Windows game Zendo installed: Proton and its controller switch
    /// apply.
    windows: bool,
    options: LaunchOptions,
    width: String,
    height: String,
    protons: Vec<ProtonChoice>,
    tools: Tools,
    error: Option<String>,
}

/// Which of the wrappers are installed.
#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one per program")]
struct Tools {
    mangohud: bool,
    mangoapp: bool,
    gamemode: bool,
    gamescope: bool,
}

impl Tools {
    fn find() -> Self {
        Self {
            mangohud: on_path("mangohud"),
            mangoapp: on_path("mangoapp"),
            gamemode: on_path("gamemoderun"),
            gamescope: on_path("gamescope"),
        }
    }
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// An entry of the Proton list: a build by name, or `None` for the
/// default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonChoice {
    name: Option<String>,
    label: String,
}

impl fmt::Display for ProtonChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone)]
pub enum SheetMessage {
    Open,
    Proton(ProtonChoice),
    MangoHud(bool),
    GameMode(bool),
    Gamescope(bool),
    Fullscreen(bool),
    Width(String),
    Height(String),
    XboxControllers(bool),
    Arguments(String),
    Environment(String),
    Save,
    Close,
}

/// Whether the game has launch options Zendo applies.
pub fn applies(game: &Game) -> bool {
    matches!(
        game.launch,
        Launch::Engine { .. } | Launch::Command { .. } | Launch::Local { .. }
    )
}

/// The Proton list: the default first, then each one installed, and the
/// one chosen even when it is gone.
fn proton_choices(chosen: Option<&str>) -> Vec<ProtonChoice> {
    let installed = proton::installed();
    let default = match proton::default() {
        Some(default) => format!("Default ({})", default.name),
        None => "Default (UMU-Proton, downloaded by umu)".to_owned(),
    };
    let mut choices = vec![ProtonChoice {
        name: None,
        label: default,
    }];
    choices.extend(installed.into_iter().map(|proton| ProtonChoice {
        label: proton.name.clone(),
        name: Some(proton.name),
    }));
    if let Some(chosen) = chosen
        && !choices
            .iter()
            .any(|choice| choice.name.as_deref() == Some(chosen))
    {
        choices.push(ProtonChoice {
            name: Some(chosen.to_owned()),
            label: format!("{chosen} (not installed; the default runs)"),
        });
    }
    choices
}

fn number(field: &str) -> Option<u32> {
    field.trim().parse().ok().filter(|&n| n > 0)
}

impl Zendo {
    pub(super) fn update_sheet(&mut self, message: SheetMessage) -> Task<Message> {
        if let SheetMessage::Open = message {
            return self.open_sheet();
        }
        let Some(sheet) = self.sheet.as_mut() else {
            return Task::none();
        };
        let options = &mut sheet.options;
        match message {
            SheetMessage::Open => {}
            SheetMessage::Proton(choice) => options.proton = choice.name,
            SheetMessage::MangoHud(on) => options.mangohud = on,
            SheetMessage::GameMode(on) => options.gamemode = on,
            SheetMessage::Gamescope(on) => options.gamescope = on,
            SheetMessage::Fullscreen(on) => options.gamescope_fullscreen = on,
            SheetMessage::Width(width) => {
                sheet.width = width.chars().filter(char::is_ascii_digit).collect();
            }
            SheetMessage::Height(height) => {
                sheet.height = height.chars().filter(char::is_ascii_digit).collect();
            }
            SheetMessage::XboxControllers(on) => options.xbox_controllers = on,
            SheetMessage::Arguments(arguments) => options.arguments = arguments,
            SheetMessage::Environment(environment) => options.environment = environment,
            SheetMessage::Save => return self.save_sheet(),
            SheetMessage::Close => self.sheet = None,
        }
        Task::none()
    }

    /// While the sheet is open, Escape closes it, Enter saves, and the
    /// page's own keys and the controller do nothing.
    pub(super) fn sheet_takes(&mut self, message: &Message) -> Option<Task<Message>> {
        self.sheet.as_ref()?;
        match message {
            Message::Back => Some(self.update_sheet(SheetMessage::Close)),
            Message::Activate => Some(self.update_sheet(SheetMessage::Save)),
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

    fn open_sheet(&mut self) -> Task<Message> {
        let Some(game) = self
            .current_id()
            .and_then(|id| self.game(&id))
            .filter(|game| applies(game))
            .cloned()
        else {
            return Task::none();
        };
        let options = if self.persist {
            match LaunchOptions::load(&game.id) {
                Ok(options) => options,
                Err(err) => {
                    self.status = Some(format!("{err:#}"));
                    return Task::none();
                }
            }
        } else {
            LaunchOptions::default()
        };
        let windows = matches!(game.launch, Launch::Engine { .. } | Launch::Local { .. })
            && launch::needs_proton(&game);
        self.sheet = Some(Sheet {
            id: game.id.clone(),
            title: game.title.clone(),
            windows,
            width: options
                .gamescope_width
                .map(|n| n.to_string())
                .unwrap_or_default(),
            height: options
                .gamescope_height
                .map(|n| n.to_string())
                .unwrap_or_default(),
            protons: if windows {
                proton_choices(options.proton.as_deref())
            } else {
                Vec::new()
            },
            options,
            tools: Tools::find(),
            error: None,
        });
        Task::none()
    }

    fn save_sheet(&mut self) -> Task<Message> {
        let Some(sheet) = self.sheet.as_mut() else {
            return Task::none();
        };
        let mut options = sheet.options.clone();
        options.gamescope_width = number(&sheet.width);
        options.gamescope_height = number(&sheet.height);
        if options.gamescope_width.is_some() != options.gamescope_height.is_some() {
            sheet.error = Some("Give gamescope both a width and a height, or neither".into());
            return Task::none();
        }
        let saved = if self.persist {
            options.save(&sheet.id)
        } else {
            options.check()
        };
        match saved {
            Ok(()) => {
                self.status = Some(format!("Saved {}'s launch options", sheet.title));
                self.sheet = None;
            }
            Err(err) => sheet.error = Some(format!("{err:#}")),
        }
        Task::none()
    }

    /// The sheet over the page, when it is open.
    pub(super) fn with_sheet<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        match &self.sheet {
            Some(sheet) => overlay(page, sheet_card(sheet)),
            None => page,
        }
    }
}

/// A card over the page, on a dimmed backdrop that takes the clicks.
pub(super) fn overlay<'a>(
    page: Element<'a, Message>,
    card: Element<'a, Message>,
) -> Element<'a, Message> {
    let backdrop = center(card).padding(32).style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.6))),
        ..container::Style::default()
    });
    iced::widget::stack![page, opaque(backdrop)].into()
}

fn sheet_card(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    let tools = sheet.tools;
    let mut body = column![
        text(format!("{} · Launch options", sheet.title))
            .size(20)
            .font(BOLD),
    ]
    .spacing(18);

    if sheet.windows {
        let selected = sheet
            .protons
            .iter()
            .find(|choice| choice.name == options.proton)
            .cloned();
        body = body.push(field(
            "Proton",
            "Runs the Windows game. GE-Proton carries the most fixes; Settings sets the default.",
            pick_list(sheet.protons.as_slice(), selected, |choice| {
                Message::Sheet(SheetMessage::Proton(choice))
            })
            .text_size(14)
            .padding([8, 12])
            .width(Length::Fill),
        ));
    }

    let mangohud_ready = if options.gamescope {
        tools.mangoapp
    } else {
        tools.mangohud
    };
    body = body.push(switch(
        "MangoHud",
        "An overlay with the frame rate, frame times and temperatures.",
        options.mangohud,
        mangohud_ready,
        SheetMessage::MangoHud,
    ));
    body = body.push(switch(
        "GameMode",
        "Asks the system for full performance while the game runs.",
        options.gamemode,
        tools.gamemode,
        SheetMessage::GameMode,
    ));
    body = body.push(switch(
        "Gamescope",
        "Runs the game in a compositor of its own: full screen, a size of your choice, \
         for games that will not go full screen themselves.",
        options.gamescope,
        tools.gamescope,
        SheetMessage::Gamescope,
    ));
    if options.gamescope {
        body = body.push(gamescope_options(sheet));
    }
    if sheet.windows {
        body = body.push(switch(
            "Controllers as Xbox pads",
            "For games that ignore a PlayStation or other controller. The game then gets no \
             touchpad, motion or adaptive triggers from it.",
            options.xbox_controllers,
            true,
            SheetMessage::XboxControllers,
        ));
    }
    body = body.push(text_fields(options));
    if let Some(error) = &sheet.error {
        body = body.push(text(error).size(13).color(BAD));
    }
    body = body.push(
        row![
            space::horizontal(),
            button(text("Cancel").size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Sheet(SheetMessage::Close)),
            button(text("Save").size(14).font(BOLD))
                .padding([10, 24])
                .style(style::primary)
                .on_press(Message::Sheet(SheetMessage::Save)),
        ]
        .spacing(10),
    );

    container(scrollable(body.padding(24)))
        .max_width(620)
        .style(style::card)
        .into()
}

/// Full screen and the size, under gamescope's switch.
fn gamescope_options(sheet: &Sheet) -> Element<'_, Message> {
    let size = row![
        text("Size").size(14).color(DIM).width(60),
        size_input("Width", &sheet.width, SheetMessage::Width),
        text("×").size(14).color(DIM),
        size_input("Height", &sheet.height, SheetMessage::Height),
        text("Empty: the game's own").size(13).color(DIM),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    column![
        toggler(sheet.options.gamescope_fullscreen)
            .label("Full screen")
            .text_size(14)
            .on_toggle(|on| Message::Sheet(SheetMessage::Fullscreen(on))),
        size,
    ]
    .spacing(12)
    .padding([0, 20])
    .into()
}

/// The game's arguments and environment.
fn text_fields(options: &LaunchOptions) -> Element<'_, Message> {
    column![
        field(
            "Arguments",
            "Passed to the game, as a shell splits them.",
            text_input("-windowed", &options.arguments)
                .on_input(|value| Message::Sheet(SheetMessage::Arguments(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
        field(
            "Environment",
            "NAME=value pairs for the game, separated by spaces.",
            text_input("DXVK_HUD=fps", &options.environment)
                .on_input(|value| Message::Sheet(SheetMessage::Environment(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
    ]
    .spacing(18)
    .into()
}

fn field<'a>(
    label: &'a str,
    about: &'a str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        text(label).size(15).font(BOLD),
        text(about).size(13).color(DIM),
        input.into(),
    ]
    .spacing(6)
    .into()
}

/// A switch, greyed out with a note when its program is missing (unless
/// already on, so it can be turned off).
fn switch<'a>(
    label: &'a str,
    about: &'a str,
    on: bool,
    installed: bool,
    message: fn(bool) -> SheetMessage,
) -> Element<'a, Message> {
    let mut about = about.to_owned();
    if !installed {
        about.push_str(" Not installed.");
    }
    let toggle = toggler(on).label(label).text_size(15);
    let toggle = if installed || on {
        toggle.on_toggle(move |on| Message::Sheet(message(on)))
    } else {
        toggle
    };
    column![toggle, text(about).size(13).color(DIM)]
        .spacing(4)
        .into()
}

fn size_input<'a>(
    placeholder: &'a str,
    value: &'a str,
    message: fn(String) -> SheetMessage,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |value| Message::Sheet(message(value)))
        .on_submit(Message::Sheet(SheetMessage::Save))
        .size(14)
        .padding([8, 12])
        .width(90)
        .style(style::input)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Page;
    use zendo_core::Source;

    fn zendo_with(game: Game) -> Zendo {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let id = game.id.clone();
        zendo.library = Some(zendo_core::Library {
            games: vec![game],
            problems: Vec::new(),
        });
        zendo.page = Page::Game(id);
        zendo
    }

    fn gog_game() -> Game {
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.platform = Some("windows".into());
        game
    }

    #[test]
    fn opens_saves_and_closes_over_the_page() {
        let mut zendo = zendo_with(gog_game());
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        let sheet = zendo.sheet.as_ref().expect("the sheet opened");
        assert!(sheet.windows);
        assert_eq!(sheet.protons[0].name, None, "the default comes first");
        // The page's keys do nothing under it.
        let _ = zendo.update(Message::Play);
        assert!(!zendo.running.is_running("gog:1"));
        let _ = zendo.update(Message::Sheet(SheetMessage::Environment("DXVK_HUD".into())));
        let _ = zendo.update(Message::Activate);
        let error = zendo.sheet.as_ref().and_then(|sheet| sheet.error.clone());
        assert_eq!(
            error.as_deref(),
            Some("\"DXVK_HUD\" in the environment is not NAME=value")
        );
        let _ = zendo.update(Message::Sheet(SheetMessage::Environment(
            "DXVK_HUD=fps".into(),
        )));
        let _ = zendo.update(Message::Sheet(SheetMessage::Gamescope(true)));
        let _ = zendo.update(Message::Sheet(SheetMessage::Width("1920".into())));
        let _ = zendo.update(Message::Activate);
        assert!(zendo.sheet.is_some(), "a width needs a height");
        let _ = zendo.update(Message::Sheet(SheetMessage::Height("10x80".into())));
        assert_eq!(
            zendo.sheet.as_ref().map(|sheet| sheet.height.as_str()),
            Some("1080")
        );
        let _ = zendo.update(Message::Activate);
        assert!(zendo.sheet.is_none());
        assert_eq!(
            zendo.status.as_deref(),
            Some("Saved Sugar's launch options")
        );

        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        let _ = zendo.update(Message::Back);
        assert!(zendo.sheet.is_none());
        assert_eq!(
            zendo.page,
            Page::Game("gog:1".into()),
            "Escape left the page be"
        );
    }

    #[test]
    fn steam_games_keep_steams_own_settings() {
        let game = Game::new(
            "steam:620".into(),
            "Portal 2".into(),
            Source::Steam,
            Launch::Uri {
                uri: "steam://rungameid/620".into(),
            },
        );
        let mut zendo = zendo_with(game);
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        assert!(zendo.sheet.is_none());
    }
}
