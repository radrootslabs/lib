#![forbid(unsafe_code)]

use crate::{
    RadrootsRelayOutcome, RadrootsRelayOutcomeKind, RadrootsRelayTargetSet,
    RadrootsRelayTransportError, RadrootsRelayUrlPolicy,
};
use futures::future::BoxFuture;
use radroots_events::draft::RadrootsSignedNostrEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[cfg(feature = "client")]
use nostr::JsonUtil;
#[cfg(feature = "client")]
use radroots_nostr::prelude::{RadrootsNostrClient, RadrootsNostrEvent};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsRelayPublishRequest {
    pub signed_event: RadrootsSignedNostrEvent,
    pub targets: RadrootsRelayTargetSet,
    pub accepted_quorum: usize,
    pub now_ms: i64,
}

impl RadrootsRelayPublishRequest {
    pub fn new(
        signed_event: RadrootsSignedNostrEvent,
        targets: RadrootsRelayTargetSet,
        now_ms: i64,
    ) -> Self {
        let accepted_quorum = targets.len();
        Self {
            signed_event,
            targets,
            accepted_quorum,
            now_ms,
        }
    }

    pub fn with_accepted_quorum(mut self, accepted_quorum: usize) -> Self {
        self.accepted_quorum = accepted_quorum;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayPublishRelayReceipt {
    pub relay_url: String,
    pub outcome: RadrootsRelayOutcome,
    pub attempted: bool,
}

impl RadrootsRelayPublishRelayReceipt {
    pub fn attempted(relay_url: impl Into<String>, outcome: RadrootsRelayOutcome) -> Self {
        Self {
            relay_url: relay_url.into(),
            outcome,
            attempted: true,
        }
    }

    pub fn skipped(relay_url: impl Into<String>, outcome: RadrootsRelayOutcome) -> Self {
        Self {
            relay_url: relay_url.into(),
            outcome,
            attempted: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadrootsRelayPublishReceipt {
    pub event_id: String,
    pub attempted_count: usize,
    pub accepted_count: usize,
    pub retryable_count: usize,
    pub terminal_count: usize,
    pub quorum: usize,
    pub quorum_met: bool,
    pub relays: Vec<RadrootsRelayPublishRelayReceipt>,
}

pub trait RadrootsRelayPublishAdapter: Send + Sync {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>;
}

pub async fn publish_signed_event<A>(
    adapter: &A,
    request: RadrootsRelayPublishRequest,
) -> Result<RadrootsRelayPublishReceipt, RadrootsRelayTransportError>
where
    A: RadrootsRelayPublishAdapter,
{
    let event_id = request.signed_event.id.clone();
    let quorum = request.accepted_quorum;
    let relays = adapter.publish(request).await?;
    let attempted_count = relays.iter().filter(|receipt| receipt.attempted).count();
    let accepted_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.counts_toward_quorum())
        .count();
    let retryable_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.is_retryable())
        .count();
    let terminal_count = relays
        .iter()
        .filter(|receipt| receipt.outcome.is_terminal_failure())
        .count();
    Ok(RadrootsRelayPublishReceipt {
        event_id,
        attempted_count,
        accepted_count,
        retryable_count,
        terminal_count,
        quorum,
        quorum_met: accepted_count >= quorum,
        relays,
    })
}

#[derive(Clone, Default)]
pub struct RadrootsMockRelayPublishAdapter {
    outcomes: BTreeMap<String, RadrootsRelayOutcome>,
    captured_raw_events: Arc<Mutex<Vec<String>>>,
}

impl RadrootsMockRelayPublishAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_outcome(
        mut self,
        relay_url: impl Into<String>,
        outcome: RadrootsRelayOutcome,
    ) -> Self {
        self.outcomes.insert(relay_url.into(), outcome);
        self
    }

    pub fn captured_raw_events(&self) -> Vec<String> {
        self.captured_raw_events
            .lock()
            .expect("captured raw event lock")
            .clone()
    }
}

impl RadrootsRelayPublishAdapter for RadrootsMockRelayPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            self.captured_raw_events
                .lock()
                .map_err(|_| {
                    RadrootsRelayTransportError::Transport(
                        "captured raw event lock poisoned".to_owned(),
                    )
                })?
                .push(request.signed_event.raw_json.clone());
            Ok(request
                .targets
                .relays()
                .iter()
                .map(|relay| {
                    let outcome = self
                        .outcomes
                        .get(relay.as_str())
                        .cloned()
                        .unwrap_or_else(RadrootsRelayOutcome::accepted);
                    RadrootsRelayPublishRelayReceipt::attempted(relay.as_str(), outcome)
                })
                .collect())
        })
    }
}

#[cfg(feature = "client")]
#[derive(Clone)]
pub struct RadrootsNostrClientPublishAdapter {
    client: RadrootsNostrClient,
}

#[cfg(feature = "client")]
impl RadrootsNostrClientPublishAdapter {
    pub fn new(client: RadrootsNostrClient) -> Self {
        Self { client }
    }
}

#[cfg(feature = "client")]
impl RadrootsRelayPublishAdapter for RadrootsNostrClientPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            let event = RadrootsNostrEvent::from_json(request.signed_event.raw_json.as_str())
                .map_err(|error| RadrootsRelayTransportError::NostrEventJson(error.to_string()))?;
            if event.id.to_hex() != request.signed_event.id {
                return Err(RadrootsRelayTransportError::NostrEventJson(
                    "raw event JSON ID does not match signed event ID".to_owned(),
                ));
            }
            let target_strings = request.targets.relay_strings();
            for relay_url in &target_strings {
                self.client
                    .add_write_relay(relay_url.as_str())
                    .await
                    .map_err(|error| RadrootsRelayTransportError::Transport(error.to_string()))?;
            }
            let output = match self
                .client
                .send_event_to(target_strings.clone(), &event)
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    let message = error.to_string();
                    return Ok(target_strings
                        .into_iter()
                        .map(|relay_url| {
                            RadrootsRelayPublishRelayReceipt::attempted(
                                relay_url,
                                RadrootsRelayOutcome::connection_failed(message.clone()),
                            )
                        })
                        .collect());
                }
            };
            let mut receipts = Vec::new();
            for relay_url in &target_strings {
                let relay =
                    crate::RadrootsRelayUrl::parse(relay_url, RadrootsRelayUrlPolicy::LocalDev)?;
                let success = output.success.iter().any(|success_url| {
                    success_url.to_string().trim_end_matches('/') == relay.as_str()
                });
                if success {
                    receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                        relay_url,
                        RadrootsRelayOutcome {
                            kind: RadrootsRelayOutcomeKind::Accepted,
                            message: Some(
                                "nostr-relay-pool-success-ok-message-unavailable".to_owned(),
                            ),
                        },
                    ));
                    continue;
                }
                let failed = output.failed.iter().find_map(|(failed_url, message)| {
                    if failed_url.to_string().trim_end_matches('/') == relay.as_str() {
                        Some(message.clone())
                    } else {
                        None
                    }
                });
                let outcome = failed
                    .map(RadrootsRelayOutcome::classify)
                    .unwrap_or_else(|| {
                        RadrootsRelayOutcome::classify("error: relay output omitted target")
                    });
                receipts.push(RadrootsRelayPublishRelayReceipt::attempted(
                    relay_url, outcome,
                ));
            }
            Ok(receipts)
        })
    }
}
