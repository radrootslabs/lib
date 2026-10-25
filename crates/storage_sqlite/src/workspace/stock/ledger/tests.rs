//! Framing-only tests: these deliberately supply synthetic body data and do
//! not claim business admission or an accepted stock operation.
use super::*;
use crate::workspace::tests::{CryptoFixture, participant};
use radroots_domain::{ActorRole, OrderId, UnixSeconds};

fn selection() -> AttachmentInput {
    let (input, _, _) = SellerStockConfig::new(
        participant(),
        participant(),
        "frame-fixture",
        UnixSeconds::new(200).unwrap(),
    )
    .unwrap()
    .enroll(
        UnixSeconds::new(100).unwrap(),
        WorkspaceId::from_bytes([1; 32]),
        ExecutorId::from_bytes([2; 32]),
        GrantId::from_bytes([3; 32]),
        AttachmentProof::from_protected_input(&[4; 32]).unwrap(),
    )
    .unwrap();
    input
}
fn header(input: &AttachmentInput) -> Header {
    Header {
        version: 5,
        family: Family::Acceptance,
        namespace: SellerAuthoritySelection::from_attachment(input),
        actor: ActorRef::new(
            participant(),
            ActorRole::Seller,
            None,
            Some(input.grant_id()),
        ),
        operation: OperationId::from_bytes([5; 32]),
        digest: CommandDigest::from_bytes([6; 32]),
        transaction: TransactionId::from_bytes([7; 32]),
        revision: Sequence::new(1).unwrap(),
        previous: None,
        head: CheckpointDigest::from_bytes([8; 32]),
        pool: None,
        order: Some(OrderId::from_bytes([9; 32])),
        related_operation: None,
        subject_grant: None,
        bytes: 0,
        parts: 0,
        body_digest: [0; 32],
    }
}
async fn database(input: &AttachmentInput) -> SqliteConnection {
    let mut connection = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    for (_, _, _, sql) in super::super::schema::objects(7).unwrap() {
        sqlx::raw_sql(sql).execute(&mut connection).await.unwrap();
    }
    // Only the foreign-key metadata needed by framing is installed. This is
    // not an enrolled NativeWorkspace and supplies no authenticated grant.
    sqlx::query("INSERT INTO stock_grants(grant_id,protected) VALUES(?,?)")
        .bind(input.grant_id().as_bytes().as_slice())
        .bind([1_u8].as_slice())
        .execute(&mut connection)
        .await
        .unwrap();
    connection
}
async fn insert(connection: &mut SqliteConnection, input: &AttachmentInput, prepared: &Prepared) {
    let value = header(input);
    let mut tx = connection.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO stock_operations(actor,grant_id,operation_id,command_digest,transaction_id,pool_id,revision,chain_head,protected,family,order_id,related_operation) VALUES(?,?,?,?,?,NULL,?,?,?,2,?,NULL)")
        .bind(input.participant().as_bytes().as_slice()).bind(input.grant_id().as_bytes().as_slice())
        .bind(value.operation.as_bytes().as_slice()).bind(value.digest.as_bytes().as_slice()).bind(value.transaction.as_bytes().as_slice())
        .bind(value.revision.get() as i64).bind(value.head.as_bytes().as_slice()).bind(&prepared.protected)
        .bind(value.order.unwrap().as_bytes().as_slice()).execute(&mut *tx).await.unwrap();
    prepared
        .insert_parts(&mut tx, value.actor, value.operation)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn actual_protected_multipart_body_roundtrips_exact_boundaries_and_sql_witnesses() {
    let fixture = CryptoFixture::new();
    let input = selection();
    let crypto = fixture.crypto();
    for length in [1, PART_BYTES, PART_BYTES + 1, BODY_MAX_BYTES] {
        let mut connection = database(&input).await;
        let body = vec![0x51; length];
        let prepared = prepare(&crypto, &input, header(&input), &body)
            .await
            .unwrap();
        assert_eq!(prepared.parts.len(), length.div_ceil(PART_BYTES));
        insert(&mut connection, &input, &prepared).await;
        let value = read(&mut connection, &input, &crypto, header(&input).operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(value.body.as_slice(), body.as_slice());
        assert_eq!(value.protected, prepared.protected);
        confirm_parts(
            &mut connection,
            value.header.actor,
            value.header.operation,
            value.parts_witness,
            value.header.parts,
        )
        .await
        .unwrap();
        sqlx::query("PRAGMA foreign_keys=OFF")
            .execute(&mut connection)
            .await
            .unwrap();
        sqlx::query("UPDATE stock_operation_parts SET grant_id=zeroblob(32) WHERE part=0")
            .execute(&mut connection)
            .await
            .unwrap();
        assert_eq!(
            confirm_parts(
                &mut connection,
                value.header.actor,
                value.header.operation,
                value.parts_witness,
                value.header.parts
            )
            .await,
            Err(OperationError::RecoveryRequired)
        );
        assert!(
            read(&mut connection, &input, &crypto, value.header.operation)
                .await
                .is_err()
        );
        sqlx::query("UPDATE stock_operation_parts SET grant_id=? WHERE part=0")
            .bind(input.grant_id().as_bytes().as_slice())
            .execute(&mut connection)
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&mut connection)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE stock_operation_parts SET protected=randomblob(length(protected)) WHERE part=0",
        )
        .execute(&mut connection)
        .await
        .unwrap();
        assert_eq!(
            confirm_parts(
                &mut connection,
                value.header.actor,
                value.header.operation,
                value.parts_witness,
                value.header.parts
            )
            .await,
            Err(OperationError::RecoveryRequired)
        );
        assert!(
            read(&mut connection, &input, &crypto, header(&input).operation)
                .await
                .is_err()
        );
        connection.close().await.unwrap();
    }
    assert!(matches!(
        prepare(&crypto, &input, header(&input), &[]).await,
        Err(OperationError::Workspace(WorkspaceError::Capacity))
    ));
    assert!(matches!(
        prepare(
            &crypto,
            &input,
            header(&input),
            &vec![0x51; BODY_MAX_BYTES + 1]
        )
        .await,
        Err(OperationError::Workspace(WorkspaceError::Capacity))
    ));
}

