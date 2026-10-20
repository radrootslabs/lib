use super::{
    AttachmentInput, AttachmentProof, WorkspaceBinding, WorkspaceClock, WorkspaceCrypto,
    WorkspaceError, admission::Journal, attempt, delivery, evidence, fs::Namespace, installation,
    operation, plan, record::GrantRecord,
};
use radroots_domain::{CommandDigest, OperationId, PublicKey};
use radroots_storage::command::{
    AdmissionOutcome, AdmissionRequest, BoundSigningPlan, CapturedSeal, ChatSigningPlan,
    ClaimAttemptOutcome, ClaimAttemptRequest, CommitAttempt, DeliveryAttempt,
    DeliveryAttemptOutcome, DeliveryAttemptRequest, DeliveryObservation,
    DeliveryObservationOutcome, EnvelopeCandidate, EvidenceCaptureOutcome, InstallationOutcome,
    InstallationRequest, OperationError, OperationSnapshot, PlanBindingOutcome, PreparedOperation,
    SealEvidence, SigningAttempt,
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
const TABLE: &str = "CREATE TABLE workspace_grant (singleton INTEGER PRIMARY KEY CHECK(singleton=1), workspace BLOB NOT NULL CHECK(length(workspace)=32), grant_id BLOB NOT NULL CHECK(length(grant_id)=32), protected BLOB NOT NULL CHECK(length(protected)>0 AND length(protected)<=262144)) STRICT";
type Reply<T, E = WorkspaceError> = oneshot::Sender<Result<T, E>>;
enum Job {
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
        PublicKey,
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
        PublicKey,
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
        Box<SealEvidence>,
        Reply<EvidenceCaptureOutcome, OperationError>,
    ),
    RecoverCapture(
        Box<AttachmentInput>,
        AttachmentProof,
        Box<SealEvidence>,
        Reply<EvidenceCaptureOutcome, OperationError>,
    ),
    Captured(
        Box<ClaimAttemptRequest>,
        AttachmentProof,
        Reply<Option<CapturedSeal>, OperationError>,
    ),
    BindPlan(
        Box<WorkspaceBinding>,
        ChatSigningPlan,
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
                let namespace = match Namespace::acquire(&path, initial.is_some()) {
                    Ok(namespace) => namespace,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        let _ = completion.send(Some(Err(error)));
                        return;
                    }
                };
                let result = runtime.block_on(async {
                    if initial.is_none() {
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
                    let initialized =
                        initialize(&mut connection, &namespace, &input, initial, &crypto).await;
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
                            let job = match job {
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
                            match job {
                                job @ Job::RecoverCapture(..) => job.reject_recovery(),
                                Job::CaptureDeliveryObservation(
                                    binding,
                                    proof,
                                    observation,
                                    reply,
                                ) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .capture_delivery_observation(*binding, proof, *observation)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::QueryDeliveryObservations(
                                    binding,
                                    operation,
                                    digest,
                                    recipient,
                                    request,
                                    proof,
                                    reply,
                                ) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_delivery_observations(
                                        *binding, operation, digest, recipient, request, proof,
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::PrepareDelivery(request, proof, candidate, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .prepare_delivery(
                                        *request,
                                        proof,
                                        candidate.map(|value| *value),
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::QueryDelivery(
                                    binding,
                                    operation,
                                    digest,
                                    recipient,
                                    request,
                                    proof,
                                    reply,
                                ) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_delivery(
                                        *binding, operation, digest, recipient, request, proof,
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Install(request, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .install_chat(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Snapshot(binding, operation, digest, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_snapshot(*binding, operation, digest, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Claim(request, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .claim_issuance(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Attempt(request, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_issuance(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Capture(selected, proof, evidence, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .capture_issuance(*selected, proof, *evidence)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Captured(request, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_captured(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::BindPlan(binding, plan, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .bind_plan(*binding, plan, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::SigningPlan(binding, operation_id, digest, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query_plan(*binding, operation_id, digest, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Authenticate(selected, proof, reply) => {
                                    let result = authenticate(
                                        connection,
                                        &namespace,
                                        &input,
                                        &crypto,
                                        clock.as_ref(),
                                        &selected,
                                        &proof,
                                    )
                                    .await
                                    .map(|(_, binding)| binding);
                                    let _ = reply.send(result);
                                }
                                Job::Revoke(selected, proof, reply) => {
                                    let result = revoke(
                                        connection,
                                        &namespace,
                                        &input,
                                        &crypto,
                                        clock.as_ref(),
                                        &selected,
                                        &proof,
                                    )
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Admit(request, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .admit(*request, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                                Job::Operation(binding, operation_id, digest, proof, reply) => {
                                    let result = Journal {
                                        connection,
                                        namespace: &namespace,
                                        selected: &input,
                                        crypto: &crypto,
                                        clock: clock.as_ref(),
                                        uncertain: &mut uncertain,
                                    }
                                    .query(*binding, operation_id, digest, proof)
                                    .await;
                                    let _ = reply.send(result);
                                }
                            }
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
                });
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
    fn send(&self, job: Job) -> Result<(), WorkspaceError> {
        let bytes = match &job {
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
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Vec<DeliveryObservation>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::QueryDeliveryObservations(
            Box::new(binding),
            operation,
            digest,
            recipient,
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
        recipient: PublicKey,
        request: DeliveryRequestId,
        proof: AttachmentProof,
    ) -> Result<Option<DeliveryAttempt>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::QueryDelivery(
            Box::new(binding),
            operation,
            digest,
            recipient,
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
        evidence: &SealEvidence,
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
        evidence: &SealEvidence,
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
    ) -> Result<Option<CapturedSeal>, OperationError> {
        let (reply, result) = oneshot::channel();
        self.send(Job::Captured(Box::new(request), proof, reply))?;
        result.await.map_err(|_| OperationError::RecoveryRequired)?
    }
    pub(super) async fn bind_plan(
        &self,
        binding: WorkspaceBinding,
        plan: ChatSigningPlan,
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
    let inspected = initialize(&mut connection, namespace, expected, None, crypto).await;
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
        sqlx::raw_sql(TABLE)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(operation::TABLE)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(operation::INDEX)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(plan::TABLE)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        sqlx::raw_sql(plan::PARTS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceError::StorageFailure)?;
        for definition in [
            attempt::TABLE,
            attempt::ID_INDEX,
            attempt::TX_INDEX,
            evidence::TABLE,
            evidence::PARTS,
            evidence::TX_INDEX,
            installation::TABLE,
            installation::OP_INDEX,
            installation::TX_INDEX,
            installation::THREAD_INDEX,
            installation::WORK,
            delivery::TABLE,
            delivery::PARTS,
            delivery::TX_INDEX,
            delivery::OBSERVATIONS,
        ] {
            sqlx::raw_sql(definition)
                .execute(&mut *transaction)
                .await
                .map_err(|_| WorkspaceError::StorageFailure)?;
        }
        sqlx::query("PRAGMA user_version=8")
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
    if !matches!(version, 7 | 8) {
        return Err(WorkspaceError::RecoveryRequired);
    }
    let catalog_matches: i64 = sqlx::query_scalar("SELECT CASE WHEN count(*)=20 AND sum(CASE WHEN type='table' AND name='workspace_grant' AND tbl_name='workspace_grant' AND sql=? THEN 1 WHEN type='table' AND name='operations' AND tbl_name='operations' AND sql=? THEN 1 WHEN type='table' AND name='signing_plans' AND tbl_name='signing_plans' AND sql=? THEN 1 WHEN type='table' AND name='signing_plan_parts' AND tbl_name='signing_plan_parts' AND sql=? THEN 1 WHEN type='table' AND name='signing_attempts' AND tbl_name='signing_attempts' AND sql=? THEN 1 WHEN type='index' AND name='signing_attempt_identity' AND tbl_name='signing_attempts' AND sql=? THEN 1 WHEN type='index' AND name='signing_attempt_transaction_identity' AND tbl_name='signing_attempts' AND sql=? THEN 1 WHEN type='table' AND name='seal_evidence' AND tbl_name='seal_evidence' AND sql=? THEN 1 WHEN type='table' AND name='seal_evidence_parts' AND tbl_name='seal_evidence_parts' AND sql=? THEN 1 WHEN type='index' AND name='seal_evidence_transaction_identity' AND tbl_name='seal_evidence' AND sql=? THEN 1 WHEN type='index' AND name='operation_transaction_identity' AND tbl_name='operations' AND sql=? THEN 1 WHEN type='table' AND name='chat_messages' AND tbl_name='chat_messages' AND sql=? THEN 1 WHEN type='index' AND name='chat_message_operation_identity' AND tbl_name='chat_messages' AND sql=? THEN 1 WHEN type='index' AND name='chat_message_transaction_identity' AND tbl_name='chat_messages' AND sql=? THEN 1 WHEN type='index' AND name='chat_message_thread' AND tbl_name='chat_messages' AND sql=? THEN 1 WHEN type='table' AND name='chat_publication_work' AND tbl_name='chat_publication_work' AND sql=? THEN 1 WHEN type='table' AND name='chat_delivery_attempts' AND tbl_name='chat_delivery_attempts' AND sql=? THEN 1 WHEN type='table' AND name='chat_delivery_parts' AND tbl_name='chat_delivery_parts' AND sql=? THEN 1 WHEN type='index' AND name='chat_delivery_transaction_identity' AND tbl_name='chat_delivery_attempts' AND sql=? THEN 1 WHEN type='table' AND name='chat_delivery_observations' AND tbl_name='chat_delivery_observations' AND sql=? THEN 1 ELSE 0 END)=20 THEN 1 ELSE 0 END FROM sqlite_schema")
        .bind(TABLE).bind(if version==7 {operation::TABLE_V7} else {operation::TABLE}).bind(plan::TABLE).bind(plan::PARTS).bind(attempt::TABLE).bind(attempt::ID_INDEX).bind(attempt::TX_INDEX).bind(evidence::TABLE).bind(evidence::PARTS).bind(evidence::TX_INDEX).bind(operation::INDEX).bind(installation::TABLE).bind(installation::OP_INDEX).bind(installation::TX_INDEX).bind(installation::THREAD_INDEX).bind(installation::WORK).bind(delivery::TABLE).bind(delivery::PARTS).bind(delivery::TX_INDEX).bind(delivery::OBSERVATIONS).fetch_one(&mut *connection).await.map_err(|_| WorkspaceError::RecoveryRequired)?;
    if catalog_matches != 1 {
        return Err(WorkspaceError::RecoveryRequired);
    }
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
    plaintext.expose_secret(GrantRecord::decode)?;
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
    let record = plaintext.expose_secret(GrantRecord::decode)?;
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
