//! Explicit clocks, identifiers, deadlines, and retry decisions.

use std::sync::Arc;

use radroots_signing::Signer;
use radroots_storage::Storage;
use radroots_transport::{EventSink, EventSource};

use crate::Engine;

const MAX_OPERATION_TIMEOUT_MS: u64 = 86_400_000;

/// Sync operation class used for identity and deadline policy.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationKind {
    Pull,
    Sign,
    Deliver,
}

/// Opaque host-generated identity for one synchronization operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyncId([u8; 16]);

impl SyncId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Ok(Self(bytes));
            }
            index += 1;
        }
        Err(Error::InvalidSyncId)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Host clock used instead of reading ambient time inside orchestration.
pub trait Clock: Send + Sync {
    fn now_unix_ms(&self) -> Result<u64, Error>;
}

/// Host identity source used instead of ambient randomness or global counters.
pub trait IdSource: Send + Sync {
    fn next_id(&self, operation: OperationKind) -> Result<SyncId, Error>;
}

/// Bounded time budgets applied to individual orchestration calls.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeadlinePolicy {
    pull_timeout_ms: u64,
    sign_timeout_ms: u64,
    delivery_timeout_ms: u64,
}

impl DeadlinePolicy {
    pub const fn new(
        pull_timeout_ms: u64,
        sign_timeout_ms: u64,
        delivery_timeout_ms: u64,
    ) -> Result<Self, Error> {
        if !valid_timeout(pull_timeout_ms)
            || !valid_timeout(sign_timeout_ms)
            || !valid_timeout(delivery_timeout_ms)
        {
            return Err(Error::InvalidDeadlinePolicy);
        }
        Ok(Self {
            pull_timeout_ms,
            sign_timeout_ms,
            delivery_timeout_ms,
        })
    }

    pub const fn timeout_ms(self, operation: OperationKind) -> u64 {
        match operation {
            OperationKind::Pull => self.pull_timeout_ms,
            OperationKind::Sign => self.sign_timeout_ms,
            OperationKind::Deliver => self.delivery_timeout_ms,
        }
    }

    pub fn deadline_unix_ms(
        self,
        operation: OperationKind,
        now_unix_ms: u64,
    ) -> Result<u64, Error> {
        if now_unix_ms == 0 {
            return Err(Error::ClockUnavailable);
        }
        now_unix_ms
            .checked_add(self.timeout_ms(operation))
            .ok_or(Error::DeadlineOverflow)
    }
}

const fn valid_timeout(value: u64) -> bool {
    value != 0 && value <= MAX_OPERATION_TIMEOUT_MS
}

/// Builder for an [`Engine`] with explicit optional transport capabilities.
pub struct EngineBuilder {
    storage: Arc<dyn Storage>,
    source: Option<Arc<dyn EventSource>>,
    sink: Option<Arc<dyn EventSink>>,
    signer: Option<Arc<dyn Signer>>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdSource>,
    deadlines: DeadlinePolicy,
}

impl EngineBuilder {
    pub(crate) fn new(
        storage: Arc<dyn Storage>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdSource>,
        deadlines: DeadlinePolicy,
    ) -> Self {
        Self {
            storage,
            source: None,
            sink: None,
            signer: None,
            clock,
            ids,
            deadlines,
        }
    }

    #[must_use]
    pub fn source(mut self, source: Arc<dyn EventSource>) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    #[must_use]
    pub fn signer(mut self, signer: Arc<dyn Signer>) -> Self {
        self.signer = Some(signer);
        self
    }

    pub fn build(self) -> Result<Engine, Error> {
        if self.signer.is_some() && self.sink.is_none() {
            return Err(Error::SignerWithoutSink);
        }
        if self.source.is_none() && self.sink.is_none() {
            return Err(Error::MissingTransportCapability);
        }
        Ok(Engine {
            storage: self.storage,
            source: self.source,
            sink: self.sink,
            signer: self.signer,
            clock: self.clock,
            ids: self.ids,
            deadlines: self.deadlines,
        })
    }
}

/// Sync composition and host-policy error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    InvalidSyncId,
    InvalidDeadlinePolicy,
    ClockUnavailable,
    DeadlineOverflow,
    MissingTransportCapability,
    SignerWithoutSink,
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSyncId => "sync identity must not be all zero",
            Self::InvalidDeadlinePolicy => "sync deadline policy is outside its bounds",
            Self::ClockUnavailable => "sync clock did not provide a valid timestamp",
            Self::DeadlineOverflow => "sync deadline overflowed",
            Self::MissingTransportCapability => "sync engine requires a source or sink",
            Self::SignerWithoutSink => "sync signer requires a sink",
        })
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SyncId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <[u8; 16] as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(bytes).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DeadlinePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            pull_timeout_ms: u64,
            sign_timeout_ms: u64,
            delivery_timeout_ms: u64,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(
            wire.pull_timeout_ms,
            wire.sign_timeout_ms,
            wire.delivery_timeout_ms,
        )
        .map_err(serde::de::Error::custom)
    }
}
