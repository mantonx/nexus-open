//! The real Nexus, over Linux hidraw.
//!
//! hidraw needs no USB library: writing to `/dev/hidrawN` sends an output
//! report (first byte = report ID) and reading returns one input report. The
//! kernel's usbhid driver stays bound and keeps polling the touch endpoint,
//! which also satisfies the device's "read before write" quirk that the Go
//! usbfs code had to handle by hand.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Instant;

use crate::protocol::{FRAME_BYTES, PACKET_BYTES, PACKETS_PER_FRAME, encode_packet};
use crate::{DeviceError, Input, Surface};

/// USB IDs of the iCUE Nexus as they appear in a hidraw uevent `HID_ID`.
const HID_ID: &str = "0003:00001B1C:00001B8E";
/// Interface 0 carries frames and touch; interface 1 is a keyboard.
const INTERFACE: &str = "00";

pub struct Nexus {
    file: File,
    path: PathBuf,
    packet: Box<[u8; PACKET_BYTES]>,
}

impl Nexus {
    /// Finds and opens the Nexus.
    pub fn open() -> Result<Self, DeviceError> {
        let path = find()?.ok_or(DeviceError::NotFound)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::PermissionDenied => DeviceError::PermissionDenied(path.clone()),
                io::ErrorKind::NotFound => DeviceError::NotFound,
                _ => DeviceError::Io(e),
            })?;
        Ok(Self {
            file,
            path,
            packet: Box::new([0; PACKET_BYTES]),
        })
    }

    /// The hidraw node in use, such as `/dev/hidraw15`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Starts a thread that forwards every input report to `tx`.
    ///
    /// The thread ends after sending `Input::Gone`, which is also how an
    /// unplug is noticed: the blocking read fails as soon as the device
    /// disappears.
    ///
    /// Sends into any channel whose message type converts from `Input`, so
    /// a caller can merge device input with its other events on one channel.
    pub fn spawn_reader<T>(&self, tx: Sender<T>) -> Result<(), DeviceError>
    where
        T: From<Input> + Send + 'static,
    {
        let mut file = self.file.try_clone().map_err(DeviceError::Io)?;
        thread::Builder::new()
            .name("nexus-touch".into())
            .spawn(move || {
                let mut buf = [0u8; 1024];
                loop {
                    match file.read(&mut buf) {
                        Ok(0) => {
                            let _ =
                                tx.send(Input::Gone(io::ErrorKind::UnexpectedEof.into()).into());
                            return;
                        }
                        Ok(n) => {
                            if tx
                                .send(Input::Report(buf[..n].to_vec(), Instant::now()).into())
                                .is_err()
                            {
                                return; // receiver dropped: nobody is listening
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        Err(e) => {
                            let _ = tx.send(Input::Gone(e).into());
                            return;
                        }
                    }
                }
            })
            .map_err(DeviceError::Io)?;
        Ok(())
    }
}

impl Surface for Nexus {
    fn send_frame(&mut self, rgba: &[u8]) -> Result<(), DeviceError> {
        if rgba.len() != FRAME_BYTES {
            return Err(DeviceError::BadFrame(rgba.len()));
        }
        for i in 0..PACKETS_PER_FRAME {
            encode_packet(rgba, i, &mut self.packet);
            self.file
                .write_all(&self.packet[..])
                .map_err(map_write_error)?;
        }
        Ok(())
    }
}

fn map_write_error(e: io::Error) -> DeviceError {
    // ENODEV / EPIPE / ESHUTDOWN: the device was unplugged mid-write.
    match e.raw_os_error() {
        Some(19 | 32 | 108) => DeviceError::Disconnected,
        _ => DeviceError::Io(e),
    }
}

/// Finds the hidraw node for interface 0 of the Nexus, if plugged in.
pub fn find() -> Result<Option<PathBuf>, DeviceError> {
    let entries = match fs::read_dir("/sys/class/hidraw") {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(DeviceError::Io(e)),
    };
    for entry in entries.flatten() {
        let dev = entry.path().join("device");
        let Ok(uevent) = fs::read_to_string(dev.join("uevent")) else {
            continue;
        };
        if !uevent.lines().any(|l| l == format!("HID_ID={HID_ID}")) {
            continue;
        }
        // device -> .../1-10.1:1.0/0003:1B1C:1B8E.000X; the USB interface is
        // the parent directory.
        let iface = fs::read_to_string(dev.join("../bInterfaceNumber")).unwrap_or_default();
        if iface.trim() == INTERFACE {
            return Ok(Some(Path::new("/dev").join(entry.file_name())));
        }
    }
    Ok(None)
}

/// One hidraw node of a Corsair device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub path: PathBuf,
    /// Its USB interface number.
    pub interface: Option<u8>,
    /// Whether this user can open it for reading and writing.
    pub access: bool,
    /// Processes that have it open, by pid and name.
    pub holders: Vec<(u32, String)>,
}

