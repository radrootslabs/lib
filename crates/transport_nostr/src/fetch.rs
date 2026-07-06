#![forbid(unsafe_code)]

use crate::{RadrootsRelayOutcome, RadrootsRelayTransportError};
use core::time::Duration;
use futures::future::BoxFuture;
use nostr::{JsonUtil, filter::MatchEventOptions};
use radroots_event_store::{
    RadrootsEventContractStatus, RadrootsEventIngest, RadrootsEventStore,
    RadrootsTransportObservation, RadrootsTransportObservationType,
};
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrEvent, RadrootsNostrFilter, radroots_event_from_nostr,
};
use radroots_transport::RadrootsTransportKind;
use serde::{Deserialize, Serialize};
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
    relay_urls: Vec<String>,
    filters: RadrootsRelayFetchFilters,
    timeout_ms: u64,
}

impl RadrootsRelayFetchRequest {
    pub fn fetch<I>(
        observed_at_ms: i64,
        max_events: usize,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        Self::new(
            RadrootsRelayFetchMode::Fetch,
            observed_at_ms,
            max_events,
            filters,
        )
    }

    pub fn subscription<I>(
        observed_at_ms: i64,
        max_events: usize,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        Self::new(
            RadrootsRelayFetchMode::Subscription,
            observed_at_ms,
            max_events,
            filters,
        )
    }

    fn new<I>(
        mode: RadrootsRelayFetchMode,
        observed_at_ms: i64,
        max_events: usize,
        filters: I,
    ) -> Result<Self, RadrootsRelayTransportError>
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        ensure_positive_limit("max_events", max_events)?;
        Ok(Self {
            mode,
            observed_at_ms,
            max_events,
            max_raw_events: default_raw_event_scan_limit(max_events),
            relay_urls: Vec::new(),
            filters: RadrootsRelayFetchFilters::new(filters)?,
            timeout_ms: DEFAULT_RELAY_FETCH_TIMEOUT_MS,
        })
    }

    pub fn with_relay_urls<I, S>(mut self, relay_urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.relay_urls = relay_urls.into_iter().map(Into::into).collect();
        self
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

    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
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
    Event {
        relay_url: String,
        raw_json: String,
        observed_at_ms: i64,
    },
    Eose {
        relay_url: String,
    },
    Closed {
        relay_url: String,
        message: String,
    },
    Notice {
        relay_url: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayFetchOutcomeKind {
    Eose,
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
    pub unsupported: bool,
    pub malformed: bool,
    pub out_of_filter: bool,
    pub skipped_over_limit: bool,
    pub projection_eligible: bool,
    pub verification_status: Option<String>,
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
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub eose_count: usize,
    pub closed_count: usize,
    pub notice_count: usize,
    pub relay_outcomes: Vec<RadrootsRelayFetchRelayOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchReceipt {
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub malformed_count: usize,
    pub out_of_filter_count: usize,
    pub skipped_over_limit_count: usize,
    pub unsupported_count: usize,
    pub eose_count: usize,
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
    let target_relays = request.relay_urls.clone();
    let max_events = request.max_events;
    let max_raw_events = request.max_raw_events;
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    Ok(
        process_relay_fetch_items(target_relays, filters, max_events, max_raw_events, items)
            .into_fetched_events_receipt(),
    )
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
    let target_relays = request.relay_urls.clone();
    let max_events = request.max_events;
    let max_raw_events = request.max_raw_events;
    let filters = request.filters.as_slice().to_vec();
    let items = adapter.fetch(request).await?;
    let processed =
        process_relay_fetch_items(target_relays, filters, max_events, max_raw_events, items);
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
            }) => {
                let event = radroots_event_from_nostr(&raw_event);
                let observation_type = match mode {
                    RadrootsRelayFetchMode::Fetch => RadrootsTransportObservationType::NostrFetch,
                    RadrootsRelayFetchMode::Subscription => {
                        RadrootsTransportObservationType::NostrSubscription
                    }
                };
                let observation = RadrootsTransportObservation::new(
                    RadrootsTransportKind::Nostr,
                    relay_url.clone(),
                    observation_type,
                    observed_at_ms,
                )?;
                let ingest = RadrootsEventIngest::new(event, observed_at_ms)
                    .with_raw_json(raw_json)
                    .with_observation(observation);
                match event_store.ingest_event(ingest).await {
                    Ok(store_receipt) => {
                        let unsupported =
                            store_receipt.contract_status != RadrootsEventContractStatus::Supported;
                        if store_receipt.inserted {
                            receipt.inserted_count += 1;
                        } else {
                            receipt.duplicate_count += 1;
                        }
                        if unsupported {
                            receipt.unsupported_count += 1;
                        }
                        receipt.events.push(RadrootsRelayFetchEventReceipt {
                            relay_url,
                            event_id: Some(store_receipt.event_id),
                            inserted: store_receipt.inserted,
                            duplicate: !store_receipt.inserted,
                            unsupported,
                            malformed: false,
                            out_of_filter: false,
                            skipped_over_limit: false,
                            projection_eligible: store_receipt.projection_eligible,
                            verification_status: Some(
                                store_receipt.verification_status.as_str().to_owned(),
                            ),
                            message: None,
                        });
                    }
                    Err(error) => {
                        receipt.malformed_count += 1;
                        receipt.events.push(RadrootsRelayFetchEventReceipt {
                            relay_url,
                            event_id: Some(raw_event.id.to_hex()),
                            inserted: false,
                            duplicate: false,
                            unsupported: false,
                            malformed: true,
                            out_of_filter: false,
                            skipped_over_limit: false,
                            projection_eligible: false,
                            verification_status: None,
                            message: Some(error.to_string()),
                        });
                    }
                }
            }
        }
    }
    Ok(receipt)
}

