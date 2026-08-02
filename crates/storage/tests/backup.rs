use radroots_storage::{
    Error, StorageReliability,
    backup::{
        BackupFormatVersion, BackupId, BackupManifest, BackupMember, BackupMemberKind,
        BackupOperation, BackupPlan, BackupSecretPolicy, BackupStage, BackupTransition,
        MemberDigest, MemberVerification, ReliabilityRevision, RestoreMemberStatus,
        RestoreOperation, RestorePlan, RestoreStage, RestoreTransition,
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
