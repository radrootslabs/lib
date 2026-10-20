//! Ordinary public commands with a signing-only provider and measured relay reads.
#[path = "receipt_public_snapshots.rs"]
mod snapshots;

use super::*;
use radroots::listings::PublishInput;
use radroots::{BusinessInstallation, Delivery, Issuance, OperationKind};
use radroots_execution::{Admission, Code};
use radroots_signing::{
    issuance::{SigningIntent, SigningOutcome},
    provider::{
        PrivateCiphertext, PrivateCryptoCapability, PrivateText, ProviderCall, ProviderState,
    },
};

type SigningPause = (oneshot::Sender<()>, oneshot::Receiver<()>);
struct PublicSigner {
    local: LocalSigner,
    calls: AtomicU64,
    private_calls: AtomicU64,
    unknown: AtomicBool,
    pause: Mutex<Option<SigningPause>>,
}
impl PublicSigner {
    fn new() -> Arc<Self> {
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
            private_calls: AtomicU64::new(0),
            unknown: AtomicBool::new(false),
            pause: Mutex::new(None),
        })
    }
}
impl Signer for PublicSigner {
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
impl ParticipantProvider for PublicSigner {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            let state = self.local.provider_state().await?;
            Ok(ProviderState::new(
                state.identity(),
                PrivateCryptoCapability::Unavailable,
                PrivateCryptoCapability::Unavailable,
                state.signing_replay(),
            )
            .with_exact_issuance(state.exact_issuance()))
        })
    }
    fn sign_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        Box::pin(async move {
            assert_eq!(intent.recipient(), None);
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.unknown.load(Ordering::SeqCst) {
                return SigningOutcome::unknown(intent.binding(), ProviderError::ProviderFailure);
            }
            let actual = self.local.sign_exact(intent).await;
            let pause = self.pause.lock().unwrap().take();
            if let Some((reached, resume)) = pause {
                reached.send(()).unwrap();
                resume.await.unwrap();
            }
            actual
        })
    }
    fn encrypt_private(
        &self,
        _call: ProviderCall,
        _recipient: PublicKey,
        _text: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        self.private_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ProviderError::CapabilityUnavailable) })
    }
    fn decrypt_private(
        &self,
        _call: ProviderCall,
        _sender: PublicKey,
        _text: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        self.private_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ProviderError::CapabilityUnavailable) })
    }
}
fn input(id: u8) -> PublishInput {
    let wire = json!({"id":format!("{id:02x}").repeat(16),"title":"  Native carrots 🥕  ","summary":"Quote required","description_markdown":"Exact public terms",
        "published_at":now().to_string(),"profile":{"profile":"radroots.listing","version":1,"state":"active",
        "product":{"name":"Carrots","description":"x".repeat(4096),"producer":null,"origin_summary":null},
        "default_option":"02".repeat(32),"options":[{"id":"02".repeat(32),"label":"Bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},
        "increment":"1","minimum":"1","price":{"type":"quote_required"},"supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Example","lead_seconds":"0"},"authority":null,"media":[]}});
    PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).unwrap()
}
fn config(relay: &Relay) -> Config {
    Config::from_profile(
        RelayProfile::explicit(
            RelayProfileKind::Simulator,
            [
                RelayEndpoint::new(&relay.url, RelayUrlPolicy::Local, RelayAccess::ReadWrite)
                    .unwrap(),
            ],
        )
        .unwrap(),
    )
    .with_timeouts(2000, 2000, 2000)
    .unwrap()
}
struct Ready {
    fixture: Fixture,
    store: Arc<NativeWorkspace>,
    client: Client,
    scope: radroots::WorkspaceSession,
    input: AttachmentInput,
    proof: AttachmentProof,
    signer: Arc<PublicSigner>,
}
async fn open(
    fixture: &Fixture,
    store: &NativeWorkspace,
    relay: &Relay,
    name: &str,
    signer: Arc<PublicSigner>,
    credential: (&AttachmentInput, &AttachmentProof),
    mode: StartMode,
) -> (Client, radroots::WorkspaceSession) {
    let (input, proof) = credential;
    let cache = fixture.path.parent().unwrap().join(name);
    std::fs::create_dir(&cache).unwrap();
    let options = OpenOptions::new(Paths::from_directory(cache).unwrap(), OpenMode::Create)
        .with_source_generation(SourceGeneration::new([41; 32]).unwrap(), 1)
        .unwrap();
    let client = Client::open(
        NativeConfig::public_read(options)
            .with_workspace(store.clone())
            .with_nostr(
                config(relay),
                SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            )
            .unwrap(),
    )
    .await
    .unwrap();
    let scope = client.attach(input.clone(), copy(proof)).await.unwrap();
    client
        .attach_signer(SignerHandle::new(signer, Arc::new(Callbacks)))
        .await
        .unwrap();
    client.start(mode).await.unwrap();
    (client, scope)
}
impl Ready {
    async fn new(relay: &Relay, mode: StartMode, signer: Arc<PublicSigner>, seller: bool) -> Self {
        let fixture = Fixture::new();
        fixture.clock.live.store(true, Ordering::SeqCst);
        let cfg = if seller {
            LocalOutboxConfig::seller(
                key(),
                key(),
                "public-workflow",
                UnixSeconds::new(now() + 600).unwrap(),
            )
        } else {
            LocalOutboxConfig::new(
                key(),
                key(),
                "public-workflow",
                UnixSeconds::new(now() + 600).unwrap(),
            )
        }
        .unwrap();
        let (store, input, proof) = NativeWorkspace::initialize_local_outbox(
            &fixture.path,
            cfg,
            fixture.crypto(),
            fixture.clock.clone(),
        )
        .await
        .unwrap();
        let store = Arc::new(store);
        let (client, scope) = open(
            &fixture,
            &store,
            relay,
            "cache",
            signer.clone(),
            (&input, &proof),
            mode,
        )
        .await;
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
    fn context(&self, id: u8, deadline: u64) -> radroots::CommandContext {
        self.scope
            .command_context(
                OperationId::from_bytes([id; 32]),
                None,
                None,
                UnixSeconds::new(deadline).unwrap(),
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
}
async fn stop(relay: Relay) {
    relay.stop.send(()).unwrap();
    relay.task.await.unwrap();
}

#[tokio::test]
async fn ordinary_public_publish_uses_no_private_capability_and_queries_the_actual_listing_after_reopen()
 {
    let mut relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    let captured = input(211);
    let deadline = now() + 120;
    let receipt = ready
        .client
        .listings()
        .publish(ready.context(211, deadline), captured.clone())
        .await
        .unwrap();
    assert_eq!(receipt.kind(), OperationKind::Listing);
    assert_eq!(
        receipt.business_installation(),
        BusinessInstallation::Installed
    );
    assert_eq!(receipt.issuance(), Issuance::Obtained);
    assert_eq!(receipt.delivery(), Delivery::Observed);
    assert_eq!(receipt.message_id(), None);
    assert!(receipt.record_ids().is_empty());
    let listing = receipt.business_result().unwrap();
    assert_eq!(listing.id(), captured.id());
    assert_eq!(listing.author(), key());
    assert_eq!(listing.text(), captured.text());
    assert_eq!(listing.terms(), captured.terms());
    assert_eq!(listing.published_at(), captured.published_at());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.requests.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    let raw = tokio::time::timeout(Duration::from_secs(5), relay.events.recv())
        .await
        .unwrap()
        .unwrap();
    let independent = nostr::Event::from_json(&raw).unwrap();
    independent.verify().unwrap();
    assert_eq!(independent.kind.as_u16(), 30402);
    assert_eq!(independent.id.as_bytes(), listing.revision().as_bytes());
    assert!(
        independent
            .tags
            .iter()
            .any(|tag| tag.as_slice().iter().any(|v| v.len() > 4096))
    );
    let snapshot = ready.snapshot(211).await;
    assert!(snapshot.publications().is_empty());
    assert!(snapshot.installed_message().is_none());
    assert!(
        snapshot
            .publication_accepted(snapshot.public_publications()[0])
            .unwrap()
    );
    assert!(matches!(
        radroots_execution::OperationReceipt::<radroots_domain::ChatMessage>::try_from_snapshot(
            &snapshot
        ),
        Err(OperationError::UnsupportedOperation)
    ));
    let status = ready
        .client
        .operations(&ready.scope)
        .get(OperationId::from_bytes([211; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.kind(), OperationKind::Listing);
    assert_eq!(status.message_id(), None);
    assert_eq!(
        status.business_result().unwrap().as_listing().unwrap(),
        listing
    );
    assert!(status.business_result().unwrap().as_chat().is_none());
    let replay = ready
        .client
        .listings()
        .publish(ready.context(211, deadline), captured)
        .await
        .unwrap();
    assert_eq!(
        replay.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    ready.client.close().await.unwrap();
    let store = ready.fixture.reopen(ready.input.clone()).await;
    let (client, scope) = open(
        &ready.fixture,
        &store,
        &relay,
        "cache-reopened",
        ready.signer.clone(),
        (&ready.input, &ready.proof),
        StartMode::Online,
    )
    .await;
    let restored = client
        .operations(&scope)
        .get(OperationId::from_bytes([211; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        restored.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    assert_eq!(
        restored.business_result().unwrap().as_listing().unwrap(),
        listing
    );
    let resumed = client
        .operations(&scope)
        .resume(
            OperationId::from_bytes([211; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(
        resumed.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_local_public_admission_returns_listing_family_without_signing_or_network() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::LocalOnly, PublicSigner::new(), true).await;
    let receipt = ready
        .client
        .listings()
        .publish(ready.context(212, now() + 120), input(212))
        .await
        .unwrap();
    assert!(receipt.durable());
    assert_eq!(receipt.kind(), OperationKind::Listing);
    assert!(receipt.business_result().is_none());
    assert_eq!(receipt.issuance(), Issuance::None);
    assert_eq!(receipt.message_id(), None);
    let status = ready
        .client
        .operations(&ready.scope)
        .get(OperationId::from_bytes([212; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.kind(), OperationKind::Listing);
    assert!(status.business_result().is_none());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 0);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_public_explicit_retry_after_first_issue_expiry_preserves_original_bytes_and_policy()
 {
    let mut relay = Relay::new().await;
    relay.accepting.store(false, Ordering::SeqCst);
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    let deadline = now() + 10;
    let captured = input(213);
    let receipt = ready
        .client
        .listings()
        .publish(ready.context(213, deadline), captured.clone())
        .await
        .unwrap();
    assert!(receipt.business_result().is_some());
    let original = relay.events.recv().await.unwrap();
    let before = ready.snapshot(213).await;
    let policy = *before.delivery_attempts()[0]
        .intent()
        .publication_policy_digest();
    assert!(
        !before
            .publication_accepted(before.public_publications()[0])
            .unwrap()
    );
    let replay = ready
        .client
        .listings()
        .publish(ready.context(213, deadline), captured)
        .await
        .unwrap();
    assert_eq!(
        replay.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    tokio::time::sleep(Duration::from_secs(deadline.saturating_sub(now()) + 1)).await;
    relay.accepting.store(true, Ordering::SeqCst);
    let resumed = ready
        .client
        .operations(&ready.scope)
        .resume(
            OperationId::from_bytes([213; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(resumed.issuance_window(), receipt.issuance_window());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), relay.events.recv())
            .await
            .unwrap()
            .unwrap(),
        original
    );
    let after = ready.snapshot(213).await;
    assert_eq!(after.delivery_attempts().len(), 2);
    assert!(
        after
            .publication_accepted(after.public_publications()[0])
            .unwrap()
    );
    assert!(
        after
            .delivery_attempts()
            .iter()
            .all(|v| v.intent().publication_policy_digest() == &policy)
    );
    assert!(
        after
            .delivery_attempts()
            .iter()
            .any(|attempt| attempt.target_ordinals() == [2])
    );
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_public_resume_requires_original_targets_to_remain_configured() {
    let mut original_relay = Relay::new().await;
    original_relay.accepting.store(false, Ordering::SeqCst);
    let replacement = Relay::new().await;
    let ready = Ready::new(
        &original_relay,
        StartMode::Online,
        PublicSigner::new(),
        true,
    )
    .await;
    let receipt = ready
        .client
        .listings()
        .publish(ready.context(218, now() + 120), input(218))
        .await
        .unwrap();
    let original_bytes = original_relay.events.recv().await.unwrap();
    let before = ready.snapshot(218).await;
    assert_eq!(before.delivery_attempts().len(), 1);
    ready.client.close().await.unwrap();

    let store = ready.fixture.reopen(ready.input.clone()).await;
    let (client, scope) = open(
        &ready.fixture,
        &store,
        &replacement,
        "cache-reconfigured",
        ready.signer.clone(),
        (&ready.input, &ready.proof),
        StartMode::Online,
    )
    .await;
    let observed = client
        .operations(&scope)
        .get(OperationId::from_bytes([218; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        observed.operation_checkpoint(),
        receipt.operation_checkpoint()
    );
    let error = client
        .operations(&scope)
        .resume(
            OperationId::from_bytes([218; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unauthorized);
    assert_eq!(error.context().admission(), Admission::Confirmed);
    assert_eq!(
        error.context().operation_id(),
        Some(OperationId::from_bytes([218; 32]))
    );
    let after = store
        .operation_snapshot(
            scope.binding().clone(),
            OperationId::from_bytes([218; 32]),
            Some(receipt.command_digest()),
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.checkpoint_digest(), before.checkpoint_digest());
    assert_eq!(after.delivery_attempts().len(), 1);
    assert_eq!(replacement.count.load(Ordering::SeqCst), 0);
    assert_eq!(replacement.requests.load(Ordering::SeqCst), 0);
    assert_eq!(original_relay.count.load(Ordering::SeqCst), 1);
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    client.close().await.unwrap();

    original_relay.accepting.store(true, Ordering::SeqCst);
    let store = ready.fixture.reopen(ready.input.clone()).await;
    let (client, scope) = open(
        &ready.fixture,
        &store,
        &original_relay,
        "cache-original-restored",
        ready.signer.clone(),
        (&ready.input, &ready.proof),
        StartMode::Online,
    )
    .await;
    let resumed = client
        .operations(&scope)
        .resume(
            OperationId::from_bytes([218; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(resumed.business_result(), observed.business_result());
    assert!(
        resumed
            .delivery_observations()
            .iter()
            .any(|observation| { observation.outcome() == radroots::TargetObservation::Accepted })
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), original_relay.events.recv())
            .await
            .unwrap()
            .unwrap(),
        original_bytes
    );
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    client.close().await.unwrap();
    stop(replacement).await;
    stop(original_relay).await;
}

#[tokio::test]
async fn public_snapshot_cannot_remove_installation_beneath_retained_delivery() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    ready
        .client
        .listings()
        .publish(ready.context(219, now() + 120), input(219))
        .await
        .unwrap();
    let snapshot = ready.snapshot(219).await;
    assert!(snapshot.listing_installation().is_some());
    assert!(!snapshot.delivery_attempts().is_empty());
    assert!(!snapshot.delivery_observations().is_empty());
    let changed = snapshot.with_public_installation(None, vec![]);
    ready.client.close().await.unwrap();
    stop(relay).await;
    assert!(matches!(changed, Err(OperationError::RecoveryRequired)));
}

#[tokio::test]
async fn ordinary_unknown_public_issuance_is_never_reissued_by_query_or_explicit_resume() {
    let relay = Relay::new().await;
    let signer = PublicSigner::new();
    signer.unknown.store(true, Ordering::SeqCst);
    let ready = Ready::new(&relay, StartMode::Online, signer, true).await;
    let receipt = ready
        .client
        .listings()
        .publish(ready.context(214, now() + 120), input(214))
        .await
        .unwrap();
    assert_eq!(receipt.issuance(), Issuance::Unknown);
    assert!(receipt.business_result().is_none());
    assert_eq!(receipt.message_id(), None);
    let resumed = ready
        .client
        .operations(&ready.scope)
        .resume(
            OperationId::from_bytes([214; 32]),
            Some(receipt.command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(resumed.issuance(), Issuance::Unknown);
    assert!(resumed.business_result().is_none());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    ready.client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_public_observer_loss_retains_actual_obtained_output_until_close_drains() {
    let relay = Relay::new().await;
    let signer = PublicSigner::new();
    let (reached, produced) = oneshot::channel();
    let (resume, released) = oneshot::channel();
    *signer.pause.lock().unwrap() = Some((reached, released));
    let ready = Ready::new(&relay, StartMode::Online, signer, true).await;
    let context = ready.context(215, now() + 120);
    let client = ready.client.clone();
    let value = input(215);
    let observer = tokio::spawn(async move { client.listings().publish(context, value).await });
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
    assert!(matches!(
        NativeWorkspace::open_existing(
            &ready.fixture.path,
            ready.input.clone(),
            ready.fixture.crypto(),
            ready.fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    resume.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), ready.client.close())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let snapshot = reopened
        .operation_snapshot(
            ready.scope.binding().clone(),
            OperationId::from_bytes([215; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.artifacts()[0].obtained().is_some());
    assert!(snapshot.listing_installation().is_some());
    assert!(snapshot.installed_message().is_none());
    reopened.close_workspace().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_public_capture_failure_retains_writer_and_recovers_original_without_new_signature()
 {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    let mut db = ready.fixture.database().await;
    sqlx::query("CREATE TRIGGER public_capture_fault BEFORE INSERT ON artifact_evidence BEGIN SELECT RAISE(ABORT,'public_capture_fault'); END").execute(&mut db).await.unwrap();
    assert!(
        ready
            .client
            .listings()
            .publish(ready.context(216, now() + 120), input(216))
            .await
            .is_err()
    );
    assert!(ready.client.close().await.is_err());
    assert!(matches!(
        NativeWorkspace::open_existing(
            &ready.fixture.path,
            ready.input.clone(),
            ready.fixture.crypto(),
            ready.fixture.clock.clone()
        )
        .await,
        Err(WorkspaceError::WriterActive)
    ));
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    sqlx::query("DROP TRIGGER public_capture_fault")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    ready.client.close().await.unwrap();
    let reopened = ready.fixture.reopen(ready.input.clone()).await;
    let snapshot = reopened
        .operation_snapshot(
            ready.scope.binding().clone(),
            OperationId::from_bytes([216; 32]),
            None,
            copy(&ready.proof),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.artifacts()[0].obtained().is_some());
    assert!(snapshot.listing_installation().is_none());
    let (client, scope) = open(
        &ready.fixture,
        &reopened,
        &relay,
        "cache-recovery",
        ready.signer.clone(),
        (&ready.input, &ready.proof),
        StartMode::Online,
    )
    .await;
    let resumed = client
        .operations(&scope)
        .resume(
            OperationId::from_bytes([216; 32]),
            Some(snapshot.prepared().identity().command_digest()),
        )
        .await
        .unwrap();
    assert_eq!(
        resumed.business_installation(),
        BusinessInstallation::Installed
    );
    assert!(resumed.business_result().unwrap().as_listing().is_some());
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(relay.count.load(Ordering::SeqCst), 1);
    client.close().await.unwrap();
    stop(relay).await;
}

#[tokio::test]
async fn ordinary_publication_never_upgrades_an_actual_buyer_outbox_to_seller_authority() {
    let relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), false).await;
    assert!(
        ready
            .client
            .listings()
            .publish(ready.context(217, now() + 120), input(217))
            .await
            .is_err()
    );
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 0);
    assert!(
        ready
            .store
            .operation(
                ready.scope.binding().clone(),
                OperationId::from_bytes([217; 32]),
                None,
                copy(&ready.proof)
            )
            .await
            .unwrap()
            .is_none()
    );
    ready.client.close().await.unwrap();
    stop(relay).await;
}
