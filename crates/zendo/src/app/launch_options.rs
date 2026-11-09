//! A game's launch options sheet, over its page: the Proton it runs with,
//! `MangoHud`, `GameMode`, gamescope, NVIDIA offload, Proton's switches
//! (controllers as Xbox pads, Wine's Wayland driver, HDR, FSR, `WoW64`,
//! esync and fsync off, DXVK's counter and frame cap), the game's
//! arguments and environment, and scripts before and after it; for Epic
//! and GOG games, the program run instead of the store's and the other
//! ways to start it the store lists. Only for the games Zendo starts
//! itself; Steam and Heroic keep their own settings.

mod gamescope;

use std::fmt;
use std::path::{Path, PathBuf};

use iced::widget::{
    button, center, column, container, opaque, pick_list, row, scrollable, space, text, text_input,
    toggler,
};
use iced::{Alignment, Background, Color, Element, Length, Task};
use zendo_core::launch_options::{self, LaunchOptions};
use zendo_core::steam_runtime::{self, Runtime};
use zendo_core::store_tasks::{self, StoreTask};
use zendo_core::{Game, Launch, Source, launch, proton};

use super::style::{self, BOLD, colors};
use super::{Message, Zendo, blocking};
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
    /// An Epic or GOG game: its engine runs another program when asked,
    /// and its store may list other ways to start it.
    store: Option<StoreKind>,
    /// The game's folder, which a program picked is kept relative to.
    folder: Option<PathBuf>,
    executable: String,
    /// The store's own way first.
    tasks: Vec<TaskChoice>,
    picking: bool,
    language: String,
    /// Whether Epic approves the game for offline play, when it says.
    offline_approved: Option<bool>,
    gamescope: gamescope::Fields,
    /// For a native game: none, then the runtimes installed.
    runtimes: Vec<Pick<Runtime>>,
    error: Option<String>,
}

/// The stores whose engines take a program and list other ways to start
/// a game; legendary takes a language too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreKind {
    Epic,
    Gog,
}

/// An entry of a list whose first entry leaves the setting unset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick<T> {
    value: Option<T>,
    label: String,
}

impl<T> fmt::Display for Pick<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

impl<T: Copy + PartialEq> Pick<T> {
    /// `none` first, then each value with its label.
    fn list(none: String, values: impl IntoIterator<Item = (T, String)>) -> Vec<Self> {
        let mut list = vec![Self {
            value: None,
            label: none,
        }];
        list.extend(values.into_iter().map(|(value, label)| Self {
            value: Some(value),
            label,
        }));
        list
    }

    /// The entry for the value set.
    fn chosen(list: &[Self], value: Option<T>) -> Option<Self> {
        list.iter().find(|pick| pick.value == value).cloned()
    }
}

/// The runtimes for a native game: none, those installed, and the one
/// chosen when it is not.
fn runtime_choices(installed: &[Runtime], chosen: Option<Runtime>) -> Vec<Pick<Runtime>> {
    let mut list = Pick::list(
        t!("options-runtime-none"),
        installed
            .iter()
            .map(|runtime| (*runtime, runtime.label().to_owned())),
    );
    if let Some(chosen) = chosen
        && !installed.contains(&chosen)
    {
        list.push(Pick {
            value: Some(chosen),
            label: t!("options-runtime-missing", name = chosen.label()),
        });
    }
    list
}

/// An entry of the list of ways to start the game: the store's own
/// (`None`) or another it lists, by its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskChoice {
    key: Option<String>,
    label: String,
}

impl fmt::Display for TaskChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The store's own way, then the others it lists, then the one chosen
/// when the store no longer lists it.
fn task_choices(listed: Vec<StoreTask>, chosen: Option<&str>) -> Vec<TaskChoice> {
    let mut choices = vec![TaskChoice {
        key: None,
        label: t!("options-task-default"),
    }];
    choices.extend(listed.into_iter().map(|task| TaskChoice {
        label: if task.detail.is_empty() {
            task.name
        } else {
            format!("{} ({})", task.name, task.detail)
        },
        key: Some(task.key),
    }));
    if let Some(chosen) = chosen
        && !choices
            .iter()
            .any(|choice| choice.key.as_deref() == Some(chosen))
    {
        choices.push(TaskChoice {
            key: Some(chosen.to_owned()),
            label: t!("options-task-gone", name = chosen),
        });
    }
    choices
}

