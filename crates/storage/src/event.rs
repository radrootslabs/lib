//! Canonical event persistence contracts.

pub use radroots_event::EventId;
use radroots_event::{SignedEvent, VerifiedEvent, admission::VisibleEvent};
pub use radroots_transport::BoxFuture;
use radroots_transport::{
    TransportId,
    source::{EventProvenance, FetchCursor, ObservedEvent},
    target::TargetFingerprint,
};
use std::collections::BTreeSet;

use crate::{Error, status::EventStoreStatus};

mod visibility;
#[doc(hidden)]
pub use visibility::{VisibilityEvaluation, VisibilityInput, evaluate_visibility};

/// Maximum events returned by one storage query.
pub const EVENT_QUERY_LIMIT_MAX: u16 = 1_000;
/// Maximum explicit event identifiers in one storage query.
pub const EVENT_QUERY_ID_MAX: usize = 256;
/// Opaque identity of one append-only canonical event source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceGeneration([u8; 32]);

impl SourceGeneration {
    /// Creates a generation from host-provided entropy.
    pub const fn new(bytes: [u8; 32]) -> Result<Self, Error> {
        if is_all_zero(&bytes) {
            return Err(Error::InvalidSourceGeneration);
        }
        Ok(Self(bytes))
    }

    /// Returns the opaque generation bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

const fn is_all_zero(bytes: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Non-zero sequence within one source generation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventSequence(u64);

impl EventSequence {
    /// Creates a non-zero source-local sequence.
    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            return Err(Error::InvalidEventSequence);
        }
        Ok(Self(value))
    }

    /// Returns the source-local sequence.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable location of an event within one source generation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventPosition {
    generation: SourceGeneration,
    sequence: EventSequence,
}

impl EventPosition {
    /// Creates a source position.
    pub const fn new(generation: SourceGeneration, sequence: EventSequence) -> Self {
        Self {
            generation,
            sequence,
        }
    }

    /// Returns the source generation.
    pub const fn generation(self) -> SourceGeneration {
        self.generation
    }

    /// Returns the generation-local sequence.
    pub const fn sequence(self) -> EventSequence {
        self.sequence
    }
}

/// Cursor after which a query resumes.
pub type EventCursor = EventPosition;

/// Validated bounds for a canonical event query.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventQueryBounds {
    limit: u16,
    after: Option<EventCursor>,
}

impl EventQueryBounds {
    /// Creates bounds for a first-page query.
    pub const fn first(limit: u16) -> Result<Self, Error> {
        if limit == 0 || limit > EVENT_QUERY_LIMIT_MAX {
            return Err(Error::InvalidEventQueryLimit);
        }
        Ok(Self { limit, after: None })
    }

    /// Resumes strictly after a prior cursor.
    #[must_use]
    pub const fn after(mut self, cursor: EventCursor) -> Self {
        self.after = Some(cursor);
        self
    }

    /// Returns the maximum number of records.
    pub const fn limit(self) -> u16 {
        self.limit
    }

    /// Returns the optional exclusive cursor.
    pub const fn cursor(self) -> Option<EventCursor> {
        self.after
    }
}

/// Bounded event selection; an empty identifier set selects every event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventQuery {
    bounds: EventQueryBounds,
    event_ids: Vec<EventId>,
}

impl EventQuery {
    /// Selects all events under the supplied bounds.
    pub const fn all(bounds: EventQueryBounds) -> Self {
        Self {
            bounds,
            event_ids: Vec::new(),
        }
    }

    /// Selects a bounded, duplicate-free set of event identifiers.
    pub fn for_ids(bounds: EventQueryBounds, event_ids: Vec<EventId>) -> Result<Self, Error> {
        if event_ids.is_empty() {
            return Err(Error::EmptyEventQueryIds);
        }
        if event_ids.len() > EVENT_QUERY_ID_MAX {
            return Err(Error::TooManyEventQueryIds);
        }
        let unique = event_ids.iter().collect::<BTreeSet<_>>();
        if unique.len() != event_ids.len() {
            return Err(Error::DuplicateEventQueryId);
        }
        Ok(Self { bounds, event_ids })
    }

