#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use base64::Engine as _;
use radroots_event::{
    RadrootsEventEnvelope,
    ids::{RadrootsAddressableCoordinate, RadrootsAddressableCoordinateParts},
    kinds::{KIND_TRADE_VALIDATION_RECEIPT, KIND_VALIDATOR_SET},
    tags::{TAG_A, TAG_D},
    wire::RadrootsNip01EventWireParts,
};
use radroots_identity::PublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const VALIDATION_RECEIPT_DOMAIN: &str = "radroots.receipt";
pub const VALIDATION_RECEIPT_VERSION: u32 = 1;
pub const VALIDATION_RECEIPT_PUBLIC_VALUES_HASH_DOMAIN: &[u8] = b"radroots:sp1-public-values:v1";
pub const VALIDATION_RECEIPT_PROOF_REFERENCE_SCHEME: &str = "radroots-proof://";
pub const VALIDATION_RECEIPT_PROOF_REFERENCE_SHA256_PREFIX: &str = "radroots-proof://sha256/";
pub const TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT: &str = "event_set_root";
pub const TAG_VALIDATION_RECEIPT_PROOF_SYSTEM: &str = "proof_system";
pub const TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH: &str = "public_values_hash";
pub const TAG_VALIDATION_RECEIPT_RECEIPT_TYPE: &str = "receipt_type";
pub const TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT: &str = "reducer_output_root";
pub const TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER: &str = "validator_set";
pub const VALIDATOR_SET_V1_OPERATOR_CONTACT_MAX_CHARS: usize = 240;
pub const VALIDATOR_SET_V1_OPERATOR_NAME_MAX_CHARS: usize = 120;
pub const VALIDATOR_SET_V1_THRESHOLD: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsValidatorSetV1 {
    pub set_id: String,
    pub validator_pubkey: PublicKey,
    pub threshold: u8,
    pub valid_from: u64,
    pub valid_until: u64,
    pub protocol_contract_hash: String,
    pub operator_name: String,
    pub operator_contact: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsVerifiedValidatorSetV1 {
    pub set: RadrootsValidatorSetV1,
    pub event_id: String,
    pub address: RadrootsAddressableCoordinate,
    pub authority_pubkey: PublicKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsValidationReceiptType {
    ListingValidation,
    TradeTransition,
    InventoryState,
    StateCheckpoint,
}

impl RadrootsValidationReceiptType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ListingValidation => "listing_validation",
            Self::TradeTransition => "trade_transition",
            Self::InventoryState => "inventory_state",
            Self::StateCheckpoint => "state_checkpoint",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "listing_validation" => Some(Self::ListingValidation),
            "trade_transition" => Some(Self::TradeTransition),
            "inventory_state" => Some(Self::InventoryState),
            "state_checkpoint" => Some(Self::StateCheckpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsValidationReceiptResult {
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsTradeValidationAuthority {
    ValidatorSetDeterministic,
    CryptographicProofVerified,
    ValidatorSetAndProofVerified,
}

impl RadrootsTradeValidationAuthority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ValidatorSetDeterministic => "validator_set_deterministic",
            Self::CryptographicProofVerified => "cryptographic_proof_verified",
            Self::ValidatorSetAndProofVerified => "validator_set_and_proof_verified",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "validator_set_deterministic" => Some(Self::ValidatorSetDeterministic),
            "cryptographic_proof_verified" => Some(Self::CryptographicProofVerified),
            "validator_set_and_proof_verified" => Some(Self::ValidatorSetAndProofVerified),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsTradeCommitmentConfidence {
    AwaitingValidation,
    CommittedByValidatorSet,
    CommittedByCryptographicProof,
    CommittedByValidatorSetAndProof,
    Invalid,
}

impl RadrootsTradeCommitmentConfidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingValidation => "awaiting_validation",
            Self::CommittedByValidatorSet => "committed_by_validator_set",
            Self::CommittedByCryptographicProof => "committed_by_cryptographic_proof",
            Self::CommittedByValidatorSetAndProof => "committed_by_validator_set_and_proof",
            Self::Invalid => "invalid",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "awaiting_validation" => Some(Self::AwaitingValidation),
            "committed_by_validator_set" => Some(Self::CommittedByValidatorSet),
            "committed_by_cryptographic_proof" => Some(Self::CommittedByCryptographicProof),
            "committed_by_validator_set_and_proof" => Some(Self::CommittedByValidatorSetAndProof),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsTradeValidationTrustPolicy {
    pub validator_set: Option<RadrootsValidatorSetV1>,
    pub validator_set_addr: Option<RadrootsAddressableCoordinate>,
    pub validator_set_event_id: Option<String>,
    pub require_cryptographic_proof: bool,
}

impl Default for RadrootsTradeValidationTrustPolicy {
    fn default() -> Self {
        Self::production()
    }
}

impl RadrootsTradeValidationTrustPolicy {
    pub fn production() -> Self {
        Self {
            validator_set: None,
            validator_set_addr: None,
            validator_set_event_id: None,
            require_cryptographic_proof: false,
        }
    }

    pub fn explicit_dev_test() -> Self {
        Self::production()
    }

    pub fn with_validator_set(
        mut self,
        validator_set: RadrootsValidatorSetV1,
        validator_set_addr: RadrootsAddressableCoordinate,
        validator_set_event_id: impl Into<String>,
    ) -> Self {
        self.validator_set = Some(validator_set);
        self.validator_set_addr = Some(validator_set_addr);
        self.validator_set_event_id = Some(validator_set_event_id.into());
        self
    }

    pub fn has_validator_set(&self) -> bool {
        self.validator_set.is_some()
            && self.validator_set_addr.is_some()
            && self.validator_set_event_id.is_some()
    }

    pub fn with_require_cryptographic_proof(mut self, require_cryptographic_proof: bool) -> Self {
        self.require_cryptographic_proof = require_cryptographic_proof;
        self
    }

    pub fn trusts_validator_pubkey(&self, pubkey: &PublicKey) -> bool {
        self.validator_set
            .as_ref()
            .is_some_and(|validator_set| validator_set.validator_pubkey == *pubkey)
    }

    pub fn validator_count(&self) -> usize {
        usize::from(self.validator_set.is_some())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsTradeValidationTrustState {
    Pending,
    Untrusted,
    ValidatorSetCommitted,
    CryptographicCommitted,
    Invalid,
}

impl RadrootsTradeValidationTrustState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Untrusted => "untrusted",
            Self::ValidatorSetCommitted => "validator_set_committed",
            Self::CryptographicCommitted => "cryptographic_committed",
            Self::Invalid => "invalid",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "untrusted" => Some(Self::Untrusted),
            "validator_set_committed" => Some(Self::ValidatorSetCommitted),
            "cryptographic_committed" => Some(Self::CryptographicCommitted),
            "invalid" => Some(Self::Invalid),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsValidationReceiptProofSystem {
    None,
    Sp1Core,
    Sp1Compressed,
    Sp1Groth16,
    Sp1Plonk,
}

impl RadrootsValidationReceiptProofSystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sp1Core => "sp1_core",
            Self::Sp1Compressed => "sp1_compressed",
            Self::Sp1Groth16 => "sp1_groth16",
            Self::Sp1Plonk => "sp1_plonk",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "sp1_core" => Some(Self::Sp1Core),
            "sp1_compressed" => Some(Self::Sp1Compressed),
            "sp1_groth16" => Some(Self::Sp1Groth16),
            "sp1_plonk" => Some(Self::Sp1Plonk),
            _ => None,
        }
    }

    const fn expected_mode(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Sp1Core => Some("core"),
            Self::Sp1Compressed => Some("compressed"),
            Self::Sp1Groth16 => Some("groth16"),
            Self::Sp1Plonk => Some("plonk"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsValidationReceiptStatement {
    pub listing_event_id: String,
    pub root_event_id: String,
    pub target_event_id: String,
    pub validator_set_addr: RadrootsAddressableCoordinate,
    pub validator_set_event_id: String,
    #[serde(rename = "type")]
    pub statement_type: RadrootsValidationReceiptType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsValidationReceiptProof {
    pub inline_proof_base64: Option<String>,
    pub mode: Option<String>,
    pub program_hash: Option<String>,
    pub proof_reference: Option<String>,
    pub system: RadrootsValidationReceiptProofSystem,
    pub verifying_key_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsTradeValidationReceipt {
    pub changed_records_root: String,
    pub domain: String,
    pub error_bitmap: String,
    pub event_set_root: String,
    pub new_state_root: String,
    pub previous_state_root: String,
    pub proof: RadrootsValidationReceiptProof,
    pub public_values_hash: String,
    pub receipt_type: RadrootsValidationReceiptType,
    pub result: RadrootsValidationReceiptResult,
    pub statement: RadrootsValidationReceiptStatement,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsValidationReceiptTags {
    pub event_set_root: String,
    pub listing_event_id: String,
    pub order_id: String,
    pub proof_system: RadrootsValidationReceiptProofSystem,
    pub public_values_hash: String,
    pub receipt_type: RadrootsValidationReceiptType,
    pub reducer_output_root: String,
    pub root_event_id: String,
    pub target_event_id: String,
    pub validator_set_addr: RadrootsAddressableCoordinate,
    pub validator_set_event_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadrootsValidationReceiptExpectedBinding<'a> {
    pub event_set_root: Option<&'a str>,
    pub listing_event_id: Option<&'a str>,
    pub order_id: Option<&'a str>,
    pub program_hash: Option<&'a str>,
    pub proof_system: Option<RadrootsValidationReceiptProofSystem>,
    pub public_values_hash: Option<&'a str>,
    pub reducer_output_root: Option<&'a str>,
    pub root_event_id: Option<&'a str>,
    pub target_event_id: Option<&'a str>,
    pub validator_set_addr: Option<&'a str>,
    pub validator_set_event_id: Option<&'a str>,
    pub verifying_key_hash: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsVerifiedValidationReceipt {
    pub receipt: RadrootsTradeValidationReceipt,
    pub tags: RadrootsValidationReceiptTags,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RadrootsValidationReceiptError {
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    #[error("invalid event kind {got}; expected {expected}")]
    InvalidKind { expected: u32, got: u32 },
    #[error("invalid validation receipt json")]
    InvalidJson,
    #[error("validation receipt json is not canonical")]
    NonCanonicalJson,
    #[error("invalid validation receipt field {0}")]
    InvalidField(&'static str),
    #[error("invalid validation receipt proof metadata {0}")]
    InvalidProofMetadata(&'static str),
    #[error("missing validation receipt tag {0}")]
    MissingTag(&'static str),
    #[error("invalid validation receipt tag {0}")]
    InvalidTag(&'static str),
    #[error("validation receipt tag {0} does not match content")]
    TagMismatch(&'static str),
    #[error("validation receipt expected binding {0} does not match")]
    ExpectedBindingMismatch(&'static str),
}

impl RadrootsValidatorSetV1 {
    pub fn validate(&self) -> Result<(), RadrootsValidationReceiptError> {
        validate_uuidv7(&self.set_id, "validator_set.set_id")?;
        if self.threshold != VALIDATOR_SET_V1_THRESHOLD {
            return Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.threshold",
            ));
        }
        if self.valid_from >= self.valid_until {
            return Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.valid_until",
            ));
        }
        validate_hash32(
            &self.protocol_contract_hash,
            "validator_set.protocol_contract_hash",
        )?;
        validate_bounded_text(
            &self.operator_name,
            VALIDATOR_SET_V1_OPERATOR_NAME_MAX_CHARS,
            "validator_set.operator_name",
        )?;
        if let Some(operator_contact) = self.operator_contact.as_ref() {
            validate_bounded_text(
                operator_contact,
                VALIDATOR_SET_V1_OPERATOR_CONTACT_MAX_CHARS,
                "validator_set.operator_contact",
            )?;
        }
        Ok(())
    }
}

pub fn validator_set_address(
    authority_pubkey: &PublicKey,
    set_id: &str,
) -> Result<RadrootsAddressableCoordinate, RadrootsValidationReceiptError> {
    validate_uuidv7(set_id, "validator_set.set_id")?;
    RadrootsAddressableCoordinate::parse(format!(
        "{KIND_VALIDATOR_SET}:{authority_pubkey}:{set_id}"
    ))
    .map_err(|_| RadrootsValidationReceiptError::InvalidField("validator_set.address"))
}

pub fn validator_set_address_from_str(
    value: impl AsRef<str>,
) -> Result<RadrootsAddressableCoordinate, RadrootsValidationReceiptError> {
    let address = RadrootsAddressableCoordinate::parse(value.as_ref())
        .map_err(|_| RadrootsValidationReceiptError::InvalidField("validator_set.address"))?;
    validate_validator_set_address(&address, "validator_set.address")?;
    Ok(address)
}

pub fn validator_set_canonical_content(
    validator_set: &RadrootsValidatorSetV1,
) -> Result<String, RadrootsValidationReceiptError> {
    validator_set.validate()?;
    Ok(serde_json::to_string(validator_set)
        .expect("validated validator sets contain only serializable contract values"))
}

pub fn validator_set_content_from_str(
    content: &str,
) -> Result<RadrootsValidatorSetV1, RadrootsValidationReceiptError> {
    let validator_set: RadrootsValidatorSetV1 =
        serde_json::from_str(content).map_err(|_| RadrootsValidationReceiptError::InvalidJson)?;
    validator_set.validate()?;
    let canonical = validator_set_canonical_content(&validator_set)?;
    if canonical != content {
        return Err(RadrootsValidationReceiptError::NonCanonicalJson);
    }
    Ok(validator_set)
}

pub fn validator_set_event_build(
    validator_set: &RadrootsValidatorSetV1,
) -> Result<RadrootsNip01EventWireParts, RadrootsValidationReceiptError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_VALIDATOR_SET,
        content: validator_set_canonical_content(validator_set)?,
        tags: vec![vec![TAG_D.to_string(), validator_set.set_id.clone()]],
    })
}

pub fn validator_set_from_event(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsVerifiedValidatorSetV1, RadrootsValidationReceiptError> {
    verify_validator_set_event(event, None)
}

pub fn verify_validator_set_event(
    event: &RadrootsEventEnvelope,
    expected_author: Option<&PublicKey>,
) -> Result<RadrootsVerifiedValidatorSetV1, RadrootsValidationReceiptError> {
    if event.kind_u32() != KIND_VALIDATOR_SET {
        return Err(RadrootsValidationReceiptError::InvalidKind {
            expected: KIND_VALIDATOR_SET,
            got: event.kind_u32(),
        });
    }
    if let Some(expected_author) = expected_author
        && event.author() != expected_author
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "validator_set.author",
        ));
    }
    let validator_set = validator_set_content_from_str(event.content())?;
    let tags = event.tags_as_vec();
    let d_tag = required_tag_value(&tags, TAG_D)?;
    if d_tag != validator_set.set_id {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "validator_set.set_id",
        ));
    }
    let address = validator_set_address(event.author(), &validator_set.set_id)?;
    Ok(RadrootsVerifiedValidatorSetV1 {
        set: validator_set,
        event_id: event.id_str().to_owned(),
        address,
        authority_pubkey: *event.author(),
    })
}

impl RadrootsTradeValidationReceipt {
    pub fn validate(&self) -> Result<(), RadrootsValidationReceiptError> {
        if self.version != VALIDATION_RECEIPT_VERSION {
            return Err(RadrootsValidationReceiptError::InvalidField("version"));
        }
        if self.domain != VALIDATION_RECEIPT_DOMAIN {
            return Err(RadrootsValidationReceiptError::InvalidField("domain"));
        }
        if self.receipt_type != self.statement.statement_type {
            return Err(RadrootsValidationReceiptError::InvalidField(
                "statement.type",
            ));
        }
        validate_hash32(&self.changed_records_root, "changed_records_root")?;
        validate_error_bitmap(&self.error_bitmap)?;
        validate_hash32(&self.event_set_root, "event_set_root")?;
        validate_hash32(&self.new_state_root, "new_state_root")?;
        validate_hash32(&self.previous_state_root, "previous_state_root")?;
        validate_hash32(&self.public_values_hash, "public_values_hash")?;
        validate_event_id(
            &self.statement.listing_event_id,
            "statement.listing_event_id",
        )?;
        validate_event_id(&self.statement.root_event_id, "statement.root_event_id")?;
        validate_event_id(&self.statement.target_event_id, "statement.target_event_id")?;
        validate_event_id(
            &self.statement.validator_set_event_id,
            "statement.validator_set_event_id",
        )?;
        validate_validator_set_address(
            &self.statement.validator_set_addr,
            "statement.validator_set_addr",
        )?;
        validate_result_error_bitmap(self.result, &self.error_bitmap)?;
        self.proof.validate()?;
        Ok(())
    }
}

impl RadrootsValidationReceiptProof {
    pub fn validate(&self) -> Result<(), RadrootsValidationReceiptError> {
        match self.system {
            RadrootsValidationReceiptProofSystem::None => {
                if self.inline_proof_base64.is_some()
                    || self.mode.is_some()
                    || self.program_hash.is_some()
                    || self.proof_reference.is_some()
                    || self.verifying_key_hash.is_some()
                {
                    return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                        "proof.system",
                    ));
                }
            }
            system => {
                validate_required_option_hash32(&self.program_hash, "proof.program_hash")?;
                validate_required_option_hash32(
                    &self.verifying_key_hash,
                    "proof.verifying_key_hash",
                )?;
                if self.mode.as_deref() != system.expected_mode() {
                    return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                        "proof.mode",
                    ));
                }
                match (&self.inline_proof_base64, &self.proof_reference) {
                    (Some(inline), None) => validate_inline_proof_base64(inline)?,
                    (None, Some(reference)) => validate_proof_reference(reference)?,
                    (None, None) => {
                        return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                            "proof.material_missing",
                        ));
                    }
                    (Some(_), Some(_)) => {
                        return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                            "proof.material_conflict",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn validation_receipt_public_values_hash_hex(public_values: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(VALIDATION_RECEIPT_PUBLIC_VALUES_HASH_DOMAIN);
    hasher.update(public_values);
    format!("0x{}", hex::encode(hasher.finalize()))
}

pub fn validation_receipt_canonical_content(
    receipt: &RadrootsTradeValidationReceipt,
) -> Result<String, RadrootsValidationReceiptError> {
    receipt.validate()?;
    serde_json::to_string(receipt).map_err(|_| RadrootsValidationReceiptError::InvalidJson)
}

pub fn validation_receipt_content_from_str(
    content: &str,
) -> Result<RadrootsTradeValidationReceipt, RadrootsValidationReceiptError> {
    let receipt: RadrootsTradeValidationReceipt =
        serde_json::from_str(content).map_err(|_| RadrootsValidationReceiptError::InvalidJson)?;
    receipt.validate()?;
    let canonical = validation_receipt_canonical_content(&receipt)?;
    if canonical != content {
        return Err(RadrootsValidationReceiptError::NonCanonicalJson);
    }
    Ok(receipt)
}

pub fn validation_receipt_tags(
    order_id: &str,
    receipt: &RadrootsTradeValidationReceipt,
) -> Result<Vec<Vec<String>>, RadrootsValidationReceiptError> {
    receipt.validate()?;
    validate_required_str(order_id, "order_id")?;
    Ok(vec![
        vec![TAG_D.to_string(), order_id.to_string()],
        vec![
            "e".to_string(),
            receipt.statement.listing_event_id.clone(),
            String::new(),
            String::new(),
            "listing".to_string(),
        ],
        vec![
            "e".to_string(),
            receipt.statement.root_event_id.clone(),
            String::new(),
            String::new(),
            "root".to_string(),
        ],
        vec![
            "e".to_string(),
            receipt.statement.target_event_id.clone(),
            String::new(),
            String::new(),
            "target".to_string(),
        ],
        vec![
            TAG_A.to_string(),
            receipt.statement.validator_set_addr.as_str().to_owned(),
            String::new(),
            TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER.to_string(),
        ],
        vec![
            "e".to_string(),
            receipt.statement.validator_set_event_id.clone(),
            String::new(),
            String::new(),
            TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER.to_string(),
        ],
        vec![
            TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT.to_string(),
            receipt.event_set_root.clone(),
        ],
        vec![
            TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT.to_string(),
            receipt.new_state_root.clone(),
        ],
        vec![
            TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH.to_string(),
            receipt.public_values_hash.clone(),
        ],
        vec![
            TAG_VALIDATION_RECEIPT_PROOF_SYSTEM.to_string(),
            receipt.proof.system.as_str().to_string(),
        ],
        vec![
            TAG_VALIDATION_RECEIPT_RECEIPT_TYPE.to_string(),
            receipt.receipt_type.as_str().to_string(),
        ],
    ])
}

pub fn validation_receipt_tags_from_tags(
    tags: &[Vec<String>],
) -> Result<RadrootsValidationReceiptTags, RadrootsValidationReceiptError> {
    let order_id = required_tag_value(tags, TAG_D)?;
    let listing_event_id = required_event_marker(tags, "listing")?;
    let root_event_id = required_event_marker(tags, "root")?;
    let target_event_id = required_event_marker(tags, "target")?;
    let validator_set_addr =
        required_address_marker(tags, TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER)?;
    let validator_set_event_id =
        required_event_marker(tags, TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER)?;
    let event_set_root = required_tag_value(tags, TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT)?;
    let reducer_output_root = required_tag_value(tags, TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT)?;
    let public_values_hash = required_tag_value(tags, TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH)?;
    let proof_system_label = required_tag_value(tags, TAG_VALIDATION_RECEIPT_PROOF_SYSTEM)?;
    let proof_system = RadrootsValidationReceiptProofSystem::from_label(&proof_system_label)
        .ok_or(RadrootsValidationReceiptError::InvalidTag(
            TAG_VALIDATION_RECEIPT_PROOF_SYSTEM,
        ))?;
    let receipt_type_label = required_tag_value(tags, TAG_VALIDATION_RECEIPT_RECEIPT_TYPE)?;
    let receipt_type = RadrootsValidationReceiptType::from_label(&receipt_type_label).ok_or(
        RadrootsValidationReceiptError::InvalidTag(TAG_VALIDATION_RECEIPT_RECEIPT_TYPE),
    )?;

    validate_event_id(&listing_event_id, "tags.e.listing")?;
    validate_event_id(&root_event_id, "tags.e.root")?;
    validate_event_id(&target_event_id, "tags.e.target")?;
    validate_event_id(&validator_set_event_id, "tags.e.validator_set")?;
    validate_validator_set_address(&validator_set_addr, "tags.a.validator_set")?;
    validate_hash32(&event_set_root, TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT)?;
    validate_hash32(
        &reducer_output_root,
        TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT,
    )?;
    validate_hash32(
        &public_values_hash,
        TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH,
    )?;

    Ok(RadrootsValidationReceiptTags {
        event_set_root,
        listing_event_id,
        order_id,
        proof_system,
        public_values_hash,
        receipt_type,
        reducer_output_root,
        root_event_id,
        target_event_id,
        validator_set_addr,
        validator_set_event_id,
    })
}

pub fn validation_receipt_event_build(
    order_id: &str,
    receipt: &RadrootsTradeValidationReceipt,
) -> Result<RadrootsNip01EventWireParts, RadrootsValidationReceiptError> {
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_TRADE_VALIDATION_RECEIPT,
        content: validation_receipt_canonical_content(receipt)?,
        tags: validation_receipt_tags(order_id, receipt)?,
    })
}

pub fn validation_receipt_from_event(
    event: &RadrootsEventEnvelope,
) -> Result<RadrootsVerifiedValidationReceipt, RadrootsValidationReceiptError> {
    verify_validation_receipt_event(event, RadrootsValidationReceiptExpectedBinding::default())
}

pub fn verify_validation_receipt_event(
    event: &RadrootsEventEnvelope,
    expected: RadrootsValidationReceiptExpectedBinding<'_>,
) -> Result<RadrootsVerifiedValidationReceipt, RadrootsValidationReceiptError> {
    if event.kind_u32() != KIND_TRADE_VALIDATION_RECEIPT {
        return Err(RadrootsValidationReceiptError::InvalidKind {
            expected: KIND_TRADE_VALIDATION_RECEIPT,
            got: event.kind_u32(),
        });
    }

    let receipt = validation_receipt_content_from_str(event.content())?;
    let event_tags = event.tags_as_vec();
    let tags = validation_receipt_tags_from_tags(&event_tags)?;

    if tags.listing_event_id != receipt.statement.listing_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "listing_event_id",
        ));
    }
    if tags.root_event_id != receipt.statement.root_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch("root_event_id"));
    }
    if tags.target_event_id != receipt.statement.target_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "target_event_id",
        ));
    }
    if tags.validator_set_addr != receipt.statement.validator_set_addr {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "validator_set_addr",
        ));
    }
    if tags.validator_set_event_id != receipt.statement.validator_set_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "validator_set_event_id",
        ));
    }
    if tags.event_set_root != receipt.event_set_root {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "event_set_root",
        ));
    }
    if tags.reducer_output_root != receipt.new_state_root {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "reducer_output_root",
        ));
    }
    if tags.public_values_hash != receipt.public_values_hash {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "public_values_hash",
        ));
    }
    if tags.proof_system != receipt.proof.system {
        return Err(RadrootsValidationReceiptError::TagMismatch("proof_system"));
    }
    if tags.receipt_type != receipt.receipt_type {
        return Err(RadrootsValidationReceiptError::TagMismatch("receipt_type"));
    }

    validate_expected_binding(&tags, &receipt, expected)?;

    Ok(RadrootsVerifiedValidationReceipt { receipt, tags })
}