/// A Corsair USB device and its hidraw nodes, in interface order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub product: u16,
    /// As the device names itself.
    pub name: String,
    pub nodes: Vec<Node>,
}

/// Every Corsair device plugged in. Reads sysfs and opens each node only to
/// see whether it can be opened: nothing is sent to any device.
pub fn scan() -> Vec<Found> {
    let mut found: Vec<(PathBuf, Found)> = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return Vec::new();
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let dev = entry.path().join("device");
        let Ok(uevent) = fs::read_to_string(dev.join("uevent")) else {
            continue;
        };
        let field = |key: &str| {
            uevent
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .map(str::to_string)
        };
        // HID_ID=0003:00001B1C:00001B7C: bus, vendor, product.
        let Some(id) = field("HID_ID=") else {
            continue;
        };
        let mut parts = id.split(':').skip(1);
        let (Some(vendor), Some(product)) = (parts.next(), parts.next()) else {
            continue;
        };
        if u32::from_str_radix(vendor, 16).ok() != Some(u32::from(crate::catalog::CORSAIR)) {
            continue;
        }
        let Ok(product) = u16::from_str_radix(product.trim_start_matches('0'), 16)
            .or_else(|_| u16::from_str_radix(product, 16))
        else {
            continue;
        };
        let interface = fs::read_to_string(dev.join("../bInterfaceNumber"))
            .ok()
            .and_then(|s| u8::from_str_radix(s.trim(), 16).ok());
        let path = Path::new("/dev").join(entry.file_name());
        let access = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .is_ok();
        let node = Node {
            holders: holders(&path),
            path,
            interface,
            access,
        };
        // The USB device: hid device -> interface -> device.
        let usb = fs::canonicalize(&dev)
            .ok()
            .and_then(|d| d.parent()?.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        match found.iter_mut().find(|(u, _)| *u == usb) {
            Some((_, f)) => f.nodes.push(node),
            None => found.push((
                usb,
                Found {
                    product,
                    name: field("HID_NAME=").unwrap_or_default(),
                    nodes: vec![node],
                },
            )),
        }
    }
    let mut out: Vec<Found> = found.into_iter().map(|(_, f)| f).collect();
    for f in &mut out {
        f.nodes.sort_by_key(|n| n.interface);
    }
    out
}

/// Processes with `node` open, found through /proc/*/fd. Processes of
/// other users are not visible, which is fine: they could not open it
/// either without the same access.
pub fn holders(node: &Path) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    let Ok(procs) = fs::read_dir("/proc") else {
        return found;
    };
    for p in procs.flatten() {
        let Some(pid) = p.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = fs::read_dir(p.path().join("fd")) else {
            continue;
        };
        if fds
            .flatten()
            .any(|fd| fs::read_link(fd.path()).is_ok_and(|target| target == node))
        {
            let name = fs::read_to_string(p.path().join("comm"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            found.push((pid, name));
        }
    }
    found
}
