//! A game's launch options sheet, over its page: the Proton it runs with,
//! `MangoHud`, `GameMode`, gamescope, NVIDIA offload, Proton's switches
//! (controllers as Xbox pads, Wine's Wayland driver, HDR, FSR, `WoW64`,
//! esync and fsync off, DXVK's counter and frame cap), and the game's
//! arguments and environment. Only for the games Zendo starts itself;
//! Steam and Heroic keep their own settings.

use std::fmt;

use iced::widget::{
    button, center, column, container, opaque, pick_list, row, scrollable, space, text, text_input,
    toggler,
};
use iced::{Alignment, Background, Color, Element, Length, Task};
use zendo_core::launch_options::{self, LaunchOptions};
use zendo_core::{Game, Launch, launch, proton};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo};
use crate::i18n::t;

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
    frame_cap: String,
    protons: Vec<ProtonChoice>,
    /// The build the game would run with, and the switches' variables its
    /// script never reads.
    build: Option<(String, Vec<&'static str>)>,
    tools: Tools,
    error: Option<String>,
}

/// The variables of Proton's switches its script reads, checked against
/// the build chosen; the others (DXVK's, Wine's) the builds share.
const CHECKED: [&str; 5] = [
    "PROTON_PREFER_SDL",
    "PROTON_NO_ESYNC",
    "PROTON_NO_FSYNC",
    "PROTON_ENABLE_WAYLAND",
    "PROTON_USE_WOW64",
];

/// The build a choice runs, and what of [`CHECKED`] it ignores; none when
/// umu would download its own.
fn build_for(choice: Option<&str>) -> Option<(String, Vec<&'static str>)> {
    let proton = choice.and_then(proton::named).or_else(proton::default)?;
    let ignored = proton.ignores(&CHECKED);
    Some((proton.name, ignored))
}

/// FSR's sharpening, as the list offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sharpening(u8);

impl Sharpening {
    fn all() -> Vec<Self> {
        (0..=launch_options::MOST_SOFT).map(Self).collect()
    }
}

impl fmt::Display for Sharpening {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.0 {
            0 => t!("options-fsr-sharpest"),
            launch_options::MOST_SOFT => t!("options-fsr-softest"),
            launch_options::WINE_SHARPENING => t!("options-fsr-default"),
            n => n.to_string(),
        };
        f.write_str(&label)
    }
}

/// Whether the computer has an NVIDIA card's driver and another card,
/// which render offload needs.
fn offload_possible() -> bool {
    let cards = std::fs::read_dir("/sys/class/drm").map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.strip_prefix("card")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
            .count()
    });
    cards > 1 && std::path::Path::new("/sys/module/nvidia").is_dir()
}

/// Which of the wrappers are installed.
#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one per program")]
struct Tools {
    mangohud: bool,
    mangoapp: bool,
    gamemode: bool,
    gamescope: bool,
    /// Not a program: an NVIDIA card beside another.
    offload: bool,
}

