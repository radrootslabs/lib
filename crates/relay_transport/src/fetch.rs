#![forbid(unsafe_code)]

use crate::RadrootsRelayTransportError;
use futures::future::BoxFuture;
use nostr::JsonUtil;
use radroots_event_store::{
    RadrootsEventContractStatus, RadrootsEventIngest, RadrootsEventStore, RadrootsRelayObservation,
    RadrootsRelayObservationType,
};
use radroots_nostr::prelude::{RadrootsNostrEvent, radroots_event_from_nostr};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
}

impl RadrootsRelayFetchRequest {
    pub fn fetch(observed_at_ms: i64, max_events: usize) -> Self {
        Self {
            mode: RadrootsRelayFetchMode::Fetch,
            observed_at_ms,
            max_events,
        }
    }

    pub fn subscription(observed_at_ms: i64, max_events: usize) -> Self {
        Self {
            mode: RadrootsRelayFetchMode::Subscription,
            observed_at_ms,
            max_events,
        }
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
pub struct RadrootsRelayFetchEventReceipt {
    pub relay_url: String,
    pub event_id: Option<String>,
    pub inserted: bool,
    pub duplicate: bool,
    pub unsupported: bool,
    pub malformed: bool,
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
                    break;
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
                let ingest = RadrootsEventIngest::verified(event, observed_at_ms)
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
                            message: Some(error.to_string()),
                        });
                    }
                }
            }
            RadrootsRelayFetchItem::Eose { .. } => {
                receipt.eose_count += 1;
            }
            RadrootsRelayFetchItem::Closed { .. } => {
                receipt.closed_count += 1;
            }
            RadrootsRelayFetchItem::Notice { .. } => {
                receipt.notice_count += 1;
            }
        }
    }
    Ok(receipt)
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
        Box::pin(async move {
            Ok(self
                .items
                .lock()
                .map_err(|_| {
                    RadrootsRelayTransportError::Transport("fetch item lock poisoned".to_owned())
                })?
                .clone())
        })
    }
}
