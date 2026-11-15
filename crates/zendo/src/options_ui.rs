//! The Launch options sheet over a game's page, for the games Zendo starts
//! itself. The form, its lists and what closing asks are
//! `zendo_shell::launch_options`; this draws them. A part
//! of the Library's glue (`library_ui`), which owns the open game.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::cloud_saves::{self, How};
use zendo_core::launch_options::{self, Filter, Scaler};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::launch_options::gamescope::{self, Setting, WindowKind};
use zendo_shell::launch_options::{Field, Press, Sharpening, Sheet, StoreKind, digits};
use zendo_shell::t;

use super::settings::chip;
use super::{Glue, with_glue};
use crate::{ChipData, MainWindow, OptSwitch, OptionsData};

/// The sheet, while it is open.
#[derive(Default)]
pub(super) struct OptionsState {
    sheet: RefCell<Option<Sheet>>,
}

// The switches' ids, as the window sends them back.
const MANGOHUD: i32 = 0;
const GAMEMODE: i32 = 1;
const GAMESCOPE: i32 = 2;
const NO_UMU: i32 = 3;
const OFFLINE: i32 = 4;
const SKIP_UPDATE: i32 = 5;
const EOS_OVERLAY: i32 = 6;
const CLOUD_SAVES: i32 = 7;
const XBOX: i32 = 8;
const NO_SYNC: i32 = 9;
const WAYLAND: i32 = 10;
const HDR: i32 = 11;
const FSR: i32 = 12;
const WOW64: i32 = 13;
const FPS_COUNTER: i32 = 14;
const OFFLOAD: i32 = 15;

