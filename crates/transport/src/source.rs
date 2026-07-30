//! Inbound event source SPI and bounded page models.

use crate::{
    Error, TransportId,
    outcome::FetchTargetOutcome,
    target::{TargetFingerprint, TargetSet},
};
use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};
use core::{fmt, future::Future, pin::Pin};
use radroots_event::SignedEvent;

pub use crate::status::SourceStatus;

/// Maximum encoded request identity length.
pub const FETCH_REQUEST_ID_MAX_BYTES: usize = 256;
/// Maximum opaque cursor length.
pub const FETCH_CURSOR_MAX_BYTES: usize = 2_048;
/// Maximum number of events one page may request.
pub const FETCH_PAGE_MAX_EVENTS: u16 = 1_000;

/// Heap-backed future returned by transport SPIs.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Validated caller identity for one fetch operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FetchRequestId(String);

impl FetchRequestId {
    /// Parses a non-empty, bounded, printable request identity.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::EmptyFetchRequestId);
        }
        if value.len() > FETCH_REQUEST_ID_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidFetchRequestId);
        }
        Ok(Self(value))
    }

    /// Returns the validated request identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for FetchRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Opaque adapter-owned continuation token.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FetchCursor(String);

impl FetchCursor {
    /// Parses a bounded printable cursor without interpreting its contents.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::EmptyFetchCursor);
        }
        if value.len() > FETCH_CURSOR_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidFetchCursor);
        }
        Ok(Self(value))
    }

    /// Returns the opaque cursor exactly as supplied by its adapter.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for FetchCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Hard bounds for one source operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchBounds {
    limit: u16,
    deadline_unix_ms: u64,
}

impl FetchBounds {
    /// Creates bounds with a non-zero page limit and absolute deadline.
    pub const fn new(limit: u16, deadline_unix_ms: u64) -> Result<Self, Error> {
        if limit == 0 || limit > FETCH_PAGE_MAX_EVENTS {
            return Err(Error::InvalidFetchLimit);
        }
        if deadline_unix_ms == 0 {
            return Err(Error::InvalidFetchDeadline);
        }
        Ok(Self {
            limit,
            deadline_unix_ms,
        })
    }

    /// Maximum number of events the adapter may return.
    pub const fn limit(self) -> u16 {
        self.limit
    }

    /// Absolute Unix deadline in milliseconds.
    pub const fn deadline_unix_ms(self) -> u64 {
        self.deadline_unix_ms
    }
}

/// Bounded request for one page from one or more transport targets.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchRequest {
    request_id: FetchRequestId,
    target_set: TargetSet,
    bounds: FetchBounds,
    cursor: Option<FetchCursor>,
}

impl FetchRequest {
    /// Creates a first-page request.
    pub fn new(
        request_id: impl Into<String>,
        target_set: TargetSet,
        bounds: FetchBounds,
    ) -> Result<Self, Error> {
        Ok(Self {
            request_id: FetchRequestId::parse(request_id)?,
            target_set,
            bounds,
            cursor: None,
        })
    }

    /// Sets the adapter-owned cursor for a continuation request.
    #[must_use]
    pub fn with_cursor(mut self, cursor: FetchCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Returns the request identity.
    pub fn request_id(&self) -> &FetchRequestId {
        &self.request_id
    }

    /// Returns the exact requested target set.
    pub fn target_set(&self) -> &TargetSet {
        &self.target_set
    }

    /// Returns the hard operation bounds.
    pub const fn bounds(&self) -> FetchBounds {
        self.bounds
    }

    /// Returns the continuation cursor, when this is not a first-page request.
    pub fn cursor(&self) -> Option<&FetchCursor> {
        self.cursor.as_ref()
    }
}

/// Transport observation attached to one inbound event.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventProvenance {
    transport_id: TransportId,
    target: TargetFingerprint,
    observed_at_unix_ms: u64,
    cursor: Option<FetchCursor>,
}

