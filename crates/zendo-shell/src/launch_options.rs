//! A game's launch options sheet, without a toolkit: what the form holds,
//! the lists it offers, what its fields build and what closing it asks. The
//! window draws it.
//!
//! The Proton it runs with, `MangoHud`, `GameMode`, gamescope, NVIDIA
//! offload, Proton's switches, the game's arguments and environment, and
//! scripts before and after it; for Epic and GOG games, the program run
//! instead of the store's and the other ways to start it the store lists.
//! Only for the games Zendo starts itself; Steam and Heroic keep their own
//! settings.

pub mod gamescope;

use std::fmt;
use std::path::{Path, PathBuf};

use zendo_core::cloud_saves::{Done, How};
use zendo_core::launch_options::{self, LaunchOptions};
use zendo_core::steam_runtime::{self, Runtime};
use zendo_core::store_tasks::{self, StoreTask};
use zendo_core::{Game, Launch, Source, launch, proton};

use crate::t;

/// The form for one game's options.
#[derive(Debug)]
pub struct Sheet {
    pub id: String,
    pub title: String,
    /// A Windows game Zendo installed: Proton and its controller switch
    /// apply.
    pub windows: bool,
    pub options: LaunchOptions,
    pub width: String,
    pub height: String,
    pub frame_cap: String,
    pub protons: Vec<ProtonChoice>,
    /// The build the game would run with, and the switches' variables its
    /// script never reads.
    pub build: Option<(String, Vec<&'static str>)>,
    pub tools: Tools,
    /// An Epic or GOG game: its engine runs another program when asked,
    /// and its store may list other ways to start it.
    pub store: Option<StoreKind>,
    /// The game's folder, which a program picked is kept relative to.
    pub folder: Option<PathBuf>,
    pub executable: String,
    /// The store's own way first.
    pub tasks: Vec<TaskChoice>,
    pub picking: bool,
    pub language: String,
    /// Whether Epic approves the game for offline play, when it says.
    pub offline_approved: Option<bool>,
    pub saves: Saves,
    pub gamescope: gamescope::Fields,
    /// For a native game: none, then the runtimes installed.
    pub runtimes: Vec<Pick<Runtime>>,
    pub error: Option<(Field, String)>,
    /// The options as they were when the sheet opened, to tell whether
    /// closing would discard anything.
    pub original: LaunchOptions,
    /// Close was pressed with changes: ask before discarding them.
    pub confirm_close: bool,
}

/// The part of the form an error belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Arguments,
    Environment,
    Gamescope,
    Other,
}

