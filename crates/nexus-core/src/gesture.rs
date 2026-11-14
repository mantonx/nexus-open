//! Turns a stream of touch samples into gestures: drag, release, tap and
//! long press.
//!
//! Ported from the Go app's `internal/touch` with its thresholds unchanged;
//! they were tuned on the hardware. Pure: each sample carries its own time,
//! so tests replay exact traces.

/// Panel width in pixels, as f32 for gesture maths.
const WIDTH: f32 = crate::WIDTH as f32;

/// Held at least this long without swiping, a press is a long press.
const LONG_PRESS: f64 = 0.5;
/// Touches starting this soon after a swipe ends are ignored: a fast flick
/// lifts the finger while reports are still in flight, and the tail would
/// otherwise start a new swipe.
const COOLDOWN: f64 = 0.25;

/// What a run of samples means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    /// The finger moved while swiping. `dx` is from the press point;
    /// negative is leftward.
    Drag { dx: f32 },
    /// A swipe ended. `velocity` is the finger's speed at lift in px/s,
    /// signed like `dx`.
    Release { dx: f32, velocity: f32 },
    /// A short press without swiping.
    Tap { x: f32 },
    /// A press held for `LONG_PRESS` without swiping.
    LongPress { x: f32 },
}

#[derive(Debug, Default)]
pub struct Recognizer {
    filter: OneEuro,
    velocity: VelocityWindow,
    last_t: Option<f64>,
    /// Set while a finger is down.
    press: Option<Press>,
    swipe_ended_at: Option<f64>,
}

#[derive(Debug)]
struct Press {
    start_x: f32,
    start_t: f64,
    /// Last filtered position while down. The device reports x = 0 on lift,
    /// so the release uses this instead.
    last_x: f32,
    swiping: bool,
}

impl Recognizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one sample: whether a finger is down, its panel x (0–639) and
    /// the time in seconds from any fixed origin.
    pub fn update(&mut self, down: bool, x: f32, t: f64) -> Option<Gesture> {
        let dt = self.last_t.map_or(1.0 / 500.0, |last| {
            let dt = t - last;
            if dt > 0.0 { dt } else { 1.0 / 500.0 }
        });
        self.last_t = Some(t);

        match (down, self.press.as_mut()) {
            (true, None) => {
                if self.swipe_ended_at.is_some_and(|end| t - end < COOLDOWN) {
                    return None;
                }
                self.filter = OneEuro::default();
                let x = self.filter.filter(x, dt);
                self.velocity = VelocityWindow::default();
                self.velocity.push(x, t);
                self.press = Some(Press {
                    start_x: x,
                    start_t: t,
                    last_x: x,
                    swiping: false,
                });
                None
            }
            (true, Some(press)) => {
                let x = self.filter.filter(x, dt).round();
                press.last_x = x;
                let dx = x - press.start_x;
                if !press.swiping && dx.abs() >= swipe_threshold(self.filter.speed()) {
                    press.swiping = true;
                }
                if press.swiping {
                    self.velocity.push(x, t);
                    return Some(Gesture::Drag { dx });
                }
                None
            }
            (false, Some(_)) => {
                let press = self.press.take()?;
                let dx = press.last_x - press.start_x;
                if press.swiping {
                    self.swipe_ended_at = Some(t);
                    Some(Gesture::Release {
                        dx,
                        velocity: self.velocity.velocity(),
                    })
                } else if t - press.start_t >= LONG_PRESS {
                    Some(Gesture::LongPress { x: press.last_x })
                } else {
                    Some(Gesture::Tap { x: press.last_x })
                }
            }
            (false, None) => None,
        }
    }

    /// True while a finger is on the strip.
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }
}

/// Distance before a press becomes a swipe: 9% of the width for a still
/// finger, down to 7% for a moving one, so a quick swipe starts sooner and a
/// tap's wobble never does.
fn swipe_threshold(speed: f32) -> f32 {
    let frac = (0.09 - (speed / 1200.0).min(0.04)).max(0.07);
    (frac * WIDTH).round()
}

/// Whether a released swipe changes page. Ported from the Go app's
/// `shouldCommitSwipe`. `progress` is the distance as a fraction of the
/// width; `speed` is px/s in the swipe's direction (negative if the finger
/// was moving back when it lifted).
pub fn should_commit(progress: f32, speed: f32) -> bool {
    const AUTO_COMMIT: f32 = 0.40;
    const AUTO_CANCEL: f32 = 0.05;
    const FLICK: (f32, f32) = (1200.0, 0.05);
    const FAST_FLICK: (f32, f32) = (800.0, 0.07);
    const MEDIUM_SPEED: f32 = 300.0;
    const SLOW_DRAG: f32 = 0.20;
    const STANDARD: f32 = 0.15;

    // New: a finger flicked back against the swipe means "never mind",
    // however far it went. This is what makes a swipe reversible.
    if speed <= -MEDIUM_SPEED {
        return false;
    }
    if progress >= AUTO_COMMIT {
        return true;
    }
    if progress < AUTO_CANCEL {
        return false;
    }
    if speed >= FLICK.0 && progress >= FLICK.1 {
        return true;
    }
    if speed >= FAST_FLICK.0 && progress >= FAST_FLICK.1 {
        return true;
    }
    if speed < MEDIUM_SPEED {
        return progress >= SLOW_DRAG;
    }
    progress >= STANDARD
}

/// One-Euro filter: smooths jitter when the finger is slow, follows closely
/// when it is fast. Parameters from the Go app.
#[derive(Debug)]
struct OneEuro {
    min_cutoff: f64,
    beta: f64,
    d_cutoff: f64,
    prev: Option<(f64, f64)>, // (x, dx)
}

