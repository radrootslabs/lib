#![forbid(unsafe_code)]

use crate::{RadrootsRelayOutcome, RadrootsRelayTransportError};
use core::time::Duration;
use futures::future::BoxFuture;
use nostr::JsonUtil;
use radroots_event_store::{
    RadrootsEventContractStatus, RadrootsEventIngest, RadrootsEventStore, RadrootsRelayObservation,
    RadrootsRelayObservationType,
};
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrEvent, RadrootsNostrFilter, radroots_event_from_nostr,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, PoisonError};

const DEFAULT_RELAY_FETCH_TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadrootsRelayFetchMode {
    Fetch,
    Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayFetchRequest {
    pub mode: RadrootsRelayFetchMode,
    pub observed_at_ms: i64,
    pub max_events: usize,
    pub relay_urls: Vec<String>,
    pub filters: Vec<RadrootsNostrFilter>,
    pub timeout_ms: u64,
}

impl RadrootsRelayFetchRequest {
    pub fn fetch(observed_at_ms: i64, max_events: usize) -> Self {
        Self {
            mode: RadrootsRelayFetchMode::Fetch,
            observed_at_ms,
            max_events,
            relay_urls: Vec::new(),
            filters: Vec::new(),
            timeout_ms: DEFAULT_RELAY_FETCH_TIMEOUT_MS,
        }
    }

    pub fn subscription(observed_at_ms: i64, max_events: usize) -> Self {
        Self {
            mode: RadrootsRelayFetchMode::Subscription,
            observed_at_ms,
            max_events,
            relay_urls: Vec::new(),
            filters: Vec::new(),
            timeout_ms: DEFAULT_RELAY_FETCH_TIMEOUT_MS,
        }
    }

    pub fn with_relay_urls<I, S>(mut self, relay_urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.relay_urls = relay_urls.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_filters<I>(mut self, filters: I) -> Self
    where
        I: IntoIterator<Item = RadrootsNostrFilter>,
    {
        self.filters = filters.into_iter().collect();
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
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
    pub projection_eligible: bool,
    pub verification_status: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayFetchReceipt {
    pub inserted_count: usize,
    pub duplicate_count: usize,
    pub malformed_count: usize,
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

pub async fn fetch_and_ingest_relay_events<A>(
    adapter: &A,
    event_store: &RadrootsEventStore,
    request: RadrootsRelayFetchRequest,
) -> Result<RadrootsRelayFetchReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayFetchAdapter,
{
    let mode = request.mode;
    let max_events = request.max_events;
    let items = adapter.fetch(request).await?;
    let mut receipt = RadrootsRelayFetchReceipt {
        inserted_count: 0,
        duplicate_count: 0,
        malformed_count: 0,
        unsupported_count: 0,
        eose_count: 0,
        closed_count: 0,
        notice_count: 0,
        events: Vec::new(),
        relay_outcomes: Vec::new(),
    };
    let mut processed_events = 0usize;
    for item in items {
        match item {
            RadrootsRelayFetchItem::Event {
                relay_url,
                raw_json,
                observed_at_ms,
            } => {
                if processed_events >= max_events {
                    continue;
                }
                processed_events += 1;
                let parsed = RadrootsNostrEvent::from_json(raw_json.as_str());
                let Ok(raw_event) = parsed else {
                    receipt.malformed_count += 1;
                    receipt.events.push(RadrootsRelayFetchEventReceipt {
                        relay_url,
                        event_id: None,
                        inserted: false,
                        duplicate: false,
                        unsupported: false,
                        malformed: true,
                        projection_eligible: false,
                        verification_status: None,
                        message: Some("event JSON parse failed".to_owned()),
                    });
                    continue;
                };
                let event = radroots_event_from_nostr(&raw_event);
                let observation_type = match mode {
                    RadrootsRelayFetchMode::Fetch => RadrootsRelayObservationType::Fetch,
                    RadrootsRelayFetchMode::Subscription => {
                        RadrootsRelayObservationType::Subscription
                    }
                };
                let ingest = RadrootsEventIngest::new(event, observed_at_ms)
                    .with_raw_json(raw_json)
                    .with_observation(RadrootsRelayObservation::new(
                        relay_url.clone(),
                        observation_type,
                        observed_at_ms,
                    ));
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
                            projection_eligible: false,
                            verification_status: None,
                            message: Some(error.to_string()),
                        });
                    }
                }
            }
            RadrootsRelayFetchItem::Eose { relay_url } => {
                receipt.eose_count += 1;
                receipt.relay_outcomes.push(RadrootsRelayFetchRelayOutcome {
                    relay_url,
                    kind: RadrootsRelayFetchOutcomeKind::Eose,
                    relay_outcome: None,
                    message: None,
                });
            }
            RadrootsRelayFetchItem::Closed { relay_url, message } => {
                receipt.closed_count += 1;
                receipt.relay_outcomes.push(RadrootsRelayFetchRelayOutcome {
                    relay_url,
                    kind: RadrootsRelayFetchOutcomeKind::Closed,
                    relay_outcome: Some(RadrootsRelayOutcome::classify(message.as_str())),
                    message: Some(message),
                });
            }
            RadrootsRelayFetchItem::Notice { relay_url, message } => {
                receipt.notice_count += 1;
                receipt.relay_outcomes.push(RadrootsRelayFetchRelayOutcome {
                    relay_url,
                    kind: RadrootsRelayFetchOutcomeKind::Notice,
                    relay_outcome: None,
                    message: Some(message),
                });
            }
        }
    }
    Ok(receipt)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RadrootsNostrClientFetchAdapter;

impl RadrootsRelayFetchAdapter for RadrootsNostrClientFetchAdapter {
    fn fetch<'a>(
        &'a self,
        request: RadrootsRelayFetchRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError>> {
        Box::pin(async move { fetch_from_nostr_relays(request).await })
    }
}

async fn fetch_from_nostr_relays(
    request: RadrootsRelayFetchRequest,
) -> Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError> {
    if request.relay_urls.is_empty() {
        return Err(RadrootsRelayTransportError::EmptyTargetSet);
    }
    if request.filters.is_empty() {
        return Err(RadrootsRelayTransportError::Transport(
            "relay fetch filters must not be empty".to_owned(),
        ));
    }
    let timeout = Duration::from_millis(request.timeout_ms);
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
        client.connect().await;
        let mut closed = false;
        for filter in request.filters.iter().cloned() {
            match client.fetch_events(filter, timeout).await {
                Ok(events) => {
                    for event in events {
                        items.push(RadrootsRelayFetchItem::Event {
                            relay_url: relay_url.clone(),
                            raw_json: event.as_json(),
                            observed_at_ms: request.observed_at_ms,
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
