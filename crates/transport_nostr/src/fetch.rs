#![forbid(unsafe_code)]

use crate::error::ensure_nonnegative_timestamp;
use crate::{RadrootsRelayOutcome, RadrootsRelayTargetSet, RadrootsRelayTransportError};
use core::time::Duration;
use futures::{StreamExt, future::BoxFuture};
use nostr::{JsonUtil, filter::MatchEventOptions};
use radroots_event::{ids::RadrootsEventId, wire::v1::DEFAULT_RAW_JSON_MAX_BYTES};
use radroots_event_store::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventPersistence,
    RadrootsEventStore, RadrootsEventVisibility, RadrootsTransportObservation,
    RadrootsTransportObservationType,
};
use radroots_nostr::prelude::{RadrootsNostrClient, RadrootsNostrEvent, RadrootsNostrFilter};
use radroots_transport::{
    RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES,
    RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES,
    RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT, RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES,
    RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS, RadrootsTransportKind, RadrootsTransportTarget,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

const DEFAULT_RELAY_FETCH_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_RELAY_FETCH_RAW_SCAN_MULTIPLIER: usize = 64;
pub const RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX: usize =
    RADROOTS_TRANSPORT_FETCH_ADMITTED_EVENT_MAX_COUNT;
pub const RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX: usize =
    RADROOTS_TRANSPORT_FETCH_RAW_ITEM_MAX_COUNT;
pub const RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX: usize =
    RADROOTS_TRANSPORT_FETCH_RAW_JSON_MAX_BYTES;
pub const RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX: usize = RADROOTS_TRANSPORT_FETCH_FILTER_MAX_COUNT;
pub const RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX: usize =
    RADROOTS_TRANSPORT_FETCH_FILTER_MAX_BYTES;
pub const RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX: usize =
    RADROOTS_TRANSPORT_FETCH_FILTERS_MAX_BYTES;
pub const RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX: u64 = RADROOTS_TRANSPORT_TOTAL_DEADLINE_MAX_MS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayFetchMode {
    Fetch,
    Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayFetchFilters {
    filters: Vec<RadrootsNostrFilter>,
}

impl RadrootsRelayFetchFilters {
    pub fn new<I>(filters: I) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        let mut bounded_filters = Vec::new();
        let mut aggregate_json_bytes = 0usize;
        for filter in filters {
            if bounded_filters.len() == RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX {
                return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                    field: "filter_count",
                    max: RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX,
                    actual: RADROOTS_RELAY_FETCH_FILTER_LIMIT_MAX + 1,
                });
            }
            let filter_json_bytes = filter.as_json().len();
            if filter_json_bytes > RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX {
                return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                    field: "filter_json_bytes",
                    max: RADROOTS_RELAY_FETCH_FILTER_JSON_BYTE_LIMIT_MAX,
                    actual: filter_json_bytes,
                });
            }
            aggregate_json_bytes = aggregate_json_bytes.checked_add(filter_json_bytes).ok_or(
                RadrootsRelayTransportError::FetchLimitTooLarge {
                    field: "filter_set_json_bytes",
                    max: RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX,
                    actual: usize::MAX,
                },
            )?;
            if aggregate_json_bytes > RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX {
                return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                    field: "filter_set_json_bytes",
                    max: RADROOTS_RELAY_FETCH_FILTER_SET_JSON_BYTE_LIMIT_MAX,
                    actual: aggregate_json_bytes,
                });
            }
            bounded_filters.push(filter);
        }
        let filters = bounded_filters;
        if filters.is_empty() {
            return Err(RadrootsRelayTransportError::EmptyFetchFilters);
        }
        Ok(Self { filters })
    }

    pub fn as_slice(&self) -> &[RadrootsNostrFilter] {
        &self.filters
    }
}

