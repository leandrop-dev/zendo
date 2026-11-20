//! Tauri's build step: it reads `tauri.conf.json`, generates the permissions
//! schemas in `gen/schemas` and embeds the window's icons.

fn main() {
    tauri_build::build();
}
