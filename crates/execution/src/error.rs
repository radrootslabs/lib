use core::fmt;
use radroots_domain::OperationId;

use crate::Detail;

macro_rules! codes {
    ($($variant:ident => ($code:literal, $message:literal, $retained:literal)),+ $(,)?) => {
        /// A stable execution failure category.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum Code {
            $(#[doc = $message] $variant,)+
        }

        impl Code {
            /// Every supported execution failure category.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Returns the stable machine code.
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code,)+ }
            }

            /// Returns an input-free bounded human message.
            pub const fn message(self) -> &'static str {
                match self { $(Self::$variant => $message,)+ }
            }

            const fn requires_reconciliation(self) -> bool {
                match self { $(Self::$variant => $retained,)+ }
            }
        }
    };
}

codes! {
    InvalidInput => ("invalid_input", "Command input is invalid", false),
    Unauthenticated => ("unauthenticated", "Authentication is required", false),
    Unauthorized => ("unauthorized", "The action is not authorized", false),
    GrantRevoked => ("grant_revoked", "The authorizing grant was revoked", false),
    MissingAuthority => ("missing_authority", "Selected execution authority is missing", false),
    StaleState => ("stale_state", "Expected state must be refreshed and approved", false),
    DeadlineExceeded => ("deadline_exceeded", "The captured first-issue deadline has elapsed", false),
    MissingSource => ("missing_source", "Required source evidence is missing", false),
    MissingHistory => ("missing_history", "Required authenticated history is missing", false),
    UnsupportedVersion => ("unsupported_version", "The version is unsupported", false),
    UnsupportedProfile => ("unsupported_profile", "The profile is unsupported", false),
    CapabilityUnavailable => ("capability_unavailable", "A required capability is unavailable", false),
    IdempotencyConflict => ("idempotency_conflict", "The operation identity binds different command inputs", false),
    InsufficientStock => ("insufficient_stock", "Available stock does not satisfy the command", false),
    Deficit => ("deficit", "Stock reconciliation is required", false),
    AuthorityConflict => ("authority_conflict", "Execution authority is conflicted", false),
    RecordFork => ("record_fork", "Authenticated records contain a causal fork", false),
    StorageFailure => ("storage_failure", "Storage reported a known failure", false),
    CommitUnknown => ("commit_unknown", "The retained transaction outcome requires reconciliation", true),
    RecoveryRequired => ("recovery_required", "Explicit recovery is required", false),
    SignerRefused => ("signer_refused", "The signer refused the request", false),
    SignerUnavailable => ("signer_unavailable", "The signer is unavailable", false),
    SignerTampering => ("signer_tampering", "The signer result does not match the retained intent", true),
    IdentityChanged => ("identity_changed", "The signer identity changed", false),
    SigningPending => ("signing_pending", "The retained signing request is pending", true),
    SigningUnknown => ("signing_unknown", "The retained signing outcome requires reconciliation", true),
    RetentionExhausted => ("retention_exhausted", "Protected retention capacity is exhausted", false),
    DeliveryPending => ("delivery_pending", "The retained delivery requires continuation", true),
}

/// How an observer may continue after the reported failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RetryClass {
    /// Do not retry this command.
    Never,
    /// Resume or reconcile the exact retained operation, without re-preparing it.
    SafeSameOperation,
    /// Refresh state and obtain any required new approval before a new command.
    AfterStateRefresh,
    /// Require explicit reconciliation by the operator.
    ManualReconciliation,
}

impl RetryClass {
    /// Returns the canonical machine retry class.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::SafeSameOperation => "safe_same_operation",
            Self::AfterStateRefresh => "after_state_refresh",
            Self::ManualReconciliation => "manual_reconciliation",
        }
    }
}

