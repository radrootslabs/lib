#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    RadrootsClient, RadrootsSdkError, RadrootsSdkRecoveryAction, RadrootsSdkTradeErrorKind,
    SatisfactionPolicy, SdkIdempotencyKey, SdkMutationState, TargetPolicy, TradesClient,
    private_store::{
        SDK_PRIVATE_STORE_SCHEMA_VERSION, SdkPrivateTradeArtifactInput,
        SdkPrivateTradeArtifactKind, SdkPrivateTradeArtifactMetadata,
    },
    runtime::sdk_now_ms,
    workflow_runtime::{
        SdkWorkflowEnqueueReceipt, SdkWorkflowEnqueueRequest, enqueue_signed_workflow,
    },
};
#[cfg(feature = "runtime")]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner};
#[cfg(feature = "runtime")]
use radroots_event::{
    draft::RadrootsEventDraft,
    ids::{RadrootsEventId, RadrootsTradeCandidateId, RadrootsTradeId, RadrootsTradeMutationId},
    kinds::TRADE_MUTATION_EVENT_KINDS,
    trade::{
        RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES, RADROOTS_TRADE_MUTATION_CONTRACT_IDS,
        RADROOTS_TRADE_SCHEMA_VERSION, RadrootsTradeDecisionV1, RadrootsTradeMutationBodyV1,
        RadrootsTradeMutationEnvelopeV1, RadrootsTradePrivateTermsRefV1,
        trade_mutation_from_canonical_content,
    },
};
#[cfg(feature = "runtime")]
use radroots_event_codec::trade::trade_mutation_event_build;
#[cfg(feature = "runtime")]
use radroots_event_store::{RadrootsStoredTradeMutation, RadrootsTradeProjectionCheckpoint};
#[cfg(feature = "runtime")]
use radroots_trade::workflow::{
    RADROOTS_TRADE_REDUCER_CONTRACT_ID, RADROOTS_TRADE_REDUCER_VERSION,
    RadrootsTradeAgreementStateV1, RadrootsTradeAttestationStateV1, RadrootsTradeConflictStateV1,
    RadrootsTradeEvidenceStateV1, RadrootsTradeFulfillmentStateV1, RadrootsTradeMutationRecordV1,
    RadrootsTradeNegotiationStateV1, RadrootsTradePaymentStateV1,
    RadrootsTradePrivateTermsEvidenceV1, RadrootsTradePrivateTermsStateV1,
    RadrootsTradeProjectionV1, RadrootsTradeReductionInputV1, reduce_trade_records,
};
#[cfg(feature = "runtime")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "runtime")]
use sha2::{Digest, Sha256};
#[cfg(feature = "runtime")]
use sqlx::{QueryBuilder, Row, Sqlite};
#[cfg(feature = "runtime")]
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "runtime")]
pub const TRADE_SUBMIT_PROPOSAL_OPERATION_KIND: &str = "trade.submit_proposal.v1";
#[cfg(feature = "runtime")]
pub const TRADE_PROPOSE_REVISION_OPERATION_KIND: &str = "trade.propose_revision.v1";
#[cfg(feature = "runtime")]
pub const TRADE_DECIDE_CANDIDATE_OPERATION_KIND: &str = "trade.decide_candidate.v1";
#[cfg(feature = "runtime")]
pub const TRADE_CANCEL_OPERATION_KIND: &str = "trade.cancel.v1";
#[cfg(feature = "runtime")]
pub const TRADE_RESUME_OPERATION_KIND: &str = "trade.resume_operation.v1";
#[cfg(feature = "runtime")]
pub const TRADE_QUERY_DEFAULT_LIMIT: u32 = 50;
#[cfg(feature = "runtime")]
pub const TRADE_QUERY_MAX_LIMIT: u32 = 100;
#[cfg(feature = "runtime")]
pub const TRADE_RUNTIME_CAPABILITY_API_VERSION: u16 = 1;
#[cfg(feature = "runtime")]
pub const TRADE_RUNTIME_PROTOCOL_PROFILE_ID: &str = "radroots.trade.protocol.v1";
#[cfg(feature = "runtime")]
pub const TRADE_RUNTIME_WIRE_PROFILE_ID: &str = "radroots.trade.nostr_regular_immutable_jcs.v1";
#[cfg(feature = "runtime")]
pub const TRADE_RUNTIME_STORAGE_PROFILE_ID: &str = "radroots.sdk.trade.sqlite.v1";
#[cfg(feature = "runtime")]
pub const TRADE_RUNTIME_PRIVATE_STORAGE_PROFILE_ID: &str =
    "radroots.sdk.trade.private_artifacts.v1";
