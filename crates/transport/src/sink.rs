//! Outbound event delivery SPI and bounded request models.

use crate::{
    Error,
    outcome::{DeliveryOutcome, Retryability, validate_delivery_code, validate_delivery_message},
    policy::{SatisfactionPolicy, SatisfactionState, evaluate_satisfaction},
    source::BoxFuture,
    target::{Target, TargetSet},
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};
use radroots_event::SignedEvent;

pub use crate::status::SinkStatus;

/// Maximum encoded delivery request identity length.
pub const DELIVERY_REQUEST_ID_MAX_BYTES: usize = 256;

/// Sink-wide typed failure retaining safe retry and partial-target evidence.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinkFailure {
    request_id: DeliveryRequestId,
    target_set: TargetSet,
    code: String,
    retryability: Retryability,
    retry_after_unix_ms: Option<u64>,
    message: Option<String>,
    partial_evidence: Vec<DeliveryTargetReceipt>,
}

/// Validated caller identity for one delivery operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeliveryRequestId(String);

impl DeliveryRequestId {
    /// Parses a non-empty, bounded, printable request identity.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::EmptyDeliveryRequestId);
        }
        if value.len() > DELIVERY_REQUEST_ID_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidDeliveryRequestId);
        }
        Ok(Self(value))
    }

    /// Returns the validated request identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Transport-neutral outbound event payload.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryPayload {
    event: SignedEvent,
}

impl DeliveryPayload {
    /// Wraps an ID-checked signed event for delivery.
    pub const fn new(event: SignedEvent) -> Self {
        Self { event }
    }

    /// Returns the signed event. Signature verification remains a caller concern.
    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
}

/// Bounded multi-target delivery request.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryRequest {
    request_id: DeliveryRequestId,
    payload: DeliveryPayload,
    target_set: TargetSet,
    satisfaction: SatisfactionPolicy,
    deadline_unix_ms: u64,
}

impl DeliveryRequest {
    /// Creates and validates one explicit delivery request.
    pub fn new(
        request_id: impl Into<String>,
        payload: DeliveryPayload,
        target_set: TargetSet,
        satisfaction: SatisfactionPolicy,
        deadline_unix_ms: u64,
    ) -> Result<Self, Error> {
        if deadline_unix_ms == 0 {
            return Err(Error::InvalidDeliveryDeadline);
        }
        satisfaction.validate_for(&target_set)?;
        Ok(Self {
            request_id: DeliveryRequestId::parse(request_id)?,
            payload,
            target_set,
            satisfaction,
            deadline_unix_ms,
        })
    }

    /// Returns the request identity.
    pub const fn request_id(&self) -> &DeliveryRequestId {
        &self.request_id
    }

    /// Returns the signed event payload.
    pub const fn payload(&self) -> &DeliveryPayload {
        &self.payload
    }

    /// Returns the exact non-empty target set.
    pub const fn target_set(&self) -> &TargetSet {
        &self.target_set
    }

    /// Returns the requested success and target policy.
    pub const fn satisfaction(&self) -> &SatisfactionPolicy {
        &self.satisfaction
    }

    /// Returns the absolute Unix deadline in milliseconds.
    pub const fn deadline_unix_ms(&self) -> u64 {
        self.deadline_unix_ms
    }
}

/// Normalized result for one requested target.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryTargetReceipt {
    target: Target,
    attempted: bool,
    outcome: DeliveryOutcome,
}

impl DeliveryTargetReceipt {
    /// Records the outcome of an attempted target.
    pub const fn attempted(target: Target, outcome: DeliveryOutcome) -> Self {
        Self {
            target,
            attempted: true,
            outcome,
        }
    }

    /// Records an unattempted target and its normalized failure reason.
    pub fn skipped(target: Target, outcome: DeliveryOutcome) -> Result<Self, Error> {
        if outcome.satisfies(crate::policy::SatisfactionClass::Accepted) {
            return Err(Error::DeliveryTargetReceiptAttemptMismatch);
        }
        Ok(Self {
            target,
            attempted: false,
            outcome,
        })
    }

    /// Returns the exact target.
    pub const fn target(&self) -> &Target {
        &self.target
    }

    /// Whether the adapter attempted remote publication.
    pub const fn was_attempted(&self) -> bool {
        self.attempted
    }

    /// Returns normalized target outcome data.
    pub const fn outcome(&self) -> &DeliveryOutcome {
        &self.outcome
    }

    fn validate(&self) -> Result<(), Error> {
        self.outcome.validate()?;
        if !self.attempted
            && self
                .outcome
                .satisfies(crate::policy::SatisfactionClass::Accepted)
        {
            return Err(Error::DeliveryTargetReceiptAttemptMismatch);
        }
        Ok(())
    }
}

/// Request-bound per-target delivery receipt.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryReceipt {
    request_id: DeliveryRequestId,
    target_set: TargetSet,
    target_receipts: Vec<DeliveryTargetReceipt>,
}

impl DeliveryReceipt {
    /// Creates a complete result set in the original request target order.
    pub fn for_request(
        request: &DeliveryRequest,
        target_receipts: Vec<DeliveryTargetReceipt>,
    ) -> Result<Self, Error> {
        Self::new(
            request.request_id.clone(),
            request.target_set.clone(),
            target_receipts,
        )
    }

