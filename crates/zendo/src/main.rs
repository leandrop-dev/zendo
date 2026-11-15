//! `zendo`: Zendo's window, in Slint. The command line, the helper windows and
//! everything else that needs no toolkit are in `zendo-shell`.

mod covers;
mod library;
mod library_ui;
mod store;
mod theme;
mod tr;
mod window;

use std::process::ExitCode;

slint::include_modules!();

fn main() -> ExitCode {
    zendo_shell::cli::main(window::run)
}