/// One switch row. `unusable` says why it cannot be turned on here; a switch
/// already on can still be turned off.
fn switch(id: i32, label: &str, about: String, on: bool, unusable: Option<String>) -> OptSwitch {
    let enabled = unusable.is_none() || on;
    let about = match unusable {
        Some(why) => format!("{about} {why}"),
        None => about,
    };
    OptSwitch {
        id,
        label: label.into(),
        about: about.into(),
        on,
        enabled,
    }
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

impl Glue {
    /// Hooks the sheet's callbacks up.
    pub(super) fn connect_options(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<crate::GameData>();
        let glue = Rc::clone(self);
        data.on_launch_options(move || glue.open_options());
        let data = window.global::<OptionsData>();
        let glue = Rc::clone(self);
        data.on_toggled(move |id, on| glue.options_toggled(id, on));
        let glue = Rc::clone(self);
        data.on_choose(move |list, index| glue.options_chose(list, index));
        let glue = Rc::clone(self);
        data.on_edited(move |field, text| glue.options_edited(field, &text));
        let glue = Rc::clone(self);
        data.on_sync(move |which| glue.options_sync(which));
        let glue = Rc::clone(self);
        data.on_browse(move || glue.options_browse());
        let glue = Rc::clone(self);
        data.on_save(move || glue.options_save());
        let glue = Rc::clone(self);
        data.on_close(move || {
            glue.options_back();
        });
        let glue = Rc::clone(self);
        data.on_discard(move || glue.options_discard());
        let glue = Rc::clone(self);
        data.on_keep_editing(move || glue.options_keep_editing());
    }

    /// Whether the sheet is open: then the page's keys and the controller do
    /// nothing.
    pub(super) fn options_open(&self) -> bool {
        self.options.sheet.borrow().is_some()
    }

    /// Enter saves; the fields have every other key.
    pub(super) fn options_key(&self, text: &str) -> bool {
        if text == char::from(slint::platform::Key::Return).to_string() {
            self.options_save();
            return true;
        }
        false
    }

    /// The Launch options button on the game's page.
    fn open_options(&self) {
        if self.options_open() {
            return;
        }
        let Some(game) = self.open_game_record() else {
            return;
        };
        match Sheet::open(&game, self.persist) {
            Ok(Some(sheet)) => {
                *self.options.sheet.borrow_mut() = Some(sheet);
                self.show_options(true);
            }
            Ok(None) => {}
            Err(err) => self.say(err),
        }
    }

    /// Escape, Cancel and a click outside: closes the sheet, or asks first
    /// when closing would discard changes. Says whether it was open.
    pub(super) fn options_back(&self) -> bool {
        let closed = {
            let mut sheet = self.options.sheet.borrow_mut();
            let Some(form) = sheet.as_mut() else {
                return false;
            };
            let closes = form.close();
            if closes {
                *sheet = None;
            }
            closes
        };
        if closed {
            self.options_closed();
        } else {
            self.show_options(false);
        }
        true
    }

    fn options_discard(&self) {
        if self.options.sheet.borrow_mut().take().is_some() {
            self.options_closed();
        }
    }

    fn options_keep_editing(&self) {
        self.edit_options(|sheet| sheet.confirm_close = false);
        self.show_options(false);
    }

    /// The sheet is gone: its fields had the keyboard, which goes back to
    /// the page.
    fn options_closed(&self) {
        self.show_options(true);
        self.focus_page();
    }

    fn edit_options(&self, change: impl FnOnce(&mut Sheet)) {
        if let Some(sheet) = self.options.sheet.borrow_mut().as_mut() {
            change(sheet);
        }
    }

    fn options_save(&self) {
        let saved = {
            let mut sheet = self.options.sheet.borrow_mut();
            let Some(form) = sheet.as_mut() else {
                return;
            };
            if form.save(self.persist) {
                let title = form.title.clone();
                *sheet = None;
                Some(title)
            } else {
                None
            }
        };
        match saved {
            Some(title) => {
                self.say(t!("options-saved", title = title));
                self.options_closed();
                // The Details card's cloud saves row follows the switch.
                self.cloud_reload();
                self.show_game();
            }
            None => self.show_options(false),
        }
    }

    fn options_toggled(&self, id: i32, on: bool) {
        self.edit_options(|sheet| {
            let options = &mut sheet.options;
            match id {
                MANGOHUD => options.mangohud = on,
                GAMEMODE => options.gamemode = on,
                GAMESCOPE => options.gamescope = on,
                NO_UMU => options.no_umu = on,
                OFFLINE => options.offline = on,
                SKIP_UPDATE => options.skip_update_check = on,
                EOS_OVERLAY => options.eos_overlay = on,
                CLOUD_SAVES => options.set_syncs_saves(on),
                XBOX => options.xbox_controllers = on,
                NO_SYNC => options.no_sync = on,
                WAYLAND => options.wayland = on,
                HDR => options.hdr = on,
                FSR => options.fsr = on,
                WOW64 => options.wow64 = on,
                FPS_COUNTER => options.fps_counter = on,
                OFFLOAD => options.nvidia_offload = on,
                _ => {}
            }
        });
        self.show_options(false);
    }

    /// A chip was pressed: the list (see `OptionsData.choose`) and the chip.
    fn options_chose(&self, list: i32, index: i32) {
        let persist = self.persist;
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        self.edit_options(|sheet| match list {
            0 => {
                if let Some(choice) = sheet.protons.get(index) {
                    let name = choice.name.clone();
                    sheet.set_proton(name, persist);
                }
            }
            1 => {
                if let Some(choice) = sheet.tasks.get(index) {
                    sheet.options.task = choice.key.clone();
                }
            }
            2 => {
                if let Some(pick) = sheet.runtimes.get(index) {
                    sheet.options.steam_runtime = pick.value;
                }
            }
            3 => {
                if let Some(Sharpening(n)) = Sharpening::all().get(index) {
                    sheet.set_fsr_strength(*n);
                }
            }
            4 => {
                if let Some(kind) = WindowKind::ALL.get(index) {
                    sheet.set_gamescope(Setting::Window(*kind));
                }
            }
            5 => {
                if let Some(pick) = gamescope::scalers().get(index) {
                    sheet.set_gamescope(Setting::Scaler(pick.clone()));
                }
            }
            6 => {
                if let Some(pick) = gamescope::filters().get(index) {
                    sheet.set_gamescope(Setting::Filter(pick.clone()));
                }
            }
            7 => {
                if let Some(pick) = gamescope::sharpness().get(index) {
                    sheet.set_gamescope(Setting::Sharpness(pick.clone()));
                }
            }
            _ => {}
        });
        self.show_options(false);
    }

    /// A field was typed in (see `OptionsData.edited`). A field that takes
    /// digits only gets what it kept written back.
    fn options_edited(&self, field: i32, text: &str) {
        let digits_only = matches!(field, 2..=7);
        let kept = if digits_only {
            digits(text)
        } else {
            text.to_owned()
        };
        // What a digit field kept, when it is not what was typed.
        let echo = (kept != text).then(|| kept.clone());
        self.edit_options(|sheet| match field {
            0 => sheet.executable = kept,
            1 => sheet.language = kept,
            2 => sheet.width = kept,
            3 => sheet.height = kept,
            4 => sheet.frame_cap = kept,
            5 => sheet.set_gamescope(Setting::GameWidth(kept)),
            6 => sheet.set_gamescope(Setting::GameHeight(kept)),
            7 => sheet.set_gamescope(Setting::Fps(kept)),
            8 => sheet.set_gamescope(Setting::More(kept)),
            9 => sheet.options.arguments = kept,
            10 => sheet.options.environment = kept,
            11 => sheet.options.before = kept,
            12 => sheet.options.after = kept,
            _ => {}
        });
        if let Some(kept) = echo
            && let Some(window) = self.window.upgrade()
        {
            let data = window.global::<OptionsData>();
            match field {
                2 => data.set_width(kept.into()),
                3 => data.set_height(kept.into()),
                4 => data.set_frame_cap(kept.into()),
                5 => data.set_game_width(kept.into()),
                6 => data.set_game_height(kept.into()),
                7 => data.set_fps(kept.into()),
                _ => {}
            }
        }
    }

    /// Sync now, Upload and Download: the sync runs on a thread; Upload and
    /// Download ask once more first.
    fn options_sync(&self, which: i32) {
        let how = match which {
            0 => How::Both,
            1 => How::ForceUpload,
            _ => How::ForceDownload,
        };
        let press = {
            let mut sheet = self.options.sheet.borrow_mut();
            let Some(form) = sheet.as_mut() else {
                return;
            };
            form.saves.press(how)
        };
        let game = self.open_game_record();
        if press == Press::Start && game.is_none() {
            self.edit_options(|sheet| sheet.saves.syncing = false);
        }
        self.show_options(false);
        let (Press::Start, Some(game)) = (press, game) else {
            return;
        };
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = cloud_saves::sync(&game, how).map_err(|err| format!("{err:#}"));
            let store = zendo_shell::format::source(game.source);
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.options_synced(result, &store));
            });
        });
    }

    fn options_synced(&self, result: Result<cloud_saves::Done, String>, store: &str) {
        self.edit_options(|sheet| sheet.saves.synced(result, store));
        self.cloud_reload();
        self.show_options(false);
        // The sheet may have closed while it ran; the page's row still moves.
        self.show_game();
    }

    /// Browse: the file dialog runs as a process of its own, on a thread.
    fn options_browse(&self) {
        {
            let mut sheet = self.options.sheet.borrow_mut();
            let Some(form) = sheet.as_mut() else {
                return;
            };
            if form.picking || !self.persist {
                return;
            }
            form.picking = true;
        }
        self.show_options(false);
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = file_dialog::pick(Kind::Program, &t!("options-executable-choose"))
                .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.options_picked(result));
            });
        });
    }

    fn options_picked(&self, result: Result<Option<PathBuf>, String>) {
        self.edit_options(|sheet| {
            sheet.picking = false;
            match result {
                Ok(Some(path)) => sheet.keep_picked_program(&path),
                Ok(None) => {}
                Err(err) => sheet.error = Some((Field::Other, err)),
            }
        });
        self.show_options(true);
    }

    /// Writes the sheet. `fields` is for when it opens or a program was
    /// chosen: writing the typed fields while the user types would move the
    /// cursor.
    fn show_options(&self, fields: bool) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<OptionsData>();
        let sheet = self.options.sheet.borrow();
        let Some(sheet) = sheet.as_ref() else {
            data.set_open(false);
            return;
        };
        data.set_open(true);
        data.set_title(sheet.title.clone().into());
        data.set_windows(sheet.windows);
        data.set_native(!sheet.windows);
        data.set_store(match sheet.store {
            None => 0,
            Some(StoreKind::Gog) => 1,
            Some(StoreKind::Epic) => 2,
        });
        data.set_has_tasks(sheet.tasks.len() > 1);
        data.set_gamescope_on(sheet.options.gamescope);
        data.set_fsr_on(sheet.options.fsr);
        data.set_sharpens(sheet.options.gamescope_filter.is_some_and(Filter::sharpens));
        data.set_confirm_close(sheet.confirm_close);
        data.set_picking(sheet.picking);
        let error_in = |which: Field| {
            sheet
                .error
                .as_ref()
                .filter(|(field, _)| *field == which)
                .map(|(_, text)| text.clone())
                .unwrap_or_default()
        };
        data.set_error(error_in(Field::Other).into());
        data.set_arguments_error(error_in(Field::Arguments).into());
        data.set_environment_error(error_in(Field::Environment).into());
        data.set_gamescope_error(error_in(Field::Gamescope).into());
        show_switches(&data, sheet);
        show_choices(&data, sheet);
        show_saves(&data, sheet);
        if fields {
            data.set_executable(sheet.executable.clone().into());
            data.set_language(sheet.language.clone().into());
            data.set_width(sheet.width.clone().into());
            data.set_height(sheet.height.clone().into());
            data.set_frame_cap(sheet.frame_cap.clone().into());
            data.set_game_width(sheet.gamescope.game_width.clone().into());
            data.set_game_height(sheet.gamescope.game_height.clone().into());
            data.set_fps(sheet.gamescope.fps.clone().into());
            data.set_more(sheet.options.gamescope_more.clone().into());
            data.set_arguments(sheet.options.arguments.clone().into());
            data.set_environment(sheet.options.environment.clone().into());
            data.set_before(sheet.options.before.clone().into());
            data.set_after(sheet.options.after.clone().into());
        }
    }
}

