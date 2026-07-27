#![forbid(unsafe_code)]
#![cfg(feature = "sqlite")]

use radroots_event_codec::wire::publication::allowlist::allow_phase1_publication_canonical_json;
use radroots_event_codec::wire::publication::{
    RadrootsPhase1MediaReadyPublicationArtifact, bind_phase1_publication_media_readiness,
};
use radroots_outbox::{
    RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT, RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES,
    RadrootsOutbox, RadrootsOutboxRollbackConfirmation, RadrootsPhase1PublicationEnqueueStatus,
    RadrootsPhase1PublicationError, RadrootsPhase1PublicationTargetPolicy,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

const VECTOR: &[u8] = include_bytes!("fixtures/phase1_publication.v1.json");
const ARTIFACT_VECTOR: &[u8] =
    include_bytes!("../../event_codec/tests/fixtures/phase1_publication_artifact.v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    schema_version: u32,
    contract_id: String,
    executor: Executor,
    identity_vector: IdentityVector,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Executor {
    id: String,
    path: String,
    test: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityVector {
    fixture: String,
    target_uri: String,
    required_target_count: usize,
    artifact_digest: String,
    readiness_digest: String,
    endpoint_fingerprint: String,
    target_policy_digest: String,
    operation_digest: String,
    dispatch_digest: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    execution: String,
    expected_outcome: String,
    expected_error: Option<String>,
}

fn ready_fixture(fixture: &str) -> RadrootsPhase1MediaReadyPublicationArtifact {
    let root: Value = serde_json::from_slice(ARTIFACT_VECTOR).unwrap();
    let canonical = root["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|vector| {
            vector["input"]["fixture"].as_str() == Some(fixture)
                && vector["kind"]
                    .as_str()
                    .is_some_and(|kind| kind.ends_with(".valid"))
        })
        .and_then(|vector| vector["expected"]["canonical_json"].as_str())
        .unwrap();
    let allowlisted = allow_phase1_publication_canonical_json(canonical.as_bytes()).unwrap();
    bind_phase1_publication_media_readiness(allowlisted, Vec::new()).unwrap()
}

#[tokio::test]
async fn phase1_publication_v1_result_vector() {
    let vector: Vector = serde_json::from_slice(VECTOR).unwrap();
    assert_eq!(vector.schema_version, 1);
    assert_eq!(vector.contract_id, "radroots_outbox.phase1_publication.v1");
    assert_eq!(
        vector.executor.id,
        "radroots_outbox.phase1_publication.v1.result_vector_executor.v1"
    );
    assert_eq!(
        vector.executor.path,
        "crates/outbox/tests/phase1_publication_v1_result_vector.rs"
    );
    assert_eq!(vector.executor.test, "phase1_publication_v1_result_vector");
    assert_eq!(
        vector
            .cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "duplicate_enqueue",
            "empty_required_policy",
            "expired_lease_reclaim",
            "identity_preimages",
            "migration_rollback_reopen",
            "stale_claim_rejected",
            "target_count_exact",
            "target_count_one_over",
            "target_uri_exact",
            "target_uri_one_over",
            "two_worker_claim_race",
            "typed_enqueue",
        ])
    );
    for case in &vector.cases {
        assert_eq!(case.execution, "direct_executor");
        assert!(!case.expected_outcome.is_empty());
        assert_eq!(
            case.expected_error.is_some(),
            case.expected_outcome == "rejected"
        );
    }

    let identity = &vector.identity_vector;
    assert_eq!(identity.fixture, "update");
    let ready = ready_fixture(&identity.fixture);
    assert_eq!(
        ready.artifact().artifact_digest().to_hex(),
        identity.artifact_digest
    );
    assert_eq!(ready.binding_digest().to_hex(), identity.readiness_digest);
    let policy = RadrootsPhase1PublicationTargetPolicy::new(
        [identity.target_uri.as_str()],
        identity.required_target_count,
    )
    .unwrap();
    assert_eq!(hex::encode(policy.digest()), identity.target_policy_digest);

    let outbox = RadrootsOutbox::open_memory().await.unwrap();
    let inserted = outbox
        .enqueue_phase1_publication(&ready, &policy, 1)
        .await
        .unwrap();
    assert_eq!(
        inserted.status(),
        RadrootsPhase1PublicationEnqueueStatus::Inserted
    );
    assert_eq!(
        hex::encode(inserted.record().operation_digest()),
        identity.operation_digest
    );
    assert_eq!(
        hex::encode(inserted.record().targets()[0].endpoint_fingerprint()),
        identity.endpoint_fingerprint
    );
    assert_eq!(
        hex::encode(inserted.record().targets()[0].dispatch_digest()),
        identity.dispatch_digest
    );
    assert_eq!(
        outbox
            .enqueue_phase1_publication(&ready, &policy, 2)
            .await
            .unwrap()
            .status(),
        RadrootsPhase1PublicationEnqueueStatus::Existing
    );

    let exact_targets = (0..RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example"))
        .collect::<Vec<_>>();
    assert!(RadrootsPhase1PublicationTargetPolicy::new(exact_targets, 16).is_ok());
    let one_over_targets = (0..=RADROOTS_PHASE1_PUBLICATION_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example"))
        .collect::<Vec<_>>();
    assert_eq!(
        RadrootsPhase1PublicationTargetPolicy::new(one_over_targets, 1)
            .unwrap_err()
            .code(),
        "phase1_publication_target_count"
    );
    let prefix = "wss://example.com/";
    let exact_uri = format!(
        "{prefix}{}",
        "a".repeat(RADROOTS_PHASE1_PUBLICATION_TARGET_URI_MAX_BYTES - prefix.len())
    );
    assert!(RadrootsPhase1PublicationTargetPolicy::new([exact_uri.clone()], 1).is_ok());
    assert_eq!(
        RadrootsPhase1PublicationTargetPolicy::new([format!("{exact_uri}a")], 1)
            .unwrap_err()
            .code(),
        "phase1_publication_target_uri_too_large"
    );
    assert_eq!(
        RadrootsPhase1PublicationTargetPolicy::new(Vec::<String>::new(), 0)
            .unwrap_err()
            .code(),
        "phase1_publication_required_target_count"
    );

    let publication_id = inserted.record().publication_id();
    let first_worker = outbox.clone();
    let second_worker = outbox.clone();
    let (first, second) = tokio::join!(
        first_worker.claim_phase1_publication_for_signing(publication_id, 0, 10, 10),
        second_worker.claim_phase1_publication_for_signing(publication_id, 0, 10, 10),
    );
    let (winner, loser) = match (first, second) {
        (Ok(winner), Err(loser)) | (Err(loser), Ok(winner)) => (winner, loser),
        _ => panic!("exactly one worker must win the revision CAS"),
    };
    assert_eq!(loser.code(), "phase1_publication_revision_conflict");
    let reclaimed = outbox
        .claim_phase1_publication_for_signing(publication_id, 1, 20, 10)
        .await
        .unwrap();
    assert_eq!(
        outbox
            .renew_phase1_publication_claim(&winner, 21, 10)
            .await
            .unwrap_err()
            .code(),
        "phase1_publication_claim_invalid"
    );
    outbox
        .release_phase1_publication_claim(&reclaimed, 21)
        .await
        .unwrap();

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("vector.sqlite");
    let file_outbox = RadrootsOutbox::open_file(&path).await.unwrap();
    let file_record = file_outbox
        .enqueue_phase1_publication(&ready, &policy, 1)
        .await
        .unwrap();
    let file_publication_id = file_record.record().publication_id();
    file_outbox.close().await;
    RadrootsOutbox::rollback_file_schema_offline(
        &path,
        1,
        RadrootsOutboxRollbackConfirmation::acknowledge_data_loss(),
    )
    .await
    .unwrap();
    let reopened = RadrootsOutbox::open_file(&path).await.unwrap();
    assert!(matches!(
        reopened.load_phase1_publication(file_publication_id).await,
        Err(RadrootsPhase1PublicationError::PublicationNotFound { .. })
    ));
}
