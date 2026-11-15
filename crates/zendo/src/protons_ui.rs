//! The Proton group of Settings: the default Proton, the Protons found,
//! GE-Proton or Proton-CachyOS releases to download (the latest few, older
//! ones on asking), one at a time with its progress, and Remove for the ones
//! in the user's own tools folder. A part of the Library's glue
//! (`library_ui`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::Size;
use zendo_core::config::Config;
use zendo_core::ge_proton::{self, Build, Release};
use zendo_core::launch_options::LaunchOptions;
use zendo_core::proton::{self, Proton};
use zendo_shell::t;

use super::settings::{Setting, bar_of, chip, note_tone};
use super::{Glue, with_glue};
use crate::{ChipData, MainWindow, SetRow, SettingsData};

/// How many releases the card offers at first.
const SHOWN: usize = 5;
/// How many more each "Show older versions" adds.
const OLDER: usize = 10;
/// How often a download's bar is redrawn.
const BAR_EVERY: Duration = Duration::from_millis(100);

/// What is known of one build's releases.
#[derive(Debug)]
struct List {
    /// The releases read so far, newest first.
    releases: Vec<Release>,
    /// Pages of GitHub's list read; 0 until asked.
    pages: u32,
    /// GitHub has pages after those.
    more: bool,
    /// How many releases are on show.
    shown: usize,
    error: Option<String>,
    asking: bool,
}

impl Default for List {
    fn default() -> Self {
        Self {
            releases: Vec::new(),
            pages: 0,
            more: false,
            shown: SHOWN,
            error: None,
            asking: false,
        }
    }
}

fn slot(build: Build) -> usize {
    match build {
        Build::Ge => 0,
        Build::CachyOs => 1,
    }
}

struct Download {
    name: String,
    done: u64,
    total: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct ProtonsState {
    installed: RefCell<Vec<Proton>>,
    /// The build whose releases are on show.
    build: Cell<Build>,
    /// Each build's releases, by [`slot`].
    lists: RefCell<[List; 2]>,
    download: RefCell<Option<Download>>,
    /// The Proton whose Remove was pressed once, and the games using it,
    /// counted then rather than as the page is drawn.
    confirm_remove: RefCell<Option<(String, usize)>>,
    /// The last thing that happened, and whether it went well.
    note: RefCell<Option<(String, bool)>>,
    /// A removal put the default back to Automatic and said so: the note of
    /// that save is not wanted.
    default_reset: Cell<bool>,
    /// What each default chip names: `None` is Automatic.
    default_names: RefCell<Vec<Option<String>>>,
    defaults: Rc<VecModel<ChipData>>,
    installed_rows: Rc<VecModel<SetRow>>,
    builds: Rc<VecModel<ChipData>>,
    release_rows: Rc<VecModel<SetRow>>,
}

impl Glue {
    pub(super) fn connect_protons(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<SettingsData>();
        let state = &self.protons;
        data.set_proton_defaults(ModelRc::from(Rc::clone(&state.defaults)));
        data.set_proton_installed(ModelRc::from(Rc::clone(&state.installed_rows)));
        data.set_proton_builds(ModelRc::from(Rc::clone(&state.builds)));
        data.set_proton_releases(ModelRc::from(Rc::clone(&state.release_rows)));
        let glue = Rc::clone(self);
        data.on_default_chosen(move |index| {
            let name = usize::try_from(index)
                .ok()
                .and_then(|index| glue.protons.default_names.borrow().get(index).cloned());
            if let Some(name) = name {
                glue.save_setting(Setting::DefaultProton(name));
            }
        });
        let glue = Rc::clone(self);
        data.on_proton_remove(move |index| {
            let name = usize::try_from(index).ok().and_then(|index| {
                glue.protons
                    .installed
                    .borrow()
                    .get(index)
                    .map(|found| found.name.clone())
            });
            if let Some(name) = name {
                glue.remove_proton(&name);
            }
        });
        let glue = Rc::clone(self);
        data.on_build_chosen(move |index| {
            if let Some(build) = usize::try_from(index)
                .ok()
                .and_then(|index| Build::ALL.get(index).copied())
            {
                glue.set_build(build);
            }
        });
        let glue = Rc::clone(self);
        data.on_proton_refresh(move || glue.refresh_protons());
        let glue = Rc::clone(self);
        data.on_proton_show_older(move || glue.show_older());
        let glue = Rc::clone(self);
        data.on_release_download(move |index| {
            let release = usize::try_from(index).ok().and_then(|index| {
                let lists = glue.protons.lists.borrow();
                lists[slot(glue.protons.build.get())]
                    .releases
                    .get(index)
                    .cloned()
            });
            if let Some(release) = release {
                glue.download_proton(release);
            }
        });
        let glue = Rc::clone(self);
        data.on_proton_stop(move || {
            if let Some(download) = glue.protons.download.borrow().as_ref() {
                download.stop.store(true, Ordering::Relaxed);
            }
            glue.show_settings();
        });
    }

