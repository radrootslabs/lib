#![forbid(unsafe_code)]

#[cfg(feature = "expensive_proofs")]
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
    #[error("SP1 proof generation requires the expensive proof lane")]
    Sp1ProofGenerationRequired,
    #[error("SP1 proof mode is required")]
    Sp1ProofModeRequired,
    #[error("SP1 setup failed: {0}")]
    Sp1SetupFailed(String),
    #[error("SP1 proof generation failed: {0}")]
    Sp1ProofFailed(String),
    #[error("SP1 proof verification failed: {0}")]
    Sp1ProofVerificationFailed(String),
    #[error("SP1 proof material failed to decode: {0}")]
    Sp1ProofMaterialDecode(String),
    #[error("SP1 proof material is synthetic")]
    Sp1SyntheticProofMaterial,
    #[error("SP1 proof mode does not match the proof artifact")]
    Sp1ProofModeMismatch,
    #[error("SP1 verifying key hash mismatch")]
    Sp1VerifyingKeyHashMismatch,
    #[error("SP1 program hash mismatch")]
    Sp1ProgramHashMismatch,
    #[error("SP1 program hash is missing")]
    MissingSp1ProgramHash,
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
pub fn sp1_program_hash_for_order_acceptance_guest() -> String {
    sp1_program_hash_for_elf(&order_acceptance_guest_elf())
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
    use sp1_sdk::{HashableKey, Prover, ProverClient, ProvingKey, SP1Stdin, StatusCode};

    let client = ProverClient::builder().light().build().await;
    let pk = client
        .setup(elf.clone())
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1SetupFailed(error.to_string()))?;
    let verifying_key_hash = pk.verifying_key().bytes32();
    let witness = witness_with_sp1_identity(
        witness,
        Some(sp1_program_hash_for_elf(&elf)),
        Some(verifying_key_hash),
    )?;
    let expected = execute_order_acceptance_public_values(&witness)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    let (public_values, report) = client
        .execute(elf, stdin)
        .calculate_gas(true)
        .expected_exit_code(StatusCode::SUCCESS)
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ExecuteFailed(error.to_string()))?;
    if report.exit_code != 0 {
        return Err(RadrootsSp1TradeHostError::Sp1ExitCode(report.exit_code));
    }

    let (committed_public_values, execution) = execution_from_sp1_public_values(public_values)?;
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

#[cfg(feature = "expensive_proofs")]
pub async fn generate_order_acceptance_sp1_proof(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    use sp1_sdk::{
        HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1Stdin, StatusCode,
    };

    let sp1_mode = sp1_proof_mode(mode)?;
    let client = ProverClient::builder().cpu().build().await;
    let elf = order_acceptance_guest_elf();
    let sp1_program_hash = sp1_program_hash_for_elf(&elf);
    let pk = client
        .setup(elf)
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1SetupFailed(error.to_string()))?;
    let verifying_key_hash = pk.verifying_key().bytes32();
    let witness =
        witness_with_sp1_identity(witness, Some(sp1_program_hash), Some(verifying_key_hash))?;
    let expected = execute_order_acceptance_public_values(&witness)?;
    let mut stdin = SP1Stdin::new();
    stdin.write(&witness);
    let proof = client
        .prove(&pk, stdin)
        .mode(sp1_mode)
        .expected_exit_code(StatusCode::SUCCESS)
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofFailed(error.to_string()))?;
    if !sp1_proof_material_is_real(&proof.proof) {
        return Err(RadrootsSp1TradeHostError::Sp1SyntheticProofMaterial);
    }
    client
        .verify(&proof, pk.verifying_key(), Some(StatusCode::SUCCESS))
        .map_err(|error| {
            RadrootsSp1TradeHostError::Sp1ProofVerificationFailed(error.to_string())
        })?;
    let (_, execution) = execution_from_sp1_public_values(proof.public_values.clone())?;
    if execution != expected {
        return Err(RadrootsSp1TradeHostError::Sp1PublicValuesMismatch);
    }
    let proof_bytes =
        bincode::serialize(&proof).map_err(|_| RadrootsSp1TradeHostError::ProofEncoding)?;
    let proof = proof_artifact_for_real_sp1_execution(&execution, mode, &proof_bytes)?;
    verify_order_acceptance_proof_artifact(&execution, &proof)?;
    Ok(RadrootsSp1TradeProofBundle { execution, proof })
}

