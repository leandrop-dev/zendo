//! Covers as Slint images. A card asks for its cover; when it is not kept,
//! a worker thread reads and decodes it and the window is told when it is
//! ready, so a scroll never waits on a decode (a 1,200-pixel cover took tens
//! of milliseconds, and a screen of them stalled the grid to 2 fps on the
//! UI thread: `docs/history.md`, item 48).
//!
//! The cache is bounded: a decoded cover is 4 bytes a pixel, and one per game,
//! kept for good, was 3.5 GB in the spike (item 45). Covers are made smaller
//! than a card needs before they are kept. A `slint::Image` is not `Send`,
//! so the workers hand over the pixels and the window makes the image.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// How many decoded covers stay: a few screens of cards.
const KEPT: usize = 150;

/// The widest a cover is kept: twice a card's, for a screen at double scale.
/// Some stores' covers are 1080 or 1200 pixels wide, which decoded is 6 MB or
/// more each, for a card 180 pixels wide.
const MAX_WIDTH: u32 = 360;

const WORKERS: usize = 2;

/// How many covers wait for a worker at most. The newest go first: a fast
/// scroll asks for rows it has already left, and those are let go.
const WAITING: usize = 96;

/// What a card's cover is, now.
#[derive(Debug, Clone)]
pub enum Cover {
    Ready(Image),
    /// Being read: the card shows an empty cover until it arrives.
    Loading,
    /// The file could not be read: the card shows its title instead.
    Missing,
}

/// The pixels of a cover a worker decoded, or `None` when it could not.
pub type Decoded = Option<SharedPixelBuffer<Rgba8Pixel>>;

#[derive(Default)]
struct Queue {
    stack: Mutex<Vec<PathBuf>>,
    ready: Condvar,
}

impl Queue {
    /// Puts a cover last, to go first. Returns the oldest one let go when
    /// too many wait.
    fn push(&self, path: PathBuf) -> Option<PathBuf> {
        let mut stack = self
            .stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stack.push(path);
        let dropped = (stack.len() > WAITING).then(|| stack.remove(0));
        self.ready.notify_one();
        dropped
    }

