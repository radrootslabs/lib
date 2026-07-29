#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec, vec::Vec};

use radroots_event::{
    deletion::RadrootsAuthoredNip09DeletionRequest, kinds::KIND_DELETION_REQUEST,
    wire::RadrootsNip01EventWireParts,
};

/// Builds deterministic unsigned kind-5 wire parts for a strict NIP-09
/// deletion request.
///
/// Every tag has exactly two elements. Canonical event targets precede
/// canonical address targets, followed by the complete unique ascending set of
/// target-kind advisories.
pub fn authored_nip09_deletion_request_to_wire_parts(
    request: &RadrootsAuthoredNip09DeletionRequest,
) -> RadrootsNip01EventWireParts {
    let mut tags = Vec::with_capacity(
        request
            .target_count()
            .saturating_add(request.kind_hints().len()),
    );
    tags.extend(
        request
            .event_targets()
            .iter()
            .map(|target| vec!["e".to_string(), target.event_id().as_str().to_string()]),
    );
    tags.extend(
        request
            .address_targets()
            .iter()
            .map(|target| vec!["a".to_string(), target.coordinate().as_str().to_string()]),
    );
    tags.extend(
        request
            .kind_hints()
            .iter()
            .map(|kind| vec!["k".to_string(), kind.to_string()]),
    );

    RadrootsNip01EventWireParts {
        kind: KIND_DELETION_REQUEST,
        content: request.content().to_string(),
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::deletion::{
        RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES, RadrootsNip09DeletionAddressTarget,
        RadrootsNip09DeletionEventTarget,
    };

    fn h(character: char) -> String {
        crate::test_fixtures::fixture_public_key_hex(character)
    }

    #[test]
    fn emits_exact_canonical_two_element_target_and_kind_tags() {
        let request = RadrootsAuthoredNip09DeletionRequest::new(
            "superseded",
            vec![
                RadrootsNip09DeletionEventTarget::parse(h('f'), 30_402).expect("event target"),
                RadrootsNip09DeletionEventTarget::parse(h('a'), 1).expect("event target"),
            ],
            vec![
                RadrootsNip09DeletionAddressTarget::parse(format!("31923:{}:market", h('e')))
                    .expect("address target"),
                RadrootsNip09DeletionAddressTarget::parse(format!("30402:{}:produce", h('b')))
                    .expect("address target"),
            ],
        )
        .expect("deletion request");

        let parts = authored_nip09_deletion_request_to_wire_parts(&request);
        assert_eq!(parts.kind, KIND_DELETION_REQUEST);
        assert_eq!(parts.content, "superseded");
        assert_eq!(
            parts.tags,
            vec![
                vec!["e".to_string(), h('a')],
                vec!["e".to_string(), h('f')],
                vec!["a".to_string(), format!("30402:{}:produce", h('b'))],
                vec!["a".to_string(), format!("31923:{}:market", h('e'))],
                vec!["k".to_string(), "1".to_string()],
                vec!["k".to_string(), "30402".to_string()],
                vec!["k".to_string(), "31923".to_string()],
            ]
        );
        assert!(parts.tags.iter().all(|tag| tag.len() == 2));
    }

    #[test]
    fn core_wire_estimator_matches_emitted_json_at_the_strict_boundary() {
        let target = RadrootsNip09DeletionEventTarget::parse(h('a'), 1).expect("event target");
        let mut lower = 0usize;
        let mut upper = radroots_event::deletion::RADROOTS_NIP09_DELETION_CONTENT_MAX_BYTES;
        while lower < upper {
            let candidate = lower + (upper - lower).div_ceil(2);
            if RadrootsAuthoredNip09DeletionRequest::new(
                "\u{0001}".repeat(candidate),
                vec![target.clone()],
                Vec::new(),
            )
            .is_ok()
            {
                lower = candidate;
            } else {
                upper = candidate - 1;
            }
        }

        let request = RadrootsAuthoredNip09DeletionRequest::new(
            "\u{0001}".repeat(lower),
            vec![target],
            Vec::new(),
        )
        .expect("largest content fitting strict signed wire");
        let parts = authored_nip09_deletion_request_to_wire_parts(&request);
        let json = serde_json::json!({
            "id": "0".repeat(64),
            "pubkey": "0".repeat(64),
            "created_at": u64::MAX,
            "kind": KIND_DELETION_REQUEST,
            "tags": parts.tags,
            "content": parts.content,
            "sig": "0".repeat(128),
        })
        .to_string();
        assert!(json.len() <= RADROOTS_NIP09_DELETION_EVENT_WIRE_MAX_BYTES);
        assert_eq!(json.len(), request.maximum_signed_event_wire_bytes());
    }

    #[test]
    fn strict_wire_size_equals_independent_max_u64_serialization_with_escaping() {
        let request = RadrootsAuthoredNip09DeletionRequest::new(
            "reason \"quoted\" \\\n\u{0001} 🌱",
            vec![RadrootsNip09DeletionEventTarget::parse(h('A'), 5).expect("event target")],
            vec![
                RadrootsNip09DeletionAddressTarget::parse(format!(
                    "30000:{}:victoria:\"crop\\row:\u{0002}:雪",
                    h('B')
                ))
                .expect("address target"),
            ],
        )
        .expect("escaped deletion request");
        let parts = authored_nip09_deletion_request_to_wire_parts(&request);
        let serialized = format!(
            "{{\"id\":{},\"pubkey\":{},\"created_at\":{},\"kind\":5,\"tags\":{},\"content\":{},\"sig\":{}}}",
            serde_json::to_string(&"0".repeat(64)).expect("id JSON"),
            serde_json::to_string(&"0".repeat(64)).expect("pubkey JSON"),
            u64::MAX,
            serde_json::to_string(&parts.tags).expect("tags JSON"),
            serde_json::to_string(&parts.content).expect("content JSON"),
            serde_json::to_string(&"0".repeat(128)).expect("signature JSON"),
        );

        assert_eq!(request.maximum_signed_event_wire_bytes(), serialized.len());
    }
}