/// What is known about durable command admission, independently of a later
/// signing, installation or delivery result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Admission {
    /// This command has not been durably admitted.
    NotAdmitted,
    /// The executor confirmed durable admission of the original command.
    Confirmed,
    /// The initial admission commit is uncertain and must be queried by ID.
    Unknown,
}

/// An execution report's operation identity and durable-admission disposition.
///
/// This value is neither authorization nor evidence that a transaction ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationContext {
    operation_id: Option<OperationId>,
    admission: Admission,
}

impl OperationContext {
    /// Reports a failure before durable admission, optionally retaining the
    /// caller's proposed operation ID.
    pub const fn proposed(operation_id: Option<OperationId>) -> Self {
        Self {
            operation_id,
            admission: Admission::NotAdmitted,
        }
    }

    /// Reports the original identity after the executor has confirmed durable
    /// admission. Constructing this report does not perform admission.
    pub const fn retained(operation_id: OperationId) -> Self {
        Self {
            operation_id: Some(operation_id),
            admission: Admission::Confirmed,
        }
    }

    /// Reports uncertain initial admission. The original ID is the query
    /// anchor; neither successful admission nor a known failed commit is claimed.
    pub const fn admission_unknown(operation_id: OperationId) -> Self {
        Self {
            operation_id: Some(operation_id),
            admission: Admission::Unknown,
        }
    }

    /// Returns the proposed or retained operation identity.
    pub const fn operation_id(self) -> Option<OperationId> {
        self.operation_id
    }

    /// Returns the explicit admission disposition.
    pub const fn admission(self) -> Admission {
        self.admission
    }
}

/// An internally inconsistent execution error report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InvalidErrorContext {
    /// Signing and delivery require confirmed durable preparation.
    ConfirmedAdmissionRequired,
    /// Commit uncertainty requires an original retained or uncertain identity.
    OperationIdentityRequired,
    /// Same-operation continuation requires retained or uncertain admission.
    RetainedRetryRequired,
    /// An uncertain or pending outcome must retain reconciliation semantics.
    ReconciliationRequired,
    /// Conflicting inputs cannot resume the operation that bound other inputs.
    ConflictingRetry,
}

impl fmt::Display for InvalidErrorContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ConfirmedAdmissionRequired => "outcome requires confirmed admission",
            Self::OperationIdentityRequired => "outcome requires an original operation identity",
            Self::RetainedRetryRequired => "retry requires retained or uncertain admission",
            Self::ReconciliationRequired => "outcome requires retained reconciliation",
            Self::ConflictingRetry => "conflicting inputs cannot resume the retained command",
        })
    }
}

impl core::error::Error for InvalidErrorContext {}

/// An allocation-free error with typed code, bounded redacted details and the
/// exact reported operation identity. It never owns backend messages or causes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    code: Code,
    context: OperationContext,
    retry: RetryClass,
    detail: Option<Detail>,
}

impl Error {
    #[cfg(feature = "workspace_session")]
    pub(crate) const fn with_context(mut self, context: OperationContext) -> Self {
        self.context = context;
        self
    }

    #[cfg(feature = "workspace_session")]
    pub(crate) const fn admission_uncertain(operation: OperationId) -> Self {
        Self {
            code: Code::CommitUnknown,
            context: OperationContext::admission_unknown(operation),
            retry: RetryClass::ManualReconciliation,
            detail: Some(Detail::new(crate::Field::Storage, crate::Reason::Unknown)),
        }
    }

