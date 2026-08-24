//! Inbound event source SPI and bounded page models.

use crate::{
    Error, TransportId,
    outcome::FetchTargetOutcome,
    target::{TargetFingerprint, TargetSet},
};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::{fmt, future::Future, pin::Pin};
use radroots_event::SignedEvent;
use radroots_identity::PublicKey;

#[cfg(feature = "serde")]
use crate::target::TARGET_SET_MAX_ITEMS;

pub use crate::status::SourceStatus;

/// Maximum encoded request identity length.
pub const FETCH_REQUEST_ID_MAX_BYTES: usize = 256;
/// Maximum opaque cursor length.
pub const FETCH_CURSOR_MAX_BYTES: usize = 2_048;
/// Maximum number of events one page may request.
pub const FETCH_PAGE_MAX_EVENTS: u16 = 1_000;
/// Maximum distinct event kinds in one source selector.
pub const FETCH_SELECTOR_MAX_KINDS: usize = 64;
/// Maximum distinct event authors in one source selector.
pub const FETCH_SELECTOR_MAX_AUTHORS: usize = 256;
/// Maximum distinct exact single-letter tag keys in one source selector.
pub const FETCH_SELECTOR_MAX_TAG_KEYS: usize = 26;
/// Maximum exact tag values across one source selector.
pub const FETCH_SELECTOR_MAX_TAG_VALUES: usize = 256;
/// Maximum UTF-8 bytes in one exact tag value.
pub const FETCH_SELECTOR_TAG_VALUE_MAX_BYTES: usize = 4_096;

// Deliberate representation indirection keeps every selector-bearing request
// and terminal value compact while allocating nothing for the common no-tag
// case. The map itself still owns its bounded tree nodes.
#[allow(clippy::box_collection)]
type ExactTagFilters = Box<BTreeMap<char, Vec<String>>>;

/// Maximum encoded live-subscription request identity length.
pub const SUBSCRIPTION_REQUEST_ID_MAX_BYTES: usize = 256;
/// Maximum number of events one live subscription may emit.
pub const SUBSCRIPTION_MAX_EVENTS: u16 = 1_000;

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

/// Validated caller identity for one bounded live subscription.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SubscriptionRequestId(String);

impl SubscriptionRequestId {
    /// Parses a non-empty, bounded, printable request identity.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Error> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(Error::EmptySubscriptionRequestId);
        }
        if value.len() > SUBSCRIPTION_REQUEST_ID_MAX_BYTES
            || value != value.trim()
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidSubscriptionRequestId);
        }
        Ok(Self(String::from(value)))
    }

    /// Returns the validated request identity.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SubscriptionRequestId {
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

/// Hard bounds for one live-subscription operation.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionBounds {
    event_limit: u16,
    deadline_unix_ms: u64,
}

impl SubscriptionBounds {
    /// Creates bounds with a non-zero event limit and absolute deadline.
    pub const fn new(event_limit: u16, deadline_unix_ms: u64) -> Result<Self, Error> {
        if event_limit == 0 || event_limit > SUBSCRIPTION_MAX_EVENTS {
            return Err(Error::InvalidSubscriptionLimit);
        }
        if deadline_unix_ms == 0 {
            return Err(Error::InvalidSubscriptionDeadline);
        }
        Ok(Self {
            event_limit,
            deadline_unix_ms,
        })
    }

    /// Maximum number of events the adapter may emit.
    pub const fn event_limit(self) -> u16 {
        self.event_limit
    }

    /// Absolute Unix deadline in milliseconds.
    pub const fn deadline_unix_ms(self) -> u64 {
        self.deadline_unix_ms
    }
}