/// A program picked, as kept: relative to the game's folder when in it.
fn kept_program(folder: Option<&Path>, picked: &Path) -> String {
    folder
        .and_then(|folder| picked.strip_prefix(folder).ok())
        .unwrap_or(picked)
        .to_string_lossy()
        .into_owned()
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
    /// Steam Linux Runtime 4.0, which Proton runs in without umu.
    runtime_4: bool,
    /// Epic's EOS overlay, installed in Settings.
    eos_overlay: bool,
}

impl Tools {
    fn find() -> Self {
        Self {
            mangohud: on_path("mangohud"),
            mangoapp: on_path("mangoapp"),
            gamemode: on_path("gamemoderun"),
            gamescope: on_path("gamescope"),
            offload: offload_possible(),
            runtime_4: Runtime::Four.find().is_some(),
            eos_overlay: zendo_core::eos_overlay::installed(),
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
    GamescopeSet(gamescope::Setting),
    NoUmu(bool),
    Offline(bool),
    SkipUpdateCheck(bool),
    EosOverlay(bool),
    SteamRuntime(Pick<Runtime>),
    Language(String),
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
    StoreTask(TaskChoice),
    Executable(String),
    BrowseExecutable,
    Browsed(Result<Option<PathBuf>, String>),
    Before(String),
    After(String),
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
            SheetMessage::GamescopeSet(setting) => sheet.gamescope.set(options, setting),
            SheetMessage::NoUmu(on) => options.no_umu = on,
            SheetMessage::Offline(on) => options.offline = on,
            SheetMessage::SkipUpdateCheck(on) => options.skip_update_check = on,
            SheetMessage::EosOverlay(on) => options.eos_overlay = on,
            SheetMessage::SteamRuntime(pick) => options.steam_runtime = pick.value,
            SheetMessage::Language(language) => sheet.language = language,
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
            SheetMessage::StoreTask(_)
            | SheetMessage::Executable(_)
            | SheetMessage::BrowseExecutable
            | SheetMessage::Browsed(_)
            | SheetMessage::Before(_)
            | SheetMessage::After(_) => return self.update_starting(message),
            SheetMessage::Save => return self.save_sheet(),
            SheetMessage::Close => self.sheet = None,
        }
        Task::none()
    }

