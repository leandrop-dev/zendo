//! `zendo-tauri`: Zendo's window in Tauri. The frontend is the `SvelteKit` app
//! in `ui/`, served over IPC from the `WebKitGTK` view.
//!
//! Slice 0 of `docs/tauri-port-plan.md`: the window and its title, the dev
//! loop, and a placeholder page. The commands and the events (slice 1 on) are
//! thin wrappers over `zendo-core`, and the tray and the controller stay in
//! `zendo-shell`.

fn main() {
    // The window's class on X11 (its WM_CLASS) and its app id on Wayland come
    // from GTK's program name, which GTK takes from the binary (`zendo-tauri`
    // in development). The class the compositors' rules match is `zendo`:
    // Hyprland floats it and both desktops blur it.
    gtk::glib::set_prgname(Some("zendo"));

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("the Tauri window failed to start");
}