impl Tools {
    fn find() -> Self {
        Self {
            mangohud: on_path("mangohud"),
            mangoapp: on_path("mangoapp"),
            gamemode: on_path("gamemoderun"),
            gamescope: on_path("gamescope"),
            offload: offload_possible(),
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
    NoSync(bool),
    Wayland(bool),
    Hdr(bool),
    Fsr(bool),
    FsrSharpening(Sharpening),
    Wow64(bool),
    FpsCounter(bool),
    FrameCap(String),
    NvidiaOffload(bool),
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
        Some(default) => t!("options-proton-default", name = default.name.as_str()),
        None => t!("options-proton-default-umu"),
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
            label: t!("options-proton-missing", name = chosen),
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
            SheetMessage::Proton(choice) => {
                sheet.build = build_for(choice.name.as_deref());
                sheet.options.proton = choice.name;
            }
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
            SheetMessage::NoSync(on) => options.no_sync = on,
            SheetMessage::Wayland(on) => options.wayland = on,
            SheetMessage::Hdr(on) => options.hdr = on,
            SheetMessage::Fsr(on) => options.fsr = on,
            SheetMessage::FsrSharpening(Sharpening(n)) => {
                options.fsr_strength = (n != launch_options::WINE_SHARPENING).then_some(n);
            }
            SheetMessage::Wow64(on) => options.wow64 = on,
            SheetMessage::FpsCounter(on) => options.fps_counter = on,
            SheetMessage::FrameCap(cap) => {
                sheet.frame_cap = cap.chars().filter(char::is_ascii_digit).collect();
            }
            SheetMessage::NvidiaOffload(on) => options.nvidia_offload = on,
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
            frame_cap: options.frame_cap.map(|n| n.to_string()).unwrap_or_default(),
            protons: if windows {
                proton_choices(options.proton.as_deref())
            } else {
                Vec::new()
            },
            build: if windows {
                build_for(options.proton.as_deref())
            } else {
                None
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
        options.frame_cap = number(&sheet.frame_cap);
        if options.gamescope_width.is_some() != options.gamescope_height.is_some() {
            sheet.error = Some(t!("options-size-both"));
            return Task::none();
        }
        let saved = if self.persist {
            options.save(&sheet.id)
        } else {
            options.check()
        };
        match saved {
            Ok(()) => {
                self.status = Some(t!("options-saved", title = sheet.title.as_str()));
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
        text(t!("options-title", title = sheet.title.as_str()))
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
            "Proton".to_owned(),
            t!("options-proton-about"),
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
    let missing = |installed: bool| (!installed).then(|| t!("options-not-installed"));
    body = body.push(switch(
        "MangoHud".to_owned(),
        t!("options-mangohud-about"),
        options.mangohud,
        missing(mangohud_ready),
        SheetMessage::MangoHud,
    ));
    body = body.push(switch(
        "GameMode".to_owned(),
        t!("options-gamemode-about"),
        options.gamemode,
        missing(tools.gamemode),
        SheetMessage::GameMode,
    ));
    body = body.push(switch(
        "Gamescope".to_owned(),
        t!("options-gamescope-about"),
        options.gamescope,
        missing(tools.gamescope),
        SheetMessage::Gamescope,
    ));
    if options.gamescope {
        body = body.push(gamescope_options(sheet));
    }
    body = body.push(switch(
        t!("options-offload"),
        t!("options-offload-about"),
        options.nvidia_offload,
        (!tools.offload).then(|| t!("options-offload-none")),
        SheetMessage::NvidiaOffload,
    ));
    if sheet.windows {
        body = body.push(proton_switches(sheet));
    }
    body = body.push(text_fields(options));
    if let Some(error) = &sheet.error {
        body = body.push(text(error).size(13).color(colors().bad));
    }
    body = body.push(
        row![
            space::horizontal(),
            button(text(t!("cancel")).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::Sheet(SheetMessage::Close)),
            button(text(t!("save")).size(14).font(BOLD))
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

/// The switches Proton, Wine and DXVK read, for a Windows game.
fn proton_switches(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    // What the chosen build does not read, said under its switch.
    let ignored = |names: &[&str]| -> Option<String> {
        let (build, ignored) = sheet.build.as_ref()?;
        let unread: Vec<&str> = names
            .iter()
            .copied()
            .filter(|name| ignored.contains(name))
            .collect();
        (!unread.is_empty()).then(|| {
            t!(
                "options-build-ignores",
                build = build.as_str(),
                names = unread.join(", ")
            )
        })
    };
    let note = |about: String, names: &[&str]| match ignored(names) {
        Some(unread) => format!("{about} {unread}"),
        None => about,
    };
    let mut switches = column![
        text(t!("options-proton-switches")).size(15).font(BOLD),
        switch(
            t!("options-xbox"),
            note(t!("options-xbox-about"), &["PROTON_PREFER_SDL"]),
            options.xbox_controllers,
            None,
            SheetMessage::XboxControllers,
        ),
        switch(
            t!("options-wayland"),
            note(t!("options-wayland-about"), &["PROTON_ENABLE_WAYLAND"]),
            options.wayland,
            None,
            SheetMessage::Wayland,
        ),
        switch(
            "HDR".to_owned(),
            t!("options-hdr-about"),
            options.hdr,
            None,
            SheetMessage::Hdr,
        ),
        switch(
            "FSR".to_owned(),
            t!("options-fsr-about"),
            options.fsr,
            None,
            SheetMessage::Fsr,
        ),
    ]
    .spacing(18);
    if options.fsr {
        switches = switches.push(fsr_sharpening(options));
    }
    switches
        .push(switch(
            t!("options-fps-counter"),
            t!("options-fps-counter-about"),
            options.fps_counter,
            None,
            SheetMessage::FpsCounter,
        ))
        .push(field(
            t!("options-frame-cap"),
            t!("options-frame-cap-about"),
            text_input(&t!("options-frame-cap-empty"), &sheet.frame_cap)
                .on_input(|value| Message::Sheet(SheetMessage::FrameCap(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .width(160)
                .style(style::input),
        ))
        .push(switch(
            "WoW64".to_owned(),
            note(t!("options-wow64-about"), &["PROTON_USE_WOW64"]),
            options.wow64,
            None,
            SheetMessage::Wow64,
        ))
        .push(switch(
            t!("options-no-sync"),
            note(
                t!("options-no-sync-about"),
                &["PROTON_NO_ESYNC", "PROTON_NO_FSYNC"],
            ),
            options.no_sync,
            None,
            SheetMessage::NoSync,
        ))
        .into()
}

/// FSR's sharpening, under its switch.
fn fsr_sharpening(options: &LaunchOptions) -> Element<'_, Message> {
    let chosen = Sharpening(
        options
            .fsr_strength
            .unwrap_or(launch_options::WINE_SHARPENING),
    );
    row![
        text(t!("options-fsr-sharpening"))
            .size(14)
            .color(colors().dim),
        pick_list(Sharpening::all(), Some(chosen), |chosen| {
            Message::Sheet(SheetMessage::FsrSharpening(chosen))
        })
        .text_size(14)
        .padding([8, 12]),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([0, 20])
    .into()
}

/// Full screen and the size, under gamescope's switch.
fn gamescope_options(sheet: &Sheet) -> Element<'_, Message> {
    let size = row![
        text(t!("options-size"))
            .size(14)
            .color(colors().dim)
            .width(60),
        size_input(&t!("options-width"), &sheet.width, SheetMessage::Width),
        text("×").size(14).color(colors().dim),
        size_input(&t!("options-height"), &sheet.height, SheetMessage::Height),
        text(t!("options-size-empty")).size(13).color(colors().dim),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    column![
        toggler(sheet.options.gamescope_fullscreen)
            .label(t!("options-full-screen"))
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
            t!("local-arguments"),
            t!("options-arguments-about"),
            text_input("-windowed", &options.arguments)
                .on_input(|value| Message::Sheet(SheetMessage::Arguments(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
        field(
            t!("options-environment"),
            t!("options-environment-about"),
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
    label: String,
    about: String,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        text(label).size(15).font(BOLD),
        text(about).size(13).color(colors().dim),
        input.into(),
    ]
    .spacing(6)
    .into()
}

/// A switch, greyed out with a note saying why when it cannot be used
/// here, as when its program is missing (unless already on, so it can be
/// turned off).
fn switch<'a>(
    label: String,
    about: String,
    on: bool,
    unusable: Option<String>,
    message: fn(bool) -> SheetMessage,
) -> Element<'a, Message> {
    let usable = unusable.is_none();
    let mut about = about;
    if let Some(why) = unusable {
        about.push(' ');
        about.push_str(&why);
    }
    let toggle = toggler(on).label(label).text_size(15);
    let toggle = if usable || on {
        toggle.on_toggle(move |on| Message::Sheet(message(on)))
    } else {
        toggle
    };
    column![toggle, text(about).size(13).color(colors().dim)]
        .spacing(4)
        .into()
}

fn size_input<'a>(
    placeholder: &str,
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
    fn keeps_protons_switches_and_the_frame_cap() {
        let mut zendo = zendo_with(gog_game());
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        for message in [
            SheetMessage::Fsr(true),
            SheetMessage::FsrSharpening(Sharpening(4)),
            SheetMessage::Wayland(true),
            SheetMessage::NoSync(true),
            SheetMessage::FrameCap("6o0".into()),
        ] {
            let _ = zendo.update(Message::Sheet(message));
        }
        let sheet = zendo.sheet.as_ref().unwrap();
        assert_eq!(sheet.frame_cap, "60");
        assert_eq!(sheet.options.fsr_strength, Some(4));
        assert!(sheet.options.wayland && sheet.options.no_sync);
        // Wine's own sharpening is not written.
        let _ = zendo.update(Message::Sheet(SheetMessage::FsrSharpening(Sharpening(
            launch_options::WINE_SHARPENING,
        ))));
        assert_eq!(zendo.sheet.as_ref().unwrap().options.fsr_strength, None);
        let _ = zendo.update(Message::Activate);
        assert!(zendo.sheet.is_none(), "saved");
        assert_eq!(Sharpening(0).to_string(), "0 (sharpest)");
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
