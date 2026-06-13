#![forbid(unsafe_code)]

use base64::Engine;
use radroots_sp1_guest_trade::{
    RadrootsSp1TradeGuestError, RadrootsSp1TradeOrderAcceptanceWitness,
    RadrootsSp1TradeProofResult, RadrootsSp1TradePublicValuesExecution,
    reduce_order_acceptance_public_values,
};
#[cfg(feature = "sp1_verify")]
use radroots_sp1_guest_trade::{
    RadrootsSp1TradeProofPublicValues, RadrootsSp1TradeProofStatementType,
};
use radroots_trade::validation_receipt::{
    RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
    RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
    RadrootsValidationReceiptStatement, RadrootsValidationReceiptType, VALIDATION_RECEIPT_DOMAIN,
    VALIDATION_RECEIPT_PROOF_REFERENCE_SHA256_PREFIX, VALIDATION_RECEIPT_VERSION,
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

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "core" => Some(Self::Core),
            "compressed" => Some(Self::Compressed),
            "groth16" => Some(Self::Groth16),
            "plonk" => Some(Self::Plonk),
            _ => None,
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

pub const RADROOTS_SP1_TRADE_PROOF_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_SP1_TRADE_REMOTE_PROVER_SCHEMA_VERSION: u32 = 1;
pub const RADROOTS_SP1_TRADE_SP1_VERSION_LINE: &str = "sp1-sdk-6.2.1";
pub const RADROOTS_SP1_TRADE_PROOF_CODEC: &str = "sp1-proof-with-public-values-bincode";
pub const RADROOTS_SP1_TRADE_VERIFYING_KEY_CODEC: &str = "sp1-verifying-key-bincode";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeProofEnvelope {
    pub schema_version: u32,
    pub sp1_version_line: String,
    pub proof_system: String,
    pub proof_mode: String,
    pub proof_codec: String,
    pub proof_content_hash: String,
    pub proof_digest: String,
    pub public_values_hash: String,
    pub canonical_public_values_hash: String,
    pub sp1_program_hash: String,
    pub sp1_verifying_key_hash: String,
    pub sp1_verifying_key_codec: String,
    pub sp1_verifying_key_base64: String,
    pub receipt_type: String,
    pub receipt_result: String,
    pub listing_event_id: String,
    pub root_event_id: String,
    pub target_event_id: String,
    pub event_set_root: String,
    pub previous_state_root: String,
    pub new_state_root: String,
    pub changed_records_root: String,
    pub error_bitmap: String,
    pub proof_content_base64: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeResolvedProofArtifact {
    pub artifact: RadrootsSp1TradeProofArtifact,
    #[serde(default)]
    pub resolved_proof_envelope_base64: Option<String>,
}

impl RadrootsSp1TradeResolvedProofArtifact {
    pub fn inline(artifact: RadrootsSp1TradeProofArtifact) -> Self {
        Self {
            artifact,
            resolved_proof_envelope_base64: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProverBackend {
    Disabled,
    DeterministicNone,
    LocalExecute,
    LocalCpuProve,
    LocalCudaProve,
    RemoteHttpProve,
}

impl RadrootsSp1TradeProverBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::DeterministicNone => "deterministic_none",
            Self::LocalExecute => "local_execute",
            Self::LocalCpuProve => "local_cpu_prove",
            Self::LocalCudaProve => "local_cuda_prove",
            Self::RemoteHttpProve => "remote_http_prove",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "deterministic_none" => Some(Self::DeterministicNone),
            "local_execute" => Some(Self::LocalExecute),
            "local_cpu_prove" => Some(Self::LocalCpuProve),
            "local_cuda_prove" => Some(Self::LocalCudaProve),
            "remote_http_prove" => Some(Self::RemoteHttpProve),
            _ => None,
        }
    }
}

impl core::fmt::Display for RadrootsSp1TradeProverBackend {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeProofEngine {
    Cpu,
    Cuda,
}

impl RadrootsSp1TradeProofEngine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "cpu" => Some(Self::Cpu),
            "cuda" => Some(Self::Cuda),
            _ => None,
        }
    }
}

impl core::fmt::Display for RadrootsSp1TradeProofEngine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeRemoteProverStatus {
    Accepted,
    Running,
    Completed,
    Failed,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeWorkerResultStatus {
    Succeeded,
}

impl RadrootsSp1TradeWorkerResultStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
        }
    }
}

impl core::fmt::Display for RadrootsSp1TradeWorkerResultStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsSp1TradeWorkerRole {
    NonAuthoritativeProver,
}

impl RadrootsSp1TradeWorkerRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonAuthoritativeProver => "non_authoritative_prover",
        }
    }
}

