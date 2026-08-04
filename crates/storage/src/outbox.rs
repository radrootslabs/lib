//! Durable outbox and delivery-evidence contracts.
//!
//! This module stores delivery intent and normalized evidence. It deliberately
//! owns no transport adapter and performs no transport I/O.

use core::fmt;
pub use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, TransportId,
    outcome::{DeliveryOutcome, DeliveryOutcomeKind, Retryability},
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
    target::{
        TARGET_SET_MAX_ITEMS, Target, TargetFingerprint, TargetLabel, TargetScope, TargetSet,
    },
};

use crate::{Error, journal::OperationInstanceId};

/// Maximum records claimed by one bounded outbox query.
pub const OUTBOX_CLAIM_LIMIT_MAX: u16 = 256;
/// Maximum UTF-8 bytes in a lease owner identity.
pub const LEASE_OWNER_MAX_BYTES: usize = 128;

/// Stable host-generated identity for one durable delivery plan.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OutboxItemId([u8; 16]);

impl OutboxItemId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidOutboxItemId);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Digest of the canonical delivery request, computed by its owning workflow.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeliveryPlanDigest([u8; 32]);

impl DeliveryPlanDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Non-zero optimistic outbox record revision.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OutboxRevision(u64);

impl OutboxRevision {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidOutboxRevision);
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
            .ok_or(Error::CorruptOutboxRecord)
    }
}

/// Non-zero delivery attempt sequence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DeliveryAttempt(u32);

impl DeliveryAttempt {
    pub const FIRST: Self = Self(1);

    pub const fn new(value: u32) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidDeliveryAttempt);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    fn next(self) -> Result<Self, Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::CorruptOutboxRecord)
    }
}

/// Opaque, caller-generated lease identity.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeaseId([u8; 16]);

impl LeaseId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidOutboxLease);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Validated worker identity recorded with a lease.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeaseOwner(String);

impl LeaseOwner {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        let invalid = [
            value.is_empty(),
            value.len() > LEASE_OWNER_MAX_BYTES,
            value != value.trim(),
            value.chars().any(char::is_control),
        ];
        if invalid.contains(&true) {
            return Err(Error::InvalidOutboxLeaseOwner);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for LeaseOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LeaseOwner")
            .field("value", &"[REDACTED]")
            .field("bytes", &self.0.len())
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for LeaseOwner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for LeaseOwner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Exclusive, expiring authority to mutate one outbox item.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxLease {
    id: LeaseId,
    owner: LeaseOwner,
    acquired_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

impl OutboxLease {
    pub fn new(
        id: LeaseId,
        owner: LeaseOwner,
        acquired_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if [
            acquired_at_unix_ms == 0,
            expires_at_unix_ms <= acquired_at_unix_ms,
        ]
        .contains(&true)
        {
            return Err(Error::InvalidOutboxLease);
        }
        Ok(Self {
            id,
            owner,
            acquired_at_unix_ms,
            expires_at_unix_ms,
        })
    }

    pub const fn id(&self) -> LeaseId {
        self.id
    }

    pub const fn owner(&self) -> &LeaseOwner {
        &self.owner
    }

    pub const fn acquired_at_unix_ms(&self) -> u64 {
        self.acquired_at_unix_ms
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    pub const fn is_active_at(&self, unix_ms: u64) -> bool {
        unix_ms >= self.acquired_at_unix_ms && unix_ms < self.expires_at_unix_ms
    }
}

/// Durable lifecycle of one delivery plan.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboxStage {
    Pending,
    Leased,
    Retryable,
    Satisfied,
    Exhausted,
}

impl OutboxStage {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Satisfied | Self::Exhausted)
    }
}

/// Latest durable evidence for one requested target.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetDeliveryEvidence {
    target: TargetFingerprint,
    attempt: DeliveryAttempt,
    attempted: bool,
    outcome: DeliveryOutcome,
    recorded_at_unix_ms: u64,
}

impl TargetDeliveryEvidence {
    pub fn new(
        target: TargetFingerprint,
        attempt: DeliveryAttempt,
        attempted: bool,
        outcome: DeliveryOutcome,
        recorded_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if recorded_at_unix_ms == 0 {
            return Err(Error::InvalidDeliveryEvidence);
        }
        Ok(Self {
            target,
            attempt,
            attempted,
            outcome,
            recorded_at_unix_ms,
        })
    }

    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }

    pub const fn attempt(&self) -> DeliveryAttempt {
        self.attempt
    }

    pub const fn was_attempted(&self) -> bool {
        self.attempted
    }

    pub const fn outcome(&self) -> &DeliveryOutcome {
        &self.outcome
    }

    pub const fn recorded_at_unix_ms(&self) -> u64 {
        self.recorded_at_unix_ms
    }
}

/// Result of evaluating the latest complete transport receipt.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SatisfactionResult {
    Pending,
    Satisfied,
    Exhausted,
}

/// Durable outbox item and all current delivery evidence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboxRecord {
    item_id: OutboxItemId,
    operation_instance_id: OperationInstanceId,
    plan_digest: DeliveryPlanDigest,
    request: DeliveryRequest,
    revision: OutboxRevision,
    stage: OutboxStage,
    lease: Option<OutboxLease>,
    last_attempt: Option<DeliveryAttempt>,
    evidence: Vec<TargetDeliveryEvidence>,
    satisfaction: SatisfactionResult,
    retry_not_before_unix_ms: Option<u64>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
}

impl OutboxRecord {
    fn from_enqueue(value: EnqueueOutboxItem) -> Self {
        Self {
            item_id: value.item_id,
            operation_instance_id: value.operation_instance_id,
            plan_digest: value.plan_digest,
            request: value.request,
            revision: OutboxRevision::INITIAL,
            stage: OutboxStage::Pending,
            lease: None,
            last_attempt: None,
            evidence: Vec::new(),
            satisfaction: SatisfactionResult::Pending,
            retry_not_before_unix_ms: None,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.created_at_unix_ms,
        }
    }

