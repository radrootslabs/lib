//! Actual T0 effects on the native connection, with independent original sources.
use super::*;
use crate::workspace::{StockConsentCapture, StockConsentMode, StockConsentOutcome};
use radroots_domain::{CommandContextData, OperationId, PoolId};
use radroots_protocol::command::CapturedOrderAcceptance;
use radroots_storage::{
    inventory::{
        InventoryOutcome, InventoryRequest, InventoryStore, StockAcceptanceOutcome,
        StockAcceptanceReceipt, StockAcceptanceRequest,
    },
    workspace::seller_stock::StockResources,
};
#[path = "signing_tests.rs"]
mod signing;

fn pool(id: u8) -> PoolId {
    PoolId::from_bytes([id; 32])
}
async fn inventory_minimum(r: &Rig) -> radroots_storage::inventory::InventoryCheckpointRef {
    r.store
        .stock_authority(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap()
}
fn context(r: &Rig, id: u8) -> CommandContextData {
    CommandContextData::new(
        OperationId::from_bytes([id; 32]),
        r.binding.actor(),
        r.enrollment.input().workspace_id(),
        r.enrollment.input().authority(),
        r.enrollment.input().route_digest(),
        None,
        None,
        UnixSeconds::new(230).unwrap(),
        None,
    )
    .unwrap()
}
async fn physical(r: &Rig, wire: serde_json::Value) {
    let request = InventoryRequest::from_protected_wire(
        r.binding.clone(),
        inventory_minimum(r).await,
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    let proof = r
        .enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap();
    assert!(matches!(
        r.store.execute_inventory(request, proof).await.unwrap(),
        InventoryOutcome::Committed(_)
    ));
}
async fn seed(r: &Rig, amount: &str) {
    base(r).await;
    for id in [80, 81] {
        physical(r, serde_json::json!({"type":"stock.create_pool","context":context(r,id),"input":{"id":pool(id),"label":"private bunch-equivalent","unit":"each","tick_scale":"0","initial_quantity":amount}})).await;
    }
    let inspected = inspect(r).await;
    let line = &inspected.terms().lines()[0];
    physical(r, serde_json::json!({"type":"stock.map_option","context":context(r,82),"input":{"listing":line.source().coordinate(),"option_id":line.option_id(),"pool_id":pool(80),"expected_mapping_version":"0","per_sale_unit":{"amount":"1","unit":"each"}}})).await;
}
async fn inspect(r: &Rig) -> StockRequestSnapshot {
    let (private, public, _inventory) = minima(r).await;
    r.store
        .stock_order_request(
            r.binding.clone(),
            _inventory,
            private,
            public,
            identity("request"),
            r.enrollment.proof(),
        )
        .await
        .unwrap()
}
async fn command(r: &Rig, id: u8, explicit: bool) -> CapturedOrderAcceptance {
    let snapshot = inspect(r).await;
    command_snapshot(r, id, explicit, &snapshot)
}
fn command_snapshot(
    r: &Rig,
    id: u8,
    explicit: bool,
    snapshot: &StockRequestSnapshot,
) -> CapturedOrderAcceptance {
    let mut allocation = serde_json::json!({"type":"automatic"});
    if explicit {
        allocation = serde_json::json!({"type":"explicit","allocations":[{"line_id":snapshot.terms().lines()[0].id(),"pool_id":pool(80),"quantity":{"amount":"1","unit":"each"},"expected_pool_version":"1"},{"line_id":snapshot.terms().lines()[0].id(),"pool_id":pool(81),"quantity":{"amount":"2","unit":"each"},"expected_pool_version":"1"}]});
    }
    let mut value = serde_json::json!({"type":"orders.accept","context":context(r,id),"input":{"request":snapshot.request(),"terms_digest":snapshot.terms_digest(),"allocation":allocation}});
    value["context"]["expected"] = serde_json::json!({"agreement":null,"buyer_head":snapshot.request(),"checkpoint":snapshot.checkpoint(),"order_id":snapshot.scope().order_id(),"seller_sequence":"0"});
    let captured = CapturedOrderAcceptance::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    value["context"]["approval"] = serde_json::json!({"intent_digest":captured.approval_intent_digest(),"scope":"orders.accept","expires_at":"230"});
    CapturedOrderAcceptance::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}
async fn inspect_named(r: &Rig, name: &str) -> StockRequestSnapshot {
    let selected: RecordId = data()["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == name)
        .unwrap()["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (private, public, _inventory) = minima(r).await;
    r.store
        .stock_order_request(
            r.binding.clone(),
            _inventory,
            private,
            public,
            selected,
            r.enrollment.proof(),
        )
        .await
        .unwrap()
}
async fn consent(r: &Rig, command: &CapturedOrderAcceptance) {
    let scope = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        r.enrollment.input().grant_id(),
        StockConsentMode::ExecuteAndRecover,
        StockResources::all_pools(),
        UnixSeconds::new(230).unwrap(),
        CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .issue_stock_consent(
                r.binding.clone(),
                inventory_minimum(r).await,
                &scope,
                r.enrollment.proof()
            )
            .await
            .unwrap(),
        StockConsentOutcome::Committed(_)
    ));
}
async fn request(r: &Rig, command: &CapturedOrderAcceptance) -> StockAcceptanceRequest {
    let (private, public, _inventory) = minima(r).await;
    StockAcceptanceRequest::capture(
        r.binding.clone(),
        inventory_minimum(r).await,
        private,
        public,
        command,
    )
    .unwrap()
}
async fn prepare(
    r: &Rig,
    command: &CapturedOrderAcceptance,
) -> Result<StockAcceptanceReceipt, OperationError> {
    let selected: &dyn InventoryStore = &r.store;
    let proof = r
        .enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
    match selected
        .prepare_stock_acceptance(request(r, command).await, proof)
        .await?
    {
        StockAcceptanceOutcome::Prepared(value) => Ok(*value),
        _ => panic!("T0 must confirm"),
    }
}

#[tokio::test]
async fn ordinary_outbox_refuses_shared_acceptance_without_creating_or_adopting_stock() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, true).await;
    let request = request(&r, &command).await;
    let (outbox, _, proof) = NativeWorkspace::initialize_local_outbox(
        &r.path.with_file_name("unsupported-acceptance-outbox"),
        crate::workspace::LocalOutboxConfig::seller(
            r.enrollment.input().participant(),
            r.enrollment.input().participant(),
            "synthetic-acceptance-outbox",
            UnixSeconds::new(500).unwrap(),
        )
        .unwrap(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    let selected: &dyn InventoryStore = &outbox;
    assert!(matches!(
        selected.prepare_stock_acceptance(request, proof).await,
        Err(OperationError::UnsupportedOperation)
    ));
    assert_pool(&r, 80, 10, 0, 1).await;
    outbox.close().await.unwrap();
    r.store.close().await.unwrap();
}
async fn assert_pool(r: &Rig, id: u8, available: u64, held: u64, version: u64) {
    let value = r
        .store
        .pool(
            r.binding.clone(),
            inventory_minimum(r).await,
            pool(id),
            r.enrollment
                .proof()
                .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    let snapshot = value.snapshot();
    assert_eq!(snapshot.version().get(), version);
    assert_eq!(snapshot.balances().physical().available.get(), available);
    assert_eq!(snapshot.balances().physical().preparatory_hold.get(), held);
    assert_eq!(snapshot.balances().introduced().get(), available + held);
}
#[tokio::test]
async fn actual_automatic_hold_retains_one_original_and_reopens_before_fresh_withdrawal_or_allocation()
 {
    let mut r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let original = prepare(&r, &command).await.unwrap();
    assert_eq!(original.held_pools().len(), 1);
    assert_eq!(original.allocation_count(), 1);
    assert_pool(&r, 80, 7, 3, 2).await;
    assert_pool(&r, 81, 10, 0, 1).await;
    retain(&r, "withdrawal").await;
    let replay = prepare(&r, &command).await.unwrap();
    assert_eq!(replay.attempt(), original.attempt());
    assert_eq!(replay.checkpoint(), original.checkpoint());
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        prepare(&r, &command).await.unwrap().attempt(),
        original.attempt()
    );
    assert_pool(&r, 80, 7, 3, 2).await;
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn actual_explicit_two_pool_hold_is_complete_and_second_decision_cannot_reuse_order() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let first = command(&r, 90, true).await;
    let second = command(&r, 91, false).await;
    consent(&r, &first).await;
    consent(&r, &second).await;
    let receipt = prepare(&r, &first).await.unwrap();
    assert_eq!(receipt.held_pools().len(), 2);
    assert_eq!(receipt.allocation_count(), 2);
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    assert_eq!(
        prepare(&r, &second).await.unwrap_err().code(),
        "stale_state"
    );
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    let mut raw = database(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_orders")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=2")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn insufficient_complete_allocation_changes_no_pool_and_writes_no_fence_or_operation() {
    let r = Rig::new().await;
    seed(&r, "1").await;
    let command = command(&r, 90, true).await;
    consent(&r, &command).await;
    let before = inventory_minimum(&r).await;
    assert_eq!(
        prepare(&r, &command).await.unwrap_err().code(),
        "stale_state"
    );
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 1, 0, 1).await;
    assert_pool(&r, 81, 1, 0, 1).await;
    let mut raw = database(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_orders")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=2")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        0
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}
#[tokio::test]
async fn unissued_capture_or_physical_inventory_rights_do_not_authorize_acceptance() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, false).await;
    assert_eq!(
        prepare(&r, &command).await.unwrap_err().code(),
        "unauthorized"
    );
    assert_pool(&r, 80, 10, 0, 1).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn each_actual_write_failure_rolls_back_all_pool_holds_and_order_fence() {
    for statement in [
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operations WHEN NEW.family=2 BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_operation_parts BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_pools WHEN hex(NEW.pool_id)=replace(hex(zeroblob(32)),'00','51') BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_orders BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE INSERT ON stock_order_states BEGIN SELECT RAISE(ABORT,'fixture'); END",
        "CREATE TRIGGER fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'fixture'); END",
    ] {
        let r = Rig::new().await;
        seed(&r, "10").await;
        let command = command(&r, 90, true).await;
        consent(&r, &command).await;
        let before = inventory_minimum(&r).await;
        let mut raw = database(&r).await;
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        assert_eq!(
            prepare(&r, &command).await.unwrap_err().code(),
            "storage_failure"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert_pool(&r, 80, 10, 0, 1).await;
        assert_pool(&r, 81, 10, 0, 1).await;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT (SELECT count(*) FROM stock_orders)+(SELECT count(*) FROM stock_order_states)")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
        sqlx::raw_sql("DROP TRIGGER fault")
            .execute(&mut raw)
            .await
            .unwrap();
        assert_eq!(prepare(&r, &command).await.unwrap().held_pools().len(), 2);
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn actual_commit_failure_returns_original_identity_and_requires_drain_before_recovery() {
    let mut r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, true).await;
    consent(&r, &command).await;
    let before = inventory_minimum(&r).await;
    let (private, public, _inventory) = minima(&r).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fault AFTER INSERT ON stock_orders BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut raw).await.unwrap();
    let StockAcceptanceOutcome::Unknown(attempt) = r
        .store
        .prepare_stock_acceptance(request(&r, &command).await, r.enrollment.proof())
        .await
        .unwrap()
    else {
        panic!("COMMIT must be uncertain");
    };
    assert_eq!(attempt.operation_id(), command.context().operation_id());
    assert_eq!(attempt.command_digest(), command.digest());
    assert!(attempt.transaction_id().is_some());
    assert_eq!(
        r.store
            .prepare_stock_acceptance(
                StockAcceptanceRequest::capture(
                    r.binding.clone(),
                    before,
                    private,
                    public,
                    &command
                )
                .unwrap(),
                r.enrollment.proof()
            )
            .await
            .unwrap_err()
            .code(),
        "recovery_required"
    );
    r.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fault; DROP TABLE deferred_fault")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(inventory_minimum(&r).await, before);
    assert_pool(&r, 80, 10, 0, 1).await;
    assert_pool(&r, 81, 10, 0, 1).await;
    assert_eq!(prepare(&r, &command).await.unwrap().held_pools().len(), 2);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn lost_observer_retains_actual_two_pool_preparation_and_drain_precedes_reopen() {
    let mut r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, true).await;
    consent(&r, &command).await;
    let request = request(&r, &command).await;
    let provider = reopen(&mut r).await;
    provider
        .gate
        .arm(Direction::Wrap, "radroots.seller_stock.authority.v16", 0);
    let mut pending = Box::pin(
        r.store
            .prepare_stock_acceptance(request, r.enrollment.proof()),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    provider.gate.entered();
    drop(pending);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &r.path,
            r.enrollment.input().clone(),
            r.fixture.crypto(),
            r.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    provider.gate.release();
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(prepare(&r, &command).await.unwrap().held_pools().len(), 2);
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn source_mapping_pool_order_and_grant_rows_are_rechecked_after_final_provider_io() {
    for statement in [
        "UPDATE stock_pools SET protected=randomblob(length(protected))",
        "UPDATE stock_mappings SET revision=revision+1",
        "UPDATE stock_source_records SET protected=randomblob(length(protected))",
        "INSERT INTO stock_source_records(record_id,protected) VALUES(randomblob(32),x'01')",
        "UPDATE stock_public_heads SET protected=randomblob(length(protected))",
        "UPDATE stock_grants SET protected=randomblob(length(protected))",
        "INSERT INTO stock_orders(actor,operation_id,order_id,revision,protected) SELECT actor,operation_id,randomblob(32),1,x'01' FROM stock_operations WHERE family=1 LIMIT 1",
    ] {
        let mut r = Rig::new().await;
        seed(&r, "10").await;
        let command = command(&r, 90, true).await;
        consent(&r, &command).await;
        let request = request(&r, &command).await;
        let provider = reopen(&mut r).await;
        let mut raw = database(&r).await;
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v16", 0);
        let mut pending = Box::pin(
            r.store
                .prepare_stock_acceptance(request, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        sqlx::raw_sql(statement).execute(&mut raw).await.unwrap();
        provider.gate.release();
        assert_eq!(
            pending.await.unwrap_err().code(),
            "recovery_required",
            "{statement}"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations WHERE family=2")
                .fetch_one(&mut raw)
                .await
                .unwrap(),
            0
        );
        raw.close().await.unwrap();
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn final_clock_recheck_refuses_expiry_and_rollback_after_protection_without_reserving() {
    for at in [149, 230, 300] {
        let mut r = Rig::new().await;
        seed(&r, "10").await;
        let command = command(&r, 90, true).await;
        consent(&r, &command).await;
        let request = request(&r, &command).await;
        let provider = reopen(&mut r).await;
        provider
            .gate
            .arm(Direction::Wrap, "radroots.seller_stock.authority.v16", 0);
        let mut pending = Box::pin(
            r.store
                .prepare_stock_acceptance(request, r.enrollment.proof()),
        );
        assert!(futures::poll!(pending.as_mut()).is_pending());
        provider.gate.entered();
        r.clock.0.store(at, Ordering::SeqCst);
        provider.gate.release();
        let error = pending.await.unwrap_err();
        assert_eq!(
            error.code(),
            match at {
                149 => "workspace_unavailable",
                230 => "deadline_exceeded",
                _ => "grant_expired",
            }
        );
        r.clock.0.store(150, Ordering::SeqCst);
        assert_pool(&r, 80, 10, 0, 1).await;
        assert_pool(&r, 81, 10, 0, 1).await;
        r.store.close().await.unwrap();
    }
}

#[tokio::test]
async fn competing_orders_share_one_pool_and_cannot_both_acquire_its_last_available_stock() {
    let r = Rig::new().await;
    seed(&r, "4").await;
    retain(&r, "request_other_order").await;
    let first = command(&r, 90, false).await;
    let second = command_snapshot(
        &r,
        91,
        false,
        &inspect_named(&r, "request_other_order").await,
    );
    consent(&r, &first).await;
    consent(&r, &second).await;
    let a = request(&r, &first).await;
    let b = request(&r, &second).await;
    let (a, b) = futures::join!(
        r.store.prepare_stock_acceptance(a, r.enrollment.proof()),
        r.store.prepare_stock_acceptance(b, r.enrollment.proof())
    );
    assert!(matches!(a.unwrap(), StockAcceptanceOutcome::Prepared(_)));
    assert_eq!(b.unwrap_err().code(), "stale_state");
    assert_pool(&r, 80, 1, 3, 2).await;
    let mut raw = database(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_orders")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn explicitly_authorized_future_promise_fences_the_order_without_inventing_a_stock_hold() {
    let r = Rig::new().await;
    r.retain(&r.capture("listing_fixed")).await;
    retain(&r, "request_future_promise").await;
    let snapshot = inspect_named(&r, "request_future_promise").await;
    let command = command_snapshot(&r, 90, false, &snapshot);
    consent(&r, &command).await;
    let receipt = prepare(&r, &command).await.unwrap();
    assert!(receipt.held_pools().is_empty());
    assert_eq!(receipt.allocation_count(), 0);
    assert_eq!(
        prepare(&r, &command).await.unwrap().attempt(),
        receipt.attempt()
    );
    let mut raw = database(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_pools")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_movements")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_orders")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn variable_weight_reserves_the_whole_authorized_line_maximum_using_explicit_units() {
    let r = Rig::new().await;
    r.retain(&r.capture("listing_fixed")).await;
    retain(&r, "request_variable_weight").await;
    physical(&r,serde_json::json!({"type":"stock.create_pool","context":context(&r,80),"input":{"id":pool(80),"label":"measured produce","unit":"kg","tick_scale":"3","initial_quantity":"10"}})).await;
    let snapshot = inspect_named(&r, "request_variable_weight").await;
    let line = &snapshot.terms().lines()[0];
    physical(&r,serde_json::json!({"type":"stock.map_option","context":context(&r,82),"input":{"listing":line.source().coordinate(),"option_id":line.option_id(),"pool_id":pool(80),"expected_mapping_version":"0","per_sale_unit":{"amount":"0.5","unit":"kg"}}})).await;
    let command = command_snapshot(&r, 90, false, &snapshot);
    consent(&r, &command).await;
    let receipt = prepare(&r, &command).await.unwrap();
    assert_eq!(receipt.held_pools().len(), 1);
    // Three nominal bunches at0.5kg are1.5kg; this request authorizes an
    // entire line maximum of2kg, which is the actual preparatory backing.
    assert_pool(&r, 80, 8000, 2000, 2).await;
    assert_eq!(
        prepare(&r, &command).await.unwrap().attempt(),
        receipt.attempt()
    );
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn current_recovery_only_consent_recovers_original_after_deadlines_without_fresh_holds() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, true).await;
    consent(&r, &command).await;
    let original = prepare(&r, &command).await.unwrap();
    r.clock.0.store(250, Ordering::SeqCst);
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "recovery operator".into(),
        UnixSeconds::new(290).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .enroll_stock_grant(
                r.binding.clone(),
                inventory_minimum(&r).await,
                r.enrollment.proof(),
                &child
            )
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    let binding = r
        .store
        .authenticate(child.input().clone(), child.proof())
        .await
        .unwrap();
    let capture = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        child.input().grant_id(),
        StockConsentMode::RecoverOnly,
        StockResources::all_pools(),
        UnixSeconds::new(280).unwrap(),
        CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .issue_stock_consent(
                r.binding.clone(),
                inventory_minimum(&r).await,
                &capture,
                r.enrollment.proof()
            )
            .await
            .unwrap(),
        StockConsentOutcome::Committed(_)
    ));
    let (private, public, _inventory) = minima(&r).await;
    let request = StockAcceptanceRequest::capture(
        binding,
        inventory_minimum(&r).await,
        private,
        public,
        &command,
    )
    .unwrap();
    let StockAcceptanceOutcome::Prepared(recovered) = r
        .store
        .prepare_stock_acceptance(request, child.proof())
        .await
        .unwrap()
    else {
        panic!();
    };
    assert_eq!(recovered.attempt(), original.attempt());
    assert_eq!(recovered.held_pools(), original.held_pools());
    assert_pool(&r, 80, 9, 1, 2).await;
    assert_pool(&r, 81, 8, 2, 2).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn recovery_only_scope_never_admits_a_missing_original_operation() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let command = command(&r, 90, false).await;
    let scope = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        r.enrollment.input().grant_id(),
        StockConsentMode::RecoverOnly,
        StockResources::all_pools(),
        UnixSeconds::new(230).unwrap(),
        CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        r.store
            .issue_stock_consent(
                r.binding.clone(),
                inventory_minimum(&r).await,
                &scope,
                r.enrollment.proof()
            )
            .await
            .unwrap(),
        StockConsentOutcome::Committed(_)
    ));
    assert_eq!(
        prepare(&r, &command).await.unwrap_err().code(),
        "unauthorized"
    );
    assert_pool(&r, 80, 10, 0, 1).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn explicit_incomplete_excess_unknown_line_incompatible_units_and_stale_versions_refuse_atomically()
 {
    for (path, value, code) in [
        (
            "/input/allocation/allocations/1/quantity/amount",
            serde_json::json!("1"),
            "invalid_command",
        ),
        (
            "/input/allocation/allocations/1/quantity/amount",
            serde_json::json!("3"),
            "invalid_command",
        ),
        (
            "/input/allocation/allocations/1/line_id",
            serde_json::json!("fa".repeat(32)),
            "invalid_command",
        ),
        (
            "/input/allocation/allocations/1/quantity/unit",
            serde_json::json!("kg"),
            "invalid_command",
        ),
        (
            "/input/allocation/allocations/1/expected_pool_version",
            serde_json::json!("2"),
            "stale_state",
        ),
    ] {
        let r = Rig::new().await;
        seed(&r, "10").await;
        let original = command(&r, 90, true).await;
        let mut raw: serde_json::Value = serde_json::from_slice(original.protected_wire()).unwrap();
        *raw.pointer_mut(path).unwrap() = value;
        raw["context"]["approval"] = serde_json::Value::Null;
        let unsigned = CapturedOrderAcceptance::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
        raw["context"]["approval"] = serde_json::json!({"intent_digest":unsigned.approval_intent_digest(),"scope":"orders.accept","expires_at":"230"});
        let command = CapturedOrderAcceptance::parse(&serde_json::to_vec(&raw).unwrap()).unwrap();
        consent(&r, &command).await;
        let before = inventory_minimum(&r).await;
        assert_eq!(
            prepare(&r, &command).await.unwrap_err().code(),
            code,
            "{path}"
        );
        assert_eq!(inventory_minimum(&r).await, before);
        assert_pool(&r, 80, 10, 0, 1).await;
        assert_pool(&r, 81, 10, 0, 1).await;
        r.store.close().await.unwrap();
    }
}
