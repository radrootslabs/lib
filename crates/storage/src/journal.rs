//! Durable operation journal contracts.
//!
//! [`JournalState::Committed`] is the local durable commit point. Cancellation
//! before that point records recoverable work and may be resumed with the same
//! idempotency key. Cancellation observed after it never claims rollback:
//! callers must receive committed state and may continue pending delivery.

use core::fmt;
pub use radroots_event::EventId;
pub use radroots_protocol::runtime::v1::OperationId;
pub use radroots_transport::BoxFuture;

use crate::Error;

/// Maximum UTF-8 bytes in a journal idempotency key.
pub const IDEMPOTENCY_KEY_MAX_BYTES: usize = 256;
/// Maximum records returned by one recoverable-work query.
pub const RECOVERABLE_QUERY_LIMIT_MAX: u16 = 256;

/// Host-generated identity for one durable operation execution.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationInstanceId([u8; 16]);

impl OperationInstanceId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if all_zero(&bytes) {
            return Err(Error::InvalidOperationInstanceId);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

const fn all_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Validated caller-owned idempotency key.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Error> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > IDEMPOTENCY_KEY_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidIdempotencyKey);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for IdempotencyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdempotencyKey")
            .field("value", &"[REDACTED]")
            .field("bytes", &self.0.len())
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for IdempotencyKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for IdempotencyKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// SHA-256 digest of canonical operation input, computed by its domain owner.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdempotencyDigest([u8; 32]);

impl IdempotencyDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Non-zero optimistic journal revision.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct JournalRevision(u64);

impl JournalRevision {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidJournalRevision);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self, Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::CorruptJournalRecord)
    }
}

/// Durable lifecycle stage.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalStage {
    Prepared,
    Signed,
    Recoverable,
    Committed,
}

/// Point from which recoverable work resumes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryPoint {
    Prepared,
    Signed { event_id: EventId },
}

/// Stable class of recoverable interruption.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReason {
    CancelledBeforeCommit,
    SignerUnavailable,
    TransportUnavailable,
    StorageUnavailable,
    DeadlineExceeded,
    Interrupted,
}

/// Durable recovery evidence without backend or secret detail.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRecord {
    point: RecoveryPoint,
    reason: RecoveryReason,
    attempt: u32,
    retry_not_before_unix_ms: Option<u64>,
}

impl RecoveryRecord {
    pub const fn new(
        point: RecoveryPoint,
        reason: RecoveryReason,
        attempt: u32,
        retry_not_before_unix_ms: Option<u64>,
    ) -> Result<Self, Error> {
        if attempt == 0 {
            return Err(Error::InvalidRecoveryAttempt);
        }
        if matches!(retry_not_before_unix_ms, Some(0)) {
            return Err(Error::InvalidRecoveryDeadline);
        }
        Ok(Self {
            point,
            reason,
            attempt,
            retry_not_before_unix_ms,
        })
    }

    pub const fn point(&self) -> &RecoveryPoint {
        &self.point
    }

    pub const fn reason(&self) -> RecoveryReason {
        self.reason
    }

    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    pub const fn retry_not_before_unix_ms(&self) -> Option<u64> {
        self.retry_not_before_unix_ms
    }
}

/// Cancellation observation relative to the local durable commit point.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationState {
    NotRequested,
    CancelledBeforeCommit,
    ObservedAfterCommit,
}

/// State-specific durable journal data.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalState {
    Prepared,
    Signed {
        event_id: EventId,
    },
    Recoverable(RecoveryRecord),
    /// Local state is durably committed; remote delivery may remain pending.
    Committed {
        event_id: EventId,
        committed_at_unix_ms: u64,
    },
}

impl JournalState {
    pub const fn stage(&self) -> JournalStage {
        match self {
            Self::Prepared => JournalStage::Prepared,
            Self::Signed { .. } => JournalStage::Signed,
            Self::Recoverable(_) => JournalStage::Recoverable,
            Self::Committed { .. } => JournalStage::Committed,
        }
    }
}

/// Durable operation record.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRecord {
    instance_id: OperationInstanceId,
    operation_id: OperationId,
    idempotency_key: IdempotencyKey,
    input_digest: IdempotencyDigest,
    prepared_at_unix_ms: u64,
    revision: JournalRevision,
    state: JournalState,
    cancellation: CancellationState,
}