impl AsRef<[RadrootsNostrFilter]> for RadrootsRelayFetchFilters {
    fn as_ref(&self) -> &[RadrootsNostrFilter] {
        self.as_slice()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayFetchRequest {
    mode: RadrootsRelayFetchMode,
    observed_at_ms: i64,
    max_events: usize,
    max_raw_events: usize,
    max_raw_json_bytes: usize,
    relay_targets: RadrootsRelayTargetSet,
    filters: RadrootsRelayFetchFilters,
    timeout_ms: u64,
}

impl RadrootsRelayFetchRequest {
    pub fn fetch<I>(
        observed_at_ms: i64,
        max_events: usize,
        relay_targets: RadrootsRelayTargetSet,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        Self::new(
            RadrootsRelayFetchMode::Fetch,
            observed_at_ms,
            max_events,
            relay_targets,
            filters,
        )
    }

    pub fn subscription<I>(
        observed_at_ms: i64,
        max_events: usize,
        relay_targets: RadrootsRelayTargetSet,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        Self::new(
            RadrootsRelayFetchMode::Subscription,
            observed_at_ms,
            max_events,
            relay_targets,
            filters,
        )
    }

    fn new<I>(
        mode: RadrootsRelayFetchMode,
        observed_at_ms: i64,
        max_events: usize,
        relay_targets: RadrootsRelayTargetSet,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        ensure_nonnegative_timestamp("observed_at_ms", observed_at_ms)?;
        ensure_event_limit("max_events", max_events)?;
        Ok(Self {
            mode,
            observed_at_ms,
            max_events,
            max_raw_events: default_raw_event_scan_limit(max_events),
            max_raw_json_bytes: RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX,
            relay_targets,
            filters: RadrootsRelayFetchFilters::new(filters)?,
            timeout_ms: DEFAULT_RELAY_FETCH_TIMEOUT_MS,
        })
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Result<Self, RadrootsRelayTransportError> {
        ensure_positive_timeout("timeout_ms", timeout_ms)?;
        self.timeout_ms = timeout_ms;
        Ok(self)
    }

    pub fn with_raw_event_scan_limit(
        mut self,
        max_raw_events: usize,
    ) -> Result<Self, RadrootsRelayTransportError> {
        ensure_bounded_fetch_limit(
            "max_raw_events",
            max_raw_events,
            RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX,
        )?;
        self.max_raw_events = max_raw_events;
        Ok(self)
    }

    pub fn with_raw_json_byte_limit(
        mut self,
        max_raw_json_bytes: usize,
    ) -> Result<Self, RadrootsRelayTransportError> {
        ensure_bounded_fetch_limit(
            "max_raw_json_bytes",
            max_raw_json_bytes,
            RADROOTS_RELAY_FETCH_RAW_JSON_BYTE_LIMIT_MAX,
        )?;
        self.max_raw_json_bytes = max_raw_json_bytes;
        Ok(self)
    }

    pub fn mode(&self) -> RadrootsRelayFetchMode {
        self.mode
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub fn max_events(&self) -> usize {
        self.max_events
    }

    pub fn max_raw_events(&self) -> usize {
        self.max_raw_events
    }

    pub fn max_raw_json_bytes(&self) -> usize {
        self.max_raw_json_bytes
    }

    pub fn relay_targets(&self) -> &RadrootsRelayTargetSet {
        &self.relay_targets
    }

    pub fn filters(&self) -> &[RadrootsNostrFilter] {
        self.filters.as_slice()
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

fn default_raw_event_scan_limit(max_events: usize) -> usize {
    max_events
        .saturating_mul(DEFAULT_RELAY_FETCH_RAW_SCAN_MULTIPLIER)
        .max(max_events)
        .min(RADROOTS_RELAY_FETCH_RAW_EVENT_LIMIT_MAX)
}

fn ensure_positive_limit(
    field: &'static str,
    value: usize,
) -> Result<(), RadrootsRelayTransportError> {
    if value == 0 {
        return Err(RadrootsRelayTransportError::InvalidFetchLimit { field });
    }
    Ok(())
}

fn ensure_event_limit(
    field: &'static str,
    value: usize,
) -> Result<(), RadrootsRelayTransportError> {
    ensure_bounded_fetch_limit(field, value, RADROOTS_RELAY_FETCH_EVENT_LIMIT_MAX)
}

fn ensure_bounded_fetch_limit(
    field: &'static str,
    value: usize,
    max: usize,
) -> Result<(), RadrootsRelayTransportError> {
    ensure_positive_limit(field, value)?;
    if value > max {
        return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field,
            max,
            actual: value,
        });
    }
    Ok(())
}

fn ensure_positive_timeout(
    field: &'static str,
    value: u64,
) -> Result<(), RadrootsRelayTransportError> {
    if value == 0 {
        return Err(RadrootsRelayTransportError::InvalidFetchLimit { field });
    }
    if value > RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX {
        return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
            field,
            max: RADROOTS_RELAY_FETCH_TIMEOUT_MS_MAX as usize,
            actual: usize::try_from(value).unwrap_or(usize::MAX),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayFetchItem {
    body: RadrootsRelayFetchItemBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RadrootsRelayFetchItemBody {
    Event { relay_url: String, raw_json: String },
    Eose { relay_url: String },
    Truncated { relay_url: String, message: String },
    Closed { relay_url: String, message: String },
    Notice { relay_url: String, message: String },
}

impl RadrootsRelayFetchItem {
    pub fn event(
        relay_url: impl Into<String>,
        raw_json: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = validate_fetch_item_relay_url(relay_url.into())?;
        let raw_json = raw_json.into();
        if raw_json.len() > DEFAULT_RAW_JSON_MAX_BYTES {
            return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                field: "event_raw_json_bytes",
                max: DEFAULT_RAW_JSON_MAX_BYTES,
                actual: raw_json.len(),
            });
        }
        Ok(Self {
            body: RadrootsRelayFetchItemBody::Event {
                relay_url,
                raw_json,
            },
        })
    }

    pub fn eose(relay_url: impl Into<String>) -> Result<Self, RadrootsRelayTransportError> {
        Ok(Self {
            body: RadrootsRelayFetchItemBody::Eose {
                relay_url: validate_fetch_item_relay_url(relay_url.into())?,
            },
        })
    }

    pub fn truncated(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::diagnostic(relay_url.into(), message.into(), |relay_url, message| {
            RadrootsRelayFetchItemBody::Truncated { relay_url, message }
        })
    }

    pub fn closed(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::diagnostic(relay_url.into(), message.into(), |relay_url, message| {
            RadrootsRelayFetchItemBody::Closed { relay_url, message }
        })
    }

    pub fn notice(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::diagnostic(relay_url.into(), message.into(), |relay_url, message| {
            RadrootsRelayFetchItemBody::Notice { relay_url, message }
        })
    }

    fn diagnostic(
        relay_url: String,
        message: String,
        body: fn(String, String) -> RadrootsRelayFetchItemBody,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = validate_fetch_item_relay_url(relay_url)?;
        if message.len() > radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES {
            return Err(RadrootsRelayTransportError::DiagnosticLimitExceeded {
                field: "relay_fetch_item_message",
                max: radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
                actual: message.len(),
            });
        }
        Ok(Self {
            body: body(relay_url, message),
        })
    }

    pub fn relay_url(&self) -> &str {
        match &self.body {
            RadrootsRelayFetchItemBody::Event { relay_url, .. }
            | RadrootsRelayFetchItemBody::Eose { relay_url }
            | RadrootsRelayFetchItemBody::Truncated { relay_url, .. }
            | RadrootsRelayFetchItemBody::Closed { relay_url, .. }
            | RadrootsRelayFetchItemBody::Notice { relay_url, .. } => relay_url,
        }
    }

    fn terminal_outcome_label(&self) -> Option<&'static str> {
        match &self.body {
            RadrootsRelayFetchItemBody::Eose { .. } => Some("eose"),
            RadrootsRelayFetchItemBody::Truncated { .. } => Some("truncated"),
            RadrootsRelayFetchItemBody::Closed { .. } => Some("closed"),
            RadrootsRelayFetchItemBody::Event { .. }
            | RadrootsRelayFetchItemBody::Notice { .. } => None,
        }
    }
}

fn validate_fetch_item_relay_url(relay_url: String) -> Result<String, RadrootsRelayTransportError> {
    RadrootsTransportTarget::nostr_relay(relay_url.as_str()).map_err(|error| {
        RadrootsRelayTransportError::InvalidFetchItemRelayUrl {
            url: if relay_url.len() <= radroots_transport::RADROOTS_TRANSPORT_ENDPOINT_URI_MAX_BYTES
            {
                relay_url.clone()
            } else {
                "<oversized>".to_owned()
            },
            reason: error.to_string(),
        }
    })?;
    Ok(relay_url)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayFetchOutcomeKind {
    Eose,
    Truncated,
    Closed,
    Notice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayFetchRelayOutcome {
    relay_url: String,
    kind: RadrootsRelayFetchOutcomeKind,
    relay_outcome: Option<RadrootsRelayOutcome>,
    message: Option<String>,
}

impl RadrootsRelayFetchRelayOutcome {
    pub fn eose(relay_url: impl Into<String>) -> Result<Self, RadrootsRelayTransportError> {
        Self::try_new(
            relay_url.into(),
            RadrootsRelayFetchOutcomeKind::Eose,
            None,
            None,
        )
    }

    pub fn truncated(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::try_new(
            relay_url.into(),
            RadrootsRelayFetchOutcomeKind::Truncated,
            None,
            Some(message.into()),
        )
    }

    pub fn closed(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let message = message.into();
        let classified = RadrootsRelayOutcome::classify(message.as_str())?;
        let relay_outcome = RadrootsRelayOutcome::try_new(classified.kind(), None)?;
        Self::try_new(
            relay_url.into(),
            RadrootsRelayFetchOutcomeKind::Closed,
            Some(relay_outcome),
            Some(message),
        )
    }

    pub fn notice(
        relay_url: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        Self::try_new(
            relay_url.into(),
            RadrootsRelayFetchOutcomeKind::Notice,
            None,
            Some(message.into()),
        )
    }

    fn try_new(
        relay_url: String,
        kind: RadrootsRelayFetchOutcomeKind,
        relay_outcome: Option<RadrootsRelayOutcome>,
        message: Option<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = canonical_fetch_receipt_relay_url(relay_url.as_str())?;
        if let Some(message) = message.as_deref() {
            validate_fetch_receipt_diagnostic("relay_outcome_message", message)?;
        }
        match kind {
            RadrootsRelayFetchOutcomeKind::Eose => {
                if relay_outcome.is_some() || message.is_some() {
                    return Err(invalid_fetch_receipt(
                        "relay_outcome",
                        "EOSE cannot carry an outcome or message",
                    ));
                }
            }
            RadrootsRelayFetchOutcomeKind::Truncated | RadrootsRelayFetchOutcomeKind::Notice => {
                if relay_outcome.is_some() || message.is_none() {
                    return Err(invalid_fetch_receipt(
                        "relay_outcome",
                        "truncated and notice outcomes require a message and no relay outcome",
                    ));
                }
            }
            RadrootsRelayFetchOutcomeKind::Closed => {
                let Some(message) = message.as_deref() else {
                    return Err(invalid_fetch_receipt(
                        "relay_outcome",
                        "closed outcomes require a message and classified relay outcome",
                    ));
                };
                let expected = RadrootsRelayOutcome::classify(message)?;
                if relay_outcome.as_ref().map(RadrootsRelayOutcome::kind) != Some(expected.kind())
                    || relay_outcome
                        .as_ref()
                        .and_then(RadrootsRelayOutcome::message)
                        .is_some()
                {
                    return Err(invalid_fetch_receipt(
                        "relay_outcome",
                        "closed outcome classification must match its message without duplicating it",
                    ));
                }
            }
        }
        Ok(Self {
            relay_url,
            kind,
            relay_outcome,
            message,
        })
    }

    pub fn relay_url(&self) -> &str {
        self.relay_url.as_str()
    }

    pub fn kind(&self) -> RadrootsRelayFetchOutcomeKind {
        self.kind
    }

    pub fn relay_outcome(&self) -> Option<&RadrootsRelayOutcome> {
        self.relay_outcome.as_ref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayFetchRelayOutcomeWire {
    relay_url: String,
    kind: RadrootsRelayFetchOutcomeKind,
    relay_outcome: Option<RadrootsRelayOutcome>,
    message: Option<String>,
}

impl<'de> Deserialize<'de> for RadrootsRelayFetchRelayOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayFetchRelayOutcomeWire::deserialize(deserializer)?;
        Self::try_new(wire.relay_url, wire.kind, wire.relay_outcome, wire.message)
            .map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsRelayFetchEventVerification {
    NotEvaluated,
    Verified,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsRelayFetchEventAdmission {
    NotEvaluated,
    Admitted,
    Unsupported,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsRelayFetchEventValidStream {
    NotEvaluated,
    Eligible,
    Ineligible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadrootsRelayFetchEventVisibility {
    NotEvaluated,
    NotPersisted,
    Visible,
    NotAdmitted,
    NotCurrent,
    Suppressed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayFetchEventReceipt {
    relay_url: String,
    event_id: Option<String>,
    inserted: bool,
    duplicate: bool,
    not_persisted: bool,
    malformed: bool,
    out_of_filter: bool,
    skipped_over_limit: bool,
    verification: RadrootsRelayFetchEventVerification,
    admission: RadrootsRelayFetchEventAdmission,
    admission_code: Option<String>,
    valid_stream: RadrootsRelayFetchEventValidStream,
    visibility: RadrootsRelayFetchEventVisibility,
    message: Option<String>,
}

impl RadrootsRelayFetchEventReceipt {
    fn checked(mut self) -> Result<Self, RadrootsRelayTransportError> {
        self.relay_url = canonical_fetch_receipt_relay_url(self.relay_url.as_str())?;
        if let Some(event_id) = self.event_id.as_deref() {
            RadrootsEventId::parse(event_id)
                .map_err(|error| invalid_fetch_receipt("event_id", error.to_string()))?;
        }
        if let Some(admission_code) = self.admission_code.as_deref()
            && admission_code.len() > radroots_transport::RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES
        {
            return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                field: "admission_code_bytes",
                max: radroots_transport::RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
                actual: admission_code.len(),
            });
        }
        if let Some(message) = self.message.as_deref() {
            validate_fetch_receipt_diagnostic("event_receipt_message", message)?;
        }

        let disposition_count = usize::from(self.inserted)
            + usize::from(self.duplicate)
            + usize::from(self.not_persisted)
            + usize::from(self.malformed)
            + usize::from(self.out_of_filter)
            + usize::from(self.skipped_over_limit);
        if disposition_count > 1 {
            return Err(invalid_fetch_receipt(
                "event_disposition",
                "event receipt dispositions are mutually exclusive",
            ));
        }
        if (self.inserted
            || self.duplicate
            || self.not_persisted
            || self.out_of_filter
            || self.skipped_over_limit)
            && self.event_id.is_none()
        {
            return Err(invalid_fetch_receipt(
                "event_id",
                "this event receipt disposition requires an event id",
            ));
        }
        if self.malformed
            && (self.event_id.is_some()
                || self.verification != RadrootsRelayFetchEventVerification::NotEvaluated)
        {
            return Err(invalid_fetch_receipt(
                "malformed",
                "malformed receipts cannot identify or verify an event",
            ));
        }
        if (self.inserted
            || self.duplicate
            || self.not_persisted
            || self.out_of_filter
            || self.skipped_over_limit)
            && self.verification != RadrootsRelayFetchEventVerification::Verified
        {
            return Err(invalid_fetch_receipt(
                "verification",
                "this event receipt disposition requires verified event bytes",
            ));
        }
        if self.verification != RadrootsRelayFetchEventVerification::Verified
            && (self.admission != RadrootsRelayFetchEventAdmission::NotEvaluated
                || self.admission_code.is_some()
                || self.valid_stream != RadrootsRelayFetchEventValidStream::NotEvaluated
                || self.visibility != RadrootsRelayFetchEventVisibility::NotEvaluated)
        {
            return Err(invalid_fetch_receipt(
                "verification",
                "unverified event receipts cannot carry semantic outcomes",
            ));
        }
        if self.admission == RadrootsRelayFetchEventAdmission::NotEvaluated
            && (self.admission_code.is_some()
                || self.valid_stream != RadrootsRelayFetchEventValidStream::NotEvaluated)
        {
            return Err(invalid_fetch_receipt(
                "admission",
                "unevaluated admission cannot carry a code or valid-stream result",
            ));
        }
        if self.admission != RadrootsRelayFetchEventAdmission::NotEvaluated
            && self.valid_stream == RadrootsRelayFetchEventValidStream::NotEvaluated
        {
            return Err(invalid_fetch_receipt(
                "valid_stream",
                "evaluated admission requires a valid-stream result",
            ));
        }
        if self.not_persisted
            != (self.visibility == RadrootsRelayFetchEventVisibility::NotPersisted)
        {
            return Err(invalid_fetch_receipt(
                "visibility",
                "not-persisted disposition and visibility must agree",
            ));
        }
        if matches!(
            self.visibility,
            RadrootsRelayFetchEventVisibility::Visible
                | RadrootsRelayFetchEventVisibility::NotAdmitted
                | RadrootsRelayFetchEventVisibility::NotCurrent
                | RadrootsRelayFetchEventVisibility::Suppressed
        ) && !(self.inserted || self.duplicate)
        {
            return Err(invalid_fetch_receipt(
                "visibility",
                "stored visibility requires an inserted or duplicate receipt",
            ));
        }
        Ok(self)
    }

    pub fn relay_url(&self) -> &str {
        self.relay_url.as_str()
    }

    pub fn event_id(&self) -> Option<&str> {
        self.event_id.as_deref()
    }

    pub fn was_inserted(&self) -> bool {
        self.inserted
    }

    pub fn was_duplicate(&self) -> bool {
        self.duplicate
    }

    pub fn was_not_persisted(&self) -> bool {
        self.not_persisted
    }

    pub fn is_malformed(&self) -> bool {
        self.malformed
    }

    pub fn is_out_of_filter(&self) -> bool {
        self.out_of_filter
    }

    pub fn was_skipped_over_limit(&self) -> bool {
        self.skipped_over_limit
    }

    pub fn verification(&self) -> RadrootsRelayFetchEventVerification {
        self.verification
    }

    pub fn admission(&self) -> RadrootsRelayFetchEventAdmission {
        self.admission
    }

    pub fn admission_code(&self) -> Option<&str> {
        self.admission_code.as_deref()
    }

    pub fn valid_stream(&self) -> RadrootsRelayFetchEventValidStream {
        self.valid_stream
    }

    pub fn visibility(&self) -> RadrootsRelayFetchEventVisibility {
        self.visibility
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayFetchEventReceiptWire {
    relay_url: String,
    event_id: Option<String>,
    inserted: bool,
    duplicate: bool,
    not_persisted: bool,
    malformed: bool,
    out_of_filter: bool,
    skipped_over_limit: bool,
    verification: RadrootsRelayFetchEventVerification,
    admission: RadrootsRelayFetchEventAdmission,
    admission_code: Option<String>,
    valid_stream: RadrootsRelayFetchEventValidStream,
    visibility: RadrootsRelayFetchEventVisibility,
    message: Option<String>,
}

impl<'de> Deserialize<'de> for RadrootsRelayFetchEventReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayFetchEventReceiptWire::deserialize(deserializer)?;
        Self {
            relay_url: wire.relay_url,
            event_id: wire.event_id,
            inserted: wire.inserted,
            duplicate: wire.duplicate,
            not_persisted: wire.not_persisted,
            malformed: wire.malformed,
            out_of_filter: wire.out_of_filter,
            skipped_over_limit: wire.skipped_over_limit,
            verification: wire.verification,
            admission: wire.admission,
            admission_code: wire.admission_code,
            valid_stream: wire.valid_stream,
            visibility: wire.visibility,
            message: wire.message,
        }
        .checked()
        .map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsRelayFetchedEvent {
    relay_url: String,
    event: RadrootsNostrEvent,
    raw_json: String,
    observed_at_ms: i64,
}

impl RadrootsRelayFetchedEvent {
    pub fn new(
        relay_url: impl Into<String>,
        event: RadrootsNostrEvent,
        raw_json: impl Into<String>,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = relay_url.into();
        let raw_json = raw_json.into();
        let fetched = Self::from_verified(relay_url, event, raw_json, observed_at_ms)?;
        RadrootsEventIngest::from_raw_json(fetched.raw_json.clone(), observed_at_ms)?;
        Ok(fetched)
    }

    fn from_verified(
        relay_url: String,
        event: RadrootsNostrEvent,
        raw_json: String,
        observed_at_ms: i64,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = canonical_fetch_receipt_relay_url(relay_url.as_str())?;
        ensure_nonnegative_timestamp("observed_at_ms", observed_at_ms)?;
        if raw_json.len() > DEFAULT_RAW_JSON_MAX_BYTES {
            return Err(RadrootsRelayTransportError::FetchLimitTooLarge {
                field: "fetched_event_raw_json_bytes",
                max: DEFAULT_RAW_JSON_MAX_BYTES,
                actual: raw_json.len(),
            });
        }
        let decoded = RadrootsNostrEvent::from_json(raw_json.as_str())
            .map_err(|error| RadrootsRelayTransportError::NostrEventJson(error.to_string()))?;
        if decoded != event {
            return Err(invalid_fetch_receipt(
                "fetched_event",
                "event object does not match its raw JSON bytes",
            ));
        }
        Ok(Self {
            relay_url,
            event,
            raw_json,
            observed_at_ms,
        })
    }

    pub fn relay_url(&self) -> &str {
        self.relay_url.as_str()
    }

    pub fn event(&self) -> &RadrootsNostrEvent {
        &self.event
    }

    pub fn raw_json(&self) -> &str {
        self.raw_json.as_str()
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    fn into_parts(self) -> (String, RadrootsNostrEvent, String, i64) {
        (
            self.relay_url,
            self.event,
            self.raw_json,
            self.observed_at_ms,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadrootsRelayFetchFailure {
    relay_url: String,
    reason: String,
}

impl RadrootsRelayFetchFailure {
    pub fn new(
        relay_url: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, RadrootsRelayTransportError> {
        let relay_url = relay_url.into();
        let reason = reason.into();
        let relay_url = canonical_fetch_receipt_relay_url(relay_url.as_str())?;
        validate_fetch_receipt_diagnostic("relay_failure_reason", reason.as_str())?;
        Ok(Self { relay_url, reason })
    }

    pub fn relay_url(&self) -> &str {
        self.relay_url.as_str()
    }

    pub fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RadrootsRelayFetchFailureWire {
    relay_url: String,
    reason: String,
}

impl<'de> Deserialize<'de> for RadrootsRelayFetchFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RadrootsRelayFetchFailureWire::deserialize(deserializer)?;
        Self::new(wire.relay_url, wire.reason).map_err(de::Error::custom)
    }
}

fn canonical_fetch_receipt_relay_url(
    relay_url: &str,
) -> Result<String, RadrootsRelayTransportError> {
    RadrootsTransportTarget::nostr_relay(relay_url)
        .map(|target| target.uri().as_str().to_owned())
        .map_err(|error| invalid_fetch_receipt("relay_url", error.to_string()))
}

fn validate_fetch_receipt_diagnostic(
    field: &'static str,
    value: &str,
) -> Result<(), RadrootsRelayTransportError> {
    if value.len() > radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES {
        return Err(RadrootsRelayTransportError::DiagnosticLimitExceeded {
            field,
            max: radroots_transport::RADROOTS_TRANSPORT_DIAGNOSTIC_MAX_BYTES,
            actual: value.len(),
        });
    }
    Ok(())
}

fn invalid_fetch_receipt(
    field: &'static str,
    reason: impl Into<String>,
) -> RadrootsRelayTransportError {
    RadrootsRelayTransportError::InvalidFetchReceipt {
        field,
        reason: reason.into(),
    }
}

#[derive(Clone, Debug)]
pub struct RadrootsRelayFetchedEventsReceipt {
    pub target_relays: Vec<String>,
    pub connected_relays: Vec<String>,
    pub failed_relays: Vec<RadrootsRelayFetchFailure>,
    pub events: Vec<RadrootsRelayFetchedEvent>,
    pub event_receipts: Vec<RadrootsRelayFetchEventReceipt>,
    pub duplicate_count: usize,
    pub verification_failed_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub eose_count: usize,
    pub truncated_count: usize,
    pub closed_count: usize,
    pub notice_count: usize,
    pub relay_outcomes: Vec<RadrootsRelayFetchRelayOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchReceipt {
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub not_persisted_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub verification_failed_count: usize,
    pub admission_unsupported_count: usize,
    pub admission_invalid_count: usize,
    pub valid_stream_eligible_count: usize,
    pub visible_count: usize,
    pub not_admitted_count: usize,
    pub not_current_count: usize,
    pub suppressed_count: usize,
    pub eose_count: usize,
    pub truncated_count: usize,
    pub closed_count: usize,
    pub notice_count: usize,
    pub events: Vec<RadrootsRelayFetchEventReceipt>,
    pub relay_outcomes: Vec<RadrootsRelayFetchRelayOutcome>,
}

pub trait RadrootsRelayFetchAdapter: Send + Sync {
    fn fetch<'a>(
        &'a self,
        request: RadrootsRelayFetchRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError>>;
}

pub async fn fetch_relay_events<A>(
    adapter: &A,
    request: RadrootsRelayFetchRequest,
) -> Result<RadrootsRelayFetchedEventsReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayFetchAdapter,
{
    let target_relays = request.relay_targets.relay_strings();
    let observed_at_ms = request.observed_at_ms;
    let max_events = request.max_events;
    let max_raw_events = request.max_raw_events;
    let max_raw_json_bytes = request.max_raw_json_bytes;
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    process_relay_fetch_items(
        target_relays,
        filters,
        observed_at_ms,
        max_events,
        max_raw_events,
        max_raw_json_bytes,
        items,
    )?
    .into_fetched_events_receipt()
}

#[cfg(feature = "runtime-tokio")]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn fetch_relay_events_blocking<A>(
    adapter: &A,
    request: RadrootsRelayFetchRequest,
) -> Result<RadrootsRelayFetchedEventsReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayFetchAdapter,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| RadrootsRelayTransportError::Transport(error.to_string()))?;
    runtime.block_on(fetch_relay_events(adapter, request))
}

pub async fn fetch_and_ingest_relay_events<A>(
    adapter: &A,
    event_store: &RadrootsEventStore,
    request: RadrootsRelayFetchRequest,
) -> Result<RadrootsRelayFetchReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayFetchAdapter,
{
    let mode = request.mode;
    let target_relays = request.relay_targets.relay_strings();
    let observed_at_ms = request.observed_at_ms;
    let max_events = request.max_events;
    let max_raw_events = request.max_raw_events;
    let max_raw_json_bytes = request.max_raw_json_bytes;
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    let processed = process_relay_fetch_items(
        target_relays,
        filters,
        observed_at_ms,
        max_events,
        max_raw_events,
        max_raw_json_bytes,
        items,
    )?;
    let mut receipt = RadrootsRelayFetchReceipt::from_processed_counts(&processed);
    for item in processed.items {
        match item {
            RadrootsRelayProcessedFetchItem::Receipt(event_receipt) => {
                receipt.events.push(event_receipt);
            }
            RadrootsRelayProcessedFetchItem::Accepted(event)
            | RadrootsRelayProcessedFetchItem::Duplicate(event) => {
                let (relay_url, raw_event, raw_json, observed_at_ms) = event.into_parts();
                let observation_type = match mode {
                    RadrootsRelayFetchMode::Fetch => RadrootsTransportObservationType::Fetch,
                    RadrootsRelayFetchMode::Subscription => {
                        RadrootsTransportObservationType::Subscription
                    }
                };
                let observation = RadrootsTransportObservation::new(
                    RadrootsTransportKind::Nostr,
                    relay_url.clone(),
                    observation_type,
                    observed_at_ms,
                )?;
                let ingest = match RadrootsEventIngest::from_raw_json(raw_json, observed_at_ms) {
                    Ok(ingest) => ingest.with_observation(observation),
                    Err(error) => {
                        receipt.verification_failed_count += 1;
                        receipt.events.push(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: Some(raw_event.id.to_hex()),
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                verification: RadrootsRelayFetchEventVerification::Failed,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some(error.to_string()),
                            }
                            .checked()?,
                        );
                        continue;
                    }
                };
                let store_receipt = event_store.ingest_event(ingest).await?;
                let admission = relay_fetch_admission(store_receipt.admission_status);
                let valid_stream = if store_receipt.valid_stream_eligible {
                    RadrootsRelayFetchEventValidStream::Eligible
                } else {
                    RadrootsRelayFetchEventValidStream::Ineligible
                };
                let (inserted, duplicate, not_persisted) = match store_receipt.persistence {
                    RadrootsEventPersistence::Inserted { .. } => {
                        receipt.inserted_count += 1;
                        (true, false, false)
                    }
                    RadrootsEventPersistence::Duplicate { .. } => {
                        receipt.duplicate_count += 1;
                        (false, true, false)
                    }
                    RadrootsEventPersistence::NotPersisted => {
                        receipt.not_persisted_count += 1;
                        (false, false, true)
                    }
                };
                let visibility = if not_persisted {
                    RadrootsRelayFetchEventVisibility::NotPersisted
                } else {
                    RadrootsRelayFetchEventVisibility::NotEvaluated
                };
                let event_receipt = RadrootsRelayFetchEventReceipt {
                    relay_url,
                    event_id: Some(store_receipt.event_id),
                    inserted,
                    duplicate,
                    not_persisted,
                    malformed: false,
                    out_of_filter: false,
                    skipped_over_limit: false,
                    verification: RadrootsRelayFetchEventVerification::Verified,
                    admission,
                    admission_code: store_receipt.admission_code,
                    valid_stream,
                    visibility,
                    message: None,
                }
                .checked()?;
                receipt.events.push(event_receipt);
            }
        }
    }
    receipt.refresh_final_semantic_outcomes(event_store).await?;
    for event in &receipt.events {
        event.clone().checked()?;
    }
    Ok(receipt)
}

fn relay_fetch_admission(
    admission: RadrootsEventAdmissionStatus,
) -> RadrootsRelayFetchEventAdmission {
    match admission {
        RadrootsEventAdmissionStatus::Admitted => RadrootsRelayFetchEventAdmission::Admitted,
        RadrootsEventAdmissionStatus::Unsupported => RadrootsRelayFetchEventAdmission::Unsupported,
        RadrootsEventAdmissionStatus::Invalid => RadrootsRelayFetchEventAdmission::Invalid,
    }
}

fn relay_fetch_visibility(
    event_id: &str,
    visibility: RadrootsEventVisibility,
) -> Result<RadrootsRelayFetchEventVisibility, RadrootsRelayTransportError> {
    match visibility {
        RadrootsEventVisibility::Visible => Ok(RadrootsRelayFetchEventVisibility::Visible),
        RadrootsEventVisibility::NotAdmitted => Ok(RadrootsRelayFetchEventVisibility::NotAdmitted),
        RadrootsEventVisibility::NotCurrent { .. } => {
            Ok(RadrootsRelayFetchEventVisibility::NotCurrent)
        }
        RadrootsEventVisibility::Suppressed { .. } => {
            Ok(RadrootsRelayFetchEventVisibility::Suppressed)
        }
        _ => Err(
            RadrootsRelayTransportError::UnsupportedStoredEventVisibility {
                event_id: event_id.to_owned(),
            },
        ),
    }
}

fn required_persisted_fetch_receipt_event_id(
    event_id: Option<&str>,
) -> Result<&str, RadrootsRelayTransportError> {
    event_id.ok_or(RadrootsRelayTransportError::MissingPersistedFetchReceiptEventId)
}

#[derive(Clone, Debug)]
enum RadrootsRelayProcessedFetchItem {
    Accepted(RadrootsRelayFetchedEvent),
    Duplicate(RadrootsRelayFetchedEvent),
    Receipt(RadrootsRelayFetchEventReceipt),
}

#[derive(Clone, Debug)]
struct RadrootsRelayProcessedFetch {
    target_relays: Vec<String>,
    items: Vec<RadrootsRelayProcessedFetchItem>,
    duplicate_count: usize,
    verification_failed_count: usize,
    malformed_count: usize,
    out_of_filter_count: usize,
    skipped_over_limit_count: usize,
    eose_count: usize,
    truncated_count: usize,
    closed_count: usize,
    notice_count: usize,
    relay_outcomes: Vec<RadrootsRelayFetchRelayOutcome>,
}

impl RadrootsRelayProcessedFetch {
    fn into_fetched_events_receipt(
        self,
    ) -> Result<RadrootsRelayFetchedEventsReceipt, RadrootsRelayTransportError> {
        let mut events = Vec::new();
        let mut event_receipts = Vec::new();
        for item in self.items {
            match item {
                RadrootsRelayProcessedFetchItem::Accepted(event) => {
                    event_receipts.push(accepted_fetch_event_receipt(&event)?);
                    events.push(event);
                }
                RadrootsRelayProcessedFetchItem::Duplicate(event) => {
                    event_receipts.push(duplicate_fetch_event_receipt(&event)?);
                }
                RadrootsRelayProcessedFetchItem::Receipt(receipt) => event_receipts.push(receipt),
            }
        }
        let connected_relays = self
            .relay_outcomes
            .iter()
            .filter(|outcome| outcome.kind() == RadrootsRelayFetchOutcomeKind::Eose)
            .map(|outcome| outcome.relay_url().to_owned())
            .collect();
        let failed_relays = self
            .relay_outcomes
            .iter()
            .filter(|outcome| outcome.kind() == RadrootsRelayFetchOutcomeKind::Closed)
            .map(|outcome| RadrootsRelayFetchFailure {
                relay_url: outcome.relay_url.clone(),
                reason: outcome.message.clone().unwrap_or_default(),
            })
            .collect();
        Ok(RadrootsRelayFetchedEventsReceipt {
            target_relays: self.target_relays,
            connected_relays,
            failed_relays,
            events,
            event_receipts,
            duplicate_count: self.duplicate_count,
            verification_failed_count: self.verification_failed_count,
            malformed_count: self.malformed_count,
            out_of_filter_count: self.out_of_filter_count,
            skipped_over_limit_count: self.skipped_over_limit_count,
            eose_count: self.eose_count,
            truncated_count: self.truncated_count,
            closed_count: self.closed_count,
            notice_count: self.notice_count,
            relay_outcomes: self.relay_outcomes,
        })
    }
}

impl RadrootsRelayFetchReceipt {
    fn from_processed_counts(processed: &RadrootsRelayProcessedFetch) -> Self {
        Self {
            inserted_count: 0,
            duplicate_count: 0,
            not_persisted_count: 0,
            malformed_count: processed.malformed_count,
            out_of_filter_count: processed.out_of_filter_count,
            skipped_over_limit_count: processed.skipped_over_limit_count,
            verification_failed_count: processed.verification_failed_count,
            admission_unsupported_count: 0,
            admission_invalid_count: 0,
            valid_stream_eligible_count: 0,
            visible_count: 0,
            not_admitted_count: 0,
            not_current_count: 0,
            suppressed_count: 0,
            eose_count: processed.eose_count,
            truncated_count: processed.truncated_count,
            closed_count: processed.closed_count,
            notice_count: processed.notice_count,
            events: Vec::new(),
            relay_outcomes: processed.relay_outcomes.clone(),
        }
    }

    async fn refresh_final_semantic_outcomes(
        &mut self,
        event_store: &RadrootsEventStore,
    ) -> Result<(), RadrootsRelayTransportError> {
        let mut event_ids = Vec::new();
        let mut seen_event_ids = BTreeSet::new();
        for event in &self.events {
            if event.not_persisted || (!event.inserted && !event.duplicate) {
                continue;
            }
            let event_id = required_persisted_fetch_receipt_event_id(event.event_id.as_deref())?;
            if seen_event_ids.insert(event_id) {
                event_ids.push(event_id.to_owned());
            }
        }
        let visibilities = event_store.event_visibilities(event_ids.iter()).await?;
        let visibilities_by_event_id = event_ids
            .into_iter()
            .zip(visibilities)
            .collect::<BTreeMap<_, _>>();

        for event in &mut self.events {
            if event.not_persisted || (!event.inserted && !event.duplicate) {
                continue;
            }
            let event_id = required_persisted_fetch_receipt_event_id(event.event_id.as_deref())?;
            let visibility = visibilities_by_event_id
                .get(event_id)
                .cloned()
                .flatten()
                .ok_or_else(
                    || RadrootsRelayTransportError::MissingStoredEventVisibility {
                        event_id: event_id.to_owned(),
                    },
                )?;
            event.visibility = relay_fetch_visibility(event_id, visibility)?;
        }

        self.admission_unsupported_count = 0;
        self.admission_invalid_count = 0;
        self.valid_stream_eligible_count = 0;
        self.visible_count = 0;
        self.not_admitted_count = 0;
        self.not_current_count = 0;
        self.suppressed_count = 0;
        for event in &self.events {
            match event.admission {
                RadrootsRelayFetchEventAdmission::Unsupported => {
                    self.admission_unsupported_count += 1;
                }
                RadrootsRelayFetchEventAdmission::Invalid => self.admission_invalid_count += 1,
                RadrootsRelayFetchEventAdmission::NotEvaluated
                | RadrootsRelayFetchEventAdmission::Admitted => {}
            }
            if event.valid_stream == RadrootsRelayFetchEventValidStream::Eligible {
                self.valid_stream_eligible_count += 1;
            }
            match event.visibility {
                RadrootsRelayFetchEventVisibility::Visible => self.visible_count += 1,
                RadrootsRelayFetchEventVisibility::NotAdmitted => {
                    self.not_admitted_count += 1;
                }
                RadrootsRelayFetchEventVisibility::NotCurrent => self.not_current_count += 1,
                RadrootsRelayFetchEventVisibility::Suppressed => self.suppressed_count += 1,
                RadrootsRelayFetchEventVisibility::NotEvaluated
                | RadrootsRelayFetchEventVisibility::NotPersisted => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RadrootsRelayFetchRawBudgetExhaustion {
    RawEvents,
    RawJsonBytes,
}

impl RadrootsRelayFetchRawBudgetExhaustion {
    const fn current_relay_message(self) -> &'static str {
        match self {
            Self::RawEvents => "raw event scan limit reached before relay EOSE",
            Self::RawJsonBytes => "aggregate raw JSON byte limit reached before relay EOSE",
        }
    }

    const fn unqueried_relay_message(self) -> &'static str {
        match self {
            Self::RawEvents => {
                "relay was not queried because the global raw event scan limit was reached"
            }
            Self::RawJsonBytes => {
                "relay was not queried because the global aggregate raw JSON byte limit was reached"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RadrootsRelayFetchRawBudget {
    remaining_events: usize,
    remaining_json_bytes: usize,
    exhausted: Option<RadrootsRelayFetchRawBudgetExhaustion>,
}

impl RadrootsRelayFetchRawBudget {
    const fn new(max_raw_events: usize, max_raw_json_bytes: usize) -> Self {
        Self {
            remaining_events: max_raw_events,
            remaining_json_bytes: max_raw_json_bytes,
            exhausted: None,
        }
    }

    fn charge(
        &mut self,
        raw_json_bytes: usize,
    ) -> Result<(), RadrootsRelayFetchRawBudgetExhaustion> {
        if let Some(reason) = self.exhausted {
            return Err(reason);
        }
        let Some(remaining_events) = self.remaining_events.checked_sub(1) else {
            self.exhausted = Some(RadrootsRelayFetchRawBudgetExhaustion::RawEvents);
            return Err(RadrootsRelayFetchRawBudgetExhaustion::RawEvents);
        };
        let Some(remaining_json_bytes) = self.remaining_json_bytes.checked_sub(raw_json_bytes)
        else {
            self.exhausted = Some(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes);
            return Err(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes);
        };
        self.remaining_events = remaining_events;
        self.remaining_json_bytes = remaining_json_bytes;
        self.exhausted = if remaining_events == 0 {
            Some(RadrootsRelayFetchRawBudgetExhaustion::RawEvents)
        } else if remaining_json_bytes == 0 {
            Some(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes)
        } else {
            None
        };
        Ok(())
    }

    const fn exhaustion_reason(self) -> Option<RadrootsRelayFetchRawBudgetExhaustion> {
        self.exhausted
    }
}

fn process_relay_fetch_items(
    target_relays: Vec<String>,
    filters: Vec<RadrootsNostrFilter>,
    observed_at_ms: i64,
    max_events: usize,
    max_raw_events: usize,
    max_raw_json_bytes: usize,
    items: Vec<RadrootsRelayFetchItem>,
) -> Result<RadrootsRelayProcessedFetch, RadrootsRelayTransportError> {
    if target_relays.is_empty() {
        return Err(RadrootsRelayTransportError::EmptyTargetSet);
    }
    let mut processed = RadrootsRelayProcessedFetch {
        target_relays,
        items: Vec::new(),
        duplicate_count: 0,
        verification_failed_count: 0,
        malformed_count: 0,
        out_of_filter_count: 0,
        skipped_over_limit_count: 0,
        eose_count: 0,
        truncated_count: 0,
        closed_count: 0,
        notice_count: 0,
        relay_outcomes: Vec::new(),
    };
    let mut raw_budget = RadrootsRelayFetchRawBudget::new(max_raw_events, max_raw_json_bytes);
    let mut accepted_events = 0usize;
    let mut seen_event_ids = BTreeSet::new();
    let mut terminal_outcomes = BTreeMap::new();
    for item in items {
        let relay_url =
            canonical_requested_fetch_relay(processed.target_relays.as_slice(), item.relay_url())?;
        if let Some(next) = item.terminal_outcome_label() {
            if let Some(first) = terminal_outcomes.get(relay_url.as_str()).copied() {
                if first == next {
                    return Err(
                        RadrootsRelayTransportError::DuplicateFetchTerminalRelayUrl {
                            url: relay_url,
                        },
                    );
                }
                return Err(
                    RadrootsRelayTransportError::ConflictingFetchTerminalRelayUrl {
                        url: relay_url,
                        first,
                        next,
                    },
                );
            }
            terminal_outcomes.insert(relay_url.clone(), next);
        }
        match item.body {
            RadrootsRelayFetchItemBody::Event { raw_json, .. } => {
                if raw_budget.charge(raw_json.len()).is_err() {
                    processed.skipped_over_limit_count += 1;
                    continue;
                }
                if raw_json.len() > DEFAULT_RAW_JSON_MAX_BYTES {
                    processed.verification_failed_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: None,
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                verification: RadrootsRelayFetchEventVerification::Failed,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some(format!(
                                    "event raw JSON exceeds {DEFAULT_RAW_JSON_MAX_BYTES} byte limit"
                                )),
                            }
                            .checked()?,
                        ));
                    continue;
                }
                let parsed = RadrootsNostrEvent::from_json(raw_json.as_str());
                let Ok(raw_event) = parsed else {
                    processed.malformed_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: None,
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: true,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                verification: RadrootsRelayFetchEventVerification::NotEvaluated,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some("event JSON parse failed".to_owned()),
                            }
                            .checked()?,
                        ));
                    continue;
                };
                if let Err(error) =
                    RadrootsEventIngest::from_raw_json(raw_json.clone(), observed_at_ms)
                {
                    processed.verification_failed_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: Some(raw_event.id.to_hex()),
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                verification: RadrootsRelayFetchEventVerification::Failed,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some(error.to_string()),
                            }
                            .checked()?,
                        ));
                    continue;
                }
                if !relay_fetch_event_matches_filters(&filters, &raw_event) {
                    processed.out_of_filter_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: Some(raw_event.id.to_hex()),
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: false,
                                out_of_filter: true,
                                skipped_over_limit: false,
                                verification: RadrootsRelayFetchEventVerification::Verified,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some("event did not match relay fetch filters".to_owned()),
                            }
                            .checked()?,
                        ));
                    continue;
                }
                let event_id = raw_event.id.to_hex();
                if !seen_event_ids.insert(event_id) {
                    processed.duplicate_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Duplicate(
                            RadrootsRelayFetchedEvent::from_verified(
                                relay_url,
                                raw_event,
                                raw_json,
                                observed_at_ms,
                            )?,
                        ));
                    continue;
                }
                if accepted_events >= max_events {
                    processed.skipped_over_limit_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: Some(raw_event.id.to_hex()),
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: true,
                                verification: RadrootsRelayFetchEventVerification::Verified,
                                admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
                                admission_code: None,
                                valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
                                visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
                                message: Some(
                                    "accepted relay fetch event limit reached".to_owned(),
                                ),
                            }
                            .checked()?,
                        ));
                    continue;
                }
                accepted_events += 1;
                processed
                    .items
                    .push(RadrootsRelayProcessedFetchItem::Accepted(
                        RadrootsRelayFetchedEvent::from_verified(
                            relay_url,
                            raw_event,
                            raw_json,
                            observed_at_ms,
                        )?,
                    ));
            }
            RadrootsRelayFetchItemBody::Eose { .. } => {
                processed.eose_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome::eose(relay_url)?);
            }
            RadrootsRelayFetchItemBody::Truncated { message, .. } => {
                processed.truncated_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome::truncated(
                        relay_url, message,
                    )?);
            }
            RadrootsRelayFetchItemBody::Closed { message, .. } => {
                processed.closed_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome::closed(relay_url, message)?);
            }
            RadrootsRelayFetchItemBody::Notice { message, .. } => {
                processed.notice_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome::notice(relay_url, message)?);
            }
        }
    }
    Ok(processed)
}

