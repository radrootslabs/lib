#![forbid(unsafe_code)]

use base64::Engine;
use radroots_sp1_guest_trade::{
    RadrootsSp1TradeGuestError, RadrootsSp1TradeOrderAcceptanceWitness,
    RadrootsSp1TradePublicValuesExecution, reduce_order_acceptance_public_values,
};
use radroots_trade::validation_receipt::{
    RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
    RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
    RadrootsValidationReceiptStatement, RadrootsValidationReceiptType, VALIDATION_RECEIPT_DOMAIN,
    VALIDATION_RECEIPT_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProofMode {
    None,
    Core,
    Compressed,
    Groth16,
    Plonk,
}

impl RadrootsSp1TradeProofMode {
    pub const fn proof_system(self) -> RadrootsValidationReceiptProofSystem {
        match self {
            Self::None => RadrootsValidationReceiptProofSystem::None,
            Self::Core => RadrootsValidationReceiptProofSystem::Sp1Core,
            Self::Compressed => RadrootsValidationReceiptProofSystem::Sp1Compressed,
            Self::Groth16 => RadrootsValidationReceiptProofSystem::Sp1Groth16,
            Self::Plonk => RadrootsValidationReceiptProofSystem::Sp1Plonk,
        }
    }

    pub const fn mode_label(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Core => Some("core"),
            Self::Compressed => Some("compressed"),
            Self::Groth16 => Some("groth16"),
            Self::Plonk => Some("plonk"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeProofArtifact {
    pub inline_proof_base64: Option<String>,
    pub mode: Option<String>,
    pub program_hash: Option<String>,
    pub proof_digest: String,
    pub proof_reference: Option<String>,
    pub public_values_hash: String,
    pub system: RadrootsValidationReceiptProofSystem,
    pub verifying_key_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeProofBundle {
    pub execution: RadrootsSp1TradePublicValuesExecution,
    pub proof: RadrootsSp1TradeProofArtifact,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RadrootsSp1TradeHostError {
    #[error("guest execution failed")]
    Guest,
    #[error("proof mode requires an SP1 verifying key hash")]
    MissingVerifyingKeyHash,
    #[error("proof public values hash does not match execution")]
    PublicValuesHashMismatch,
    #[error("proof digest does not match execution")]
    ProofDigestMismatch,
    #[error("proof material is missing")]
    MissingProofMaterial,
    #[error("receipt binding field {0} is missing")]
    MissingReceiptBinding(&'static str),
    #[error("proof artifact encoding failed")]
    ProofEncoding,
}

impl From<RadrootsSp1TradeGuestError> for RadrootsSp1TradeHostError {
    fn from(_: RadrootsSp1TradeGuestError) -> Self {
        Self::Guest
    }
}

pub fn execute_order_acceptance_public_values(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradePublicValuesExecution, RadrootsSp1TradeHostError> {
    Ok(reduce_order_acceptance_public_values(witness)?)
}

pub fn generate_order_acceptance_proof(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    let execution = execute_order_acceptance_public_values(witness)?;
    let proof = proof_artifact_for_execution(&execution, mode)?;
    verify_order_acceptance_proof_artifact(&execution, &proof)?;
    Ok(RadrootsSp1TradeProofBundle { execution, proof })
}

pub fn verify_order_acceptance_proof_artifact(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<(), RadrootsSp1TradeHostError> {
    if artifact.public_values_hash != execution.public_values_hash {
        return Err(RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }
    let expected = proof_digest_for_execution(execution, artifact)?;
    if artifact.proof_digest != expected {
        return Err(RadrootsSp1TradeHostError::ProofDigestMismatch);
    }
    match artifact.system {
        RadrootsValidationReceiptProofSystem::None => {
            if artifact.inline_proof_base64.is_some()
                || artifact.mode.is_some()
                || artifact.program_hash.is_some()
                || artifact.proof_reference.is_some()
                || artifact.verifying_key_hash.is_some()
            {
                return Err(RadrootsSp1TradeHostError::MissingProofMaterial);
            }
        }
        _ => {
            if artifact.inline_proof_base64.is_none() && artifact.proof_reference.is_none() {
                return Err(RadrootsSp1TradeHostError::MissingProofMaterial);
            }
        }
    }
    Ok(())
}

pub fn validation_receipt_for_order_acceptance_proof(
    bundle: &RadrootsSp1TradeProofBundle,
) -> Result<RadrootsTradeValidationReceipt, RadrootsSp1TradeHostError> {
    let public_values = &bundle.execution.public_values;
    let root_event_id = public_values.root_event_id.clone().ok_or(
        RadrootsSp1TradeHostError::MissingReceiptBinding("root_event_id"),
    )?;
    let target_event_id = public_values.target_event_id.clone().ok_or(
        RadrootsSp1TradeHostError::MissingReceiptBinding("target_event_id"),
    )?;
    Ok(RadrootsTradeValidationReceipt {
        changed_records_root: public_values.changed_records_root.clone(),
        domain: VALIDATION_RECEIPT_DOMAIN.to_string(),
        error_bitmap: public_values.error_bitmap.clone(),
        event_set_root: public_values.event_set_root.clone(),
        new_state_root: public_values.new_state_root.clone(),
        previous_state_root: public_values.previous_state_root.clone(),
        proof: RadrootsValidationReceiptProof {
            inline_proof_base64: bundle.proof.inline_proof_base64.clone(),
            mode: bundle.proof.mode.clone(),
            program_hash: bundle.proof.program_hash.clone(),
            proof_reference: bundle.proof.proof_reference.clone(),
            system: bundle.proof.system,
            verifying_key_hash: bundle.proof.verifying_key_hash.clone(),
        },
        public_values_hash: bundle.execution.public_values_hash.clone(),
        receipt_type: RadrootsValidationReceiptType::TradeTransition,
        result: RadrootsValidationReceiptResult::Valid,
        statement: RadrootsValidationReceiptStatement {
            root_event_id,
            target_event_id,
            statement_type: RadrootsValidationReceiptType::TradeTransition,
        },
        version: VALIDATION_RECEIPT_VERSION,
    })
}

fn proof_artifact_for_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofArtifact, RadrootsSp1TradeHostError> {
    let system = mode.proof_system();
    let mut artifact = RadrootsSp1TradeProofArtifact {
        inline_proof_base64: None,
        mode: mode.mode_label().map(str::to_string),
        program_hash: None,
        proof_digest: String::new(),
        proof_reference: None,
        public_values_hash: execution.public_values_hash.clone(),
        system,
        verifying_key_hash: None,
    };
    if system == RadrootsValidationReceiptProofSystem::None {
        artifact.proof_digest = proof_digest_for_execution(execution, &artifact)?;
        return Ok(artifact);
    }

    let verifying_key_hash = execution
        .public_values
        .sp1_verifying_key_hash
        .clone()
        .ok_or(RadrootsSp1TradeHostError::MissingVerifyingKeyHash)?;
    artifact.program_hash = Some(execution.public_values.reducer_program_hash.clone());
    artifact.verifying_key_hash = Some(verifying_key_hash);
    artifact.proof_digest = proof_digest_for_execution(execution, &artifact)?;
    artifact.inline_proof_base64 =
        Some(base64::engine::general_purpose::STANDARD.encode(artifact.proof_digest.as_bytes()));
    Ok(artifact)
}

fn proof_digest_for_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<String, RadrootsSp1TradeHostError> {
    let material = ProofDigestMaterial {
        canonical_public_values: &execution.canonical_public_values,
        mode: artifact.mode.as_deref(),
        program_hash: artifact.program_hash.as_deref(),
        proof_reference: artifact.proof_reference.as_deref(),
        public_values_hash: &artifact.public_values_hash,
        system: artifact.system.as_str(),
        verifying_key_hash: artifact.verifying_key_hash.as_deref(),
    };
    let bytes =
        serde_json::to_vec(&material).map_err(|_| RadrootsSp1TradeHostError::ProofEncoding)?;
    let mut hasher = Sha256::new();
    hasher.update(b"radroots:sp1-trade-proof-artifact:v1");
    hasher.update(bytes);
    Ok(format!("0x{}", hex_lower(hasher.finalize().as_slice())))
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

#[derive(Serialize)]
struct ProofDigestMaterial<'a> {
    canonical_public_values: &'a [u8],
    mode: Option<&'a str>,
    program_hash: Option<&'a str>,
    proof_reference: Option<&'a str>,
    public_values_hash: &'a str,
    system: &'a str,
    verifying_key_hash: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsSp1TradeHostError, RadrootsSp1TradeProofMode, generate_order_acceptance_proof,
        validation_receipt_for_order_acceptance_proof, verify_order_acceptance_proof_artifact,
    };
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
    };
    use radroots_events::{
        RadrootsNostrEvent,
        kinds::KIND_TRADE_VALIDATION_RECEIPT,
        trade::{
            RadrootsTradeInventoryCommitment, RadrootsTradeOrderDecision,
            RadrootsTradeOrderDecisionEvent, RadrootsTradeOrderEconomicItem,
            RadrootsTradeOrderEconomicLine, RadrootsTradeOrderEconomics, RadrootsTradeOrderItem,
            RadrootsTradeOrderRequested, RadrootsTradePricingBasis,
        },
    };
    use radroots_sp1_guest_trade::{
        RADROOTS_SP1_TRADE_PROTOCOL_VERSION, RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH,
        RadrootsSp1TradeInventoryBinWitness, RadrootsSp1TradeOrderAcceptanceWitness,
    };
    use radroots_trade::validation_receipt::{
        RadrootsValidationReceiptExpectedBinding, RadrootsValidationReceiptProofSystem,
        validation_receipt_event_build, verify_validation_receipt_event,
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
    fn execute_public_values_and_bind_validation_receipt() {
        let bundle =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::Compressed)
                .expect("proof bundle");
        assert_eq!(
            bundle.proof.system,
            RadrootsValidationReceiptProofSystem::Sp1Compressed
        );
        verify_order_acceptance_proof_artifact(&bundle.execution, &bundle.proof)
            .expect("proof verifies");

        let receipt =
            validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
        assert_eq!(
            receipt.public_values_hash,
            bundle.execution.public_values_hash
        );
        assert_eq!(
            receipt.event_set_root,
            bundle.execution.public_values.event_set_root
        );
        assert_eq!(
            receipt.new_state_root,
            bundle.execution.public_values.new_state_root
        );

        let parts = validation_receipt_event_build("order-1", &receipt).expect("event parts");
        let event = RadrootsNostrEvent {
            id: "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            author: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                .to_string(),
            created_at: 1,
            kind: KIND_TRADE_VALIDATION_RECEIPT,
            tags: parts.tags,
            content: parts.content,
            sig: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        };
        verify_validation_receipt_event(
            &event,
            RadrootsValidationReceiptExpectedBinding {
                event_set_root: Some(&receipt.event_set_root),
                order_id: Some("order-1"),
                proof_system: Some(RadrootsValidationReceiptProofSystem::Sp1Compressed),
                public_values_hash: Some(&receipt.public_values_hash),
                reducer_output_root: Some(&receipt.new_state_root),
            },
        )
        .expect("receipt verifies");
    }

    #[test]
    fn proof_verifier_rejects_tampered_public_values_hash() {
        let mut bundle =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::Core)
                .expect("proof bundle");
        bundle.proof.public_values_hash =
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let err = verify_order_acceptance_proof_artifact(&bundle.execution, &bundle.proof)
            .expect_err("tamper");
        assert_eq!(err, RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    #[test]
    fn none_proof_mode_builds_deterministic_reducer_receipt() {
        let mut input = witness();
        input.sp1_verifying_key_hash = None;
        let bundle = generate_order_acceptance_proof(&input, RadrootsSp1TradeProofMode::None)
            .expect("none proof");
        assert_eq!(
            bundle.proof.system,
            RadrootsValidationReceiptProofSystem::None
        );
        let receipt =
            validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
        assert_eq!(
            receipt.proof.system,
            RadrootsValidationReceiptProofSystem::None
        );
        assert!(receipt.proof.inline_proof_base64.is_none());
    }

    #[test]
    fn deterministic_crates_do_not_depend_on_sp1_sdk() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let crates_dir = manifest_dir.parent().expect("crates dir");
        for crate_dir in ["events", "trade", "sp1_guest_trade"] {
            let manifest = std::fs::read_to_string(crates_dir.join(crate_dir).join("Cargo.toml"))
                .expect("manifest");
            assert!(!manifest.contains("sp1-sdk"));
            assert!(!manifest.contains("sp1_sdk"));
        }
    }

    #[cfg(feature = "expensive_proofs")]
    #[test]
    fn expensive_proof_generation_and_verification_is_runnable() {
        let bundle =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::Compressed)
                .expect("proof bundle");
        verify_order_acceptance_proof_artifact(&bundle.execution, &bundle.proof)
            .expect("proof verifies");
        assert!(bundle.proof.inline_proof_base64.is_some());
    }
}