/// Transport-neutral constraints applied before a source page is bounded.
///
/// An empty kind, author, or tag collection means "any" for that dimension.
/// Values for one tag key are alternatives, while distinct tag keys are
/// conjunctive. Time bounds are inclusive Unix seconds. Adapters must apply
/// every configured dimension remotely when their protocol supports it and
/// must defensively exclude non-matching events before returning a page.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchSelector {
    kinds: Vec<u32>,
    authors: Vec<PublicKey>,
    #[cfg_attr(
        feature = "serde",
        serde(serialize_with = "serde_impl::serialize_exact_tags")
    )]
    exact_tags: Option<ExactTagFilters>,
    since_unix_seconds: Option<u64>,
    until_unix_seconds: Option<u64>,
}

impl FetchSelector {
    /// Creates a selector that accepts every event within request bounds.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            kinds: Vec::new(),
            authors: Vec::new(),
            exact_tags: None,
            since_unix_seconds: None,
            until_unix_seconds: None,
        }
    }

    /// Restricts the selector to exact, unique event kinds.
    pub fn with_kinds(mut self, mut kinds: Vec<u32>) -> Result<Self, Error> {
        if kinds.len() > FETCH_SELECTOR_MAX_KINDS {
            return Err(Error::FetchSelectorTooLarge);
        }
        kinds.sort_unstable();
        if kinds.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::DuplicateFetchKind);
        }
        self.kinds = kinds;
        Ok(self)
    }

    /// Restricts the selector to exact, unique canonical authors.
    pub fn with_authors(mut self, mut authors: Vec<PublicKey>) -> Result<Self, Error> {
        if authors.len() > FETCH_SELECTOR_MAX_AUTHORS {
            return Err(Error::FetchSelectorTooLarge);
        }
        authors.sort();
        if authors.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::DuplicateFetchAuthor);
        }
        self.authors = authors;
        Ok(self)
    }

    /// Requires one exact indexed single-letter tag value.
    ///
    /// Repeating a key adds an alternative value for that key. Different keys
    /// are conjunctive. Keys are lowercase ASCII letters and values are
    /// non-empty bounded UTF-8 strings.
    pub fn with_exact_tag_value(
        mut self,
        key: char,
        value: impl AsRef<str>,
    ) -> Result<Self, Error> {
        if !key.is_ascii_lowercase() {
            return Err(Error::InvalidFetchTagKey);
        }
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > FETCH_SELECTOR_TAG_VALUE_MAX_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(Error::InvalidFetchTagValue);
        }
        let exact_tags = self.exact_tags.as_deref();
        let total_values = exact_tags
            .into_iter()
            .flat_map(BTreeMap::values)
            .map(Vec::len)
            .sum::<usize>();
        if (!exact_tags.is_some_and(|tags| tags.contains_key(&key))
            && exact_tags.is_some_and(|tags| tags.len() == FETCH_SELECTOR_MAX_TAG_KEYS))
            || total_values == FETCH_SELECTOR_MAX_TAG_VALUES
        {
            return Err(Error::FetchSelectorTooLarge);
        }
        let values = self
            .exact_tags
            .get_or_insert_with(|| Box::new(BTreeMap::new()))
            .entry(key)
            .or_default();
        match values.binary_search_by(|candidate| candidate.as_str().cmp(value)) {
            Ok(_) => return Err(Error::DuplicateFetchTagValue),
            Err(position) => values.insert(position, String::from(value)),
        }
        Ok(self)
    }

    /// Sets an inclusive lower event-time bound.
    pub fn with_since_unix_seconds(mut self, since: u64) -> Result<Self, Error> {
        if self.until_unix_seconds.is_some_and(|until| since > until) {
            return Err(Error::InvalidFetchTimeRange);
        }
        self.since_unix_seconds = Some(since);
        Ok(self)
    }

    /// Sets an inclusive upper event-time bound.
    pub fn with_until_unix_seconds(mut self, until: u64) -> Result<Self, Error> {
        if self.since_unix_seconds.is_some_and(|since| since > until) {
            return Err(Error::InvalidFetchTimeRange);
        }
        self.until_unix_seconds = Some(until);
        Ok(self)
    }

    /// Returns sorted exact event kinds, or an empty slice for any kind.
    pub fn kinds(&self) -> &[u32] {
        self.kinds.as_slice()
    }

    /// Returns sorted exact authors, or an empty slice for any author.
    pub fn authors(&self) -> &[PublicKey] {
        self.authors.as_slice()
    }

    /// Returns exact tag filters in canonical key order.
    pub fn exact_tag_filters(&self) -> impl Iterator<Item = (char, &[String])> + '_ {
        self.exact_tags
            .iter()
            .flat_map(|tags| tags.iter())
            .map(|(key, values)| (*key, values.as_slice()))
    }

    /// Returns the inclusive lower event-time bound.
    pub const fn since_unix_seconds(&self) -> Option<u64> {
        self.since_unix_seconds
    }

    /// Returns the inclusive upper event-time bound.
    pub const fn until_unix_seconds(&self) -> Option<u64> {
        self.until_unix_seconds
    }

    /// Returns whether one canonical signed event satisfies every dimension.
    #[must_use]
    pub fn matches(&self, event: &SignedEvent) -> bool {
        (self.kinds.is_empty() || self.kinds.binary_search(&event.kind()).is_ok())
            && (self.authors.is_empty() || self.authors.binary_search(event.pubkey()).is_ok())
            && self.exact_tags.as_deref().is_none_or(|tags| {
                tags.iter().all(|(key, values)| {
                    event.envelope().tag_slices().iter().any(|tag| {
                        let elements = tag.as_slice();
                        elements.first().is_some_and(|candidate| {
                            candidate.len() == 1 && candidate.starts_with(*key)
                        }) && elements.get(1).is_some_and(|value| {
                            values
                                .binary_search_by(|candidate| candidate.as_str().cmp(value))
                                .is_ok()
                        })
                    })
                })
            })
            && self
                .since_unix_seconds
                .is_none_or(|since| event.created_at() >= since)
            && self
                .until_unix_seconds
                .is_none_or(|until| event.created_at() <= until)
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
    selector: FetchSelector,
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
            selector: FetchSelector::all(),
        })
    }

    /// Sets the adapter-owned cursor for a continuation request.
    #[must_use]
    pub fn with_cursor(mut self, cursor: FetchCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Applies explicit transport-neutral event constraints.
    #[must_use]
    pub fn with_selector(mut self, selector: FetchSelector) -> Self {
        self.selector = selector;
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

    /// Returns the exact event constraints for this request.
    pub const fn selector(&self) -> &FetchSelector {
        &self.selector
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
    selector: FetchSelector,
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
            selector: request.selector.clone(),
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
            if !self.selector.matches(observed.event()) {
                return Err(Error::UnexpectedFetchEvent);
            }
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
            || self.selector != *request.selector()
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

/// Per-target continuation point for a live subscription.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionCheckpoint {
    target: TargetFingerprint,
    cursor: FetchCursor,
}

impl SubscriptionCheckpoint {
    /// Binds an opaque adapter cursor to one exact target.
    pub const fn new(target: TargetFingerprint, cursor: FetchCursor) -> Self {
        Self { target, cursor }
    }

    /// Returns the exact target fingerprint.
    pub const fn target(&self) -> &TargetFingerprint {
        &self.target
    }

    /// Returns the opaque adapter cursor.
    pub const fn cursor(&self) -> &FetchCursor {
        &self.cursor
    }
}

/// Bounded request for a live stream from one or more transport targets.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionRequest {
    request_id: SubscriptionRequestId,
    target_set: TargetSet,
    bounds: SubscriptionBounds,
    selector: FetchSelector,
    checkpoints: Vec<SubscriptionCheckpoint>,
}

impl SubscriptionRequest {
    /// Creates a live request with no prior target checkpoints.
    pub fn new(
        request_id: impl AsRef<str>,
        target_set: TargetSet,
        bounds: SubscriptionBounds,
    ) -> Result<Self, Error> {
        Ok(Self {
            request_id: SubscriptionRequestId::parse(request_id)?,
            target_set,
            bounds,
            selector: FetchSelector::all(),
            checkpoints: Vec::new(),
        })
    }

    /// Applies explicit transport-neutral event constraints.
    #[must_use]
    pub fn with_selector(mut self, selector: FetchSelector) -> Self {
        self.selector = selector;
        self
    }

    /// Applies a bounded, unique checkpoint subset in canonical target order.
    pub fn with_checkpoints<I>(mut self, checkpoints: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = SubscriptionCheckpoint>,
    {
        self.checkpoints = normalize_subscription_checkpoints(&self.target_set, checkpoints)?;
        Ok(self)
    }

    /// Returns the request identity.
    pub const fn request_id(&self) -> &SubscriptionRequestId {
        &self.request_id
    }

    /// Returns the exact requested target set.
    pub const fn target_set(&self) -> &TargetSet {
        &self.target_set
    }

    /// Returns the hard operation bounds.
    pub const fn bounds(&self) -> SubscriptionBounds {
        self.bounds
    }

    /// Returns the exact event constraints for this request.
    pub const fn selector(&self) -> &FetchSelector {
        &self.selector
    }

    /// Returns prior per-target checkpoints in canonical target order.
    pub fn checkpoints(&self) -> &[SubscriptionCheckpoint] {
        self.checkpoints.as_slice()
    }
}

/// One request-bound live event and its resulting per-target checkpoint.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionEvent {
    request: SubscriptionRequest,
    observed: ObservedEvent,
    checkpoint: SubscriptionCheckpoint,
}

impl SubscriptionEvent {
    /// Creates and validates one live event against its originating request.
    pub fn for_request(
        request: &SubscriptionRequest,
        observed: ObservedEvent,
        checkpoint: SubscriptionCheckpoint,
    ) -> Result<Self, Error> {
        let event = Self {
            request: request.clone(),
            observed,
            checkpoint,
        };
        event.validate_for_request(request)?;
        Ok(event)
    }

    /// Validates target, transport, selector, cursor, and request identity.
    pub fn validate_for_request(&self, request: &SubscriptionRequest) -> Result<(), Error> {
        if self.request != *request {
            return Err(Error::UnexpectedSubscriptionEvent);
        }
        let provenance = self.observed.provenance();
        let Some(target) = request
            .target_set
            .targets()
            .iter()
            .find(|target| target.fingerprint() == provenance.target())
        else {
            return Err(Error::UnexpectedSubscriptionEvent);
        };
        if *target.kind() != provenance.transport_id()
            || !request.selector.matches(self.observed.event())
        {
            return Err(Error::UnexpectedSubscriptionEvent);
        }
        if self.checkpoint.target() != provenance.target()
            || provenance.cursor() != Some(self.checkpoint.cursor())
        {
            return Err(Error::SubscriptionEventCheckpointMismatch);
        }
        Ok(())
    }

    /// Returns the request identity.
    pub const fn request_id(&self) -> &SubscriptionRequestId {
        self.request.request_id()
    }

    /// Returns the exact originating request.
    pub const fn request(&self) -> &SubscriptionRequest {
        &self.request
    }

    /// Returns the observed event.
    pub const fn observed(&self) -> &ObservedEvent {
        &self.observed
    }

    /// Returns the checkpoint established by this event.
    pub const fn checkpoint(&self) -> &SubscriptionCheckpoint {
        &self.checkpoint
    }
}

/// Stable terminal reason for a bounded live subscription.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionEndReason {
    /// The requested maximum event count was emitted.
    EventLimit,
    /// The absolute request deadline was reached.
    Deadline,
    /// Explicit or future-drop cancellation was observed.
    Cancelled,
    /// The underlying source closed before another event was available.
    SourceClosed,
}

