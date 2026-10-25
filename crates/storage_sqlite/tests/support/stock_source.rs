use super::*;
use radroots_nostr::{commerce, key::SecretKey, signing::LocalSigner};
use radroots_signing::{provider::ProviderCall, request::CancellationSignal};
use radroots_storage::workspace::seller_stock::{
    SellerAuthoritySelection, StockSourceCheckpointRef,
};
use radroots_storage_sqlite::workspace::{
    StockGrantEnrollment, StockGrantOutcome, StockSourceCapture, StockSourceOutcome,
    StockSourceProfile, StockSourceReceipt,
};
use serde::Deserialize;
use std::num::NonZeroU64;

#[path = "source_provider.rs"]
mod provider;

#[derive(Clone, Deserialize)]
struct Vector {
    name: String,
    recipient_scalar: u64,
    raw_wrap: String,
    raw_seal: String,
    raw_rumor: String,
}
fn vectors() -> Vec<Vector> {
    serde_json::from_str(include_str!("../fixtures/stock_sources.v1.json")).unwrap()
}
fn source_vector(name: &str) -> Vector {
    vectors().into_iter().find(|row| row.name == name).unwrap()
}
fn signer(row: &Vector) -> LocalSigner {
    LocalSigner::with_clock(
        SecretKey::parse(&format!("{:064x}", row.recipient_scalar)).unwrap(),
        || Ok(1000),
    )
    .unwrap()
}
async fn capture(e: &StockEnrollment, row: &Vector) -> StockSourceCapture {
    let signer = signer(row);
    let call = ProviderCall::new(
        signer.public_key(),
        NonZeroU64::new(2000).unwrap(),
        CancellationSignal::new(),
    );
    let opened = commerce::open(&signer, call, &row.raw_wrap).await.unwrap();
    StockSourceCapture::prepare(
        SellerAuthoritySelection::from_attachment(e.input()),
        &opened,
    )
    .unwrap()
}
async fn create_source(
    f: &Fixture,
    row: &Vector,
    clock: Arc<dyn WorkspaceClock>,
) -> (
    NativeWorkspace,
    StockEnrollment,
    radroots_storage::workspace::WorkspaceBinding,
) {
    let e = StockEnrollment::prepare(
        SellerStockConfig::new(
            signer(row).public_key(),
            key(),
            "source-owner",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        clock.as_ref(),
    )
    .unwrap();
    let store = NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock)
        .await
        .unwrap();
    let binding = store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    (store, e, binding)
}
async fn source_minimum(store: &NativeWorkspace, e: &StockEnrollment) -> StockSourceCheckpointRef {
    store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap()
        .source_checkpoint()
        .unwrap()
}
fn retained(outcome: StockSourceOutcome) -> StockSourceReceipt {
    match outcome {
        StockSourceOutcome::Committed(receipt) => receipt,
        StockSourceOutcome::Unknown { .. } => panic!("expected a confirmed source transaction"),
    }
}