fn canonical_requested_fetch_relay(
    target_relays: &[String],
    relay_url: &str,
) -> Result<String, RadrootsRelayTransportError> {
    let target = RadrootsTransportTarget::nostr_relay(relay_url).map_err(|error| {
        RadrootsRelayTransportError::InvalidFetchItemRelayUrl {
            url: relay_url.to_owned(),
            reason: error.to_string(),
        }
    })?;
    let canonical = target.uri().as_str();
    if !target_relays.iter().any(|requested| requested == canonical) {
        return Err(RadrootsRelayTransportError::UnexpectedFetchItemRelayUrl {
            url: canonical.to_owned(),
        });
    }
    Ok(canonical.to_owned())
}

fn accepted_fetch_event_receipt(
    event: &RadrootsRelayFetchedEvent,
) -> Result<RadrootsRelayFetchEventReceipt, RadrootsRelayTransportError> {
    RadrootsRelayFetchEventReceipt {
        relay_url: event.relay_url().to_owned(),
        event_id: Some(event.event().id.to_hex()),
        inserted: false,
        duplicate: false,
        not_persisted: false,
        malformed: false,
        out_of_filter: false,
        skipped_over_limit: false,
        verification: RadrootsRelayFetchEventVerification::Verified,
        admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
        admission_code: None,
        valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
        visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
        message: Some("event accepted by relay fetch filters".to_owned()),
    }
    .checked()
}

