//! A game's page: its wide art with the title over it, one bar of actions,
//! then what it is about, its DLC, mods and GOG changelog beside a short
//! panel of details, with the technical ones folded under it.

use std::collections::BTreeSet;
use std::path::Path;

use iced::widget::{button, column, container, image, row, scrollable, space, stack, text};
use iced::{Alignment, ContentFit, Element, Font, Length, Task};
use zendo_core::downloads::{Action, Job};
use zendo_core::launch;
use zendo_core::owned::Owned;
use zendo_core::{Game, Size, Source};

use super::artwork::ArtMessage;
use super::categories::CategoriesMessage;
use super::downloads::{DownloadMessage, FolderFor, UpdateState, action_doing_alone, update_line};
use super::protondb::RatingState;
use super::steam_installs::SteamInstallMessage;
use super::store::InfoState;
use super::style::{self, BOLD, SPACE_L, SPACE_M, SPACE_S, SPACE_XL, SPACE_XS, colors};
use super::{Message, Page, Zendo, format, game_scroll_id, now};
use crate::i18n::t;

/// The page's top: the cover, two buttons under it, and the padding.
const HEADER_HEIGHT: f32 = 408.0;
const COVER_WIDTH: f32 = 200.0;
const COVER_HEIGHT: f32 = 300.0;
/// How much of the wide art shows behind the header.
const BACKDROP_OPACITY: f32 = 0.22;
pub(super) const LABEL_WIDTH: f32 = 130.0;
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
    /// How many changelog entries are shown beyond the newest.
    pub(super) changelog_more: usize,
    /// How many achievements are shown beyond the first ones.
    pub(super) achievements_more: usize,
}

#[derive(Debug, Clone)]
pub enum GameViewMessage {
    ToggleMore,
    ToggleTechnical,
    /// Uninstall pressed once on a DLC: pressed again, it goes.
    AskRemoveDlc(String),
    /// Show earlier updates, on the changelog card.
    MoreChangelog,
    /// Show more, on the achievements card.
    MoreAchievements,
}

impl Zendo {
    pub(super) fn update_game_view(&mut self, message: GameViewMessage) -> Task<Message> {
        let view = &mut self.game_view;
        match message {
            GameViewMessage::ToggleMore => view.more = !view.more,
            GameViewMessage::ToggleTechnical => view.technical = !view.technical,
            GameViewMessage::AskRemoveDlc(id) => view.confirm_dlc = Some(id),
            GameViewMessage::MoreChangelog => view.changelog_more += super::gog_changelog::MORE,
            GameViewMessage::MoreAchievements => {
                view.achievements_more += super::achievements::SHOWN;
            }
        }
        Task::none()
    }

    pub(super) fn game_page<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let trail = button(text(t!("game-back").to_uppercase()).size(14))
            .padding(0)
            .style(style::link)
            .on_press(Message::Navigate(Page::Library));

        let update = matches!(
            self.downloads.updates.get(&game.id),
            Some(UpdateState::Available(_))
        );
        let mut page = column![
            trail,
            self.game_header(game, update),
            self.action_bar(game, update),
            Self::organize_bar(game),
            stat_tiles(game),
        ]
        .spacing(SPACE_XL)
        .padding(SPACE_XL);
        if self.game_view.more && has_more(game) {
            page = page.push(self.more_row(game));
        }

        let owned = self.store.owned(&game.id);
        let mut main: Vec<Element<'a, Message>> = Vec::new();
        if let Some(summary) = owned.and_then(|owned| owned.summary.as_deref()) {
            main.push(about_card(summary));
        }
        self.add_anti_cheat_card(&game.id, &mut main);
        if let Some(dlc) = self.dlc_card(game) {
            main.push(dlc);
        }
        if let Some(mods) = self.mods_card(game) {
            main.push(mods);
        }
        if let Some(prefix) = self.prefix_card(game) {
            main.push(prefix);
        }
        if let Some(changelog) = self.changelog_card(game) {
            main.push(changelog);
        }
        main.extend(self.achievements_card(&game.id));
        main.extend(self.requirements_card(&game.id));
        let side = column![self.details_card(game), self.technical_card(game)].spacing(24);

