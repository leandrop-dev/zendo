//! The Wine prefix card on the page of a Windows game Zendo starts: Wine's
//! settings, winetricks' window, components installed through winetricks, a
//! Windows program run in the prefix (an installer, a patch), GOG's setup,
//! the game's known fixes and the prefix's folder. One tool at a time per
//! game, and none while the game runs; each runs on a thread through
//! `zendo_core::prefix_tools`. A part of the Library's glue (`library_ui`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::Game;
use zendo_core::gog_setup;
use zendo_core::known_fixes::{self, Fixes};
use zendo_core::launch;
use zendo_core::launch_options::LaunchOptions;
use zendo_core::prefix_tools::{self, Ran, Tool};
use zendo_shell::file_dialog::{self, Kind};
use zendo_shell::t;

use super::{Glue, with_glue};
use crate::{Fact, GameData, MainWindow};

/// What is known of the prefix tools, by game.
pub(super) struct PrefixState {
    /// What runs in each game's prefix now.
    busy: RefCell<HashMap<String, Tool>>,
    /// The components typed to install.
    components: RefCell<HashMap<String, String>>,
    /// How each game's last tool went: the words, and whether it went well.
    notes: RefCell<HashMap<String, (String, bool)>>,
    /// Each game's known fixes, read as its page opens; `None` when the list
    /// has none for it.
    fixes: RefCell<HashMap<String, Option<Fixes>>>,
    fix_lines: Rc<VecModel<Fact>>,
}

impl PrefixState {
    pub(super) fn new() -> Self {
        Self {
            busy: RefCell::default(),
            components: RefCell::default(),
            notes: RefCell::default(),
            fixes: RefCell::default(),
            fix_lines: Rc::new(VecModel::default()),
        }
    }
}

/// What the card says runs.
fn label(tool: &Tool) -> String {
    match tool {
        Tool::WineSettings => t!("prefix-wine-settings"),
        Tool::Winetricks => "winetricks".to_owned(),
        Tool::Components(verbs) => verbs.join(" "),
        Tool::GogSetup => t!("prefix-gog-setup"),
        Tool::KnownFixes => t!("prefix-known-fixes"),
        Tool::Program(program) => program
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
    }
}

/// What a components run did: what it installed, and what was there.
fn installed(ran: &Ran) -> String {
    match (ran.installed.is_empty(), ran.already.is_empty()) {
        (_, true) => t!("prefix-installed", components = ran.installed.join(" ")),
        (true, false) => t!("prefix-all-there", components = ran.already.join(" ")),
        (false, false) => t!(
            "prefix-installed-some",
            components = ran.installed.join(" "),
            already = ran.already.join(" ")
        ),
    }
}

impl Glue {
    /// Hooks the card's callbacks up and gives the page its model.
    pub(super) fn connect_prefix(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameData>();
        data.set_prefix_fixes(ModelRc::from(Rc::clone(&self.prefix.fix_lines)));
        let glue = Rc::clone(self);
        data.on_prefix_run(move |which| {
            let tool = match which {
                0 => Tool::WineSettings,
                1 => Tool::Winetricks,
                2 => Tool::GogSetup,
                _ => Tool::KnownFixes,
            };
            if let Some(id) = glue.open.borrow().clone() {
                glue.run_tool(id, tool);
            }
        });
        let glue = Rc::clone(self);
        data.on_prefix_pick(move || {
            if let Some(id) = glue.open.borrow().clone() {
                glue.pick_program(id);
            }
        });
        let glue = Rc::clone(self);
        data.on_prefix_typed(move |typed| {
            if let Some(id) = glue.open.borrow().clone() {
                glue.prefix
                    .components
                    .borrow_mut()
                    .insert(id, typed.to_string());
            }
        });
        let glue = Rc::clone(self);
        data.on_prefix_install(move || {
            if let Some(id) = glue.open.borrow().clone() {
                glue.install_components(id);
            }
        });
    }

    /// Whether a tool may run in the game's prefix: none runs there, and the
    /// game does not.
    fn tools_free(&self, id: &str) -> bool {
        !self.prefix.busy.borrow().contains_key(id) && !self.game.running.borrow().is_running(id)
    }

