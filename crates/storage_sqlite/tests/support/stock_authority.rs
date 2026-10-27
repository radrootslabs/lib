use super::*;
use radroots_storage::workspace::WorkspaceScope;
use radroots_storage_sqlite::workspace::{SellerStockConfig, StockEnrollment};
use sqlx::Connection;
fn enrollment(f: &Fixture) -> StockEnrollment {
    StockEnrollment::prepare(
        SellerStockConfig::new(
            key(),
            key(),
            "stock-owner-device",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        f.clock.as_ref(),
    )
    .unwrap()
}
async fn create(f: &Fixture, e: &StockEnrollment) -> NativeWorkspace {
    NativeWorkspace::initialize_seller_stock(&f.path, e, f.crypto(), f.clock.clone())
        .await
        .unwrap()
}
async fn reopen(f: &Fixture, e: &StockEnrollment) -> NativeWorkspace {
    NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
        .await
        .unwrap()
}
// Prepared outside the future under test so observer loss cannot discard a
// not-yet-submitted revocation. Production callers retain its actual receipt.
struct PreparedRevocation {
    binding: radroots_storage::workspace::WorkspaceBinding,
    minimum: radroots_storage::inventory::InventoryCheckpointRef,
    transaction: radroots_domain::TransactionId,
}
impl PreparedRevocation {
    async fn new(store: &NativeWorkspace, e: &StockEnrollment) -> Self {
        Self {
            binding: store
                .authenticate(e.input().clone(), e.proof())
                .await
                .unwrap(),
            minimum: store
                .stock_authority(e.input().clone(), e.proof())
                .await
                .unwrap()
                .inventory_checkpoint()
                .unwrap(),
            transaction: radroots_domain::TransactionId::from_bytes([0xAD; 32]),
        }
    }
    async fn run(
        &self,
        store: &NativeWorkspace,
        e: &StockEnrollment,
    ) -> Result<(), WorkspaceError> {
        match store
            .revoke_stock_grant(
                self.binding.clone(),
                self.minimum,
                e.proof(),
                self.transaction,
                e.input().grant_id(),
            )
            .await?
        {
            radroots_storage_sqlite::workspace::StockGrantOutcome::Committed(_) => Ok(()),
            radroots_storage_sqlite::workspace::StockGrantOutcome::Unknown(_) => {
                Err(WorkspaceError::CommitUnknown)
            }
        }
    }
}
#[tokio::test]
async fn explicit_stock_creation_has_retained_receipt_and_reopens() {
    let f = Fixture::new();
    let e = enrollment(&f);
    assert!(!f.path.exists());
    let store = create(&f, &e).await;
    let binding = store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(binding.scope(), WorkspaceScope::SellerStockOwner);
    assert_eq!(binding.actor().role(), radroots_domain::ActorRole::Seller);
    let receipt = store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(receipt.input(), e.input());
    assert_eq!(receipt.transaction_id(), e.transaction_id());
    assert_eq!(receipt.revocation_generation().get(), 1);
    assert!(f.path.join("stock_execution_v1.sqlite").exists());
    assert!(!f.path.join("execution_v1.sqlite").exists());
    store.close().await.unwrap();
    let store = reopen(&f, &e).await;
    assert_eq!(
        store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap(),
        receipt
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn proof_expiry_revocation_and_wrong_selection_are_current() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    let prepared_revocation = PreparedRevocation::new(&store, &e).await;
    let wrong = AttachmentProof::from_protected_input(&[0xAB; 32]).unwrap();
    assert_eq!(
        store.authenticate(e.input().clone(), &wrong).await,
        Err(WorkspaceError::Unauthorized)
    );
    for field in 0..9 {
        assert!(
            store
                .authenticate(altered(e.input(), field), e.proof())
                .await
                .is_err()
        );
    }
    f.clock.0.store(200, Ordering::SeqCst);
    assert_eq!(
        store.authenticate(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::GrantExpired)
    );
    f.clock.0.store(100, Ordering::SeqCst);
    prepared_revocation.run(&store, &e).await.unwrap();
    assert_eq!(
        store.authenticate(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::GrantRevoked)
    );
    store.close().await.unwrap();
    let store = reopen(&f, &e).await;
    assert_eq!(
        store.authenticate(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::GrantRevoked)
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn stock_and_outbox_factories_never_adopt_or_reset_one_another() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    assert!(matches!(
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await,
        Err(WorkspaceError::WriterActive)
    ));
    store.close().await.unwrap();
    let database = f.path.join("stock_execution_v1.sqlite");
    let before = std::fs::read(&database).unwrap();
    assert!(
        NativeWorkspace::open_existing(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .is_err()
    );
    assert!(
        NativeWorkspace::initialize_local_outbox(&f.path, f.config(), f.crypto(), f.clock.clone())
            .await
            .is_err()
    );
    assert!(
        NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), f.clock.clone())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&database).unwrap(), before);
    let g = Fixture::new();
    let (outbox, input, _) = g.create().await;
    outbox.close().await.unwrap();
    let database = g.path.join("execution_v1.sqlite");
    let before = std::fs::read(&database).unwrap();
    assert!(
        NativeWorkspace::open_seller_stock(&g.path, input, g.crypto(), g.clock.clone())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&database).unwrap(), before);
}
#[tokio::test]
async fn protected_key_and_every_selection_field_refuse_without_writes() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    store.close().await.unwrap();
    let database = f.path.join("stock_execution_v1.sqlite");
    let before = std::fs::read(&database).unwrap();
    assert!(
        NativeWorkspace::open_seller_stock(
            &f.path,
            e.input().clone(),
            f.crypto_with([0x99; 32]),
            f.clock.clone()
        )
        .await
        .is_err()
    );
    for field in 0..9 {
        assert!(
            NativeWorkspace::open_seller_stock(
                &f.path,
                altered(e.input(), field),
                f.crypto(),
                f.clock.clone()
            )
            .await
            .is_err()
        );
    }
    assert_eq!(std::fs::read(&database).unwrap(), before);
}
#[tokio::test]
async fn stock_catalog_and_protected_record_corruption_are_never_repaired() {
    for sql in [
        "PRAGMA user_version=99",
        "CREATE TABLE unexpected(value INTEGER)",
        "CREATE TRIGGER foreign_trigger BEFORE INSERT ON stock_grants BEGIN SELECT RAISE(ABORT,'fault'); END",
        "UPDATE stock_authority SET protected=zeroblob(64)",
        "UPDATE stock_grants SET protected=zeroblob(64)",
        "DELETE FROM stock_grants",
    ] {
        let f = Fixture::new();
        let e = enrollment(&f);
        let store = create(&f, &e).await;
        store.close().await.unwrap();
        let database = f.path.join("stock_execution_v1.sqlite");
        let mut connection = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(&database),
        )
        .await
        .unwrap();
        sqlx::raw_sql(sql).execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
        let before = std::fs::read(&database).unwrap();
        assert!(
            NativeWorkspace::open_seller_stock(
                &f.path,
                e.input().clone(),
                f.crypto(),
                f.clock.clone()
            )
            .await
            .is_err(),
            "{sql}"
        );
        assert_eq!(std::fs::read(&database).unwrap(), before);
    }
}
#[tokio::test]
async fn cancelled_stock_revocation_drains_then_remains_revoked() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let store = NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock.clone())
        .await
        .unwrap();
    let prepared_revocation = PreparedRevocation::new(&store, &e).await;
    clock.arm();
    let mut revoked = Box::pin(prepared_revocation.run(&store, &e));
    assert!(futures::poll!(revoked.as_mut()).is_pending());
    clock.entered();
    drop(revoked);
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
    let store = reopen(&f, &e).await;
    assert_eq!(
        store.authenticate(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::GrantRevoked)
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn actual_commit_error_fences_the_owner_until_original_state_reopens() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    let prepared_revocation = PreparedRevocation::new(&store, &e).await;
    let database = f.path.join("stock_execution_v1.sqlite");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&database),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE deferred_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_commit AFTER UPDATE ON stock_grants BEGIN INSERT INTO deferred_fault VALUES(2); END").execute(&mut connection).await.unwrap();
    assert_eq!(
        prepared_revocation.run(&store, &e).await,
        Err(WorkspaceError::CommitUnknown)
    );
    assert_eq!(
        store.authenticate(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::RecoveryRequired)
    );
    assert_eq!(
        store.stock_authority(e.input().clone(), e.proof()).await,
        Err(WorkspaceError::RecoveryRequired)
    );
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fail_commit; DROP TABLE deferred_fault")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let store = reopen(&f, &e).await;
    let receipt = store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(receipt.transaction_id(), e.transaction_id());
    assert_eq!(receipt.revocation_generation().get(), 1);
    store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    store.close().await.unwrap();
}
struct CreationClock {
    calls: AtomicU64,
    paused: PausedClock,
}
impl WorkspaceClock for CreationClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            self.paused.arm();
            self.paused.now()
        } else {
            Ok(UnixSeconds::new(100).unwrap())
        }
    }
}
#[tokio::test]
async fn lost_creation_observer_recovers_original_prepared_identity() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let clock = Arc::new(CreationClock {
        calls: AtomicU64::new(0),
        paused: PausedClock {
            state: std::sync::Mutex::new((false, false, false)),
            changed: std::sync::Condvar::new(),
        },
    });
    let mut creation = Box::pin(NativeWorkspace::initialize_seller_stock(
        &f.path,
        &e,
        f.crypto(),
        clock.clone(),
    ));
    assert!(futures::poll!(creation.as_mut()).is_pending());
    clock.paused.entered();
    drop(creation);
    assert!(matches!(
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await,
        Err(WorkspaceError::WriterActive)
    ));
    clock.paused.release();
    // The first observer owns no handle after cancellation. The worker still
    // commits and explicitly closes its connection before releasing authority.
    // Observe the actual persistent writer-ownership transition. Attempt counts
    // cannot establish completion on a slower scheduler or emulated target.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.path.join(".stock_execution_v1.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    fs2::FileExt::unlock(&lock).unwrap();
    drop(lock);
    let store = reopen(&f, &e).await;
    let receipt = store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(receipt.transaction_id(), e.transaction_id());
    assert_eq!(receipt.input(), e.input());
    store.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_journal_is_not_a_stock_transaction_path() {
    use radroots_storage::command::{OperationError, OperationJournal};
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    let binding = store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    let proof = e
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap();
    assert!(matches!(
        store
            .operation(
                binding,
                radroots_domain::OperationId::from_bytes([1; 32]),
                None,
                proof
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
}

#[tokio::test]
async fn changed_protected_grant_after_decryption_is_refused_before_binding() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let store = NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock.clone())
        .await
        .unwrap();
    clock.arm();
    let mut auth = Box::pin(store.authenticate(e.input().clone(), e.proof()));
    assert!(futures::poll!(auth.as_mut()).is_pending());
    clock.entered();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.path.join("stock_execution_v1.sqlite")),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE stock_grants SET protected=zeroblob(64)")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    clock.release();
    assert_eq!(auth.await, Err(WorkspaceError::RecoveryRequired));
    store.close().await.unwrap();
}
#[tokio::test]
async fn statement_failure_rolls_back_both_revocation_records() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    let prepared_revocation = PreparedRevocation::new(&store, &e).await;
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.path.join("stock_execution_v1.sqlite")),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fail_revocation BEFORE UPDATE ON stock_grants BEGIN SELECT RAISE(ABORT,'fault'); END").execute(&mut connection).await.unwrap();
    assert_eq!(
        prepared_revocation.run(&store, &e).await,
        Err(WorkspaceError::StorageFailure)
    );
    let receipt = store
        .stock_authority(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(receipt.revocation_generation().get(), 1);
    sqlx::raw_sql("DROP TRIGGER fail_revocation")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    store.close().await.unwrap();
    let store = reopen(&f, &e).await;
    store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    store.close().await.unwrap();
}

