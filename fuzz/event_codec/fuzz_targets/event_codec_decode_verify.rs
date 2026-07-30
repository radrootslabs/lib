#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_event_codec::{decode, verify};

fuzz_target!(|data: &[u8]| {
    if data.len() > decode::MAX_EVENT_JSON_BYTES {
        return;
    }
    let Ok(raw_json) = core::str::from_utf8(data) else {
        return;
    };
    if let Ok(event) = decode::event(raw_json) {
        let _ = verify::id(event);
    }
});