    fn new(
        request_id: DeliveryRequestId,
        target_set: TargetSet,
        target_receipts: Vec<DeliveryTargetReceipt>,
    ) -> Result<Self, Error> {
        let mut by_fingerprint = BTreeMap::new();
        for receipt in target_receipts {
            receipt.validate()?;
            let fingerprint = receipt.target().fingerprint().as_str().to_string();
            if !target_set
                .targets()
                .iter()
                .any(|target| target.fingerprint().as_str() == fingerprint)
            {
                return Err(Error::UnexpectedDeliveryTargetReceipt);
            }
            if by_fingerprint.insert(fingerprint, receipt).is_some() {
                return Err(Error::DuplicateDeliveryTargetReceipt);
            }
        }

        let mut ordered = Vec::with_capacity(target_set.len());
        for target in target_set.targets() {
            let Some(receipt) = by_fingerprint.remove(target.fingerprint().as_str()) else {
                return Err(Error::MissingDeliveryTargetReceipt);
            };
            ordered.push(receipt);
        }
        Ok(Self {
            request_id,
            target_set,
            target_receipts: ordered,
        })
    }

    /// Validates the receipt against the exact request identity and targets.
    pub fn validate_for_request(&self, request: &DeliveryRequest) -> Result<(), Error> {
        if &self.request_id != request.request_id() {
            return Err(Error::DeliveryReceiptRequestIdMismatch);
        }
        if self.target_set != *request.target_set() {
            return Err(Error::DeliveryReceiptTargetSetMismatch);
        }
        let rebuilt = Self::for_request(request, self.target_receipts.clone())?;
        if rebuilt != *self {
            return Err(Error::DeliveryReceiptTargetSetMismatch);
        }
        Ok(())
    }

    /// Returns whether the receipt satisfies the request's exact policy.
    pub fn is_satisfied(&self, request: &DeliveryRequest) -> Result<bool, Error> {
        Ok(matches!(
            self.satisfaction(request)?,
            SatisfactionState::Satisfied
        ))
    }

    /// Evaluates this receipt as satisfied, pending, or exhausted.
    pub fn satisfaction(&self, request: &DeliveryRequest) -> Result<SatisfactionState, Error> {
        self.validate_for_request(request)?;
        evaluate_satisfaction(
            request.satisfaction(),
            request.target_set(),
            self.target_receipts
                .iter()
                .map(|receipt| (receipt.target().fingerprint(), receipt.outcome())),
        )
    }

    /// Returns the request identity.
    pub const fn request_id(&self) -> &DeliveryRequestId {
        &self.request_id
    }

    /// Returns per-target results in request order.
    pub fn target_receipts(&self) -> &[DeliveryTargetReceipt] {
        self.target_receipts.as_slice()
    }
}

impl SinkFailure {
    /// Creates a request-bound sink-wide failure with validated partial evidence.
    pub fn for_request(
        request: &DeliveryRequest,
        code: impl Into<String>,
        retryability: Retryability,
        retry_after_unix_ms: Option<u64>,
        message: Option<String>,
        partial_evidence: Vec<DeliveryTargetReceipt>,
    ) -> Result<Self, Error> {
        let failure = Self {
            request_id: request.request_id.clone(),
            target_set: request.target_set.clone(),
            code: code.into(),
            retryability,
            retry_after_unix_ms,
            message,
            partial_evidence,
        };
        failure.validate_for_request(request)?;
        Ok(failure)
    }

    /// Returns a terminal adapter-contract failure for an exact request.
    pub fn invalid_contract(request: &DeliveryRequest) -> Self {
        Self::for_request(
            request,
            "invalid_transport_contract",
            Retryability::Terminal,
            None,
            Some("transport adapter returned invalid evidence".to_string()),
            Vec::new(),
        )
        .expect("static sink failure is valid")
    }

    /// Validates identity, retry timing, and bounded partial evidence.
    pub fn validate_for_request(&self, request: &DeliveryRequest) -> Result<(), Error> {
        if self.request_id != *request.request_id() {
            return Err(Error::DeliveryReceiptRequestIdMismatch);
        }
        if self.target_set != *request.target_set() {
            return Err(Error::DeliveryReceiptTargetSetMismatch);
        }
        validate_delivery_code(self.code.as_str())?;
        if matches!(self.retryability, Retryability::NotApplicable)
            || matches!(self.retry_after_unix_ms, Some(0))
            || (self.retry_after_unix_ms.is_some()
                && !matches!(self.retryability, Retryability::Retryable))
        {
            return Err(Error::InvalidDeliveryOutcome);
        }
        if let Some(message) = &self.message {
            validate_delivery_message(message)?;
        }
        let mut observed = BTreeSet::new();
        for receipt in &self.partial_evidence {
            receipt.validate()?;
            if !request
                .target_set()
                .contains(receipt.target().fingerprint())
            {
                return Err(Error::UnexpectedDeliveryTargetReceipt);
            }
            if !observed.insert(receipt.target().fingerprint().as_str()) {
                return Err(Error::DuplicateDeliveryTargetReceipt);
            }
        }
        Ok(())
    }

