//! Access to the Corsair iCUE Nexus: the real device over Linux hidraw, and
//! an in-memory simulator with the same interface.

use std::fmt;
use std::io;
use std::path::PathBuf;

pub mod hidraw;
pub mod protocol;
pub mod simulator;

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