impl OperationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        instance_id: OperationInstanceId,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
        input_digest: IdempotencyDigest,
        prepared_at_unix_ms: u64,
        revision: JournalRevision,
        state: JournalState,
        cancellation: CancellationState,
    ) -> Result<Self, Error> {
        if prepared_at_unix_ms == 0 {
            return Err(Error::InvalidOperationTimestamp);
        }
        validate_state(prepared_at_unix_ms, &state, cancellation)?;
        Ok(Self {
            instance_id,
            operation_id,
            idempotency_key,
            input_digest,
            prepared_at_unix_ms,
            revision,
            state,
            cancellation,
        })
    }

    pub const fn instance_id(&self) -> OperationInstanceId {
        self.instance_id
    }

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    pub const fn input_digest(&self) -> IdempotencyDigest {
        self.input_digest
    }

    pub const fn prepared_at_unix_ms(&self) -> u64 {
        self.prepared_at_unix_ms
    }

    pub const fn revision(&self) -> JournalRevision {
        self.revision
    }

    pub const fn state(&self) -> &JournalState {
        &self.state
    }

    pub const fn cancellation(&self) -> CancellationState {
        self.cancellation
    }

    /// Applies one optimistic transition without allowing lifecycle regressions.
    pub fn transition(&self, transition: &JournalTransition) -> Result<Self, Error> {
        if transition.instance_id != self.instance_id {
            return Err(Error::OperationIdentityMismatch);
        }
        if transition.expected_revision != self.revision {
            return Err(Error::JournalRevisionConflict);
        }

        let (state, cancellation) = apply_transition(self, &transition.kind)?;
        Self::from_parts(
            self.instance_id,
            self.operation_id,
            self.idempotency_key.clone(),
            self.input_digest,
            self.prepared_at_unix_ms,
            self.revision.next()?,
            state,
            cancellation,
        )
    }
}

fn validate_state(
    prepared_at: u64,
    state: &JournalState,
    cancellation: CancellationState,
) -> Result<(), Error> {
    if let JournalState::Committed {
        committed_at_unix_ms,
        ..
    } = state
        && (*committed_at_unix_ms == 0 || *committed_at_unix_ms < prepared_at)
    {
        return Err(Error::CorruptJournalRecord);
    }
    if let JournalState::Recoverable(recovery) = state
        && recovery
            .retry_not_before_unix_ms()
            .is_some_and(|deadline| deadline < prepared_at)
    {
        return Err(Error::CorruptJournalRecord);
    }
    match state {
        JournalState::Committed { .. }
            if cancellation == CancellationState::CancelledBeforeCommit =>
        {
            Err(Error::CorruptJournalRecord)
        }
        JournalState::Recoverable(_) if cancellation == CancellationState::ObservedAfterCommit => {
            Err(Error::CorruptJournalRecord)
        }
        JournalState::Recoverable(recovery)
            if (recovery.reason() == RecoveryReason::CancelledBeforeCommit)
                != (cancellation == CancellationState::CancelledBeforeCommit) =>
        {
            Err(Error::CorruptJournalRecord)
        }
        JournalState::Prepared | JournalState::Signed { .. }
            if cancellation != CancellationState::NotRequested =>
        {
            Err(Error::CorruptJournalRecord)
        }
        _ => Ok(()),
    }
}

fn apply_transition(
    record: &OperationRecord,
    transition: &JournalTransitionKind,
) -> Result<(JournalState, CancellationState), Error> {
    match (record.state(), transition) {
        (JournalState::Prepared, JournalTransitionKind::Signed { event_id })
            if record.cancellation() == CancellationState::NotRequested =>
        {
            Ok((
                JournalState::Signed {
                    event_id: *event_id,
                },
                CancellationState::NotRequested,
            ))
        }
        (JournalState::Committed { .. }, JournalTransitionKind::Cancelled { observed_at })
            if *observed_at >= record.prepared_at_unix_ms() =>
        {
            Ok((
                record.state().clone(),
                CancellationState::ObservedAfterCommit,
            ))
        }
        (JournalState::Committed { .. }, _) => Err(Error::JournalOperationCommitted),
        (_, JournalTransitionKind::Recoverable { record: recovery }) => Ok((
            JournalState::Recoverable(recovery.clone()),
            if recovery.reason() == RecoveryReason::CancelledBeforeCommit {
                CancellationState::CancelledBeforeCommit
            } else {
                CancellationState::NotRequested
            },
        )),
        (JournalState::Recoverable(recovery), JournalTransitionKind::Resume) => Ok((
            match recovery.point() {
                RecoveryPoint::Prepared => JournalState::Prepared,
                RecoveryPoint::Signed { event_id } => JournalState::Signed {
                    event_id: *event_id,
                },
            },
            CancellationState::NotRequested,
        )),
        (
            JournalState::Signed {
                event_id: signed_id,
            },
            JournalTransitionKind::Committed {
                event_id,
                committed_at,
            },
        ) if signed_id == event_id && *committed_at >= record.prepared_at_unix_ms() => Ok((
            JournalState::Committed {
                event_id: *event_id,
                committed_at_unix_ms: *committed_at,
            },
            CancellationState::NotRequested,
        )),
        (JournalState::Prepared, JournalTransitionKind::Cancelled { observed_at })
            if *observed_at >= record.prepared_at_unix_ms() =>
        {
            cancelled(RecoveryPoint::Prepared)
        }
        (JournalState::Signed { event_id }, JournalTransitionKind::Cancelled { observed_at })
            if *observed_at >= record.prepared_at_unix_ms() =>
        {
            cancelled(RecoveryPoint::Signed {
                event_id: *event_id,
            })
        }
        _ => Err(Error::InvalidJournalTransition),
    }
}

