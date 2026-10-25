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
async fn current_grant_set_reopens_with_original_namespace_after_one_grant_change() {
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
    assert_eq!(schema::admit(&mut connection).await, Ok(8));
    assert_eq!(schema::integrity(&mut connection).await, Ok(()));
    let root: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_authority")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    let clear = fixture
        .crypto()
        .open_stock_root(enrollment.input(), 8, &root)
        .await
        .unwrap();
    let authority = clear
        .expose_secret(|bytes| Authority::decode(bytes, 8))
        .unwrap();
    clear.expose_secret(|bytes| {
        let original:serde_json::Value=serde_json::from_slice(bytes).unwrap();
        assert_eq!(original["inventory"]["mappings"],"0");
        for bad in [serde_json::json!([]),serde_json::json!({"revision":"1","pools":"1","movements":"1","head":original["inventory"]["head"]})] {
            let mut value=original.clone();value["inventory"]=bad;
            assert!(Authority::decode(&serde_json::to_vec(&value).unwrap(),8).is_err());
        }
        for bad in [
            serde_json::json!([]),
            serde_json::json!({"revision":"0","records":"0","parts":"0"}),
            serde_json::json!({"revision":"0","records":"0","parts":"0","head":null,"extra":true}),
            serde_json::json!({"revision":"1","records":"0","parts":"2","head":"01".repeat(32)}),
            serde_json::json!({"revision":"1","records":"0","parts":"13","head":"01".repeat(32)}),
            serde_json::json!({"revision":"1","records":"2","parts":"3","head":"01".repeat(32)}),
        ] {
            let mut value=original.clone();value["sources"]=bad;
            assert!(Authority::decode(&serde_json::to_vec(&value).unwrap(),8).is_err());
        }
    });
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
async fn current_and_retained_roots_bind_the_complete_namespace_with_distinct_catalog_contexts() {
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
    for version in [4, 5, 6, 7, 8] {
        let current = crypto
            .seal_stock_root(&owner, version, clear)
            .await
            .unwrap();
        crypto
            .open_stock_root(&caller, version, &current)
            .await
            .unwrap()
            .expose_secret(|bytes| assert_eq!(bytes, clear));
        let retained = crypto.seal_stock_root(&owner, 3, clear).await.unwrap();
        assert!(crypto.open_stock_root(&caller, 3, &retained).await.is_err());
        assert!(crypto.open_stock_root(&owner, 3, &current).await.is_err());
        assert!(
            crypto
                .open_stock_root(&owner, if version == 4 { 5 } else { 4 }, &current)
                .await
                .is_err()
        );
        let value = serde_json::to_value(&namespace).unwrap();
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
                        version,
                        &current
                    )
                    .await
                    .is_err(),
                "{path}"
            );
        }
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
#[tokio::test]
async fn retained_inventory_v4_preserves_scoped_grants_and_original_effects_without_adding_mappings()
 {
    retained_inventory_catalog(4).await;
}
#[tokio::test]
async fn retained_inventory_v5_preserves_mappings_and_never_adds_source_tables() {
    retained_inventory_catalog(5).await;
}
#[tokio::test]
async fn retained_inventory_v6_preserves_original_physical_and_source_capabilities() {
    retained_inventory_catalog(6).await;
}
#[tokio::test]
async fn retained_inventory_v7_keeps_consent_ledger_without_public_source_tables() {
    retained_inventory_catalog(7).await;
}
#[tokio::test]
async fn current_v8_preserves_physical_and_private_source_progress_with_public_sources() {
    assert_eq!(schema::CURRENT, 8);
    retained_inventory_catalog(8).await;
}
// Exact catalog fixtures exercise retained/successor codecs; they are not
// evidence from a predecessor executable or a factory migration.
pub(super) async fn literal_catalog(
    fixture: &CryptoFixture,
    path: &std::path::Path,
    catalog: i64,
) -> (AttachmentInput, AttachmentProof) {
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
    let crypto = fixture.crypto();
    let grant_clear = if catalog >= 4 {
        StockGrantRecord::from_enrollment(record)
            .unwrap()
            .encode()
            .unwrap()
    } else {
        record.encode().unwrap()
    };
    let grant = if catalog >= 4 {
        crypto
            .seal_stock_record(&input, "grant", input.grant_id().as_bytes(), &grant_clear)
            .await
            .unwrap()
    } else {
        crypto
            .seal_stock(&input, false, &grant_clear)
            .await
            .unwrap()
    };
    // The v4 namespace, grant2 and operation3 envelopes remain frozen literals.
    let mut value = if catalog >= 4 {
        serde_json::json!({"version":4,"namespace":SellerAuthoritySelection::from_attachment(&input),"administrator":input.grant_id(),"transaction":TransactionId::from_bytes([9;32]),"revocation":"1","inventory":{"revision":"0","pools":"0","movements":"0","head":null},"authorization":grants::single_state(input.grant_id(),&grant).unwrap()})
    } else {
        serde_json::json!({"version":catalog,"input":input,"transaction":TransactionId::from_bytes([9;32]),"revocation":"1","inventory":{"revision":"0","pools":"0","movements":"0","head":null}})
    };
    if catalog >= 5 {
        value["version"] = serde_json::json!(catalog);
        value["inventory"]["mappings"] = serde_json::json!("0");
    }
    if catalog >= 6 {
        value["sources"] =
            serde_json::json!({"revision":"0","records":"0","parts":"0","head":null});
    }
    if catalog >= 7 {
        value["inventory"]["orders"] = serde_json::json!("0");
    }
    if catalog == 8 {
        value["public_sources"] =
            serde_json::json!({"revision":"0","events":"0","heads":"0","parts":"0","head":null});
    }
    let clear = zeroize::Zeroizing::new(serde_json::to_vec(&value).unwrap());
    let root = crypto
        .seal_stock_root(&input, catalog, &clear)
        .await
        .unwrap();
    let namespace = Namespace::acquire_kind(path, true, Kind::SellerStock).unwrap();
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
    sqlx::query(match catalog {
        2 => "PRAGMA user_version=2",
        3 => "PRAGMA user_version=3",
        4 => "PRAGMA user_version=4",
        5 => "PRAGMA user_version=5",
        6 => "PRAGMA user_version=6",
        7 => "PRAGMA user_version=7",
        8 => "PRAGMA user_version=8",
        _ => unreachable!(),
    })
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
    drop(namespace);
    (input, proof)
}

