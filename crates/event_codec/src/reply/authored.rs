#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::{
    envelope::kind::KIND_POST,
    post::reply::{RadrootsAuthoredNip10Reply, RadrootsNip10ReplyReference},
    wire::RadrootsNip01EventWireParts,
};

/// Builds deterministic unsigned kind-1 wire parts for a strict marked NIP-10
/// reply.
pub fn authored_nip10_reply_to_wire_parts(
    reply: &RadrootsAuthoredNip10Reply,
) -> RadrootsNip01EventWireParts {
    let mut tags = Vec::with_capacity(if reply.parent().is_some() { 4 } else { 2 });
    tags.push(event_tag(reply.root(), "root"));
    if let Some(parent) = reply.parent() {
        tags.push(event_tag(parent, "reply"));
    }
    tags.push(public_key_tag(reply.root()));
    if let Some(parent) = reply
        .parent()
        .filter(|parent| parent.author() != reply.root().author())
    {
        tags.push(public_key_tag(parent));
    }
    RadrootsNip01EventWireParts {
        kind: KIND_POST,
        content: reply.content().to_string(),
        tags,
    }
}

fn event_tag(reference: &RadrootsNip10ReplyReference, marker: &str) -> Vec<String> {
    vec![
        "e".to_string(),
        reference.event_id().to_hex(),
        reference.relay_or_empty().to_string(),
        marker.to_string(),
    ]
}

fn public_key_tag(reference: &RadrootsNip10ReplyReference) -> Vec<String> {
    vec!["p".to_string(), reference.author().to_hex()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(character: char) -> String {
        crate::test_fixtures::fixture_public_key_hex(character)
    }

    #[test]
    fn nested_reply_deduplicates_equal_reference_authors() {
        let author = h('b');
        let root =
            RadrootsNip10ReplyReference::parse(h('a'), &author, None).expect("root reference");
        let parent =
            RadrootsNip10ReplyReference::parse(h('c'), &author, None).expect("parent reference");
        let reply =
            RadrootsAuthoredNip10Reply::nested("Reply", root, parent).expect("nested reply");

        assert_eq!(
            authored_nip10_reply_to_wire_parts(&reply).tags,
            vec![
                vec!["e".to_string(), h('a'), String::new(), "root".to_string(),],
                vec!["e".to_string(), h('c'), String::new(), "reply".to_string(),],
                vec!["p".to_string(), author],
            ]
        );
    }
}
