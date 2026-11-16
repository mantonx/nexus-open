//! A layout file or `nexusctl edit` text: parsed, validated, and, if it
//! is accepted, written and parsed again to the same layout.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok((config, notes)) = nexus_store::text::parse(text) {
        let again = nexus_store::text::write(&config, &notes);
        let (back, _) = nexus_store::text::parse(&again).expect("written text parses");
        assert_eq!(back.pages, config.pages);
        assert_eq!(back.takeovers, config.takeovers);
    }
});