    /// Returns the query bounds.
    pub const fn bounds(&self) -> EventQueryBounds {
        self.bounds
    }

    /// Returns the selected identifiers; empty means all.
    pub fn event_ids(&self) -> &[EventId] {
        self.event_ids.as_slice()
    }

    /// Reports whether an identifier is selected.
    pub fn selects(&self, event_id: &EventId) -> bool {
        self.event_ids.is_empty() || self.event_ids.contains(event_id)
    }
}

/// Durable event admission stage.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionStage {
    /// Structurally valid and ID-checked, but not signature verified.
    Raw,
    /// Canonical identifier and signature verified.
    Verified,
    /// Contract-admitted and visibility-authorized.
    Visible,
}

/// One canonical event admission with exact transport provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventAdmission {
    observed: ObservedEvent,
    state: AdmissionState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AdmissionState {
    Raw,
    Verified(VerifiedEvent),
    Visible(VisibleEvent),
}

impl EventAdmission {
    /// Retains an observed signed event without claiming signature verification.
    pub const fn raw(observed: ObservedEvent) -> Self {
        Self {
            observed,
            state: AdmissionState::Raw,
        }
    }

    /// Retains a verified event after proving it matches the observed payload.
    pub fn verified(observed: ObservedEvent, verified: VerifiedEvent) -> Result<Self, Error> {
        if observed.event().envelope() != verified.event() {
            return Err(Error::AdmissionEventMismatch);
        }
        Ok(Self {
            observed,
            state: AdmissionState::Verified(verified),
        })
    }

    /// Retains a visible event after proving it matches the observed payload.
    pub fn visible(observed: ObservedEvent, visible: VisibleEvent) -> Result<Self, Error> {
        if observed.event().envelope() != visible.event() {
            return Err(Error::AdmissionEventMismatch);
        }
        Ok(Self {
            observed,
            state: AdmissionState::Visible(visible),
        })
    }

    /// Returns the durable stage represented by this admission.
    pub const fn stage(&self) -> AdmissionStage {
        match self.state {
            AdmissionState::Raw => AdmissionStage::Raw,
            AdmissionState::Verified(_) => AdmissionStage::Verified,
            AdmissionState::Visible(_) => AdmissionStage::Visible,
        }
    }

    /// Returns the exact observed signed event.
    pub const fn event(&self) -> &SignedEvent {
        self.observed.event()
    }

    /// Returns the event identifier.
    pub fn event_id(&self) -> &EventId {
        self.event().id()
    }

    /// Returns the transport observation attached to this admission.
    pub const fn provenance(&self) -> &EventProvenance {
        self.observed.provenance()
    }

    /// Returns the verified event when this admission reached verification.
    pub const fn verified_event(&self) -> Option<&VerifiedEvent> {
        match &self.state {
            AdmissionState::Raw => None,
            AdmissionState::Verified(event) => Some(event),
            AdmissionState::Visible(event) => {
                Some(event.admitted_event().validated_event().verified_event())
            }
        }
    }

    /// Returns the visible event when visibility was authorized.
    pub const fn visible_event(&self) -> Option<&VisibleEvent> {
        match &self.state {
            AdmissionState::Visible(event) => Some(event),
            AdmissionState::Raw | AdmissionState::Verified(_) => None,
        }
    }
}

/// Persistence result for one idempotent admission.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionDisposition {
    Inserted,
    Advanced,
    Duplicate,
}

/// Request-bound durable admission receipt.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionReceipt {
    event_id: EventId,
    position: EventPosition,
    stage: AdmissionStage,
    disposition: AdmissionDisposition,
}

