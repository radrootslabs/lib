//! Nostr implementation of the transport event sink.

use crate::{NostrTransport, RelayUrl};
use core::time::Duration;
use radroots_nostr::event::Event;
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, EventSink,
    capability::{Availability, Maturity, SinkCapabilities},
    outcome::{DeliveryOutcome, Retryability},
    sink::{DeliveryTargetReceipt, SinkStatus},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct RelayPublishResult {
    relay: RelayUrl,
    outcome: DeliveryOutcome,
}

pub(crate) trait RelayClient: Send + Sync {
    fn publish<'a>(
        &'a self,
        relays: Vec<RelayUrl>,
        event: Event,
        connect_timeout: Duration,
    ) -> BoxFuture<'a, Vec<RelayPublishResult>>;
}

#[derive(Clone, Debug)]
pub(crate) struct LiveRelayClient {
    client: nostr_sdk::Client,
}

impl LiveRelayClient {
    pub(crate) const fn new(client: nostr_sdk::Client) -> Self {
        Self { client }
    }

    #[cfg(test)]
    pub(crate) fn isolated() -> Self {
        let client = nostr_sdk::Client::default();
        client.automatic_authentication(false);
        Self::new(client)
    }
}

impl RelayClient for LiveRelayClient {
    fn publish<'a>(
        &'a self,
        relays: Vec<RelayUrl>,
        event: Event,
        connect_timeout: Duration,
    ) -> BoxFuture<'a, Vec<RelayPublishResult>> {
        Box::pin(async move {
            let mut results = Vec::with_capacity(relays.len());
            for relay in relays {
                let url = relay.as_str().to_owned();
                let outcome = match self.client.add_relay(url.as_str()).await {
                    Err(error) => connection_failure(error.to_string()),
                    Ok(_) => match self
                        .client
                        .try_connect_relay(url.as_str(), connect_timeout)
                        .await
                    {
                        Err(error) => connection_failure(error.to_string()),
                        Ok(()) => match self.client.send_event_to([url.as_str()], &event).await {
                            Err(error) => classify_failure(error.to_string()),
                            Ok(output) => output
                                .success
                                .iter()
                                .any(|accepted| accepted.to_string().trim_end_matches('/') == url)
                                .then(DeliveryOutcome::accepted)
                                .or_else(|| {
                                    output.failed.iter().find_map(|(failed, message)| {
                                        (failed.to_string().trim_end_matches('/') == url)
                                            .then(|| classify_failure(message.clone()))
                                    })
                                })
                                .unwrap_or_else(|| classify_failure("relay omitted result".into())),
                        },
                    },
                };
                results.push(RelayPublishResult { relay, outcome });
            }
            results
        })
    }
}

impl EventSink for NostrTransport {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, radroots_transport::Error>> {
        let configured = !self.config().relays().is_empty();
        Box::pin(async move {
            Ok(SinkStatus::new(
                radroots_transport::TransportId::NOSTR,
                configured,
                Maturity::Preview,
                Availability::Available,
                SinkCapabilities::DELIVER,
                "bounded Nostr event delivery configured",
            ))
        })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::Error>> {
        Box::pin(async move {
            let mut requested = Vec::new();
            let mut skipped = Vec::new();
            for target in request.target_set().targets() {
                match RelayUrl::from_target(target, self.config().relay_url_policy()) {
                    Ok(relay) if self.config().relays().contains(&relay) => {
                        requested.push((relay, target.clone()));
                    }
                    _ => skipped.push(DeliveryTargetReceipt::skipped(
                        target.clone(),
                        DeliveryOutcome::rejected().with_detail(
                            "target_denied",
                            "target is not configured for this sink",
                        )?,
                    )?),
                }
            }

            let event = match radroots_nostr::event::to_nostr(request.payload().event().envelope())
            {
                Ok(event) => event,
                Err(_) => return Err(radroots_transport::Error::InvalidDeliveryOutcome),
            };
            let expected: BTreeSet<_> = requested.iter().map(|(relay, _)| relay.clone()).collect();
            let results = self
                .client
                .publish(
                    requested.iter().map(|(relay, _)| relay.clone()).collect(),
                    event,
                    Duration::from_millis(self.config().connect_timeout_ms()),
                )
                .await;
            let mut by_relay = BTreeMap::new();
            for result in results {
                if expected.contains(&result.relay)
                    && by_relay.insert(result.relay, result.outcome).is_none()
                {
                    continue;
                }
                return Err(radroots_transport::Error::InvalidDeliveryOutcome);
            }

            let mut receipts = skipped;
            for (relay, target) in requested {
                let outcome = by_relay.remove(&relay).unwrap_or_else(|| {
                    DeliveryOutcome::unavailable()
                        .with_detail("missing_result", "relay returned no result")
                        .expect("static normalized outcome")
                });
                receipts.push(DeliveryTargetReceipt::attempted(target, outcome));
            }
            DeliveryReceipt::for_request(&request, receipts)
        })
    }
}

fn connection_failure(message: String) -> DeliveryOutcome {
    normalized(DeliveryOutcome::unavailable(), "connection_failed", message)
}

fn classify_failure(message: String) -> DeliveryOutcome {
    let normalized_message = message.to_ascii_lowercase();
    if normalized_message.contains("duplicate") || normalized_message.contains("already have") {
        normalized(DeliveryOutcome::accepted(), "duplicate", message)
    } else if normalized_message.contains("auth") {
        normalized(
            DeliveryOutcome::failed(Retryability::Retryable)
                .expect("retryable failure classification"),
            "auth_required",
            message,
        )
    } else if normalized_message.contains("blocked")
        || normalized_message.contains("restricted")
        || normalized_message.contains("invalid")
    {
        normalized(DeliveryOutcome::rejected(), "rejected", message)
    } else if normalized_message.contains("rate") {
        normalized(DeliveryOutcome::unavailable(), "rate_limited", message)
    } else if normalized_message.contains("timeout") {
        normalized(DeliveryOutcome::unavailable(), "timeout", message)
    } else {
        normalized(DeliveryOutcome::unavailable(), "relay_failure", message)
    }
}

fn normalized(outcome: DeliveryOutcome, code: &'static str, message: String) -> DeliveryOutcome {
    let clean = message
        .chars()
        .filter(|character| !character.is_control())
        .take(1_024)
        .collect::<String>();
    let clean = if clean.trim().is_empty() {
        "relay operation failed".to_owned()
    } else {
        clean.trim().to_owned()
    };
    outcome
        .with_detail(code, clean)
        .expect("normalized static relay outcome")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, RelayUrlPolicy};
    use radroots_transport::{
        Target, TargetSet,
        outcome::DeliveryOutcomeKind,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        sink::DeliveryPayload,
    };
    use std::sync::Arc;

