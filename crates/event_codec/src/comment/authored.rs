#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    comment::{
        RadrootsAuthoredNip22Comment, RadrootsNip22AddressRootReference,
        RadrootsNip22CommentParentReference, RadrootsNip22CommentPosition,
        RadrootsNip22CommentRoot, RadrootsNip22EventRootReference,
    },
    kinds::KIND_COMMENT,
    relay_hint::RadrootsNostrRelayHint,
    wire::RadrootsNip01EventWireParts,
};
use radroots_identity::PublicKey;

/// Builds deterministic unsigned kind-1111 wire parts for a strict NIP-22
/// comment.
pub fn authored_nip22_comment_to_wire_parts(
    comment: &RadrootsAuthoredNip22Comment,
) -> RadrootsNip01EventWireParts {
    let root = comment.root();
    let position = comment.position();
    let mut tags = Vec::with_capacity(6);
    match root {
        RadrootsNip22CommentRoot::Event(reference) => {
            tags.push(event_reference_tag("E", reference));
        }
        RadrootsNip22CommentRoot::Address(reference) => {
            tags.push(address_reference_tag("A", reference));
        }
    }
    tags.push(vec!["K".to_string(), root.kind().as_u32().to_string()]);
    tags.push(participant_tag("P", root.author(), root.relay()));

    match (root, position) {
        (
            RadrootsNip22CommentRoot::Event(reference),
            RadrootsNip22CommentPosition::TopLevelEvent,
        ) => {
            tags.push(event_reference_tag("e", reference));
            tags.push(vec!["k".to_string(), root.kind().as_u32().to_string()]);
            tags.push(participant_tag("p", reference.author(), reference.relay()));
        }
        (
            RadrootsNip22CommentRoot::Address(reference),
            RadrootsNip22CommentPosition::TopLevelAddress { current_revision },
        ) => {
            tags.push(address_reference_tag("a", reference));
            tags.push(optional_relay_tag(
                "e",
                current_revision.to_hex().as_str(),
                reference.relay(),
            ));
            tags.push(vec!["k".to_string(), root.kind().as_u32().to_string()]);
            tags.push(participant_tag("p", reference.author(), reference.relay()));
        }
        (_, RadrootsNip22CommentPosition::Nested { parent }) => {
            tags.push(parent_reference_tag(parent));
            tags.push(vec!["k".to_string(), KIND_COMMENT.to_string()]);
            tags.push(participant_tag("p", parent.author(), parent.relay()));
        }
        _ => unreachable!("authored Comment constructors preserve root and position compatibility"),
    }

    RadrootsNip01EventWireParts {
        kind: KIND_COMMENT,
        content: comment.content().to_string(),
        tags,
    }
}

fn event_reference_tag(name: &str, reference: &RadrootsNip22EventRootReference) -> Vec<String> {
    vec![
        name.to_string(),
        reference.event_id().to_hex(),
        reference.relay_or_empty().to_string(),
        reference.author().to_hex(),
    ]
}

fn address_reference_tag(name: &str, reference: &RadrootsNip22AddressRootReference) -> Vec<String> {
    optional_relay_tag(name, reference.coordinate().as_str(), reference.relay())
}

fn parent_reference_tag(reference: &RadrootsNip22CommentParentReference) -> Vec<String> {
    vec![
        "e".to_string(),
        reference.event_id().to_hex(),
        reference.relay_or_empty().to_string(),
        reference.author().to_hex(),
    ]
}

fn participant_tag(
    name: &str,
    author: &PublicKey,
    relay: Option<&RadrootsNostrRelayHint>,
) -> Vec<String> {
    optional_relay_tag(name, &author.to_hex(), relay)
}

