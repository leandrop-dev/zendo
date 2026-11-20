//! The Launch options sheet over a game's page, for the games Zendo starts
//! itself: the Proton it runs with, `MangoHud`, `GameMode`, gamescope, NVIDIA
//! offload, Proton's switches, the game's arguments and environment, and the
//! scripts before and after it.
//!
//! The form is `zendo_shell::launch_options::Sheet`'s. This part keeps the
//! sheet while it is open and turns it into the rows the page draws.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::Game;
use zendo_core::cloud_saves::{self, How};
use zendo_core::launch_options::{self, Filter};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::launch_options::gamescope::{self, Setting, WindowKind};
use zendo_shell::launch_options::{Field, Press, Sharpening, Sheet, StoreKind};
use zendo_shell::t;

use crate::library::LibraryState;

/// The switches' ids, as the page sends them back (the page's own).
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

/// The form, while its sheet is open.
#[derive(Default)]
pub struct OptionsState {
    sheet: Mutex<Option<Sheet>>,
}

fn lock(state: &OptionsState) -> MutexGuard<'_, Option<Sheet>> {
    state.sheet.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A switch of the form, with the words under it.
#[derive(Debug, Serialize)]
pub struct SwitchDto {
    id: i32,
    label: String,
    about: String,
    on: bool,
    /// False for a switch that cannot be used here (its program is missing),
    /// unless it is already on.
    enabled: bool,
}

/// An entry of a list of choices: its place and whether it is the one.
#[derive(Debug, Serialize)]
pub struct ChipDto {
    id: i32,
    label: String,
    chosen: bool,
}

/// The sheet as the page draws it.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one fact the form is drawn by"
)]
#[derive(Debug, Default, Serialize)]
pub struct OptionsSheetDto {
    open: bool,
    title: String,
    /// "{title} · Launch options".
    heading: String,
    /// A Windows game Zendo installed: Proton and its controller switch
    /// apply.
    windows: bool,
    /// Not a Windows game: the runtime list applies.
    native: bool,
    /// 0 no store, 1 GOG, 2 Epic.
    store: i32,
    has_tasks: bool,
    gamescope_on: bool,
    fsr_on: bool,
    /// gamescope's filter sharpens, so its sharpness is offered.
    sharpens: bool,
    /// Close was pressed with changes: ask before discarding them.
    confirm_close: bool,
    /// The program's file dialog is open.
    picking: bool,
    error: String,
    arguments_error: String,
    environment_error: String,
    gamescope_error: String,
    start_switches: Vec<SwitchDto>,
    saves_switch: Vec<SwitchDto>,
    tool_switches: Vec<SwitchDto>,
    offload_switch: Vec<SwitchDto>,
    proton_a: Vec<SwitchDto>,
    proton_b: Vec<SwitchDto>,
    proton_c: Vec<SwitchDto>,
    protons: Vec<ChipDto>,
    tasks: Vec<ChipDto>,
    runtimes: Vec<ChipDto>,
    fsr_sharpening: Vec<ChipDto>,
    windows_kinds: Vec<ChipDto>,
    scalers: Vec<ChipDto>,
    filters: Vec<ChipDto>,
    sharpness: Vec<ChipDto>,
    /// Sync now, Upload and Download; the pressed one asks again first.
    sync_labels: Vec<String>,
    syncing: bool,
    saves_note: String,
    saves_note_good: bool,
    executable: String,
    language: String,
    width: String,
    height: String,
    frame_cap: String,
    game_width: String,
    game_height: String,
    fps: String,
    more: String,
    arguments: String,
    environment: String,
    before: String,
    after: String,
}

/// One switch row; `unusable` says why it cannot be turned on here.
fn switch(id: i32, label: &str, about: String, on: bool, unusable: Option<String>) -> SwitchDto {
    let enabled = unusable.is_none() || on;
    let about = match unusable {
        Some(why) => format!("{about} {why}"),
        None => about,
    };
    SwitchDto {
        id,
        label: label.to_owned(),
        about,
        on,
        enabled,
    }
}

/// The chips of a list, each chip's id being its place in it.
fn chips(labels: impl IntoIterator<Item = (String, bool)>) -> Vec<ChipDto> {
    labels
        .into_iter()
        .enumerate()
        .map(|(at, (label, chosen))| ChipDto {
            id: i32::try_from(at).unwrap_or(i32::MAX),
            label,
            chosen,
        })
        .collect()
}

