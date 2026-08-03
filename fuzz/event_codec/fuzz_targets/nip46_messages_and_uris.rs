#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_nostr_connect::{
    message::{RequestMessage, ResponseEnvelope},
    uri::Uri,
};

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<RequestMessage>(data);
    let _ = serde_json::from_slice::<ResponseEnvelope>(data);
    if let Ok(value) = core::str::from_utf8(data) {
        let _ = Uri::parse(value);
    }
});
