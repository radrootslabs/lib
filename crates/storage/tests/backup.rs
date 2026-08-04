use radroots_storage::{
    Error,
    backup::{
        BackupFormatVersion, BackupId, BackupManifest, BackupMember, BackupMemberKind,
        BackupOperation, BackupPlan, BackupSecretPolicy, BackupStage, BackupTransition,
        MemberDigest, MemberVerification, ReliabilityRevision, RestoreMemberStatus,
        RestoreOperation, RestorePlan, RestoreStage, RestoreTransition, StorageReliability,
    },
    status::{
        IntegrityHealth, IntegrityStatus, ShutdownState, StorageBackend, StorageOpenMode,
        StorageStatus, WriterPolicy,
    },
};

fn member(path: &str, kind: BackupMemberKind, byte: u8) -> BackupMember {
    BackupMember::new(path, kind, 100, MemberDigest::new([byte; 32])).expect("backup member")
}

fn manifest(policy: BackupSecretPolicy) -> BackupManifest {
    let mut members = vec![member(
        "runtime/runtime.sqlite",
        BackupMemberKind::Runtime,
        1,
    )];
    if policy == BackupSecretPolicy::IncludeProtectedStorage {
        members.push(member(
            "private/private.sqlite",
            BackupMemberKind::Protected,
            2,
        ));
    }
    BackupManifest::new(
        BackupFormatVersion::V1,
        BackupId::new([7; 16]).expect("backup id"),
        100,
        policy,
        members,
    )
    .expect("backup manifest")
}

#[test]
fn manifests_reject_unsafe_duplicate_and_policy_violating_members() {
    assert_eq!(
        BackupMember::new(
            "../runtime.sqlite",
            BackupMemberKind::Runtime,
            1,
            MemberDigest::new([1; 32]),
        ),
        Err(Error::InvalidBackupMemberPath)
    );
    let runtime = member("runtime.sqlite", BackupMemberKind::Runtime, 1);
    assert_eq!(
        BackupManifest::new(
            BackupFormatVersion::V1,
            BackupId::new([1; 16]).expect("id"),
            1,
            BackupSecretPolicy::ExcludeProtectedStorage,
            vec![runtime.clone(), runtime],
        ),
        Err(Error::DuplicateBackupMember)
    );
    assert_eq!(
        BackupManifest::new(
            BackupFormatVersion::V1,
            BackupId::new([1; 16]).expect("id"),
            1,
            BackupSecretPolicy::ExcludeProtectedStorage,
            vec![member("private.sqlite", BackupMemberKind::Protected, 2)],
        ),
        Err(Error::BackupSecretPolicyViolation)
    );
}

#[test]
fn backup_requires_capture_verification_before_atomic_finalization() {
    let plan = BackupPlan::new(
        BackupId::new([7; 16]).expect("backup id"),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        90,
    )
    .expect("backup plan");
    let planned = BackupOperation::planned(plan);
    assert_eq!(
        planned.transition(
            ReliabilityRevision::INITIAL,
            BackupTransition::Finalize,
            100,
        ),
        Err(Error::InvalidBackupTransition)
    );
    let captured = planned
        .transition(
            ReliabilityRevision::INITIAL,
            BackupTransition::Captured(manifest(BackupSecretPolicy::IncludeProtectedStorage)),
            100,
        )
        .expect("captured backup");
    let verified = captured
        .transition(captured.revision(), BackupTransition::Verified, 110)
        .expect("verified backup");
    let finalized = verified
        .transition(verified.revision(), BackupTransition::Finalize, 120)
        .expect("finalized backup");
    assert_eq!(finalized.stage(), BackupStage::Finalized);
    assert_eq!(finalized.manifest().expect("manifest").total_bytes(), 200);
    assert_eq!(
        finalized.transition(finalized.revision(), BackupTransition::Fail, 130),
        Err(Error::ReliabilityOperationTerminal)
    );
}

#[test]
fn restore_stages_and_verifies_every_member_before_replacement() {
    let manifest = manifest(BackupSecretPolicy::IncludeProtectedStorage);
    assert_eq!(
        RestorePlan::new(
            manifest.clone(),
            BackupSecretPolicy::ExcludeProtectedStorage,
            200,
        ),
        Err(Error::BackupSecretPolicyViolation)
    );
    let plan = RestorePlan::new(
        manifest.clone(),
        BackupSecretPolicy::IncludeProtectedStorage,
        200,
    )
    .expect("restore plan");
    let staging = RestoreOperation::staging(plan);
    let verifying = staging
        .transition(ReliabilityRevision::INITIAL, RestoreTransition::Staged, 210)
        .expect("staged restore");
    let failed_evidence = vec![
        RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified)
            .expect("status"),
        RestoreMemberStatus::new("private/private.sqlite", MemberVerification::HashMismatch)
            .expect("status"),
    ];
    assert_eq!(
        verifying.transition(
            verifying.revision(),
            RestoreTransition::Verified(failed_evidence),
            220,
        ),
        Err(Error::RestoreMemberVerificationFailed)
    );
    let verified = manifest
        .members()
        .iter()
        .map(|member| {
            RestoreMemberStatus::new(member.relative_path(), MemberVerification::Verified)
                .expect("verified status")
        })
        .collect();
    let finalizing = verifying
        .transition(
            verifying.revision(),
            RestoreTransition::Verified(verified),
            220,
        )
        .expect("verified restore");
    let finalized = finalizing
        .transition(finalizing.revision(), RestoreTransition::Finalize, 230)
        .expect("finalized restore");
    assert_eq!(finalized.stage(), RestoreStage::Finalized);
}