    /// The page just opened: reads the components last typed, and asks for
    /// the game's known fixes (Heroic's list, asked again once a day).
    pub(super) fn want_prefix(&self, id: &str) {
        if !self.prefix.components.borrow().contains_key(id) {
            let typed = if self.persist {
                LaunchOptions::load(id)
                    .ok()
                    .and_then(|options| options.winetricks_components)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            self.prefix
                .components
                .borrow_mut()
                .insert(id.to_owned(), typed);
        }
        let Some(game) = self
            .browser
            .borrow()
            .game(id)
            .filter(|game| known_fixes::applies(game))
            .cloned()
        else {
            return;
        };
        if !self.persist {
            return;
        }
        let owned = id.to_owned();
        self.ask(
            move || {
                let _ = known_fixes::refresh();
                Ok(known_fixes::for_game(&game))
            },
            move |glue, result| {
                if let Ok(fixes) = result {
                    glue.prefix.fixes.borrow_mut().insert(owned, fixes);
                    glue.show_game();
                }
            },
        );
    }

    fn save_components(&self, id: &str, typed: &str) {
        if !self.persist {
            return;
        }
        if let Ok(mut options) = LaunchOptions::load(id) {
            options.winetricks_components = Some(typed.trim())
                .filter(|typed| !typed.is_empty())
                .map(str::to_owned);
            let _ = options.save(id);
        }
    }

    /// Runs the tool on a thread, when none runs and the game does not.
    fn run_tool(&self, id: String, tool: Tool) {
        if !self.tools_free(&id) {
            return;
        }
        let Some(game) = self.browser.borrow().game(&id).cloned() else {
            return;
        };
        self.prefix
            .busy
            .borrow_mut()
            .insert(id.clone(), tool.clone());
        self.prefix.notes.borrow_mut().remove(&id);
        if self.persist {
            self.ask(
                move || prefix_tools::run(&game, &tool).map_err(|err| format!("{err:#}")),
                move |glue, result| glue.prefix_done(id, result),
            );
        }
        self.show_game();
    }

    /// Asks which program to run, then runs it in the prefix.
    fn pick_program(&self, id: String) {
        if !self.persist || !self.tools_free(&id) {
            return;
        }
        let weak = self.window.clone();
        std::thread::spawn(move || {
            let result = file_dialog::pick(Kind::Installer, &t!("prefix-choose-program"))
                .map_err(|err| format!("{err:#}"));
            let _ = weak.upgrade_in_event_loop(move |_| {
                with_glue(|glue| glue.program_picked(id, result));
            });
        });
    }

    pub(super) fn program_picked(&self, id: String, result: Result<Option<PathBuf>, String>) {
        match result {
            Ok(Some(program)) => self.run_tool(id, Tool::Program(program)),
            Ok(None) => {}
            Err(err) => {
                self.prefix.notes.borrow_mut().insert(id, (err, false));
                self.show_game();
            }
        }
    }

    /// Installs the components typed, after checking them.
    pub(super) fn install_components(&self, id: String) {
        let typed = self
            .prefix
            .components
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_default();
        match prefix_tools::components(&typed) {
            Ok(verbs) => {
                self.save_components(&id, &typed);
                self.run_tool(id, Tool::Components(verbs));
            }
            Err(err) => {
                self.prefix
                    .notes
                    .borrow_mut()
                    .insert(id, (format!("{err:#}"), false));
                self.show_game();
            }
        }
    }

    /// A tool ended: says how it went.
    pub(super) fn prefix_done(&self, id: String, result: Result<Ran, String>) {
        let Some(tool) = self.prefix.busy.borrow_mut().remove(&id) else {
            return;
        };
        let note = match (&tool, result) {
            (Tool::Components(_), Ok(ran)) => {
                if let Some(text) = self.prefix.components.borrow_mut().get_mut(&id) {
                    text.clear();
                }
                (installed(&ran), true)
            }
            (tool, Ok(_)) => (t!("prefix-done", what = label(tool)), true),
            (_, Err(err)) => (err, false),
        };
        self.prefix.notes.borrow_mut().insert(id, note);
        self.show_game();
    }

    /// Writes the card of the open game: none for a game Zendo does not
    /// start in a prefix.
    pub(super) fn show_prefix(&self, game: &Game, data: &GameData<'_>) {
        let show = prefix_tools::applies(game);
        data.set_prefix_show(show);
        if !show {
            self.prefix.fix_lines.set_vec(Vec::new());
            return;
        }
        let id = game.id.as_str();
        let running = self.game.running.borrow().is_running(id);
        data.set_prefix_free(self.tools_free(id));
        data.set_prefix_gog(gog_setup::applies(game));
        // Reads the user's config for the games folder: not in tests.
        let folder = if self.persist {
            launch::prefix(game)
                .filter(|prefix| prefix.is_dir())
                .map(|prefix| prefix.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        data.set_prefix_folder(folder.into());
        let typed = self
            .prefix
            .components
            .borrow()
            .get(id)
            .cloned()
            .unwrap_or_default();
        if data.get_prefix_components() != typed.as_str() {
            data.set_prefix_components(typed.into());
        }

        let mut lines = Vec::new();
        if let Some(Some(fixes)) = self.prefix.fixes.borrow().get(id) {
            for fix in fixes.winetricks.iter().chain(fixes.run_in_prefix.iter()) {
                let line = match fixes.notes.get(fix) {
                    Some(note) => format!("{fix}: {note}"),
                    None => fix.clone(),
                };
                lines.push(Fact {
                    label: line.into(),
                    value: "".into(),
                    path: false,
                    mono: false,
                    url: "".into(),
                    tone: 0,
                });
            }
        }
        self.prefix.fix_lines.set_vec(lines);

        let (status, tone) = if let Some(tool) = self.prefix.busy.borrow().get(id) {
            (t!("prefix-running", what = label(tool)), 3)
        } else if running {
            (t!("prefix-game-running"), 3)
        } else if let Some((note, good)) = self.prefix.notes.borrow().get(id) {
            (note.clone(), if *good { 1 } else { 2 })
        } else {
            (String::new(), 0)
        };
        data.set_prefix_status(status.into());
        data.set_prefix_status_tone(tone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_a_components_run_did() {
        let ran = Ran {
            installed: vec!["vcrun2022".into()],
            already: vec!["d3dx9".into()],
        };
        assert_eq!(
            installed(&ran),
            "Installed vcrun2022; d3dx9 was there already"
        );
        assert_eq!(
            label(&Tool::Components(vec!["a".into(), "b".into()])),
            "a b"
        );
        assert_eq!(label(&Tool::WineSettings), "Wine settings");
    }
}