/// Every switch, with the words that depend on what is installed and on what
/// the chosen Proton reads.
fn show_switches(data: &OptionsData<'_>, sheet: &Sheet) {
    let options = &sheet.options;
    let tools = sheet.tools;
    let missing = |installed: bool| (!installed).then(|| t!("options-not-installed"));
    let mangohud_ready = if options.gamescope {
        tools.mangoapp
    } else {
        tools.mangohud
    };
    let epic = sheet.store == Some(StoreKind::Epic);
    let offline_note = match sheet.offline_approved {
        Some(false) => format!(
            "{} {}",
            t!("options-offline-about"),
            t!("options-offline-not-approved")
        ),
        _ => t!("options-offline-about"),
    };
    data.set_start_switches(model(if epic {
        vec![
            switch(
                OFFLINE,
                &t!("options-offline"),
                offline_note,
                options.offline,
                None,
            ),
            switch(
                SKIP_UPDATE,
                &t!("options-skip-update"),
                t!("options-skip-update-about"),
                options.skip_update_check,
                None,
            ),
        ]
    } else {
        Vec::new()
    }));
    data.set_saves_switch(model(vec![switch(
        CLOUD_SAVES,
        &t!("saves-around"),
        t!("saves-around-about"),
        options.syncs_saves(),
        None,
    )]));
    data.set_tool_switches(model(vec![
        switch(
            MANGOHUD,
            "MangoHud",
            t!("options-mangohud-about"),
            options.mangohud,
            missing(mangohud_ready),
        ),
        switch(
            GAMEMODE,
            "GameMode",
            t!("options-gamemode-about"),
            options.gamemode,
            missing(tools.gamemode),
        ),
        switch(
            GAMESCOPE,
            "Gamescope",
            t!("options-gamescope-about"),
            options.gamescope,
            missing(tools.gamescope),
        ),
    ]));
    data.set_offload_switch(model(vec![switch(
        OFFLOAD,
        &t!("options-offload"),
        t!("options-offload-about"),
        options.nvidia_offload,
        (!tools.offload).then(|| t!("options-offload-none")),
    )]));
    show_proton_switches(data, sheet);
}