impl EventProvenance {
    /// Creates provenance for an event observed from one exact target.
    pub fn new(
        transport_id: TransportId,
        target: TargetFingerprint,
        observed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if observed_at_unix_ms == 0 {
            return Err(Error::InvalidObservedAt);
        }
        Ok(Self {
            transport_id,
            target,
            observed_at_unix_ms,
            cursor: None,
        })
    }

    /// Attaches the adapter cursor that located this event.
    #[must_use]
    pub fn with_cursor(mut self, cursor: FetchCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Returns the transport that produced this observation.
    pub const fn transport_id(&self) -> TransportId {
        self.transport_id
    }

    /// Returns the exact target fingerprint that produced this observation.
    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }

    /// Returns the host-recorded observation time.
    pub const fn observed_at_unix_ms(&self) -> u64 {
        self.observed_at_unix_ms
    }

    /// Returns the optional adapter cursor at the observation point.
    pub const fn cursor(&self) -> Option<&FetchCursor> {
        self.cursor.as_ref()
    }
}

/// ID-checked signed event plus transport provenance.
///
/// Signature verification, contract validation, canonical admission, storage,
/// and projection results intentionally remain outside this transport model.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedEvent {
    event: SignedEvent,
    provenance: EventProvenance,
}

impl ObservedEvent {
    /// Attaches provenance to an inbound signed event.
    pub const fn new(event: SignedEvent, provenance: EventProvenance) -> Self {
        Self { event, provenance }
    }

    /// Returns the unverified signed event payload.
    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }

    /// Returns the transport observation.
    pub const fn provenance(&self) -> &EventProvenance {
        &self.provenance
    }
}

/// State required to continue or conclude a bounded fetch.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NextPage {
    /// Every requested target reached its current end.
    Complete,
    /// More results are available from this exact cursor.
    Cursor(FetchCursor),
    /// The operation was cancelled and may optionally be resumed.
    Cancelled { resume_from: Option<FetchCursor> },
}

/// One validated, request-bound page of inbound observations.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchPage {
    request_id: FetchRequestId,
    target_set: TargetSet,
    limit: u16,
    events: Vec<ObservedEvent>,
    target_outcomes: Vec<FetchTargetOutcome>,
    next_page: NextPage,
}

impl FetchPage {
    /// Creates and validates one page against its originating request.
    pub fn for_request(
        request: &FetchRequest,
        events: Vec<ObservedEvent>,
        target_outcomes: Vec<FetchTargetOutcome>,
        next_page: NextPage,
    ) -> Result<Self, Error> {
        let page = Self {
            request_id: request.request_id.clone(),
            target_set: request.target_set.clone(),
            limit: request.bounds.limit,
            events,
            target_outcomes,
            next_page,
        };
        page.validate()?;
        Ok(page)
    }

    /// Validates internal cardinality, target provenance, and outcome identity.
    pub fn validate(&self) -> Result<(), Error> {
        if self.limit == 0 || self.limit > FETCH_PAGE_MAX_EVENTS {
            return Err(Error::InvalidFetchLimit);
        }
        if self.events.len() > usize::from(self.limit) {
            return Err(Error::FetchPageLimitExceeded);
        }

        for observed in &self.events {
            let provenance = observed.provenance();
            let Some(target) = self
                .target_set
                .targets()
                .iter()
                .find(|target| target.fingerprint() == provenance.target())
            else {
                return Err(Error::UnexpectedFetchProvenance);
            };
            if *target.kind() != provenance.transport_id() {
                return Err(Error::UnexpectedFetchProvenance);
            }
        }

        let requested: BTreeSet<&str> = self
            .target_set
            .targets()
            .iter()
            .map(|target| target.fingerprint().as_str())
            .collect();
        let mut outcomes = BTreeSet::new();
        for outcome in &self.target_outcomes {
            if !requested.contains(outcome.target().as_str()) {
                return Err(Error::UnexpectedFetchTargetOutcome);
            }
            if !outcomes.insert(outcome.target().as_str()) {
                return Err(Error::DuplicateFetchTargetOutcome);
            }
        }
        Ok(())
    }

