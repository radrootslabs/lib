use std::panic::{AssertUnwindSafe, catch_unwind};

use radroots_event::envelope::{RadrootsEventTag, RadrootsEventTags};
use radroots_event::id::{DTag, EventId, EventSignature, Nip01Coordinate};
use radroots_event::tag::relay_hint::RadrootsNostrRelayHint;
use radroots_event::wire::v1::RadrootsNip01EventWire;

const VALID_WIRE_VECTOR: &str =
    include_str!("../../../contracts/conformance/vectors/event/nip01_wire.v1.json");

#[test]
fn wire_and_tag_parsers_are_total_over_deterministic_mutation_corpus() {
    for case in 0_u64..256 {
        let bytes = corpus_bytes(case, ((case * 37) % 2049) as usize);
        assert_parser_totality(bytes.as_slice(), case);
    }

    let valid_bytes = VALID_WIRE_VECTOR.as_bytes();
    for case in 0_u64..256 {
        let mut mutated = valid_bytes.to_vec();
        let index = ((case * 131) as usize) % mutated.len();
        mutated[index] ^= (case as u8).wrapping_mul(29).wrapping_add(1);
        assert_parser_totality(mutated.as_slice(), case + 256);
    }
}

fn assert_parser_totality(bytes: &[u8], case: u64) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Ok(text) = core::str::from_utf8(bytes) {
            let _ = RadrootsNip01EventWire::parse_json(text);
            let _ = DTag::parse(text);
            let _ = EventId::parse(text);
            let _ = EventSignature::parse(text);
            let _ = Nip01Coordinate::parse(text);
            let _ = RadrootsNostrRelayHint::parse(text);
        }

        let tags = tags_from_bytes(bytes);
        for (index, tag) in tags.iter().cloned().enumerate() {
            let _ = RadrootsEventTag::new(index, tag);
        }
        let _ = RadrootsEventTags::new(tags);
    }));

    assert!(result.is_ok(), "parser mutation case {case} panicked");
}

fn corpus_bytes(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    (0..length)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 32) as u8
        })
        .collect()
}

fn tags_from_bytes(bytes: &[u8]) -> Vec<Vec<String>> {
    bytes
        .chunks(64)
        .take(16)
        .map(|tag| {
            tag.chunks(8)
                .take(8)
                .map(|element| String::from_utf8_lossy(element).into_owned())
                .collect()
        })
        .collect()
}
