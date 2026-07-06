#[cfg(feature = "signer-adapters")]
use crate::workflow_runtime::enqueue_configured_signed_workflow;
#[cfg(feature = "runtime")]
use crate::{
    DvmClient, RadrootsSdkError, RadrootsSdkTimestamp, SdkIdempotencyKey, SdkMutationState,
    SdkRelayTargetPolicy, SdkRelayUrlPolicy, SyncProjectionRefreshReceipt,
    SyncProjectionRefreshRequest,
    runtime::sdk_now_ms,
    sync_runtime::refresh_product_projections_for_sdk,
    workflow_runtime::{SdkWorkflowEnqueueRequest, enqueue_signed_workflow},
};
#[cfg(feature = "runtime")]
use radroots_authority::{RadrootsActorContext, RadrootsEventSigner, authorize_actor_for_draft};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventIngest;
#[cfg(feature = "runtime")]
use radroots_events::{
    RadrootsNostrEvent,
    draft::RadrootsFrozenEventDraft,
    ids::{RadrootsEventId, RadrootsListingAddress, RadrootsOrderId, RadrootsPublicKey},
    kinds::KIND_TRADE_TRANSITION_PROOF_REQUEST,
};
#[cfg(feature = "runtime")]
use radroots_events_codec::wire::{WireEventParts, canonicalize_tags, to_frozen_draft};
#[cfg(feature = "runtime")]
use radroots_trade::dvm::RadrootsTradeInventoryBinWitnessDto;
#[cfg(feature = "runtime")]
use radroots_trade::validation_receipt::{
    RadrootsTradeCommitmentConfidence, RadrootsTradeValidationAuthority,
    RadrootsValidationReceiptExpectedBinding, RadrootsValidationReceiptProofSystem,
    RadrootsValidationReceiptResult, RadrootsValidationReceiptType,
    verify_validation_receipt_event,
};

#[cfg(feature = "runtime")]
const RADROOTS_TRADE_TRANSITION_WITNESS_VERSION: u32 = 1;
#[cfg(feature = "runtime")]
const RADROOTS_TRADE_TRANSITION_PROTOCOL_VERSION: &str = "radroots.trade.v1";
#[cfg(feature = "runtime")]
const RADROOTS_TRADE_TRANSITION_REDUCER_PROGRAM_HASH: &str =
    "0x3d8f7f463904d71f2d0d14b1551450756697e51c7b658e10c6d5c20a7bc61f08";
#[cfg(feature = "runtime")]
const RADROOTS_TRADE_TRANSITION_PROOF_TARGET: &str = "trade.order_acceptance.v1";

#[cfg(feature = "runtime")]
pub const DVM_TRADE_TRANSITION_PROOF_REQUEST_CONTRACT_ID: &str =
    "radroots.trade.transition_proof.request.v1";
#[cfg(feature = "runtime")]
pub const DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND: &str =
    "dvm.trade_transition_proof.request.v1";

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DvmProofMode {
    #[default]
    None,
    Core,
    Compressed,
    Groth16,
    Plonk,
}

