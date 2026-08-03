#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_event_codec::manifest::parse_knowledge_contract_manifest_json;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = core::str::from_utf8(data) {
        let _ = parse_knowledge_contract_manifest_json(value);
    }
});