    /// The newest cover waiting, when there is one; waits for one otherwise.
    fn next(&self) -> PathBuf {
        let mut stack = self
            .stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if let Some(path) = stack.pop() {
                return path;
            }
            stack = self
                .ready
                .wait(stack)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

struct Slot {
    /// `None` when the file could not be read: it is not tried again at
    /// every redraw.
    image: Option<Image>,
    used: u64,
}

pub struct Covers {
    slots: HashMap<PathBuf, Slot>,
    /// Asked for and not yet delivered.
    pending: HashSet<PathBuf>,
    queue: Arc<Queue>,
    /// Counts the lookups, to tell the least recently used.
    tick: u64,
}

impl Covers {
    /// Starts the workers. `ready` is called on a worker's thread with each
    /// cover's pixels; it must hand them to the window's thread, which gives
    /// them to [`Covers::deliver`].
    pub fn new(ready: impl Fn(PathBuf, Decoded) + Send + Sync + 'static) -> Self {
        let queue = Arc::new(Queue::default());
        let ready = Arc::new(ready);
        for _ in 0..WORKERS {
            let (queue, ready) = (Arc::clone(&queue), Arc::clone(&ready));
            thread::spawn(move || {
                loop {
                    let path = queue.next();
                    let pixels = decode(&path);
                    ready(path, pixels);
                }
            });
        }
        Self {
            slots: HashMap::new(),
            pending: HashSet::new(),
            queue,
            tick: 0,
        }
    }

    /// The cover at `path`: kept, or asked of a worker.
    pub fn get(&mut self, path: &Path) -> Cover {
        self.tick += 1;
        if let Some(slot) = self.slots.get_mut(path) {
            slot.used = self.tick;
            return slot.image.clone().map_or(Cover::Missing, Cover::Ready);
        }
        if self.pending.insert(path.to_path_buf())
            && let Some(dropped) = self.queue.push(path.to_path_buf())
        {
            self.pending.remove(&dropped);
        }
        Cover::Loading
    }

    /// A worker's result, on the window's thread. Keeps the cover, and says
    /// whether a card may now show something different.
    pub fn deliver(&mut self, path: PathBuf, pixels: Decoded) -> bool {
        if !self.pending.remove(&path) {
            return false;
        }
        if self.slots.len() >= KEPT {
            self.forget_oldest();
        }
        self.tick += 1;
        self.slots.insert(
            path,
            Slot {
                image: pixels.map(Image::from_rgba8),
                used: self.tick,
            },
        );
        true
    }

    fn forget_oldest(&mut self) {
        // A scan of at most `KEPT` slots, once per cover newly kept.
        if let Some(oldest) = self
            .slots
            .iter()
            .min_by_key(|(_, slot)| slot.used)
            .map(|(path, _)| path.clone())
        {
            self.slots.remove(&oldest);
        }
    }

    #[cfg(test)]
    fn kept(&self) -> usize {
        self.slots.len()
    }
}

/// Reads and decodes a cover, made smaller when it is bigger than a card
/// needs.
fn decode(path: &Path) -> Decoded {
    let mut decoded = image::open(path).ok()?;
    if decoded.width() > MAX_WIDTH {
        decoded = decoded.resize(MAX_WIDTH, u32::MAX, image::imageops::FilterType::Triangle);
    }
    let pixels = decoded.into_rgba8();
    Some(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    fn covers() -> (Covers, mpsc::Receiver<(PathBuf, Decoded)>) {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        let covers = Covers::new(move |path, pixels| {
            let _ = sender
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .send((path, pixels));
        });
        (covers, receiver)
    }

    fn picture(name: &str, width: u32, height: u32) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("zendo-covers-{}-{name}.png", std::process::id()));
        image::RgbaImage::new(width, height)
            .save(&path)
            .expect("the picture is written");
        path
    }

    #[test]
    fn a_cover_is_read_on_a_worker_and_made_smaller_when_big() {
        let (mut covers, receiver) = covers();
        let big = picture("big", 1080, 1440);
        assert!(matches!(covers.get(&big), Cover::Loading));
        // Asked again while it waits: still one request.
        assert!(matches!(covers.get(&big), Cover::Loading));
        let (path, pixels) = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("a result");
        assert_eq!(path, big);
        let pixels = pixels.expect("it decoded");
        assert_eq!((pixels.width(), pixels.height()), (MAX_WIDTH, 480));
        assert!(receiver.recv_timeout(Duration::from_millis(200)).is_err());
        assert!(covers.deliver(path, Some(pixels)));
        assert!(matches!(covers.get(&big), Cover::Ready(_)));
        let _ = std::fs::remove_file(big);
    }

    #[test]
    fn a_small_cover_keeps_its_size_and_a_missing_file_is_remembered() {
        let (mut covers, receiver) = covers();
        let small = picture("small", 300, 450);
        let missing = PathBuf::from("/nonexistent/cover.jpg");
        let _ = covers.get(&small);
        let _ = covers.get(&missing);
        for _ in 0..2 {
            let (path, pixels) = receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("a result");
            if path == small {
                let pixels = pixels.as_ref().expect("it decoded");
                assert_eq!((pixels.width(), pixels.height()), (300, 450));
            } else {
                assert!(pixels.is_none());
            }
            covers.deliver(path, pixels);
        }
        assert!(matches!(covers.get(&missing), Cover::Missing));
        let _ = std::fs::remove_file(small);
    }

    #[test]
    fn the_cache_stays_bounded() {
        let (mut covers, _receiver) = covers();
        for n in 0..(KEPT + 50) {
            let path = PathBuf::from(format!("/nonexistent/{n}.jpg"));
            let _ = covers.get(&path);
            covers.deliver(path, None);
        }
        assert_eq!(covers.kept(), KEPT);
    }

    #[test]
    fn a_fast_scroll_lets_the_oldest_requests_go() {
        let queue = Queue::default();
        let mut dropped = None;
        for n in 0..=WAITING {
            dropped = queue.push(PathBuf::from(format!("{n}")));
        }
        assert_eq!(dropped, Some(PathBuf::from("0")));
        assert_eq!(queue.next(), PathBuf::from(format!("{WAITING}")));
    }
}
