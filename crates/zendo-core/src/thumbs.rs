//! Downscaled copies of the big images, so the window never decodes them on
//! the thread that draws.
//!
//! Heroes (the wide image behind a game page's header) are 1920 pixels wide
//! or more, often 3840 and up to 7184, and are shown at a fraction of their
//! opacity: decoding one took 12 ms on average and up to 150 ms, in
//! a release build (checked 2026-11-13 on the 598 heroes in one library).
//! Covers are already small (300 by 450 for most), so they get no copy.
//!
//! A copy is kept in `$XDG_CACHE_HOME/zendo/thumbs/`, named from the source's
//! path, size and modification time, so a changed image makes a new copy and
//! the old one is left to be cleared with the cache.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Context;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

use crate::paths;

/// How wide a hero's copy is: twice the window's usual header, so the
/// backdrop is not soft at 22 % opacity, a quarter of a 1920 hero's pixels.
pub const BACKDROP_WIDTH: u32 = 960;

/// The JPEG quality of a copy.
const QUALITY: u8 = 85;

/// A copy with transparency stays a PNG; one without is a JPEG.
const EXTENSIONS: [&str; 2] = ["jpg", "png"];

/// Where the copies are kept.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    paths::cache_home().map(|dir| dir.join("zendo/thumbs"))
}

/// The copy of a hero, when one was made: the file to draw instead of the
/// source. The source itself when it is small enough already.
#[must_use]
pub fn backdrop(source: &Path) -> Option<PathBuf> {
    backdrop_in(&dir()?, source)
}

/// [`backdrop`], making the copy when there is none. Slow for a big image:
/// call it off the thread that draws.
///
/// # Errors
///
/// When the image cannot be read or decoded, or the copy cannot be written.
pub fn make_backdrop(source: &Path) -> anyhow::Result<PathBuf> {
    let dir = dir().context("no home folder to keep thumbnails in")?;
    make_backdrop_in(&dir, source)
}

fn backdrop_in(dir: &Path, source: &Path) -> Option<PathBuf> {
    let stem = stem(source)?;
    EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
}

fn make_backdrop_in(dir: &Path, source: &Path) -> anyhow::Result<PathBuf> {
    if let Some(done) = backdrop_in(dir, source) {
        return Ok(done);
    }
    let stem = stem(source).with_context(|| format!("could not read {}", source.display()))?;
    let image = image::open(source).with_context(|| format!("could not decode {}", source.display()))?;
    // Already small: nothing to gain, so the source is drawn as it is.
    if image.width() <= BACKDROP_WIDTH {
        return Ok(source.to_path_buf());
    }
    // Fit inside the width; the height follows the image's shape.
    let small = image.resize(BACKDROP_WIDTH, u32::MAX, FilterType::Triangle);
    let (ext, bytes) = if small.color().has_alpha() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        small
            .write_to(&mut bytes, image::ImageFormat::Png)
            .context("could not encode the thumbnail")?;
        ("png", bytes.into_inner())
    } else {
        let mut bytes = Vec::new();
        small
            .to_rgb8()
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, QUALITY))
            .context("could not encode the thumbnail")?;
        ("jpg", bytes)
    };
    let path = dir.join(format!("{stem}.{ext}"));
    paths::write_atomic(&path, &bytes)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(path)
}

/// The copy's name without the extension: the source's path, size and
/// modification time and the width, hashed. `None` when the source is gone.
fn stem(source: &Path) -> Option<String> {
    let meta = std::fs::metadata(source).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    modified.hash(&mut hasher);
    BACKDROP_WIDTH.hash(&mut hasher);
    Some(format!("backdrop-{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zendo-thumbs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_wide_hero_gets_a_narrow_copy_kept_for_next_time() {
        let dir = scratch("wide");
        let source = dir.join("hero.png");
        RgbImage::from_pixel(1920, 620, Rgb([10, 90, 200]))
            .save(&source)
            .unwrap();
        let thumbs = dir.join("thumbs");
        assert_eq!(backdrop_in(&thumbs, &source), None, "none made yet");
        let made = make_backdrop_in(&thumbs, &source).unwrap();
        assert_ne!(made, source);
        assert_eq!(made.extension().and_then(|e| e.to_str()), Some("jpg"));
        let (width, height) = image::image_dimensions(&made).unwrap();
        assert_eq!((width, height), (BACKDROP_WIDTH, 310));
        assert_eq!(backdrop_in(&thumbs, &source), Some(made.clone()));
        assert_eq!(make_backdrop_in(&thumbs, &source).unwrap(), made, "reused");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hero_with_transparency_keeps_it() {
        let dir = scratch("alpha");
        let source = dir.join("hero.png");
        RgbaImage::from_pixel(1920, 620, Rgba([10, 90, 200, 40]))
            .save(&source)
            .unwrap();
        let made = make_backdrop_in(&dir.join("thumbs"), &source).unwrap();
        assert_eq!(made.extension().and_then(|e| e.to_str()), Some("png"));
        let thumb = image::open(&made).unwrap().to_rgba8();
        assert_eq!(thumb.get_pixel(0, 0)[3], 40);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_small_image_is_drawn_as_it_is() {
        let dir = scratch("small");
        let source = dir.join("hero.png");
        RgbImage::from_pixel(800, 300, Rgb([1, 2, 3]))
            .save(&source)
            .unwrap();
        let thumbs = dir.join("thumbs");
        assert_eq!(make_backdrop_in(&thumbs, &source).unwrap(), source);
        assert!(!thumbs.exists(), "nothing was written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_source_makes_a_new_copy_and_a_missing_or_broken_one_fails() {
        let dir = scratch("changed");
        let source = dir.join("hero.png");
        RgbImage::from_pixel(1920, 620, Rgb([0, 0, 0]))
            .save(&source)
            .unwrap();
        let thumbs = dir.join("thumbs");
        let first = make_backdrop_in(&thumbs, &source).unwrap();
        // A different size is a different image.
        RgbImage::from_pixel(2560, 670, Rgb([0, 0, 0]))
            .save(&source)
            .unwrap();
        let second = make_backdrop_in(&thumbs, &source).unwrap();
        assert_ne!(first, second);

        assert!(make_backdrop_in(&thumbs, &dir.join("gone.png")).is_err());
        let broken = dir.join("broken.png");
        std::fs::write(&broken, b"not an image").unwrap();
        assert!(make_backdrop_in(&thumbs, &broken).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