impl Default for OneEuro {
    fn default() -> Self {
        Self {
            min_cutoff: 1.0,
            beta: 0.007,
            d_cutoff: 1.0,
            prev: None,
        }
    }
}

impl OneEuro {
    fn alpha(cutoff: f64, dt: f64) -> f64 {
        let tau = 1.0 / (std::f64::consts::TAU * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    fn filter(&mut self, x: f32, dt: f64) -> f32 {
        let x = f64::from(x);
        let Some((prev_x, prev_dx)) = self.prev else {
            self.prev = Some((x, 0.0));
            return x as f32;
        };
        let dx = (x - prev_x) / dt;
        let a_d = Self::alpha(self.d_cutoff, dt);
        let dx_hat = a_d * dx + (1.0 - a_d) * prev_dx;
        let cutoff = self.min_cutoff + self.beta * dx_hat.abs();
        let a = Self::alpha(cutoff, dt);
        let x_hat = a * x + (1.0 - a) * prev_x;
        self.prev = Some((x_hat, dx_hat));
        x_hat as f32
    }

    /// Smoothed speed in px/s.
    fn speed(&self) -> f32 {
        self.prev.map_or(0.0, |(_, dx)| dx.abs() as f32)
    }
}

/// The last few positions, for the speed at lift. Recent motion is weighted
/// most, as UIKit does, so the final flick dominates.
#[derive(Debug, Default)]
struct VelocityWindow {
    samples: std::collections::VecDeque<(f32, f64)>,
}

impl VelocityWindow {
    const SIZE: usize = 4;

    fn push(&mut self, x: f32, t: f64) {
        if self.samples.len() == Self::SIZE {
            self.samples.pop_front();
        }
        self.samples.push_back((x, t));
    }

    /// Weighted px/s, signed; weights 1, 2, 3 from the oldest pair.
    fn velocity(&self) -> f32 {
        let (mut sum, mut weights) = (0.0, 0.0);
        let pairs = self.samples.iter().zip(self.samples.iter().skip(1));
        for (i, (&(x0, t0), &(x1, t1))) in pairs.enumerate() {
            let dt = t1 - t0;
            if dt > 0.0 {
                let w = (i + 1) as f64;
                sum += f64::from(x1 - x0) / dt * w;
                weights += w;
            }
        }
        if weights == 0.0 {
            0.0
        } else {
            (sum / weights) as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays a trace of (down, x) at 500 Hz, then a lift, and returns
    /// every gesture produced.
    fn replay(xs: &[f32], hold_after: f64) -> Vec<Gesture> {
        let mut r = Recognizer::new();
        let mut out = Vec::new();
        let mut t = 0.0;
        for &x in xs {
            out.extend(r.update(true, x, t));
            t += 0.002;
        }
        t += hold_after;
        out.extend(r.update(false, 0.0, t));
        out
    }

    fn line(from: f32, to: f32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| from + (to - from) * i as f32 / (samples - 1) as f32)
            .collect()
    }

    #[test]
    fn still_press_is_a_tap_then_long_press() {
        let still = vec![300.0; 20];
        assert_eq!(replay(&still, 0.0), [Gesture::Tap { x: 300.0 }]);
        assert_eq!(replay(&still, 0.6), [Gesture::LongPress { x: 300.0 }]);
    }

    #[test]
    fn wobble_under_the_threshold_stays_a_tap() {
        // ±20 px of jitter: well under the 7–9% (45–58 px) swipe threshold.
        let wobble: Vec<f32> = (0..40)
            .map(|i| 300.0 + if i % 2 == 0 { 20.0 } else { -20.0 })
            .collect();
        assert!(matches!(replay(&wobble, 0.0)[..], [Gesture::Tap { .. }]));
    }

    #[test]
    fn swipe_drags_then_releases_with_signed_velocity() {
        let g = replay(&line(500.0, 200.0, 100), 0.0);
        assert!(matches!(g[0], Gesture::Drag { dx } if dx < 0.0));
        let Some(&Gesture::Release { dx, velocity }) = g.last() else {
            panic!("{g:?}")
        };
        assert!(dx < -200.0, "{dx}");
        assert!(velocity < -500.0, "leftward and fast: {velocity}");
    }

    #[test]
    fn touches_right_after_a_swipe_are_ignored() {
        let mut r = Recognizer::new();
        let mut t = 0.0;
        for x in line(500.0, 200.0, 100) {
            r.update(true, x, t);
            t += 0.002;
        }
        assert!(matches!(
            r.update(false, 0.0, t),
            Some(Gesture::Release { .. })
        ));
        assert_eq!(r.update(true, 100.0, t + 0.1), None);
        assert!(!r.pressed(), "a stray report does not start a press");
        r.update(true, 100.0, t + 0.3);
        assert!(r.pressed(), "after the cooldown, presses work again");
    }

    #[test]
    fn commit_rules() {
        assert!(should_commit(0.45, 0.0), "dragged past 40%");
        assert!(!should_commit(0.04, 5000.0), "barely moved");
        assert!(should_commit(0.06, 1500.0), "strong flick");
        assert!(should_commit(0.08, 900.0), "fast flick");
        assert!(!should_commit(0.18, 100.0), "slow drag needs 20%");
        assert!(should_commit(0.16, 500.0), "medium speed needs 15%");
        assert!(!should_commit(0.45, -400.0), "flicked back: reversed");
    }
}
