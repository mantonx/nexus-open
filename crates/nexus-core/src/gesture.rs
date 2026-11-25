//! Turns a stream of touch samples into gestures: drag, release, tap, long
//! press, and hold-then-slide.
//!
//! Ported from the Go app's `internal/touch` with its thresholds unchanged;
//! they were tuned on the hardware. Pure: each sample carries its own time,
//! so tests replay exact traces.
//!
//! The strip senses one axis only (`docs/USB_PROTOCOL.md`), so there is no
//! up or down swipe. Hold-then-slide stands in for one: a press held still
//! until it arms, then moved `HOLD_SLIDE`, is its own gesture, which the
//! engine uses to switch layouts.

/// Panel width in pixels, as f32 for gesture maths.
const WIDTH: f32 = crate::WIDTH as f32;

/// Held this long within `HOLD_STILL` of where it landed, a press is
/// armed: its long press is ready, and fires when the finger lifts.
/// Sliding away instead is a hold-then-slide, and the long press never
/// fires.
pub const LONG_PRESS: f64 = 0.5;
/// How far a holding finger may wander from where it landed and still arm.
/// Until 2026-11-22 "still" meant only "not yet past the swipe threshold"
/// (45–58 px), which let a slow swipe, still short of it after half a
/// second, count as a hold. Now a press that wanders this far first never
/// arms: it is a swipe once past the threshold, a tap if lifted before
/// `LONG_PRESS`, and nothing if lifted after. About 5 mm on the panel: a
/// resting finger's wobble stays well inside it (the One-Euro filter takes
/// most of it out), while a drag of any speed above about 60 px/s from the
/// moment it lands is out of it before the half second is up.
pub const HOLD_STILL: f32 = 24.0;
/// How far an armed finger moves, from where it was when it armed, before
/// the hold becomes a hold-then-slide. The same 5 mm: a short, sure push,
/// half the smallest swipe threshold, so the picker answers at once.
pub const HOLD_SLIDE: f32 = 24.0;
/// Shorter presses are ignored as brushes: the strip sits above the
/// keyboard, and a knuckle grazing it while typing must not fire an action.
/// From Matt's recordings (tests/traces): the strip reports every ~10 ms,
/// his shortest deliberate tap lasted 46 ms (5 reports), and a finger
/// bouncing as it lifts leaves single-report contacts of 8–12 ms. Brushes
/// while typing are not recorded yet; revisit with a `brush-typing` trace.
pub(crate) const MIN_TAP: f64 = 0.03;
/// Touches starting this soon after a swipe (or a hold-then-slide) ends are
/// ignored: a fast flick
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
    /// A short press without swiping, on release.
    Tap { x: f32 },
    /// A press held still for `LONG_PRESS`, at that moment, with the finger
    /// still down: the long press is ready. Nothing acts yet.
    Armed { x: f32 },
    /// An armed press lifted without sliding away: the long press. `x` is
    /// where the finger landed, the zone that showed the hold filling.
    LongPress { x: f32 },
    /// An armed press moved `HOLD_SLIDE`, and every move after. `dx` is
    /// from where the finger was when it got that far (0 on the first);
    /// negative is leftward.
    HoldDrag { dx: f32 },
    /// The finger lifted after a `HoldDrag`; `dx` as there.
    HoldRelease { dx: f32 },
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
    /// The finger went `HOLD_STILL` from where it landed before it armed:
    /// this press is no hold.
    wandered: bool,
    /// Where the finger was when the press armed; `None` before.
    armed_at: Option<f32>,
    /// Where the hold-then-slide began: `HOLD_SLIDE` from `armed_at`.
    slide_from: Option<f32>,
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
                    wandered: false,
                    armed_at: None,
                    slide_from: None,
                });
                None
            }
            (true, Some(press)) => {
                let x = self.filter.filter(x, dt).round();
                press.last_x = x;
                if let Some(from) = press.slide_from {
                    return Some(Gesture::HoldDrag { dx: x - from });
                }
                // Armed, the finger can only slide: the swipe threshold no
                // longer applies.
                if let Some(armed) = press.armed_at {
                    if (x - armed).abs() >= HOLD_SLIDE {
                        press.slide_from = Some(x);
                        return Some(Gesture::HoldDrag { dx: 0.0 });
                    }
                    return None;
                }
                let dx = x - press.start_x;
                press.wandered |= dx.abs() >= HOLD_STILL;
                if !press.swiping && dx.abs() >= swipe_threshold(self.filter.speed()) {
                    press.swiping = true;
                }
                if press.swiping {
                    self.velocity.push(x, t);
                    return Some(Gesture::Drag { dx });
                }
                self.tick(t)
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
                } else if let Some(from) = press.slide_from {
                    self.swipe_ended_at = Some(t);
                    Some(Gesture::HoldRelease {
                        dx: press.last_x - from,
                    })
                } else if press.armed_at.is_some()
                    || (!press.wandered && t - press.start_t >= LONG_PRESS)
                {
                    // Armed, or held still that long with no sample or
                    // tick between the threshold and the lift.
                    Some(Gesture::LongPress { x: press.start_x })
                } else if t - press.start_t >= LONG_PRESS {
                    // Wandered, then held: neither a tap nor a hold.
                    None
                } else if t - press.start_t >= MIN_TAP {
                    Some(Gesture::Tap { x: press.last_x })
                } else {
                    None
                }
            }
            (false, None) => None,
        }
    }

    /// Arms the long press once `t` reaches its threshold, if a still
    /// finger is down, whether or not a sample arrives then.
    pub fn tick(&mut self, t: f64) -> Option<Gesture> {
        let press = self.press.as_mut()?;
        if press.swiping
            || press.wandered
            || press.armed_at.is_some()
            || t - press.start_t < LONG_PRESS
        {
            return None;
        }
        press.armed_at = Some(press.last_x);
        Some(Gesture::Armed { x: press.start_x })
    }

    /// True while a finger is on the strip.
    pub fn pressed(&self) -> bool {
        self.press.is_some()
    }

    /// A still finger holding: where it landed and when. Kept once it has
    /// armed, until it lifts; `None` once it wanders, swipes or slides.
    pub fn holding(&self) -> Option<(f32, f64)> {
        self.press
            .as_ref()
            .filter(|p| !p.swiping && !p.wandered && p.slide_from.is_none())
            .map(|p| (p.start_x, p.start_t))
    }

    /// The finger holding has armed: lifting now is the long press.
    pub fn armed(&self) -> bool {
        self.press
            .as_ref()
            .is_some_and(|p| p.armed_at.is_some() && p.slide_from.is_none())
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
    fn a_long_press_arms_at_the_threshold_and_fires_once_on_the_lift() {
        let mut r = Recognizer::new();
        let mut t = 0.0;
        while t < 0.49 {
            assert_eq!(r.update(true, 300.0, t), None);
            t += 0.01;
        }
        assert_eq!(r.holding(), Some((300.0, 0.0)));
        assert!(!r.armed());
        assert_eq!(
            r.update(true, 300.0, 0.5),
            Some(Gesture::Armed { x: 300.0 })
        );
        assert!(r.armed());
        assert_eq!(r.holding(), Some((300.0, 0.0)), "still held, now armed");
        // A long hold: nothing more while the finger stays.
        let mut t = 0.51;
        while t < 3.0 {
            assert_eq!(r.update(true, 300.0, t), None, "at {t}");
            assert_eq!(r.tick(t), None);
            t += 0.01;
        }
        assert_eq!(
            r.update(false, 0.0, 3.0),
            Some(Gesture::LongPress { x: 300.0 }),
            "fires on the lift"
        );
        assert_eq!(r.update(false, 0.0, 3.01), None, "once");
        // With no sample at the threshold, a tick arms it.
        r.update(true, 100.0, 5.0);
        assert_eq!(r.tick(5.4), None);
        assert_eq!(r.tick(5.5), Some(Gesture::Armed { x: 100.0 }));
        assert_eq!(r.tick(5.6), None);
        assert_eq!(
            r.update(false, 0.0, 5.7),
            Some(Gesture::LongPress { x: 100.0 })
        );
    }

    #[test]
    fn a_long_press_acts_where_the_finger_landed() {
        // It drifts 20 px while held: the zone that filled is the one
        // under the landing point.
        let mut r = Recognizer::new();
        let mut t = 0.0;
        while t < 0.8 {
            let x = 300.0 + (t * 25.0) as f32;
            r.update(true, x, t);
            t += 0.01;
        }
        assert_eq!(
            r.update(false, 0.0, t),
            Some(Gesture::LongPress { x: 300.0 })
        );
    }

    /// A finger still at `x` for `hold` seconds, then moving to `to` over
    /// `secs`, resting there 0.3 s, then lifted, at 100 Hz as the strip
    /// reports. Every gesture produced.
    fn hold_then_slide(x: f32, hold: f64, to: f32, secs: f64) -> Vec<Gesture> {
        let mut r = Recognizer::new();
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < hold {
            out.extend(r.update(true, x, t));
            t += 0.01;
        }
        let start = t;
        while t < start + secs {
            let k = ((t - start) / secs) as f32;
            out.extend(r.update(true, x + (to - x) * k, t));
            t += 0.01;
        }
        let end = t + 0.3;
        while t < end {
            out.extend(r.update(true, to, t));
            t += 0.01;
        }
        out.extend(r.update(false, 0.0, t));
        out
    }

    #[test]
    fn held_then_moved_is_a_hold_slide_and_never_a_long_press() {
        let g = hold_then_slide(300.0, 0.6, 500.0, 0.6);
        assert_eq!(g[0], Gesture::Armed { x: 300.0 });
        assert_eq!(g[1], Gesture::HoldDrag { dx: 0.0 }, "{g:?}");
        assert!(
            g[1..g.len() - 1]
                .iter()
                .all(|g| matches!(g, Gesture::HoldDrag { .. }))
        );
        let Some(&Gesture::HoldRelease { dx }) = g.last() else {
            panic!("{g:?}")
        };
        // From where it got 24 px from the armed point, so about 176 px.
        assert!((150.0..=176.0).contains(&dx), "{dx}");
        assert!(!g.iter().any(|g| matches!(
            g,
            Gesture::LongPress { .. } | Gesture::Drag { .. } | Gesture::Tap { .. }
        )));
        // Leftward is negative.
        let g = hold_then_slide(300.0, 0.6, 150.0, 0.5);
        assert!(matches!(g.last(), Some(&Gesture::HoldRelease { dx }) if dx < -100.0));
    }

    #[test]
    fn an_armed_finger_that_wobbles_under_the_slide_threshold_is_still_a_long_press() {
        let g = hold_then_slide(300.0, 0.6, 315.0, 0.2);
        assert_eq!(
            g,
            [Gesture::Armed { x: 300.0 }, Gesture::LongPress { x: 300.0 }]
        );
    }

    #[test]
    fn moving_before_the_threshold_is_a_swipe_however_slow() {
        // Lands, rests 0.2 s, then a slow drag (200 px/s); and one that
        // creeps from the start (80 px/s). Neither was still for the half
        // second, so both swipe, however long they take to get there.
        for g in [
            hold_then_slide(300.0, 0.2, 100.0, 1.0),
            hold_then_slide(300.0, 0.0, 140.0, 2.0),
        ] {
            assert!(matches!(g.last(), Some(Gesture::Release { .. })), "{g:?}");
            assert!(!g.iter().any(|g| matches!(
                g,
                Gesture::Armed { .. } | Gesture::HoldDrag { .. } | Gesture::LongPress { .. }
            )));
        }
    }

    #[test]
    fn a_press_that_wanders_before_the_threshold_never_arms() {
        // 30 px in the first 0.3 s, then still: under the swipe threshold,
        // past the still radius. Lifted late, it is nothing.
        let mut r = Recognizer::new();
        let mut out = Vec::new();
        let mut t: f64 = 0.0;
        while t < 1.0 {
            let x = 300.0 + (t.min(0.3) * 100.0) as f32;
            out.extend(r.update(true, x, t));
            out.extend(r.tick(t));
            t += 0.01;
        }
        assert_eq!(r.holding(), None, "no hold fill once it wanders");
        out.extend(r.update(false, 0.0, t));
        assert_eq!(out, []);
    }

    #[test]
    fn a_touch_right_after_a_hold_slide_is_ignored() {
        let mut r = Recognizer::new();
        let mut t = 0.0;
        while t < 0.6 {
            r.update(true, 300.0, t);
            t += 0.01;
        }
        while t < 1.0 {
            r.update(true, 300.0 + ((t - 0.6) * 400.0) as f32, t);
            t += 0.01;
        }
        assert!(matches!(
            r.update(false, 0.0, t),
            Some(Gesture::HoldRelease { .. })
        ));
        r.update(true, 400.0, t + 0.05);
        assert!(!r.pressed(), "the lift's tail starts nothing");
    }

    #[test]
    fn a_graze_is_not_a_tap() {
        // Four samples, 6 ms: shorter than any deliberate tap.
        assert_eq!(replay(&[300.0; 4], 0.0), []);
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
