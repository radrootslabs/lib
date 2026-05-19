#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_TRADE_RECEIPT, KIND_TRADE_VALIDATION_RECEIPT},
    tags::TAG_D,
};
use radroots_events_codec::wire::WireEventParts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const VALIDATION_RECEIPT_DOMAIN: &str = "radroots.receipt";
pub const VALIDATION_RECEIPT_VERSION: u32 = 1;
pub const VALIDATION_RECEIPT_PUBLIC_VALUES_HASH_DOMAIN: &[u8] = b"radroots:sp1-public-values:v1";
pub const VALIDATION_RECEIPT_PROOF_REFERENCE_SCHEME: &str = "radroots-proof://";
pub const TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT: &str = "event_set_root";
pub const TAG_VALIDATION_RECEIPT_PROOF_SYSTEM: &str = "proof_system";
pub const TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH: &str = "public_values_hash";
pub const TAG_VALIDATION_RECEIPT_RECEIPT_TYPE: &str = "receipt_type";
pub const TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT: &str = "reducer_output_root";

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
    pub root_event_id: String,
    pub target_event_id: String,
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
    pub order_id: String,
    pub proof_system: RadrootsValidationReceiptProofSystem,
    pub public_values_hash: String,
    pub receipt_type: RadrootsValidationReceiptType,
    pub reducer_output_root: String,
    pub root_event_id: String,
    pub target_event_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadrootsValidationReceiptExpectedBinding<'a> {
    pub event_set_root: Option<&'a str>,
    pub order_id: Option<&'a str>,
    pub program_hash: Option<&'a str>,
    pub proof_system: Option<RadrootsValidationReceiptProofSystem>,
    pub public_values_hash: Option<&'a str>,
    pub reducer_output_root: Option<&'a str>,
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
    #[error("buyer receipt kind 3434 is not a validation receipt")]
    BuyerReceiptKind,
    #[error("validation receipt kind 3440 is not a buyer receipt")]
    ValidationReceiptKind,
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
        validate_event_id(&self.statement.root_event_id, "statement.root_event_id")?;
        validate_event_id(&self.statement.target_event_id, "statement.target_event_id")?;
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
                    _ => {
                        return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                            "proof.material",
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
    let root_event_id = required_event_marker(tags, "root")?;
    let target_event_id = required_event_marker(tags, "target")?;
    let event_set_root = required_tag_value(tags, TAG_VALIDATION_RECEIPT_EVENT_SET_ROOT)?;
    let reducer_output_root = required_tag_value(tags, TAG_VALIDATION_RECEIPT_REDUCER_OUTPUT_ROOT)?;
    let public_values_hash = required_tag_value(tags, TAG_VALIDATION_RECEIPT_PUBLIC_VALUES_HASH)?;
    let proof_system = RadrootsValidationReceiptProofSystem::from_label(&required_tag_value(
        tags,
        TAG_VALIDATION_RECEIPT_PROOF_SYSTEM,
    )?)
    .ok_or(RadrootsValidationReceiptError::InvalidTag(
        TAG_VALIDATION_RECEIPT_PROOF_SYSTEM,
    ))?;
    let receipt_type = RadrootsValidationReceiptType::from_label(&required_tag_value(
        tags,
        TAG_VALIDATION_RECEIPT_RECEIPT_TYPE,
    )?)
    .ok_or(RadrootsValidationReceiptError::InvalidTag(
        TAG_VALIDATION_RECEIPT_RECEIPT_TYPE,
    ))?;

    validate_event_id(&root_event_id, "tags.e.root")?;
    validate_event_id(&target_event_id, "tags.e.target")?;
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
        order_id,
        proof_system,
        public_values_hash,
        receipt_type,
        reducer_output_root,
        root_event_id,
        target_event_id,
    })
}

pub fn validation_receipt_event_build(
    order_id: &str,
    receipt: &RadrootsTradeValidationReceipt,
) -> Result<WireEventParts, RadrootsValidationReceiptError> {
    Ok(WireEventParts {
        kind: KIND_TRADE_VALIDATION_RECEIPT,
        content: validation_receipt_canonical_content(receipt)?,
        tags: validation_receipt_tags(order_id, receipt)?,
    })
}

pub fn validation_receipt_from_event(
    event: &RadrootsNostrEvent,
) -> Result<RadrootsVerifiedValidationReceipt, RadrootsValidationReceiptError> {
    verify_validation_receipt_event(event, RadrootsValidationReceiptExpectedBinding::default())
}

