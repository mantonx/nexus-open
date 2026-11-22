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
        let path = locate()?;
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

/// The Nexus's USB product id.
const PRODUCT: u16 = 0x1b8e;

/// Finds the hidraw node for interface 0 of the Nexus or, when there is
/// none, says why: unplugged, still appearing, or taken by another
/// program. Reads sysfs and /proc only.
pub fn locate() -> Result<PathBuf, DeviceError> {
    locate_in(Path::new("/sys"), Path::new("/proc"))
}

fn locate_in(sys: &Path, proc: &Path) -> Result<PathBuf, DeviceError> {
    match find_in(sys)? {
        Some(node) => Ok(node),
        None => Err(missing_in(sys, proc, PRODUCT, 0).into()),
    }
}

/// Why a USB interface of a Corsair device has no hidraw node.
///
/// hidraw nodes exist only while the kernel's usbhid driver is bound to the
/// interface. A program that claims the interface through libusb or usbfs
/// detaches usbhid first, and the node goes with it; unless the program
/// reattaches usbhid when it lets go, the interface stays unbound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// No such USB device is plugged in.
    Unplugged,
    /// Bound to usbhid; the node is still being made.
    Pending,
    /// Bound to another driver, `usbfs` for a program using libusb or
    /// usbfs. With the processes that have the USB device open.
    Claimed {
        driver: String,
        holders: Vec<(u32, String)>,
    },
    /// Bound to no driver, with the processes that have the USB device open.
    Unbound { holders: Vec<(u32, String)> },
}

impl From<Missing> for DeviceError {
    fn from(m: Missing) -> Self {
        match m {
            Missing::Unplugged => DeviceError::NotFound,
            Missing::Pending => DeviceError::NotReady,
            Missing::Claimed { holders, .. } => DeviceError::InUse(holders),
            Missing::Unbound { holders } if holders.is_empty() => DeviceError::Detached,
            Missing::Unbound { holders } => DeviceError::InUse(holders),
        }
    }
}

/// Why `interface` of the Corsair device `product` has no hidraw node.
pub fn missing(product: u16, interface: u8) -> Missing {
    missing_in(Path::new("/sys"), Path::new("/proc"), product, interface)
}

fn missing_in(sys: &Path, proc: &Path, product: u16, interface: u8) -> Missing {
    let Ok(entries) = fs::read_dir(sys.join("bus/usb/devices")) else {
        return Missing::Unplugged;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    let read = |path: PathBuf| fs::read_to_string(path).ok().map(|s| s.trim().to_string());
    for dev in entries {
        // Interfaces (1-10.1:1.0) and root hubs are listed too; only a
        // device has these ids.
        if read(dev.join("idVendor")) != Some(format!("{:04x}", crate::catalog::CORSAIR))
            || read(dev.join("idProduct")) != Some(format!("{product:04x}"))
        {
            continue;
        }
        // Its interfaces are the subdirectories named <device>:<config>.<n>.
        let iface = fs::read_dir(&dev).ok().and_then(|children| {
            children.flatten().map(|c| c.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains(':'))
                    && read(p.join("bInterfaceNumber"))
                        .and_then(|n| u8::from_str_radix(&n, 16).ok())
                        == Some(interface)
            })
        });
        let driver = iface.and_then(|i| {
            fs::read_link(i.join("driver"))
                .ok()?
                .file_name()?
                .to_str()
                .map(str::to_string)
        });
        if driver.as_deref() == Some("usbhid") {
            return Missing::Pending;
        }
        let number = |name: &str| read(dev.join(name)).and_then(|n| n.parse::<u32>().ok());
        let holders = match (number("busnum"), number("devnum")) {
            (Some(bus), Some(dev)) => {
                holders_in(proc, Path::new(&format!("/dev/bus/usb/{bus:03}/{dev:03}")))
            }
            _ => Vec::new(),
        };
        return match driver {
            Some(driver) => Missing::Claimed { driver, holders },
            None => Missing::Unbound { holders },
        };
    }
    Missing::Unplugged
}