    /// Looks at the Protons on the machine. Nothing is read in tests.
    fn read_protons(&self) {
        if self.persist {
            *self.protons.installed.borrow_mut() = proton::installed();
        }
    }

    /// Looks at the Protons as Settings opens, and asks GitHub for releases
    /// once a session.
    pub(super) fn want_protons(&self) {
        self.read_protons();
        let build = self.protons.build.get();
        let fresh = {
            let lists = self.protons.lists.borrow();
            let list = &lists[slot(build)];
            list.pages == 0 && !list.asking
        };
        if self.persist && fresh {
            self.ask_page(build, 1);
        }
    }

    fn ask_page(&self, build: Build, page: u32) {
        {
            let mut lists = self.protons.lists.borrow_mut();
            let list = &mut lists[slot(build)];
            list.asking = true;
            list.error = None;
        }
        if self.persist {
            self.ask(
                move || ge_proton::releases(build, page).map_err(|err| format!("{err:#}")),
                move |glue, result| glue.proton_releases(build, page, result),
            );
        }
        self.show_settings();
    }

    /// A page of a build's GitHub list, and whether more follow.
    pub(super) fn proton_releases(
        &self,
        build: Build,
        page: u32,
        result: Result<(Vec<Release>, bool), String>,
    ) {
        {
            let mut lists = self.protons.lists.borrow_mut();
            let list = &mut lists[slot(build)];
            list.asking = false;
            match result {
                Ok((releases, more)) => {
                    if page == 1 {
                        list.releases.clear();
                    }
                    list.releases.extend(releases);
                    list.pages = page;
                    list.more = more;
                }
                Err(err) => list.error = Some(err),
            }
        }
        self.show_settings();
    }

    fn refresh_protons(&self) {
        self.read_protons();
        let build = self.protons.build.get();
        let ask = {
            let mut lists = self.protons.lists.borrow_mut();
            let list = &mut lists[slot(build)];
            list.shown = SHOWN;
            !list.asking
        };
        if ask {
            self.ask_page(build, 1);
        } else {
            self.show_settings();
        }
    }

    fn show_older(&self) {
        let build = self.protons.build.get();
        let next = {
            let mut lists = self.protons.lists.borrow_mut();
            let list = &mut lists[slot(build)];
            list.shown += OLDER;
            (list.shown > list.releases.len() && list.more && !list.asking).then(|| list.pages + 1)
        };
        match next {
            Some(page) => self.ask_page(build, page),
            None => self.show_settings(),
        }
    }

    pub(super) fn set_build(&self, build: Build) {
        self.protons.build.set(build);
        let fresh = {
            let lists = self.protons.lists.borrow();
            let list = &lists[slot(build)];
            list.pages == 0 && !list.asking
        };
        if fresh {
            self.ask_page(build, 1);
        } else {
            self.show_settings();
        }
    }