#[derive(Clone, Debug)]
enum RadrootsRelayProcessedFetchItem {
    Accepted(RadrootsRelayFetchedEvent),
    Receipt(RadrootsRelayFetchEventReceipt),
}

#[derive(Clone, Debug)]
struct RadrootsRelayProcessedFetch {
    target_relays: Vec<String>,
    items: Vec<RadrootsRelayProcessedFetchItem>,
    malformed_count: usize,
    out_of_filter_count: usize,
    skipped_over_limit_count: usize,
    eose_count: usize,
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
            malformed_count: self.malformed_count,
            out_of_filter_count: self.out_of_filter_count,
            skipped_over_limit_count: self.skipped_over_limit_count,
            eose_count: self.eose_count,
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
            malformed_count: processed.malformed_count,
            out_of_filter_count: processed.out_of_filter_count,
            skipped_over_limit_count: processed.skipped_over_limit_count,
            unsupported_count: 0,
            eose_count: processed.eose_count,
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
    max_events: usize,
    max_raw_events: usize,
    items: Vec<RadrootsRelayFetchItem>,
) -> RadrootsRelayProcessedFetch {
    let mut processed = RadrootsRelayProcessedFetch {
        target_relays,
        items: Vec::new(),
        malformed_count: 0,
        out_of_filter_count: 0,
        skipped_over_limit_count: 0,
        eose_count: 0,
        closed_count: 0,
        notice_count: 0,
        relay_outcomes: Vec::new(),
    };
    let mut scanned_raw_events = 0usize;
    let mut accepted_events = 0usize;
    for item in items {
        match item {
            RadrootsRelayFetchItem::Event {
                relay_url,
                raw_json,
                observed_at_ms,
            } => {
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
                                unsupported: false,
                                malformed: true,
                                out_of_filter: false,
                                skipped_over_limit: false,
                                projection_eligible: false,
                                verification_status: None,
                                message: Some("event JSON parse failed".to_owned()),
                            },
                        ));
                    continue;
                };
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
                                unsupported: false,
                                malformed: false,
                                out_of_filter: true,
                                skipped_over_limit: false,
                                projection_eligible: false,
                                verification_status: None,
                                message: Some("event did not match relay fetch filters".to_owned()),
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
                                unsupported: false,
                                malformed: false,
                                out_of_filter: false,
                                skipped_over_limit: true,
                                projection_eligible: false,
                                verification_status: None,
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
            RadrootsRelayFetchItem::Eose { relay_url } => {
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
            RadrootsRelayFetchItem::Closed { relay_url, message } => {
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
            RadrootsRelayFetchItem::Notice { relay_url, message } => {
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
    processed
}

fn accepted_fetch_event_receipt(
    event: &RadrootsRelayFetchedEvent,
) -> RadrootsRelayFetchEventReceipt {
    RadrootsRelayFetchEventReceipt {
        relay_url: event.relay_url.clone(),
        event_id: Some(event.event.id.to_hex()),
        inserted: false,
        duplicate: false,
        unsupported: false,
        malformed: false,
        out_of_filter: false,
        skipped_over_limit: false,
        projection_eligible: false,
        verification_status: None,
        message: Some("event accepted by relay fetch filters".to_owned()),
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
    if request.relay_urls.is_empty() {
        return Err(RadrootsRelayTransportError::EmptyTargetSet);
    }
    if request.filters.as_slice().is_empty() {
        return Err(RadrootsRelayTransportError::EmptyFetchFilters);
    }
    let timeout = Duration::from_millis(request.timeout_ms);
    let filters = request.filters.as_slice().to_vec();
    let observed_at_ms = request.observed_at_ms;
    let mut items = Vec::new();
    for relay_url in request.relay_urls {
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
        for filter in filters.iter().cloned() {
            match client.fetch_events(filter, timeout).await {
                Ok(events) => {
                    for event in events {
                        items.push(RadrootsRelayFetchItem::Event {
                            relay_url: relay_url.clone(),
                            raw_json: event.as_json(),
                            observed_at_ms,
                        });
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
        if !closed {
            items.push(RadrootsRelayFetchItem::Eose { relay_url });
        }
    }
    Ok(items)
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
        RadrootsNostrEvent, relay_fetch_event_matches_filters, summarize_nostr_output_failures,
    };
    use nostr::JsonUtil;
    use radroots_nostr::prelude::{
        RadrootsNostrFilter, RadrootsNostrKeys, RadrootsNostrKind, RadrootsNostrSecretKey,
        radroots_nostr_build_event,
    };
    use std::collections::HashMap;

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";

    fn signed_raw_event() -> RadrootsNostrEvent {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let event = radroots_nostr_build_event(1, "hello", Vec::new())
            .expect("event builder")
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
}