#[test]
fn integrity_and_storage_status_reject_inconsistent_runtime_claims() {
    let integrity =
        IntegrityStatus::new(IntegrityHealth::Healthy, Some(100), 2, 0).expect("integrity status");
    let status = StorageStatus::new(
        StorageBackend::Sqlite,
        StorageOpenMode::ReadWriteExisting,
        WriterPolicy::AdvisoryProcessLock,
        ShutdownState::Open,
        integrity,
        true,
        5_000,
    )
    .expect("storage status");
    assert!(status.wal_enabled());
    assert_eq!(status.busy_timeout_ms(), 5_000);
    assert_eq!(
        IntegrityStatus::new(IntegrityHealth::Healthy, Some(100), 1, 1),
        Err(Error::InvalidIntegrityStatus)
    );
    assert_eq!(
        StorageStatus::new(
            StorageBackend::Sqlite,
            StorageOpenMode::ReadWriteExisting,
            WriterPolicy::NoWriter,
            ShutdownState::Open,
            integrity,
            true,
            5_000,
        ),
        Err(Error::InvalidStorageStatus)
    );
}

#[test]
fn reliability_spi_is_dyn_compatible_and_versions_are_independent() {
    fn accepts_dyn(_: Option<&dyn StorageReliability>) {}
    accepts_dyn(None);
    assert_eq!(BackupFormatVersion::V1.get(), 1);
    assert_eq!(
        BackupFormatVersion::new(0),
        Err(Error::InvalidBackupVersion)
    );
    assert_eq!(BackupId::new([0; 16]), Err(Error::InvalidBackupId));
}

#[test]
fn backup_json_cannot_bypass_constructor_invariants() {
    let valid = serde_json::to_value(manifest(BackupSecretPolicy::ExcludeProtectedStorage))
        .expect("serialize manifest");

    let mut unsafe_path = valid.clone();
    unsafe_path["members"][0]["relative_path"] = serde_json::json!("../runtime.sqlite");
    assert!(serde_json::from_value::<BackupManifest>(unsafe_path).is_err());

    let mut forged_total = valid.clone();
    forged_total["total_bytes"] = serde_json::json!(101);
    assert!(serde_json::from_value::<BackupManifest>(forged_total).is_err());

    let mut duplicate = valid.clone();
    duplicate["members"] =
        serde_json::json!([valid["members"][0].clone(), valid["members"][0].clone()]);
    duplicate["total_bytes"] = serde_json::json!(200);
    assert!(serde_json::from_value::<BackupManifest>(duplicate).is_err());

    assert!(serde_json::from_str::<BackupId>("[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]").is_err());
    assert!(serde_json::from_str::<BackupFormatVersion>("0").is_err());
    assert!(serde_json::from_str::<ReliabilityRevision>("0").is_err());
}

#[test]
fn restore_json_cannot_bypass_policy_or_timestamp_invariants() {
    let restore = RestorePlan::new(
        manifest(BackupSecretPolicy::IncludeProtectedStorage),
        BackupSecretPolicy::IncludeProtectedStorage,
        200,
    )
    .expect("restore plan");
    let valid = serde_json::to_value(restore).expect("serialize restore plan");

    let mut rejected_policy = valid.clone();
    rejected_policy["accepted_secret_policy"] = serde_json::json!("exclude_protected_storage");
    assert!(serde_json::from_value::<RestorePlan>(rejected_policy).is_err());

    let mut zero_timestamp = valid;
    zero_timestamp["requested_at_unix_ms"] = serde_json::json!(0);
    assert!(serde_json::from_value::<RestorePlan>(zero_timestamp).is_err());

    let unsafe_status = serde_json::json!({
        "relative_path": "runtime/../private.sqlite",
        "verification": "verified"
    });
    assert!(serde_json::from_value::<RestoreMemberStatus>(unsafe_status).is_err());
}

