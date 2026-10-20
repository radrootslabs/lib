use super::*;
use futures::{SinkExt, StreamExt};
use radroots_nostr::{
    key::parse_secret_key,
    signing::{LocalSigner, OsNonceSource, gift_wrap::wrap_retained_seal},
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent},
    provider::{ParticipantProvider, ProviderCall},
    request::CancellationSignal,
};
use radroots_storage::command::{
    ArtifactEvidence, DeliveryAttempt, DeliveryAttemptOutcome, DeliveryAttemptRequest,
    DeliveryIntent, DeliveryObservation, DeliveryObservationKind, DeliveryObservationOutcome,
    EnvelopeCandidate, EvidenceCaptureOutcome, InstallationOutcome, InstallationRequest,
    PublicationWork,
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
#[path = "receipt_delivery_admission.rs"]
mod admission_validation;
#[path = "receipt_delivery_faults.rs"]
mod faults;
#[path = "receipt_delivery_validation.rs"]
mod validation;

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
    work: PublicationWork,
    candidate: EnvelopeCandidate,
    config: Config,
}
impl Ready {
    async fn new(urls: &[String], id: u8) -> Self {
        let fixture = Fixture::new();
        let time = now_ms() / 1000;
        fixture.clock.now.store(time, Ordering::SeqCst);
        let (store, input, proof, binding) = fixture.create_with_expiry(time + 600).await;
        let mut value: serde_json::Value = serde_json::from_slice(
            request_until(
                &binding,
                id,
                "one exact native delivery",
                "delivery",
                time + 120,
            )
            .command()
            .protected_wire(),
        )
        .unwrap();
        value["input"]["peer"] = keys(2).public_key().to_hex().into();
        let admitted = AdmissionRequest::from_protected_wire(
            binding.clone(),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let prepared = committed(
            store
                .admit_command_and_hold(admitted, copy(&proof))
                .await
                .unwrap(),
        );
        let plan = plan_with_routes(
            prepared.request().chat_command().unwrap(),
            time,
            "delivery",
            Some(urls),
        );
        let digest = *plan.digest();
        let recipient = prepared.request().chat_command().unwrap().peer();
        bound(
            store
                .bind_signing_intents(binding.clone(), (plan).into(), copy(&proof))
                .await
                .unwrap(),
        );
        let claim = issuance::claim(
            store
                .claim_attempt(
                    issuance::selection(&binding, &prepared, digest, recipient),
                    copy(&proof),
                )
                .await
                .unwrap(),
        );
        let signer = LocalSigner::with_clock(
            parse_secret_key("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
            signer_now,
        )
        .unwrap()
        .with_nonce_source(Arc::new(OsNonceSource));
        let intent = SigningIntent::from_chat_seal(
            claim.binding().operation_id(),
            prepared.identity().command_digest(),
            claim.seal().unwrap(),
            ProviderCall::new(key(), claim.deadline_unix_ms(), CancellationSignal::new()),
        )
        .unwrap();
        let (_, result) = signer.sign_exact(intent).await.into_parts();
        let OutcomeKind::Obtained(artifact) = result else {
            panic!("synthetic participant seal must be obtained")
        };
        let seal = artifact.with_json(|raw| {
            ArtifactEvidence::verify(&claim, raw, artifact.observed_at_unix_ms()).unwrap()
        });
        assert!(matches!(
            store
                .capture_evidence(input.clone(), &proof, &seal)
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
            InstallationOutcome::Committed(_)
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
        let work = snapshot.publications()[0];
        let pending = wrap_retained_seal(
            seal.raw_json(),
            recipient,
            NonZeroU64::new(time).unwrap(),
            &OsNonceSource,
        )
        .unwrap();
        let candidate = pending.with_wrapping_key(|key| {
            EnvelopeCandidate::from_obtained(work, pending.signed_json(), key).unwrap()
        });
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
            candidate,
            config,
        }
    }
    fn request(&self, id: u8) -> DeliveryAttemptRequest {
        let targets = TargetSet::new(
            self.config
                .relays()
                .iter()
                .map(|relay| relay.to_target().unwrap())
                .collect(),
        )
        .unwrap();
        let intent = DeliveryIntent::new(
            &format!("{id:02x}").repeat(32),
            self.candidate.event_id(),
            &targets,
            &SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            NonZeroU64::new(self.work.queued_at().get() * 1000 + 60_000).unwrap(),
        )
        .unwrap();
        DeliveryAttemptRequest::new(self.binding.clone(), self.work, intent).unwrap()
    }
    async fn retain(&self, id: u8) -> DeliveryAttempt {
        match self
            .store
            .prepare_delivery_attempt(self.request(id), copy(&self.proof), Some(&self.candidate))
            .await
            .unwrap()
        {
            DeliveryAttemptOutcome::Claimed(value) => *value,
            _ => panic!("new actual retained delivery expected"),
        }
    }
    async fn query(
        &self,
        store: &NativeWorkspace,
        id: &radroots_transport::sink::DeliveryRequestId,
    ) -> DeliveryAttempt {
        let e = self.work.evidence();
        store
            .delivery_attempt(
                self.binding.clone(),
                e.operation_id(),
                e.command_digest(),
                e.recipient().unwrap(),
                id.clone(),
                copy(&self.proof),
            )
            .await
            .unwrap()
            .unwrap()
    }
    async fn observations(
        &self,
        store: &NativeWorkspace,
        id: &radroots_transport::sink::DeliveryRequestId,
    ) -> Vec<DeliveryObservation> {
        let e = self.work.evidence();
        store
            .delivery_observations(
                self.binding.clone(),
                e.operation_id(),
                e.command_digest(),
                e.recipient().unwrap(),
                id.clone(),
                copy(&self.proof),
            )
            .await
            .unwrap()
    }
}
fn captured(outcome: DeliveryObservationOutcome) -> DeliveryObservation {
    match outcome {
        DeliveryObservationOutcome::Committed(value) => value,
        _ => panic!("confirmed original observation expected"),
    }
}
#[tokio::test]
async fn native_delivery_retains_exact_outer_before_loopback_and_reopen_preserves_original_observation()
 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ready = Ready::new(&[format!("ws://{}", listener.local_addr().unwrap())], 230).await;
    let request = ready.request(1);
    let original_intent = request.intent().clone();
    let DeliveryAttemptOutcome::Claimed(attempt) = ready
        .store
        .prepare_delivery_attempt(request, copy(&ready.proof), Some(&ready.candidate))
        .await
        .unwrap()
    else {
        panic!("new retention required")
    };
    let replay = ready
        .store
        .prepare_delivery_attempt(
            DeliveryAttemptRequest::new(ready.binding.clone(), ready.work, original_intent)
                .unwrap(),
            copy(&ready.proof),
            None,
        )
        .await
        .unwrap();
    assert!(matches!(replay, DeliveryAttemptOutcome::Retained(_)));
    let expected = format!("[\"EVENT\",{}]", attempt.raw_json());
    let e = ready.work.evidence();
    let pending = ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            e.operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.delivery_attempts().len(), 1);
    assert!(pending.delivery_observations().is_empty());
    let projected = radroots_execution::OperationReceipt::from_snapshot(&pending);
    assert_eq!(projected.delivery(), radroots_execution::Delivery::Unknown);
    assert_eq!(
        projected.delivery_attempts()[0].envelope_digest(),
        attempt.envelope_digest()
    );
    let expected_message = pending.installed_message().unwrap().id();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        let raw = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Message::Text(raw) = socket.next().await.unwrap().unwrap() {
                    break raw.to_string();
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(raw, expected);
        let event: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let outer = nostr::Event::from_json(event[1].to_string()).unwrap();
        let recovered = nostr::nips::nip59::extract_rumor(&keys(2), &outer)
            .await
            .unwrap();
        assert_eq!(recovered.sender, keys(1).public_key());
        assert_eq!(
            recovered.rumor.id.unwrap().as_bytes(),
            expected_message.as_bytes()
        );
        assert_eq!(recovered.rumor.content, "one exact native delivery");
        socket
            .send(Message::Text(
                json!(["OK", "ff".repeat(32), true, ""]).to_string().into(),
            ))
            .await
            .unwrap();
        socket
            .send(Message::Text(
                json!(["OK", event[1]["id"], true, ""]).to_string().into(),
            ))
            .await
            .unwrap();
    });
    let transport = radroots_transport_nostr_native::create_transport(ready.config.clone());
    let request = attempt.transport_request().unwrap();
    let receipt = transport.deliver(request.clone()).await.unwrap();
    receipt.validate_for_request(&request).unwrap();
    assert!(receipt.is_satisfied(&request).unwrap());
    server.await.unwrap();
    let target = request.target_set().targets()[0].fingerprint();
    let observed = DeliveryObservation::from_receipt(&attempt, &receipt, target, None).unwrap();
    assert_eq!(observed.kind(), DeliveryObservationKind::Accepted);
    let first = captured(
        ready
            .store
            .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observed)
            .await
            .unwrap(),
    );
    assert_eq!(first.observed_at_unix_ms(), None);
    let complete = ready
        .store
        .operation_snapshot(
            ready.binding.clone(),
            e.operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_ne!(complete.checkpoint_digest(), pending.checkpoint_digest());
    let projected = radroots_execution::OperationReceipt::from_snapshot(&complete);
    assert_eq!(
        projected.business_installation(),
        radroots_execution::BusinessInstallation::Installed
    );
    assert_eq!(projected.delivery(), radroots_execution::Delivery::Observed);
    assert!(
        !projected
            .recovery()
            .contains(&radroots_execution::RecoveryRequirement::ResumeDelivery)
    );
    assert_eq!(projected.delivery_observations().len(), 1);
    assert_eq!(
        projected.delivery_observations()[0].event_id(),
        attempt.intent().event_id()
    );
    assert_eq!(
        projected.delivery_observations()[0].request_id(),
        attempt.intent().request_id().as_str()
    );
    assert_eq!(projected.delivery_observations()[0].target_ordinal(), 1);
    assert_eq!(
        projected.delivery_observations()[0].outcome(),
        radroots_execution::TargetObservation::Accepted
    );
    let later =
        DeliveryObservation::from_receipt(&attempt, &receipt, target, NonZeroU64::new(now_ms()))
            .unwrap();
    assert_eq!(
        captured(
            ready
                .store
                .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), later)
                .await
                .unwrap()
        ),
        first
    );
    ready.store.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let retained = ready.query(&reopened, attempt.intent().request_id()).await;
    assert_eq!(retained.raw_json(), ready.candidate.raw_json());
    assert_eq!(retained.transaction_id(), attempt.transaction_id());
    assert_eq!(retained.target_ordinals(), &[1]);
    assert_eq!(
        ready
            .observations(&reopened, attempt.intent().request_id())
            .await,
        vec![first]
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn predecessor_schema_six_preserves_original_installation_and_seal_without_automatic_upgrade()
{
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 236).await;
    ready.store.close().await.unwrap();
    let mut database = ready.fixture.database().await;
    sqlx::raw_sql("DROP TABLE chat_delivery_observations; DROP TABLE chat_delivery_parts; DROP TABLE chat_delivery_attempts; PRAGMA user_version=6").execute(&mut database).await.unwrap();
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM sqlite_schema),(SELECT count(*) FROM chat_messages),(SELECT count(*) FROM artifact_evidence)").fetch_one(&mut database).await.unwrap();
    assert_eq!(counts, (16, 1, 1));
    database.close().await.unwrap();
    let path = ready.fixture.path.join("execution_v1.sqlite");
    let original = std::fs::read(&path).unwrap();
    assert!(matches!(
        NativeWorkspace::open_existing(
            &ready.fixture.path,
            ready.input,
            ready.fixture.crypto(),
            ready.fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::RecoveryRequired)
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[tokio::test]
async fn native_retry_reuses_exact_bytes_and_enforces_twelve_potential_attempts_per_endpoint() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 231).await;
    let first = ready.retain(1).await;
    for ordinal in 2..=12u8 {
        let DeliveryAttemptOutcome::Claimed(attempt) = ready
            .store
            .prepare_delivery_attempt(ready.request(ordinal), copy(&ready.proof), None)
            .await
            .unwrap()
        else {
            panic!("fresh bounded retry expected")
        };
        assert_eq!(attempt.target_ordinals(), &[ordinal]);
        assert_eq!(attempt.raw_json(), first.raw_json());
        assert_eq!(attempt.envelope_digest(), first.envelope_digest());
        assert_ne!(attempt.transaction_id(), first.transaction_id());
    }
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(13), copy(&ready.proof), None)
            .await,
        Err(OperationError::DeliveryAttemptLimit)
    ));
    let original = ready.query(&ready.store, first.intent().request_id()).await;
    assert_eq!(original.raw_json(), first.raw_json());
    assert_eq!(original.target_ordinals(), &[1]);
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_delivery_part_failure_rolls_back_attempt_and_keeps_original_installation() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 232).await;
    let mut database = ready.fixture.database().await;
    sqlx::raw_sql("CREATE TRIGGER refuse_delivery_part BEFORE INSERT ON chat_delivery_parts BEGIN SELECT RAISE(ABORT,'synthetic part failure'); END").execute(&mut database).await.unwrap();
    assert!(matches!(
        ready
            .store
            .prepare_delivery_attempt(ready.request(1), copy(&ready.proof), Some(&ready.candidate))
            .await,
        Err(OperationError::StorageFailure)
    ));
    let counts:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM chat_delivery_attempts),(SELECT count(*) FROM chat_delivery_parts),(SELECT count(*) FROM chat_messages),(SELECT count(*) FROM artifact_evidence)").fetch_one(&mut database).await.unwrap();
    assert_eq!(counts, (0, 0, 1, 1));
    sqlx::query("DROP TRIGGER refuse_delivery_part")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    assert_eq!(ready.retain(1).await.target_ordinals(), &[1]);
    ready.store.close().await.unwrap();
}

