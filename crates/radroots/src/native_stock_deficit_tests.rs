//! Real signed/installed acceptance, owner-private loss and protected reopen.
use super::*;
use radroots_domain::{
    Sequence,
    stock::{AvailableStockCorrection, StockTicks},
};
use radroots_storage::inventory::InventoryReceipt;

#[tokio::test]
async fn current_inspection_reports_aggregate_backing_instead_of_old_receipt_totals() {
    use AvailableStockCorrection::{Add, Remove, Spoil};
    use sqlx::Connection;
    let r = Rig::new().await;
    let (_command, _status) = r.accept().await;
    let (_, _, minimum) = r.minima().await;
    let initial = r
        .client
        .stock(&r.scope)
        .inspect(minimum, PoolId::from_bytes([80; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial.checkpoint(), minimum);
    assert_eq!(initial.installed_obligations().remaining().get(), 3);
    assert_eq!(initial.installed_obligations().backed().get(), 3);
    assert_eq!(initial.installed_obligations().deficit(), StockTicks::ZERO);
    let first = committed(
        correct(
            &r,
            130,
            initial.pool().snapshot().version().get(),
            Spoil,
            "8",
            "first synthetic loss",
        )
        .await
        .unwrap(),
    );
    let current = r
        .client
        .stock(&r.scope)
        .inspect(first.checkpoint(), first.pool().id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.checkpoint(), first.checkpoint());
    assert_eq!(current.installed_obligations().remaining().get(), 3);
    assert_eq!(current.installed_obligations().backed().get(), 2);
    assert_eq!(current.installed_obligations().deficit().get(), 1);
    let second = committed(
        correct(
            &r,
            131,
            first.pool().version().get(),
            Remove,
            "1",
            "second synthetic loss",
        )
        .await
        .unwrap(),
    );
    let added = committed(
        correct(
            &r,
            132,
            second.pool().version().get(),
            Add,
            "2",
            "explicit new source",
        )
        .await
        .unwrap(),
    );
    let current = r
        .client
        .stock(&r.scope)
        .inspect(added.checkpoint(), added.pool().id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.deficits()[0].backing().deficit().get(), 1);
    assert_eq!(current.installed_obligations().remaining().get(), 3);
    assert_eq!(current.installed_obligations().backed().get(), 1);
    assert_eq!(current.installed_obligations().deficit().get(), 2);
    assert_eq!(
        current
            .pool()
            .snapshot()
            .balances()
            .physical()
            .available
            .get(),
        2
    );
    assert_eq!(
        format!("{current:?}"),
        "StockSnapshot(<private current metadata>)"
    );
    // Read fences do not append operations or reinterpret absent projections.
    assert_eq!(r.minima().await.2, added.checkpoint());
    let mut fault = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        r._temp
            .path()
            .join("stock/stock_execution_v1.sqlite")
            .display()
    ))
    .await
    .unwrap();
    sqlx::query("DELETE FROM stock_deficits WHERE operation_id=?")
        .bind(first.attempt().operation_id().as_bytes().as_slice())
        .execute(&mut fault)
        .await
        .unwrap();
    assert_eq!(
        r.client
            .stock(&r.scope)
            .inspect(added.checkpoint(), added.pool().id())
            .await
            .unwrap_err()
            .code(),
        crate::Code::RecoveryRequired
    );
    let original = first.deficits()[0];
    sqlx::query("INSERT INTO stock_deficits(actor,operation_id,acceptance_id,installation_id,line_id,pool_id) VALUES(?,?,?,?,?,?)")
        .bind(r.enrollment.input().participant().as_bytes().as_slice()).bind(first.attempt().operation_id().as_bytes().as_slice())
        .bind(original.acceptance().as_bytes().as_slice()).bind(original.installation().as_bytes().as_slice())
        .bind(original.line().as_bytes().as_slice()).bind(original.pool().as_bytes().as_slice()).execute(&mut fault).await.unwrap();
    fault.close().await.unwrap();
    r.clock.0.store(0, Ordering::SeqCst);
    assert!(
        r.client
            .stock(&r.scope)
            .inspect(added.checkpoint(), added.pool().id())
            .await
            .is_err()
    );
    r.clock.0.store(150, Ordering::SeqCst);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.sent.lock().unwrap().len(), 2);
    r.client.close().await.unwrap();
    let crypto = WorkspaceCrypto::new(
        r.vault.clone(),
        SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        ),
        SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        AccessPolicy::standard(),
    )
    .unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &r._temp.path().join("stock"),
        r.enrollment.input().clone(),
        crypto,
        r.clock.clone(),
    )
    .await
    .unwrap();
    let binding = reopened
        .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let recovered = reopened
        .inspect_stock(
            binding,
            added.checkpoint(),
            added.pool().id(),
            proof(r.enrollment.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered, current);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn current_inspection_keeps_pending_claims_separate_from_installed_obligations() {
    let r = Arc::new(Rig::new().await);
    let (_, _, minimum) = r.minima().await;
    assert!(
        r.client
            .stock(&r.scope)
            .inspect(minimum, PoolId::from_bytes([99; 32]))
            .await
            .unwrap()
            .is_none()
    );
    r.control.pause_peer.store(true, Ordering::SeqCst);
    let task = {
        let r = r.clone();
        tokio::spawn(async move { r.accept().await })
    };
    r.control.entered.notified().await;
    let (_, _, minimum) = r.minima().await;
    let signs = r.control.signs.load(Ordering::SeqCst);
    let pending = r
        .client
        .stock(&r.scope)
        .inspect(minimum, PoolId::from_bytes([80; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        pending
            .pool()
            .snapshot()
            .balances()
            .physical()
            .preparatory_hold
            .get(),
        3
    );
    assert_eq!(
        pending
            .pool()
            .snapshot()
            .balances()
            .physical()
            .available
            .get(),
        7
    );
    assert_eq!(
        pending.installed_obligations().remaining(),
        StockTicks::ZERO
    );
    assert_eq!(pending.installed_obligations().backed(), StockTicks::ZERO);
    assert_eq!(pending.installed_obligations().deficit(), StockTicks::ZERO);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), signs);
    r.control.release.notify_one();
    task.await.unwrap();
    let (_, _, minimum) = r.minima().await;
    let installed = r
        .client
        .stock(&r.scope)
        .inspect(minimum, pending.pool().snapshot().id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(installed.installed_obligations().remaining().get(), 3);
    assert_eq!(installed.installed_obligations().backed().get(), 3);
    assert_eq!(
        installed
            .pool()
            .snapshot()
            .balances()
            .physical()
            .preparatory_hold,
        StockTicks::ZERO
    );
    r.client.close().await.unwrap();
}

async fn correct(
    r: &Rig,
    id: u8,
    version: u64,
    kind: AvailableStockCorrection,
    amount: &str,
    reason: &str,
) -> crate::Result<InventoryOutcome> {
    let (_, _, minimum) = r.minima().await;
    let context = r
        .scope
        .command_context(
            OperationId::from_bytes([id; 32]),
            None,
            None,
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    let input = radroots_protocol::command::PoolCorrectionInput::new(
        PoolId::from_bytes([80; 32]),
        Sequence::new(version).unwrap(),
        kind,
        amount.parse().unwrap(),
        reason,
    )
    .unwrap();
    if kind == AvailableStockCorrection::Spoil {
        r.client
            .stock(&r.scope)
            .spoil(context, minimum, input)
            .await
    } else {
        r.client
            .stock(&r.scope)
            .adjust(context, minimum, input)
            .await
    }
}
fn committed(outcome: InventoryOutcome) -> InventoryReceipt {
    let InventoryOutcome::Committed(receipt) = outcome else {
        panic!("actual correction must confirm its transaction");
    };
    *receipt
}

async fn failed_native_correction(uncertain_commit: bool) {
    use sqlx::Connection;
    let r = Rig::new().await;
    let (_, installed) = r.accept().await;
    let (_, _, minimum) = r.minima().await;
    let before = r
        .client
        .stock(&r.scope)
        .pool(minimum, PoolId::from_bytes([80; 32]))
        .await
        .unwrap()
        .unwrap()
        .snapshot();
    let mut fault = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        r._temp
            .path()
            .join("stock/stock_execution_v1.sqlite")
            .display()
    ))
    .await
    .unwrap();
    let counts_sql = "SELECT (SELECT count(*) FROM stock_operations),(SELECT count(*) FROM stock_movements),(SELECT count(*) FROM stock_deficits)";
    let counts: (i64, i64, i64) = sqlx::query_as(counts_sql)
        .fetch_one(&mut fault)
        .await
        .unwrap();
    if uncertain_commit {
        sqlx::raw_sql("CREATE TABLE deficit_deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER controlled_deficit_failure AFTER INSERT ON stock_deficits BEGIN INSERT INTO deficit_deferred_fault VALUES(2); END").execute(&mut fault).await.unwrap();
    } else {
        sqlx::query("CREATE TRIGGER controlled_deficit_failure BEFORE INSERT ON stock_deficits BEGIN SELECT RAISE(ABORT, 'controlled_deficit_failure'); END").execute(&mut fault).await.unwrap();
    }
    let result = correct(
        &r,
        120,
        before.version().get(),
        AvailableStockCorrection::Spoil,
        "8",
        "synthetic loss at actual failed transaction",
    )
    .await;
    if uncertain_commit {
        let InventoryOutcome::Unknown(anchor) = result.unwrap() else {
            panic!("actual COMMIT failure must remain unknown");
        };
        assert_eq!(anchor.operation_id(), OperationId::from_bytes([120; 32]));
        assert!(anchor.transaction_id().is_some());
        assert_eq!(
            r.native
                .inventory_operation(
                    r.scope.binding().clone(),
                    minimum,
                    anchor.operation_id(),
                    Some(anchor.command_digest()),
                    proof(r.enrollment.proof())
                )
                .await
                .unwrap_err(),
            OperationError::RecoveryRequired
        );
    } else {
        assert_eq!(result.unwrap_err().code(), crate::Code::StorageFailure);
    }
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64)>(counts_sql)
            .fetch_one(&mut fault)
            .await
            .unwrap(),
        counts
    );
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.sent.lock().unwrap().len(), 2);
    // No obtained material is pending for an owner-private correction. Close
    // may release this original writer without a current clock or new effect.
    r.clock.0.store(0, Ordering::SeqCst);
    r.client.close().await.unwrap();
    sqlx::raw_sql(
        "DROP TRIGGER controlled_deficit_failure; DROP TABLE IF EXISTS deficit_deferred_fault",
    )
    .execute(&mut fault)
    .await
    .unwrap();
    fault.close().await.unwrap();
    r.clock.0.store(150, Ordering::SeqCst);
    let crypto = WorkspaceCrypto::new(
        r.vault.clone(),
        SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        ),
        SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        AccessPolicy::standard(),
    )
    .unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &r._temp.path().join("stock"),
        r.enrollment.input().clone(),
        crypto,
        r.clock.clone(),
    )
    .await
    .unwrap();
    let binding = reopened
        .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let pool = reopened
        .pool(
            binding.clone(),
            minimum,
            before.id(),
            proof(r.enrollment.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.snapshot(), before);
    assert!(
        reopened
            .inventory_operation(
                binding.clone(),
                minimum,
                OperationId::from_bytes([120; 32]),
                None,
                proof(r.enrollment.proof())
            )
            .await
            .unwrap()
            .is_none()
    );
    let current = reopened
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    assert_eq!(current.inventory_checkpoint(), Some(installed.checkpoint()));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn native_inventory_proof_work_reserves_the_live_queue_budget_until_sql_confirmation() {
    inventory_read_budget(false).await;
}

#[tokio::test]
async fn native_inspection_reserves_the_live_queue_budget_until_sql_confirmation() {
    inventory_read_budget(true).await;
}

async fn inventory_read_budget(inspect: bool) {
    use sqlx::Connection;
    let r = Rig::new().await;
    let (_, _, minimum) = r.minima().await;
    let pool = PoolId::from_bytes([80; 32]);
    let mut blocker = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        r._temp
            .path()
            .join("stock/stock_execution_v1.sqlite")
            .display()
    ))
    .await
    .unwrap();
    let transaction = blocker.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let read = || async {
        if inspect {
            r.native
                .inspect_stock(
                    r.scope.binding().clone(),
                    minimum,
                    pool,
                    proof(r.enrollment.proof()),
                )
                .await
                .map(|value| value.map(|value| value.pool().clone()))
        } else {
            r.native
                .pool(
                    r.scope.binding().clone(),
                    minimum,
                    pool,
                    proof(r.enrollment.proof()),
                )
                .await
        }
    };
    let mut reads: Vec<_> = (0..4).map(|_| Box::pin(read())).collect();
    for read in &mut reads {
        assert!(futures::poll!(read.as_mut()).is_pending());
    }
    let fifth = read();
    let error = tokio::time::timeout(std::time::Duration::from_secs(1), fifth)
        .await
        .expect("full queue must refuse without waiting for its blocked writer")
        .unwrap_err();
    assert_eq!(
        error,
        OperationError::Workspace(WorkspaceError::Unavailable)
    );
    transaction.rollback().await.unwrap();
    for result in futures::future::join_all(reads).await {
        assert_eq!(
            result
                .unwrap()
                .unwrap()
                .snapshot()
                .balances()
                .physical()
                .available
                .get(),
            10
        );
    }
    blocker.close().await.unwrap();
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn native_deficit_statement_failure_rolls_back_pool_movement_obligation_and_root() {
    failed_native_correction(false).await;
}