fn optional_relay_tag(
    name: &str,
    value: &str,
    relay: Option<&RadrootsNostrRelayHint>,
) -> Vec<String> {
    let mut tag = vec![name.to_string(), value.to_string()];
    if let Some(relay) = relay {
        tag.push(relay.as_str().to_string());
    }
    tag
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{
        comment::{
            RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES, RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES,
            RadrootsNip22AddressRootReference, RadrootsNip22CommentError,
            RadrootsNip22CommentParentReference, RadrootsNip22EventRootReference,
        },
        kinds::{KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_CLASSIFIED_LISTING},
    };

    fn h(character: char) -> String {
        crate::test_fixtures::fixture_public_key_hex(character)
    }

    fn event_root(relay: Option<&str>) -> RadrootsNip22EventRootReference {
        RadrootsNip22EventRootReference::parse(h('a'), h('b'), KIND_CLASSIFIED_LISTING, relay)
            .expect("event root")
    }

    fn address_root(relay: Option<&str>) -> RadrootsNip22AddressRootReference {
        RadrootsNip22AddressRootReference::parse(format!("31922:{}:market-day", h('b')), relay)
            .expect("address root")
    }

    fn parent(relay: Option<&str>) -> RadrootsNip22CommentParentReference {
        RadrootsNip22CommentParentReference::parse(h('c'), h('d'), relay).expect("parent")
    }

    #[derive(Clone, Copy)]
    enum AuthoredShape {
        TopEvent,
        TopAddress,
        NestedEvent,
        NestedAddress,
    }

    fn authored_shape(
        shape: AuthoredShape,
        content: String,
    ) -> Result<RadrootsAuthoredNip22Comment, RadrootsNip22CommentError> {
        match shape {
            AuthoredShape::TopEvent => {
                RadrootsAuthoredNip22Comment::top_level_event(content, event_root(None))
            }
            AuthoredShape::TopAddress => RadrootsAuthoredNip22Comment::parse_top_level_address(
                content,
                address_root(None),
                h('e'),
            ),
            AuthoredShape::NestedEvent => {
                RadrootsAuthoredNip22Comment::nested(content, event_root(None), parent(None))
            }
            AuthoredShape::NestedAddress => {
                RadrootsAuthoredNip22Comment::nested(content, address_root(None), parent(None))
            }
        }
    }

    #[test]
    fn emits_exact_top_level_event_tags_with_empty_relay_slot() {
        let comment = RadrootsAuthoredNip22Comment::top_level_event("Comment", event_root(None))
            .expect("comment");
        assert_eq!(
            authored_nip22_comment_to_wire_parts(&comment).tags,
            vec![
                vec!["E".to_string(), h('a'), String::new(), h('b'),],
                vec!["K".to_string(), KIND_CLASSIFIED_LISTING.to_string()],
                vec!["P".to_string(), h('b')],
                vec!["e".to_string(), h('a'), String::new(), h('b'),],
                vec!["k".to_string(), KIND_CLASSIFIED_LISTING.to_string()],
                vec!["p".to_string(), h('b')],
            ]
        );
    }

    #[test]
    fn emits_exact_top_level_address_tags_with_revision_without_author() {
        let comment = RadrootsAuthoredNip22Comment::parse_top_level_address(
            "Comment",
            address_root(Some("wss://relay.example")),
            h('e'),
        )
        .expect("comment");
        let tags = authored_nip22_comment_to_wire_parts(&comment).tags;
        assert_eq!(tags.len(), 7);
        assert_eq!(
            tags.iter().map(|tag| tag[0].as_str()).collect::<Vec<_>>(),
            vec!["A", "K", "P", "a", "e", "k", "p"]
        );
        assert_eq!(
            tags[4],
            vec!["e".to_string(), h('e'), "wss://relay.example".to_string()]
        );
    }

    #[test]
    fn emits_exact_nested_event_and_address_shapes() {
        for root in [
            RadrootsNip22CommentRoot::from(
                RadrootsNip22EventRootReference::parse(
                    h('a'),
                    h('b'),
                    KIND_CALENDAR_TIME_EVENT,
                    Some("wss://root.example"),
                )
                .expect("event root"),
            ),
            RadrootsNip22CommentRoot::from(
                RadrootsNip22AddressRootReference::parse(
                    format!("31922:{}:market", h('b')),
                    Some("wss://root.example"),
                )
                .expect("address root"),
            ),
        ] {
            let comment = RadrootsAuthoredNip22Comment::nested(
                "Reply",
                root,
                parent(Some("wss://parent.example")),
            )
            .expect("comment");
            let tags = authored_nip22_comment_to_wire_parts(&comment).tags;
            assert_eq!(tags.len(), 6);
            assert_eq!(tags[4], vec!["k".to_string(), KIND_COMMENT.to_string()]);
            assert_eq!(
                tags[5],
                vec!["p".to_string(), h('d'), "wss://parent.example".to_string()]
            );
        }

        assert_eq!(KIND_CALENDAR_DATE_EVENT, 31_922);
    }

    #[test]
    fn core_wire_estimator_matches_emitted_json_for_all_authored_shapes() {
        for shape in [
            AuthoredShape::TopEvent,
            AuthoredShape::TopAddress,
            AuthoredShape::NestedEvent,
            AuthoredShape::NestedAddress,
        ] {
            let mut lower = 1usize;
            let mut upper = RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES;
            while lower < upper {
                let candidate = lower + (upper - lower).div_ceil(2);
                if authored_shape(shape, "\u{0001}".repeat(candidate)).is_ok() {
                    lower = candidate;
                } else {
                    upper = candidate - 1;
                }
            }

            let exact = authored_shape(shape, "\u{0001}".repeat(lower))
                .expect("largest content fitting the signed wire");
            let parts = authored_nip22_comment_to_wire_parts(&exact);
            let exact_json = serde_json::json!({
                "id": "0".repeat(64),
                "pubkey": "0".repeat(64),
                "created_at": u64::MAX,
                "kind": KIND_COMMENT,
                "tags": &parts.tags,
                "content": exact.content(),
                "sig": "0".repeat(128),
            })
            .to_string();
            assert!(exact_json.len() <= RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES);

            let overflow_content = "\u{0001}".repeat(lower + 1);
            let error = authored_shape(shape, overflow_content.clone())
                .expect_err("one more escaped byte must cross the signed wire limit");
            let overflow_json = serde_json::json!({
                "id": "0".repeat(64),
                "pubkey": "0".repeat(64),
                "created_at": u64::MAX,
                "kind": KIND_COMMENT,
                "tags": &parts.tags,
                "content": overflow_content,
                "sig": "0".repeat(128),
            })
            .to_string();
            assert!(matches!(
                error,
                RadrootsNip22CommentError::EventWireTooLarge { max, actual }
                    if max == RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES
                        && actual == overflow_json.len()
            ));
        }
    }
}