impl Sheet {
    /// The form for a game, or `None` when Zendo does not start it. With
    /// `persist` false nothing is read from the computer: no saved options,
    /// no tools, no Protons, runtimes or store lists.
    ///
    /// # Errors
    ///
    /// The saved options could not be read.
    pub fn open(game: &Game, persist: bool) -> Result<Option<Self>, String> {
        if !applies(game) {
            return Ok(None);
        }
        let mut options = if persist {
            LaunchOptions::load(&game.id).map_err(|err| format!("{err:#}"))?
        } else {
            LaunchOptions::default()
        };
        let windows = matches!(game.launch, Launch::Engine { .. } | Launch::Local { .. })
            && launch::needs_proton(game);
        let store = match (&game.launch, game.source) {
            (Launch::Engine { .. }, Source::Epic) => Some(StoreKind::Epic),
            (Launch::Engine { .. }, Source::Gog) => Some(StoreKind::Gog),
            _ => None,
        };
        let listed = if store.is_some() && persist {
            let listed = store_tasks::listed(game);
            // Epic's options are keyed by their number now: a task saved
            // as a display name moves to its number and is written back.
            if let Some(saved) = options.task.as_deref()
                && !listed.iter().any(|task| task.key == saved)
                && let Some(task) = listed.iter().find(|task| task.name == saved)
            {
                options.task = Some(task.key.clone());
            }
            listed
        } else {
            Vec::new()
        };
        let installed_runtimes = if persist {
            steam_runtime::installed()
        } else {
            Vec::new()
        };
        Ok(Some(Self {
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
                proton_choices(options.proton.as_deref(), persist)
            } else {
                Vec::new()
            },
            build: if windows && persist {
                build_for(options.proton.as_deref())
            } else {
                None
            },
            store,
            folder: game.details.install_path.clone(),
            executable: options.executable.clone().unwrap_or_default(),
            tasks: if store.is_some() {
                task_choices(listed, options.task.as_deref())
            } else {
                Vec::new()
            },
            picking: false,
            language: options.language.clone().unwrap_or_default(),
            offline_approved: if persist {
                store_tasks::epic_offline(game)
            } else {
                None
            },
            saves: Saves::default(),
            gamescope: gamescope::Fields::of(&options),
            runtimes: if windows {
                Vec::new()
            } else {
                runtime_choices(&installed_runtimes, options.steam_runtime)
            },
            original: options.clone(),
            options,
            tools: if persist {
                Tools::find()
            } else {
                Tools::default()
            },
            error: None,
            confirm_close: false,
        }))
    }

    /// The options the form would save, and the field at fault when it
    /// cannot.
    ///
    /// # Errors
    ///
    /// The part of the form at fault, and what is wrong with it.
    pub fn build(&self) -> Result<LaunchOptions, (Field, String)> {
        let mut options = self.options.clone();
        options.gamescope_width = number(&self.width);
        options.gamescope_height = number(&self.height);
        options.frame_cap = number(&self.frame_cap);
        options.executable = Some(self.executable.trim().to_owned())
            .filter(|program| self.store.is_some() && !program.is_empty());
        options.language = Some(self.language.trim().to_owned())
            .filter(|language| self.store == Some(StoreKind::Epic) && !language.is_empty());
        options
            .arguments()
            .map_err(|err| (Field::Arguments, format!("{err:#}")))?;
        options
            .environment()
            .map_err(|err| (Field::Environment, format!("{err:#}")))?;
        self.gamescope
            .keep(&mut options)
            .map_err(|err| (Field::Gamescope, err))?;
        if options.gamescope_width.is_some() != options.gamescope_height.is_some() {
            return Err((Field::Gamescope, t!("options-size-both")));
        }
        options
            .check()
            .map_err(|err| (Field::Other, format!("{err:#}")))?;
        Ok(options)
    }

    /// Whether closing would discard something: the form does not build, or
    /// builds to other options than it opened with.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.build()
            .map_or(true, |options| options != self.original)
    }

    /// Close was pressed: true when the sheet may go, false when it asks
    /// first (`confirm_close` is then set).
    pub fn close(&mut self) -> bool {
        if self.changed() {
            self.confirm_close = true;
            false
        } else {
            true
        }
    }

    /// Writes the options (only checks them when `persist` is false). True
    /// when they were kept; otherwise `error` says why.
    pub fn save(&mut self, persist: bool) -> bool {
        let options = match self.build() {
            Ok(options) => options,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        let saved = if persist {
            options.save(&self.id)
        } else {
            options.check()
        };
        match saved {
            Ok(()) => true,
            Err(err) => {
                self.error = Some((Field::Other, format!("{err:#}")));
                false
            }
        }
    }

    /// Chooses a Proton by name (`None` for the default).
    pub fn set_proton(&mut self, name: Option<String>, persist: bool) {
        self.build = if persist {
            build_for(name.as_deref())
        } else {
            None
        };
        self.options.proton = name;
    }

    /// FSR's sharpening, as the list offers it: Wine's own is not written.
    pub fn set_fsr_strength(&mut self, strength: u8) {
        self.options.fsr_strength =
            (strength != launch_options::WINE_SHARPENING).then_some(strength);
    }

    /// One of gamescope's settings.
    pub fn set_gamescope(&mut self, setting: gamescope::Setting) {
        self.gamescope.set(&mut self.options, setting);
    }

    /// What the chosen build does not read of `names`, said under their
    /// switch.
    #[must_use]
    pub fn ignored_note(&self, names: &[&str]) -> Option<String> {
        let (build, ignored) = self.build.as_ref()?;
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
    }

    /// A switch's words with what the chosen build ignores added.
    #[must_use]
    pub fn noted(&self, about: String, names: &[&str]) -> String {
        match self.ignored_note(names) {
            Some(unread) => format!("{about} {unread}"),
            None => about,
        }
    }

    /// The program picked in the file dialog, kept relative to the game's
    /// folder when it is in it.
    pub fn keep_picked_program(&mut self, picked: &Path) {
        self.executable = kept_program(self.folder.as_deref(), picked);
    }
}

/// The cloud saves part: the sync running, the Upload or Download pressed
/// once, and how the last went.
#[derive(Debug, Default)]
pub struct Saves {
    pub syncing: bool,
    pub confirm: Option<How>,
    pub note: Option<(String, bool)>,
}

