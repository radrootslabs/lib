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

#[test]
fn private_artifact_value_matrix_covers_every_bound_and_accessor() {
    let id = PrivateArtifactId::new([1; 16]).expect("id");
    assert_eq!(id.as_bytes(), &[1; 16]);
    for value in ["", "Uppercase", " leading", "trailing ", "bad/slash"] {
        assert_eq!(
            ArtifactKind::parse(value),
            Err(Error::InvalidPrivateArtifactKind)
        );
        assert_eq!(
            ArtifactSchemaId::parse(value),
            Err(Error::InvalidPrivateArtifactSchema)
        );
    }
    assert_eq!(
        ArtifactKind::parse(
            "x".repeat(radroots_storage::private_artifact::ARTIFACT_KIND_MAX_BYTES + 1)
        ),
        Err(Error::InvalidPrivateArtifactKind)
    );
    assert_eq!(
        ArtifactSchemaId::parse(
            "x".repeat(radroots_storage::private_artifact::ARTIFACT_SCHEMA_MAX_BYTES + 1)
        ),
        Err(Error::InvalidPrivateArtifactSchema)
    );
    let kind = ArtifactKind::parse("trade.private_terms").unwrap();
    let schema = ArtifactSchemaId::parse("trade.private_terms.v1").unwrap();
    assert_eq!(kind.as_str(), "trade.private_terms");
    assert_eq!(schema.as_str(), "trade.private_terms.v1");
    let commitment = ArtifactCommitment::new([2; 32]);
    assert_eq!(commitment.as_bytes(), &[2; 32]);

    for (provider, reference, version) in [
        ("", "token", 1),
        ("Bad", "token", 1),
        ("keyring", "", 1),
        ("keyring", " token", 1),
        ("keyring", "token ", 1),
        ("keyring", "bad\ntoken", 1),
        ("keyring", "token", 0),
    ] {
        assert_eq!(
            DurableSecretReference::new(provider, reference, version),
            Err(Error::InvalidPrivateArtifactSecretReference)
        );
    }
    assert_eq!(
        DurableSecretReference::new(
            "x".repeat(radroots_storage::private_artifact::SECRET_PROVIDER_MAX_BYTES + 1),
            "token",
            1,
        ),
        Err(Error::InvalidPrivateArtifactSecretReference)
    );
    assert_eq!(
        DurableSecretReference::new(
            "keyring",
            "x".repeat(radroots_storage::private_artifact::SECRET_REFERENCE_MAX_BYTES + 1),
            1,
        ),
        Err(Error::InvalidPrivateArtifactSecretReference)
    );
    let secret = DurableSecretReference::new("keyring", "opaque", 1).unwrap();
    assert_eq!(secret.provider(), "keyring");
    assert_eq!(secret.opaque_reference(), "opaque");
    assert_eq!(secret.key_version(), 1);

    assert_eq!(
        RetentionPolicy::new(None, Some(0)),
        Err(Error::InvalidPrivateArtifactRetention)
    );
    let retention = RetentionPolicy::new(Some(200), Some(150)).unwrap();
    assert_eq!(retention.delete_not_before_unix_ms(), Some(200));
    assert_eq!(retention.expires_at_unix_ms(), Some(150));
    assert!(!retention.is_expired_at(149));
    assert!(retention.is_expired_at(150));
    assert!(!retention.permits_deletion_at(199));
    assert!(retention.permits_deletion_at(200));
    let indefinite = RetentionPolicy::indefinite();
    assert!(!indefinite.is_expired_at(u64::MAX));
    assert!(indefinite.permits_deletion_at(0));
    assert_eq!(
        PrivateArtifactRevision::new(0),
        Err(Error::InvalidPrivateArtifactRevision)
    );
    assert_eq!(PrivateArtifactRevision::new(2).unwrap().get(), 2);

    let value = metadata(retention);
    assert_eq!(value.artifact_id(), id);
    assert_eq!(value.kind(), &kind);
    assert_eq!(value.schema_id(), &schema);
    assert_eq!(value.commitment(), commitment);
    assert_eq!(value.protected_size_bytes(), 512);
    assert_eq!(
        value.secret_reference().opaque_reference(),
        "opaque-key-token"
    );
    assert_eq!(value.retention(), retention);
    assert_eq!(value.revision(), PrivateArtifactRevision::INITIAL);
    assert_eq!(value.stage(), PrivateArtifactStage::Active);
    assert_eq!(value.created_at_unix_ms(), 100);
    assert_eq!(value.updated_at_unix_ms(), 100);
    assert!(value.tombstone_record().is_none());
}

