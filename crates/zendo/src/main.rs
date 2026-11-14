//! `zendo`: the library window, in Iced. The command line, the helper windows
//! and the parts that need no toolkit are in `zendo-shell`.

mod app;

// The toolkit-free parts live in `zendo-shell`; the window's code refers to
// them as `crate::i18n`, `crate::tray` and so on.
pub(crate) use zendo_shell::{file_dialog, gamepad, i18n, links, presence, sign_in_window, tray};

use std::process::ExitCode;

fn main() -> ExitCode {
    zendo_shell::cli::main(app::run)
}