impl AdmissionReceipt {
    /// Creates a backend receipt from validated durable state.
    pub const fn new(
        event_id: EventId,
        position: EventPosition,
        stage: AdmissionStage,
        disposition: AdmissionDisposition,
    ) -> Self {
        Self {
            event_id,
            position,
            stage,
            disposition,
        }
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn position(&self) -> EventPosition {
        self.position
    }

    pub const fn stage(&self) -> AdmissionStage {
        self.stage
    }

    pub const fn disposition(&self) -> AdmissionDisposition {
        self.disposition
    }
}

/// Raw event returned from canonical storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRawEvent {
    position: EventPosition,
    event: SignedEvent,
    stage: AdmissionStage,
}

impl StoredRawEvent {
    pub const fn new(position: EventPosition, event: SignedEvent, stage: AdmissionStage) -> Self {
        Self {
            position,
            event,
            stage,
        }
    }

    pub const fn position(&self) -> EventPosition {
        self.position
    }

    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }

    pub const fn stage(&self) -> AdmissionStage {
        self.stage
    }
}

/// Canonical event returned with durable signature-verification evidence.
///
/// The signed event is returned rather than forging an in-memory verification
/// typestate from persisted bytes. [`EventStore`] guarantees that only records
/// durably admitted at [`AdmissionStage::Verified`] or later appear here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredVerifiedEvent {
    position: EventPosition,
    event: SignedEvent,
}

impl StoredVerifiedEvent {
    pub const fn new(position: EventPosition, event: SignedEvent) -> Self {
        Self { position, event }
    }

    pub const fn position(&self) -> EventPosition {
        self.position
    }

    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
}

/// Canonical event returned with durable visibility evidence.
///
/// The signed event is returned rather than rerunning a host authorization
/// policy during a storage read. [`EventStore`] guarantees that only records
/// durably admitted at [`AdmissionStage::Visible`] appear here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredVisibleEvent {
    position: EventPosition,
    event: SignedEvent,
}

/// Deterministic digest of one complete visibility rebuild.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VisibilityDigest([u8; 32]);

impl VisibilityDigest {
    /// Returns the canonical SHA-256 bytes for the rebuilt visibility state.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Complete deterministic result of rebuilding current event visibility.
///
/// Current heads remain listed even when their selected event is suppressed.
/// This prevents a deleted head from resurrecting an older revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibilitySnapshot {
    generation: SourceGeneration,
    current_heads: Vec<radroots_event::envelope::event_head::CurrentEventHead>,
    deletion_request_ids: Vec<EventId>,
    visible_event_ids: Vec<EventId>,
    suppressed_event_ids: Vec<EventId>,
    superseded_event_ids: Vec<EventId>,
    digest: VisibilityDigest,
}

impl VisibilitySnapshot {
    pub(crate) const fn new(
        generation: SourceGeneration,
        current_heads: Vec<radroots_event::envelope::event_head::CurrentEventHead>,
        deletion_request_ids: Vec<EventId>,
        visible_event_ids: Vec<EventId>,
        suppressed_event_ids: Vec<EventId>,
        superseded_event_ids: Vec<EventId>,
        digest: VisibilityDigest,
    ) -> Self {
        Self {
            generation,
            current_heads,
            deletion_request_ids,
            visible_event_ids,
            suppressed_event_ids,
            superseded_event_ids,
            digest,
        }
    }

    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    pub fn current_heads(&self) -> &[radroots_event::envelope::event_head::CurrentEventHead] {
        self.current_heads.as_slice()
    }

    pub fn deletion_request_ids(&self) -> &[EventId] {
        self.deletion_request_ids.as_slice()
    }

    pub fn visible_event_ids(&self) -> &[EventId] {
        self.visible_event_ids.as_slice()
    }

    pub fn suppressed_event_ids(&self) -> &[EventId] {
        self.suppressed_event_ids.as_slice()
    }

    pub fn superseded_event_ids(&self) -> &[EventId] {
        self.superseded_event_ids.as_slice()
    }

    pub const fn digest(&self) -> VisibilityDigest {
        self.digest
    }
}

impl StoredVisibleEvent {
    pub const fn new(position: EventPosition, event: SignedEvent) -> Self {
        Self { position, event }
    }

    pub const fn position(&self) -> EventPosition {
        self.position
    }

