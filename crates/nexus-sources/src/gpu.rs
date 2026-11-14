//! GPU temperature and load.
//!
//! NVIDIA cards are read through NVML, the library `nvidia-smi` itself
//! uses, loaded at runtime: a machine without the NVIDIA driver simply has
//! no NVML. AMD cards on Linux are read from sysfs. Only the first GPU
//! found is read. macOS has neither yet, so there is never a GPU there.

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use readable::Gpu;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use unreadable::Gpu;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unreadable {
    use crate::Reading;

    /// No GPU can be read on this system.
    pub enum Gpu {}

    impl Gpu {
        pub fn find() -> Option<Gpu> {
            None
        }

        pub fn sample(&mut self, _: &mut Vec<Reading>) {}
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod readable {
    use nexus_core::signal::{Unit, Value};

    #[cfg(target_os = "linux")]
    use super::amd;
    use crate::Reading;

    pub enum Gpu {
        Nvidia(Box<nvml_wrapper::Nvml>),
        #[cfg(target_os = "linux")]
        Amd(amd::Card),
    }

    impl Gpu {
        /// The first GPU that can be read, if any.
        pub fn find() -> Option<Gpu> {
            if let Ok(nvml) = nvml_wrapper::Nvml::init()
                && nvml.device_count().is_ok_and(|n| n > 0)
            {
                return Some(Gpu::Nvidia(Box::new(nvml)));
            }
            #[cfg(target_os = "linux")]
            if let Some(card) = amd::Card::find() {
                return Some(Gpu::Amd(card));
            }
            None
        }

        pub fn sample(&mut self, out: &mut Vec<Reading>) {
            let (temp, load) = match self {
                Gpu::Nvidia(nvml) => {
                    use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
                    let Ok(device) = nvml.device_by_index(0) else {
                        return;
                    };
                    (
                        device
                            .temperature(TemperatureSensor::Gpu)
                            .ok()
                            .map(f64::from),
                        device.utilization_rates().ok().map(|u| f64::from(u.gpu)),
                    )
                }
                #[cfg(target_os = "linux")]
                Gpu::Amd(card) => (card.temperature(), card.load()),
            };
            if let Some(t) = temp {
                out.push(("gpu.temp", Value::Number(t, Unit::Celsius)));
            }
            if let Some(l) = load {
                out.push((
                    "gpu.load",
                    Value::Number(l.clamp(0.0, 100.0), Unit::Percent),
                ));
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod amd {
    use std::fs;
    use std::path::{Path, PathBuf};

    /// An amdgpu card's sysfs device directory.
    pub struct Card {
        device: PathBuf,
        /// Its hwmon directory, holding `temp1_input` (the edge sensor).
        hwmon: Option<PathBuf>,
    }

    impl Card {
        pub fn find() -> Option<Card> {
            Card::find_in(Path::new("/sys/class/drm"))
        }

        fn find_in(drm: &Path) -> Option<Card> {
            let mut cards: Vec<PathBuf> = fs::read_dir(drm)
                .ok()?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("card") && !n.contains('-'))
                })
                .collect();
            cards.sort();
            let device = cards
                .into_iter()
                .map(|c| c.join("device"))
                .find(|d| d.join("gpu_busy_percent").exists())?;
            let hwmon = fs::read_dir(device.join("hwmon"))
                .ok()
                .and_then(|mut d| d.next())
                .and_then(|e| e.ok())
                .map(|e| e.path());
            Some(Card { device, hwmon })
        }

        pub fn load(&self) -> Option<f64> {
            read_number(&self.device.join("gpu_busy_percent"))
        }

        pub fn temperature(&self) -> Option<f64> {
            // hwmon reports millidegrees.
            read_number(&self.hwmon.as_ref()?.join("temp1_input")).map(|m| m / 1000.0)
        }
    }

    fn read_number(path: &Path) -> Option<f64> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_a_fake_card() {
            let root = std::env::temp_dir().join(format!("nexus-amd-{}", std::process::id()));
            let device = root.join("card1/device");
            fs::create_dir_all(device.join("hwmon/hwmon5")).unwrap();
            fs::create_dir_all(root.join("card1-DP-1")).unwrap();
            fs::write(device.join("gpu_busy_percent"), "37\n").unwrap();
            fs::write(device.join("hwmon/hwmon5/temp1_input"), "61000\n").unwrap();

            let card = Card::find_in(&root).expect("card found");
            assert_eq!(card.load(), Some(37.0));
            assert_eq!(card.temperature(), Some(61.0));
            fs::remove_dir_all(root).unwrap();
        }
    }
}
