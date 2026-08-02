use radroots_storage::{
    Error,
    private_artifact::{
        ArtifactCommitment, ArtifactKind, ArtifactSchemaId, DeletionReason, DurableSecretReference,
        PrivateArtifactId, PrivateArtifactMetadata, PrivateArtifactRevision, PrivateArtifactStage,
        PrivateArtifactStore, RetentionPolicy,
    },
};

fn metadata(retention: RetentionPolicy) -> PrivateArtifactMetadata {
    PrivateArtifactMetadata::new(
        PrivateArtifactId::new([1; 16]).expect("artifact id"),
        ArtifactKind::parse("trade.private_terms").expect("artifact kind"),
        ArtifactSchemaId::parse("trade.private_terms.v1").expect("schema id"),
        ArtifactCommitment::new([2; 32]),
        512,
        DurableSecretReference::new("keyring", "opaque-key-token", 3).expect("secret reference"),
        retention,
        100,
    )
    .expect("metadata")
}

#[test]
fn retention_distinguishes_expiry_from_earliest_deletion() {
    let policy = RetentionPolicy::new(Some(400), Some(300)).expect("retention policy");
    assert!(!policy.is_expired_at(299));
    assert!(policy.is_expired_at(300));
    assert!(!policy.permits_deletion_at(399));
    assert!(policy.permits_deletion_at(400));
    assert_eq!(
        RetentionPolicy::new(Some(0), None),
        Err(Error::InvalidPrivateArtifactRetention)
    );
}

#[test]
fn expiry_is_revision_bound_monotonic_and_policy_driven() {
    let active = metadata(RetentionPolicy::new(None, Some(300)).expect("retention"));
    assert_eq!(
        active.mark_expired(PrivateArtifactRevision::INITIAL, 299),
        Err(Error::PrivateArtifactNotExpired)
    );
    let expired = active
        .mark_expired(PrivateArtifactRevision::INITIAL, 300)
        .expect("expired metadata");
    assert_eq!(expired.stage(), PrivateArtifactStage::Expired);
    assert_eq!(expired.revision().get(), 2);
    assert_eq!(
        expired.mark_expired(PrivateArtifactRevision::INITIAL, 301),
        Err(Error::PrivateArtifactRevisionConflict)
    );
}

#[test]
fn tombstones_preserve_commitment_and_enforce_retention() {
    let active = metadata(RetentionPolicy::new(Some(400), Some(300)).expect("retention"));
    let expired = active
        .mark_expired(PrivateArtifactRevision::INITIAL, 300)
        .expect("expired metadata");
    assert_eq!(
        expired.tombstone(expired.revision(), 399, DeletionReason::RetentionExpired,),
        Err(Error::PrivateArtifactRetentionActive)
    );
    let deleted = expired
        .tombstone(expired.revision(), 400, DeletionReason::RetentionExpired)
        .expect("tombstone");
    assert_eq!(deleted.stage(), PrivateArtifactStage::Tombstoned);
    let tombstone = deleted.tombstone_record().expect("tombstone record");
    assert_eq!(tombstone.commitment(), active.commitment());
    assert_eq!(tombstone.deleted_at_unix_ms(), 400);
    assert_eq!(tombstone.reason(), DeletionReason::RetentionExpired);
    assert_eq!(
        deleted.tombstone(deleted.revision(), 401, DeletionReason::OperatorRequested,),
        Err(Error::PrivateArtifactTombstoned)
    );
}

#[test]
#[cfg(feature = "serde")]
fn secret_references_are_bounded_redacted_and_round_trip() {
    let reference =
        DurableSecretReference::new("keyring", "opaque-key-token", 3).expect("secret reference");
    let diagnostic = format!("{reference:?}");
    assert!(diagnostic.contains("[REDACTED]"));
    assert!(!diagnostic.contains("opaque-key-token"));

    let encoded = serde_json::to_string(&reference).expect("reference JSON");
    let decoded: DurableSecretReference =
        serde_json::from_str(encoded.as_str()).expect("reference round trip");
    assert_eq!(decoded, reference);
    assert!(
        serde_json::from_str::<DurableSecretReference>(
            r#"{"provider":"keyring","opaque_reference":"token","key_version":0}"#,
        )
        .is_err()
    );
    assert_eq!(
        DurableSecretReference::new("keyring", "token", 0),
        Err(Error::InvalidPrivateArtifactSecretReference)
    );
}

#[test]
fn metadata_contains_only_protected_size_commitment_and_reference() {
    fn accepts_dyn(_: Option<&dyn PrivateArtifactStore>) {}
    accepts_dyn(None);

    let value = metadata(RetentionPolicy::indefinite());
    assert_eq!(value.protected_size_bytes(), 512);
    assert_eq!(value.secret_reference().provider(), "keyring");
    assert_eq!(value.secret_reference().key_version(), 3);
    assert_eq!(
        PrivateArtifactId::new([0; 16]),
        Err(Error::InvalidPrivateArtifactId)
    );
    assert_eq!(
        ArtifactKind::parse("Not Canonical"),
        Err(Error::InvalidPrivateArtifactKind)
    );
}