pub fn verify_validation_receipt_event(
    event: &RadrootsNostrEvent,
    expected: RadrootsValidationReceiptExpectedBinding<'_>,
) -> Result<RadrootsVerifiedValidationReceipt, RadrootsValidationReceiptError> {
    if event.kind == KIND_TRADE_RECEIPT {
        return Err(RadrootsValidationReceiptError::BuyerReceiptKind);
    }
    if event.kind != KIND_TRADE_VALIDATION_RECEIPT {
        return Err(RadrootsValidationReceiptError::InvalidKind {
            expected: KIND_TRADE_VALIDATION_RECEIPT,
            got: event.kind,
        });
    }

    let receipt = validation_receipt_content_from_str(&event.content)?;
    let tags = validation_receipt_tags_from_tags(&event.tags)?;

    if tags.root_event_id != receipt.statement.root_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch("root_event_id"));
    }
    if tags.target_event_id != receipt.statement.target_event_id {
        return Err(RadrootsValidationReceiptError::TagMismatch(
            "target_event_id",
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

pub fn reject_validation_receipt_as_buyer_receipt(
    event: &RadrootsNostrEvent,
) -> Result<(), RadrootsValidationReceiptError> {
    if event.kind == KIND_TRADE_VALIDATION_RECEIPT {
        return Err(RadrootsValidationReceiptError::ValidationReceiptKind);
    }
    Ok(())
}

fn validate_expected_binding(
    tags: &RadrootsValidationReceiptTags,
    receipt: &RadrootsTradeValidationReceipt,
    expected: RadrootsValidationReceiptExpectedBinding<'_>,
) -> Result<(), RadrootsValidationReceiptError> {
    if let Some(order_id) = expected.order_id {
        if tags.order_id != order_id {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "order_id",
            ));
        }
    }
    if let Some(event_set_root) = expected.event_set_root {
        if tags.event_set_root != event_set_root {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "event_set_root",
            ));
        }
    }
    if let Some(reducer_output_root) = expected.reducer_output_root {
        if tags.reducer_output_root != reducer_output_root {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "reducer_output_root",
            ));
        }
    }
    if let Some(public_values_hash) = expected.public_values_hash {
        if tags.public_values_hash != public_values_hash {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "public_values_hash",
            ));
        }
    }
    if let Some(proof_system) = expected.proof_system {
        if tags.proof_system != proof_system {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "proof_system",
            ));
        }
    }
    if let Some(program_hash) = expected.program_hash {
        if receipt.proof.program_hash.as_deref() != Some(program_hash) {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "program_hash",
            ));
        }
    }
    if let Some(verifying_key_hash) = expected.verifying_key_hash {
        if receipt.proof.verifying_key_hash.as_deref() != Some(verifying_key_hash) {
            return Err(RadrootsValidationReceiptError::ExpectedBindingMismatch(
                "verifying_key_hash",
            ));
        }
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
    let value = tag
        .get(1)
        .ok_or(RadrootsValidationReceiptError::InvalidTag(marker))?;
    validate_required_str(value, marker)?;
    Ok(value.clone())
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

fn validate_inline_proof_base64(value: &str) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, "proof.inline_proof_base64")?;
    if value.len() % 4 != 0 {
        return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
            "proof.inline_proof_base64",
        ));
    }

    let bytes = value.as_bytes();
    let mut padding_started = false;
    let mut padding_count = 0usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' if !padding_started => {}
            b'=' => {
                padding_started = true;
                padding_count += 1;
                if padding_count > 2 || index < bytes.len().saturating_sub(2) {
                    return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                        "proof.inline_proof_base64",
                    ));
                }
            }
            _ => {
                return Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                    "proof.inline_proof_base64",
                ));
            }
        }
    }

    Ok(())
}