fn validate_expected_binding(
    tags: &RadrootsValidationReceiptTags,
    receipt: &RadrootsTradeValidationReceipt,
    expected: RadrootsValidationReceiptExpectedBinding<'_>,
) -> Result<(), RadrootsValidationReceiptError> {
    if let Some(order_id) = expected.order_id
        && tags.order_id != order_id
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "order_id",
        ));
    }
    if let Some(listing_event_id) = expected.listing_event_id
        && tags.listing_event_id != listing_event_id
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "listing_event_id",
        ));
    }
    if let Some(root_event_id) = expected.root_event_id
        && tags.root_event_id != root_event_id
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "root_event_id",
        ));
    }
    if let Some(target_event_id) = expected.target_event_id
        && tags.target_event_id != target_event_id
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "target_event_id",
        ));
    }
    if let Some(validator_set_addr) = expected.validator_set_addr
        && tags.validator_set_addr.as_str() != validator_set_addr
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "validator_set_addr",
        ));
    }
    if let Some(validator_set_event_id) = expected.validator_set_event_id
        && tags.validator_set_event_id != validator_set_event_id
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "validator_set_event_id",
        ));
    }
    if let Some(event_set_root) = expected.event_set_root
        && tags.event_set_root != event_set_root
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "event_set_root",
        ));
    }
    if let Some(reducer_output_root) = expected.reducer_output_root
        && tags.reducer_output_root != reducer_output_root
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "reducer_output_root",
        ));
    }
    if let Some(public_values_hash) = expected.public_values_hash
        && tags.public_values_hash != public_values_hash
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "public_values_hash",
        ));
    }
    if let Some(proof_system) = expected.proof_system
        && tags.proof_system != proof_system
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "proof_system",
        ));
    }
    if let Some(program_hash) = expected.program_hash
        && receipt.proof.program_hash.as_deref() != Some(program_hash)
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "program_hash",
        ));
    }
    if let Some(verifying_key_hash) = expected.verifying_key_hash
        && receipt.proof.verifying_key_hash.as_deref() != Some(verifying_key_hash)
    {
        return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
            "verifying_key_hash",
        ));
    }
    Ok(())
}