struct ExpiringCreationClock(AtomicU64);
impl WorkspaceClock for ExpiringCreationClock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        Ok(
            UnixSeconds::new(if self.0.fetch_add(1, Ordering::SeqCst) < 2 {
                100
            } else {
                200
            })
            .unwrap(),
        )
    }
}
#[tokio::test]
async fn expiry_is_rechecked_inside_actual_creation_transaction() {
    let f = Fixture::new();
    let e = enrollment(&f);
    assert!(matches!(
        NativeWorkspace::initialize_seller_stock(
            &f.path,
            &e,
            f.crypto(),
            Arc::new(ExpiringCreationClock(AtomicU64::new(0)))
        )
        .await,
        Err(WorkspaceError::GrantExpired)
    ));
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(f.path.join("stock_execution_v1.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_schema")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert_eq!(count, 0);
    connection.close().await.unwrap();
    assert!(
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn prepared_enrollment_cannot_create_a_second_authority_namespace() {
    let f = Fixture::new();
    let e = enrollment(&f);
    let store = create(&f, &e).await;
    let other = f.path.parent().unwrap().join("second-authority");
    assert!(matches!(
        NativeWorkspace::initialize_seller_stock(&other, &e, f.crypto(), f.clock.clone()).await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert!(!other.exists());
    store.close().await.unwrap();
    assert!(matches!(
        NativeWorkspace::initialize_seller_stock(&other, &e, f.crypto(), f.clock.clone()).await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    let store = reopen(&f, &e).await;
    assert_eq!(
        store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap()
            .transaction_id(),
        e.transaction_id()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn changed_catalog_after_protected_reads_is_refused_before_authenticated_completion() {
    for revocation in [false, true] {
        let f = Fixture::new();
        let e = enrollment(&f);
        let clock = Arc::new(PausedClock {
            state: std::sync::Mutex::new((false, false, false)),
            changed: std::sync::Condvar::new(),
        });
        let store =
            NativeWorkspace::initialize_seller_stock(&f.path, &e, f.crypto(), clock.clone())
                .await
                .unwrap();
        let prepared_revocation = PreparedRevocation::new(&store, &e).await;
        clock.arm();
        let mut operation: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), WorkspaceError>> + '_>,
        > = if revocation {
            Box::pin(prepared_revocation.run(&store, &e))
        } else {
            Box::pin(async {
                store
                    .authenticate(e.input().clone(), e.proof())
                    .await
                    .map(|_| ())
            })
        };
        assert!(futures::poll!(operation.as_mut()).is_pending());
        clock.entered();
        let mut connection = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(f.path.join("stock_execution_v1.sqlite")),
        )
        .await
        .unwrap();
        sqlx::query("PRAGMA user_version=2")
            .execute(&mut connection)
            .await
            .unwrap();
        clock.release();
        assert_eq!(operation.await, Err(WorkspaceError::RecoveryRequired));
        store.close().await.unwrap();
        sqlx::query("PRAGMA user_version=15")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        let reopened = reopen(&f, &e).await;
        assert_eq!(
            reopened
                .stock_authority(e.input().clone(), e.proof())
                .await
                .unwrap()
                .revocation_generation()
                .get(),
            1
        );
        reopened.close().await.unwrap();
    }
}