#[tokio::test]
async fn actual_delivery_commit_uncertainty_latches_and_reopen_preserves_business_facts() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 233).await;
    let request = ready.request(1);
    let id = request.intent().request_id().clone();
    let mut database = ready.fixture.database().await;
    sqlx::raw_sql("CREATE TABLE delivery_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER delivery_commit_fault_trigger AFTER INSERT ON chat_delivery_attempts BEGIN INSERT INTO delivery_commit_fault VALUES(2); END").execute(&mut database).await.unwrap();
    database.close().await.unwrap();
    let DeliveryAttemptOutcome::Unknown { operation, attempt } = ready
        .store
        .prepare_delivery_attempt(request, copy(&ready.proof), Some(&ready.candidate))
        .await
        .unwrap()
    else {
        panic!("actual commit failure must remain unknown")
    };
    assert_eq!(attempt, id);
    assert_eq!(
        operation.operation_id(),
        ready.work.evidence().operation_id()
    );
    assert!(operation.transaction_id().is_some());
    let e = ready.work.evidence();
    assert!(matches!(
        ready
            .store
            .delivery_attempt(
                ready.binding.clone(),
                e.operation_id(),
                e.command_digest(),
                e.recipient().unwrap(),
                id.clone(),
                copy(&ready.proof)
            )
            .await,
        Err(OperationError::RecoveryRequired)
    ));
    ready.store.close().await.unwrap();
    let mut database = ready.fixture.database().await;
    sqlx::raw_sql("DROP TRIGGER delivery_commit_fault_trigger; DROP TABLE delivery_commit_fault")
        .execute(&mut database)
        .await
        .unwrap();
    database.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    assert!(
        reopened
            .delivery_attempt(
                ready.binding.clone(),
                e.operation_id(),
                e.command_digest(),
                e.recipient().unwrap(),
                id,
                copy(&ready.proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    let snapshot = reopened
        .operation_snapshot(
            ready.binding.clone(),
            e.operation_id(),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.installed_message().is_some());
    assert_eq!(snapshot.publications(), &[ready.work]);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn admitted_delivery_survives_lost_observer_and_close_drains_original_attempt() {
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 234).await;
    let request = ready.request(1);
    let id = request.intent().request_id().clone();
    let (arrived, resume) = ready.fixture.clock.arm(1);
    let owner = ready.store.clone();
    let proof = copy(&ready.proof);
    let candidate = ready.candidate.retained_copy();
    let observer = tokio::spawn(async move {
        owner
            .prepare_delivery_attempt(request, proof, Some(&candidate))
            .await
    });
    reached(arrived).await;
    observer.abort();
    assert!(observer.await.unwrap_err().is_cancelled());
    resume.send(()).unwrap();
    ready.store.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    assert_eq!(
        ready.query(&reopened, &id).await.raw_json(),
        ready.candidate.raw_json()
    );
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn late_observation_append_requires_original_credential_but_no_fresh_clock_or_dispatch_grant()
{
    let ready = Ready::new(&[String::from("ws://127.0.0.1:9")], 235).await;
    let attempt = ready.retain(1).await;
    let request = attempt.transport_request().unwrap();
    let failure = radroots_transport::SinkFailure::transport_closed(&request);
    let observation = DeliveryObservation::from_failure(
        &attempt,
        &failure,
        request.target_set().targets()[0].fingerprint(),
        None,
    )
    .unwrap();
    assert_eq!(observation.kind(), DeliveryObservationKind::NotAttempted);
    ready
        .store
        .revoke(ready.input.clone(), &ready.proof)
        .await
        .unwrap();
    ready.fixture.clock.now.store(0, Ordering::SeqCst);
    let wrong = AttachmentProof::from_protected_input(&[0x33; 32]).unwrap();
    assert!(
        ready
            .store
            .record_delivery_observation(ready.binding.clone(), wrong, observation.clone())
            .await
            .is_err()
    );
    let original = captured(
        ready
            .store
            .record_delivery_observation(ready.binding.clone(), copy(&ready.proof), observation)
            .await
            .unwrap(),
    );
    assert_eq!(original.observed_at_unix_ms(), None);
    ready.store.close().await.unwrap();
    // Reopening validates the protected schema, but does not renew revoked access.
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    assert!(
        reopened
            .authenticate(ready.input.clone(), &ready.proof)
            .await
            .is_err()
    );
    reopened.close().await.unwrap();
}
