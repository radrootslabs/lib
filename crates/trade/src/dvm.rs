#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    ids::{
        RadrootsEventId, RadrootsIdParseError, RadrootsInventoryBinId, RadrootsListingAddress,
        RadrootsOrderId, RadrootsPublicKey,
    },
    kinds::{
        KIND_JOB_FEEDBACK, KIND_TRADE_TRANSITION_PROOF_REQUEST, KIND_TRADE_TRANSITION_PROOF_RESULT,
    },
    tags::{TAG_A, TAG_E, TAG_I, TAG_P, TAG_STATUS},
};
use thiserror::Error;

pub const RADROOTS_DVM_TAG_REQUEST: &str = "request";
pub const RADROOTS_DVM_TAG_LISTING_EVENT: &str = "radroots:listing_event";
pub const RADROOTS_DVM_TAG_ROOT_EVENT: &str = "radroots:root_event";
pub const RADROOTS_DVM_TAG_TARGET_EVENT: &str = "radroots:target_event";
pub const RADROOTS_DVM_TAG_VALIDATION_RECEIPT: &str = "radroots:validation_receipt";
pub const RADROOTS_DVM_INPUT_TYPE_EVENT: &str = "event";

#[derive(Debug, Error)]
pub enum RadrootsTradeDvmError {
    #[error("unsupported DVM event kind: expected {expected}, received {actual}")]
    UnsupportedKind { expected: u32, actual: u32 },
    #[error("missing required {tag} tag")]
    MissingTag { tag: &'static str },
    #[error("invalid {tag} tag value {value}: {source}")]
    InvalidTag {
        tag: &'static str,
        value: String,
        source: RadrootsIdParseError,
    },
    #[error("invalid DVM content JSON: {0}")]
    InvalidContent(serde_json::Error),
    #[error("failed to serialize DVM request event: {0}")]
    SerializeRequestEvent(serde_json::Error),
    #[error("invalid stringified DVM request event: {0}")]
    InvalidRequestEvent(serde_json::Error),
    #[error("stringified DVM request event has unsupported kind: {kind}")]
    RequestEventKind { kind: u32 },
    #[error("DVM content field {field} does not match required tags")]
    ContentMismatch { field: &'static str },
    #[error("DVM request event id does not match the result e tag")]
    RequestEventIdMismatch,
    #[error("invalid proof mode: {value}")]
    InvalidProofMode { value: String },
    #[error("invalid DVM input role: {value}")]
    InvalidInputRole { value: String },
    #[error("invalid hash in {field}")]
    InvalidHash { field: &'static str },
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeProofMode {
    None,
    Core,
    Compressed,
    Groth16,
    Plonk,
}

impl RadrootsTradeProofMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Core => "core",
            Self::Compressed => "compressed",
            Self::Groth16 => "groth16",
            Self::Plonk => "plonk",
        }
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTradeDvmError> {
        match value.as_ref() {
            "none" => Ok(Self::None),
            "core" => Ok(Self::Core),
            "compressed" => Ok(Self::Compressed),
            "groth16" => Ok(Self::Groth16),
            "plonk" => Ok(Self::Plonk),
            value => Err(RadrootsTradeDvmError::InvalidProofMode {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeDvmInputRole {
    Listing,
    OrderRequest,
    OrderDecision,
}

impl RadrootsTradeDvmInputRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Listing => "radroots:listing_event",
            Self::OrderRequest => "radroots:order_request_event",
            Self::OrderDecision => "radroots:order_decision_event",
        }
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTradeDvmError> {
        match value.as_ref() {
            "radroots:listing_event" => Ok(Self::Listing),
            "radroots:order_request_event" => Ok(Self::OrderRequest),
            "radroots:order_decision_event" => Ok(Self::OrderDecision),
            value => Err(RadrootsTradeDvmError::InvalidInputRole {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDvmInputTag {
    pub event_id: RadrootsEventId,
    pub role: RadrootsTradeDvmInputRole,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeCanonicalEventEvidenceRole {
    Buyer,
    Seller,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeCanonicalEventWorkflowPosition {
    Listing,
    OrderRequest,
    OrderDecision,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeCanonicalEventEvidenceDto {
    pub event_id: RadrootsEventId,
    pub signer_pubkey: RadrootsPublicKey,
    pub kind: u32,
    pub canonical_event_hash: String,
    pub signature_hash: String,
    pub preverified_signature: bool,
    pub role: RadrootsTradeCanonicalEventEvidenceRole,
    pub workflow_position: RadrootsTradeCanonicalEventWorkflowPosition,
    pub content_hash: String,
    pub tags_hash: String,
    pub ordering_key: String,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeInventoryBinWitnessDto {
    pub bin_id: RadrootsInventoryBinId,
    pub listing_capacity: u64,
    pub previous_reserved: u64,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderItemWitnessDto {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderRequestWitnessDto {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub items: Vec<RadrootsTradeOrderItemWitnessDto>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsTradeOrderDecisionWitnessDto {
    Accepted {
        inventory_commitments: Vec<RadrootsTradeInventoryCommitmentWitnessDto>,
    },
    Declined {
        reason: String,
    },
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeInventoryCommitmentWitnessDto {
    pub bin_id: RadrootsInventoryBinId,
    pub bin_count: u32,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeOrderDecisionEventWitnessDto {
    pub order_id: RadrootsOrderId,
    pub listing_addr: RadrootsListingAddress,
    pub buyer_pubkey: RadrootsPublicKey,
    pub seller_pubkey: RadrootsPublicKey,
    pub decision: RadrootsTradeOrderDecisionWitnessDto,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofRequestV1 {
    pub witness_version: u32,
    pub proof_target: String,
    pub listing_event_id: RadrootsEventId,
    pub request_event_id: RadrootsEventId,
    pub decision_event_id: RadrootsEventId,
    pub event_evidence: Vec<RadrootsTradeCanonicalEventEvidenceDto>,
    pub request: RadrootsTradeOrderRequestWitnessDto,
    pub decision: RadrootsTradeOrderDecisionEventWitnessDto,
    pub inventory_bins: Vec<RadrootsTradeInventoryBinWitnessDto>,
    pub inventory_sequence: u128,
    pub previous_state_root: Option<String>,
    pub proof_mode: RadrootsTradeProofMode,
    pub reducer_program_hash: String,
    pub radroots_protocol_version: String,
    pub sp1_program_hash: Option<String>,
    pub sp1_verifying_key_hash: Option<String>,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofResultV1 {
    pub version: u32,
    pub listing_event_id: RadrootsEventId,
    pub root_event_id: RadrootsEventId,
    pub target_event_id: RadrootsEventId,
    pub validation_receipt_event_id: Option<RadrootsEventId>,
    pub proof_mode: RadrootsTradeProofMode,
    pub proof_reference: Option<String>,
    pub inline_proof_base64: Option<String>,
    pub program_hash: Option<String>,
    pub verifying_key_hash: Option<String>,
    pub public_values_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofRequestTags {
    pub worker_pubkey: RadrootsPublicKey,
    pub listing_addr: RadrootsListingAddress,
    pub inputs: Vec<RadrootsTradeDvmInputTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofRequestEnvelope {
    pub tags: RadrootsTradeTransitionProofRequestTags,
    pub content: RadrootsTradeTransitionProofRequestV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofResultBinding {
    pub listing_event_id: RadrootsEventId,
    pub root_event_id: RadrootsEventId,
    pub target_event_id: RadrootsEventId,
    pub validation_receipt_event_id: Option<RadrootsEventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeTransitionProofResultTags {
    pub request_event: RadrootsNostrEvent,
    pub request_event_id: RadrootsEventId,
    pub customer_pubkey: RadrootsPublicKey,
    pub inputs: Vec<RadrootsTradeDvmInputTag>,
    pub binding: RadrootsTradeTransitionProofResultBinding,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsTradeDvmFeedbackStatus {
    PaymentRequired,
    Processing,
    Error,
    Success,
    Partial,
}

impl RadrootsTradeDvmFeedbackStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PaymentRequired => "payment-required",
            Self::Processing => "processing",
            Self::Error => "error",
            Self::Success => "success",
            Self::Partial => "partial",
        }
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsTradeDvmError> {
        match value.as_ref() {
            "payment-required" => Ok(Self::PaymentRequired),
            "processing" => Ok(Self::Processing),
            "error" => Ok(Self::Error),
            "success" => Ok(Self::Success),
            "partial" => Ok(Self::Partial),
            value => Err(RadrootsTradeDvmError::InvalidTag {
                tag: TAG_STATUS,
                value: value.to_string(),
                source: RadrootsIdParseError::InvalidFormat,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTradeDvmFeedbackTags {
    pub status: RadrootsTradeDvmFeedbackStatus,
    pub request_event_id: RadrootsEventId,
    pub customer_pubkey: RadrootsPublicKey,
}

pub fn build_transition_proof_request_tags(
    worker_pubkey: &RadrootsPublicKey,
    request: &RadrootsTradeTransitionProofRequestV1,
) -> Vec<Vec<String>> {
    vec![
        vec![TAG_P.to_string(), worker_pubkey.as_str().to_string()],
        vec![
            TAG_A.to_string(),
            request.request.listing_addr.as_str().to_string(),
        ],
        input_event_tag(
            RadrootsTradeDvmInputRole::Listing,
            &request.listing_event_id,
        ),
        input_event_tag(
            RadrootsTradeDvmInputRole::OrderRequest,
            &request.request_event_id,
        ),
        input_event_tag(
            RadrootsTradeDvmInputRole::OrderDecision,
            &request.decision_event_id,
        ),
    ]
}

pub fn parse_transition_proof_request_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsTradeTransitionProofRequestEnvelope, RadrootsTradeDvmError> {
    if event.kind != KIND_TRADE_TRANSITION_PROOF_REQUEST {
        return Err(RadrootsTradeDvmError::UnsupportedKind {
            expected: KIND_TRADE_TRANSITION_PROOF_REQUEST,
            actual: event.kind,
        });
    }
    let tags = parse_transition_proof_request_tags(&event.tags)?;
    let content: RadrootsTradeTransitionProofRequestV1 =
        serde_json::from_str(&event.content).map_err(RadrootsTradeDvmError::InvalidContent)?;
    validate_transition_proof_request_binding(&tags, &content)?;
    validate_transition_proof_request_content(&content)?;
    Ok(RadrootsTradeTransitionProofRequestEnvelope { tags, content })
}

pub fn parse_transition_proof_request_tags(
    tags: &[Vec<String>],
) -> Result<RadrootsTradeTransitionProofRequestTags, RadrootsTradeDvmError> {
    let worker_pubkey = parse_pubkey_tag(TAG_P, required_tag_value(tags, TAG_P)?)?;
    let listing_addr = parse_listing_addr_tag(TAG_A, required_tag_value(tags, TAG_A)?)?;
    let inputs = parse_input_tags(tags)?;
    if inputs.is_empty() {
        return Err(RadrootsTradeDvmError::MissingTag { tag: TAG_I });
    }
    Ok(RadrootsTradeTransitionProofRequestTags {
        worker_pubkey,
        listing_addr,
        inputs,
    })
}

pub fn build_transition_proof_result_tags(
    request_event: &RadrootsNostrEvent,
    customer_pubkey: &RadrootsPublicKey,
    inputs: &[RadrootsTradeDvmInputTag],
    binding: &RadrootsTradeTransitionProofResultBinding,
) -> Result<Vec<Vec<String>>, RadrootsTradeDvmError> {
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).map_err(|source| {
        RadrootsTradeDvmError::InvalidTag {
            tag: TAG_E,
            value: request_event.id.clone(),
            source,
        }
    })?;
    let request_json = serde_json::to_string(request_event)
        .map_err(RadrootsTradeDvmError::SerializeRequestEvent)?;
    let mut tags = vec![
        vec![RADROOTS_DVM_TAG_REQUEST.to_string(), request_json],
        vec![TAG_E.to_string(), request_event_id.as_str().to_string()],
        vec![TAG_P.to_string(), customer_pubkey.as_str().to_string()],
        vec![
            RADROOTS_DVM_TAG_LISTING_EVENT.to_string(),
            binding.listing_event_id.as_str().to_string(),
        ],
        vec![
            RADROOTS_DVM_TAG_ROOT_EVENT.to_string(),
            binding.root_event_id.as_str().to_string(),
        ],
        vec![
            RADROOTS_DVM_TAG_TARGET_EVENT.to_string(),
            binding.target_event_id.as_str().to_string(),
        ],
    ];
    if let Some(receipt_event_id) = binding.validation_receipt_event_id.as_ref() {
        tags.push(vec![
            RADROOTS_DVM_TAG_VALIDATION_RECEIPT.to_string(),
            receipt_event_id.as_str().to_string(),
        ]);
    }
    for input in inputs {
        tags.push(input_event_tag(input.role, &input.event_id));
    }
    Ok(tags)
}

pub fn parse_transition_proof_result_tags(
    kind: u32,
    tags: &[Vec<String>],
) -> Result<RadrootsTradeTransitionProofResultTags, RadrootsTradeDvmError> {
    if kind != KIND_TRADE_TRANSITION_PROOF_RESULT {
        return Err(RadrootsTradeDvmError::UnsupportedKind {
            expected: KIND_TRADE_TRANSITION_PROOF_RESULT,
            actual: kind,
        });
    }
    let request_event_json = required_tag_value(tags, RADROOTS_DVM_TAG_REQUEST)?;
    let request_event: RadrootsNostrEvent = serde_json::from_str(request_event_json)
        .map_err(RadrootsTradeDvmError::InvalidRequestEvent)?;
    if request_event.kind != KIND_TRADE_TRANSITION_PROOF_REQUEST {
        return Err(RadrootsTradeDvmError::RequestEventKind {
            kind: request_event.kind,
        });
    }
    let request_event_id = parse_event_id_tag(TAG_E, required_tag_value(tags, TAG_E)?)?;
    if request_event.id != request_event_id.as_str() {
        return Err(RadrootsTradeDvmError::RequestEventIdMismatch);
    }
    let customer_pubkey = parse_pubkey_tag(TAG_P, required_tag_value(tags, TAG_P)?)?;
    let binding = RadrootsTradeTransitionProofResultBinding {
        listing_event_id: parse_event_id_tag(
            RADROOTS_DVM_TAG_LISTING_EVENT,
            required_tag_value(tags, RADROOTS_DVM_TAG_LISTING_EVENT)?,
        )?,
        root_event_id: parse_event_id_tag(
            RADROOTS_DVM_TAG_ROOT_EVENT,
            required_tag_value(tags, RADROOTS_DVM_TAG_ROOT_EVENT)?,
        )?,
        target_event_id: parse_event_id_tag(
            RADROOTS_DVM_TAG_TARGET_EVENT,
            required_tag_value(tags, RADROOTS_DVM_TAG_TARGET_EVENT)?,
        )?,
        validation_receipt_event_id: optional_event_id_tag(
            RADROOTS_DVM_TAG_VALIDATION_RECEIPT,
            tag_value(tags, RADROOTS_DVM_TAG_VALIDATION_RECEIPT),
        )?,
    };
    Ok(RadrootsTradeTransitionProofResultTags {
        request_event,
        request_event_id,
        customer_pubkey,
        inputs: parse_input_tags(tags)?,
        binding,
    })
}

pub fn build_job_feedback_tags(
    status: RadrootsTradeDvmFeedbackStatus,
    request_event_id: &RadrootsEventId,
    customer_pubkey: &RadrootsPublicKey,
) -> Vec<Vec<String>> {
    vec![
        vec![TAG_STATUS.to_string(), status.as_str().to_string()],
        vec![TAG_E.to_string(), request_event_id.as_str().to_string()],
        vec![TAG_P.to_string(), customer_pubkey.as_str().to_string()],
    ]
}

pub fn parse_job_feedback_tags(
    kind: u32,
    tags: &[Vec<String>],
) -> Result<RadrootsTradeDvmFeedbackTags, RadrootsTradeDvmError> {
    if kind != KIND_JOB_FEEDBACK {
        return Err(RadrootsTradeDvmError::UnsupportedKind {
            expected: KIND_JOB_FEEDBACK,
            actual: kind,
        });
    }
    Ok(RadrootsTradeDvmFeedbackTags {
        status: RadrootsTradeDvmFeedbackStatus::parse(required_tag_value(tags, TAG_STATUS)?)?,
        request_event_id: parse_event_id_tag(TAG_E, required_tag_value(tags, TAG_E)?)?,
        customer_pubkey: parse_pubkey_tag(TAG_P, required_tag_value(tags, TAG_P)?)?,
    })
}

fn validate_transition_proof_request_binding(
    tags: &RadrootsTradeTransitionProofRequestTags,
    content: &RadrootsTradeTransitionProofRequestV1,
) -> Result<(), RadrootsTradeDvmError> {
    if tags.listing_addr != content.request.listing_addr {
        return Err(RadrootsTradeDvmError::ContentMismatch {
            field: "request.listing_addr",
        });
    }
    require_input(
        &tags.inputs,
        RadrootsTradeDvmInputRole::Listing,
        &content.listing_event_id,
    )?;
    require_input(
        &tags.inputs,
        RadrootsTradeDvmInputRole::OrderRequest,
        &content.request_event_id,
    )?;
    require_input(
        &tags.inputs,
        RadrootsTradeDvmInputRole::OrderDecision,
        &content.decision_event_id,
    )?;
    Ok(())
}

fn validate_transition_proof_request_content(
    content: &RadrootsTradeTransitionProofRequestV1,
) -> Result<(), RadrootsTradeDvmError> {
    if content.request.order_id != content.decision.order_id {
        return Err(RadrootsTradeDvmError::ContentMismatch { field: "order_id" });
    }
    if content.request.listing_addr != content.decision.listing_addr {
        return Err(RadrootsTradeDvmError::ContentMismatch {
            field: "listing_addr",
        });
    }
    if content.request.buyer_pubkey != content.decision.buyer_pubkey {
        return Err(RadrootsTradeDvmError::ContentMismatch {
            field: "buyer_pubkey",
        });
    }
    if content.request.seller_pubkey != content.decision.seller_pubkey {
        return Err(RadrootsTradeDvmError::ContentMismatch {
            field: "seller_pubkey",
        });
    }
    validate_hash32(&content.reducer_program_hash, "reducer_program_hash")?;
    if let Some(previous_state_root) = content.previous_state_root.as_ref() {
        validate_hash32(previous_state_root, "previous_state_root")?;
    }
    if let Some(sp1_program_hash) = content.sp1_program_hash.as_ref() {
        validate_hash32(sp1_program_hash, "sp1_program_hash")?;
    }
    if let Some(sp1_verifying_key_hash) = content.sp1_verifying_key_hash.as_ref() {
        validate_hash32(sp1_verifying_key_hash, "sp1_verifying_key_hash")?;
    }
    Ok(())
}

fn require_input(
    inputs: &[RadrootsTradeDvmInputTag],
    role: RadrootsTradeDvmInputRole,
    expected_event_id: &RadrootsEventId,
) -> Result<(), RadrootsTradeDvmError> {
    if inputs
        .iter()
        .any(|input| input.role == role && &input.event_id == expected_event_id)
    {
        return Ok(());
    }
    Err(RadrootsTradeDvmError::ContentMismatch {
        field: role.as_str(),
    })
}

fn parse_input_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsTradeDvmInputTag>, RadrootsTradeDvmError> {
    let mut inputs = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some(TAG_I))
    {
        let Some(value) = tag.get(1) else {
            return Err(RadrootsTradeDvmError::MissingTag { tag: TAG_I });
        };
        let input_type = tag.get(2).map(String::as_str).unwrap_or("");
        if input_type != RADROOTS_DVM_INPUT_TYPE_EVENT {
            return Err(RadrootsTradeDvmError::InvalidTag {
                tag: TAG_I,
                value: input_type.to_string(),
                source: RadrootsIdParseError::InvalidFormat,
            });
        }
        let Some(role_value) = tag.get(3) else {
            return Err(RadrootsTradeDvmError::MissingTag { tag: TAG_I });
        };
        inputs.push(RadrootsTradeDvmInputTag {
            event_id: parse_event_id_tag(TAG_I, value)?,
            role: RadrootsTradeDvmInputRole::parse(role_value)?,
        });
    }
    Ok(inputs)
}

fn input_event_tag(role: RadrootsTradeDvmInputRole, event_id: &RadrootsEventId) -> Vec<String> {
    vec![
        TAG_I.to_string(),
        event_id.as_str().to_string(),
        RADROOTS_DVM_INPUT_TYPE_EVENT.to_string(),
        role.as_str().to_string(),
    ]
}

fn required_tag_value<'a>(
    tags: &'a [Vec<String>],
    tag: &'static str,
) -> Result<&'a str, RadrootsTradeDvmError> {
    tag_value(tags, tag).ok_or(RadrootsTradeDvmError::MissingTag { tag })
}

fn tag_value<'a>(tags: &'a [Vec<String>], tag: &str) -> Option<&'a str> {
    tags.iter()
        .find(|candidate| candidate.first().map(String::as_str) == Some(tag))
        .and_then(|candidate| candidate.get(1))
        .map(String::as_str)
}

fn optional_event_id_tag(
    tag: &'static str,
    value: Option<&str>,
) -> Result<Option<RadrootsEventId>, RadrootsTradeDvmError> {
    value
        .map(|value| parse_event_id_tag(tag, value))
        .transpose()
}

fn parse_event_id_tag(
    tag: &'static str,
    value: &str,
) -> Result<RadrootsEventId, RadrootsTradeDvmError> {
    RadrootsEventId::parse(value).map_err(|source| RadrootsTradeDvmError::InvalidTag {
        tag,
        value: value.to_string(),
        source,
    })
}

fn parse_pubkey_tag(
    tag: &'static str,
    value: &str,
) -> Result<RadrootsPublicKey, RadrootsTradeDvmError> {
    RadrootsPublicKey::parse(value).map_err(|source| RadrootsTradeDvmError::InvalidTag {
        tag,
        value: value.to_string(),
        source,
    })
}

fn parse_listing_addr_tag(
    tag: &'static str,
    value: &str,
) -> Result<RadrootsListingAddress, RadrootsTradeDvmError> {
    RadrootsListingAddress::parse(value).map_err(|source| RadrootsTradeDvmError::InvalidTag {
        tag,
        value: value.to_string(),
        source,
    })
}

fn validate_hash32(value: &str, field: &'static str) -> Result<(), RadrootsTradeDvmError> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(RadrootsTradeDvmError::InvalidHash { field });
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RadrootsTradeDvmError::InvalidHash { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::kinds::KIND_LISTING;

    const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const WORKER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn event_id(raw: u8) -> RadrootsEventId {
        RadrootsEventId::parse(format!("{raw:064x}")).expect("event id")
    }

    fn public_key(raw: &str) -> RadrootsPublicKey {
        RadrootsPublicKey::parse(raw).expect("public key")
    }

    fn listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"))
            .expect("listing address")
    }

    fn hash(raw: char) -> String {
        format!("0x{}", raw.to_string().repeat(64))
    }

    fn request_content() -> RadrootsTradeTransitionProofRequestV1 {
        RadrootsTradeTransitionProofRequestV1 {
            witness_version: 1,
            proof_target: "trade_transition".to_string(),
            listing_event_id: event_id(1),
            request_event_id: event_id(2),
            decision_event_id: event_id(3),
            event_evidence: Vec::new(),
            request: RadrootsTradeOrderRequestWitnessDto {
                order_id: RadrootsOrderId::parse("order-1").expect("order id"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                items: vec![RadrootsTradeOrderItemWitnessDto {
                    bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
                    bin_count: 2,
                }],
            },
            decision: RadrootsTradeOrderDecisionEventWitnessDto {
                order_id: RadrootsOrderId::parse("order-1").expect("order id"),
                listing_addr: listing_addr(),
                buyer_pubkey: public_key(BUYER),
                seller_pubkey: public_key(SELLER),
                decision: RadrootsTradeOrderDecisionWitnessDto::Accepted {
                    inventory_commitments: vec![RadrootsTradeInventoryCommitmentWitnessDto {
                        bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
                        bin_count: 2,
                    }],
                },
            },
            inventory_bins: vec![RadrootsTradeInventoryBinWitnessDto {
                bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
                listing_capacity: 10,
                previous_reserved: 1,
            }],
            inventory_sequence: 7,
            previous_state_root: Some(hash('b')),
            proof_mode: RadrootsTradeProofMode::None,
            reducer_program_hash: hash('a'),
            radroots_protocol_version: "radroots-trade-v1".to_string(),
            sp1_program_hash: None,
            sp1_verifying_key_hash: None,
        }
    }

    fn request_event(content: &RadrootsTradeTransitionProofRequestV1) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: event_id(10).into_string(),
            author: BUYER.to_string(),
            created_at: 1,
            kind: KIND_TRADE_TRANSITION_PROOF_REQUEST,
            tags: build_transition_proof_request_tags(&public_key(WORKER), content),
            content: serde_json::to_string(content).expect("content"),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn transition_proof_request_tags_and_content_parse_together() {
        let content = request_content();
        let envelope =
            parse_transition_proof_request_event(&request_event(&content)).expect("request");

        assert_eq!(envelope.tags.worker_pubkey, public_key(WORKER));
        assert_eq!(envelope.tags.listing_addr, listing_addr());
        assert_eq!(envelope.content.request_event_id, event_id(2));
        assert_eq!(envelope.tags.inputs.len(), 3);
    }

    #[test]
    fn transition_proof_result_tags_bind_stringified_request() {
        let content = request_content();
        let request_event = request_event(&content);
        let request_tags =
            parse_transition_proof_request_tags(&request_event.tags).expect("request tags");
        let binding = RadrootsTradeTransitionProofResultBinding {
            listing_event_id: content.listing_event_id.clone(),
            root_event_id: content.request_event_id.clone(),
            target_event_id: content.decision_event_id.clone(),
            validation_receipt_event_id: Some(event_id(11)),
        };
        let tags = build_transition_proof_result_tags(
            &request_event,
            &public_key(BUYER),
            &request_tags.inputs,
            &binding,
        )
        .expect("result tags");

        let parsed = parse_transition_proof_result_tags(KIND_TRADE_TRANSITION_PROOF_RESULT, &tags)
            .expect("result tags");

        assert_eq!(parsed.request_event_id, event_id(10));
        assert_eq!(parsed.customer_pubkey, public_key(BUYER));
        assert_eq!(parsed.inputs, request_tags.inputs);
        assert_eq!(parsed.binding, binding);
    }

    #[test]
    fn job_feedback_tags_parse_required_status_event_and_customer() {
        let tags = build_job_feedback_tags(
            RadrootsTradeDvmFeedbackStatus::Processing,
            &event_id(10),
            &public_key(BUYER),
        );

        let parsed = parse_job_feedback_tags(KIND_JOB_FEEDBACK, &tags).expect("feedback tags");

        assert_eq!(parsed.status, RadrootsTradeDvmFeedbackStatus::Processing);
        assert_eq!(parsed.request_event_id, event_id(10));
        assert_eq!(parsed.customer_pubkey, public_key(BUYER));
    }

    #[test]
    fn transition_proof_request_rejects_missing_input_tags() {
        let content = request_content();
        let mut event = request_event(&content);
        event
            .tags
            .retain(|tag| tag.first().map(String::as_str) != Some(TAG_I));

        assert!(matches!(
            parse_transition_proof_request_event(&event),
            Err(RadrootsTradeDvmError::MissingTag { tag: TAG_I })
        ));
    }
}