fn validate_proof_reference(value: &str) -> Result<(), RadrootsValidationReceiptError> {
    validate_required_str(value, "proof.proof_reference")?;
    let body = value
        .strip_prefix(VALIDATION_RECEIPT_PROOF_REFERENCE_SCHEME)
        .ok_or(RadrootsValidationReceiptError::InvalidProofMetadata(
            "proof.proof_reference",
        ))?;
    validate_required_str(body, "proof.proof_reference")
        .map_err(|_| RadrootsValidationReceiptError::InvalidProofMetadata("proof.proof_reference"))
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
        RadrootsTradeValidationReceipt, RadrootsValidationReceiptError,
        RadrootsValidationReceiptExpectedBinding, RadrootsValidationReceiptProof,
        RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
        RadrootsValidationReceiptStatement, RadrootsValidationReceiptType,
        reject_validation_receipt_as_buyer_receipt, validation_receipt_canonical_content,
        validation_receipt_content_from_str, validation_receipt_event_build,
        validation_receipt_from_event, validation_receipt_public_values_hash_hex,
        validation_receipt_tags, verify_validation_receipt_event,
    };
    use radroots_events::{
        RadrootsNostrEvent,
        kinds::{KIND_TRADE_RECEIPT, KIND_TRADE_VALIDATION_RECEIPT},
    };
    use radroots_events_codec::trade::active_trade_buyer_receipt_from_event;

    fn hash32(c: char) -> String {
        format!("0x{}", c.to_string().repeat(64))
    }

    fn event_id(c: char) -> String {
        c.to_string().repeat(64)
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
                root_event_id: event_id('1'),
                target_event_id: event_id('2'),
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
            proof_reference: Some("radroots-proof://proof-1".to_string()),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: Some(hash32('b')),
        };
        receipt
    }

    fn sample_validation_receipt_event() -> RadrootsNostrEvent {
        let receipt = sample_validation_receipt();
        let parts = validation_receipt_event_build("order-1", &receipt).expect("event parts");
        RadrootsNostrEvent {
            id: event_id('9'),
            author: event_id('a'),
            created_at: 1,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig: "signature".to_string(),
        }
    }

    #[test]
    fn validation_receipt_round_trips_canonical_payload_and_tags() {
        let receipt = sample_validation_receipt();
        let content = validation_receipt_canonical_content(&receipt).expect("canonical content");
        assert_eq!(
            content,
            format!(
                "{{\"changed_records_root\":\"{}\",\"domain\":\"radroots.receipt\",\"error_bitmap\":\"0x00000000000000000000000000000000\",\"event_set_root\":\"{}\",\"new_state_root\":\"{}\",\"previous_state_root\":\"{}\",\"proof\":{{\"inline_proof_base64\":null,\"mode\":null,\"program_hash\":null,\"proof_reference\":null,\"system\":\"none\",\"verifying_key_hash\":null}},\"public_values_hash\":\"{}\",\"receipt_type\":\"trade_transition\",\"result\":\"valid\",\"statement\":{{\"root_event_id\":\"{}\",\"target_event_id\":\"{}\",\"type\":\"trade_transition\"}},\"version\":1}}",
                hash32('6'),
                hash32('c'),
                hash32('4'),
                hash32('3'),
                receipt.public_values_hash,
                event_id('1'),
                event_id('2'),
            )
        );
        assert_eq!(
            validation_receipt_content_from_str(&content).expect("parsed content"),
            receipt
        );

        let event = sample_validation_receipt_event();
        assert_eq!(event.kind, KIND_TRADE_VALIDATION_RECEIPT);
        let verified = validation_receipt_from_event(&event).expect("verified receipt");
        assert_eq!(verified.tags.order_id, "order-1");
        assert_eq!(verified.tags.event_set_root, hash32('c'));
        assert_eq!(verified.tags.reducer_output_root, hash32('4'));
        assert_eq!(
            verified.tags.proof_system,
            RadrootsValidationReceiptProofSystem::None
        );
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
    fn validation_receipt_verifier_rejects_buyer_receipt_kind_3434() {
        let mut event = sample_validation_receipt_event();
        event.kind = KIND_TRADE_RECEIPT;
        assert_eq!(
            validation_receipt_from_event(&event),
            Err(RadrootsValidationReceiptError::BuyerReceiptKind)
        );
    }

    #[test]
    fn validation_receipt_kind_3440_is_rejected_as_buyer_receipt() {
        let event = sample_validation_receipt_event();
        assert_eq!(
            reject_validation_receipt_as_buyer_receipt(&event),
            Err(RadrootsValidationReceiptError::ValidationReceiptKind)
        );
        let buyer_receipt_error = active_trade_buyer_receipt_from_event(&event)
            .expect_err("validation receipt must not parse as buyer receipt");
        assert!(buyer_receipt_error.to_string().contains("3440"));
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

        let mut missing_event_set = event.clone();
        missing_event_set
            .tags
            .retain(|tag| tag.first().map(|value| value.as_str()) != Some("event_set_root"));
        assert_eq!(
            validation_receipt_from_event(&missing_event_set),
            Err(RadrootsValidationReceiptError::MissingTag("event_set_root"))
        );

        let mut wrong_reducer_output = event.clone();
        let reducer_tag = wrong_reducer_output
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(|value| value.as_str()) == Some("reducer_output_root"))
            .expect("reducer output tag");
        reducer_tag[1] = hash32('8');
        assert_eq!(
            validation_receipt_from_event(&wrong_reducer_output),
            Err(RadrootsValidationReceiptError::TagMismatch(
                "reducer_output_root"
            ))
        );

        let mut wrong_public_values = event.clone();
        let public_values_tag = wrong_public_values
            .tags
            .iter_mut()
            .find(|tag| tag.first().map(|value| value.as_str()) == Some("public_values_hash"))
            .expect("public values tag");
        public_values_tag[1] = hash32('b');
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
                "proof.material"
            ))
        );

        receipt.proof.proof_reference = Some("radroots-proof://proof-1".to_string());
        let parts = validation_receipt_event_build("order-1", &receipt).expect("sp1 event parts");
        let mut event = sample_validation_receipt_event();
        event.content = parts.content;
        event.tags = parts.tags;
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
                "proof.material"
            ))
        );

        let mut missing_material = sample_sp1_reference_receipt();
        missing_material.proof.proof_reference = None;
        assert_eq!(
            missing_material.validate(),
            Err(RadrootsValidationReceiptError::InvalidProofMetadata(
                "proof.material"
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
    }

    #[test]
    fn validation_receipt_expected_binding_enforces_sp1_identity() {
        let receipt = sample_sp1_reference_receipt();
        let parts = validation_receipt_event_build("order-1", &receipt).expect("sp1 event parts");
        let mut event = sample_validation_receipt_event();
        event.content = parts.content;
        event.tags = parts.tags;

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
    fn validation_receipt_tag_builder_rejects_empty_order_id() {
        assert_eq!(
            validation_receipt_tags("", &sample_validation_receipt()),
            Err(RadrootsValidationReceiptError::EmptyField("order_id"))
        );
    }
}