#[cfg(feature = "expensive_proofs")]
pub async fn verify_order_acceptance_sp1_proof_artifact(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<(), RadrootsSp1TradeHostError> {
    use sp1_sdk::{HashableKey, Prover, ProverClient, ProvingKey, StatusCode};

    verify_order_acceptance_proof_artifact(execution, artifact)?;
    let mode = artifact_proof_mode(artifact)?;
    let proof = decode_sp1_proof_artifact(artifact)?;
    if !sp1_proof_material_is_real(&proof.proof) {
        return Err(RadrootsSp1TradeHostError::Sp1SyntheticProofMaterial);
    }
    if !sp1_proof_matches_mode(&proof.proof, mode) {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeMismatch);
    }
    let client = ProverClient::builder().cpu().build().await;
    let pk = client
        .setup(order_acceptance_guest_elf())
        .await
        .map_err(|error| RadrootsSp1TradeHostError::Sp1SetupFailed(error.to_string()))?;
    let verifying_key_hash = pk.verifying_key().bytes32();
    if artifact.verifying_key_hash.as_deref() != Some(verifying_key_hash.as_str()) {
        return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
    }
    let sp1_program_hash = sp1_program_hash_for_order_acceptance_guest();
    if artifact.program_hash.as_deref() != Some(sp1_program_hash.as_str()) {
        return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
    }
    client
        .verify(&proof, pk.verifying_key(), Some(StatusCode::SUCCESS))
        .map_err(|error| {
            RadrootsSp1TradeHostError::Sp1ProofVerificationFailed(error.to_string())
        })?;
    let (_, proof_execution) = execution_from_sp1_public_values(proof.public_values)?;
    if &proof_execution != execution {
        return Err(RadrootsSp1TradeHostError::Sp1PublicValuesMismatch);
    }
    Ok(())
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
            if artifact.program_hash.as_deref()
                != execution.public_values.sp1_program_hash.as_deref()
            {
                return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
            }
            if artifact.verifying_key_hash.as_deref()
                != execution.public_values.sp1_verifying_key_hash.as_deref()
            {
                return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
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

    Err(RadrootsSp1TradeHostError::Sp1ProofGenerationRequired)
}

#[cfg(feature = "expensive_proofs")]
fn proof_artifact_for_real_sp1_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    mode: RadrootsSp1TradeProofMode,
    proof_bytes: &[u8],
) -> Result<RadrootsSp1TradeProofArtifact, RadrootsSp1TradeHostError> {
    let system = mode.proof_system();
    if system == RadrootsValidationReceiptProofSystem::None {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired);
    }
    let mut artifact = RadrootsSp1TradeProofArtifact {
        inline_proof_base64: Some(base64::engine::general_purpose::STANDARD.encode(proof_bytes)),
        mode: mode.mode_label().map(str::to_string),
        program_hash: Some(
            execution
                .public_values
                .sp1_program_hash
                .clone()
                .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?,
        ),
        proof_digest: String::new(),
        proof_reference: None,
        public_values_hash: execution.public_values_hash.clone(),
        system,
        verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
    };
    artifact.proof_digest = proof_digest_for_execution(execution, &artifact)?;
    Ok(artifact)
}

