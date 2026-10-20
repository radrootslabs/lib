//! Actual encrypted public T0/T1/S. Installation and delivery remain separate.
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
    retain_legacy_issuance_catalog(&mut db).await;
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

mod public_issuance {
    use super::*;
    use core::{cell::Cell, num::NonZeroU64};
    use nostr::JsonUtil;
    use radroots_nostr::{
        key::parse_secret_key,
        signing::{LocalSigner, OsNonceSource},
    };
    use radroots_protocol::plan::SigningTarget;
    use radroots_signing::{
        issuance::{OutcomeKind, SigningIntent},
        provider::{ParticipantProvider, ProviderCall},
        request::CancellationSignal,
    };
    use radroots_storage::command::{
        ArtifactEvidence, ClaimAttemptOutcome, EvidenceCaptureOutcome, SigningAttempt,
    };
    thread_local! { static CALLS:Cell<usize>=const {Cell::new(0)}; }
    fn signer_clock() -> Result<u64, radroots_signing::Error> {
        let first = CALLS.with(|calls| {
            let n = calls.get();
            calls.set(n + 1);
            n == 0
        });
        if first {
            Ok(100_000)
        } else {
            Err(radroots_signing::Error::new(
                radroots_signing::error::Kind::InternalError,
            ))
        }
    }
    fn selection(
        binding: &WorkspaceBinding,
        prepared: &PreparedOperation,
        digest: [u8; 32],
    ) -> ClaimAttemptRequest {
        ClaimAttemptRequest::for_artifact(
            binding.clone(),
            prepared.identity().operation_id(),
            prepared.identity().command_digest(),
            digest,
            SigningTarget::PublicEvent,
        )
    }
    fn claimed(outcome: ClaimAttemptOutcome) -> SigningAttempt {
        match outcome {
            ClaimAttemptOutcome::Claimed(attempt) => attempt,
            _ => panic!("first claim must confirm commit"),
        }
    }
    async fn prepared(
        fixture: &Fixture,
        store: &NativeWorkspace,
        binding: &WorkspaceBinding,
        proof: &AttachmentProof,
        id: u8,
    ) -> (PreparedOperation, [u8; 32]) {
        let request = public_request(binding, id, "public signature fixture", "original");
        let mut wire: serde_json::Value =
            serde_json::from_slice(request.command().protected_wire()).unwrap();
        wire["input"]["profile"]["product"]["description"] = json!("x".repeat(4096));
        let request = AdmissionRequest::from_protected_wire(
            binding.clone(),
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        let prepared = committed(
            store
                .admit_command_and_hold(request, copy(proof))
                .await
                .unwrap(),
        );
        let plan = public_plan(
            &prepared,
            100,
            &["wss://public-signature.fixture.example/a".into()],
        );
        assert!(
            plan.event()
                .fields()
                .tags
                .iter()
                .any(|tag| tag.iter().any(|field| field.len() > 4096))
        );
        let digest = *plan.digest();
        planned(
            store
                .bind_signing_intents(binding.clone(), plan.into(), copy(proof))
                .await
                .unwrap(),
        );
        let _ = fixture;
        (prepared, digest)
    }
    async fn obtain(attempt: &SigningAttempt) -> ArtifactEvidence {
        CALLS.with(|calls| calls.set(0));
        // Published synthetic scalar, independent of workstation custody.
        let signer = LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            signer_clock,
        )
        .unwrap()
        .with_nonce_source(Arc::new(OsNonceSource));
        let state = signer.provider_state().await.unwrap();
        assert_eq!(state.identity(), key());
        assert_eq!(
            state.exact_issuance(),
            Some(radroots_signing::recovery::ReplayCapability::NonReplayable)
        );
        let cancellation = CancellationSignal::new();
        let intent = SigningIntent::from_plan(
            attempt.bound_plan().plan(),
            SigningTarget::PublicEvent,
            ProviderCall::new(key(), attempt.deadline_unix_ms(), cancellation.clone()),
        )
        .unwrap();
        assert_eq!(intent.binding(), attempt.binding());
        assert_eq!(intent.recipient(), None);
        let outcome = signer.sign_exact(intent).await;
        cancellation.cancel();
        let (_, kind) = outcome.into_parts();
        let OutcomeKind::Obtained(artifact) = kind else {
            panic!("qualified local signer must obtain public event")
        };
        assert_eq!(artifact.observed_at_unix_ms(), None);
        artifact.with_json(|raw| {
            let independent = nostr::Event::from_json(raw).unwrap();
            independent.verify().unwrap();
            assert_eq!(independent.kind.as_u16(), 30402);
            assert_eq!(
                independent.id.as_bytes(),
                &attempt.artifact().unwrap().event_id()
            );
            ArtifactEvidence::verify(attempt, raw, None).unwrap()
        })
    }
    fn captured(outcome: EvidenceCaptureOutcome) -> radroots_storage::command::EvidenceReceipt {
        match outcome {
            EvidenceCaptureOutcome::Committed(receipt) => receipt,
            _ => panic!("capture must confirm commit"),
        }
    }
    #[tokio::test]
    async fn actual_public_signature_captures_original_bytes_reopens_and_never_reclaims() {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
        let (prepared, digest) = prepared(&fixture, &store, &binding, &proof, 160).await;
        let attempt = claimed(
            store
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
        );
        assert!(matches!(
            attempt.seal(),
            Err(OperationError::UnsupportedOperation)
        ));
        assert!(matches!(
            store
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
            ClaimAttemptOutcome::Retained(_)
        ));
        let evidence = obtain(&attempt).await;
        assert_eq!(evidence.target().recipient(), None);
        use radroots_storage::command::protected::issuance::{ClaimRecord, EvidenceManifest};
        let claim_bytes = ClaimRecord::capture(&attempt).unwrap().encode().unwrap();
        let claim_json: serde_json::Value = serde_json::from_slice(&claim_bytes).unwrap();
        assert_eq!(claim_json["version"], 2);
        assert_eq!(claim_json["role"], 0);
        assert!(claim_json["recipient"].is_null());
        for field in [
            "version",
            "role",
            "recipient",
            "missing_role",
            "missing_recipient",
            "unknown",
        ] {
            let mut changed = claim_json.clone();
            match field {
                "version" => changed["version"] = json!(1),
                "role" => changed["role"] = json!(1),
                "recipient" => changed["recipient"] = json!(key()),
                "missing_role" => {
                    changed.as_object_mut().unwrap().remove("role");
                }
                "missing_recipient" => {
                    changed.as_object_mut().unwrap().remove("recipient");
                }
                _ => changed["unknown"] = json!(true),
            }
            let bound = store
                .signing_plan(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    prepared.identity().command_digest(),
                    copy(&proof),
                )
                .await
                .unwrap()
                .unwrap();
            assert!(
                ClaimRecord::decode(
                    &serde_json::to_vec(&changed).unwrap(),
                    bound,
                    SigningTarget::PublicEvent
                )
                .is_err(),
                "{field}"
            );
        }
        let manifest = EvidenceManifest::capture(
            &attempt,
            &evidence,
            radroots_domain::TransactionId::from_bytes([9; 32]),
        )
        .unwrap()
        .encode()
        .unwrap();
        let metadata: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(metadata["version"], 2);
        assert_eq!(metadata["role"], 0);
        assert!(metadata["recipient"].is_null());
        for field in ["version", "role", "recipient"] {
            let mut changed = metadata.clone();
            changed[field] = match field {
                "recipient" => json!(key()),
                _ => json!(1),
            };
            assert!(
                EvidenceManifest::decode(&serde_json::to_vec(&changed).unwrap(), &attempt).is_err()
            );
        }
        for (field, value) in [
            ("sig", json!("00".repeat(64))),
            ("id", json!("00".repeat(32))),
            ("kind", json!(13)),
            ("tags", json!([])),
            ("content", json!("other")),
            ("pubkey", json!("00".repeat(32))),
            ("created_at", json!(99)),
        ] {
            let mut changed: serde_json::Value = serde_json::from_str(evidence.raw_json()).unwrap();
            changed[field] = value;
            assert!(
                matches!(
                    ArtifactEvidence::verify(&attempt, &changed.to_string(), None),
                    Err(OperationError::InvalidEvidence)
                ),
                "{field}"
            );
        }
        assert!(ArtifactEvidence::verify(&attempt, &"x".repeat(65537), None).is_err());

        let raw = evidence.raw_json().to_owned();
        let receipt = captured(
            store
                .capture_evidence(input.clone(), &proof, &evidence)
                .await
                .unwrap(),
        );
        assert_eq!(receipt.target(), SigningTarget::PublicEvent);
        assert_eq!(receipt.observed_at_unix_ms(), None);
        let repeated = ArtifactEvidence::verify(&attempt, &raw, NonZeroU64::new(112_000)).unwrap();
        assert_eq!(
            captured(
                store
                    .capture_evidence(input.clone(), &proof, &repeated)
                    .await
                    .unwrap()
            ),
            receipt
        );
        // The same valid event with another exact JSON encoding is a conflicting
        // callback, not permission to replace the original obtained bytes.
        let different =
            serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
                .unwrap();
        assert_ne!(raw, different);
        let alternate = ArtifactEvidence::verify(&attempt, &different, None).unwrap();
        assert!(matches!(
            store
                .capture_evidence(input.clone(), &proof, &alternate)
                .await,
            Err(OperationError::IdempotencyConflict)
        ));
        let snapshot = store
            .operation_snapshot(
                binding.clone(),
                prepared.identity().operation_id(),
                Some(prepared.identity().command_digest()),
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.artifacts().len(), 1);
        assert_eq!(snapshot.artifacts()[0].target(), SigningTarget::PublicEvent);
        assert_eq!(snapshot.artifacts()[0].obtained(), Some(receipt));
        assert!(snapshot.installation().is_none());
        store.close().await.unwrap();
        for name in ["execution_v1.sqlite", "execution_v1.sqlite-wal"] {
            if let Ok(bytes) = std::fs::read(fixture.path.join(name)) {
                assert!(!bytes.windows(raw.len()).any(|part| part == raw.as_bytes()));
            }
        }
        fixture.clock.now.store(160, Ordering::SeqCst);
        let reopened = fixture.reopen(input).await;
        let recovered = reopened
            .obtained_evidence(selection(&binding, &prepared, digest), copy(&proof))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.receipt(), receipt);
        assert_eq!(recovered.evidence().raw_json(), raw);
        assert_eq!(recovered.attempt().attempt_id(), attempt.attempt_id());
        assert!(matches!(
            reopened
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
            ClaimAttemptOutcome::Retained(_)
        ));
        reopened.close().await.unwrap();
    }
    #[tokio::test]
    async fn public_obtained_capture_survives_revocation_expiry_and_later_clock_failure() {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
        let (prepared, digest) = prepared(&fixture, &store, &binding, &proof, 161).await;
        let attempt = claimed(
            store
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
        );
        let evidence = obtain(&attempt).await;
        store.revoke(input.clone(), &proof).await.unwrap();
        fixture.clock.now.store(0, Ordering::SeqCst);
        let receipt = captured(
            store
                .capture_evidence(input.clone(), &proof, &evidence)
                .await
                .unwrap(),
        );
        assert_eq!(
            captured(
                store
                    .capture_evidence(input.clone(), &proof, &evidence)
                    .await
                    .unwrap()
            ),
            receipt
        );
        assert!(
            store
                .signing_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .is_err()
        );
        store.close().await.unwrap();
    }
    #[tokio::test]
    async fn public_claim_checks_current_proof_window_and_exact_target_without_dispatch() {
        let fixture = Fixture::new();
        let (store, _, proof, binding) = fixture.create_scoped(200, true).await;
        let (prepared, digest) = prepared(&fixture, &store, &binding, &proof, 162).await;
        assert!(matches!(
            store
                .claim_attempt(
                    selection(&binding, &prepared, digest),
                    AttachmentProof::from_protected_input(&[0; 32]).unwrap()
                )
                .await,
            Err(OperationError::Workspace(WorkspaceError::Unauthorized))
        ));
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
        fixture.clock.now.store(150, Ordering::SeqCst);
        assert!(matches!(
            store
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await,
            Err(OperationError::DeadlineExceeded)
        ));
        let mut db = fixture.database().await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_attempts")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(count, 0);
        db.close().await.unwrap();
        store.close().await.unwrap();
    }
    #[tokio::test]
    async fn lost_public_claim_observer_retains_potential_issuance_without_new_dispatch() {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
        let (prepared, digest) = prepared(&fixture, &store, &binding, &proof, 163).await;
        let (arrived, resume) = fixture.clock.arm(1);
        let mut pending = store.claim_attempt(selection(&binding, &prepared, digest), copy(&proof));
        assert!(futures::poll!(pending.as_mut()).is_pending());
        reached(arrived).await;
        drop(pending);
        resume.send(()).unwrap();
        store.close().await.unwrap();
        let reopened = fixture.reopen(input).await;
        assert!(matches!(
            reopened
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
            ClaimAttemptOutcome::Retained(_)
        ));
        assert!(
            reopened
                .obtained_evidence(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap()
                .is_none()
        );
        reopened.close().await.unwrap();
    }
    #[tokio::test]
    async fn accepted_public_capture_survives_observer_loss_and_drains_on_close() {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
        let (prepared, digest) = prepared(&fixture, &store, &binding, &proof, 164).await;
        let attempt = claimed(
            store
                .claim_attempt(selection(&binding, &prepared, digest), copy(&proof))
                .await
                .unwrap(),
        );
        let evidence = obtain(&attempt).await;
        let mut db = fixture.database().await;
        let transaction = db.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let mut pending = store.capture_evidence(input.clone(), &proof, &evidence);
        assert!(futures::poll!(pending.as_mut()).is_pending());
        drop(pending);
        transaction.rollback().await.unwrap();
        db.close().await.unwrap();
        store.close().await.unwrap();
        let reopened = fixture.reopen(input).await;
        let captured = reopened
            .obtained_evidence(selection(&binding, &prepared, digest), copy(&proof))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.evidence().raw_json(), evidence.raw_json());
        reopened.close().await.unwrap();
    }
}

