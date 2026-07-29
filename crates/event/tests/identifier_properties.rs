use radroots_event::id::{CandidateId, EventId, EventPointer, EventSignature, MutationId, TradeId};

macro_rules! assert_hex_identifier_properties {
    ($type:ty, $byte_length:expr, $seed:expr) => {{
        let mut state = $seed;
        for case in 0..256 {
            let mut bytes = [0_u8; $byte_length];
            for byte in &mut bytes {
                state = xorshift64(state);
                *byte = state as u8;
            }

            let expected = encode_hex(&bytes);
            let mut mixed_case = expected.clone().into_bytes();
            for (index, byte) in mixed_case.iter_mut().enumerate() {
                if index % 3 == case % 3 && matches!(*byte, b'a'..=b'f') {
                    *byte = byte.to_ascii_uppercase();
                }
            }
            let mixed_case = String::from_utf8(mixed_case).expect("ASCII hex");

            let parsed = <$type>::parse(&mixed_case).expect("generated valid identifier");
            assert_eq!(parsed.as_bytes(), &bytes);
            assert_eq!(parsed.to_hex(), expected);
            assert_eq!(<$type>::parse(parsed.to_hex()).expect("round trip"), parsed);
        }

        for length in 0..=($byte_length * 2 + 2) {
            if length == $byte_length * 2 {
                continue;
            }
            assert!(
                <$type>::parse("a".repeat(length)).is_err(),
                "length {length} must be rejected"
            );
        }

        for invalid in [b'g', b'/', b' ', b'\n', 0, 0x7f, 0xff] {
            let mut candidate = vec![b'a'; $byte_length * 2];
            let invalid_index = usize::from(invalid) % candidate.len();
            candidate[invalid_index] = invalid;
            assert!(
                <$type>::parse(String::from_utf8_lossy(&candidate)).is_err(),
                "byte {invalid:#04x} must be rejected"
            );
        }
    }};
}

#[test]
fn canonical_hex_identifier_parsers_hold_under_generated_inputs() {
    assert_hex_identifier_properties!(EventId, 32, 0x9e37_79b9_7f4a_7c15);
    assert_hex_identifier_properties!(EventSignature, 64, 0xd1b5_4a32_d192_ed03);
    assert_hex_identifier_properties!(TradeId, 16, 0x94d0_49bb_1331_11eb);
    assert_hex_identifier_properties!(CandidateId, 32, 0x8538_eb3d_64dc_8717);
    assert_hex_identifier_properties!(MutationId, 32, 0xda94_2042_e4dd_58b5);
    assert_hex_identifier_properties!(EventPointer, 32, 0xa409_3822_299f_31d0);
}

fn xorshift64(mut value: u64) -> u64 {
    value ^= value << 13;
    value ^= value >> 7;
    value ^ (value << 17)
}

fn encode_hex(bytes: &[u8]) -> String {
    const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(LOWER_HEX[usize::from(byte >> 4)] as char);
        encoded.push(LOWER_HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}
