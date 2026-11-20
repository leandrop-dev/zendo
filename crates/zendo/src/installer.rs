//! A mod's FOMOD installer sheet: its steps one at a time, the options of each
//! group as radios or boxes, the option last touched described with its
//! picture, then Install builds the mod. Mods whose installer comes while one
//! is open wait their turn.
//!
//! The sheet and its rules are [`zendo_shell::installer`]'s: this part keeps
//! it while it is open, turns it into what the page draws, and builds the mod
//! through the Mods card's `Work::Install`.
//! The sheet is over every page: a download that ends anywhere can bring one
//! up, which is what `installer_changed` says.
//!
//! The picture of the option described is a file inside the mod's unpacked
//! folder, which the asset protocol does not reach: it is copied under
//! `$CACHE/zendo/installer-pictures` as it is described, one at a time.

#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its arguments by value"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use zendo_core::mods::{self, Pending};
use zendo_shell::installer::{InstallerState, Opening, Ready};
use zendo_shell::t;

use crate::library::LibraryState;
use crate::mods::ModsHandle;

/// The sheet while it is open, and the picture it draws.
#[derive(Debug, Default)]
pub struct InstallerHandle {
    state: Mutex<InstallerState>,
    picture: Mutex<Picture>,
    /// Orders the picture files, so a new one is not served from the last
    /// one's place in the cache.
    counter: AtomicU64,
}

impl InstallerHandle {
    fn lock(&self) -> MutexGuard<'_, InstallerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn picture(&self) -> MutexGuard<'_, Picture> {
        self.picture.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The picture of the option described: the file the sheet asked for, and the
/// copy the page draws.
#[derive(Debug, Default)]
struct Picture {
    asked: Option<PathBuf>,
    served: Option<Served>,
}

#[derive(Debug)]
struct Served {
    /// The file's path, as the page's asset URL takes it.
    url: String,
    /// Where the copy is, to clean it up.
    file: PathBuf,
}

/// An option of a group: a radio when its group takes one, else a box.
#[derive(Debug, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one fact the row is drawn by"
)]
pub struct OptionDto {
    group: usize,
    option: usize,
    label: String,
    single: bool,
    on: bool,
    /// A click changes it.
    enabled: bool,
    /// Plain dim text: the mod says it cannot be used.
    dim: bool,
    /// The option last touched, or under the cursor.
    focused: bool,
}

#[derive(Debug, Serialize)]
pub struct GroupDto {
    /// The group's name and its rule: "Name · choose one".
    title: String,
    options: Vec<OptionDto>,
}

/// The sheet as the page draws it.
#[derive(Debug, Default, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one fact the sheet is drawn by"
)]
pub struct InstallerSheetDto {
    open: bool,
    title: String,
    /// "Step 1 of 2 · Name".
    step_line: String,
    /// The mod says it needs files that were not found.
    needs: bool,
    groups: Vec<GroupDto>,
    /// The picture of the option described, once it is in the cache.
    picture: Option<String>,
    about_name: String,
    about_text: String,
    back_enabled: bool,
    /// Install, not Next.
    final_step: bool,
    /// Install on the last step, else Next.
    primary_label: String,
}

/// The sheet as it stands; closed when none is open.
fn dto(app: &AppHandle) -> InstallerSheetDto {
    let handle = app.state::<InstallerHandle>();
    let state = handle.lock();
    let Some(view) = state.view() else {
        // The mod is built or dropped: its picture has nothing to describe.
        picture_of(&handle, None);
        return InstallerSheetDto::default();
    };
    let picture = picture_of(&handle, view.about.picture.as_deref());
    InstallerSheetDto {
        open: true,
        title: view.title,
        step_line: view.step_line,
        needs: view.needs_warning,
        groups: view
            .groups
            .iter()
            .enumerate()
            .map(|(group, shown)| GroupDto {
                title: shown.title.clone(),
                options: shown
                    .options
                    .iter()
                    .enumerate()
                    .map(|(option, each)| OptionDto {
                        group,
                        option,
                        label: each.label.clone(),
                        single: each.single,
                        on: each.on,
                        enabled: each.enabled,
                        dim: each.dim,
                        focused: each.focused,
                    })
                    .collect(),
            })
            .collect(),
        picture,
        about_name: view.about.name,
        about_text: view.about.text,
        back_enabled: view.back_enabled,
        final_step: view.final_step,
        primary_label: if view.final_step {
            t!("installer-install")
        } else {
            t!("installer-next")
        },
    }
}

/// The copy of the picture the sheet describes, made or dropped as the
/// described option changes.
fn picture_of(handle: &InstallerHandle, wanted: Option<&Path>) -> Option<String> {
    let mut kept = handle.picture();
    if kept.asked.as_deref() == wanted {
        return kept.served.as_ref().map(|served| served.url.clone());
    }
    if let Some(served) = kept.served.take() {
        let _ = fs::remove_file(served.file);
    }
    kept.asked = wanted.map(Path::to_path_buf);
    kept.served = wanted.and_then(|path| copy_picture(handle, path));
    kept.served.as_ref().map(|served| served.url.clone())
}

