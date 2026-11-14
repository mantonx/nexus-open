//! The Nexus wire protocol: frame packets out, touch reports in.
//!
//! Pure functions only, so the whole protocol is unit-tested without
//! hardware. See `docs/USB_PROTOCOL.md` for how it was worked out.

/// Panel size in pixels.
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 48;
/// One frame of RGBA pixels.
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;

/// Every frame packet is exactly this long. On hidraw the first byte is the
/// HID report ID (0x02), followed by the 1023 bytes the report descriptor
/// declares.
pub const PACKET_BYTES: usize = 1024;
const HEADER_BYTES: usize = 8;
/// Pixel bytes per packet: 254 whole pixels, so no pixel straddles packets.
const PAYLOAD_BYTES: usize = PACKET_BYTES - HEADER_BYTES;
/// `ceil(122880 / 1016)`; the last packet carries 960 bytes.
pub const PACKETS_PER_FRAME: usize = FRAME_BYTES.div_ceil(PAYLOAD_BYTES);

/// Writes packet `index` of `rgba` into `packet`.
///
/// Header: `02 05 40 <last> <index lo> <index hi> <len lo> <len hi>`, then
/// the pixels with red and blue swapped (the panel takes BGRA), then zero
/// padding up to `PACKET_BYTES`.
///
/// # Panics
/// Panics if `rgba` is not exactly one frame or `index` is out of range.
pub fn encode_packet(rgba: &[u8], index: usize, packet: &mut [u8; PACKET_BYTES]) {
    assert_eq!(rgba.len(), FRAME_BYTES, "frame must be 640×48 RGBA");
    assert!(index < PACKETS_PER_FRAME, "packet index out of range");

    let start = index * PAYLOAD_BYTES;
    let end = (start + PAYLOAD_BYTES).min(FRAME_BYTES);
    let len = end - start;
    let last = index == PACKETS_PER_FRAME - 1;

    packet[..HEADER_BYTES].copy_from_slice(&[
        0x02,
        0x05,
        0x40,
        u8::from(last),
        index as u8,
        (index >> 8) as u8,
        len as u8,
        (len >> 8) as u8,
    ]);

    // `as_chunks` views the bytes as `[u8; 4]` pixels. The payload length is
    // always a multiple of 4, so the remainders are empty.
    let (dst, _) = packet[HEADER_BYTES..HEADER_BYTES + len].as_chunks_mut::<4>();
    let (src, _) = rgba[start..end].as_chunks::<4>();
    for (d, &[r, g, b, a]) in dst.iter_mut().zip(src) {
        *d = [b, g, r, a];
    }
    packet[HEADER_BYTES + len..].fill(0);
}

/// Raw touch X reaches at least this value; seen on real hardware. Readings
/// above it raise the calibration (see `TouchCalibration`).
pub const TOUCH_RAW_MAX: u16 = 486;

/// One decoded touch report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawTouch {
    /// True while a finger is on the strip.
    pub down: bool,
    /// Raw horizontal position, 0 to about `TOUCH_RAW_MAX`.
    pub x: u16,
}

/// Decodes a touch input report as read from hidraw.
///
/// Returns `None` for anything that is not a touch report: too short, or a
/// header other than `01 02 21`.
///
/// Byte layout (from `docs/USB_PROTOCOL.md`):
///
/// ```text
/// 0     report ID, 0x01
/// 1     report type, 0x02
/// 2     touch marker, 0x21
/// 3–4   unknown, ignore
/// 5     touch state: 0 = not touching, anything else = finger down
/// 6     X position, low byte
/// 7     X position, high byte
/// ```
pub fn parse_touch(report: &[u8]) -> Option<RawTouch> {
    // A slice pattern checks the length and the header in one step: any
    // report shorter than 8 bytes or with a different header falls through
    // to `None`.
    match report {
        [0x01, 0x02, 0x21, _, _, state, x_lo, x_hi, ..] => Some(RawTouch {
            down: *state != 0,
            x: u16::from_le_bytes([*x_lo, *x_hi]),
        }),
        _ => None,
    }
}

/// Maps raw touch X to panel pixels, raising the calibration if the device
/// ever reports more than the expected maximum.
#[derive(Clone, Debug)]
pub struct TouchCalibration {
    raw_max: u16,
}

impl Default for TouchCalibration {
    fn default() -> Self {
        Self {
            raw_max: TOUCH_RAW_MAX,
        }
    }
}