#[tokio::test]
async fn substituted_family_scope_and_missing_or_reordered_body_parts_refuse() {
    let fixture = CryptoFixture::new();
    let input = selection();
    let crypto = fixture.crypto();
    for statement in [
        "DELETE FROM stock_operation_parts WHERE part=1",
        "UPDATE stock_operation_parts SET protected=(SELECT protected FROM stock_operation_parts WHERE part=0) WHERE part=1",
        "UPDATE stock_operations SET command_digest=zeroblob(32)",
        "UPDATE stock_operations SET transaction_id=zeroblob(32)",
        "UPDATE stock_operations SET order_id=zeroblob(32)",
    ] {
        let mut connection = database(&input).await;
        let prepared = prepare(&crypto, &input, header(&input), &vec![0x51; PART_BYTES + 1])
            .await
            .unwrap();
        insert(&mut connection, &input, &prepared).await;
        sqlx::raw_sql(statement)
            .execute(&mut connection)
            .await
            .unwrap();
        assert!(
            read(&mut connection, &input, &crypto, header(&input).operation)
                .await
                .is_err()
        );
        connection.close().await.unwrap();
    }
    let mut bad = header(&input);
    bad.family = Family::Inventory;
    assert!(matches!(
        prepare(&crypto, &input, bad, b"frame").await,
        Err(OperationError::RecoveryRequired)
    ));
    let mut bad = header(&input);
    bad.related_operation = Some(OperationId::from_bytes([0xa1; 32]));
    assert!(matches!(
        prepare(&crypto, &input, bad, b"frame").await,
        Err(OperationError::RecoveryRequired)
    ));
}
