use radroots_events::kinds::KIND_POST;
use radroots_events_codec::wire::{WireEventParts, canonicalize_tags, empty_content, to_draft};

#[test]
fn canonicalize_tags_trims_sorts_and_dedups() {
    let mut tags = vec![
        vec![" z ".to_string(), "b".to_string()],
        vec!["t".to_string(), "a".to_string()],
        vec!["".to_string(), "x".to_string()],
        vec![" t ".to_string(), "a ".to_string()],
        vec!["t".to_string(), "a".to_string()],
    ];

    canonicalize_tags(&mut tags);

    assert_eq!(
        tags,
        vec![
            vec!["t".to_string(), "a".to_string()],
            vec!["z".to_string(), "b".to_string()],
        ]
    );
}

#[test]
fn to_draft_copies_fields() {
    let parts = WireEventParts {
        kind: KIND_POST,
        content: "hello".to_string(),
        tags: vec![vec!["t".to_string(), "a".to_string()]],
    };

    let draft = to_draft(parts, "author", 99);

    assert_eq!(draft.kind, KIND_POST);
    assert_eq!(draft.created_at, 99);
    assert_eq!(draft.author, "author");
    assert_eq!(draft.content, "hello");
    assert_eq!(draft.tags.len(), 1);
}

#[test]
fn empty_content_is_empty_string() {
    let content = empty_content();
    assert!(content.is_empty());
}
