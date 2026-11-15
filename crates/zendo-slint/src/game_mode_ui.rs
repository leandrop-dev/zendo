//! Game Mode in the window: the full-screen screen of one game large over a
//! row of covers (`ui/game_mode.slint`), and what F11, Select, the keys and
//! the pad do with it. The rules (shelves, the hero, the menu) are
//! `zendo_shell::game_mode`. A part of the Library's glue (`library_ui`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};
use zendo_core::browse::Move;
use zendo_shell::game_mode::{self, GameMode, MenuChoice, Shelf};
use zendo_shell::gamepad::Pad;
use zendo_shell::running;

use super::Glue;
use crate::covers::Cover;
use crate::{ChipData, GameModeData, GmTile, MainWindow, Page};

/// What Game Mode keeps: its state while it is on, the page it came from and
/// the rows drawn.
pub(super) struct GameModeState {
    mode: RefCell<Option<GameMode>>,
    came_from: Cell<Page>,
    shelves: Rc<VecModel<ChipData>>,
    tiles: Rc<VecModel<GmTile>>,
    menu: Rc<VecModel<ChipData>>,
}

impl GameModeState {
    pub(super) fn new() -> Self {
        Self {
            mode: RefCell::new(None),
            came_from: Cell::new(Page::Library),
            shelves: Rc::new(VecModel::default()),
            tiles: Rc::new(VecModel::default()),
            menu: Rc::new(VecModel::default()),
        }
    }
}

fn chip(id: usize, label: String, count: String, chosen: bool) -> ChipData {
    ChipData {
        id: i32::try_from(id).unwrap_or(0),
        label: label.into(),
        count: count.into(),
        chosen,
    }
}

impl Glue {
    /// Hooks the screen's callbacks up.
    pub(super) fn connect_game_mode(self: &Rc<Self>, window: &MainWindow) {
        let data = window.global::<GameModeData>();
        data.set_shelves(ModelRc::from(Rc::clone(&self.game_mode.shelves)));
        data.set_tiles(ModelRc::from(Rc::clone(&self.game_mode.tiles)));
        data.set_menu(ModelRc::from(Rc::clone(&self.game_mode.menu)));
        let glue = Rc::clone(self);
        data.on_resized(move || glue.show_game_mode());
        let glue = Rc::clone(self);
        data.on_shelf(move |at| {
            if let Some(shelf) = usize::try_from(at).ok().and_then(|at| Shelf::ALL.get(at)) {
                glue.with_mode(|mode, _| mode.set_shelf(*shelf));
            }
        });
        let glue = Rc::clone(self);
        data.on_pick(move |at| {
            let at = usize::try_from(at).unwrap_or(0);
            let first = glue.first_visible();
            glue.with_mode(|mode, _| mode.index = first + at);
        });
        let glue = Rc::clone(self);
        data.on_open_menu(move || glue.with_mode(|mode, _| mode.open_menu()));
        let glue = Rc::clone(self);
        data.on_close_menu(move || glue.with_mode(|mode, _| mode.menu = None));
        let glue = Rc::clone(self);
        data.on_choose(move |at| {
            if let Some(choice) = usize::try_from(at)
                .ok()
                .and_then(|at| MenuChoice::ALL.get(at))
            {
                glue.run_choice(*choice);
            }
        });
        let glue = Rc::clone(self);
        data.on_play(move || glue.play_hero());
        let glue = Rc::clone(self);
        data.on_details(move || glue.hero_details());
        let glue = Rc::clone(self);
        data.on_show(move || {
            glue.with_mode(|mode, _| mode.browsing = false);
            glue.navigate(Page::Library);
        });
        let glue = Rc::clone(self);
        data.on_relabel(move || glue.show_game_mode());
    }

    /// Whether Game Mode is on.
    pub(super) fn game_mode_on(&self) -> bool {
        self.game_mode.mode.borrow().is_some()
    }