/// The switches Proton, Wine and DXVK read, for a Windows game, with what
/// the chosen build ignores said under them.
fn show_proton_switches(data: &OptionsData<'_>, sheet: &Sheet) {
    let options = &sheet.options;
    let tools = sheet.tools;
    let epic = sheet.store == Some(StoreKind::Epic);
    data.set_proton_a(model(vec![
        switch(
            XBOX,
            &t!("options-xbox"),
            sheet.noted(t!("options-xbox-about"), &["PROTON_PREFER_SDL"]),
            options.xbox_controllers,
            None,
        ),
        switch(
            WAYLAND,
            &t!("options-wayland"),
            sheet.noted(t!("options-wayland-about"), &["PROTON_ENABLE_WAYLAND"]),
            options.wayland,
            None,
        ),
        switch(HDR, "HDR", t!("options-hdr-about"), options.hdr, None),
        switch(FSR, "FSR", t!("options-fsr-about"), options.fsr, None),
    ]));
    data.set_proton_b(model(vec![switch(
        FPS_COUNTER,
        &t!("options-fps-counter"),
        t!("options-fps-counter-about"),
        options.fps_counter,
        None,
    )]));
    let mut last = vec![
        switch(
            WOW64,
            "WoW64",
            sheet.noted(t!("options-wow64-about"), &["PROTON_USE_WOW64"]),
            options.wow64,
            None,
        ),
        switch(
            NO_SYNC,
            &t!("options-no-sync"),
            sheet.noted(
                t!("options-no-sync-about"),
                &["PROTON_NO_ESYNC", "PROTON_NO_FSYNC"],
            ),
            options.no_sync,
            None,
        ),
    ];
    if epic {
        last.push(switch(
            EOS_OVERLAY,
            &t!("options-eos-overlay"),
            t!("options-eos-overlay-about"),
            options.eos_overlay,
            (!tools.eos_overlay).then(|| t!("options-eos-overlay-missing")),
        ));
    }
    last.push(switch(
        NO_UMU,
        &t!("options-no-umu"),
        t!("options-no-umu-about"),
        options.no_umu,
        (!tools.runtime_4).then(|| t!("options-no-umu-runtime")),
    ));
    data.set_proton_c(model(last));
}

