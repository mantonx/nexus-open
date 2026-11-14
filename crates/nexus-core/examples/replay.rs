//! Replays a touch recording from `nexusd trace` through the gesture
//! recognizer and prints, per touch, how long it lasted, how far it moved
//! and what it was read as. For tuning taps, holds and the brush guard:
//!
//! ```text
//! cargo run -p nexus-core --example replay -- FILE
//! ```

use std::env;
use std::fs;

use nexus_core::gesture::Recognizer;

fn main() {
    let path = env::args().nth(1).expect("usage: replay FILE");
    let text = fs::read_to_string(&path).expect("readable trace");
    let samples: Vec<(f64, bool, f32)> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (f[0].parse().unwrap(), f[1] == "1", f[2].parse().unwrap())
        })
        .collect();

    let mut r = Recognizer::new();
    let mut touch: Option<(f64, f32, f32, f32, usize)> = None; // start, first x, min, max, samples
    let mut gestures = Vec::new();
    let mut n = 0;
    println!("  #   start   length  samples  x from → range       read as");
    for &(t, down, x) in &samples {
        if let Some(g) = r.update(down, x, t) {
            gestures.push(format!("{g:?}"));
        }
        match (down, touch.as_mut()) {
            (true, None) => touch = Some((t, x, x, x, 1)),
            (true, Some(c)) => {
                c.2 = c.2.min(x);
                c.3 = c.3.max(x);
                c.4 += 1;
            }
            (false, Some(&mut (start, first, lo, hi, count))) => {
                n += 1;
                let read = if gestures.is_empty() {
                    "nothing".to_string()
                } else {
                    // Drags are many; show the last gesture with a count.
                    let drags = gestures.iter().filter(|g| g.starts_with("Drag")).count();
                    let last = gestures.last().unwrap();
                    if drags > 0 {
                        format!("{drags} drags, then {last}")
                    } else {
                        last.clone()
                    }
                };
                println!(
                    "{n:3}  {start:6.2}s  {:5.0} ms  {count:5}    {first:5.0} → {lo:.0}–{hi:.0}   {read}",
                    (t - start) * 1000.0
                );
                touch = None;
                gestures.clear();
            }
            (false, None) => {}
        }
    }
    let gaps: Vec<f64> = samples
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .filter(|g| *g > 0.0 && *g < 0.05)
        .collect();
    if !gaps.is_empty() {
        let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
        println!(
            "\nreport interval while touching: {:.1} ms on average",
            mean * 1000.0
        );
    }
}