    #[cfg(feature = "workspace_session")]
    /// Projects a typed journal refusal with its observed admission context.
    /// This data conversion performs no transaction or authorization check.
    pub const fn operation(
        error: radroots_storage::command::OperationError,
        context: OperationContext,
    ) -> Self {
        use radroots_storage::command::OperationError as O;
        let (code, retry) = match error {
            O::Workspace(error) => {
                return Self::workspace(error, context.operation_id()).with_context(context);
            }
            O::InvalidCommand => (Code::InvalidInput, RetryClass::Never),
            O::InvalidEvidence => (Code::RecoveryRequired, RetryClass::ManualReconciliation),
            O::Unauthorized | O::UnverifiedApproval => (Code::Unauthorized, RetryClass::Never),
            O::UnsupportedOperation => (Code::CapabilityUnavailable, RetryClass::Never),
            O::StaleState => (Code::StaleState, RetryClass::AfterStateRefresh),
            O::MissingHistory => (Code::MissingHistory, RetryClass::AfterStateRefresh),
            O::DeadlineExceeded => (Code::DeadlineExceeded, RetryClass::AfterStateRefresh),
            O::IdempotencyConflict => (Code::IdempotencyConflict, RetryClass::Never),
            O::StorageFailure => (Code::StorageFailure, RetryClass::Never),
            O::RecoveryRequired | O::DeliveryAttemptLimit => {
                (Code::RecoveryRequired, RetryClass::ManualReconciliation)
            }
            _ => (Code::RecoveryRequired, RetryClass::ManualReconciliation),
        };
        Self {
            code,
            context,
            retry,
            detail: None,
        }
    }
    #[cfg(feature = "receipts")]
    pub(crate) const fn retained_deadline_exceeded(operation: OperationId) -> Self {
        Self {
            code: Code::DeadlineExceeded,
            context: OperationContext::retained(operation),
            retry: RetryClass::ManualReconciliation,
            detail: Some(Detail::new(
                crate::Field::Operation,
                crate::Reason::OutOfRange,
            )),
        }
    }
    #[cfg(feature = "receipts")]
    pub(crate) const fn retained_signing_unknown(operation: OperationId) -> Self {
        Self {
            code: Code::SigningUnknown,
            context: OperationContext::retained(operation),
            retry: RetryClass::ManualReconciliation,
            detail: Some(Detail::new(crate::Field::Signer, crate::Reason::Unknown)),
        }
    }
    #[cfg(feature = "workspace_session")]
    pub(crate) const fn workspace(
        error: radroots_storage::workspace::WorkspaceError,
        operation: Option<OperationId>,
    ) -> Self {
        use crate::{Field as F, Reason as R};
        use radroots_storage::workspace::WorkspaceError as W;
        let (code, retry, field, reason) = match error {
            W::InvalidInput => (
                Code::InvalidInput,
                RetryClass::Never,
                F::Input,
                R::OutOfRange,
            ),
            W::Unauthorized => (
                Code::Unauthorized,
                RetryClass::Never,
                F::Authority,
                R::Missing,
            ),
            W::GrantExpired => (
                Code::Unauthorized,
                RetryClass::AfterStateRefresh,
                F::Authority,
                R::OutOfRange,
            ),
            W::GrantRevoked => (
                Code::GrantRevoked,
                RetryClass::AfterStateRefresh,
                F::Authority,
                R::Conflict,
            ),
            W::WriterActive => (
                Code::MissingAuthority,
                RetryClass::AfterStateRefresh,
                F::Authority,
                R::Conflict,
            ),
            W::WrongNamespace => (
                Code::Unauthorized,
                RetryClass::Never,
                F::Authority,
                R::Conflict,
            ),
            W::StaleAuthority => (
                Code::StaleState,
                RetryClass::AfterStateRefresh,
                F::Authority,
                R::Conflict,
            ),
            W::StaleCheckpoint => (
                Code::StaleState,
                RetryClass::AfterStateRefresh,
                F::History,
                R::Conflict,
            ),
            W::Unavailable => (
                Code::CapabilityUnavailable,
                RetryClass::Never,
                F::Authority,
                R::Unavailable,
            ),
            W::Closed => (
                Code::CapabilityUnavailable,
                RetryClass::Never,
                F::Operation,
                R::Unavailable,
            ),
            W::StorageFailure => (
                Code::StorageFailure,
                RetryClass::Never,
                F::Storage,
                R::Unavailable,
            ),
            W::NamespaceUnavailable => (
                Code::RecoveryRequired,
                RetryClass::ManualReconciliation,
                F::Storage,
                R::Unavailable,
            ),
            // Attachment/context are pre-admission. An adapter's uncertain
            // mutation cannot be reported as this command's confirmed admission.
            W::RecoveryRequired | W::CommitUnknown => (
                Code::RecoveryRequired,
                RetryClass::ManualReconciliation,
                F::Storage,
                R::Unknown,
            ),
            _ => (
                Code::RecoveryRequired,
                RetryClass::ManualReconciliation,
                F::Authority,
                R::Unknown,
            ),
        };
        Self {
            code,
            context: OperationContext::proposed(operation),
            retry,
            detail: Some(Detail::new(field, reason)),
        }
    }
    #[cfg(feature = "client_session")]
    pub(crate) const fn identity_changed() -> Self {
        Self {
            code: Code::IdentityChanged,
            context: OperationContext::proposed(None),
            retry: RetryClass::Never,
            detail: Some(Detail::new(crate::Field::Signer, crate::Reason::Conflict)),
        }
    }