pub(super) async fn retain_legacy_issuance_catalog(db: &mut SqliteConnection) {
    use sqlx::Row;
    let rows=sqlx::query("SELECT name,sql FROM sqlite_schema WHERE name IN ('signing_attempts','signing_attempt_identity','signing_attempt_transaction_identity','artifact_evidence','artifact_evidence_transaction_identity','artifact_evidence_parts','chat_publication_work')").fetch_all(&mut *db).await.unwrap();
    let definitions = rows
        .into_iter()
        .map(|row| {
            let name: String = row.get(0);
            let ddl: String = row.get(1);
            let ddl = ddl
                .replace("artifact_evidence", "seal_evidence")
                .replace("artifact_key", "recipient")
                .replace(
                    "CHECK(length(recipient)=32 OR (length(recipient)=1 AND hex(recipient)='00'))",
                    "CHECK(length(recipient)=32)",
                );
            (name, ddl)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for table in [
        "signing_attempts",
        "artifact_evidence",
        "artifact_evidence_parts",
        "chat_publication_work",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TEMP TABLE retained_{table} AS SELECT * FROM {table}"
        )))
        .execute(&mut *db)
        .await
        .unwrap();
    }
    for table in [
        "chat_publication_work",
        "artifact_evidence_parts",
        "artifact_evidence",
        "signing_attempts",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE {table}")))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    for name in [
        "signing_attempts",
        "signing_attempt_identity",
        "signing_attempt_transaction_identity",
        "artifact_evidence",
        "artifact_evidence_transaction_identity",
        "artifact_evidence_parts",
        "chat_publication_work",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(definitions[name].clone()))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    for table in [
        "signing_attempts",
        "artifact_evidence",
        "artifact_evidence_parts",
        "chat_publication_work",
    ] {
        let legacy = table.replace("artifact_evidence", "seal_evidence");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {legacy} SELECT * FROM retained_{table}"
        )))
        .execute(&mut *db)
        .await
        .unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE retained_{table}")))
            .execute(&mut *db)
            .await
            .unwrap();
    }
    sqlx::query("PRAGMA user_version=8")
        .execute(db)
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_generation_eight_preserves_public_t1_and_refuses_public_s_without_rewrite() {
    use radroots_protocol::plan::SigningTarget;
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create_scoped(200, true).await;
    let prepared = committed(
        store
            .admit_command_and_hold(
                public_request(&binding, 165, "retained generation8 public plan", "first"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let plan = public_plan(
        &prepared,
        100,
        &["wss://public-signature.fixture.example/a".into()],
    );
    let wire = plan.protected_wire().to_vec();
    let digest = *plan.digest();
    planned(
        store
            .bind_signing_intents(binding.clone(), plan.into(), copy(&proof))
            .await
            .unwrap(),
    );
    store.close().await.unwrap();
    let mut db = fixture.database().await;
    retain_legacy_issuance_catalog(&mut db).await;
    db.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    let retained = reopened
        .signing_plan(
            binding.clone(),
            prepared.identity().operation_id(),
            prepared.identity().command_digest(),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.plan().protected_wire(), wire);
    let request = || {
        ClaimAttemptRequest::for_artifact(
            binding.clone(),
            prepared.identity().operation_id(),
            prepared.identity().command_digest(),
            digest,
            SigningTarget::PublicEvent,
        )
    };
    assert!(
        reopened
            .obtained_evidence(request(), copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        reopened.claim_attempt(request(), copy(&proof)).await,
        Err(OperationError::UnsupportedOperation)
    ));
    reopened.close().await.unwrap();
    let mut db = fixture.database().await;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut db)
        .await
        .unwrap();
    assert_eq!(version, 8);
    db.close().await.unwrap();
}
