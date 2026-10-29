mod dispatch;
use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceCrypto,
    WorkspaceError,
    admission::Journal,
    fs::{Kind, Namespace},
    record::GrantRecord,
    schema, stock,
};
use radroots_domain::{CommandDigest, OperationId};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, ArtifactEvidence, BoundSigningPlan, CapturedArtifact,
    CapturedSigningPlan, ClaimAttemptOutcome, ClaimAttemptRequest, CommitAttempt, DeliveryAttempt,
    DeliveryAttemptOutcome, DeliveryAttemptRequest, DeliveryObservation,
    DeliveryObservationOutcome, EnvelopeCandidate, EvidenceCaptureOutcome, InstallationOutcome,
    InstallationRequest, OperationError, OperationSnapshot, PlanBindingOutcome, PreparedOperation,
    SigningAttempt, SigningTarget,
};
use radroots_transport::sink::DeliveryRequestId;
use sqlx::{
    Connection, Row, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::{oneshot, watch};

// Count conservative retained input bounds across queued and in-flight jobs.
// Plan jobs retain both their canonical bytes and bounded decoded fields.
const QUEUE_MAX_BYTES: usize = 4 * 1024 * 1024;
const QUEUE_CAPACITY: usize = 32;
type Reply<T, E = WorkspaceError> = oneshot::Sender<Result<T, E>>;
enum Job {
    InstallStockAgreement(
        Box<stock::StockInstallationRequest>,
        AttachmentProof,
        Reply<stock::StockInstallationOutcome, OperationError>,
    ),
    StockAgreementInstallation(
        Box<stock::StockInstallationRequest>,
        AttachmentProof,
        Reply<Option<stock::StockInstallationReceipt>, OperationError>,
    ),
    AbortStockPreparation(
        Box<stock::StockAbortRequest>,
        AttachmentProof,
        Reply<stock::StockAbortOutcome, OperationError>,
    ),
    StockPreparationAbort(
        Box<stock::StockAbortRequest>,
        AttachmentProof,
        Reply<Option<stock::StockAbortReceipt>, OperationError>,
    ),
    StockSnapshot(
        Box<stock::StockSnapshotRequest>,
        AttachmentProof,
        Reply<stock::StockOperationSnapshot, OperationError>,
    ),
    StockEvidencePage(
        Box<stock::StockClaimRequest>,
        Option<radroots_domain::Sequence>,
        AttachmentProof,
        Reply<stock::StockEvidencePage, OperationError>,
    ),
    RecoverStockEvidence(
        Box<radroots_storage::inventory::StockEvidenceRequest>,
        AttachmentProof,
        Reply<stock::StockEvidenceOutcome, OperationError>,
    ),
    CaptureStockUnissued(
        Box<stock::issuance::unissued::Request>,
        AttachmentProof,
        Reply<stock::StockUnissuedOutcome, OperationError>,
    ),
    RecoverStockUnissued(
        Box<stock::issuance::unissued::Request>,
        AttachmentProof,
        Reply<stock::StockUnissuedOutcome, OperationError>,
    ),
    StockUnissued(
        Box<stock::StockClaimRequest>,
        AttachmentProof,
        Reply<Option<stock::StockUnissuedReceipt>, OperationError>,
    ),
    PrepareStockDelivery(
        Box<radroots_storage::inventory::StockDeliveryRequest>,
        AttachmentProof,
        Reply<radroots_storage::inventory::StockDeliveryAttemptOutcome, OperationError>,
    ),
    StockDeliveryAttempt(
        Box<radroots_storage::inventory::StockEnvelopeRequest>,
        radroots_transport::sink::DeliveryRequestId,
        AttachmentProof,
        Reply<Option<radroots_storage::inventory::StockDeliveryAttempt>, OperationError>,
    ),
    StockDeliverySummary(
        Box<radroots_storage::inventory::StockEnvelopeRequest>,
        AttachmentProof,
        Reply<radroots_storage::inventory::StockDeliverySummary, OperationError>,
    ),
    RecordStockDeliveryObservation(
        Box<WorkspaceBinding>,
        AttachmentProof,
        Box<radroots_storage::inventory::StockDeliveryObservation>,
        Reply<radroots_storage::inventory::StockDeliveryObservationOutcome, OperationError>,
    ),
    StockDeliveryObservations(
        Box<radroots_storage::inventory::StockEnvelopeRequest>,
        Option<radroots_domain::Sequence>,
        AttachmentProof,
        Reply<radroots_storage::inventory::StockDeliveryObservationPage, OperationError>,
    ),
    RecoverStockEnvelope(
        stock::envelope::CaptureRequest,
        AttachmentProof,
        Box<radroots_storage::command::PrivateEnvelopeCandidate>,
        Reply<radroots_storage::inventory::StockEnvelopeOutcome, OperationError>,
    ),
    RecoverStockObservation(
        Box<WorkspaceBinding>,
        AttachmentProof,
        Box<radroots_storage::inventory::StockDeliveryObservation>,
        Reply<radroots_storage::inventory::StockDeliveryObservationOutcome, OperationError>,
    ),
    CaptureStockEnvelope(
        stock::envelope::CaptureRequest,
        AttachmentProof,
        Box<radroots_storage::command::PrivateEnvelopeCandidate>,
        Reply<radroots_storage::inventory::StockEnvelopeOutcome, OperationError>,
    ),
    StockEnvelope(
        Box<radroots_storage::inventory::StockEnvelopeRequest>,
        AttachmentProof,
        Reply<Option<radroots_storage::inventory::StockEnvelopeSource>, OperationError>,
    ),
    PrepareStockRewrap(
        Box<radroots_storage::inventory::StockRewrapRequest>,
        AttachmentProof,
        Reply<radroots_storage::inventory::StockRewrapOutcome, OperationError>,
    ),
    StockRewrapAttempt(
        Box<radroots_storage::inventory::StockRewrapRequest>,
        AttachmentProof,
        Reply<Option<radroots_storage::inventory::StockRewrapState>, OperationError>,
    ),
    RecoverStockRewrapAdmission(
        Box<radroots_storage::inventory::StockRewrapRequest>,
        AttachmentProof,
        Reply<(), OperationError>,
    ),
    StockRewraps(
        Box<radroots_storage::inventory::StockEnvelopeRequest>,
        Option<radroots_domain::Sequence>,
        AttachmentProof,
        Reply<radroots_storage::inventory::StockRewrapPage, OperationError>,
    ),
    CaptureStockEvidence(
        Box<radroots_storage::inventory::StockEvidenceRequest>,
        AttachmentProof,
        Reply<stock::StockEvidenceOutcome, OperationError>,
    ),
    StockEvidence(
        Box<stock::StockClaimRequest>,
        [u8; 32],
        AttachmentProof,
        Reply<Option<stock::StockCapturedEvidence>, OperationError>,
    ),
    ClaimStockSigning(
        Box<stock::StockClaimRequest>,
        AttachmentProof,
        Reply<stock::StockClaimOutcome, OperationError>,
    ),
    AuthorizeStockExactReplay(
        Box<stock::StockClaimRequest>,
        AttachmentProof,
        Reply<stock::StockExactReplayPermit, OperationError>,
    ),
    StockSigningAttempt(
        Box<stock::StockClaimRequest>,
        AttachmentProof,
        Reply<Option<stock::StockSigningAttempt>, OperationError>,
    ),
    BindStockSigningPlan(
        Box<stock::StockSigningPlanRequest>,
        AttachmentProof,
        Reply<stock::StockSigningPlanOutcome, OperationError>,
    ),
    StockSigningPlan(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        OperationId,
        CommandDigest,
        AttachmentProof,
        Reply<Option<stock::StockBoundSigningPlan>, OperationError>,
    ),
    PrepareStockAcceptance(
        Box<stock::StockAcceptanceRequest>,
        AttachmentProof,
        Reply<stock::StockAcceptanceOutcome, OperationError>,
    ),
    StockOrderRequest(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        radroots_domain::RecordId,
        AttachmentProof,
        Reply<stock::StockRequestSnapshot, OperationError>,
    ),
    StockOrderHistory(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        radroots_domain::causal::CausalScope,
        AttachmentProof,
        Reply<stock::StockHistorySnapshot, OperationError>,
    ),
    RetainStockListing(
        Box<WorkspaceBinding>,
        radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        Box<stock::StockListingCapture>,
        AttachmentProof,
        Reply<stock::StockListingOutcome, OperationError>,
    ),
    StockListing(
        Box<WorkspaceBinding>,
        radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        stock::public_source::Lookup,
        AttachmentProof,
        Reply<Option<stock::StockListingSnapshot>, OperationError>,
    ),
    IssueStockConsent(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        Box<stock::StockConsentCapture>,
        AttachmentProof,
        Reply<stock::StockConsentOutcome, OperationError>,
    ),
    StockConsent(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        OperationId,
        radroots_domain::GrantId,
        AttachmentProof,
        Reply<Option<stock::StockConsentReceipt>, OperationError>,
    ),
    RetainStockSource(
        Box<WorkspaceBinding>,
        radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        Box<stock::StockSourceCapture>,
        AttachmentProof,
        Reply<stock::StockSourceOutcome, OperationError>,
    ),
    StockSource(
        Box<WorkspaceBinding>,
        radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        radroots_domain::EnvelopeEventId,
        AttachmentProof,
        Reply<Option<stock::StockSourceSnapshot>, OperationError>,
    ),
    OptionMapping(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        radroots_domain::stock::OptionMappingKey,
        AttachmentProof,
        Reply<Option<radroots_domain::stock::OptionPoolMapping>, OperationError>,
    ),
    ChangeStockGrant(
        Box<stock::grants::Change>,
        AttachmentProof,
        Reply<stock::StockGrantOutcome>,
    ),
    ExecuteInventory(
        Box<radroots_storage::inventory::InventoryRequest>,
        AttachmentProof,
        Reply<radroots_storage::inventory::InventoryOutcome, OperationError>,
    ),
    InventoryOperation(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        OperationId,
        Option<CommandDigest>,
        AttachmentProof,
        Reply<Option<radroots_storage::inventory::InventoryReceipt>, OperationError>,
    ),
    Pool(
        Box<WorkspaceBinding>,
        radroots_storage::inventory::InventoryCheckpointRef,
        radroots_domain::PoolId,
        AttachmentProof,
        Reply<Option<radroots_domain::stock::PoolView>, OperationError>,
    ),
    StockAuthority(
        AttachmentInput,
        AttachmentProof,
        Reply<super::StockAuthorityReceipt>,
    ),
    CaptureDeliveryObservation(
        Box<WorkspaceBinding>,
        AttachmentProof,
        Box<DeliveryObservation>,
        Reply<DeliveryObservationOutcome, OperationError>,
    ),
    QueryDeliveryObservations(
        Box<WorkspaceBinding>,
        OperationId,
        CommandDigest,
        SigningTarget,
        DeliveryRequestId,
        AttachmentProof,
        Reply<Vec<DeliveryObservation>, OperationError>,
    ),
    PrepareDelivery(
        Box<DeliveryAttemptRequest>,
        AttachmentProof,
        Option<Box<EnvelopeCandidate>>,
        Reply<DeliveryAttemptOutcome, OperationError>,
    ),
    QueryDelivery(
        Box<WorkspaceBinding>,
        OperationId,
        CommandDigest,
        SigningTarget,
        DeliveryRequestId,
        AttachmentProof,
        Reply<Option<DeliveryAttempt>, OperationError>,
    ),
    Install(
        Box<InstallationRequest>,
        AttachmentProof,
        Reply<InstallationOutcome, OperationError>,
    ),
    Snapshot(
        Box<WorkspaceBinding>,
        OperationId,
        Option<CommandDigest>,
        AttachmentProof,
        Reply<Option<OperationSnapshot>, OperationError>,
    ),
    Claim(
        Box<ClaimAttemptRequest>,
        AttachmentProof,
        Reply<ClaimAttemptOutcome, OperationError>,
    ),
    Attempt(
        Box<ClaimAttemptRequest>,
        AttachmentProof,
        Reply<Option<SigningAttempt>, OperationError>,
    ),
    Capture(
        Box<AttachmentInput>,
        AttachmentProof,
        Box<ArtifactEvidence>,
        Reply<EvidenceCaptureOutcome, OperationError>,
    ),
    RecoverCapture(
        Box<AttachmentInput>,
        AttachmentProof,
        Box<ArtifactEvidence>,
        Reply<EvidenceCaptureOutcome, OperationError>,
    ),
    Captured(
        Box<ClaimAttemptRequest>,
        AttachmentProof,
        Reply<Option<CapturedArtifact>, OperationError>,
    ),
    BindPlan(
        Box<WorkspaceBinding>,
        CapturedSigningPlan,
        AttachmentProof,
        Reply<PlanBindingOutcome, OperationError>,
    ),
    SigningPlan(
        Box<WorkspaceBinding>,
        OperationId,
        CommandDigest,
        AttachmentProof,
        Reply<Option<BoundSigningPlan>, OperationError>,
    ),
    Authenticate(AttachmentInput, AttachmentProof, Reply<WorkspaceBinding>),
    Revoke(AttachmentInput, AttachmentProof, Reply<()>),
    Admit(
        Box<AdmissionRequest>,
        AttachmentProof,
        Reply<AdmissionOutcome, OperationError>,
    ),
    Operation(
        Box<WorkspaceBinding>,
        OperationId,
        Option<CommandDigest>,
        AttachmentProof,
        Reply<Option<PreparedOperation>, OperationError>,
    ),
}
impl Job {
    fn reject_recovery(self) {
        match self {
            Self::InstallStockAgreement(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockAgreementInstallation(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::AbortStockPreparation(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockPreparationAbort(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockSnapshot(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockEvidencePage(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::CaptureStockUnissued(_, _, reply) | Self::RecoverStockUnissued(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockUnissued(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::CaptureStockEvidence(_, _, reply) | Self::RecoverStockEvidence(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::PrepareStockDelivery(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockDeliveryAttempt(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockDeliverySummary(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RecordStockDeliveryObservation(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockDeliveryObservations(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RecoverStockEnvelope(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RecoverStockObservation(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::CaptureStockEnvelope(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockEnvelope(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::PrepareStockRewrap(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockRewrapAttempt(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RecoverStockRewrapAdmission(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockRewraps(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockEvidence(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::ClaimStockSigning(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::AuthorizeStockExactReplay(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockSigningAttempt(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::BindStockSigningPlan(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockSigningPlan(_, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::PrepareStockAcceptance(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockOrderRequest(_, _, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockOrderHistory(_, _, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RetainStockListing(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockListing(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::IssueStockConsent(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockConsent(_, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::RetainStockSource(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockSource(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::OptionMapping(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::ChangeStockGrant(_, _, reply) => {
                let _ = reply.send(Err(WorkspaceError::RecoveryRequired));
            }
            Self::ExecuteInventory(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::InventoryOperation(_, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Pool(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::StockAuthority(_, _, reply) => {
                let _ = reply.send(Err(WorkspaceError::RecoveryRequired));
            }
            Self::CaptureDeliveryObservation(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::QueryDeliveryObservations(_, _, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::PrepareDelivery(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::QueryDelivery(_, _, _, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Install(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Snapshot(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Claim(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Attempt(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Capture(_, _, _, reply) | Self::RecoverCapture(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Captured(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::BindPlan(_, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::SigningPlan(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Authenticate(_, _, reply) => {
                let _ = reply.send(Err(WorkspaceError::RecoveryRequired));
            }
            Self::Revoke(_, _, reply) => {
                let _ = reply.send(Err(WorkspaceError::RecoveryRequired));
            }
            Self::Admit(_, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
            Self::Operation(_, _, _, _, reply) => {
                let _ = reply.send(Err(OperationError::RecoveryRequired));
            }
        }
    }
}
struct Budget {
    used: Arc<AtomicUsize>,
    bytes: usize,
}
impl Drop for Budget {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
struct Queued {
    job: Job,
    _budget: Budget,
}
pub(super) struct Handle {
    sender: Mutex<Option<SyncSender<Queued>>>,
    queued_bytes: Arc<AtomicUsize>,
    completed: watch::Receiver<Option<Result<(), WorkspaceError>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl Handle {
    pub(super) async fn launch(
        path: PathBuf,
        input: AttachmentInput,
        initial: Option<GrantRecord>,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        Self::launch_kind(path, input, initial, None, Kind::Outbox, crypto, clock).await
    }
    pub(super) async fn launch_stock(
        path: PathBuf,
        input: AttachmentInput,
        initial: Option<(
            GrantRecord,
            radroots_domain::TransactionId,
            radroots_domain::UnixSeconds,
        )>,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let initial = initial.map(|(grant, transaction, expires_at)| stock::Initial {
            grant,
            transaction,
            expires_at,
            clock: clock.clone(),
        });
        Self::launch_kind(path, input, None, initial, Kind::SellerStock, crypto, clock).await
    }
    async fn launch_kind(
        path: PathBuf,
        input: AttachmentInput,
        initial: Option<GrantRecord>,
        stock_initial: Option<stock::Initial>,
        kind: Kind,
        crypto: WorkspaceCrypto,
        clock: Arc<dyn WorkspaceClock>,
    ) -> Result<Self, WorkspaceError> {
        let creating = initial.is_some() || stock_initial.is_some();
        let (sender, requests) = mpsc::sync_channel::<Queued>(QUEUE_CAPACITY);
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let (ready, started) = oneshot::channel();
        let (completion, completed) = watch::channel(None);
        let thread = std::thread::Builder::new()
            .name("radroots_workspace".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = ready.send(Err(WorkspaceError::Unavailable));
                        let _ = completion.send(Some(Err(WorkspaceError::Unavailable)));
                        return;
                    }
                };
                // Construction, every request and explicit close complete on this
                // owner even when an async observer disappears between await points.
                let namespace = match Namespace::acquire_kind(&path, creating, kind) {
                    Ok(namespace) => namespace,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        let _ = completion.send(Some(Err(error)));
                        return;
                    }
                };
                let result = runtime.block_on(Box::pin(async {
                    if !creating {
                        let preflight = preflight_existing(&namespace, &input, &crypto).await;
                        if let Err(error) = preflight {
                            let _ = ready.send(Err(error));
                            return Err(error);
                        }
                    }
                    let options = SqliteConnectOptions::new()
                        .filename(namespace.database_path())
                        .create_if_missing(false)
                        .journal_mode(SqliteJournalMode::Wal)
                        .synchronous(SqliteSynchronous::Full)
                        .foreign_keys(true)
                        .busy_timeout(Duration::from_secs(5));
                    let mut connection = match SqliteConnection::connect_with(&options).await {
                        Ok(connection) => connection,
                        Err(_) => {
                            let _ = ready.send(Err(WorkspaceError::StorageFailure));
                            return Err(WorkspaceError::StorageFailure);
                        }
                    };
                    let initialized = match kind {
                        Kind::Outbox => {
                            initialize(&mut connection, &namespace, &input, initial, &crypto).await
                        }
                        Kind::SellerStock => {
                            stock::initialize(
                                &mut connection,
                                &namespace,
                                &input,
                                stock_initial,
                                &crypto,
                            )
                            .await
                        }
                    };
                    if let Err(error) = initialized {
                        let _ = ready.send(Err(error));
                        // Do not release namespace authority at the first failure;
                        // first await the actual connection's close completion.
                        let _ = connection.close().await;
                        return Err(error);
                    }
                    let mut uncertain = false;
                    let mut cached_grant = None;
                    let mut connection = Some(connection);
                    if ready.send(Ok(())).is_ok() {
                        while let Ok(Queued { job, _budget }) = requests.recv() {
                            if kind == Kind::SellerStock
                                && !matches!(
                                    &job,
                                    Job::Authenticate(..)
                                        | Job::RetainStockListing(..)
                                        | Job::StockListing(..)
                                        | Job::IssueStockConsent(..)
                                        | Job::StockConsent(..)
                                        | Job::ChangeStockGrant(..)
                                        | Job::ExecuteInventory(..)
                                        | Job::InventoryOperation(..)
                                        | Job::Pool(..)
                                        | Job::OptionMapping(..)
                                        | Job::RetainStockSource(..)
                                        | Job::StockSource(..)
                                        | Job::StockOrderHistory(..)
                                        | Job::StockOrderRequest(..)
                                        | Job::PrepareStockAcceptance(..)
                                        | Job::BindStockSigningPlan(..)
                                        | Job::StockSigningPlan(..)
                                        | Job::ClaimStockSigning(..)
                                        | Job::StockSigningAttempt(..)
                                        | Job::AuthorizeStockExactReplay(..)
                                        | Job::CaptureStockUnissued(..)
                                        | Job::RecoverStockUnissued(..)
                                        | Job::StockUnissued(..)
                                        | Job::PrepareStockDelivery(..)
                                        | Job::StockDeliveryAttempt(..)
                                        | Job::StockDeliverySummary(..)
                                        | Job::RecordStockDeliveryObservation(..)
                                        | Job::StockDeliveryObservations(..)
                                        | Job::RecoverStockEnvelope(..)
                                        | Job::RecoverStockObservation(..)
                                        | Job::CaptureStockEnvelope(..)
                                        | Job::StockEnvelope(..)
                                        | Job::PrepareStockRewrap(..)
                                        | Job::StockRewrapAttempt(..)
                                        | Job::RecoverStockRewrapAdmission(..)
                                        | Job::StockRewraps(..)
                                        | Job::CaptureStockEvidence(..)
                                        | Job::RecoverStockEvidence(..)
                                        | Job::StockEvidence(..)
                                        | Job::StockEvidencePage(..)
                                        | Job::StockSnapshot(..)
                                        | Job::InstallStockAgreement(..)
                                        | Job::StockAgreementInstallation(..)
                                        | Job::AbortStockPreparation(..)
                                        | Job::StockPreparationAbort(..)
                                        | Job::Revoke(..)
                                        | Job::StockAuthority(..)
                                )
                            {
                                job.reject_recovery();
                                continue;
                            }
                            let job = match job {
                                Job::RecoverStockRewrapAdmission(request, proof, reply) => {
                                    let result = Box::pin(
                                        super::recovery::RecoveryTurn {
                                            connection: &mut connection,
                                            cached_grant: &mut cached_grant,
                                            namespace: &namespace,
                                            selected: &input,
                                            crypto: &crypto,
                                            clock: clock.as_ref(),
                                            uncertain: &mut uncertain,
                                            options: &options,
                                        }
                                        .reconcile_rewrap_admission(*request, proof),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                Job::RecoverStockEnvelope(request, proof, candidate, reply) => {
                                    let result = Box::pin(
                                        super::recovery::RecoveryTurn {
                                            connection: &mut connection,
                                            cached_grant: &mut cached_grant,
                                            namespace: &namespace,
                                            selected: &input,
                                            crypto: &crypto,
                                            clock: clock.as_ref(),
                                            uncertain: &mut uncertain,
                                            options: &options,
                                        }
                                        .capture_envelope(request, proof, *candidate),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                Job::RecoverStockObservation(
                                    binding,
                                    proof,
                                    observation,
                                    reply,
                                ) => {
                                    let result = Box::pin(
                                        super::recovery::RecoveryTurn {
                                            connection: &mut connection,
                                            cached_grant: &mut cached_grant,
                                            namespace: &namespace,
                                            selected: &input,
                                            crypto: &crypto,
                                            clock: clock.as_ref(),
                                            uncertain: &mut uncertain,
                                            options: &options,
                                        }
                                        .capture_observation(*binding, proof, *observation),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                Job::RecoverStockUnissued(request, proof, reply) => {
                                    let result = Box::pin(
                                        super::recovery::RecoveryTurn {
                                            connection: &mut connection,
                                            cached_grant: &mut cached_grant,
                                            namespace: &namespace,
                                            selected: &input,
                                            crypto: &crypto,
                                            clock: clock.as_ref(),
                                            uncertain: &mut uncertain,
                                            options: &options,
                                        }
                                        .capture_unissued(*request, proof),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                Job::RecoverStockEvidence(request, proof, reply) => {
                                    let result = Box::pin(
                                        super::recovery::RecoveryTurn {
                                            connection: &mut connection,
                                            cached_grant: &mut cached_grant,
                                            namespace: &namespace,
                                            selected: &input,
                                            crypto: &crypto,
                                            clock: clock.as_ref(),
                                            uncertain: &mut uncertain,
                                            options: &options,
                                        }
                                        .capture_stock(*request, proof),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                Job::RecoverCapture(selected, proof, evidence, reply) => {
                                    let result = super::recovery::RecoveryTurn {
                                        connection: &mut connection,
                                        cached_grant: &mut cached_grant,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                        options: &options,
                                    }
                                    .capture(*selected, proof, *evidence)
                                    .await;
                                    let _ = reply.send(result);
                                    continue;
                                }
                                job => job,
                            };
                            let Some(connection) = connection.as_mut() else {
                                job.reject_recovery();
                                continue;
                            };
                            dispatch::run(
                                job,
                                kind,
                                stock::inventory::Turn {
                                    connection,
                                    namespace: &namespace,
                                    selected: &input,
                                    crypto: &crypto,
                                    clock: clock.as_ref(),
                                    uncertain: &mut uncertain,
                                },
                            )
                            .await;
                        }
                    }
                    // The admission sender is gone. All admitted work has drained.
                    let Some(mut connection) = connection else {
                        return Err(WorkspaceError::RecoveryRequired);
                    };
                    let checkpoint = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                        .fetch_one(&mut connection)
                        .await;
                    let checked = match checkpoint {
                        Ok(row) if row.try_get::<i64, _>(0).ok() == Some(0) => Ok(()),
                        _ => Err(WorkspaceError::StorageFailure),
                    };
                    let closed = connection
                        .close()
                        .await
                        .map_err(|_| WorkspaceError::StorageFailure);
                    checked.and(closed).and_then(|()| namespace.validate())
                }));
                drop(namespace);
                let _ = completion.send(Some(result));
            })
            .map_err(|_| WorkspaceError::Unavailable)?;
        let handle = Self {
            sender: Mutex::new(Some(sender)),
            queued_bytes,
            completed,
            thread: Mutex::new(Some(thread)),
        };
        match started.await {
            Ok(Ok(())) => Ok(handle),
            Ok(Err(error)) => {
                let _ = handle.close().await;
                Err(error)
            }
            Err(_) => {
                let _ = handle.close().await;
                Err(WorkspaceError::Unavailable)
            }
        }
    }
    pub(super) async fn change_stock_grant(
        &self,
        change: stock::grants::Change,
        proof: AttachmentProof,
    ) -> Result<stock::StockGrantOutcome, WorkspaceError> {
        let transaction = change.transaction;
        let (reply, receive) = oneshot::channel();
        self.send(Job::ChangeStockGrant(Box::new(change), proof, reply))?;
        receive
            .await
            .unwrap_or(Ok(stock::StockGrantOutcome::Unknown(transaction)))
    }
    pub(super) async fn stock_authority(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> Result<super::StockAuthorityReceipt, WorkspaceError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockAuthority(input, proof, reply))?;
        result.await.map_err(|_| WorkspaceError::RecoveryRequired)?
    }
    pub(super) async fn execute_inventory(
        &self,
        request: radroots_storage::inventory::InventoryRequest,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::InventoryOutcome, OperationError> {
        let c = request.command().context();
        let anchor = CommitAttempt::new(
            c.workspace_id(),
            c.operation_id(),
            request.command().digest(),
            None,
        );
        let (reply, result) = oneshot::channel();
        self.send(Job::ExecuteInventory(Box::new(request), proof, reply))?;
        result
            .await
            .unwrap_or(Ok(radroots_storage::inventory::InventoryOutcome::Unknown(
                anchor,
            )))
    }
    pub(super) async fn inventory_operation(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_storage::inventory::InventoryReceipt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::InventoryOperation(
            Box::new(binding),
            minimum,
            operation,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn issue_stock_consent(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        capture: stock::StockConsentCapture,
        proof: AttachmentProof,
    ) -> Result<stock::StockConsentOutcome, OperationError> {
        let anchor = stock::StockConsentOutcome::Unknown {
            control_operation: capture.control_operation_id(),
            transaction: capture.transaction_id(),
            command_operation: capture.command().context().operation_id(),
            subject: capture.subject_grant(),
        };
        let (reply, result) = oneshot::channel();
        self.send(Job::IssueStockConsent(
            Box::new(binding),
            minimum,
            Box::new(capture),
            proof,
            reply,
        ))?;
        result.await.unwrap_or(Ok(anchor))
    }
    pub(super) async fn stock_consent(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        operation: OperationId,
        subject: radroots_domain::GrantId,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockConsentReceipt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockConsent(
            Box::new(binding),
            minimum,
            operation,
            subject,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn retain_stock_listing(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        capture: stock::StockListingCapture,
        proof: AttachmentProof,
    ) -> Result<stock::StockListingOutcome, OperationError> {
        let anchor = stock::StockListingOutcome::Unknown {
            transaction: capture.transaction_id(),
            listing: capture.listing_revision(),
        };
        let (reply, result) = oneshot::channel();
        self.send(Job::RetainStockListing(
            Box::new(binding),
            minimum,
            Box::new(capture),
            proof,
            reply,
        ))?;
        result.await.unwrap_or(Ok(anchor))
    }
    pub(super) async fn stock_listing(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        lookup: stock::public_source::Lookup,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockListingSnapshot>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockListing(
            Box::new(binding),
            minimum,
            lookup,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn retain_stock_source(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        capture: stock::StockSourceCapture,
        proof: AttachmentProof,
    ) -> Result<stock::StockSourceOutcome, OperationError> {
        let anchor = stock::StockSourceOutcome::Unknown {
            transaction: capture.transaction_id(),
            envelope: capture.envelope_id(),
        };
        let (reply, result) = oneshot::channel();
        self.send(Job::RetainStockSource(
            Box::new(binding),
            minimum,
            Box::new(capture),
            proof,
            reply,
        ))?;
        result.await.unwrap_or(Ok(anchor))
    }
    pub(super) async fn install_stock_agreement(
        &self,
        request: stock::StockInstallationRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockInstallationOutcome, OperationError> {
        let unknown = stock::StockInstallationOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::InstallStockAgreement(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_agreement_installation(
        &self,
        request: stock::StockInstallationRequest,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockInstallationReceipt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockAgreementInstallation(
            Box::new(request),
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn abort_proven_unissued(
        &self,
        request: stock::StockAbortRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockAbortOutcome, OperationError> {
        let unknown = stock::StockAbortOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::AbortStockPreparation(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_preparation_abort(
        &self,
        request: stock::StockAbortRequest,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockAbortReceipt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockPreparationAbort(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_operation_snapshot(
        &self,
        request: stock::StockSnapshotRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockOperationSnapshot, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockSnapshot(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_signing_evidence_page(
        &self,
        request: stock::StockClaimRequest,
        after: Option<radroots_domain::Sequence>,
        proof: AttachmentProof,
    ) -> Result<stock::StockEvidencePage, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockEvidencePage(
            Box::new(request),
            after,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn capture_stock_original_unissued(
        &self,
        request: stock::issuance::unissued::Request,
        proof: AttachmentProof,
        reconcile: bool,
    ) -> Result<stock::StockUnissuedOutcome, OperationError> {
        let unknown = stock::StockUnissuedOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(if reconcile {
            Job::RecoverStockUnissued(Box::new(request), proof, reply)
        } else {
            Job::CaptureStockUnissued(Box::new(request), proof, reply)
        })?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_original_unissued(
        &self,
        request: stock::StockClaimRequest,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockUnissuedReceipt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockUnissued(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn reconcile_stock_evidence(
        &self,
        request: radroots_storage::inventory::StockEvidenceRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockEvidenceOutcome, OperationError> {
        let unknown = stock::StockEvidenceOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::RecoverStockEvidence(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }

    pub(super) async fn reconcile_stock_envelope(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        proof: AttachmentProof,
        candidate: radroots_storage::command::PrivateEnvelopeCandidate,
    ) -> Result<radroots_storage::inventory::StockEnvelopeOutcome, OperationError> {
        let unknown = radroots_storage::inventory::StockEnvelopeOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::RecoverStockEnvelope(
            stock::envelope::CaptureRequest::Original(Box::new(request)),
            proof,
            Box::new(candidate),
            reply,
        ))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn prepare_stock_delivery_attempt(
        &self,
        request: radroots_storage::inventory::StockDeliveryRequest,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::StockDeliveryAttemptOutcome, OperationError> {
        let unknown = radroots_storage::inventory::StockDeliveryAttemptOutcome::Unknown {
            operation: request.attempt(),
            request: request.intent().request_id().clone(),
        };
        let (reply, result) = oneshot::channel();
        self.send(Job::PrepareStockDelivery(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_delivery_attempt(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        request_id: radroots_transport::sink::DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_storage::inventory::StockDeliveryAttempt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockDeliveryAttempt(
            Box::new(request),
            request_id,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_delivery_summary(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::StockDeliverySummary, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockDeliverySummary(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn record_stock_delivery_observation(
        &self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        observation: radroots_storage::inventory::StockDeliveryObservation,
        recovering: bool,
    ) -> Result<radroots_storage::inventory::StockDeliveryObservationOutcome, OperationError> {
        let source = observation.source().attempt();
        let unknown = radroots_storage::inventory::StockDeliveryObservationOutcome::Unknown(
            radroots_storage::command::CommitAttempt::new(
                source.workspace_id(),
                source.operation_id(),
                source.command_digest(),
                None,
            ),
        );
        let (reply, result) = oneshot::channel();
        self.send(if recovering {
            Job::RecoverStockObservation(Box::new(binding), proof, Box::new(observation), reply)
        } else {
            Job::RecordStockDeliveryObservation(
                Box::new(binding),
                proof,
                Box::new(observation),
                reply,
            )
        })?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_delivery_observations(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        after: Option<radroots_domain::Sequence>,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::StockDeliveryObservationPage, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockDeliveryObservations(
            Box::new(request),
            after,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn capture_stock_envelope(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        proof: AttachmentProof,
        candidate: radroots_storage::command::PrivateEnvelopeCandidate,
    ) -> Result<radroots_storage::inventory::StockEnvelopeOutcome, OperationError> {
        let unknown = radroots_storage::inventory::StockEnvelopeOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::CaptureStockEnvelope(
            stock::envelope::CaptureRequest::Original(Box::new(request)),
            proof,
            Box::new(candidate),
            reply,
        ))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_envelope(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_storage::inventory::StockEnvelopeSource>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockEnvelope(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn prepare_stock_rewrap(
        &self,
        request: radroots_storage::inventory::StockRewrapRequest,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::StockRewrapOutcome, OperationError> {
        let unknown = radroots_storage::inventory::StockRewrapOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::PrepareStockRewrap(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_rewrap_attempt(
        &self,
        request: radroots_storage::inventory::StockRewrapRequest,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_storage::inventory::StockRewrapState>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockRewrapAttempt(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn reconcile_stock_rewrap_admission(
        &self,
        request: radroots_storage::inventory::StockRewrapRequest,
        proof: AttachmentProof,
    ) -> Result<(), OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::RecoverStockRewrapAdmission(
            Box::new(request),
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn capture_stock_rewrap(
        &self,
        request: radroots_storage::inventory::StockRewrapRequest,
        proof: AttachmentProof,
        candidate: radroots_storage::command::PrivateEnvelopeCandidate,
        reconcile: bool,
    ) -> Result<radroots_storage::inventory::StockEnvelopeOutcome, OperationError> {
        let unknown = radroots_storage::inventory::StockEnvelopeOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        let request = stock::envelope::CaptureRequest::Rewrap(Box::new(request));
        self.send(if reconcile {
            Job::RecoverStockEnvelope(request, proof, Box::new(candidate), reply)
        } else {
            Job::CaptureStockEnvelope(request, proof, Box::new(candidate), reply)
        })?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_rewraps(
        &self,
        request: radroots_storage::inventory::StockEnvelopeRequest,
        after: Option<radroots_domain::Sequence>,
        proof: AttachmentProof,
    ) -> Result<radroots_storage::inventory::StockRewrapPage, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockRewraps(Box::new(request), after, proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn capture_stock_signing_evidence(
        &self,
        evidence: radroots_storage::inventory::StockEvidenceRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockEvidenceOutcome, OperationError> {
        let unknown = stock::StockEvidenceOutcome::Unknown(evidence.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::CaptureStockEvidence(Box::new(evidence), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_signing_evidence(
        &self,
        request: stock::StockClaimRequest,
        digest: [u8; 32],
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockCapturedEvidence>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockEvidence(Box::new(request), digest, proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn claim_stock_signing_attempt(
        &self,
        request: stock::StockClaimRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockClaimOutcome, OperationError> {
        let unknown = stock::StockClaimOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::ClaimStockSigning(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn authorize_stock_exact_replay(
        &self,
        request: stock::StockClaimRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockExactReplayPermit, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::AuthorizeStockExactReplay(
            Box::new(request),
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_signing_attempt(
        &self,
        request: stock::StockClaimRequest,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockSigningAttempt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockSigningAttempt(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn bind_stock_signing_intents(
        &self,
        request: stock::StockSigningPlanRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockSigningPlanOutcome, OperationError> {
        let unknown = stock::StockSigningPlanOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::BindStockSigningPlan(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_signing_plan(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        operation: OperationId,
        digest: CommandDigest,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockBoundSigningPlan>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockSigningPlan(
            Box::new(binding),
            minimum,
            operation,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn prepare_stock_acceptance(
        &self,
        request: stock::StockAcceptanceRequest,
        proof: AttachmentProof,
    ) -> Result<stock::StockAcceptanceOutcome, OperationError> {
        let unknown = stock::StockAcceptanceOutcome::Unknown(request.attempt());
        let (reply, result) = oneshot::channel();
        self.send(Job::PrepareStockAcceptance(Box::new(request), proof, reply))?;
        result.await.unwrap_or(Ok(unknown))
    }
    pub(super) async fn stock_order_request(
        &self,
        binding: WorkspaceBinding,
        inventory_minimum: radroots_storage::inventory::InventoryCheckpointRef,
        minimum: radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        listings: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        selected: radroots_domain::RecordId,
        proof: AttachmentProof,
    ) -> Result<stock::StockRequestSnapshot, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockOrderRequest(
            Box::new(binding),
            inventory_minimum,
            minimum,
            listings,
            selected,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_order_history(
        &self,
        binding: WorkspaceBinding,
        inventory_minimum: radroots_storage::inventory::InventoryCheckpointRef,
        minimum: radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        listings: radroots_storage::workspace::seller_stock::StockListingCheckpointRef,
        scope: radroots_domain::causal::CausalScope,
        proof: AttachmentProof,
    ) -> Result<stock::StockHistorySnapshot, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockOrderHistory(
            Box::new(binding),
            inventory_minimum,
            minimum,
            listings,
            scope,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn stock_source(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::workspace::seller_stock::StockSourceCheckpointRef,
        envelope: radroots_domain::EnvelopeEventId,
        proof: AttachmentProof,
    ) -> Result<Option<stock::StockSourceSnapshot>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::StockSource(
            Box::new(binding),
            minimum,
            envelope,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn option_mapping(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        key: radroots_domain::stock::OptionMappingKey,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_domain::stock::OptionPoolMapping>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::OptionMapping(
            Box::new(binding),
            minimum,
            key,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn pool(
        &self,
        binding: WorkspaceBinding,
        minimum: radroots_storage::inventory::InventoryCheckpointRef,
        id: radroots_domain::PoolId,
        proof: AttachmentProof,
    ) -> Result<Option<radroots_domain::stock::PoolView>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Pool(Box::new(binding), minimum, id, proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    fn send(&self, job: Job) -> Result<(), WorkspaceError> {
        let bytes = match &job {
            Job::RetainStockListing(..)
            | Job::CaptureStockUnissued(..)
            | Job::RecoverStockUnissued(..)
            | Job::CaptureStockEvidence(..)
            | Job::RecoverStockEvidence(..) => 256 * 1024,
            Job::PrepareStockDelivery(..)
            | Job::StockDeliveryAttempt(..)
            | Job::StockDeliverySummary(..)
            | Job::RecordStockDeliveryObservation(..)
            | Job::StockDeliveryObservations(..)
            | Job::RecoverStockEnvelope(..)
            | Job::RecoverStockObservation(..)
            | Job::CaptureStockEnvelope(..)
            | Job::StockEnvelope(..)
            | Job::PrepareStockRewrap(..)
            | Job::StockRewrapAttempt(..)
            | Job::RecoverStockRewrapAdmission(..)
            | Job::StockRewraps(..)
            | Job::StockEvidence(..)
            | Job::StockUnissued(..)
            | Job::StockEvidencePage(..)
            | Job::StockSnapshot(..)
            | Job::InstallStockAgreement(..)
            | Job::StockAgreementInstallation(..)
            | Job::AbortStockPreparation(..)
            | Job::StockPreparationAbort(..) => 1024 * 1024,
            Job::IssueStockConsent(..) | Job::PrepareStockAcceptance(..) => 512 * 1024,
            Job::BindStockSigningPlan(..)
            | Job::StockSigningPlan(..)
            | Job::ClaimStockSigning(..)
            | Job::StockSigningAttempt(..)
            | Job::AuthorizeStockExactReplay(..) => 1024 * 1024,
            Job::RetainStockSource(..) => 256 * 1024,
            Job::ChangeStockGrant(..) | Job::ExecuteInventory(..) => 64 * 1024,
            Job::CaptureDeliveryObservation(..) => 96 * 1024,
            Job::PrepareDelivery(..) => 1024 * 1024,
            Job::BindPlan(..) => 1024 * 1024,
            Job::Capture(..) | Job::RecoverCapture(..) => 256 * 1024,
            // Public capture can retain a full 64 KiB wire plus decoded
            // bounded fields. Include collection/descriptor ownership as well.
            Job::Admit(request, ..) if request.command().listing().is_some() => 192 * 1024,
            Job::Admit(..) => 96 * 1024,
            _ => 16 * 1024,
        };
        self.queued_bytes
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes)
                    .filter(|next| *next <= QUEUE_MAX_BYTES)
            })
            .map_err(|_| WorkspaceError::Unavailable)?;
        let queued = Queued {
            job,
            _budget: Budget {
                used: self.queued_bytes.clone(),
                bytes,
            },
        };
        self.sender
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .as_ref()
            .ok_or(WorkspaceError::Closed)?
            .try_send(queued)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => WorkspaceError::Unavailable,
                mpsc::TrySendError::Disconnected(_) => WorkspaceError::Closed,
            })
    }
    pub(super) async fn prepare_delivery(
        &self,
        request: DeliveryAttemptRequest,
        proof: AttachmentProof,
        candidate: Option<&EnvelopeCandidate>,
    ) -> Result<DeliveryAttemptOutcome, OperationError> {
        let anchor = request.identity();
        let identity = request.intent().request_id().clone();
        let (reply, result) = oneshot::channel();
        self.send(Job::PrepareDelivery(
            Box::new(request),
            proof,
            candidate.map(|value| Box::new(value.retained_copy())),
            reply,
        ))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(DeliveryAttemptOutcome::Unknown {
                operation: anchor,
                attempt: identity,
            }),
        }
    }
    pub(super) async fn capture_delivery_observation(
        &self,
        binding: WorkspaceBinding,
        proof: AttachmentProof,
        observation: DeliveryObservation,
    ) -> Result<DeliveryObservationOutcome, OperationError> {
        let fallback = observation.clone();
        let (reply, result) = oneshot::channel();
        self.send(Job::CaptureDeliveryObservation(
            Box::new(binding),
            proof,
            Box::new(observation),
            reply,
        ))?;
        result
            .await
            .unwrap_or(Ok(DeliveryObservationOutcome::Unknown(fallback)))
    }
    pub(super) async fn query_delivery_observations(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        target: SigningTarget,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Vec<DeliveryObservation>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::QueryDeliveryObservations(
            Box::new(binding),
            operation,
            digest,
            target,
            request,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn query_delivery(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: CommandDigest,
        target: SigningTarget,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Option<DeliveryAttempt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::QueryDelivery(
            Box::new(binding),
            operation,
            digest,
            target,
            request,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn operation_snapshot(
        &self,
        binding: WorkspaceBinding,
        operation: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<OperationSnapshot>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Snapshot(
            Box::new(binding),
            operation,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn install_chat(
        &self,
        request: InstallationRequest,
        proof: AttachmentProof,
    ) -> Result<InstallationOutcome, OperationError> {
        let anchor = request.identity();
        let (reply, result) = oneshot::channel();
        self.send(Job::Install(Box::new(request), proof, reply))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(InstallationOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn claim_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<ClaimAttemptOutcome, OperationError> {
        let anchor = request.identity();
        let (reply, result) = oneshot::channel();
        self.send(Job::Claim(Box::new(request), proof, reply))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(ClaimAttemptOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn signing_attempt(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<SigningAttempt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Attempt(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn capture_evidence(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
        evidence: &ArtifactEvidence,
    ) -> Result<EvidenceCaptureOutcome, OperationError> {
        let anchor = CommitAttempt::new(
            evidence.workspace_id(),
            evidence.operation_id(),
            evidence.command_digest(),
            None,
        );
        let copied = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        let (reply, result) = oneshot::channel();
        self.send(Job::Capture(
            Box::new(input),
            copied,
            Box::new(evidence.retained_copy()),
            reply,
        ))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(EvidenceCaptureOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn reconcile_unknown(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
        evidence: &ArtifactEvidence,
    ) -> Result<EvidenceCaptureOutcome, OperationError> {
        let anchor = CommitAttempt::new(
            evidence.workspace_id(),
            evidence.operation_id(),
            evidence.command_digest(),
            None,
        );
        let copied = proof.with_secret(|bytes| AttachmentProof::from_protected_input(bytes))?;
        let (reply, result) = oneshot::channel();
        self.send(Job::RecoverCapture(
            Box::new(input),
            copied,
            Box::new(evidence.retained_copy()),
            reply,
        ))?;
        result
            .await
            .unwrap_or(Ok(EvidenceCaptureOutcome::Unknown(anchor)))
    }
    pub(super) async fn obtained_evidence(
        &self,
        request: ClaimAttemptRequest,
        proof: AttachmentProof,
    ) -> Result<Option<CapturedArtifact>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Captured(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn bind_plan(
        &self,
        binding: WorkspaceBinding,
        plan: CapturedSigningPlan,
        proof: AttachmentProof,
    ) -> Result<PlanBindingOutcome, OperationError> {
        let anchor = CommitAttempt::new(
            binding.input().workspace_id(),
            plan.operation_id(),
            plan.command_digest(),
            None,
        );
        let (reply, result) = oneshot::channel();
        self.send(Job::BindPlan(Box::new(binding), plan, proof, reply))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(PlanBindingOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn signing_plan(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        digest: CommandDigest,
        proof: AttachmentProof,
    ) -> Result<Option<BoundSigningPlan>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::SigningPlan(
            Box::new(binding),
            operation_id,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> Result<WorkspaceBinding, WorkspaceError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Authenticate(input, proof, reply))?;
        result.await.map_err(|_| WorkspaceError::Unavailable)?
    }
    pub(super) async fn revoke(
        &self,
        input: AttachmentInput,
        proof: &AttachmentProof,
    ) -> Result<(), WorkspaceError> {
        let proof = proof.with_secret(|secret| AttachmentProof::from_protected_input(secret))?;
        let (reply, result) = oneshot::channel();
        self.send(Job::Revoke(input, proof, reply))?;
        result.await.map_err(|_| WorkspaceError::CommitUnknown)?
    }
    pub(super) async fn admit(
        &self,
        request: AdmissionRequest,
        proof: AttachmentProof,
    ) -> Result<AdmissionOutcome, OperationError> {
        let context = request.command().context();
        let anchor = CommitAttempt::new(
            context.workspace_id(),
            context.operation_id(),
            request.command().digest(),
            None,
        );
        let (reply, result) = oneshot::channel();
        self.send(Job::Admit(Box::new(request), proof, reply))?;
        match result.await {
            Ok(result) => result,
            Err(_) => Ok(AdmissionOutcome::Unknown(anchor)),
        }
    }
    pub(super) async fn operation(
        &self,
        binding: WorkspaceBinding,
        operation_id: OperationId,
        digest: Option<CommandDigest>,
        proof: AttachmentProof,
    ) -> Result<Option<PreparedOperation>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Operation(
            Box::new(binding),
            operation_id,
            digest,
            proof,
            reply,
        ))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn close(&self) -> Result<(), WorkspaceError> {
        self.sender
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .take();
        let mut completed = self.completed.clone();
        let result = loop {
            if let Some(result) = *completed.borrow_and_update() {
                break result;
            }
            completed
                .changed()
                .await
                .map_err(|_| WorkspaceError::Unavailable)?;
        };
        if let Some(thread) = self
            .thread
            .lock()
            .map_err(|_| WorkspaceError::Unavailable)?
            .take()
        {
            thread.join().map_err(|_| WorkspaceError::Unavailable)?;
        }
        result
    }
}

pub(super) async fn preflight_existing(
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    // Inspect through SQLite read-only before any persistent journal setting or
    // writable connection. Do not turn a foreign schema into a workspace.
    let options = SqliteConnectOptions::new()
        .filename(namespace.database_path())
        .read_only(true)
        .create_if_missing(false)
        .pragma("query_only", "ON")
        .busy_timeout(Duration::from_secs(5));
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    let inspected = match namespace.kind() {
        Kind::Outbox => initialize(&mut connection, namespace, expected, None, crypto).await,
        Kind::SellerStock => {
            stock::initialize(&mut connection, namespace, expected, None, crypto).await
        }
    };
    let closed = connection
        .close()
        .await
        .map_err(|_| WorkspaceError::StorageFailure);
    inspected.and(closed)
}

pub(super) async fn initialize(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    initial: Option<GrantRecord>,
    crypto: &WorkspaceCrypto,
) -> Result<(), WorkspaceError> {
    namespace.validate()?;
    if let Some(initial) = initial {
        let plaintext = initial.encode()?;
        let protected = crypto
            .seal(expected.workspace_id(), expected.grant_id(), &plaintext)
            .await?;
        let mut transaction = connection
            .begin()
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        for object in schema::catalog(schema::CURRENT_VERSION)? {
            sqlx::raw_sql(object.sql)
                .execute(&mut *transaction)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        }
        sqlx::query("PRAGMA user_version=11")
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::query(
            "INSERT INTO workspace_grant(singleton,workspace,grant_id,protected) VALUES(1,?,?,?)",
        )
        .bind(expected.workspace_id().as_bytes().as_slice())
        .bind(expected.grant_id().as_bytes().as_slice())
        .bind(protected)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkspaceError::CommitUnknown)?;
    }
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    schema::admit(connection, version).await?;
    let integrity: String = sqlx::query_scalar("PRAGMA quick_check(1)")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if integrity != "ok" {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let protected = read(connection, expected).await?;
    let plaintext = crypto
        .open(expected.workspace_id(), expected.grant_id(), &protected)
        .await?;
    plaintext.expose_secret(GrantRecord::decode_outbox)?;
    namespace.validate()
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    expected: &AttachmentInput,
) -> Result<Vec<u8>, WorkspaceError> {
    // Refuse oversized input in SQL before materializing the protected blob.
    let row = sqlx::query("SELECT CASE WHEN length(workspace)=32 THEN workspace END,CASE WHEN length(grant_id)=32 THEN grant_id END,length(protected),CASE WHEN length(protected)<=262144 THEN protected END FROM workspace_grant WHERE singleton=1")
        .fetch_optional(connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?.ok_or(WorkspaceError::RecoveryRequired)?;
    let workspace: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(0)
        .map_err(|_| WorkspaceError::RecoveryRequired)?
        .ok_or(WorkspaceError::RecoveryRequired)?;
    let grant: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(1)
        .map_err(|_| WorkspaceError::RecoveryRequired)?
        .ok_or(WorkspaceError::RecoveryRequired)?;
    if workspace != expected.workspace_id().as_bytes() || grant != expected.grant_id().as_bytes() {
        return Err(WorkspaceError::WrongNamespace);
    }
    let length: i64 = row
        .try_get(2)
        .map_err(|_| WorkspaceError::RecoveryRequired)?;
    if length <= 0 || length > 262144 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    row.try_get(3).map_err(|_| WorkspaceError::RecoveryRequired)
}
async fn authenticate(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(GrantRecord, WorkspaceBinding), WorkspaceError> {
    namespace.validate()?;
    let protected = read(connection, expected).await?;
    let plaintext = crypto
        .open(expected.workspace_id(), expected.grant_id(), &protected)
        .await?;
    let record = plaintext.expose_secret(GrantRecord::decode_outbox)?;
    let binding = record.authenticate(selected, proof, clock.now()?)?;
    namespace.validate()?;
    Ok((record, binding))
}
async fn revoke(
    connection: &mut SqliteConnection,
    namespace: &Namespace,
    expected: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    clock: &dyn WorkspaceClock,
    selected: &AttachmentInput,
    proof: &AttachmentProof,
) -> Result<(), WorkspaceError> {
    let (mut record, _) = authenticate(
        connection, namespace, expected, crypto, clock, selected, proof,
    )
    .await?;
    record.revoke()?;
    let plaintext = record.encode()?;
    let protected = crypto
        .seal(expected.workspace_id(), expected.grant_id(), &plaintext)
        .await?;
    namespace.validate()?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| WorkspaceError::StorageFailure)?;
    let changed = sqlx::query(
        "UPDATE workspace_grant SET protected=? WHERE singleton=1 AND workspace=? AND grant_id=?",
    )
    .bind(protected)
    .bind(expected.workspace_id().as_bytes().as_slice())
    .bind(expected.grant_id().as_bytes().as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(|_| WorkspaceError::StorageFailure)?;
    if changed.rows_affected() != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
    transaction
        .commit()
        .await
        .map_err(|_| WorkspaceError::CommitUnknown)
}