impl core::fmt::Display for RadrootsSp1TradeWorkerRole {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeRemoteProverRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub proof_target: String,
    pub proof_mode: RadrootsSp1TradeProofMode,
    pub sp1_version_line: String,
    pub witness: RadrootsSp1TradeOrderAcceptanceWitness,
    pub expected_sp1_program_hash: String,
    pub expected_sp1_verifying_key_hash: String,
    pub expected_public_values_hash: String,
    pub expected_reducer_program_hash: String,
    pub expected_protocol_version: String,
    pub expected_witness_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeRemoteProverResponse {
    pub schema_version: u32,
    pub request_id: String,
    pub status: RadrootsSp1TradeRemoteProverStatus,
    #[serde(default)]
    pub status_url: Option<String>,
    #[serde(default)]
    pub status_path: Option<String>,
    #[serde(default)]
    pub proof_system: Option<RadrootsValidationReceiptProofSystem>,
    #[serde(default)]
    pub proof_mode: Option<RadrootsSp1TradeProofMode>,
    #[serde(default)]
    pub public_values_hash: Option<String>,
    #[serde(default)]
    pub sp1_program_hash: Option<String>,
    #[serde(default)]
    pub sp1_verifying_key_hash: Option<String>,
    #[serde(default)]
    pub proof_artifact: Option<RadrootsSp1TradeProofArtifact>,
    #[serde(default)]
    pub resolved_proof_envelope_base64: Option<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsSp1TradeWorkerResultPayload {
    pub cryptographic_proof_verified: bool,
    pub decision_event_id: Option<String>,
    pub event_set_root: Option<String>,
    pub listing_event_id: Option<String>,
    pub order_id: Option<String>,
    pub proof_generated: bool,
    pub proof_mode: RadrootsSp1TradeProofMode,
    pub proof_system: RadrootsValidationReceiptProofSystem,
    pub public_values_hash: String,
    pub prover_backend: RadrootsSp1TradeProverBackend,
    pub receipt_event_id: String,
    pub receipt_kind: Option<u32>,
    pub reducer_output_root: Option<String>,
    pub request_event_id: Option<String>,
    pub sp1_execute_checked: bool,
    pub sp1_execute_public_values_hash: Option<String>,
    pub status: RadrootsSp1TradeWorkerResultStatus,
    pub worker_role: Option<RadrootsSp1TradeWorkerRole>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeProofBundle {
    pub execution: RadrootsSp1TradePublicValuesExecution,
    pub proof: RadrootsSp1TradeProofArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeValidationReceiptVerification {
    pub canonical_public_values_len: usize,
    pub proof_mode: RadrootsSp1TradeProofMode,
    pub proof_system: RadrootsValidationReceiptProofSystem,
    pub public_values_hash: String,
    pub sp1_program_hash: String,
    pub sp1_verifying_key_hash: String,
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
    #[error("proof material has conflicting inline and reference sources")]
    ProofMaterialConflict,
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
    #[error("SP1 proof generation requires the sp1_proving feature")]
    Sp1ProofGenerationRequired,
    #[error("SP1 CUDA proof generation is unavailable: {0}")]
    Sp1CudaProofEngineUnavailable(String),
    #[error("SP1 proof mode is required")]
    Sp1ProofModeRequired,
    #[error("SP1 setup failed: {0}")]
    Sp1SetupFailed(String),
    #[error("SP1 proof generation failed: {0}")]
    Sp1ProofFailed(String),
    #[error("SP1 proof verifier is unavailable in this build")]
    Sp1ProofVerifierUnavailable,
    #[error("SP1 proof verification failed: {0}")]
    Sp1ProofVerificationFailed(String),
    #[error("SP1 proof material failed to decode: {0}")]
    Sp1ProofMaterialDecode(String),
    #[error("SP1 proof material is synthetic")]
    Sp1SyntheticProofMaterial,
    #[error("SP1 proof reference is unresolved")]
    Sp1ProofReferenceUnresolved,
    #[error("SP1 proof reference is invalid")]
    InvalidSp1ProofReference,
    #[error("SP1 proof reference digest does not match resolved envelope")]
    Sp1ProofReferenceDigestMismatch,
    #[error("SP1 proof mode does not match the proof artifact")]
    Sp1ProofModeMismatch,
    #[error("SP1 verifying key hash mismatch")]
    Sp1VerifyingKeyHashMismatch,
    #[error("SP1 program hash mismatch")]
    Sp1ProgramHashMismatch,
    #[error("SP1 program hash is missing")]
    MissingSp1ProgramHash,
    #[error("validation receipt field {0} does not match SP1 public values")]
    ValidationReceiptBindingMismatch(&'static str),
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

#[cfg(feature = "sp1_proving")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeExecuteReport {
    pub exit_code: u64,
    pub gas: Option<u64>,
    pub total_instruction_count: u64,
    pub total_syscall_count: u64,
}

#[cfg(feature = "sp1_proving")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSp1TradeExecuteBundle {
    pub committed_public_values: Vec<u8>,
    pub execution: RadrootsSp1TradePublicValuesExecution,
    pub report: RadrootsSp1TradeExecuteReport,
}

#[cfg(all(feature = "sp1_verify", radroots_sp1_guest_elf))]
pub fn order_acceptance_guest_elf() -> sp1_sdk::Elf {
    sp1_sdk::include_elf!("radroots_sp1_trade_order_acceptance_guest")
}

#[cfg(all(feature = "sp1_verify", radroots_sp1_guest_elf))]
pub fn sp1_program_hash_for_order_acceptance_guest() -> String {
    sp1_program_hash_for_elf(&order_acceptance_guest_elf())
}

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
pub async fn execute_order_acceptance_sp1_public_values(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradeExecuteBundle, RadrootsSp1TradeHostError> {
    execute_order_acceptance_sp1_public_values_with_elf(order_acceptance_guest_elf(), witness).await
}

#[cfg(all(feature = "sp1_proving", not(radroots_sp1_guest_elf)))]
pub async fn execute_order_acceptance_sp1_public_values(
    _witness: &RadrootsSp1TradeOrderAcceptanceWitness,
) -> Result<RadrootsSp1TradeExecuteBundle, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1SetupFailed(
        "SP1 guest ELF build is not enabled".to_owned(),
    ))
}

#[cfg(feature = "sp1_proving")]
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

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
pub async fn generate_order_acceptance_sp1_proof(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    generate_order_acceptance_sp1_proof_with_engine(witness, mode, RadrootsSp1TradeProofEngine::Cpu)
        .await
}

#[cfg(all(feature = "sp1_proving", not(radroots_sp1_guest_elf)))]
pub async fn generate_order_acceptance_sp1_proof(
    _witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    _mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1SetupFailed(
        "SP1 guest ELF build is not enabled".to_owned(),
    ))
}

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
pub async fn generate_order_acceptance_sp1_proof_with_engine(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
    engine: RadrootsSp1TradeProofEngine,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    match engine {
        RadrootsSp1TradeProofEngine::Cpu => {
            let client = sp1_sdk::ProverClient::builder().cpu().build().await;
            generate_order_acceptance_sp1_proof_with_client(&client, witness, mode).await
        }
        RadrootsSp1TradeProofEngine::Cuda => {
            generate_order_acceptance_sp1_cuda_proof(witness, mode).await
        }
    }
}

#[cfg(all(feature = "sp1_proving", not(radroots_sp1_guest_elf)))]
pub async fn generate_order_acceptance_sp1_proof_with_engine(
    _witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    _mode: RadrootsSp1TradeProofMode,
    _engine: RadrootsSp1TradeProofEngine,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1SetupFailed(
        "SP1 guest ELF build is not enabled".to_owned(),
    ))
}

#[cfg(all(feature = "sp1_proving", feature = "sp1_cuda", radroots_sp1_guest_elf))]
async fn generate_order_acceptance_sp1_cuda_proof(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    use futures::FutureExt;
    let client = std::panic::AssertUnwindSafe(sp1_sdk::ProverClient::builder().cuda().build())
        .catch_unwind()
        .await
        .map_err(cuda_panic_to_host_error)?;
    generate_order_acceptance_sp1_proof_with_client(&client, witness, mode).await
}

#[cfg(all(feature = "sp1_proving", feature = "sp1_cuda", radroots_sp1_guest_elf))]
fn cuda_panic_to_host_error(payload: Box<dyn std::any::Any + Send>) -> RadrootsSp1TradeHostError {
    let message = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "CUDA prover initialization failed".to_owned());
    RadrootsSp1TradeHostError::Sp1CudaProofEngineUnavailable(message)
}

#[cfg(all(
    feature = "sp1_proving",
    not(feature = "sp1_cuda"),
    radroots_sp1_guest_elf
))]
async fn generate_order_acceptance_sp1_cuda_proof(
    _witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    _mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1CudaProofEngineUnavailable(
        "build without sp1_cuda feature".to_owned(),
    ))
}

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
async fn generate_order_acceptance_sp1_proof_with_client<P>(
    client: &P,
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError>
where
    P: sp1_sdk::Prover,
{
    use sp1_sdk::{HashableKey, ProveRequest, ProvingKey, SP1Stdin, StatusCode};

    let sp1_mode = sp1_proof_mode(mode)?;
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
    let verifying_key_bytes = bincode::serialize(pk.verifying_key())
        .map_err(|_| RadrootsSp1TradeHostError::ProofEncoding)?;
    let proof = proof_artifact_for_real_sp1_execution(
        &execution,
        mode,
        &proof_bytes,
        &verifying_key_bytes,
    )?;
    verify_order_acceptance_proof_artifact_structure(&execution, &proof)?;
    Ok(RadrootsSp1TradeProofBundle { execution, proof })
}

#[cfg(feature = "sp1_verify")]
pub async fn verify_order_acceptance_resolved_sp1_proof_artifact(
    execution: &RadrootsSp1TradePublicValuesExecution,
    resolved: &RadrootsSp1TradeResolvedProofArtifact,
) -> Result<(), RadrootsSp1TradeHostError> {
    use sp1_sdk::{HashableKey, Prover, ProverClient, StatusCode};

    let artifact = &resolved.artifact;
    verify_order_acceptance_proof_artifact_structure(execution, artifact)?;
    let envelope_base64 = resolved_proof_envelope_base64(resolved)?;
    let envelope = decode_proof_envelope_base64(envelope_base64)?;
    verify_proof_envelope(execution, artifact, &envelope)?;
    let mode = artifact_proof_mode(artifact)?;
    let proof = decode_sp1_proof_envelope(&envelope)?;
    if !sp1_proof_material_is_real(&proof.proof) {
        return Err(RadrootsSp1TradeHostError::Sp1SyntheticProofMaterial);
    }
    if !sp1_proof_matches_mode(&proof.proof, mode) {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeMismatch);
    }
    let client = ProverClient::builder().cpu().build().await;
    let verifying_key = decode_sp1_verifying_key_envelope(&envelope)?;
    let verifying_key_hash = verifying_key.bytes32();
    if artifact.verifying_key_hash.as_deref() != Some(verifying_key_hash.as_str()) {
        return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
    }
    let sp1_program_hash = artifact
        .program_hash
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;
    client
        .verify(&proof, &verifying_key, Some(StatusCode::SUCCESS))
        .map_err(|error| {
            RadrootsSp1TradeHostError::Sp1ProofVerificationFailed(error.to_string())
        })?;
    let (_, proof_execution) = execution_from_sp1_public_values(proof.public_values)?;
    require_public_values_sp1_identity(
        &proof_execution.public_values,
        sp1_program_hash,
        verifying_key_hash.as_str(),
    )?;
    if &proof_execution != execution {
        return Err(RadrootsSp1TradeHostError::Sp1PublicValuesMismatch);
    }
    Ok(())
}

#[cfg(not(feature = "sp1_verify"))]
pub async fn verify_order_acceptance_resolved_sp1_proof_artifact(
    _execution: &RadrootsSp1TradePublicValuesExecution,
    _resolved: &RadrootsSp1TradeResolvedProofArtifact,
) -> Result<(), RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable)
}