    /// Says which Proton games without one of their own use now, once the
    /// choice is in `config.toml`.
    pub(super) fn default_proton_set(&self) {
        // A removal that reset the default has said so already.
        if self.protons.default_reset.take() {
            return;
        }
        let config = self.settings_config();
        let installed = self.protons.installed.borrow();
        let shown = config
            .default_proton
            .as_deref()
            .and_then(|name| installed.iter().find(|found| found.name == name))
            .or_else(|| installed.first())
            .map_or_else(|| t!("protons-umu"), |found| found.name.clone());
        *self.protons.note.borrow_mut() = Some((t!("protons-default-set", name = shown), true));
    }

    pub(super) fn download_proton(&self, release: Release) {
        let state = &self.protons;
        if state.download.borrow().is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *state.download.borrow_mut() = Some(Download {
            name: release.name.clone(),
            done: 0,
            total: release.size,
            stop: Arc::clone(&stop),
        });
        *state.note.borrow_mut() = None;
        self.show_settings();
        if !self.persist {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let name = release.name.clone();
            let progress = weak.clone();
            let progress_name = name.clone();
            let mut last: Option<Instant> = None;
            let result = ge_proton::install(&release, &stop, &mut |done, total| {
                if last.is_some_and(|at| at.elapsed() < BAR_EVERY) {
                    return;
                }
                last = Some(Instant::now());
                let name = progress_name.clone();
                let _ = progress.upgrade_in_event_loop(move |_| {
                    with_glue(|glue| glue.proton_progress(&name, done, total));
                });
            });
            let result = result.map(|_| ()).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.proton_installed(&name, result));
            });
        });
    }

    /// How far a download got: only the bar is written, not the page.
    pub(super) fn proton_progress(&self, name: &str, done: u64, total: u64) {
        let bar = {
            let mut download = self.protons.download.borrow_mut();
            let Some(download) = download.as_mut().filter(|d| d.name == name) else {
                return;
            };
            download.done = done;
            download.total = total;
            bar_of(name, done, total, download.stop.load(Ordering::Relaxed))
        };
        if let Some(window) = self.window.upgrade() {
            window.global::<SettingsData>().set_proton_bar(bar);
        }
    }

    pub(super) fn proton_installed(&self, name: &str, result: Result<(), String>) {
        let stopped = self
            .protons
            .download
            .borrow_mut()
            .take()
            .is_some_and(|download| download.stop.load(Ordering::Relaxed));
        self.read_protons();
        *self.protons.note.borrow_mut() = Some(match result {
            Ok(()) => (t!("protons-installed-note", name = name), true),
            Err(_) if stopped => (t!("protons-stopped", name = name), true),
            Err(err) => (err, false),
        });
        self.show_settings();
    }

    /// Remove, pressed once to ask and again to delete.
    pub(super) fn remove_proton(self: &Rc<Self>, name: &str) {
        let state = &self.protons;
        let confirming = state
            .confirm_remove
            .borrow()
            .as_ref()
            .is_some_and(|(chosen, _)| chosen == name);
        if !confirming {
            // Counted once, here: the page must not read launch options.
            let users = if self.persist { games_using(name) } else { 0 };
            *state.confirm_remove.borrow_mut() = Some((name.to_owned(), users));
            self.show_settings();
            return;
        }
        *state.confirm_remove.borrow_mut() = None;
        let Some(found) = state
            .installed
            .borrow()
            .iter()
            .find(|found| found.name == name)
            .cloned()
        else {
            self.show_settings();
            return;
        };
        if !self.persist {
            self.proton_removed(name, Ok(()));
            return;
        }
        // A Proton tree is hundreds of megabytes, so it goes on a thread.
        let removed = name.to_owned();
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = ge_proton::remove(&found).map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.proton_removed(&removed, result));
            });
        });
        self.show_settings();
    }

    /// What came of a Remove: the list reads again, and a default that is
    /// gone goes back to Automatic.
    pub(super) fn proton_removed(self: &Rc<Self>, name: &str, result: Result<(), String>) {
        self.read_protons();
        if let Err(err) = result {
            *self.protons.note.borrow_mut() = Some((err, false));
            self.show_settings();
            return;
        }
        // Games that named it go back to Automatic, and the default too, or
        // they would launch with a Proton that is gone. Launch options are
        // the user's files: not touched in tests.
        let reset = if self.persist {
            reset_games_using(name)
        } else {
            0
        };
        let was_default = self.settings_config().default_proton.as_deref() == Some(name);
        let note = if was_default {
            self.protons.default_reset.set(true);
            self.save_setting(Setting::DefaultProton(None));
            t!("protons-removed-default", name = name)
        } else if reset > 0 {
            t!("protons-reset", name = name, count = reset)
        } else {
            t!("protons-removed", name = name)
        };
        // After the default's own note, which says the same thing in other
        // words.
        *self.protons.note.borrow_mut() = Some((note, true));
        self.show_settings();
    }

    /// Automatic, each Proton found, and the one chosen even when gone.
    fn default_choices(&self, config: &Config) -> Vec<(Option<String>, String)> {
        let installed = self.protons.installed.borrow();
        let automatic = match installed.first() {
            Some(first) => t!("protons-automatic", name = first.name.as_str()),
            None => t!("protons-automatic-umu"),
        };
        let mut choices = vec![(None, automatic)];
        choices.extend(
            installed
                .iter()
                .map(|found| (Some(found.name.clone()), found.name.clone())),
        );
        if let Some(chosen) = &config.default_proton
            && !choices
                .iter()
                .any(|(name, _)| name.as_ref() == Some(chosen))
        {
            choices.push((
                Some(chosen.clone()),
                t!("protons-missing", name = chosen.as_str()),
            ));
        }
        choices
    }

    fn installed_rows(&self) -> Vec<SetRow> {
        let confirm = self.protons.confirm_remove.borrow();
        self.protons
            .installed
            .borrow()
            .iter()
            .map(|found| {
                let (confirming, users) = match confirm.as_ref() {
                    Some((chosen, users)) if chosen == &found.name => (true, *users),
                    _ => (false, 0),
                };
                let removable = ge_proton::removable(found);
                SetRow {
                    title: found.name.clone().into(),
                    path: found.path.display().to_string().into(),
                    note: if users > 0 {
                        t!("protons-chosen-by", count = users).into()
                    } else {
                        "".into()
                    },
                    note_tone: 2,
                    remove: if !removable {
                        "".into()
                    } else if confirming {
                        t!("really-remove").into()
                    } else {
                        t!("remove").into()
                    },
                    remove_on: true,
                    remove_danger: confirming,
                    ..SetRow::default()
                }
            })
            .collect()
    }

    fn release_rows(&self, list: &List) -> Vec<SetRow> {
        let installed = self.protons.installed.borrow();
        let idle = self.protons.download.borrow().is_none();
        list.releases
            .iter()
            .take(list.shown)
            .map(|release| {
                let here = installed
                    .iter()
                    .any(|found| ge_proton::is_release(found, release));
                SetRow {
                    title: release.name.clone().into(),
                    sub: format!("{} · {}", release.published, Size::Bytes(release.size)).into(),
                    action: if here {
                        "".into()
                    } else {
                        t!("download").into()
                    },
                    action_on: idle,
                    status: if here {
                        t!("installed").into()
                    } else {
                        "".into()
                    },
                    status_tone: 1,
                    ..SetRow::default()
                }
            })
            .collect()
    }

    /// Writes the group.
    pub(super) fn show_protons(&self, data: &SettingsData<'_>, config: &Config) {
        let state = &self.protons;
        let choices = self.default_choices(config);
        state.defaults.set_vec(
            choices
                .iter()
                .enumerate()
                .map(|(index, (name, label))| {
                    chip(
                        i32::try_from(index).unwrap_or(0),
                        label.clone(),
                        *name == config.default_proton,
                    )
                })
                .collect::<Vec<_>>(),
        );
        *state.default_names.borrow_mut() = choices.into_iter().map(|(name, _)| name).collect();
        state.installed_rows.set_vec(self.installed_rows());
        data.set_proton_installed_none(if state.installed.borrow().is_empty() {
            t!("protons-none").into()
        } else {
            "".into()
        });

        let build = state.build.get();
        state.builds.set_vec(
            Build::ALL
                .into_iter()
                .enumerate()
                .map(|(index, each)| {
                    chip(
                        i32::try_from(index).unwrap_or(0),
                        each.label().to_owned(),
                        each == build,
                    )
                })
                .collect::<Vec<_>>(),
        );
        data.set_proton_releases_title(t!("protons-releases", build = build.label()).into());
        let lists = state.lists.borrow();
        let list = &lists[slot(build)];
        state.release_rows.set_vec(self.release_rows(list));
        data.set_proton_refresh_on(!list.asking);
        let (line, tone) = if list.asking {
            (t!("protons-asking"), 3)
        } else if let Some(err) = &list.error {
            (err.clone(), 2)
        } else {
            (String::new(), 0)
        };
        data.set_proton_line(line.into());
        data.set_proton_line_tone(tone);
        data.set_proton_older(
            !list.asking && list.error.is_none() && (list.releases.len() > list.shown || list.more),
        );
        data.set_proton_bar(
            state
                .download
                .borrow()
                .as_ref()
                .map(|download| {
                    bar_of(
                        &download.name,
                        download.done,
                        download.total,
                        download.stop.load(Ordering::Relaxed),
                    )
                })
                .unwrap_or_default(),
        );
        let (note, good) = state.note.borrow().clone().unwrap_or_default();
        data.set_proton_note(note.into());
        data.set_proton_note_tone(note_tone(good));
    }
}

