use super::*;

#[tokio::test]
async fn real_lifetime_introduced_capacity_is_not_reused_when_available_stock_is_depleted() {
    use radroots_domain::stock::{
        AvailableStockCorrection::{Add, Remove, Spoil},
        StockTicks,
    };
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let selection = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([1; 32]),
        binding.actor(),
        selection.workspace_id(),
        selection.authority(),
        selection.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    let input = CreatePoolInput::new(
        PoolId::from_bytes([2; 32]),
        "capacity",
        PoolUnit::new(MeasurementUnit::Kilogram, 6).unwrap(),
        Quantity::new("999999999999".parse().unwrap(), MeasurementUnit::Kilogram).unwrap(),
    )
    .unwrap();
    let original = InventoryRequest::new(
        binding.clone(),
        minimum(&binding),
        CapturedPoolCreation::new(context, input).unwrap(),
    )
    .unwrap();
    store
        .execute_inventory(original, proof(e.proof()))
        .await
        .unwrap();
    let full = committed(
        store
            .execute_inventory(correction(&binding, 3, 1, Add, "1"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(full.pool().balances().introduced(), StockTicks::MAX);
    store
        .execute_inventory(
            correction(&binding, 4, 2, Remove, "999999999999"),
            proof(e.proof()),
        )
        .await
        .unwrap();
    let depleted = committed(
        store
            .execute_inventory(correction(&binding, 5, 3, Spoil, "1"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(
        depleted.pool().balances().physical().available,
        StockTicks::ZERO
    );
    assert_eq!(depleted.pool().balances().introduced(), StockTicks::MAX);
    assert_eq!(
        store
            .execute_inventory(
                correction(&binding, 9, 4, Add, "0.000001"),
                proof(e.proof())
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Capacity))
    );
    assert_eq!(
        store
            .stock_authority(e.input().clone(), e.proof())
            .await
            .unwrap()
            .inventory_checkpoint(),
        Some(depleted.checkpoint())
    );
    assert!(
        store
            .inventory_operation(
                binding,
                depleted.checkpoint(),
                OperationId::from_bytes([9; 32]),
                None,
                proof(e.proof())
            )
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
}

fn correction(
    binding: &radroots_storage::workspace::WorkspaceBinding,
    operation: u8,
    version: u64,
    kind: radroots_domain::stock::AvailableStockCorrection,
    amount: &str,
) -> InventoryRequest {
    let selection = binding.input();
    let context = CommandContextData::new(
        OperationId::from_bytes([operation; 32]),
        binding.actor(),
        selection.workspace_id(),
        selection.authority(),
        selection.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    InventoryRequest::new(
        binding.clone(),
        minimum(binding),
        radroots_protocol::command::CapturedPoolCorrection::new(
            context,
            radroots_protocol::command::PoolCorrectionInput::new(
                PoolId::from_bytes([2; 32]),
                radroots_domain::Sequence::new(version).unwrap(),
                kind,
                amount.parse().unwrap(),
                "verified harvest correction",
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn actual_corrections_and_spoilage_conserve_stock_reopen_and_replay_original_effects() {
    use radroots_domain::stock::AvailableStockCorrection::{Add, Remove, Spoil};
    use radroots_storage::inventory::InventoryCommandKind;
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let created = committed(
        store
            .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let added = committed(
        store
            .execute_inventory(correction(&binding, 3, 1, Add, "0.125"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(added.kind(), InventoryCommandKind::AddStock);
    assert_eq!(added.pool().balances().introduced().get(), 375);
    assert_eq!(added.pool().balances().physical().available.get(), 375);
    assert_eq!(added.pool().version().get(), 2);
    let removed = committed(
        store
            .execute_inventory(correction(&binding, 4, 2, Remove, "0.05"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(removed.kind(), InventoryCommandKind::RemoveStock);
    assert_eq!(removed.pool().balances().introduced().get(), 375);
    assert_eq!(removed.pool().balances().physical().available.get(), 325);
    assert_eq!(removed.pool().balances().physical().removed.get(), 50);
    let spoiled = committed(
        store
            .execute_inventory(correction(&binding, 5, 3, Spoil, "0.075"), proof(e.proof()))
            .await
            .unwrap(),
    );
    assert_eq!(spoiled.kind(), InventoryCommandKind::SpoilStock);
    assert_eq!(spoiled.pool().version().get(), 4);
    assert_eq!(spoiled.pool().balances().introduced().get(), 375);
    assert_eq!(spoiled.pool().balances().physical().available.get(), 250);
    assert_eq!(spoiled.pool().balances().physical().waste.get(), 75);
    assert_eq!(spoiled.pool().balances().physical().removed.get(), 50);
    for (version, kind, amount, expected) in [
        (1, Add, "1", OperationError::StaleState),
        (4, Remove, "1", OperationError::StaleState),
        (4, Spoil, "0.000001", OperationError::InvalidCommand),
    ] {
        assert_eq!(
            store
                .execute_inventory(
                    correction(&binding, 9, version, kind, amount),
                    proof(e.proof())
                )
                .await,
            Err(expected)
        );
    }
    f.clock.0.store(175, Ordering::SeqCst);
    assert_eq!(
        committed(
            store
                .execute_inventory(correction(&binding, 3, 1, Add, "0.125"), proof(e.proof()))
                .await
                .unwrap()
        ),
        added
    );
    assert_eq!(
        store
            .execute_inventory(correction(&binding, 3, 1, Add, "0.25"), proof(e.proof()))
            .await,
        Err(OperationError::IdempotencyConflict)
    );
    assert_eq!(
        store
            .pool(
                binding.clone(),
                spoiled.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        spoiled.pool()
    );
    assert_eq!(created.kind(), InventoryCommandKind::CreatePool);
    store.close().await.unwrap();
    let store =
        NativeWorkspace::open_seller_stock(&f.path, e.input().clone(), f.crypto(), f.clock.clone())
            .await
            .unwrap();
    assert_eq!(
        store
            .inventory_operation(
                binding.clone(),
                spoiled.checkpoint(),
                OperationId::from_bytes([3; 32]),
                Some(added.attempt().command_digest()),
                proof(e.proof())
            )
            .await
            .unwrap(),
        Some(added)
    );
    assert_eq!(
        store
            .pool(
                binding,
                spoiled.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .label(),
        "Carrots"
    );
    store.close().await.unwrap();
    let mut conn = database(&f).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_pools")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_operations")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_movements")
            .fetch_one(&mut conn)
            .await
            .unwrap(),
        4
    );
    let protected: Vec<u8> =
        sqlx::query_scalar("SELECT protected FROM stock_operations WHERE operation_id=?")
            .bind(OperationId::from_bytes([3; 32]).as_bytes().as_slice())
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&protected).contains("verified harvest"));
    conn.close().await.unwrap();
}

#[tokio::test]
async fn every_actual_correction_write_failure_rolls_back_the_original_pool_and_receipt() {
    use radroots_domain::stock::AvailableStockCorrection::Add;
    for trigger in [
        "CREATE TRIGGER correction_fault BEFORE UPDATE ON stock_pools BEGIN SELECT RAISE(ABORT,'isolated correction fault'); END",
        "CREATE TRIGGER correction_fault BEFORE INSERT ON stock_operations BEGIN SELECT RAISE(ABORT,'isolated correction fault'); END",
        "CREATE TRIGGER correction_fault BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT,'isolated correction fault'); END",
        "CREATE TRIGGER correction_fault BEFORE UPDATE ON stock_authority BEGIN SELECT RAISE(ABORT,'isolated correction fault'); END",
    ] {
        let f = Fixture::new();
        let (store, e, binding) = create(&f).await;
        let original = committed(
            store
                .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
                .await
                .unwrap(),
        );
        let mut conn = database(&f).await;
        sqlx::raw_sql(trigger).execute(&mut conn).await.unwrap();
        assert!(matches!(
            store
                .execute_inventory(correction(&binding, 3, 1, Add, "0.125"), proof(e.proof()))
                .await,
            Err(OperationError::StorageFailure)
        ));
        assert_eq!(
            store
                .pool(
                    binding.clone(),
                    original.checkpoint(),
                    PoolId::from_bytes([2; 32]),
                    proof(e.proof())
                )
                .await
                .unwrap()
                .unwrap()
                .snapshot(),
            original.pool()
        );
        assert!(
            store
                .inventory_operation(
                    binding.clone(),
                    original.checkpoint(),
                    OperationId::from_bytes([3; 32]),
                    None,
                    proof(e.proof())
                )
                .await
                .unwrap()
                .is_none()
        );
        sqlx::raw_sql("DROP TRIGGER correction_fault")
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        store.close().await.unwrap();
        let store = NativeWorkspace::open_seller_stock(
            &f.path,
            e.input().clone(),
            f.crypto(),
            f.clock.clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            store
                .stock_authority(e.input().clone(), e.proof())
                .await
                .unwrap()
                .inventory_checkpoint(),
            Some(original.checkpoint())
        );
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn actual_correction_commit_failure_preserves_original_anchor_and_requires_reopen() {
    use radroots_domain::stock::AvailableStockCorrection::Spoil;
    let f = Fixture::new();
    let (store, e, binding) = create(&f).await;
    let original = committed(
        store
            .execute_inventory(request(&binding, 1, 2, "Carrots"), proof(e.proof()))
            .await
            .unwrap(),
    );
    let mut conn = database(&f).await;
    sqlx::raw_sql("CREATE TABLE deferred_correction_fault(value INTEGER REFERENCES stock_authority(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER correction_commit_fault AFTER INSERT ON stock_movements BEGIN INSERT INTO deferred_correction_fault VALUES(2); END").execute(&mut conn).await.unwrap();
    let command = correction(&binding, 3, 1, Spoil, "0.125");
    let digest = command.command().digest();
    let unknown = match store
        .execute_inventory(command, proof(e.proof()))
        .await
        .unwrap()
    {
        InventoryOutcome::Unknown(anchor) => anchor,
        InventoryOutcome::Committed(_) => panic!("actual COMMIT must fail"),
    };
    assert_eq!(unknown.command_digest(), digest);
    assert_eq!(unknown.operation_id(), OperationId::from_bytes([3; 32]));
    assert!(unknown.transaction_id().is_some());
    assert!(matches!(
        store
            .pool(
                binding.clone(),
                original.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER correction_commit_fault; DROP TABLE deferred_correction_fault")
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
            .inventory_operation(
                binding.clone(),
                original.checkpoint(),
                OperationId::from_bytes([3; 32]),
                Some(digest),
                proof(e.proof())
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .pool(
                binding,
                original.checkpoint(),
                PoolId::from_bytes([2; 32]),
                proof(e.proof())
            )
            .await
            .unwrap()
            .unwrap()
            .snapshot(),
        original.pool()
    );
    store.close().await.unwrap();
}