    /// Validates that this page is bound to the exact originating request.
    pub fn validate_for_request(&self, request: &FetchRequest) -> Result<(), Error> {
        self.validate()?;
        if &self.request_id != request.request_id()
            || self.target_set != *request.target_set()
            || self.limit != request.bounds().limit()
        {
            return Err(Error::FetchPageRequestMismatch);
        }
        Ok(())
    }

    /// Returns the request identity.
    pub const fn request_id(&self) -> &FetchRequestId {
        &self.request_id
    }

    /// Returns the observations in adapter order.
    pub fn events(&self) -> &[ObservedEvent] {
        self.events.as_slice()
    }

    /// Returns zero or more target-specific outcomes; omitted targets remain unreported.
    pub fn target_outcomes(&self) -> &[FetchTargetOutcome] {
        self.target_outcomes.as_slice()
    }

    /// Returns continuation, completion, or cancellation state.
    pub const fn next_page(&self) -> &NextPage {
        &self.next_page
    }
}

/// Host SPI for inbound event retrieval.
///
/// This trait supports external implementations and is dyn-compatible. Its
/// futures are `Send`; implementations must not borrow request data after a
/// future completes. `status` observes source state and does not initiate a
/// fetch. `fetch` performs at most the work bounded by its request and owns no
/// hidden retry loop.
///
/// Dropping a returned future requests cancellation. If it is dropped before
/// a remote request is published, the implementation must leave no remote
/// operation behind. Once publication may have occurred, cancellation cannot
/// claim rollback; a later observation may report the remote outcome. The
/// explicit request deadline bounds work independently of future cancellation.
pub trait EventSource: Send + Sync {
    /// Returns the source's current runtime status.
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>>;

    /// Fetches one bounded page of transport-neutral events.
    fn fetch(&self, request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>>;
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;

    impl serde::Serialize for FetchRequestId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> serde::Deserialize<'de> for FetchRequestId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = <String as serde::Deserialize>::deserialize(deserializer)?;
            Self::parse(value).map_err(serde::de::Error::custom)
        }
    }

    impl serde::Serialize for FetchCursor {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> serde::Deserialize<'de> for FetchCursor {
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
    struct FetchBoundsWire {
        limit: u16,
        deadline_unix_ms: u64,
    }

    impl<'de> serde::Deserialize<'de> for FetchBounds {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = FetchBoundsWire::deserialize(deserializer)?;
            Self::new(wire.limit, wire.deadline_unix_ms).map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FetchRequestWire {
        request_id: String,
        target_set: TargetSet,
        bounds: FetchBounds,
        cursor: Option<FetchCursor>,
    }

    impl<'de> serde::Deserialize<'de> for FetchRequest {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = FetchRequestWire::deserialize(deserializer)?;
            Self::new(wire.request_id, wire.target_set, wire.bounds)
                .map(|request| match wire.cursor {
                    Some(cursor) => request.with_cursor(cursor),
                    None => request,
                })
                .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EventProvenanceWire {
        transport_id: TransportId,
        target: TargetFingerprint,
        observed_at_unix_ms: u64,
        cursor: Option<FetchCursor>,
    }

    impl<'de> serde::Deserialize<'de> for EventProvenance {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = EventProvenanceWire::deserialize(deserializer)?;
            Self::new(wire.transport_id, wire.target, wire.observed_at_unix_ms)
                .map(|provenance| match wire.cursor {
                    Some(cursor) => provenance.with_cursor(cursor),
                    None => provenance,
                })
                .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FetchPageWire {
        request_id: FetchRequestId,
        target_set: TargetSet,
        limit: u16,
        events: Vec<ObservedEvent>,
        target_outcomes: Vec<FetchTargetOutcome>,
        next_page: NextPage,
    }

    impl<'de> serde::Deserialize<'de> for FetchPage {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = FetchPageWire::deserialize(deserializer)?;
            let page = Self {
                request_id: wire.request_id,
                target_set: wire.target_set,
                limit: wire.limit,
                events: wire.events,
                target_outcomes: wire.target_outcomes,
                next_page: wire.next_page,
            };
            page.validate().map_err(serde::de::Error::custom)?;
            Ok(page)
        }
    }
}
