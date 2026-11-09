//! A game's page: its wide art with the title over it, one bar of actions,
//! then what it is about, its DLC and mods beside a short panel of details,
//! with the technical ones folded under it.

use std::f32::consts::{FRAC_PI_2, PI};
use std::path::Path;

use iced::gradient::Linear;
use iced::widget::{button, column, container, image, row, scrollable, space, stack, text};
use iced::{
    Alignment, Background, Border, Color, ContentFit, Element, Font, Length, Radians, Task,
};
use zendo_core::downloads::{Action, Job};
use zendo_core::launch;
use zendo_core::owned::Owned;
use zendo_core::{Game, Size, Source};

use super::downloads::{DownloadMessage, UpdateState, action_doing_alone, update_line};
use super::steam_installs::SteamInstallMessage;
use super::store::InfoState;
use super::style::{self, BOLD, colors};
use super::{Message, Page, Zendo, format, game_scroll_id, now, sidebar};
use crate::i18n::t;

const HERO_HEIGHT: f32 = 340.0;
const COVER_WIDTH: f32 = 140.0;
const COVER_HEIGHT: f32 = 210.0;
const RADIUS: f32 = 14.0;
const LABEL_WIDTH: f32 = 130.0;
/// The page's width from which the details go beside the rest.
pub(super) const TWO_COLUMNS: f32 = 960.0;

/// What is open on a game's page; it starts folded on each game.
#[derive(Debug, Default)]
pub struct GameView {
    /// The More row of less used actions.
    more: bool,
    /// The Technical section under the details.
    pub(super) technical: bool,
    /// The DLC whose Uninstall was pressed once.
    pub(super) confirm_dlc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum GameViewMessage {
    ToggleMore,
    ToggleTechnical,
    /// Uninstall pressed once on a DLC: pressed again, it goes.
    AskRemoveDlc(String),
}

impl Zendo {
    pub(super) fn update_game_view(&mut self, message: GameViewMessage) -> Task<Message> {
        let view = &mut self.game_view;
        match message {
            GameViewMessage::ToggleMore => view.more = !view.more,
            GameViewMessage::ToggleTechnical => view.technical = !view.technical,
            GameViewMessage::AskRemoveDlc(id) => view.confirm_dlc = Some(id),
        }
        Task::none()
    }

    pub(super) fn game_page<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let trail = row![
            button(text(t!("game-back")).size(14))
                .padding(0)
                .style(style::link)
                .on_press(Message::Navigate(Page::Library)),
            text("/").size(14).color(colors().dim),
            text(&game.title).size(14).color(colors().dim),
        ]
        .spacing(8);

        let update = matches!(
            self.downloads.updates.get(&game.id),
            Some(UpdateState::Available(_))
        );
        let mut page = column![
            trail,
            game_hero(game, update, self.rating_pill(&game.id)),
            self.action_bar(game, update),
        ]
        .spacing(20)
        .padding(24);
        if self.game_view.more {
            page = page.push(self.more_row(game));
        }

        let owned = self.store.owned(&game.id);
        let mut main: Vec<Element<'a, Message>> = Vec::new();
        if let Some(summary) = owned.and_then(|owned| owned.summary.as_deref()) {
            main.push(about_card(summary));
        }
        if let Some(dlc) = self.dlc_card(game) {
            main.push(dlc);
        }
        if let Some(mods) = self.mods_card(game) {
            main.push(mods);
        }
        if let Some(prefix) = self.prefix_card(game) {
            main.push(prefix);
        }
        let side = column![self.details_card(game), self.technical_card(game)].spacing(20);

        let two = self.window_width - sidebar::WIDTH >= TWO_COLUMNS;
        page = if main.is_empty() {
            page.push(side)
        } else if two {
            page.push(
                row![
                    column(main).spacing(20).width(Length::FillPortion(3)),
                    side.width(Length::FillPortion(2)),
                ]
                .spacing(20),
            )
        } else {
            page.extend(main).push(side)
        };
        scrollable(page)
            .id(game_scroll_id())
            .height(Length::Fill)
            .into()
    }