async fn retained_inventory_catalog(catalog: i64) {
    use radroots_storage::inventory::{InventoryCommandKind, InventoryOutcome};
    let fixture = CryptoFixture::new();
    let path = fixture.root().join("retained-v2");
    let (input, proof) = literal_catalog(&fixture, &path, catalog).await;
    let store =
        NativeWorkspace::open_seller_stock(&path, input.clone(), fixture.crypto(), Arc::new(Clock))
            .await
            .unwrap();
    let binding = store.authenticate(input.clone(), &proof).await.unwrap();
    let authority = store.stock_authority(input.clone(), &proof).await.unwrap();
    assert_eq!(authority.source_checkpoint().is_some(), catalog >= 6);
    assert_eq!(authority.listing_checkpoint().is_some(), catalog == 8);
    let minimum = authority.inventory_checkpoint().unwrap();
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
    let mut mapping = serde_json::json!({"type":"stock.map_option","context":context,"input":{"listing":{"kind":30402,"author":participant(),"d":"0a".repeat(16)},"option_id":"0b".repeat(32),"pool_id":PoolId::from_bytes([8;32]),"expected_mapping_version":"0","per_sale_unit":{"amount":"1","unit":"each"}}});
    mapping["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([11; 32]));
    let mapped = store
        .execute_inventory(
            InventoryRequest::from_protected_wire(
                binding.clone(),
                created.checkpoint(),
                &serde_json::to_vec(&mapping).unwrap(),
            )
            .unwrap(),
            copy(),
        )
        .await;
    if catalog < 5 {
        assert_eq!(mapped, Err(OperationError::UnsupportedOperation));
    } else {
        let InventoryOutcome::Committed(mapped) = mapped.unwrap() else {
            panic!("retained mapping must confirm")
        };
        assert_eq!(mapped.mapping().unwrap().version().get(), 1);
        assert_eq!(mapped.pool(), created.pool());
    }
    let mut corrected = serde_json::json!({"type":"stock.adjust","context":context,"input":{"pool_id":PoolId::from_bytes([8;32]),"expected_version":"1","direction":"add","amount":"1","reason":"new correction profile"}});
    corrected["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([10; 32]));
    let correction = store
        .execute_inventory(
            InventoryRequest::from_protected_wire(
                binding.clone(),
                created.checkpoint(),
                &serde_json::to_vec(&corrected).unwrap(),
            )
            .unwrap(),
            copy(),
        )
        .await;
    let mut latest = created.checkpoint();
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
                latest = value.checkpoint();
            }
            InventoryOutcome::Unknown(_) => panic!("retained correction must confirm"),
        }
    }
    if catalog >= 6 {
        // The caller's minimum is newer than this original operation. A
        // hidden SQL identity must still fail protected absence verification.
        let mut raw = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path.join("stock_execution_v1.sqlite"))
                .foreign_keys(false),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE stock_operations SET operation_id=? WHERE operation_id=?")
            .bind([0x78_u8; 32].as_slice())
            .bind([7_u8; 32].as_slice())
            .execute(&mut raw)
            .await
            .unwrap();
        assert!(matches!(
            store
                .inventory_operation(
                    binding.clone(),
                    latest,
                    OperationId::from_bytes([7; 32]),
                    None,
                    copy()
                )
                .await,
            Err(OperationError::RecoveryRequired
                | OperationError::Workspace(WorkspaceError::RecoveryRequired))
        ));
        assert!(matches!(
            store
                .execute_inventory(
                    InventoryRequest::from_protected_wire(
                        binding.clone(),
                        latest,
                        &serde_json::to_vec(&wire).unwrap()
                    )
                    .unwrap(),
                    copy()
                )
                .await,
            Err(OperationError::RecoveryRequired
                | OperationError::Workspace(WorkspaceError::RecoveryRequired))
        ));
        sqlx::query("UPDATE stock_operations SET operation_id=? WHERE operation_id=?")
            .bind([7_u8; 32].as_slice())
            .bind([0x78_u8; 32].as_slice())
            .execute(&mut raw)
            .await
            .unwrap();
        raw.close().await.unwrap();
        assert_eq!(
            store
                .inventory_operation(
                    binding.clone(),
                    latest,
                    OperationId::from_bytes([7; 32]),
                    None,
                    copy()
                )
                .await
                .unwrap(),
            Some(created.clone())
        );
    }
    if catalog >= 4 {
        let child = StockGrantEnrollment::prepare(
            &SellerAuthoritySelection::from_attachment(&input),
            "retained-device".into(),
            UnixSeconds::new(200).unwrap(),
            StockGrantScope::full_inventory(),
        )
        .unwrap();
        let receipt = match store
            .enroll_stock_grant(binding.clone(), latest, &proof, &child)
            .await
            .unwrap()
        {
            StockGrantOutcome::Committed(value) => value,
            _ => panic!("retained grant must confirm"),
        };
        latest = latest.with_authorization(receipt.authorization());
        let delegated = store
            .authenticate(child.input().clone(), child.proof())
            .await
            .unwrap();
        let child_proof = child
            .proof()
            .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
            .unwrap();
        assert_eq!(
            store
                .pool(delegated, latest, PoolId::from_bytes([8; 32]), child_proof)
                .await
                .unwrap()
                .unwrap()
                .snapshot()
                .balances()
                .physical()
                .available
                .get(),
            2
        );
    }
    store.close().await.unwrap();
    let reopened =
        NativeWorkspace::open_seller_stock(&path, input.clone(), fixture.crypto(), Arc::new(Clock))
            .await
            .unwrap();
    assert_eq!(
        reopened
            .inventory_operation(
                binding.clone(),
                latest,
                OperationId::from_bytes([7; 32]),
                None,
                copy()
            )
            .await
            .unwrap(),
        Some(created)
    );
    reopened.close().await.unwrap();
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
    let old = if matches!(catalog, 7 | 8) {
        let retained = ledger::read(
            &mut connection,
            &input,
            &fixture.crypto(),
            OperationId::from_bytes([7; 32]),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(retained.header.version, 5);
        assert_eq!(retained.header.family.code(), 0);
        retained.body
    } else {
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
        old.expose_secret(|bytes| zeroize::Zeroizing::new(bytes.to_vec()))
    };
    {
        let bytes = old.as_slice();
        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let value = if catalog >= 4 {
            assert_eq!(value["version"], if catalog == 4 { 3 } else { 4 });
            if catalog == 4 {
                assert!(value.get("mapping").is_none());
            } else {
                assert_eq!(value["mapping"], serde_json::Value::Null);
            }
            value["operation"].clone()
        } else {
            value
        };
        assert_eq!(value["version"], if catalog == 2 { 1 } else { 2 });
        if catalog == 2 {
            assert!(value.get("before").is_none());
        } else {
            assert_eq!(value["before"], serde_json::Value::Null);
        }
    }
    if catalog >= 6 {
        // Exercise the actual minimum/projection witness after provider IO,
        // inside a real SQL transaction. Row counts and the protected header
        // stay unchanged while one referenced ciphertext changes.
        for part in [false, true] {
            if part && !matches!(catalog, 7 | 8) {
                continue;
            }
            let (authority, _, _, _) = read(&mut connection, &input, &fixture.crypto())
                .await
                .unwrap();
            let minimum = radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                input.authority(),
                Sequence::new(0).unwrap(),
                None,
            )
            .unwrap()
            .with_optional_authorization(authority.authorization);
            let witness = inventory::grant_minimum(
                &mut connection,
                &input,
                &fixture.crypto(),
                &binding,
                &authority,
                minimum,
            )
            .await
            .unwrap();
            let (select, mutate, restore) = if part {
                (
                    "SELECT protected FROM stock_operation_parts WHERE operation_id=? AND part=0",
                    "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE operation_id=? AND part=0",
                    "UPDATE stock_operation_parts SET protected=? WHERE operation_id=? AND part=0",
                )
            } else {
                (
                    "SELECT protected FROM stock_movements WHERE operation_id=?",
                    "UPDATE stock_movements SET protected=randomblob(length(protected)) WHERE operation_id=?",
                    "UPDATE stock_movements SET protected=? WHERE operation_id=?",
                )
            };
            let original: Vec<u8> = sqlx::query_scalar(select)
                .bind(OperationId::from_bytes([7; 32]).as_bytes().as_slice())
                .fetch_one(&mut connection)
                .await
                .unwrap();
            sqlx::query(mutate)
                .bind(OperationId::from_bytes([7; 32]).as_bytes().as_slice())
                .execute(&mut connection)
                .await
                .unwrap();
            let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
            assert_eq!(
                witness.confirm(&mut tx).await,
                Err(OperationError::RecoveryRequired)
            );
            tx.rollback().await.unwrap();
            sqlx::query(restore)
                .bind(original)
                .bind(OperationId::from_bytes([7; 32]).as_bytes().as_slice())
                .execute(&mut connection)
                .await
                .unwrap();
            let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
            assert_eq!(witness.confirm(&mut tx).await, Ok(()));
            tx.rollback().await.unwrap();
        }
    }
    sqlx::query(match catalog {
        2 => "PRAGMA user_version=3",
        3 => "PRAGMA user_version=4",
        4 => "PRAGMA user_version=5",
        5 => "PRAGMA user_version=6",
        6 => "PRAGMA user_version=7",
        7 => "PRAGMA user_version=6",
        8 => "PRAGMA user_version=7",
        _ => unreachable!(),
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
