//! `zendo-slint`: Zendo's window in Slint, the port from Iced (items 47 to 54
//! in `docs/backlog.md`). It becomes the `zendo` binary at the cut-over. The
//! command line, the helper windows and everything else that needs no toolkit
//! are in `zendo-shell`.

mod covers;
mod library;
mod library_ui;
mod theme;
mod tr;
mod window;

use std::process::ExitCode;

slint::include_modules!();

fn main() -> ExitCode {
    zendo_shell::cli::main(window::run)
}