/// The switches Proton, Wine and DXVK read, for a Windows game, with what
/// the chosen build ignores said under them.
fn proton_switches(sheet: &Sheet, epic: bool) -> (Vec<SwitchDto>, Vec<SwitchDto>, Vec<SwitchDto>) {
    let options = &sheet.options;
    let tools = sheet.tools;
    let first = vec![
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
    ];
    let middle = vec![switch(
        FPS_COUNTER,
        &t!("options-fps-counter"),
        t!("options-fps-counter-about"),
        options.fps_counter,
        None,
    )];
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
    (first, middle, last)
}

/// The switches above the Proton ones: the store's, cloud saves and the
/// wrappers.
fn tool_switches(sheet: &Sheet, epic: bool) -> (Vec<SwitchDto>, Vec<SwitchDto>, Vec<SwitchDto>) {
    let options = &sheet.options;
    let tools = sheet.tools;
    let missing = |installed: bool| (!installed).then(|| t!("options-not-installed"));
    let mangohud_ready = if options.gamescope {
        tools.mangoapp
    } else {
        tools.mangohud
    };
    let start = if epic {
        let offline = match sheet.offline_approved {
            Some(false) => format!(
                "{} {}",
                t!("options-offline-about"),
                t!("options-offline-not-approved")
            ),
            _ => t!("options-offline-about"),
        };
        vec![
            switch(
                OFFLINE,
                &t!("options-offline"),
                offline,
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
    };
    let saves = vec![switch(
        CLOUD_SAVES,
        &t!("saves-around"),
        t!("saves-around-about"),
        options.syncs_saves(),
        None,
    )];
    let tools = vec![
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
    ];
    (start, saves, tools)
}

/// The lists of chips, from the sheet's own.
fn choices(sheet: &Sheet) -> Vec<Vec<ChipDto>> {
    let options = &sheet.options;
    let strength = options
        .fsr_strength
        .unwrap_or(launch_options::WINE_SHARPENING);
    let kind = WindowKind::of(options);
    let scaler = options.gamescope_scaler;
    let filter: Option<Filter> = options.gamescope_filter;
    vec![
        chips(
            sheet
                .protons
                .iter()
                .map(|choice| (choice.label.clone(), choice.name == options.proton)),
        ),
        chips(
            sheet
                .tasks
                .iter()
                .map(|choice| (choice.label.clone(), choice.key == options.task)),
        ),
        chips(
            sheet
                .runtimes
                .iter()
                .map(|pick| (pick.label.clone(), pick.value == options.steam_runtime)),
        ),
        chips(
            Sharpening::all()
                .into_iter()
                .map(|sharpening| (sharpening.to_string(), sharpening.0 == strength)),
        ),
        chips(
            WindowKind::ALL
                .into_iter()
                .map(|each| (each.to_string(), each == kind)),
        ),
        chips(
            gamescope::scalers()
                .into_iter()
                .map(|pick| (pick.label, pick.value == scaler)),
        ),
        chips(
            gamescope::filters()
                .into_iter()
                .map(|pick| (pick.label, pick.value == filter)),
        ),
        chips(
            gamescope::sharpness()
                .into_iter()
                .map(|pick| (pick.label, pick.value == options.gamescope_sharpness)),
        ),
    ]
}

fn error_in(sheet: &Sheet, field: Field) -> String {
    sheet
        .error
        .as_ref()
        .filter(|(which, _)| *which == field)
        .map_or_default(|(_, text)| text.clone())
}

/// The sheet as the page draws it.
fn sheet_dto(sheet: &Sheet) -> OptionsSheetDto {
    let options = &sheet.options;
    let epic = sheet.store == Some(StoreKind::Epic);
    let (start_switches, saves_switch, tool_switches) = tool_switches(sheet, epic);
    let (proton_a, proton_b, proton_c) = proton_switches(sheet, epic);
    let mut lists = choices(sheet).into_iter();
    let label = |text: String, how: How| {
        if sheet.saves.confirm == Some(how) {
            t!("saves-really", action = text)
        } else {
            text
        }
    };
    let (saves_note, saves_note_good) = sheet.saves.note.clone().unwrap_or_default();
    OptionsSheetDto {
        open: true,
        title: sheet.title.clone(),
        heading: t!("options-title", title = sheet.title.as_str()),
        windows: sheet.windows,
        native: !sheet.windows,
        store: match sheet.store {
            None => 0,
            Some(StoreKind::Gog) => 1,
            Some(StoreKind::Epic) => 2,
        },
        has_tasks: sheet.tasks.len() > 1,
        gamescope_on: options.gamescope,
        fsr_on: options.fsr,
        sharpens: options.gamescope_filter.is_some_and(Filter::sharpens),
        confirm_close: sheet.confirm_close,
        picking: sheet.picking,
        error: error_in(sheet, Field::Other),
        arguments_error: error_in(sheet, Field::Arguments),
        environment_error: error_in(sheet, Field::Environment),
        gamescope_error: error_in(sheet, Field::Gamescope),
        start_switches,
        saves_switch,
        tool_switches,
        offload_switch: vec![switch(
            OFFLOAD,
            &t!("options-offload"),
            t!("options-offload-about"),
            options.nvidia_offload,
            (!sheet.tools.offload).then(|| t!("options-offload-none")),
        )],
        proton_a,
        proton_b,
        proton_c,
        protons: lists.next().unwrap_or_default(),
        tasks: lists.next().unwrap_or_default(),
        runtimes: lists.next().unwrap_or_default(),
        fsr_sharpening: lists.next().unwrap_or_default(),
        windows_kinds: lists.next().unwrap_or_default(),
        scalers: lists.next().unwrap_or_default(),
        filters: lists.next().unwrap_or_default(),
        sharpness: lists.next().unwrap_or_default(),
        sync_labels: vec![
            label(t!("saves-sync-now"), How::Both),
            label(t!("saves-upload"), How::ForceUpload),
            label(t!("saves-download"), How::ForceDownload),
        ],
        syncing: sheet.saves.syncing,
        saves_note,
        saves_note_good,
        executable: sheet.executable.clone(),
        language: sheet.language.clone(),
        width: sheet.width.clone(),
        height: sheet.height.clone(),
        frame_cap: sheet.frame_cap.clone(),
        game_width: sheet.gamescope.game_width.clone(),
        game_height: sheet.gamescope.game_height.clone(),
        fps: sheet.gamescope.fps.clone(),
        more: options.gamescope_more.clone(),
        arguments: options.arguments.clone(),
        environment: options.environment.clone(),
        before: options.before.clone(),
        after: options.after.clone(),
    }
}

/// The sheet as it stands; closed when none is open.
fn dto(app: &AppHandle) -> OptionsSheetDto {
    let state = app.state::<OptionsState>();
    let slot = lock(&state);
    match slot.as_ref() {
        Some(sheet) => sheet_dto(sheet),
        None => OptionsSheetDto::default(),
    }
}

/// The sheet was written to, or the page asked for it: the rows follow.
fn changed(app: &AppHandle) {
    let _ = app.emit("options_changed", ());
}

/// A change of the form, when one is open.
fn edit(app: &AppHandle, change: impl FnOnce(&mut Sheet)) {
    let state = app.state::<OptionsState>();
    let mut slot = lock(&state);
    if let Some(sheet) = slot.as_mut() {
        change(sheet);
    }
}

/// The page opened on another game, or closed: the sheet goes.
pub fn reset(app: &AppHandle) {
    let state = app.state::<OptionsState>();
    *lock(&state) = None;
}

fn game_of(app: &AppHandle, id: &str) -> Option<Game> {
    let library = app.state::<LibraryState>();
    library.game(id)
}

#[tauri::command]
pub fn options_get(app: AppHandle) -> OptionsSheetDto {
    dto(&app)
}

/// The Launch options button: the form for the game, or closed when Zendo
/// does not start it.
#[tauri::command]
pub fn options_open(id: String, app: AppHandle) -> Result<OptionsSheetDto, String> {
    let game = game_of(&app, &id).ok_or_else(|| format!("no game with the id {id:?}"))?;
    let sheet = Sheet::open(&game, true)?;
    {
        let state = app.state::<OptionsState>();
        *lock(&state) = sheet;
    }
    Ok(dto(&app))
}

/// Escape, Cancel and a click outside: closes the sheet, or asks first when
/// closing would discard changes.
#[tauri::command]
pub fn options_close(app: AppHandle) -> OptionsSheetDto {
    {
        let state = app.state::<OptionsState>();
        let mut slot = lock(&state);
        if let Some(sheet) = slot.as_mut()
            && sheet.close()
        {
            *slot = None;
        }
    }
    dto(&app)
}

#[tauri::command]
pub fn options_discard(app: AppHandle) -> OptionsSheetDto {
    {
        let state = app.state::<OptionsState>();
        *lock(&state) = None;
    }
    dto(&app)
}

#[tauri::command]
pub fn options_keep_editing(app: AppHandle) -> OptionsSheetDto {
    edit(&app, |sheet| sheet.confirm_close = false);
    dto(&app)
}

/// Save: writes the options and closes the sheet, or stays open with what is
/// wrong said beside Save.
#[derive(Debug, Serialize)]
pub struct SaveDto {
    sheet: OptionsSheetDto,
    saved: bool,
    title: String,
}

#[tauri::command]
pub fn options_save(app: AppHandle) -> SaveDto {
    let saved = {
        let state = app.state::<OptionsState>();
        let mut slot = lock(&state);
        match slot.as_mut() {
            None => None,
            Some(sheet) => sheet
                .save(true)
                .then(|| (sheet.id.clone(), sheet.title.clone())),
        }
    };
    let Some((id, title)) = saved else {
        return SaveDto {
            sheet: dto(&app),
            saved: false,
            title: String::new(),
        };
    };
    {
        let state = app.state::<OptionsState>();
        *lock(&state) = None;
    }
    // The details' cloud saves row follows the switch.
    crate::game::emit_game_changed(&app, &id);
    changed(&app);
    SaveDto {
        sheet: dto(&app),
        saved: true,
        title,
    }
}

#[tauri::command]
pub fn options_toggle(id: i32, on: bool, app: AppHandle) -> OptionsSheetDto {
    edit(&app, |sheet| {
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
    dto(&app)
}

/// A chip was pressed: the list (0 Proton, 1 task, 2 runtime, 3 FSR
/// sharpening, 4 window, 5 scaler, 6 filter, 7 sharpness) and the chip's id.
#[tauri::command]
pub fn options_choose(list: i32, index: i32, app: AppHandle) -> OptionsSheetDto {
    let Ok(index) = usize::try_from(index) else {
        return dto(&app);
    };
    edit(&app, |sheet| match list {
        0 => {
            if let Some(choice) = sheet.protons.get(index) {
                let name = choice.name.clone();
                sheet.set_proton(name, true);
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
    dto(&app)
}

/// A field was typed in (0 executable, 1 language, 2 width, 3 height, 4 frame
/// cap, 5 game width, 6 game height, 7 fps, 8 more, 9 arguments, 10
/// environment, 11 before, 12 after). A field that takes digits only keeps
/// them, and `echo` says what to write back.
#[derive(Debug, Serialize)]
pub struct EditDto {
    sheet: OptionsSheetDto,
    echo: Option<String>,
}

#[tauri::command]
pub fn options_edit(field: i32, text: String, app: AppHandle) -> EditDto {
    let digits_only = matches!(field, 2..=7);
    let kept = if digits_only {
        zendo_shell::launch_options::digits(&text)
    } else {
        text.clone()
    };
    let echo = (kept != text).then(|| kept.clone());
    edit(&app, |sheet| match field {
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
    EditDto {
        sheet: dto(&app),
        echo,
    }
}

/// Sync now, Upload and Download: the sync runs on a thread of its own; the
/// last two ask once more first.
#[tauri::command]
pub fn options_sync(which: i32, app: AppHandle) -> OptionsSheetDto {
    let how = match which {
        0 => How::Both,
        1 => How::ForceUpload,
        _ => How::ForceDownload,
    };
    let (press, id) = {
        let state = app.state::<OptionsState>();
        let mut slot = lock(&state);
        match slot.as_mut() {
            None => return dto(&app),
            Some(sheet) => (sheet.saves.press(how), sheet.id.clone()),
        }
    };
    if press != Press::Start {
        return dto(&app);
    }
    let Some(game) = game_of(&app, &id) else {
        edit(&app, |sheet| sheet.saves.syncing = false);
        return dto(&app);
    };
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = cloud_saves::sync(&game, how).map_err(|err| format!("{err:#}"));
        let store = zendo_shell::format::source(game.source);
        {
            let state = handle.state::<OptionsState>();
            let mut slot = lock(&state);
            if let Some(sheet) = slot.as_mut() {
                sheet.saves.synced(result, &store);
            }
        }
        // The sheet may have closed while it ran; the page's row still moves.
        crate::game::emit_game_changed(&handle, &game.id);
        changed(&handle);
    });
    dto(&app)
}

/// Browse: the file dialog runs on a thread of its own; the program picked is
/// kept relative to the game's folder.
#[tauri::command]
pub fn options_browse(app: AppHandle) -> OptionsSheetDto {
    {
        let state = app.state::<OptionsState>();
        let mut slot = lock(&state);
        let Some(sheet) = slot.as_mut() else {
            return dto(&app);
        };
        if sheet.picking {
            return dto(&app);
        }
        sheet.picking = true;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = file_dialog::pick(Kind::Program, &t!("options-executable-choose"))
            .map_err(|err| format!("{err:#}"));
        {
            let state = handle.state::<OptionsState>();
            let mut slot = lock(&state);
            if let Some(sheet) = slot.as_mut() {
                sheet.picking = false;
                match result {
                    Ok(Some(path)) => sheet.keep_picked_program(&path),
                    Ok(None) => {}
                    Err(err) => sheet.error = Some((Field::Other, err)),
                }
            }
        }
        changed(&handle);
    });
    dto(&app)
}