    /// Play, Update when one waits, Launch options, Pin and More, then the
    /// job queued for the game or the last thing said.
    fn action_bar<'a>(&'a self, game: &'a Game, update: bool) -> Element<'a, Message> {
        let job = self.downloads.job_for(&game.id);
        let mut bar = row![self.play_buttons(game)]
            .spacing(10)
            .align_y(Alignment::Center);
        if update && game.installed_by_zendo() {
            bar = bar.push(
                button(
                    text(t!("action-label", action = "update"))
                        .size(14)
                        .font(BOLD),
                )
                .padding([10, 18])
                .style(style::secondary)
                .on_press_maybe(job.is_none().then(|| queue(game, Action::Update)).flatten()),
            );
        }
        bar = bar
            .push(launch_options_button(game))
            .push(
                button(
                    text(if game.pinned {
                        t!("game-unpin")
                    } else {
                        t!("game-pin")
                    })
                    .size(14),
                )
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::TogglePinned),
            )
            .push(
                button(
                    text(if self.game_view.more {
                        t!("game-less")
                    } else {
                        t!("game-more")
                    })
                    .size(14),
                )
                .padding([10, 18])
                .style(style::secondary)
                .on_press(Message::GameView(GameViewMessage::ToggleMore)),
            )
            .push(space::horizontal());
        if let Some((job, running)) = job {
            let state = match (running, job.paused) {
                (true, _) => "now",
                (false, true) => "paused",
                (false, false) => "queued",
            };
            bar = bar
                .push(
                    text(t!(
                        "downloads-job-state",
                        doing = action_doing_alone(job.action),
                        state = state
                    ))
                    .size(14)
                    .color(colors().dim),
                )
                .push(
                    button(text(t!("see-downloads")).size(13))
                        .padding([6, 12])
                        .style(style::secondary)
                        .on_press(Message::Navigate(Page::Downloads)),
                );
        } else {
            let status = self.status.clone().unwrap_or_else(|| t!("game-ready"));
            bar = bar.push(text(status).size(14).color(colors().dim));
        }
        bar.into()
    }

    /// The less used actions: Hide, and for a game Zendo installed Check for
    /// updates, Repair, Move and Uninstall; for one added by hand Edit and
    /// Remove.
    fn more_row<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let mut more = row![
            button(
                text(if game.hidden {
                    t!("game-unhide")
                } else {
                    t!("game-hide")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press(Message::ToggleHidden),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if game.installed_by_zendo() {
            let free = self.downloads.job_for(&game.id).is_none();
            let checking = self.downloads.updates.get(&game.id) == Some(&UpdateState::Checking);
            more =
                more.push(
                    button(text(t!("downloads-check-updates")).size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe((!checking).then(|| {
                            Message::Download(DownloadMessage::CheckUpdate(game.id.clone()))
                        })),
                );
            more = more.push(
                button(text(t!("action-label", action = "repair")).size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(free.then(|| queue(game, Action::Repair)).flatten()),
            );
            let movable = free && !self.running.is_running(&game.id);
            more =
                more.push(
                    button(text(t!("downloads-move")).size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(movable.then(|| {
                            Message::Download(DownloadMessage::ChooseMove(game.id.clone()))
                        })),
                );
            let confirming = self.downloads.confirm_uninstall.as_deref() == Some(game.id.as_str());
            more =
                more.push(if confirming {
                    button(text(t!("downloads-press-again")).size(14).font(BOLD))
                        .padding([8, 14])
                        .style(style::danger)
                        .on_press_maybe(free.then(|| queue(game, Action::Uninstall)).flatten())
                } else {
                    button(text(t!("action-label", action = "uninstall")).size(14))
                        .padding([8, 14])
                        .style(style::secondary)
                        .on_press_maybe(free.then(|| {
                            Message::Download(DownloadMessage::AskUninstall(game.id.clone()))
                        }))
                });
        }
        if let Some(app) = steam_app(game) {
            more = more.push(self.steam_buttons(game, app));
        }
        more = more.push(self.local_buttons(game));
        container(more)
            .padding([12, 16])
            .width(Length::Fill)
            .style(style::card)
            .into()
    }

    /// Verify and Uninstall for a Steam game, asked of Steam, which shows
    /// its own question and progress.
    fn steam_buttons<'a>(&self, game: &'a Game, app: &'a str) -> Element<'a, Message> {
        let ask = |make: fn(String, String) -> SteamInstallMessage| {
            Message::SteamInstall(make(app.to_owned(), game.title.clone()))
        };
        let verifying = self.steam_installs.verifying(app);
        let uninstall = if self.steam_installs.removing(app) {
            t!("steam-ask-again-uninstall")
        } else {
            t!("steam-uninstall")
        };
        row![
            button(
                text(if verifying {
                    t!("steam-verifying")
                } else {
                    t!("steam-verify")
                })
                .size(14)
            )
            .padding([8, 14])
            .style(style::secondary)
            .on_press_maybe((!verifying).then(|| ask(SteamInstallMessage::Verify))),
            button(text(uninstall).size(14))
                .padding([8, 14])
                .style(style::secondary)
                .on_press(ask(SteamInstallMessage::Uninstall)),
        ]
        .spacing(10)
        .into()
    }

    /// The details worth a look: the store and install, the latest from
    /// the store, and the time played.
    fn details_card<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        let mut add = |label: String, value: Element<'a, Message>| {
            rows.push(label_row(label, value));
        };
        add(t!("fact-store"), plain(format::source(game.source)));
        add(t!("fact-started-by"), plain(game.launcher()));
        if let Some(platform) = &game.details.platform {
            add(t!("fact-platform"), plain(platform_label(platform)));
        }
        if let Some(version) = &game.details.version {
            add(t!("fact-version"), plain(version.clone()));
        }
        if let Some(size) = &game.details.size {
            add(t!("fact-install-size"), plain(size.to_string()));
        }
        if game.installed_by_zendo() {
            let (line, color) = update_line(self.downloads.updates.get(&game.id));
            if !line.is_empty() {
                add(t!("fact-updates"), text(line).size(14).color(color).into());
            }
        }
        if let Some(line) = self.cloud_line(game) {
            add(t!("fact-cloud-saves"), line);
        }
        let facts = store_facts(
            self.store.owned(&game.id),
            self.store.info.get(&game.id),
            game.details.size.is_some(),
        );
        for (label, value) in facts {
            add(label, plain(value));
        }
        if let Some(path) = &game.details.install_path {
            add(
                t!("fact-install-path"),
                column![
                    text(path.display().to_string())
                        .size(12)
                        .font(Font::MONOSPACE),
                    open_button(t!("files-open-folder"), path),
                ]
                .spacing(6)
                .into(),
            );
        }
        add(
            t!("fact-last-played"),
            plain(
                game.played
                    .last
                    .map_or_else(|| t!("fact-never"), |last| format::ago(last, now())),
            ),
        );
        if game.played.minutes > 0 {
            add(
                t!("fact-play-time"),
                plain(format::play_time(game.played.minutes)),
            );
        }
        add(
            t!("fact-started-from-zendo"),
            plain(t!("count-times", count = game.played.launches)),
        );
        card(t!("game-details"), column(rows).spacing(12).into())
    }

    /// The launch command, Zendo's id, and for a game Zendo starts its Wine
    /// prefix and last log; folded until asked for.
    fn technical_card<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let open = self.game_view.technical;
        let header = button(
            row![
                text(t!("game-technical")).size(18).font(BOLD),
                space::horizontal(),
                text(if open { "−" } else { "+" })
                    .size(18)
                    .color(colors().dim),
            ]
            .align_y(Alignment::Center),
        )
        .padding(0)
        .style(style::link)
        .on_press(Message::GameView(GameViewMessage::ToggleTechnical));
        let mut body = column![header].spacing(16);
        if open {
            let code = |value: String| -> Element<'a, Message> {
                text(value).size(12).font(Font::MONOSPACE).into()
            };
            let mut rows = column![
                label_row(t!("fact-launch-command"), code(game.command_line())),
                label_row(t!("fact-zendo-id"), code(game.id.clone())),
            ]
            .spacing(12);
            let starts_itself = game.installed_by_zendo() || game.source == Source::Local;
            if starts_itself
                && launch::needs_proton(game)
                && let Some(dir) = self.config.install_dir()
            {
                let prefix = launch::prefix_in(&dir, game);
                rows = rows.push(path_row(t!("run-wine-prefix"), &prefix));
            }
            if starts_itself && let Some(log) = launch::log_path(&game.id) {
                rows = rows.push(path_row(t!("run-last-log"), &log));
            }
            body = body.push(rows);
        }
        container(body)
            .padding(20)
            .width(Length::Fill)
            .style(style::card)
            .into()
    }
}

/// The job for an action on a game Zendo installed, as a message.
fn queue(game: &Game, action: Action) -> Option<Message> {
    Job::for_game(game, action).map(|job| Message::Download(DownloadMessage::Queue(job)))
}

/// A card with a heading.
pub(super) fn card(heading: String, body: Element<'_, Message>) -> Element<'_, Message> {
    container(column![text(heading).size(18).font(BOLD), body].spacing(16))
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
}

pub(super) fn label_row(label: String, value: Element<'_, Message>) -> Element<'_, Message> {
    row![
        text(label).size(14).color(colors().dim).width(LABEL_WIDTH),
        container(value).width(Length::Fill),
    ]
    .spacing(8)
    .into()
}

pub(super) fn plain<'a>(value: String) -> Element<'a, Message> {
    text(value).size(14).into()
}

fn open_button<'a>(label: String, path: &Path) -> Element<'a, Message> {
    button(text(label).size(13))
        .padding([4, 10])
        .style(style::secondary)
        .on_press_maybe(
            path.exists()
                .then(|| Message::Open(path.to_string_lossy().into_owned())),
        )
        .into()
}

/// A folder or file with Open under it.
fn path_row<'a>(label: String, path: &Path) -> Element<'a, Message> {
    label_row(
        label,
        column![
            text(path.display().to_string())
                .size(12)
                .font(Font::MONOSPACE),
            open_button(t!("run-open"), path),
        ]
        .spacing(6)
        .into(),
    )
}

/// What the wide art at the top of a game's page shows.
pub struct Hero<'a> {
    pub title: &'a str,
    pub source: Source,
    pub art: Option<&'a Path>,
    pub cover: Option<&'a Path>,
    pub badges: Vec<(String, Color)>,
    pub developer: Option<&'a str>,
    pub facts: Vec<String>,
    /// The `ProtonDB` pill, beside the badges.
    pub rating: Option<Element<'a, Message>>,
    /// The game whose art "Change art" picks.
    pub id: &'a str,
}

fn game_hero<'a>(
    game: &'a Game,
    update: bool,
    rating: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut badges = Vec::new();
    if update {
        badges.push((t!("badge-update"), colors().accent));
    }
    if let Some(platform) = &game.details.platform {
        badges.push((platform_label(platform), colors().dim));
    }
    if game.pinned {
        badges.push((t!("badge-pinned"), colors().accent));
    }
    if game.hidden {
        badges.push((t!("badge-hidden"), colors().dim));
    }
    hero(Hero {
        title: &game.title,
        source: game.source,
        art: game.hero.as_deref(),
        cover: game.cover.as_deref(),
        badges,
        developer: game.details.developer.as_deref(),
        facts: facts(game),
        rating,
        id: &game.id,
    })
}

pub fn hero(hero: Hero<'_>) -> Element<'_, Message> {
    let art: Element<'_, Message> = if let Some(path) = hero.art {
        image(image::Handle::from_path(path))
            .width(Length::Fill)
            .height(HERO_HEIGHT)
            .content_fit(ContentFit::Cover)
            .border_radius(RADIUS)
            .into()
    } else {
        let tint = style::source_color(hero.source);
        shade(
            Linear::new(Radians(FRAC_PI_2))
                .add_stop(0.0, colors().surface)
                .add_stop(1.0, Color { a: 0.25, ..tint }),
        )
    };
    // Darkened toward the bottom and the left, where the title sits.
    let down = shade(
        Linear::new(Radians(PI))
            .add_stop(0.0, Color::TRANSPARENT)
            .add_stop(
                0.45,
                Color {
                    a: 0.2,
                    ..colors().background
                },
            )
            .add_stop(
                1.0,
                Color {
                    a: 0.95,
                    ..colors().background
                },
            ),
    );
    let across = shade(
        Linear::new(Radians(FRAC_PI_2))
            .add_stop(
                0.0,
                Color {
                    a: 0.7,
                    ..colors().background
                },
            )
            .add_stop(0.6, Color::TRANSPARENT),
    );

    let mut badges = row![style::badge(
        format::source(hero.source),
        style::source_color(hero.source)
    )]
    .spacing(6);
    for (label, color) in hero.badges {
        badges = badges.push(style::badge(label, color));
    }
    if let Some(rating) = hero.rating {
        badges = badges.push(rating);
    }
    let mut info = column![badges, text(hero.title).size(38).font(BOLD)].spacing(8);
    if let Some(developer) = hero.developer {
        info = info.push(text(developer).size(15).color(colors().dim));
    }
    let facts: Vec<Element<'_, Message>> = hero.facts.into_iter().map(fact_chip).collect();
    if !facts.is_empty() {
        info = info.push(row(facts).spacing(8));
    }

    let mut content = row![info.width(Length::Fill)]
        .spacing(24)
        .align_y(Alignment::End);
    if let Some(cover) = hero.cover {
        content = content.push(
            image(image::Handle::from_path(cover))
                .width(COVER_WIDTH)
                .height(COVER_HEIGHT)
                .content_fit(ContentFit::Cover)
                .border_radius(10.0),
        );
    }
    let content = container(content)
        .padding(28)
        .width(Length::Fill)
        .height(HERO_HEIGHT)
        .align_y(Alignment::End);
    let change = container(
        button(text(t!("game-change-art")).size(13))
            .padding([6, 12])
            .style(style::secondary)
            .on_press(Message::Art(super::artwork::ArtMessage::Open(
                hero.id.to_owned(),
            ))),
    )
    .padding(16)
    .width(Length::Fill)
    .align_x(Alignment::End);

    stack![art, down, across, content, change].into()
}

/// A rounded panel the hero's size filled with a gradient.
fn shade<'a>(gradient: Linear) -> Element<'a, Message> {
    container(space::horizontal())
        .width(Length::Fill)
        .height(HERO_HEIGHT)
        .style(move |_| container::Style {
            background: Some(Background::Gradient(gradient.into())),
            border: Border {
                radius: RADIUS.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The few numbers worth seeing first.
fn facts(game: &Game) -> Vec<String> {
    let mut facts = Vec::new();
    if game.played.minutes > 0 {
        facts.push(t!(
            "played-time",
            time = format::play_time(game.played.minutes)
        ));
    }
    if let Some(last) = game.played.last {
        facts.push(t!("game-last-played", when = format::ago(last, now())));
    }
    if let Some(size) = &game.details.size {
        facts.push(size.to_string());
    }
    facts
}

fn fact_chip<'a>(fact: String) -> Element<'a, Message> {
    container(text(fact).size(13))
        .padding([4, 10])
        .style(|_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.7,
                ..colors().surface
            })),
            border: Border {
                color: colors().line,
                width: 1.0,
                radius: 14.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Stores write platforms as `Windows`, `windows`, `linux`; shown alike.
fn platform_label(platform: &str) -> String {
    match platform.to_lowercase().as_str() {
        "windows" => "Windows".to_owned(),
        "linux" => "Linux".to_owned(),
        "mac" | "osx" => "macOS".to_owned(),
        _ => platform.to_owned(),
    }
}

/// What the store says about a game: sizes and the latest version asked of
/// its engine, then what its list records. `installed` leaves out the size
/// on disk, which the install's own record gives.
pub fn store_facts(
    owned: Option<&Owned>,
    info: Option<&InfoState>,
    installed: bool,
) -> Vec<(String, String)> {
    let mut facts = Vec::new();
    match info {
        // Steam's store is asked for the page's details, not sizes.
        Some(InfoState::Asking) if owned.is_some_and(|owned| owned.store().is_none()) => {
            facts.push((t!("fact-store-details"), t!("fact-asking")));
        }
        Some(InfoState::Asking) => facts.push((t!("fact-download-size"), t!("fact-asking"))),
        Some(InfoState::Got(info)) => {
            if let Some(bytes) = info.download_size {
                facts.push((t!("fact-download-size"), Size::Bytes(bytes).to_string()));
            }
            if let Some(bytes) = info.install_size.filter(|_| !installed) {
                facts.push((t!("fact-size-on-disk"), Size::Bytes(bytes).to_string()));
            }
            if let Some(version) = &info.version {
                facts.push((t!("fact-latest-version"), version.clone()));
            }
            if let Some(day) = &info.updated {
                facts.push((t!("fact-latest-build"), day.clone()));
            }
        }
        Some(InfoState::Failed(err)) => {
            facts.push((
                t!("fact-store-details"),
                t!("fact-store-failed", error = err.as_str()),
            ));
        }
        None => {}
    }
    if let Some(owned) = owned {
        if let Some(sizes) = owned.sizes {
            facts.push((
                t!("fact-download-size"),
                Size::Bytes(sizes.download).to_string(),
            ));
            if !installed {
                facts.push((
                    t!("fact-size-on-disk"),
                    Size::Bytes(sizes.install).to_string(),
                ));
            }
        } else if let Some(bytes) = owned.space.filter(|_| !installed) {
            facts.push((t!("fact-space-needed"), Size::Bytes(bytes).to_string()));
        }
        if let Some(publisher) = &owned.publisher {
            facts.push((t!("fact-publisher"), publisher.clone()));
        }
        if let Some(day) = &owned.released {
            facts.push((t!("fact-released"), day.clone()));
        }
        if !owned.genres.is_empty() {
            facts.push((t!("fact-genres"), owned.genres.join(", ")));
        }
    }
    facts
}

/// A game Steam installed, by its app id.
fn steam_app(game: &Game) -> Option<&str> {
    (game.source == Source::Steam)
        .then(|| game.id.strip_prefix("steam:"))
        .flatten()
}

/// What the store says the game is about.
pub fn about_card(summary: &str) -> Element<'_, Message> {
    container(
        column![
            text(t!("game-about")).size(18).font(BOLD),
            text(summary).size(14).line_height(1.5),
        ]
        .spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

/// Opens the launch options sheet, for the games Zendo starts itself.
fn launch_options_button(game: &Game) -> Element<'_, Message> {
    if !super::launch_options::applies(game) {
        return space::horizontal().width(0).into();
    }
    button(text(t!("game-launch-options")).size(14))
        .padding([10, 18])
        .style(style::secondary)
        .on_press(Message::Sheet(super::launch_options::SheetMessage::Open))
        .into()
}
