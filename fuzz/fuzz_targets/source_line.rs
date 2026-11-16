//! One line printed by an external source program.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let line = String::from_utf8_lossy(data);
    let _ = nexus_sources::external::parse_line(&line);
});