/// Request-bound terminal result for a live subscription.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionEnd {
    request: SubscriptionRequest,
    event_count: u16,
    checkpoints: Vec<SubscriptionCheckpoint>,
    reason: SubscriptionEndReason,
}

impl SubscriptionEnd {
    /// Creates a terminal result with canonical final checkpoints.
    pub fn for_request<I>(
        request: &SubscriptionRequest,
        event_count: u16,
        checkpoints: I,
        reason: SubscriptionEndReason,
    ) -> Result<Self, Error>
    where
        I: IntoIterator<Item = SubscriptionCheckpoint>,
    {
        if event_count > request.bounds.event_limit {
            return Err(Error::SubscriptionEndLimitExceeded);
        }
        if reason == SubscriptionEndReason::EventLimit && event_count != request.bounds.event_limit
        {
            return Err(Error::InvalidSubscriptionEnd);
        }
        Ok(Self {
            request: request.clone(),
            event_count,
            checkpoints: normalize_subscription_checkpoints(&request.target_set, checkpoints)?,
            reason,
        })
    }

    /// Validates that this result belongs to the exact originating request.
    pub fn validate_for_request(&self, request: &SubscriptionRequest) -> Result<(), Error> {
        if self.request != *request || self.event_count > request.bounds.event_limit {
            return Err(Error::SubscriptionEndRequestMismatch);
        }
        normalize_subscription_checkpoints(&request.target_set, self.checkpoints.clone())?;
        Ok(())
    }