fn proof_digest_for_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<String, RadrootsSp1TradeHostError> {
    let material = ProofDigestMaterial {
        canonical_public_values: &execution.canonical_public_values,
        inline_proof_base64: artifact.inline_proof_base64.as_deref(),
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
fn sp1_program_hash_for_elf(elf: &sp1_sdk::Elf) -> String {
    let bytes: &[u8] = match elf {
        sp1_sdk::Elf::Static(bytes) => bytes,
        sp1_sdk::Elf::Dynamic(bytes) => bytes,
    };
    hash_bytes("radroots:sp1-guest-elf:v1", bytes)
}

#[cfg(feature = "expensive_proofs")]
fn hash_bytes(domain: &'static str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    format!("0x{}", hex_lower(hasher.finalize().as_slice()))
}

#[cfg(feature = "expensive_proofs")]
fn witness_with_sp1_identity(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    sp1_program_hash: Option<String>,
    sp1_verifying_key_hash: Option<String>,
) -> Result<RadrootsSp1TradeOrderAcceptanceWitness, RadrootsSp1TradeHostError> {
    if let (Some(existing), Some(actual)) = (
        witness.sp1_program_hash.as_deref(),
        sp1_program_hash.as_deref(),
    ) {
        if existing != actual {
            return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
        }
    }
    if let (Some(existing), Some(actual)) = (
        witness.sp1_verifying_key_hash.as_deref(),
        sp1_verifying_key_hash.as_deref(),
    ) {
        if existing != actual {
            return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
        }
    }

    let mut bound = witness.clone();
    if let Some(hash) = sp1_program_hash {
        bound.sp1_program_hash = Some(hash);
    }
    if let Some(hash) = sp1_verifying_key_hash {
        bound.sp1_verifying_key_hash = Some(hash);
    }
    Ok(bound)
}

#[cfg(feature = "expensive_proofs")]
fn public_values_prefix(bytes: &[u8]) -> String {
    const PREFIX_LEN: usize = 32;
    hex_lower(&bytes[..bytes.len().min(PREFIX_LEN)])
}

#[cfg(feature = "expensive_proofs")]
fn execution_from_sp1_public_values(
    public_values: sp1_sdk::SP1PublicValues,
) -> Result<(Vec<u8>, RadrootsSp1TradePublicValuesExecution), RadrootsSp1TradeHostError> {
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
    Ok((
        committed_public_values,
        RadrootsSp1TradePublicValuesExecution {
            public_values: decoded,
            public_values_hash,
            canonical_public_values,
        },
    ))
}

#[cfg(feature = "expensive_proofs")]
fn sp1_proof_mode(
    mode: RadrootsSp1TradeProofMode,
) -> Result<sp1_sdk::SP1ProofMode, RadrootsSp1TradeHostError> {
    match mode {
        RadrootsSp1TradeProofMode::None => Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired),
        RadrootsSp1TradeProofMode::Core => Ok(sp1_sdk::SP1ProofMode::Core),
        RadrootsSp1TradeProofMode::Compressed => Ok(sp1_sdk::SP1ProofMode::Compressed),
        RadrootsSp1TradeProofMode::Groth16 => Ok(sp1_sdk::SP1ProofMode::Groth16),
        RadrootsSp1TradeProofMode::Plonk => Ok(sp1_sdk::SP1ProofMode::Plonk),
    }
}

#[cfg(feature = "expensive_proofs")]
fn artifact_proof_mode(
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<RadrootsSp1TradeProofMode, RadrootsSp1TradeHostError> {
    match artifact.system {
        RadrootsValidationReceiptProofSystem::None => {
            Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired)
        }
        RadrootsValidationReceiptProofSystem::Sp1Core => Ok(RadrootsSp1TradeProofMode::Core),
        RadrootsValidationReceiptProofSystem::Sp1Compressed => {
            Ok(RadrootsSp1TradeProofMode::Compressed)
        }
        RadrootsValidationReceiptProofSystem::Sp1Groth16 => Ok(RadrootsSp1TradeProofMode::Groth16),
        RadrootsValidationReceiptProofSystem::Sp1Plonk => Ok(RadrootsSp1TradeProofMode::Plonk),
    }
}

#[cfg(feature = "expensive_proofs")]
fn decode_sp1_proof_artifact(
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<sp1_sdk::SP1ProofWithPublicValues, RadrootsSp1TradeHostError> {
    let inline = artifact
        .inline_proof_base64
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingProofMaterial)?;
    let proof_bytes = base64::engine::general_purpose::STANDARD
        .decode(inline)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))?;
    bincode::deserialize::<sp1_sdk::SP1ProofWithPublicValues>(&proof_bytes)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