#[test]
fn backup_value_models_cover_all_bounds_and_accessors() {
    let backup_id = BackupId::new([1; 16]).unwrap();
    assert_eq!(backup_id.as_bytes(), &[1; 16]);
    let digest = MemberDigest::new([2; 32]);
    assert_eq!(digest.as_bytes(), &[2; 32]);
    for path in [
        "",
        "/absolute",
        "../escape",
        "a/../b",
        "a/./b",
        "a//b",
        "a\\b",
        " leading",
        "bad\npath",
    ] {
        assert_eq!(
            BackupMember::new(path, BackupMemberKind::Runtime, 1, digest),
            Err(Error::InvalidBackupMemberPath)
        );
        assert_eq!(
            RestoreMemberStatus::new(path, MemberVerification::Verified),
            Err(Error::InvalidBackupMemberPath)
        );
    }
    assert_eq!(
        BackupMember::new("runtime.sqlite", BackupMemberKind::Runtime, 0, digest),
        Err(Error::InvalidBackupMemberLength)
    );
    let runtime =
        BackupMember::new("runtime.sqlite", BackupMemberKind::Metadata, 10, digest).unwrap();
    assert_eq!(runtime.relative_path(), "runtime.sqlite");
    assert_eq!(runtime.kind(), BackupMemberKind::Metadata);
    assert_eq!(runtime.byte_length(), 10);
    assert_eq!(runtime.sha256(), digest);
    for (created, members) in [(0, vec![runtime.clone()]), (1, vec![])] {
        assert_eq!(
            BackupManifest::new(
                BackupFormatVersion::V1,
                backup_id,
                created,
                BackupSecretPolicy::ExcludeProtectedStorage,
                members,
            ),
            Err(Error::InvalidBackupManifest)
        );
    }
    let huge = BackupMember::new("huge", BackupMemberKind::Runtime, u64::MAX, digest).unwrap();
    let one = BackupMember::new("one", BackupMemberKind::Runtime, 1, digest).unwrap();
    assert_eq!(
        BackupManifest::new(
            BackupFormatVersion::V1,
            backup_id,
            1,
            BackupSecretPolicy::ExcludeProtectedStorage,
            vec![huge, one],
        ),
        Err(Error::InvalidBackupManifest)
    );
    let manifest = manifest(BackupSecretPolicy::IncludeProtectedStorage);
    assert_eq!(manifest.format_version(), BackupFormatVersion::V1);
    assert_eq!(manifest.backup_id().as_bytes(), &[7; 16]);
    assert_eq!(manifest.created_at_unix_ms(), 100);
    assert_eq!(
        manifest.secret_policy(),
        BackupSecretPolicy::IncludeProtectedStorage
    );
    assert_eq!(manifest.members().len(), 2);
    assert!(manifest.member("runtime/runtime.sqlite").is_some());
    assert!(manifest.member("missing").is_none());

    assert_eq!(
        BackupPlan::new(
            backup_id,
            BackupFormatVersion::V1,
            BackupSecretPolicy::ExcludeProtectedStorage,
            0,
        ),
        Err(Error::InvalidBackupTimestamp)
    );
    let plan = BackupPlan::new(
        backup_id,
        BackupFormatVersion::V1,
        BackupSecretPolicy::ExcludeProtectedStorage,
        10,
    )
    .unwrap();
    assert_eq!(plan.backup_id(), backup_id);
    assert_eq!(plan.format_version(), BackupFormatVersion::V1);
    assert_eq!(
        plan.secret_policy(),
        BackupSecretPolicy::ExcludeProtectedStorage
    );
    assert_eq!(plan.requested_at_unix_ms(), 10);
    assert_eq!(
        ReliabilityRevision::new(0),
        Err(Error::InvalidReliabilityRevision)
    );
    assert_eq!(ReliabilityRevision::new(2).unwrap().get(), 2);
}

