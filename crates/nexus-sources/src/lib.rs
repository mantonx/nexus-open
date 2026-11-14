//! Built-in sources: readings from this machine, and the weather.
//!
//! A source publishes raw readings in base units (°C, percent, bytes per
//! second) under fixed signal names; how they look is up to the widgets.
//! A reading that cannot be taken is simply not published, so its widget
//! keeps showing "—" rather than a made-up number.
//!
//! Each source runs on its own thread and hands its readings to a callback,
//! so this crate knows nothing about the daemon.

use std::thread;
use std::time::Duration;

use nexus_core::signal::Value;

mod cpu_sensor;
mod gpu;
mod machine;
mod weather;

pub use weather::spawn as spawn_weather;

/// One reading: a signal name and its value.
pub type Reading = (&'static str, Value);

/// How often machine readings are taken. Graphs keep 60 samples, so they
/// span the last minute.
pub const MACHINE_INTERVAL: Duration = Duration::from_secs(1);

/// Samples CPU, memory, network and GPU every [`MACHINE_INTERVAL`] and
/// passes each batch to `publish`. Stops when `publish` returns false.
pub fn spawn_machine(mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    thread::Builder::new()
        .name("machine-source".into())
        .spawn(move || {
            let mut machine = match machine::Machine::new() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("nexusd: cannot read this machine's CPU and memory: {e}");
                    return;
                }
            };
            loop {
                thread::sleep(MACHINE_INTERVAL);
                if !publish(machine.sample()) {
                    return;
                }
            }
        })
        .expect("spawning a thread");
}