    /// The program, the store's way to start the game, and the scripts.
    fn update_starting(&mut self, message: SheetMessage) -> Task<Message> {
        let Some(sheet) = self.sheet.as_mut() else {
            return Task::none();
        };
        match message {
            SheetMessage::StoreTask(choice) => sheet.options.task = choice.key,
            SheetMessage::Executable(program) => sheet.executable = program,
            SheetMessage::BrowseExecutable => {
                if sheet.picking {
                    return Task::none();
                }
                sheet.picking = true;
                return Task::perform(
                    blocking(|| {
                        crate::file_dialog::pick(
                            crate::file_dialog::Kind::Program,
                            &t!("options-executable-choose"),
                        )
                        .map_err(|err| format!("{err:#}"))
                    }),
                    |result| Message::Sheet(SheetMessage::Browsed(result)),
                );
            }
            SheetMessage::Browsed(result) => {
                sheet.picking = false;
                match result {
                    Ok(Some(path)) => {
                        sheet.executable = kept_program(sheet.folder.as_deref(), &path);
                    }
                    Ok(None) => {}
                    Err(err) => sheet.error = Some(err),
                }
            }
            SheetMessage::Before(line) => sheet.options.before = line,
            SheetMessage::After(line) => sheet.options.after = line,
            _ => {}
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
        let store = match (&game.launch, game.source) {
            (Launch::Engine { .. }, Source::Epic) => Some(StoreKind::Epic),
            (Launch::Engine { .. }, Source::Gog) => Some(StoreKind::Gog),
            _ => None,
        };
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
            store,
            folder: game.details.install_path.clone(),
            executable: options.executable.clone().unwrap_or_default(),
            tasks: if store.is_some() {
                task_choices(store_tasks::listed(&game), options.task.as_deref())
            } else {
                Vec::new()
            },
            picking: false,
            language: options.language.clone().unwrap_or_default(),
            offline_approved: store_tasks::epic_offline(&game),
            gamescope: gamescope::Fields::of(&options),
            runtimes: if windows {
                Vec::new()
            } else {
                runtime_choices(&steam_runtime::installed(), options.steam_runtime)
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
        options.executable = Some(sheet.executable.trim().to_owned())
            .filter(|program| sheet.store.is_some() && !program.is_empty());
        options.language = Some(sheet.language.trim().to_owned())
            .filter(|language| sheet.store == Some(StoreKind::Epic) && !language.is_empty());
        if let Err(err) = sheet.gamescope.keep(&mut options) {
            sheet.error = Some(err);
            return Task::none();
        }
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

    if sheet.store.is_some() {
        body = body.push(starting(sheet));
    }

    body = body.push(tool_switches(sheet));
    if !sheet.windows {
        body = body.push(field(
            t!("options-runtime"),
            t!("options-runtime-about"),
            pick_list(
                sheet.runtimes.as_slice(),
                Pick::chosen(&sheet.runtimes, options.steam_runtime),
                |pick| Message::Sheet(SheetMessage::SteamRuntime(pick)),
            )
            .text_size(14)
            .padding([8, 12])
            .width(Length::Fill),
        ));
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
    body = body.push(scripts(options));
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
        .push(runner_switches(sheet))
        .into()
}

/// Epic's overlay for an Epic game, and Proton without umu.
fn runner_switches(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    let mut switches = column![].spacing(18);
    if sheet.store == Some(StoreKind::Epic) {
        switches = switches.push(switch(
            t!("options-eos-overlay"),
            t!("options-eos-overlay-about"),
            options.eos_overlay,
            (!sheet.tools.eos_overlay).then(|| t!("options-eos-overlay-missing")),
            SheetMessage::EosOverlay,
        ));
    }
    switches
        .push(switch(
            t!("options-no-umu"),
            t!("options-no-umu-about"),
            options.no_umu,
            (!sheet.tools.runtime_4).then(|| t!("options-no-umu-runtime")),
            SheetMessage::NoUmu,
        ))
        .into()
}

/// `MangoHud`, `GameMode` and gamescope, with gamescope's settings when on.
fn tool_switches(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    let tools = sheet.tools;
    let mangohud_ready = if options.gamescope {
        tools.mangoapp
    } else {
        tools.mangohud
    };
    let missing = |installed: bool| (!installed).then(|| t!("options-not-installed"));
    let mut switches = column![
        switch(
            "MangoHud".to_owned(),
            t!("options-mangohud-about"),
            options.mangohud,
            missing(mangohud_ready),
            SheetMessage::MangoHud,
        ),
        switch(
            "GameMode".to_owned(),
            t!("options-gamemode-about"),
            options.gamemode,
            missing(tools.gamemode),
            SheetMessage::GameMode,
        ),
        switch(
            "Gamescope".to_owned(),
            t!("options-gamescope-about"),
            options.gamescope,
            missing(tools.gamescope),
            SheetMessage::Gamescope,
        ),
    ]
    .spacing(18);
    if options.gamescope {
        switches = switches.push(gamescope::view(sheet));
    }
    switches.into()
}

/// What starts: the store's way or another it lists, and the program run
/// instead of the store's.
fn starting(sheet: &Sheet) -> Element<'_, Message> {
    let tasks: Element<'_, Message> = if sheet.tasks.len() > 1 {
        let selected = sheet
            .tasks
            .iter()
            .find(|choice| choice.key == sheet.options.task)
            .cloned();
        pick_list(sheet.tasks.as_slice(), selected, |choice| {
            Message::Sheet(SheetMessage::StoreTask(choice))
        })
        .text_size(14)
        .padding([8, 12])
        .width(Length::Fill)
        .into()
    } else {
        text(t!("options-task-none"))
            .size(14)
            .color(colors().dim)
            .into()
    };
    let browse = button(text(t!("options-executable-browse")).size(14))
        .padding([8, 14])
        .style(style::secondary)
        .on_press_maybe((!sheet.picking).then_some(Message::Sheet(SheetMessage::BrowseExecutable)));
    let mut starting = column![
        field(t!("options-task"), t!("options-task-about"), tasks),
        field(
            t!("options-executable"),
            t!("options-executable-about"),
            row![
                text_input(&t!("options-executable-empty"), &sheet.executable)
                    .on_input(|value| Message::Sheet(SheetMessage::Executable(value)))
                    .on_submit(Message::Sheet(SheetMessage::Save))
                    .size(14)
                    .padding([8, 12])
                    .style(style::input),
                browse,
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ),
    ]
    .spacing(18);
    if sheet.store == Some(StoreKind::Epic) {
        starting = starting.push(field(
            t!("options-language"),
            t!("options-language-about"),
            text_input(&t!("options-language-empty"), &sheet.language)
                .on_input(|value| Message::Sheet(SheetMessage::Language(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .width(200)
                .style(style::input),
        ));
        let offline_note = match sheet.offline_approved {
            Some(false) => format!(
                "{} {}",
                t!("options-offline-about"),
                t!("options-offline-not-approved")
            ),
            _ => t!("options-offline-about"),
        };
        starting = starting
            .push(switch(
                t!("options-offline"),
                offline_note,
                sheet.options.offline,
                None,
                SheetMessage::Offline,
            ))
            .push(switch(
                t!("options-skip-update"),
                t!("options-skip-update-about"),
                sheet.options.skip_update_check,
                None,
                SheetMessage::SkipUpdateCheck,
            ));
    }
    starting.into()
}

/// Commands before the game starts and after it ends.
fn scripts(options: &LaunchOptions) -> Element<'_, Message> {
    column![
        field(
            t!("options-before"),
            t!("options-before-about"),
            text_input("notify-send 'Starting'", &options.before)
                .on_input(|value| Message::Sheet(SheetMessage::Before(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
        field(
            t!("options-after"),
            t!("options-after-about"),
            text_input("~/bin/back-up-saves", &options.after)
                .on_input(|value| Message::Sheet(SheetMessage::After(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([8, 12])
                .style(style::input),
        ),
    ]
    .spacing(18)
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
    fn keeps_the_program_the_way_to_start_and_the_scripts() {
        let mut zendo = zendo_with(gog_game());
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        let sheet = zendo.sheet.as_ref().unwrap();
        assert_eq!(sheet.store, Some(StoreKind::Gog));
        assert_eq!(sheet.tasks.len(), 1, "the store's own only");
        for message in [
            SheetMessage::Executable("  bin/x64/game.exe ".into()),
            SheetMessage::Before("notify-send hi".into()),
            SheetMessage::After("true".into()),
        ] {
            let _ = zendo.update(Message::Sheet(message));
        }
        let _ = zendo.update(Message::Activate);
        assert!(zendo.sheet.is_none(), "saved");

        let listed = vec![StoreTask {
            key: "bin\\x64\\game.exe".into(),
            name: "Game".into(),
            detail: "bin/x64/game.exe".into(),
        }];
        let choices = task_choices(listed, Some("old.exe"));
        let labels: Vec<String> = choices.iter().map(ToString::to_string).collect();
        assert_eq!(
            labels,
            [
                "The store's own",
                "Game (bin/x64/game.exe)",
                "old.exe (no longer listed; the store's own runs)"
            ]
        );
        assert_eq!(
            kept_program(
                Some(Path::new("/games/Sugar")),
                Path::new("/games/Sugar/bin/sugar")
            ),
            "bin/sugar"
        );
        assert_eq!(
            kept_program(Some(Path::new("/games/Sugar")), Path::new("/opt/x")),
            "/opt/x"
        );
    }

    #[test]
    fn offers_runtimes_to_native_games_and_a_language_to_epics() {
        let mut native = gog_game();
        native.details.platform = Some("linux".into());
        let mut zendo = zendo_with(native);
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        let sheet = zendo.sheet.as_ref().unwrap();
        assert!(!sheet.windows);
        assert_eq!(sheet.runtimes[0].value, None, "none first");
        let choices = runtime_choices(&[Runtime::Soldier], Some(Runtime::Sniper));
        let labels: Vec<String> = choices.iter().map(ToString::to_string).collect();
        assert_eq!(
            labels,
            [
                "None: the system's libraries",
                "Steam Linux Runtime 2.0 (soldier)",
                "Steam Linux Runtime 3.0 (sniper) (not installed)"
            ]
        );

        let mut epic = gog_game();
        epic.id = "epic:Sugar".into();
        epic.source = Source::Epic;
        let mut zendo = zendo_with(epic);
        let _ = zendo.update(Message::Sheet(SheetMessage::Open));
        assert_eq!(
            zendo.sheet.as_ref().map(|sheet| sheet.store),
            Some(Some(StoreKind::Epic))
        );
        let _ = zendo.update(Message::Sheet(SheetMessage::Language("french".into())));
        let _ = zendo.update(Message::Activate);
        assert!(
            zendo
                .sheet
                .as_ref()
                .and_then(|sheet| sheet.error.as_ref())
                .is_some(),
            "not a code"
        );
        let _ = zendo.update(Message::Sheet(SheetMessage::Language(" fr ".into())));
        let _ = zendo.update(Message::Sheet(SheetMessage::NoUmu(true)));
        let _ = zendo.update(Message::Activate);
        assert!(zendo.sheet.is_none(), "saved");
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
