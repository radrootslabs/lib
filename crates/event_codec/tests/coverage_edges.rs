#![cfg(feature = "json")]

use radroots_event::envelope::kind::KIND_POST;
use radroots_event_codec::decode::EventParseError;

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
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_string(),
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
        radroots_event_codec::decode::article::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30023",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::coop::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30360",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::farm_crdt::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "78",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::farm_workspace::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30078",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::file_metadata::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "1063",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::http_auth::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "27235",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::profile::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "0",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::reaction::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "7",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::relay_auth::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "22242",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::report::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "1984",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::repost::repost_parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "6",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::repost::generic_repost_parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "16",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::resource_area::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30370",
        KIND_POST,
    );

    let (id, author, created_at, kind, content, tags, sig) = parsed_args();
    assert_invalid_kind(
        radroots_event_codec::decode::resource_cap::parsed_from_event(
            id, author, created_at, kind, content, tags, sig,
        ),
        "30371",
        KIND_POST,
    );
}
