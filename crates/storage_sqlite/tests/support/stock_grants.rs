use super::*;
use radroots_domain::TransactionId;
use radroots_storage::workspace::seller_stock::{
    SellerAuthoritySelection, StockAction, StockGrantScope, StockResources,
};
use radroots_storage_sqlite::workspace::{StockGrantEnrollment, StockGrantOutcome};

#[tokio::test]
async fn cancelled_enrollment_drains_and_recovers_the_original_grant_transaction() {
    let f = Fixture::new();
    let enrollment = StockEnrollment::prepare(
        SellerStockConfig::new(key(), key(), "owner", UnixSeconds::new(200).unwrap()).unwrap(),
        f.clock.as_ref(),
    )
    .unwrap();
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let store =
        NativeWorkspace::initialize_seller_stock(&f.path, &enrollment, f.crypto(), clock.clone())
            .await
            .unwrap();
    let owner = store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    let current = minimum(&owner);
    let delegated = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(owner.input()),
        "cancelled-device".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    clock.arm();
    let mut operation =
        Box::pin(store.enroll_stock_grant(owner.clone(), current, enrollment.proof(), &delegated));
    assert!(futures::poll!(operation.as_mut()).is_pending());
    clock.entered();
    drop(operation);
    let mut closing = Box::pin(store.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    drop(closing);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(
            &f.path,
            enrollment.input().clone(),
            f.crypto(),
            f.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    clock.release();
    store.close().await.unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &f.path,
        delegated.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    assert!(
        reopened
            .authenticate(delegated.input().clone(), delegated.proof())
            .await
            .is_ok()
    );
    let original = grant_committed(
        reopened
            .enroll_stock_grant(owner, current, enrollment.proof(), &delegated)
            .await
            .unwrap(),
    );
    assert_eq!(original.transaction_id(), delegated.transaction_id());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn grant_capacity_is_explicit_and_revoked_credentials_are_never_evicted_to_admit_new_grants()
{
    let f = Fixture::new();
    let (store, enrollment, owner) = create(&f).await;
    let namespace = SellerAuthoritySelection::from_attachment(owner.input());
    let mut current = minimum(&owner);
    let mut first = None;
    for index in 0..63 {
        let grant = StockGrantEnrollment::prepare(
            &namespace,
            format!("bounded-device-{index}"),
            UnixSeconds::new(200).unwrap(),
            StockGrantScope::full_inventory(),
        )
        .unwrap();
        let receipt = grant_committed(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &grant)
                .await
                .unwrap(),
        );
        current = current.with_authorization(receipt.authorization());
        if index == 0 {
            first = Some(grant);
        }
    }
    let first = first.unwrap();
    let receipt = grant_committed(
        store
            .revoke_stock_grant(
                owner.clone(),
                current,
                enrollment.proof(),
                TransactionId::from_bytes([111; 32]),
                first.input().grant_id(),
            )
            .await
            .unwrap(),
    );
    current = current.with_authorization(receipt.authorization());
    let over = StockGrantEnrollment::prepare(
        &namespace,
        "over-capacity".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert_eq!(
        store
            .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &over)
            .await,
        Err(WorkspaceError::Capacity)
    );
    assert_eq!(
        store
            .authenticate(first.input().clone(), first.proof())
            .await,
        Err(WorkspaceError::GrantRevoked)
    );
    assert_eq!(
        checkpoint(&store, owner.input(), enrollment.proof()).await,
        current
    );
    store.close().await.unwrap();
}

struct RegressingClock {
    calls: std::sync::atomic::AtomicUsize,
    regress_at: std::sync::atomic::AtomicUsize,
}
impl RegressingClock {
    fn new() -> Self {
        Self {
            calls: std::sync::atomic::AtomicUsize::new(0),
            regress_at: std::sync::atomic::AtomicUsize::new(usize::MAX),
        }
    }
    fn arm(&self, at: usize) {
        self.calls.store(0, Ordering::SeqCst);
        self.regress_at.store(at, Ordering::SeqCst);
    }
}
impl WorkspaceClock for RegressingClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(
            UnixSeconds::new(if count >= self.regress_at.load(Ordering::SeqCst) {
                50
            } else {
                100
            })
            .unwrap(),
        )
    }
}
#[tokio::test]
async fn current_grant_clock_regression_refuses_fresh_and_retained_administrative_completion() {
    for regress_at in [1, 3] {
        let f = Fixture::new();
        let clock = Arc::new(RegressingClock::new());
        let enrollment = StockEnrollment::prepare(
            SellerStockConfig::new(key(), key(), "owner", UnixSeconds::new(200).unwrap()).unwrap(),
            clock.as_ref(),
        )
        .unwrap();
        let store = NativeWorkspace::initialize_seller_stock(
            &f.path,
            &enrollment,
            f.crypto(),
            clock.clone(),
        )
        .await
        .unwrap();
        let owner = store
            .authenticate(enrollment.input().clone(), enrollment.proof())
            .await
            .unwrap();
        let current = minimum(&owner);
        let grant = StockGrantEnrollment::prepare(
            &SellerAuthoritySelection::from_attachment(owner.input()),
            "clock-device".into(),
            UnixSeconds::new(200).unwrap(),
            StockGrantScope::full_inventory(),
        )
        .unwrap();
        clock.arm(regress_at);
        assert_eq!(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &grant)
                .await,
            Err(WorkspaceError::Unavailable)
        );
        clock.arm(usize::MAX);
        assert_eq!(
            checkpoint(&store, owner.input(), enrollment.proof()).await,
            current
        );
        let first = grant_committed(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &grant)
                .await
                .unwrap(),
        );
        clock.arm(2);
        assert_eq!(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &grant)
                .await,
            Err(WorkspaceError::Unavailable)
        );
        clock.arm(usize::MAX);
        assert_eq!(
            grant_committed(
                store
                    .enroll_stock_grant(owner, current, enrollment.proof(), &grant)
                    .await
                    .unwrap()
            ),
            first
        );
        store.close().await.unwrap();
    }
}

