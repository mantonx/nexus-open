//! `nexusd trace`: records raw touches to a file, for tuning and testing
//! what counts as a tap, a long press, a swipe or an accidental brush.
//!
//! It only reads from the strip, so it runs alongside the daemon: Linux
//! gives every reader of a hidraw device its own copy of each report.
//!
//! The file has one sample per line, `seconds down x`: seconds since the
//! first sample, 1 or 0 for finger down, and the panel x (0–639). Files in
//! `crates/nexus-core/tests/traces/` are replayed by the tests; the name's
//! first word says what the trace must produce (see the test there).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use nexus_device::Input;
use nexus_device::hidraw::Nexus;
use nexus_device::protocol::{TouchCalibration, parse_touch};

pub fn trace(out: &Path, seconds: u64) -> Result<(), Box<dyn std::error::Error>> {
    let nexus = Nexus::open()?;
    let (tx, rx) = mpsc::channel::<Input>();
    nexus.spawn_reader(tx)?;
    let mut file = BufWriter::new(File::create(out)?);
    let mut calibration = TouchCalibration::default();
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut first: Option<Instant> = None;
    let (mut samples, mut touches, mut was_down) = (0u32, 0u32, false);
    eprintln!(
        "Recording touches to {} for {seconds} s. Touch the strip now.",
        out.display()
    );
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let report = match rx.recv_timeout(left) {
            Ok(Input::Report(report, at)) => (report, at),
            Ok(Input::Gone(e)) => return Err(e.into()),
            Err(_) => break,
        };
        let Some(t) = parse_touch(&report.0) else {
            continue;
        };
        let start = *first.get_or_insert(report.1);
        let x = calibration.to_panel_x(t.x);
        let down = u8::from(t.down);
        writeln!(
            file,
            "{:.4} {down} {x}",
            report.1.duration_since(start).as_secs_f64()
        )?;
        samples += 1;
        if t.down && !was_down {
            touches += 1;
            eprint!("\rtouches: {touches}");
        }
        was_down = t.down;
    }
    file.flush()?;
    eprintln!("\nDone: {touches} touches, {samples} samples.");
    Ok(())
}