/// The lists of chips, each chip's id being its place in its list.
fn show_choices(data: &OptionsData<'_>, sheet: &Sheet) {
    let options = &sheet.options;
    let chips = |labels: Vec<(String, bool)>| -> ModelRc<ChipData> {
        model(
            labels
                .into_iter()
                .enumerate()
                .map(|(index, (label, chosen))| {
                    chip(i32::try_from(index).unwrap_or(i32::MAX), label, chosen)
                })
                .collect(),
        )
    };
    data.set_protons(chips(
        sheet
            .protons
            .iter()
            .map(|choice| (choice.label.clone(), choice.name == options.proton))
            .collect(),
    ));
    data.set_tasks(chips(
        sheet
            .tasks
            .iter()
            .map(|choice| (choice.label.clone(), choice.key == options.task))
            .collect(),
    ));
    data.set_runtimes(chips(
        sheet
            .runtimes
            .iter()
            .map(|pick| (pick.label.clone(), pick.value == options.steam_runtime))
            .collect(),
    ));
    let strength = options
        .fsr_strength
        .unwrap_or(launch_options::WINE_SHARPENING);
    data.set_fsr_sharpening(chips(
        Sharpening::all()
            .into_iter()
            .map(|sharpening| (sharpening.to_string(), sharpening.0 == strength))
            .collect(),
    ));
    let kind = WindowKind::of(options);
    data.set_windows_kinds(chips(
        WindowKind::ALL
            .into_iter()
            .map(|each| (each.to_string(), each == kind))
            .collect(),
    ));
    let scaler: Option<Scaler> = options.gamescope_scaler;
    data.set_scalers(chips(
        gamescope::scalers()
            .into_iter()
            .map(|pick| (pick.label, pick.value == scaler))
            .collect(),
    ));
    let filter: Option<Filter> = options.gamescope_filter;
    data.set_filters(chips(
        gamescope::filters()
            .into_iter()
            .map(|pick| (pick.label, pick.value == filter))
            .collect(),
    ));
    data.set_sharpness(chips(
        gamescope::sharpness()
            .into_iter()
            .map(|pick| (pick.label, pick.value == options.gamescope_sharpness))
            .collect(),
    ));
}

/// The cloud saves part's buttons, and how the last sync went.
fn show_saves(data: &OptionsData<'_>, sheet: &Sheet) {
    let saves = &sheet.saves;
    let label = |text: String, how: How| {
        if saves.confirm == Some(how) {
            slint::SharedString::from(t!("saves-really", action = text))
        } else {
            slint::SharedString::from(text)
        }
    };
    data.set_sync_labels(model(vec![
        label(t!("saves-sync-now"), How::Both),
        label(t!("saves-upload"), How::ForceUpload),
        label(t!("saves-download"), How::ForceDownload),
    ]));
    data.set_syncing(saves.syncing);
    let (note, good) = saves.note.clone().unwrap_or_default();
    data.set_saves_note(note.into());
    data.set_saves_note_good(good);
}

