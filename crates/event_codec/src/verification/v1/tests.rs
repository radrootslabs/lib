use super::*;
use radroots_event::RadrootsEventEnvelopeParts;

#[test]
fn id_verification_returns_the_exact_envelope() {
    let event = signed_max_kind_event();
    let verified = verify_event_id(event.clone()).expect("canonical event id");

    assert_eq!(verified.event(), &event);
    assert_eq!(verified.into_event(), event);
}

#[cfg(feature = "nostr")]
#[test]
fn signature_verification_returns_the_exact_envelope() {
    let event = signed_max_kind_event();
    let verified = verify_nip01_event(event.clone()).expect("valid Schnorr signature");

    assert_eq!(verified.event(), &event);
    assert_eq!(verified.into_event(), event);
}

#[cfg(not(feature = "nostr"))]
#[test]
fn signature_verification_reports_unavailable_without_nostr() {
    let event = verify_event_id(signed_max_kind_event()).expect("canonical event id");

    assert_eq!(
        verify_event_signature(event),
        Err(RadrootsNip01VerificationError::SignatureVerificationUnavailable)
    );
}

#[test]
fn verification_error_codes_are_stable() {
    let errors = [
        (
            RadrootsNip01VerificationError::MalformedEnvelope,
            "malformed_envelope",
        ),
        (
            RadrootsNip01VerificationError::KindOutOfRange { kind: 65_536 },
            "kind_out_of_range",
        ),
        (
            RadrootsNip01VerificationError::IdMismatch {
                expected: "expected".to_string(),
                actual: "actual".to_string(),
            },
            "id_mismatch",
        ),
        (
            RadrootsNip01VerificationError::SignatureInvalid,
            "signature_invalid",
        ),
        (
            RadrootsNip01VerificationError::SignatureVerificationUnavailable,
            "signature_verification_unavailable",
        ),
    ];

    for (error, expected) in errors {
        assert_eq!(error.code(), expected);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn id_verification_rejects_an_out_of_range_kind_before_hashing() {
    let original = signed_max_kind_event();
    let kind = u32::from(u16::MAX) + 1;
    let id = compute_canonical_nip01_event_id_v1(
        &original.author().to_hex(),
        original.created_at_u64(),
        kind,
        &original.tags_as_vec(),
        original.content(),
    )
    .expect("canonical hash remains mechanically computable")
    .into_string();
    let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id,
        author: original.author().to_hex().to_owned(),
        created_at: original.created_at_u64(),
        kind,
        tags: original.tags_as_vec(),
        content: original.content().to_owned(),
        sig: original.signature_hex(),
    })
    .expect("base envelope permits the wider internal kind representation");

    assert_eq!(
        verify_event_id(event),
        Err(RadrootsNip01VerificationError::KindOutOfRange { kind })
    );
}

fn signed_max_kind_event() -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: "a07878757d705d3cd848b9264791d699069068a5f0a575112f351367b0987958"
                .to_string(),
            author: "1b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"
                .to_string(),
            created_at: 1_800_000_104,
            kind: u32::from(u16::MAX),
            tags: Vec::new(),
            content: "maximum-kind".to_string(),
            sig: "d79b19843a0bfd769c02c73866d44a3a06f7b11e107a5257971b60e700aa25565802fd3a7eed4042fe8db7d709a465e5f61478eb8291178831bf48f6b0980671"
                .to_string(),
        })
        .expect("valid event envelope")
}
