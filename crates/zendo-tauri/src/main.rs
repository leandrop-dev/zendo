//! `zendo-tauri`: Zendo's window in Tauri. The frontend is the `SvelteKit` app
//! in `ui/`, served over IPC from the `WebKitGTK` view.
//!
//! Slice 1 of `docs/tauri-port-plan.md` is the shell: the window's words and
//! colours, hiding to the tray, the controller, and the command line. The
//! commands are thin wrappers over `zendo-core` and `zendo-shell`, and the
//! tray, the controller and the words stay in `zendo-shell`.

use std::process::ExitCode;

use anyhow::Context;
use tauri::Manager;

mod accounts;
mod cards;
mod cloud;
mod collections;
mod commands;
mod downloads;
mod engines;
mod game;
mod launchers;
mod library;
mod local;
mod mods;
mod platform;
mod prefix;
mod presence;
mod protons;
mod settings;
mod steam;
mod store;

fn main() -> ExitCode {
    // The window's class on X11 (its WM_CLASS) and its app id on Wayland come
    // from GTK's program name, which GTK takes from the binary (`zendo-tauri`
    // in development). The class the compositors' rules match is `zendo`:
    // Hyprland floats it and both desktops blur it.
    gtk::glib::set_prgname(Some(zendo_shell::APP_ID));

    zendo_shell::cli::main(run_window)
}

