//! An in-memory Nexus for tests and the settings UI preview.

use crate::protocol::FRAME_BYTES;
use crate::{DeviceError, Surface};

/// Keeps the last frame sent and counts frames, so tests can assert on what
/// the daemon would have shown.
#[derive(Default)]
pub struct Simulator {
    frame: Option<Vec<u8>>,
    frames_sent: u64,
}

impl Simulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The last frame sent, as 640×48 RGBA.
    pub fn frame(&self) -> Option<&[u8]> {
        self.frame.as_deref()
    }

    pub fn frames_sent(&self) -> u64 {
        self.frames_sent
    }
}

impl Surface for Simulator {
    fn send_frame(&mut self, rgba: &[u8]) -> Result<(), DeviceError> {
        if rgba.len() != FRAME_BYTES {
            return Err(DeviceError::BadFrame(rgba.len()));
        }
        self.frame = Some(rgba.to_vec());
        self.frames_sent += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_last_frame() {
        let mut sim = Simulator::new();
        sim.send_frame(&vec![7; FRAME_BYTES]).unwrap();
        assert_eq!(sim.frames_sent(), 1);
        assert_eq!(sim.frame().unwrap()[0], 7);
    }

    #[test]
    fn rejects_wrong_size() {
        let mut sim = Simulator::new();
        assert!(matches!(
            sim.send_frame(&[0; 10]),
            Err(DeviceError::BadFrame(10))
        ));
        assert_eq!(sim.frames_sent(), 0);
    }
}