#[tokio::test]
async fn authenticated_original_profiles_replay_and_reopen_without_changing_inventory() {
    for row in vectors() {
        let f = Fixture::new();
        let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
        let before = store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap();
        let min = before.source_checkpoint().unwrap();
        let original = capture(&e, &row).await;
        let expected = match row.name.as_str() {
            "future_profile" | "future_version" | "other_rumor_kind" => {
                StockSourceProfile::Unsupported
            }
            "record_missing_field" => StockSourceProfile::Rejected,
            _ => StockSourceProfile::Known,
        };
        assert_eq!(original.profile(), expected);
        let receipt = retained(
            store
                .retain_stock_source(binding.clone(), min, &original, e.proof())
                .await
                .unwrap(),
        );
        assert_eq!(receipt.transaction_id(), original.transaction_id());
        assert_eq!(receipt.envelope_id(), original.envelope_id());
        assert_eq!(receipt.record_id(), original.record_id());
        assert_eq!(receipt.checkpoint().revision().get(), 1);
        let replay = capture(&e, &row).await;
        assert_ne!(replay.transaction_id(), original.transaction_id());
        assert_eq!(
            retained(
                store
                    .retain_stock_source(binding.clone(), receipt.checkpoint(), &replay, e.proof())
                    .await
                    .unwrap()
            ),
            receipt
        );
        let after = store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap();
        assert_eq!(before.inventory_checkpoint(), after.inventory_checkpoint());
        store.close().await.unwrap();
        let store = NativeWorkspace::open_seller_stock(
            &f.path,
            e.input().clone(),
            f.crypto(),
            f.clock.clone(),
        )
        .await
        .unwrap();
        let value = store
            .stock_source(
                binding,
                receipt.checkpoint(),
                original.envelope_id(),
                e.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(value.receipt(), &receipt);
        assert_eq!(value.protected_outer(), row.raw_wrap.as_bytes());
        assert_eq!(value.protected_seal(), row.raw_seal.as_bytes());
        assert_eq!(value.protected_rumor(), row.raw_rumor.as_bytes());
        assert_eq!(
            value.protected_canonical_record().is_some(),
            expected == StockSourceProfile::Known
        );
        assert_eq!(format!("{value:?}"), "StockSourceSnapshot(<redacted>)");
        let mut conn = database(&f).await;
        let protected: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_source_evidence")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert!(!String::from_utf8_lossy(&protected).contains("source-owner"));
        let max: i64 = sqlx::query_scalar("SELECT max(length(protected)) FROM stock_source_parts")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert!(max <= 262144);
        conn.close().await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn every_source_write_window_rolls_back_canonical_parts_header_and_progress() {
    for trigger in [
        "CREATE TRIGGER source_fault BEFORE INSERT ON stock_source_records BEGIN SELECT RAISE(ABORT,'isolated source fault'); END",
        "CREATE TRIGGER source_fault BEFORE INSERT ON stock_source_evidence BEGIN SELECT RAISE(ABORT,'isolated source fault'); END",
        "CREATE TRIGGER source_fault BEFORE INSERT ON stock_source_parts WHEN NEW.role=0 AND NEW.part=0 BEGIN SELECT RAISE(ABORT,'isolated source fault'); END",
        "CREATE TRIGGER source_fault BEFORE INSERT ON stock_source_parts WHEN NEW.role=2 BEGIN SELECT RAISE(ABORT,'isolated source fault'); END",
        "CREATE TRIGGER source_fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'isolated source fault'); END",
    ] {
        let row = source_vector("request/peer");
        let f = Fixture::new();
        let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
        let min = source_minimum(&store, &e).await;
        let original = capture(&e, &row).await;
        let mut conn = database(&f).await;
        sqlx::raw_sql(trigger).execute(&mut conn).await.unwrap();
        assert_eq!(
            store
                .retain_stock_source(binding.clone(), min, &original, e.proof())
                .await,
            Err(OperationError::StorageFailure)
        );
        assert_eq!(source_minimum(&store, &e).await, min);
        for (table, statement) in [
            ("records", "SELECT count(*) FROM stock_source_records"),
            ("evidence", "SELECT count(*) FROM stock_source_evidence"),
            ("parts", "SELECT count(*) FROM stock_source_parts"),
        ] {
            let count: i64 = sqlx::query_scalar(statement)
                .fetch_one(&mut conn)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        sqlx::raw_sql("DROP TRIGGER source_fault")
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(
            store
                .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
                .await
                .unwrap()
                .is_none()
        );
        let receipt = retained(
            store
                .retain_stock_source(binding, min, &original, e.proof())
                .await
                .unwrap(),
        );
        assert_eq!(receipt.transaction_id(), original.transaction_id());
        conn.close().await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_source_commit_is_unknown_fences_access_and_recovers_original_anchor() {
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    let mut conn = database(&f).await;
    sqlx::raw_sql("CREATE TABLE source_commit_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER source_commit_fault_trigger AFTER INSERT ON stock_source_evidence BEGIN INSERT INTO source_commit_fault VALUES(2); END").execute(&mut conn).await.unwrap();
    assert_eq!(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
        StockSourceOutcome::Unknown {
            transaction: original.transaction_id(),
            envelope: original.envelope_id()
        }
    );
    assert!(matches!(
        store
            .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER source_commit_fault_trigger; DROP TABLE source_commit_fault")
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert!(
        store
            .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
            .await
            .unwrap()
            .is_none()
    );
    let receipt = retained(
        store
            .retain_stock_source(binding, min, &original, e.proof())
            .await
            .unwrap(),
    );
    assert_eq!(receipt.transaction_id(), original.transaction_id());
    store.close().await.unwrap();
}

#[tokio::test]
async fn current_proof_time_and_administration_gate_source_reads_and_original_replay() {
    use radroots_storage::workspace::seller_stock::StockGrantScope;
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    let receipt = retained(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
    );
    let wrong = AttachmentProof::from_protected_input(&[0xb7; 32]).unwrap();
    assert!(matches!(
        store
            .stock_source(binding.clone(), min, original.envelope_id(), &wrong)
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    for time in [0, 99, 200] {
        f.clock.0.store(time, Ordering::SeqCst);
        assert!(
            store
                .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
                .await
                .is_err()
        );
        assert!(
            store
                .retain_stock_source(binding.clone(), min, &original, e.proof())
                .await
                .is_err()
        );
    }
    f.clock.0.store(100, Ordering::SeqCst);
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(e.input()),
        "physical-only".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    let inventory = store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let authorization = match store
        .enroll_stock_grant(binding.clone(), inventory, e.proof(), &child)
        .await
        .unwrap()
    {
        StockGrantOutcome::Committed(receipt) => receipt.authorization(),
        _ => panic!("grant must confirm"),
    };
    let physical = store
        .authenticate(child.input().clone(), child.proof())
        .await
        .unwrap();
    let current_min = receipt.checkpoint().with_authorization(authorization);
    assert!(matches!(
        store
            .stock_source(
                physical.clone(),
                current_min,
                original.envelope_id(),
                child.proof()
            )
            .await,
        Err(OperationError::Unauthorized)
    ));
    assert!(matches!(
        store
            .retain_stock_source(physical, current_min, &original, child.proof())
            .await,
        Err(OperationError::Unauthorized)
    ));
    assert!(matches!(
        store
            .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    assert_eq!(
        retained(
            store
                .retain_stock_source(binding, current_min, &original, e.proof())
                .await
                .unwrap()
        ),
        receipt
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn source_transaction_rechecks_clock_and_expiry_after_protection_io() {
    for next in [0, 99, 200] {
        let row = source_vector("request/peer");
        let f = Fixture::new();
        let clock = Arc::new(AdmissionClock {
            armed: std::sync::atomic::AtomicBool::new(false),
            calls: AtomicU64::new(0),
            next,
        });
        let (store, e, binding) = create_source(&f, &row, clock.clone()).await;
        let min = source_minimum(&store, &e).await;
        let original = capture(&e, &row).await;
        clock.armed.store(true, Ordering::SeqCst);
        let result = store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await;
        if next == 200 {
            assert_eq!(
                result,
                Err(OperationError::Workspace(WorkspaceError::GrantExpired))
            );
        } else {
            assert_eq!(
                result,
                Err(OperationError::Workspace(WorkspaceError::Unavailable))
            );
        }
        clock.armed.store(false, Ordering::SeqCst);
        assert_eq!(source_minimum(&store, &e).await, min);
        assert!(
            store
                .stock_source(binding, min, original.envelope_id(), e.proof())
                .await
                .unwrap()
                .is_none()
        );
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn admitted_source_retention_survives_observer_loss_and_close_drains_original_identity() {
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let (store, e, binding) = create_source(&f, &row, clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    clock.arm();
    let mut future =
        Box::pin(store.retain_stock_source(binding.clone(), min, &original, e.proof()));
    assert!(futures::poll!(future.as_mut()).is_pending());
    clock.entered();
    drop(future);
    let mut closing = Box::pin(store.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    drop(closing);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await,
        Err(WorkspaceError::WriterActive)
    ));
    clock.release();
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let snapshot = store
        .stock_source(binding.clone(), min, original.envelope_id(), e.proof())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot.receipt().transaction_id(),
        original.transaction_id()
    );
    assert_eq!(
        &retained(
            store
                .retain_stock_source(binding, min, &original, e.proof())
                .await
                .unwrap()
        ),
        snapshot.receipt()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn missing_or_substituted_original_source_parts_and_canonical_records_are_never_repaired() {
    for statement in [
        "UPDATE stock_source_parts SET protected=randomblob(length(protected)) WHERE role=0 AND part=0",
        "UPDATE stock_source_records SET protected=randomblob(length(protected))",
        "UPDATE stock_source_evidence SET transaction_id=zeroblob(32)",
        "DELETE FROM stock_source_parts WHERE role=2",
    ] {
        let row = source_vector("request/peer");
        let f = Fixture::new();
        let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
        let min = source_minimum(&store, &e).await;
        let original = capture(&e, &row).await;
        let receipt = retained(
            store
                .retain_stock_source(binding.clone(), min, &original, e.proof())
                .await
                .unwrap(),
        );
        let mut conn = database(&f).await;
        sqlx::raw_sql(statement).execute(&mut conn).await.unwrap();
        assert!(
            store
                .stock_source(
                    binding.clone(),
                    receipt.checkpoint(),
                    original.envelope_id(),
                    e.proof()
                )
                .await
                .is_err()
        );
        assert!(
            store
                .retain_stock_source(binding, receipt.checkpoint(), &original, e.proof())
                .await
                .is_err()
        );
        conn.close().await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn retained_source_checkpoint_refuses_full_database_restore_and_foreign_or_forked_minima() {
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    store.close().await.unwrap();
    let path = f.path.join("stock_execution_v1.sqlite");
    let original_db = std::fs::read(&path).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let original = capture(&e, &row).await;
    let receipt = retained(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
    );
    let fork = StockSourceCheckpointRef::from_host_metadata(
        min.authority(),
        receipt.checkpoint().revision(),
        Some(radroots_domain::CheckpointDigest::from_bytes([0xc9; 32])),
    )
    .unwrap()
    .with_optional_authorization(min.authorization());
    assert!(matches!(
        store
            .stock_source(binding.clone(), fork, original.envelope_id(), e.proof())
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    let foreign = StockSourceCheckpointRef::from_host_metadata(
        AuthorityRef::new(
            ExecutorId::from_bytes([0xc7; 32]),
            min.authority().generation(),
        ),
        min.revision(),
        None,
    )
    .unwrap()
    .with_optional_authorization(min.authorization());
    assert!(matches!(
        store
            .stock_source(binding.clone(), foreign, original.envelope_id(), e.proof())
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    store.close().await.unwrap();
    std::fs::write(&path, original_db).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert!(matches!(
        store
            .stock_source(
                binding.clone(),
                receipt.checkpoint(),
                original.envelope_id(),
                e.proof()
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    assert!(matches!(
        store
            .retain_stock_source(binding, receipt.checkpoint(), &original, e.proof())
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn separate_authenticated_envelopes_keep_both_originals_and_one_logical_record() {
    use nostr::{EventBuilder, JsonUtil, Keys, UnsignedEvent};
    let row = source_vector("quote/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    let sender = Keys::parse(&format!("{:064x}", 1)).unwrap();
    let recipient = nostr::PublicKey::from_hex(&signer(&row).public_key().to_hex()).unwrap();
    let rumor = UnsignedEvent::from_json(&row.raw_rumor).unwrap();
    let event = EventBuilder::gift_wrap(&sender, &recipient, rumor, [])
        .await
        .unwrap();
    let mut second = row.clone();
    second.raw_wrap = event.as_json();
    let other = capture(&e, &second).await;
    assert_eq!(other.record_id(), original.record_id());
    assert_ne!(other.envelope_id(), original.envelope_id());
    let first = retained(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
    );
    let next = retained(
        store
            .retain_stock_source(binding.clone(), first.checkpoint(), &other, e.proof())
            .await
            .unwrap(),
    );
    assert_eq!(next.checkpoint().revision().get(), 2);
    assert_ne!(first.checkpoint().head(), next.checkpoint().head());
    let mut conn = database(&f).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_source_records")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(count, 1);
    conn.close().await.unwrap();
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    for (capture, raw) in [
        (&original, row.raw_wrap.as_bytes()),
        (&other, second.raw_wrap.as_bytes()),
    ] {
        let value = store
            .stock_source(
                binding.clone(),
                first.checkpoint(),
                capture.envelope_id(),
                e.proof(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(value.protected_outer(), raw);
    }
    assert_eq!(
        retained(
            store
                .retain_stock_source(binding, next.checkpoint(), &original, e.proof())
                .await
                .unwrap()
        ),
        first
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn same_envelope_identity_with_different_original_bytes_is_a_conflict() {
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    let receipt = retained(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
    );
    let mut alias = row.clone();
    alias.raw_wrap = serde_json::to_string_pretty(
        &serde_json::from_str::<serde_json::Value>(&row.raw_wrap).unwrap(),
    )
    .unwrap();
    let alias = capture(&e, &alias).await;
    assert_eq!(alias.envelope_id(), original.envelope_id());
    assert_eq!(
        store
            .retain_stock_source(binding.clone(), receipt.checkpoint(), &alias, e.proof())
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    assert_eq!(source_minimum(&store, &e).await, receipt.checkpoint());
    let value = store
        .stock_source(
            binding,
            receipt.checkpoint(),
            original.envelope_id(),
            e.proof(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(value.protected_outer(), row.raw_wrap.as_bytes());
    store.close().await.unwrap();
}

#[tokio::test]
async fn inventory_progress_preserves_source_checkpoint_and_original_evidence() {
    let row = source_vector("request/peer");
    let f = Fixture::new();
    let (store, e, binding) = create_source(&f, &row, f.clock.clone()).await;
    let min = source_minimum(&store, &e).await;
    let original = capture(&e, &row).await;
    let receipt = retained(
        store
            .retain_stock_source(binding.clone(), min, &original, e.proof())
            .await
            .unwrap(),
    );
    let inventory = committed(
        store
            .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(inventory.checkpoint().revision().get(), 1);
    assert_eq!(source_minimum(&store, &e).await, receipt.checkpoint());
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .stock_source(
                binding,
                receipt.checkpoint(),
                original.envelope_id(),
                e.proof()
            )
            .await
            .unwrap()
            .unwrap()
            .receipt(),
        &receipt
    );
    store.close().await.unwrap();
}