/// What a press of a sync button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// One sync at a time: nothing.
    Ignored,
    /// Upload and Download replace one side, so they ask once more.
    Asked,
    /// Start the sync.
    Start,
}

impl Saves {
    /// A sync button was pressed.
    pub fn press(&mut self, how: How) -> Press {
        if self.syncing {
            return Press::Ignored;
        }
        let forced = matches!(how, How::ForceUpload | How::ForceDownload);
        if forced && self.confirm != Some(how) {
            self.confirm = Some(how);
            return Press::Asked;
        }
        self.confirm = None;
        self.note = None;
        self.syncing = true;
        Press::Start
    }

    /// The sync ended; `store` is the game's store, named when it keeps no
    /// cloud saves.
    pub fn synced(&mut self, result: Result<Done, String>, store: &str) {
        self.syncing = false;
        self.note = Some(match result {
            Ok(Done::Synced) => (t!("saves-synced"), true),
            Ok(Done::NoCloudSaves) => (t!("cloud-none", store = store), true),
            Err(err) => (err, false),
        });
    }
}

/// The stores whose engines take a program and list other ways to start
/// a game; legendary takes a language too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    Epic,
    Gog,
}

/// An entry of a list whose first entry leaves the setting unset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick<T> {
    pub value: Option<T>,
    pub label: String,
}

impl<T> fmt::Display for Pick<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