    pub const fn event(&self) -> &SignedEvent {
        &self.event
    }
}

/// One bounded, generation-consistent page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventPage<T> {
    generation: SourceGeneration,
    items: Vec<T>,
    next: Option<EventCursor>,
}

impl<T> EventPage<T> {
    /// Creates a page and enforces its caller-supplied item bound.
    pub fn new(
        generation: SourceGeneration,
        items: Vec<T>,
        next: Option<EventCursor>,
        bounds: EventQueryBounds,
    ) -> Result<Self, Error> {
        if items.len() > usize::from(bounds.limit()) {
            return Err(Error::EventPageLimitExceeded);
        }
        if let Some(cursor) = next
            && cursor.generation() != generation
        {
            return Err(Error::CursorGenerationMismatch);
        }
        Ok(Self {
            generation,
            items,
            next,
        })
    }

    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    pub fn items(&self) -> &[T] {
        self.items.as_slice()
    }

    pub const fn next_cursor(&self) -> Option<EventCursor> {
        self.next
    }
}

/// Provenance retained for one event observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredEventProvenance {
    position: EventPosition,
    provenance: EventProvenance,
}

impl StoredEventProvenance {
    pub const fn new(position: EventPosition, provenance: EventProvenance) -> Self {
        Self {
            position,
            provenance,
        }
    }

    pub const fn position(&self) -> EventPosition {
        self.position
    }

    pub const fn provenance(&self) -> &EventProvenance {
        &self.provenance
    }

    /// Reconstructs validated backend-neutral provenance from durable fields.
    pub fn from_stored_parts(
        position: EventPosition,
        transport_id: &str,
        target_fingerprint: &str,
        observed_at_unix_ms: u64,
        cursor: Option<&str>,
    ) -> Result<Self, Error> {
        let transport_id =
            TransportId::parse(transport_id).map_err(|_| Error::CorruptStoredEvent)?;
        let target =
            TargetFingerprint::parse(target_fingerprint).map_err(|_| Error::CorruptStoredEvent)?;
        let mut provenance = EventProvenance::new(transport_id, target, observed_at_unix_ms)
            .map_err(|_| Error::CorruptStoredEvent)?;
        if let Some(cursor) = cursor {
            provenance = provenance
                .with_cursor(FetchCursor::parse(cursor).map_err(|_| Error::CorruptStoredEvent)?);
        }
        Ok(Self::new(position, provenance))
    }
}

/// Backend-neutral canonical event storage SPI.
///
/// Implementations are dyn-compatible and return `Send` futures. They may not
/// expose backend transactions, handles, SQL, or filesystem paths. Admission is
/// idempotent for an identical event and provenance observation; a stage may
/// advance but never regress. Query cursors are bound to one source generation.
pub trait EventStore: Send + Sync {
    /// Returns passive event-store status without initiating maintenance.
    fn status(&self) -> BoxFuture<'_, Result<EventStoreStatus, Error>>;

    /// Durably admits or advances one canonical event observation.
    fn admit(&self, admission: EventAdmission) -> BoxFuture<'_, Result<AdmissionReceipt, Error>>;

    /// Queries retained raw events.
    fn query_raw(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredRawEvent>, Error>>;

    /// Queries signature-verified events.
    fn query_verified(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVerifiedEvent>, Error>>;

    /// Queries visibility-authorized events.
    fn query_visible(
        &self,
        query: EventQuery,
    ) -> BoxFuture<'_, Result<EventPage<StoredVisibleEvent>, Error>>;

    /// Rebuilds current visibility from immutable retained event truth.
    ///
    /// Implementations must use the same reducer as [`Self::query_visible`].
    fn rebuild_visibility(&self) -> BoxFuture<'_, Result<VisibilitySnapshot, Error>>;

    /// Queries bounded provenance for one event.
    fn query_provenance(
        &self,
        event_id: EventId,
        bounds: EventQueryBounds,
    ) -> BoxFuture<'_, Result<EventPage<StoredEventProvenance>, Error>>;
}