#[tokio::test]
async fn native_deficit_commit_failure_stays_unknown_and_reopen_retains_original_backing() {
    failed_native_correction(true).await;
}

#[tokio::test]
async fn installed_backing_loss_retains_obligation_exact_replay_deficits_and_reopen() {
    use AvailableStockCorrection::{Add, Remove, Spoil};
    let r = Rig::new().await;
    let original_request = r.request().await;
    let line = &original_request.terms().lines()[0];
    let key =
        radroots_domain::stock::OptionMappingKey::new(line.source().coordinate(), line.option_id());
    let (acceptance, installed) = r.accept().await;
    let (_, _, minimum) = r.minima().await;
    let before = r
        .client
        .stock(&r.scope)
        .pool(minimum, PoolId::from_bytes([80; 32]))
        .await
        .unwrap()
        .unwrap()
        .snapshot();
    assert_eq!(before.balances().physical().available.get(), 7);
    assert_eq!(before.balances().physical().committed_hold.get(), 3);
    // A later mapping cannot change the original three-tick obligation into
    // a six-tick obligation or redirect this loss to a newly selected plan.
    let (_, _, mapping_minimum) = r.minima().await;
    let mapping = r
        .client
        .stock(&r.scope)
        .option_mapping(mapping_minimum, key)
        .await
        .unwrap()
        .unwrap();
    let context = r
        .scope
        .command_context(
            OperationId::from_bytes([109; 32]),
            None,
            None,
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    let changed = radroots_protocol::command::MapOptionInput::new(
        key,
        before.id(),
        radroots_domain::Quantity::new("2".parse().unwrap(), before.unit().unit()).unwrap(),
        mapping.version(),
    )
    .unwrap();
    r.client
        .stock(&r.scope)
        .map_option(context, mapping_minimum, changed)
        .await
        .unwrap();
    let first = committed(
        correct(
            &r,
            110,
            before.version().get(),
            Spoil,
            "8",
            "synthetic recorded spoilage",
        )
        .await
        .unwrap(),
    );
    let physical = first.pool().balances().physical();
    assert_eq!(physical.available, StockTicks::ZERO);
    assert_eq!(physical.committed_hold.get(), 2);
    assert_eq!(physical.waste.get(), 8);
    assert_eq!(first.pool().balances().introduced().get(), 10);
    assert_eq!(first.deficits().len(), 1);
    let deficit = first.deficits()[0];
    assert_eq!(deficit.acceptance(), acceptance.context().operation_id());
    assert_eq!(deficit.pool(), before.id());
    assert_eq!(deficit.backing().remaining().get(), 3);
    assert_eq!(deficit.backing().backed().get(), 2);
    assert_eq!(deficit.backing().deficit().get(), 1);
    assert_eq!(
        format!("{deficit:?}"),
        "StockDeficit(<private effect metadata>)"
    );
    let repeat = committed(
        correct(
            &r,
            110,
            before.version().get(),
            Spoil,
            "8",
            "synthetic recorded spoilage",
        )
        .await
        .unwrap(),
    );
    assert_eq!(repeat, first);
    assert_eq!(
        correct(
            &r,
            110,
            before.version().get(),
            Spoil,
            "8",
            "changed captured reason"
        )
        .await
        .unwrap_err()
        .code(),
        crate::Code::IdempotencyConflict
    );
    assert_eq!(
        correct(&r, 111, before.version().get(), Spoil, "1", "stale version")
            .await
            .unwrap_err()
            .code(),
        crate::Code::StaleState
    );
    let second = committed(
        correct(
            &r,
            112,
            first.pool().version().get(),
            Remove,
            "2",
            "synthetic recorded removal",
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        second.pool().balances().physical().committed_hold,
        StockTicks::ZERO
    );
    assert_eq!(second.pool().balances().physical().removed.get(), 2);
    assert_eq!(second.deficits()[0].backing().remaining().get(), 3);
    assert_eq!(second.deficits()[0].backing().backed(), StockTicks::ZERO);
    assert_eq!(second.deficits()[0].backing().deficit().get(), 3);
    assert_eq!(
        correct(
            &r,
            113,
            second.pool().version().get(),
            Spoil,
            "1",
            "cannot deplete an unbacked obligation"
        )
        .await
        .unwrap_err()
        .code(),
        crate::Code::StaleState
    );
    let added = committed(
        correct(
            &r,
            114,
            second.pool().version().get(),
            Add,
            "2",
            "explicit new physical source",
        )
        .await
        .unwrap(),
    );
    assert!(added.deficits().is_empty());
    assert_eq!(added.pool().balances().physical().available.get(), 2);
    assert_eq!(
        added.pool().balances().physical().committed_hold,
        StockTicks::ZERO
    );
    assert_eq!(added.pool().balances().introduced().get(), 12);
    let (_, _, minimum) = r.minima().await;
    let old = r
        .client
        .stock(&r.scope)
        .operation(
            minimum,
            first.attempt().operation_id(),
            Some(first.attempt().command_digest()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old, first);
    // The current minimum lies after this original loss. It must not permit
    // trusting a forged historical deficit projection beneath that minimum.
    use sqlx::Connection;
    let mut fault = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        r._temp
            .path()
            .join("stock/stock_execution_v1.sqlite")
            .display()
    ))
    .await
    .unwrap();
    sqlx::query("UPDATE stock_deficits SET installation_id=acceptance_id WHERE operation_id=?")
        .bind(first.attempt().operation_id().as_bytes().as_slice())
        .execute(&mut fault)
        .await
        .unwrap();
    assert_eq!(
        r.client
            .stock(&r.scope)
            .operation(minimum, first.attempt().operation_id(), None)
            .await
            .unwrap_err()
            .code(),
        crate::Code::RecoveryRequired
    );
    sqlx::query("UPDATE stock_deficits SET installation_id=? WHERE operation_id=?")
        .bind(deficit.installation().as_bytes().as_slice())
        .bind(first.attempt().operation_id().as_bytes().as_slice())
        .execute(&mut fault)
        .await
        .unwrap();
    fault.close().await.unwrap();
    let status = r
        .client
        .orders(&r.scope)
        .acceptance_status(
            minimum,
            acceptance.context().operation_id(),
            acceptance.digest(),
        )
        .await
        .unwrap();
    assert_eq!(status.installation(), installed.installation());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.wraps.load(Ordering::SeqCst), 2);
    assert_eq!(r.io.sent.lock().unwrap().len(), 2);
    r.client.close().await.unwrap();
    let crypto = WorkspaceCrypto::new(
        r.vault.clone(),
        SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        ),
        SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        AccessPolicy::standard(),
    )
    .unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &r._temp.path().join("stock"),
        r.enrollment.input().clone(),
        crypto,
        r.clock.clone(),
    )
    .await
    .unwrap();
    let binding = reopened
        .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let recovered = reopened
        .inventory_operation(
            binding,
            minimum,
            second.attempt().operation_id(),
            Some(second.attempt().command_digest()),
            proof(r.enrollment.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered, second);
    reopened.close().await.unwrap();
}
