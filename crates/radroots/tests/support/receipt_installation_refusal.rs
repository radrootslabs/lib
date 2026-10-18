use super::*;
use sqlx::Row;

async fn message_row(database: &mut SqliteConnection) -> Vec<Vec<u8>> {
    let row = sqlx::query(
        "SELECT message_id,thread_id,plan_digest,transaction_id,protected FROM chat_messages",
    )
    .fetch_one(database)
    .await
    .unwrap();
    (0..5).map(|index| row.try_get(index).unwrap()).collect()
}
async fn work_row(database: &mut SqliteConnection) -> Vec<Vec<u8>> {
    let row=sqlx::query("SELECT recipient,installation_transaction,capture_transaction,event_id,evidence_digest,transaction_id,protected FROM chat_publication_work").fetch_one(database).await.unwrap();
    (0..7).map(|index| row.try_get(index).unwrap()).collect()
}

#[tokio::test]
async fn protected_message_and_work_projection_corruption_is_refused_without_repair() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 225, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    capture(&store, &input, &proof, &attempt).await;
    installed(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let mut database = fixture.database().await;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut database)
        .await
        .unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    let original = message_row(&mut database).await;
    for statement in [
        "UPDATE chat_messages SET message_id=zeroblob(32)",
        "UPDATE chat_messages SET thread_id=zeroblob(32)",
        "UPDATE chat_messages SET plan_digest=zeroblob(32)",
        "UPDATE chat_messages SET transaction_id=zeroblob(32)",
        "UPDATE chat_messages SET protected=zeroblob(0)",
        "UPDATE chat_messages SET protected=zeroblob(262145)",
        "UPDATE chat_messages SET protected=zeroblob(262144)",
        "UPDATE chat_messages SET message_id=zeroblob(31)",
        "UPDATE chat_messages SET thread_id=zeroblob(31)",
        "UPDATE chat_messages SET plan_digest=zeroblob(31)",
        "UPDATE chat_messages SET transaction_id=zeroblob(31)",
    ] {
        sqlx::query(statement).execute(&mut database).await.unwrap();
        let corrupt = message_row(&mut database).await;
        assert!(
            store
                .operation_snapshot(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    None,
                    copy(&proof)
                )
                .await
                .is_err(),
            "{statement}"
        );
        assert_eq!(message_row(&mut database).await, corrupt);
        sqlx::query("UPDATE chat_messages SET message_id=?,thread_id=?,plan_digest=?,transaction_id=?,protected=?")
            .bind(&original[0]).bind(&original[1]).bind(&original[2]).bind(&original[3]).bind(&original[4]).execute(&mut database).await.unwrap();
    }
    let original = work_row(&mut database).await;
    for statement in [
        "UPDATE chat_publication_work SET recipient=zeroblob(32)",
        "UPDATE chat_publication_work SET installation_transaction=zeroblob(32)",
        "UPDATE chat_publication_work SET capture_transaction=zeroblob(32)",
        "UPDATE chat_publication_work SET event_id=zeroblob(32)",
        "UPDATE chat_publication_work SET evidence_digest=zeroblob(32)",
        "UPDATE chat_publication_work SET transaction_id=zeroblob(32)",
        "UPDATE chat_publication_work SET protected=zeroblob(0)",
        "UPDATE chat_publication_work SET protected=zeroblob(262145)",
        "UPDATE chat_publication_work SET protected=zeroblob(262144)",
        "UPDATE chat_publication_work SET installation_transaction=zeroblob(31)",
        "UPDATE chat_publication_work SET capture_transaction=zeroblob(31)",
        "UPDATE chat_publication_work SET event_id=zeroblob(31)",
        "UPDATE chat_publication_work SET evidence_digest=zeroblob(31)",
        "UPDATE chat_publication_work SET transaction_id=zeroblob(31)",
    ] {
        sqlx::query(statement).execute(&mut database).await.unwrap();
        let corrupt = work_row(&mut database).await;
        assert!(
            store
                .operation_snapshot(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    None,
                    copy(&proof)
                )
                .await
                .is_err(),
            "{statement}"
        );
        assert_eq!(work_row(&mut database).await, corrupt);
        sqlx::query("UPDATE chat_publication_work SET recipient=?,installation_transaction=?,capture_transaction=?,event_id=?,evidence_digest=?,transaction_id=?,protected=?")
            .bind(&original[0]).bind(&original[1]).bind(&original[2]).bind(&original[3]).bind(&original[4]).bind(&original[5]).bind(&original[6]).execute(&mut database).await.unwrap();
    }
    // A retained installation cannot silently recreate its missing atomic basis.
    sqlx::query("DELETE FROM chat_publication_work")
        .execute(&mut database)
        .await
        .unwrap();
    assert!(matches!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_publication_work")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO chat_publication_work(workspace,operation_id,recipient,installation_transaction,capture_transaction,event_id,evidence_digest,transaction_id,protected) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(binding.input().workspace_id().as_bytes().as_slice()).bind(prepared.identity().operation_id().as_bytes().as_slice()).bind(&original[0]).bind(&original[1]).bind(&original[2]).bind(&original[3]).bind(&original[4]).bind(&original[5]).bind(&original[6]).execute(&mut database).await.unwrap();
    for number in 0..10u8 {
        sqlx::query("INSERT INTO chat_publication_work SELECT workspace,operation_id,?1,installation_transaction,capture_transaction,event_id,evidence_digest,transaction_id,protected FROM chat_publication_work WHERE recipient=?2")
            .bind([0x80+number;32].as_slice()).bind(&original[0]).execute(&mut database).await.unwrap();
    }
    assert!(
        store
            .operation_snapshot(
                binding.clone(),
                prepared.identity().operation_id(),
                None,
                copy(&proof)
            )
            .await
            .is_err()
    );
    assert!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof)
            )
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_publication_work")
        .fetch_one(&mut database)
        .await
        .unwrap();
    assert_eq!(count, 11);
    sqlx::query("DELETE FROM chat_publication_work WHERE recipient!=?")
        .bind(&original[0])
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    assert!(
        snapshot(&store, &binding, &prepared, &proof)
            .await
            .installed_message()
            .is_some()
    );
    store.close().await.unwrap();
}

