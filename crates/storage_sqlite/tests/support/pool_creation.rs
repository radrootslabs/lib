use super::*;
use radroots_domain::{
    CommandContextData, Decimal, OperationId, PoolId, Quantity, listing::MeasurementUnit,
    stock::PoolUnit,
};
use radroots_protocol::command::{CapturedPoolCreation, CreatePoolInput};
use radroots_storage::{
    command::OperationError,
    inventory::{
        CreatePoolRequest, InventoryCheckpointRef, InventoryStore, PoolCreationOutcome,
        PoolCreationReceipt,
    },
};
use radroots_storage_sqlite::workspace::{SellerStockConfig, StockEnrollment};
use sqlx::Connection;
fn proof(value: &AttachmentProof) -> AttachmentProof {
    value
        .with_secret(|b| AttachmentProof::from_protected_input(b))
        .unwrap()
}
fn minimum(binding: &radroots_storage::workspace::WorkspaceBinding) -> InventoryCheckpointRef {
    InventoryCheckpointRef::from_host_metadata(
        binding.input().authority(),
        radroots_domain::Sequence::new(0).unwrap(),
        None,
    )
    .unwrap()
}
fn request(
    binding: &radroots_storage::workspace::WorkspaceBinding,
    operation: u8,
    pool: u8,
    label: &str,
) -> CreatePoolRequest {
    let input = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([operation; 32]),
        binding.actor(),
        input.workspace_id(),
        input.authority(),
        input.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let quantity = Quantity::new(
        "0.25".parse::<Decimal>().unwrap(),
        MeasurementUnit::Kilogram,
    )
    .unwrap();
    CreatePoolRequest::new(
        binding.clone(),
        minimum(binding),
        CapturedPoolCreation::new(
            context,
            CreatePoolInput::new(
                PoolId::from_bytes([pool; 32]),
                label,
                PoolUnit::new(MeasurementUnit::Kilogram, 3).unwrap(),
                quantity,
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
async fn create(
    f: &Fixture,
) -> (
    NativeWorkspace,
    StockEnrollment,
    radroots_storage::workspace::WorkspaceBinding,
) {
    let enrollment = StockEnrollment::prepare(
        SellerStockConfig::new(
            key(),
            key(),
            "inventory-owner",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        f.clock.as_ref(),
    )
    .unwrap();
    let store =
        NativeWorkspace::initialize_seller_stock(&f.path, &enrollment, f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let binding = store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    (store, enrollment, binding)
}
fn committed(outcome: PoolCreationOutcome) -> PoolCreationReceipt {
    match outcome {
        PoolCreationOutcome::Committed(receipt) => *receipt,
        PoolCreationOutcome::Unknown(_) => panic!("expected confirmed inventory commit"),
    }
}
async fn database(f: &Fixture) -> sqlx::SqliteConnection {
    sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.path.join("stock_execution_v1.sqlite"))
            .foreign_keys(true),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn real_pool_creation_commits_reopens_and_replays_original_effect_after_command_expiry() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let original = request(&binding, 1, 2, "Carrots");
    let digest = original.command().digest();
    let receipt = committed(store.create_pool(original, proof(e.proof())).await.unwrap());
    assert_eq!(receipt.pool().balances().physical().available.get(), 250);
    assert_eq!(receipt.pool().balances().introduced().get(), 250);
    assert_eq!(receipt.pool().unit().scale(), 3);
    assert_eq!(receipt.pool().version().get(), 1);
    assert_eq!(
        store
            .pool(
                binding.clone(),
                minimum(&binding),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .map(|view| {
                assert_eq!(view.label(), "Carrots");
                view.snapshot()
            }),
        Some(receipt.pool())
    );
    assert_eq!(
        store
            .pool_creation(
                binding.clone(),
                minimum(&binding),
                OperationId::from_bytes([1; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(receipt.clone())
    );
    f.clock.0.store(175, Ordering::SeqCst);
    assert_eq!(
        committed(
            store
                .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
                .await
                .unwrap()
        ),
        receipt
    );
    assert!(matches!(
        store
            .create_pool(request(&binding, 1, 2, "Changed"), proof(e.proof()))
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        store
            .create_pool(request(&binding, 3, 4, "Late"), proof(e.proof()))
            .await,
        Err(OperationError::DeadlineExceeded)
    ));
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .pool_creation(
                binding.clone(),
                minimum(&binding),
                OperationId::from_bytes([1; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    store.close().await.unwrap();
    let mut connection = database(&f).await;
    for table in ["stock_pools", "stock_operations", "stock_movements"] {
        let sql = format!("SELECT count(*) FROM {table}");
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
    let protected: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_pools")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&protected).contains("Carrots"));
    connection.close().await.unwrap();
}
#[tokio::test]
async fn each_actual_inventory_write_failure_rolls_back_pool_journal_movement_and_root() {
    for (table, statement) in [
        (
            "stock_pools",
            "CREATE TRIGGER fail_inventory BEFORE INSERT ON stock_pools BEGIN SELECT RAISE(ABORT,'isolated fault'); END",
        ),
        (
            "stock_operations",
            "CREATE TRIGGER fail_inventory BEFORE INSERT ON stock_operations BEGIN SELECT RAISE(ABORT,'isolated fault'); END",
        ),
        (
            "stock_movements",
            "CREATE TRIGGER fail_inventory BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT,'isolated fault'); END",
        ),
        (
            "stock_authority",
            "CREATE TRIGGER fail_inventory BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'isolated fault'); END",
        ),
    ] {
        let f = Fixture::new();
        let (store, e, binding) = create(&f).await;
        let mut connection = database(&f).await;
        sqlx::raw_sql(statement)
            .execute(&mut connection)
            .await
            .unwrap();
        assert!(
            store
                .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
                .await
                .is_err(),
            "{table}"
        );
        for name in ["stock_pools", "stock_operations", "stock_movements"] {
            let sql = format!("SELECT count(*) FROM {name}");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(&mut connection)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table}/{name}");
        }
        store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap();
        sqlx::raw_sql("DROP TRIGGER fail_inventory")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        store.close().await.unwrap();
        let store = NativeWorkspace::open_seller_stock(
            &f.path,
            e.input().clone(),
            f.crypto(),
            f.clock.clone(),
        )
        .await
        .unwrap();
        assert!(
            store
                .pool(
                    binding.clone(),
                    minimum(&binding),
                    PoolId::from_bytes([2; 32]),
                    proof(e.proof())
                )
                .await
                .unwrap()
                .is_none()
        );
        store.close().await.unwrap();
    }
}
#[tokio::test]
async fn actual_inventory_commit_failure_is_unknown_and_fenced_until_reopen() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let mut connection = database(&f).await;
    sqlx::raw_sql("CREATE TABLE deferred_inventory_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER inventory_commit_fault AFTER INSERT ON stock_movements BEGIN INSERT INTO deferred_inventory_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    let original = request(&binding, 1, 2, "Carrots");
    let digest = original.command().digest();
    let anchor = match store.create_pool(original, proof(e.proof())).await.unwrap() {
        PoolCreationOutcome::Unknown(anchor) => anchor,
        PoolCreationOutcome::Committed(_) => panic!("COMMIT must fail"),
    };
    assert_eq!(anchor.command_digest(), digest);
    assert!(anchor.transaction_id().is_some());
    assert!(matches!(
        store
            .pool_creation(
                binding.clone(),
                minimum(&binding),
                OperationId::from_bytes([1; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER inventory_commit_fault; DROP TABLE deferred_inventory_fault")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert!(
        store
            .pool_creation(
                binding.clone(),
                minimum(&binding),
                OperationId::from_bytes([1; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .pool(
                binding.clone(),
                minimum(&binding),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn sealed_root_detects_missing_inventory_history_instead_of_fresh_stock() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    committed(
        store
            .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let mut connection = database(&f).await;
    sqlx::raw_sql(
        "DELETE FROM stock_movements; DELETE FROM stock_operations; DELETE FROM stock_pools",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    connection.close().await.unwrap();
    assert!(
        store
            .pool(
                binding.clone(),
                minimum(&binding),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .is_err()
    );
    store.close().await.unwrap();
    assert!(
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn acknowledged_minimum_refuses_a_complete_older_database_snapshot() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    store.close().await.unwrap();
    let path = f.path.join("stock_execution_v1.sqlite");
    let original = std::fs::read(&path).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let receipt = committed(
        store
            .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let acknowledged = receipt.checkpoint();
    assert_eq!(acknowledged.revision().get(), 1);
    assert!(acknowledged.head().is_some());
    assert_eq!(
        store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap()
            .inventory_checkpoint(),
        Some(acknowledged)
    );
    store.close().await.unwrap();
    std::fs::write(&path, original).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert!(matches!(
        store
            .pool(
                binding.clone(),
                acknowledged,
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    assert!(matches!(
        store
            .pool_creation(
                binding.clone(),
                acknowledged,
                OperationId::from_bytes([1; 32]),
                Some(receipt.attempt().command_digest()),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    let captured = request(&binding, 3, 4, "New stock");
    let captured = CreatePoolRequest::new(
        binding,
        acknowledged,
        CapturedPoolCreation::parse(captured.command().protected_wire()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        store.create_pool(captured, proof(e.proof())).await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    store.close().await.unwrap();
    let mut connection = database(&f).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_pools")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 0);
    connection.close().await.unwrap();
}

#[tokio::test]
async fn an_empty_pool_is_a_zero_genesis_without_a_fictitious_movement() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let input = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([7; 32]),
        binding.actor(),
        input.workspace_id(),
        input.authority(),
        input.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let command = CapturedPoolCreation::new(
        context,
        CreatePoolInput::new(
            PoolId::from_bytes([8; 32]),
            "Empty",
            PoolUnit::new(MeasurementUnit::Each, 0).unwrap(),
            Quantity::new(Decimal::ZERO, MeasurementUnit::Each).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let digest = command.digest();
    let request = CreatePoolRequest::new(binding.clone(), minimum(&binding), command).unwrap();
    let receipt = committed(store.create_pool(request, proof(e.proof())).await.unwrap());
    assert_eq!(receipt.pool().balances().introduced().get(), 0);
    assert_eq!(receipt.checkpoint().revision().get(), 1);
    assert_eq!(
        store
            .pool_creation(
                binding.clone(),
                minimum(&binding),
                OperationId::from_bytes([7; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(receipt)
    );
    store.close().await.unwrap();
    let mut connection = database(&f).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_movements")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 0);
    connection.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .pool(
                binding.clone(),
                minimum(&binding),
                PoolId::from_bytes([8; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .snapshot()
            .balances()
            .introduced()
            .get(),
        0
    );
    store.close().await.unwrap();
}

struct AdmissionClock {
    armed: std::sync::atomic::AtomicBool,
    calls: AtomicU64,
    next: u64,
}
impl WorkspaceClock for AdmissionClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let next =
            if self.armed.load(Ordering::SeqCst) && self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
                self.next
            } else {
                100
            };
        if next == 0 {
            Err(WorkspaceError::Unavailable)
        } else {
            Ok(UnixSeconds::new(next).unwrap())
        }
    }
}
#[tokio::test]
async fn expiry_or_clock_failure_inside_inventory_transaction_has_no_effect() {
    for next in [0, 150, 200] {
        let f = Fixture::new();
        let clock = Arc::new(AdmissionClock {
            armed: std::sync::atomic::AtomicBool::new(false),
            calls: AtomicU64::new(0),
            next,
        });
        let e = StockEnrollment::prepare(
            SellerStockConfig::new(
                key(),
                key(),
                "inventory-owner",
                UnixSeconds::new(200).unwrap(),
            )
            .unwrap(),
            clock.as_ref(),
        )
        .unwrap();
        let store =
            NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock.clone())
                .await
                .unwrap();
        let binding = store
            .authenticate(e.input().clone(), e.proof())
            .await
            .unwrap();
        clock.armed.store(true, Ordering::SeqCst);
        let result = store
            .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await;
        match next {
            0 => assert!(matches!(
                result,
                Err(OperationError::Workspace(WorkspaceError::Unavailable))
            )),
            150 => assert!(matches!(result, Err(OperationError::DeadlineExceeded))),
            200 => assert!(matches!(
                result,
                Err(OperationError::Workspace(WorkspaceError::GrantExpired))
            )),
            _ => unreachable!(),
        }
        clock.armed.store(false, Ordering::SeqCst);
        assert_eq!(
            store
                .stock_authority(e.input().clone(), e.proof())
                .await
                .unwrap()
                .inventory_checkpoint()
                .unwrap()
                .revision()
                .get(),
            0
        );
        store.close().await.unwrap();
        let mut connection = database(&f).await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_pools")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(count, 0);
        connection.close().await.unwrap();
    }
}
#[tokio::test]
async fn admitted_inventory_work_survives_observer_and_close_cancellation() {
    let f = Fixture::new();
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let e = StockEnrollment::prepare(
        SellerStockConfig::new(
            key(),
            key(),
            "inventory-owner",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        clock.as_ref(),
    )
    .unwrap();
    let store = NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock.clone())
        .await
        .unwrap();
    let binding = store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    let original = request(&binding, 1, 2, "Carrots");
    let digest = original.command().digest();
    clock.arm();
    let mut future = store.create_pool(original, proof(e.proof()));
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
    let receipt = store
        .pool_creation(
            binding.clone(),
            minimum(&binding),
            OperationId::from_bytes([1; 32]),
            Some(digest),
            proof(e.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.pool().balances().physical().available.get(), 250);
    assert_eq!(receipt.checkpoint().revision().get(), 1);
    store.close().await.unwrap();
}

#[tokio::test]
async fn current_credentials_guard_pool_reads_replay_and_new_effects_after_revocation() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let receipt = committed(
        store
            .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let wrong = || AttachmentProof::from_protected_input(&[0xB7; 32]).unwrap();
    assert!(matches!(
        store
            .create_pool(request(&binding, 1, 2, "Carrots"), wrong())
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    assert!(matches!(
        store
            .pool(
                binding.clone(),
                receipt.checkpoint(),
                PoolId::from_bytes([2; 32]),
                wrong()
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    assert!(matches!(
        store
            .pool_creation(
                binding.clone(),
                receipt.checkpoint(),
                OperationId::from_bytes([1; 32]),
                None,
                wrong()
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    store.revoke(e.input().clone(), e.proof()).await.unwrap();
    assert!(matches!(
        store
            .create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert!(matches!(
        store
            .create_pool(request(&binding, 3, 4, "New"), proof(e.proof()))
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert!(matches!(
        store
            .pool(
                binding.clone(),
                receipt.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert!(matches!(
        store
            .pool_creation(
                binding,
                receipt.checkpoint(),
                OperationId::from_bytes([1; 32]),
                None,
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    store.close().await.unwrap();
    let mut conn = database(&f).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(count, 1);
    conn.close().await.unwrap();
}

#[tokio::test]
async fn simultaneous_pool_creations_serialize_identity_and_preserve_earlier_checkpoint_ancestry() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let (left, right) = tokio::join!(
        store.create_pool(request(&binding, 1, 2, "Carrots"), proof(e.proof())),
        store.create_pool(request(&binding, 3, 2, "Competing"), proof(e.proof()))
    );
    let first = match (left, right) {
        (Ok(value), Err(OperationError::StaleState))
        | (Err(OperationError::StaleState), Ok(value)) => committed(value),
        other => panic!("one serialized creation must win: {other:?}"),
    };
    let next = request(&binding, 5, 6, "Parsnips");
    let next = CreatePoolRequest::new(
        binding.clone(),
        first.checkpoint(),
        CapturedPoolCreation::parse(next.command().protected_wire()).unwrap(),
    )
    .unwrap();
    let second = committed(store.create_pool(next, proof(e.proof())).await.unwrap());
    assert_eq!(first.checkpoint().revision().get(), 1);
    assert_eq!(second.checkpoint().revision().get(), 2);
    assert_ne!(first.checkpoint().head(), second.checkpoint().head());
    assert_eq!(
        store
            .pool_creation(
                binding.clone(),
                second.checkpoint(),
                first.attempt().operation_id(),
                Some(first.attempt().command_digest()),
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(first.clone())
    );
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .pool(
                binding.clone(),
                first.checkpoint(),
                PoolId::from_bytes([6; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .label(),
        "Parsnips"
    );
    let conflicting = InventoryCheckpointRef::from_host_metadata(
        binding.input().authority(),
        radroots_domain::Sequence::new(1).unwrap(),
        second.checkpoint().head(),
    )
    .unwrap();
    assert!(matches!(
        store
            .pool(
                binding,
                conflicting,
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    store.close().await.unwrap();
    let mut conn = database(&f).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_pools")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_movements")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        2
    );
    conn.close().await.unwrap();
}

#[tokio::test]
async fn acknowledged_branch_cannot_be_satisfied_by_unprotected_history_index_fields() {
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    store.close().await.unwrap();
    let path = f.path.join("stock_execution_v1.sqlite");
    let genesis = std::fs::read(&path).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    let acknowledged = committed(
        store
            .create_pool(request(&binding, 1, 2, "First branch"), proof(e.proof()))
            .await
            .unwrap(),
    )
    .checkpoint();
    store.close().await.unwrap();
    // Simulate an older complete local backup and its divergent continuation.
    std::fs::write(&path, genesis).unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    store
        .create_pool(request(&binding, 3, 4, "Other branch"), proof(e.proof()))
        .await
        .unwrap();
    store
        .create_pool(
            request(&binding, 5, 6, "Later other branch"),
            proof(e.proof()),
        )
        .await
        .unwrap();
    store.close().await.unwrap();
    let mut conn = database(&f).await;
    sqlx::query("UPDATE stock_operations SET chain_head=? WHERE revision=1")
        .bind(acknowledged.head().unwrap().as_bytes().as_slice())
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert!(matches!(
        store
            .pool(
                binding,
                acknowledged,
                PoolId::from_bytes([4; 32]),
                proof(e.proof())
            )
            .await,
        Err(OperationError::RecoveryRequired)
            | Err(OperationError::Workspace(WorkspaceError::StaleCheckpoint))
    ));
    store.close().await.unwrap();
}
