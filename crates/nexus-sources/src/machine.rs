//! CPU load and temperature, memory and network, through `sysinfo`, plus
//! the GPU.

use std::ffi::OsStr;
use std::time::Instant;

use nexus_core::signal::{Unit, Value};
use sysinfo::{Components, Networks, System};

use crate::{Reading, cpu_sensor, gpu};

pub struct Machine {
    system: System,
    networks: Option<Networks>,
    components: Option<Components>,
    /// Index into `components` of the CPU's sensor, chosen once.
    cpu_sensor: Option<usize>,
    gpu: Option<gpu::Gpu>,
    last: Instant,
}

impl Machine {
    pub fn new() -> Result<Self, sysinfo::Error> {
        let mut system = System::new()?;
        // CPU load is the busy share between two refreshes; this is the
        // first one.
        system.refresh_cpu_usage();
        let components = Components::new_with_refreshed_list().ok();
        let cpu_sensor = components.as_ref().and_then(|c| {
            let labels: Vec<&str> = c.iter().map(|s| s.label()).collect();
            cpu_sensor::pick(&labels)
        });
        Ok(Machine {
            system,
            networks: Networks::new_with_refreshed_list().ok(),
            components,
            cpu_sensor,
            gpu: gpu::Gpu::find(),
            last: Instant::now(),
        })
    }

    pub fn sample(&mut self) -> Vec<Reading> {
        let mut out = Vec::with_capacity(8);
        let now = Instant::now();
        let seconds = now.duration_since(self.last).as_secs_f64();
        self.last = now;

        self.system.refresh_cpu_usage();
        out.push(percent("cpu.load", self.system.global_cpu_usage().into()));

        self.system.refresh_memory();
        let total = self.system.total_memory();
        if total > 0 {
            let used = total.saturating_sub(self.system.available_memory());
            out.push(percent("mem.used", 100.0 * used as f64 / total as f64));
        }

        if let (Some(c), Some(i)) = (&mut self.components, self.cpu_sensor)
            && let Some(sensor) = c.get_mut(i)
        {
            sensor.refresh();
            if let Some(t) = sensor.temperature() {
                out.push(("cpu.temp", Value::Number(t.into(), Unit::Celsius)));
            }
        }

        if let Some(n) = &mut self.networks
            && seconds > 0.0
        {
            n.refresh(true);
            let (mut down, mut up) = (0, 0);
            for (_, data) in n.iter().filter(|(name, _)| counts(name)) {
                down += data.received();
                up += data.transmitted();
            }
            out.push(rate("net.down", down as f64 / seconds));
            out.push(rate("net.up", up as f64 / seconds));
        }

        if let Some(gpu) = &mut self.gpu {
            gpu.sample(&mut out);
        }
        out
    }
}

fn percent(name: &'static str, v: f64) -> Reading {
    (name, Value::Number(v.clamp(0.0, 100.0), Unit::Percent))
}

fn rate(name: &'static str, v: f64) -> Reading {
    (name, Value::Number(v, Unit::BytesPerSecond))
}

/// Whether an interface's traffic counts toward `net.down`/`net.up`: only
/// real network hardware. Loopback, containers, bridges and VPN tunnels are
/// left out, because their traffic either never leaves the machine or is
/// already counted on the physical interface it travels over.
#[cfg(target_os = "linux")]
fn counts(name: &OsStr) -> bool {
    // Physical interfaces (Ethernet, Wi-Fi) have a backing device in sysfs;
    // virtual ones do not.
    std::path::Path::new("/sys/class/net")
        .join(name)
        .join("device")
        .exists()
}

#[cfg(not(target_os = "linux"))]
fn counts(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    !(name.starts_with("lo") || name.contains("loopback") || name.starts_with("utun"))
}