    #[derive(Debug)]
    struct MockRelayClient {
        outcomes: BTreeMap<RelayUrl, DeliveryOutcome>,
    }

    impl RelayClient for MockRelayClient {
        fn publish<'a>(
            &'a self,
            relays: Vec<RelayUrl>,
            _event: Event,
            _connect_timeout: Duration,
        ) -> BoxFuture<'a, Vec<RelayPublishResult>> {
            Box::pin(async move {
                relays
                    .into_iter()
                    .map(|relay| RelayPublishResult {
                        outcome: self
                            .outcomes
                            .get(&relay)
                            .cloned()
                            .unwrap_or_else(DeliveryOutcome::accepted),
                        relay,
                    })
                    .collect()
            })
        }
    }

    fn payload() -> DeliveryPayload {
        let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
        DeliveryPayload::new(radroots_event_codec::decode::signed_event(raw).expect("signed event"))
    }

    fn request() -> DeliveryRequest {
        DeliveryRequest::new(
            "nostr-delivery",
            payload(),
            TargetSet::new(vec![
                Target::nostr_relay("wss://one.example").expect("one"),
                Target::nostr_relay("wss://two.example").expect("two"),
            ])
            .expect("targets"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_800_000_000_000,
        )
        .expect("request")
    }

    #[test]
    fn sink_returns_normalized_per_relay_partial_success() {
        let config = Config::new(
            RelayUrlPolicy::Public,
            ["wss://one.example", "wss://two.example"],
        )
        .expect("config");
        let two = RelayUrl::parse("wss://two.example", RelayUrlPolicy::Public).expect("two");
        let client = MockRelayClient {
            outcomes: BTreeMap::from([(two, classify_failure("rate limited".to_owned()))]),
        };
        let transport = NostrTransport::with_client(config, Arc::new(client));
        let request = request();
        let receipt = futures::executor::block_on(transport.deliver(request.clone()))
            .expect("delivery receipt");

        assert_eq!(receipt.target_receipts().len(), 2);
        assert_eq!(
            receipt.target_receipts()[0].outcome().kind(),
            DeliveryOutcomeKind::Accepted
        );
        assert_eq!(
            receipt.target_receipts()[1].outcome().kind(),
            DeliveryOutcomeKind::Unavailable
        );
        assert!(!receipt.is_satisfied(&request).expect("satisfaction"));
    }

    #[test]
    fn upstream_messages_map_to_stable_outcomes() {
        let cases = [
            ("duplicate: already have", DeliveryOutcomeKind::Accepted),
            ("blocked by policy", DeliveryOutcomeKind::Rejected),
            ("rate limited", DeliveryOutcomeKind::Unavailable),
            ("auth required", DeliveryOutcomeKind::Failed),
            ("connection timeout", DeliveryOutcomeKind::Unavailable),
            ("connection failed", DeliveryOutcomeKind::Unavailable),
        ];
        for (message, expected) in cases {
            assert_eq!(classify_failure(message.to_owned()).kind(), expected);
        }
    }
}