impl<T: Copy + PartialEq> Pick<T> {
    /// `none` first, then each value with its label.
    pub fn list(none: String, values: impl IntoIterator<Item = (T, String)>) -> Vec<Self> {
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
    #[must_use]
    pub fn chosen(list: &[Self], value: Option<T>) -> Option<Self> {
        list.iter().find(|pick| pick.value == value).cloned()
    }
}

/// The runtimes for a native game: none, those installed, and the one
/// chosen when it is not.
#[must_use]
pub fn runtime_choices(installed: &[Runtime], chosen: Option<Runtime>) -> Vec<Pick<Runtime>> {
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
    pub key: Option<String>,
    pub label: String,
}

impl fmt::Display for TaskChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The store's own way, then the others it lists, then the one chosen
/// when the store no longer lists it.
#[must_use]
pub fn task_choices(listed: Vec<StoreTask>, chosen: Option<&str>) -> Vec<TaskChoice> {
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
#[must_use]
pub fn kept_program(folder: Option<&Path>, picked: &Path) -> String {
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
#[must_use]
pub fn build_for(choice: Option<&str>) -> Option<(String, Vec<&'static str>)> {
    let proton = choice.and_then(proton::named).or_else(proton::default)?;
    let ignored = proton.ignores(&CHECKED);
    Some((proton.name, ignored))
}

/// FSR's sharpening, as the list offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sharpening(pub u8);

impl Sharpening {
    #[must_use]
    pub fn all() -> Vec<Self> {
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
    cards > 1 && Path::new("/sys/module/nvidia").is_dir()
}

/// Which of the wrappers are installed.
#[derive(Debug, Clone, Copy, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one per program")]
pub struct Tools {
    pub mangohud: bool,
    pub mangoapp: bool,
    pub gamemode: bool,
    pub gamescope: bool,
    /// Not a program: an NVIDIA card beside another.
    pub offload: bool,
    /// Steam Linux Runtime 4.0, which Proton runs in without umu.
    pub runtime_4: bool,
    /// Epic's EOS overlay, installed in Settings.
    pub eos_overlay: bool,
}

impl Tools {
    #[must_use]
    pub fn find() -> Self {
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
    pub name: Option<String>,
    pub label: String,
}

impl fmt::Display for ProtonChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// Whether the game has launch options Zendo applies.
#[must_use]
pub fn applies(game: &Game) -> bool {
    matches!(
        game.launch,
        Launch::Engine { .. } | Launch::Command { .. } | Launch::Local { .. }
    )
}

/// The Proton list: the default first, then each one installed, and the
/// one chosen even when it is gone. With `persist` false, the default only.
#[must_use]
pub fn proton_choices(chosen: Option<&str>, persist: bool) -> Vec<ProtonChoice> {
    let (installed, default) = if persist {
        (proton::installed(), proton::default())
    } else {
        (Vec::new(), None)
    };
    let default = match default {
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

/// A number typed: positive, or nothing.
#[must_use]
pub fn number(field: &str) -> Option<u32> {
    field.trim().parse().ok().filter(|&n| n > 0)
}

/// What is left of a field that takes digits only.
#[must_use]
pub fn digits(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn open(game: &Game) -> Sheet {
        Sheet::open(game, false).unwrap().expect("the sheet opens")
    }

    #[test]
    fn closing_with_changes_asks_first() {
        let mut sheet = open(&gog_game());
        assert!(sheet.close(), "unchanged: it closes at once");
        sheet.options.mangohud = true;
        assert!(!sheet.close(), "changed: it asks");
        assert!(sheet.confirm_close);
    }

    #[test]
    fn a_form_that_does_not_build_counts_as_changed() {
        let mut sheet = open(&gog_game());
        sheet.width = "1920".into();
        assert!(sheet.changed(), "a width needs a height");
        assert!(!sheet.save(false));
        assert_eq!(
            sheet.error.as_ref().map(|(field, _)| *field),
            Some(Field::Gamescope)
        );
    }

    #[test]
    fn saves_what_builds_and_names_the_field_at_fault() {
        let mut sheet = open(&gog_game());
        assert!(sheet.windows);
        assert_eq!(sheet.protons[0].name, None, "the default comes first");
        sheet.options.environment = "DXVK_HUD".into();
        assert!(!sheet.save(false));
        assert_eq!(
            sheet.error,
            Some((
                Field::Environment,
                "\"DXVK_HUD\" in the environment is not NAME=value".to_owned()
            ))
        );
        sheet.options.environment = "DXVK_HUD=fps".into();
        sheet.width = digits("19x20");
        sheet.height = digits("10x80");
        assert_eq!(
            (sheet.width.as_str(), sheet.height.as_str()),
            ("1920", "1080")
        );
        assert!(sheet.save(false));
    }

    #[test]
    fn keeps_protons_switches_and_the_frame_cap() {
        let mut sheet = open(&gog_game());
        sheet.options.fsr = true;
        sheet.set_fsr_strength(4);
        sheet.options.wayland = true;
        sheet.frame_cap = digits("6o0");
        assert_eq!(sheet.frame_cap, "60");
        assert_eq!(sheet.options.fsr_strength, Some(4));
        // Wine's own sharpening is not written.
        sheet.set_fsr_strength(launch_options::WINE_SHARPENING);
        assert_eq!(sheet.options.fsr_strength, None);
        assert!(sheet.save(false));
        assert_eq!(Sharpening(0).to_string(), "0 (sharpest)");
    }

    #[test]
    fn keeps_the_program_the_way_to_start_and_the_scripts() {
        let mut sheet = open(&gog_game());
        assert_eq!(sheet.store, Some(StoreKind::Gog));
        assert_eq!(sheet.tasks.len(), 1, "the store's own only");
        sheet.executable = "  bin/x64/game.exe ".into();
        sheet.options.before = "notify-send hi".into();
        assert_eq!(
            sheet.build().unwrap().executable.as_deref(),
            Some("bin/x64/game.exe")
        );

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
        let sheet = open(&native);
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
        let mut sheet = open(&epic);
        assert_eq!(sheet.store, Some(StoreKind::Epic));
        sheet.language = "french".into();
        assert!(!sheet.save(false), "not a code");
        sheet.language = " fr ".into();
        assert!(sheet.save(false));
    }

    #[test]
    fn syncs_saves_by_hand_and_asks_before_replacing_a_side() {
        let mut saves = Saves::default();
        assert_eq!(saves.press(How::ForceUpload), Press::Asked);
        assert_eq!(saves.confirm, Some(How::ForceUpload));
        assert_eq!(saves.press(How::ForceUpload), Press::Start);
        assert!(saves.syncing && saves.confirm.is_none());
        assert_eq!(saves.press(How::Both), Press::Ignored, "one at a time");
        saves.synced(Err("conflict".into()), "GOG");
        assert!(!saves.syncing);
        assert_eq!(saves.note, Some(("conflict".to_owned(), false)));
        assert_eq!(
            saves.press(How::Both),
            Press::Start,
            "Sync now asks no more"
        );

        let mut sheet = open(&gog_game());
        assert!(sheet.options.syncs_saves(), "on by default");
        sheet.options.set_syncs_saves(false);
        assert_eq!(sheet.options.cloud_saves, Some(false));
        sheet.options.set_syncs_saves(true);
        assert_eq!(
            sheet.options.cloud_saves, None,
            "the default is not written"
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
        assert!(!applies(&game));
        assert!(Sheet::open(&game, false).unwrap().is_none());
    }
}
