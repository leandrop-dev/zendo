//! The window's icons: Lucide (ISC licence, `crates/zendo/icons/LICENSE`),
//! built into the binary as SVG and drawn in the colour the caller gives.
//! To add one, put its SVG from <https://lucide.dev> in `crates/zendo/icons/`
//! and a variant here.

use iced::widget::{Svg, svg};
use iced::{Color, Theme};

/// One of the icons in `crates/zendo/icons/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Download,
    Plus,
    Refresh,
    Settings,
}

impl Icon {
    #[cfg(test)]
    const ALL: [Self; 4] = [Self::Download, Self::Plus, Self::Refresh, Self::Settings];

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Download => include_bytes!("../../icons/download.svg"),
            Self::Plus => include_bytes!("../../icons/plus.svg"),
            Self::Refresh => include_bytes!("../../icons/refresh-cw.svg"),
            Self::Settings => include_bytes!("../../icons/settings.svg"),
        }
    }
}

/// The icon, `size` pixels square, in `color`. The SVGs stroke in
/// `currentColor`, so the colour tints every drawn pixel.
pub fn icon<'a>(icon: Icon, size: f32, color: Color) -> Svg<'a> {
    svg(svg::Handle::from_memory(icon.bytes()))
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| svg::Style { color: Some(color) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_an_svg() {
        for icon in Icon::ALL {
            let text = std::str::from_utf8(icon.bytes()).expect("an SVG is text");
            assert!(text.contains("<svg"), "{icon:?}");
        }
    }
}
