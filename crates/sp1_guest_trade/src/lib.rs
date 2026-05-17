#![forbid(unsafe_code)]

use radroots_events::trade::{
    RadrootsTradeOrderDecision, RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderRequested,
};
use radroots_trade::validation_receipt::validation_receipt_public_values_hash_hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

pub const RADROOTS_SP1_TRADE_PUBLIC_VALUES_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_SP1_TRADE_PROTOCOL_VERSION: &str = "radroots.trade.v1";
pub const RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH: &str =
    "0x3d8f7f463904d71f2d0d14b1551450756697e51c7b658e10c6d5c20a7bc61f08";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProofStatementType {
    TradeTransition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProofTransitionKind {
    OrderAccepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProofResult {
    Valid,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeProofPublicValues {
    pub schema_version: u32,
    pub statement_type: RadrootsSp1TradeProofStatementType,
    pub radroots_protocol_version: String,
    pub reducer_program_hash: String,
    pub sp1_verifying_key_hash: Option<String>,
    pub event_set_root: String,
    pub listing_addr_hash: Option<String>,
    pub listing_event_id: Option<String>,
    pub order_id_hash: Option<String>,
    pub root_event_id: Option<String>,
    pub target_event_id: Option<String>,
    pub previous_state_root: String,
    pub new_state_root: String,
    pub transition: Option<RadrootsSp1TradeProofTransitionKind>,
    pub result: RadrootsSp1TradeProofResult,
    pub error_bitmap: String,
    pub inventory_delta_root: Option<String>,
    pub inventory_sequence: Option<u128>,
    pub inventory_prev_root: Option<String>,
    pub inventory_new_root: Option<String>,
    pub changed_records_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeInventoryBinWitness {
    pub bin_id: String,
    pub listing_capacity: u64,
    pub previous_reserved: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeOrderAcceptanceWitness {
    pub listing_event_id: String,
    pub request_event_id: String,
    pub decision_event_id: String,
    pub request: RadrootsTradeOrderRequested,
    pub decision: RadrootsTradeOrderDecisionEvent,
    pub inventory_bins: Vec<RadrootsSp1TradeInventoryBinWitness>,
    pub inventory_sequence: u128,
    pub previous_state_root: Option<String>,
    pub reducer_program_hash: String,
    pub radroots_protocol_version: String,
    pub sp1_verifying_key_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradePublicValuesExecution {
    pub public_values: RadrootsSp1TradeProofPublicValues,
    pub canonical_public_values: Vec<u8>,
    pub public_values_hash: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsSp1TradeGuestError {
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    #[error("invalid event id field {0}")]
    InvalidEventId(&'static str),
    #[error("invalid hash field {0}")]
    InvalidHash(&'static str),
    #[error("invalid order request")]
    InvalidOrderRequest,
    #[error("invalid order decision")]
    InvalidOrderDecision,
    #[error("order decision is not accepted")]
    DecisionNotAccepted,
    #[error("order field {0} does not match")]
    OrderBindingMismatch(&'static str),
    #[error("inventory bin {0} is missing")]
    MissingInventoryBin(String),
    #[error("inventory bin {0} is duplicated")]
    DuplicateInventoryBin(String),
    #[error("inventory commitment does not match order request")]
    InventoryCommitmentMismatch,
    #[error("inventory bin {0} would overcommit listing capacity")]
    InventoryOvercommit(String),
    #[error("inventory quantity overflow")]
    InventoryOverflow,
    #[error("public values encoding failed")]
    PublicValuesEncoding,
}

pub fn reduce_order_acceptance_public_values(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradePublicValuesExecution, RadrootsSp1TradeGuestError> {
    validate_witness_header(witness)?;
    witness
        .request
        .validate()
        .map_err(|_| RadrootsSp1TradeGuestError::InvalidOrderRequest)?;
    witness
        .decision
        .validate()
        .map_err(|_| RadrootsSp1TradeGuestError::InvalidOrderDecision)?;
    validate_order_binding(witness)?;

    let request_counts = aggregate_requested_counts(&witness.request)?;
    let accepted_counts = aggregate_accepted_counts(&witness.decision)?;
    if request_counts != accepted_counts {
        return Err(RadrootsSp1TradeGuestError::InventoryCommitmentMismatch);
    }

    let inventory_bins = inventory_bins_by_id(&witness.inventory_bins)?;
    let next_inventory = apply_inventory_delta(&request_counts, &inventory_bins)?;
    let previous_state_root = witness
        .previous_state_root
        .clone()
        .unwrap_or_else(empty_state_root);
    validate_hash32(&previous_state_root, "previous_state_root")?;

    let event_set_root = event_set_root([
        witness.listing_event_id.as_str(),
        witness.request_event_id.as_str(),
        witness.decision_event_id.as_str(),
    ]);
    let inventory_delta_root = hash_json("radroots:inventory-delta:v1", &request_counts)?;
    let inventory_prev_root = hash_json("radroots:inventory-prev:v1", &inventory_bins)?;
    let inventory_new_root = hash_json("radroots:inventory-new:v1", &next_inventory)?;
    let changed_records_root = hash_json(
        "radroots:changed-records:v1",
        &ChangedRecordsMaterial {
            order_id: &witness.request.order_id,
            listing_addr: &witness.request.listing_addr,
            target_event_id: &witness.decision_event_id,
            inventory_new_root: &inventory_new_root,
        },
    )?;
    let new_state_root = hash_json(
        "radroots:state-root:v1",
        &StateRootMaterial {
            previous_state_root: &previous_state_root,
            event_set_root: &event_set_root,
            changed_records_root: &changed_records_root,
            inventory_new_root: &inventory_new_root,
        },
    )?;

    let public_values = RadrootsSp1TradeProofPublicValues {
        schema_version: RADROOTS_SP1_TRADE_PUBLIC_VALUES_SCHEMA_VERSION,
        statement_type: RadrootsSp1TradeProofStatementType::TradeTransition,
        radroots_protocol_version: witness.radroots_protocol_version.clone(),
        reducer_program_hash: witness.reducer_program_hash.clone(),
        sp1_verifying_key_hash: witness.sp1_verifying_key_hash.clone(),
        event_set_root,
        listing_addr_hash: Some(hash_bytes(
            "radroots:listing-addr:v1",
            witness.request.listing_addr.as_bytes(),
        )),
        listing_event_id: Some(witness.listing_event_id.clone()),
        order_id_hash: Some(hash_bytes(
            "radroots:order-id:v1",
            witness.request.order_id.as_bytes(),
        )),
        root_event_id: Some(witness.request_event_id.clone()),
        target_event_id: Some(witness.decision_event_id.clone()),
        previous_state_root,
        new_state_root,
        transition: Some(RadrootsSp1TradeProofTransitionKind::OrderAccepted),
        result: RadrootsSp1TradeProofResult::Valid,
        error_bitmap: zero_error_bitmap().to_string(),
        inventory_delta_root: Some(inventory_delta_root),
        inventory_sequence: Some(witness.inventory_sequence),
        inventory_prev_root: Some(inventory_prev_root),
        inventory_new_root: Some(inventory_new_root),
        changed_records_root,
    };
    let canonical_public_values = canonical_public_values_bytes(&public_values)?;
    let public_values_hash = validation_receipt_public_values_hash_hex(&canonical_public_values);
    Ok(RadrootsSp1TradePublicValuesExecution {
        public_values,
        canonical_public_values,
        public_values_hash,
    })
}

pub fn canonical_public_values_bytes(
    public_values: &RadrootsSp1TradeProofPublicValues,
) -> Result<Vec<u8>, RadrootsSp1TradeGuestError> {
    validate_public_values(public_values)?;
    serde_json::to_vec(public_values).map_err(|_| RadrootsSp1TradeGuestError::PublicValuesEncoding)
}

pub fn reduce_order_acceptance_canonical_public_values(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<Vec<u8>, RadrootsSp1TradeGuestError> {
    Ok(reduce_order_acceptance_public_values(witness)?.canonical_public_values)
}

pub fn public_values_hash_hex(
    public_values: &RadrootsSp1TradeProofPublicValues,
) -> Result<String, RadrootsSp1TradeGuestError> {
    let bytes = canonical_public_values_bytes(public_values)?;
    Ok(validation_receipt_public_values_hash_hex(&bytes))
}

pub fn empty_state_root() -> String {
    hash_bytes("radroots:state-empty:v1", &[])
}

fn validate_witness_header(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<(), RadrootsSp1TradeGuestError> {
    validate_event_id(&witness.listing_event_id, "listing_event_id")?;
    validate_event_id(&witness.request_event_id, "request_event_id")?;
    validate_event_id(&witness.decision_event_id, "decision_event_id")?;
    validate_required_str(&witness.reducer_program_hash, "reducer_program_hash")?;
    validate_hash32(&witness.reducer_program_hash, "reducer_program_hash")?;
    validate_required_str(
        &witness.radroots_protocol_version,
        "radroots_protocol_version",
    )?;
    if let Some(hash) = &witness.sp1_verifying_key_hash {
        validate_hash32(hash, "sp1_verifying_key_hash")?;
    }
    Ok(())
}

fn validate_order_binding(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<(), RadrootsSp1TradeGuestError> {
    if !matches!(
        witness.decision.decision,
        RadrootsTradeOrderDecision::Accepted { .. }
    ) {
        return Err(RadrootsSp1TradeGuestError::DecisionNotAccepted);
    }
    if witness.request.order_id != witness.decision.order_id {
        return Err(RadrootsSp1TradeGuestError::OrderBindingMismatch("order_id"));
    }
    if witness.request.listing_addr != witness.decision.listing_addr {
        return Err(RadrootsSp1TradeGuestError::OrderBindingMismatch(
            "listing_addr",
        ));
    }
    if witness.request.buyer_pubkey != witness.decision.buyer_pubkey {
        return Err(RadrootsSp1TradeGuestError::OrderBindingMismatch(
            "buyer_pubkey",
        ));
    }
    if witness.request.seller_pubkey != witness.decision.seller_pubkey {
        return Err(RadrootsSp1TradeGuestError::OrderBindingMismatch(
            "seller_pubkey",
        ));
    }
    Ok(())
}

fn aggregate_requested_counts(
    request: &RadrootsTradeOrderRequested,
) -> Result<BTreeMap<String, u64>, RadrootsSp1TradeGuestError> {
    let mut counts = BTreeMap::new();
    for item in &request.items {
        let entry = counts.entry(item.bin_id.clone()).or_insert(0u64);
        *entry = entry
            .checked_add(u64::from(item.bin_count))
            .ok_or(RadrootsSp1TradeGuestError::InventoryOverflow)?;
    }
    Ok(counts)
}

fn aggregate_accepted_counts(
    decision: &RadrootsTradeOrderDecisionEvent,
) -> Result<BTreeMap<String, u64>, RadrootsSp1TradeGuestError> {
    let RadrootsTradeOrderDecision::Accepted {
        inventory_commitments,
    } = &decision.decision
    else {
        return Err(RadrootsSp1TradeGuestError::DecisionNotAccepted);
    };
    let mut counts = BTreeMap::new();
    for commitment in inventory_commitments {
        let entry = counts.entry(commitment.bin_id.clone()).or_insert(0u64);
        *entry = entry
            .checked_add(u64::from(commitment.bin_count))
            .ok_or(RadrootsSp1TradeGuestError::InventoryOverflow)?;
    }
    Ok(counts)
}

fn inventory_bins_by_id(
    bins: &[RadrootsSp1TradeInventoryBinWitness],
) -> Result<BTreeMap<String, RadrootsSp1TradeInventoryBinWitness>, RadrootsSp1TradeGuestError> {
    let mut result = BTreeMap::new();
    for bin in bins {
        validate_required_str(&bin.bin_id, "inventory_bins.bin_id")?;
        if result.insert(bin.bin_id.clone(), bin.clone()).is_some() {
            return Err(RadrootsSp1TradeGuestError::DuplicateInventoryBin(
                bin.bin_id.clone(),
            ));
        }
    }
    Ok(result)
}

fn apply_inventory_delta(
    request_counts: &BTreeMap<String, u64>,
    bins: &BTreeMap<String, RadrootsSp1TradeInventoryBinWitness>,
) -> Result<BTreeMap<String, u64>, RadrootsSp1TradeGuestError> {
    let mut next = BTreeMap::new();
    for (bin_id, requested) in request_counts {
        let bin = bins
            .get(bin_id)
            .ok_or_else(|| RadrootsSp1TradeGuestError::MissingInventoryBin(bin_id.clone()))?;
        let reserved = bin
            .previous_reserved
            .checked_add(*requested)
            .ok_or(RadrootsSp1TradeGuestError::InventoryOverflow)?;
        if reserved > bin.listing_capacity {
            return Err(RadrootsSp1TradeGuestError::InventoryOvercommit(
                bin_id.clone(),
            ));
        }
        next.insert(bin_id.clone(), reserved);
    }
    Ok(next)
}

fn validate_public_values(
    public_values: &RadrootsSp1TradeProofPublicValues,
) -> Result<(), RadrootsSp1TradeGuestError> {
    if public_values.schema_version != RADROOTS_SP1_TRADE_PUBLIC_VALUES_SCHEMA_VERSION {
        return Err(RadrootsSp1TradeGuestError::InvalidHash("schema_version"));
    }
    validate_required_str(
        &public_values.radroots_protocol_version,
        "radroots_protocol_version",
    )?;
    validate_hash32(&public_values.reducer_program_hash, "reducer_program_hash")?;
    if let Some(hash) = &public_values.sp1_verifying_key_hash {
        validate_hash32(hash, "sp1_verifying_key_hash")?;
    }
    validate_hash32(&public_values.event_set_root, "event_set_root")?;
    if let Some(hash) = &public_values.listing_addr_hash {
        validate_hash32(hash, "listing_addr_hash")?;
    }
    if let Some(event_id) = &public_values.listing_event_id {
        validate_event_id(event_id, "listing_event_id")?;
    }
    if let Some(hash) = &public_values.order_id_hash {
        validate_hash32(hash, "order_id_hash")?;
    }
    if let Some(event_id) = &public_values.root_event_id {
        validate_event_id(event_id, "root_event_id")?;
    }
    if let Some(event_id) = &public_values.target_event_id {
        validate_event_id(event_id, "target_event_id")?;
    }
    validate_hash32(&public_values.previous_state_root, "previous_state_root")?;
    validate_hash32(&public_values.new_state_root, "new_state_root")?;
    validate_hash32(&public_values.changed_records_root, "changed_records_root")?;
    if public_values.error_bitmap != zero_error_bitmap() {
        return Err(RadrootsSp1TradeGuestError::InvalidHash("error_bitmap"));
    }
    if let Some(hash) = &public_values.inventory_delta_root {
        validate_hash32(hash, "inventory_delta_root")?;
    }
    if let Some(hash) = &public_values.inventory_prev_root {
        validate_hash32(hash, "inventory_prev_root")?;
    }
    if let Some(hash) = &public_values.inventory_new_root {
        validate_hash32(hash, "inventory_new_root")?;
    }
    Ok(())
}

fn event_set_root<'a>(event_ids: impl IntoIterator<Item = &'a str>) -> String {
    let mut sorted = event_ids.into_iter().collect::<Vec<_>>();
    sorted.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(b"radroots:event-set:v1");
    for event_id in sorted {
        hasher.update(event_id.as_bytes());
    }
    format!("0x{}", hex_lower(hasher.finalize().as_slice()))
}

fn hash_json<T: Serialize>(
    domain: &'static str,
    value: &T,
) -> Result<String, RadrootsSp1TradeGuestError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| RadrootsSp1TradeGuestError::PublicValuesEncoding)?;
    Ok(hash_bytes(domain, &bytes))
}

fn hash_bytes(domain: &'static str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    format!("0x{}", hex_lower(hasher.finalize().as_slice()))
}

fn validate_required_str(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsSp1TradeGuestError> {
    if value.trim().is_empty() {
        return Err(RadrootsSp1TradeGuestError::EmptyField(field));
    }
    Ok(())
}

fn validate_hash32(value: &str, field: &'static str) -> Result<(), RadrootsSp1TradeGuestError> {
    if value.len() != 66 || !value.starts_with("0x") || !is_lower_hex(&value[2..]) {
        return Err(RadrootsSp1TradeGuestError::InvalidHash(field));
    }
    Ok(())
}

fn validate_event_id(value: &str, field: &'static str) -> Result<(), RadrootsSp1TradeGuestError> {
    if value.len() != 64 || !is_lower_hex(value) {
        return Err(RadrootsSp1TradeGuestError::InvalidEventId(field));
    }
    Ok(())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn zero_error_bitmap() -> &'static str {
    "0x00000000000000000000000000000000"
}

#[derive(Serialize)]
struct ChangedRecordsMaterial<'a> {
    order_id: &'a str,
    listing_addr: &'a str,
    target_event_id: &'a str,
    inventory_new_root: &'a str,
}

#[derive(Serialize)]
struct StateRootMaterial<'a> {
    previous_state_root: &'a str,
    event_set_root: &'a str,
    changed_records_root: &'a str,
    inventory_new_root: &'a str,
}

#[cfg(test)]
mod tests {
    use super::{
        RADROOTS_SP1_TRADE_PROTOCOL_VERSION, RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH,
        RadrootsSp1TradeGuestError, RadrootsSp1TradeInventoryBinWitness,
        RadrootsSp1TradeOrderAcceptanceWitness, RadrootsSp1TradeProofResult,
        RadrootsSp1TradeProofTransitionKind, canonical_public_values_bytes,
        reduce_order_acceptance_canonical_public_values, reduce_order_acceptance_public_values,
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::trade::{
        RadrootsTradeInventoryCommitment, RadrootsTradeOrderDecision,
        RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderEconomicItem,
        RadrootsTradeOrderEconomicLine, RadrootsTradeOrderEconomics, RadrootsTradeOrderItem,
        RadrootsTradeOrderRequested, RadrootsTradePricingBasis,
    };

    fn witness() -> RadrootsSp1TradeOrderAcceptanceWitness {
        RadrootsSp1TradeOrderAcceptanceWitness {
            listing_event_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            request_event_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
            decision_event_id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            request: request(2),
            decision: decision(2),
            inventory_bins: vec![RadrootsSp1TradeInventoryBinWitness {
                bin_id: "bin-1".to_string(),
                listing_capacity: 5,
                previous_reserved: 1,
            }],
            inventory_sequence: 7,
            previous_state_root: None,
            reducer_program_hash: RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH.to_string(),
            radroots_protocol_version: RADROOTS_SP1_TRADE_PROTOCOL_VERSION.to_string(),
            sp1_verifying_key_hash: Some(
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
        }
    }

    fn request(bin_count: u32) -> RadrootsTradeOrderRequested {
        RadrootsTradeOrderRequested {
            order_id: "order-1".to_string(),
            listing_addr:
                "30402:1111111111111111111111111111111111111111111111111111111111111111:listing-1"
                    .to_string(),
            buyer_pubkey: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            seller_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            items: vec![RadrootsTradeOrderItem {
                bin_id: "bin-1".to_string(),
                bin_count,
            }],
            economics: economics(bin_count),
        }
    }

    fn decision(bin_count: u32) -> RadrootsTradeOrderDecisionEvent {
        RadrootsTradeOrderDecisionEvent {
            order_id: "order-1".to_string(),
            listing_addr:
                "30402:1111111111111111111111111111111111111111111111111111111111111111:listing-1"
                    .to_string(),
            buyer_pubkey: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            seller_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            decision: RadrootsTradeOrderDecision::Accepted {
                inventory_commitments: vec![RadrootsTradeInventoryCommitment {
                    bin_id: "bin-1".to_string(),
                    bin_count,
                }],
            },
        }
    }

    fn economics(bin_count: u32) -> RadrootsTradeOrderEconomics {
        let subtotal =
            (RadrootsCoreDecimal::from(5u32) * RadrootsCoreDecimal::from(bin_count)).to_string();
        RadrootsTradeOrderEconomics {
            quote_id: "quote-1".to_string(),
            quote_version: 1,
            pricing_basis: RadrootsTradePricingBasis::ListingEvent,
            currency: RadrootsCoreCurrency::USD,
            items: vec![RadrootsTradeOrderEconomicItem {
                bin_id: "bin-1".to_string(),
                bin_count,
                quantity_amount: decimal("1"),
                quantity_unit: RadrootsCoreUnit::Each,
                unit_price_amount: decimal("5"),
                unit_price_currency: RadrootsCoreCurrency::USD,
                line_subtotal: usd(&subtotal),
            }],
            discounts: Vec::<RadrootsTradeOrderEconomicLine>::new(),
            adjustments: Vec::<RadrootsTradeOrderEconomicLine>::new(),
            subtotal: usd(&subtotal),
            discount_total: usd("0"),
            adjustment_total: usd("0"),
            total: usd(&subtotal),
        }
    }

    fn decimal(raw: &str) -> RadrootsCoreDecimal {
        raw.parse().expect("decimal")
    }

    fn usd(raw: &str) -> RadrootsCoreMoney {
        RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
    }

    #[test]
    fn order_acceptance_public_values_are_deterministic() {
        let left = reduce_order_acceptance_public_values(&witness()).expect("left execution");
        let right = reduce_order_acceptance_public_values(&witness()).expect("right execution");
        assert_eq!(left.public_values, right.public_values);
        assert_eq!(left.canonical_public_values, right.canonical_public_values);
        assert_eq!(left.public_values_hash, right.public_values_hash);
        assert_eq!(
            left.public_values.transition,
            Some(RadrootsSp1TradeProofTransitionKind::OrderAccepted)
        );
        assert_eq!(
            left.public_values.result,
            RadrootsSp1TradeProofResult::Valid
        );
        assert_eq!(
            left.public_values.root_event_id.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert_eq!(
            left.public_values.target_event_id.as_deref(),
            Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        );
    }

    #[test]
    fn public_values_canonical_bytes_reencode_identically() {
        let execution = reduce_order_acceptance_public_values(&witness()).expect("execution");
        let decoded: super::RadrootsSp1TradeProofPublicValues =
            serde_json::from_slice(&execution.canonical_public_values).expect("decode");
        let encoded = canonical_public_values_bytes(&decoded).expect("reencode");
        assert_eq!(execution.canonical_public_values, encoded);
    }

    #[test]
    fn guest_public_values_output_is_canonical_bytes() {
        let execution = reduce_order_acceptance_public_values(&witness()).expect("execution");
        let bytes =
            reduce_order_acceptance_canonical_public_values(&witness()).expect("guest bytes");
        assert_eq!(bytes, execution.canonical_public_values);
    }

    #[test]
    fn overcommitted_inventory_is_rejected() {
        let mut input = witness();
        input.inventory_bins[0].listing_capacity = 2;
        let err = reduce_order_acceptance_public_values(&input).expect_err("overcommit");
        assert_eq!(
            err,
            RadrootsSp1TradeGuestError::InventoryOvercommit("bin-1".to_string())
        );
    }

    #[test]
    fn mismatched_commitment_is_rejected() {
        let mut input = witness();
        input.decision = decision(1);
        let err = reduce_order_acceptance_public_values(&input).expect_err("mismatch");
        assert_eq!(err, RadrootsSp1TradeGuestError::InventoryCommitmentMismatch);
    }
}
