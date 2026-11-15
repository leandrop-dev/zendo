//! gamescope's part of the launch options sheet, under its switch: how its
//! window shows, the size it shows at and the game's, how it scales, the
//! game's frames a second, and more of its options as typed.

use iced::widget::{column, pick_list, row, text, text_input};
use iced::{Alignment, Element};
use zendo_core::launch_options::{Filter, LaunchOptions};
pub use zendo_shell::launch_options::gamescope::Setting;
use zendo_shell::launch_options::gamescope::{self as shared, WindowKind};

use super::{Field, Pick, Sheet, SheetMessage, field, size_input};
use crate::app::Message;
use crate::app::style::{self, colors};
use crate::i18n::t;

/// gamescope's settings, under its switch.
pub fn view(sheet: &Sheet) -> Element<'_, Message> {
    let options = &sheet.options;
    let fields = &sheet.gamescope;
    let set = |setting| Message::Sheet(SheetMessage::GamescopeSet(setting));
    let mut sizes = column![
        size_row(
            t!("options-output-size"),
            (&sheet.width, SheetMessage::Width),
            (&sheet.height, SheetMessage::Height),
        ),
        size_row(
            t!("options-game-size"),
            (&fields.game_width, |value| {
                SheetMessage::GamescopeSet(Setting::GameWidth(value))
            }),
            (&fields.game_height, |value| {
                SheetMessage::GamescopeSet(Setting::GameHeight(value))
            }),
        ),
        text(t!("options-sizes-about")).size(13).color(colors().dim),
    ]
    .spacing(style::SPACE_S);
    if let Some((Field::Gamescope, error)) = &sheet.error {
        sizes = sizes.push(text(error).size(13).color(colors().bad));
    }
    let scaling = scaling(options);
    column![
        labelled(
            t!("options-gamescope-shows"),
            pick_list(
                WindowKind::ALL,
                Some(WindowKind::of(options)),
                move |kind| set(Setting::Window(kind)),
            )
            .text_size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::pick_list)
            .menu_style(style::menu),
        ),
        sizes,
        scaling,
        labelled(
            t!("options-gamescope-fps"),
            text_input(&t!("options-gamescope-fps-empty"), &fields.fps)
                .on_input(move |value| set(Setting::Fps(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([style::SPACE_S, style::SPACE_M])
                .width(200)
                .style(style::input),
        ),
        field(
            t!("options-gamescope-more"),
            t!("options-gamescope-more-about"),
            text_input("--adaptive-sync", &options.gamescope_more)
                .on_input(move |value| set(Setting::More(value)))
                .on_submit(Message::Sheet(SheetMessage::Save))
                .size(14)
                .padding([style::SPACE_S, style::SPACE_M])
                .style(style::input),
        ),
    ]
    .spacing(style::SPACE_L)
    .padding([0, 20])
    .into()
}

/// The fit, the scaling and its sharpening.
fn scaling(options: &LaunchOptions) -> Element<'_, Message> {
    let set = |setting| Message::Sheet(SheetMessage::GamescopeSet(setting));
    let scalers = shared::scalers();
    let filters = shared::filters();
    let mut scaling = row![
        labelled(
            t!("options-scaler"),
            pick_list(
                scalers.clone(),
                Pick::chosen(&scalers, options.gamescope_scaler),
                move |pick| set(Setting::Scaler(pick)),
            )
            .text_size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::pick_list)
            .menu_style(style::menu),
        ),
        labelled(
            t!("options-filter"),
            pick_list(
                filters.clone(),
                Pick::chosen(&filters, options.gamescope_filter),
                move |pick| set(Setting::Filter(pick)),
            )
            .text_size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::pick_list)
            .menu_style(style::menu),
        ),
    ]
    .spacing(style::SPACE_L);
    if options.gamescope_filter.is_some_and(Filter::sharpens) {
        let sharpness = shared::sharpness();
        scaling = scaling.push(labelled(
            t!("options-fsr-sharpening"),
            pick_list(
                sharpness.clone(),
                Pick::chosen(&sharpness, options.gamescope_sharpness),
                move |pick| set(Setting::Sharpness(pick)),
            )
            .text_size(14)
            .padding([style::SPACE_S, style::SPACE_M])
            .style(style::pick_list)
            .menu_style(style::menu),
        ));
    }
    scaling.into()
}

/// A setting with its name before it.
fn labelled<'a>(label: String, input: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![text(label).size(14).color(colors().dim), input.into()]
        .spacing(style::SPACE_M)
        .align_y(Alignment::Center)
        .into()
}

/// A width and a height.
fn size_row<'a>(
    label: String,
    (width, on_width): (&'a str, fn(String) -> SheetMessage),
    (height, on_height): (&'a str, fn(String) -> SheetMessage),
) -> Element<'a, Message> {
    row![
        text(label).size(14).color(colors().dim).width(110),
        size_input(&t!("options-width"), width, on_width),
        text("×").size(14).color(colors().dim),
        size_input(&t!("options-height"), height, on_height),
    ]
    .spacing(style::SPACE_M)
    .align_y(Alignment::Center)
    .into()
}