#[cfg(feature = "expensive_proofs")]
fn sp1_proof_material_is_real(proof: &sp1_sdk::SP1Proof) -> bool {
    match proof {
        sp1_sdk::SP1Proof::Core(chunks) => !chunks.is_empty(),
        sp1_sdk::SP1Proof::Compressed(_) => true,
        sp1_sdk::SP1Proof::Groth16(proof) => !proof.encoded_proof.is_empty(),
        sp1_sdk::SP1Proof::Plonk(proof) => !proof.encoded_proof.is_empty(),
    }
}

#[cfg(feature = "expensive_proofs")]
fn sp1_proof_matches_mode(proof: &sp1_sdk::SP1Proof, mode: RadrootsSp1TradeProofMode) -> bool {
    matches!(
        (proof, mode),
        (sp1_sdk::SP1Proof::Core(_), RadrootsSp1TradeProofMode::Core)
            | (
                sp1_sdk::SP1Proof::Compressed(_),
                RadrootsSp1TradeProofMode::Compressed
            )
            | (
                sp1_sdk::SP1Proof::Groth16(_),
                RadrootsSp1TradeProofMode::Groth16
            )
            | (
                sp1_sdk::SP1Proof::Plonk(_),
                RadrootsSp1TradeProofMode::Plonk
            )
    )
}