    #[cfg(feature = "signer_session")]
    pub(crate) const fn signer_unavailable() -> Self {
        Self {
            code: Code::SignerUnavailable,
            context: OperationContext::proposed(None),
            retry: RetryClass::Never,
            detail: Some(Detail::new(
                crate::Field::Signer,
                crate::Reason::Unavailable,
            )),
        }
    }

    #[cfg(feature = "session")]
    pub(crate) const fn session_unavailable(reason: crate::Reason) -> Self {
        Self {
            code: Code::CapabilityUnavailable,
            context: OperationContext::proposed(None),
            retry: RetryClass::Never,
            detail: Some(Detail::new(crate::Field::Operation, reason)),
        }
    }

    /// Validates the relationship between the outcome, durable identity and
    /// retry policy. Safe same-operation continuation never means blind reissue.
    pub const fn new(
        code: Code,
        context: OperationContext,
        retry: RetryClass,
        detail: Option<Detail>,
    ) -> core::result::Result<Self, InvalidErrorContext> {
        if matches!(
            code,
            Code::SigningPending
                | Code::SigningUnknown
                | Code::DeliveryPending
                | Code::SignerTampering
        ) && !matches!(context.admission, Admission::Confirmed)
        {
            return Err(InvalidErrorContext::ConfirmedAdmissionRequired);
        }
        if matches!(code, Code::CommitUnknown)
            && matches!(context.admission, Admission::NotAdmitted)
        {
            return Err(InvalidErrorContext::OperationIdentityRequired);
        }
        if matches!(retry, RetryClass::SafeSameOperation)
            && matches!(context.admission, Admission::NotAdmitted)
        {
            return Err(InvalidErrorContext::RetainedRetryRequired);
        }
        if code.requires_reconciliation()
            && !matches!(
                retry,
                RetryClass::SafeSameOperation | RetryClass::ManualReconciliation
            )
        {
            return Err(InvalidErrorContext::ReconciliationRequired);
        }
        if matches!(code, Code::IdempotencyConflict)
            && matches!(retry, RetryClass::SafeSameOperation)
        {
            return Err(InvalidErrorContext::ConflictingRetry);
        }
        Ok(Self {
            code,
            context,
            retry,
            detail,
        })
    }

    /// Returns the stable error category.
    pub const fn code(self) -> Code {
        self.code
    }

    /// Returns the operation identity and admission disposition.
    pub const fn context(self) -> OperationContext {
        self.context
    }

    /// Returns the required continuation class.
    pub const fn retry(self) -> RetryClass {
        self.retry
    }

    /// Returns the optional fixed diagnostic.
    pub const fn detail(self) -> Option<Detail> {
        self.detail
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.code.message())
    }
}

impl core::error::Error for Error {}