    /// Returns the exact originating request.
    pub const fn request(&self) -> &SubscriptionRequest {
        &self.request
    }

    /// Returns the number of events emitted before termination.
    pub const fn event_count(&self) -> u16 {
        self.event_count
    }

    /// Returns final per-target checkpoints in canonical target order.
    pub fn checkpoints(&self) -> &[SubscriptionCheckpoint] {
        self.checkpoints.as_slice()
    }

    /// Returns why the bounded operation terminated.
    pub const fn reason(&self) -> SubscriptionEndReason {
        self.reason
    }
}

/// One event or the stable terminal result from a live subscription.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionNext {
    /// One request-bound event was observed.
    Event(Box<SubscriptionEvent>),
    /// The subscription reached its stable terminal state.
    End(SubscriptionEnd),
}

fn normalize_subscription_checkpoints<I>(
    target_set: &TargetSet,
    checkpoints: I,
) -> Result<Vec<SubscriptionCheckpoint>, Error>
where
    I: IntoIterator<Item = SubscriptionCheckpoint>,
{
    let maximum = target_set.len();
    let mut checkpoints: Vec<_> = checkpoints.into_iter().take(maximum + 1).collect();
    if checkpoints.len() > maximum {
        return Err(Error::SubscriptionCheckpointSetTooLarge);
    }

    let mut seen = BTreeSet::new();
    for checkpoint in &checkpoints {
        if target_position(target_set, checkpoint.target()).is_none() {
            return Err(Error::UnexpectedSubscriptionCheckpoint);
        }
        if !seen.insert(checkpoint.target().as_str()) {
            return Err(Error::DuplicateSubscriptionCheckpoint);
        }
    }
    checkpoints.sort_by_key(|checkpoint| {
        target_position(target_set, checkpoint.target()).expect("checkpoint target validated")
    });
    Ok(checkpoints)
}

