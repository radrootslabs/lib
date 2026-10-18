use super::*;
use radroots_execution::{Admission, Code, session::SessionState};
use radroots_signing::{
    issuance::{SigningIntent, SigningOutcome},
    provider::{PrivateCiphertext, PrivateText, ProviderCall, ProviderState},
};

type SigningPause = (oneshot::Sender<()>, oneshot::Receiver<()>);
#[derive(Clone, Copy, Default)]
enum CryptoFault {
    #[default]
    None,
    EncryptionUnavailable,
    DecryptionUnavailable,
    WrongPlaintext,
}
struct RecordedSigner {
    local: LocalSigner,
    calls: AtomicU64,
    uncertain: Option<PublicKey>,
    pause: Mutex<Option<SigningPause>>,
    crypto_fault: Mutex<CryptoFault>,
    crypto_calls: AtomicU64,
}
impl RecordedSigner {
    fn new(uncertain: Option<PublicKey>) -> Arc<Self> {
        Arc::new(Self {
            local: LocalSigner::new(
                parse_secret_key(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .unwrap(),
            )
            .unwrap()
            .with_nonce_source(Arc::new(OsNonceSource)),
            calls: AtomicU64::new(0),
            uncertain,
            pause: Mutex::new(None),
            crypto_fault: Mutex::new(CryptoFault::None),
            crypto_calls: AtomicU64::new(0),
        })
    }
}
impl Signer for RecordedSigner {
    fn status(
        &self,
    ) -> BoxFuture<'_, Result<radroots_signing::SignerStatus, radroots_signing::Error>> {
        self.local.status()
    }
    fn sign(
        &self,
        request: radroots_signing::SignRequest,
    ) -> BoxFuture<'_, Result<radroots_signing::SignReceipt, radroots_signing::Error>> {
        self.local.sign(request)
    }
}
impl ParticipantProvider for RecordedSigner {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            use radroots_signing::provider::PrivateCryptoCapability::Unavailable;
            let state = self.local.provider_state().await?;
            let fault = *self.crypto_fault.lock().unwrap();
            let (encryption, decryption) = match fault {
                CryptoFault::EncryptionUnavailable => (Unavailable, state.decryption()),
                CryptoFault::DecryptionUnavailable => (state.encryption(), Unavailable),
                CryptoFault::None | CryptoFault::WrongPlaintext => return Ok(state),
            };
            Ok(ProviderState::new(
                state.identity(),
                encryption,
                decryption,
                state.signing_replay(),
            )
            .with_exact_issuance(state.exact_issuance()))
        })
    }
    fn encrypt_private(
        &self,
        call: ProviderCall,
        recipient: PublicKey,
        text: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        self.crypto_calls.fetch_add(1, Ordering::SeqCst);
        self.local.encrypt_private(call, recipient, text)
    }
    fn decrypt_private(
        &self,
        call: ProviderCall,
        sender: PublicKey,
        text: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        self.crypto_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let actual = self.local.decrypt_private(call, sender, text).await?;
            if matches!(
                *self.crypto_fault.lock().unwrap(),
                CryptoFault::WrongPlaintext
            ) {
                PrivateText::new("injected post-decryption mismatch")
            } else {
                Ok(actual)
            }
        })
    }
    fn sign_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if intent.recipient() == self.uncertain {
                return SigningOutcome::unknown(intent.binding(), ProviderError::ProviderFailure);
            }
            let outcome = self.local.sign_exact(intent).await;
            let pause = self.pause.lock().unwrap().take();
            if let Some((reached, resume)) = pause {
                reached.send(()).unwrap();
                resume.await.unwrap();
            }
            outcome
        })
    }
}
struct Ready {
    fixture: Fixture,
    store: Arc<NativeWorkspace>,
    client: Client,
    scope: radroots::WorkspaceSession,
    input: AttachmentInput,
    proof: AttachmentProof,
    signer: Arc<RecordedSigner>,
}
impl Ready {
    async fn new(relay: &Relay, mode: StartMode, signer: Arc<RecordedSigner>) -> Self {
        let fixture = Fixture::new();
        fixture.clock.live.store(true, Ordering::SeqCst);
        let (store, input, proof, _) = fixture.create_with_expiry(now() + 600).await;
        let cache = fixture.path.parent().unwrap().join("cache");
        std::fs::create_dir(&cache).unwrap();
        let options = OpenOptions::new(Paths::from_directory(cache).unwrap(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([19; 32]).unwrap(), 1)
            .unwrap();
        let client = Client::open(
            NativeConfig::public_read(options)
                .with_workspace(store.as_ref().clone())
                .with_nostr(
                    relay.config(),
                    SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
                )
                .unwrap(),
        )
        .await
        .unwrap();
        let scope = client.attach(input.clone(), copy(&proof)).await.unwrap();
        client
            .attach_signer(SignerHandle::new(signer.clone(), Arc::new(Callbacks)))
            .await
            .unwrap();
        client.start(mode).await.unwrap();
        Self {
            fixture,
            store,
            client,
            scope,
            input,
            proof,
            signer,
        }
    }
    fn context(&self, id: u8, not_after: u64) -> radroots::CommandContext {
        self.scope
            .command_context_with_correlation(
                OperationId::from_bytes([id; 32]),
                None,
                None,
                UnixSeconds::new(not_after).unwrap(),
                Some(CorrelationId::new("workflow-fault").unwrap()),
            )
            .unwrap()
    }
    async fn snapshot(&self, id: u8) -> radroots_storage::command::OperationSnapshot {
        self.store
            .operation_snapshot(
                self.scope.binding().clone(),
                OperationId::from_bytes([id; 32]),
                None,
                copy(&self.proof),
            )
            .await
            .unwrap()
            .unwrap()
    }
    async fn writer_is_retained(&self) {
        assert!(matches!(
            NativeWorkspace::open_existing(
                &self.fixture.path,
                self.input.clone(),
                self.fixture.crypto(),
                self.fixture.clock.clone()
            )
            .await,
            Err(WorkspaceError::WriterActive)
        ));
    }
}
fn inquiry() -> InquireInput {
    InquireInput::new(
        ThreadId::from_bytes([34; 32]),
        PublicKey::from_bytes(route_key(2).public_key().to_bytes()).unwrap(),
        None,
        "ordinary fault path",
    )
    .unwrap()
}
async fn stop(relay: Relay) {
    relay.stop.send(()).unwrap();
    relay.task.await.unwrap();
}

#[tokio::test]
async fn planning_refuses_wrong_participant_missing_private_methods_and_altered_decryption_before_t1()
 {
    let relay = Relay::new().await;
    for (index, fault) in [
        CryptoFault::None,
        CryptoFault::EncryptionUnavailable,
        CryptoFault::DecryptionUnavailable,
        CryptoFault::WrongPlaintext,
    ]
    .into_iter()
    .enumerate()
    {
        let mut signer = RecordedSigner::new(None);
        if index == 0 {
            Arc::get_mut(&mut signer).unwrap().local = LocalSigner::new(
                parse_secret_key(
                    "0000000000000000000000000000000000000000000000000000000000000002",
                )
                .unwrap(),
            )
            .unwrap()
            .with_nonce_source(Arc::new(OsNonceSource));
        }
        *signer.crypto_fault.lock().unwrap() = fault;
        let ready = Ready::new(&relay, StartMode::Online, signer).await;
        let id = 210 + index as u8;
        let error = ready
            .client
            .inbox()
            .inquire(ready.context(id, now() + 120), inquiry())
            .await
            .unwrap_err();
        assert_eq!(error.context().admission(), Admission::Confirmed);
        assert_eq!(
            error.code(),
            if index == 3 {
                Code::RecoveryRequired
            } else {
                Code::Unauthorized
            }
        );
        assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            ready.signer.crypto_calls.load(Ordering::SeqCst),
            if index == 3 { 2 } else { 0 }
        );
        assert!(ready.snapshot(id).await.bound_plan().is_none());
        assert_eq!(relay.count.load(Ordering::SeqCst), 0);
        ready.client.close().await.unwrap();
    }
    stop(relay).await;
}