#[derive(Serialize)]
struct ProofDigestMaterial<'a> {
    canonical_public_values: &'a [u8],
    inline_proof_base64: Option<&'a str>,
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
    #[cfg(feature = "expensive_proofs")]
    use base64::Engine;
    use radroots_events::{RadrootsNostrEvent, kinds::KIND_TRADE_VALIDATION_RECEIPT};
    use radroots_sp1_guest_trade::{
        RADROOTS_SP1_TRADE_KIND_LISTING, RADROOTS_SP1_TRADE_KIND_ORDER_DECISION,
        RADROOTS_SP1_TRADE_KIND_ORDER_REQUEST, RADROOTS_SP1_TRADE_ORDER_ACCEPTANCE_PROOF_TARGET,
        RADROOTS_SP1_TRADE_PROTOCOL_VERSION, RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH,
        RADROOTS_SP1_TRADE_WITNESS_VERSION, RadrootsSp1TradeCanonicalEventEvidence,
        RadrootsSp1TradeEventEvidenceRole, RadrootsSp1TradeEventWorkflowPosition,
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
            witness_version: RADROOTS_SP1_TRADE_WITNESS_VERSION,
            proof_target: RADROOTS_SP1_TRADE_ORDER_ACCEPTANCE_PROOF_TARGET.to_string(),
            listing_event_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            request_event_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
            decision_event_id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_string(),
            event_evidence: event_evidence(),
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
            sp1_program_hash: Some(
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            sp1_verifying_key_hash: Some(
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
        }
    }

    #[cfg(feature = "expensive_proofs")]
    fn witness_without_sp1_identity() -> RadrootsSp1TradeOrderAcceptanceWitness {
        let mut input = witness();
        input.sp1_program_hash = None;
        input.sp1_verifying_key_hash = None;
        input
    }

    fn event_evidence() -> Vec<RadrootsSp1TradeCanonicalEventEvidence> {
        vec![
            RadrootsSp1TradeCanonicalEventEvidence {
                event_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
                signer_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
                kind: RADROOTS_SP1_TRADE_KIND_LISTING,
                canonical_event_hash:
                    "0x1010101010101010101010101010101010101010101010101010101010101010".to_string(),
                signature_hash:
                    "0x1111111111111111111111111111111111111111111111111111111111111111".to_string(),
                preverified_signature: true,
                role: RadrootsSp1TradeEventEvidenceRole::Seller,
                workflow_position: RadrootsSp1TradeEventWorkflowPosition::Listing,
                content_hash: "0x1212121212121212121212121212121212121212121212121212121212121212"
                    .to_string(),
                tags_hash: "0x1313131313131313131313131313131313131313131313131313131313131313"
                    .to_string(),
                ordering_key: "001:listing".to_string(),
            },
            RadrootsSp1TradeCanonicalEventEvidence {
                event_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
                signer_pubkey: "2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
                kind: RADROOTS_SP1_TRADE_KIND_ORDER_REQUEST,
                canonical_event_hash:
                    "0x2020202020202020202020202020202020202020202020202020202020202020".to_string(),
                signature_hash:
                    "0x2121212121212121212121212121212121212121212121212121212121212121".to_string(),
                preverified_signature: true,
                role: RadrootsSp1TradeEventEvidenceRole::Buyer,
                workflow_position: RadrootsSp1TradeEventWorkflowPosition::OrderRequest,
                content_hash: "0x2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
                tags_hash: "0x2323232323232323232323232323232323232323232323232323232323232323"
                    .to_string(),
                ordering_key: "002:order_request".to_string(),
            },
            RadrootsSp1TradeCanonicalEventEvidence {
                event_id: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
                signer_pubkey: "1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
                kind: RADROOTS_SP1_TRADE_KIND_ORDER_DECISION,
                canonical_event_hash:
                    "0x3030303030303030303030303030303030303030303030303030303030303030".to_string(),
                signature_hash:
                    "0x3131313131313131313131313131313131313131313131313131313131313131".to_string(),
                preverified_signature: true,
                role: RadrootsSp1TradeEventEvidenceRole::Seller,
                workflow_position: RadrootsSp1TradeEventWorkflowPosition::OrderDecision,
                content_hash: "0x3232323232323232323232323232323232323232323232323232323232323232"
                    .to_string(),
                tags_hash: "0x3333333333333333333333333333333333333333333333333333333333333333"
                    .to_string(),
                ordering_key: "003:order_decision".to_string(),
            },
        ]
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
        let bundle = generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::None)
            .expect("proof bundle");
        assert_eq!(
            bundle.proof.system,
            RadrootsValidationReceiptProofSystem::None
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
                proof_system: Some(RadrootsValidationReceiptProofSystem::None),
                public_values_hash: Some(&receipt.public_values_hash),
                reducer_output_root: Some(&receipt.new_state_root),
            },
        )
        .expect("receipt verifies");
    }

    #[test]
    fn proof_verifier_rejects_tampered_public_values_hash() {
        let mut bundle =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::None)
                .expect("proof bundle");
        bundle.proof.public_values_hash =
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let err = verify_order_acceptance_proof_artifact(&bundle.execution, &bundle.proof)
            .expect_err("tamper");
        assert_eq!(err, RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    #[test]
    fn sp1_modes_require_the_expensive_proof_lane() {
        let err =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::Compressed)
                .expect_err("synthetic sp1 proof");
        assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProofGenerationRequired);
    }

    #[test]
    fn sp1_artifact_program_hash_must_match_public_values_identity() {
        let mut input = witness();
        input.sp1_program_hash =
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        let execution =
            super::execute_order_acceptance_public_values(&input).expect("deterministic execution");
        let mut artifact = super::RadrootsSp1TradeProofArtifact {
            inline_proof_base64: Some("cHJvb2Y=".to_string()),
            mode: Some("core".to_string()),
            program_hash: Some(
                "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            ),
            proof_digest: String::new(),
            proof_reference: None,
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        artifact.proof_digest =
            super::proof_digest_for_execution(&execution, &artifact).expect("proof digest");
        let err = verify_order_acceptance_proof_artifact(&execution, &artifact)
            .expect_err("program hash mismatch");
        assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
    }

    #[test]
    fn sp1_artifact_program_hash_is_distinct_from_reducer_hash() {
        let mut input = witness();
        input.sp1_program_hash =
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        let execution =
            super::execute_order_acceptance_public_values(&input).expect("deterministic execution");
        let mut artifact = super::RadrootsSp1TradeProofArtifact {
            inline_proof_base64: Some("cHJvb2Y=".to_string()),
            mode: Some("core".to_string()),
            program_hash: execution.public_values.sp1_program_hash.clone(),
            proof_digest: String::new(),
            proof_reference: None,
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        artifact.proof_digest =
            super::proof_digest_for_execution(&execution, &artifact).expect("proof digest");
        verify_order_acceptance_proof_artifact(&execution, &artifact).expect("artifact verifies");
        assert_ne!(
            artifact.program_hash.as_deref(),
            Some(execution.public_values.reducer_program_hash.as_str())
        );
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
        let input = witness_without_sp1_identity();
        let execution = super::execute_order_acceptance_sp1_public_values(&input)
            .await
            .expect("sp1 execute");
        let mut expected_input = input;
        expected_input.sp1_program_hash =
            execution.execution.public_values.sp1_program_hash.clone();
        expected_input.sp1_verifying_key_hash = execution
            .execution
            .public_values
            .sp1_verifying_key_hash
            .clone();
        let expected = super::execute_order_acceptance_public_values(&expected_input)
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
    #[tokio::test]
    async fn expensive_proof_generation_and_verification_is_runnable() {
        let bundle = super::generate_order_acceptance_sp1_proof(
            &witness_without_sp1_identity(),
            RadrootsSp1TradeProofMode::Core,
        )
        .await
        .expect("proof bundle");
        super::verify_order_acceptance_sp1_proof_artifact(&bundle.execution, &bundle.proof)
            .await
            .expect("proof verifies");
        assert_eq!(
            bundle.proof.system,
            RadrootsValidationReceiptProofSystem::Sp1Core
        );
        assert!(bundle.proof.inline_proof_base64.is_some());
    }

    #[cfg(feature = "expensive_proofs")]
    #[tokio::test]
    async fn real_sp1_verifier_rejects_missing_and_synthetic_material() {
        let execution =
            super::execute_order_acceptance_sp1_public_values(&witness_without_sp1_identity())
                .await
                .expect("sp1 execute")
                .execution;
        let mut missing = super::RadrootsSp1TradeProofArtifact {
            inline_proof_base64: None,
            mode: Some("core".to_string()),
            program_hash: execution.public_values.sp1_program_hash.clone(),
            proof_digest: "0x00".to_string(),
            proof_reference: None,
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: Some(
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
        };
        let err = super::verify_order_acceptance_sp1_proof_artifact(&execution, &missing)
            .await
            .expect_err("missing proof material");
        assert_eq!(err, RadrootsSp1TradeHostError::ProofDigestMismatch);

        missing.proof_digest =
            super::proof_digest_for_execution(&execution, &missing).expect("missing proof digest");
        let err = super::verify_order_acceptance_sp1_proof_artifact(&execution, &missing)
            .await
            .expect_err("missing proof material");
        assert_eq!(err, RadrootsSp1TradeHostError::MissingProofMaterial);

        let mut synthetic = missing;
        synthetic.inline_proof_base64 =
            Some(base64::engine::general_purpose::STANDARD.encode(b"synthetic proof material"));
        synthetic.proof_digest = super::proof_digest_for_execution(&execution, &synthetic)
            .expect("synthetic proof digest");
        let err = super::verify_order_acceptance_sp1_proof_artifact(&execution, &synthetic)
            .await
            .expect_err("synthetic proof material");
        assert!(matches!(
            err,
            RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(_)
        ));
    }
}