fn target_position(target_set: &TargetSet, fingerprint: &TargetFingerprint) -> Option<usize> {
    target_set
        .targets()
        .iter()
        .position(|target| target.fingerprint() == fingerprint)
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

/// One active, bounded live-subscription operation.
///
/// Implementations must enforce the request's event limit and absolute
/// deadline without hidden retries. Once [`SubscriptionNext::End`] has been
/// returned, every later `next` or `cancel` call must return the exact same
/// terminal result. Dropping a pending future requests cancellation but does
/// not claim that an already-observed remote event was rolled back.
pub trait EventSubscription: Send {
    /// Returns the exact request governing this operation.
    fn request(&self) -> &SubscriptionRequest;

    /// Returns the next request-bound event or stable terminal result.
    fn next(&mut self) -> BoxFuture<'_, Result<SubscriptionNext, Error>>;

    /// Requests cancellation and returns the stable terminal result.
    fn cancel(&mut self) -> BoxFuture<'_, Result<SubscriptionEnd, Error>>;
}

/// Heap-owned live-subscription capability returned by adapters.
pub type BoxSubscription = Box<dyn EventSubscription>;

/// Host SPI for beginning bounded live subscriptions.
///
/// This is separate from [`EventSource`] so existing bounded-fetch producers
/// remain source-compatible until they explicitly adopt live delivery.
pub trait EventSubscriber: Send + Sync {
    /// Begins one exact bounded live-subscription request.
    fn subscribe(
        &self,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<BoxSubscription, Error>>;
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;

    pub(super) fn serialize_exact_tags<S>(
        exact_tags: &Option<ExactTagFilters>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match exact_tags {
            Some(exact_tags) => serde::Serialize::serialize(exact_tags, serializer),
            None => serde::Serialize::serialize(&BTreeMap::<char, Vec<String>>::new(), serializer),
        }
    }

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

    impl serde::Serialize for SubscriptionRequestId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> serde::Deserialize<'de> for SubscriptionRequestId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = <String as serde::Deserialize>::deserialize(deserializer)?;
            Self::parse(value.as_str()).map_err(serde::de::Error::custom)
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
    struct SubscriptionBoundsWire {
        event_limit: u16,
        deadline_unix_ms: u64,
    }

    impl<'de> serde::Deserialize<'de> for SubscriptionBounds {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = SubscriptionBoundsWire::deserialize(deserializer)?;
            Self::new(wire.event_limit, wire.deadline_unix_ms).map_err(serde::de::Error::custom)
        }
    }

    #[derive(Default)]
    struct ExactTagsWire(Vec<(char, Vec<String>)>);

    impl<'de> serde::Deserialize<'de> for ExactTagsWire {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct ExactTagsVisitor;

            impl<'de> serde::de::Visitor<'de> for ExactTagsVisitor {
                type Value = ExactTagsWire;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a map of unique exact single-letter tag filters")
                }

                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::MapAccess<'de>,
                {
                    let mut entries = Vec::<(char, Vec<String>)>::new();
                    while let Some((key, values)) = map.next_entry::<char, Vec<String>>()? {
                        if entries.iter().any(|(candidate, _)| *candidate == key) {
                            return Err(serde::de::Error::custom(
                                "transport fetch selector contains a duplicate tag key",
                            ));
                        }
                        entries.push((key, values));
                    }
                    Ok(ExactTagsWire(entries))
                }
            }

            deserializer.deserialize_map(ExactTagsVisitor)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FetchRequestWire {
        request_id: String,
        target_set: TargetSet,
        bounds: FetchBounds,
        cursor: Option<FetchCursor>,
        #[serde(default)]
        selector: FetchSelector,
    }

    impl<'de> serde::Deserialize<'de> for FetchRequest {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = FetchRequestWire::deserialize(deserializer)?;
            Self::new(wire.request_id, wire.target_set, wire.bounds)
                .map(|request| request.with_selector(wire.selector))
                .map(|request| match wire.cursor {
                    Some(cursor) => request.with_cursor(cursor),
                    None => request,
                })
                .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SubscriptionRequestWire {
        request_id: String,
        target_set: TargetSet,
        bounds: SubscriptionBounds,
        #[serde(default)]
        selector: FetchSelector,
        #[serde(default)]
        #[serde(deserialize_with = "deserialize_subscription_checkpoints")]
        checkpoints: Vec<SubscriptionCheckpoint>,
    }

    impl<'de> serde::Deserialize<'de> for SubscriptionRequest {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = SubscriptionRequestWire::deserialize(deserializer)?;
            Self::new(wire.request_id.as_str(), wire.target_set, wire.bounds)
                .map(|request| request.with_selector(wire.selector))
                .and_then(|request| request.with_checkpoints(wire.checkpoints))
                .map_err(serde::de::Error::custom)
        }
    }

    impl<'de> serde::Deserialize<'de> for FetchSelector {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                #[serde(default)]
                kinds: Vec<u32>,
                #[serde(default)]
                authors: Vec<PublicKey>,
                #[serde(default)]
                exact_tags: ExactTagsWire,
                since_unix_seconds: Option<u64>,
                until_unix_seconds: Option<u64>,
            }

            let wire = Wire::deserialize(deserializer)?;
            let selector = FetchSelector::all()
                .with_kinds(wire.kinds)
                .and_then(|selector| selector.with_authors(wire.authors))
                .and_then(|selector| {
                    wire.exact_tags
                        .0
                        .into_iter()
                        .try_fold(selector, |selector, (key, values)| {
                            values.into_iter().try_fold(selector, |selector, value| {
                                selector.with_exact_tag_value(key, value)
                            })
                        })
                })
                .and_then(|selector| match wire.since_unix_seconds {
                    Some(since) => selector.with_since_unix_seconds(since),
                    None => Ok(selector),
                })
                .and_then(|selector| match wire.until_unix_seconds {
                    Some(until) => selector.with_until_unix_seconds(until),
                    None => Ok(selector),
                });
            selector.map_err(serde::de::Error::custom)
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
        #[serde(default)]
        selector: FetchSelector,
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
                selector: wire.selector,
                events: wire.events,
                target_outcomes: wire.target_outcomes,
                next_page: wire.next_page,
            };
            page.validate().map_err(serde::de::Error::custom)?;
            Ok(page)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SubscriptionEventWire {
        request: SubscriptionRequest,
        observed: ObservedEvent,
        checkpoint: SubscriptionCheckpoint,
    }

    impl<'de> serde::Deserialize<'de> for SubscriptionEvent {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = SubscriptionEventWire::deserialize(deserializer)?;
            Self::for_request(&wire.request, wire.observed, wire.checkpoint)
                .map_err(serde::de::Error::custom)
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SubscriptionEndWire {
        request: SubscriptionRequest,
        event_count: u16,
        #[serde(deserialize_with = "deserialize_subscription_checkpoints")]
        checkpoints: Vec<SubscriptionCheckpoint>,
        reason: SubscriptionEndReason,
    }

    impl<'de> serde::Deserialize<'de> for SubscriptionEnd {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = SubscriptionEndWire::deserialize(deserializer)?;
            Self::for_request(
                &wire.request,
                wire.event_count,
                wire.checkpoints,
                wire.reason,
            )
            .map_err(serde::de::Error::custom)
        }
    }

    fn deserialize_subscription_checkpoints<'de, D>(
        deserializer: D,
    ) -> Result<Vec<SubscriptionCheckpoint>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CheckpointVisitor;

        impl<'de> serde::de::Visitor<'de> for CheckpointVisitor {
            type Value = Vec<SubscriptionCheckpoint>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded transport subscription checkpoint sequence")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let capacity = sequence.size_hint().unwrap_or(0).min(TARGET_SET_MAX_ITEMS);
                let mut checkpoints = Vec::with_capacity(capacity);
                while let Some(checkpoint) = sequence.next_element()? {
                    if checkpoints.len() == TARGET_SET_MAX_ITEMS {
                        return Err(serde::de::Error::custom(
                            Error::SubscriptionCheckpointSetTooLarge,
                        ));
                    }
                    checkpoints.push(checkpoint);
                }
                Ok(checkpoints)
            }
        }

        deserializer.deserialize_seq(CheckpointVisitor)
    }
}
