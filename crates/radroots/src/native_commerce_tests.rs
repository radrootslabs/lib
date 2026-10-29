//! Real facade, SQLx and local crypto with explicitly controlled command IO.
//! Delivery cases additionally exercise explicit loopback Nostr transport.
//! Host custody and live route discovery remain outside these fixtures.
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
    command::{OperationError, PrivateEnvelopeCandidate},
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
struct Clock(AtomicU64, AtomicBool);
impl Clock {
    fn enable_realtime(&self) {
        self.1.store(true, Ordering::SeqCst);
    }
}
impl WorkspaceClock for Clock {
    fn now(&self) -> Result<UnixSeconds, WorkspaceError> {
        let seconds = if self.1.load(Ordering::SeqCst) {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| WorkspaceError::Unavailable)?
                .as_secs()
        } else {
            self.0.load(Ordering::SeqCst)
        };
        UnixSeconds::new(seconds).map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Control {
    pause_refused: AtomicBool,
    pause_rejected: AtomicBool,
    tamper_recovery: AtomicBool,
    tamper_self: AtomicBool,
    rejected: std::sync::Mutex<Option<radroots_signing::issuance::RejectedArtifact>>,
    recovery_supported: AtomicBool,
    signs: AtomicUsize,
    recoveries: AtomicUsize,
    fail_self: AtomicBool,
    fail_peer: AtomicBool,
    pause_peer: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl Control {
    fn new() -> Self {
        Self {
            pause_refused: AtomicBool::new(false),
            pause_rejected: AtomicBool::new(false),
            tamper_recovery: AtomicBool::new(false),
            tamper_self: AtomicBool::new(false),
            rejected: std::sync::Mutex::new(None),
            recovery_supported: AtomicBool::new(true),
            signs: AtomicUsize::new(0),
            recoveries: AtomicUsize::new(0),
            fail_self: AtomicBool::new(false),
            fail_peer: AtomicBool::new(false),
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
            if (is_self && self.control.fail_self.load(Ordering::SeqCst))
                || (!is_self && self.control.fail_peer.load(Ordering::SeqCst))
            {
                if self.control.pause_refused.swap(false, Ordering::SeqCst) {
                    self.control.entered.notify_one();
                    self.control.release.notified().await;
                }
                return SigningOutcome::not_dispatched(intent.binding(), ProviderError::Cancelled);
            }
            let result = self.inner.sign_exact(intent).await;
            if is_self && self.control.tamper_self.load(Ordering::SeqCst) {
                let foreign = radroots_signing::issuance::IntentBinding::from_components(
                    OperationId::from_bytes([0xa1; 32]),
                    [0xa2; 32],
                    [0xa3; 32],
                    radroots_signing::issuance::ArtifactRole::BusinessSeal,
                    Some(self.inner.public_key()),
                    NonZeroU64::new(230000).unwrap(),
                )
                .unwrap();
                let rejected = result.enforce_binding(foreign);
                let radroots_signing::issuance::OutcomeKind::Rejected(metadata) = rejected.kind()
                else {
                    panic!("actual artifact must retain bounded mismatch metadata")
                };
                *self.control.rejected.lock().unwrap() = Some(*metadata);
                if self.control.pause_rejected.swap(false, Ordering::SeqCst) {
                    self.control.entered.notify_one();
                    self.control.release.notified().await;
                }
                return rejected;
            }
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
            let result = self.inner.recover_exact(request).await;
            if self.control.tamper_recovery.load(Ordering::SeqCst) {
                let foreign = radroots_signing::issuance::IntentBinding::from_components(
                    OperationId::from_bytes([0xa1; 32]),
                    [0xa2; 32],
                    [0xa3; 32],
                    radroots_signing::issuance::ArtifactRole::BusinessSeal,
                    Some(self.inner.public_key()),
                    NonZeroU64::new(230000).unwrap(),
                )
                .unwrap();
                let rejected = result.enforce_binding(foreign);
                let radroots_signing::issuance::ExactRecoveryKind::Rejected(metadata) =
                    rejected.kind()
                else {
                    panic!("actual recovered artifact must retain bounded mismatch metadata")
                };
                *self.control.rejected.lock().unwrap() = Some(*metadata);
                return rejected;
            }
            result
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
struct Io {
    clock: Arc<Clock>,
    next: AtomicUsize,
    wraps: AtomicUsize,
    sent: std::sync::Mutex<Vec<String>>,
    missing_ack: AtomicBool,
    pause_delivery: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    route_override: Option<Vec<String>>,
    transport: std::sync::Mutex<Option<Arc<radroots_transport_nostr::NostrTransport>>>,
}
impl Io {
    fn new(clock: Arc<Clock>) -> Self {
        Self {
            clock,
            next: AtomicUsize::new(1),
            wraps: AtomicUsize::new(0),
            sent: std::sync::Mutex::new(Vec::new()),
            missing_ack: AtomicBool::new(false),
            pause_delivery: AtomicBool::new(false),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            route_override: None,
            transport: std::sync::Mutex::new(None),
        }
    }
    fn selected(&self) -> TargetSet {
        self.route_override.as_ref().map_or_else(targets, |uris| {
            TargetSet::new(
                uris.iter()
                    .map(|uri| Target::nostr_relay(uri).unwrap())
                    .collect(),
            )
            .unwrap()
        })
    }
}
fn targets() -> TargetSet {
    TargetSet::new(vec![Target::nostr_relay("wss://relay.example").unwrap()]).unwrap()
}
impl CommandIo for Io {
    fn enabled(&self) -> bool {
        true
    }
    fn now(&self) -> Result<UnixSeconds, OperationError> {
        self.clock.now().map_err(Into::into)
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
            if let Some(uris) = &self.route_override {
                use nostr::JsonUtil;
                let keys = [2, 3]
                    .into_iter()
                    .map(|scalar| {
                        nostr::Keys::new(
                            nostr::SecretKey::from_hex(&format!("{scalar:064x}")).unwrap(),
                        )
                    })
                    .find(|key| key.public_key().to_hex() == recipient.to_hex())
                    .unwrap();
                let raw = nostr::EventBuilder::new(nostr::Kind::Custom(10050), "")
                    .tags(
                        uris.iter()
                            .map(|uri| nostr::Tag::parse(["relay", uri.as_str()]).unwrap()),
                    )
                    .custom_created_at(nostr::Timestamp::from_secs(100))
                    .sign_with_keys(&keys)
                    .unwrap()
                    .as_json();
                return capture_dm_route(recipient, [raw.as_str()], &self.selected());
            }
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
        selected_dm_targets(route, &self.selected())
    }
    fn publication_policy(&self) -> radroots_transport::policy::SatisfactionPolicy {
        use radroots_transport::policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy};
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all())
    }
    fn private_envelope(
        &self,
        recipient: PublicKey,
        raw: &str,
    ) -> Result<PrivateEnvelopeCandidate, OperationError> {
        self.wraps.fetch_add(1, Ordering::SeqCst);
        let wrapped = radroots_nostr::signing::gift_wrap::wrap_retained_seal(
            raw,
            recipient,
            NonZeroU64::new(self.now()?.get()).unwrap(),
            &radroots_nostr::signing::OsNonceSource,
        )
        .map_err(|_| OperationError::InvalidEvidence)?;
        wrapped.with_wrapping_key(|key| {
            PrivateEnvelopeCandidate::from_obtained(recipient, wrapped.signed_json(), key)
        })
    }
    fn delivery_id(&self) -> Result<String, OperationError> {
        Ok(format!("{:064x}", self.next.fetch_add(1, Ordering::SeqCst)))
    }
    fn publish(
        &self,
        request: radroots_transport::sink::DeliveryRequest,
        selected: TargetSet,
    ) -> HostFuture<'_, Result<radroots_transport::DeliveryReceipt, radroots_transport::SinkFailure>>
    {
        Box::pin(async move {
            self.sent
                .lock()
                .unwrap()
                .push(request.payload().event().raw_json().to_owned());
            if self.pause_delivery.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            let transport = self.transport.lock().unwrap().clone();
            if let Some(transport) = transport {
                use radroots_transport::EventSink;
                return transport.deliver_selected(request, selected).await;
            }
            use radroots_transport::{
                DeliveryReceipt, outcome::DeliveryOutcome, sink::DeliveryTargetReceipt,
            };
            let outcome = if self.missing_ack.load(Ordering::SeqCst) {
                DeliveryOutcome::unavailable()
            } else {
                DeliveryOutcome::accepted()
            };
            Ok(DeliveryReceipt::for_request(
                &request,
                selected
                    .targets()
                    .iter()
                    .map(|target| DeliveryTargetReceipt::attempted(target.clone(), outcome.clone()))
                    .collect(),
            )
            .unwrap())
        })
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
    io: Arc<Io>,
}
impl Rig {
    async fn new() -> Self {
        Self::configured(None, 300).await
    }
    async fn configured(route: Option<Vec<String>>, expiry: u64) -> Self {
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
        let clock = Arc::new(Clock(AtomicU64::new(150), AtomicBool::new(false)));
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
                UnixSeconds::new(expiry).unwrap(),
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
        let mut io = Io::new(clock.clone());
        io.route_override = route;
        let io = Arc::new(io);
        client.session = client
            .session
            .clone()
            .with_command_runtime(Arc::new(CommandRuntime::new(io.clone())));
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
            io,
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
    assert_eq!(r.io.sent.lock().unwrap().len(), 2);
    let (_, _, minimum) = r.minima().await;
    let observed = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(observed.installation(), status.installation());
    // Actual protected stock delivery uses controlled transport outcomes here.
    // S remains independently readable through the same native owner.
    assert!(observed.delivery().unwrap().iter().all(|value| {
        value.checkpoint() == observed.checkpoint()
            && value.source().is_some()
            && value.attempt_count() == 1
            && value.acceptance_satisfied().unwrap()
    }));
    let evidence = observed.recipients()[0].evidence.unwrap();
    let source = radroots_execution::session::commerce::AcceptanceStore::signing_evidence(
        &crate::native_commerce::NativeCommerce(Arc::new(r.native.clone())),
        radroots_execution::session::commerce::AcceptanceSelection {
            binding: r.scope.binding().clone(),
            minimum,
            operation: command.context().operation_id(),
            digest: command.digest(),
        },
        evidence,
        r.enrollment.proof(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(source.receipt(), evidence);
    source.with_json(|raw| {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(raw).unwrap()["kind"],
            13
        )
    });
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

#[path = "native_stock_delivery_tests.rs"]
mod delivery;
#[path = "native_refusal_tests.rs"]
mod original_refusals;

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
    assert_eq!(r.io.sent.lock().unwrap().len(), 1);
    for (delivery, recipient) in status.delivery().unwrap().iter().zip(status.recipients()) {
        assert_eq!(
            delivery.attempt_count(),
            u16::from(recipient.evidence.is_some())
        );
        assert_eq!(
            delivery.acceptance_satisfied().unwrap(),
            recipient.evidence.is_some()
        );
    }
    let original_refusal = status
        .recipients()
        .iter()
        .find_map(|v| v.original_unissued)
        .expect("actual original refusal must be captured");
    assert_eq!(original_refusal.reason, ProviderError::Cancelled);
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
    assert_eq!(
        resumed
            .recipients()
            .iter()
            .find_map(|v| v.original_unissued),
        Some(original_refusal)
    );
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

fn assert_rejected(error: crate::ExecutionFailure, operation: OperationId, control: &Control) {
    assert_eq!(error.code(), crate::Code::SignerTampering);
    assert_eq!(error.context().operation_id(), Some(operation));
    assert_eq!(error.context().admission(), crate::Admission::Confirmed);
    assert_eq!(error.retry(), crate::RetryClass::ManualReconciliation);
    let rejected = error
        .signer_rejection()
        .expect("preserve the actual bounded rejection");
    assert_eq!(
        rejected,
        *control.rejected.lock().unwrap().as_ref().unwrap()
    );
    let fingerprint = rejected
        .fingerprint()
        .expect("actual returned bytes have fingerprint coverage");
    assert!(fingerprint.covered_bytes() > 0);
    assert_eq!(fingerprint.covered_bytes(), fingerprint.total_bytes());
    assert!(fingerprint.covered_bytes() <= 65536);
}

#[tokio::test]
async fn rejected_self_callback_returns_bounded_tampering_without_losing_peer_installation() {
    let r = Rig::new().await;
    r.control.tamper_self.store(true, Ordering::SeqCst);
    let (command, context) = r.captured().await;
    let (private, public, minimum) = r.minima().await;
    let error = r
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
    assert_rejected(error, command.context().operation_id(), &r.control);
    let status = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    let installed = status
        .installation()
        .expect("actual peer S installs before self rejection");
    assert_eq!(
        status
            .recipients()
            .iter()
            .filter(|v| v.evidence.is_some())
            .count(),
        1
    );
    assert!(!status.aborted());
    assert_eq!(r.io.sent.lock().unwrap().len(), 1);
    for (delivery, recipient) in status.delivery().unwrap().iter().zip(status.recipients()) {
        assert_eq!(
            delivery.attempt_count(),
            u16::from(recipient.evidence.is_some())
        );
        assert_eq!(
            delivery.acceptance_satisfied().unwrap(),
            recipient.evidence.is_some()
        );
    }
    r.control.tamper_self.store(false, Ordering::SeqCst);
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.installation(), Some(installed));
    assert!(resumed.recipients().iter().all(|v| v.evidence.is_some()));
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 1);
    r.client.close().await.unwrap();
    assert!(r.provider.upgrade().is_none());
}

#[tokio::test]
async fn generic_inquiry_installs_actual_peer_and_returns_original_bounded_self_rejection() {
    for fail_install in [false, true] {
        let r = Rig::new().await;
        r.control
            .pause_rejected
            .store(fail_install, Ordering::SeqCst);
        r.control.tamper_self.store(true, Ordering::SeqCst);
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
        let key = r.enrollment.input().participant();
        let (native, input, credential) = NativeWorkspace::initialize_local_outbox(
            &r._temp.path().join("chat"),
            radroots_storage_sqlite::workspace::LocalOutboxConfig::new(
                key,
                key,
                "synthetic-rejected-chat",
                UnixSeconds::new(300).unwrap(),
            )
            .unwrap(),
            crypto,
            r.clock.clone(),
        )
        .await
        .unwrap();
        let path = r._temp.path().join("chat_cache");
        std::fs::create_dir(&path).unwrap();
        let options = OpenOptions::new(Paths::from_directory(path).unwrap(), OpenMode::Create)
            .with_source_generation(SourceGeneration::new([20; 32]).unwrap(), 1)
            .unwrap();
        let mut client = Client::open(NativeConfig::public_read(options).with_workspace(native))
            .await
            .unwrap();
        client.session =
            client
                .session
                .clone()
                .with_command_runtime(Arc::new(CommandRuntime::new(Arc::new(Io::new(
                    r.clock.clone(),
                )))));
        let scope = client.attach(input, proof(&credential)).await.unwrap();
        client
            .attach_signer(SignerHandle::new(
                r.provider.upgrade().unwrap(),
                Arc::new(Callbacks),
            ))
            .await
            .unwrap();
        client.start(StartMode::LocalOnly).await.unwrap();
        let operation = OperationId::from_bytes([95; 32]);
        let context = scope
            .command_context(operation, None, None, UnixSeconds::new(230).unwrap())
            .unwrap();
        let peer = data()["scope"]["buyer"].as_str().unwrap().parse().unwrap();
        let input = crate::inbox::InquireInput::new(
            radroots_domain::ThreadId::from_bytes([96; 32]),
            peer,
            None,
            "synthetic private inquiry",
        )
        .unwrap();
        let observer_client = client.clone();
        let observer =
            tokio::spawn(async move { observer_client.inbox().inquire(context, input).await });
        let mut injected = None;
        if fail_install {
            use sqlx::Connection;
            tokio::time::timeout(
                std::time::Duration::from_secs(20),
                r.control.entered.notified(),
            )
            .await
            .unwrap();
            let path = r._temp.path().join("chat/execution_v1.sqlite");
            let mut connection =
                sqlx::SqliteConnection::connect(&format!("sqlite://{}", path.display()))
                    .await
                    .unwrap();
            sqlx::query("CREATE TRIGGER controlled_t2_failure BEFORE INSERT ON chat_messages BEGIN SELECT RAISE(ABORT, 'controlled_t2_failure'); END").execute(&mut connection).await.unwrap();
            injected = Some(connection);
            r.control.release.notify_one();
        }
        let error = observer.await.unwrap().unwrap_err();
        if fail_install {
            assert_eq!(error.code(), crate::Code::StorageFailure);
            assert_eq!(error.context().operation_id(), Some(operation));
            assert_eq!(error.context().admission(), crate::Admission::Confirmed);
            assert_eq!(error.retry(), crate::RetryClass::ManualReconciliation);
            assert_eq!(
                error.signer_rejection(),
                *r.control.rejected.lock().unwrap()
            );
        } else {
            assert_rejected(error, operation, &r.control)
        }
        if let Some(mut connection) = injected {
            use sqlx::Connection;
            sqlx::query("DROP TRIGGER controlled_t2_failure")
                .execute(&mut connection)
                .await
                .unwrap();
            connection.close().await.unwrap();
        }
        let status = client
            .operations(&scope)
            .get(operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            status.business_installation(),
            if fail_install {
                crate::BusinessInstallation::NotInstalled
            } else {
                crate::BusinessInstallation::Installed
            }
        );
        assert_eq!(
            status
                .artifacts()
                .iter()
                .filter(|v| v.issuance() == crate::Issuance::Obtained)
                .count(),
            1
        );
        assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
        client.close().await.unwrap();
        r.client.close().await.unwrap();
        assert!(r.provider.upgrade().is_none());
    }
}

#[tokio::test]
async fn rejected_exact_recovery_keeps_original_binding_and_cannot_release_installed_peer() {
    let r = Rig::new().await;
    r.control.fail_self.store(true, Ordering::SeqCst);
    let (command, status) = r.accept().await;
    let installed = status.installation().unwrap();
    r.control.fail_self.store(false, Ordering::SeqCst);
    r.control.tamper_recovery.store(true, Ordering::SeqCst);
    let (_, _, minimum) = r.minima().await;
    let error = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap_err();
    assert_rejected(error, command.context().operation_id(), &r.control);
    let current = r
        .client
        .orders(&r.scope)
        .acceptance_status(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(current.installation(), Some(installed));
    assert_eq!(
        current
            .recipients()
            .iter()
            .filter(|v| v.evidence.is_some())
            .count(),
        1
    );
    assert!(!current.aborted());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 2);
    assert_eq!(r.control.recoveries.load(Ordering::SeqCst), 1);
    r.client.close().await.unwrap();
}
