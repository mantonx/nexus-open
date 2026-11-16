//! Times frames of the default layout with moving demo data: a redraw per
//! update, as the daemon does each second, and every frame of a swipe to
//! the next page, the busiest thing the strip draws. Prints the median
//! and slowest per kind, and exits with an error if a median is over its
//! budget, so CI fails on a render-cost regression:
//!
//! ```text
//! cargo run --release -p nexus-core --example render_cost
//! ```
//!
//! Medians, not means: a shared CI machine pauses now and then, and one
//! pause must not fail the build.

use std::process::ExitCode;
use std::time::Instant;

use jiff::Zoned;
use nexus_core::config::Config;
use nexus_core::demo;
use nexus_core::engine::{Engine, Input};

/// Frames timed per kind.
const FRAMES: usize = 600;
/// Budgets for a median frame, in microseconds. On the desktop this was
/// written on (2026-11) an update took 161 µs and a swipe frame 5 µs: a
/// slide only shifts two page pictures rendered once. CI machines are
/// slower and shared, so the budgets leave about 6× and 20× room.
const UPDATE_BUDGET_US: f64 = 1_000.0;
const SWIPE_BUDGET_US: f64 = 100.0;

fn main() -> ExitCode {
    let now = Zoned::now();
    let mut engine = Engine::new(Config::default_layout());
    feed(&mut engine, 0.0);
    // The first frame fills the glyph cache; the daemon pays that once.
    engine.frame(&now, 0.0);

    let mut update = Vec::with_capacity(FRAMES);
    for i in 0..FRAMES {
        let t = i as f64;
        feed(&mut engine, t);
        let start = Instant::now();
        let frame = engine.frame(&now, t);
        update.push(start.elapsed().as_secs_f64() * 1e6);
        assert!(frame.is_some(), "new readings draw a frame");
    }

    let mut swipe = Vec::with_capacity(FRAMES);
    let mut t = FRAMES as f64;
    while swipe.len() < FRAMES {
        // A finger from the right edge to the left over a third of a
        // second, one sample per 10 ms, then the page springs into place.
        for step in 0..=30 {
            let x = 600.0 - step as f32 * 16.0;
            engine.handle(Input::Touch {
                down: true,
                x,
                at: t,
            });
            time(&mut engine, &now, t, &mut swipe);
            t += 0.010;
        }
        engine.handle(Input::Touch {
            down: false,
            x: 120.0,
            at: t,
        });
        while engine.animating() {
            time(&mut engine, &now, t, &mut swipe);
            t += 0.010;
        }
        assert_eq!(engine.page(), 1, "the swipe turned the page");
        // Back to the first page for the next swipe.
        engine.handle(Input::ShowPage(0));
        engine.frame(&now, t);
        t += 1.0;
    }

    let ok = report("update", &mut update, UPDATE_BUDGET_US)
        & report("swipe", &mut swipe, SWIPE_BUDGET_US);
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn feed(engine: &mut Engine, t: f64) {
    for (name, value) in demo::readings(t) {
        engine.handle(Input::Signal(name.into(), value));
    }
}

fn time(engine: &mut Engine, now: &Zoned, t: f64, out: &mut Vec<f64>) {
    let start = Instant::now();
    if engine.frame(now, t).is_some() {
        out.push(start.elapsed().as_secs_f64() * 1e6);
    }
}

/// Prints one kind's figures; false if its median is over `budget`.
fn report(kind: &str, us: &mut [f64], budget: f64) -> bool {
    us.sort_by(f64::total_cmp);
    let median = us[us.len() / 2];
    let worst = us[us.len() - 1];
    let ok = median <= budget;
    println!(
        "{kind:<7} {n} frames  median {median:>7.0} µs  slowest {worst:>7.0} µs  budget {budget:>6.0} µs  {}",
        if ok { "ok" } else { "OVER BUDGET" },
        n = us.len(),
    );
    ok
}