fn required_tag_value(
    tags: &[Vec<String>],
    name: &'static str,
) -> Result<String, RadrootsValidationReceiptError> {
    let mut matches = tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(name));
    let tag = matches
        .next()
        .ok_or(RadrootsValidationReceiptError::MissingTag(name))?;
    if matches.next().is_some() {
        return Err(RadrootsValidationReceiptError::InvalidTag(name));
    }
    let value = tag
        .get(1)
        .ok_or(RadrootsValidationReceiptError::InvalidTag(name))?;
    validate_required_str(value, name)?;
    Ok(value.clone())
}

fn required_event_marker(
    tags: &[Vec<String>],
    marker: &'static str,
) -> Result<String, RadrootsValidationReceiptError> {
    let mut matches = tags.iter().filter(|tag| {
        tag.first().map(|value| value.as_str()) == Some("e")
            && tag.get(4).map(|value| value.as_str()) == Some(marker)
    });
    let tag = matches
        .next()
        .ok_or(RadrootsValidationReceiptError::MissingTag(marker))?;
    if matches.next().is_some() {
        return Err(RadrootsValidationReceiptError::InvalidTag(marker));
    }
    let value = &tag[1];
    validate_required_str(value, marker)?;
    Ok(value.clone())
}

fn required_address_marker(
    tags: &[Vec<String>],
    marker: &'static str,
) -> Result<RadrootsAddressableCoordinate, RadrootsValidationReceiptError> {
    let mut matches = tags.iter().filter(|tag| {
        tag.first().map(|value| value.as_str()) == Some(TAG_A)
            && tag.get(3).map(|value| value.as_str()) == Some(marker)
    });
    let tag = matches
        .next()
        .ok_or(RadrootsValidationReceiptError::MissingTag(marker))?;
    if matches.next().is_some() {
        return Err(RadrootsValidationReceiptError::InvalidTag(marker));
    }
    let value = tag
        .get(1)
        .ok_or(RadrootsValidationReceiptError::InvalidTag(marker))?;
    validate_required_str(value, marker)?;
    RadrootsAddressableCoordinate::parse(value)
        .map_err(|_| RadrootsValidationReceiptError::InvalidTag(marker))
}