/// A copy of a picture in the cache, under a name no earlier one had.
fn copy_picture(handle: &InstallerHandle, path: &Path) -> Option<Served> {
    let dir = zendo_shell::artwork::thumb_dir()?.with_file_name("installer-pictures");
    fs::create_dir_all(&dir).ok()?;
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map_or_else(|| "png".to_owned(), str::to_lowercase);
    let number = handle.counter.fetch_add(1, Ordering::Relaxed);
    let file = dir.join(format!("picture-{number}.{extension}"));
    fs::copy(path, &file).ok()?;
    Some(Served {
        url: file.display().to_string(),
        file,
    })
}

/// The sheet was opened, or closed, without a command saying so.
fn changed(app: &AppHandle) {
    let _ = app.emit("installer_changed", ());
}

/// A line for the window's foot.
fn say(app: &AppHandle, line: String) {
    let _ = app.emit("installer_said", line);
}

/// A mod's installer is to be run: its sheet opens, or it waits for the slot,
/// or (with no step to show) it installs as it is.
pub(crate) fn opened(app: &AppHandle, game: String, pending: Box<Pending>) {
    let free = app.state::<ModsHandle>().lock().free();
    let opening = app
        .state::<InstallerHandle>()
        .lock()
        .open(game, pending, free);
    handle_opening(app, opening);
}

fn handle_opening(app: &AppHandle, opening: Opening) {
    match opening {
        Opening::Waits => {}
        Opening::Shown(status) => {
            say(app, status);
            changed(app);
        }
        Opening::InstallNow(ready) => build(app, ready),
    }
}

/// The slot is free: the installer that waited longest opens.
pub(crate) fn next_waiting(app: &AppHandle) {
    let free = app.state::<ModsHandle>().lock().free();
    let opening = app.state::<InstallerHandle>().lock().next_waiting(free);
    if let Some(opening) = opening {
        handle_opening(app, opening);
    }
}

/// Builds the mod with the options chosen, on a thread of the mods'.
fn build(app: &AppHandle, ready: Ready) {
    let Ready {
        game,
        pending,
        selection,
    } = ready;
    let library = app.state::<LibraryState>();
    let runnable = library.game(&game).is_some();
    let job = app
        .state::<ModsHandle>()
        .lock()
        .install(&game, pending, selection, runnable);
    crate::mods::run(app, &library, job);
}

/// The last step's Install, if it was one. The next installer opens when the
/// mods are free again.
fn finish(app: &AppHandle, ready: Option<Ready>) {
    if let Some(ready) = ready {
        build(app, ready);
        next_waiting(app);
    }
}

#[tauri::command]
pub fn installer_get(app: AppHandle) -> InstallerSheetDto {
    dto(&app)
}

/// An option clicked: its group and place, and on or off.
#[tauri::command]
pub fn installer_pick(group: i32, option: i32, on: bool, app: AppHandle) -> InstallerSheetDto {
    let (Ok(group), Ok(option)) = (usize::try_from(group), usize::try_from(option)) else {
        return dto(&app);
    };
    app.state::<InstallerHandle>()
        .lock()
        .pick(group, option, on);
    dto(&app)
}

/// The cursor moved by this many options in the step, round the ends.
#[tauri::command]
pub fn installer_focus(delta: i32, app: AppHandle) -> InstallerSheetDto {
    let delta = isize::try_from(delta).unwrap_or_default();
    app.state::<InstallerHandle>().lock().focus(delta);
    dto(&app)
}

/// On to the next step, or Install on the last one.
#[tauri::command]
pub fn installer_next(app: AppHandle) -> InstallerSheetDto {
    let ready = app.state::<InstallerHandle>().lock().advance();
    finish(&app, ready);
    dto(&app)
}

/// Back to the step before.
#[tauri::command]
pub fn installer_back(app: AppHandle) -> InstallerSheetDto {
    app.state::<InstallerHandle>().lock().back();
    dto(&app)
}

/// Enter: turns the focused option on or off, or goes on when none is.
#[tauri::command]
pub fn installer_activate(app: AppHandle) -> InstallerSheetDto {
    let ready = app.state::<InstallerHandle>().lock().activate();
    finish(&app, ready);
    dto(&app)
}

/// Escape and Cancel: the sheet closes without installing, and the unpacked
/// mod goes.
#[tauri::command]
pub fn installer_cancel(app: AppHandle) -> InstallerSheetDto {
    let cancelled = app.state::<InstallerHandle>().lock().cancel();
    if let Some(cancelled) = cancelled {
        say(&app, cancelled.status);
        let pending = cancelled.pending;
        std::thread::spawn(move || mods::cancel_install(&pending));
        changed(&app);
        next_waiting(&app);
    }
    dto(&app)
}

