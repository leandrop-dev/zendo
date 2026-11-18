//! The Wine prefix card's data: the folder a Windows game Zendo starts runs
//! in, and the fixes the community list has for it. The tools that act on
//! the prefix (Wine's settings, winetricks, programs, GOG's setup) are the
//! next part of this slice; the card shows what there is now.

use serde::Serialize;
use tauri::command;
use zendo_core::config::Config;
use zendo_core::{Game, gog_setup, known_fixes, launch, prefix_tools};

/// What the page's prefix card shows, when it has one.
#[derive(Debug, Serialize)]
pub struct PrefixDto {
    /// The prefix's folder, when it is there yet.
    pub folder: Option<String>,
    /// The fixes' lines, verb or path with its note.
    pub fixes: Vec<String>,
    /// GOG's setup can run in the prefix.
    pub gog: bool,
}

/// The card's data for a game, or `None` when Zendo does not start it in a
/// prefix.
pub(crate) fn prefix_of(game: &Game) -> Option<PrefixDto> {
    if !prefix_tools::applies(game) {
        return None;
    }
    let config = Config::load_file().unwrap_or_default();
    let folder = config
        .install_dir()
        .map(|dir| launch::prefix_in(&dir, game))
        .filter(|prefix| prefix.is_dir())
        .map(|prefix| prefix.to_string_lossy().into_owned());
    let fixes = known_fixes::for_game(game).map_or_else(Vec::new, |fixes| {
        fixes
            .winetricks
            .iter()
            .chain(fixes.run_in_prefix.iter())
            .map(|fix| match fixes.notes.get(fix) {
                Some(note) => format!("{fix}: {note}"),
                None => fix.clone(),
            })
            .collect()
    });
    Some(PrefixDto {
        folder,
        fixes,
        gog: gog_setup::applies(game),
    })
}

/// Opens the prefix's folder, with the desktop's own file manager.
#[command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its argument by value"
)]
pub fn prefix_open(path: String) -> Result<(), String> {
    zendo_shell::open::target(&path)
}
