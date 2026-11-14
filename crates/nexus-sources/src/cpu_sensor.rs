//! Which temperature sensor is "the CPU".
//!
//! Machines expose dozens of sensors: drives, memory, Wi-Fi, the
//! motherboard, every CPU core. The labels look like "chip sensor", for
//! example (from Matt's machine):
//!
//! ```text
//! coretemp Package id 0     Intel: the whole CPU package
//! coretemp Core 8           Intel: one core
//! acpitz temp1              motherboard ACPI zone (NOT the CPU; the Go app read this)
//! nvme Composite ...        a drive
//! spd5118 temp1             a memory module
//! ```
//!
//! On AMD the chip is `k10temp` (sensors `Tctl`, `Tdie`, `Tccd1`, …) or
//! `zenpower`; on Raspberry Pi and other ARM boards it is `cpu_thermal`.
//! macOS and Windows report differently again; on those, `sysinfo` labels
//! vary by machine.

/// Picks the CPU's sensor from every sensor label, or `None` if no sensor
/// is clearly the CPU (then `cpu.temp` shows "—" rather than a wrong value).
pub fn pick(labels: &[&str]) -> Option<usize> {
    // Best first. Only sensors that are certainly the CPU: a motherboard
    // zone such as `acpitz` reads 20–40 °C below the CPU, and a wrong number
    // shown with confidence is worse than a dash.
    const PREFERRED: &[&str] = &[
        "coretemp Package id 0", // Intel: the whole package
        "k10temp Tdie",          // AMD: true die temperature, where offered
        "k10temp Tctl",          // AMD: control temperature (Tdie on newer parts)
        "zenpower Tdie",
        "zenpower Tctl",
        "cpu_thermal", // Raspberry Pi and other ARM boards
    ];
    PREFERRED.iter().find_map(|want| {
        labels.iter().position(|label| {
            // Exact, or the chip name followed by a sensor name.
            label
                .strip_prefix(want)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real labels from Matt's machine, in the order sysinfo gave them.
    const MATT: &[&str] = &[
        "iwlwifi_1 temp1",
        "spd5118 temp1",
        "nvme Composite Samsung SSD 960 EVO 500GB",
        "acpitz temp1",
        "coretemp Core 28",
        "coretemp Core 20",
        "coretemp Package id 0",
        "coretemp Core 0",
    ];

    #[test]
    fn intel_package_not_motherboard() {
        assert_eq!(pick(MATT), Some(6));
    }

    #[test]
    fn amd_and_nothing() {
        assert_eq!(
            pick(&["nvme Composite X", "k10temp Tccd1", "k10temp Tctl"]),
            Some(2)
        );
        assert_eq!(pick(&["nvme Composite X", "spd5118 temp1"]), None);
        assert_eq!(pick(&["acpitz temp1"]), None, "never the motherboard zone");
        assert_eq!(pick(&["k10temp Tctl", "k10temp Tdie"]), Some(1));
        assert_eq!(pick(&["cpu_thermal temp1"]), Some(0));
    }
}
