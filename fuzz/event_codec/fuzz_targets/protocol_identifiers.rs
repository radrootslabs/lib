#![no_main]

use libfuzzer_sys::fuzz_target;
use radroots_protocol::{capability::v1::TransportKind, event::v1::TradeState, schema::SchemaId};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = core::str::from_utf8(data) {
        let _ = SchemaId::parse(value);
        let _ = TransportKind::parse(value);
        let _ = TradeState::parse(value);
    }
});