#[cfg(feature = "runtime")]
impl DvmProofMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Core => "core",
            Self::Compressed => "compressed",
            Self::Groth16 => "groth16",
            Self::Plonk => "plonk",
        }
    }

    pub const fn proof_system(self) -> RadrootsValidationReceiptProofSystem {
        match self {
            Self::None => RadrootsValidationReceiptProofSystem::None,
            Self::Core => RadrootsValidationReceiptProofSystem::Sp1Core,
            Self::Compressed => RadrootsValidationReceiptProofSystem::Sp1Compressed,
            Self::Groth16 => RadrootsValidationReceiptProofSystem::Sp1Groth16,
            Self::Plonk => RadrootsValidationReceiptProofSystem::Sp1Plonk,
        }
    }

    const fn requires_sp1_identity(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct DvmTradeTransitionProofPrepareRequest {
    #[serde(serialize_with = "crate::actor_json::serialize_actor_context")]
    pub actor: RadrootsActorContext,
    pub worker_pubkey: RadrootsPublicKey,
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub request_event_id: RadrootsEventId,
    pub decision_event_id: RadrootsEventId,
    pub inventory_bins: Vec<RadrootsTradeInventoryBinWitnessDto>,
    pub inventory_sequence: u128,
    pub previous_state_root: Option<String>,
    pub proof_mode: DvmProofMode,
    pub sp1_program_hash: Option<String>,
    pub sp1_verifying_key_hash: Option<String>,
    pub created_at: Option<RadrootsSdkTimestamp>,
}

#[cfg(feature = "runtime")]
impl DvmTradeTransitionProofPrepareRequest {
    pub fn new(
        actor: RadrootsActorContext,
        worker_pubkey: RadrootsPublicKey,
        listing_addr: RadrootsListingAddress,
        listing_event_id: RadrootsEventId,
        request_event_id: RadrootsEventId,
        decision_event_id: RadrootsEventId,
        inventory_bins: Vec<RadrootsTradeInventoryBinWitnessDto>,
    ) -> Self {
        Self {
            actor,
            worker_pubkey,
            listing_addr,
            listing_event_id,
            request_event_id,
            decision_event_id,
            inventory_bins,
            inventory_sequence: 0,
            previous_state_root: None,
            proof_mode: DvmProofMode::None,
            sp1_program_hash: None,
            sp1_verifying_key_hash: None,
            created_at: None,
        }
    }

    pub fn with_inventory_sequence(mut self, inventory_sequence: u128) -> Self {
        self.inventory_sequence = inventory_sequence;
        self
    }

    pub fn with_previous_state_root(mut self, previous_state_root: impl Into<String>) -> Self {
        self.previous_state_root = Some(previous_state_root.into());
        self
    }

    pub fn with_proof_mode(mut self, proof_mode: DvmProofMode) -> Self {
        self.proof_mode = proof_mode;
        self
    }

    pub fn with_sp1_identity(
        mut self,
        program_hash: impl Into<String>,
        verifying_key_hash: impl Into<String>,
    ) -> Self {
        self.sp1_program_hash = Some(program_hash.into());
        self.sp1_verifying_key_hash = Some(verifying_key_hash.into());
        self
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct DvmTradeTransitionProofEnqueueRequest {
    #[serde(flatten)]
    pub prepare: DvmTradeTransitionProofPrepareRequest,
    pub target_relays: SdkRelayTargetPolicy,
    pub idempotency_key: Option<SdkIdempotencyKey>,
}

#[cfg(feature = "runtime")]
impl DvmTradeTransitionProofEnqueueRequest {
    pub fn new(
        actor: RadrootsActorContext,
        worker_pubkey: RadrootsPublicKey,
        listing_addr: RadrootsListingAddress,
        listing_event_id: RadrootsEventId,
        request_event_id: RadrootsEventId,
        decision_event_id: RadrootsEventId,
        inventory_bins: Vec<RadrootsTradeInventoryBinWitnessDto>,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self::from_prepare(
            DvmTradeTransitionProofPrepareRequest::new(
                actor,
                worker_pubkey,
                listing_addr,
                listing_event_id,
                request_event_id,
                decision_event_id,
                inventory_bins,
            ),
            target_relays,
        )
    }

    pub fn from_prepare(
        prepare: DvmTradeTransitionProofPrepareRequest,
        target_relays: SdkRelayTargetPolicy,
    ) -> Self {
        Self {
            prepare,
            target_relays,
            idempotency_key: None,
        }
    }

    pub fn try_with_target_relays<I, S>(
        mut self,
        target_relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.target_relays = SdkRelayTargetPolicy::try_explicit(target_relays, policy)?;
        Ok(self)
    }

    pub fn with_idempotency_key(mut self, idempotency_key: SdkIdempotencyKey) -> Self {
        self.idempotency_key = Some(idempotency_key);
        self
    }

    pub fn try_with_idempotency_key(
        mut self,
        idempotency_key: impl AsRef<str>,
    ) -> Result<Self, RadrootsSdkError> {
        self.idempotency_key = Some(SdkIdempotencyKey::new(idempotency_key)?);
        Ok(self)
    }

    pub fn with_inventory_sequence(mut self, inventory_sequence: u128) -> Self {
        self.prepare.inventory_sequence = inventory_sequence;
        self
    }

    pub fn with_previous_state_root(mut self, previous_state_root: impl Into<String>) -> Self {
        self.prepare.previous_state_root = Some(previous_state_root.into());
        self
    }

    pub fn with_proof_mode(mut self, proof_mode: DvmProofMode) -> Self {
        self.prepare.proof_mode = proof_mode;
        self
    }

    pub fn with_sp1_identity(
        mut self,
        program_hash: impl Into<String>,
        verifying_key_hash: impl Into<String>,
    ) -> Self {
        self.prepare.sp1_program_hash = Some(program_hash.into());
        self.prepare.sp1_verifying_key_hash = Some(verifying_key_hash.into());
        self
    }

    pub fn with_created_at(mut self, created_at: RadrootsSdkTimestamp) -> Self {
        self.prepare.created_at = Some(created_at);
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DvmTradeTransitionProofRequestPayload {
    pub witness_version: u32,
    pub proof_target: String,
    pub listing_event_id: String,
    pub request_event_id: String,
    pub decision_event_id: String,
    pub inventory_bins: Vec<RadrootsTradeInventoryBinWitnessDto>,
    pub inventory_sequence: u128,
    pub previous_state_root: Option<String>,
    pub proof_mode: DvmProofMode,
    pub reducer_program_hash: String,
    pub radroots_protocol_version: String,
    pub sp1_program_hash: Option<String>,
    pub sp1_verifying_key_hash: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DvmTradeTransitionProofPlan {
    pub worker_pubkey: RadrootsPublicKey,
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub request_event_id: RadrootsEventId,
    pub decision_event_id: RadrootsEventId,
    pub proof_mode: DvmProofMode,
    pub expected_receipt_proof_system: RadrootsValidationReceiptProofSystem,
    pub expected_event_id: RadrootsEventId,
    pub frozen_draft: RadrootsFrozenEventDraft,
    pub payload: DvmTradeTransitionProofRequestPayload,
    pub created_at: RadrootsSdkTimestamp,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DvmTradeTransitionProofReceipt {
    pub worker_pubkey: RadrootsPublicKey,
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub request_event_id: RadrootsEventId,
    pub decision_event_id: RadrootsEventId,
    pub expected_event_id: RadrootsEventId,
    pub signed_event_id: RadrootsEventId,
    pub proof_mode: DvmProofMode,
    pub expected_receipt_proof_system: RadrootsValidationReceiptProofSystem,
    pub local_event_seq: i64,
    pub outbox_operation_id: i64,
    pub outbox_event_id: i64,
    pub state: SdkMutationState,
    pub idempotency_digest_prefix: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, serde::Serialize)]
#[non_exhaustive]
pub struct DvmValidationReceiptIngestRequest {
    pub event: RadrootsNostrEvent,
    pub observed_at: Option<RadrootsSdkTimestamp>,
    pub expected_order_id: Option<RadrootsOrderId>,
    pub expected_listing_event_id: Option<RadrootsEventId>,
    pub expected_root_event_id: Option<RadrootsEventId>,
    pub expected_target_event_id: Option<RadrootsEventId>,
    pub projection_refresh: SyncProjectionRefreshRequest,
}

#[cfg(feature = "runtime")]
impl DvmValidationReceiptIngestRequest {
    pub fn new(event: RadrootsNostrEvent) -> Self {
        Self {
            event,
            observed_at: None,
            expected_order_id: None,
            expected_listing_event_id: None,
            expected_root_event_id: None,
            expected_target_event_id: None,
            projection_refresh: SyncProjectionRefreshRequest::new(),
        }
    }

    pub fn with_observed_at(mut self, observed_at: RadrootsSdkTimestamp) -> Self {
        self.observed_at = Some(observed_at);
        self
    }

    pub fn with_expected_order_id(mut self, order_id: RadrootsOrderId) -> Self {
        self.expected_order_id = Some(order_id);
        self
    }

    pub fn with_expected_listing_event_id(mut self, listing_event_id: RadrootsEventId) -> Self {
        self.expected_listing_event_id = Some(listing_event_id);
        self
    }

    pub fn with_expected_root_event_id(mut self, root_event_id: RadrootsEventId) -> Self {
        self.expected_root_event_id = Some(root_event_id);
        self
    }

    pub fn with_expected_target_event_id(mut self, target_event_id: RadrootsEventId) -> Self {
        self.expected_target_event_id = Some(target_event_id);
        self
    }

    pub fn with_projection_refresh(mut self, refresh: SyncProjectionRefreshRequest) -> Self {
        self.projection_refresh = refresh;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DvmValidationReceiptIngestReceipt {
    pub receipt_event_id: RadrootsEventId,
    pub order_id: RadrootsOrderId,
    pub listing_event_id: RadrootsEventId,
    pub root_event_id: RadrootsEventId,
    pub target_event_id: RadrootsEventId,
    pub receipt_type: RadrootsValidationReceiptType,
    pub result: RadrootsValidationReceiptResult,
    pub proof_system: RadrootsValidationReceiptProofSystem,
    pub validation_authority: Option<RadrootsTradeValidationAuthority>,
    pub commitment_confidence: RadrootsTradeCommitmentConfidence,
    pub local_event_seq: i64,
    pub inserted: bool,
    pub refresh: SyncProjectionRefreshReceipt,
}

#[cfg(feature = "runtime")]
impl<'sdk> DvmClient<'sdk> {
    pub fn prepare_trade_transition_proof_request(
        &self,
        request: DvmTradeTransitionProofPrepareRequest,
    ) -> Result<DvmTradeTransitionProofPlan, RadrootsSdkError> {
        let created_at = match request.created_at {
            Some(created_at) => created_at,
            None => self.sdk.now()?,
        };
        dvm_trade_transition_proof_plan(request, created_at)
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn enqueue_trade_transition_proof_request(
        &self,
        request: DvmTradeTransitionProofEnqueueRequest,
    ) -> Result<DvmTradeTransitionProofReceipt, RadrootsSdkError> {
        let actor = request.prepare.actor.clone();
        let target_relays = request.target_relays.clone();
        let idempotency_key = request.idempotency_key.clone();
        let plan = self.prepare_trade_transition_proof_request(request.prepare)?;
        let enqueue = enqueue_configured_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND,
                actor: &actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
        )
        .await?;
        Ok(dvm_trade_transition_proof_receipt(plan, enqueue))
    }

    pub async fn enqueue_trade_transition_proof_request_with_explicit_signer(
        &self,
        request: DvmTradeTransitionProofEnqueueRequest,
        signer: &dyn RadrootsEventSigner,
    ) -> Result<DvmTradeTransitionProofReceipt, RadrootsSdkError> {
        let actor = request.prepare.actor.clone();
        let target_relays = request.target_relays.clone();
        let idempotency_key = request.idempotency_key.clone();
        let plan = self.prepare_trade_transition_proof_request(request.prepare)?;
        let enqueue = enqueue_signed_workflow(
            self.sdk,
            SdkWorkflowEnqueueRequest {
                operation_kind: DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND,
                actor: &actor,
                frozen_draft: &plan.frozen_draft,
                target_relays,
                idempotency_key,
            },
            signer,
        )
        .await?;
        Ok(dvm_trade_transition_proof_receipt(plan, enqueue))
    }

    pub async fn ingest_validation_receipt(
        &self,
        request: DvmValidationReceiptIngestRequest,
    ) -> Result<DvmValidationReceiptIngestReceipt, RadrootsSdkError> {
        let verified = verify_validation_receipt_event(
            &request.event,
            RadrootsValidationReceiptExpectedBinding {
                order_id: request
                    .expected_order_id
                    .as_ref()
                    .map(RadrootsOrderId::as_str),
                listing_event_id: request
                    .expected_listing_event_id
                    .as_ref()
                    .map(RadrootsEventId::as_str),
                root_event_id: request
                    .expected_root_event_id
                    .as_ref()
                    .map(RadrootsEventId::as_str),
                target_event_id: request
                    .expected_target_event_id
                    .as_ref()
                    .map(RadrootsEventId::as_str),
                ..RadrootsValidationReceiptExpectedBinding::default()
            },
        )
        .map_err(validation_receipt_sdk_error)?;
        let receipt_event_id = parse_event_id(request.event.id.as_str(), "receipt event id")?;
        let order_id =
            RadrootsOrderId::parse(verified.tags.order_id.as_str()).map_err(|error| {
                RadrootsSdkError::InvalidRequest {
                    message: format!("validation receipt order id is invalid: {error}"),
                }
            })?;
        let listing_event_id = RadrootsEventId::parse(verified.tags.listing_event_id.as_str())
            .expect("verified validation receipt listing event id is valid");
        let root_event_id = RadrootsEventId::parse(verified.tags.root_event_id.as_str())
            .expect("verified validation receipt root event id is valid");
        let target_event_id = RadrootsEventId::parse(verified.tags.target_event_id.as_str())
            .expect("verified validation receipt target event id is valid");
        let observed_at_ms = match request.observed_at {
            Some(observed_at) => sdk_timestamp_ms(observed_at)?,
            None => sdk_now_ms(self.sdk)?,
        };
        let ingest = self
            .sdk
            ._event_store
            .ingest_event(RadrootsEventIngest::new(request.event, observed_at_ms))
            .await?;
        let refresh =
            refresh_product_projections_for_sdk(self.sdk, request.projection_refresh).await?;
        Ok(DvmValidationReceiptIngestReceipt {
            receipt_event_id,
            order_id,
            listing_event_id,
            root_event_id,
            target_event_id,
            receipt_type: verified.receipt.receipt_type,
            result: verified.receipt.result,
            proof_system: verified.receipt.proof.system,
            validation_authority: None,
            commitment_confidence: validation_receipt_ingest_confidence(verified.receipt.result),
            local_event_seq: ingest.seq,
            inserted: ingest.inserted,
            refresh,
        })
    }
}

#[cfg(feature = "runtime")]
fn validation_receipt_ingest_confidence(
    result: RadrootsValidationReceiptResult,
) -> RadrootsTradeCommitmentConfidence {
    match result {
        RadrootsValidationReceiptResult::Valid => RadrootsTradeCommitmentConfidence::LocalOnly,
        RadrootsValidationReceiptResult::Invalid => RadrootsTradeCommitmentConfidence::Invalid,
    }
}

#[cfg(feature = "runtime")]
fn dvm_trade_transition_proof_plan(
    request: DvmTradeTransitionProofPrepareRequest,
    created_at: RadrootsSdkTimestamp,
) -> Result<DvmTradeTransitionProofPlan, RadrootsSdkError> {
    validate_inventory_bins(&request.inventory_bins)?;
    validate_sp1_identity(&request)?;
    let payload = DvmTradeTransitionProofRequestPayload {
        witness_version: RADROOTS_TRADE_TRANSITION_WITNESS_VERSION,
        proof_target: RADROOTS_TRADE_TRANSITION_PROOF_TARGET.to_owned(),
        listing_event_id: request.listing_event_id.as_str().to_owned(),
        request_event_id: request.request_event_id.as_str().to_owned(),
        decision_event_id: request.decision_event_id.as_str().to_owned(),
        inventory_bins: request.inventory_bins.clone(),
        inventory_sequence: request.inventory_sequence,
        previous_state_root: request.previous_state_root.clone(),
        proof_mode: request.proof_mode,
        reducer_program_hash: RADROOTS_TRADE_TRANSITION_REDUCER_PROGRAM_HASH.to_owned(),
        radroots_protocol_version: RADROOTS_TRADE_TRANSITION_PROTOCOL_VERSION.to_owned(),
        sp1_program_hash: request.sp1_program_hash.clone(),
        sp1_verifying_key_hash: request.sp1_verifying_key_hash.clone(),
    };
    let content = serde_json::to_string(&payload).expect("DVM proof request payload serializes");
    let mut tags = vec![
        vec!["a".to_owned(), request.listing_addr.as_str().to_owned()],
        vec!["p".to_owned(), request.worker_pubkey.as_str().to_owned()],
        vec![
            "i".to_owned(),
            request.decision_event_id.as_str().to_owned(),
            "event".to_owned(),
            "radroots:order_decision_event".to_owned(),
        ],
    ];
    canonicalize_tags(&mut tags);
    let frozen_draft = to_frozen_draft(
        WireEventParts {
            kind: KIND_TRADE_TRANSITION_PROOF_REQUEST,
            content,
            tags,
        },
        DVM_TRADE_TRANSITION_PROOF_REQUEST_CONTRACT_ID,
        request.actor.pubkey().as_str(),
        created_at.try_into_nostr_created_at()?,
    )
    .expect("DVM proof request draft is valid");
    authorize_actor_for_draft(&request.actor, &frozen_draft)?;
    let expected_event_id = RadrootsEventId::parse(frozen_draft.expected_event_id.as_str())
        .expect("frozen DVM proof request draft produces a valid event id");
    Ok(DvmTradeTransitionProofPlan {
        worker_pubkey: request.worker_pubkey,
        listing_addr: request.listing_addr,
        listing_event_id: request.listing_event_id,
        request_event_id: request.request_event_id,
        decision_event_id: request.decision_event_id,
        proof_mode: request.proof_mode,
        expected_receipt_proof_system: request.proof_mode.proof_system(),
        expected_event_id,
        frozen_draft,
        payload,
        created_at,
    })
}

#[cfg(feature = "runtime")]
fn validate_inventory_bins(
    inventory_bins: &[RadrootsTradeInventoryBinWitnessDto],
) -> Result<(), RadrootsSdkError> {
    if inventory_bins.is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "DVM proof request inventory bins cannot be empty".to_owned(),
        });
    }
    for bin in inventory_bins {
        if bin.bin_id.as_str().trim().is_empty() {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "DVM proof request inventory bin id cannot be empty".to_owned(),
            });
        }
        if bin.previous_reserved > bin.listing_capacity {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!(
                    "DVM proof request inventory bin `{}` previous_reserved exceeds listing_capacity",
                    bin.bin_id.as_str()
                ),
            });
        }
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn validate_sp1_identity(
    request: &DvmTradeTransitionProofPrepareRequest,
) -> Result<(), RadrootsSdkError> {
    validate_optional_hash32(&request.previous_state_root, "previous_state_root")?;
    validate_optional_hash32(&request.sp1_program_hash, "sp1_program_hash")?;
    validate_optional_hash32(&request.sp1_verifying_key_hash, "sp1_verifying_key_hash")?;
    let has_sp1_identity =
        request.sp1_program_hash.is_some() || request.sp1_verifying_key_hash.is_some();
    if request.proof_mode == DvmProofMode::None && has_sp1_identity {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "DVM proof mode none cannot include SP1 identity hashes".to_owned(),
        });
    }
    if request.proof_mode.requires_sp1_identity()
        && (request.sp1_program_hash.is_none() || request.sp1_verifying_key_hash.is_none())
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "DVM proof mode {} requires SP1 program and verifying key hashes",
                request.proof_mode.as_str()
            ),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn validate_optional_hash32(
    value: &Option<String>,
    field: &'static str,
) -> Result<(), RadrootsSdkError> {
    if let Some(value) = value {
        let hash = value.as_str();
        if hash.len() != 66 || !hash.starts_with("0x") || !is_lower_hex(&hash[2..]) {
            return Err(RadrootsSdkError::InvalidRequest {
                message: format!("DVM proof request {field} must be 0x-prefixed lowercase hex32"),
            });
        }
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(feature = "runtime")]
fn dvm_trade_transition_proof_receipt(
    plan: DvmTradeTransitionProofPlan,
    enqueue: crate::workflow_runtime::SdkWorkflowEnqueueReceipt,
) -> DvmTradeTransitionProofReceipt {
    DvmTradeTransitionProofReceipt {
        worker_pubkey: plan.worker_pubkey,
        listing_addr: plan.listing_addr,
        listing_event_id: plan.listing_event_id,
        request_event_id: plan.request_event_id,
        decision_event_id: plan.decision_event_id,
        expected_event_id: plan.expected_event_id,
        signed_event_id: enqueue.signed_event_id,
        proof_mode: plan.proof_mode,
        expected_receipt_proof_system: plan.expected_receipt_proof_system,
        local_event_seq: enqueue.local_event_seq,
        outbox_operation_id: enqueue.outbox_operation_id,
        outbox_event_id: enqueue.outbox_event_id,
        state: enqueue.state.into(),
        idempotency_digest_prefix: Some(enqueue.idempotency_digest_prefix),
    }
}

#[cfg(feature = "runtime")]
fn parse_event_id(value: &str, field: &'static str) -> Result<RadrootsEventId, RadrootsSdkError> {
    RadrootsEventId::parse(value).map_err(|error| RadrootsSdkError::InvalidRequest {
        message: format!("{field} is invalid: {error}"),
    })
}

#[cfg(feature = "runtime")]
fn validation_receipt_sdk_error(
    error: radroots_trade::validation_receipt::RadrootsValidationReceiptError,
) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: format!("validation receipt event is invalid: {error}"),
    }
}

#[cfg(feature = "runtime")]
fn sdk_timestamp_ms(timestamp: RadrootsSdkTimestamp) -> Result<i64, RadrootsSdkError> {
    let seconds = i64::try_from(timestamp.unix_seconds()).map_err(|_| {
        RadrootsSdkError::TimestampOutOfRange {
            value: timestamp.unix_seconds(),
        }
    })?;
    seconds
        .checked_mul(1_000)
        .ok_or(RadrootsSdkError::TimestampOutOfRange {
            value: timestamp.unix_seconds(),
        })
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/dvm_runtime_tests.rs"]
mod tests;
