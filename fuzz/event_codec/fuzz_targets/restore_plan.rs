#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_storage::backup::RestorePlan;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<RestorePlan>(data);
});