#[cfg(feature = "runtime")]
const TRADE_MUTATION_QUERY_LIMIT: u32 = 1_000;
#[cfg(feature = "runtime")]
const TRADE_LIST_CURSOR_VERSION: u8 = 1;

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeCommandService<'client> {
    sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeCommandService<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn submit_proposal(
        &self,
        request: SubmitProposalRequest,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::SubmitProposal(request);
        enqueue_configured_trade_command(self.sdk, command).await
    }

    pub async fn submit_proposal_with_explicit_signer(
        &self,
        request: SubmitProposalRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::SubmitProposal(request);
        enqueue_trade_command_with_explicit_signer(self.sdk, command, signer).await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn propose_revision(
        &self,
        request: ProposeRevisionRequest,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::ProposeRevision(request);
        enqueue_configured_trade_command(self.sdk, command).await
    }

    pub async fn propose_revision_with_explicit_signer(
        &self,
        request: ProposeRevisionRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::ProposeRevision(request);
        enqueue_trade_command_with_explicit_signer(self.sdk, command, signer).await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn decide_candidate(
        &self,
        request: DecideCandidateRequest,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::DecideCandidate(request);
        enqueue_configured_trade_command(self.sdk, command).await
    }

    pub async fn decide_candidate_with_explicit_signer(
        &self,
        request: DecideCandidateRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::DecideCandidate(request);
        enqueue_trade_command_with_explicit_signer(self.sdk, command, signer).await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn cancel_trade(
        &self,
        request: CancelTradeRequest,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::CancelTrade(request);
        enqueue_configured_trade_command(self.sdk, command).await
    }

    pub async fn cancel_trade_with_explicit_signer(
        &self,
        request: CancelTradeRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::CancelTrade(request);
        enqueue_trade_command_with_explicit_signer(self.sdk, command, signer).await
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn resume_operation(
        &self,
        request: ResumeOperationRequest,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::ResumeOperation(request);
        enqueue_configured_trade_command(self.sdk, command).await
    }

    pub async fn resume_operation_with_explicit_signer(
        &self,
        request: ResumeOperationRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<TradeCommandReceipt, RadrootsSdkError> {
        let command = TradeCommandRequest::ResumeOperation(request);
        enqueue_trade_command_with_explicit_signer(self.sdk, command, signer).await
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeQueryService<'client> {
    sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeQueryService<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }

    pub async fn get_trade(
        &self,
        request: GetTradeRequest,
    ) -> Result<TradeStatusView, RadrootsSdkError> {
        trade_status_view(self.sdk, &request.trade_id).await
    }

    pub async fn list_trades(
        &self,
        request: ListTradesRequest,
    ) -> Result<Page<TradeSummaryView>, RadrootsSdkError> {
        list_trade_views(self.sdk, request).await
    }

    pub async fn refresh_evidence(
        &self,
        request: RefreshTradeEvidenceRequest,
    ) -> Result<EvidenceRefreshReceipt, RadrootsSdkError> {
        let view = trade_status_view(self.sdk, &request.trade_id).await?;
        let last = last_trade_mutation_snapshot(self.sdk, &request.trade_id).await?;
        let checkpoint = RadrootsTradeProjectionCheckpoint {
            trade_id: view.trade_id.clone(),
            reducer_contract_id: RADROOTS_TRADE_REDUCER_CONTRACT_ID.to_owned(),
            reducer_version: RADROOTS_TRADE_REDUCER_VERSION,
            projection_digest: view.projection.projection_digest.clone(),
            root_mutation_id: view.projection.root_mutation_id.clone(),
            negotiation_state: enum_label(&view.projection.negotiation_state)?,
            agreement_state: enum_label(&view.projection.agreement_state)?,
            evidence_state: enum_label(&view.projection.evidence_state)?,
            conflict_state: enum_label(&view.projection.conflict_state)?,
            private_terms_state: enum_label(&view.projection.private_terms_state)?,
            attestation_state: enum_label(&view.projection.attestation_state)?,
            fulfillment_state: enum_label(&view.projection.fulfillment_state)?,
            payment_state: enum_label(&view.projection.payment_state)?,
            projection_json: serde_json::to_string(&view.projection)
                .map_err(trade_query_store_error)?,
            last_mutation_id: last.mutation_id,
            last_transport_event_seq: last.event_seq,
            updated_at_ms: sdk_now_ms(self.sdk)?,
        };
        self.sdk
            ._event_store
            .update_trade_projection_checkpoint(&checkpoint)
            .await?;
        Ok(EvidenceRefreshReceipt {
            api_version: 1,
            trade_id: view.trade_id,
            evidence_count: view.private_terms.len(),
            projection_digest: view.projection.projection_digest,
            projection_state: view.projection.private_terms_state,
        })
    }

    pub async fn inspect_evidence(
        &self,
        request: InspectEvidenceRequest,
    ) -> Result<Page<EvidenceView>, RadrootsSdkError> {
        inspect_evidence_views(self.sdk, request).await
    }
}

#[cfg(feature = "runtime")]
impl<'client> TradesClient<'client> {
    pub fn capabilities(&self) -> TradeRuntimeCapabilityReport {
        trade_runtime_capabilities()
    }

    pub fn commands(&self) -> TradeCommandService<'client> {
        TradeCommandService::new(self.sdk)
    }

    pub fn queries(&self) -> TradeQueryService<'client> {
        TradeQueryService::new(self.sdk)
    }

    pub async fn seal_private_artifact(
        &self,
        request: TradePrivateArtifactSealRequest,
    ) -> Result<TradePrivateArtifactSealReceipt, RadrootsSdkError> {
        seal_private_artifact(self.sdk, request).await
    }

    pub async fn open_private_artifact(
        &self,
        request: TradePrivateArtifactOpenRequest,
    ) -> Result<Option<TradePrivateArtifactOpenReceipt>, RadrootsSdkError> {
        open_private_artifact(self.sdk, request).await
    }

    pub async fn delete_private_artifact(
        &self,
        request: TradePrivateArtifactDeleteRequest,
    ) -> Result<TradePrivateArtifactDeleteReceipt, RadrootsSdkError> {
        delete_private_artifact(self.sdk, request).await
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct SubmitProposalRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub target_policy: TargetPolicy,
    pub satisfaction_policy: SatisfactionPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
}

#[cfg(feature = "runtime")]
impl SubmitProposalRequest {
    pub fn new(
        actor: RadrootsActorContext,
        envelope: RadrootsTradeMutationEnvelopeV1,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            envelope,
            target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: None,
        }
    }

    pub fn with_satisfaction_policy(mut self, policy: SatisfactionPolicy) -> Self {
        self.satisfaction_policy = policy;
        self
    }

    pub fn with_idempotency_key(mut self, key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(key)?);
        Ok(self)
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ProposeRevisionRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub target_policy: TargetPolicy,
    pub satisfaction_policy: SatisfactionPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
}

#[cfg(feature = "runtime")]
impl ProposeRevisionRequest {
    pub fn new(
        actor: RadrootsActorContext,
        envelope: RadrootsTradeMutationEnvelopeV1,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            envelope,
            target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: None,
        }
    }

    pub fn with_satisfaction_policy(mut self, policy: SatisfactionPolicy) -> Self {
        self.satisfaction_policy = policy;
        self
    }

    pub fn with_idempotency_key(mut self, key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct DecideCandidateRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub target_policy: TargetPolicy,
    pub satisfaction_policy: SatisfactionPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub private_terms_acknowledged: bool,
}

#[cfg(feature = "runtime")]
impl DecideCandidateRequest {
    pub fn new(
        actor: RadrootsActorContext,
        envelope: RadrootsTradeMutationEnvelopeV1,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            envelope,
            target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: None,
            private_terms_acknowledged: false,
        }
    }

    pub fn with_satisfaction_policy(mut self, policy: SatisfactionPolicy) -> Self {
        self.satisfaction_policy = policy;
        self
    }

    pub fn with_idempotency_key(mut self, key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn acknowledge_private_terms(mut self) -> Self {
        self.private_terms_acknowledged = true;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct CancelTradeRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub target_policy: TargetPolicy,
    pub satisfaction_policy: SatisfactionPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
}

#[cfg(feature = "runtime")]
impl CancelTradeRequest {
    pub fn new(
        actor: RadrootsActorContext,
        envelope: RadrootsTradeMutationEnvelopeV1,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            envelope,
            target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: None,
        }
    }

    pub fn with_idempotency_key(mut self, key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ResumeOperationRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub envelope: RadrootsTradeMutationEnvelopeV1,
    pub operation_kind: &'static str,
    pub target_policy: TargetPolicy,
    pub satisfaction_policy: SatisfactionPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
    pub private_terms_acknowledged: bool,
}

#[cfg(feature = "runtime")]
impl ResumeOperationRequest {
    pub fn new(
        actor: RadrootsActorContext,
        envelope: RadrootsTradeMutationEnvelopeV1,
        operation_kind: &'static str,
        target_policy: TargetPolicy,
    ) -> Self {
        Self {
            actor,
            envelope,
            operation_kind,
            target_policy,
            satisfaction_policy: SatisfactionPolicy::AllAccepted,
            idempotency_key: None,
            private_terms_acknowledged: false,
        }
    }

    pub fn with_idempotency_key(mut self, key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn acknowledge_private_terms(mut self) -> Self {
        self.private_terms_acknowledged = true;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeCommandLifecycleState {
    Committed,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradeCommandReceipt {
    pub api_version: u16,
    pub operation_kind: String,
    pub operation_state: TradeCommandLifecycleState,
    pub trade_id: RadrootsTradeId,
    pub mutation_id: RadrootsTradeMutationId,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub delivery_state: SdkMutationState,
    pub projection_state: Option<RadrootsTradeProjectionV1>,
    pub idempotency_digest_prefix: String,
    pub recovery_actions: Vec<RadrootsSdkRecoveryAction>,
    pub warnings: Vec<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradePrivateArtifactKind {
    BindingTerms,
    Message,
    ContactBundle,
    DeliveryInstruction,
}

#[cfg(feature = "runtime")]
impl From<TradePrivateArtifactKind> for SdkPrivateTradeArtifactKind {
    fn from(value: TradePrivateArtifactKind) -> Self {
        match value {
            TradePrivateArtifactKind::BindingTerms => Self::BindingTerms,
            TradePrivateArtifactKind::Message => Self::Message,
            TradePrivateArtifactKind::ContactBundle => Self::ContactBundle,
            TradePrivateArtifactKind::DeliveryInstruction => Self::DeliveryInstruction,
        }
    }
}

#[cfg(feature = "runtime")]
impl From<SdkPrivateTradeArtifactKind> for TradePrivateArtifactKind {
    fn from(value: SdkPrivateTradeArtifactKind) -> Self {
        match value {
            SdkPrivateTradeArtifactKind::BindingTerms => Self::BindingTerms,
            SdkPrivateTradeArtifactKind::Message => Self::Message,
            SdkPrivateTradeArtifactKind::ContactBundle => Self::ContactBundle,
            SdkPrivateTradeArtifactKind::DeliveryInstruction => Self::DeliveryInstruction,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct TradePrivateArtifactSealRequest {
    pub artifact_id: String,
    pub trade_id: RadrootsTradeId,
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub artifact_kind: TradePrivateArtifactKind,
    pub schema_id: String,
    pub plaintext: Vec<u8>,
    pub retention_class: String,
    pub expires_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
impl TradePrivateArtifactSealRequest {
    pub fn binding_terms(
        artifact_id: impl Into<String>,
        trade_id: RadrootsTradeId,
        schema_id: impl Into<String>,
        plaintext: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            trade_id,
            candidate_id: None,
            artifact_kind: TradePrivateArtifactKind::BindingTerms,
            schema_id: schema_id.into(),
            plaintext: plaintext.into(),
            retention_class: "trade_private_terms".to_owned(),
            expires_at_ms: None,
        }
    }

    pub fn with_retention_class(mut self, retention_class: impl Into<String>) -> Self {
        self.retention_class = retention_class.into();
        self
    }

    pub fn with_candidate_id(mut self, candidate_id: RadrootsTradeCandidateId) -> Self {
        self.candidate_id = Some(candidate_id);
        self
    }

    pub fn with_expires_at_ms(mut self, expires_at_ms: i64) -> Self {
        self.expires_at_ms = Some(expires_at_ms);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradePrivateArtifactSealReceipt {
    pub artifact_id: String,
    pub trade_id: RadrootsTradeId,
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub artifact_kind: TradePrivateArtifactKind,
    pub schema_id: String,
    pub ciphertext_commitment: String,
    pub private_terms_ref: Option<RadrootsTradePrivateTermsRefV1>,
    pub retention_class: String,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct TradePrivateArtifactOpenRequest {
    pub artifact_id: String,
}

#[cfg(feature = "runtime")]
impl TradePrivateArtifactOpenRequest {
    pub fn new(artifact_id: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradePrivateArtifactOpenReceipt {
    pub artifact_id: String,
    pub trade_id: RadrootsTradeId,
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub artifact_kind: TradePrivateArtifactKind,
    pub schema_id: String,
    pub plaintext: Vec<u8>,
    pub retention_class: String,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub deleted_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct TradePrivateArtifactDeleteRequest {
    pub artifact_id: String,
}

#[cfg(feature = "runtime")]
impl TradePrivateArtifactDeleteRequest {
    pub fn new(artifact_id: impl Into<String>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradePrivateArtifactDeleteReceipt {
    pub artifact_id: String,
    pub deleted: bool,
    pub deleted_at_ms: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TradeRuntimeCapabilityReport {
    pub api_version: u16,
    pub protocol: TradeProtocolCapabilityReport,
    pub storage: TradeStorageCapabilityReport,
    pub core_mvp: TradeCoreMvpCapabilityReport,
    pub optional_integrations: TradeOptionalIntegrationCapabilityReport,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TradeProtocolCapabilityReport {
    pub protocol_profile_id: &'static str,
    pub wire_profile_id: &'static str,
    pub schema_version: u16,
    pub mutation_contract_ids: Vec<&'static str>,
    pub mutation_event_kinds: Vec<u32>,
    pub reducer_contract_id: &'static str,
    pub reducer_version: u16,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TradeStorageCapabilityReport {
    pub storage_profile_id: &'static str,
    pub private_storage_profile_id: &'static str,
    pub private_store_schema_version: i64,
    pub max_private_artifact_bytes: usize,
    pub private_artifact_kinds: Vec<TradePrivateArtifactKind>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TradeCoreMvpCapabilityReport {
    pub commands: bool,
    pub queries: bool,
    pub local_event_store: bool,
    pub semantic_outbox: bool,
    pub protected_private_artifacts: bool,
    pub backup_restore: bool,
    pub local_signer: bool,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct TradeOptionalIntegrationCapabilityReport {
    pub myc_nip46_signer: bool,
    pub radrootsd_execution: bool,
    pub rhi_attestation: bool,
    pub tangle_transport: bool,
    pub reticulum_transport: bool,
}

#[cfg(feature = "runtime")]
fn trade_runtime_capabilities() -> TradeRuntimeCapabilityReport {
    TradeRuntimeCapabilityReport {
        api_version: TRADE_RUNTIME_CAPABILITY_API_VERSION,
        protocol: TradeProtocolCapabilityReport {
            protocol_profile_id: TRADE_RUNTIME_PROTOCOL_PROFILE_ID,
            wire_profile_id: TRADE_RUNTIME_WIRE_PROFILE_ID,
            schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
            mutation_contract_ids: RADROOTS_TRADE_MUTATION_CONTRACT_IDS.to_vec(),
            mutation_event_kinds: TRADE_MUTATION_EVENT_KINDS.to_vec(),
            reducer_contract_id: RADROOTS_TRADE_REDUCER_CONTRACT_ID,
            reducer_version: RADROOTS_TRADE_REDUCER_VERSION,
        },
        storage: TradeStorageCapabilityReport {
            storage_profile_id: TRADE_RUNTIME_STORAGE_PROFILE_ID,
            private_storage_profile_id: TRADE_RUNTIME_PRIVATE_STORAGE_PROFILE_ID,
            private_store_schema_version: SDK_PRIVATE_STORE_SCHEMA_VERSION,
            max_private_artifact_bytes: RADROOTS_TRADE_MAX_PRIVATE_ARTIFACT_BYTES,
            private_artifact_kinds: vec![
                TradePrivateArtifactKind::BindingTerms,
                TradePrivateArtifactKind::Message,
                TradePrivateArtifactKind::ContactBundle,
                TradePrivateArtifactKind::DeliveryInstruction,
            ],
        },
        core_mvp: TradeCoreMvpCapabilityReport {
            commands: true,
            queries: true,
            local_event_store: true,
            semantic_outbox: true,
            protected_private_artifacts: true,
            backup_restore: true,
            local_signer: cfg!(feature = "local-signer"),
        },
        optional_integrations: TradeOptionalIntegrationCapabilityReport {
            myc_nip46_signer: cfg!(feature = "signer-adapters"),
            radrootsd_execution: cfg!(feature = "radrootsd-execution"),
            rhi_attestation: false,
            tangle_transport: false,
            reticulum_transport: false,
        },
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct GetTradeRequest {
    pub trade_id: RadrootsTradeId,
}

#[cfg(feature = "runtime")]
impl GetTradeRequest {
    pub fn new(trade_id: RadrootsTradeId) -> Self {
        Self { trade_id }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct RefreshTradeEvidenceRequest {
    pub trade_id: RadrootsTradeId,
}

#[cfg(feature = "runtime")]
impl RefreshTradeEvidenceRequest {
    pub fn new(trade_id: RadrootsTradeId) -> Self {
        Self { trade_id }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct InspectEvidenceRequest {
    pub trade_id: RadrootsTradeId,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[cfg(feature = "runtime")]
impl InspectEvidenceRequest {
    pub fn new(trade_id: RadrootsTradeId) -> Self {
        Self {
            trade_id,
            limit: None,
            cursor: None,
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TradeListFilter {
    pub buyer_pubkey: Option<String>,
    pub seller_pubkey: Option<String>,
    pub participant_pubkeys_any_of: Vec<String>,
    pub agreement_states_any_of: Vec<RadrootsTradeAgreementStateV1>,
    pub any_of: Vec<TradeListAnyOf>,
}

#[cfg(feature = "runtime")]
impl TradeListFilter {
    pub fn buyer(mut self, pubkey: impl Into<String>) -> Self {
        self.buyer_pubkey = Some(pubkey.into());
        self
    }

    pub fn seller(mut self, pubkey: impl Into<String>) -> Self {
        self.seller_pubkey = Some(pubkey.into());
        self
    }

    pub fn participant_any_of<I, S>(mut self, pubkeys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.participant_pubkeys_any_of = pubkeys.into_iter().map(Into::into).collect();
        self
    }

    pub fn agreement_states_any_of<I>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = RadrootsTradeAgreementStateV1>,
    {
        self.agreement_states_any_of = states.into_iter().collect();
        self
    }

    pub fn any_of<I>(mut self, clauses: I) -> Self
    where
        I: IntoIterator<Item = TradeListAnyOf>,
    {
        self.any_of = clauses.into_iter().collect();
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum TradeListAnyOf {
    TradeId(String),
    BuyerPubkey(String),
    SellerPubkey(String),
    ParticipantPubkey(String),
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeListSort {
    UpdatedDesc,
}

#[cfg(feature = "runtime")]
impl Default for TradeListSort {
    fn default() -> Self {
        Self::UpdatedDesc
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ListTradesRequest {
    pub filter: TradeListFilter,
    pub sort: TradeListSort,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[cfg(feature = "runtime")]
impl ListTradesRequest {
    pub fn new() -> Self {
        Self {
            filter: TradeListFilter::default(),
            sort: TradeListSort::UpdatedDesc,
            limit: None,
            cursor: None,
        }
    }

    pub fn with_filter(mut self, filter: TradeListFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }
}

#[cfg(feature = "runtime")]
impl Default for ListTradesRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradeStatusView {
    pub trade_id: RadrootsTradeId,
    pub projection: RadrootsTradeProjectionV1,
    pub source_event_count: usize,
    pub private_terms: Vec<TradePrivateTermsAvailabilityView>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradeSummaryView {
    pub trade_id: RadrootsTradeId,
    pub root_mutation_id: Option<RadrootsTradeMutationId>,
    pub buyer_pubkey: Option<String>,
    pub seller_pubkey: Option<String>,
    pub farm_id: Option<String>,
    pub negotiation_state: RadrootsTradeNegotiationStateV1,
    pub agreement_state: RadrootsTradeAgreementStateV1,
    pub evidence_state: RadrootsTradeEvidenceStateV1,
    pub conflict_state: RadrootsTradeConflictStateV1,
    pub private_terms_state: RadrootsTradePrivateTermsStateV1,
    pub attestation_state: RadrootsTradeAttestationStateV1,
    pub fulfillment_state: RadrootsTradeFulfillmentStateV1,
    pub payment_state: RadrootsTradePaymentStateV1,
    pub projection_digest: String,
    pub source_event_count: usize,
    pub updated_event_seq: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradePrivateTermsAvailabilityView {
    pub candidate_id: RadrootsTradeCandidateId,
    pub artifact_id: Option<String>,
    pub schema_id: Option<String>,
    pub ciphertext_commitment: Option<String>,
    pub state: RadrootsTradePrivateTermsStateV1,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EvidenceRefreshReceipt {
    pub api_version: u16,
    pub trade_id: RadrootsTradeId,
    pub evidence_count: usize,
    pub projection_digest: String,
    pub projection_state: RadrootsTradePrivateTermsStateV1,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EvidenceView {
    pub artifact_id: String,
    pub trade_id: RadrootsTradeId,
    pub candidate_id: Option<RadrootsTradeCandidateId>,
    pub artifact_kind: TradePrivateArtifactKind,
    pub schema_id: String,
    pub ciphertext_commitment: String,
    pub retention_class: String,
    pub state: RadrootsTradePrivateTermsStateV1,
    pub created_at_ms: i64,
    pub expires_at_ms: Option<i64>,
    pub deleted_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
struct TradeCommandPlan {
    operation_kind: &'static str,
    actor: RadrootsActorContext,
    frozen_draft: RadrootsEventDraft,
    trade_id: RadrootsTradeId,
    mutation_id: RadrootsTradeMutationId,
    target_policy: TargetPolicy,
    satisfaction_policy: SatisfactionPolicy,
    idempotency_key: Option<SdkIdempotencyKey>,
}

#[cfg(feature = "runtime")]
enum TradeCommandRequest {
    SubmitProposal(SubmitProposalRequest),
    ProposeRevision(ProposeRevisionRequest),
    DecideCandidate(DecideCandidateRequest),
    CancelTrade(CancelTradeRequest),
    ResumeOperation(ResumeOperationRequest),
}

#[cfg(feature = "runtime")]
impl TradeCommandRequest {
    fn operation_kind(&self) -> &'static str {
        match self {
            Self::SubmitProposal(_) => TRADE_SUBMIT_PROPOSAL_OPERATION_KIND,
            Self::ProposeRevision(_) => TRADE_PROPOSE_REVISION_OPERATION_KIND,
            Self::DecideCandidate(_) => TRADE_DECIDE_CANDIDATE_OPERATION_KIND,
            Self::CancelTrade(_) => TRADE_CANCEL_OPERATION_KIND,
            Self::ResumeOperation(request) => request.operation_kind,
        }
    }

    fn into_parts(
        self,
    ) -> (
        RadrootsActorContext,
        RadrootsTradeMutationEnvelopeV1,
        TargetPolicy,
        SatisfactionPolicy,
        Option<SdkIdempotencyKey>,
        bool,
    ) {
        match self {
            Self::SubmitProposal(request) => (
                request.actor,
                request.envelope,
                request.target_policy,
                request.satisfaction_policy,
                request.idempotency_key,
                false,
            ),
            Self::ProposeRevision(request) => (
                request.actor,
                request.envelope,
                request.target_policy,
                request.satisfaction_policy,
                request.idempotency_key,
                false,
            ),
            Self::DecideCandidate(request) => (
                request.actor,
                request.envelope,
                request.target_policy,
                request.satisfaction_policy,
                request.idempotency_key,
                request.private_terms_acknowledged,
            ),
            Self::CancelTrade(request) => (
                request.actor,
                request.envelope,
                request.target_policy,
                request.satisfaction_policy,
                request.idempotency_key,
                false,
            ),
            Self::ResumeOperation(request) => (
                request.actor,
                request.envelope,
                request.target_policy,
                request.satisfaction_policy,
                request.idempotency_key,
                request.private_terms_acknowledged,
            ),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TradeListCursorPayload {
    version: u8,
    sort: TradeListSort,
    filter_digest: String,
    updated_event_seq: i64,
    trade_id: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
struct TradeListRow {
    trade_id: RadrootsTradeId,
    updated_event_seq: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
struct LastTradeMutationSnapshot {
    mutation_id: Option<RadrootsTradeMutationId>,
    event_seq: Option<i64>,
}

#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
async fn enqueue_configured_trade_command(
    sdk: &RadrootsClient,
    request: TradeCommandRequest,
) -> Result<TradeCommandReceipt, RadrootsSdkError> {
    let plan = trade_command_plan(sdk, request).await?;
    let enqueue = enqueue_configured_signed_workflow(sdk, workflow_request(&plan)).await?;
    trade_command_receipt(sdk, plan, enqueue).await
}

#[cfg(feature = "runtime")]
async fn enqueue_trade_command_with_explicit_signer(
    sdk: &RadrootsClient,
    request: TradeCommandRequest,
    signer: &dyn RadrootsEventSigner,
) -> Result<TradeCommandReceipt, RadrootsSdkError> {
    let plan = trade_command_plan(sdk, request).await?;
    let enqueue = enqueue_signed_workflow(sdk, workflow_request(&plan), signer).await?;
    trade_command_receipt(sdk, plan, enqueue).await
}

#[cfg(feature = "runtime")]
async fn trade_command_plan(
    sdk: &RadrootsClient,
    request: TradeCommandRequest,
) -> Result<TradeCommandPlan, RadrootsSdkError> {
    let operation_kind = request.operation_kind();
    let (actor, envelope, target_policy, satisfaction_policy, idempotency_key, acknowledged) =
        request.into_parts();
    validate_operation_body(operation_kind, &envelope)?;
    validate_actor_matches_envelope(operation_kind, &actor, &envelope)?;
    let wire = trade_mutation_event_build(envelope.clone()).map_err(|error| {
        trade_command_error(
            RadrootsSdkTradeErrorKind::InvalidEnvelope,
            operation_kind,
            error.to_string(),
        )
    })?;
    let canonical =
        trade_mutation_from_canonical_content(wire.content.as_str()).map_err(|error| {
            trade_command_error(
                RadrootsSdkTradeErrorKind::InvalidEnvelope,
                operation_kind,
                error.to_string(),
            )
        })?;
    validate_command_private_terms(sdk, operation_kind, &canonical, acknowledged).await?;
    let mutation_id = canonical.mutation_id.clone().ok_or_else(|| {
        trade_command_error(
            RadrootsSdkTradeErrorKind::InvalidEnvelope,
            operation_kind,
            "canonical trade mutation is missing mutation id",
        )
    })?;
    let frozen_draft = RadrootsEventDraft::new(
        canonical.contract_id.as_str(),
        wire.kind,
        canonical.authored_at_unix_s,
        wire.tags,
        wire.content,
        actor.pubkey().as_str(),
    )
    .map_err(|error| {
        trade_command_error(
            RadrootsSdkTradeErrorKind::InvalidEnvelope,
            operation_kind,
            error.to_string(),
        )
    })?;
    Ok(TradeCommandPlan {
        operation_kind,
        actor,
        frozen_draft,
        trade_id: canonical.trade_id.clone(),
        mutation_id,
        target_policy,
        satisfaction_policy,
        idempotency_key,
    })
}

#[cfg(feature = "runtime")]
fn workflow_request(plan: &TradeCommandPlan) -> SdkWorkflowEnqueueRequest<'_> {
    SdkWorkflowEnqueueRequest {
        operation_kind: plan.operation_kind,
        actor: &plan.actor,
        frozen_draft: &plan.frozen_draft,
        target_policy: plan.target_policy.clone(),
        satisfaction_policy: plan.satisfaction_policy.clone(),
        idempotency_key: plan.idempotency_key.clone(),
    }
}

#[cfg(feature = "runtime")]
async fn trade_command_receipt(
    sdk: &RadrootsClient,
    plan: TradeCommandPlan,
    enqueue: SdkWorkflowEnqueueReceipt,
) -> Result<TradeCommandReceipt, RadrootsSdkError> {
    let projection_state = trade_projection_for_trade(sdk, &plan.trade_id).await.ok();
    Ok(TradeCommandReceipt {
        api_version: 1,
        operation_kind: plan.operation_kind.to_owned(),
        operation_state: TradeCommandLifecycleState::Committed,
        trade_id: plan.trade_id,
        mutation_id: plan.mutation_id,
        expected_event_id: RadrootsEventId::parse(plan.frozen_draft.expected_event_id_str())
            .expect("trade workflow draft has a valid expected event id"),
        signed_event_id: enqueue.signed_event_id,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        delivery_state: enqueue.state.into(),
        projection_state,
        idempotency_digest_prefix: enqueue.idempotency_digest_prefix,
        recovery_actions: Vec::new(),
        warnings: Vec::new(),
    })
}

#[cfg(feature = "runtime")]
fn validate_operation_body(
    operation_kind: &'static str,
    envelope: &RadrootsTradeMutationEnvelopeV1,
) -> Result<(), RadrootsSdkError> {
    let valid = match operation_kind {
        TRADE_SUBMIT_PROPOSAL_OPERATION_KIND => {
            matches!(envelope.body, RadrootsTradeMutationBodyV1::Proposal { .. })
        }
        TRADE_PROPOSE_REVISION_OPERATION_KIND => {
            matches!(
                envelope.body,
                RadrootsTradeMutationBodyV1::RevisionProposal { .. }
            )
        }
        TRADE_DECIDE_CANDIDATE_OPERATION_KIND | TRADE_RESUME_OPERATION_KIND => matches!(
            envelope.body,
            RadrootsTradeMutationBodyV1::Decision { .. }
                | RadrootsTradeMutationBodyV1::RevisionDecision { .. }
        ),
        TRADE_CANCEL_OPERATION_KIND => {
            matches!(
                envelope.body,
                RadrootsTradeMutationBodyV1::Cancellation { .. }
            )
        }
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(trade_command_error(
            RadrootsSdkTradeErrorKind::InvalidCommandBody,
            operation_kind,
            "trade command operation kind does not match mutation body",
        ))
    }
}

#[cfg(feature = "runtime")]
fn validate_actor_matches_envelope(
    operation_kind: &'static str,
    actor: &RadrootsActorContext,
    envelope: &RadrootsTradeMutationEnvelopeV1,
) -> Result<(), RadrootsSdkError> {
    if actor.pubkey().as_str() == envelope.author_pubkey.as_str() {
        Ok(())
    } else {
        Err(RadrootsSdkError::UnauthorizedActor {
            operation: operation_kind.to_owned(),
            reason: "actor pubkey must match trade mutation author_pubkey".to_owned(),
        })
    }
}

#[cfg(feature = "runtime")]
async fn validate_command_private_terms(
    sdk: &RadrootsClient,
    operation_kind: &'static str,
    envelope: &RadrootsTradeMutationEnvelopeV1,
    private_terms_acknowledged: bool,
) -> Result<(), RadrootsSdkError> {
    match &envelope.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            let Some(candidate_id) = &candidate.candidate_id else {
                return Ok(());
            };
            if candidate.fulfillment.requires_private_terms && candidate.private_terms.is_none() {
                return Err(trade_command_error(
                    RadrootsSdkTradeErrorKind::PrivateArtifactMissing,
                    operation_kind,
                    "candidate requires private terms but no private terms reference is present",
                ));
            }
            if let Some(private_ref) = &candidate.private_terms {
                ensure_private_terms_ref_available(
                    sdk,
                    operation_kind,
                    &envelope.trade_id,
                    candidate_id,
                    private_ref,
                )
                .await?;
            }
            Ok(())
        }
        RadrootsTradeMutationBodyV1::Decision {
            proposal_mutation_id,
            candidate_id,
            decision,
        }
        | RadrootsTradeMutationBodyV1::RevisionDecision {
            proposal_mutation_id,
            candidate_id,
            decision,
        } => {
            if !matches!(decision, RadrootsTradeDecisionV1::Accepted { .. }) {
                return Ok(());
            }
            let Some(candidate_ref) = referenced_candidate_for_decision(
                sdk,
                operation_kind,
                proposal_mutation_id,
                candidate_id,
            )
            .await?
            else {
                return Ok(());
            };
            let requires_private_terms = candidate_ref.fulfillment.requires_private_terms
                || candidate_ref.private_terms.is_some();
            if !requires_private_terms {
                return Ok(());
            }
            let Some(private_ref) = &candidate_ref.private_terms else {
                return Err(trade_command_error(
                    RadrootsSdkTradeErrorKind::PrivateArtifactMissing,
                    operation_kind,
                    "accepted candidate requires private terms but no private terms reference is present",
                ));
            };
            if private_ref.required_acknowledgement && !private_terms_acknowledged {
                return Err(trade_command_error(
                    RadrootsSdkTradeErrorKind::PrivateArtifactAcknowledgementMissing,
                    operation_kind,
                    "accepted candidate private terms require explicit acknowledgement",
                ));
            }
            ensure_private_terms_ref_available(
                sdk,
                operation_kind,
                &envelope.trade_id,
                candidate_id,
                private_ref,
            )
            .await
        }
        RadrootsTradeMutationBodyV1::Cancellation { .. } => Ok(()),
    }
}

#[cfg(feature = "runtime")]
async fn referenced_candidate_for_decision(
    sdk: &RadrootsClient,
    operation_kind: &'static str,
    proposal_mutation_id: &RadrootsTradeMutationId,
    candidate_id: &RadrootsTradeCandidateId,
) -> Result<Option<radroots_event::trade::RadrootsTradeCandidateTermsV1>, RadrootsSdkError> {
    let Some(stored) = sdk
        ._event_store
        .get_trade_mutation(proposal_mutation_id)
        .await?
    else {
        return Err(trade_command_error(
            RadrootsSdkTradeErrorKind::TradeNotFound,
            operation_kind,
            "referenced proposal mutation is not present in the local event store",
        ));
    };
    let envelope = stored_trade_envelope(&stored)?;
    let candidate = match envelope.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => candidate,
        _ => {
            return Err(trade_command_error(
                RadrootsSdkTradeErrorKind::InvalidCommandBody,
                operation_kind,
                "referenced mutation is not a candidate proposal",
            ));
        }
    };
    if candidate.candidate_id.as_ref() != Some(candidate_id) {
        return Err(trade_command_error(
            RadrootsSdkTradeErrorKind::InvalidCommandBody,
            operation_kind,
            "decision candidate_id does not match referenced proposal candidate_id",
        ));
    }
    Ok(Some(candidate))
}

#[cfg(feature = "runtime")]
async fn ensure_private_terms_ref_available(
    sdk: &RadrootsClient,
    operation_kind: &'static str,
    trade_id: &RadrootsTradeId,
    candidate_id: &RadrootsTradeCandidateId,
    private_ref: &RadrootsTradePrivateTermsRefV1,
) -> Result<(), RadrootsSdkError> {
    let evidence = sdk
        ._private_store
        .private_terms_evidence(
            trade_id.as_str(),
            candidate_id.as_str(),
            private_ref.artifact_id.as_str(),
            private_ref.schema_id.as_str(),
            private_ref.ciphertext_commitment.as_str(),
        )
        .await?;
    match evidence.state {
        RadrootsTradePrivateTermsStateV1::AvailableVerified => Ok(()),
        RadrootsTradePrivateTermsStateV1::CommitmentMismatch => Err(trade_command_error(
            RadrootsSdkTradeErrorKind::PrivateArtifactCommitmentMismatch,
            operation_kind,
            "private artifact commitment does not match candidate private terms reference",
        )),
        _ => Err(trade_command_error(
            RadrootsSdkTradeErrorKind::PrivateArtifactMissing,
            operation_kind,
            "private artifact is unavailable for candidate private terms reference",
        )),
    }
}

#[cfg(feature = "runtime")]
async fn seal_private_artifact(
    sdk: &RadrootsClient,
    request: TradePrivateArtifactSealRequest,
) -> Result<TradePrivateArtifactSealReceipt, RadrootsSdkError> {
    let now_ms = sdk_now_ms(sdk)?;
    let input = SdkPrivateTradeArtifactInput {
        artifact_id: request.artifact_id,
        trade_id: request.trade_id.as_str().to_owned(),
        candidate_id: request
            .candidate_id
            .as_ref()
            .map(|candidate_id| candidate_id.as_str().to_owned()),
        artifact_kind: request.artifact_kind.into(),
        schema_id: request.schema_id,
        plaintext: request.plaintext,
        retention_class: request.retention_class,
        created_at_ms: now_ms,
        expires_at_ms: request.expires_at_ms,
    };
    let metadata = sdk._private_store.upsert_trade_artifact(&input).await?;
    Ok(seal_receipt_from_metadata(metadata))
}

#[cfg(feature = "runtime")]
fn seal_receipt_from_metadata(
    metadata: SdkPrivateTradeArtifactMetadata,
) -> TradePrivateArtifactSealReceipt {
    let artifact_kind = TradePrivateArtifactKind::from(metadata.artifact_kind);
    let private_terms_ref = if artifact_kind == TradePrivateArtifactKind::BindingTerms {
        Some(RadrootsTradePrivateTermsRefV1 {
            artifact_id: metadata.artifact_id.clone(),
            schema_id: metadata.schema_id.clone(),
            ciphertext_commitment: metadata.ciphertext_commitment.clone(),
            required_acknowledgement: true,
        })
    } else {
        None
    };
    TradePrivateArtifactSealReceipt {
        artifact_id: metadata.artifact_id,
        trade_id: RadrootsTradeId::parse(metadata.trade_id).expect("stored trade id is valid"),
        candidate_id: parse_optional_candidate_id(metadata.candidate_id),
        artifact_kind,
        schema_id: metadata.schema_id,
        ciphertext_commitment: metadata.ciphertext_commitment,
        private_terms_ref,
        retention_class: metadata.retention_class,
        created_at_ms: metadata.created_at_ms,
        expires_at_ms: metadata.expires_at_ms,
    }
}

#[cfg(feature = "runtime")]
async fn open_private_artifact(
    sdk: &RadrootsClient,
    request: TradePrivateArtifactOpenRequest,
) -> Result<Option<TradePrivateArtifactOpenReceipt>, RadrootsSdkError> {
    let Some(record) = sdk
        ._private_store
        .trade_artifact(request.artifact_id.as_str())
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(TradePrivateArtifactOpenReceipt {
        artifact_id: record.artifact_id,
        trade_id: RadrootsTradeId::parse(record.trade_id).expect("stored trade id is valid"),
        candidate_id: parse_optional_candidate_id(record.candidate_id),
        artifact_kind: record.artifact_kind.into(),
        schema_id: record.schema_id,
        plaintext: record.plaintext,
        retention_class: record.retention_class,
        created_at_ms: record.created_at_ms,
        expires_at_ms: record.expires_at_ms,
        deleted_at_ms: record.deleted_at_ms,
    }))
}

#[cfg(feature = "runtime")]
async fn delete_private_artifact(
    sdk: &RadrootsClient,
    request: TradePrivateArtifactDeleteRequest,
) -> Result<TradePrivateArtifactDeleteReceipt, RadrootsSdkError> {
    let deleted_at_ms = sdk_now_ms(sdk)?;
    let deleted = sdk
        ._private_store
        .delete_trade_artifact(request.artifact_id.as_str(), deleted_at_ms)
        .await?;
    Ok(TradePrivateArtifactDeleteReceipt {
        artifact_id: request.artifact_id,
        deleted,
        deleted_at_ms,
    })
}

#[cfg(feature = "runtime")]
async fn trade_status_view(
    sdk: &RadrootsClient,
    trade_id: &RadrootsTradeId,
) -> Result<TradeStatusView, RadrootsSdkError> {
    let projection = trade_projection_for_trade(sdk, trade_id).await?;
    let private_terms = private_terms_views_for_trade(sdk, trade_id).await?;
    let source_event_count = sdk
        ._event_store
        .trade_mutations_for_trade(trade_id, TRADE_MUTATION_QUERY_LIMIT)
        .await?
        .len();
    Ok(TradeStatusView {
        trade_id: trade_id.clone(),
        projection,
        source_event_count,
        private_terms,
    })
}

#[cfg(feature = "runtime")]
async fn trade_projection_for_trade(
    sdk: &RadrootsClient,
    trade_id: &RadrootsTradeId,
) -> Result<RadrootsTradeProjectionV1, RadrootsSdkError> {
    let stored = sdk
        ._event_store
        .trade_mutations_for_trade(trade_id, TRADE_MUTATION_QUERY_LIMIT)
        .await?;
    if stored.is_empty() {
        return Err(trade_query_error(
            RadrootsSdkTradeErrorKind::TradeNotFound,
            "trade.get",
            "trade is not present in the local event store",
        ));
    }
    let mut input = RadrootsTradeReductionInputV1::new(trade_id.clone());
    input.mutations = stored
        .iter()
        .map(stored_trade_mutation_record)
        .collect::<Result<Vec<_>, _>>()?;
    input.private_terms =
        private_terms_evidence_for_mutations(sdk, trade_id, &input.mutations).await?;
    input.observed_at_unix_s = Some(sdk.now()?.unix_seconds());
    Ok(reduce_trade_records(input))
}

#[cfg(feature = "runtime")]
async fn private_terms_views_for_trade(
    sdk: &RadrootsClient,
    trade_id: &RadrootsTradeId,
) -> Result<Vec<TradePrivateTermsAvailabilityView>, RadrootsSdkError> {
    let stored = sdk
        ._event_store
        .trade_mutations_for_trade(trade_id, TRADE_MUTATION_QUERY_LIMIT)
        .await?;
    let records = stored
        .iter()
        .map(stored_trade_mutation_record)
        .collect::<Result<Vec<_>, _>>()?;
    let evidence = private_terms_evidence_for_mutations(sdk, trade_id, &records).await?;
    let mut evidence_by_candidate = evidence
        .into_iter()
        .map(|item| (item.candidate_id.clone(), item.state))
        .collect::<BTreeMap<_, _>>();
    let mut views = BTreeMap::<RadrootsTradeCandidateId, TradePrivateTermsAvailabilityView>::new();
    for record in records {
        if let Some((candidate_id, private_ref)) = candidate_private_ref(&record.mutation) {
            let state = evidence_by_candidate
                .remove(&candidate_id)
                .unwrap_or(RadrootsTradePrivateTermsStateV1::Missing);
            views.insert(
                candidate_id.clone(),
                TradePrivateTermsAvailabilityView {
                    candidate_id,
                    artifact_id: Some(private_ref.artifact_id.clone()),
                    schema_id: Some(private_ref.schema_id.clone()),
                    ciphertext_commitment: Some(private_ref.ciphertext_commitment.clone()),
                    state,
                },
            );
        }
    }
    Ok(views.into_values().collect())
}

#[cfg(feature = "runtime")]
async fn private_terms_evidence_for_mutations(
    sdk: &RadrootsClient,
    trade_id: &RadrootsTradeId,
    mutations: &[RadrootsTradeMutationRecordV1],
) -> Result<Vec<RadrootsTradePrivateTermsEvidenceV1>, RadrootsSdkError> {
    let mut evidence = Vec::new();
    let mut seen = BTreeSet::new();
    for record in mutations {
        if let Some((candidate_id, private_ref)) = candidate_private_ref(&record.mutation) {
            if !seen.insert(candidate_id.clone()) {
                continue;
            }
            evidence.push(
                sdk._private_store
                    .private_terms_evidence(
                        trade_id.as_str(),
                        candidate_id.as_str(),
                        private_ref.artifact_id.as_str(),
                        private_ref.schema_id.as_str(),
                        private_ref.ciphertext_commitment.as_str(),
                    )
                    .await?,
            );
        }
    }
    Ok(evidence)
}

#[cfg(feature = "runtime")]
fn candidate_private_ref(
    envelope: &RadrootsTradeMutationEnvelopeV1,
) -> Option<(RadrootsTradeCandidateId, RadrootsTradePrivateTermsRefV1)> {
    match &envelope.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate }
        | RadrootsTradeMutationBodyV1::RevisionProposal { candidate } => {
            let candidate_id = candidate.candidate_id.clone()?;
            let private_ref = candidate.private_terms.clone()?;
            Some((candidate_id, private_ref))
        }
        _ => None,
    }
}

#[cfg(feature = "runtime")]
fn stored_trade_mutation_record(
    stored: &RadrootsStoredTradeMutation,
) -> Result<RadrootsTradeMutationRecordV1, RadrootsSdkError> {
    Ok(RadrootsTradeMutationRecordV1 {
        transport_event_id: Some(stored.first_transport_event_id.clone()),
        mutation: stored_trade_envelope(stored)?,
    })
}

#[cfg(feature = "runtime")]
fn stored_trade_envelope(
    stored: &RadrootsStoredTradeMutation,
) -> Result<RadrootsTradeMutationEnvelopeV1, RadrootsSdkError> {
    let content =
        std::str::from_utf8(stored.canonical_payload_bytes.as_slice()).map_err(|error| {
            RadrootsSdkError::Projection {
                message: error.to_string(),
            }
        })?;
    trade_mutation_from_canonical_content(content).map_err(|error| RadrootsSdkError::Projection {
        message: error.to_string(),
    })
}

#[cfg(feature = "runtime")]
async fn list_trade_views(
    sdk: &RadrootsClient,
    request: ListTradesRequest,
) -> Result<Page<TradeSummaryView>, RadrootsSdkError> {
    let limit = bounded_limit(request.limit, "trade.list")?;
    let filter_digest = filter_digest(&request)?;
    let cursor = request
        .cursor
        .as_deref()
        .map(|cursor| decode_trade_list_cursor(cursor, &filter_digest, request.sort))
        .transpose()?;
    let rows = list_trade_rows(sdk, &request, cursor.as_ref(), limit + 1).await?;
    let mut items = Vec::new();
    for row in rows
        .iter()
        .take(usize::try_from(limit).expect("limit fits usize"))
    {
        let view = trade_status_view(sdk, &row.trade_id).await?;
        items.push(TradeSummaryView {
            trade_id: view.trade_id,
            root_mutation_id: view.projection.root_mutation_id,
            buyer_pubkey: view.projection.buyer_pubkey.map(|value| value.to_string()),
            seller_pubkey: view.projection.seller_pubkey.map(|value| value.to_string()),
            farm_id: view.projection.farm_id.map(|value| value.to_string()),
            negotiation_state: view.projection.negotiation_state,
            agreement_state: view.projection.agreement_state,
            evidence_state: view.projection.evidence_state,
            conflict_state: view.projection.conflict_state,
            private_terms_state: view.projection.private_terms_state,
            attestation_state: view.projection.attestation_state,
            fulfillment_state: view.projection.fulfillment_state,
            payment_state: view.projection.payment_state,
            projection_digest: view.projection.projection_digest,
            source_event_count: view.source_event_count,
            updated_event_seq: row.updated_event_seq,
        });
    }
    let next_cursor = if rows.len() > usize::try_from(limit).expect("limit fits usize") {
        rows.get(usize::try_from(limit - 1).expect("limit fits usize"))
            .map(|row| encode_trade_list_cursor(row, &filter_digest, request.sort))
            .transpose()?
            .flatten()
    } else {
        None
    };
    Ok(Page { items, next_cursor })
}

#[cfg(feature = "runtime")]
async fn list_trade_rows(
    sdk: &RadrootsClient,
    request: &ListTradesRequest,
    cursor: Option<&TradeListCursorPayload>,
    limit: u32,
) -> Result<Vec<TradeListRow>, RadrootsSdkError> {
    let mut query: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT m.trade_id, MAX(m.first_event_seq) AS updated_event_seq FROM trade_mutation m",
    );
    if !request.filter.agreement_states_any_of.is_empty() {
        query.push(" JOIN trade_projection_checkpoint c ON c.trade_id = m.trade_id");
    }
    query.push(" WHERE 1 = 1");
    if let Some(buyer_pubkey) = &request.filter.buyer_pubkey {
        query.push(" AND m.buyer_pubkey = ");
        query.push_bind(buyer_pubkey);
    }
    if let Some(seller_pubkey) = &request.filter.seller_pubkey {
        query.push(" AND m.seller_pubkey = ");
        query.push_bind(seller_pubkey);
    }
    if !request.filter.participant_pubkeys_any_of.is_empty() {
        query.push(" AND (m.buyer_pubkey IN (");
        push_string_list(&mut query, &request.filter.participant_pubkeys_any_of);
        query.push(") OR m.seller_pubkey IN (");
        push_string_list(&mut query, &request.filter.participant_pubkeys_any_of);
        query.push("))");
    }
    if !request.filter.agreement_states_any_of.is_empty() {
        let labels = request
            .filter
            .agreement_states_any_of
            .iter()
            .map(enum_label)
            .collect::<Result<Vec<_>, _>>()?;
        query.push(" AND c.agreement_state IN (");
        push_string_list(&mut query, &labels);
        query.push(")");
    }
    if !request.filter.any_of.is_empty() {
        query.push(" AND (");
        for (index, clause) in request.filter.any_of.iter().enumerate() {
            if index > 0 {
                query.push(" OR ");
            }
            match clause {
                TradeListAnyOf::TradeId(value) => {
                    query.push("m.trade_id = ");
                    query.push_bind(value);
                }
                TradeListAnyOf::BuyerPubkey(value) => {
                    query.push("m.buyer_pubkey = ");
                    query.push_bind(value);
                }
                TradeListAnyOf::SellerPubkey(value) => {
                    query.push("m.seller_pubkey = ");
                    query.push_bind(value);
                }
                TradeListAnyOf::ParticipantPubkey(value) => {
                    query.push("(m.buyer_pubkey = ");
                    query.push_bind(value);
                    query.push(" OR m.seller_pubkey = ");
                    query.push_bind(value);
                    query.push(")");
                }
            }
        }
        query.push(")");
    }
    query.push(" GROUP BY m.trade_id");
    if let Some(cursor) = cursor {
        query.push(" HAVING (updated_event_seq < ");
        query.push_bind(cursor.updated_event_seq);
        query.push(" OR (updated_event_seq = ");
        query.push_bind(cursor.updated_event_seq);
        query.push(" AND m.trade_id > ");
        query.push_bind(cursor.trade_id.as_str());
        query.push("))");
    }
    query.push(" ORDER BY updated_event_seq DESC, m.trade_id ASC LIMIT ");
    query.push_bind(i64::from(limit));
    let rows = query
        .build()
        .fetch_all(sdk._event_store.pool())
        .await
        .map_err(trade_query_store_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(TradeListRow {
                trade_id: RadrootsTradeId::parse(
                    row.try_get::<String, _>("trade_id")
                        .map_err(trade_query_store_error)?,
                )
                .map_err(|error| {
                    trade_query_error(
                        RadrootsSdkTradeErrorKind::InvalidEnvelope,
                        "trade.list",
                        format!("stored trade id is invalid: {error}"),
                    )
                })?,
                updated_event_seq: row
                    .try_get("updated_event_seq")
                    .map_err(trade_query_store_error)?,
            })
        })
        .collect()
}

#[cfg(feature = "runtime")]
fn push_string_list(query: &mut QueryBuilder<Sqlite>, values: &[String]) {
    let mut separated = query.separated(", ");
    for value in values {
        separated.push_bind(value.as_str());
    }
}

#[cfg(feature = "runtime")]
async fn inspect_evidence_views(
    sdk: &RadrootsClient,
    request: InspectEvidenceRequest,
) -> Result<Page<EvidenceView>, RadrootsSdkError> {
    let limit = bounded_limit(request.limit, "trade.inspect_evidence")?;
    let offset = evidence_cursor_offset(request.cursor.as_deref())?;
    let metadata = sdk
        ._private_store
        .trade_artifact_metadata_for_trade(request.trade_id.as_str())
        .await?;
    let mut items = Vec::new();
    for item in metadata
        .iter()
        .skip(offset)
        .take(usize::try_from(limit).expect("limit fits usize"))
    {
        let state = if item.deleted_at_ms.is_some() {
            RadrootsTradePrivateTermsStateV1::Missing
        } else {
            RadrootsTradePrivateTermsStateV1::AvailableVerified
        };
        items.push(EvidenceView {
            artifact_id: item.artifact_id.clone(),
            trade_id: RadrootsTradeId::parse(item.trade_id.clone())
                .expect("stored trade id is valid"),
            candidate_id: parse_optional_candidate_id(item.candidate_id.clone()),
            artifact_kind: item.artifact_kind.into(),
            schema_id: item.schema_id.clone(),
            ciphertext_commitment: item.ciphertext_commitment.clone(),
            retention_class: item.retention_class.clone(),
            state,
            created_at_ms: item.created_at_ms,
            expires_at_ms: item.expires_at_ms,
            deleted_at_ms: item.deleted_at_ms,
        });
    }
    let next_offset = offset + items.len();
    let next_cursor = if metadata.len() > next_offset {
        Some(encode_offset_cursor(next_offset)?)
    } else {
        None
    };
    Ok(Page { items, next_cursor })
}

#[cfg(feature = "runtime")]
async fn last_trade_mutation_snapshot(
    sdk: &RadrootsClient,
    trade_id: &RadrootsTradeId,
) -> Result<LastTradeMutationSnapshot, RadrootsSdkError> {
    let row = sqlx::query(
        "SELECT mutation_id, first_event_seq FROM trade_mutation WHERE trade_id = ? ORDER BY first_event_seq DESC, mutation_id DESC LIMIT 1",
    )
    .bind(trade_id.as_str())
    .fetch_optional(sdk._event_store.pool())
    .await
    .map_err(trade_query_store_error)?;
    match row {
        Some(row) => Ok(LastTradeMutationSnapshot {
            mutation_id: Some(
                RadrootsTradeMutationId::parse(
                    row.try_get::<String, _>("mutation_id")
                        .map_err(trade_query_store_error)?,
                )
                .map_err(|error| {
                    trade_query_error(
                        RadrootsSdkTradeErrorKind::InvalidEnvelope,
                        "trade.refresh_evidence",
                        format!("stored mutation id is invalid: {error}"),
                    )
                })?,
            ),
            event_seq: Some(
                row.try_get("first_event_seq")
                    .map_err(trade_query_store_error)?,
            ),
        }),
        None => Ok(LastTradeMutationSnapshot {
            mutation_id: None,
            event_seq: None,
        }),
    }
}

#[cfg(feature = "runtime")]
fn bounded_limit(limit: Option<u32>, operation: &'static str) -> Result<u32, RadrootsSdkError> {
    let limit = limit.unwrap_or(TRADE_QUERY_DEFAULT_LIMIT);
    if !(1..=TRADE_QUERY_MAX_LIMIT).contains(&limit) {
        return Err(trade_query_error(
            RadrootsSdkTradeErrorKind::QueryLimitInvalid,
            operation,
            format!("trade query limit must be between 1 and {TRADE_QUERY_MAX_LIMIT}"),
        ));
    }
    Ok(limit)
}

#[cfg(feature = "runtime")]
fn filter_digest(request: &ListTradesRequest) -> Result<String, RadrootsSdkError> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "filter": request.filter,
        "sort": request.sort
    }))
    .map_err(trade_query_store_error)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(feature = "runtime")]
fn encode_trade_list_cursor(
    row: &TradeListRow,
    filter_digest: &str,
    sort: TradeListSort,
) -> Result<Option<String>, RadrootsSdkError> {
    let payload = TradeListCursorPayload {
        version: TRADE_LIST_CURSOR_VERSION,
        sort,
        filter_digest: filter_digest.to_owned(),
        updated_event_seq: row.updated_event_seq,
        trade_id: row.trade_id.to_string(),
    };
    let bytes = serde_json::to_vec(&payload).map_err(trade_query_store_error)?;
    Ok(Some(URL_SAFE_NO_PAD.encode(bytes)))
}

#[cfg(feature = "runtime")]
fn decode_trade_list_cursor(
    cursor: &str,
    filter_digest: &str,
    sort: TradeListSort,
) -> Result<TradeListCursorPayload, RadrootsSdkError> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|error| {
        trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.list",
            error.to_string(),
        )
    })?;
    let payload: TradeListCursorPayload =
        serde_json::from_slice(bytes.as_slice()).map_err(|error| {
            trade_query_error(
                RadrootsSdkTradeErrorKind::CursorInvalid,
                "trade.list",
                error.to_string(),
            )
        })?;
    if payload.version != TRADE_LIST_CURSOR_VERSION
        || payload.filter_digest != filter_digest
        || payload.sort != sort
    {
        return Err(trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.list",
            "trade list cursor does not match request filter or sort",
        ));
    }
    Ok(payload)
}

#[cfg(feature = "runtime")]
fn evidence_cursor_offset(cursor: Option<&str>) -> Result<usize, RadrootsSdkError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|error| {
        trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.inspect_evidence",
            error.to_string(),
        )
    })?;
    let value: serde_json::Value = serde_json::from_slice(bytes.as_slice()).map_err(|error| {
        trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.inspect_evidence",
            error.to_string(),
        )
    })?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            trade_query_error(
                RadrootsSdkTradeErrorKind::CursorInvalid,
                "trade.inspect_evidence",
                "evidence cursor is missing version",
            )
        })?;
    let offset = value
        .get("offset")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            trade_query_error(
                RadrootsSdkTradeErrorKind::CursorInvalid,
                "trade.inspect_evidence",
                "evidence cursor is missing offset",
            )
        })?;
    if version != 1 {
        return Err(trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.inspect_evidence",
            "evidence cursor version is unsupported",
        ));
    }
    usize::try_from(offset).map_err(|_| {
        trade_query_error(
            RadrootsSdkTradeErrorKind::CursorInvalid,
            "trade.inspect_evidence",
            "evidence cursor offset is too large",
        )
    })
}

#[cfg(feature = "runtime")]
fn encode_offset_cursor(offset: usize) -> Result<String, RadrootsSdkError> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "offset": offset
    }))
    .map_err(trade_query_store_error)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(feature = "runtime")]
fn enum_label<T: Serialize>(value: &T) -> Result<String, RadrootsSdkError> {
    serde_json::to_value(value)
        .map_err(trade_query_store_error)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| RadrootsSdkError::Projection {
            message: "projection state label did not serialize to string".to_owned(),
        })
}

#[cfg(feature = "runtime")]
fn parse_optional_candidate_id(value: Option<String>) -> Option<RadrootsTradeCandidateId> {
    value.map(|candidate_id| {
        RadrootsTradeCandidateId::parse(candidate_id).expect("stored candidate id is valid")
    })
}

#[cfg(feature = "runtime")]
fn trade_command_error(
    kind: RadrootsSdkTradeErrorKind,
    operation: &'static str,
    message: impl Into<String>,
) -> RadrootsSdkError {
    RadrootsSdkError::Trade {
        kind,
        operation: operation.to_owned(),
        message: message.into(),
    }
}

#[cfg(feature = "runtime")]
fn trade_query_error(
    kind: RadrootsSdkTradeErrorKind,
    operation: &'static str,
    message: impl Into<String>,
) -> RadrootsSdkError {
    RadrootsSdkError::Trade {
        kind,
        operation: operation.to_owned(),
        message: message.into(),
    }
}

#[cfg(feature = "runtime")]
fn trade_query_store_error(error: impl ToString) -> RadrootsSdkError {
    RadrootsSdkError::Projection {
        message: error.to_string(),
    }
}

#[cfg(all(test, feature = "runtime", feature = "signer-adapters"))]
#[path = "../tests/unit/trade_runtime_tests.rs"]
mod tests;
