#![cfg(feature = "serde_json")]

use radroots_event::kinds::KIND_POST;
use radroots_event_codec::error::EventParseError;

fn assert_invalid_kind<T>(result: Result<T, EventParseError>, expected: &'static str, got: u32) {
    match result {
        Err(EventParseError::InvalidKind {
            expected: found,
            got: actual,
        }) => {
            assert_eq!(found, expected);
            assert_eq!(actual, got);
        }
        Err(other) => panic!("unexpected parse error: {other:?}"),
        Ok(_) => panic!("expected invalid kind"),
    }
}

fn parsed_args() -> (String, String, u64, u32, String, Vec<Vec<String>>, String) {
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        1,
        KIND_POST,
        String::new(),
        Vec::new(),
        concat!(
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        )
        .to_string(),
    )
}

#[test]
fn event_parse_error_duplicate_tag_code_and_display_are_stable() {
    let error = EventParseError::DuplicateTag("d");

    assert_eq!(error.code(), "duplicate_tag");
    assert_eq!(error.to_string(), "duplicate tag: d");
}

#[test]
fn parsed_wrappers_propagate_invalid_kind_errors() {
    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::article::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30023",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::coop::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30360",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::farm_crdt::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "78",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::farm_workspace::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30078",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::file_metadata::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "1063",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::http_auth::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "27235",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::profile::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "0",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::reaction::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "7",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::relay_auth::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "22242",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::report::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "1984",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::repost::decode::repost_parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "6",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::repost::decode::generic_repost_parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "16",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::resource_area::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30370",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::resource_cap::decode::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30371",
        KIND_POST,
    );
}