    /// Reconstructs and validates one record at a durable backend boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn from_durable_parts(
        enqueue: EnqueueOutboxItem,
        revision: OutboxRevision,
        stage: OutboxStage,
        lease: Option<OutboxLease>,
        last_attempt: Option<DeliveryAttempt>,
        evidence: Vec<TargetDeliveryEvidence>,
        satisfaction: SatisfactionResult,
        retry_not_before_unix_ms: Option<u64>,
        updated_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        let invalid = [
            updated_at_unix_ms < enqueue.created_at_unix_ms,
            matches!(stage, OutboxStage::Leased) != lease.is_some(),
            matches!(stage, OutboxStage::Retryable) && last_attempt.is_none(),
            matches!(stage, OutboxStage::Satisfied)
                != matches!(satisfaction, SatisfactionResult::Satisfied),
            matches!(stage, OutboxStage::Exhausted)
                != matches!(satisfaction, SatisfactionResult::Exhausted),
            matches!(
                stage,
                OutboxStage::Pending | OutboxStage::Leased | OutboxStage::Retryable
            ) && !matches!(satisfaction, SatisfactionResult::Pending),
            stage.is_terminal() && retry_not_before_unix_ms.is_some(),
        ];
        if invalid.contains(&true) {
            return Err(Error::CorruptOutboxRecord);
        }

        validate_evidence(
            enqueue.request(),
            last_attempt,
            evidence.as_slice(),
            satisfaction,
            enqueue.created_at_unix_ms,
            updated_at_unix_ms,
        )?;
        Ok(Self {
            item_id: enqueue.item_id,
            operation_instance_id: enqueue.operation_instance_id,
            plan_digest: enqueue.plan_digest,
            request: enqueue.request,
            revision,
            stage,
            lease,
            last_attempt,
            evidence,
            satisfaction,
            retry_not_before_unix_ms,
            created_at_unix_ms: enqueue.created_at_unix_ms,
            updated_at_unix_ms,
        })
    }

    pub const fn item_id(&self) -> OutboxItemId {
        self.item_id
    }
    pub const fn operation_instance_id(&self) -> OperationInstanceId {
        self.operation_instance_id
    }
    pub const fn plan_digest(&self) -> DeliveryPlanDigest {
        self.plan_digest
    }
    pub const fn request(&self) -> &DeliveryRequest {
        &self.request
    }
    pub const fn revision(&self) -> OutboxRevision {
        self.revision
    }
    pub const fn stage(&self) -> OutboxStage {
        self.stage
    }
    pub const fn lease(&self) -> Option<&OutboxLease> {
        self.lease.as_ref()
    }
    pub const fn last_attempt(&self) -> Option<DeliveryAttempt> {
        self.last_attempt
    }
    pub fn evidence(&self) -> &[TargetDeliveryEvidence] {
        self.evidence.as_slice()
    }
    /// Returns the latest evidence for one target without discarding history.
    pub fn latest_target_evidence(
        &self,
        target: &TargetFingerprint,
    ) -> Option<&TargetDeliveryEvidence> {
        self.evidence
            .iter()
            .rev()
            .find(|evidence| evidence.target() == target)
    }
    pub const fn satisfaction(&self) -> SatisfactionResult {
        self.satisfaction
    }
    pub const fn retry_not_before_unix_ms(&self) -> Option<u64> {
        self.retry_not_before_unix_ms
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }

    /// Claims this item if it is ready and has no active lease.
    pub fn claim(&mut self, lease: OutboxLease) -> Result<(), Error> {
        if self.stage.is_terminal() {
            return Err(Error::OutboxItemTerminal);
        }
        if lease.acquired_at_unix_ms() < self.updated_at_unix_ms {
            return Err(Error::InvalidOutboxTimestamp);
        }
        if matches!(self.retry_not_before_unix_ms, Some(not_before) if lease.acquired_at_unix_ms() < not_before)
        {
            return Err(Error::OutboxItemNotReady);
        }
        if self
            .lease
            .as_ref()
            .is_some_and(|current| current.is_active_at(lease.acquired_at_unix_ms()))
        {
            return Err(Error::OutboxLeaseConflict);
        }
        self.revision = self.revision.next()?;
        self.updated_at_unix_ms = lease.acquired_at_unix_ms();
        self.stage = OutboxStage::Leased;
        self.lease = Some(lease);
        Ok(())
    }

    /// Applies one request-bound transport receipt under the active lease.
    pub fn record_attempt(&mut self, value: DeliveryAttemptEvidence) -> Result<(), Error> {
        self.validate_lease(value.lease_id, value.recorded_at_unix_ms)?;
        if value.item_id != self.item_id || value.expected_revision != self.revision {
            return Err(Error::OutboxRevisionConflict);
        }
        let expected_attempt = self
            .last_attempt
            .map_or(Ok(DeliveryAttempt::FIRST), DeliveryAttempt::next)?;
        if value.attempt != expected_attempt {
            return Err(Error::InvalidDeliveryAttempt);
        }
        value
            .receipt
            .validate_for_request(&self.request)
            .map_err(|_| Error::InvalidDeliveryEvidence)?;
        self.evidence
            .extend(
                value
                    .receipt
                    .target_receipts()
                    .iter()
                    .map(|receipt| TargetDeliveryEvidence {
                        target: receipt.target().fingerprint().clone(),
                        attempt: value.attempt,
                        attempted: receipt.was_attempted(),
                        outcome: receipt.outcome().clone(),
                        recorded_at_unix_ms: value.recorded_at_unix_ms,
                    }),
            );
        self.last_attempt = Some(value.attempt);
        self.satisfaction = evaluate_satisfaction(&self.request, &self.evidence);
        self.stage = match self.satisfaction {
            SatisfactionResult::Pending => OutboxStage::Retryable,
            SatisfactionResult::Satisfied => OutboxStage::Satisfied,
            SatisfactionResult::Exhausted => OutboxStage::Exhausted,
        };
        self.lease = None;
        self.retry_not_before_unix_ms = None;
        self.updated_at_unix_ms = value.recorded_at_unix_ms;
        self.revision = self.revision.next()?;
        Ok(())
    }

    /// Releases an active lease and optionally defers the next claim.
    pub fn release(
        &mut self,
        lease_id: LeaseId,
        expected_revision: OutboxRevision,
        released_at_unix_ms: u64,
        retry_not_before_unix_ms: Option<u64>,
    ) -> Result<(), Error> {
        self.validate_lease(lease_id, released_at_unix_ms)?;
        if expected_revision != self.revision {
            return Err(Error::OutboxRevisionConflict);
        }
        // `validate_lease` already proves this timestamp is within a lease
        // whose acquisition timestamp is non-zero.
        if matches!(retry_not_before_unix_ms, Some(value) if value <= released_at_unix_ms) {
            return Err(Error::InvalidOutboxTimestamp);
        }
        self.lease = None;
        self.stage = if self.last_attempt.is_some() {
            OutboxStage::Retryable
        } else {
            OutboxStage::Pending
        };
        self.retry_not_before_unix_ms = retry_not_before_unix_ms;
        self.updated_at_unix_ms = released_at_unix_ms;
        self.revision = self.revision.next()?;
        Ok(())
    }

    fn validate_lease(&self, lease_id: LeaseId, at_unix_ms: u64) -> Result<(), Error> {
        let lease = self.lease.as_ref().ok_or(Error::OutboxLeaseConflict)?;
        if lease.id() != lease_id {
            return Err(Error::OutboxLeaseConflict);
        }
        if !lease.is_active_at(at_unix_ms) {
            return Err(Error::OutboxLeaseExpired);
        }
        Ok(())
    }
}