/// Finds the hidraw node for interface 0 of the Nexus, if it has one.
fn find_in(sys: &Path) -> Result<Option<PathBuf>, DeviceError> {
    let entries = match fs::read_dir(sys.join("class/hidraw")) {
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
///
/// Inside nexusd's sandbox nothing is visible: systemd runs a sandboxed
/// user service in its own user namespace, and the kernel lets a process
/// read the fd links of another only from the same namespace (listing the
/// fd directory works; reading a link fails). Callers must cope with an
/// empty answer.
pub fn holders(node: &Path) -> Vec<(u32, String)> {
    holders_in(Path::new("/proc"), node)
}

fn holders_in(proc: &Path, node: &Path) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    let Ok(procs) = fs::read_dir(proc) else {
        return found;
    };
    let mut procs: Vec<_> = procs.flatten().collect();
    procs.sort_by_key(|p| p.file_name());
    for p in procs {
        let Some(pid) = p.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = fs::read_dir(p.path().join("fd")) else {
            continue;
        };
        let mut holds = false;
        for fd in fds.flatten() {
            match fs::read_link(fd.path()) {
                Ok(target) if target == node => {
                    holds = true;
                    break;
                }
                Ok(_) => {}
                // A closed fd races the listing; anything else (EPERM, as in
                // the sandbox) applies to every fd of this process.
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(_) => break,
            }
        }
        if holds {
            let name = fs::read_to_string(p.path().join("comm"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            found.push((pid, name));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// A fake /sys and /proc holding a Nexus at USB 1-10.1 (bus 1,
    /// device 15), laid out as the kernel lays them out, symlinks included.
    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn new(name: &str) -> Tree {
            let root =
                std::env::temp_dir().join(format!("nexus-hidraw-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("sys/bus/usb/devices")).unwrap();
            fs::create_dir_all(root.join("sys/class/hidraw")).unwrap();
            fs::create_dir_all(root.join("proc")).unwrap();
            Tree { root }
        }

        fn sys(&self) -> PathBuf {
            self.root.join("sys")
        }

        fn proc(&self) -> PathBuf {
            self.root.join("proc")
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn link(&self, path: &str, target: &str) {
            let path = self.root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            symlink(target, path).unwrap();
        }

        /// The strip on the bus, with interface 0 bound to `driver` (to
        /// none for `None`) and interface 1 to usbhid.
        fn plug(&self, driver: Option<&str>) {
            let dev = "sys/devices/pci0000:00/usb1/1-10/1-10.1";
            self.write(&format!("{dev}/idVendor"), "1b1c\n");
            self.write(&format!("{dev}/idProduct"), "1b8e\n");
            self.write(&format!("{dev}/busnum"), "1\n");
            self.write(&format!("{dev}/devnum"), "15\n");
            self.write(&format!("{dev}/1-10.1:1.0/bInterfaceNumber"), "00\n");
            self.write(&format!("{dev}/1-10.1:1.1/bInterfaceNumber"), "01\n");
            self.link(
                &format!("{dev}/1-10.1:1.1/driver"),
                "../../../../../../bus/usb/drivers/usbhid",
            );
            self.link(
                "sys/bus/usb/devices/1-10.1",
                "../../../devices/pci0000:00/usb1/1-10/1-10.1",
            );
            if let Some(driver) = driver {
                self.link(
                    &format!("{dev}/1-10.1:1.0/driver"),
                    &format!("../../../../../../bus/usb/drivers/{driver}"),
                );
            }
            if driver == Some("usbhid") {
                let hid = format!("{dev}/1-10.1:1.0/0003:1B1C:1B8E.0001");
                self.write(
                    &format!("{hid}/uevent"),
                    "DRIVER=hid-generic\nHID_ID=0003:00001B1C:00001B8E\n",
                );
                self.link(
                    "sys/class/hidraw/hidraw15/device",
                    &format!("../../../{}", hid.trim_start_matches("sys/")),
                );
            }
        }

        /// A process with `target` open as fd 7.
        fn process(&self, pid: u32, name: &str, target: &str) {
            self.write(&format!("proc/{pid}/comm"), &format!("{name}\n"));
            self.link(&format!("proc/{pid}/fd/0"), "/dev/null");
            self.link(&format!("proc/{pid}/fd/7"), target);
        }

        fn locate(&self) -> Result<PathBuf, DeviceError> {
            locate_in(&self.sys(), &self.proc())
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn unplugged_is_not_found() {
        let t = Tree::new("absent");
        t.process(42, "nexus-open", "/dev/bus/usb/001/015");
        assert!(matches!(t.locate(), Err(DeviceError::NotFound)));
    }

    #[test]
    fn bound_to_usbhid_is_found_at_its_node() {
        let t = Tree::new("usbhid");
        t.plug(Some("usbhid"));
        assert_eq!(t.locate().unwrap(), Path::new("/dev/hidraw15"));
    }

    #[test]
    fn bound_to_usbhid_without_a_node_yet_is_not_ready() {
        let t = Tree::new("pending");
        t.plug(Some("usbhid"));
        fs::remove_file(t.root.join("sys/class/hidraw/hidraw15/device")).unwrap();
        assert!(matches!(t.locate(), Err(DeviceError::NotReady)));
    }

    #[test]
    fn claimed_through_usbfs_names_the_program_holding_it() {
        let t = Tree::new("usbfs-held");
        t.plug(Some("usbfs"));
        t.process(42, "nexus-open", "/dev/bus/usb/001/015");
        t.process(43, "firefox", "/dev/bus/usb/003/002");
        assert_eq!(
            missing_in(&t.sys(), &t.proc(), PRODUCT, 0),
            Missing::Claimed {
                driver: "usbfs".into(),
                holders: vec![(42, "nexus-open".into())],
            }
        );
        let e = t.locate().unwrap_err();
        assert!(matches!(&e, DeviceError::InUse(h) if h == &[(42, "nexus-open".into())]));
        assert_eq!(e.to_string(), "the strip is in use by nexus-open (pid 42)");
    }

    #[test]
    fn claimed_with_no_holder_in_sight_is_still_in_use() {
        let t = Tree::new("usbfs-hidden");
        t.plug(Some("usbfs"));
        let e = t.locate().unwrap_err();
        assert!(matches!(&e, DeviceError::InUse(h) if h.is_empty()));
        assert!(e.to_string().contains("in use by another program"), "{e}");
    }

    #[test]
    fn left_without_a_driver_says_to_replug() {
        let t = Tree::new("unbound");
        t.plug(None);
        assert_eq!(
            missing_in(&t.sys(), &t.proc(), PRODUCT, 0),
            Missing::Unbound { holders: vec![] }
        );
        let e = t.locate().unwrap_err();
        assert!(matches!(e, DeviceError::Detached));
        assert!(
            e.to_string().contains("unplug it and plug it back in"),
            "{e}"
        );
    }

    #[test]
    fn unbound_but_still_open_is_in_use() {
        let t = Tree::new("unbound-held");
        t.plug(None);
        t.process(42, "nexus-open", "/dev/bus/usb/001/015");
        assert!(
            matches!(t.locate(), Err(DeviceError::InUse(h)) if h == [(42, "nexus-open".into())])
        );
    }
}