#[cfg(test)]
mod tests {
    use slint::Model;
    use slint::platform::Key;
    use zendo_core::{Game, Launch, Library, Sort, Source};

    use super::super::tests::page;
    use super::*;
    use crate::{GameData, LibraryData};

    /// A GOG game for Windows, which Zendo starts through its engine.
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

    /// A window whose library is the games given, the first one's page open.
    fn opened(games: Vec<Game>) -> (MainWindow, Rc<Glue>) {
        let (window, glue) = page(0);
        let first = games[0].id.clone();
        let library = Library {
            games,
            problems: Vec::new(),
        };
        let outcome =
            glue.browser
                .borrow_mut()
                .loaded(library, Some(Sort::Title), Vec::new(), Vec::new());
        glue.apply(outcome);
        glue.sync();
        glue.open_game(&first);
        (window, glue)
    }

    fn press(glue: &Rc<Glue>, key: Key) -> bool {
        glue.key(&String::from(char::from(key)), false, false, false)
    }

    #[test]
    fn a_steam_game_has_no_launch_options_button() {
        let steam = Game::new(
            "steam:620".into(),
            "Portal 2".into(),
            Source::Steam,
            Launch::Uri {
                uri: "steam://rungameid/620".into(),
            },
        );
        let (window, _glue) = opened(vec![steam]);
        assert!(!window.global::<GameData>().get_launch_options_shown());
    }

    #[test]
    fn the_button_shows_for_the_games_zendo_starts_and_opens_the_sheet() {
        let (window, _glue) = opened(vec![gog_game()]);
        let game = window.global::<GameData>();
        assert!(game.get_launch_options_shown());
        game.invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        assert!(sheet.get_open());
        assert_eq!(sheet.get_title(), "Sugar");
        assert!(sheet.get_windows() && !sheet.get_native());
        assert_eq!(sheet.get_store(), 1, "GOG");
        assert_eq!(sheet.get_tool_switches().row_count(), 3);
        assert_eq!(sheet.get_start_switches().row_count(), 0, "Epic's only");
        assert_eq!(sheet.get_protons().row_count(), 1, "the default only");
        assert!(sheet.get_protons().row_data(0).unwrap().chosen);
    }

    #[test]
    fn closing_with_changes_asks_and_escape_goes_through_the_same_door() {
        let (window, glue) = opened(vec![gog_game()]);
        window.global::<GameData>().invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        // Unchanged: Escape closes the sheet and leaves the page be.
        window.global::<LibraryData>().invoke_back();
        assert!(!sheet.get_open());
        assert!(window.global::<GameData>().get_open());

        window.global::<GameData>().invoke_launch_options();
        sheet.invoke_toggled(0, true);
        assert!(sheet.get_tool_switches().row_data(0).unwrap().on);
        window.global::<LibraryData>().invoke_back();
        assert!(sheet.get_open() && sheet.get_confirm_close(), "it asks");
        sheet.invoke_keep_editing();
        assert!(!sheet.get_confirm_close());
        sheet.invoke_close();
        assert!(sheet.get_confirm_close());
        sheet.invoke_discard();
        assert!(!sheet.get_open());
        // The keyboard is the page's again.
        assert!(window.global::<GameData>().get_open());
        assert!(glue.options.sheet.borrow().is_none());
    }

    #[test]
    fn a_form_that_does_not_build_stays_open_and_a_good_one_saves() {
        let (window, glue) = opened(vec![gog_game()]);
        window.global::<GameData>().invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        sheet.invoke_toggled(2, true);
        assert!(sheet.get_gamescope_on());
        sheet.invoke_edited(2, "19x20".into());
        assert_eq!(sheet.get_width(), "1920", "digits only");
        sheet.invoke_save();
        assert!(sheet.get_open());
        assert!(
            !sheet.get_gamescope_error().is_empty(),
            "a width needs a height"
        );
        sheet.invoke_edited(9, "-windowed".into());
        sheet.invoke_edited(10, "DXVK_HUD".into());
        sheet.invoke_edited(3, "1080".into());
        sheet.invoke_save();
        assert!(sheet.get_gamescope_error().is_empty());
        assert!(sheet.get_environment_error().contains("NAME=value"));
        sheet.invoke_edited(10, "DXVK_HUD=fps".into());
        // Enter saves with no field focused too.
        assert!(press(&glue, Key::Return));
        assert!(!sheet.get_open());
        assert_eq!(
            window.global::<LibraryData>().get_status(),
            t!("options-saved", title = "Sugar")
        );
    }