    /// Whether the Game Mode screen is what the window shows: Game Mode is
    /// on, the page is the Library's, no game's page is open and its own grid
    /// was not asked for.
    pub(super) fn game_mode_screen(&self) -> bool {
        self.game_mode
            .mode
            .borrow()
            .as_ref()
            .is_some_and(|mode| !mode.browsing)
            && self.open.borrow().is_none()
            && self
                .window
                .upgrade()
                .is_some_and(|window| window.get_page() == Page::Library)
    }

    /// Changes the state and draws the screen again.
    fn with_mode(&self, change: impl FnOnce(&mut GameMode, &[zendo_core::Game])) {
        {
            let mut mode = self.game_mode.mode.borrow_mut();
            let Some(mode) = mode.as_mut() else {
                return;
            };
            change(mode, self.browser.borrow().games());
        }
        self.show_game_mode();
    }

    /// F11 and Select: Game Mode on or off. On: full screen, on its screen;
    /// off: the window again, on the page it was left from.
    pub(super) fn toggle_game_mode(&self) {
        if self.game_mode_on() {
            self.leave_game_mode();
        } else {
            self.enter_game_mode();
        }
    }

    fn enter_game_mode(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        self.game_mode.came_from.set(window.get_page());
        self.navigate(Page::Library);
        *self.game_mode.mode.borrow_mut() = Some(GameMode::default());
        window.window().set_fullscreen(true);
        self.show_game_mode();
    }

    fn leave_game_mode(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        *self.game_mode.mode.borrow_mut() = None;
        window.window().set_fullscreen(false);
        self.navigate(self.game_mode.came_from.get());
        self.show_game_mode();
    }

    /// Begins in Game Mode when Zendo was started with `--game-mode`: the
    /// window is made full screen already.
    pub(super) fn start_game_mode(&self) {
        self.game_mode.came_from.set(Page::Library);
        *self.game_mode.mode.borrow_mut() = Some(GameMode::default());
        self.show_game_mode();
    }

    /// The place of the first tile shown, among the shelf's games.
    fn first_visible(&self) -> usize {
        let mode = self.game_mode.mode.borrow();
        let Some(mode) = mode.as_ref() else {
            return 0;
        };
        let Some(window) = self.window.upgrade() else {
            return 0;
        };
        let browser = self.browser.borrow();
        let games = browser.games();
        let len = game_mode::shelf_games(games, mode.shown_shelf(games)).len();
        let count = game_mode::tiles_that_fit(window.global::<GameModeData>().get_width()).min(len);
        game_mode::first_tile(mode.hero_index(games), len, count)
    }

