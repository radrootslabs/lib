//! Native storage tests use real synthetic NIP-44 data, never host custody.
#[path = "workspace_issuance.rs"]
mod issuance;
use super::*;
use nostr::{JsonUtil, nips::nip44};
use radroots_domain::EnvelopeEventId;
use radroots_protocol::chat::{CapturedDmRoute, ChatReplay, ChatRumor, ChatSeal, ChatSigningPlan};
use radroots_storage::command::{BoundSigningPlan, PlanBindingOutcome};
fn keys(last: u8) -> nostr::Keys {
    let mut bytes = [0; 32];
    bytes[31] = last;
    nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap())
}
fn peer_request(binding: &WorkspaceBinding, id: u8) -> AdmissionRequest {
    let request = request(binding, id, "protected complete private plan", "original");
    let mut value: serde_json::Value =
        serde_json::from_slice(request.command().protected_wire()).unwrap();
    value["input"]["peer"] = keys(2).public_key().to_hex().into();
    AdmissionRequest::from_protected_wire(binding.clone(), &serde_json::to_vec(&value).unwrap())
        .unwrap()
}
fn plan(
    command: &radroots_protocol::command::CapturedChatCommand,
    created: u64,
    suffix: &str,
) -> ChatSigningPlan {
    let rumor = ChatRumor::from_command(command, UnixSeconds::new(created).unwrap()).unwrap();
    let author = keys(1);
    let mut seals = Vec::new();
    let mut routes = Vec::new();
    for recipient in rumor.recipients() {
        let target = nostr::PublicKey::from_slice(recipient.as_bytes()).unwrap();
        let ciphertext = nip44::encrypt(
            author.secret_key(),
            &target,
            rumor.json(),
            nip44::Version::V2,
        )
        .unwrap();
        seals.push(
            ChatSeal::new(
                &rumor,
                *recipient,
                UnixSeconds::new(created - 1).unwrap(),
                &ciphertext,
            )
            .unwrap(),
        );
        let signer = if recipient == &key() {
            keys(1)
        } else {
            keys(2)
        };
        let url = format!("wss://relay.example/{suffix}");
        let source = nostr::UnsignedEvent {
            id: None,
            pubkey: signer.public_key(),
            created_at: nostr::Timestamp::from_secs(created - 1),
            kind: nostr::Kind::Custom(10050),
            tags: nostr::Tags::from_list(vec![nostr::Tag::parse(["relay", &url]).unwrap()]),
            content: "public signed route provenance ".repeat(1000),
        }
        .sign_with_keys(&signer)
        .unwrap();
        source.verify().unwrap();
        routes.push(
            CapturedDmRoute::from_observed_descriptor(
                *recipient,
                EnvelopeEventId::from_bytes(*source.id.as_bytes()),
                &source.as_json(),
                &[url],
            )
            .unwrap(),
        );
    }
    ChatSigningPlan::new(
        command,
        UnixSeconds::new(created).unwrap(),
        seals,
        routes,
        ChatReplay::NonReplayable,
    )
    .unwrap()
}
fn bound(outcome: PlanBindingOutcome) -> BoundSigningPlan {
    match outcome {
        PlanBindingOutcome::Committed(bound) => bound,
        PlanBindingOutcome::Unknown(_) => panic!("fixture must confirm T1"),
    }
}
#[tokio::test]
async fn complete_private_plan_commits_reopens_and_replays_exact_original_bytes_after_expiry() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let prepared = committed(
        store
            .admit_command_and_hold(peer_request(&binding, 90), copy(&proof))
            .await
            .unwrap(),
    );
    let original = plan(prepared.request().chat_command().unwrap(), 100, "original");
    let wire = original.protected_wire().to_vec();
    let digest = prepared.identity().command_digest();
    let operation = prepared.identity().operation_id();
    assert!(
        store
            .signing_plan(binding.clone(), operation, digest, copy(&proof))
            .await
            .unwrap()
            .is_none()
    );
    let retained = bound(
        store
            .bind_signing_intents(binding.clone(), original, copy(&proof))
            .await
            .unwrap(),
    );
    assert_eq!(retained.plan().seals().len(), 2);
    assert_eq!(retained.plan().protected_wire(), wire);
    assert_eq!(
        retained.prepared().transaction_id(),
        prepared.transaction_id()
    );
    let transaction = retained.transaction_id();
    let altered = plan(prepared.request().chat_command().unwrap(), 100, "changed");
    assert!(matches!(
        store
            .bind_signing_intents(binding.clone(), altered, copy(&proof))
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    fixture.clock.now.store(151, Ordering::SeqCst);
    let replay = ChatSigningPlan::parse(prepared.request().chat_command().unwrap(), &wire).unwrap();
    assert_eq!(
        bound(
            store
                .bind_signing_intents(binding.clone(), replay, copy(&proof))
                .await
                .unwrap()
        )
        .transaction_id(),
        transaction
    );
    store.close().await.unwrap();
    let database_bytes = std::fs::read(fixture.path.join("execution_v1.sqlite")).unwrap();
    assert!(
        !database_bytes
            .windows(wire.len())
            .any(|bytes| bytes == wire)
    );
    assert!(
        !database_bytes
            .windows(31)
            .any(|bytes| bytes == b"protected complete private plan")
    );
    let reopened = fixture.reopen(input).await;
    let recovered = reopened
        .signing_plan(binding.clone(), operation, digest, copy(&proof))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.plan().protected_wire(), wire);
    assert_eq!(recovered.transaction_id(), transaction);
    assert_eq!(recovered.bound_at().get(), 100);
    assert!(matches!(
        reopened
            .signing_plan(
                binding,
                operation,
                CommandDigest::from_bytes([0; 32]),
                copy(&proof)
            )
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn no_new_plan_commits_after_window_or_after_current_grant_revocation() {
    for revoked in [false, true] {
        let fixture = Fixture::new();
        let (store, input, proof, binding) = fixture.create().await;
        let prepared = committed(
            store
                .admit_command_and_hold(peer_request(&binding, 91), copy(&proof))
                .await
                .unwrap(),
        );
        let supplied = plan(prepared.request().chat_command().unwrap(), 100, "one");
        if revoked {
            store.revoke(input, &proof).await.unwrap();
        } else {
            fixture.clock.now.store(150, Ordering::SeqCst);
        }
        let result = store
            .bind_signing_intents(binding, supplied, copy(&proof))
            .await;
        assert!(if revoked {
            matches!(
                result,
                Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
            )
        } else {
            matches!(result, Err(OperationError::DeadlineExceeded))
        });
        store.close().await.unwrap();
        let mut database = fixture.database().await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plans")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(count, 0);
        database.close().await.unwrap();
    }
}
#[tokio::test]
async fn admitted_plan_outlives_its_observer_and_byte_budget_refuses_unadmitted_work() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let mut preparations = Vec::new();
    for id in 100..105 {
        preparations.push(committed(
            store
                .admit_command_and_hold(peer_request(&binding, id), copy(&proof))
                .await
                .unwrap(),
        ));
    }
    let (arrived, resume) = fixture.clock.arm(0);
    let mut first = store.bind_signing_intents(
        binding.clone(),
        plan(
            preparations[0].request().chat_command().unwrap(),
            100,
            "one",
        ),
        copy(&proof),
    );
    assert!(futures::poll!(first.as_mut()).is_pending());
    reached(arrived).await;
    drop(first);
    let mut accepted = Vec::new();
    for prepared in &preparations[1..4] {
        let mut next = store.bind_signing_intents(
            binding.clone(),
            plan(prepared.request().chat_command().unwrap(), 100, "one"),
            copy(&proof),
        );
        assert!(futures::poll!(next.as_mut()).is_pending());
        accepted.push(next);
    }
    assert!(matches!(
        store
            .bind_signing_intents(
                binding.clone(),
                plan(
                    preparations[4].request().chat_command().unwrap(),
                    100,
                    "one"
                ),
                copy(&proof)
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unavailable))
    ));
    resume.send(()).unwrap();
    for next in accepted {
        bound(next.await.unwrap());
    }
    let recovered = store
        .signing_plan(
            binding.clone(),
            preparations[0].identity().operation_id(),
            preparations[0].identity().command_digest(),
            copy(&proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.plan().operation_id(),
        preparations[0].identity().operation_id()
    );
    assert!(
        store
            .signing_plan(
                binding,
                preparations[4].identity().operation_id(),
                preparations[4].identity().command_digest(),
                copy(&proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    store.close().await.unwrap();
}
#[tokio::test]
async fn expiry_during_protection_and_known_sql_rollback_preserve_unbound_preparation() {
    for expire in [false, true] {
        let fixture = Fixture::new();
        let (store, _, proof, binding) = fixture.create().await;
        let prepared = committed(
            store
                .admit_command_and_hold(peer_request(&binding, 110), copy(&proof))
                .await
                .unwrap(),
        );
        let supplied = plan(prepared.request().chat_command().unwrap(), 100, "one");
        let mut database = fixture.database().await;
        if !expire {
            sqlx::raw_sql("CREATE TRIGGER reject_plan BEFORE INSERT ON signing_plan_parts WHEN NEW.part=2 BEGIN SELECT RAISE(ABORT,'synthetic storage failure'); END").execute(&mut database).await.unwrap();
        }
        let (arrived, resume) = fixture.clock.arm(1);
        let mut call = store.bind_signing_intents(binding.clone(), supplied, copy(&proof));
        assert!(futures::poll!(call.as_mut()).is_pending());
        reached(arrived).await;
        if expire {
            fixture.clock.now.store(150, Ordering::SeqCst);
        }
        resume.send(()).unwrap();
        let result = call.await;
        assert!(if expire {
            matches!(result, Err(OperationError::DeadlineExceeded))
        } else {
            matches!(result, Err(OperationError::StorageFailure))
        });
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM signing_plans")
            .fetch_one(&mut database)
            .await
            .unwrap();
        assert_eq!(count, 0);
        if !expire {
            sqlx::raw_sql("DROP TRIGGER reject_plan")
                .execute(&mut database)
                .await
                .unwrap();
        }
        database.close().await.unwrap();
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
        store.close().await.unwrap();
    }
}

#[tokio::test]
async fn unknown_plan_completion_latches_until_reopen_and_preserves_original_operation_anchor() {
    let fixture = Fixture::new();
    let (store, input, proof, binding) = fixture.create().await;
    let prepared = committed(
        store
            .admit_command_and_hold(peer_request(&binding, 120), copy(&proof))
            .await
            .unwrap(),
    );
    let supplied = plan(prepared.request().chat_command().unwrap(), 100, "one");
    let mut database = fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER unknown_plan BEFORE INSERT ON signing_plan_parts WHEN NEW.part=2 BEGIN SELECT RAISE(ROLLBACK,'synthetic unknown completion'); END").execute(&mut database).await.unwrap();
    let outcome = store
        .bind_signing_intents(binding.clone(), supplied, copy(&proof))
        .await
        .unwrap();
    match outcome {
        PlanBindingOutcome::Unknown(anchor) => {
            assert_eq!(anchor.workspace_id(), prepared.identity().workspace_id());
            assert_eq!(anchor.operation_id(), prepared.identity().operation_id());
            assert_eq!(
                anchor.command_digest(),
                prepared.identity().command_digest()
            );
            assert!(anchor.transaction_id().is_some());
        }
        _ => panic!("uncertain rollback must remain unknown"),
    }
    assert!(matches!(
        store
            .signing_plan(
                binding.clone(),
                prepared.identity().operation_id(),
                prepared.identity().command_digest(),
                copy(&proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    sqlx::raw_sql("DROP TRIGGER unknown_plan")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    store.close().await.unwrap();
    let reopened = fixture.reopen(input).await;
    assert!(
        reopened
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
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn retained_plan_refuses_index_manifest_and_part_substitution_without_repair() {
    let fixture = Fixture::new();
    let (store, _, proof, binding) = fixture.create().await;
    let prepared = committed(
        store
            .admit_command_and_hold(peer_request(&binding, 121), copy(&proof))
            .await
            .unwrap(),
    );
    bound(
        store
            .bind_signing_intents(
                binding.clone(),
                plan(prepared.request().chat_command().unwrap(), 100, "one"),
                copy(&proof),
            )
            .await
            .unwrap(),
    );
    let mut database = fixture.database().await;
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut database)
        .await
        .unwrap();
    let header: (Vec<u8>, Vec<u8>, i64, i64, Vec<u8>) = sqlx::query_as(
        "SELECT plan_digest,transaction_id,part_count,byte_count,protected FROM signing_plans",
    )
    .fetch_one(&mut database)
    .await
    .unwrap();
    let part: Vec<u8> = sqlx::query_scalar("SELECT protected FROM signing_plan_parts WHERE part=1")
        .fetch_one(&mut database)
        .await
        .unwrap();
    for change in [
        "UPDATE signing_plans SET plan_digest=zeroblob(32)",
        "UPDATE signing_plans SET transaction_id=zeroblob(32)",
        "UPDATE signing_plans SET part_count=25",
        "UPDATE signing_plans SET byte_count=393217",
        "UPDATE signing_plans SET protected=zeroblob(262145)",
        "UPDATE signing_plan_parts SET protected=zeroblob(262145) WHERE part=1",
        "UPDATE signing_plan_parts SET protected=zeroblob(64) WHERE part=1",
    ] {
        sqlx::query("UPDATE signing_plans SET plan_digest=?,transaction_id=?,part_count=?,byte_count=?,protected=?").bind(&header.0).bind(&header.1).bind(header.2).bind(header.3).bind(&header.4).execute(&mut database).await.unwrap();
        sqlx::query("UPDATE signing_plan_parts SET protected=? WHERE part=1")
            .bind(&part)
            .execute(&mut database)
            .await
            .unwrap();
        sqlx::query(change).execute(&mut database).await.unwrap();
        let error = store
            .signing_plan(
                binding.clone(),
                prepared.identity().operation_id(),
                prepared.identity().command_digest(),
                copy(&proof),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "recovery_required", "{change}: {error:?}");
    }
    database.close().await.unwrap();
    store.close().await.unwrap();
}