#[cfg(feature = "sp1_verify")]
pub async fn verify_order_acceptance_validation_receipt_inline_sp1_proof(
    receipt: &RadrootsTradeValidationReceipt,
) -> Result<RadrootsSp1TradeValidationReceiptVerification, RadrootsSp1TradeHostError> {
    use sp1_sdk::{HashableKey, Prover, ProverClient, StatusCode};

    if receipt.proof.system == RadrootsValidationReceiptProofSystem::None {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired);
    }
    if receipt.proof.proof_reference.is_some() {
        return Err(RadrootsSp1TradeHostError::Sp1ProofReferenceUnresolved);
    }

    let artifact = RadrootsSp1TradeProofArtifact {
        inline_proof_base64: Some(
            receipt
                .proof
                .inline_proof_base64
                .clone()
                .ok_or(RadrootsSp1TradeHostError::MissingProofMaterial)?,
        ),
        mode: receipt.proof.mode.clone(),
        program_hash: receipt.proof.program_hash.clone(),
        proof_digest: String::new(),
        proof_reference: None,
        public_values_hash: receipt.public_values_hash.clone(),
        system: receipt.proof.system,
        verifying_key_hash: receipt.proof.verifying_key_hash.clone(),
    };
    let mode = artifact_proof_mode(&artifact)?;
    let proof = decode_sp1_proof_artifact(&artifact)?;
    if !sp1_proof_material_is_real(&proof.proof) {
        return Err(RadrootsSp1TradeHostError::Sp1SyntheticProofMaterial);
    }
    if !sp1_proof_matches_mode(&proof.proof, mode) {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeMismatch);
    }

    let envelope = decode_proof_envelope(&artifact)?;
    let verifying_key = decode_sp1_verifying_key_envelope(&envelope)?;
    let verifying_key_hash = verifying_key.bytes32();
    if artifact.verifying_key_hash.as_deref() != Some(verifying_key_hash.as_str()) {
        return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
    }
    let sp1_program_hash = artifact
        .program_hash
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;

    let client = ProverClient::builder().cpu().build().await;
    client
        .verify(&proof, &verifying_key, Some(StatusCode::SUCCESS))
        .map_err(|error| {
            RadrootsSp1TradeHostError::Sp1ProofVerificationFailed(error.to_string())
        })?;
    let (_, execution) = execution_from_sp1_public_values(proof.public_values)?;
    require_public_values_sp1_identity(
        &execution.public_values,
        sp1_program_hash,
        verifying_key_hash.as_str(),
    )?;
    verify_proof_envelope(&execution, &artifact, &envelope)?;
    verify_validation_receipt_matches_public_values(receipt, &execution.public_values)?;
    if execution.public_values_hash != receipt.public_values_hash {
        return Err(RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    Ok(RadrootsSp1TradeValidationReceiptVerification {
        canonical_public_values_len: execution.canonical_public_values.len(),
        proof_mode: mode,
        proof_system: receipt.proof.system,
        public_values_hash: execution.public_values_hash,
        sp1_program_hash: sp1_program_hash.to_owned(),
        sp1_verifying_key_hash: verifying_key_hash,
    })
}

#[cfg(not(feature = "sp1_verify"))]
pub async fn verify_order_acceptance_validation_receipt_inline_sp1_proof(
    _receipt: &RadrootsTradeValidationReceipt,
) -> Result<RadrootsSp1TradeValidationReceiptVerification, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable)
}

#[cfg(feature = "sp1_verify")]
pub async fn verify_order_acceptance_validation_receipt_resolved_sp1_proof(
    receipt: &RadrootsTradeValidationReceipt,
    resolved: &RadrootsSp1TradeResolvedProofArtifact,
) -> Result<RadrootsSp1TradeValidationReceiptVerification, RadrootsSp1TradeHostError> {
    use sp1_sdk::{HashableKey, Prover, ProverClient, StatusCode};

    if receipt.proof.system == RadrootsValidationReceiptProofSystem::None {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired);
    }
    verify_receipt_proof_matches_artifact(receipt, &resolved.artifact)?;
    let envelope_base64 = resolved_proof_envelope_base64(resolved)?;
    let envelope = decode_proof_envelope_base64(envelope_base64)?;
    let mode = artifact_proof_mode(&resolved.artifact)?;
    let proof = decode_sp1_proof_envelope(&envelope)?;
    if !sp1_proof_material_is_real(&proof.proof) {
        return Err(RadrootsSp1TradeHostError::Sp1SyntheticProofMaterial);
    }
    if !sp1_proof_matches_mode(&proof.proof, mode) {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeMismatch);
    }

    let verifying_key = decode_sp1_verifying_key_envelope(&envelope)?;
    let verifying_key_hash = verifying_key.bytes32();
    if resolved.artifact.verifying_key_hash.as_deref() != Some(verifying_key_hash.as_str()) {
        return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
    }
    let sp1_program_hash = resolved
        .artifact
        .program_hash
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;

    let client = ProverClient::builder().cpu().build().await;
    client
        .verify(&proof, &verifying_key, Some(StatusCode::SUCCESS))
        .map_err(|error| {
            RadrootsSp1TradeHostError::Sp1ProofVerificationFailed(error.to_string())
        })?;
    let (_, execution) = execution_from_sp1_public_values(proof.public_values)?;
    require_public_values_sp1_identity(
        &execution.public_values,
        sp1_program_hash,
        verifying_key_hash.as_str(),
    )?;
    verify_order_acceptance_proof_artifact_structure(&execution, &resolved.artifact)?;
    verify_proof_envelope(&execution, &resolved.artifact, &envelope)?;
    verify_validation_receipt_matches_public_values(receipt, &execution.public_values)?;
    if execution.public_values_hash != receipt.public_values_hash {
        return Err(RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    Ok(RadrootsSp1TradeValidationReceiptVerification {
        canonical_public_values_len: execution.canonical_public_values.len(),
        proof_mode: mode,
        proof_system: receipt.proof.system,
        public_values_hash: execution.public_values_hash,
        sp1_program_hash: sp1_program_hash.to_owned(),
        sp1_verifying_key_hash: verifying_key_hash,
    })
}

#[cfg(not(feature = "sp1_verify"))]
pub async fn verify_order_acceptance_validation_receipt_resolved_sp1_proof(
    _receipt: &RadrootsTradeValidationReceipt,
    _resolved: &RadrootsSp1TradeResolvedProofArtifact,
) -> Result<RadrootsSp1TradeValidationReceiptVerification, RadrootsSp1TradeHostError> {
    Err(RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable)
}

pub fn generate_order_acceptance_proof(
    witness: &RadrootsSp1TradeOrderAcceptanceWitness,
    mode: RadrootsSp1TradeProofMode,
) -> Result<RadrootsSp1TradeProofBundle, RadrootsSp1TradeHostError> {
    let execution = execute_order_acceptance_public_values(witness)?;
    let proof = proof_artifact_for_execution(&execution, mode)?;
    verify_order_acceptance_proof_artifact_structure(&execution, &proof)?;
    Ok(RadrootsSp1TradeProofBundle { execution, proof })
}

#[cfg(feature = "sp1_verify")]
fn verify_validation_receipt_matches_public_values(
    receipt: &RadrootsTradeValidationReceipt,
    public_values: &RadrootsSp1TradeProofPublicValues,
) -> Result<(), RadrootsSp1TradeHostError> {
    let program_hash = public_values
        .sp1_program_hash
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;
    let verifying_key_hash = public_values
        .sp1_verifying_key_hash
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingVerifyingKeyHash)?;
    if receipt.proof.program_hash.as_deref() != Some(program_hash) {
        return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
    }
    if receipt.proof.verifying_key_hash.as_deref() != Some(verifying_key_hash) {
        return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
    }
    if public_values.statement_type != RadrootsSp1TradeProofStatementType::TradeTransition {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "statement_type",
        ));
    }
    if receipt.receipt_type != RadrootsValidationReceiptType::TradeTransition
        || receipt.statement.statement_type != RadrootsValidationReceiptType::TradeTransition
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "receipt_type",
        ));
    }
    if public_values.event_set_root != receipt.event_set_root {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "event_set_root",
        ));
    }
    if public_values.previous_state_root != receipt.previous_state_root {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "previous_state_root",
        ));
    }
    if public_values.new_state_root != receipt.new_state_root {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "new_state_root",
        ));
    }
    if public_values.changed_records_root != receipt.changed_records_root {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "changed_records_root",
        ));
    }
    if public_values.error_bitmap != receipt.error_bitmap {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "error_bitmap",
        ));
    }
    if public_values.listing_event_id.as_deref()
        != Some(receipt.statement.listing_event_id.as_str())
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "listing_event_id",
        ));
    }
    if public_values.root_event_id.as_deref() != Some(receipt.statement.root_event_id.as_str()) {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "root_event_id",
        ));
    }
    if public_values.target_event_id.as_deref() != Some(receipt.statement.target_event_id.as_str())
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "target_event_id",
        ));
    }
    if !receipt_result_matches_public_values(receipt.result, public_values.result) {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "result",
        ));
    }
    Ok(())
}

