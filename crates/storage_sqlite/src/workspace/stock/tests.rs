use super::*;
use crate::workspace::{
    fs::Kind,
    tests::{CryptoFixture, participant},
};
use radroots_domain::{CommandContextData, OperationId, PoolId, UnixSeconds};
use radroots_storage::{
    command::OperationError,
    inventory::{InventoryCheckpointRef, InventoryRequest, InventoryStore},
};
struct Clock;
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        Ok(UnixSeconds::new(100).unwrap())
    }
}

#[tokio::test]
async fn catalog4_grant_set_reopens_with_original_namespace_after_one_grant_change() {
    let fixture = CryptoFixture::new();
    let path = fixture.root().join("grant-reopen");
    let enrollment = StockEnrollment::prepare(
        SellerStockConfig::new(
            participant(),
            participant(),
            "owner",
            UnixSeconds::new(200).unwrap(),
        )
        .unwrap(),
        &Clock,
    )
    .unwrap();
    let store = NativeWorkspace::initialize_seller_stock(
        &path,
        &enrollment,
        fixture.crypto(),
        Arc::new(Clock),
    )
    .await
    .unwrap();
    let binding = store
        .authenticate(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap();
    let minimum = store
        .stock_authority(enrollment.input().clone(), enrollment.proof())
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([7; 32]),
        binding.actor(),
        enrollment.input().workspace_id(),
        enrollment.input().authority(),
        enrollment.input().route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let wire = serde_json::json!({"type":"stock.create_pool","context":context,"input":{"id":PoolId::from_bytes([8;32]),"label":"reopen","unit":"each","tick_scale":"0","initial_quantity":"1"}});
    let copy = enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap();
    let result = store
        .execute_inventory(
            InventoryRequest::from_protected_wire(
                binding.clone(),
                minimum,
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap(),
            copy,
        )
        .await
        .unwrap();
    let minimum = match result {
        radroots_storage::inventory::InventoryOutcome::Committed(v) => v.checkpoint(),
        _ => panic!("creation must confirm"),
    };
    let grant = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(enrollment.input()),
        "device".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    assert!(matches!(
        store
            .enroll_stock_grant(binding, minimum, enrollment.proof(), &grant)
            .await
            .unwrap(),
        StockGrantOutcome::Committed(_)
    ));
    store.close().await.unwrap();
    std::fs::copy(
        path.join("stock_execution_v1.sqlite"),
        fixture.root().join("synthetic-before-revocation.sqlite"),
    )
    .unwrap();
    let namespace = Namespace::acquire_kind(&path, false, Kind::SellerStock).unwrap();
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(namespace.database_path())
            .read_only(true),
    )
    .await
    .unwrap();
    assert_eq!(schema::admit(&mut connection).await, Ok(4));
    assert_eq!(schema::integrity(&mut connection).await, Ok(()));
    let root: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_authority")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    let clear = fixture
        .crypto()
        .open_stock_root(enrollment.input(), 4, &root)
        .await
        .unwrap();
    let authority = clear
        .expose_secret(|bytes| Authority::decode(bytes, 4))
        .unwrap();
    assert_eq!(authority.input, *enrollment.input());
    assert_eq!(
        grants::check_state(&mut connection, authority.authorization.unwrap()).await,
        Ok(())
    );
    assert!(
        read(&mut connection, enrollment.input(), &fixture.crypto())
            .await
            .is_ok()
    );
    connection.close().await.unwrap();
    drop(namespace);
    let reopened = NativeWorkspace::open_seller_stock(
        &path,
        enrollment.input().clone(),
        fixture.crypto(),
        Arc::new(Clock),
    )
    .await
    .unwrap();
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn root4_context_binds_the_complete_namespace_independently_of_caller_grants() {
    use radroots_storage::workspace::seller_stock::SellerAuthoritySelection;
    use serde_json::json;
    let fixture = CryptoFixture::new();
    let (owner, _, _) = SellerStockConfig::new(
        participant(),
        participant(),
        "root-context",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(100).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[7; 32]).unwrap(),
    )
    .unwrap();
    let namespace = SellerAuthoritySelection::from_attachment(&owner);
    let caller = namespace.attachment(GrantId::from_bytes([9; 32])).unwrap();
    let crypto = fixture.crypto();
    let clear = b"synthetic context payload";
    let current = crypto.seal_stock_root(&owner, 4, clear).await.unwrap();
    crypto
        .open_stock_root(&caller, 4, &current)
        .await
        .unwrap()
        .expose_secret(|bytes| assert_eq!(bytes, clear));
    let retained = crypto.seal_stock_root(&owner, 3, clear).await.unwrap();
    assert!(crypto.open_stock_root(&caller, 3, &retained).await.is_err());
    assert!(crypto.open_stock_root(&owner, 3, &current).await.is_err());
    let value = serde_json::to_value(namespace).unwrap();
    let other_key = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    for (path, bad) in [
        ("/workspace_id", json!("0b".repeat(32))),
        ("/authority/executor_id", json!("0b".repeat(32))),
        ("/authority/generation", json!("2")),
        ("/participant", json!(other_key)),
        ("/endpoint", json!("local://another-namespace")),
        ("/endpoint_identity", json!(other_key)),
        ("/checkpoint/digest", json!("0b".repeat(32))),
        ("/route_digest", json!("0b".repeat(32))),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        let selected: SellerAuthoritySelection = serde_json::from_value(changed).unwrap();
        assert!(
            crypto
                .open_stock_root(
                    &selected.attachment(GrantId::from_bytes([9; 32])).unwrap(),
                    4,
                    &current
                )
                .await
                .is_err(),
            "{path}"
        );
    }
}

#[tokio::test]
async fn retained_inventory_v2_keeps_original_records_and_refuses_new_correction_without_upgrade() {
    retained_inventory_catalog(2).await;
}
#[tokio::test]
async fn retained_inventory_v3_preserves_correction_and_original_records_without_upgrade() {
    retained_inventory_catalog(3).await;
}
async fn retained_inventory_catalog(catalog: i64) {
    use radroots_storage::inventory::{InventoryCommandKind, InventoryOutcome};
    let fixture = CryptoFixture::new();
    let path = fixture.root().join("retained-v2");
    let (input, proof, record) = SellerStockConfig::new(
        participant(),
        participant(),
        "retained-stock-owner",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(100).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[0xA3; 32]).unwrap(),
    )
    .unwrap();
    // Literal retained catalog/root shapes from published creation/correction source; not predecessor-binary evidence.
    let clear=zeroize::Zeroizing::new(serde_json::to_vec(&serde_json::json!({"version":catalog,"input":input,"transaction":TransactionId::from_bytes([9;32]),"revocation":"1","inventory":{"revision":"0","pools":"0","movements":"0","head":null}})).unwrap());
    let crypto = fixture.crypto();
    let root = crypto
        .seal_stock_root(&input, catalog, &clear)
        .await
        .unwrap();
    let grant = crypto
        .seal_stock(&input, false, &record.encode().unwrap())
        .await
        .unwrap();
    let namespace = Namespace::acquire_kind(&path, true, Kind::SellerStock).unwrap();
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(namespace.database_path())
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
    )
    .await
    .unwrap();
    let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
    for (_, _, _, sql) in schema::objects(catalog).unwrap() {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO stock_authority(singleton,protected) VALUES(1,?)")
        .bind(root)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
        .bind(input.grant_id().as_bytes().as_slice())
        .bind(grant)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(if catalog == 2 {
        "PRAGMA user_version=2"
    } else {
        "PRAGMA user_version=3"
    })
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
    drop(namespace);
    let store =
        NativeWorkspace::open_seller_stock(&path, input.clone(), fixture.crypto(), Arc::new(Clock))
            .await
            .unwrap();
    let binding = store.authenticate(input.clone(), &proof).await.unwrap();
    let minimum = store
        .stock_authority(input.clone(), &proof)
        .await
        .unwrap()
        .inventory_checkpoint()
        .unwrap();
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
    let wire = serde_json::json!({"type":"stock.create_pool","context":context,"input":{"id":PoolId::from_bytes([8;32]),"label":"retained","unit":"each","tick_scale":"0","initial_quantity":"1"}});
    let copy = || {
        proof
            .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
            .unwrap()
    };
    let created = match store
        .execute_inventory(
            InventoryRequest::from_protected_wire(
                binding.clone(),
                minimum,
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap(),
            copy(),
        )
        .await
        .unwrap()
    {
        InventoryOutcome::Committed(receipt) => *receipt,
        InventoryOutcome::Unknown(_) => panic!("retained creation must confirm"),
    };
    assert_eq!(created.kind(), InventoryCommandKind::CreatePool);
    assert_eq!(created.pool().balances().physical().available.get(), 1);
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&path, input.clone(), fixture.crypto(), Arc::new(Clock))
            .await
            .unwrap();
    assert_eq!(
        store
            .inventory_operation(
                binding.clone(),
                created.checkpoint(),
                OperationId::from_bytes([7; 32]),
                Some(created.attempt().command_digest()),
                copy()
            )
            .await
            .unwrap(),
        Some(created.clone())
    );
    let mut corrected = serde_json::json!({"type":"stock.adjust","context":context,"input":{"pool_id":PoolId::from_bytes([8;32]),"expected_version":"1","direction":"add","amount":"1","reason":"new correction profile"}});
    corrected["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([10; 32]));
    let correction = store
        .execute_inventory(
            InventoryRequest::from_protected_wire(
                binding,
                created.checkpoint(),
                &serde_json::to_vec(&corrected).unwrap(),
            )
            .unwrap(),
            copy(),
        )
        .await;
    if catalog == 2 {
        assert!(matches!(
            correction,
            Err(OperationError::UnsupportedOperation)
        ));
    } else {
        match correction.unwrap() {
            InventoryOutcome::Committed(value) => {
                assert_eq!(value.pool().version().get(), 2);
                assert_eq!(value.pool().balances().physical().available.get(), 2);
            }
            InventoryOutcome::Unknown(_) => panic!("retained correction must confirm"),
        }
    }
    store.close().await.unwrap();
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(path.join("stock_execution_v1.sqlite")),
    )
    .await
    .unwrap();
    assert_eq!(schema::admit(&mut connection).await.unwrap(), catalog);
    let protected: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM stock_operations WHERE operation_id=?")
            .bind(OperationId::from_bytes([7; 32]).as_bytes().as_slice())
            .fetch_one(&mut connection)
            .await
            .unwrap();
    let old = fixture
        .crypto()
        .open_stock_record_for_catalog(
            &input,
            "operation",
            OperationId::from_bytes([7; 32]).as_bytes(),
            catalog,
            &protected,
        )
        .await
        .unwrap();
    old.expose_secret(|bytes| {
        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(value["version"], if catalog == 2 { 1 } else { 2 });
        if catalog == 2 {
            assert!(value.get("before").is_none());
        } else {
            assert_eq!(value["before"], serde_json::Value::Null);
        }
    });
    sqlx::query(if catalog == 2 {
        "PRAGMA user_version=3"
    } else {
        "PRAGMA user_version=4"
    })
    .execute(&mut connection)
    .await
    .unwrap();
    connection.close().await.unwrap();
    assert!(
        NativeWorkspace::open_seller_stock(&path, input, fixture.crypto(), Arc::new(Clock))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn retained_enrollment_v1_authenticates_but_never_auto_creates_inventory() {
    let fixture = CryptoFixture::new();
    let path = fixture.root().join("retained-v1");
    let (input, proof, record) = SellerStockConfig::new(
        participant(),
        participant(),
        "retained-stock-owner",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(100).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[0xA3; 32]).unwrap(),
    )
    .unwrap();
    // Exact version1 shape introduced by a4cc1a1c; this is a literal retained
    // codec/catalog fixture, not a claim that a predecessor binary ran here.
    let clear=zeroize::Zeroizing::new(serde_json::to_vec(&serde_json::json!({"version":1,"input":input,"transaction":TransactionId::from_bytes([9;32]),"revocation":"1"})).unwrap());
    let crypto = fixture.crypto();
    let root = crypto.seal_stock(&input, true, &clear).await.unwrap();
    let grant = crypto
        .seal_stock(&input, false, &record.encode().unwrap())
        .await
        .unwrap();
    let namespace = Namespace::acquire_kind(&path, true, Kind::SellerStock).unwrap();
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(namespace.database_path())
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
    )
    .await
    .unwrap();
    let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
    for (_, _, _, sql) in schema::objects(1).unwrap() {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO stock_authority(singleton,protected) VALUES(1,?)")
        .bind(root)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
        .bind(input.grant_id().as_bytes().as_slice())
        .bind(grant)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=1")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
    drop(namespace);
    let store =
        NativeWorkspace::open_seller_stock(&path, input.clone(), fixture.crypto(), Arc::new(Clock))
            .await
            .unwrap();
    let binding = store.authenticate(input.clone(), &proof).await.unwrap();
    assert!(
        store
            .stock_authority(input.clone(), &proof)
            .await
            .unwrap()
            .inventory_checkpoint()
            .is_none()
    );
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
    let wire = serde_json::json!({"type":"stock.create_pool","context":context,"input":{"id":PoolId::from_bytes([8;32]),"label":"retained","unit":"each","tick_scale":"0","initial_quantity":"1"}});
    let request = InventoryRequest::from_protected_wire(
        binding,
        InventoryCheckpointRef::from_host_metadata(
            input.authority(),
            Sequence::new(0).unwrap(),
            None,
        )
        .unwrap(),
        &serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    let copy = proof
        .with_secret(|b| AttachmentProof::from_protected_input(b))
        .unwrap();
    assert!(matches!(
        store.execute_inventory(request, copy).await,
        Err(OperationError::UnsupportedOperation)
    ));
    store.close().await.unwrap();
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path.join("stock_execution_v1.sqlite"))
            .read_only(true),
    )
    .await
    .unwrap();
    assert_eq!(schema::admit(&mut connection).await.unwrap(), 1);
    connection.close().await.unwrap();
}