fn validate_required_option_hash32(
    value: &Option<String>,
    field: &'static str,
) -> Result<(), RadrootsValidationReceiptError> {
    match value {
        Some(value) => validate_hash32(value, field),
        None => Err(RadrootsValidationReceiptError::InvalidProofMetadata(field)),
    }
}

fn validate_required_str(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsValidationReceiptError> {
    if value.trim().is_empty() {
        return Err(RadrootsValidationReceiptError::EmptyField(field));
    }
    Ok(())
}

fn validate_bounded_text(
    value: &str,
    max_chars: usize,
    field: &'static str,
) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, field)?;
    if value.chars().count() > max_chars {
        return Err(RadrootsValidationReceiptError::InvalidField(field));
    }
    Ok(())
}

fn validate_uuidv7(value: &str, field: &'static str) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, field)?;
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || bytes[8] != b'-'
        || bytes[13] != b'-'
        || bytes[18] != b'-'
        || bytes[23] != b'-'
        || bytes[14] != b'7'
        || !matches!(bytes[19], b'8'..=b'9' | b'a'..=b'b')
    {
        return Err(RadrootsValidationReceiptError::InvalidField(field));
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte) {
            return Err(RadrootsValidationReceiptError::InvalidField(field));
        }
    }
    Ok(())
}

fn validate_validator_set_address(
    value: &RadrootsAddressableCoordinate,
    field: &'static str,
) -> Result<(), RadrootsValidationReceiptError> {
    let parts = RadrootsAddressableCoordinateParts::parse(value.as_str())
        .expect("typed addressable coordinates must contain valid coordinate parts");
    if parts.kind != KIND_VALIDATOR_SET {
        return Err(RadrootsValidationReceiptError::InvalidField(field));
    }
    validate_uuidv7(parts.d_tag.as_str(), field)
}

fn validate_inline_proof_base64(value: &str) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, "proof.inline_proof_base64")?;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| {
            RadrootsValidationReceiptError::InvalidProofMetadata("proof.inline_proof_base64")
        })?;

    Ok(())
}

fn validate_proof_reference(value: &str) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, "proof.proof_reference")?;
    let digest = value
        .strip_prefix(VALIDATION_RECEIPT_PROOF_REFERENCE_SHA256_PREFIX)
        .ok_or(RadrootsValidationReceiptError::InvalidProofMetadata(
            "proof.proof_reference",
        ))?;
    if digest.len() != 64 || !is_lower_hex(digest) {
        return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
            "proof.proof_reference",
        ));
    }
    Ok(())
}

fn validate_result_error_bitmap(
    result: RadrootsValidationReceiptResult,
    error_bitmap: &str,
) -> Result<(), RadrootsValidationReceiptError> {
    match result {
        RadrootsValidationReceiptResult::Valid if error_bitmap != zero_error_bitmap() => {
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        }
        RadrootsValidationReceiptResult::Invalid if error_bitmap == zero_error_bitmap() => {
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        }
        _ => Ok(()),
    }
}

fn validate_error_bitmap(value: &str) -> Result<(), RadrootsValidationReceiptError> {
    if value.len() != 34 || !value.starts_with("0x") || !is_lower_hex(&value[2..]) {
        return Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"));
    }
    Ok(())
}

fn validate_hash32(value: &str, field: &'static str) -> Result<(), RadrootsValidationReceiptError> {
    if value.len() != 66 || !value.starts_with("0x") || !is_lower_hex(&value[2..]) {
        return Err(RadrootsValidationReceiptError::InvalidField(field));
    }
    Ok(())
}

