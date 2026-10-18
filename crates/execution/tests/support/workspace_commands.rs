// Port-model orchestration tests, not native persistence/authentication evidence.
use super::*;
use radroots_execution::inbox::InquireInput;
use radroots_execution::{Admission, Code, OperationContext, RetryClass};
use radroots_storage::command::*;
type BoxFuture<'a, T> = radroots_storage::event::BoxFuture<'a, T>;

type Stored = Option<(WorkspaceBinding, Vec<u8>)>;
struct Journal {
    model: Shared<Model>,
    trace: Mutex<Vec<&'static str>>,
    stored: Mutex<Stored>,
    uncertain: AtomicBool,
    absent: AtomicBool,
    admission_error: Mutex<Option<OperationError>>,
    query_error: Mutex<Option<OperationError>>,
}
impl WorkspaceStore for Journal {
    fn authenticate(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<WorkspaceBinding, WorkspaceError>> {
        self.model.authenticate(input, proof)
    }
    fn close_workspace(&self) -> BoxFuture<'_, Result<(), WorkspaceError>> {
        Box::pin(async { Ok(()) })
    }
}
fn prepared(request: AdmissionRequest) -> PreparedOperation {
    PreparedOperation::from_committed_store(
        request,
        TransactionId::from_bytes([21; 32]),
        Sequence::new(1).unwrap(),
        IssuanceWindow::new(
            UnixSeconds::new(100).unwrap(),
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn journal() -> Shared<Journal> {
    Shared::new(Journal {
        model: new_model(),
        trace: Mutex::new(Vec::new()),
        stored: Mutex::new(None),
        uncertain: AtomicBool::new(false),
        absent: AtomicBool::new(false),
        admission_error: Mutex::new(None),
        query_error: Mutex::new(None),
    })
}
fn context(scope: &WorkspaceSession) -> radroots_execution::workspace::CommandContext {
    scope
        .command_context(
            OperationId::from_bytes([22; 32]),
            None,
            None,
            UnixSeconds::new(150).unwrap(),
        )
        .unwrap()
}
fn inquiry() -> InquireInput {
    InquireInput::new(
        ThreadId::from_bytes([23; 32]),
        key(),
        None,
        "private model text",
    )
    .unwrap()
}
async fn attached(journal: &Shared<Journal>) -> (ClientSession<Host>, WorkspaceSession) {
    let client = ClientSession::new(Host, core::num::NonZeroU16::MIN);
    let scope = client
        .attach_workspace(journal.clone(), input(5), proof())
        .await
        .unwrap();
    client.start().await.unwrap();
    (client, scope)
}
impl OperationJournal for Journal {
    fn record_delivery_observation(
        &self,
        _binding: WorkspaceBinding,
        _proof: AttachmentProof,
        _observation: DeliveryObservation,
    ) -> BoxFuture<'_, Result<DeliveryObservationOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn delivery_observations(
        &self,
        _binding: WorkspaceBinding,
        _operation: radroots_domain::OperationId,
        _digest: radroots_domain::CommandDigest,
        _recipient: radroots_domain::PublicKey,
        _request: radroots_transport::sink::DeliveryRequestId,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Vec<DeliveryObservation>, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn prepare_delivery_attempt<'a>(
        &'a self,
        _request: DeliveryAttemptRequest,
        _proof: AttachmentProof,
        _candidate: Option<&'a EnvelopeCandidate>,
    ) -> BoxFuture<'a, Result<DeliveryAttemptOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn delivery_attempt(
        &self,
        _binding: WorkspaceBinding,
        _operation: OperationId,
        _digest: CommandDigest,
        _recipient: radroots_domain::PublicKey,
        _request: radroots_transport::sink::DeliveryRequestId,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<DeliveryAttempt>, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn install_transition_and_outbox(
        &self,
        _request: InstallationRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<InstallationOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn operation_snapshot(
        &self,
        _binding: WorkspaceBinding,
        operation_id: OperationId,
        expected_digest: Option<CommandDigest>,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<OperationSnapshot>, OperationError>> {
        Box::pin(async move {
            self.trace.lock().unwrap().push("query");
            if let Some(error) = *self.query_error.lock().unwrap() {
                return Err(error);
            }
            if self.absent.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let retained = self.stored.lock().unwrap();
            let Some((binding, bytes)) = retained.as_ref() else {
                return Ok(None);
            };
            let request = AdmissionRequest::from_protected_wire(binding.clone(), bytes).unwrap();
            assert_eq!(operation_id, request.command().context().operation_id());
            assert!(expected_digest.is_none_or(|digest| digest == request.command().digest()));
            let snapshot = OperationSnapshot::from_store_report(
                RetainedOperation::Prepared(prepared(request)),
                vec![RecipientSnapshot::from_store_report(
                    key(),
                    None,
                    None,
                    None,
                )],
                UnixSeconds::new(101).unwrap(),
            )
            .unwrap();
            Ok(Some(snapshot))
        })
    }
    fn claim_attempt(
        &self,
        _request: ClaimAttemptRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<ClaimAttemptOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn signing_attempt(
        &self,
        _request: ClaimAttemptRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<SigningAttempt>, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn capture_evidence<'a>(
        &'a self,
        _input: radroots_storage::workspace::AttachmentInput,
        _proof: &'a AttachmentProof,
        _evidence: &'a SealEvidence,
    ) -> BoxFuture<'a, Result<EvidenceCaptureOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn obtained_evidence(
        &self,
        _request: ClaimAttemptRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<CapturedSeal>, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn bind_signing_intents(
        &self,
        _binding: WorkspaceBinding,
        _plan: ChatSigningPlan,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<PlanBindingOutcome, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn signing_plan(
        &self,
        _binding: WorkspaceBinding,
        _operation_id: OperationId,
        _expected_digest: CommandDigest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<BoundSigningPlan>, OperationError>> {
        panic!("unexpected journal effect")
    }
    fn admit_command_and_hold(
        &self,
        request: AdmissionRequest,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<AdmissionOutcome, OperationError>> {
        Box::pin(async move {
            self.trace.lock().unwrap().push("admit");
            let operation = request.command().context().operation_id();
            let digest = request.command().digest();
            if self.uncertain.load(Ordering::SeqCst) {
                return Ok(AdmissionOutcome::Unknown(CommitAttempt::new(
                    request.binding().input().workspace_id(),
                    operation,
                    digest,
                    None,
                )));
            }
            if let Some(error) = *self.admission_error.lock().unwrap() {
                return Err(error);
            }
            *self.stored.lock().unwrap() = Some((
                request.binding().clone(),
                request.command().protected_wire().to_vec(),
            ));
            Ok(AdmissionOutcome::Committed(prepared(request)))
        })
    }
    fn operation(
        &self,
        _binding: WorkspaceBinding,
        _operation_id: OperationId,
        _expected_digest: Option<CommandDigest>,
        _proof: AttachmentProof,
    ) -> BoxFuture<'_, Result<Option<PreparedOperation>, OperationError>> {
        panic!("unexpected journal effect")
    }
}

#[test]
fn only_confirmed_admission_reaches_query_and_unknown_preserves_original_id() {
    block_on(async {
        let journal = journal();
        let (client, scope) = attached(&journal).await;
        journal.uncertain.store(true, Ordering::SeqCst);
        let error = client
            .inquire(journal.clone(), context(&scope), inquiry())
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::CommitUnknown);
        assert_eq!(
            error.context(),
            OperationContext::admission_unknown(OperationId::from_bytes([22; 32]))
        );
        assert_eq!(error.retry(), RetryClass::ManualReconciliation);
        assert_eq!(*journal.trace.lock().unwrap(), vec!["admit"]);
        journal.uncertain.store(false, Ordering::SeqCst);
        journal.trace.lock().unwrap().clear();
        let receipt = client
            .inquire(journal.clone(), context(&scope), inquiry())
            .await
            .unwrap();
        assert!(receipt.durable());
        assert!(receipt.business_result().is_none());
        assert_eq!(*journal.trace.lock().unwrap(), vec!["admit", "query"]);
        journal.trace.lock().unwrap().clear();
        let queried = client
            .operation(
                journal.clone(),
                &scope,
                OperationId::from_bytes([22; 32]),
                Some(receipt.command_digest()),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            queried.operation_checkpoint(),
            receipt.operation_checkpoint()
        );
        assert_eq!(*journal.trace.lock().unwrap(), vec!["query"]);
        journal.absent.store(true, Ordering::SeqCst);
        assert!(
            client
                .operation(
                    journal.clone(),
                    &scope,
                    OperationId::from_bytes([22; 32]),
                    None
                )
                .await
                .unwrap()
                .is_none()
        );
        let error = client
            .inquire(journal.clone(), context(&scope), inquiry())
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::RecoveryRequired);
        assert_eq!(error.context().admission(), Admission::Confirmed);
        client.close().await.unwrap();
    });
}

#[test]
fn port_failures_preserve_the_actual_admission_stage_and_fixed_error_contract() {
    block_on(async {
        let journal = journal();
        let (client, scope) = attached(&journal).await;
        for (port_error, code) in [
            (OperationError::InvalidCommand, Code::InvalidInput),
            (OperationError::InvalidEvidence, Code::RecoveryRequired),
            (OperationError::Unauthorized, Code::Unauthorized),
            (OperationError::UnverifiedApproval, Code::Unauthorized),
            (OperationError::MissingHistory, Code::MissingHistory),
            (OperationError::DeadlineExceeded, Code::DeadlineExceeded),
            (
                OperationError::IdempotencyConflict,
                Code::IdempotencyConflict,
            ),
            (OperationError::StorageFailure, Code::StorageFailure),
            (OperationError::RecoveryRequired, Code::RecoveryRequired),
            (OperationError::DeliveryAttemptLimit, Code::RecoveryRequired),
            (
                OperationError::Workspace(WorkspaceError::GrantRevoked),
                Code::GrantRevoked,
            ),
        ] {
            *journal.admission_error.lock().unwrap() = Some(port_error);
            let error = client
                .inquire(journal.clone(), context(&scope), inquiry())
                .await
                .unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.context().admission(), Admission::NotAdmitted);
            assert_eq!(
                error.context().operation_id(),
                Some(OperationId::from_bytes([22; 32]))
            );
            *journal.admission_error.lock().unwrap() = None;
            *journal.query_error.lock().unwrap() = Some(port_error);
            let error = client
                .inquire(journal.clone(), context(&scope), inquiry())
                .await
                .unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(error.context().admission(), Admission::Confirmed);
            assert!(!format!("{error:?}").contains("private model text"));
            let error = client
                .operation(
                    journal.clone(),
                    &scope,
                    OperationId::from_bytes([22; 32]),
                    None,
                )
                .await
                .unwrap_err();
            assert_eq!(error.code(), code);
            *journal.query_error.lock().unwrap() = None;
        }
        client.close().await.unwrap();
    });
}

#[test]
fn closed_and_foreign_scopes_do_not_reach_journal_and_capacity_is_not_reopened() {
    block_on(async {
        let journal = journal();
        let (client, scope) = attached(&journal).await;
        let other = WorkspaceSession::attach(journal.clone(), input(5), proof())
            .await
            .unwrap();
        assert_eq!(
            client
                .inquire(journal.clone(), context(&other), inquiry())
                .await
                .unwrap_err()
                .code(),
            Code::Unauthorized
        );
        assert_eq!(
            client
                .operation(
                    journal.clone(),
                    &other,
                    OperationId::from_bytes([22; 32]),
                    None
                )
                .await
                .unwrap_err()
                .code(),
            Code::Unauthorized
        );
        let captured = context(&scope);
        scope.close();
        assert_eq!(
            client
                .inquire(journal.clone(), captured, inquiry())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        assert_eq!(
            client
                .operation(
                    journal.clone(),
                    &scope,
                    OperationId::from_bytes([22; 32]),
                    None
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        assert!(journal.trace.lock().unwrap().is_empty());
        let captured = context(&other);
        client.close().await.unwrap();
        assert_eq!(
            client
                .inquire(journal.clone(), captured, inquiry())
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
        assert_eq!(
            client
                .operation(
                    journal.clone(),
                    &other,
                    OperationId::from_bytes([22; 32]),
                    None
                )
                .await
                .unwrap_err()
                .code(),
            Code::CapabilityUnavailable
        );
    });
}