/// Opens the window: `fullscreen` is Game Mode's start (slice 6), and `link`
/// the `nxm://` link it was started for (slice 7).
#[expect(
    clippy::needless_pass_by_value,
    reason = "zendo_shell::cli::RunWindow fixes the signature"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the command list is the one place every command is registered"
)]
fn run_window(fullscreen: bool, link: Option<String>) -> anyhow::Result<()> {
    // Nexus links arrive with the Nexus pages.
    let _ = link;

    zendo_shell::i18n::set_from_settings();
    // The settings alone, without the keyring: starting the window must not
    // ask the desktop for its keys.
    let config = zendo_core::config::Config::load_file().unwrap_or_else(|err| {
        eprintln!("zendo: {err:#}");
        zendo_core::config::Config::default()
    });
    let themes = zendo_shell::theme::Themes::chosen(
        config.theme.as_deref(),
        zendo_core::theme::Theme::dir().as_deref(),
    );

    tauri::Builder::default()
        .manage(platform::Shell::new(themes, config.minimize_to_tray))
        .manage(library::LibraryState::new())
        .manage(cards::CardsState::default())
        .manage(prefix::PrefixState::default())
        .manage(cloud::CloudState::default())
        .manage(mods::ModsHandle::default())
        .manage(settings::SettingsState::new())
        .manage(presence::PresenceState::new(config.discord_presence))
        .manage(launchers::LaunchersState::default())
        .manage(engines::EnginesState::default())
        .manage(engines::RuntimesState::default())
        .manage(engines::OverlayState::default())
        .manage(protons::ProtonsState::default())
        .manage(accounts::AccountsState::default())
        .manage(collections::CollectionsState::default())
        .manage(store::StoreState::default())
        .manage(downloads::DownloadsState::default())
        .manage(steam::SteamState::default())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::tr,
            commands::window_fullscreen_toggle,
            commands::open_link,
            library::library_list,
            library::library_pick,
            library::library_search,
            library::library_cycle_sort,
            library::library_click,
            library::library_step,
            library::library_fit,
            library::library_back,
            library::library_show_hidden,
            library::library_toggle_mark,
            game::game_get,
            game::game_launch,
            game::game_stop,
            game::game_set_pinned,
            game::game_set_hidden,
            game::game_set_status,
            local::local_get,
            local::local_about,
            local::local_pick_program,
            local::local_save,
            local::local_remove,
            prefix::prefix_open,
            prefix::prefix_run,
            prefix::prefix_install,
            prefix::prefix_pick_program,
            prefix::prefix_want,
            mods::mods_get,
            mods::mods_begin,
            mods::mods_toggle_mod,
            mods::mods_move_mod,
            mods::mods_toggle_plugin,
            mods::mods_move_plugin,
            mods::mods_remove,
            mods::mods_clear_overwrite,
            mods::mods_folder_save,
            mods::mods_folder_known,
            mods::mods_profile_switch,
            mods::mods_profile_naming,
            mods::mods_profile_save,
            mods::mods_profile_cancel,
            mods::mods_profile_delete,
            mods::mods_apply,
            mods::mods_add,
            mods::mods_find_nexus,
            settings::settings_get,
            settings::settings_set_language,
            settings::settings_set_theme,
            settings::settings_set_tray,
            settings::settings_set_discord,
            settings::settings_set_covers,
            settings::settings_set_auto_update,
            settings::settings_set_workers,
            settings::settings_set_path_first,
            settings::settings_default_chosen,
            settings::settings_download_covers,
            settings::settings_copy_report,
            settings::settings_open,
            settings::settings_key_edited,
            settings::settings_key_toggle_shown,
            settings::settings_key_check,
            settings::settings_key_save,
            settings::settings_key_get,
            settings::settings_nexus_key_edited,
            settings::settings_nexus_key_toggle_shown,
            settings::settings_nexus_key_check,
            settings::settings_nexus_key_save,
            settings::settings_nexus_key_get,
            settings::settings_nexus_become_handler,
            launchers::settings_launcher_act,
            launchers::settings_launchers_stop,
            engines::settings_engines_check,
            engines::settings_engine_act,
            engines::settings_engine_remove,
            engines::settings_engines_stop,
            engines::settings_runtime_act,
            engines::settings_runtime_remove,
            engines::settings_overlay_do_install,
            engines::settings_overlay_do_remove,
            protons::settings_proton_remove,
            protons::settings_build_chosen,
            protons::settings_proton_refresh,
            protons::settings_proton_show_older,
            protons::settings_release_download,
            protons::settings_proton_stop,
            accounts::accounts_get,
            accounts::accounts_check,
            accounts::accounts_start,
            accounts::accounts_sign_out,
            accounts::accounts_cancel,
            accounts::accounts_use_browser,
            accounts::accounts_code_edited,
            accounts::accounts_finish,
            accounts::accounts_steam_sign_in,
            accounts::accounts_steam_cancel,
            accounts::accounts_steam_sign_out,
            accounts::accounts_xbox_open,
            accounts::accounts_set_gog_playtime,
            accounts::accounts_set_gog_galaxy,
            collections::collections_open,
            collections::collections_close,
            collections::collections_get,
            collections::collections_draft,
            collections::collections_rename_draft,
            collections::collections_create,
            collections::collections_save_view,
            collections::collections_rename,
            collections::collections_rename_done,
            collections::collections_set_member,
            collections::collections_delete,
            collections::collections_apply,
            store::store_get,
            store::store_pick,
            store::store_search,
            store::store_cycle_sort,
            store::store_click,
            store::store_step,
            store::store_fit,
            store::store_back,
            store::store_refresh,
            store::store_accounts_changed,
            store::owned_get,
            downloads::downloads_get,
            downloads::downloads_pause,
            downloads::downloads_resume,
            downloads::downloads_cancel,
            downloads::downloads_clear,
            downloads::downloads_open,
            downloads::downloads_close_anyway,
            downloads::downloads_stay_open,
            downloads::owned_install,
            downloads::owned_import,
            downloads::owned_resume,
            downloads::owned_toggle_dlc,
            downloads::owned_forget,
            steam::steam_stop_waiting,
            steam::steam_open_downloads,
            cards::cards_get,
            cards::cards_retry,
            cards::cards_more_achievements,
            cards::cards_more_changelog,
            cards::steam_app_set,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                platform::start(app.handle(), &window, fullscreen)?;
            }
            library::start(app.handle().clone());
            downloads::start(app.handle().clone());
            steam::start(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .context("the Tauri window failed to start")
}
