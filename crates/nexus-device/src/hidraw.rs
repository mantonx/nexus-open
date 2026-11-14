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

use crate::protocol::{FRAME_BYTES, PACKET_BYTES, PACKETS_PER_FRAME, encode_packet};
use crate::{DeviceError, Surface};

/// USB IDs of the iCUE Nexus as they appear in a hidraw uevent `HID_ID`.
const HID_ID: &str = "0003:00001B1C:00001B8E";
/// Interface 0 carries frames and touch; interface 1 is a keyboard.
const INTERFACE: &str = "00";

/// What the touch reader thread reports.
#[derive(Debug)]
pub enum Input {
    /// One raw input report, report ID included.
    Report(Vec<u8>),
    /// The device went away; the reader thread has stopped.
    Gone(io::Error),
}

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
    pub fn spawn_reader(&self, tx: Sender<Input>) -> Result<(), DeviceError> {
        let mut file = self.file.try_clone().map_err(DeviceError::Io)?;
        thread::Builder::new()
            .name("nexus-touch".into())
            .spawn(move || {
                let mut buf = [0u8; 1024];
                loop {
                    match file.read(&mut buf) {
                        Ok(0) => {
                            let _ = tx.send(Input::Gone(io::ErrorKind::UnexpectedEof.into()));
                            return;
                        }
                        Ok(n) => {
                            if tx.send(Input::Report(buf[..n].to_vec())).is_err() {
                                return; // receiver dropped: nobody is listening
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        Err(e) => {
                            let _ = tx.send(Input::Gone(e));
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
