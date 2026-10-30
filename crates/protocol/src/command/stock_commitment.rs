//! Closed common capture for the one stock commitment execution pipeline.
use super::{
    AllocationSelection, CapturedOrderAcceptance, CapturedTermsInstallation, CommandError,
    check_frame, parse_object,
};
use alloc::string::String;
use core::fmt;
use radroots_domain::{
    ApprovalIntentDigest, CommandContextData, CommandDigest, RecordId, TermsDigest,
};
use serde::Deserialize;

/// Exact known stock commitment commands. These are captured data, never
/// execution, signing, consent, installed-state or allocation capabilities.
pub enum CapturedStockCommitment {
    /// Initial buyer-request acceptance.
    Acceptance(CapturedOrderAcceptance),
    /// Installation of exact mutually authorized complete proposed terms.
    TermsInstallation(CapturedTermsInstallation),
}
impl From<CapturedOrderAcceptance> for CapturedStockCommitment {
    fn from(value: CapturedOrderAcceptance) -> Self {
        Self::Acceptance(value)
    }
}
impl From<CapturedTermsInstallation> for CapturedStockCommitment {
    fn from(value: CapturedTermsInstallation) -> Self {
        Self::TermsInstallation(value)
    }
}
impl CapturedStockCommitment {
    /// Dispatches only to the corresponding exact bounded closed parser.
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        #[derive(Deserialize)]
        struct Probe {
            #[serde(rename = "type")]
            kind: String,
        }
        let probe: Probe = parse_object(raw)?;
        match probe.kind.as_str() {
            "orders.accept" => CapturedOrderAcceptance::parse(raw).map(Self::Acceptance),
            "orders.install_terms" => {
                CapturedTermsInstallation::parse(raw).map(Self::TermsInstallation)
            }
            _ => Err(CommandError::UnsupportedCommand),
        }
    }
    /// Exact command scope; not proof that its approval is genuine/current.
    pub const fn command_name(&self) -> &'static str {
        match self {
            Self::Acceptance(_) => "orders.accept",
            Self::TermsInstallation(_) => "orders.install_terms",
        }
    }
    /// Captured context data; current authority must be independently checked.
    pub const fn context(&self) -> &CommandContextData {
        match self {
            Self::Acceptance(value) => value.context(),
            Self::TermsInstallation(value) => value.context(),
        }
    }
    /// Exact normalized protected command bytes.
    pub fn protected_wire(&self) -> &[u8] {
        match self {
            Self::Acceptance(value) => value.protected_wire(),
            Self::TermsInstallation(value) => value.protected_wire(),
        }
    }
    /// Command identity bound to its exact semantic input and approval.
    pub const fn digest(&self) -> CommandDigest {
        match self {
            Self::Acceptance(value) => value.digest(),
            Self::TermsInstallation(value) => value.digest(),
        }
    }
    /// Displayed intent identity before approval and observer correlation.
    pub const fn approval_intent_digest(&self) -> ApprovalIntentDigest {
        match self {
            Self::Acceptance(value) => value.approval_intent_digest(),
            Self::TermsInstallation(value) => value.approval_intent_digest(),
        }
    }
    /// Exact buyer request or terms authorization selected by this command.
    pub const fn authorization(&self) -> RecordId {
        match self {
            Self::Acceptance(value) => value.input().request(),
            Self::TermsInstallation(value) => value.input().authorization(),
        }
    }
    /// Exact proposal when the command installs mutually authorized terms.
    pub const fn proposal(&self) -> Option<RecordId> {
        match self {
            Self::Acceptance(_) => None,
            Self::TermsInstallation(value) => Some(value.input().proposal()),
        }
    }
    /// Expected complete terms digest; source recomputation belongs to execution.
    pub const fn terms_digest(&self) -> TermsDigest {
        match self {
            Self::Acceptance(value) => value.input().terms_digest(),
            Self::TermsInstallation(value) => value.input().terms_digest(),
        }
    }
    /// Bounded allocation selection data for native planning.
    pub const fn allocation(&self) -> &AllocationSelection {
        match self {
            Self::Acceptance(value) => value.input().allocation(),
            Self::TermsInstallation(value) => value.input().allocation(),
        }
    }
}
impl fmt::Debug for CapturedStockCommitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedStockCommitment(<redacted>)")
    }
}