impl TouchCalibration {
    /// Panel X (0–639) for a raw reading.
    pub fn to_panel_x(&mut self, raw: u16) -> u16 {
        if raw > self.raw_max {
            self.raw_max = raw;
        }
        let max_x = (WIDTH - 1) as u32;
        ((u32::from(raw) * max_x + u32::from(self.raw_max) / 2) / u32::from(self.raw_max)) as u16
    }

    pub fn raw_max(&self) -> u16 {
        self.raw_max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with(pixel: [u8; 4]) -> Vec<u8> {
        pixel.repeat(WIDTH * HEIGHT)
    }

    #[test]
    fn frame_needs_121_packets() {
        assert_eq!(PACKETS_PER_FRAME, 121);
        assert_eq!(PAYLOAD_BYTES % 4, 0, "pixels must not straddle packets");
    }

    #[test]
    fn first_packet_header_and_pixel_order() {
        let mut p = [0xAA; PACKET_BYTES];
        encode_packet(&frame_with([1, 2, 3, 4]), 0, &mut p);
        assert_eq!(&p[..8], &[0x02, 0x05, 0x40, 0, 0, 0, 0xF8, 0x03]);
        // RGBA 1,2,3,4 goes out as BGRA 3,2,1,4.
        assert_eq!(&p[8..12], &[3, 2, 1, 4]);
        assert_eq!(&p[PACKET_BYTES - 4..], &[3, 2, 1, 4]);
    }

    #[test]
    fn last_packet_is_flagged_short_and_zero_padded() {
        let mut p = [0xAA; PACKET_BYTES];
        encode_packet(&frame_with([9, 9, 9, 9]), 120, &mut p);
        // last = 1, index 120 = 0x78, length 960 = 0x03C0.
        assert_eq!(&p[..8], &[0x02, 0x05, 0x40, 1, 0x78, 0, 0xC0, 0x03]);
        assert_eq!(p[8 + 959], 9);
        assert!(p[8 + 960..].iter().all(|&b| b == 0));
    }

    #[test]
    fn packets_cover_every_pixel_once() {
        let rgba: Vec<u8> = (0..FRAME_BYTES).map(|i| (i % 251) as u8).collect();
        let mut rebuilt = Vec::with_capacity(FRAME_BYTES);
        let mut p = [0; PACKET_BYTES];
        for i in 0..PACKETS_PER_FRAME {
            encode_packet(&rgba, i, &mut p);
            let len = usize::from(p[6]) | usize::from(p[7]) << 8;
            for &[b, g, r, a] in p[8..8 + len].as_chunks::<4>().0 {
                rebuilt.extend_from_slice(&[r, g, b, a]);
            }
        }
        assert_eq!(rebuilt, rgba);
    }

    #[test]
    fn calibration_maps_ends_and_rises() {
        let mut c = TouchCalibration::default();
        assert_eq!(c.to_panel_x(0), 0);
        assert_eq!(c.to_panel_x(486), 639);
        assert_eq!(c.to_panel_x(243), 320);
        assert_eq!(c.to_panel_x(500), 639);
        assert_eq!(c.raw_max(), 500);
    }

    // ---- parse_touch ----

    fn report(bytes: &[u8]) -> Vec<u8> {
        let mut r = bytes.to_vec();
        r.resize(512, 0);
        r
    }

    #[test]
    fn touch_down_with_little_endian_x() {
        // x = 0x01E6 = 486
        let r = report(&[0x01, 0x02, 0x21, 0, 0, 1, 0xE6, 0x01]);
        assert_eq!(parse_touch(&r), Some(RawTouch { down: true, x: 486 }));
    }

    #[test]
    fn any_nonzero_state_means_down() {
        let r = report(&[0x01, 0x02, 0x21, 0, 0, 0x80, 10, 0]);
        assert_eq!(parse_touch(&r), Some(RawTouch { down: true, x: 10 }));
    }

    #[test]
    fn finger_lifted() {
        let r = report(&[0x01, 0x02, 0x21, 0, 0, 0, 0x2C, 0x01]);
        assert_eq!(
            parse_touch(&r),
            Some(RawTouch {
                down: false,
                x: 300
            })
        );
    }

    #[test]
    fn other_reports_are_ignored() {
        assert_eq!(
            parse_touch(&report(&[0x01, 0x02, 0x00, 0, 0, 1, 5, 0])),
            None
        );
        assert_eq!(
            parse_touch(&report(&[0x03, 0x02, 0x21, 0, 0, 1, 5, 0])),
            None
        );
    }

    #[test]
    fn short_reports_are_ignored() {
        assert_eq!(parse_touch(&[0x01, 0x02, 0x21, 0, 0, 1, 5]), None);
        assert_eq!(parse_touch(&[]), None);
    }
}