fn validate_event_id(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsValidationReceiptError> {
    if value.len() != 64 || !is_lower_hex(value) {
        return Err(RadrootsValidationReceiptError::InvalidField(field));
    }
    Ok(())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn zero_error_bitmap() -> &'static str {
    "0x00000000000000000000000000000000"
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsTradeCommitmentConfidence, RadrootsTradeValidationAuthority,
        RadrootsTradeValidationReceipt, RadrootsTradeValidationTrustPolicy,
        RadrootsTradeValidationTrustState, RadrootsValidationReceiptError,
        RadrootsValidationReceiptExpectedBinding, RadrootsValidationReceiptProof,
        RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
        RadrootsValidationReceiptStatement, RadrootsValidationReceiptType, RadrootsValidatorSetV1,
        TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT, TAG_VALIDATION_RECEIPT_PROOF_SYSTEM,
        TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH, TAG_VALIDATION_RECEIPT_RECEIPT_TYPE,
        TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT, TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER,
        validation_receipt_canonical_content, validation_receipt_content_from_str,
        validation_receipt_event_build, validation_receipt_from_event,
        validation_receipt_public_values_hash_hex, validation_receipt_tags,
        validation_receipt_tags_from_tags, validator_set_address, validator_set_canonical_content,
        validator_set_content_from_str, validator_set_event_build, validator_set_from_event,
        verify_validation_receipt_event, verify_validator_set_event,
    };
    use radroots_event::{
        RadrootsEventEnvelope, RadrootsEventEnvelopeParts,
        ids::RadrootsAddressableCoordinate,
        kinds::{KIND_TRADE_VALIDATION_RECEIPT, KIND_VALIDATOR_SET},
        tags::TAG_D,
    };
    use radroots_identity::PublicKey;
    use radroots_test_fixtures::{
        FIXTURE_BOB_PUBLIC_KEY_HEX, FIXTURE_CAROL_PUBLIC_KEY_HEX, FIXTURE_DIEGO_PUBLIC_KEY_HEX,
    };

    fn hash32(c: char) -> String {
        format!("0x{}", c.to_string().repeat(64))
    }

    fn event_id(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn validator_set_id() -> String {
        "018f3d99-7d35-7c0c-8a0f-7f3b645abcde".to_string()
    }

    fn validator_set_author() -> PublicKey {
        PublicKey::from_hex(FIXTURE_DIEGO_PUBLIC_KEY_HEX).expect("validator set author")
    }

    fn validator_set_pubkey() -> PublicKey {
        PublicKey::from_hex(FIXTURE_CAROL_PUBLIC_KEY_HEX).expect("validator pubkey")
    }

    fn validator_set_addr() -> radroots_event::ids::RadrootsAddressableCoordinate {
        validator_set_address(&validator_set_author(), &validator_set_id())
            .expect("validator set address")
    }

    fn sample_validator_set() -> RadrootsValidatorSetV1 {
        RadrootsValidatorSetV1 {
            set_id: validator_set_id(),
            validator_pubkey: validator_set_pubkey(),
            threshold: 1,
            valid_from: 1_700_000_000,
            valid_until: 1_800_000_000,
            protocol_contract_hash: hash32('7'),
            operator_name: "Radroots validation operator".to_string(),
            operator_contact: Some("validator@example.invalid".to_string()),
        }
    }

    fn sample_validation_receipt() -> RadrootsTradeValidationReceipt {
        RadrootsTradeValidationReceipt {
            changed_records_root: hash32('6'),
            domain: "radroots.receipt".to_string(),
            error_bitmap: "0x00000000000000000000000000000000".to_string(),
            event_set_root: hash32('c'),
            new_state_root: hash32('4'),
            previous_state_root: hash32('3'),
            proof: RadrootsValidationReceiptProof {
                inline_proof_base64: None,
                mode: None,
                program_hash: None,
                proof_reference: None,
                system: RadrootsValidationReceiptProofSystem::None,
                verifying_key_hash: None,
            },
            public_values_hash: validation_receipt_public_values_hash_hex(
                br#"{"schema_version":1}"#,
            ),
            receipt_type: RadrootsValidationReceiptType::TradeTransition,
            result: RadrootsValidationReceiptResult::Valid,
            statement: RadrootsValidationReceiptStatement {
                listing_event_id: event_id('0'),
                root_event_id: event_id('1'),
                target_event_id: event_id('2'),
                validator_set_addr: validator_set_addr(),
                validator_set_event_id: event_id('8'),
                statement_type: RadrootsValidationReceiptType::TradeTransition,
            },
            version: 1,
        }
    }

    fn sample_sp1_reference_receipt() -> RadrootsTradeValidationReceipt {
        let mut receipt = sample_validation_receipt();
        receipt.proof = RadrootsValidationReceiptProof {
            inline_proof_base64: None,
            mode: Some("core".to_string()),
            program_hash: Some(hash32('a')),
            proof_reference: Some(format!("radroots-proof://sha256/{}", "1".repeat(64))),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: Some(hash32('b')),
        };
        receipt
    }

    fn sample_validation_receipt_event() -> RadrootsEventEnvelope {
        let receipt = sample_validation_receipt();
        let parts = validation_receipt_event_build("order-1", &receipt).expect("event parts");
        validation_receipt_event_with_parts(parts.kind, parts.tags, parts.content)
    }

    fn validation_receipt_event_with_parts(
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: event_id('9'),
            author: event_id('a'),
            created_at: 1,
            kind,
            tags,
            content,
            sig: "f".repeat(128),
        })
        .expect("receipt event")
    }

    fn validation_receipt_event_with_tags(tags: Vec<Vec<String>>) -> RadrootsEventEnvelope {
        let receipt = sample_validation_receipt();
        let parts = validation_receipt_event_build("order-1", &receipt).expect("event parts");
        validation_receipt_event_with_parts(parts.kind, tags, parts.content)
    }

    fn validator_set_event_with_parts(
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: event_id('7'),
            author: validator_set_author().to_hex(),
            created_at: 1_700_000_001,
            kind,
            tags,
            content,
            sig: "f".repeat(128),
        })
        .expect("validator set event")
    }

    #[test]
    fn validation_receipt_labels_cover_all_variants() {
        assert_eq!(
            RadrootsValidationReceiptType::ListingValidation.as_str(),
            "listing_validation"
        );
        assert_eq!(
            RadrootsValidationReceiptType::TradeTransition.as_str(),
            "trade_transition"
        );
        assert_eq!(
            RadrootsValidationReceiptType::InventoryState.as_str(),
            "inventory_state"
        );
        assert_eq!(
            RadrootsValidationReceiptType::StateCheckpoint.as_str(),
            "state_checkpoint"
        );
        assert_eq!(
            RadrootsValidationReceiptType::from_label("listing_validation"),
            Some(RadrootsValidationReceiptType::ListingValidation)
        );
        assert_eq!(
            RadrootsValidationReceiptType::from_label("trade_transition"),
            Some(RadrootsValidationReceiptType::TradeTransition)
        );
        assert_eq!(
            RadrootsValidationReceiptType::from_label("inventory_state"),
            Some(RadrootsValidationReceiptType::InventoryState)
        );
        assert_eq!(
            RadrootsValidationReceiptType::from_label("state_checkpoint"),
            Some(RadrootsValidationReceiptType::StateCheckpoint)
        );
        assert_eq!(RadrootsValidationReceiptType::from_label("unknown"), None);

        assert_eq!(RadrootsValidationReceiptProofSystem::None.as_str(), "none");
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Core.as_str(),
            "sp1_core"
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Compressed.as_str(),
            "sp1_compressed"
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Groth16.as_str(),
            "sp1_groth16"
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Plonk.as_str(),
            "sp1_plonk"
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("none"),
            Some(RadrootsValidationReceiptProofSystem::None)
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("sp1_core"),
            Some(RadrootsValidationReceiptProofSystem::Sp1Core)
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("sp1_compressed"),
            Some(RadrootsValidationReceiptProofSystem::Sp1Compressed)
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("sp1_groth16"),
            Some(RadrootsValidationReceiptProofSystem::Sp1Groth16)
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("sp1_plonk"),
            Some(RadrootsValidationReceiptProofSystem::Sp1Plonk)
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::from_label("unknown"),
            None
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::None.expected_mode(),
            None
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Core.expected_mode(),
            Some("core")
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Compressed.expected_mode(),
            Some("compressed")
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Groth16.expected_mode(),
            Some("groth16")
        );
        assert_eq!(
            RadrootsValidationReceiptProofSystem::Sp1Plonk.expected_mode(),
            Some("plonk")
        );
    }

    #[test]
    fn validation_trust_policy_builders_preserve_explicit_settings() {
        let trusted = validator_set_pubkey();
        let other = PublicKey::from_hex(FIXTURE_BOB_PUBLIC_KEY_HEX).expect("other validator");
        let policy = RadrootsTradeValidationTrustPolicy::production()
            .with_validator_set(sample_validator_set(), validator_set_addr(), event_id('8'))
            .with_require_cryptographic_proof(false);

        assert_eq!(policy.validator_count(), 1);
        assert!(policy.has_validator_set());
        assert!(policy.trusts_validator_pubkey(&trusted));
        assert!(!policy.trusts_validator_pubkey(&other));
        assert!(!policy.require_cryptographic_proof);
        assert!(!RadrootsTradeValidationTrustPolicy::default().require_cryptographic_proof);
        assert!(
            !RadrootsTradeValidationTrustPolicy::explicit_dev_test().require_cryptographic_proof
        );
    }

    #[test]
    fn validator_set_round_trips_canonical_payload_and_address() {
        let validator_set = sample_validator_set();
        let content =
            validator_set_canonical_content(&validator_set).expect("validator set content");
        assert_eq!(
            content,
            format!(
                "{{\"set_id\":\"{}\",\"validator_pubkey\":\"{}\",\"threshold\":1,\"valid_from\":1700000000,\"valid_until\":1800000000,\"protocol_contract_hash\":\"{}\",\"operator_name\":\"Radroots validation operator\",\"operator_contact\":\"validator@example.invalid\"}}",
                validator_set_id(),
                validator_set_pubkey(),
                hash32('7'),
            )
        );
        let parts = validator_set_event_build(&validator_set).expect("validator set parts");
        assert_eq!(parts.kind, KIND_VALIDATOR_SET);
        assert_eq!(
            parts.tags,
            vec![vec![TAG_D.to_string(), validator_set_id()]]
        );

        let event = RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: event_id('7'),
            author: validator_set_author().to_hex(),
            created_at: 1_700_000_001,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig: "f".repeat(128),
        })
        .expect("validator set event");

        let verified = validator_set_from_event(&event).expect("verified validator set");
        assert_eq!(verified.set, validator_set);
        assert_eq!(verified.event_id, event_id('7'));
        assert_eq!(verified.address, validator_set_addr());
        assert_eq!(verified.authority_pubkey, validator_set_author());
        verify_validator_set_event(&event, Some(&validator_set_author()))
            .expect("expected authority");
        assert_eq!(
            verify_validator_set_event(&event, Some(&validator_set_pubkey())),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "validator_set.author"
            ))
        );
    }

    #[test]
    fn validator_set_parsing_and_event_verification_reject_each_boundary() {
        let validator_set = sample_validator_set();
        let canonical = validator_set_canonical_content(&validator_set).expect("canonical content");
        let pretty = serde_json::to_string_pretty(&validator_set).expect("pretty content");
        assert_eq!(
            validator_set_content_from_str(&pretty),
            Err(RadrootsValidationReceiptError::NonCanonicalJson)
        );
        assert_eq!(
            validator_set_content_from_str("{"),
            Err(RadrootsValidationReceiptError::InvalidJson)
        );

        let parts = validator_set_event_build(&validator_set).expect("validator set parts");
        let wrong_kind = validator_set_event_with_parts(
            KIND_TRADE_VALIDATION_RECEIPT,
            parts.tags.clone(),
            canonical.clone(),
        );
        assert_eq!(
            verify_validator_set_event(&wrong_kind, None),
            Err(RadrootsValidationReceiptError::InvalidKind {
                expected: KIND_VALIDATOR_SET,
                got: KIND_TRADE_VALIDATION_RECEIPT,
            })
        );

        let mut mismatched_tags = parts.tags;
        mismatched_tags[0][1] = "018f3d99-7d35-7c0c-8a0f-7f3b645abcdf".to_string();
        let mismatched =
            validator_set_event_with_parts(KIND_VALIDATOR_SET, mismatched_tags, canonical);
        assert_eq!(
            verify_validator_set_event(&mismatched, None),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "validator_set.set_id"
            ))
        );

        let mut invalid = sample_validator_set();
        invalid.threshold = 2;
        assert_eq!(
            validator_set_event_build(&invalid),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.threshold"
            ))
        );
    }

    #[test]
    fn validation_receipt_validate_rejects_core_field_errors() {
        let mut receipt = sample_validation_receipt();
        receipt.version = 2;
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("version"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.domain = "other.domain".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("domain"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.statement_type = RadrootsValidationReceiptType::ListingValidation;
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.type"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.changed_records_root = "0x1".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "changed_records_root"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.event_set_root = format!("zz{}", "1".repeat(64));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "event_set_root"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.public_values_hash = format!("0x{}", "A".repeat(64));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "public_values_hash"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.error_bitmap = "0x1".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.error_bitmap = format!("zz{}", "0".repeat(32));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.error_bitmap = format!("0x{}", "A".repeat(32));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.listing_event_id = "bad".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.listing_event_id"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.root_event_id = "g".repeat(64);
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.root_event_id"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.new_state_root = "bad".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "new_state_root"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.previous_state_root = "bad".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "previous_state_root"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.target_event_id = "bad".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.target_event_id"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.validator_set_event_id = "bad".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.validator_set_event_id"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.statement.validator_set_addr = RadrootsAddressableCoordinate::parse(format!(
            "1:{}:{}",
            validator_set_author(),
            validator_set_id()
        ))
        .expect("typed non-validator address");
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "statement.validator_set_addr"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.error_bitmap = "0x00000000000000000000000000000001".to_string();
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.result = RadrootsValidationReceiptResult::Invalid;
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidField("error_bitmap"))
        );

        let mut receipt = sample_validation_receipt();
        receipt.result = RadrootsValidationReceiptResult::Invalid;
        receipt.error_bitmap = "0x00000000000000000000000000000001".to_string();
        receipt
            .validate()
            .expect("invalid result with nonzero bitmap");
    }

    #[test]
    fn validation_receipt_proof_validation_covers_identity_modes_and_material_errors() {
        let mut receipt = sample_validation_receipt();
        receipt.proof.mode = Some("core".to_string());
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.system"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.proof.program_hash = Some(hash32('a'));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.system"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.proof.proof_reference = Some(format!("radroots-proof://sha256/{}", "1".repeat(64)));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.system"
            ))
        );

        let mut receipt = sample_validation_receipt();
        receipt.proof.verifying_key_hash = Some(hash32('b'));
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.system"
            ))
        );

        let mut missing_program = sample_sp1_reference_receipt();
        missing_program.proof.program_hash = None;
        assert_eq!(
            missing_program.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.program_hash"
            ))
        );

        let mut missing_verifying_key = sample_sp1_reference_receipt();
        missing_verifying_key.proof.verifying_key_hash = None;
        assert_eq!(
            missing_verifying_key.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.verifying_key_hash"
            ))
        );

        let mut wrong_mode = sample_sp1_reference_receipt();
        wrong_mode.proof.mode = Some("compressed".to_string());
        assert_eq!(
            wrong_mode.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.mode"
            ))
        );

        let mut empty_reference = sample_sp1_reference_receipt();
        empty_reference.proof.proof_reference = Some(" ".to_string());
        assert_eq!(
            empty_reference.validate(),
            Err(RadrootsValidationReceiptError::EmptyField(
                "proof.proof_reference"
            ))
        );

        let mut compressed = sample_sp1_reference_receipt();
        compressed.proof.system = RadrootsValidationReceiptProofSystem::Sp1Compressed;
        compressed.proof.mode = Some("compressed".to_string());
        compressed.validate().expect("compressed proof metadata");

        let mut groth16 = sample_sp1_reference_receipt();
        groth16.proof.system = RadrootsValidationReceiptProofSystem::Sp1Groth16;
        groth16.proof.mode = Some("groth16".to_string());
        groth16.validate().expect("groth16 proof metadata");

        let mut plonk = sample_sp1_reference_receipt();
        plonk.proof.system = RadrootsValidationReceiptProofSystem::Sp1Plonk;
        plonk.proof.mode = Some("plonk".to_string());
        plonk.validate().expect("plonk proof metadata");
    }

    #[test]
    fn validation_receipt_tag_parser_rejects_invalid_shapes_and_labels() {
        let tags = validation_receipt_tags("order-1", &sample_validation_receipt()).unwrap();

        let mut duplicate_order = tags.clone();
        duplicate_order.push(vec![TAG_D.to_string(), "other-order".to_string()]);
        assert_eq!(
            validation_receipt_tags_from_tags(&duplicate_order),
            Err(RadrootsValidationReceiptError::InvalidTag(TAG_D))
        );

        let mut malformed_order = tags.clone();
        malformed_order[0] = vec![TAG_D.to_string()];
        assert_eq!(
            validation_receipt_tags_from_tags(&malformed_order),
            Err(RadrootsValidationReceiptError::InvalidTag(TAG_D))
        );

        let mut empty_order = tags.clone();
        empty_order[0][1] = " ".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&empty_order),
            Err(RadrootsValidationReceiptError::EmptyField(TAG_D))
        );

        let mut duplicate_listing = tags.clone();
        duplicate_listing.push(vec![
            "e".to_string(),
            event_id('3'),
            String::new(),
            String::new(),
            "listing".to_string(),
        ]);
        assert_eq!(
            validation_receipt_tags_from_tags(&duplicate_listing),
            Err(RadrootsValidationReceiptError::InvalidTag("listing"))
        );

        let mut empty_listing = tags.clone();
        empty_listing[1][1] = " ".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&empty_listing),
            Err(RadrootsValidationReceiptError::EmptyField("listing"))
        );

        let mut invalid_listing = tags.clone();
        invalid_listing[1][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_listing),
            Err(RadrootsValidationReceiptError::InvalidField(
                "tags.e.listing"
            ))
        );

        let mut invalid_root = tags.clone();
        invalid_root[2][1] = "g".repeat(64);
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_root),
            Err(RadrootsValidationReceiptError::InvalidField("tags.e.root"))
        );

        let mut invalid_target = tags.clone();
        invalid_target[3][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_target),
            Err(RadrootsValidationReceiptError::InvalidField(
                "tags.e.target"
            ))
        );

        let mut invalid_validator_set_addr = tags.clone();
        invalid_validator_set_addr[4][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_validator_set_addr),
            Err(RadrootsValidationReceiptError::InvalidTag(
                TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER
            ))
        );

        let mut duplicate_validator_set_addr = tags.clone();
        duplicate_validator_set_addr.push(tags[4].clone());
        assert_eq!(
            validation_receipt_tags_from_tags(&duplicate_validator_set_addr),
            Err(RadrootsValidationReceiptError::InvalidTag(
                TAG_VALIDATION_RECEIPT_VALIDATOR_SET_MARKER
            ))
        );

        let mut invalid_validator_set_event = tags.clone();
        invalid_validator_set_event[5][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_validator_set_event),
            Err(RadrootsValidationReceiptError::InvalidField(
                "tags.e.validator_set"
            ))
        );

        let mut invalid_event_set = tags.clone();
        invalid_event_set[6][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_event_set),
            Err(RadrootsValidationReceiptError::InvalidField(
                TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT
            ))
        );

        let mut invalid_reducer = tags.clone();
        invalid_reducer[7][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_reducer),
            Err(RadrootsValidationReceiptError::InvalidField(
                TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT
            ))
        );

        let mut invalid_public_values = tags.clone();
        invalid_public_values[8][1] = "bad".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_public_values),
            Err(RadrootsValidationReceiptError::InvalidField(
                TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH
            ))
        );

        let mut invalid_proof_system = tags.clone();
        invalid_proof_system[9][1] = "sp1_unknown".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_proof_system),
            Err(RadrootsValidationReceiptError::InvalidTag(
                TAG_VALIDATION_RECEIPT_PROOF_SYSTEM
            ))
        );

        let mut invalid_receipt_type = tags.clone();
        invalid_receipt_type[10][1] = "unknown".to_string();
        assert_eq!(
            validation_receipt_tags_from_tags(&invalid_receipt_type),
            Err(RadrootsValidationReceiptError::InvalidTag(
                TAG_VALIDATION_RECEIPT_RECEIPT_TYPE
            ))
        );

        for marker in ["root", "target"] {
            let missing = tags
                .iter()
                .filter(|tag| tag.get(4).map(String::as_str) != Some(marker))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(
                validation_receipt_tags_from_tags(&missing),
                Err(RadrootsValidationReceiptError::MissingTag(marker))
            );
        }

        let mut malformed_root = tags.clone();
        malformed_root[2] = vec![
            "e".to_string(),
            String::new(),
            String::new(),
            "root".to_string(),
        ];
        assert_eq!(
            validation_receipt_tags_from_tags(&malformed_root),
            Err(RadrootsValidationReceiptError::MissingTag("root"))
        );

        let mut missing_root_value = tags.clone();
        missing_root_value[2] = vec![
            "e".to_string(),
            String::new(),
            String::new(),
            "root".to_string(),
        ];
        missing_root_value[2].insert(1, event_id('1'));
        missing_root_value[2].remove(1);
        assert_eq!(
            validation_receipt_tags_from_tags(&missing_root_value),
            Err(RadrootsValidationReceiptError::MissingTag("root"))
        );

        let mut malformed_target = tags.clone();
        malformed_target[3] = vec![
            "e".to_string(),
            String::new(),
            String::new(),
            "target".to_string(),
        ];
        assert_eq!(
            validation_receipt_tags_from_tags(&malformed_target),
            Err(RadrootsValidationReceiptError::MissingTag("target"))
        );
    }

    #[test]
    fn validation_receipt_verifier_rejects_each_tag_mismatch() {
        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[1][1] = event_id('3');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "listing_event_id"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[2][1] = event_id('3');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch("root_event_id"))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[3][1] = event_id('3');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "target_event_id"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[4][1] = format!(
            "{}:{}:{}",
            KIND_VALIDATOR_SET,
            event_id('a'),
            validator_set_id()
        );
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "validator_set_addr"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[5][1] = event_id('3');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "validator_set_event_id"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[6][1] = hash32('d');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "event_set_root"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[7][1] = hash32('d');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "reducer_output_root"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[8][1] = hash32('d');
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "public_values_hash"
            ))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[9][1] = "sp1_core".to_string();
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch("proof_system"))
        );

        let mut tags = sample_validation_receipt_event().tags_as_vec();
        tags[10][1] = "listing_validation".to_string();
        let event = validation_receipt_event_with_tags(tags);
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::TagMismatch("receipt_type"))
        );
    }

    #[test]
    fn validation_receipt_expected_binding_checks_all_supported_fields() {
        let event = sample_validation_receipt_event();
        let validator_set_addr = validator_set_addr();
        let validator_set_addr_raw = validator_set_addr.as_str().to_string();
        verify_validation_receipt_event(
            &event,
            RadrootsValidationReceiptExpectedBinding {
                event_set_root: Some(&hash32('c')),
                listing_event_id: Some(&event_id('0')),
                order_id: Some("order-1"),
                proof_system: Some(RadrootsValidationReceiptProofSystem::None),
                public_values_hash: Some(&validation_receipt_public_values_hash_hex(
                    br#"{"schema_version":1}"#,
                )),
                reducer_output_root: Some(&hash32('4')),
                root_event_id: Some(&event_id('1')),
                target_event_id: Some(&event_id('2')),
                validator_set_addr: Some(validator_set_addr_raw.as_str()),
                validator_set_event_id: Some(&event_id('8')),
                ..RadrootsValidationReceiptExpectedBinding::default()
            },
        )
        .expect("matching expected binding");

        let wrong_validator_set_addr = format!(
            "{}:{}:{}",
            KIND_VALIDATOR_SET,
            event_id('a'),
            validator_set_id()
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    listing_event_id: Some(&event_id('3')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "listing_event_id"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    root_event_id: Some(&event_id('3')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "root_event_id"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    target_event_id: Some(&event_id('3')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "target_event_id"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    validator_set_addr: Some(wrong_validator_set_addr.as_str()),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "validator_set_addr"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    validator_set_event_id: Some(&event_id('3')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "validator_set_event_id"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    event_set_root: Some(&hash32('d')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "event_set_root"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    reducer_output_root: Some(&hash32('d')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "reducer_output_root"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    public_values_hash: Some(&hash32('d')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "public_values_hash"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    proof_system: Some(RadrootsValidationReceiptProofSystem::Sp1Core),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "proof_system"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    verifying_key_hash: Some(&hash32('b')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "verifying_key_hash"
            ))
        );
    }

    #[test]
    fn validation_receipt_round_trips_canonical_payload_and_tags() {
        let receipt = sample_validation_receipt();
        let content = validation_receipt_canonical_content(&receipt).expect("canonical content");
        assert_eq!(
            content,
            format!(
                "{{\"changed_records_root\":\"{}\",\"domain\":\"radroots.receipt\",\"error_bitmap\":\"0x00000000000000000000000000000000\",\"event_set_root\":\"{}\",\"new_state_root\":\"{}\",\"previous_state_root\":\"{}\",\"proof\":{{\"inline_proof_base64\":null,\"mode\":null,\"program_hash\":null,\"proof_reference\":null,\"system\":\"none\",\"verifying_key_hash\":null}},\"public_values_hash\":\"{}\",\"receipt_type\":\"trade_transition\",\"result\":\"valid\",\"statement\":{{\"listing_event_id\":\"{}\",\"root_event_id\":\"{}\",\"target_event_id\":\"{}\",\"validator_set_addr\":\"{}\",\"validator_set_event_id\":\"{}\",\"type\":\"trade_transition\"}},\"version\":1}}",
                hash32('6'),
                hash32('c'),
                hash32('4'),
                hash32('3'),
                receipt.public_values_hash,
                event_id('0'),
                event_id('1'),
                event_id('2'),
                validator_set_addr().as_str(),
                event_id('8'),
            )
        );
        assert_eq!(
            validation_receipt_content_from_str(&content).expect("parsed content"),
            receipt
        );

        let event = sample_validation_receipt_event();
        assert_eq!(event.kind_u32(), KIND_TRADE_VALIDATION_RECEIPT);
        let verified = validation_receipt_from_event(&event).expect("verified receipt");
        assert_eq!(verified.tags.order_id, "order-1");
        assert_eq!(verified.tags.listing_event_id, event_id('0'));
        assert_eq!(verified.tags.validator_set_addr, validator_set_addr());
        assert_eq!(verified.tags.validator_set_event_id, event_id('8'));
        assert_eq!(verified.tags.event_set_root, hash32('c'));
        assert_eq!(verified.tags.reducer_output_root, hash32('4'));
        assert_eq!(
            verified.tags.proof_system,
            RadrootsValidationReceiptProofSystem::None
        );
    }

    #[test]
    fn validation_authority_contract_uses_stable_snake_case_labels() {
        for (authority, label) in [
            (
                RadrootsTradeValidationAuthority::ValidatorSetDeterministic,
                "validator_set_deterministic",
            ),
            (
                RadrootsTradeValidationAuthority::CryptographicProofVerified,
                "cryptographic_proof_verified",
            ),
            (
                RadrootsTradeValidationAuthority::ValidatorSetAndProofVerified,
                "validator_set_and_proof_verified",
            ),
        ] {
            assert_eq!(authority.as_str(), label);
            assert_eq!(
                RadrootsTradeValidationAuthority::from_label(label),
                Some(authority)
            );
            assert_eq!(
                serde_json::to_string(&authority).expect("serialize authority"),
                format!("\"{label}\"")
            );
            assert_eq!(
                serde_json::from_str::<RadrootsTradeValidationAuthority>(&format!("\"{label}\""))
                    .expect("deserialize authority"),
                authority
            );
        }
        assert_eq!(RadrootsTradeValidationAuthority::from_label("legacy"), None);
    }

    #[test]
    fn commitment_confidence_contract_uses_stable_snake_case_labels() {
        for (confidence, label) in [
            (
                RadrootsTradeCommitmentConfidence::AwaitingValidation,
                "awaiting_validation",
            ),
            (
                RadrootsTradeCommitmentConfidence::CommittedByValidatorSet,
                "committed_by_validator_set",
            ),
            (
                RadrootsTradeCommitmentConfidence::CommittedByCryptographicProof,
                "committed_by_cryptographic_proof",
            ),
            (
                RadrootsTradeCommitmentConfidence::CommittedByValidatorSetAndProof,
                "committed_by_validator_set_and_proof",
            ),
            (RadrootsTradeCommitmentConfidence::Invalid, "invalid"),
        ] {
            assert_eq!(confidence.as_str(), label);
            assert_eq!(
                RadrootsTradeCommitmentConfidence::from_label(label),
                Some(confidence)
            );
            assert_eq!(
                serde_json::to_string(&confidence).expect("serialize confidence"),
                format!("\"{label}\"")
            );
            assert_eq!(
                serde_json::from_str::<RadrootsTradeCommitmentConfidence>(&format!("\"{label}\""))
                    .expect("deserialize confidence"),
                confidence
            );
        }
        assert_eq!(
            RadrootsTradeCommitmentConfidence::from_label("legacy"),
            None
        );
    }

    #[test]
    fn validation_trust_state_contract_uses_stable_snake_case_labels() {
        for (state, label) in [
            (RadrootsTradeValidationTrustState::Pending, "pending"),
            (RadrootsTradeValidationTrustState::Untrusted, "untrusted"),
            (
                RadrootsTradeValidationTrustState::ValidatorSetCommitted,
                "validator_set_committed",
            ),
            (
                RadrootsTradeValidationTrustState::CryptographicCommitted,
                "cryptographic_committed",
            ),
            (RadrootsTradeValidationTrustState::Invalid, "invalid"),
        ] {
            assert_eq!(state.as_str(), label);
            assert_eq!(
                RadrootsTradeValidationTrustState::from_label(label),
                Some(state)
            );
            assert_eq!(
                serde_json::to_string(&state).expect("serialize trust state"),
                format!("\"{label}\"")
            );
            assert_eq!(
                serde_json::from_str::<RadrootsTradeValidationTrustState>(&format!("\"{label}\""))
                    .expect("deserialize trust state"),
                state
            );
        }
        assert_eq!(
            RadrootsTradeValidationTrustState::from_label("legacy"),
            None
        );
    }

    #[test]
    fn validation_trust_policy_defaults_to_empty_production_trust() {
        let production = RadrootsTradeValidationTrustPolicy::default();
        assert!(production.validator_set.is_none());
        assert!(production.validator_set_addr.is_none());
        assert!(production.validator_set_event_id.is_none());
        assert!(!production.has_validator_set());
        assert!(!production.require_cryptographic_proof);
        assert_eq!(production.validator_count(), 0);

        let dev_test = RadrootsTradeValidationTrustPolicy::explicit_dev_test();
        assert!(!dev_test.has_validator_set());
        assert!(!dev_test.require_cryptographic_proof);
    }

    #[test]
    fn validation_receipt_public_values_hash_uses_domain_separator() {
        assert_ne!(
            validation_receipt_public_values_hash_hex(br#"{"schema_version":1}"#),
            validation_receipt_public_values_hash_hex(br#"{"schema_version":2}"#)
        );
        assert_eq!(
            validation_receipt_public_values_hash_hex(br#"{"schema_version":1}"#),
            "0x0db3f9b2dbde90b932ea992c18bca5e4563b741258ed911c3c36fbbeeea88015"
        );
    }

    #[test]
    fn validation_receipt_verifier_rejects_non_validation_receipt_kind() {
        let sample = sample_validation_receipt_event();
        let event = validation_receipt_event_with_parts(
            3434,
            sample.tags_as_vec(),
            sample.content().to_owned(),
        );
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::InvalidKind {
                expected: KIND_TRADE_VALIDATION_RECEIPT,
                got: 3434
            })
        );
    }

    #[test]
    fn validation_receipt_verifier_rejects_missing_and_wrong_bindings() {
        let event = sample_validation_receipt_event();
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    order_id: Some("other-order"),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "order_id"
            ))
        );

        let mut tags = event.tags_as_vec();
        tags.retain(|tag| tag.first().map(|value| value.as_str()) != Some("event_set_root"));
        let missing_event_set =
            validation_receipt_event_with_parts(event.kind_u32(), tags, event.content().to_owned());
        assert_eq!(
            validation_receipt_from_event(&missing_event_set),
            Err(RadrootsValidationReceiptError::MissingTag("event_set_root"))
        );

        let mut tags = event.tags_as_vec();
        let reducer_tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(|value| value.as_str()) == Some("reducer_output_root"))
            .expect("reducer output tag");
        reducer_tag[1] = hash32('8');
        let wrong_reducer_output =
            validation_receipt_event_with_parts(event.kind_u32(), tags, event.content().to_owned());
        assert_eq!(
            validation_receipt_from_event(&wrong_reducer_output),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "reducer_output_root"
            ))
        );

        let mut tags = event.tags_as_vec();
        let public_values_tag = tags
            .iter_mut()
            .find(|tag| tag.first().map(|value| value.as_str()) == Some("public_values_hash"))
            .expect("public values tag");
        public_values_tag[1] = hash32('b');
        let wrong_public_values =
            validation_receipt_event_with_parts(event.kind_u32(), tags, event.content().to_owned());
        assert_eq!(
            validation_receipt_from_event(&wrong_public_values),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "public_values_hash"
            ))
        );
    }

    #[test]
    fn validation_receipt_rejects_mismatched_proof_system_metadata() {
        let mut receipt = sample_validation_receipt();
        receipt.proof = RadrootsValidationReceiptProof {
            inline_proof_base64: None,
            mode: Some("compressed".to_string()),
            program_hash: Some(hash32('a')),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Compressed,
            verifying_key_hash: Some(hash32('b')),
        };
        assert_eq!(
            receipt.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.material_missing"
            ))
        );

        receipt.proof.proof_reference = Some(format!("radroots-proof://sha256/{}", "1".repeat(64)));
        let parts = validation_receipt_event_build("order-1", &receipt).expect("sp1 event parts");
        let event = validation_receipt_event_with_parts(parts.kind, parts.tags, parts.content);
        let verified = verify_validation_receipt_event(
            &event,
            RadrootsValidationReceiptExpectedBinding {
                proof_system: Some(RadrootsValidationReceiptProofSystem::Sp1Compressed),
                ..RadrootsValidationReceiptExpectedBinding::default()
            },
        )
        .expect("sp1 receipt verifies with proof reference");
        assert_eq!(
            verified.receipt.proof.system,
            RadrootsValidationReceiptProofSystem::Sp1Compressed
        );
    }

    #[test]
    fn validation_receipt_enforces_none_and_sp1_material_rules() {
        let mut none_with_material = sample_validation_receipt();
        none_with_material.proof.inline_proof_base64 = Some("cHJvb2Y=".to_string());
        assert_eq!(
            none_with_material.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.system"
            ))
        );

        let mut both_material_sources = sample_sp1_reference_receipt();
        both_material_sources.proof.inline_proof_base64 = Some("cHJvb2Y=".to_string());
        assert_eq!(
            both_material_sources.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.material_conflict"
            ))
        );

        let mut missing_material = sample_sp1_reference_receipt();
        missing_material.proof.proof_reference = None;
        assert_eq!(
            missing_material.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.material_missing"
            ))
        );
    }

    #[test]
    fn validation_receipt_rejects_invalid_sp1_material_shape() {
        let mut invalid_inline = sample_sp1_reference_receipt();
        invalid_inline.proof.proof_reference = None;
        invalid_inline.proof.inline_proof_base64 = Some("not canonical base64".to_string());
        assert_eq!(
            invalid_inline.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.inline_proof_base64"
            ))
        );

        invalid_inline.proof.inline_proof_base64 = Some("cHJvb2Y=".to_string());
        invalid_inline.validate().expect("valid inline proof shape");

        invalid_inline.proof.inline_proof_base64 = Some("AA==".to_string());
        invalid_inline
            .validate()
            .expect("canonical zero byte inline proof shape");

        invalid_inline.proof.inline_proof_base64 = Some("AB==".to_string());
        assert_eq!(
            invalid_inline.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.inline_proof_base64"
            ))
        );

        invalid_inline.proof.inline_proof_base64 = Some(String::new());
        assert_eq!(
            invalid_inline.validate(),
            Err(RadrootsValidationReceiptError::EmptyField(
                "proof.inline_proof_base64"
            ))
        );

        let mut invalid_reference = sample_sp1_reference_receipt();
        invalid_reference.proof.proof_reference = Some("https://example.test/proof".to_string());
        assert_eq!(
            invalid_reference.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.proof_reference"
            ))
        );

        invalid_reference.proof.proof_reference = Some("radroots-proof://".to_string());
        assert_eq!(
            invalid_reference.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.proof_reference"
            ))
        );

        invalid_reference.proof.proof_reference =
            Some(format!("radroots-proof://sha256/{}", "A".repeat(64)));
        assert_eq!(
            invalid_reference.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.proof_reference"
            ))
        );

        invalid_reference.proof.proof_reference =
            Some(format!("radroots-proof://sha256/{}", "1".repeat(63)));
        assert_eq!(
            invalid_reference.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.proof_reference"
            ))
        );

        invalid_reference.proof.proof_reference =
            Some(format!("radroots-proof://sha256/{}/proof", "1".repeat(64)));
        assert_eq!(
            invalid_reference.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.proof_reference"
            ))
        );

        invalid_reference.proof.proof_reference =
            Some(format!("radroots-proof://sha256/{}", "1".repeat(64)));
        invalid_reference
            .validate()
            .expect("valid sha256 proof reference");
    }

    #[test]
    fn validation_receipt_expected_binding_enforces_sp1_identity() {
        let receipt = sample_sp1_reference_receipt();
        let parts = validation_receipt_event_build("order-1", &receipt).expect("sp1 event parts");
        let event = validation_receipt_event_with_parts(parts.kind, parts.tags, parts.content);

        verify_validation_receipt_event(
            &event,
            RadrootsValidationReceiptExpectedBinding {
                program_hash: Some(&hash32('a')),
                verifying_key_hash: Some(&hash32('b')),
                ..RadrootsValidationReceiptExpectedBinding::default()
            },
        )
        .expect("sp1 identity binding matches");

        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    program_hash: Some(&hash32('c')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "program_hash"
            ))
        );
        assert_eq!(
            verify_validation_receipt_event(
                &event,
                RadrootsValidationReceiptExpectedBinding {
                    verifying_key_hash: Some(&hash32('d')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "verifying_key_hash"
            ))
        );

        assert_eq!(
            verify_validation_receipt_event(
                &sample_validation_receipt_event(),
                RadrootsValidationReceiptExpectedBinding {
                    program_hash: Some(&hash32('a')),
                    ..RadrootsValidationReceiptExpectedBinding::default()
                },
            ),
            Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "program_hash"
            ))
        );
    }

    #[test]
    fn validation_receipt_rejects_malformed_canonical_json() {
        let receipt = sample_validation_receipt();
        let pretty = serde_json::to_string_pretty(&receipt).expect("pretty json");
        assert_eq!(
            validation_receipt_content_from_str(&pretty),
            Err(RadrootsValidationReceiptError::NonCanonicalJson)
        );

        let mut unknown_field = validation_receipt_canonical_content(&receipt).expect("canonical");
        unknown_field.insert_str(1, "\"extra\":true,");
        assert_eq!(
            validation_receipt_content_from_str(&unknown_field),
            Err(RadrootsValidationReceiptError::InvalidJson)
        );
    }

    #[test]
    fn validation_receipt_builders_reject_invalid_receipts_before_serializing() {
        let mut receipt = sample_validation_receipt();
        receipt.version = 2;
        let content = serde_json::to_string(&receipt).unwrap();

        assert_eq!(
            validation_receipt_canonical_content(&receipt),
            Err(RadrootsValidationReceiptError::InvalidField("version"))
        );
        assert_eq!(
            validation_receipt_content_from_str(&content),
            Err(RadrootsValidationReceiptError::InvalidField("version"))
        );
        assert_eq!(
            validation_receipt_tags("order-1", &receipt),
            Err(RadrootsValidationReceiptError::InvalidField("version"))
        );
        assert!(matches!(
            validation_receipt_event_build("order-1", &receipt),
            Err(RadrootsValidationReceiptError::InvalidField("version"))
        ));
    }

    #[test]
    fn validation_receipt_tag_builder_rejects_empty_order_id() {
        assert_eq!(
            validation_receipt_tags("", &sample_validation_receipt()),
            Err(RadrootsValidationReceiptError::EmptyField("order_id"))
        );
    }

    #[test]
    fn validator_set_validation_covers_every_boundary() {
        let valid_id = validator_set_id();
        for variant in ["8", "9", "a", "b"] {
            let mut value = valid_id.clone();
            value.replace_range(19..20, variant);
            assert_eq!(super::validate_uuidv7(&value, "uuid"), Ok(()));
        }
        let mut invalid_ids = vec![String::new(), "bad".to_string()];
        for (range, replacement) in [
            (8..9, "0"),
            (13..14, "0"),
            (18..19, "0"),
            (23..24, "0"),
            (14..15, "6"),
            (19..20, "7"),
            (0..1, "g"),
        ] {
            let mut value = valid_id.clone();
            value.replace_range(range, replacement);
            invalid_ids.push(value);
        }
        for invalid in invalid_ids {
            assert!(matches!(
                super::validate_uuidv7(&invalid, "uuid"),
                Err(RadrootsValidationReceiptError::EmptyField("uuid"))
                    | Err(RadrootsValidationReceiptError::InvalidField("uuid"))
            ));
        }

        let mut validator_set = sample_validator_set();
        validator_set.threshold = 2;
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.threshold"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.valid_until = validator_set.valid_from;
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.valid_until"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.protocol_contract_hash = "bad".to_string();
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.protocol_contract_hash"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.operator_name = " ".to_string();
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::EmptyField(
                "validator_set.operator_name"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.operator_name = "x".repeat(121);
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.operator_name"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.operator_contact = Some(" ".to_string());
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::EmptyField(
                "validator_set.operator_contact"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.operator_contact = Some("x".repeat(241));
        assert_eq!(
            validator_set.validate(),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.operator_contact"
            ))
        );
        let mut validator_set = sample_validator_set();
        validator_set.operator_contact = None;
        validator_set.validate().expect("contact is optional");

        let address = super::validator_set_address_from_str(validator_set_addr().as_str())
            .expect("validator set address");
        assert_eq!(address, validator_set_addr());
        assert_eq!(
            super::validator_set_address_from_str("bad"),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.address"
            ))
        );
        let wrong_kind = format!("1:{}:{}", validator_set_author(), validator_set_id());
        assert_eq!(
            super::validator_set_address_from_str(wrong_kind),
            Err(RadrootsValidationReceiptError::InvalidField(
                "validator_set.address"
            ))
        );

        let empty = RadrootsTradeValidationTrustPolicy::production();
        assert!(!empty.has_validator_set());
        assert!(!empty.trusts_validator_pubkey(&validator_set_pubkey()));
        assert_eq!(empty.validator_count(), 0);

        let partial = RadrootsTradeValidationTrustPolicy {
            validator_set: Some(sample_validator_set()),
            validator_set_addr: None,
            validator_set_event_id: Some(event_id('8')),
            require_cryptographic_proof: false,
        };
        assert!(!partial.has_validator_set());
    }
}