fn cancelled(point: RecoveryPoint) -> Result<(JournalState, CancellationState), Error> {
    Ok((
        JournalState::Recoverable(RecoveryRecord::new(
            point,
            RecoveryReason::CancelledBeforeCommit,
            1,
            None,
        )?),
        CancellationState::CancelledBeforeCommit,
    ))
}

/// Input for an idempotent prepare operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareOperation {
    instance_id: OperationInstanceId,
    operation_id: OperationId,
    idempotency_key: IdempotencyKey,
    input_digest: IdempotencyDigest,
    prepared_at_unix_ms: u64,
}

impl PrepareOperation {
    pub fn new(
        instance_id: OperationInstanceId,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
        input_digest: IdempotencyDigest,
        prepared_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if prepared_at_unix_ms == 0 {
            return Err(Error::InvalidOperationTimestamp);
        }
        Ok(Self {
            instance_id,
            operation_id,
            idempotency_key,
            input_digest,
            prepared_at_unix_ms,
        })
    }

    pub const fn instance_id(&self) -> OperationInstanceId {
        self.instance_id
    }

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    pub const fn input_digest(&self) -> IdempotencyDigest {
        self.input_digest
    }

    pub fn into_record(self) -> Result<OperationRecord, Error> {
        OperationRecord::from_parts(
            self.instance_id,
            self.operation_id,
            self.idempotency_key,
            self.input_digest,
            self.prepared_at_unix_ms,
            JournalRevision::INITIAL,
            JournalState::Prepared,
            CancellationState::NotRequested,
        )
    }
}

/// Result of preparing an idempotent operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareDisposition {
    Created,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareReceipt {
    disposition: PrepareDisposition,
    record: OperationRecord,
}

impl PrepareReceipt {
    pub const fn new(disposition: PrepareDisposition, record: OperationRecord) -> Self {
        Self {
            disposition,
            record,
        }
    }

    pub const fn disposition(&self) -> PrepareDisposition {
        self.disposition
    }

    pub const fn record(&self) -> &OperationRecord {
        &self.record
    }
}

/// Validated optimistic transition request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalTransition {
    instance_id: OperationInstanceId,
    expected_revision: JournalRevision,
    kind: JournalTransitionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JournalTransitionKind {
    Signed {
        event_id: EventId,
    },
    Recoverable {
        record: RecoveryRecord,
    },
    Resume,
    Committed {
        event_id: EventId,
        committed_at: u64,
    },
    Cancelled {
        observed_at: u64,
    },
}

impl JournalTransition {
    pub const fn signed(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        event_id: EventId,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            kind: JournalTransitionKind::Signed { event_id },
        }
    }

    pub const fn recoverable(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        record: RecoveryRecord,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            kind: JournalTransitionKind::Recoverable { record },
        }
    }

    pub const fn resume(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            kind: JournalTransitionKind::Resume,
        }
    }

    pub const fn committed(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        event_id: EventId,
        committed_at: u64,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            kind: JournalTransitionKind::Committed {
                event_id,
                committed_at,
            },
        }
    }

    pub const fn cancelled(
        instance_id: OperationInstanceId,
        expected_revision: JournalRevision,
        observed_at: u64,
    ) -> Self {
        Self {
            instance_id,
            expected_revision,
            kind: JournalTransitionKind::Cancelled { observed_at },
        }
    }

    pub const fn instance_id(&self) -> OperationInstanceId {
        self.instance_id
    }
}

/// Backend-neutral durable operation journal SPI.
pub trait Journal: Send + Sync {
    /// Creates a record or replays the exact existing operation. A reused key
    /// with a different operation kind, instance, or digest is a conflict.
    fn prepare(&self, operation: PrepareOperation) -> BoxFuture<'_, Result<PrepareReceipt, Error>>;

    fn operation(
        &self,
        instance_id: OperationInstanceId,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>>;

    fn by_idempotency_key(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> BoxFuture<'_, Result<Option<OperationRecord>, Error>>;

    /// Applies one lifecycle transition atomically at its expected revision.
    fn transition(
        &self,
        transition: JournalTransition,
    ) -> BoxFuture<'_, Result<OperationRecord, Error>>;

    /// Returns recoverable records in backend-stable order.
    fn recoverable(&self, limit: u16) -> BoxFuture<'_, Result<Vec<OperationRecord>, Error>>;
}