    /// Writes the screen: the shelves, the hero, the carousel and the menu.
    /// Cheap when Game Mode is off.
    pub(super) fn show_game_mode(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let data = window.global::<GameModeData>();
        let mut want_facts: Option<String> = None;
        {
            let mode = self.game_mode.mode.borrow();
            let Some(mode) = mode.as_ref() else {
                data.set_on(false);
                data.set_menu_open(false);
                return;
            };
            data.set_on(true);
            data.set_browsing(mode.browsing);
            let browser = self.browser.borrow();
            let games = browser.games();
            let shown = mode.shown_shelf(games);
            self.game_mode.shelves.set_vec(
                Shelf::ALL
                    .iter()
                    .enumerate()
                    .map(|(at, shelf)| {
                        chip(
                            at,
                            shelf.label(),
                            game_mode::shelf_games(games, *shelf).len().to_string(),
                            *shelf == shown,
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            let list = game_mode::shelf_games(games, shown);
            data.set_empty(list.is_empty());
            let index = mode.hero_index(games);
            let count = game_mode::tiles_that_fit(data.get_width()).min(list.len());
            let first = game_mode::first_tile(index, list.len(), count);
            let mut covers = self.covers.borrow_mut();
            let tiles: Vec<GmTile> = (first..first + count)
                .map(|at| {
                    let game = list[at];
                    let cover = game.cover.as_deref().map(|path| covers.get(path));
                    let loading = matches!(cover, Some(Cover::Loading));
                    let image = match cover {
                        Some(Cover::Ready(image)) => Some(image),
                        _ => None,
                    };
                    GmTile {
                        title: game.title.clone().into(),
                        has_cover: image.is_some(),
                        loading,
                        cover: image.unwrap_or_default(),
                        current: at == index,
                    }
                })
                .collect();
            self.game_mode.tiles.set_vec(tiles);

            if let Some(game) = list.get(index) {
                let summary = self
                    .store
                    .browser
                    .borrow()
                    .owned(&game.id)
                    .and_then(|owned| owned.summary.clone())
                    .or_else(|| {
                        self.facts_of(&game.id)
                            .and_then(|facts| facts.details.summary)
                    });
                data.set_meta(game_mode::meta_line(game, running::now()).into());
                data.set_title(game.title.clone().into());
                data.set_description(game_mode::description(summary.as_deref(), game).into());
                let cover = game.cover.as_deref().map(|path| covers.get(path));
                data.set_hero_loading(matches!(cover, Some(Cover::Loading)));
                let image = match cover {
                    Some(Cover::Ready(image)) => Some(image),
                    _ => None,
                };
                data.set_hero_has_cover(image.is_some());
                data.set_hero_cover(image.unwrap_or_default());
                if mode.facts_for.as_deref() != Some(game.id.as_str()) {
                    want_facts = Some(game.id.clone());
                }
            }
            data.set_menu_open(mode.menu.is_some());
            self.game_mode.menu.set_vec(
                MenuChoice::ALL
                    .iter()
                    .enumerate()
                    .map(|(at, choice)| {
                        chip(at, choice.label(), String::new(), mode.menu == Some(at))
                    })
                    .collect::<Vec<_>>(),
            );
        }
        // Steam's summary for the game that took the hero, asked once.
        if let Some(id) = want_facts {
            if let Some(mode) = self.game_mode.mode.borrow_mut().as_mut() {
                mode.facts_for = Some(id.clone());
            }
            self.want_known_facts(&id);
        }
    }

    fn run_choice(&self, choice: MenuChoice) {
        let leave = {
            let mut mode = self.game_mode.mode.borrow_mut();
            mode.as_mut().is_some_and(|mode| mode.choose(choice))
        };
        if leave {
            self.leave_game_mode();
        } else {
            self.show_game_mode();
            if self
                .game_mode
                .mode
                .borrow()
                .as_ref()
                .is_some_and(|mode| mode.browsing)
            {
                self.navigate(Page::Library);
            }
        }
    }

    /// Starts the game in the hero.
    fn play_hero(&self) {
        let game = {
            let mode = self.game_mode.mode.borrow();
            let browser = self.browser.borrow();
            mode.as_ref()
                .and_then(|mode| mode.hero(browser.games()).cloned())
        };
        if let Some(game) = game {
            let id = game.id.clone();
            self.play_record(game);
            // The launch moves the game to the front of Continue playing: the
            // hero follows it instead of staying on the same place.
            self.with_mode(|mode, games| {
                let shown = mode.shown_shelf(games);
                if let Some(at) = game_mode::shelf_games(games, shown)
                    .iter()
                    .position(|game| game.id == id)
                {
                    mode.index = at;
                }
            });
        }
    }

    /// Opens the hero's own page; Escape there comes back to the screen.
    fn hero_details(&self) {
        let id = {
            let mode = self.game_mode.mode.borrow();
            let browser = self.browser.borrow();
            mode.as_ref()
                .and_then(|mode| mode.hero(browser.games()).map(|game| game.id.clone()))
        };
        if let Some(id) = id {
            self.open_game(&id);
        }
    }

    /// Escape on the screen: the menu opens, or closes. Says whether it was
    /// taken.
    pub(super) fn game_mode_back(&self) -> bool {
        if !self.game_mode_screen() {
            return false;
        }
        self.with_mode(|mode, _| {
            mode.menu = if mode.menu.is_some() { None } else { Some(0) };
        });
        true
    }

    /// A key on the screen: the arrows move along the carousel (or the
    /// menu), Enter plays (or chooses), Tab changes the shelf; the rest is
    /// not for the grid behind it.
    pub(super) fn game_mode_key(&self, key: char) -> bool {
        let menu_open = self
            .game_mode
            .mode
            .borrow()
            .as_ref()
            .is_some_and(|mode| mode.menu.is_some());
        let enter = key == char::from(Key::Return) || key == '\n';
        let step = if key == char::from(Key::LeftArrow) {
            Some(Move::Left)
        } else if key == char::from(Key::RightArrow) {
            Some(Move::Right)
        } else if key == char::from(Key::UpArrow) {
            Some(Move::Up)
        } else if key == char::from(Key::DownArrow) {
            Some(Move::Down)
        } else if key == char::from(Key::Home) {
            Some(Move::First)
        } else if key == char::from(Key::End) {
            Some(Move::Last)
        } else if key == char::from(Key::PageUp) {
            Some(Move::PageUp)
        } else if key == char::from(Key::PageDown) {
            Some(Move::PageDown)
        } else {
            None
        };
        if menu_open {
            if let Some(step) = step {
                self.with_mode(|mode, _| mode.move_menu(step));
            } else if enter {
                self.choose_under_cursor();
            }
        } else if step == Some(Move::Left) {
            self.with_mode(|mode, games| mode.step(-1, games));
        } else if step == Some(Move::Right) {
            self.with_mode(|mode, games| mode.step(1, games));
        } else if enter {
            self.play_hero();
        } else if key == char::from(Key::Tab) {
            self.with_mode(|mode, games| mode.cycle_shelf(1, games));
        }
        true
    }

    fn choose_under_cursor(&self) {
        let choice = self
            .game_mode
            .mode
            .borrow()
            .as_ref()
            .and_then(GameMode::menu_choice);
        if let Some(choice) = choice {
            self.run_choice(choice);
        }
    }

    /// The pad on the screen: A plays (or chooses), B opens the menu (or
    /// closes it), LB and RB and Y change the shelf.
    pub(super) fn game_mode_pad(&self, pad: Pad) {
        let menu_open = self
            .game_mode
            .mode
            .borrow()
            .as_ref()
            .is_some_and(|mode| mode.menu.is_some());
        match pad {
            Pad::Left | Pad::Up if menu_open => self.with_mode(|mode, _| mode.move_menu(Move::Up)),
            Pad::Right | Pad::Down if menu_open => {
                self.with_mode(|mode, _| mode.move_menu(Move::Down));
            }
            Pad::Activate if menu_open => self.choose_under_cursor(),
            Pad::Left => self.with_mode(|mode, games| mode.step(-1, games)),
            Pad::Right => self.with_mode(|mode, games| mode.step(1, games)),
            Pad::Activate => self.play_hero(),
            Pad::Back => {
                self.game_mode_back();
            }
            Pad::Sort | Pad::NextShelf => self.with_mode(|mode, games| mode.cycle_shelf(1, games)),
            Pad::PrevShelf => self.with_mode(|mode, games| mode.cycle_shelf(-1, games)),
            Pad::Up | Pad::Down | Pad::Hide | Pad::ShowHidden | Pad::ToggleGameMode => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use slint::Model;

    use super::super::tests::page;
    use super::*;
    use crate::GameData;

    fn press(glue: &Rc<Glue>, key: Key) -> bool {
        glue.key(&String::from(char::from(key)), false, false, false)
    }

    #[test]
    fn f11_enters_and_leaves_game_mode_on_the_page_it_came_from() {
        let (window, glue) = page(3);
        let data = window.global::<GameModeData>();
        data.set_width(1400.0);
        window.invoke_page_requested(Page::Store);
        assert!(!data.get_on());

        assert!(press(&glue, Key::F11));
        assert!(data.get_on());
        assert!(glue.game_mode_screen());
        assert_eq!(
            window.get_page(),
            Page::Library,
            "Game Mode opens on the Library"
        );
        assert_eq!(data.get_shelves().row_count(), 2);
        assert_eq!(
            data.get_tiles().row_count(),
            3,
            "every game fits in a wide window"
        );
        assert!(data.get_tiles().row_data(0).unwrap().current);
        assert_eq!(data.get_title(), "Game 00");

        assert!(press(&glue, Key::F11));
        assert!(!data.get_on());
        assert!(!glue.game_mode_screen());
        assert_eq!(window.get_page(), Page::Store, "back where it was left");
    }

    #[test]
    fn the_arrows_move_the_hero_and_enter_plays_it() {
        let (window, glue) = page(3);
        let data = window.global::<GameModeData>();
        data.set_width(1400.0);
        press(&glue, Key::F11);
        assert!(press(&glue, Key::LeftArrow));
        assert_eq!(data.get_title(), "Game 00", "not before the first");
        press(&glue, Key::RightArrow);
        assert_eq!(data.get_title(), "Game 01");
        assert!(data.get_tiles().row_data(1).unwrap().current);
        press(&glue, Key::RightArrow);
        press(&glue, Key::RightArrow);
        assert_eq!(data.get_title(), "Game 02", "not past the last");

        press(&glue, Key::Return);
        // In tests Play hands the game to its launcher and starts nothing,
        // but the launch is counted.
        let launches = |id| {
            glue.browser
                .borrow()
                .game(id)
                .map(|game| game.played.launches)
        };
        assert_eq!(launches("desktop:2"), Some(1));
        assert_eq!(launches("desktop:1"), Some(0), "only the hero");
        assert_eq!(
            data.get_title(),
            "Game 02",
            "the hero stays on the game played"
        );

        // On Continue playing a played game moves to the front: the hero
        // follows it, so Enter on the second tile leaves it on the first.
        let _ = glue.browser.borrow_mut().launched("desktop:0", 100);
        let _ = glue.browser.borrow_mut().launched("desktop:1", 200);
        data.invoke_shelf(0);
        press(&glue, Key::RightArrow);
        press(&glue, Key::RightArrow);
        assert_eq!(data.get_title(), "Game 00", "the oldest of the three");
        press(&glue, Key::Return);
        assert_eq!(data.get_title(), "Game 00");
        assert!(data.get_tiles().row_data(0).unwrap().current);
        // Typed text and other keys do not reach the grid behind the screen.
        assert!(glue.key("x", false, false, false));
        assert_eq!(glue.browser.borrow().search(), "");
    }

    #[test]
    fn escape_opens_the_menu_and_its_choices_work() {
        let (window, glue) = page(2);
        let data = window.global::<GameModeData>();
        data.set_width(1400.0);
        press(&glue, Key::F11);
        glue.back();
        assert!(data.get_menu_open());
        assert_eq!(data.get_menu().row_count(), 3);
        glue.back();
        assert!(!data.get_menu_open(), "Escape closes it again");

        // Library: the grid shows, the screen does not, and the chip returns.
        glue.back();
        press(&glue, Key::DownArrow);
        press(&glue, Key::Return);
        assert!(data.get_browsing());
        assert!(!glue.game_mode_screen());
        assert!(data.get_on(), "still in Game Mode");
        data.invoke_show();
        assert!(glue.game_mode_screen());

        // Leave Game Mode.
        glue.back();
        press(&glue, Key::End);
        press(&glue, Key::Return);
        assert!(!data.get_on());
        assert!(!glue.game_mode_on());
    }

    #[test]
    fn tab_changes_the_shelf_and_details_opens_the_page_and_escape_comes_back() {
        let (window, glue) = page(3);
        let data = window.global::<GameModeData>();
        data.set_width(1400.0);
        let _ = glue.browser.borrow_mut().launched("desktop:1", 500);
        press(&glue, Key::F11);
        assert_eq!(
            data.get_title(),
            "Game 01",
            "Continue shows what was played"
        );
        assert_eq!(data.get_tiles().row_count(), 1);
        press(&glue, Key::Tab);
        assert_eq!(data.get_tiles().row_count(), 3, "All games");
        data.invoke_shelf(0);
        assert_eq!(data.get_tiles().row_count(), 1);

        data.invoke_details();
        assert!(window.global::<GameData>().get_open());
        assert!(!glue.game_mode_screen(), "the page shows over the screen");
        glue.back();
        assert!(glue.game_mode_screen(), "Escape on the page comes back");
    }
}