fn duplicate_fetch_event_receipt(
    event: &RadrootsRelayFetchedEvent,
) -> Result<RadrootsRelayFetchEventReceipt, RadrootsRelayTransportError> {
    RadrootsRelayFetchEventReceipt {
        relay_url: event.relay_url().to_owned(),
        event_id: Some(event.event().id.to_hex()),
        inserted: false,
        duplicate: true,
        not_persisted: false,
        malformed: false,
        out_of_filter: false,
        skipped_over_limit: false,
        verification: RadrootsRelayFetchEventVerification::Verified,
        admission: RadrootsRelayFetchEventAdmission::NotEvaluated,
        admission_code: None,
        valid_stream: RadrootsRelayFetchEventValidStream::NotEvaluated,
        visibility: RadrootsRelayFetchEventVisibility::NotEvaluated,
        message: Some("event ID was already observed in this relay fetch".to_owned()),
    }
    .checked()
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn relay_fetch_event_matches_filters(
    filters: &[RadrootsNostrFilter],
    event: &RadrootsNostrEvent,
) -> bool {
    !filters.is_empty()
        && filters
            .iter()
            .any(|filter| filter.match_event(event, MatchEventOptions::new()))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RadrootsNostrClientFetchAdapter;

impl RadrootsRelayFetchAdapter for RadrootsNostrClientFetchAdapter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch<'a>(
        &'a self,
        request: RadrootsRelayFetchRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError>> {
        Box::pin(async move { fetch_from_nostr_relays(request).await })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn fetch_from_nostr_relays(
    request: RadrootsRelayFetchRequest,
) -> Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError> {
    if request.filters.as_slice().is_empty() {
        return Err(RadrootsRelayTransportError::EmptyFetchFilters);
    }
    let timeout = Duration::from_millis(request.timeout_ms);
    let filters = request.filters.as_slice().to_vec();
    let mut raw_budget =
        RadrootsRelayFetchRawBudget::new(request.max_raw_events, request.max_raw_json_bytes);
    let mut items = Vec::new();
    let relay_urls = request.relay_targets.relay_strings();
    for (relay_index, relay_url) in relay_urls.iter().cloned().enumerate() {
        if let Some(reason) = raw_budget.exhaustion_reason() {
            for relay_url in relay_urls[relay_index..].iter().cloned() {
                items.push(RadrootsRelayFetchItem::truncated(
                    relay_url,
                    reason.unqueried_relay_message(),
                )?);
            }
            break;
        }
        let client = RadrootsNostrClient::new_signerless();
        if let Err(error) = client.add_read_relay(relay_url.as_str()).await {
            items.push(RadrootsRelayFetchItem::closed(
                relay_url,
                error.to_string(),
            )?);
            continue;
        }
        let connection_output = client.try_connect(timeout).await;
        if connection_output.success.is_empty() {
            items.push(RadrootsRelayFetchItem::closed(
                relay_url,
                summarize_nostr_output_failures(&connection_output.failed),
            )?);
            continue;
        }
        let mut closed = false;
        let mut truncated_message = None;
        for filter in filters.iter().cloned() {
            if let Some(reason) = raw_budget.exhaustion_reason() {
                truncated_message = Some(reason.current_relay_message().to_owned());
                break;
            }
            let filter_limit = filter
                .limit
                .unwrap_or(raw_budget.remaining_events)
                .min(raw_budget.remaining_events);
            match client
                .stream_events(filter.limit(filter_limit), timeout)
                .await
            {
                Ok(mut events) => {
                    loop {
                        if let Some(reason) = raw_budget.exhaustion_reason() {
                            truncated_message = Some(reason.current_relay_message().to_owned());
                            break;
                        }
                        let Some(event) = events.next().await else {
                            break;
                        };
                        let raw_json = event.as_json();
                        if let Err(reason) = raw_budget.charge(raw_json.len()) {
                            truncated_message = Some(reason.current_relay_message().to_owned());
                            break;
                        }
                        if raw_json.len() > DEFAULT_RAW_JSON_MAX_BYTES {
                            truncated_message = Some(format!(
                                "relay event raw JSON exceeds {DEFAULT_RAW_JSON_MAX_BYTES} byte limit"
                            ));
                            break;
                        }
                        items.push(RadrootsRelayFetchItem::event(relay_url.clone(), raw_json)?);
                    }
                    if truncated_message.is_some() {
                        break;
                    }
                }
                Err(error) => {
                    items.push(RadrootsRelayFetchItem::closed(
                        relay_url.clone(),
                        error.to_string(),
                    )?);
                    closed = true;
                    break;
                }
            }
        }
        if let Some(message) = truncated_message {
            items.push(RadrootsRelayFetchItem::truncated(relay_url, message)?);
        } else if !closed {
            items.push(unproven_relay_stream_completion(relay_url)?);
        }
    }
    Ok(items)
}

fn unproven_relay_stream_completion(
    relay_url: String,
) -> Result<RadrootsRelayFetchItem, RadrootsRelayTransportError> {
    RadrootsRelayFetchItem::closed(
        relay_url,
        "relay stream ended before EOSE could be observed",
    )
}

fn summarize_nostr_output_failures<K, E>(failed: &std::collections::HashMap<K, E>) -> String
where
    K: std::fmt::Display + Eq + std::hash::Hash,
    E: std::fmt::Display,
{
    if failed.is_empty() {
        return "no relay acknowledged the operation".to_owned();
    }
    failed
        .iter()
        .map(|(relay, error)| format!("{relay}: {error}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Clone, Default)]
pub struct RadrootsMockRelayFetchAdapter {
    items: Arc<Mutex<Vec<RadrootsRelayFetchItem>>>,
}

impl RadrootsMockRelayFetchAdapter {
    pub fn new(items: Vec<RadrootsRelayFetchItem>) -> Self {
        Self {
            items: Arc::new(Mutex::new(items)),
        }
    }
}

impl RadrootsRelayFetchAdapter for RadrootsMockRelayFetchAdapter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch<'a>(
        &'a self,
        _request: RadrootsRelayFetchRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError>> {
        Box::pin(async move { Ok(self.items.lock().map_err(fetch_item_lock_error)?.clone()) })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn fetch_item_lock_error<T>(_error: PoisonError<T>) -> RadrootsRelayTransportError {
    RadrootsRelayTransportError::Transport("fetch item lock poisoned".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsNostrEvent, RadrootsRelayFetchEventAdmission, RadrootsRelayFetchEventValidStream,
        RadrootsRelayFetchEventVerification, RadrootsRelayFetchEventVisibility,
        RadrootsRelayFetchItem, RadrootsRelayFetchRawBudget, RadrootsRelayFetchRawBudgetExhaustion,
        RadrootsRelayTransportError, relay_fetch_event_matches_filters, relay_fetch_visibility,
        required_persisted_fetch_receipt_event_id, summarize_nostr_output_failures,
        unproven_relay_stream_completion,
    };
    use nostr::JsonUtil;
    use radroots_event_store::{RadrootsEventVisibility, RadrootsNip09SuppressionReason};
    use radroots_nostr::prelude::{
        RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrSecretKey,
    };
    use std::collections::HashMap;

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";

    fn signed_raw_event() -> RadrootsNostrEvent {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let event = nostr::EventBuilder::new(nostr::Kind::TextNote, "hello")
            .sign_with_keys(&keys)
            .expect("signed event");
        RadrootsNostrEvent::from_json(event.as_json().as_str()).expect("raw event")
    }

    #[test]
    fn relay_fetch_filter_helper_rejects_empty_filter_set() {
        let event = signed_raw_event();
        assert!(!relay_fetch_event_matches_filters(&[], &event));
        assert!(relay_fetch_event_matches_filters(
            &[RadrootsNostrFilter::new().kind(RadrootsNostrKind::TextNote)],
            &event
        ));
    }

    #[test]
    fn nostr_output_failure_summary_covers_empty_and_reported_failures() {
        assert_eq!(
            summarize_nostr_output_failures::<String, String>(&HashMap::new()),
            "no relay acknowledged the operation"
        );

        let mut failures = HashMap::new();
        failures.insert("wss://relay.example.com".to_owned(), "timeout".to_owned());
        failures.insert("wss://relay-2.example.com".to_owned(), "denied".to_owned());

        let summary = summarize_nostr_output_failures(&failures);
        assert!(summary.contains("wss://relay.example.com: timeout"));
        assert!(summary.contains("wss://relay-2.example.com: denied"));
        assert!(summary.contains("; "));
    }

    #[test]
    fn unproven_sdk_stream_completion_never_claims_eose() {
        assert_eq!(
            unproven_relay_stream_completion("wss://relay.example".to_owned())
                .expect("bounded completion item"),
            RadrootsRelayFetchItem::closed(
                "wss://relay.example",
                "relay stream ended before EOSE could be observed",
            )
            .expect("bounded completion item")
        );
    }

    #[test]
    fn raw_fetch_budget_allows_exact_boundaries_and_sticks_after_exhaustion() {
        let mut count_budget = RadrootsRelayFetchRawBudget::new(1, 10);
        assert_eq!(count_budget.charge(1), Ok(()));
        assert_eq!(
            count_budget.exhaustion_reason(),
            Some(RadrootsRelayFetchRawBudgetExhaustion::RawEvents)
        );
        assert_eq!(
            count_budget.charge(1),
            Err(RadrootsRelayFetchRawBudgetExhaustion::RawEvents)
        );

        let mut byte_budget = RadrootsRelayFetchRawBudget::new(2, 5);
        assert_eq!(byte_budget.charge(5), Ok(()));
        assert_eq!(
            byte_budget.exhaustion_reason(),
            Some(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes)
        );
        assert_eq!(
            byte_budget.charge(0),
            Err(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes)
        );
    }

    #[test]
    fn raw_fetch_budget_rejects_crossing_without_partial_charge() {
        let mut budget = RadrootsRelayFetchRawBudget::new(2, 4);
        assert_eq!(
            budget.charge(usize::MAX),
            Err(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes)
        );
        assert_eq!(budget.remaining_events, 2);
        assert_eq!(budget.remaining_json_bytes, 4);
        assert_eq!(
            budget.charge(1),
            Err(RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes)
        );
        assert_eq!(
            RadrootsRelayFetchRawBudgetExhaustion::RawEvents.current_relay_message(),
            "raw event scan limit reached before relay EOSE"
        );
        assert!(
            RadrootsRelayFetchRawBudgetExhaustion::RawJsonBytes
                .unqueried_relay_message()
                .contains("aggregate raw JSON byte limit")
        );
    }

    #[test]
    fn event_store_visibility_maps_without_admission_collapse() {
        let cases = [
            (
                RadrootsEventVisibility::Visible,
                RadrootsRelayFetchEventVisibility::Visible,
            ),
            (
                RadrootsEventVisibility::NotAdmitted,
                RadrootsRelayFetchEventVisibility::NotAdmitted,
            ),
            (
                RadrootsEventVisibility::NotCurrent {
                    raw_head_event_id: "head".to_owned(),
                },
                RadrootsRelayFetchEventVisibility::NotCurrent,
            ),
            (
                RadrootsEventVisibility::Suppressed {
                    reason: RadrootsNip09SuppressionReason::EventIdReference,
                    event_reference_request_id: None,
                    address_reference_request_id: None,
                    address_reference_cutoff: None,
                },
                RadrootsRelayFetchEventVisibility::Suppressed,
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(
                relay_fetch_visibility("event", source).expect("known visibility"),
                expected
            );
        }
    }

    #[test]
    fn persisted_fetch_receipts_require_an_event_id() {
        assert_eq!(
            required_persisted_fetch_receipt_event_id(Some("event")).expect("event id"),
            "event"
        );
        assert!(matches!(
            required_persisted_fetch_receipt_event_id(None),
            Err(RadrootsRelayTransportError::MissingPersistedFetchReceiptEventId)
        ));
    }

    #[test]
    fn event_processing_outcomes_have_stable_exhaustive_wire_values() {
        let cases = [
            serde_json::to_string(&RadrootsRelayFetchEventVerification::NotEvaluated)
                .expect("verification"),
            serde_json::to_string(&RadrootsRelayFetchEventVerification::Verified)
                .expect("verification"),
            serde_json::to_string(&RadrootsRelayFetchEventVerification::Failed)
                .expect("verification"),
            serde_json::to_string(&RadrootsRelayFetchEventAdmission::NotEvaluated)
                .expect("admission"),
            serde_json::to_string(&RadrootsRelayFetchEventAdmission::Admitted).expect("admission"),
            serde_json::to_string(&RadrootsRelayFetchEventAdmission::Unsupported)
                .expect("admission"),
            serde_json::to_string(&RadrootsRelayFetchEventAdmission::Invalid).expect("admission"),
            serde_json::to_string(&RadrootsRelayFetchEventValidStream::NotEvaluated)
                .expect("valid stream"),
            serde_json::to_string(&RadrootsRelayFetchEventValidStream::Eligible)
                .expect("valid stream"),
            serde_json::to_string(&RadrootsRelayFetchEventValidStream::Ineligible)
                .expect("valid stream"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::NotEvaluated)
                .expect("visibility"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::NotPersisted)
                .expect("visibility"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::Visible).expect("visibility"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::NotAdmitted)
                .expect("visibility"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::NotCurrent)
                .expect("visibility"),
            serde_json::to_string(&RadrootsRelayFetchEventVisibility::Suppressed)
                .expect("visibility"),
        ];
        assert_eq!(
            cases,
            [
                "\"not_evaluated\"",
                "\"verified\"",
                "\"failed\"",
                "\"not_evaluated\"",
                "\"admitted\"",
                "\"unsupported\"",
                "\"invalid\"",
                "\"not_evaluated\"",
                "\"eligible\"",
                "\"ineligible\"",
                "\"not_evaluated\"",
                "\"not_persisted\"",
                "\"visible\"",
                "\"not_admitted\"",
                "\"not_current\"",
                "\"suppressed\"",
            ]
        );
    }
}