#[test]
fn backup_transition_matrix_rejects_revision_time_and_manifest_mismatch() {
    let plan = BackupPlan::new(
        BackupId::new([7; 16]).unwrap(),
        BackupFormatVersion::V1,
        BackupSecretPolicy::IncludeProtectedStorage,
        90,
    )
    .unwrap();
    let planned = BackupOperation::planned(plan.clone());
    assert_eq!(planned.plan(), &plan);
    assert_eq!(planned.revision(), ReliabilityRevision::INITIAL);
    assert_eq!(planned.stage(), BackupStage::Planned);
    assert!(planned.manifest().is_none());
    assert_eq!(planned.updated_at_unix_ms(), 90);
    assert_eq!(
        planned.transition(
            ReliabilityRevision::new(2).unwrap(),
            BackupTransition::Fail,
            100
        ),
        Err(Error::ReliabilityRevisionConflict)
    );
    assert_eq!(
        planned.transition(planned.revision(), BackupTransition::Fail, 89),
        Err(Error::InvalidBackupTimestamp)
    );
    let wrong_id = BackupManifest::new(
        BackupFormatVersion::V1,
        BackupId::new([8; 16]).unwrap(),
        100,
        BackupSecretPolicy::IncludeProtectedStorage,
        vec![member("runtime", BackupMemberKind::Runtime, 1)],
    )
    .unwrap();
    assert_eq!(
        planned.transition(
            planned.revision(),
            BackupTransition::Captured(wrong_id),
            100
        ),
        Err(Error::BackupManifestPlanMismatch)
    );
    let wrong_version = BackupManifest::new(
        BackupFormatVersion::new(2).unwrap(),
        plan.backup_id(),
        100,
        plan.secret_policy(),
        vec![member("runtime", BackupMemberKind::Runtime, 1)],
    )
    .unwrap();
    assert_eq!(
        planned.transition(
            planned.revision(),
            BackupTransition::Captured(wrong_version),
            100
        ),
        Err(Error::BackupManifestPlanMismatch)
    );
    let wrong_policy = manifest(BackupSecretPolicy::ExcludeProtectedStorage);
    assert_eq!(
        planned.transition(
            planned.revision(),
            BackupTransition::Captured(wrong_policy),
            100
        ),
        Err(Error::BackupManifestPlanMismatch)
    );
    assert_eq!(
        planned.transition(planned.revision(), BackupTransition::Verified, 100),
        Err(Error::InvalidBackupTransition)
    );
    let failed = planned
        .transition(planned.revision(), BackupTransition::Fail, 100)
        .unwrap();
    assert_eq!(failed.stage(), BackupStage::Failed);
    assert_eq!(
        failed.transition(failed.revision(), BackupTransition::Fail, 101),
        Err(Error::ReliabilityOperationTerminal)
    );
}

#[test]
fn restore_transition_and_member_verification_matrix_is_complete() {
    let backup_manifest = manifest(BackupSecretPolicy::IncludeProtectedStorage);
    assert_eq!(
        RestorePlan::new(
            backup_manifest.clone(),
            BackupSecretPolicy::IncludeProtectedStorage,
            0,
        ),
        Err(Error::InvalidRestoreTimestamp)
    );
    let plan = RestorePlan::new(
        backup_manifest.clone(),
        BackupSecretPolicy::IncludeProtectedStorage,
        200,
    )
    .unwrap();
    assert_eq!(plan.manifest(), &backup_manifest);
    assert_eq!(
        plan.accepted_secret_policy(),
        BackupSecretPolicy::IncludeProtectedStorage
    );
    assert_eq!(plan.requested_at_unix_ms(), 200);
    let staging = RestoreOperation::staging(plan.clone());
    assert_eq!(staging.plan(), &plan);
    assert_eq!(staging.revision(), ReliabilityRevision::INITIAL);
    assert_eq!(staging.stage(), RestoreStage::Staging);
    assert!(staging.member_status().is_empty());
    assert_eq!(
        staging.transition(
            ReliabilityRevision::new(2).unwrap(),
            RestoreTransition::Staged,
            201
        ),
        Err(Error::ReliabilityRevisionConflict)
    );
    assert_eq!(
        staging.transition(staging.revision(), RestoreTransition::Staged, 199),
        Err(Error::InvalidRestoreTimestamp)
    );
    assert_eq!(
        staging.transition(staging.revision(), RestoreTransition::Finalize, 201),
        Err(Error::InvalidRestoreTransition)
    );
    let verifying = staging
        .transition(staging.revision(), RestoreTransition::Staged, 201)
        .unwrap();
    for statuses in [
        vec![],
        vec![
            RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified)
                .unwrap(),
            RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified)
                .unwrap(),
        ],
        vec![
            RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified)
                .unwrap(),
            RestoreMemberStatus::new("foreign", MemberVerification::Verified).unwrap(),
        ],
        vec![
            RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified)
                .unwrap(),
            RestoreMemberStatus::new("private/private.sqlite", MemberVerification::Missing)
                .unwrap(),
        ],
    ] {
        assert_eq!(
            verifying.transition(
                verifying.revision(),
                RestoreTransition::Verified(statuses),
                202
            ),
            Err(Error::RestoreMemberVerificationFailed)
        );
    }
    let status =
        RestoreMemberStatus::new("runtime/runtime.sqlite", MemberVerification::Verified).unwrap();
    assert_eq!(status.relative_path(), "runtime/runtime.sqlite");
    assert_eq!(status.verification(), MemberVerification::Verified);
    let failed = verifying
        .transition(verifying.revision(), RestoreTransition::Fail, 202)
        .unwrap();
    assert_eq!(failed.stage(), RestoreStage::Failed);
    assert_eq!(
        failed.transition(failed.revision(), RestoreTransition::Fail, 203),
        Err(Error::ReliabilityOperationTerminal)
    );
}
