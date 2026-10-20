//! Actual native public D over the existing journal and qualified Nostr sink.
use super::*;
use futures::{SinkExt, StreamExt};
use nostr::JsonUtil;
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource},
};
use radroots_protocol::{listing::PublicSigningPlan, plan::SigningReplay};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{ParticipantProvider, ProviderCall},
    request::CancellationSignal,
};
use radroots_storage::command::{
    ArtifactEvidence, ClaimAttemptOutcome, ClaimAttemptRequest, DeliveryAttempt,
    DeliveryAttemptOutcome, DeliveryAttemptRequest, DeliveryIntent, DeliveryObservation,
    DeliveryObservationKind, DeliveryObservationOutcome, EvidenceCaptureOutcome,
    InstallationOutcome, InstallationRequest, PlanBindingOutcome, PublicPublicationWork,
    PublicationArtifact, SigningTarget,
};
use radroots_transport::{
    EventSink, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use std::{
    num::NonZeroU64,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}
fn signer_now() -> Result<u64, radroots_signing::Error> {
    Ok(now_ms())
}
struct Ready {
    fixture: Fixture,
    store: Arc<NativeWorkspace>,
    input: AttachmentInput,
    proof: AttachmentProof,
    binding: WorkspaceBinding,
    work: PublicPublicationWork,
    raw: String,
    config: Config,
}
impl Ready {
    async fn new(urls: &[String], id: u8) -> Self {
        let fixture = Fixture::new();
        let time = now_ms() / 1000;
        fixture.clock.now.store(time, Ordering::SeqCst);
        let (store, input, proof) = NativeWorkspace::initialize_local_outbox(
            &fixture.path,
            LocalOutboxConfig::seller(
                key(),
                key(),
                "public-delivery",
                UnixSeconds::new(time + 600).unwrap(),
            )
            .unwrap(),
            fixture.crypto(),
            fixture.clock.clone(),
        )
        .await
        .unwrap();
        let binding = store.authenticate(input.clone(), &proof).await.unwrap();
        let store = Arc::new(store);
        let context = CommandContextData::new(
            OperationId::from_bytes([id; 32]),
            binding.actor(),
            input.workspace_id(),
            input.authority(),
            input.route_digest(),
            None,
            None,
            UnixSeconds::new(time + 120).unwrap(),
            None,
        )
        .unwrap();
        let wire = json!({"type":"listing.publish","context":context,"input":{"id":format!("{id:02x}").repeat(16),"title":"Native public delivery","summary":"Quote required","description_markdown":"Public terms","published_at":time.to_string(),"profile":{"profile":"radroots.listing","version":1,"state":"active","product":{"name":"Carrots","description":"x".repeat(4096),"producer":null,"origin_summary":null},"default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"Bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},"increment":"1","minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],"fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}}});
        let prepared = committed(
            store
                .admit_command_and_hold(
                    AdmissionRequest::from_protected_wire(
                        binding.clone(),
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap(),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        let plan = PublicSigningPlan::capture(
            prepared.request().command().listing().unwrap(),
            UnixSeconds::new(time).unwrap(),
            urls,
            SigningReplay::NonReplayable,
        )
        .unwrap();
        let digest = *plan.digest();
        assert!(
            plan.event()
                .fields()
                .tags
                .iter()
                .any(|tag| tag.iter().any(|field| field.len() > 4096))
        );
        let PlanBindingOutcome::Committed(_) = store
            .bind_signing_intents(binding.clone(), plan.into(), copy(&proof))
            .await
            .unwrap()
        else {
            panic!("actual T1 commit")
        };
        let ClaimAttemptOutcome::Claimed(attempt) = store
            .claim_attempt(
                ClaimAttemptRequest::for_artifact(
                    binding.clone(),
                    prepared.identity().operation_id(),
                    prepared.identity().command_digest(),
                    digest,
                    SigningTarget::PublicEvent,
                ),
                copy(&proof),
            )
            .await
            .unwrap()
        else {
            panic!("actual first claim")
        };
        let signer = LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            signer_now,
        )
        .unwrap()
        .with_nonce_source(Arc::new(OsNonceSource));
        let intent = SigningIntent::from_plan(
            attempt.bound_plan().plan(),
            SigningTarget::PublicEvent,
            ProviderCall::new(key(), attempt.deadline_unix_ms(), CancellationSignal::new()),
        )
        .unwrap();
        let (_, outcome) = signer.sign_exact(intent).await.into_parts();
        let OutcomeKind::Obtained(artifact) = outcome else {
            panic!("actual public signature")
        };
        let evidence = artifact.with_json(|raw| {
            nostr::Event::from_json(raw).unwrap().verify().unwrap();
            ArtifactEvidence::verify(&attempt, raw, artifact.observed_at_unix_ms()).unwrap()
        });
        let raw = evidence.raw_json().to_owned();
        assert!(matches!(
            store
                .capture_evidence(input.clone(), &proof, &evidence)
                .await
                .unwrap(),
            EvidenceCaptureOutcome::Committed(_)
        ));
        assert!(matches!(
            store
                .install_transition_and_outbox(
                    InstallationRequest::new(
                        binding.clone(),
                        prepared.identity().operation_id(),
                        prepared.identity().command_digest(),
                        digest
                    ),
                    copy(&proof)
                )
                .await
                .unwrap(),
            InstallationOutcome::ListingCommitted(_)
        ));
        let snapshot = store
            .operation_snapshot(
                binding.clone(),
                prepared.identity().operation_id(),
                None,
                copy(&proof),
            )
            .await
            .unwrap()
            .unwrap();
        let work = snapshot.public_publications()[0];
        let config = Config::from_profile(
            RelayProfile::explicit(
                RelayProfileKind::Simulator,
                urls.iter().map(|url| {
                    RelayEndpoint::new(url, RelayUrlPolicy::Local, RelayAccess::ReadWrite).unwrap()
                }),
            )
            .unwrap(),
        )
        .with_timeouts(1000, 1000, 500)
        .unwrap();
        Self {
            fixture,
            store,
            input,
            proof,
            binding,
            work,
            raw,
            config,
        }
    }
    fn targets(&self) -> TargetSet {
        TargetSet::new(
            self.config
                .relays()
                .iter()
                .map(|v| v.to_target().unwrap())
                .collect(),
        )
        .unwrap()
    }
    fn request(&self, id: u8) -> DeliveryAttemptRequest {
        let now = self.fixture.clock.now().unwrap().get() * 1000;
        DeliveryAttemptRequest::new(
            self.binding.clone(),
            self.work,
            DeliveryIntent::public(
                &format!("{id:02x}").repeat(32),
                self.work.evidence().event_id(),
                &self.targets(),
                &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
                NonZeroU64::new(now + 60_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    async fn retain(&self, id: u8) -> DeliveryAttempt {
        match self
            .store
            .prepare_delivery_attempt(self.request(id), copy(&self.proof), None)
            .await
            .unwrap()
        {
            DeliveryAttemptOutcome::Claimed(v) => *v,
            _ => panic!("actual fresh public D claim"),
        }
    }
    async fn snapshot(
        &self,
        store: &NativeWorkspace,
    ) -> radroots_storage::command::OperationSnapshot {
        store
            .operation_snapshot(
                self.binding.clone(),
                self.work.evidence().operation_id(),
                Some(self.work.evidence().command_digest()),
                copy(&self.proof),
            )
            .await
            .unwrap()
            .unwrap()
    }
    async fn query(
        &self,
        store: &NativeWorkspace,
        id: &radroots_transport::sink::DeliveryRequestId,
    ) -> Option<DeliveryAttempt> {
        store
            .delivery_attempt(
                self.binding.clone(),
                self.work.evidence().operation_id(),
                self.work.evidence().command_digest(),
                SigningTarget::PublicEvent,
                id.clone(),
                copy(&self.proof),
            )
            .await
            .unwrap()
    }
}
#[tokio::test]
async fn public_d_retains_original_before_actual_loopback_and_reopens_independent_ack() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ready = Ready::new(&[format!("ws://{}", listener.local_addr().unwrap())], 180).await;
    let attempt = ready.retain(1).await;
    assert_eq!(attempt.raw_json(), ready.raw);
    assert_eq!(attempt.work(), PublicationArtifact::Public(ready.work));
    assert_eq!(attempt.work().recipient(), None);
    assert_eq!(
        attempt.intent().event_id(),
        ready.work.evidence().event_id()
    );
    let snapshot = ready.snapshot(&ready.store).await;
    let checkpoint = snapshot.checkpoint_digest();
    assert!(snapshot.installed_message().is_none());
    assert!(snapshot.publications().is_empty());
    assert_eq!(snapshot.public_publications(), &[ready.work]);
    assert_eq!(snapshot.delivery_attempts().len(), 1);
    assert!(!snapshot.publication_accepted(ready.work).unwrap());
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), None)
            .await
            .unwrap(),
        DeliveryAttemptOutcome::Retained(_)
    ));
    let expected = format!("[\"EVENT\",{}]", ready.raw);
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        let raw = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                    break raw;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(raw.as_str(), expected);
        let frame: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let event = nostr::Event::from_json(frame[1].to_string()).unwrap();
        event.verify().unwrap();
        assert_eq!(event.kind.as_u16(), 30402);
        socket
            .send(Message::Text(
                json!(["OK", "ff".repeat(32), true, ""]).to_string().into(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                json!(["OK", frame[1]["id"], true, ""]).to_string().into(),
            ))
            .await
            .unwrap();
    });
    let transport = radroots_transport_nostr_native::create_transport(ready.config.clone());
    let request = attempt.transport_request().unwrap();
    let selected = attempt.intent().selected_target_set().unwrap();
    let receipt = transport
        .deliver_selected(request.clone(), selected)
        .await
        .unwrap();
    receipt.validate_for_request(&request).unwrap();
    assert!(receipt.is_satisfied(&request).unwrap());
    server.await.unwrap();
    let observation = DeliveryObservation::from_receipt(
        &attempt,
        &receipt,
        request.target_set().targets()[0].fingerprint(),
        None,
    )
    .unwrap();
    assert_eq!(observation.kind(), DeliveryObservationKind::Accepted);
    let DeliveryObservationOutcome::Committed(first) = ready
        .store
        .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
        .await
        .unwrap()
    else {
        panic!("actual durable ACK")
    };
    let complete = ready.snapshot(&ready.store).await;
    assert_ne!(complete.checkpoint_digest(), checkpoint);
    assert!(complete.publication_accepted(ready.work).unwrap());
    assert_eq!(
        complete.listing_installation(),
        snapshot.listing_installation()
    );
    transport.shutdown().await;
    ready.store.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let retained = ready
        .query(&reopened, attempt.intent().request_id())
        .await
        .unwrap();
    assert_eq!(retained.raw_json(), ready.raw);
    assert_eq!(retained.transaction_id(), attempt.transaction_id());
    let observations = reopened
        .delivery_observations(
            ready.binding.clone(),
            ready.work.evidence().operation_id(),
            ready.work.evidence().command_digest(),
            SigningTarget::PublicEvent,
            attempt.intent().request_id().clone(),
            copy(&ready.proof),
        )
        .await
        .unwrap();
    assert_eq!(observations, vec![first]);
    assert!(
        ready
            .snapshot(&reopened)
            .await
            .publication_accepted(ready.work)
            .unwrap()
    );
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn public_d_requires_original_targets_role_and_current_proof_before_any_claim() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 181).await;
    assert!(
        DeliveryAttemptRequest::new(
            ready.binding.clone(),
            ready.work,
            DeliveryIntent::new(
                &"11".repeat(32),
                ready.work.evidence().event_id(),
                &ready.targets(),
                &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
                NonZeroU64::new(now_ms() + 60_000).unwrap()
            )
            .unwrap()
        )
        .is_err()
    );
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(
                ready.request(1),
                AttachmentProof::from_protected_input(&[0; 32]).unwrap(),
                None
            )
            .await,
        Err(OperationError::Workspace(WorkspaceError::Unauthorized))
    ));
    let other = TargetSet::new(vec![
        radroots_transport::Target::nostr_relay("ws://127.0.0.1:10").unwrap(),
    ])
    .unwrap();
    let request = DeliveryAttemptRequest::new(
        ready.binding.clone(),
        ready.work,
        DeliveryIntent::public(
            &"11".repeat(32),
            ready.work.evidence().event_id(),
            &other,
            &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            NonZeroU64::new(now_ms() + 60_000).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(request, copy(&ready.proof), None)
            .await,
        Err(OperationError::UnverifiedApproval)
    ));
    assert!(
        ready
            .snapshot(&ready.store)
            .await
            .delivery_attempts()
            .is_empty()
    );
    ready.store.close().await.unwrap();
}
#[tokio::test]
async fn public_retries_reuse_original_after_first_issue_expiry_and_spend_twelve_endpoint_slots() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 182).await;
    ready.fixture.clock.now.fetch_add(121, Ordering::SeqCst);
    for id in 1..=12 {
        let attempt = ready.retain(id).await;
        assert_eq!(attempt.raw_json(), ready.raw);
        assert_eq!(attempt.target_ordinals(), &[id]);
    }
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(13), copy(&ready.proof), None)
            .await,
        Err(OperationError::DeliveryAttemptLimit)
    ));
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), None)
            .await
            .unwrap(),
        DeliveryAttemptOutcome::Retained(_)
    ));
    let state = ready.snapshot(&ready.store).await;
    assert_eq!(state.delivery_attempts().len(), 12);
    assert!(state.delivery_observations().is_empty());
    assert!(!state.publication_accepted(ready.work).unwrap());
    ready.store.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let state = ready.snapshot(&reopened).await;
    assert_eq!(state.delivery_attempts().len(), 12);
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn public_d_part_insert_failure_rolls_back_without_losing_installed_revision() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 183).await;
    let mut db = ready.fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER fail_public_d BEFORE INSERT ON artifact_delivery_parts BEGIN SELECT RAISE(ABORT,'isolated fixture'); END").execute(&mut db).await.unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), None)
            .await,
        Err(OperationError::StorageFailure)
    ));
    let state = ready.snapshot(&ready.store).await;
    assert!(state.listing_installation().is_some());
    assert!(state.delivery_attempts().is_empty());
    sqlx::query("DROP TRIGGER fail_public_d")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let retained = ready.retain(1).await;
    assert_eq!(retained.raw_json(), ready.raw);
    ready.store.close().await.unwrap();
}
#[tokio::test]
async fn lost_public_d_observer_never_turns_reopen_into_dispatch_permission() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 184).await;
    let (arrived, resume) = ready.fixture.clock.arm(1);
    let mut pending =
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), None);
    assert!(futures::poll!(pending.as_mut()).is_pending());
    reached(arrived).await;
    drop(pending);
    resume.send(()).unwrap();
    ready.store.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    assert!(matches!(
        reopened
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), None)
            .await
            .unwrap(),
        DeliveryAttemptOutcome::Retained(_)
    ));
    assert_eq!(ready.snapshot(&reopened).await.delivery_attempts().len(), 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn actual_public_rejection_disconnect_and_missing_matching_ack_remain_independent() {
    for (mode, expected) in [
        (0, DeliveryObservationKind::Rejected),
        (1, DeliveryObservationKind::Disconnected),
        (2, DeliveryObservationKind::TimedOut),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ready = Ready::new(
            &[format!("ws://{}", listener.local_addr().unwrap())],
            185 + mode,
        )
        .await;
        let deadline = NonZeroU64::new(now_ms() + 2_000).unwrap();
        let request = DeliveryAttemptRequest::new(
            ready.binding.clone(),
            ready.work,
            DeliveryIntent::public(
                &format!("{mode:02x}").repeat(32),
                ready.work.evidence().event_id(),
                &ready.targets(),
                &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
                deadline,
            )
            .unwrap(),
        )
        .unwrap();
        let DeliveryAttemptOutcome::Claimed(attempt) = ready
            .store
            .prepare_delivery_attempt(request, copy(&ready.proof), None)
            .await
            .unwrap()
        else {
            panic!("actual public D")
        };
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            let raw = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                        break raw;
                    }
                }
            })
            .await
            .unwrap();
            let event: serde_json::Value = serde_json::from_str(&raw).unwrap();
            match mode {
                0 => socket
                    .send(Message::Text(
                        json!(["OK", event[1]["id"], false, "blocked: denied"])
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap(),
                1 => {
                    socket.close(None).await.unwrap();
                }
                _ => socket
                    .send(Message::Text(
                        json!(["OK", "ff".repeat(32), true, ""]).to_string().into(),
                    ))
                    .await
                    .unwrap(),
            }
            let _ = held.await;
        });
        let transport = radroots_transport_nostr_native::create_transport(ready.config.clone());
        let request = attempt.transport_request().unwrap();
        let target = request.target_set().targets()[0].fingerprint();
        let result = transport
            .deliver_selected(
                request.clone(),
                attempt.intent().selected_target_set().unwrap(),
            )
            .await;
        let observation = match result {
            Ok(receipt) => {
                assert!(!receipt.is_satisfied(&request).unwrap());
                DeliveryObservation::from_receipt(&attempt, &receipt, target, None).unwrap()
            }
            Err(failure) => {
                DeliveryObservation::from_failure(&attempt, &failure, target, None).unwrap()
            }
        };
        assert_eq!(observation.kind(), expected);
        // Already obtained original target evidence remains capturable while a
        // fresh clock/access check would refuse another operation.
        ready.fixture.clock.now.store(0, Ordering::SeqCst);
        assert!(matches!(
            ready
                .store
                .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
                .await
                .unwrap(),
            DeliveryObservationOutcome::Committed(_)
        ));
        ready
            .fixture
            .clock
            .now
            .store(now_ms() / 1000, Ordering::SeqCst);
        assert!(
            !ready
                .snapshot(&ready.store)
                .await
                .publication_accepted(ready.work)
                .unwrap()
        );
        let _ = release.send(());
        server.await.unwrap();
        transport.shutdown().await;
        ready.store.close().await.unwrap();
    }
}
#[tokio::test]
async fn unknown_public_d_commit_requires_safe_reopen_and_never_reports_a_dispatch_right() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 188).await;
    let mut db = ready.fixture.database().await;
    sqlx::raw_sql("CREATE TABLE deferred_d_fault(value INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_public_d_commit AFTER INSERT ON artifact_delivery_attempts BEGIN INSERT INTO deferred_d_fault VALUES(2); END").execute(&mut db).await.unwrap();
    let request = ready.request(1);
    let id = request.intent().request_id().clone();
    let DeliveryAttemptOutcome::Unknown { operation, attempt } = ready
        .store
        .prepare_delivery_attempt(request, copy(&ready.proof), None)
        .await
        .unwrap()
    else {
        panic!("actual failed COMMIT must be unknown")
    };
    assert_eq!(attempt, id);
    assert_eq!(
        operation.operation_id(),
        ready.work.evidence().operation_id()
    );
    assert!(operation.transaction_id().is_some());
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(2), copy(&ready.proof), None)
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    ready.store.close().await.unwrap();
    sqlx::raw_sql("DROP TRIGGER fail_public_d_commit; DROP TABLE deferred_d_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    assert!(ready.query(&reopened, &id).await.is_none());
    let state = ready.snapshot(&reopened).await;
    assert!(state.delivery_attempts().is_empty());
    assert!(state.listing_installation().is_some());
    assert!(!state.publication_accepted(ready.work).unwrap());
    reopened.close().await.unwrap();
}
#[tokio::test]
async fn current_public_scope_and_original_policy_cannot_be_replaced_by_a_new_attempt() {
    let ready = Ready::new(&["ws://127.0.0.1:9".into()], 189).await;
    let original = ready.retain(1).await;
    let changed = DeliveryAttemptRequest::new(
        ready.binding.clone(),
        ready.work,
        DeliveryIntent::public(
            &"02".repeat(32),
            ready.work.evidence().event_id(),
            &ready.targets(),
            &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            NonZeroU64::new(now_ms() + 60_000).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(changed, copy(&ready.proof), None)
            .await,
        Err(OperationError::IdempotencyConflict)
    ));
    ready
        .store
        .revoke(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(2), copy(&ready.proof), None)
            .await,
        Err(OperationError::Workspace(WorkspaceError::GrantRevoked))
    ));
    assert_eq!(original.raw_json(), ready.raw);
    ready.store.close().await.unwrap();
}
