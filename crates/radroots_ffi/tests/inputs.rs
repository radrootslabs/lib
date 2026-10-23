use radroots_ffi::{Error, InputFailure, InquiryInput, ListingInput, ListingReference};

#[test]
fn actual_shared_parser_retains_invalid_unsupported_and_limit_failures() {
    assert!(
        matches!(ListingInput::parse(b"{}".to_vec()), Err(Error::InvalidInput { field }) if field == "listing_input")
    );
    for input in [
        br#"{"profile":{"version":2}}"#.as_slice(),
        br#"{"profile":{"profile":"unknown"}}"#,
    ] {
        assert!(
            matches!(ListingInput::parse(input.to_vec()), Err(Error::CommandInput { field, kind: InputFailure::UnsupportedProfile }) if field == "listing_input")
        );
    }
    for input in [
        br#"{"profile":{"profile":"radroots.listing","version":1}}"#.as_slice(),
        br#"{"profile":[]}"#,
    ] {
        assert!(
            matches!(ListingInput::parse(input.to_vec()), Err(Error::InvalidInput { field }) if field == "listing_input")
        );
    }
    assert!(
        matches!(ListingInput::parse(vec![b' '; 65_537]), Err(Error::CommandInput { field, kind: InputFailure::LimitExceeded }) if field == "listing_input")
    );
}

#[test]
fn exact_keys_and_private_text_limits_fail_without_input_disclosure() {
    let peer = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    let secret_text = "private input must not enter the error";
    let error = InquiryInput::new(
        "22".repeat(32),
        peer.to_uppercase(),
        None,
        secret_text.into(),
    )
    .err()
    .unwrap();
    assert!(matches!(&error, Error::InvalidInput { field } if field == "peer"));
    assert!(!format!("{error:?} {error}").contains(secret_text));
    assert!(matches!(
        InquiryInput::new("22".repeat(32), peer.into(), None, "x".repeat(65_537)),
        Err(Error::CommandInput {
            kind: InputFailure::LimitExceeded,
            ..
        })
    ));
}

#[test]
fn untrusted_listing_descriptors_validate_exact_source_fields_without_authority() {
    let peer = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    let reference = ListingReference {
        author: peer.into(),
        discriminator: "11".repeat(16),
        event_id: "44".repeat(32),
    };
    assert!(
        InquiryInput::new(
            "22".repeat(32),
            peer.into(),
            Some(reference.clone()),
            "inquiry".into()
        )
        .is_ok()
    );
    for (changed, field) in [
        (
            ListingReference {
                author: "bad".into(),
                ..reference.clone()
            },
            "listing_author",
        ),
        (
            ListingReference {
                event_id: "BAD".into(),
                ..reference.clone()
            },
            "listing_event_id",
        ),
        (
            ListingReference {
                discriminator: "bad".into(),
                ..reference
            },
            "listing_reference",
        ),
    ] {
        assert!(
            matches!(InquiryInput::new("22".repeat(32), peer.into(), Some(changed), "private text".into()), Err(Error::InvalidInput { field: actual }) if actual == field)
        );
    }
    assert!(
        matches!(InquiryInput::new("invalid".into(), peer.into(), None, "private".into()), Err(Error::InvalidInput { field }) if field == "thread_id")
    );
    assert!(
        matches!(InquiryInput::new("22".repeat(32), "bad".into(), None, "private".into()), Err(Error::InvalidInput { field }) if field == "peer")
    );
}
