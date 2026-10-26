//! Original obtained bytes outlive any fallible storage-evidence conversion.
use radroots_domain::OperationId;
use radroots_signing::issuance::SignedArtifact;
use radroots_storage::command::{ArtifactEvidence, OperationError, SigningAttempt};
use std::sync::OnceLock;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "command_obtained_tests.rs"]
mod tests;

pub(super) struct Obtained {
    attempt: SigningAttempt,
    artifact: SignedArtifact,
    evidence: OnceLock<Result<ArtifactEvidence, OperationError>>,
}
impl Obtained {
    pub(super) fn new(attempt: SigningAttempt, artifact: SignedArtifact) -> Self {
        Self {
            attempt,
            artifact,
            evidence: OnceLock::new(),
        }
    }
    pub(super) fn operation(&self) -> OperationId {
        self.attempt.identity().operation_id()
    }
    pub(super) fn evidence(&self) -> Result<&ArtifactEvidence, OperationError> {
        self.evidence
            .get_or_init(|| {
                self.artifact.with_json(|raw| {
                    ArtifactEvidence::verify(
                        &self.attempt,
                        raw,
                        self.artifact.observed_at_unix_ms(),
                    )
                })
            })
            .as_ref()
            .map_err(|error| *error)
    }
}
