//! Durable outbox and delivery-evidence contracts.
//!
//! This module stores delivery intent and normalized evidence. It deliberately
//! owns no transport adapter and performs no transport I/O.

use core::fmt;
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest,
    outcome::{DeliveryOutcome, Retryability},
    target::TargetFingerprint,
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
        if value.is_empty()
            || value.len() > LEASE_OWNER_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
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
        if acquired_at_unix_ms == 0 || expires_at_unix_ms <= acquired_at_unix_ms {
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
        let satisfied = value
            .receipt
            .is_satisfied(&self.request)
            .map_err(|_| Error::InvalidDeliveryEvidence)?;
        let retryable = value
            .receipt
            .target_receipts()
            .iter()
            .any(|receipt| matches!(receipt.outcome().retryability(), Retryability::Retryable));
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
        self.satisfaction = if satisfied {
            SatisfactionResult::Satisfied
        } else if retryable {
            SatisfactionResult::Pending
        } else {
            SatisfactionResult::Exhausted
        };
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
        if released_at_unix_ms == 0
            || matches!(retry_not_before_unix_ms, Some(value) if value <= released_at_unix_ms)
        {
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
        if now_unix_ms == 0 || lease_expires_at_unix_ms <= now_unix_ms {
            return Err(Error::InvalidOutboxLease);
        }
        if limit == 0 || limit > OUTBOX_CLAIM_LIMIT_MAX {
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
