//! Access to the Corsair iCUE Nexus, and an in-memory simulator with the same
//! interface.
//!
//! The protocol and simulator are portable. Device access is per OS: Linux
//! uses hidraw today; macOS and Windows backends come later and sit behind
//! the same `Surface` trait. Keep OS-specific code inside `cfg`-gated modules.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

#[cfg(target_os = "linux")]
pub mod hidraw;
pub mod protocol;
pub mod simulator;

/// What a device's input reader reports.
#[derive(Debug)]
pub enum Input {
    /// One raw input report, report ID included, and when it was read.
    /// Stamped by the reader so gesture velocity is not skewed by however
    /// long the report waits in a queue.
    Report(Vec<u8>, Instant),
    /// The device went away; the reader has stopped.
    Gone(io::Error),
}

/// Something that can show a 640×48 frame.
pub trait Surface {
    /// Shows one frame of 640×48 RGBA pixels.
    fn send_frame(&mut self, rgba: &[u8]) -> Result<(), DeviceError>;
}

#[derive(Debug)]
pub enum DeviceError {
    /// No Nexus is plugged in.
    NotFound,
    /// The Nexus is present but this user may not open it.
    PermissionDenied(PathBuf),
    /// The Nexus was unplugged while in use.
    Disconnected,
    /// A frame of the wrong size; holds the size given.
    BadFrame(usize),
    /// Device access is not implemented on this operating system yet.
    Unsupported,
    Io(io::Error),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceError::NotFound => write!(f, "no iCUE Nexus found; is it plugged in?"),
            DeviceError::PermissionDenied(path) => write!(
                f,
                "no permission to open {}; install data/udev/70-nexus.rules into \
                 /etc/udev/rules.d/ and replug the device",
                path.display()
            ),
            DeviceError::Disconnected => write!(f, "the iCUE Nexus was unplugged"),
            DeviceError::BadFrame(len) => write!(
                f,
                "frame is {len} bytes; expected {}",
                protocol::FRAME_BYTES
            ),
            DeviceError::Unsupported => write!(
                f,
                "talking to the iCUE Nexus is not implemented on this operating system yet"
            ),
            DeviceError::Io(e) => write!(f, "device I/O error: {e}"),
        }
    }
}

impl std::error::Error for DeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DeviceError::Io(e) => Some(e),
            _ => None,
        }
    }
}
