use crate::{ContextError, UnixSeconds};

/// Maximum interval between checked preparation and first signing invocation.
pub const FIRST_ISSUANCE_MAX_SECONDS: u64 = 120;

/// Validated retained timing data. This value observes no clock and grants no
/// permission to sign. Current authority and dispatch must be checked separately;
/// expiry never proves that an already dispatched request remained unissued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IssuanceWindow {
    prepared_at: UnixSeconds,
    first_issue_not_after: UnixSeconds,
}
impl IssuanceWindow {
    /// Validates an explicitly retained, possibly stricter first-issue interval.
    pub fn new(
        prepared_at: UnixSeconds,
        first_issue_not_after: UnixSeconds,
    ) -> Result<Self, ContextError> {
        if prepared_at.get() == 0
            || first_issue_not_after <= prepared_at
            || first_issue_not_after.get() - prepared_at.get() > FIRST_ISSUANCE_MAX_SECONDS
        {
            return Err(ContextError::OutOfRange);
        }
        Ok(Self {
            prepared_at,
            first_issue_not_after,
        })
    }
    /// Captures the default interval using the host's checked observation and
    /// exact authorization deadline. Neither input is independently trusted here.
    pub fn capture(
        prepared_at: UnixSeconds,
        authorization_not_after: UnixSeconds,
    ) -> Result<Self, ContextError> {
        let ceiling = prepared_at
            .get()
            .checked_add(FIRST_ISSUANCE_MAX_SECONDS)
            .ok_or(ContextError::OutOfRange)?;
        let deadline = UnixSeconds::new(core::cmp::min(ceiling, authorization_not_after.get()))?;
        Self::new(prepared_at, deadline)
    }
    /// Returns the original checked preparation observation.
    pub const fn prepared_at(self) -> UnixSeconds {
        self.prepared_at
    }
    /// Returns the exclusive first-issue bound; retries must never extend it.
    pub const fn first_issue_not_after(self) -> UnixSeconds {
        self.first_issue_not_after
    }
}