    /// Returns the stable normalized failure code.
    pub fn code(&self) -> &str {
        self.code.as_str()
    }

    /// Returns whether retrying the same request may be useful.
    pub const fn retryability(&self) -> Retryability {
        self.retryability
    }

    /// Returns the earliest absolute Unix millisecond retry time, when supplied.
    pub const fn retry_after_unix_ms(&self) -> Option<u64> {
        self.retry_after_unix_ms
    }

    /// Returns bounded caller-safe diagnostic detail.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns safe target evidence collected before the sink-wide failure.
    pub fn partial_evidence(&self) -> &[DeliveryTargetReceipt] {
        self.partial_evidence.as_slice()
    }
}

/// Host SPI for outbound event delivery.
///
/// This trait supports external implementations and is dyn-compatible. Its
/// futures are `Send`; implementations must not borrow request data after a
/// future completes. `status` observes sink state and does not initiate
/// delivery. `deliver` performs only the attempts authorized by its request,
/// returns partial success per target, and owns no hidden retry loop.
///
/// Dropping a returned future requests cancellation. If it is dropped before
/// a remote request is published, the implementation must leave no remote
/// operation behind. Once publication may have occurred, cancellation cannot
/// claim rollback; a later observation may report the remote outcome. An
/// explicit request deadline bounds work independently of future cancellation.
pub trait EventSink: Send + Sync {
    /// Returns the sink's current runtime status.
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>>;

    /// Delivers an event according to the request's bounded target policy.
    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>>;
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;

    impl serde::Serialize for DeliveryRequestId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> serde::Deserialize<'de> for DeliveryRequestId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = <String as serde::Deserialize>::deserialize(deserializer)?;
            Self::parse(value).map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeliveryRequestWire {
        request_id: String,
        payload: DeliveryPayload,
        target_set: TargetSet,
        satisfaction: SatisfactionPolicy,
        deadline_unix_ms: u64,
    }

    impl<'de> serde::Deserialize<'de> for DeliveryRequest {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = DeliveryRequestWire::deserialize(deserializer)?;
            Self::new(
                wire.request_id,
                wire.payload,
                wire.target_set,
                wire.satisfaction,
                wire.deadline_unix_ms,
            )
            .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeliveryTargetReceiptWire {
        target: Target,
        attempted: bool,
        outcome: DeliveryOutcome,
    }

    impl<'de> serde::Deserialize<'de> for DeliveryTargetReceipt {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = DeliveryTargetReceiptWire::deserialize(deserializer)?;
            let receipt = Self {
                target: wire.target,
                attempted: wire.attempted,
                outcome: wire.outcome,
            };
            receipt.validate().map_err(serde::de::Error::custom)?;
            Ok(receipt)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct DeliveryReceiptWire {
        request_id: DeliveryRequestId,
        target_set: TargetSet,
        target_receipts: Vec<DeliveryTargetReceipt>,
    }

    impl<'de> serde::Deserialize<'de> for DeliveryReceipt {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = DeliveryReceiptWire::deserialize(deserializer)?;
            Self::new(wire.request_id, wire.target_set, wire.target_receipts)
                .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SinkFailureWire {
        request_id: DeliveryRequestId,
        target_set: TargetSet,
        code: String,
        retryability: Retryability,
        retry_after_unix_ms: Option<u64>,
        message: Option<String>,
        partial_evidence: Vec<DeliveryTargetReceipt>,
    }

    impl<'de> serde::Deserialize<'de> for SinkFailure {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = SinkFailureWire::deserialize(deserializer)?;
            let failure = Self {
                request_id: wire.request_id,
                target_set: wire.target_set,
                code: wire.code,
                retryability: wire.retryability,
                retry_after_unix_ms: wire.retry_after_unix_ms,
                message: wire.message,
                partial_evidence: wire.partial_evidence,
            };
            validate_delivery_code(failure.code.as_str()).map_err(serde::de::Error::custom)?;
            if matches!(failure.retryability, Retryability::NotApplicable)
                || matches!(failure.retry_after_unix_ms, Some(0))
                || (failure.retry_after_unix_ms.is_some()
                    && !matches!(failure.retryability, Retryability::Retryable))
            {
                return Err(serde::de::Error::custom(Error::InvalidDeliveryOutcome));
            }
            if let Some(message) = &failure.message {
                validate_delivery_message(message).map_err(serde::de::Error::custom)?;
            }
            let mut observed = BTreeSet::new();
            for receipt in &failure.partial_evidence {
                receipt.validate().map_err(serde::de::Error::custom)?;
                if !failure.target_set.contains(receipt.target().fingerprint()) {
                    return Err(serde::de::Error::custom(
                        Error::UnexpectedDeliveryTargetReceipt,
                    ));
                }
                if !observed.insert(receipt.target().fingerprint().as_str()) {
                    return Err(serde::de::Error::custom(
                        Error::DuplicateDeliveryTargetReceipt,
                    ));
                }
            }
            Ok(failure)
        }
    }
}