        let two = self.window_width >= TWO_COLUMNS;
        page = if main.is_empty() {
            page.push(side)
        } else if two {
            page.push(
                row![
                    column(main).spacing(24).width(Length::FillPortion(3)),
                    side.width(Length::FillPortion(2)),
                ]
                .spacing(24),
            )
        } else {
            page.extend(main).push(side)
        };
        scrollable(page)
            .id(game_scroll_id())
            .height(Length::Fill)
            .into()
    }

    /// The top of the page: the game's wide art faint behind, the portrait
    /// cover on the left with the ways to change the pictures, and on the
    /// right the title, where it comes from and the links to read more.
    fn game_header<'a>(&'a self, game: &'a Game, update: bool) -> Element<'a, Message> {
        let cover: Element<'a, Message> = match &game.cover {
            Some(path) => image(image::Handle::from_path(path))
                .width(COVER_WIDTH)
                .height(COVER_HEIGHT)
                .content_fit(ContentFit::Cover)
                .border_radius(style::RADIUS_PANEL)
                .into(),
            None => container(text(&game.title).size(20).center())
                .padding(SPACE_M)
                .center(Length::Fill)
                .width(COVER_WIDTH)
                .height(COVER_HEIGHT)
                .style(style::placeholder)
                .into(),
        };
        let change = |label: String, message: Message| {
            button(text(label.to_uppercase()).size(12))
                .padding([SPACE_XS, SPACE_S])
                .width(Length::Fill)
                .style(style::secondary)
                .on_press(message)
        };
        let pictures = column![
            cover,
            change(
                t!("game-change-cover"),
                Message::Art(ArtMessage::Open(game.id.clone())),
            ),
            change(
                t!("game-change-backdrop"),
                Message::Art(ArtMessage::OpenHero(game.id.clone())),
            ),
        ]
        .spacing(SPACE_S)
        .width(COVER_WIDTH);

        let mut source = vec![format::source(game.source)];
        source.extend(game.details.developer.clone());
        source.extend(game.details.platform.as_deref().map(platform_label));
        let mut links = row![].spacing(SPACE_S).align_y(Alignment::Center);
        if update {
            links = links.push(style::badge(t!("badge-update"), colors().accent));
        }
        if game.pinned {
            links = links.push(style::badge(t!("badge-favorite"), colors().accent));
        }
        if game.hidden {
            links = links.push(style::badge(t!("badge-hidden"), colors().dim));
        }
        links = links.extend(self.hero_pills(&game.id));
        links = links.push(self.wiki_chip(game));
        let info = column![
            text(&game.title).size(34).font(BOLD),
            text(source.join(" · ").to_uppercase())
                .size(13)
                .color(colors().dim),
            container(links.wrap().vertical_spacing(SPACE_S)).padding([SPACE_S, 0.0]),
        ]
        .spacing(SPACE_S)
        .width(Length::Fill);

        let content = container(
            row![pictures, info]
                .spacing(SPACE_XL)
                .align_y(Alignment::Start),
        )
        .padding(SPACE_L)
        .width(Length::Fill)
        .height(HEADER_HEIGHT);
        match &game.hero {
            Some(path) => stack![
                image(image::Handle::from_path(path))
                    .width(Length::Fill)
                    .height(HEADER_HEIGHT)
                    .content_fit(ContentFit::Cover)
                    .opacity(BACKDROP_OPACITY),
                content,
            ]
            .into(),
            None => content.into(),
        }
    }

    /// The `PCGamingWiki` chip: by Steam's app id when the game has one (its
    /// own, or the one `ProtonDB` found), else a search by title.
    fn wiki_chip<'a>(&self, game: &Game) -> Element<'a, Message> {
        let app = steam_app(game)
            .and_then(|app| app.parse::<u32>().ok())
            .or_else(|| match self.ratings.get(&game.id) {
                Some(RatingState::Got(Some(rating))) => Some(rating.app),
                _ => None,
            });
        button(text("PCGAMINGWIKI").size(11).font(BOLD))
            .padding([2, 7])
            .style(style::pill(colors().dim))
            .on_press(Message::Open(pcgamingwiki_url(app, &game.title)))
            .into()
    }

    /// The game's categories, and the way to change them.
    fn organize_bar(game: &Game) -> Element<'_, Message> {
        let mut bar =
            row![container(style::label(&t!("library-organize"))).padding([SPACE_XS, 0.0]),]
                .spacing(SPACE_S)
                .align_y(Alignment::Center);
        if game.categories.is_empty() {
            bar = bar.push(text(t!("categories-none")).size(13).color(colors().dim));
        }
        for name in &game.categories {
            bar = bar.push(style::badge(name.to_uppercase(), colors().text));
        }
        bar.push(
            button(text(t!("game-edit-categories").to_uppercase()).size(12))
                .padding([SPACE_XS, SPACE_S])
                .style(style::secondary)
                .on_press(change_categories(game)),
        )
        .wrap()
        .vertical_spacing(SPACE_S)
        .into()
    }

    /// Play, Update when one waits, Favorite, Launch options, Hide, Manage
    /// in Steam and More, then the job queued for the game or the last
    /// thing said.
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
                .on_press_maybe(queue(game, Action::Update).filter(|_| job.is_none())),
            );
        }
        if super::launch_options::applies(game) {
            bar = bar.push(launch_options_button(game));
        }
        let action = |label: String, message: Message| {
            button(text(label.to_uppercase()).size(14))
                .padding([10, 18])
                .style(style::secondary)
                .on_press(message)
        };
        bar = bar.push(action(
            if game.pinned {
                t!("game-unfavorite")
            } else {
                t!("game-favorite")
            },
            Message::TogglePinned,
        ));
        bar = bar.push(action(
            if game.hidden {
                t!("game-unhide")
            } else {
                t!("game-hide")
            },
            Message::ToggleHidden,
        ));
        if let Some(app) = steam_app(game) {
            bar = bar.push(action(
                t!("game-manage-steam"),
                Message::Open(format!("steam://nav/games/details/{app}")),
            ));
        }
        if has_more(game) {
            bar = bar.push(action(
                if self.game_view.more {
                    t!("game-less")
                } else {
                    t!("game-more")
                },
                Message::GameView(GameViewMessage::ToggleMore),
            ));
        }
        let mut line = row![].spacing(10).align_y(Alignment::Center);
        if let Some((job, running)) = job {
            let state = match (running, job.paused) {
                (true, _) => "now",
                (false, true) => "paused",
                (false, false) => "queued",
            };
            line = line
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
            line = line.push(text(status).size(14).color(colors().dim));
        }
        column![bar.wrap().vertical_spacing(SPACE_S), line]
            .spacing(SPACE_S)
            .into()
    }

    /// The less used actions: for a game Zendo installed Check for updates,
    /// Repair, Move and Uninstall; for a Steam game Verify and Uninstall;
    /// for one added by hand Edit and Remove.
    fn more_row<'a>(&'a self, game: &'a Game) -> Element<'a, Message> {
        let mut more = row![].spacing(10).align_y(Alignment::Center);
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
                    .on_press_maybe(queue(game, Action::Repair).filter(|_| free)),
            );
            let movable = free && !self.running.is_open(&game.id);
            more = more.push(
                button(text(t!("downloads-move")).size(14))
                    .padding([8, 14])
                    .style(style::secondary)
                    .on_press_maybe(movable.then(|| {
                        Message::Download(DownloadMessage::ChooseFolder(
                            FolderFor::Move,
                            game.id.clone(),
                        ))
                    })),
            );
            let confirming = self.downloads.confirm_uninstall.as_deref() == Some(game.id.as_str());
            more =
                more.push(if confirming {
                    button(text(t!("downloads-press-again")).size(14).font(BOLD))
                        .padding([8, 14])
                        .style(style::danger)
                        .on_press_maybe(queue(game, Action::Uninstall).filter(|_| free))
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
        if game.source == Source::Local {
            more = more.push(self.local_buttons(game));
        }
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
#[expect(
    clippy::needless_pass_by_value,
    reason = "callers hand over a heading they just built"
)]
pub(super) fn card(heading: String, body: Element<'_, Message>) -> Element<'_, Message> {
    container(column![text(heading.to_uppercase()).size(14).font(BOLD), body].spacing(16))
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
        .into()
}

/// A card that could not be asked, with the way to ask again.
pub(super) fn failed_card<'a>(heading: String, id: &str, error: &str) -> Element<'a, Message> {
    card(
        heading,
        column![
            text(t!("card-failed", error = error))
                .size(13)
                .color(colors().dim),
            button(text(t!("try-again")).size(13))
                .padding([6, 12])
                .style(style::secondary)
                .on_press(Message::Retry(id.to_owned())),
        ]
        .spacing(10)
        .into(),
    )
}

pub(super) fn label_row(label: String, value: Element<'_, Message>) -> Element<'_, Message> {
    row![
        text(label).size(14).color(colors().dim).width(LABEL_WIDTH),
        container(value).width(Length::Fill),
    ]
    .spacing(8)
    .into()
}

/// The Categories sheet, opened for one game: the details card's button.
pub(super) fn change_categories(game: &Game) -> Message {
    Message::Categories(CategoriesMessage::Open(BTreeSet::from([game.id.clone()])))
}

pub(super) fn plain<'a>(value: String) -> Element<'a, Message> {
    text(value).size(14).into()
}

fn open_button<'a>(label: String, path: &Path) -> Element<'a, Message> {
    // Offered whether or not the folder is there: a `stat` here would run
    // on every redraw, and opening a missing one says so on the status line.
    button(text(label).size(13))
        .padding([4, 10])
        .style(style::secondary)
        .on_press(Message::Open(path.to_string_lossy().into_owned()))
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

/// Whether the More row has anything for the game.
fn has_more(game: &Game) -> bool {
    game.installed_by_zendo() || steam_app(game).is_some() || game.source == Source::Local
}

/// The page of the game on `PCGamingWiki`: the redirect for a Steam app id,
/// else a search for the title.
fn pcgamingwiki_url(app: Option<u32>, title: &str) -> String {
    if let Some(app) = app {
        return format!("https://www.pcgamingwiki.com/api/appid.php?appid={app}");
    }
    let search: String = title
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    format!("https://www.pcgamingwiki.com/w/index.php?search={search}")
}

/// Four tiles with the numbers worth seeing first: the time played, when it
/// was last played, the achievements unlocked and the share of them.
fn stat_tiles(game: &Game) -> Element<'_, Message> {
    let dash = || "—".to_owned();
    let tile = |label: String, value: String| {
        container(column![style::label(&label), text(value).size(22).font(BOLD),].spacing(SPACE_S))
            .padding(SPACE_L)
            .width(Length::FillPortion(1))
            .style(style::card)
    };
    let (unlocked, share) = game.achievements.map_or_else(
        || (dash(), dash()),
        |(unlocked, total)| {
            (
                format!("{unlocked} / {total}"),
                super::library::progress_of((unlocked, total))
                    .map_or_else(dash, |share| format!("{:.0}%", share * 100.0)),
            )
        },
    );
    row![
        tile(
            t!("fact-play-time"),
            if game.played.minutes > 0 {
                format::play_time(game.played.minutes)
            } else {
                dash()
            },
        ),
        tile(
            t!("fact-last-played"),
            game.played
                .last
                .map_or_else(dash, |last| format::ago(last, now())),
        ),
        tile(t!("achievements-title"), unlocked),
        tile(t!("stat-completion"), share),
    ]
    .spacing(SPACE_M)
    .into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_to_pcgamingwiki_by_app_id_or_by_title() {
        assert_eq!(
            pcgamingwiki_url(Some(620), "Portal 2"),
            "https://www.pcgamingwiki.com/api/appid.php?appid=620"
        );
        assert_eq!(
            pcgamingwiki_url(None, "Baldur's Gate 3 – GOTY"),
            "https://www.pcgamingwiki.com/w/index.php?search=Baldur%27s%20Gate%203%20%E2%80%93%20GOTY"
        );
    }

    #[test]
    fn the_page_sections_open_and_close() {
        let (mut zendo, _) = Zendo::boot();
        zendo.persist = false;
        let _ = zendo.update(Message::GameView(GameViewMessage::ToggleMore));
        assert!(zendo.game_view.more);
        let _ = zendo.update(Message::GameView(GameViewMessage::ToggleMore));
        assert!(!zendo.game_view.more);
        let _ = zendo.update(Message::GameView(GameViewMessage::ToggleTechnical));
        assert!(zendo.game_view.technical);
        let _ = zendo.update(Message::GameView(GameViewMessage::MoreAchievements));
        assert_eq!(
            zendo.game_view.achievements_more,
            crate::app::achievements::SHOWN
        );
        let _ = zendo.update(Message::GameView(GameViewMessage::MoreChangelog));
        assert_eq!(
            zendo.game_view.changelog_more,
            crate::app::gog_changelog::MORE
        );
    }
}
