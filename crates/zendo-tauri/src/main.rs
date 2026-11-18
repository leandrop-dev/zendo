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

mod commands;
mod game;
mod library;
mod local;
mod platform;
mod prefix;

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
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::tr,
            commands::window_fullscreen_toggle,
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
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                platform::start(app.handle(), &window, fullscreen)?;
            }
            library::start(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .context("the Tauri window failed to start")
}