/// Validated durable plan enqueue request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnqueueOutboxItem {
    item_id: OutboxItemId,
    operation_instance_id: OperationInstanceId,
    plan_digest: DeliveryPlanDigest,
    request: DeliveryRequest,
    created_at_unix_ms: u64,
}

impl EnqueueOutboxItem {
    pub fn new(
        item_id: OutboxItemId,
        operation_instance_id: OperationInstanceId,
        plan_digest: DeliveryPlanDigest,
        request: DeliveryRequest,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if created_at_unix_ms == 0 {
            return Err(Error::InvalidOutboxTimestamp);
        }
        Ok(Self {
            item_id,
            operation_instance_id,
            plan_digest,
            request,
            created_at_unix_ms,
        })
    }

    pub const fn item_id(&self) -> OutboxItemId {
        self.item_id
    }
    pub const fn operation_instance_id(&self) -> OperationInstanceId {
        self.operation_instance_id
    }
    pub const fn plan_digest(&self) -> DeliveryPlanDigest {
        self.plan_digest
    }
    pub const fn request(&self) -> &DeliveryRequest {
        &self.request
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub fn into_record(self) -> OutboxRecord {
        OutboxRecord::from_enqueue(self)
    }
}

/// Idempotent enqueue result.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnqueueDisposition {
    Created,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnqueueReceipt {
    disposition: EnqueueDisposition,
    record: OutboxRecord,
}

impl EnqueueReceipt {
    pub const fn new(disposition: EnqueueDisposition, record: OutboxRecord) -> Self {
        Self {
            disposition,
            record,
        }
    }
    pub const fn disposition(&self) -> EnqueueDisposition {
        self.disposition
    }
    pub const fn record(&self) -> &OutboxRecord {
        &self.record
    }
}

/// Bounded lease acquisition request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimOutboxItems {
    owner: LeaseOwner,
    lease_id_seed: LeaseId,
    now_unix_ms: u64,
    lease_expires_at_unix_ms: u64,
    limit: u16,
}

impl ClaimOutboxItems {
    pub fn new(
        owner: LeaseOwner,
        lease_id_seed: LeaseId,
        now_unix_ms: u64,
        lease_expires_at_unix_ms: u64,
        limit: u16,
    ) -> Result<Self, Error> {
        if [now_unix_ms == 0, lease_expires_at_unix_ms <= now_unix_ms].contains(&true) {
            return Err(Error::InvalidOutboxLease);
        }
        if [limit == 0, limit > OUTBOX_CLAIM_LIMIT_MAX].contains(&true) {
            return Err(Error::InvalidOutboxClaimLimit);
        }
        Ok(Self {
            owner,
            lease_id_seed,
            now_unix_ms,
            lease_expires_at_unix_ms,
            limit,
        })
    }
    pub const fn owner(&self) -> &LeaseOwner {
        &self.owner
    }
    pub const fn lease_id_seed(&self) -> LeaseId {
        self.lease_id_seed
    }
    /// Derives a stable, item-specific token from the caller's unique seed.
    pub fn lease_id_for(&self, item_id: OutboxItemId) -> LeaseId {
        let mut bytes = *self.lease_id_seed.as_bytes();
        for (byte, item_byte) in bytes.iter_mut().zip(item_id.as_bytes()) {
            *byte ^= item_byte;
        }
        if bytes_are_zero(&bytes) {
            bytes[0] = 1;
        }
        LeaseId(bytes)
    }
    pub const fn now_unix_ms(&self) -> u64 {
        self.now_unix_ms
    }
    pub const fn lease_expires_at_unix_ms(&self) -> u64 {
        self.lease_expires_at_unix_ms
    }
    pub const fn limit(&self) -> u16 {
        self.limit
    }
}

/// Claimed outbox item with exact lease authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedOutboxItem {
    record: OutboxRecord,
    lease: OutboxLease,
}

impl ClaimedOutboxItem {
    pub const fn new(record: OutboxRecord, lease: OutboxLease) -> Self {
        Self { record, lease }
    }
    pub const fn record(&self) -> &OutboxRecord {
        &self.record
    }
    pub const fn lease(&self) -> &OutboxLease {
        &self.lease
    }
}

/// Request-bound evidence for one complete adapter attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryAttemptEvidence {
    item_id: OutboxItemId,
    lease_id: LeaseId,
    expected_revision: OutboxRevision,
    attempt: DeliveryAttempt,
    receipt: DeliveryReceipt,
    recorded_at_unix_ms: u64,
}

impl DeliveryAttemptEvidence {
    pub fn new(
        item_id: OutboxItemId,
        lease_id: LeaseId,
        expected_revision: OutboxRevision,
        attempt: DeliveryAttempt,
        receipt: DeliveryReceipt,
        recorded_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if recorded_at_unix_ms == 0 {
            return Err(Error::InvalidOutboxTimestamp);
        }
        Ok(Self {
            item_id,
            lease_id,
            expected_revision,
            attempt,
            receipt,
            recorded_at_unix_ms,
        })
    }
    pub const fn item_id(&self) -> OutboxItemId {
        self.item_id
    }
    pub const fn lease_id(&self) -> LeaseId {
        self.lease_id
    }
    pub const fn expected_revision(&self) -> OutboxRevision {
        self.expected_revision
    }
    pub const fn attempt(&self) -> DeliveryAttempt {
        self.attempt
    }
    pub const fn receipt(&self) -> &DeliveryReceipt {
        &self.receipt
    }
    pub const fn recorded_at_unix_ms(&self) -> u64 {
        self.recorded_at_unix_ms
    }
}

