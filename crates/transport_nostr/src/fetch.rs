#![forbid(unsafe_code)]

use crate::error::ensure_nonnegative_timestamp;
use crate::{RadrootsRelayOutcome, RadrootsRelayTargetSet, RadrootsRelayTransportError};
use core::time::Duration;
use futures::{StreamExt, future::BoxFuture};
use nostr::{JsonUtil, filter::MatchEventOptions};
use radroots_event_store::{
    RadrootsEventAdmissionStatus, RadrootsEventIngest, RadrootsEventPersistence,
    RadrootsEventStore, RadrootsTransportObservation, RadrootsTransportObservationType,
};
use radroots_nostr::prelude::{RadrootsNostrClient, RadrootsNostrEvent, RadrootsNostrFilter};
use radroots_transport::{RadrootsTransportKind, RadrootsTransportTarget};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

const DEFAULT_RELAY_FETCH_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_RELAY_FETCH_RAW_SCAN_MULTIPLIER: usize = 64;

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
        let filters = filters.into_iter().collect::<Vec<_>>();
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
        ensure_positive_limit("max_events", max_events)?;
        Ok(Self {
            mode,
            observed_at_ms,
            max_events,
            max_raw_events: default_raw_event_scan_limit(max_events),
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
        ensure_positive_limit("max_raw_events", max_raw_events)?;
        self.max_raw_events = max_raw_events;
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

fn ensure_positive_timeout(
    field: &'static str,
    value: u64,
) -> Result<(), RadrootsRelayTransportError> {
    if value == 0 {
        return Err(RadrootsRelayTransportError::InvalidFetchLimit { field });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsRelayFetchItem {
    Event { relay_url: String, raw_json: String },
    Eose { relay_url: String },
    Truncated { relay_url: String, message: String },
    Closed { relay_url: String, message: String },
    Notice { relay_url: String, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayFetchOutcomeKind {
    Eose,
    Truncated,
    Closed,
    Notice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchRelayOutcome {
    pub relay_url: String,
    pub kind: RadrootsRelayFetchOutcomeKind,
    pub relay_outcome: Option<RadrootsRelayOutcome>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchEventReceipt {
    pub relay_url: String,
    pub event_id: Option<String>,
    pub inserted: bool,
    pub duplicate: bool,
    pub not_persisted: bool,
    pub unsupported: bool,
    pub invalid: bool,
    pub malformed: bool,
    pub out_of_filter: bool,
    pub skipped_over_limit: bool,
    pub valid_stream_eligible: bool,
    pub admission_status: Option<String>,
    pub admission_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RadrootsRelayFetchedEvent {
    pub relay_url: String,
    pub event: RadrootsNostrEvent,
    pub raw_json: String,
    pub observed_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchFailure {
    pub relay_url: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct RadrootsRelayFetchedEventsReceipt {
    pub target_relays: Vec<String>,
    pub connected_relays: Vec<String>,
    pub failed_relays: Vec<RadrootsRelayFetchFailure>,
    pub events: Vec<RadrootsRelayFetchedEvent>,
    pub event_receipts: Vec<RadrootsRelayFetchEventReceipt>,
    pub duplicate_count: usize,
    pub invalid_count: usize,
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
    pub unsupported_count: usize,
    pub invalid_count: usize,
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
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    Ok(process_relay_fetch_items(
        target_relays,
        filters,
        observed_at_ms,
        max_events,
        max_raw_events,
        items,
    )?
    .into_fetched_events_receipt())
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
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    let processed = process_relay_fetch_items(
        target_relays,
        filters,
        observed_at_ms,
        max_events,
        max_raw_events,
        items,
    )?;
    let mut receipt = RadrootsRelayFetchReceipt::from_processed_counts(&processed);
    for item in processed.items {
        match item {
            RadrootsRelayProcessedFetchItem::Receipt(event_receipt) => {
                receipt.events.push(event_receipt);
            }
            RadrootsRelayProcessedFetchItem::Accepted(RadrootsRelayFetchedEvent {
                relay_url,
                event: raw_event,
                raw_json,
                observed_at_ms,
            })
            | RadrootsRelayProcessedFetchItem::Duplicate(RadrootsRelayFetchedEvent {
                relay_url,
                event: raw_event,
                raw_json,
                observed_at_ms,
            }) => {
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
                        receipt.malformed_count += 1;
                        receipt.events.push(RadrootsRelayFetchEventReceipt {
                            relay_url,
                            event_id: Some(raw_event.id.to_hex()),
                            inserted: false,
                            duplicate: false,
                            not_persisted: false,
                            unsupported: false,
                            invalid: false,
                            malformed: true,
                            out_of_filter: false,
                            skipped_over_limit: false,
                            valid_stream_eligible: false,
                            admission_status: None,
                            admission_code: None,
                            message: Some(error.to_string()),
                        });
                        continue;
                    }
                };
                let store_receipt = event_store.ingest_event(ingest).await?;
                let unsupported =
                    store_receipt.admission_status == RadrootsEventAdmissionStatus::Unsupported;
                let invalid =
                    store_receipt.admission_status == RadrootsEventAdmissionStatus::Invalid;
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
                if unsupported {
                    receipt.unsupported_count += 1;
                }
                if invalid {
                    receipt.invalid_count += 1;
                }
                receipt.events.push(RadrootsRelayFetchEventReceipt {
                    relay_url,
                    event_id: Some(store_receipt.event_id),
                    inserted,
                    duplicate,
                    not_persisted,
                    unsupported,
                    invalid,
                    malformed: false,
                    out_of_filter: false,
                    skipped_over_limit: false,
                    valid_stream_eligible: store_receipt.valid_stream_eligible,
                    admission_status: Some(store_receipt.admission_status.as_str().to_owned()),
                    admission_code: store_receipt.admission_code,
                    message: None,
                });
            }
        }
    }
    Ok(receipt)
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
    invalid_count: usize,
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
    fn into_fetched_events_receipt(self) -> RadrootsRelayFetchedEventsReceipt {
        let mut events = Vec::new();
        let mut event_receipts = Vec::new();
        for item in self.items {
            match item {
                RadrootsRelayProcessedFetchItem::Accepted(event) => {
                    event_receipts.push(accepted_fetch_event_receipt(&event));
                    events.push(event);
                }
                RadrootsRelayProcessedFetchItem::Duplicate(event) => {
                    event_receipts.push(duplicate_fetch_event_receipt(&event));
                }
                RadrootsRelayProcessedFetchItem::Receipt(receipt) => event_receipts.push(receipt),
            }
        }
        let connected_relays = self
            .relay_outcomes
            .iter()
            .filter(|outcome| outcome.kind == RadrootsRelayFetchOutcomeKind::Eose)
            .map(|outcome| outcome.relay_url.clone())
            .collect();
        let failed_relays = self
            .relay_outcomes
            .iter()
            .filter(|outcome| outcome.kind == RadrootsRelayFetchOutcomeKind::Closed)
            .map(|outcome| RadrootsRelayFetchFailure {
                relay_url: outcome.relay_url.clone(),
                reason: outcome.message.clone().unwrap_or_default(),
            })
            .collect();
        RadrootsRelayFetchedEventsReceipt {
            target_relays: self.target_relays,
            connected_relays,
            failed_relays,
            events,
            event_receipts,
            duplicate_count: self.duplicate_count,
            invalid_count: self.invalid_count,
            malformed_count: self.malformed_count,
            out_of_filter_count: self.out_of_filter_count,
            skipped_over_limit_count: self.skipped_over_limit_count,
            eose_count: self.eose_count,
            truncated_count: self.truncated_count,
            closed_count: self.closed_count,
            notice_count: self.notice_count,
            relay_outcomes: self.relay_outcomes,
        }
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
            unsupported_count: 0,
            invalid_count: processed.invalid_count,
            eose_count: processed.eose_count,
            truncated_count: processed.truncated_count,
            closed_count: processed.closed_count,
            notice_count: processed.notice_count,
            events: Vec::new(),
            relay_outcomes: processed.relay_outcomes.clone(),
        }
    }
}

fn process_relay_fetch_items(
    target_relays: Vec<String>,
    filters: Vec<RadrootsNostrFilter>,
    observed_at_ms: i64,
    max_events: usize,
    max_raw_events: usize,
    items: Vec<RadrootsRelayFetchItem>,
) -> Result<RadrootsRelayProcessedFetch, RadrootsRelayTransportError> {
    if target_relays.is_empty() {
        return Err(RadrootsRelayTransportError::EmptyTargetSet);
    }
    let mut processed = RadrootsRelayProcessedFetch {
        target_relays,
        items: Vec::new(),
        duplicate_count: 0,
        invalid_count: 0,
        malformed_count: 0,
        out_of_filter_count: 0,
        skipped_over_limit_count: 0,
        eose_count: 0,
        truncated_count: 0,
        closed_count: 0,
        notice_count: 0,
        relay_outcomes: Vec::new(),
    };
    let mut scanned_raw_events = 0usize;
    let mut accepted_events = 0usize;
    let mut seen_event_ids = BTreeSet::new();
    let mut terminal_outcomes = BTreeMap::new();
    for item in items {
        let item_relay_url = match &item {
            RadrootsRelayFetchItem::Event { relay_url, .. }
            | RadrootsRelayFetchItem::Eose { relay_url }
            | RadrootsRelayFetchItem::Truncated { relay_url, .. }
            | RadrootsRelayFetchItem::Closed { relay_url, .. }
            | RadrootsRelayFetchItem::Notice { relay_url, .. } => relay_url,
        };
        let relay_url = canonical_requested_fetch_relay(
            processed.target_relays.as_slice(),
            item_relay_url.as_str(),
        )?;
        if let Some(next) = fetch_terminal_outcome_label(&item) {
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
        match item {
            RadrootsRelayFetchItem::Event { raw_json, .. } => {
                if scanned_raw_events >= max_raw_events {
                    processed.skipped_over_limit_count += 1;
                    continue;
                }
                scanned_raw_events += 1;
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
                                unsupported: false,
                                invalid: false,
                                malformed: true,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                valid_stream_eligible: false,
                                admission_status: None,
                                admission_code: None,
                                message: Some("event JSON parse failed".to_owned()),
                            },
                        ));
                    continue;
                };
                if let Err(error) =
                    RadrootsEventIngest::from_raw_json(raw_json.clone(), observed_at_ms)
                {
                    processed.invalid_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Receipt(
                            RadrootsRelayFetchEventReceipt {
                                relay_url,
                                event_id: Some(raw_event.id.to_hex()),
                                inserted: false,
                                duplicate: false,
                                not_persisted: false,
                                unsupported: false,
                                invalid: true,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                valid_stream_eligible: false,
                                admission_status: None,
                                admission_code: None,
                                message: Some(error.to_string()),
                            },
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
                                unsupported: false,
                                invalid: false,
                                malformed: false,
                                out_of_filter: true,
                                skipped_over_limit: false,
                                valid_stream_eligible: false,
                                admission_status: None,
                                admission_code: None,
                                message: Some("event did not match relay fetch filters".to_owned()),
                            },
                        ));
                    continue;
                }
                let event_id = raw_event.id.to_hex();
                if !seen_event_ids.insert(event_id) {
                    processed.duplicate_count += 1;
                    processed
                        .items
                        .push(RadrootsRelayProcessedFetchItem::Duplicate(
                            RadrootsRelayFetchedEvent {
                                relay_url,
                                event: raw_event,
                                raw_json,
                                observed_at_ms,
                            },
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
                                unsupported: false,
                                invalid: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: true,
                                valid_stream_eligible: false,
                                admission_status: None,
                                admission_code: None,
                                message: Some(
                                    "accepted relay fetch event limit reached".to_owned(),
                                ),
                            },
                        ));
                    continue;
                }
                accepted_events += 1;
                processed
                    .items
                    .push(RadrootsRelayProcessedFetchItem::Accepted(
                        RadrootsRelayFetchedEvent {
                            relay_url,
                            event: raw_event,
                            raw_json,
                            observed_at_ms,
                        },
                    ));
            }
            RadrootsRelayFetchItem::Eose { .. } => {
                processed.eose_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome {
                        relay_url,
                        kind: RadrootsRelayFetchOutcomeKind::Eose,
                        relay_outcome: None,
                        message: None,
                    });
            }
            RadrootsRelayFetchItem::Truncated { message, .. } => {
                processed.truncated_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome {
                        relay_url,
                        kind: RadrootsRelayFetchOutcomeKind::Truncated,
                        relay_outcome: None,
                        message: Some(message),
                    });
            }
            RadrootsRelayFetchItem::Closed { message, .. } => {
                processed.closed_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome {
                        relay_url,
                        kind: RadrootsRelayFetchOutcomeKind::Closed,
                        relay_outcome: Some(RadrootsRelayOutcome::classify(message.as_str())),
                        message: Some(message),
                    });
            }
            RadrootsRelayFetchItem::Notice { message, .. } => {
                processed.notice_count += 1;
                processed
                    .relay_outcomes
                    .push(RadrootsRelayFetchRelayOutcome {
                        relay_url,
                        kind: RadrootsRelayFetchOutcomeKind::Notice,
                        relay_outcome: None,
                        message: Some(message),
                    });
            }
        }
    }
    Ok(processed)
}