#[cfg(feature = "sp1_verify")]
fn require_public_values_sp1_identity(
    public_values: &RadrootsSp1TradeProofPublicValues,
    expected_program_hash: &str,
    expected_verifying_key_hash: &str,
) -> Result<(), RadrootsSp1TradeHostError> {
    match public_values.sp1_program_hash.as_deref() {
        Some(value) if value == expected_program_hash => {}
        Some(_) => return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch),
        None => return Err(RadrootsSp1TradeHostError::MissingSp1ProgramHash),
    }
    match public_values.sp1_verifying_key_hash.as_deref() {
        Some(value) if value == expected_verifying_key_hash => {}
        Some(_) => return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch),
        None => return Err(RadrootsSp1TradeHostError::MissingVerifyingKeyHash),
    }
    Ok(())
}

#[cfg(feature = "sp1_verify")]
fn receipt_result_matches_public_values(
    receipt_result: RadrootsValidationReceiptResult,
    public_values_result: RadrootsSp1TradeProofResult,
) -> bool {
    matches!(
        (receipt_result, public_values_result),
        (
            RadrootsValidationReceiptResult::Valid,
            RadrootsSp1TradeProofResult::Valid
        ) | (
            RadrootsValidationReceiptResult::Invalid,
            RadrootsSp1TradeProofResult::Invalid
        )
    )
}

pub fn verify_order_acceptance_proof_artifact_structure(
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
            match (&artifact.inline_proof_base64, &artifact.proof_reference) {
                (None, None) => return Err(RadrootsSp1TradeHostError::MissingProofMaterial),
                (Some(_), Some(_)) => {
                    return Err(RadrootsSp1TradeHostError::ProofMaterialConflict);
                }
                (None, Some(reference)) => {
                    proof_reference_digest(reference)?;
                }
                _ => {}
            }
            let public_values_program_hash = execution
                .public_values
                .sp1_program_hash
                .as_deref()
                .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;
            let public_values_verifying_key_hash = execution
                .public_values
                .sp1_verifying_key_hash
                .as_deref()
                .ok_or(RadrootsSp1TradeHostError::MissingVerifyingKeyHash)?;
            if artifact.program_hash.as_deref() != Some(public_values_program_hash) {
                return Err(RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
            }
            if artifact.verifying_key_hash.as_deref() != Some(public_values_verifying_key_hash) {
                return Err(RadrootsSp1TradeHostError::Sp1VerifyingKeyHashMismatch);
            }
            if artifact.inline_proof_base64.is_some() {
                let envelope = decode_proof_envelope(artifact)?;
                verify_proof_envelope(execution, artifact, &envelope)?;
            }
        }
    }
    Ok(())
}

pub fn validation_receipt_for_order_acceptance_proof(
    bundle: &RadrootsSp1TradeProofBundle,
) -> Result<RadrootsTradeValidationReceipt, RadrootsSp1TradeHostError> {
    let public_values = &bundle.execution.public_values;
    let listing_event_id = public_values.listing_event_id.clone().ok_or(
        RadrootsSp1TradeHostError::MissingReceiptBinding("listing_event_id"),
    )?;
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
        result: validation_receipt_result_from_public_values(public_values.result),
        statement: RadrootsValidationReceiptStatement {
            listing_event_id,
            root_event_id,
            target_event_id,
            statement_type: RadrootsValidationReceiptType::TradeTransition,
        },
        version: VALIDATION_RECEIPT_VERSION,
    })
}

fn validation_receipt_result_from_public_values(
    result: RadrootsSp1TradeProofResult,
) -> RadrootsValidationReceiptResult {
    match result {
        RadrootsSp1TradeProofResult::Valid => RadrootsValidationReceiptResult::Valid,
        RadrootsSp1TradeProofResult::Invalid => RadrootsValidationReceiptResult::Invalid,
    }
}

#[cfg(feature = "sp1_verify")]
fn verify_receipt_proof_matches_artifact(
    receipt: &RadrootsTradeValidationReceipt,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<(), RadrootsSp1TradeHostError> {
    if receipt.public_values_hash != artifact.public_values_hash {
        return Err(RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }
    if receipt.proof.inline_proof_base64.as_deref() != artifact.inline_proof_base64.as_deref()
        || receipt.proof.mode.as_deref() != artifact.mode.as_deref()
        || receipt.proof.program_hash.as_deref() != artifact.program_hash.as_deref()
        || receipt.proof.proof_reference.as_deref() != artifact.proof_reference.as_deref()
        || receipt.proof.system != artifact.system
        || receipt.proof.verifying_key_hash.as_deref() != artifact.verifying_key_hash.as_deref()
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "proof",
        ));
    }
    Ok(())
}

fn validation_receipt_result_label(result: RadrootsValidationReceiptResult) -> &'static str {
    match result {
        RadrootsValidationReceiptResult::Valid => "valid",
        RadrootsValidationReceiptResult::Invalid => "invalid",
    }
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

pub fn referenced_order_acceptance_proof_artifact_for_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    mode: RadrootsSp1TradeProofMode,
    proof_reference: String,
) -> Result<RadrootsSp1TradeProofArtifact, RadrootsSp1TradeHostError> {
    proof_reference_digest(proof_reference.as_str())?;
    let system = mode.proof_system();
    if system == RadrootsValidationReceiptProofSystem::None {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired);
    }
    let mut artifact = RadrootsSp1TradeProofArtifact {
        inline_proof_base64: None,
        mode: mode.mode_label().map(str::to_string),
        program_hash: execution.public_values.sp1_program_hash.clone(),
        proof_digest: String::new(),
        proof_reference: Some(proof_reference),
        public_values_hash: execution.public_values_hash.clone(),
        system,
        verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
    };
    artifact.proof_digest = proof_digest_for_execution(execution, &artifact)?;
    verify_order_acceptance_proof_artifact_structure(execution, &artifact)?;
    Ok(artifact)
}

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
fn proof_artifact_for_real_sp1_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    mode: RadrootsSp1TradeProofMode,
    proof_bytes: &[u8],
    verifying_key_bytes: &[u8],
) -> Result<RadrootsSp1TradeProofArtifact, RadrootsSp1TradeHostError> {
    let system = mode.proof_system();
    if system == RadrootsValidationReceiptProofSystem::None {
        return Err(RadrootsSp1TradeHostError::Sp1ProofModeRequired);
    }
    let program_hash = execution
        .public_values
        .sp1_program_hash
        .clone()
        .ok_or(RadrootsSp1TradeHostError::MissingSp1ProgramHash)?;
    let verifying_key_hash = execution
        .public_values
        .sp1_verifying_key_hash
        .clone()
        .ok_or(RadrootsSp1TradeHostError::MissingVerifyingKeyHash)?;
    let mut envelope = proof_envelope_for_real_sp1_execution(
        execution,
        system,
        mode,
        program_hash.as_str(),
        verifying_key_hash.as_str(),
        proof_bytes,
        verifying_key_bytes,
    )?;
    envelope.proof_digest = proof_digest_for_envelope(&envelope)?;
    let envelope_json =
        serde_json::to_vec(&envelope).map_err(|_| RadrootsSp1TradeHostError::ProofEncoding)?;
    Ok(RadrootsSp1TradeProofArtifact {
        inline_proof_base64: Some(base64::engine::general_purpose::STANDARD.encode(envelope_json)),
        mode: mode.mode_label().map(str::to_string),
        program_hash: Some(program_hash),
        proof_digest: envelope.proof_digest,
        proof_reference: None,
        public_values_hash: execution.public_values_hash.clone(),
        system,
        verifying_key_hash: Some(verifying_key_hash),
    })
}