/// Passive outbox state summary.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutboxStatus {
    pub pending: u64,
    pub leased: u64,
    pub retryable: u64,
    pub satisfied: u64,
    pub exhausted: u64,
}

impl OutboxStatus {
    pub fn total(self) -> Option<u64> {
        self.pending
            .checked_add(self.leased)?
            .checked_add(self.retryable)?
            .checked_add(self.satisfied)?
            .checked_add(self.exhausted)
    }
}

/// Backend-neutral durable delivery-plan SPI.
pub trait Outbox: Send + Sync {
    fn enqueue(&self, item: EnqueueOutboxItem) -> BoxFuture<'_, Result<EnqueueReceipt, Error>>;
    fn item(&self, item_id: OutboxItemId) -> BoxFuture<'_, Result<Option<OutboxRecord>, Error>>;
    fn claim(
        &self,
        request: ClaimOutboxItems,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, Error>>;
    fn record_attempt(
        &self,
        evidence: DeliveryAttemptEvidence,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>>;
    fn release(
        &self,
        item_id: OutboxItemId,
        lease_id: LeaseId,
        expected_revision: OutboxRevision,
        released_at_unix_ms: u64,
        retry_not_before_unix_ms: Option<u64>,
    ) -> BoxFuture<'_, Result<OutboxRecord, Error>>;
    fn status(&self) -> BoxFuture<'_, Result<OutboxStatus, Error>>;
}

const fn bytes_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn validate_evidence(
    request: &DeliveryRequest,
    last_attempt: Option<DeliveryAttempt>,
    evidence: &[TargetDeliveryEvidence],
    satisfaction: SatisfactionResult,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
) -> Result<(), Error> {
    let Some(last_attempt) = last_attempt else {
        return if evidence.is_empty() & matches!(satisfaction, SatisfactionResult::Pending) {
            Ok(())
        } else {
            Err(Error::CorruptOutboxRecord)
        };
    };
    let target_count = request.target_set().len();
    if evidence.len() != target_count.saturating_mul(last_attempt.get() as usize) {
        return Err(Error::CorruptOutboxRecord);
    }
    if evidence.iter().any(|entry| {
        [
            entry.recorded_at_unix_ms() < created_at_unix_ms,
            entry.recorded_at_unix_ms() > updated_at_unix_ms,
            !request
                .target_set()
                .targets()
                .iter()
                .any(|target| target.fingerprint() == entry.target()),
        ]
        .contains(&true)
    }) {
        return Err(Error::CorruptOutboxRecord);
    }

    let mut previous_recorded_at = created_at_unix_ms;
    for attempt in 1..=last_attempt.get() {
        let mut recorded_at = None;
        let receipts = request
            .target_set()
            .targets()
            .iter()
            .map(|target| {
                let mut matches = evidence.iter().filter(|entry| {
                    entry.attempt().get() == attempt && entry.target() == target.fingerprint()
                });
                let entry = matches.next().ok_or(Error::CorruptOutboxRecord)?;
                if matches.next().is_some() {
                    return Err(Error::CorruptOutboxRecord);
                }
                if recorded_at
                    .replace(entry.recorded_at_unix_ms())
                    .is_some_and(|prior| prior != entry.recorded_at_unix_ms())
                {
                    return Err(Error::CorruptOutboxRecord);
                }
                if entry.was_attempted() {
                    Ok(DeliveryTargetReceipt::attempted(
                        target.clone(),
                        entry.outcome().clone(),
                    ))
                } else {
                    DeliveryTargetReceipt::skipped(target.clone(), entry.outcome().clone())
                        .map_err(|_| Error::CorruptOutboxRecord)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let recorded_at = recorded_at.ok_or(Error::CorruptOutboxRecord)?;
        if recorded_at < previous_recorded_at {
            return Err(Error::CorruptOutboxRecord);
        }
        previous_recorded_at = recorded_at;
        DeliveryReceipt::for_request(request, receipts).map_err(|_| Error::CorruptOutboxRecord)?;
    }
    let expected = evaluate_satisfaction(request, evidence);
    if expected == satisfaction {
        Ok(())
    } else {
        Err(Error::CorruptOutboxRecord)
    }
}

fn evaluate_satisfaction(
    request: &DeliveryRequest,
    evidence: &[TargetDeliveryEvidence],
) -> SatisfactionResult {
    let class = request.satisfaction().class();
    let targets = request.target_set().targets();
    let is_successful = |target: &TargetFingerprint| {
        evidence
            .iter()
            .any(|entry| entry.target() == target && entry.outcome().satisfies(class))
    };
    let is_retryable = |target: &TargetFingerprint| {
        evidence
            .iter()
            .rev()
            .find(|entry| entry.target() == target)
            .is_some_and(|entry| entry.outcome().is_retryable())
    };
    let successful = targets
        .iter()
        .filter(|target| is_successful(target.fingerprint()))
        .count();
    let retryable = targets
        .iter()
        .filter(|target| !is_successful(target.fingerprint()) && is_retryable(target.fingerprint()))
        .count();
    let policy = request.satisfaction().targets();
    let (satisfied, possible) = if policy.is_any() {
        (successful != 0, successful + retryable != 0)
    } else if policy.is_all() {
        (
            successful == targets.len(),
            successful + retryable == targets.len(),
        )
    } else if let Some(threshold) = policy.quorum_threshold() {
        let threshold = usize::from(threshold);
        (successful >= threshold, successful + retryable >= threshold)
    } else {
        // `TargetPolicy` is closed over any/all/quorum/required; after the
        // preceding branches, the required-target slice is necessarily set.
        let required = policy.required_targets().unwrap_or_default();
        (
            required.iter().all(&is_successful),
            required
                .iter()
                .all(|target| is_successful(target) || is_retryable(target)),
        )
    };
    if satisfied {
        SatisfactionResult::Satisfied
    } else if possible {
        SatisfactionResult::Pending
    } else {
        SatisfactionResult::Exhausted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{SignedEvent, wire::Nip01EventWire};
    use radroots_transport::sink::DeliveryPayload;

    fn signed_event() -> SignedEvent {
        let mut wire = Nip01EventWire {
            id: "0".repeat(64),
            pubkey: "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df".to_owned(),
            created_at: 1_800_000_100,
            kind: 0,
            tags: vec![],
            content: "{}".to_owned(),
            sig: "42".repeat(64),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().unwrap().to_hex();
        let raw = serde_json::json!({
            "id": wire.id,
            "pubkey": wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": wire.tags,
            "content": wire.content,
            "sig": wire.sig,
        })
        .to_string();
        SignedEvent::from_wire_verified_id(wire, raw).unwrap()
    }

    fn request() -> DeliveryRequest {
        DeliveryRequest::new(
            "storage-outbox-unit",
            DeliveryPayload::new(signed_event()),
            TargetSet::new(vec![Target::nostr_relay("wss://relay.example").unwrap()]).unwrap(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_000,
        )
        .unwrap()
    }

    fn enqueue() -> EnqueueOutboxItem {
        EnqueueOutboxItem::new(
            OutboxItemId::new([1; 16]).unwrap(),
            OperationInstanceId::new([2; 16]).unwrap(),
            DeliveryPlanDigest::new([3; 32]),
            request(),
            10,
        )
        .unwrap()
    }

    fn lease(id: u8, acquired: u64, expires: u64) -> OutboxLease {
        OutboxLease::new(
            LeaseId::new([id; 16]).unwrap(),
            LeaseOwner::parse("worker").unwrap(),
            acquired,
            expires,
        )
        .unwrap()
    }

    fn receipt_for(request: &DeliveryRequest, outcome: DeliveryOutcome) -> DeliveryReceipt {
        DeliveryReceipt::for_request(
            request,
            vec![DeliveryTargetReceipt::attempted(
                request.target_set().targets()[0].clone(),
                outcome,
            )],
        )
        .unwrap()
    }

    #[test]
    fn scalar_types_and_lease_policy_cover_all_bounds() {
        assert_eq!(OutboxItemId::new([0; 16]), Err(Error::InvalidOutboxItemId));
        assert_eq!(LeaseId::new([0; 16]), Err(Error::InvalidOutboxLease));
        assert_eq!(OutboxRevision::new(0), Err(Error::InvalidOutboxRevision));
        assert_eq!(DeliveryAttempt::new(0), Err(Error::InvalidDeliveryAttempt));
        let item = OutboxItemId::new([1; 16]).unwrap();
        let digest = DeliveryPlanDigest::new([2; 32]);
        assert_eq!(item.as_bytes(), &[1; 16]);
        assert_eq!(digest.as_bytes(), &[2; 32]);
        assert_eq!(OutboxRevision::INITIAL.get(), 1);
        assert_eq!(DeliveryAttempt::FIRST.get(), 1);
        assert_eq!(
            OutboxRevision(u64::MAX).next(),
            Err(Error::CorruptOutboxRecord)
        );
        assert_eq!(
            DeliveryAttempt(u32::MAX).next(),
            Err(Error::CorruptOutboxRecord)
        );

        for invalid in ["", " worker", "worker ", "bad\nworker"] {
            assert_eq!(
                LeaseOwner::parse(invalid),
                Err(Error::InvalidOutboxLeaseOwner)
            );
        }
        assert_eq!(
            LeaseOwner::parse("x".repeat(LEASE_OWNER_MAX_BYTES + 1)),
            Err(Error::InvalidOutboxLeaseOwner)
        );
        let owner = LeaseOwner::parse("worker").unwrap();
        assert_eq!(owner.as_str(), "worker");
        let debug = format!("{owner:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("worker"));

        let id = LeaseId::new([4; 16]).unwrap();
        assert_eq!(
            OutboxLease::new(id, owner.clone(), 0, 2),
            Err(Error::InvalidOutboxLease)
        );
        assert_eq!(
            OutboxLease::new(id, owner.clone(), 2, 2),
            Err(Error::InvalidOutboxLease)
        );
        assert_eq!(
            OutboxLease::new(id, owner, 2, 1),
            Err(Error::InvalidOutboxLease)
        );
        let value = lease(4, 2, 4);
        assert_eq!(value.id().as_bytes(), &[4; 16]);
        assert_eq!(value.owner().as_str(), "worker");
        assert_eq!(value.acquired_at_unix_ms(), 2);
        assert_eq!(value.expires_at_unix_ms(), 4);
        assert!(!value.is_active_at(1));
        assert!(value.is_active_at(2));
        assert!(value.is_active_at(3));
        assert!(!value.is_active_at(4));
        assert!(!OutboxStage::Pending.is_terminal());
        assert!(!OutboxStage::Leased.is_terminal());
        assert!(!OutboxStage::Retryable.is_terminal());
        assert!(OutboxStage::Satisfied.is_terminal());
        assert!(OutboxStage::Exhausted.is_terminal());
    }

    #[test]
    fn enqueue_claim_and_evidence_models_cover_accessors_and_bounds() {
        assert_eq!(
            EnqueueOutboxItem::new(
                OutboxItemId::new([1; 16]).unwrap(),
                OperationInstanceId::new([2; 16]).unwrap(),
                DeliveryPlanDigest::new([3; 32]),
                request(),
                0,
            ),
            Err(Error::InvalidOutboxTimestamp)
        );
        let value = enqueue();
        assert_eq!(value.item_id().as_bytes(), &[1; 16]);
        assert_eq!(value.operation_instance_id().as_bytes(), &[2; 16]);
        assert_eq!(value.plan_digest().as_bytes(), &[3; 32]);
        assert_eq!(value.request().request_id().as_str(), "storage-outbox-unit");
        assert_eq!(value.created_at_unix_ms(), 10);
        let record = value.into_record();
        let receipt = EnqueueReceipt::new(EnqueueDisposition::Created, record.clone());
        assert_eq!(receipt.disposition(), EnqueueDisposition::Created);
        assert_eq!(receipt.record(), &record);

        for (now, expiry, limit, error) in [
            (0, 2, 1, Error::InvalidOutboxLease),
            (2, 2, 1, Error::InvalidOutboxLease),
            (2, 3, 0, Error::InvalidOutboxClaimLimit),
            (
                2,
                3,
                OUTBOX_CLAIM_LIMIT_MAX + 1,
                Error::InvalidOutboxClaimLimit,
            ),
        ] {
            assert_eq!(
                ClaimOutboxItems::new(
                    LeaseOwner::parse("worker").unwrap(),
                    LeaseId::new([1; 16]).unwrap(),
                    now,
                    expiry,
                    limit,
                ),
                Err(error)
            );
        }
        let claim = ClaimOutboxItems::new(
            LeaseOwner::parse("worker").unwrap(),
            LeaseId::new([1; 16]).unwrap(),
            2,
            3,
            1,
        )
        .unwrap();
        assert_eq!(claim.owner().as_str(), "worker");
        assert_eq!(claim.lease_id_seed().as_bytes(), &[1; 16]);
        assert_eq!(
            claim
                .lease_id_for(OutboxItemId::new([1; 16]).unwrap())
                .as_bytes()[0],
            1
        );
        assert_eq!(claim.now_unix_ms(), 2);
        assert_eq!(claim.lease_expires_at_unix_ms(), 3);
        assert_eq!(claim.limit(), 1);

        let claimed = ClaimedOutboxItem::new(record.clone(), lease(5, 10, 20));
        assert_eq!(claimed.record(), &record);
        assert_eq!(claimed.lease().id().as_bytes(), &[5; 16]);
        assert_eq!(
            TargetDeliveryEvidence::new(
                record.request().target_set().targets()[0]
                    .fingerprint()
                    .clone(),
                DeliveryAttempt::FIRST,
                true,
                DeliveryOutcome::accepted(),
                0,
            ),
            Err(Error::InvalidDeliveryEvidence)
        );
        let target_evidence = TargetDeliveryEvidence::new(
            record.request().target_set().targets()[0]
                .fingerprint()
                .clone(),
            DeliveryAttempt::FIRST,
            true,
            DeliveryOutcome::accepted(),
            12,
        )
        .unwrap();
        assert_eq!(target_evidence.attempt(), DeliveryAttempt::FIRST);
        assert!(target_evidence.was_attempted());
        assert_eq!(target_evidence.recorded_at_unix_ms(), 12);
        assert!(
            target_evidence
                .outcome()
                .satisfies(SatisfactionClass::Accepted)
        );
    }

    #[test]
    fn durable_record_and_claim_reject_inconsistent_state() {
        let base = enqueue();
        let valid = OutboxRecord::from_durable_parts(
            base.clone(),
            OutboxRevision::INITIAL,
            OutboxStage::Pending,
            None,
            None,
            vec![],
            SatisfactionResult::Pending,
            None,
            10,
        )
        .unwrap();
        assert_eq!(valid.item_id().as_bytes(), &[1; 16]);
        assert_eq!(valid.operation_instance_id().as_bytes(), &[2; 16]);
        assert_eq!(valid.plan_digest().as_bytes(), &[3; 32]);
        assert_eq!(valid.revision(), OutboxRevision::INITIAL);
        assert_eq!(valid.stage(), OutboxStage::Pending);
        assert!(valid.lease().is_none());
        assert!(valid.last_attempt().is_none());
        assert!(valid.evidence().is_empty());
        assert!(
            valid
                .latest_target_evidence(valid.request().target_set().targets()[0].fingerprint())
                .is_none()
        );
        assert_eq!(valid.satisfaction(), SatisfactionResult::Pending);
        assert_eq!(valid.retry_not_before_unix_ms(), None);
        assert_eq!(valid.created_at_unix_ms(), 10);
        assert_eq!(valid.updated_at_unix_ms(), 10);

        let cases = [
            (
                OutboxStage::Pending,
                None,
                None,
                SatisfactionResult::Pending,
                None,
                9,
            ),
            (
                OutboxStage::Leased,
                None,
                None,
                SatisfactionResult::Pending,
                None,
                10,
            ),
            (
                OutboxStage::Pending,
                Some(lease(1, 10, 20)),
                None,
                SatisfactionResult::Pending,
                None,
                10,
            ),
            (
                OutboxStage::Retryable,
                None,
                None,
                SatisfactionResult::Pending,
                None,
                10,
            ),
            (
                OutboxStage::Satisfied,
                None,
                None,
                SatisfactionResult::Pending,
                None,
                10,
            ),
            (
                OutboxStage::Exhausted,
                None,
                None,
                SatisfactionResult::Pending,
                None,
                10,
            ),
            (
                OutboxStage::Pending,
                None,
                None,
                SatisfactionResult::Satisfied,
                None,
                10,
            ),
            (
                OutboxStage::Satisfied,
                None,
                None,
                SatisfactionResult::Satisfied,
                Some(20),
                10,
            ),
        ];
        for (stage, lease, last_attempt, satisfaction, retry, updated) in cases {
            assert_eq!(
                OutboxRecord::from_durable_parts(
                    base.clone(),
                    OutboxRevision::INITIAL,
                    stage,
                    lease,
                    last_attempt,
                    vec![],
                    satisfaction,
                    retry,
                    updated,
                ),
                Err(Error::CorruptOutboxRecord)
            );
        }

        let mut record = valid;
        assert_eq!(
            record.release(LeaseId::new([1; 16]).unwrap(), record.revision(), 11, None),
            Err(Error::OutboxLeaseConflict)
        );
        record.claim(lease(1, 10, 20)).unwrap();
        assert_eq!(
            record.claim(lease(2, 9, 20)),
            Err(Error::InvalidOutboxTimestamp)
        );
        assert_eq!(
            record.claim(lease(2, 11, 20)),
            Err(Error::OutboxLeaseConflict)
        );
        assert_eq!(
            record.release(LeaseId::new([2; 16]).unwrap(), record.revision(), 12, None),
            Err(Error::OutboxLeaseConflict)
        );
        assert_eq!(
            record.release(
                LeaseId::new([1; 16]).unwrap(),
                OutboxRevision::INITIAL,
                12,
                None
            ),
            Err(Error::OutboxRevisionConflict)
        );
        let revision = record.revision();
        assert_eq!(
            record.release(LeaseId::new([1; 16]).unwrap(), revision, 12, Some(12)),
            Err(Error::InvalidOutboxTimestamp)
        );
        record
            .release(LeaseId::new([1; 16]).unwrap(), revision, 12, Some(13))
            .unwrap();
        assert_eq!(record.stage(), OutboxStage::Pending);
        assert_eq!(
            record.claim(lease(3, 12, 20)),
            Err(Error::OutboxItemNotReady)
        );
        record.claim(lease(3, 13, 20)).unwrap();
    }

    #[test]
    fn outbox_status_detects_each_overflow_position() {
        assert_eq!(
            OutboxStatus {
                pending: 1,
                leased: 2,
                retryable: 3,
                satisfied: 4,
                exhausted: 5
            }
            .total(),
            Some(15)
        );
        for status in [
            OutboxStatus {
                pending: u64::MAX,
                leased: 1,
                retryable: 0,
                satisfied: 0,
                exhausted: 0,
            },
            OutboxStatus {
                pending: 0,
                leased: u64::MAX,
                retryable: 1,
                satisfied: 0,
                exhausted: 0,
            },
            OutboxStatus {
                pending: 0,
                leased: 0,
                retryable: u64::MAX,
                satisfied: 1,
                exhausted: 0,
            },
            OutboxStatus {
                pending: 0,
                leased: 0,
                retryable: 0,
                satisfied: u64::MAX,
                exhausted: 1,
            },
        ] {
            assert_eq!(status.total(), None);
        }
    }

    #[test]
    fn evidence_reconstruction_and_attempt_errors_are_fail_closed() {
        let enqueue = enqueue();
        let target = enqueue.request().target_set().targets()[0]
            .fingerprint()
            .clone();
        let accepted = TargetDeliveryEvidence::new(
            target.clone(),
            DeliveryAttempt::FIRST,
            true,
            DeliveryOutcome::accepted(),
            20,
        )
        .unwrap();
        assert_eq!(
            OutboxRecord::from_durable_parts(
                enqueue.clone(),
                OutboxRevision::new(2).unwrap(),
                OutboxStage::Satisfied,
                None,
                Some(DeliveryAttempt::FIRST),
                vec![accepted.clone()],
                SatisfactionResult::Satisfied,
                None,
                20,
            )
            .unwrap()
            .latest_target_evidence(&target),
            Some(&accepted)
        );
        let terminal = OutboxRecord::from_durable_parts(
            enqueue.clone(),
            OutboxRevision::new(2).unwrap(),
            OutboxStage::Satisfied,
            None,
            Some(DeliveryAttempt::FIRST),
            vec![accepted.clone()],
            SatisfactionResult::Satisfied,
            None,
            20,
        )
        .unwrap();
        let mut terminal = terminal;
        assert_eq!(
            terminal.claim(lease(2, 21, 30)),
            Err(Error::OutboxItemTerminal)
        );

        let retryable_evidence = TargetDeliveryEvidence::new(
            target.clone(),
            DeliveryAttempt::FIRST,
            true,
            DeliveryOutcome::unavailable(),
            20,
        )
        .unwrap();
        let mut retryable = OutboxRecord::from_durable_parts(
            enqueue.clone(),
            OutboxRevision::new(2).unwrap(),
            OutboxStage::Retryable,
            None,
            Some(DeliveryAttempt::FIRST),
            vec![retryable_evidence],
            SatisfactionResult::Pending,
            None,
            20,
        )
        .unwrap();
        retryable.claim(lease(3, 21, 30)).unwrap();
        let revision = retryable.revision();
        retryable
            .release(LeaseId::new([3; 16]).unwrap(), revision, 22, None)
            .unwrap();
        assert_eq!(retryable.stage(), OutboxStage::Retryable);

        let malformed = [
            (
                None,
                vec![accepted.clone()],
                SatisfactionResult::Pending,
                20,
            ),
            (
                Some(DeliveryAttempt::FIRST),
                vec![],
                SatisfactionResult::Pending,
                20,
            ),
            (
                Some(DeliveryAttempt::FIRST),
                vec![
                    TargetDeliveryEvidence::new(
                        target.clone(),
                        DeliveryAttempt::FIRST,
                        true,
                        DeliveryOutcome::accepted(),
                        9,
                    )
                    .unwrap(),
                ],
                SatisfactionResult::Satisfied,
                20,
            ),
            (
                Some(DeliveryAttempt::FIRST),
                vec![
                    TargetDeliveryEvidence::new(
                        target.clone(),
                        DeliveryAttempt::FIRST,
                        true,
                        DeliveryOutcome::accepted(),
                        21,
                    )
                    .unwrap(),
                ],
                SatisfactionResult::Satisfied,
                20,
            ),
            (
                Some(DeliveryAttempt::FIRST),
                vec![
                    TargetDeliveryEvidence::new(
                        Target::nostr_relay("wss://foreign.example")
                            .unwrap()
                            .fingerprint()
                            .clone(),
                        DeliveryAttempt::FIRST,
                        true,
                        DeliveryOutcome::accepted(),
                        20,
                    )
                    .unwrap(),
                ],
                SatisfactionResult::Satisfied,
                20,
            ),
            (
                Some(DeliveryAttempt::FIRST),
                vec![accepted.clone()],
                SatisfactionResult::Pending,
                20,
            ),
        ];
        for (last_attempt, evidence, satisfaction, updated) in malformed {
            assert_eq!(
                OutboxRecord::from_durable_parts(
                    enqueue.clone(),
                    OutboxRevision::new(2).unwrap(),
                    OutboxStage::Retryable,
                    None,
                    last_attempt,
                    evidence,
                    satisfaction,
                    None,
                    updated,
                ),
                Err(Error::CorruptOutboxRecord)
            );
        }

        let mut record = enqueue.into_record();
        let active_lease = lease(1, 20, 40);
        record.claim(active_lease.clone()).unwrap();
        let make_evidence = |item_id, revision, attempt, request: &DeliveryRequest| {
            DeliveryAttemptEvidence::new(
                item_id,
                active_lease.id(),
                revision,
                attempt,
                receipt_for(request, DeliveryOutcome::accepted()),
                30,
            )
            .unwrap()
        };
        assert_eq!(
            record.record_attempt(make_evidence(
                OutboxItemId::new([9; 16]).unwrap(),
                record.revision(),
                DeliveryAttempt::FIRST,
                record.request(),
            )),
            Err(Error::OutboxRevisionConflict)
        );
        assert_eq!(
            record.record_attempt(make_evidence(
                record.item_id(),
                OutboxRevision::INITIAL,
                DeliveryAttempt::FIRST,
                record.request(),
            )),
            Err(Error::OutboxRevisionConflict)
        );
        assert_eq!(
            record.record_attempt(make_evidence(
                record.item_id(),
                record.revision(),
                DeliveryAttempt::new(2).unwrap(),
                record.request(),
            )),
            Err(Error::InvalidDeliveryAttempt)
        );
        let other = DeliveryRequest::new(
            "other-request",
            DeliveryPayload::new(signed_event()),
            TargetSet::new(vec![Target::nostr_relay("wss://other.example").unwrap()]).unwrap(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_000,
        )
        .unwrap();
        assert_eq!(
            record.record_attempt(make_evidence(
                record.item_id(),
                record.revision(),
                DeliveryAttempt::FIRST,
                &other,
            )),
            Err(Error::InvalidDeliveryEvidence)
        );
        let evidence = make_evidence(
            record.item_id(),
            record.revision(),
            DeliveryAttempt::FIRST,
            record.request(),
        );
        assert_eq!(evidence.item_id(), record.item_id());
        assert_eq!(evidence.lease_id(), active_lease.id());
        assert_eq!(evidence.expected_revision(), record.revision());
        assert_eq!(evidence.attempt(), DeliveryAttempt::FIRST);
        assert_eq!(evidence.recorded_at_unix_ms(), 30);
        assert_eq!(
            evidence.receipt().request_id(),
            record.request().request_id()
        );
        record.record_attempt(evidence).unwrap();
        assert_eq!(record.stage(), OutboxStage::Satisfied);
    }

    #[test]
    fn evidence_validation_and_satisfaction_cover_multi_target_policy_edges() {
        let targets = vec![
            Target::nostr_relay("wss://one.example").unwrap(),
            Target::nostr_relay("wss://two.example").unwrap(),
        ];
        let request_with = |policy| {
            DeliveryRequest::new(
                "storage-outbox-policy-matrix",
                DeliveryPayload::new(signed_event()),
                TargetSet::new(targets.clone()).unwrap(),
                SatisfactionPolicy::new(SatisfactionClass::Accepted, policy),
                1_000,
            )
            .unwrap()
        };
        let evidence = |target: &Target,
                        attempt: u32,
                        was_attempted: bool,
                        outcome: DeliveryOutcome,
                        recorded_at_unix_ms| {
            TargetDeliveryEvidence::new(
                target.fingerprint().clone(),
                DeliveryAttempt::new(attempt).unwrap(),
                was_attempted,
                outcome,
                recorded_at_unix_ms,
            )
            .unwrap()
        };

        let all_request = request_with(TargetPolicy::all());
        let accepted = vec![
            evidence(&targets[0], 1, true, DeliveryOutcome::accepted(), 20),
            evidence(&targets[1], 1, true, DeliveryOutcome::accepted(), 20),
        ];
        assert_eq!(
            validate_evidence(
                &all_request,
                Some(DeliveryAttempt::FIRST),
                &accepted,
                SatisfactionResult::Satisfied,
                10,
                20,
            ),
            Ok(())
        );
        assert_eq!(
            validate_evidence(
                &all_request,
                None,
                &[],
                SatisfactionResult::Satisfied,
                10,
                20,
            ),
            Err(Error::CorruptOutboxRecord)
        );

        let duplicated = vec![accepted[0].clone(), accepted[0].clone()];
        assert_eq!(
            validate_evidence(
                &all_request,
                Some(DeliveryAttempt::FIRST),
                &duplicated,
                SatisfactionResult::Satisfied,
                10,
                20,
            ),
            Err(Error::CorruptOutboxRecord)
        );
        let mismatched_times = vec![
            accepted[0].clone(),
            evidence(&targets[1], 1, true, DeliveryOutcome::accepted(), 21),
        ];
        assert_eq!(
            validate_evidence(
                &all_request,
                Some(DeliveryAttempt::FIRST),
                &mismatched_times,
                SatisfactionResult::Satisfied,
                10,
                21,
            ),
            Err(Error::CorruptOutboxRecord)
        );
        let skipped = vec![
            evidence(&targets[0], 1, false, DeliveryOutcome::unavailable(), 20),
            evidence(&targets[1], 1, false, DeliveryOutcome::unavailable(), 20),
        ];
        assert_eq!(
            validate_evidence(
                &all_request,
                Some(DeliveryAttempt::FIRST),
                &skipped,
                SatisfactionResult::Pending,
                10,
                20,
            ),
            Ok(())
        );
        let regressing = vec![
            accepted[0].clone(),
            accepted[1].clone(),
            evidence(&targets[0], 2, true, DeliveryOutcome::accepted(), 19),
            evidence(&targets[1], 2, true, DeliveryOutcome::accepted(), 19),
        ];
        assert_eq!(
            validate_evidence(
                &all_request,
                Some(DeliveryAttempt::new(2).unwrap()),
                &regressing,
                SatisfactionResult::Satisfied,
                10,
                20,
            ),
            Err(Error::CorruptOutboxRecord)
        );

        let retryable = vec![evidence(
            &targets[0],
            1,
            true,
            DeliveryOutcome::unavailable(),
            20,
        )];
        let one_accepted = vec![accepted[0].clone()];
        let any_request = request_with(TargetPolicy::any());
        assert_eq!(
            evaluate_satisfaction(&any_request, &[]),
            SatisfactionResult::Exhausted
        );
        assert_eq!(
            evaluate_satisfaction(&any_request, &retryable),
            SatisfactionResult::Pending
        );
        assert_eq!(
            evaluate_satisfaction(&any_request, &one_accepted),
            SatisfactionResult::Satisfied
        );
        let quorum_request = request_with(TargetPolicy::quorum(2).unwrap());
        assert_eq!(
            evaluate_satisfaction(&quorum_request, &one_accepted),
            SatisfactionResult::Exhausted
        );
        let required_request =
            request_with(TargetPolicy::required(vec![targets[0].fingerprint().clone()]).unwrap());
        assert_eq!(
            evaluate_satisfaction(&required_request, &one_accepted),
            SatisfactionResult::Satisfied
        );
        assert_eq!(
            evaluate_satisfaction(&required_request, &retryable),
            SatisfactionResult::Pending
        );

        assert_eq!(
            DeliveryAttemptEvidence::new(
                OutboxItemId::new([1; 16]).unwrap(),
                LeaseId::new([2; 16]).unwrap(),
                OutboxRevision::INITIAL,
                DeliveryAttempt::FIRST,
                receipt_for(&request(), DeliveryOutcome::accepted()),
                0,
            ),
            Err(Error::InvalidOutboxTimestamp)
        );
    }
}
