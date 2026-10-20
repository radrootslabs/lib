use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CheckpointDigest, CheckpointRef, Coverage, ExecutorId,
    Generation, GrantId, PublicKey, RouteDigest, Sequence, UnixSeconds, WorkspaceId,
};
use radroots_storage::workspace::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceError,
};
fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
fn input() -> AttachmentInput {
    AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        key(),
        "local://workspace/selector",
        key(),
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![],
        )
        .unwrap(),
        RouteDigest::from_bytes([5; 32]),
    )
    .unwrap()
}
fn actor() -> ActorRef {
    ActorRef::new(key(), ActorRole::Buyer, None, Some(input().grant_id()))
}

#[test]
fn proof_has_exact_width_explicit_exposure_and_redacted_diagnostics() {
    for bytes in [&[][..], &[7; 31][..], &[7; 33][..]] {
        assert_eq!(
            AttachmentProof::from_protected_input(bytes).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    let proof = AttachmentProof::from_protected_input(&[7; 32]).unwrap();
    proof.with_secret(|bytes| assert_eq!(bytes, &[7; 32]));
    assert_eq!(format!("{proof:?}"), "AttachmentProof(<redacted>)");
}
#[test]
fn selections_are_bounded_untrusted_data_with_complete_getters() {
    let input = input();
    assert_eq!(input.workspace_id(), WorkspaceId::from_bytes([1; 32]));
    assert_eq!(
        input.authority().executor_id(),
        ExecutorId::from_bytes([2; 32])
    );
    assert_eq!(input.participant(), key());
    assert_eq!(input.endpoint_identity(), key());
    assert_eq!(input.endpoint(), "local://workspace/selector");
    assert_eq!(input.grant_id(), GrantId::from_bytes([3; 32]));
    assert_eq!(input.checkpoint().sequence().get(), 0);
    assert_eq!(input.route_digest(), RouteDigest::from_bytes([5; 32]));
    for endpoint in [String::new(), "x".repeat(2049), "local://bad\n".into()] {
        assert_eq!(
            AttachmentInput::new(
                input.workspace_id(),
                input.authority(),
                input.participant(),
                endpoint,
                input.endpoint_identity(),
                input.grant_id(),
                input.checkpoint().clone(),
                input.route_digest()
            )
            .unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    assert_eq!(
        format!("{input:?}"),
        "AttachmentInput(<untrusted descriptor>)"
    );
}
#[test]
fn host_observations_validate_internal_binding_without_authenticating_metadata() {
    let binding = |actor, principal: &str, revision, expiry, observed| {
        WorkspaceBinding::from_authenticated_host(
            input(),
            radroots_storage::workspace::LocalOutboxScope::NewBuyerOutbox,
            actor,
            principal,
            Sequence::new(revision).unwrap(),
            UnixSeconds::new(expiry).unwrap(),
            UnixSeconds::new(observed).unwrap(),
        )
    };
    let observed = binding(actor(), "device", 1, 200, 100).unwrap();
    assert_eq!(observed.input(), &input());
    assert_eq!(
        observed.scope(),
        radroots_storage::workspace::LocalOutboxScope::NewBuyerOutbox
    );
    assert_eq!(observed.actor(), actor());
    assert_eq!(observed.principal_id(), "device");
    assert_eq!(observed.grant_revision().get(), 1);
    assert_eq!(observed.expires_at().get(), 200);
    assert_eq!(observed.observed_at().get(), 100);
    assert_eq!(format!("{observed:?}"), "WorkspaceBinding(<redacted>)");
    for principal in [String::new(), "x".repeat(129), "device\n".into()] {
        assert_eq!(
            binding(actor(), principal.as_str(), 1, 200, 100).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    for (revision, expiry, observed) in [
        (0, 200, 100),
        (1, 0, 100),
        (1, 100, 0),
        (1, 100, 100),
        (1, 99, 100),
    ] {
        assert_eq!(
            binding(actor(), "device", revision, expiry, observed).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
    let other_key =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
            .unwrap();
    for invalid in [
        ActorRef::new(key(), ActorRole::Seller, None, Some(input().grant_id())),
        ActorRef::new(
            key(),
            ActorRole::Coordinator,
            None,
            Some(input().grant_id()),
        ),
        ActorRef::new(
            key(),
            ActorRole::Buyer,
            Some(key()),
            Some(input().grant_id()),
        ),
        ActorRef::new(other_key, ActorRole::Buyer, None, Some(input().grant_id())),
        ActorRef::new(key(), ActorRole::Buyer, None, None),
        ActorRef::new(
            key(),
            ActorRole::Buyer,
            None,
            Some(GrantId::from_bytes([9; 32])),
        ),
    ] {
        assert_eq!(
            binding(invalid, "device", 1, 200, 100).unwrap_err(),
            WorkspaceError::InvalidInput
        );
    }
}
#[cfg(feature = "serde")]
#[test]
fn selection_json_rejects_missing_duplicate_unknown_and_invalid_data() {
    let input = input();
    let encoded = serde_json::to_string(&input).unwrap();
    assert_eq!(
        serde_json::from_str::<AttachmentInput>(&encoded).unwrap(),
        input
    );
    let value = serde_json::to_value(&input).unwrap();
    for field in value.as_object().unwrap().keys() {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<AttachmentInput>(missing).is_err());
        let duplicate = format!("{{\"{field}\":{},{}", value[field], &encoded[1..]);
        assert!(serde_json::from_str::<AttachmentInput>(&duplicate).is_err());
    }
    let mut unknown = value.clone();
    unknown["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AttachmentInput>(unknown).is_err());
    for invalid in ["", "bad\n"] {
        let mut data = value.clone();
        data["endpoint"] = serde_json::json!(invalid);
        assert!(serde_json::from_value::<AttachmentInput>(data).is_err());
    }
}
#[test]
fn workspace_failures_have_fixed_bounded_codes_and_no_sources() {
    use WorkspaceError::*;
    for error in [
        InvalidInput,
        Unavailable,
        NamespaceUnavailable,
        WriterActive,
        WrongNamespace,
        RecoveryRequired,
        Unauthorized,
        GrantExpired,
        GrantRevoked,
        StaleAuthority,
        StaleCheckpoint,
        Closed,
        StorageFailure,
        CommitUnknown,
    ] {
        assert_eq!(error.to_string(), error.code());
        assert!(error.code().len() <= 32);
        let source: &dyn core::error::Error = &error;
        assert!(source.source().is_none());
    }
}