fn proof_digest_for_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<String, RadrootsSp1TradeHostError> {
    if artifact.inline_proof_base64.is_some() {
        return proof_digest_for_envelope(&decode_proof_envelope(artifact)?);
    }
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

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
fn proof_envelope_for_real_sp1_execution(
    execution: &RadrootsSp1TradePublicValuesExecution,
    system: RadrootsValidationReceiptProofSystem,
    mode: RadrootsSp1TradeProofMode,
    program_hash: &str,
    verifying_key_hash: &str,
    proof_bytes: &[u8],
    verifying_key_bytes: &[u8],
) -> Result<RadrootsSp1TradeProofEnvelope, RadrootsSp1TradeHostError> {
    Ok(RadrootsSp1TradeProofEnvelope {
        schema_version: RADROOTS_SP1_TRADE_PROOF_ARTIFACT_SCHEMA_VERSION,
        sp1_version_line: RADROOTS_SP1_TRADE_SP1_VERSION_LINE.to_owned(),
        proof_system: system.as_str().to_owned(),
        proof_mode: mode
            .mode_label()
            .ok_or(RadrootsSp1TradeHostError::Sp1ProofModeRequired)?
            .to_owned(),
        proof_codec: RADROOTS_SP1_TRADE_PROOF_CODEC.to_owned(),
        proof_content_hash: hash_bytes("radroots:sp1-proof-content:v1", proof_bytes),
        proof_digest: String::new(),
        public_values_hash: execution.public_values_hash.clone(),
        canonical_public_values_hash: hash_bytes(
            "radroots:sp1-canonical-public-values:v1",
            &execution.canonical_public_values,
        ),
        sp1_program_hash: program_hash.to_owned(),
        sp1_verifying_key_hash: verifying_key_hash.to_owned(),
        sp1_verifying_key_codec: RADROOTS_SP1_TRADE_VERIFYING_KEY_CODEC.to_owned(),
        sp1_verifying_key_base64: base64::engine::general_purpose::STANDARD
            .encode(verifying_key_bytes),
        receipt_type: RadrootsValidationReceiptType::TradeTransition
            .as_str()
            .to_owned(),
        receipt_result: validation_receipt_result_label(
            validation_receipt_result_from_public_values(execution.public_values.result),
        )
        .to_owned(),
        listing_event_id: execution.public_values.listing_event_id.clone().ok_or(
            RadrootsSp1TradeHostError::MissingReceiptBinding("listing_event_id"),
        )?,
        root_event_id: execution.public_values.root_event_id.clone().ok_or(
            RadrootsSp1TradeHostError::MissingReceiptBinding("root_event_id"),
        )?,
        target_event_id: execution.public_values.target_event_id.clone().ok_or(
            RadrootsSp1TradeHostError::MissingReceiptBinding("target_event_id"),
        )?,
        event_set_root: execution.public_values.event_set_root.clone(),
        previous_state_root: execution.public_values.previous_state_root.clone(),
        new_state_root: execution.public_values.new_state_root.clone(),
        changed_records_root: execution.public_values.changed_records_root.clone(),
        error_bitmap: execution.public_values.error_bitmap.clone(),
        proof_content_base64: base64::engine::general_purpose::STANDARD.encode(proof_bytes),
    })
}

fn decode_proof_envelope(
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<RadrootsSp1TradeProofEnvelope, RadrootsSp1TradeHostError> {
    let inline = artifact
        .inline_proof_base64
        .as_deref()
        .ok_or(RadrootsSp1TradeHostError::MissingProofMaterial)?;
    decode_proof_envelope_base64(inline)
}

fn decode_proof_envelope_base64(
    value: &str,
) -> Result<RadrootsSp1TradeProofEnvelope, RadrootsSp1TradeHostError> {
    let envelope_bytes = proof_envelope_bytes_from_base64(value)?;
    serde_json::from_slice::<RadrootsSp1TradeProofEnvelope>(&envelope_bytes)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

fn proof_envelope_bytes_from_base64(value: &str) -> Result<Vec<u8>, RadrootsSp1TradeHostError> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

pub fn proof_reference_for_proof_envelope_base64(
    value: &str,
) -> Result<String, RadrootsSp1TradeHostError> {
    let envelope_bytes = proof_envelope_bytes_from_base64(value)?;
    let mut hasher = Sha256::new();
    hasher.update(envelope_bytes);
    Ok(format!(
        "{VALIDATION_RECEIPT_PROOF_REFERENCE_SHA256_PREFIX}{}",
        hex_lower(hasher.finalize().as_slice())
    ))
}

fn proof_reference_digest(value: &str) -> Result<&str, RadrootsSp1TradeHostError> {
    let Some(digest) = value.strip_prefix(VALIDATION_RECEIPT_PROOF_REFERENCE_SHA256_PREFIX) else {
        return Err(RadrootsSp1TradeHostError::InvalidSp1ProofReference);
    };
    if digest.len() != 64 || !is_lower_hex(digest) {
        return Err(RadrootsSp1TradeHostError::InvalidSp1ProofReference);
    }
    Ok(digest)
}

#[cfg(feature = "sp1_verify")]
fn resolved_proof_envelope_base64(
    resolved: &RadrootsSp1TradeResolvedProofArtifact,
) -> Result<&str, RadrootsSp1TradeHostError> {
    match (
        resolved.artifact.inline_proof_base64.as_deref(),
        resolved.artifact.proof_reference.as_deref(),
        resolved.resolved_proof_envelope_base64.as_deref(),
    ) {
        (Some(inline), None, None) => Ok(inline),
        (Some(_), None, Some(_)) => Err(RadrootsSp1TradeHostError::ProofMaterialConflict),
        (None, Some(reference), Some(envelope)) => {
            let expected = proof_reference_digest(reference)?;
            let actual = proof_reference_for_proof_envelope_base64(envelope)?;
            let actual = proof_reference_digest(actual.as_str())?;
            if actual != expected {
                return Err(RadrootsSp1TradeHostError::Sp1ProofReferenceDigestMismatch);
            }
            Ok(envelope)
        }
        (None, Some(_), None) => Err(RadrootsSp1TradeHostError::Sp1ProofReferenceUnresolved),
        (Some(_), Some(_), _) => Err(RadrootsSp1TradeHostError::ProofMaterialConflict),
        (None, None, _) => Err(RadrootsSp1TradeHostError::MissingProofMaterial),
    }
}

fn proof_content_bytes_from_envelope(
    envelope: &RadrootsSp1TradeProofEnvelope,
) -> Result<Vec<u8>, RadrootsSp1TradeHostError> {
    base64::engine::general_purpose::STANDARD
        .decode(envelope.proof_content_base64.as_str())
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

fn proof_digest_for_envelope(
    envelope: &RadrootsSp1TradeProofEnvelope,
) -> Result<String, RadrootsSp1TradeHostError> {
    let material = ProofEnvelopeDigestMaterial {
        schema_version: envelope.schema_version,
        sp1_version_line: envelope.sp1_version_line.as_str(),
        proof_system: envelope.proof_system.as_str(),
        proof_mode: envelope.proof_mode.as_str(),
        proof_codec: envelope.proof_codec.as_str(),
        proof_content_hash: envelope.proof_content_hash.as_str(),
        public_values_hash: envelope.public_values_hash.as_str(),
        canonical_public_values_hash: envelope.canonical_public_values_hash.as_str(),
        sp1_program_hash: envelope.sp1_program_hash.as_str(),
        sp1_verifying_key_hash: envelope.sp1_verifying_key_hash.as_str(),
        sp1_verifying_key_codec: envelope.sp1_verifying_key_codec.as_str(),
        sp1_verifying_key_base64: envelope.sp1_verifying_key_base64.as_str(),
        receipt_type: envelope.receipt_type.as_str(),
        receipt_result: envelope.receipt_result.as_str(),
        listing_event_id: envelope.listing_event_id.as_str(),
        root_event_id: envelope.root_event_id.as_str(),
        target_event_id: envelope.target_event_id.as_str(),
        event_set_root: envelope.event_set_root.as_str(),
        previous_state_root: envelope.previous_state_root.as_str(),
        new_state_root: envelope.new_state_root.as_str(),
        changed_records_root: envelope.changed_records_root.as_str(),
        error_bitmap: envelope.error_bitmap.as_str(),
    };
    let bytes =
        serde_json::to_vec(&material).map_err(|_| RadrootsSp1TradeHostError::ProofEncoding)?;
    Ok(hash_bytes(
        "radroots:sp1-trade-proof-envelope-digest:v1",
        &bytes,
    ))
}

fn verify_proof_envelope(
    execution: &RadrootsSp1TradePublicValuesExecution,
    artifact: &RadrootsSp1TradeProofArtifact,
    envelope: &RadrootsSp1TradeProofEnvelope,
) -> Result<(), RadrootsSp1TradeHostError> {
    if envelope.schema_version != RADROOTS_SP1_TRADE_PROOF_ARTIFACT_SCHEMA_VERSION
        || envelope.sp1_version_line != RADROOTS_SP1_TRADE_SP1_VERSION_LINE
        || envelope.proof_codec != RADROOTS_SP1_TRADE_PROOF_CODEC
        || envelope.proof_system != artifact.system.as_str()
        || Some(envelope.proof_mode.as_str()) != artifact.mode.as_deref()
        || envelope.public_values_hash != artifact.public_values_hash
        || envelope.sp1_program_hash.as_str() != artifact.program_hash.as_deref().unwrap_or("")
        || envelope.sp1_verifying_key_hash.as_str()
            != artifact.verifying_key_hash.as_deref().unwrap_or("")
        || envelope.sp1_verifying_key_codec != RADROOTS_SP1_TRADE_VERIFYING_KEY_CODEC
        || envelope.sp1_verifying_key_base64.is_empty()
    {
        return Err(RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(
            "proof envelope metadata mismatch".to_owned(),
        ));
    }
    if envelope.proof_digest != proof_digest_for_envelope(envelope)? {
        return Err(RadrootsSp1TradeHostError::ProofDigestMismatch);
    }
    let proof_bytes = proof_content_bytes_from_envelope(envelope)?;
    if envelope.proof_content_hash != hash_bytes("radroots:sp1-proof-content:v1", &proof_bytes) {
        return Err(RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(
            "proof envelope content hash mismatch".to_owned(),
        ));
    }
    let expected_canonical_public_values_hash = hash_bytes(
        "radroots:sp1-canonical-public-values:v1",
        &execution.canonical_public_values,
    );
    if envelope.canonical_public_values_hash != expected_canonical_public_values_hash {
        return Err(RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }
    if envelope.receipt_type != RadrootsValidationReceiptType::TradeTransition.as_str() {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "receipt_type",
        ));
    }
    if envelope.receipt_result
        != validation_receipt_result_label(validation_receipt_result_from_public_values(
            execution.public_values.result,
        ))
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "result",
        ));
    }
    if envelope.listing_event_id.as_str()
        != execution
            .public_values
            .listing_event_id
            .as_deref()
            .unwrap_or("")
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "listing_event_id",
        ));
    }
    if envelope.root_event_id.as_str()
        != execution
            .public_values
            .root_event_id
            .as_deref()
            .unwrap_or("")
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "root_event_id",
        ));
    }
    if envelope.target_event_id.as_str()
        != execution
            .public_values
            .target_event_id
            .as_deref()
            .unwrap_or("")
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "target_event_id",
        ));
    }
    if envelope.event_set_root != execution.public_values.event_set_root
        || envelope.previous_state_root != execution.public_values.previous_state_root
        || envelope.new_state_root != execution.public_values.new_state_root
        || envelope.changed_records_root != execution.public_values.changed_records_root
        || envelope.error_bitmap != execution.public_values.error_bitmap
    {
        return Err(RadrootsSp1TradeHostError::ValidationReceiptBindingMismatch(
            "state_roots",
        ));
    }
    Ok(())
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

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(feature = "sp1_verify")]
fn sp1_program_hash_for_elf(elf: &sp1_sdk::Elf) -> String {
    let bytes: &[u8] = match elf {
        sp1_sdk::Elf::Static(bytes) => bytes,
        sp1_sdk::Elf::Dynamic(bytes) => bytes,
    };
    hash_bytes("radroots:sp1-guest-elf:v1", bytes)
}