fn grant_committed(
    value: StockGrantOutcome,
) -> radroots_storage_sqlite::workspace::StockGrantReceipt {
    match value {
        StockGrantOutcome::Committed(v) => v,
        StockGrantOutcome::Unknown(_) => panic!("expected confirmed grant mutation"),
    }
}
async fn checkpoint(
    store: &NativeWorkspace,
    input: &radroots_storage::workspace::AttachmentInput,
    proof: &AttachmentProof,
) -> InventoryCheckpointRef {
    store
        .stock_authority(input.clone(), proof)
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap()
}
#[tokio::test]
async fn scoped_grants_execute_and_replay_original_inventory_without_reallocation_and_revoke_current_access()
 {
    let f = Fixture::new();
    let (store, enrollment, owner) = create(&f).await;
    let initial = minimum(&owner);
    let original = request(&owner, 91, 92, "scoped original");
    let digest = original.command().digest();
    let captured = original.command().protected_wire().to_vec();
    let effect = committed(
        store
            .execute_inventory(original, proof(enrollment.proof()))
            .await
            .unwrap(),
    );
    let namespace = SellerAuthoritySelection::from_attachment(owner.input());
    let delegated = StockGrantEnrollment::prepare(
        &namespace,
        "scoped-device".to_owned(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::new(
            StockResources::pools(vec![PoolId::from_bytes([92; 32])]).unwrap(),
            vec![
                StockAction::ReadPools,
                StockAction::ReadOperations,
                StockAction::AddStock,
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let added = grant_committed(
        store
            .enroll_stock_grant(
                owner.clone(),
                effect.checkpoint(),
                enrollment.proof(),
                &delegated,
            )
            .await
            .unwrap(),
    );
    assert_eq!(added.transaction_id(), delegated.transaction_id());
    assert_eq!(added.authorization().generation().get(), 2);
    let current = checkpoint(&store, owner.input(), enrollment.proof()).await;
    assert_eq!(current.authorization(), Some(added.authorization()));
    let device = store
        .authenticate(delegated.input().clone(), delegated.proof())
        .await
        .unwrap();
    // Old acknowledged grant state cannot be reused after mutation.
    assert_eq!(
        store
            .pool(
                owner.clone(),
                initial,
                PoolId::from_bytes([92; 32]),
                proof(enrollment.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    );
    assert!(
        store
            .pool(
                device.clone(),
                current,
                PoolId::from_bytes([92; 32]),
                proof(delegated.proof())
            )
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        store
            .pool(
                device.clone(),
                current,
                PoolId::from_bytes([93; 32]),
                proof(delegated.proof())
            )
            .await,
        Err(OperationError::Unauthorized)
    );
    let retained = store
        .inventory_operation(
            device.clone(),
            current,
            OperationId::from_bytes([91; 32]),
            Some(digest),
            proof(delegated.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained, effect);
    let replay =
        InventoryRequest::replay_from_protected_wire(device.clone(), current, &captured).unwrap();
    assert!(InventoryRequest::from_protected_wire(device.clone(), current, &captured).is_err());
    let unseen = request(&owner, 99, 92, "missing retained command");
    let unseen = InventoryRequest::replay_from_protected_wire(
        device.clone(),
        current,
        unseen.command().protected_wire(),
    )
    .unwrap();
    assert_eq!(
        store
            .execute_inventory(unseen, proof(delegated.proof()))
            .await,
        Err(OperationError::RecoveryRequired)
    );
    assert_eq!(
        committed(
            store
                .execute_inventory(replay, proof(delegated.proof()))
                .await
                .unwrap()
        ),
        effect
    );
    // Reusing the same operation with another grant creates a different exact
    // command digest and cannot acquire a second stock effect.
    let conflict = request(&device, 91, 92, "scoped original");
    let conflict = InventoryRequest::from_protected_wire(
        device.clone(),
        current,
        conflict.command().protected_wire(),
    )
    .unwrap();
    assert_eq!(
        store
            .execute_inventory(conflict, proof(delegated.proof()))
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    let forbidden = request(&device, 94, 92, "cannot create");
    let forbidden = InventoryRequest::from_protected_wire(
        device.clone(),
        current,
        forbidden.command().protected_wire(),
    )
    .unwrap();
    assert_eq!(
        store
            .execute_inventory(forbidden, proof(delegated.proof()))
            .await,
        Err(OperationError::Unauthorized)
    );
    let another = StockGrantEnrollment::prepare(
        &namespace,
        "no-admin".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert_eq!(
        store
            .enroll_stock_grant(device.clone(), current, delegated.proof(), &another)
            .await,
        Err(WorkspaceError::Unauthorized)
    );
    let context = CommandContextData::new(
        OperationId::from_bytes([96; 32]),
        device.actor(),
        device.input().workspace_id(),
        device.input().authority(),
        device.input().route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let mut wire = serde_json::json!({"type":"stock.adjust","context":context,"input":{"pool_id":PoolId::from_bytes([92;32]),"expected_version":"1","direction":"add","amount":"0.125","reason":"delegated harvest"}});
    let add = InventoryRequest::from_protected_wire(
        device.clone(),
        current,
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    let added_stock = committed(
        store
            .execute_inventory(add, proof(delegated.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(
        added_stock.pool().balances().physical().available.get(),
        375
    );
    wire["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([97; 32]));
    wire["input"]["expected_version"] = serde_json::json!("2");
    wire["input"]["direction"] = serde_json::json!("remove");
    let remove = InventoryRequest::from_protected_wire(
        device.clone(),
        added_stock.checkpoint(),
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .execute_inventory(remove, proof(delegated.proof()))
            .await,
        Err(OperationError::Unauthorized)
    );
    let current = added_stock.checkpoint();
    let revoked = grant_committed(
        store
            .revoke_stock_grant(
                owner.clone(),
                current,
                enrollment.proof(),
                TransactionId::from_bytes([95; 32]),
                delegated.input().grant_id(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(revoked.authorization().generation().get(), 3);
    assert_eq!(
        store
            .authenticate(delegated.input().clone(), delegated.proof())
            .await,
        Err(WorkspaceError::GrantRevoked)
    );
    let next = current.with_authorization(revoked.authorization());
    assert_eq!(
        store
            .inventory_operation(
                device,
                next,
                OperationId::from_bytes([91; 32]),
                Some(digest),
                proof(delegated.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    );
    let mut connection = database(&f).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grant_changes")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 2);
    connection.close().await.unwrap();
    store.close().await.unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &f.path,
        delegated.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        reopened
            .authenticate(delegated.input().clone(), delegated.proof())
            .await,
        Err(WorkspaceError::GrantRevoked)
    );
    let owner = reopened
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    assert!(
        reopened
            .pool(
                owner,
                next,
                PoolId::from_bytes([92; 32]),
                proof(enrollment.proof())
            )
            .await
            .unwrap()
            .is_some()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn acknowledged_revocation_refuses_a_complete_older_database_and_partial_grant_restore() {
    let f = Fixture::new();
    let (store, enrollment, owner) = create(&f).await;
    let pool = committed(
        store
            .execute_inventory(
                request(&owner, 101, 102, "restore stock"),
                proof(enrollment.proof()),
            )
            .await
            .unwrap(),
    );
    let delegated = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(owner.input()),
        "restore-device".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    grant_committed(
        store
            .enroll_stock_grant(
                owner.clone(),
                pool.checkpoint(),
                enrollment.proof(),
                &delegated,
            )
            .await
            .unwrap(),
    );
    let before = checkpoint(&store, owner.input(), enrollment.proof()).await;
    store.close().await.unwrap();
    let original = f.path.join("stock_execution_v1.sqlite");
    let backup = f
        .path
        .parent()
        .unwrap()
        .join("synthetic-before-revocation.sqlite");
    std::fs::copy(&original, &backup).unwrap();
    let store = NativeWorkspace::open_seller_stock(
        &f.path,
        enrollment.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    let owner = store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    let mut connection = database(&f).await;
    let old_grant: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM stock_grants WHERE grant_id=?")
            .bind(delegated.input().grant_id().as_bytes().as_slice())
            .fetch_one(&mut connection)
            .await
            .unwrap();
    let receipt = grant_committed(
        store
            .revoke_stock_grant(
                owner.clone(),
                before,
                enrollment.proof(),
                TransactionId::from_bytes([103; 32]),
                delegated.input().grant_id(),
            )
            .await
            .unwrap(),
    );
    let minimum = before.with_authorization(receipt.authorization());
    // Restoring just the old encrypted grant is caught by the protected root's
    // grant-set digest, even with a syntactically valid authentic envelope.
    sqlx::query("UPDATE stock_grants SET protected=? WHERE grant_id=?")
        .bind(old_grant)
        .bind(delegated.input().grant_id().as_bytes().as_slice())
        .execute(&mut connection)
        .await
        .unwrap();
    assert_eq!(
        store
            .authenticate(enrollment.input().clone(), enrollment.proof())
            .await,
        Err(WorkspaceError::RecoveryRequired)
    );
    connection.close().await.unwrap();
    store.close().await.unwrap();
    std::fs::copy(&backup, &original).unwrap();
    let restored = NativeWorkspace::open_seller_stock(
        &f.path,
        enrollment.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    let binding = restored
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    assert_eq!(
        restored
            .pool(
                binding,
                minimum,
                PoolId::from_bytes([102; 32]),
                proof(enrollment.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    );
    restored.close().await.unwrap();
}

#[tokio::test]
async fn grant_mutation_faults_roll_back_all_rows_and_original_transaction_replays() {
    for table in ["stock_grants", "stock_grant_changes", "stock_authority"] {
        let f = Fixture::new();
        let (store, enrollment, owner) = create(&f).await;
        let current = minimum(&owner);
        let delegated = StockGrantEnrollment::prepare(
            &SellerAuthoritySelection::from_attachment(owner.input()),
            "fault-device".into(),
            UnixSeconds::new(200).unwrap(),
            StockGrantScope::full_inventory(),
        )
        .unwrap();
        let mut connection = database(&f).await;
        let sql = match table {
            "stock_grants" => {
                "CREATE TRIGGER grant_fault BEFORE INSERT ON stock_grants BEGIN SELECT RAISE(ABORT,'synthetic grant failure'); END"
            }
            "stock_grant_changes" => {
                "CREATE TRIGGER grant_fault BEFORE INSERT ON stock_grant_changes BEGIN SELECT RAISE(ABORT,'synthetic grant failure'); END"
            }
            "stock_authority" => {
                "CREATE TRIGGER grant_fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'synthetic grant failure'); END"
            }
            _ => unreachable!(),
        };
        sqlx::raw_sql(sql).execute(&mut connection).await.unwrap();
        assert_eq!(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &delegated)
                .await,
            Err(WorkspaceError::StorageFailure)
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grants")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(count, 1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grant_changes")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(
            checkpoint(&store, owner.input(), enrollment.proof()).await,
            current
        );
        sqlx::raw_sql("DROP TRIGGER grant_fault")
            .execute(&mut connection)
            .await
            .unwrap();
        let first = grant_committed(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &delegated)
                .await
                .unwrap(),
        );
        // Exact original identity and pre-state recover a successful attempt,
        // including a lost response, without installing another grant or row.
        let second = grant_committed(
            store
                .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &delegated)
                .await
                .unwrap(),
        );
        assert_eq!(first, second);
        assert_eq!(first.transaction_id(), delegated.transaction_id());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_grant_changes")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(count, 1);
        connection.close().await.unwrap();
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn unknown_grant_commit_fences_the_worker_and_reopen_recovers_the_original_prepared_identity()
{
    let f = Fixture::new();
    let (store, enrollment, owner) = create(&f).await;
    let current = minimum(&owner);
    let delegated = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(owner.input()),
        "unknown-device".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    let mut connection = database(&f).await;
    sqlx::raw_sql("CREATE TABLE grant_commit_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER grant_commit_failure AFTER INSERT ON stock_grant_changes BEGIN INSERT INTO grant_commit_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    assert_eq!(
        store
            .enroll_stock_grant(owner.clone(), current, enrollment.proof(), &delegated)
            .await
            .unwrap(),
        StockGrantOutcome::Unknown(delegated.transaction_id())
    );
    assert_eq!(
        store
            .authenticate(enrollment.input().clone(), enrollment.proof())
            .await,
        Err(WorkspaceError::RecoveryRequired)
    );
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER grant_commit_failure; DROP TABLE grant_commit_fault")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &f.path,
        enrollment.input().clone(),
        f.crypto(),
        f.clock.clone(),
    )
    .await
    .unwrap();
    let binding = reopened
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    assert_eq!(
        checkpoint(&reopened, binding.input(), enrollment.proof()).await,
        current
    );
    let result = grant_committed(
        reopened
            .enroll_stock_grant(binding, current, enrollment.proof(), &delegated)
            .await
            .unwrap(),
    );
    assert_eq!(result.transaction_id(), delegated.transaction_id());
    reopened.close().await.unwrap();
}