    #[test]
    fn over_the_sheet_the_page_has_no_keys_and_the_lists_follow_the_choices() {
        let (window, glue) = opened(vec![gog_game()]);
        window.global::<GameData>().invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        assert!(!press(&glue, Key::DownArrow));
        assert!(!glue.key("x", false, false, false));
        assert!(
            sheet.get_windows_kinds().row_data(0).unwrap().chosen,
            "a window"
        );
        sheet.invoke_choose(4, 2);
        let kinds: Vec<bool> = sheet.get_windows_kinds().iter().map(|c| c.chosen).collect();
        assert_eq!(kinds, [false, false, true]);
        // A scaler is chosen, and "its own" goes back.
        sheet.invoke_choose(5, 2);
        assert!(sheet.get_scalers().row_data(2).unwrap().chosen);
        sheet.invoke_choose(5, 0);
        assert!(sheet.get_scalers().row_data(0).unwrap().chosen);
        // A filter that sharpens shows the sharpness list.
        assert!(!sheet.get_sharpens());
        sheet.invoke_choose(6, 3);
        assert!(sheet.get_sharpens());
        // FSR's list appears with its switch and Wine's default is chosen.
        sheet.invoke_toggled(12, true);
        assert!(sheet.get_fsr_on());
        let chosen: Vec<usize> = sheet
            .get_fsr_sharpening()
            .iter()
            .enumerate()
            .filter(|(_, chip)| chip.chosen)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(chosen, [usize::from(launch_options::WINE_SHARPENING)]);
    }

    #[test]
    fn upload_asks_before_replacing_a_side() {
        let (window, _glue) = opened(vec![gog_game()]);
        window.global::<GameData>().invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        assert_eq!(sheet.get_sync_labels().row_count(), 3);
        sheet.invoke_sync(1);
        let asked = sheet.get_sync_labels().row_data(1).unwrap();
        assert_eq!(asked, t!("saves-really", action = t!("saves-upload")));
        assert!(!sheet.get_syncing());
        sheet.invoke_sync(1);
        assert!(sheet.get_syncing(), "uploading (a test starts nothing)");
    }

    #[test]
    fn the_way_to_start_and_the_runtime_are_chosen_from_their_lists() {
        use zendo_core::steam_runtime::Runtime;
        use zendo_shell::launch_options::{Pick, TaskChoice};

        let (window, glue) = opened(vec![gog_game()]);
        window.global::<GameData>().invoke_launch_options();
        let sheet = window.global::<OptionsData>();
        assert!(!sheet.get_has_tasks(), "the store's own only");
        // The lists come from the store and from Steam's folders, which a
        // test does not read: put an entry of each into the form.
        glue.edit_options(|form| {
            form.tasks.push(TaskChoice {
                key: Some("bin\\cfg.exe".into()),
                label: "Config (bin/cfg.exe)".into(),
            });
            form.runtimes.push(Pick {
                value: None,
                label: "None: the system's libraries".into(),
            });
            form.runtimes.push(Pick {
                value: Some(Runtime::Sniper),
                label: "Steam Linux Runtime 3.0 (sniper)".into(),
            });
        });
        glue.show_options(false);
        assert!(sheet.get_has_tasks());
        sheet.invoke_choose(1, 1);
        let chosen: Vec<bool> = sheet.get_tasks().iter().map(|chip| chip.chosen).collect();
        assert_eq!(chosen, [false, true]);
        sheet.invoke_choose(2, 0);
        sheet.invoke_choose(2, 1);
        let chosen: Vec<bool> = sheet
            .get_runtimes()
            .iter()
            .map(|chip| chip.chosen)
            .collect();
        assert_eq!(chosen, [false, true]);
        {
            let form = glue.options.sheet.borrow();
            let form = form.as_ref().unwrap();
            assert_eq!(form.options.task.as_deref(), Some("bin\\cfg.exe"));
            assert_eq!(form.options.steam_runtime, Some(Runtime::Sniper));
        }
        // The store's own way goes back to the unset key.
        sheet.invoke_choose(1, 0);
        let form = glue.options.sheet.borrow();
        assert_eq!(form.as_ref().unwrap().options.task, None);
    }
}