#[test]
fn metadata_construction_and_durable_state_fail_closed() {
    let id = PrivateArtifactId::new([1; 16]).unwrap();
    let kind = ArtifactKind::parse("trade.private_terms").unwrap();
    let schema = ArtifactSchemaId::parse("trade.private_terms.v1").unwrap();
    let commitment = ArtifactCommitment::new([2; 32]);
    let secret = DurableSecretReference::new("keyring", "opaque", 1).unwrap();
    for (size, created, retention) in [
        (0, 100, RetentionPolicy::indefinite()),
        (1, 0, RetentionPolicy::indefinite()),
        (1, 100, RetentionPolicy::new(Some(99), None).unwrap()),
        (1, 100, RetentionPolicy::new(None, Some(99)).unwrap()),
    ] {
        assert_eq!(
            PrivateArtifactMetadata::new(
                id,
                kind.clone(),
                schema.clone(),
                commitment,
                size,
                secret.clone(),
                retention,
                created,
            ),
            Err(Error::InvalidPrivateArtifactMetadata)
        );
    }

    let retention = RetentionPolicy::new(Some(200), Some(150)).unwrap();
    let durable = |revision, stage, updated, tombstone| {
        PrivateArtifactMetadata::from_durable_parts(
            id,
            kind.clone(),
            schema.clone(),
            commitment,
            1,
            secret.clone(),
            retention,
            revision,
            stage,
            100,
            updated,
            tombstone,
        )
    };
    assert!(
        durable(
            PrivateArtifactRevision::INITIAL,
            PrivateArtifactStage::Active,
            100,
            None
        )
        .is_ok()
    );
    assert!(
        durable(
            PrivateArtifactRevision::new(2).unwrap(),
            PrivateArtifactStage::Expired,
            150,
            None
        )
        .is_ok()
    );
    assert!(
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            200,
            Some((200, DeletionReason::RetentionExpired, commitment)),
        )
        .is_ok()
    );
    for result in [
        durable(
            PrivateArtifactRevision::INITIAL,
            PrivateArtifactStage::Active,
            99,
            None,
        ),
        durable(
            PrivateArtifactRevision::new(2).unwrap(),
            PrivateArtifactStage::Active,
            100,
            None,
        ),
        durable(
            PrivateArtifactRevision::new(2).unwrap(),
            PrivateArtifactStage::Expired,
            149,
            None,
        ),
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            200,
            None,
        ),
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            200,
            Some((199, DeletionReason::UserRequested, commitment)),
        ),
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            200,
            Some((
                200,
                DeletionReason::UserRequested,
                ArtifactCommitment::new([9; 32]),
            )),
        ),
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            199,
            Some((199, DeletionReason::UserRequested, commitment)),
        ),
        durable(
            PrivateArtifactRevision::new(3).unwrap(),
            PrivateArtifactStage::Tombstoned,
            149,
            Some((149, DeletionReason::RetentionExpired, commitment)),
        ),
    ] {
        assert_eq!(result, Err(Error::CorruptPrivateArtifactMetadata));
    }
}

#[test]
fn transition_and_status_edge_matrix_is_complete() {
    let active = metadata(RetentionPolicy::new(Some(200), Some(150)).unwrap());
    assert_eq!(
        active.mark_expired(PrivateArtifactRevision::new(2).unwrap(), 150),
        Err(Error::PrivateArtifactRevisionConflict)
    );
    assert_eq!(
        active.mark_expired(PrivateArtifactRevision::INITIAL, 99),
        Err(Error::InvalidPrivateArtifactTimestamp)
    );
    assert_eq!(
        active.tombstone(
            PrivateArtifactRevision::INITIAL,
            150,
            DeletionReason::RetentionExpired,
        ),
        Err(Error::PrivateArtifactRetentionActive)
    );
    let direct = active
        .tombstone(
            PrivateArtifactRevision::INITIAL,
            200,
            DeletionReason::UserRequested,
        )
        .unwrap();
    assert_eq!(direct.revision().get(), 2);
    assert_eq!(direct.stage(), PrivateArtifactStage::Tombstoned);
    let tombstone = direct.tombstone_record().unwrap();
    assert_eq!(tombstone.deleted_at_unix_ms(), 200);
    assert_eq!(tombstone.reason(), DeletionReason::UserRequested);
    assert_eq!(tombstone.commitment(), active.commitment());

    assert_eq!(
        radroots_storage::private_artifact::PrivateArtifactStatus {
            active: 1,
            expired: 2,
            tombstoned: 3,
        }
        .total(),
        Some(6)
    );
    assert_eq!(
        radroots_storage::private_artifact::PrivateArtifactStatus {
            active: u64::MAX,
            expired: 1,
            tombstoned: 0,
        }
        .total(),
        None
    );
    assert_eq!(
        radroots_storage::private_artifact::PrivateArtifactStatus {
            active: 0,
            expired: u64::MAX,
            tombstoned: 1,
        }
        .total(),
        None
    );
}
