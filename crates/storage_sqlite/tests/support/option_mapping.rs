use super::*;
use radroots_domain::{
    ListingOptionId, Sequence,
    listing::{ListingCoordinate, ListingId},
    stock::OptionMappingKey,
};
use radroots_protocol::command::{CapturedOptionMapping, MapOptionInput};

#[tokio::test]
async fn admitted_mapping_survives_observer_loss_and_close_drains_before_original_recovery() {
    let f = Fixture::new();
    let clock = Arc::new(PausedClock {
        state: std::sync::Mutex::new((false, false, false)),
        changed: std::sync::Condvar::new(),
    });
    let e = StockEnrollment::prepare(
        SellerStockConfig::new(key(), key(), "owner", UnixSeconds::new(200).unwrap()).unwrap(),
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
    let pool = committed(
        store
            .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let key = mapping_key(5, 6);
    let original = mapping_request(&binding, 7, key, 2, 0, grams("250"));
    let digest = original.command().digest();
    clock.arm();
    let mut future = store.execute_inventory(original, proof(e.proof()));
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
        .inventory_operation(
            binding.clone(),
            pool.checkpoint(),
            OperationId::from_bytes([7; 32]),
            Some(digest),
            proof(e.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.pool(), pool.pool());
    assert_eq!(receipt.mapping().unwrap().version().get(), 1);
    assert_eq!(
        store
            .option_mapping(binding.clone(), receipt.checkpoint(), key, proof(e.proof()))
            .await
            .unwrap(),
        receipt.mapping()
    );
    assert_eq!(
        committed(
            store
                .execute_inventory(
                    mapping_request(&binding, 7, key, 2, 0, grams("250")),
                    proof(e.proof())
                )
                .await
                .unwrap()
        ),
        receipt
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn mapping_rechecks_clock_deadline_and_credential_expiry_inside_its_actual_transaction() {
    for next in [0, 99, 150, 200] {
        let f = Fixture::new();
        let clock = Arc::new(AdmissionClock {
            armed: std::sync::atomic::AtomicBool::new(false),
            calls: AtomicU64::new(0),
            next,
        });
        let e = StockEnrollment::prepare(
            SellerStockConfig::new(key(), key(), "owner", UnixSeconds::new(200).unwrap()).unwrap(),
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
        let pool = committed(
            store
                .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
                .await
                .unwrap(),
        );
        clock.armed.store(true, Ordering::SeqCst);
        let result = store
            .execute_inventory(
                mapping_request(&binding, 7, mapping_key(5, 6), 2, 0, grams("250")),
                proof(e.proof()),
            )
            .await;
        match next {
            0 | 99 => assert_eq!(
                result,
                Err(OperationError::Workspace(WorkspaceError::Unavailable))
            ),
            150 => assert_eq!(result, Err(OperationError::DeadlineExceeded)),
            200 => assert_eq!(
                result,
                Err(OperationError::Workspace(WorkspaceError::GrantExpired))
            ),
            _ => unreachable!(),
        }
        clock.armed.store(false, Ordering::SeqCst);
        assert_eq!(
            store
                .option_mapping(
                    binding,
                    pool.checkpoint(),
                    mapping_key(5, 6),
                    proof(e.proof())
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .stock_authority(e.input().clone(), e.proof())
                .await
                .unwrap()
                .inventory_checkpoint(),
            Some(pool.checkpoint())
        );
        store.close().await.unwrap();
    }
}

fn mapping_key(listing: u8, option: u8) -> OptionMappingKey {
    OptionMappingKey::new(
        ListingCoordinate::new(key(), ListingId::from_bytes([listing; 16])),
        ListingOptionId::from_bytes([option; 32]),
    )
}
fn mapping_request(
    binding: &radroots_storage::workspace::WorkspaceBinding,
    operation: u8,
    key: OptionMappingKey,
    pool: u8,
    version: u64,
    quantity: Quantity,
) -> InventoryRequest {
    let selected = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([operation; 32]),
        binding.actor(),
        selected.workspace_id(),
        selected.authority(),
        selected.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    InventoryRequest::new(
        binding.clone(),
        minimum(binding),
        CapturedOptionMapping::new(
            context,
            MapOptionInput::new(
                key,
                PoolId::from_bytes([pool; 32]),
                quantity,
                Sequence::new(version).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn grams(amount: &str) -> Quantity {
    Quantity::new(amount.parse().unwrap(), MeasurementUnit::Gram).unwrap()
}
#[tokio::test]
async fn mapping_queries_and_original_replay_refuse_initial_and_confirmation_clock_regressions() {
    let f = Fixture::new();
    let clock = Arc::new(AdmissionClock {
        armed: std::sync::atomic::AtomicBool::new(false),
        calls: AtomicU64::new(0),
        next: 99,
    });
    let e = StockEnrollment::prepare(
        SellerStockConfig::new(key(), key(), "owner", UnixSeconds::new(200).unwrap()).unwrap(),
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
    store
        .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
        .await
        .unwrap();
    let key = mapping_key(5, 6);
    let receipt = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 2, 0, grams("250")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    for initial in [false, true] {
        for choice in 0..3 {
            clock.calls.store(u64::from(initial), Ordering::SeqCst);
            clock.armed.store(true, Ordering::SeqCst);
            let error = match choice {
                0 => store
                    .option_mapping(binding.clone(), receipt.checkpoint(), key, proof(e.proof()))
                    .await
                    .unwrap_err(),
                1 => store
                    .inventory_operation(
                        binding.clone(),
                        receipt.checkpoint(),
                        OperationId::from_bytes([7; 32]),
                        None,
                        proof(e.proof()),
                    )
                    .await
                    .unwrap_err(),
                _ => store
                    .execute_inventory(
                        mapping_request(&binding, 7, key, 2, 0, grams("250")),
                        proof(e.proof()),
                    )
                    .await
                    .unwrap_err(),
            };
            assert_eq!(
                error,
                OperationError::Workspace(WorkspaceError::Unavailable)
            );
            clock.armed.store(false, Ordering::SeqCst);
        }
    }
    assert_eq!(
        store
            .option_mapping(binding, receipt.checkpoint(), key, proof(e.proof()))
            .await
            .unwrap(),
        receipt.mapping()
    );
    store.close().await.unwrap();
}
async fn seeded(
    f: &Fixture,
) -> (
    NativeWorkspace,
    StockEnrollment,
    radroots_storage::workspace::WorkspaceBinding,
    InventoryReceipt,
) {
    let (store, e, binding) = create(f).await;
    let pool = committed(
        store
            .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    (store, e, binding, pool)
}
#[tokio::test]
async fn actual_mapping_create_update_remap_retains_original_effects_and_leaves_physical_stock_unchanged()
 {
    let f = Fixture::new();
    let (store, e, binding, pool) = seeded(&f).await;
    let other = committed(
        store
            .execute_inventory(request(&binding, 3, 4, "Beets"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let key = mapping_key(5, 6);
    let foreign = OptionMappingKey::new(
        ListingCoordinate::new(
            PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")
                .unwrap(),
            key.listing().d(),
        ),
        key.option_id(),
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 6, foreign, 2, 0, grams("1")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Unauthorized)
    );
    let original = mapping_request(&binding, 7, key, 2, 0, grams("250"));
    let wire = original.command().protected_wire().to_vec();
    let first = committed(
        store
            .execute_inventory(original, proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(first.pool(), pool.pool());
    assert_eq!(first.mapping().unwrap().per_sale_ticks().get(), 250);
    let second_key = mapping_key(5, 8);
    let second = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 9, second_key, 2, 0, grams("125")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    assert_eq!(second.pool(), pool.pool());
    let updated = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 10, key, 2, 1, grams("500")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    assert_eq!(updated.mapping().unwrap().version().get(), 2);
    let remapped = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 11, key, 4, 2, grams("1000")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    assert_eq!(remapped.pool(), other.pool());
    assert_eq!(remapped.mapping().unwrap().version().get(), 3);
    // The complete listing coordinate, not just the option ID, forms the key.
    let distinct = mapping_key(12, 6);
    store
        .execute_inventory(
            mapping_request(&binding, 13, distinct, 2, 0, grams("10")),
            proof(e.proof()),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .option_mapping(
                binding.clone(),
                remapped.checkpoint(),
                key,
                proof(e.proof())
            )
            .await
            .unwrap(),
        remapped.mapping()
    );
    assert_eq!(
        store
            .option_mapping(
                binding.clone(),
                remapped.checkpoint(),
                second_key,
                proof(e.proof())
            )
            .await
            .unwrap(),
        second.mapping()
    );
    assert_eq!(
        store
            .inventory_operation(
                binding.clone(),
                remapped.checkpoint(),
                OperationId::from_bytes([7; 32]),
                None,
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(first.clone())
    );
    assert_eq!(
        store
            .pool(
                binding.clone(),
                remapped.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        pool.pool()
    );
    assert_eq!(
        store
            .pool(
                binding.clone(),
                remapped.checkpoint(),
                PoolId::from_bytes([4; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        other.pool()
    );
    f.clock.0.store(175, Ordering::SeqCst);
    assert_eq!(
        committed(
            store
                .execute_inventory(
                    InventoryRequest::from_protected_wire(
                        binding.clone(),
                        remapped.checkpoint(),
                        &wire
                    )
                    .unwrap(),
                    proof(e.proof())
                )
                .await
                .unwrap()
        ),
        first
    );
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .option_mapping(
                binding.clone(),
                remapped.checkpoint(),
                key,
                proof(e.proof())
            )
            .await
            .unwrap(),
        remapped.mapping()
    );
    assert_eq!(
        store
            .inventory_operation(
                binding,
                remapped.checkpoint(),
                OperationId::from_bytes([7; 32]),
                None,
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(first)
    );
    store.close().await.unwrap();
    let mut db = database(&f).await;
    for (sql, expected) in [
        ("SELECT count(*) FROM stock_pools", 2),
        ("SELECT count(*) FROM stock_movements", 2),
        ("SELECT count(*) FROM stock_operations", 7),
        ("SELECT count(*) FROM stock_mappings", 3),
    ] {
        let count: i64 = sqlx::query_scalar(sql).fetch_one(&mut db).await.unwrap();
        assert_eq!(count, expected);
    }
    db.close().await.unwrap();
}

#[tokio::test]
async fn mapping_versions_precision_units_and_global_operation_identity_are_enforced() {
    let f = Fixture::new();
    let (store, e, binding, _) = seeded(&f).await;
    let key = mapping_key(5, 6);
    for quantity in [
        grams("0.1"),
        Quantity::new("1".parse().unwrap(), MeasurementUnit::Litre).unwrap(),
    ] {
        assert_eq!(
            store
                .execute_inventory(
                    mapping_request(&binding, 7, key, 2, 0, quantity),
                    proof(e.proof())
                )
                .await,
            Err(OperationError::InvalidCommand)
        );
    }
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 2, 1, grams("1")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::StaleState)
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 20, 0, grams("1")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::StaleState)
    );
    let first = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 2, 0, grams("250")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 8, key, 2, 0, grams("1")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::StaleState)
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 2, 0, grams("500")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    assert_eq!(
        store
            .execute_inventory(
                request(&binding, 7, 10, "duplicate physical identity"),
                proof(e.proof())
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 1, key, 2, 0, grams("1")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    assert_eq!(
        store
            .option_mapping(
                binding.clone(),
                first.checkpoint(),
                mapping_key(5, 9),
                proof(e.proof())
            )
            .await
            .unwrap(),
        None
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn delegated_full_inventory_does_not_confer_mapping_configuration_read_or_write() {
    use radroots_storage::workspace::seller_stock::{SellerAuthoritySelection, StockGrantScope};
    use radroots_storage_sqlite::workspace::{StockGrantEnrollment, StockGrantOutcome};
    let f = Fixture::new();
    let (store, e, owner, _) = seeded(&f).await;
    let key = mapping_key(5, 6);
    let original = committed(
        store
            .execute_inventory(
                mapping_request(&owner, 7, key, 2, 0, grams("250")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(owner.input()),
        "inventory-only".into(),
        UnixSeconds::new(200).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    let state = match store
        .enroll_stock_grant(owner.clone(), original.checkpoint(), e.proof(), &child)
        .await
        .unwrap()
    {
        StockGrantOutcome::Committed(v) => v.authorization(),
        _ => panic!("expected grant"),
    };
    let binding = store
        .authenticate(child.input().clone(), child.proof())
        .await
        .unwrap();
    let current = original.checkpoint().with_authorization(state);
    assert_eq!(
        store
            .option_mapping(binding.clone(), current, key, proof(child.proof()))
            .await,
        Err(OperationError::Unauthorized)
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 8, key, 2, 1, grams("500")),
                proof(child.proof())
            )
            .await,
        Err(OperationError::Unauthorized)
    );
    assert_eq!(
        store
            .inventory_operation(
                binding.clone(),
                current,
                OperationId::from_bytes([7; 32]),
                None,
                proof(child.proof())
            )
            .await,
        Err(OperationError::Unauthorized)
    );
    let replay = InventoryRequest::replay_from_protected_wire(
        binding,
        current,
        mapping_request(&owner, 7, key, 2, 0, grams("250"))
            .command()
            .protected_wire(),
    )
    .unwrap();
    assert_eq!(
        store.execute_inventory(replay, proof(child.proof())).await,
        Err(OperationError::Unauthorized)
    );
    let owner = store
        .authenticate(e.input().clone(), e.proof())
        .await
        .unwrap();
    assert_eq!(
        store
            .option_mapping(owner, current, key, proof(e.proof()))
            .await
            .unwrap(),
        original.mapping()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn every_mapping_operation_and_root_write_failure_rolls_back_the_whole_effect() {
    for updating in [false, true] {
        for (table, event) in [
            ("stock_mappings", if updating { "UPDATE" } else { "INSERT" }),
            ("stock_operations", "INSERT"),
            ("stock_authority", "UPDATE"),
        ] {
            let f = Fixture::new();
            let (store, e, binding, pool) = seeded(&f).await;
            let key = mapping_key(5, 6);
            let previous = if updating {
                Some(committed(
                    store
                        .execute_inventory(
                            mapping_request(&binding, 7, key, 2, 0, grams("250")),
                            proof(e.proof()),
                        )
                        .await
                        .unwrap(),
                ))
            } else {
                None
            };
            let minimum = previous.as_ref().unwrap_or(&pool).checkpoint();
            let mut db = database(&f).await;
            let trigger = format!(
                "CREATE TRIGGER mapping_fault BEFORE {event} ON {table} BEGIN SELECT RAISE(ABORT,'synthetic mapping failure'); END"
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
                .execute(&mut db)
                .await
                .unwrap();
            assert_eq!(
                store
                    .execute_inventory(
                        mapping_request(&binding, 8, key, 2, u64::from(updating), grams("500")),
                        proof(e.proof())
                    )
                    .await,
                Err(OperationError::StorageFailure),
                "{table}/{updating}"
            );
            sqlx::query("DROP TRIGGER mapping_fault")
                .execute(&mut db)
                .await
                .unwrap();
            assert_eq!(
                store
                    .stock_authority(e.input().clone(), e.proof())
                    .await
                    .unwrap()
                    .inventory_checkpoint(),
                Some(minimum)
            );
            assert_eq!(
                store
                    .option_mapping(binding.clone(), minimum, key, proof(e.proof()))
                    .await
                    .unwrap(),
                previous.as_ref().and_then(|value| value.mapping())
            );
            assert_eq!(
                store
                    .inventory_operation(
                        binding.clone(),
                        minimum,
                        OperationId::from_bytes([8; 32]),
                        None,
                        proof(e.proof())
                    )
                    .await
                    .unwrap(),
                None
            );
            assert_eq!(
                store
                    .pool(
                        binding.clone(),
                        minimum,
                        PoolId::from_bytes([2; 32]),
                        proof(e.proof())
                    )
                    .await
                    .unwrap()
                    .unwrap()
                    .snapshot(),
                pool.pool()
            );
            db.close().await.unwrap();
            store.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn failed_mapping_commit_reports_unknown_fences_the_owner_and_reopens_without_a_partial_effect()
 {
    let f = Fixture::new();
    let (store, e, binding, pool) = seeded(&f).await;
    let key = mapping_key(5, 6);
    let original = mapping_request(&binding, 7, key, 2, 0, grams("250"));
    let digest = original.command().digest();
    let mut db = database(&f).await;
    sqlx::raw_sql("CREATE TABLE mapping_commit_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER mapping_commit_failure AFTER INSERT ON stock_mappings BEGIN INSERT INTO mapping_commit_fault VALUES(2); END").execute(&mut db).await.unwrap();
    let anchor = match store
        .execute_inventory(original, proof(e.proof()))
        .await
        .unwrap()
    {
        InventoryOutcome::Unknown(value) => value,
        _ => panic!("commit must be unknown"),
    };
    assert_eq!(anchor.operation_id(), OperationId::from_bytes([7; 32]));
    assert_eq!(anchor.command_digest(), digest);
    assert!(anchor.transaction_id().is_some());
    assert_eq!(
        store
            .option_mapping(binding.clone(), pool.checkpoint(), key, proof(e.proof()))
            .await,
        Err(OperationError::RecoveryRequired)
    );
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER mapping_commit_failure; DROP TABLE mapping_commit_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .inventory_operation(
                binding.clone(),
                pool.checkpoint(),
                anchor.operation_id(),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .option_mapping(binding.clone(), pool.checkpoint(), key, proof(e.proof()))
            .await
            .unwrap(),
        None
    );
    let result = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 7, key, 2, 0, grams("250")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    assert_eq!(result.mapping().unwrap().version().get(), 1);
    assert_eq!(result.pool(), pool.pool());
    store.close().await.unwrap();
}

#[tokio::test]
async fn older_authentic_mapping_projection_cannot_replace_the_latest_protected_configuration() {
    let f = Fixture::new();
    let (store, e, binding, _) = seeded(&f).await;
    let key = mapping_key(5, 6);
    store
        .execute_inventory(
            mapping_request(&binding, 7, key, 2, 0, grams("250")),
            proof(e.proof()),
        )
        .await
        .unwrap();
    let mut db = database(&f).await;
    let old: Vec<u8> = sqlx::query_scalar("SELECT protected FROM stock_mappings")
        .fetch_one(&mut db)
        .await
        .unwrap();
    let latest = committed(
        store
            .execute_inventory(
                mapping_request(&binding, 8, key, 2, 1, grams("500")),
                proof(e.proof()),
            )
            .await
            .unwrap(),
    );
    sqlx::query("UPDATE stock_mappings SET revision=1,protected=?")
        .bind(old)
        .execute(&mut db)
        .await
        .unwrap();
    assert_eq!(
        store
            .option_mapping(binding.clone(), latest.checkpoint(), key, proof(e.proof()))
            .await,
        Err(OperationError::RecoveryRequired)
    );
    assert_eq!(
        store
            .execute_inventory(
                mapping_request(&binding, 9, key, 2, 1, grams("1000")),
                proof(e.proof())
            )
            .await,
        Err(OperationError::RecoveryRequired)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_operations")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(count, 3);
    db.close().await.unwrap();
    store.close().await.unwrap();
}