fn fetch_terminal_outcome_label(item: &RadrootsRelayFetchItem) -> Option<&'static str> {
    match item {
        RadrootsRelayFetchItem::Eose { .. } => Some("eose"),
        RadrootsRelayFetchItem::Truncated { .. } => Some("truncated"),
        RadrootsRelayFetchItem::Closed { .. } => Some("closed"),
        RadrootsRelayFetchItem::Event { .. } | RadrootsRelayFetchItem::Notice { .. } => None,
    }
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
) -> RadrootsRelayFetchEventReceipt {
    RadrootsRelayFetchEventReceipt {
        relay_url: event.relay_url.clone(),
        event_id: Some(event.event.id.to_hex()),
        inserted: false,
        duplicate: false,
        not_persisted: false,
        unsupported: false,
        invalid: false,
        malformed: false,
        out_of_filter: false,
        skipped_over_limit: false,
        valid_stream_eligible: false,
        admission_status: None,
        admission_code: None,
        message: Some("event accepted by relay fetch filters".to_owned()),
    }
}

fn duplicate_fetch_event_receipt(
    event: &RadrootsRelayFetchedEvent,
) -> RadrootsRelayFetchEventReceipt {
    RadrootsRelayFetchEventReceipt {
        relay_url: event.relay_url.clone(),
        event_id: Some(event.event.id.to_hex()),
        inserted: false,
        duplicate: true,
        not_persisted: false,
        unsupported: false,
        invalid: false,
        malformed: false,
        out_of_filter: false,
        skipped_over_limit: false,
        valid_stream_eligible: false,
        admission_status: None,
        admission_code: None,
        message: Some("event ID was already observed in this relay fetch".to_owned()),
    }
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
    let mut remaining_raw_events = request.max_raw_events;
    let mut items = Vec::new();
    let relay_urls = request.relay_targets.relay_strings();
    for (relay_index, relay_url) in relay_urls.iter().cloned().enumerate() {
        if remaining_raw_events == 0 {
            items.extend(relay_urls[relay_index..].iter().cloned().map(|relay_url| {
                RadrootsRelayFetchItem::Truncated {
                    relay_url,
                    message:
                        "relay was not queried because the global raw event scan limit was reached"
                            .to_owned(),
                }
            }));
            break;
        }
        let client = RadrootsNostrClient::new_signerless();
        if let Err(error) = client.add_read_relay(relay_url.as_str()).await {
            items.push(RadrootsRelayFetchItem::Closed {
                relay_url,
                message: error.to_string(),
            });
            continue;
        }
        let connection_output = client.try_connect(timeout).await;
        if connection_output.success.is_empty() {
            items.push(RadrootsRelayFetchItem::Closed {
                relay_url,
                message: summarize_nostr_output_failures(&connection_output.failed),
            });
            continue;
        }
        let mut closed = false;
        let mut truncated = false;
        for filter in filters.iter().cloned() {
            if remaining_raw_events == 0 {
                truncated = true;
                break;
            }
            let filter_limit = filter
                .limit
                .unwrap_or(remaining_raw_events)
                .min(remaining_raw_events);
            match client
                .stream_events(filter.limit(filter_limit), timeout)
                .await
            {
                Ok(mut events) => {
                    loop {
                        if remaining_raw_events == 0 {
                            truncated = true;
                            break;
                        }
                        let Some(event) = events.next().await else {
                            break;
                        };
                        items.push(RadrootsRelayFetchItem::Event {
                            relay_url: relay_url.clone(),
                            raw_json: event.as_json(),
                        });
                        remaining_raw_events -= 1;
                    }
                    if truncated {
                        break;
                    }
                }
                Err(error) => {
                    items.push(RadrootsRelayFetchItem::Closed {
                        relay_url: relay_url.clone(),
                        message: error.to_string(),
                    });
                    closed = true;
                    break;
                }
            }
        }
        if truncated {
            items.push(RadrootsRelayFetchItem::Truncated {
                relay_url,
                message: "raw event scan limit reached before relay EOSE".to_owned(),
            });
        } else if !closed {
            items.push(unproven_relay_stream_completion(relay_url));
        }
    }
    Ok(items)
}

fn unproven_relay_stream_completion(relay_url: String) -> RadrootsRelayFetchItem {
    RadrootsRelayFetchItem::Closed {
        relay_url,
        message: "relay stream ended before EOSE could be observed".to_owned(),
    }
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
        RadrootsNostrEvent, RadrootsRelayFetchItem, relay_fetch_event_matches_filters,
        summarize_nostr_output_failures, unproven_relay_stream_completion,
    };
    use nostr::JsonUtil;
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
            unproven_relay_stream_completion("wss://relay.example".to_owned()),
            RadrootsRelayFetchItem::Closed {
                relay_url: "wss://relay.example".to_owned(),
                message: "relay stream ended before EOSE could be observed".to_owned(),
            }
        );
    }
}
