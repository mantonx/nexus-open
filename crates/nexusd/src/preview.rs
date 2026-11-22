//! `nexusd preview`: the demo page, the legibility card or a folder of
//! mockup frames on the real strip (Linux only for now).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use nexus_core::demo::system_page;
use nexus_core::fonts::FontChoice;
use nexus_core::gesture::{Gesture, Recognizer};
use nexus_core::theme::Theme;
use nexus_core::widget::Renderer;
use nexus_device::Input;
use nexus_device::hidraw::Nexus;
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};

use crate::slides::Slide;

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(100);

/// What `preview` shows.
pub enum Show {
    /// The demo System page, in this font; `--cycle` moves through the fonts.
    Demo(FontChoice),
    /// Frames stepped through by tapping (right half forward, left half
    /// back) or, with `--cycle`, by time.
    Slides(Vec<Slide>),
}

pub fn preview(show: Show, cycle: Option<Duration>, touch: bool) -> Result<(), DeviceError> {
    let mut deck = Deck::new(show);
    let (tx, rx) = mpsc::channel();

    let mut device = Nexus::open()?;
    device.spawn_reader(tx.clone())?;
    println!("opened {}", device.path().display());
    deck.show(&mut device, true)?;

    let mut calibration = TouchCalibration::default();
    let mut gestures = Recognizer::new();
    let origin = Instant::now();
    let mut last_switch = Instant::now();
    loop {
        let wait = cycle.map_or(Duration::from_secs(3600), |c| {
            c.saturating_sub(last_switch.elapsed())
        });
        match rx.recv_timeout(wait) {
            Ok(Input::Report(report, read_at)) => {
                let Some(t) = parse_touch(&report) else {
                    continue;
                };
                let x = calibration.to_panel_x(t.x);
                if touch {
                    println!(
                        "touch {} x={x:>3} (raw {})",
                        if t.down { "down" } else { "up  " },
                        t.x
                    );
                }
                let at = read_at.duration_since(origin).as_secs_f64();
                if let Some(Gesture::Tap { x }) = gestures.update(t.down, f32::from(x), at)
                    && deck.tap(x)
                {
                    deck.show(&mut device, true)?;
                    last_switch = Instant::now();
                }
            }
            Ok(Input::Gone(e)) => {
                println!("device gone ({e}); waiting for it to come back");
                let lost = Instant::now();
                device = wait_for_device();
                device.spawn_reader(tx.clone())?;
                deck.show(&mut device, false)?;
                println!("recovered in {} ms", lost.elapsed().as_millis());
            }
            Err(RecvTimeoutError::Timeout) => {
                deck.step(true);
                deck.show(&mut device, true)?;
                last_switch = Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => unreachable!("main holds a sender"),
        }
    }
}

/// The frame on the strip and how to move to the next one.
enum Deck {
    Demo { font: FontChoice, frame: Vec<u8> },
    Slides { slides: Vec<Slide>, at: usize },
}

impl Deck {
    fn new(show: Show) -> Self {
        match show {
            Show::Demo(font) => Deck::Demo {
                font,
                frame: render(font),
            },
            Show::Slides(slides) => {
                assert!(!slides.is_empty(), "caller checks there is a frame to show");
                Deck::Slides { slides, at: 0 }
            }
        }
    }

    /// Moves one frame forward or back, wrapping at the ends.
    fn step(&mut self, forward: bool) {
        match self {
            Deck::Demo { font, frame } => {
                // The demo only moves forward, by time.
                *font = next(*font);
                *frame = render(*font);
            }
            Deck::Slides { slides, at } => *at = stepped(*at, slides.len(), forward),
        }
    }

    /// Handles a tap at panel `x`; returns whether the frame changed. Taps
    /// do nothing on the demo page.
    fn tap(&mut self, x: f32) -> bool {
        match self {
            Deck::Demo { .. } => false,
            Deck::Slides { .. } => {
                self.step(x >= (nexus_core::WIDTH / 2) as f32);
                true
            }
        }
    }

    /// Sends the current frame and says what it is. `notes` also prints the
    /// slide's notes, which a recovery after replug does not repeat.
    fn show(&self, device: &mut Nexus, notes: bool) -> Result<(), DeviceError> {
        let start = Instant::now();
        match self {
            Deck::Demo { font, frame } => {
                device.send_frame(frame)?;
                println!(
                    "showing {:<22} frame sent in {:.1} ms",
                    font.display_name(),
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
            Deck::Slides { slides, at } => {
                let slide = &slides[*at];
                device.send_frame(&slide.rgba)?;
                println!(
                    "showing frame {}/{}: {}  (sent in {:.1} ms)",
                    at + 1,
                    slides.len(),
                    slide.name,
                    start.elapsed().as_secs_f64() * 1000.0
                );
                if notes {
                    for line in &slide.notes {
                        println!("    {line}");
                    }
                }
            }
        }
        Ok(())
    }
}

/// The index after `at` (or before it, if not `forward`) among `len`,
/// wrapping around.
fn stepped(at: usize, len: usize, forward: bool) -> usize {
    if forward {
        (at + 1) % len
    } else {
        (at + len - 1) % len
    }
}

fn render(font: FontChoice) -> Vec<u8> {
    let mut renderer = Renderer::new(Theme::default(), font.load());
    renderer.render_page(&system_page(), None).rgba().to_vec()
}

/// Blocks until the Nexus can be opened again.
fn wait_for_device() -> Nexus {
    loop {
        match Nexus::open() {
            Ok(device) => return device,
            // Right after replug the node can exist before udev has applied
            // the ACL, so permission errors are retried too.
            Err(DeviceError::NotFound | DeviceError::PermissionDenied(_)) => {
                thread::sleep(RECONNECT_POLL);
            }
            Err(e) => {
                eprintln!("reconnect: {e}");
                thread::sleep(RECONNECT_POLL);
            }
        }
    }
}

fn next(font: FontChoice) -> FontChoice {
    let all = FontChoice::ALL;
    let i = all.iter().position(|&f| f == font).unwrap_or(0);
    all[(i + 1) % all.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wraps_both_ways() {
        assert_eq!(stepped(0, 3, true), 1);
        assert_eq!(stepped(2, 3, true), 0);
        assert_eq!(stepped(0, 3, false), 2);
        assert_eq!(stepped(0, 1, false), 0);
    }

    #[test]
    fn taps_go_forward_on_the_right_half_and_back_on_the_left() {
        let slide = |n: &str| Slide {
            name: n.into(),
            notes: Vec::new(),
            rgba: Vec::new(),
        };
        let mut deck = Deck::new(Show::Slides(vec![slide("a"), slide("b"), slide("c")]));
        let at = |d: &Deck| match d {
            Deck::Slides { at, .. } => *at,
            Deck::Demo { .. } => unreachable!(),
        };
        assert!(deck.tap(500.0));
        assert_eq!(at(&deck), 1);
        assert!(deck.tap(320.0));
        assert_eq!(at(&deck), 2);
        assert!(deck.tap(319.0));
        assert_eq!(at(&deck), 1);
        assert!(deck.tap(0.0));
        assert!(deck.tap(0.0));
        assert_eq!(at(&deck), 2, "back from the first wraps to the last");
    }
}