#[tokio::test]
async fn installation_checks_original_scope_current_grant_and_transaction_clock_without_releasing_evidence()
 {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let (prepared, digest, recipients) =
        prepare(&fixture, &store, &binding, &proof, 226, false).await;
    let attempt = claim(
        store
            .claim_attempt(
                selection(&binding, &prepared, digest, recipients[0]),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let original = capture(&store, &input, &proof, &attempt).await;
    let wrong = InstallationRequest::new(
        binding.clone(),
        prepared.identity().operation_id(),
        CommandDigest::from_bytes([0x91; 32]),
        digest,
    );
    assert!(matches!(
        store
            .install_transition_and_outbox(wrong, copy(&proof))
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    assert!(matches!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, [0x92; 32]),
                copy(&proof)
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    fixture.clock.now.store(110, Ordering::SeqCst);
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let selected = request_install(&binding, &prepared, digest);
    let secret = copy(&proof);
    let observer =
        tokio::spawn(async move { owner.install_transition_and_outbox(selected, secret).await });
    reached(arrived).await;
    fixture.clock.now.store(105, Ordering::SeqCst);
    resume.send(()).unwrap();
    assert!(matches!(
        observer.await.unwrap(),
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    let state = snapshot(&store, &binding, &prepared, &proof).await;
    assert!(state.installation().is_none());
    assert_eq!(
        state
            .recipients()
            .iter()
            .filter_map(|value| value.obtained())
            .collect::<Vec<_>>(),
        vec![original]
    );
    store.revoke(input, &proof).await.unwrap();
    assert!(
        store
            .install_transition_and_outbox(
                request_install(&binding, &prepared, digest),
                copy(&proof)
            )
            .await
            .is_err()
    );
    let mut database = fixture.database().await;
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM chat_messages),(SELECT count(*) FROM chat_publication_work),(SELECT count(*) FROM seal_evidence)").fetch_one(&mut database).await.unwrap();
    assert_eq!(counts, (0, 0, 1));
    database.close().await.unwrap();
    store.close().await.unwrap();
}
