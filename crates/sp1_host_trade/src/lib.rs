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
    #[error("SP1 execution failed: {0}")]
    Sp1ExecuteFailed(String),
    #[error("SP1 execution returned exit code {0}")]
    Sp1ExitCode(u64),
    #[error("SP1 public values failed to decode: {0}")]
    Sp1PublicValuesDecode(String),
    #[error("SP1 public values do not match deterministic reducer output")]
    Sp1PublicValuesMismatch,
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

#[cfg(feature = "expensive_proofs")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeExecuteReport {
    pub exit_code: u64,
    pub gas: Option<u64>,
    pub total_instruction_count: u64,
    pub total_syscall_count: u64,
}

#[cfg(feature = "expensive_proofs")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeExecuteBundle {
    pub committed_public_values: Vec<u8>,
    pub execution: RadrootsSp1TradePublicValuesExecution,
    pub report: RadrootsSp1TradeExecuteReport,
}

#[cfg(feature = "expensive_proofs")]
pub fn order_acceptance_guest_elf() -> sp1_sdk::Elf {
    sp1_sdk::include_elf!("radroots_sp1_trade_order_acceptance_guest")
}

#[cfg(feature = "expensive_proofs")]
pub async fn execute_order_acceptance_sp1_public_values(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradeExecuteBundle, RadrootsSp1TradeHostError> {
    execute_order_acceptance_sp1_public_values_with_elf(order_acceptance_guest_elf(), witness).await
}

#[cfg(feature = "expensive_proofs")]
pub async fn execute_order_acceptance_sp1_public_values_with_elf(
    elf: sp1_sdk::Elf,
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradeExecuteBundle, RadrootsSp1TradeHostError> {
    use sp1_sdk::{Prover, ProverClient, SP1Stdin, StatusCode};

    let expected = execute_order_acceptance_public_values(witness)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(witness);
    let client = ProverClient::builder().light().build().await;
    let (public_values, report) = client
        .execute(elf, stdin)
        .calculate_gas(true)
        .expected_exit_code(StatusCode::SUCCESS)
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ExecuteFailed(error.to_string()))?;
    if report.exit_code != 0 {
        return Err(RadrootsSp1TradeHostError::Sp1ExitCode(report.exit_code));
    }

    let committed_public_values = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut public_values = public_values;
        public_values.read::<Vec<u8>>()
    }))
    .map_err(|_| {
        RadrootsSp1TradeHostError::Sp1PublicValuesDecode(
            "SP1 public values stream did not contain canonical bytes".to_string(),
        )
    })?;
    let decoded: radroots_sp1_guest_trade::RadrootsSp1TradeProofPublicValues =
        serde_json::from_slice(&committed_public_values).map_err(|error| {
            RadrootsSp1TradeHostError::Sp1PublicValuesDecode(format!(
                "{error}; public values bytes={}; prefix={}",
                committed_public_values.len(),
                public_values_prefix(&committed_public_values)
            ))
        })?;
    let canonical_public_values = radroots_sp1_guest_trade::canonical_public_values_bytes(&decoded)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1PublicValuesDecode(error.to_string()))?;
    let public_values_hash = radroots_sp1_guest_trade::public_values_hash_hex(&decoded)?;
    let execution = RadrootsSp1TradePublicValuesExecution {
        public_values: decoded,
        public_values_hash,
        canonical_public_values,
    };
    if execution != expected {
        return Err(RadrootsSp1TradeHostError::Sp1PublicValuesMismatch);
    }

    Ok(RadrootsSp1TradeExecuteBundle {
        committed_public_values,
        execution,
        report: RadrootsSp1TradeExecuteReport {
            exit_code: report.exit_code,
            gas: report.gas(),
            total_instruction_count: report.total_instruction_count(),
            total_syscall_count: report.total_syscall_count(),
        },
    })
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

#[cfg(feature = "expensive_proofs")]
fn public_values_prefix(bytes: &[u8]) -> String {
    const PREFIX_LEN: usize = 32;
    hex_lower(&bytes[..bytes.len().min(PREFIX_LEN)])
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
    use radroots_events::{RadrootsNostrEvent, kinds::KIND_TRADE_VALIDATION_RECEIPT};
    use radroots_sp1_guest_trade::{
        RADROOTS_SP1_TRADE_PROTOCOL_VERSION, RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH,
        RadrootsSp1TradeInventoryBinWitness, RadrootsSp1TradeInventoryCommitmentWitness,
        RadrootsSp1TradeOrderAcceptanceWitness, RadrootsSp1TradeOrderDecisionEventWitness,
        RadrootsSp1TradeOrderDecisionWitness, RadrootsSp1TradeOrderItemWitness,
        RadrootsSp1TradeOrderRequestWitness,
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

    fn request(bin_count: u32) -> RadrootsSp1TradeOrderRequestWitness {
        RadrootsSp1TradeOrderRequestWitness {
            order_id: "order-1".to_string(),
            listing_addr:
                "30402:1111111111111111111111111111111111111111111111111111111111111111:listing-1"
                    .to_string(),
            buyer_pubkey: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            seller_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            items: vec![RadrootsSp1TradeOrderItemWitness {
                bin_id: "bin-1".to_string(),
                bin_count,
            }],
        }
    }

    fn decision(bin_count: u32) -> RadrootsSp1TradeOrderDecisionEventWitness {
        RadrootsSp1TradeOrderDecisionEventWitness {
            order_id: "order-1".to_string(),
            listing_addr:
                "30402:1111111111111111111111111111111111111111111111111111111111111111:listing-1"
                    .to_string(),
            buyer_pubkey: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            seller_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            decision: RadrootsSp1TradeOrderDecisionWitness::Accepted {
                inventory_commitments: vec![RadrootsSp1TradeInventoryCommitmentWitness {
                    bin_id: "bin-1".to_string(),
                    bin_count,
                }],
            },
        }
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
    #[tokio::test]
    async fn sp1_execute_public_values_match_deterministic_reducer() {
        let execution = super::execute_order_acceptance_sp1_public_values(&witness())
            .await
            .expect("sp1 execute");
        let expected = super::execute_order_acceptance_public_values(&witness())
            .expect("deterministic execution");
        assert_eq!(execution.execution, expected);
        assert_eq!(
            execution.committed_public_values,
            expected.canonical_public_values
        );
        assert_eq!(execution.report.exit_code, 0);
        assert!(execution.report.total_instruction_count > 0);
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