/// The sheet's lines are built in the window's language: a change asks for
/// them again.
pub(crate) fn relabelled(app: &AppHandle) {
    if app.state::<InstallerHandle>().lock().is_open() {
        changed(app);
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::fomod::Installer;
    use zendo_core::mods::{self, Pending};

    use super::*;

    fn pending() -> Box<Pending> {
        let installer = Installer::parse(
            br#"<config><moduleName>Fancy</moduleName><installSteps order="Explicit">
              <installStep name="One"><optionalFileGroups>
                <group name="Maybe" type="SelectAtMostOne"><plugins>
                  <plugin name="A"><description>About A</description>
                    <files><file source="a"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                  <plugin name="B"><files><file source="b"/></files>
                    <typeDescriptor><type name="NotUsable"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
              <installStep name="Two"><optionalFileGroups>
                <group name="Any" type="SelectAny"><plugins>
                  <plugin name="C"><files><file source="c"/></files>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep>
            </installSteps></config>"#,
        )
        .unwrap();
        Box::new(mods::test_pending("Fancy", installer))
    }

    /// The shared state as an opened sheet leaves it.
    fn shown() -> InstallerState {
        let mut state = InstallerState::default();
        assert!(matches!(
            state.open("local:mods".into(), pending(), true),
            Opening::Shown(_)
        ));
        state
    }

    fn dto_of(state: InstallerState) -> InstallerSheetDto {
        let view = state.view().expect("a step to show");
        InstallerSheetDto {
            open: true,
            title: view.title,
            step_line: view.step_line,
            needs: view.needs_warning,
            groups: view
                .groups
                .iter()
                .enumerate()
                .map(|(group, shown)| GroupDto {
                    title: shown.title.clone(),
                    options: shown
                        .options
                        .iter()
                        .enumerate()
                        .map(|(option, each)| OptionDto {
                            group,
                            option,
                            label: each.label.clone(),
                            single: each.single,
                            on: each.on,
                            enabled: each.enabled,
                            dim: each.dim,
                            focused: each.focused,
                        })
                        .collect(),
                })
                .collect(),
            picture: None,
            about_name: view.about.name,
            about_text: view.about.text,
            back_enabled: view.back_enabled,
            final_step: view.final_step,
            primary_label: if view.final_step {
                t!("installer-install")
            } else {
                t!("installer-next")
            },
        }
    }

    #[test]
    fn a_closed_sheet_draws_nothing() {
        let dto = InstallerSheetDto::default();
        assert!(!dto.open);
        assert!(dto.groups.is_empty());
    }

    #[test]
    fn the_sheet_lists_the_groups_and_names_the_install_on_its_last_step() {
        let dto = dto_of(shown());
        assert_eq!(dto.title, "Fancy");
        assert_eq!(dto.step_line, "Step 1 of 2 · One");
        assert_eq!(dto.primary_label, "Next");
        assert!(!dto.back_enabled);
        let options = &dto.groups[0].options;
        assert_eq!(dto.groups[0].title, "Maybe · choose one or none");
        assert_eq!((options[0].group, options[0].option), (0, 0));
        assert!(options[0].single && options[0].enabled && !options[0].on);
        assert!(!options[1].enabled && options[1].dim);
        assert_eq!(dto.about_name, "A");
        assert_eq!(dto.about_text, "About A");
    }

    #[test]
    fn the_last_step_says_install_and_back_is_there() {
        let mut state = shown();
        state.advance();
        let dto = dto_of(state);
        assert_eq!(dto.step_line, "Step 2 of 2 · Two");
        assert_eq!(dto.primary_label, "Install");
        assert!(dto.back_enabled && dto.final_step);
        assert!(!dto.groups[0].options[0].single, "a box");
    }

    #[test]
    fn a_picture_is_copied_once_and_dropped_when_another_is_described() {
        let dir = std::env::temp_dir().join(format!("zendo-installer-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let first = dir.join("one.png");
        let second = dir.join("two.png");
        fs::write(&first, b"one").unwrap();
        fs::write(&second, b"two").unwrap();
        let handle = InstallerHandle::default();
        let Some(one) = picture_of(&handle, Some(&first)) else {
            panic!("the first picture is copied");
        };
        assert_eq!(picture_of(&handle, Some(&first)), Some(one.clone()));
        let Some(two) = picture_of(&handle, Some(&second)) else {
            panic!("the second picture is copied");
        };
        assert_ne!(one, two, "a new name, so the page does not keep the old");
        assert!(!Path::new(&one).exists(), "the first copy is gone");
        assert!(Path::new(&two).exists());
        assert_eq!(picture_of(&handle, None), None);
        assert!(!Path::new(&two).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
