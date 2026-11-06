//! Controllers, through gilrs, as a stream of the same moves the keyboard
//! makes. gilrs blocks while it waits for events, so it runs on a thread of
//! its own and hands them over a channel.

use std::thread;
use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gilrs};
use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};

/// What a controller asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pad {
    Left,
    Right,
    Up,
    Down,
    Activate,
    Back,
    Sort,
    Hide,
    ShowHidden,
    ToggleFullscreen,
}

/// How far a stick must lean to count as a press, and how close to the
/// centre it must come back before it can press again.
const PRESS: f32 = 0.6;
const RELEASE: f32 = 0.3;

pub fn events() -> impl Stream<Item = Pad> {
    iced::stream::channel(32, async |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        thread::spawn(move || poll(&sender));
        while let Some(pad) = receiver.next().await {
            if output.send(pad).await.is_err() {
                break;
            }
        }
    })
}

/// Reads controllers until the window side of the channel goes away. With no
/// access to input devices gilrs fails to start, and the window simply works
/// without controllers.
fn poll(sender: &mpsc::UnboundedSender<Pad>) {
    let Ok(mut gilrs) = Gilrs::new() else {
        return;
    };
    let mut stick = Stick::default();
    while !sender.is_closed() {
        let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(500))) else {
            continue;
        };
        let pad = match event.event {
            EventType::ButtonPressed(button, _) => button_pad(button),
            EventType::AxisChanged(axis, value, _) => stick.lean(axis, value),
            _ => None,
        };
        if let Some(pad) = pad {
            let _ = sender.unbounded_send(pad);
        }
    }
}

fn button_pad(button: Button) -> Option<Pad> {
    Some(match button {
        Button::DPadLeft => Pad::Left,
        Button::DPadRight => Pad::Right,
        Button::DPadUp => Pad::Up,
        Button::DPadDown => Pad::Down,
        Button::South => Pad::Activate,
        Button::East => Pad::Back,
        Button::North => Pad::Sort,
        Button::West => Pad::Hide,
        Button::Start => Pad::ShowHidden,
        Button::Select => Pad::ToggleFullscreen,
        _ => return None,
    })
}

/// The left stick as four buttons: one move per lean.
#[derive(Default)]
struct Stick {
    x_leaning: bool,
    y_leaning: bool,
}

impl Stick {
    fn lean(&mut self, axis: Axis, value: f32) -> Option<Pad> {
        let (leaning, negative, positive) = match axis {
            Axis::LeftStickX => (&mut self.x_leaning, Pad::Left, Pad::Right),
            // gilrs reports up as positive.
            Axis::LeftStickY => (&mut self.y_leaning, Pad::Down, Pad::Up),
            _ => return None,
        };
        if value.abs() < RELEASE {
            *leaning = false;
            None
        } else if value.abs() > PRESS && !*leaning {
            *leaning = true;
            Some(if value < 0.0 { negative } else { positive })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stick_moves_once_per_lean() {
        let mut stick = Stick::default();
        assert_eq!(stick.lean(Axis::LeftStickX, 0.7), Some(Pad::Right));
        assert_eq!(stick.lean(Axis::LeftStickX, 0.9), None);
        assert_eq!(stick.lean(Axis::LeftStickX, 0.1), None);
        assert_eq!(stick.lean(Axis::LeftStickY, 0.8), Some(Pad::Up));
        assert_eq!(stick.lean(Axis::LeftStickX, -0.8), Some(Pad::Left));
    }
}
