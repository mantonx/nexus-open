//! The preview as the strip: a press on it, in Try mode, becomes a touch
//! the daemon acts on, as if made on the strip itself.

use std::time::Duration;

/// Farther than this between press and release, in strip pixels, is a
/// swipe (or a drag along a slider), not a tap.
const SWIPE: f32 = 24.0;
/// Held at least this long, it is a long press. The strip's own is 0.5 s.
const HOLD: Duration = Duration::from_millis(500);

/// The request for a press at `from`, released at `to` after `held`.
pub fn gesture(from: f32, to: f32, held: Duration) -> String {
    let clamp = |x: f32| x.clamp(0.0, 639.0);
    let (from, to) = (clamp(from), clamp(to));
    if (to - from).abs() > SWIPE {
        format!("drag {from:.0} {to:.0}")
    } else if held >= HOLD {
        format!("hold {from:.0}")
    } else {
        format!("tap {from:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUICK: Duration = Duration::from_millis(120);

    #[test]
    fn a_still_press_is_a_tap_or_a_long_press() {
        assert_eq!(gesture(100.0, 104.0, QUICK), "tap 100");
        assert_eq!(
            gesture(100.0, 104.0, Duration::from_millis(700)),
            "hold 100"
        );
    }

    #[test]
    fn a_moving_press_is_a_drag_inside_the_strip() {
        assert_eq!(gesture(500.0, 120.0, QUICK), "drag 500 120");
        assert_eq!(gesture(600.0, 900.0, QUICK), "drag 600 639");
        assert_eq!(gesture(-5.0, 300.0, Duration::from_secs(2)), "drag 0 300");
    }
}
