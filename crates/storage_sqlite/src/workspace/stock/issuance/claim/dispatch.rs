//! The fresh native permit owns its one original exact-issuance invocation.
use super::{OperationError, StockDispatchPermit, StockSigningAttempt};
use radroots_signing::{
    issuance::{ExactIssuer, OutcomeKind, RejectedArtifact, SignedArtifact},
    provider::ProviderError,
    request::CancellationSignal,
    signer::BoxFuture,
};

/// Live observation from the owned original invocation on a qualified issuer.
/// Private construction prevents caller reports or later recovery refusals from
/// becoming this observation. It is neither durable evidence nor an operation-
/// wide/global nonissuance guarantee, and cannot release any stock by itself.
///
/// ```compile_fail
/// use radroots_storage_sqlite::workspace::{StockSigningAttempt, StockUnissuedInvocation};
/// fn forge(attempt: StockSigningAttempt) -> StockUnissuedInvocation {
///     StockUnissuedInvocation { attempt, reason: radroots_signing::provider::ProviderError::Cancelled }
/// }
/// ```
/// ```compile_fail
/// use radroots_storage_sqlite::workspace::StockUnissuedInvocation;
/// fn duplicate(value: StockUnissuedInvocation) -> (StockUnissuedInvocation, StockUnissuedInvocation) {
///     (value.clone(), value)
/// }
/// ```
pub struct StockUnissuedInvocation {
    attempt: StockSigningAttempt,
    reason: ProviderError,
}
impl StockUnissuedInvocation {
    pub const fn attempt(&self) -> &StockSigningAttempt {
        &self.attempt
    }
    pub const fn reason(&self) -> ProviderError {
        self.reason
    }
}
impl core::fmt::Debug for StockUnissuedInvocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockUnissuedInvocation(<redacted>)")
    }
}

/// Independent original invocation facts. None implies durable S or T2.
#[derive(Debug)]
pub enum StockDispatchOutcome {
    Obtained {
        attempt: StockSigningAttempt,
        artifact: SignedArtifact,
    },
    Unissued(StockUnissuedInvocation),
    Unknown {
        attempt: StockSigningAttempt,
        reason: ProviderError,
    },
    Rejected {
        attempt: StockSigningAttempt,
        rejection: RejectedArtifact,
    },
}
impl StockDispatchPermit {
    /// Consumes the actual newly confirmed permit and invokes this issuer once.
    /// The selected issuer must be qualified for its reported outcomes. Losing
    /// this wait retains the durable claim's uncertainty; no second permit is
    /// created. Exact recovery never enters this method. Caller-provided outcome
    /// values cannot construct its original unissued observation.
    pub fn dispatch(
        self,
        issuer: &dyn ExactIssuer,
        cancellation: CancellationSignal,
    ) -> BoxFuture<'_, Result<StockDispatchOutcome, OperationError>> {
        Box::pin(async move {
            let (intent, attempt) = self.into_signing_intent(cancellation)?;
            let binding = attempt.binding();
            let (_, kind) = issuer
                .issue_exact(intent)
                .await
                .enforce_binding(binding)
                .into_parts();
            Ok(match kind {
                OutcomeKind::Obtained(artifact) => {
                    StockDispatchOutcome::Obtained { attempt, artifact }
                }
                OutcomeKind::NotDispatched(reason) => {
                    StockDispatchOutcome::Unissued(StockUnissuedInvocation { attempt, reason })
                }
                OutcomeKind::Unknown(reason) => StockDispatchOutcome::Unknown { attempt, reason },
                OutcomeKind::Rejected(rejection) => {
                    StockDispatchOutcome::Rejected { attempt, rejection }
                }
                _ => StockDispatchOutcome::Unknown {
                    attempt,
                    reason: ProviderError::ProviderFailure,
                },
            })
        })
    }
}