fn hash_bytes(domain: &'static str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    format!("0x{}", hex_lower(hasher.finalize().as_slice()))
}

#[cfg(feature = "sp1_proving")]
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

#[cfg(feature = "sp1_verify")]
fn public_values_prefix(bytes: &[u8]) -> String {
    const PREFIX_LEN: usize = 32;
    hex_lower(&bytes[..bytes.len().min(PREFIX_LEN)])
}

#[cfg(feature = "sp1_verify")]
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

#[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
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

#[cfg(feature = "sp1_verify")]
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

#[cfg(feature = "sp1_verify")]
fn decode_sp1_proof_artifact(
    artifact: &RadrootsSp1TradeProofArtifact,
) -> Result<sp1_sdk::SP1ProofWithPublicValues, RadrootsSp1TradeHostError> {
    let envelope = decode_proof_envelope(artifact)?;
    decode_sp1_proof_envelope(&envelope)
}

#[cfg(feature = "sp1_verify")]
fn decode_sp1_proof_envelope(
    envelope: &RadrootsSp1TradeProofEnvelope,
) -> Result<sp1_sdk::SP1ProofWithPublicValues, RadrootsSp1TradeHostError> {
    let proof_bytes = proof_content_bytes_from_envelope(&envelope)?;
    bincode::deserialize::<sp1_sdk::SP1ProofWithPublicValues>(&proof_bytes)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

#[cfg(feature = "sp1_verify")]
fn decode_sp1_verifying_key_envelope(
    envelope: &RadrootsSp1TradeProofEnvelope,
) -> Result<sp1_sdk::SP1VerifyingKey, RadrootsSp1TradeHostError> {
    if envelope.sp1_verifying_key_codec != RADROOTS_SP1_TRADE_VERIFYING_KEY_CODEC {
        return Err(RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(
            "proof envelope verifying key codec mismatch".to_owned(),
        ));
    }
    let verifying_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(envelope.sp1_verifying_key_base64.as_str())
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))?;
    bincode::deserialize::<sp1_sdk::SP1VerifyingKey>(&verifying_key_bytes)
        .map_err(|error| RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(error.to_string()))
}

#[cfg(feature = "sp1_verify")]
fn sp1_proof_material_is_real(proof: &sp1_sdk::SP1Proof) -> bool {
    match proof {
        sp1_sdk::SP1Proof::Core(chunks) => !chunks.is_empty(),
        sp1_sdk::SP1Proof::Compressed(_) => true,
        sp1_sdk::SP1Proof::Groth16(proof) => !proof.encoded_proof.is_empty(),
        sp1_sdk::SP1Proof::Plonk(proof) => !proof.encoded_proof.is_empty(),
    }
}

#[cfg(feature = "sp1_verify")]
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
    mode: Option<&'a str>,
    program_hash: Option<&'a str>,
    proof_reference: Option<&'a str>,
    public_values_hash: &'a str,
    system: &'a str,
    verifying_key_hash: Option<&'a str>,
}

