//! Real facade, SQLx and local crypto with explicitly controlled command IO.
//! This is no live Nostr transport or host-custody qualification.
use super::{Client, NativeConfig, StartMode};
use radroots_domain::{CommandContextData, OperationId, PoolId, PublicKey, UnixSeconds};
use radroots_execution::{
    session::{
        HostFuture,
        command::{CommandIo, CommandRuntime, capture_dm_route, selected_dm_targets},
    },
    workspace::CommandContext,
};
use radroots_nostr::{key::SecretKey, public_ingress::ObservedHead, signing::LocalSigner};
use radroots_protocol::{
    command::{AcceptOrderInput, AllocationSelection, CapturedOrderAcceptance},
    nip59::CapturedDmRoute,
};
use radroots_secrets::{
    SecretId, SecretRef,
    envelope::Nonce,
    file::{FileOpenMode, FileProvider},
    id::{BackendKind, KeyVersion},
    provider::AccessPolicy,
    wrapping::SecretMaterial,
};
use radroots_signing::{
    SignReceipt, SignRequest, Signer, SignerStatus,
    issuance::{
        ExactRecoveryCapability, ExactRecoveryOutcome, ExactRecoveryRequest, SigningIntent,
        SigningOutcome,
    },
    provider::{
        ParticipantProvider, PrivateCiphertext, PrivateText, ProviderCall, ProviderError,
        ProviderState, SignerCallbacks, SignerHandle,
    },
    signer::BoxFuture,
};
use radroots_storage::{
    command::{EnvelopeCandidate, OperationError, PublicationWork},
    event::SourceGeneration,
    inventory::{InventoryCheckpointRef, InventoryOutcome, InventoryRequest, InventoryStore},
    workspace::{
        AttachmentProof, WorkspaceClock, WorkspaceError,
        seller_stock::{
            SellerAuthoritySelection, StockGrantScope, StockListingCheckpointRef, StockResources,
            StockSourceCheckpointRef,
        },
    },
};
use radroots_storage_sqlite::{
    OpenMode, OpenOptions, Paths,
    workspace::{
        NativeWorkspace, SellerStockConfig, StockConsentCapture, StockConsentMode, StockEnrollment,
        StockGrantEnrollment, StockListingCapture, StockRequestSnapshot, StockSnapshotRequest,
        StockSourceCapture, WorkspaceCrypto,
    },
};
use radroots_transport::{Target, TargetSet};
use std::{
    num::NonZeroU64,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

fn data() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../tests/fixtures/ordinary_acceptance/inputs.v1.json"
    ))
    .unwrap()
}
fn proof(value: &AttachmentProof) -> AttachmentProof {
    value
        .with_secret(|b| AttachmentProof::from_protected_input(b))
        .unwrap()
}
struct Clock(AtomicU64);
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        UnixSeconds::new(self.0.load(Ordering::SeqCst)).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Control {
    recovery_supported: AtomicBool,
    signs: AtomicUsize,
    recoveries: AtomicUsize,
    fail_self: AtomicBool,
    pause_peer: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl Control {
    fn new() -> Self {
        Self {
            recovery_supported: AtomicBool::new(true),
            signs: AtomicUsize::new(0),
            recoveries: AtomicUsize::new(0),
            fail_self: AtomicBool::new(false),
            pause_peer: AtomicBool::new(false),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }
}
struct Provider {
    inner: LocalSigner,
    control: Arc<Control>,
}
impl Signer for Provider {
    fn status(&self) -> BoxFuture<'_, Result<SignerStatus, radroots_signing::Error>> {
        self.inner.status()
    }
    fn sign(&self, _: SignRequest) -> BoxFuture<'_, Result<SignReceipt, radroots_signing::Error>> {
        panic!("acceptance must never invoke legacy signing")
    }
}
impl ParticipantProvider for Provider {
    fn provider_state(&self) -> BoxFuture<'_, Result<ProviderState, ProviderError>> {
        Box::pin(async move {
            let state = self.inner.provider_state().await?;
            Ok(if self.control.recovery_supported.load(Ordering::SeqCst) {
                state
            } else {
                state.with_exact_recovery(ExactRecoveryCapability::Unavailable)
            })
        })
    }
    fn encrypt_private(
        &self,
        c: ProviderCall,
        p: PublicKey,
        t: PrivateText,
    ) -> BoxFuture<'_, Result<PrivateCiphertext, ProviderError>> {
        self.inner.encrypt_private(c, p, t)
    }
    fn decrypt_private(
        &self,
        c: ProviderCall,
        p: PublicKey,
        t: PrivateCiphertext,
    ) -> BoxFuture<'_, Result<PrivateText, ProviderError>> {
        self.inner.decrypt_private(c, p, t)
    }
    fn sign_exact(&self, intent: SigningIntent) -> BoxFuture<'_, SigningOutcome> {
        Box::pin(async move {
            self.control.signs.fetch_add(1, Ordering::SeqCst);
            let is_self = intent.recipient() == Some(self.inner.public_key());
            if is_self && self.control.fail_self.load(Ordering::SeqCst) {
                return SigningOutcome::not_dispatched(intent.binding(), ProviderError::Cancelled);
            }
            let result = self.inner.sign_exact(intent).await;
            if !is_self && self.control.pause_peer.swap(false, Ordering::SeqCst) {
                self.control.entered.notify_one();
                self.control.release.notified().await;
            }
            result
        })
    }
    fn recover_exact(&self, request: ExactRecoveryRequest) -> BoxFuture<'_, ExactRecoveryOutcome> {
        Box::pin(async move {
            self.control.recoveries.fetch_add(1, Ordering::SeqCst);
            self.inner.recover_exact(request).await
        })
    }
}
struct Callbacks;
impl SignerCallbacks for Callbacks {
    fn quiesce(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
    fn close(&self) -> BoxFuture<'_, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}
struct Io(Arc<Clock>);
fn targets() -> TargetSet {
    TargetSet::new(vec![Target::nostr_relay("wss://relay.example").unwrap()]).unwrap()
}
impl CommandIo for Io {
    fn enabled(&self) -> bool {
        true
    }
    fn now(&self) -> Result<UnixSeconds, OperationError> {
        self.0.now().map_err(Into::into)
    }
    fn seal_time(&self, now: UnixSeconds) -> Result<UnixSeconds, OperationError> {
        Ok(now)
    }
    fn route(
        &self,
        recipient: PublicKey,
        _: NonZeroU64,
    ) -> HostFuture<'_, Result<CapturedDmRoute, OperationError>> {
        Box::pin(async move {
            let value = data();
            let row = value["stock_routes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["recipient"].as_str().unwrap() == recipient.to_hex())
                .unwrap();
            capture_dm_route(recipient, [row["raw"].as_str().unwrap()], &targets())
        })
    }
    fn targets(&self, route: &CapturedDmRoute) -> Result<TargetSet, OperationError> {
        selected_dm_targets(route, &targets())
    }
    fn publication_policy(&self) -> radroots_transport::policy::SatisfactionPolicy {
        panic!("acceptance does not start D")
    }
    fn envelope(&self, _: PublicationWork, _: &str) -> Result<EnvelopeCandidate, OperationError> {
        panic!("acceptance does not wrap")
    }
    fn delivery_id(&self) -> Result<String, OperationError> {
        panic!("acceptance does not claim D")
    }
    fn publish(
        &self,
        _: radroots_transport::sink::DeliveryRequest,
        _: TargetSet,
    ) -> HostFuture<'_, Result<radroots_transport::DeliveryReceipt, radroots_transport::SinkFailure>>
    {
        panic!("acceptance does not publish")
    }
}
struct Rig {
    vault: Arc<FileProvider>,
    _temp: tempfile::TempDir,
    client: Client,
    native: NativeWorkspace,
    enrollment: StockEnrollment,
    scope: radroots_execution::workspace::WorkspaceSession,
    clock: Arc<Clock>,
    control: Arc<Control>,
    provider: Weak<Provider>,
}
impl Rig {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let vault = Arc::new(
            FileProvider::open(
                root.join("vault"),
                FileOpenMode::CreateNew,
                SecretMaterial::from_slice(&[0xa5; 32]).unwrap(),
            )
            .unwrap(),
        );
        let reference = SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        );
        vault
            .provision(
                &reference,
                &SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
                Nonce::new([0x11; 24]),
            )
            .unwrap();
        let crypto = WorkspaceCrypto::new(
            vault.clone(),
            reference,
            SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
            AccessPolicy::standard(),
        )
        .unwrap();
        let clock = Arc::new(Clock(AtomicU64::new(150)));
        let control = Arc::new(Control::new());
        let provider = Arc::new(Provider {
            inner: LocalSigner::with_clock(
                SecretKey::parse(&format!("{:064x}", 3)).unwrap(),
                || Ok(150000),
            )
            .unwrap()
            .with_nonce_source(Arc::new(radroots_nostr::signing::OsNonceSource)),
            control: control.clone(),
        });
        let key = provider.inner.public_key();
        assert_eq!(key.to_hex(), data()["scope"]["seller"].as_str().unwrap());
        let enrollment = StockEnrollment::prepare(
            SellerStockConfig::new(
                key,
                key,
                "synthetic-ordinary-acceptance",
                UnixSeconds::new(300).unwrap(),
            )
            .unwrap(),
            clock.as_ref(),
        )
        .unwrap();
        let native = NativeWorkspace::initialize_seller_stock(
            &root.join("stock"),
            &enrollment,
            crypto,
            clock.clone(),
        )
        .await
        .unwrap();
        std::fs::create_dir(root.join("cache")).unwrap();
        let options = OpenOptions::new(
            Paths::from_directory(root.join("cache")).unwrap(),
            OpenMode::Create,
        )
        .with_source_generation(SourceGeneration::new([17; 32]).unwrap(), 1)
        .unwrap();
        let mut client =
            Client::open(NativeConfig::public_read(options).with_workspace(native.clone()))
                .await
                .unwrap();
        // Only IO is controlled. This preserves the actual client task host,
        // attached scopes, stock evidence owner and concrete native adapter.
        client.session = client
            .session
            .clone()
            .with_command_runtime(Arc::new(CommandRuntime::new(Arc::new(Io(clock.clone())))));
        let scope = client
            .attach(enrollment.input().clone(), proof(enrollment.proof()))
            .await
            .unwrap();
        let weak = Arc::downgrade(&provider);
        client
            .attach_signer(SignerHandle::new(provider, Arc::new(Callbacks)))
            .await
            .unwrap();
        client.start(StartMode::LocalOnly).await.unwrap();
        let rig = Self {
            vault,
            _temp: temp,
            client,
            native,
            enrollment,
            scope,
            clock,
            control,
            provider: weak,
        };
        rig.seed().await;
        rig
    }
    async fn minima(
        &self,
    ) -> (
        StockSourceCheckpointRef,
        StockListingCheckpointRef,
        InventoryCheckpointRef,
    ) {
        let current = self
            .native
            .stock_authority(self.enrollment.input().clone(), self.enrollment.proof())
            .await
            .unwrap();
        (
            current.source_checkpoint().unwrap(),
            current.listing_checkpoint().unwrap(),
            current.inventory_checkpoint().unwrap(),
        )
    }
    fn context(&self, id: u8) -> CommandContextData {
        self.scope
            .command_context(
                OperationId::from_bytes([id; 32]),
                None,
                None,
                UnixSeconds::new(230).unwrap(),
            )
            .unwrap()
            .data()
            .clone()
    }
    async fn seed(&self) {
        let value = data();
        let key = self.enrollment.input().participant();
        let raw = value["listing_raw"].as_str().unwrap();
        let listing: serde_json::Value = serde_json::from_str(raw).unwrap();
        let d = listing["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v[0] == "d")
            .unwrap()[1]
            .as_str()
            .unwrap();
        let mut head = ObservedHead::listing(key, d).unwrap();
        head.observe(raw).unwrap();
        let capture = StockListingCapture::prepare(
            SellerAuthoritySelection::from_attachment(self.enrollment.input()),
            &head,
        )
        .unwrap();
        let (_, minimum, _) = self.minima().await;
        self.native
            .retain_stock_listing(
                self.scope.binding().clone(),
                minimum,
                &capture,
                self.enrollment.proof(),
            )
            .await
            .unwrap();
        for row in value["sources"].as_array().unwrap() {
            let local = LocalSigner::with_clock(
                SecretKey::parse(&format!(
                    "{:064x}",
                    row["recipient_scalar"].as_u64().unwrap()
                ))
                .unwrap(),
                || Ok(1000),
            )
            .unwrap();
            let call = ProviderCall::new(
                local.public_key(),
                NonZeroU64::new(2000).unwrap(),
                radroots_signing::request::CancellationSignal::new(),
            );
            let opened =
                radroots_nostr::commerce::open(&local, call, row["raw_wrap"].as_str().unwrap())
                    .await
                    .unwrap();
            let capture = StockSourceCapture::prepare(
                SellerAuthoritySelection::from_attachment(self.enrollment.input()),
                &opened,
            )
            .unwrap();
            let (minimum, _, _) = self.minima().await;
            self.native
                .retain_stock_source(
                    self.scope.binding().clone(),
                    minimum,
                    &capture,
                    self.enrollment.proof(),
                )
                .await
                .unwrap();
        }
        let snapshot = self.request().await;
        let line = &snapshot.terms().lines()[0];
        for (id, wire) in [
            (
                80,
                serde_json::json!({"type":"stock.create_pool","context":self.context(80),"input":{"id":PoolId::from_bytes([80;32]),"label":"private test stock","unit":"each","tick_scale":"0","initial_quantity":"10"}}),
            ),
            (
                81,
                serde_json::json!({"type":"stock.map_option","context":self.context(81),"input":{"listing":line.source().coordinate(),"option_id":line.option_id(),"pool_id":PoolId::from_bytes([80;32]),"expected_mapping_version":"0","per_sale_unit":{"amount":"1","unit":"each"}}}),
            ),
        ] {
            assert_eq!(
                wire["context"]["operation_id"],
                serde_json::to_value(OperationId::from_bytes([id; 32])).unwrap()
            );
            let (_, _, minimum) = self.minima().await;
            let request = InventoryRequest::from_protected_wire(
                self.scope.binding().clone(),
                minimum,
                &serde_json::to_vec(&wire).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                self.native
                    .execute_inventory(request, proof(self.enrollment.proof()))
                    .await
                    .unwrap(),
                InventoryOutcome::Committed(_)
            ));
        }
    }
    async fn request(&self) -> StockRequestSnapshot {
        let (private, public, minimum) = self.minima().await;
        self.native
            .stock_order_request(
                self.scope.binding().clone(),
                minimum,
                private,
                public,
                data()["request"].as_str().unwrap().parse().unwrap(),
                self.enrollment.proof(),
            )
            .await
            .unwrap()
    }
    async fn captured(&self) -> (CapturedOrderAcceptance, CommandContext) {
        let snapshot = self.request().await;
        let mut value = serde_json::json!({"type":"orders.accept","context":self.context(90),"input":{"request":snapshot.request(),"terms_digest":snapshot.terms_digest(),"allocation":{"type":"automatic"}}});
        value["context"]["expected"] = serde_json::json!({"agreement":null,"buyer_head":snapshot.request(),"checkpoint":snapshot.checkpoint(),"order_id":snapshot.scope().order_id(),"seller_sequence":"0"});
        let first = CapturedOrderAcceptance::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        value["context"]["approval"] = serde_json::json!({"intent_digest":first.approval_intent_digest(),"scope":"orders.accept","expires_at":"230"});
        let command = CapturedOrderAcceptance::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let context = self
            .scope
            .command_context(
                command.context().operation_id(),
                command.context().expected().cloned(),
                command.context().approval().cloned(),
                UnixSeconds::new(230).unwrap(),
            )
            .unwrap();
        let capture = StockConsentCapture::prepare(
            SellerAuthoritySelection::from_attachment(self.enrollment.input()),
            self.enrollment.input().grant_id(),
            StockConsentMode::ExecuteAndRecover,
            StockResources::all_pools(),
            UnixSeconds::new(230).unwrap(),
            CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
        )
        .unwrap();
        let (_, _, minimum) = self.minima().await;
        self.native
            .issue_stock_consent(
                self.scope.binding().clone(),
                minimum,
                &capture,
                self.enrollment.proof(),
            )
            .await
            .unwrap();
        (command, context)
    }
    async fn accept(&self) -> (CapturedOrderAcceptance, crate::orders::AcceptanceStatus) {
        let (command, context) = self.captured().await;
        let (private, public, minimum) = self.minima().await;
        let input = AcceptOrderInput::new(
            command.input().request(),
            command.input().terms_digest(),
            AllocationSelection::automatic(),
        );
        let status = self
            .client
            .orders(&self.scope)
            .accept(context, minimum, private, public, input)
            .await
            .unwrap();
        (command, status)
    }
}

