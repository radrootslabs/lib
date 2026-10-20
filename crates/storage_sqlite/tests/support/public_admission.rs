//! Actual encrypted T0 only. No signature, installation or delivery is claimed.
use super::*;
use radroots_protocol::{listing::PublicSigningPlan, plan::SigningReplay};
use radroots_storage::command::{BoundSigningPlan, ClaimAttemptRequest, PlanBindingOutcome};

fn public_plan(prepared: &PreparedOperation, time: u64, targets: &[String]) -> PublicSigningPlan {
    PublicSigningPlan::capture(
        prepared.request().command().listing().unwrap(),
        UnixSeconds::new(time).unwrap(),
        targets,
        SigningReplay::NonReplayable,
    )
    .unwrap()
}
fn planned(outcome: PlanBindingOutcome) -> BoundSigningPlan {
    match outcome {
        PlanBindingOutcome::Committed(plan) => plan,
        PlanBindingOutcome::Unknown(_) => panic!("unexpected unknown T1"),
    }
}

#[tokio::test]
async fn public_t1_retains_exact_encrypted_plan_reopens_and_replays_without_fresh_deadline() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let prepared = committed(
        store
            .admit_command_and_hold(
                public_request(&binding, 80, "original public T1", "first"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let targets = vec!["wss://public-plan.fixture.example/a".to_string()];
    fixture.clock.now.store(110, Ordering::SeqCst);
    let plan = public_plan(&prepared, 101, &targets);
    let wire = plan.protected_wire().to_vec();
    let digest = *plan.digest();
    let event = *plan.event().event_id();
    let retained = planned(
        store
            .bind_signing_intents(binding.clone(), plan.into(), copy(&proof))
            .await
            .unwrap(),
    );
    let transaction = retained.transaction_id();
    assert_eq!(retained.bound_at().get(), 110);
    assert_eq!(retained.plan().digest(), &digest);
    assert_eq!(
        retained.plan().listing().unwrap().event().event_id(),
        &event
    );
    assert_eq!(
        retained.chat_plan().err(),
        Some(OperationError::UnsupportedOperation)
    );
    assert!(matches!(
        store
            .claim_attempt(
                ClaimAttemptRequest::new(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    prepared.identity().command_digest(),
                    digest,
                    key()
                ),
                copy(&proof)
            )
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    let mut db = fixture.database().await;
    let attempts: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_attempts")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(attempts, 0);
    db.close().await.unwrap();
    store.close().await.unwrap();
    for name in ["execution_v1.sqlite", "execution_v1.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(fixture.path.join(name)) {
            assert!(
                !bytes
                    .windows(targets[0].len())
                    .any(|w| w == targets[0].as_bytes())
            );
        }
    }
    fixture.clock.now.store(160, Ordering::SeqCst);
    let reopened = fixture.reopen(input).await;
    let queried = reopened
        .signing_plan(
            binding.clone(),
            prepared.identity().operation_id(),
            prepared.identity().command_digest(),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queried.transaction_id(), transaction);
    assert_eq!(queried.plan().protected_wire(), wire);
    let replay = planned(
        reopened
            .bind_signing_intents(
                binding.clone(),
                public_plan(&prepared, 101, &targets).into(),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(replay.transaction_id(), transaction);
    assert_eq!(replay.plan().protected_wire(), wire);
    let other = vec!["wss://public-plan.fixture.example/other".to_string()];
    assert!(matches!(
        reopened
            .bind_signing_intents(
                binding,
                public_plan(&prepared, 101, &other).into(),
                copy(&proof)
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn public_t1_refuses_wrong_proof_and_fresh_expiry_without_creating_a_plan() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create_scoped(200, true).await;
    let prepared = committed(
        store
            .admit_command_and_hold(
                public_request(&binding, 81, "fresh public T1", "first"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let targets = vec!["wss://public-plan.fixture.example/a".to_string()];
    assert!(matches!(
        store
            .bind_signing_intents(
                binding.clone(),
                public_plan(&prepared, 100, &targets).into(),
                AttachmentProof::from_protected_input(&[0; 32]).unwrap()
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    fixture.clock.now.store(150, Ordering::SeqCst);
    assert!(matches!(
        store
            .bind_signing_intents(
                binding.clone(),
                public_plan(&prepared, 100, &targets).into(),
                copy(&proof)
            )
            .await,
        Err(OperationError::DeadlineExceeded)
    ));
    assert!(
        store
            .signing_plan(
                binding,
                prepared.identity().operation_id(),
                prepared.identity().command_digest(),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    let mut db = fixture.database().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plans")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.close().await.unwrap();
    store.close().await.unwrap();
}

#[tokio::test]
async fn accepted_public_t1_survives_observer_loss_and_close_drains_the_original_plan() {
    use futures::FutureExt;
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let prepared = committed(
        store
            .admit_command_and_hold(
                public_request(&binding, 82, "owned public T1", "first"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let plan = public_plan(
        &prepared,
        100,
        &["wss://public-plan.fixture.example/a".to_string()],
    );
    let wire = plan.protected_wire().to_vec();
    let (arrived, resume) = fixture.clock.arm(0);
    assert!(
        store
            .bind_signing_intents(binding.clone(), plan.into(), copy(&proof))
            .now_or_never()
            .is_none()
    );
    reached(arrived).await;
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let retained = reopened
        .signing_plan(
            binding,
            prepared.identity().operation_id(),
            prepared.identity().command_digest(),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.plan().protected_wire(), wire);
    assert!(retained.plan().listing().is_some());
    reopened.close().await.unwrap();
}

fn public_request(
    binding: &WorkspaceBinding,
    id: u8,
    title: &str,
    correlation: &str,
) -> AdmissionRequest {
    let context = CommandContextData::new(
        OperationId::from_bytes([id; 32]),
        binding.actor(),
        binding.input().workspace_id(),
        binding.input().authority(),
        binding.input().route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        Some(CorrelationId::new(correlation).unwrap()),
    )
    .unwrap();
    let wire = json!({"type":"listing.publish", "context":context,"input":{
        "id":"01".repeat(16),"title":title,"summary":"Quote required","description_markdown":"Public terms",
        "published_at":"100","profile":{"profile":"radroots.listing","version":1,"state":"active",
        "product":{"name":"Carrots","description":"Public description","producer":null,"origin_summary":null},
        "default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"One bunch",
        "unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},"increment":"1",
        "minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Example region","lead_seconds":"0"},"authority":null,"media":[]}}});
    AdmissionRequest::from_protected_wire(binding.clone(), &serde_json::to_vec(&wire).unwrap())
        .unwrap()
}
async fn count(fixture: &Fixture) -> i64 {
    let mut db = fixture.database().await;
    let count = sqlx::query_scalar("SELECT count(*) FROM operations")
        .fetch_one(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    count
}

#[tokio::test]
async fn public_t0_reopen_replay_retains_original_bytes_and_uses_the_same_operation_namespace() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let captured = public_request(&binding, 70, "public-preparation-carrots", "first");
    let digest = captured.command().digest();
    let wire = captured.command().protected_wire().to_vec();
    let original = committed(
        store
            .admit_command_and_hold(captured, copy(&proof))
            .await
            .unwrap(),
    );
    let anchor = original.identity();
    assert_eq!(original.revision().get(), 1);
    assert!(original.request().command().listing().is_some());
    assert_eq!(
        original.request().chat_command().err(),
        Some(OperationError::UnsupportedOperation)
    );
    store.close().await.unwrap();
    for name in ["execution_v1.sqlite", "execution_v1.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(fixture.path.join(name)) {
            assert!(
                !bytes
                    .windows(b"public-preparation-carrots".len())
                    .any(|w| w == b"public-preparation-carrots")
            );
        }
    }
    fixture.clock.now.store(160, Ordering::SeqCst);
    let reopened = fixture.reopen(input).await;
    let retained = reopened
        .operation(
            binding.clone(),
            anchor.operation_id(),
            Some(digest),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.identity(), anchor);
    assert_eq!(retained.issuance_window(), original.issuance_window());
    assert_eq!(retained.request().command().protected_wire(), wire);
    let replay = committed(
        reopened
            .admit_command_and_hold(
                public_request(&binding, 70, "public-preparation-carrots", "second"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(replay.identity(), anchor);
    assert_eq!(replay.request().command().protected_wire(), wire);
    for captured in [
        public_request(&binding, 70, "changed terms", "second"),
        request(&binding, 70, "different family", "second"),
    ] {
        assert!(matches!(
            reopened
                .admit_command_and_hold(captured, copy(&proof))
                .await,
            Err(OperationError::IdempotencyConflict)
        ));
    }
    assert!(matches!(
        reopened
            .admit_command_and_hold(
                public_request(&binding, 71, "new expired work", "second"),
                copy(&proof)
            )
            .await,
        Err(OperationError::DeadlineExceeded)
    ));
    assert_eq!(count(&fixture).await, 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn public_t0_requires_actual_seller_access_and_refuses_unverified_prerequisites_without_rows()
{
    let buyer = Fixture::new();
    let (store, _, proof, binding) = buyer.create().await;
    assert!(matches!(
        store
            .admit_command_and_hold(
                public_request(&binding, 72, "wrong scope", "one"),
                copy(&proof)
            )
            .await,
        Err(OperationError::Unauthorized)
    ));
    assert_eq!(count(&buyer).await, 0);
    store.close().await.unwrap();
    let seller = Fixture::new();
    let (store, input, proof, binding) = seller.create_scoped(200, true).await;
    let wrong = AttachmentProof::from_protected_input(&[0; 32]).unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(public_request(&binding, 72, "wrong proof", "one"), wrong)
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    let captured = public_request(&binding, 72, "prerequisites", "one");
    let base: serde_json::Value =
        serde_json::from_slice(captured.command().protected_wire()).unwrap();
    for (field, value, expected) in [
        (
            "approval",
            json!({"intent_digest":"22".repeat(32),"scope":"unverified","expires_at":"150"}),
            OperationError::UnverifiedApproval,
        ),
        (
            "expected",
            serde_json::to_value(radroots_domain::ExpectedOrder::new(
                radroots_domain::OrderId::from_bytes([0x33; 32]),
                None,
                radroots_domain::Sequence::new(1).unwrap(),
                None,
                binding.input().checkpoint().clone(),
            ))
            .unwrap(),
            OperationError::MissingHistory,
        ),
    ] {
        let mut wire = base.clone();
        wire["context"][field] = value;
        let request = AdmissionRequest::from_protected_wire(
            binding.clone(),
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        assert!(
            matches!(store.admit_command_and_hold(request,copy(&proof)).await,Err(error) if error==expected)
        );
    }
    assert_eq!(count(&seller).await, 0);
    store.revoke(input, &proof).await.unwrap();
    assert!(matches!(
        store
            .admit_command_and_hold(public_request(&binding, 72, "revoked", "one"), copy(&proof))
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert_eq!(count(&seller).await, 0);
    store.close().await.unwrap();
}

#[tokio::test]
async fn accepted_public_t0_survives_observer_loss_and_close_drains_the_existing_owner() {
    use futures::FutureExt;
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let captured = public_request(&binding, 73, "retained after observer loss", "one");
    let digest = captured.command().digest();
    let (arrived, resume) = fixture.clock.arm(0);
    assert!(
        store
            .admit_command_and_hold(captured, copy(&proof))
            .now_or_never()
            .is_none()
    );
    reached(arrived).await;
    resume.send(()).unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let retained = reopened
        .operation(
            binding,
            OperationId::from_bytes([73; 32]),
            Some(digest),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(retained.request().command().listing().is_some());
    assert_eq!(count(&fixture).await, 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn public_queue_counts_decoded_ownership_and_releases_budget_after_draining() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let (arrived, resume) = fixture.clock.arm(1);
    let owner = store.clone();
    let first = public_request(&binding, 100, "first", "one");
    let secret = copy(&proof);
    let blocked = tokio::spawn(async move { owner.admit_command_and_hold(first, secret).await });
    reached(arrived).await;
    let mut queued = Vec::new();
    // Twenty queued plus one in-flight public capture consume 4032 KiB.
    for id in 1..=20 {
        let mut future = store
            .admit_command_and_hold(public_request(&binding, id, "queued", "one"), copy(&proof));
        assert!(futures::poll!(future.as_mut()).is_pending());
        queued.push(future);
    }
    assert!(matches!(
        store
            .admit_command_and_hold(
                public_request(&binding, 21, "overflow", "one"),
                copy(&proof)
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    resume.send(()).unwrap();
    committed(blocked.await.unwrap().unwrap());
    store.close().await.unwrap();
    for future in queued {
        assert!(
            committed(future.await.unwrap())
                .request()
                .command()
                .listing()
                .is_some()
        );
    }
    assert_eq!(count(&fixture).await, 21);
    let reopened = fixture.reopen(input).await;
    assert!(
        reopened
            .operation(
                binding.clone(),
                OperationId::from_bytes([21; 32]),
                None,
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    committed(
        reopened
            .admit_command_and_hold(
                public_request(&binding, 21, "after drain", "one"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert_eq!(count(&fixture).await, 22);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn exact_generation_seven_retains_chat_without_rewrite_and_explicitly_refuses_public_writes()
{
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let original = committed(
        store
            .admit_command_and_hold(
                request(&binding, 74, "original chat", "first"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    store.close().await.unwrap();
    // Construct the exact old catalog around unchanged encrypted rows. This is
    // a retained-schema fixture, not a claim that an old binary was executed.
    let mut db = fixture.database().await;
    let current: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name='operations'")
            .fetch_one(&mut db)
            .await
            .unwrap();
    let index: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE name='operation_transaction_identity'",
    )
    .fetch_one(&mut db)
    .await
    .unwrap();
    let legacy = current.replace("'inbox.reply','listing.publish'", "'inbox.reply'");
    assert_ne!(legacy, current);
    for sql in [
        "CREATE TEMP TABLE retained_operations AS SELECT * FROM operations",
        "DROP TABLE operations",
        &legacy,
        &index,
        "INSERT INTO operations SELECT * FROM retained_operations",
        "DROP TABLE retained_operations",
        "PRAGMA user_version=7",
    ] {
        // Only exact catalog SQL read from our own synthetic database is used.
        sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(&mut db)
            .await
            .unwrap();
    }
    db.close().await.unwrap();
    let reopened = fixture.reopen(input.clone()).await;
    let retained = reopened
        .operation(
            binding.clone(),
            original.identity().operation_id(),
            Some(original.identity().command_digest()),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.identity(), original.identity());
    assert_eq!(
        retained.request().command().protected_wire(),
        original.request().command().protected_wire()
    );
    committed(
        reopened
            .admit_command_and_hold(
                request(&binding, 75, "continued chat", "next"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    assert!(matches!(
        reopened
            .admit_command_and_hold(
                public_request(&binding, 76, "unsupported public", "one"),
                copy(&proof)
            )
            .await,
        Err(OperationError::UnsupportedOperation)
    ));
    reopened.close().await.unwrap();
    let mut db = fixture.database().await;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(version, 7);
    let retained_sql: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name='operations'")
            .fetch_one(&mut db)
            .await
            .unwrap();
    assert_eq!(retained_sql, legacy);
    assert_eq!(count(&fixture).await, 2);
    // A misleading generation tag never makes the new schema an old schema.
    sqlx::query("PRAGMA user_version=8")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &fixture.path,
            input,
            fixture.crypto(),
            fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
}