#[tokio::test]
async fn explicit_resume_refuses_foreign_and_closed_attachments_before_read_or_dispatch() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, RecordedSigner::new(None)).await;
    let foreign = Ready::new(&relay, StartMode::Online, RecordedSigner::new(None)).await;
    let operation = OperationId::from_bytes([214; 32]);
    let error = ready
        .client
        .operations(&foreign.scope)
        .resume(operation, None)
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unauthorized);
    assert_eq!(error.context().admission(), Admission::NotAdmitted);
    ready.scope.close();
    let error = ready
        .client
        .operations(&ready.scope)
        .resume(operation, None)
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::CapabilityUnavailable);
    assert_eq!(error.context().admission(), Admission::NotAdmitted);
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    ready.client.close().await.unwrap();
    foreign.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn local_only_admits_without_crypto_network_or_mode_escalation() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::LocalOnly, RecordedSigner::new(None)).await;
    let absent = ready
        .client
        .operations(&ready.scope)
        .resume(OperationId::from_bytes([202; 32]), None)
        .await
        .unwrap_err();
    assert_eq!(absent.code(), Code::MissingHistory);
    assert_eq!(absent.context().admission(), Admission::NotAdmitted);
    let receipt = ready
        .client
        .inbox()
        .inquire(ready.context(202, now() + 120), inquiry())
        .await
        .unwrap();
    assert!(receipt.durable());
    assert!(receipt.business_result().is_none());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    assert!(ready.client.start(StartMode::Online).await.is_err());
    assert!(
        ready
            .client
            .operations(&ready.scope)
            .get(OperationId::from_bytes([202; 32]))
            .await
            .unwrap()
            .is_some()
    );
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn explicit_delivery_retry_after_first_issue_expiry_preserves_original_bytes_and_criterion() {
    let mut relay = Relay::new().await;
    relay.accepting.store(false, Ordering::SeqCst);
    let ready = Ready::new(&relay, StartMode::Online, RecordedSigner::new(None)).await;
    let not_after = now() + 10;
    let first = ready
        .client
        .inbox()
        .inquire(ready.context(203, not_after), inquiry())
        .await
        .unwrap();
    assert!(first.business_result().is_some());
    let initial_count = relay.count.load(Ordering::SeqCst);
    assert!((1..=2).contains(&initial_count));
    let snapshot = ready.snapshot(203).await;
    assert!(
        snapshot
            .publications()
            .iter()
            .all(|work| !snapshot.publication_accepted(*work).unwrap())
    );
    let mut original = Vec::new();
    assert_eq!(snapshot.delivery_attempts().len(), 2);
    for attempt in snapshot.delivery_attempts() {
        let retained = ready
            .store
            .delivery_attempt(
                ready.scope.binding().clone(),
                OperationId::from_bytes([203; 32]),
                first.command_digest(),
                attempt.work().evidence().recipient(),
                attempt.intent().request_id().clone(),
                copy(&ready.proof),
            )
            .await
            .unwrap()
            .unwrap();
        original.push(retained.raw_json().to_owned());
    }
    while relay.events.try_recv().is_ok() {}
    let replay = ready
        .client
        .inbox()
        .inquire(ready.context(203, not_after), inquiry())
        .await
        .unwrap();
    assert_eq!(replay.operation_checkpoint(), first.operation_checkpoint());
    assert_eq!(relay.count.load(Ordering::SeqCst), initial_count);
    tokio::time::sleep(Duration::from_secs(not_after.saturating_sub(now()) + 1)).await;
    relay.accepting.store(true, Ordering::SeqCst);
    let resumed = ready
        .client
        .operations(&ready.scope)
        .resume(
            OperationId::from_bytes([203; 32]),
            Some(first.command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(resumed.issuance_window(), first.issuance_window());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    let mut retried = Vec::new();
    for _ in 0..2 {
        retried.push(relay.events.recv().await.unwrap());
    }
    retried.sort();
    original.sort();
    assert!(
        retried == original,
        "retry changed the exact original outer representation"
    );
    let snapshot = ready.snapshot(203).await;
    assert_eq!(snapshot.delivery_attempts().len(), 4);
    assert!(
        snapshot
            .publications()
            .iter()
            .all(|work| snapshot.publication_accepted(*work).unwrap())
    );
    for work in snapshot.publications() {
        let attempts: Vec<_> = snapshot
            .delivery_attempts()
            .iter()
            .filter(|attempt| attempt.work() == *work)
            .collect();
        assert_eq!(
            attempts[0].intent().publication_policy_digest(),
            attempts[1].intent().publication_policy_digest()
        );
        assert_eq!(attempts[0].envelope_digest(), attempts[1].envelope_digest());
    }
    ready
        .client
        .operations(&ready.scope)
        .resume(OperationId::from_bytes([203; 32]), None)
        .await
        .unwrap();
    assert_eq!(relay.count.load(Ordering::SeqCst), initial_count + 2);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn uncertain_second_recipient_never_reissues_and_preserves_actual_first_copy_installation() {
    let relay = Relay::new().await;
    let uncertain = PublicKey::from_bytes(route_key(2).public_key().to_bytes()).unwrap();
    let ready = Ready::new(
        &relay,
        StartMode::Online,
        RecordedSigner::new(Some(uncertain)),
    )
    .await;
    let first = ready
        .client
        .inbox()
        .inquire(ready.context(204, now() + 120), inquiry())
        .await
        .unwrap();
    assert!(first.business_result().is_some());
    let snapshot = ready.snapshot(204).await;
    assert_eq!(
        snapshot
            .recipients()
            .iter()
            .filter(|recipient| recipient.obtained().is_some())
            .count(),
        1
    );
    assert_eq!(snapshot.publications().len(), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    ready
        .client
        .operations(&ready.scope)
        .resume(OperationId::from_bytes([204; 32]), None)
        .await
        .unwrap();
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn failed_obtained_capture_blocks_close_retains_writer_and_retries_without_signing_again() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, RecordedSigner::new(None)).await;
    let mut db = ready.fixture.database().await;
    sqlx::query("CREATE TRIGGER capture_insert_fault BEFORE INSERT ON seal_evidence BEGIN SELECT RAISE(ABORT, 'capture_insert_fault'); END").execute(&mut db).await.unwrap();
    let error = ready
        .client
        .inbox()
        .inquire(ready.context(205, now() + 120), inquiry())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::RecoveryRequired);
    assert_eq!(error.context().admission(), Admission::Confirmed);
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    let error = ready.client.close().await.unwrap_err();
    assert_eq!(error.code(), Code::RecoveryRequired);
    assert_eq!(ready.client.state(), SessionState::Closing);
    ready.writer_is_retained().await;
    sqlx::query("DROP TRIGGER capture_insert_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    ready.client.close().await.unwrap();
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    let reopened = ready.fixture.reopen(ready.input).await;
    let snapshot = reopened
        .operation_snapshot(
            ready.scope.binding().clone(),
            OperationId::from_bytes([205; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        snapshot
            .recipients()
            .iter()
            .filter(|recipient| recipient.obtained().is_some())
            .count(),
        1
    );
    reopened.close_workspace().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn explicit_resume_recovers_unknown_capture_under_same_writer_before_continuing_original() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, RecordedSigner::new(None)).await;
    let mut db = ready.fixture.database().await;
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE capture_commit_fault (parent INTEGER REFERENCES workspace_grant(singleton) DEFERRABLE INITIALLY DEFERRED)").execute(&mut db).await.unwrap();
    sqlx::query("CREATE TRIGGER capture_commit_fault AFTER INSERT ON seal_evidence BEGIN INSERT INTO capture_commit_fault(parent) VALUES(2); END").execute(&mut db).await.unwrap();
    let error = ready
        .client
        .inbox()
        .inquire(ready.context(207, now() + 120), inquiry())
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::RecoveryRequired);
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert!(
        ready
            .store
            .operation_snapshot(
                ready.scope.binding().clone(),
                OperationId::from_bytes([207; 32]),
                None,
                copy(&ready.proof)
            )
            .await
            .is_err()
    );
    assert_eq!(
        ready
            .client
            .operations(&ready.scope)
            .resume(OperationId::from_bytes([207; 32]), None)
            .await
            .unwrap_err()
            .code(),
        Code::RecoveryRequired
    );
    ready.writer_is_retained().await;
    sqlx::query("DROP TRIGGER capture_commit_fault")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::query("DROP TABLE capture_commit_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    let receipt = ready
        .client
        .operations(&ready.scope)
        .resume(OperationId::from_bytes([207; 32]), None)
        .await
        .unwrap();
    assert!(receipt.business_result().is_some());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    let snapshot = ready.snapshot(207).await;
    assert!(
        snapshot
            .recipients()
            .iter()
            .all(|recipient| recipient.obtained().is_some())
    );
    assert!(
        snapshot
            .publications()
            .iter()
            .all(|work| snapshot.publication_accepted(*work).unwrap())
    );
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn lost_observer_and_cancelled_close_preserve_actual_obtained_signature_until_capture() {
    let relay = Relay::new().await;
    let signer = RecordedSigner::new(None);
    let (reached, produced) = oneshot::channel();
    let (resume, released) = oneshot::channel();
    *signer.pause.lock().unwrap() = Some((reached, released));
    let ready = Ready::new(&relay, StartMode::Online, signer).await;
    let context = ready.context(206, now() + 120);
    let client = ready.client.clone();
    let observer = tokio::spawn(async move { client.inbox().inquire(context, inquiry()).await });
    tokio::time::timeout(Duration::from_secs(5), produced)
        .await
        .unwrap()
        .unwrap();
    observer.abort();
    assert!(observer.await.unwrap_err().is_cancelled());
    {
        let mut close = Box::pin(ready.client.close());
        assert!(futures::poll!(&mut close).is_pending());
    }
    assert_eq!(ready.client.state(), SessionState::Closing);
    ready.writer_is_retained().await;
    resume.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), ready.client.close())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    let reopened = ready.fixture.reopen(ready.input).await;
    let snapshot = reopened
        .operation_snapshot(
            ready.scope.binding().clone(),
            OperationId::from_bytes([206; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        snapshot
            .recipients()
            .iter()
            .any(|recipient| recipient.obtained().is_some())
    );
    reopened.close_workspace().await.unwrap();
    stop(relay).await;
}
