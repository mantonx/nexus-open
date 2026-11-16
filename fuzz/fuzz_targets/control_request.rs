//! One request line on the control socket.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let line = String::from_utf8_lossy(data);
    let _ = nexus_control::Request::parse(&line);
});