/// Puts every game that chose this Proton back to Automatic, and says how
/// many.
fn reset_games_using(name: &str) -> usize {
    let Ok(all) = LaunchOptions::load_all() else {
        return 0;
    };
    let mut reset = 0;
    for (id, mut options) in all {
        if options.proton.as_deref() == Some(name) {
            options.proton = None;
            if options.save(&id).is_ok() {
                reset += 1;
            }
        }
    }
    reset
}

/// How many games' launch options chose this Proton.
fn games_using(name: &str) -> usize {
    LaunchOptions::load_all().map_or(0, |all| {
        all.values()
            .filter(|options| options.proton.as_deref() == Some(name))
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Themes;

    fn page() -> (MainWindow, Rc<Glue>) {
        i_slint_backend_testing::init_no_event_loop();
        let window = MainWindow::new().expect("the window opens");
        let glue = Glue::new(&window, false, Themes::plain());
        glue.connect(&window);
        (window, glue)
    }

    fn release(name: &str) -> Release {
        Release {
            build: Build::Ge,
            name: name.into(),
            published: "2026-11-03".into(),
            file: format!("{name}.tar.gz"),
            tarball: String::new(),
            size: 1000,
            checksum: None,
        }
    }

    fn list_of(glue: &Glue) -> std::cell::Ref<'_, List> {
        std::cell::Ref::map(glue.protons.lists.borrow(), |lists| {
            &lists[slot(glue.protons.build.get())]
        })
    }

    #[test]
    fn downloads_one_at_a_time_and_says_how_it_ended() {
        let (_window, glue) = page();
        glue.download_proton(release("GE-Proton11-12"));
        glue.download_proton(release("GE-Proton11-11"));
        assert_eq!(
            glue.protons
                .download
                .borrow()
                .as_ref()
                .map(|d| d.name.clone()),
            Some("GE-Proton11-12".to_owned())
        );
        glue.proton_progress("GE-Proton11-12", 500, 1000);
        assert_eq!(
            glue.protons.download.borrow().as_ref().map(|d| d.done),
            Some(500)
        );
        if let Some(download) = glue.protons.download.borrow().as_ref() {
            download.stop.store(true, Ordering::Relaxed);
        }
        glue.proton_installed("GE-Proton11-12", Err("stopped".into()));
        assert!(glue.protons.download.borrow().is_none());
        assert_eq!(
            *glue.protons.note.borrow(),
            Some(("Stopped downloading GE-Proton11-12".into(), true))
        );
        glue.download_proton(release("GE-Proton11-11"));
        glue.proton_installed(
            "GE-Proton11-11",
            Err("the download of GE-Proton11-11 does not match its checksum".into()),
        );
        assert_eq!(
            *glue.protons.note.borrow(),
            Some((
                "the download of GE-Proton11-11 does not match its checksum".into(),
                false
            ))
        );
    }

    #[test]
    fn remove_asks_once_first() {
        let (_window, glue) = page();
        *glue.protons.installed.borrow_mut() = vec![Proton {
            name: "GE-Proton11-12".into(),
            path: "/nowhere/GE-Proton11-12".into(),
        }];
        glue.remove_proton("GE-Proton11-12");
        assert_eq!(
            glue.protons
                .confirm_remove
                .borrow()
                .as_ref()
                .map(|(name, _)| name.clone()),
            Some("GE-Proton11-12".to_owned())
        );
        assert!(glue.protons.note.borrow().is_none());
    }

    #[test]
    fn removing_the_default_puts_automatic_back_and_says_so() {
        let (_window, glue) = page();
        *glue.protons.installed.borrow_mut() = vec![Proton {
            name: "GE-Proton11-12".into(),
            path: "/nowhere/GE-Proton11-12".into(),
        }];
        let mut config = Config::default();
        config.default_proton = Some("GE-Proton11-12".into());
        *glue.config.borrow_mut() = Some(config);
        glue.remove_proton("GE-Proton11-12");
        glue.remove_proton("GE-Proton11-12");
        assert_eq!(glue.settings_config().default_proton, None);
        assert_eq!(
            *glue.protons.note.borrow(),
            Some((
                "Removed GE-Proton11-12; it was the default, so Automatic is used now".into(),
                true
            ))
        );
    }

    #[test]
    fn shows_older_versions_a_few_at_a_time() {
        let (_window, glue) = page();
        let page_of = |from: usize, to: usize| {
            (from..to)
                .map(|n| release(&format!("GE-Proton11-{n}")))
                .collect::<Vec<_>>()
        };
        glue.proton_releases(Build::Ge, 1, Ok((page_of(0, 12), true)));
        assert_eq!(list_of(&glue).shown, SHOWN);
        glue.show_older();
        assert_eq!(list_of(&glue).shown, SHOWN + OLDER);
        assert!(
            list_of(&glue).asking,
            "past what was read, the next page is asked"
        );
        glue.proton_releases(Build::Ge, 2, Ok((page_of(12, 20), false)));
        assert_eq!(list_of(&glue).releases.len(), 20);
        assert_eq!(list_of(&glue).pages, 2);
        assert!(!list_of(&glue).more);
    }

    #[test]
    fn each_build_keeps_its_own_releases() {
        let (_window, glue) = page();
        glue.proton_releases(Build::Ge, 1, Ok((vec![release("GE-Proton11-12")], false)));
        glue.set_build(Build::CachyOs);
        assert_eq!(glue.protons.build.get(), Build::CachyOs);
        assert_eq!(list_of(&glue).releases.len(), 0);
        assert!(list_of(&glue).asking, "a build not read yet is asked");
        glue.proton_releases(
            Build::CachyOs,
            1,
            Ok((
                vec![release("proton-cachyos-11.0-20261015-slr-x86_64")],
                false,
            )),
        );
        glue.set_build(Build::Ge);
        assert_eq!(list_of(&glue).releases.len(), 1);
        assert_eq!(list_of(&glue).releases[0].name, "GE-Proton11-12");
        assert!(!list_of(&glue).asking, "already read, not asked again");
    }

    #[test]
    fn the_default_choices_start_with_automatic() {
        let (_window, glue) = page();
        *glue.protons.installed.borrow_mut() = vec![Proton {
            name: "GE-Proton11-12".into(),
            path: "/nowhere/GE-Proton11-12".into(),
        }];
        let mut config = Config::default();
        config.default_proton = Some("GE-Proton9-1".into());
        let choices = glue.default_choices(&config);
        assert_eq!(choices[0].0, None, "Automatic comes first");
        assert_eq!(choices[1].0.as_deref(), Some("GE-Proton11-12"));
        assert_eq!(
            choices[2].0.as_deref(),
            Some("GE-Proton9-1"),
            "the default that is gone stays listed"
        );
    }
}
