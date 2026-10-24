use super::*;
use crate::workspace::{
    fs::Kind,
    tests::{CryptoFixture, participant},
};
use radroots_domain::{CommandContextData, OperationId, PoolId, UnixSeconds};
use radroots_storage::{
    command::OperationError,
    inventory::{CreatePoolRequest, InventoryCheckpointRef, InventoryStore},
};
struct Clock;
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        Ok(UnixSeconds::new(100).unwrap())
    }
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
    let request = CreatePoolRequest::from_protected_wire(
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
        store.create_pool(request, copy).await,
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
