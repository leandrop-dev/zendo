//! Choosing a file in the desktop's own dialog:
//! `zendo pick-file <kind> <title>`, where the kind is `program` (from
//! `~/Games`), `mod` (an archive, from `~/Downloads`) or `installer` (a
//! Windows program for a game's prefix, from `~/Downloads`);
//! a second process like the sign-in window, since GTK needs its own loop
//! apart from Iced's. GTK's native chooser goes through the desktop portal
//! when there is one, so it is the dialog every other app shows. The path
//! chosen goes to stdout; closing it writes nothing.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, bail};
use gtk::prelude::*;

/// What is being chosen, which picks the folder the dialog starts in and
/// the files it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A game's program.
    Program,
    /// A mod's archive.
    Mod,
    /// A Windows program to run in a game's prefix: an installer, a patch.
    Installer,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Mod => "mod",
            Self::Installer => "installer",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Program, Self::Mod, Self::Installer]
            .into_iter()
            .find(|kind| kind.name() == name)
    }

    fn start(self) -> &'static str {
        match self {
            Self::Program => "Games",
            Self::Mod | Self::Installer => "Downloads",
        }
    }
}

/// Windows programs and installers, as either case spells them.
const WINDOWS_PROGRAMS: [&str; 4] = ["*.exe", "*.EXE", "*.msi", "*.MSI"];

/// The archives 7-Zip unpacks that mods come in.
const ARCHIVES: [&str; 9] = [
    "*.zip",
    "*.7z",
    "*.rar",
    "*.tar",
    "*.tar.gz",
    "*.tgz",
    "*.tar.xz",
    "*.tar.bz2",
    "*.tar.zst",
];

/// `zendo pick-file <kind> <title>`: shows the dialog, prints the file
/// chosen.
pub fn main(kind: Kind, title: &str) -> ExitCode {
    crate::i18n::set_from_settings();
    gtk::glib::set_prgname(Some(crate::app::APP_ID));
    gtk::glib::set_application_name("Zendo");
    if let Err(err) = gtk::init() {
        eprintln!("zendo: could not start GTK: {err}");
        return ExitCode::FAILURE;
    }
    let dialog = gtk::FileChooserNative::new(
        Some(title),
        None::<&gtk::Window>,
        gtk::FileChooserAction::Open,
        Some(&crate::i18n::t!("dialog-choose")),
        Some(&crate::i18n::t!("cancel")),
    );
    if let Some(home) = std::env::var_os("HOME") {
        let folder = PathBuf::from(home).join(kind.start());
        let start = if folder.is_dir() {
            folder
        } else {
            folder.parent().map(PathBuf::from).unwrap_or_default()
        };
        let _ = dialog.set_current_folder(start);
    }
    let shown = match kind {
        Kind::Mod => Some((crate::i18n::t!("dialog-archives"), &ARCHIVES[..])),
        Kind::Installer => Some((
            crate::i18n::t!("dialog-windows-programs"),
            &WINDOWS_PROGRAMS[..],
        )),
        Kind::Program => None,
    };
    if let Some((name, patterns)) = shown {
        let files = gtk::FileFilter::new();
        files.set_name(Some(&name));
        for pattern in patterns {
            files.add_pattern(pattern);
        }
        dialog.add_filter(files);
        let all = gtk::FileFilter::new();
        all.set_name(Some(&crate::i18n::t!("dialog-all-files")));
        all.add_pattern("*");
        dialog.add_filter(all);
    }
    if dialog.run() != gtk::ResponseType::Accept {
        return ExitCode::SUCCESS;
    }
    let Some(path) = dialog.file().and_then(|file| file.path()) else {
        return ExitCode::SUCCESS;
    };
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{}", path.display())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Opens the dialog and waits for it: the file chosen, or `None` when it
/// was closed. It blocks; the window calls it on a thread.
///
/// # Errors
///
/// When the dialog cannot be started or fails.
pub fn pick(kind: Kind, title: &str) -> anyhow::Result<Option<PathBuf>> {
    let exe = std::env::current_exe().context("could not find Zendo's own program")?;
    let output = Command::new(exe)
        .args(["pick-file", kind.name(), title])
        .stdin(Stdio::null())
        .output()
        .context("could not open the file dialog")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("zendo: "))
            .map_or_else(|| output.status.to_string(), str::to_owned);
        bail!("the file dialog failed: {reason}");
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_owned();
    Ok((!path.is_empty()).then(|| PathBuf::from(path)))
}
