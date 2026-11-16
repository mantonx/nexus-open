//! A HID input report from the strip, of any length and content.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = nexus_device::protocol::parse_touch(data);
});