#[tokio::test]
async fn ordinary_acceptance_uses_real_t0_t1_s_t2_and_preserves_original_retry() {
    let r = Rig::new().await;
    let (command, status) = r.accept().await;
    assert!(status.installation().is_some());
    assert!(status.recipients().iter().all(|v| v.evidence.is_some()));
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    let (_, _, minimum) = r.minima().await;
    let observed = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(observed.installation(), status.installation());
    r.clock.0.store(231, Ordering::SeqCst);
    let refused = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap_err();
    assert_eq!(refused.code(), crate::Code::DeadlineExceeded);
    // Expired issuing consent is not silently renewed. An actual authorized
    // RecoverOnly replacement permits historical reads, never a new signature.
    let child = StockGrantEnrollment::prepare(
        &SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        "replacement recovery operator".into(),
        UnixSeconds::new(290).unwrap(),
        StockGrantScope::full_inventory(),
    )
    .unwrap();
    r.native
        .enroll_stock_grant(
            r.scope.binding().clone(),
            minimum,
            r.enrollment.proof(),
            &child,
        )
        .await
        .unwrap();
    let (_, _, minimum) = r.minima().await;
    let capture = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        child.input().grant_id(),
        StockConsentMode::RecoverOnly,
        StockResources::all_pools(),
        UnixSeconds::new(280).unwrap(),
        CapturedOrderAcceptance::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    r.native
        .issue_stock_consent(
            r.scope.binding().clone(),
            minimum,
            &capture,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let recovery_scope = r
        .client
        .attach(child.input().clone(), proof(child.proof()))
        .await
        .unwrap();
    let resumed = r
        .client
        .orders(&recovery_scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.installation(), status.installation());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
}

#[tokio::test]
async fn lost_observer_cannot_discard_actual_peer_artifact_and_close_installs_before_key_disposal()
{
    let r = Rig::new().await;
    r.control.pause_peer.store(true, Ordering::SeqCst);
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let input = AcceptOrderInput::new(
        command.input().request(),
        command.input().terms_digest(),
        AllocationSelection::automatic(),
    );
    let client = r.client.clone();
    let scope = r.scope.clone();
    let waiter = tokio::spawn(async move {
        client
            .orders(&scope)
            .accept(context, minimum, private, public, input)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        r.control.entered.notified(),
    )
    .await
    .unwrap();
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    let mut closing = Box::pin(r.client.close());
    assert!(futures::poll!(closing.as_mut()).is_pending());
    assert!(r.provider.upgrade().is_some());
    r.control.release.notify_one();
    closing.await.unwrap();
    assert!(r.provider.upgrade().is_none());
    let crypto = WorkspaceCrypto::new(
        r.vault.clone(),
        SecretRef::new(
            SecretId::parse("workspace-key").unwrap(),
            BackendKind::File,
            KeyVersion::new(1).unwrap(),
        ),
        SecretMaterial::from_slice(&[0x41; 32]).unwrap(),
        AccessPolicy::standard(),
    )
    .unwrap();
    let reopened = NativeWorkspace::open_seller_stock(
        &r._temp.path().join("stock"),
        r.enrollment.input().clone(),
        crypto,
        r.clock.clone(),
    )
    .await
    .unwrap();
    let binding = reopened
        .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    let snapshot = reopened
        .stock_operation_snapshot(
            StockSnapshotRequest::new(
                binding,
                minimum,
                command.context().operation_id(),
                command.digest(),
            ),
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    assert!(snapshot.installation().is_some());
    assert_eq!(
        snapshot
            .recipients()
            .iter()
            .filter(|v| v.first_evidence().is_some())
            .count(),
        1
    );
    assert_eq!(
        snapshot.preparation().attempt().operation_id(),
        command.context().operation_id()
    );
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn peer_installation_survives_self_refusal_and_resume_uses_original_exact_recovery() {
    let r = Rig::new().await;
    r.control.fail_self.store(true, Ordering::SeqCst);
    let (command, status) = r.accept().await;
    let installation = status
        .installation()
        .expect("peer S must install before self refusal");
    assert_eq!(
        status
            .recipients()
            .iter()
            .filter(|v| v.evidence.is_some())
            .count(),
        1
    );
    assert!(!status.aborted());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    r.control.fail_self.store(false, Ordering::SeqCst);
    let (_, _, minimum) = r.minima().await;
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.installation(), Some(installation));
    assert!(resumed.recipients().iter().all(|v| v.evidence.is_some()));
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 1);
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
}

#[tokio::test]
async fn later_local_capability_cannot_upgrade_an_original_non_replayable_unknown_claim() {
    let r = Rig::new().await;
    r.control.recovery_supported.store(false, Ordering::SeqCst);
    r.control.fail_self.store(true, Ordering::SeqCst);
    let (command, status) = r.accept().await;
    assert!(status.installation().is_some());
    r.control.fail_self.store(false, Ordering::SeqCst);
    r.control.recovery_supported.store(true, Ordering::SeqCst);
    let (_, _, minimum) = r.minima().await;
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.installation(), status.installation());
    assert_eq!(
        resumed
            .recipients()
            .iter()
            .filter(|v| v.evidence.is_some())
            .count(),
        1
    );
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
    assert!(!resumed.aborted());
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn actual_s_insert_failure_retains_obtained_peer_bytes_and_resumes_without_peer_reissue() {
    use sqlx::Connection;
    let r = Rig::new().await;
    r.control.pause_peer.store(true, Ordering::SeqCst);
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let input = AcceptOrderInput::new(
        command.input().request(),
        command.input().terms_digest(),
        AllocationSelection::automatic(),
    );
    let client = r.client.clone();
    let scope = r.scope.clone();
    let waiter = tokio::spawn(async move {
        client
            .orders(&scope)
            .accept(context, minimum, private, public, input)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        r.control.entered.notified(),
    )
    .await
    .unwrap();
    // The real peer artifact already exists. Fail the actual SQL INSERT,
    // independently of Core's retained-material owner and transaction result.
    let path = r._temp.path().join("stock/stock_execution_v1.sqlite");
    let mut fault = sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER controlled_s_failure BEFORE INSERT ON stock_signing_evidence BEGIN SELECT RAISE(ABORT, 'controlled_s_failure'); END").execute(&mut fault).await.unwrap();
    r.control.release.notify_one();
    let error = waiter.await.unwrap().unwrap_err();
    assert_eq!(error.code(), crate::Code::StorageFailure);
    assert_eq!(error.context().admission(), crate::Admission::Confirmed);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    let context = r
        .scope
        .command_context(
            command.context().operation_id(),
            command.context().expected().cloned(),
            command.context().approval().cloned(),
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    let blocked = r
        .client
        .orders(&r.scope)
        .accept(
            context,
            minimum,
            private,
            public,
            AcceptOrderInput::new(
                command.input().request(),
                command.input().terms_digest(),
                AllocationSelection::automatic(),
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(blocked.code(), crate::Code::RecoveryRequired);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 1);
    sqlx::query("DROP TRIGGER controlled_s_failure")
        .execute(&mut fault)
        .await
        .unwrap();
    fault.close().await.unwrap();
    r.client.stock(&r.scope).drain_evidence().await.unwrap();
    let status = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert!(status.installation().is_some());
    assert!(status.recipients().iter().all(|v| v.evidence.is_some()));
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 0);
    let pool = r
        .native
        .pool(
            r.scope.binding().clone(),
            status.checkpoint(),
            PoolId::from_bytes([80; 32]),
            proof(r.enrollment.proof()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.snapshot().balances().physical().available.get(), 7);
    assert_eq!(
        pool.snapshot().balances().physical().preparatory_hold.get(),
        0
    );
    assert_eq!(
        pool.snapshot().balances().physical().committed_hold.get(),
        3
    );
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
}

#[tokio::test]
async fn identical_authenticated_foreign_scope_cannot_start_this_clients_acceptance() {
    let r = Rig::new().await;
    let (command, _) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let path = r._temp.path().join("other_cache");
    std::fs::create_dir(&path).unwrap();
    let options = OpenOptions::new(Paths::from_directory(path).unwrap(), OpenMode::Create)
        .with_source_generation(SourceGeneration::new([19; 32]).unwrap(), 1)
        .unwrap();
    let other = Client::open(NativeConfig::public_read(options).with_workspace(r.native.clone()))
        .await
        .unwrap();
    let foreign = other
        .attach(r.enrollment.input().clone(), proof(r.enrollment.proof()))
        .await
        .unwrap();
    assert_eq!(foreign.binding().input(), r.scope.binding().input());
    let context = foreign
        .command_context(
            command.context().operation_id(),
            command.context().expected().cloned(),
            command.context().approval().cloned(),
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    assert_eq!(context.data(), command.context());
    let error = r
        .client
        .orders(&foreign)
        .accept(
            context,
            minimum,
            private,
            public,
            AcceptOrderInput::new(
                command.input().request(),
                command.input().terms_digest(),
                AllocationSelection::automatic(),
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), crate::Code::Unauthorized);
    assert_eq!(error.context().admission(), crate::Admission::NotAdmitted);
    assert_eq!(r.minima().await.2, minimum);
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 0);
    r.client.close().await.unwrap();
    other.close().await.unwrap();
}
