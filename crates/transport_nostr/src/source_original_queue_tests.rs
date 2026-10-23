use super::*;

#[test]
fn pending_count_refuses_next_entry_without_evicting_originals() {
    let mut queue = OriginalQueue::default();
    let baseline = queue.retained_bytes();
    for i in 0..MAX_PENDING_EVENTS {
        assert!(queue.insert(1, i as u64, [0; 32], "synthetic original"));
    }
    let exact = queue.retained_bytes();
    assert_eq!(
        exact,
        baseline + MAX_PENDING_EVENTS * "synthetic original".len()
    );
    assert!(!queue.insert(2, 999, [0; 32], "next"));
    assert_eq!(queue.retained_bytes(), exact);
    assert_eq!(
        queue.take(0, [0; 32]).as_deref(),
        Some("synthetic original")
    );
    assert!(queue.insert(2, 999, [0; 32], "next"));
}

#[test]
fn complete_byte_bound_includes_reserved_entries_and_releases_only_taken_or_owned_data() {
    let mut queue = OriginalQueue::default();
    let baseline = queue.retained_bytes();
    let mut index = 0;
    while MAX_PENDING_BYTES - queue.retained_bytes() > MAX_ORIGINAL_EVENT_BYTES {
        assert!(queue.insert(1, index, [0; 32], &"a".repeat(MAX_ORIGINAL_EVENT_BYTES)));
        index += 1;
    }
    let remaining = MAX_PENDING_BYTES - queue.retained_bytes();
    assert!(queue.insert(2, index, [0; 32], &"b".repeat(remaining)));
    assert_eq!(queue.retained_bytes(), MAX_PENDING_BYTES);
    assert!(!queue.insert(3, index + 1, [0; 32], "x"));
    assert_eq!(queue.retained_bytes(), MAX_PENDING_BYTES);
    assert_eq!(queue.take(index, [0; 32]).unwrap().len(), remaining);
    assert_eq!(queue.retained_bytes(), MAX_PENDING_BYTES - remaining);
    queue.remove_owner(1);
    assert_eq!(queue.retained_bytes(), baseline);
    assert!(queue.entries.is_empty());
    assert!(!queue.insert(1, 1, [0; 32], &"a".repeat(MAX_ORIGINAL_EVENT_BYTES + 1)));
    assert_eq!(queue.retained_bytes(), baseline);
}

#[test]
fn correlation_is_scoped_fifo_and_never_reconstructs_original_spelling() {
    let mut queue = OriginalQueue::default();
    assert!(queue.insert(1, 10, [1; 32], "{ \"a\": 1 }"));
    assert!(queue.insert(1, 10, [1; 32], "{\"a\":1}"));
    assert!(queue.insert(2, 20, [1; 32], "another scope"));
    assert!(queue.take(10, [2; 32]).is_none());
    assert!(queue.take(99, [1; 32]).is_none());
    assert_eq!(queue.take(10, [1; 32]).as_deref(), Some("{ \"a\": 1 }"));
    queue.remove_scope(10);
    assert!(queue.take(10, [1; 32]).is_none());
    assert_eq!(queue.take(20, [1; 32]).as_deref(), Some("another scope"));
    assert!(!format!("{queue:?}").contains("another scope"));
}

#[test]
fn raw_frame_projection_preserves_object_bytes_and_bounds_header_decoding() {
    let raw = "[\"EV\\u0045NT\",\"a\\u0062\", { \"unknown\": [1, 2] }]";
    let (subscription, value) = frame(raw).unwrap();
    assert_eq!(subscription, "ab");
    assert_eq!(value.get(), "{ \"unknown\": [1, 2] }");
    for raw in [
        "[]".into(),
        "{}".into(),
        "[\"EVENT\",\"a\"]".into(),
        "[\"EVENT\",\"\",{}]".into(),
        "[\"EOSE\",\"a\",{}]".into(),
        format!("[\"{}\",\"a\",{{}}]", "E".repeat(33)),
        format!("[\"EVENT\",\"{}\",{{}}]", "a".repeat(6 * 256 + 1)),
        format!("[\"EVENT\",\"{}\",{{}}]", "a".repeat(257)),
        format!(
            "[\"EVENT\",\"a\",{{\"large\":\"{}\"}}]",
            "a".repeat(MAX_ORIGINAL_EVENT_BYTES)
        ),
        "x".repeat(crate::session_socket::MAX_WIRE_MESSAGE_BYTES + 1),
    ] {
        assert!(frame(&raw).is_none());
    }
}
