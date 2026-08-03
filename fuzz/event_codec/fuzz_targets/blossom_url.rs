#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_blossom::BlobUrl;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = core::str::from_utf8(data) {
        let _ = BlobUrl::parse(value);
    }
});