#[derive(Serialize)]
struct ProofEnvelopeDigestMaterial<'a> {
    schema_version: u32,
    sp1_version_line: &'a str,
    proof_system: &'a str,
    proof_mode: &'a str,
    proof_codec: &'a str,
    proof_content_hash: &'a str,
    public_values_hash: &'a str,
    canonical_public_values_hash: &'a str,
    sp1_program_hash: &'a str,
    sp1_verifying_key_hash: &'a str,
    sp1_verifying_key_codec: &'a str,
    sp1_verifying_key_base64: &'a str,
    receipt_type: &'a str,
    receipt_result: &'a str,
    listing_event_id: &'a str,
    root_event_id: &'a str,
    target_event_id: &'a str,
    event_set_root: &'a str,
    previous_state_root: &'a str,
    new_state_root: &'a str,
    changed_records_root: &'a str,
    error_bitmap: &'a str,
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsSp1TradeHostError, RadrootsSp1TradeProofMode, generate_order_acceptance_proof,
        validation_receipt_for_order_acceptance_proof,
        verify_order_acceptance_proof_artifact_structure,
    };
    #[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
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
        RadrootsSp1TradeOrderRequestWitness, RadrootsSp1TradeProofResult,
        RadrootsSp1TradePublicValuesExecution,
    };
    use radroots_trade::validation_receipt::{
        RadrootsValidationReceiptExpectedBinding, RadrootsValidationReceiptProof,
        RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
        validation_receipt_event_build, verify_validation_receipt_event,
    };
    #[cfg(feature = "sp1_verify")]
    use serde::Deserialize;

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

    #[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
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
        verify_order_acceptance_proof_artifact_structure(&bundle.execution, &bundle.proof)
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
                ..RadrootsValidationReceiptExpectedBinding::default()
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
        let err =
            verify_order_acceptance_proof_artifact_structure(&bundle.execution, &bundle.proof)
                .expect_err("tamper");
        assert_eq!(err, RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    #[test]
    fn validation_receipt_result_tracks_public_values_result() {
        let mut bundle =
            generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::None)
                .expect("proof bundle");
        bundle.execution.public_values.result = RadrootsSp1TradeProofResult::Invalid;
        let receipt =
            validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
        assert_eq!(receipt.result, RadrootsValidationReceiptResult::Invalid);
    }

    #[test]
    fn sp1_modes_require_the_sp1_proving_lane() {
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
            inline_proof_base64: None,
            mode: Some("core".to_string()),
            program_hash: Some(
                "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string(),
            ),
            proof_digest: String::new(),
            proof_reference: Some(format!("radroots-proof://sha256/{}", "1".repeat(64))),
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        artifact.proof_digest =
            super::proof_digest_for_execution(&execution, &artifact).expect("proof digest");
        let err = verify_order_acceptance_proof_artifact_structure(&execution, &artifact)
            .expect_err("program hash mismatch");
        assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);
    }

    #[test]
    fn sp1_artifact_requires_public_values_sp1_identity() {
        let mut input = witness();
        input.sp1_program_hash = None;
        let execution =
            super::execute_order_acceptance_public_values(&input).expect("deterministic execution");
        let mut artifact = super::RadrootsSp1TradeProofArtifact {
            inline_proof_base64: None,
            mode: Some("core".to_string()),
            program_hash: Some(
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            proof_digest: String::new(),
            proof_reference: Some(format!("radroots-proof://sha256/{}", "1".repeat(64))),
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        artifact.proof_digest =
            super::proof_digest_for_execution(&execution, &artifact).expect("proof digest");
        let err = verify_order_acceptance_proof_artifact_structure(&execution, &artifact)
            .expect_err("missing program hash");
        assert_eq!(err, RadrootsSp1TradeHostError::MissingSp1ProgramHash);
    }

    #[cfg(feature = "sp1_verify")]
    #[tokio::test]
    async fn verify_order_acceptance_validation_receipt_inline_sp1_proof_rejects_raw_material() {
        let bundle = generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::None)
            .expect("proof bundle");
        let mut receipt =
            validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
        receipt.proof = RadrootsValidationReceiptProof {
            inline_proof_base64: Some("cHJvb2Y=".to_string()),
            mode: Some("core".to_string()),
            program_hash: Some(
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: Some(
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ),
        };
        let error = super::verify_order_acceptance_validation_receipt_inline_sp1_proof(&receipt)
            .await
            .expect_err("raw material");
        assert!(matches!(
            error,
            RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(_)
        ));
    }

    #[test]
    fn sp1_artifact_program_hash_is_distinct_from_reducer_hash() {
        let mut input = witness();
        input.sp1_program_hash =
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        let execution =
            super::execute_order_acceptance_public_values(&input).expect("deterministic execution");
        let mut artifact = super::RadrootsSp1TradeProofArtifact {
            inline_proof_base64: None,
            mode: Some("core".to_string()),
            program_hash: execution.public_values.sp1_program_hash.clone(),
            proof_digest: String::new(),
            proof_reference: Some(format!("radroots-proof://sha256/{}", "1".repeat(64))),
            public_values_hash: execution.public_values_hash.clone(),
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        artifact.proof_digest =
            super::proof_digest_for_execution(&execution, &artifact).expect("proof digest");
        verify_order_acceptance_proof_artifact_structure(&execution, &artifact)
            .expect("artifact verifies");
        assert_ne!(
            artifact.program_hash.as_deref(),
            Some(execution.public_values.reducer_program_hash.as_str())
        );
    }

    #[test]
    fn proof_reference_requires_canonical_sha256_uri() {
        let execution =
            super::execute_order_acceptance_public_values(&witness()).expect("execution");
        let err = super::referenced_order_acceptance_proof_artifact_for_execution(
            &execution,
            RadrootsSp1TradeProofMode::Core,
            "radroots-proof://sha256/xyz".to_string(),
        )
        .expect_err("invalid reference");
        assert_eq!(err, RadrootsSp1TradeHostError::InvalidSp1ProofReference);

        let artifact = super::referenced_order_acceptance_proof_artifact_for_execution(
            &execution,
            RadrootsSp1TradeProofMode::Core,
            format!("radroots-proof://sha256/{}", "1".repeat(64)),
        )
        .expect("referenced artifact");
        verify_order_acceptance_proof_artifact_structure(&execution, &artifact)
            .expect("referenced artifact is structurally valid");
    }

    #[cfg(not(feature = "sp1_verify"))]
    #[test]
    fn sp1_verification_apis_report_unavailable_without_sp1_verify_feature() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let bundle =
                generate_order_acceptance_proof(&witness(), RadrootsSp1TradeProofMode::None)
                    .expect("proof bundle");
            let mut inline_receipt =
                validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
            inline_receipt.proof = RadrootsValidationReceiptProof {
                inline_proof_base64: Some("cHJvb2Y=".to_string()),
                mode: Some("core".to_string()),
                program_hash: Some(
                    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ),
                proof_reference: None,
                system: RadrootsValidationReceiptProofSystem::Sp1Core,
                verifying_key_hash: Some(
                    "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                ),
            };
            let err =
                super::verify_order_acceptance_validation_receipt_inline_sp1_proof(&inline_receipt)
                    .await
                    .expect_err("verifier unavailable");
            assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable);

            let execution =
                super::execute_order_acceptance_public_values(&witness()).expect("execution");
            let artifact = super::referenced_order_acceptance_proof_artifact_for_execution(
                &execution,
                RadrootsSp1TradeProofMode::Core,
                format!("radroots-proof://sha256/{}", "1".repeat(64)),
            )
            .expect("referenced artifact");
            let resolved = super::RadrootsSp1TradeResolvedProofArtifact {
                artifact: artifact.clone(),
                resolved_proof_envelope_base64: None,
            };
            let err =
                super::verify_order_acceptance_resolved_sp1_proof_artifact(&execution, &resolved)
                    .await
                    .expect_err("verifier unavailable");
            assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable);

            let mut referenced_receipt =
                validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
            referenced_receipt.proof = RadrootsValidationReceiptProof {
                inline_proof_base64: artifact.inline_proof_base64.clone(),
                mode: artifact.mode.clone(),
                program_hash: artifact.program_hash.clone(),
                proof_reference: artifact.proof_reference.clone(),
                system: artifact.system,
                verifying_key_hash: artifact.verifying_key_hash.clone(),
            };
            let err = super::verify_order_acceptance_validation_receipt_resolved_sp1_proof(
                &referenced_receipt,
                &resolved,
            )
            .await
            .expect_err("verifier unavailable");
            assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProofVerifierUnavailable);
        });
    }

    #[test]
    fn remote_prover_contract_round_trips_provider_neutral_payloads() {
        let request = super::RadrootsSp1TradeRemoteProverRequest {
            schema_version: super::RADROOTS_SP1_TRADE_REMOTE_PROVER_SCHEMA_VERSION,
            request_id: "request-1".to_string(),
            proof_target: RADROOTS_SP1_TRADE_ORDER_ACCEPTANCE_PROOF_TARGET.to_string(),
            proof_mode: RadrootsSp1TradeProofMode::Core,
            sp1_version_line: super::RADROOTS_SP1_TRADE_SP1_VERSION_LINE.to_string(),
            witness: witness(),
            expected_sp1_program_hash:
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            expected_sp1_verifying_key_hash:
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            expected_public_values_hash:
                "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            expected_reducer_program_hash: RADROOTS_SP1_TRADE_REDUCER_PROGRAM_HASH.to_string(),
            expected_protocol_version: RADROOTS_SP1_TRADE_PROTOCOL_VERSION.to_string(),
            expected_witness_version: RADROOTS_SP1_TRADE_WITNESS_VERSION,
        };
        let request_json = serde_json::to_string(&request).expect("request json");
        let decoded_request: super::RadrootsSp1TradeRemoteProverRequest =
            serde_json::from_str(&request_json).expect("decoded request");
        assert_eq!(decoded_request, request);

        let response = super::RadrootsSp1TradeRemoteProverResponse {
            schema_version: super::RADROOTS_SP1_TRADE_REMOTE_PROVER_SCHEMA_VERSION,
            request_id: "request-1".to_string(),
            status: super::RadrootsSp1TradeRemoteProverStatus::Accepted,
            status_url: None,
            status_path: Some("/proofs/request-1".to_string()),
            proof_system: None,
            proof_mode: None,
            public_values_hash: None,
            sp1_program_hash: None,
            sp1_verifying_key_hash: None,
            proof_artifact: None,
            resolved_proof_envelope_base64: None,
            reason_code: None,
            message: None,
            detail: None,
        };
        let response_json = serde_json::to_string(&response).expect("response json");
        let decoded_response: super::RadrootsSp1TradeRemoteProverResponse =
            serde_json::from_str(&response_json).expect("decoded response");
        assert_eq!(decoded_response, response);
        assert_eq!(
            super::RadrootsSp1TradeProverBackend::from_label("remote_http_prove"),
            Some(super::RadrootsSp1TradeProverBackend::RemoteHttpProve)
        );
    }

    #[cfg(feature = "sp1_verify")]
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RemoteReturnedProofFixture {
        schema_version: u32,
        fixture_id: String,
        proof_target: String,
        proof_mode: RadrootsSp1TradeProofMode,
        proof_system: RadrootsValidationReceiptProofSystem,
        sp1_version_line: String,
        remote_prover_request: super::RadrootsSp1TradeRemoteProverRequest,
        remote_prover_response: super::RadrootsSp1TradeRemoteProverResponse,
    }

    #[cfg(feature = "sp1_verify")]
    fn remote_returned_proof_fixture() -> RemoteReturnedProofFixture {
        serde_json::from_str(include_str!(
            "../tests/fixtures/remote_returned_proof_order_acceptance_core_v1.json"
        ))
        .expect("remote returned proof fixture")
    }

    #[cfg(feature = "sp1_verify")]
    fn verified_remote_returned_proof_fixture() -> (
        RadrootsSp1TradePublicValuesExecution,
        super::RadrootsSp1TradeProofArtifact,
    ) {
        let fixture = remote_returned_proof_fixture();
        assert_eq!(fixture.schema_version, 1);
        assert_eq!(
            fixture.fixture_id,
            "remote_returned_proof_order_acceptance_core_v1"
        );
        assert_eq!(
            fixture.proof_target,
            RADROOTS_SP1_TRADE_ORDER_ACCEPTANCE_PROOF_TARGET
        );
        assert_eq!(fixture.proof_mode, RadrootsSp1TradeProofMode::Core);
        assert_eq!(
            fixture.proof_system,
            RadrootsValidationReceiptProofSystem::Sp1Core
        );
        assert_eq!(
            fixture.sp1_version_line,
            super::RADROOTS_SP1_TRADE_SP1_VERSION_LINE
        );

        let request = fixture.remote_prover_request;
        let response = fixture.remote_prover_response;
        let execution = super::execute_order_acceptance_public_values(&request.witness)
            .expect("deterministic execution");
        assert_eq!(
            execution.public_values_hash,
            request.expected_public_values_hash
        );
        assert_eq!(
            execution.public_values.sp1_program_hash.as_deref(),
            Some(request.expected_sp1_program_hash.as_str())
        );
        assert_eq!(
            execution.public_values.sp1_verifying_key_hash.as_deref(),
            Some(request.expected_sp1_verifying_key_hash.as_str())
        );
        assert_eq!(
            response.status,
            super::RadrootsSp1TradeRemoteProverStatus::Completed
        );
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(
            response.public_values_hash.as_deref(),
            Some(request.expected_public_values_hash.as_str())
        );
        assert_eq!(
            response.sp1_program_hash.as_deref(),
            Some(request.expected_sp1_program_hash.as_str())
        );
        assert_eq!(
            response.sp1_verifying_key_hash.as_deref(),
            Some(request.expected_sp1_verifying_key_hash.as_str())
        );

        let artifact = response.proof_artifact.expect("proof artifact");
        verify_order_acceptance_proof_artifact_structure(&execution, &artifact)
            .expect("artifact structure");
        (execution, artifact)
    }

    #[cfg(feature = "sp1_verify")]
    #[test]
    fn remote_returned_proof_artifact_fixture_is_structurally_valid() {
        verified_remote_returned_proof_fixture();
    }

    #[cfg(all(feature = "sp1_verify", radroots_sp1_real_proof_tests))]
    #[tokio::test]
    async fn remote_returned_proof_artifact_real_sp1_verifies() {
        let (execution, artifact) = verified_remote_returned_proof_fixture();
        super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact::inline(artifact.clone()),
        )
        .await
        .expect("remote proof verifies");

        let mut digest_mismatch = artifact.clone();
        digest_mismatch.proof_digest =
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let err = verify_order_acceptance_proof_artifact_structure(&execution, &digest_mismatch)
            .expect_err("digest mismatch");
        assert_eq!(err, RadrootsSp1TradeHostError::ProofDigestMismatch);

        let mut identity_mismatch = execution.clone();
        identity_mismatch.public_values.sp1_program_hash =
            Some("0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string());
        let err = verify_order_acceptance_proof_artifact_structure(&identity_mismatch, &artifact)
            .expect_err("identity mismatch");
        assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProgramHashMismatch);

        let mut public_values_mismatch = request.witness;
        public_values_mismatch.inventory_sequence += 1;
        let mismatch_execution =
            super::execute_order_acceptance_public_values(&public_values_mismatch)
                .expect("mismatch execution");
        let err = verify_order_acceptance_proof_artifact_structure(&mismatch_execution, &artifact)
            .expect_err("public values mismatch");
        assert_eq!(err, RadrootsSp1TradeHostError::PublicValuesHashMismatch);
    }

    #[cfg(feature = "sp1_verify")]
    #[tokio::test]
    async fn resolved_reference_rejects_unresolved_and_digest_mismatch() {
        let execution =
            super::execute_order_acceptance_public_values(&witness()).expect("execution");
        let artifact = super::referenced_order_acceptance_proof_artifact_for_execution(
            &execution,
            RadrootsSp1TradeProofMode::Core,
            format!("radroots-proof://sha256/{}", "1".repeat(64)),
        )
        .expect("referenced artifact");

        let err = super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact {
                artifact: artifact.clone(),
                resolved_proof_envelope_base64: None,
            },
        )
        .await
        .expect_err("unresolved reference");
        assert_eq!(err, RadrootsSp1TradeHostError::Sp1ProofReferenceUnresolved);

        let err = super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact {
                artifact,
                resolved_proof_envelope_base64: Some("cHJvb2Y=".to_string()),
            },
        )
        .await
        .expect_err("digest mismatch");
        assert_eq!(
            err,
            RadrootsSp1TradeHostError::Sp1ProofReferenceDigestMismatch
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

    #[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
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

    #[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
    #[tokio::test]
    async fn expensive_proof_generation_and_verification_is_runnable() {
        let bundle = super::generate_order_acceptance_sp1_proof(
            &witness_without_sp1_identity(),
            RadrootsSp1TradeProofMode::Core,
        )
        .await
        .expect("proof bundle");
        super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &bundle.execution,
            &super::RadrootsSp1TradeResolvedProofArtifact::inline(bundle.proof.clone()),
        )
        .await
        .expect("proof verifies");
        let receipt =
            validation_receipt_for_order_acceptance_proof(&bundle).expect("validation receipt");
        let verification =
            super::verify_order_acceptance_validation_receipt_inline_sp1_proof(&receipt)
                .await
                .expect("receipt proof verifies");
        assert_eq!(
            bundle.proof.system,
            RadrootsValidationReceiptProofSystem::Sp1Core
        );
        assert!(bundle.proof.inline_proof_base64.is_some());
        assert_eq!(verification.proof_mode, RadrootsSp1TradeProofMode::Core);
        assert_eq!(verification.public_values_hash, receipt.public_values_hash);
        assert_eq!(
            verification.sp1_program_hash,
            receipt.proof.program_hash.expect("receipt program hash")
        );
        assert_eq!(
            verification.sp1_verifying_key_hash,
            receipt
                .proof
                .verifying_key_hash
                .expect("receipt verifying key hash")
        );
    }

    #[cfg(all(feature = "sp1_proving", radroots_sp1_guest_elf))]
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
            verifying_key_hash: execution.public_values.sp1_verifying_key_hash.clone(),
        };
        let err = super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact::inline(missing.clone()),
        )
        .await
        .expect_err("missing proof material");
        assert_eq!(err, RadrootsSp1TradeHostError::ProofDigestMismatch);

        missing.proof_digest =
            super::proof_digest_for_execution(&execution, &missing).expect("missing proof digest");
        let err = super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact::inline(missing.clone()),
        )
        .await
        .expect_err("missing proof material");
        assert_eq!(err, RadrootsSp1TradeHostError::MissingProofMaterial);

        let mut synthetic = missing;
        synthetic.inline_proof_base64 =
            Some(base64::engine::general_purpose::STANDARD.encode(b"synthetic proof material"));
        synthetic.proof_digest = super::proof_digest_for_execution(&execution, &synthetic)
            .expect("synthetic proof digest");
        let err = super::verify_order_acceptance_resolved_sp1_proof_artifact(
            &execution,
            &super::RadrootsSp1TradeResolvedProofArtifact::inline(synthetic),
        )
        .await
        .expect_err("synthetic proof material");
        assert!(matches!(
            err,
            RadrootsSp1TradeHostError::Sp1ProofMaterialDecode(_)
        ));
    }
}
