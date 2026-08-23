//! Nostr implementation of the transport event sink.

use crate::{NostrTransport, RelayUrl, status};
use core::{fmt, time::Duration};
use futures::{StreamExt, stream};
use radroots_nostr::event::Event;
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, EventSink, SinkFailure, Target,
    outcome::DeliveryOutcome,
    sink::{DeliveryTargetReceipt, SinkStatus},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct RelayPublishResult {
    relay: RelayUrl,
    outcome: DeliveryOutcome,
}

/// Sealed, no-I/O result of validating one delivery against this adapter.
///
/// The value retains the exact request and converted signed event. It is
/// constructed only by [`NostrTransport::prepare_delivery`] and is consumed by
/// [`NostrTransport::execute_prepared_delivery`]. Ordinary `Debug` never
/// exposes event bytes, request identities, or relay destinations.
#[must_use = "prepared delivery must be durably bound before execution or deliberately discarded"]
pub struct PreparedDelivery {
    request: DeliveryRequest,
    config: crate::Config,
    event: Event,
    authorized: Vec<(RelayUrl, Target)>,
    skipped: Vec<DeliveryTargetReceipt>,
}

impl PreparedDelivery {
    /// Returns the exact validated request retained for persistence binding.
    #[must_use]
    pub const fn request(&self) -> &DeliveryRequest {
        &self.request
    }
}

impl fmt::Debug for PreparedDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PreparedDelivery([redacted])")
    }
}

pub(crate) trait RelayClient: Send + Sync {
    fn publish<'a>(
        &'a self,
        relays: Vec<RelayUrl>,
        event: Event,
        max_connections: usize,
        connect_timeout: Duration,
        operation_timeout: Duration,
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
    // The live SDK loop requires external relays. Its result normalization is
    // covered through the injected RelayClient boundary below.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn publish<'a>(
        &'a self,
        relays: Vec<RelayUrl>,
        event: Event,
        max_connections: usize,
        connect_timeout: Duration,
        operation_timeout: Duration,
    ) -> BoxFuture<'a, Vec<RelayPublishResult>> {
        Box::pin(async move {
            stream::iter(relays.into_iter().map(|relay| {
                let event = event.clone();
                async move {
                    let url = relay.as_str().to_owned();
                    let attempt = async {
                        self.client.add_relay(url.as_str()).await?;
                        self.client
                            .try_connect_relay(url.as_str(), connect_timeout)
                            .await?;
                        self.client.send_event_to([url.as_str()], &event).await
                    };
                    let outcome = match tokio::time::timeout(operation_timeout, attempt).await {
                        Err(_) => status::delivery_failure("timeout"),
                        Ok(Err(error)) => status::delivery_failure(error.to_string().as_str()),
                        Ok(Ok(output)) => output
                            .success
                            .iter()
                            .any(|accepted| accepted.to_string().trim_end_matches('/') == url)
                            .then(DeliveryOutcome::accepted)
                            .or_else(|| {
                                output.failed.iter().find_map(|(failed, message)| {
                                    (failed.to_string().trim_end_matches('/') == url)
                                        .then(|| status::delivery_failure(message.as_str()))
                                })
                            })
                            .unwrap_or_else(|| status::delivery_failure("relay omitted result")),
                    };
                    RelayPublishResult { relay, outcome }
                }
            }))
            .buffered(max_connections)
            .collect()
            .await
        })
    }
}

impl NostrTransport {
    /// Validates and converts one delivery without reading a clock or performing relay I/O.
    pub fn prepare_delivery(
        &self,
        request: DeliveryRequest,
    ) -> Result<PreparedDelivery, Box<SinkFailure>> {
        let mut authorized = Vec::new();
        let mut skipped = Vec::new();
        for target in request.target_set().targets() {
            match self.config().endpoint_for_target(target) {
                Some(endpoint) if endpoint.access().can_write() => {
                    authorized.push((endpoint.url().clone(), target.clone()));
                }
                None | Some(_) => skipped.push(
                    DeliveryTargetReceipt::skipped(
                        target.clone(),
                        DeliveryOutcome::rejected()
                            .with_detail("target_denied", "target is not configured for this sink")
                            .map_err(|_| Box::new(SinkFailure::invalid_contract(&request)))?,
                    )
                    .map_err(|_| Box::new(SinkFailure::invalid_contract(&request)))?,
                ),
            }
        }
        let event = radroots_nostr::event::to_nostr(request.payload().event().envelope())
            .map_err(|_| Box::new(SinkFailure::invalid_contract(&request)))?;
        Ok(PreparedDelivery {
            request,
            config: self.config().clone(),
            event,
            authorized,
            skipped,
        })
    }

    /// Performs relay I/O for one exact prepared delivery and consumes its authority.
    pub fn execute_prepared_delivery(
        &self,
        prepared: PreparedDelivery,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async move {
            let PreparedDelivery {
                request,
                config,
                event,
                authorized,
                mut skipped,
            } = prepared;
            if config != *self.config() {
                return Err(SinkFailure::invalid_contract(&request));
            }
            let now_unix_ms = unix_time_ms();
            let mut requested = Vec::new();
            for (relay, target) in authorized {
                if self.status.may_write(&relay, now_unix_ms) {
                    requested.push((relay, target));
                } else {
                    skipped.push(
                        DeliveryTargetReceipt::skipped(
                            target,
                            DeliveryOutcome::unavailable()
                                .with_detail(
                                    "reconnect_backoff",
                                    "relay reconnect backoff is active",
                                )
                                .map_err(|_| SinkFailure::invalid_contract(&request))?,
                        )
                        .map_err(|_| SinkFailure::invalid_contract(&request))?,
                    );
                }
            }
            let remaining_ms = request.deadline_unix_ms().saturating_sub(now_unix_ms);
            let operation_timeout_ms = remaining_ms.min(self.config().request_timeout_ms());
            if operation_timeout_ms == 0 {
                for (relay, _) in &requested {
                    self.status.record_write(relay, false, true, now_unix_ms);
                }
                let timeout = status::delivery_failure("timeout");
                skipped.extend(requested.into_iter().map(|(_, target)| {
                    DeliveryTargetReceipt::skipped(target, timeout.clone())
                        .expect("normalized timeout cannot satisfy delivery")
                }));
                return DeliveryReceipt::for_request(&request, skipped)
                    .map_err(|_| SinkFailure::invalid_contract(&request));
            }
            let expected: BTreeSet<_> = requested.iter().map(|(relay, _)| relay.clone()).collect();
            for (relay, _) in &requested {
                self.status.begin_write(relay, now_unix_ms);
            }
            let results = self
                .client
                .publish(
                    requested.iter().map(|(relay, _)| relay.clone()).collect(),
                    event,
                    self.config().max_connections(),
                    Duration::from_millis(self.config().connect_timeout_ms()),
                    Duration::from_millis(operation_timeout_ms),
                )
                .await;
            let mut by_relay = BTreeMap::new();
            let observed_at_unix_ms = unix_time_ms().max(now_unix_ms);
            for result in results {
                if !expected.contains(&result.relay) || by_relay.contains_key(&result.relay) {
                    return Err(SinkFailure::invalid_contract(&request));
                }
                let succeeded = status::delivery_succeeded(&result.outcome);
                self.status.record_write(
                    &result.relay,
                    succeeded,
                    result.outcome.is_retryable(),
                    observed_at_unix_ms,
                );
                by_relay.insert(result.relay, result.outcome);
            }

            let mut receipts = skipped;
            for (relay, target) in requested {
                let outcome = by_relay.remove(&relay).unwrap_or_else(|| {
                    self.status
                        .record_write(&relay, false, true, observed_at_unix_ms);
                    DeliveryOutcome::unavailable()
                        .with_detail("missing_result", "relay returned no result")
                        .expect("static normalized outcome")
                });
                receipts.push(DeliveryTargetReceipt::attempted(target, outcome));
            }
            DeliveryReceipt::for_request(&request, receipts)
                .map_err(|_| SinkFailure::invalid_contract(&request))
        })
    }
}

impl EventSink for NostrTransport {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, radroots_transport::Error>> {
        Box::pin(async move { Ok(status::sink_status(&self.status)) })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        Box::pin(async move {
            let prepared = self.prepare_delivery(request).map_err(|failure| *failure)?;
            self.execute_prepared_delivery(prepared).await
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Debug)]
    struct MockRelayClient {
        outcomes: BTreeMap<RelayUrl, DeliveryOutcome>,
    }

    impl RelayClient for MockRelayClient {
        fn publish<'a>(
            &'a self,
            relays: Vec<RelayUrl>,
            _event: Event,
            _max_connections: usize,
            _connect_timeout: Duration,
            _operation_timeout: Duration,
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

    #[derive(Debug)]
    struct CountingRelayClient(Arc<AtomicUsize>);

    impl RelayClient for CountingRelayClient {
        fn publish<'a>(
            &'a self,
            _relays: Vec<RelayUrl>,
            _event: Event,
            _max_connections: usize,
            _connect_timeout: Duration,
            _operation_timeout: Duration,
        ) -> BoxFuture<'a, Vec<RelayPublishResult>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Vec::new() })
        }
    }

    fn payload() -> DeliveryPayload {
        let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
        DeliveryPayload::new(radroots_event_codec::decode::signed_event(raw).expect("signed event"))
    }

    fn request() -> DeliveryRequest {
        request_with_deadline(1_800_000_000_000)
    }

    fn request_with_deadline(deadline_unix_ms: u64) -> DeliveryRequest {
        DeliveryRequest::new(
            "nostr-delivery",
            payload(),
            TargetSet::new(vec![
                Target::nostr_relay("wss://one.example").expect("one"),
                Target::nostr_relay("wss://two.example").expect("two"),
            ])
            .expect("targets"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            deadline_unix_ms,
        )
        .expect("request")
    }

    #[test]
    fn sink_returns_normalized_per_relay_partial_success() {
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        let two = RelayUrl::parse("wss://two.example", RelayUrlPolicy::Public).expect("two");
        let client = MockRelayClient {
            outcomes: BTreeMap::from([(two, status::delivery_failure("rate limited"))]),
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
            assert_eq!(status::delivery_failure(message).kind(), expected);
        }
    }

    #[test]
    fn dropping_an_unpolled_delivery_performs_no_relay_work() {
        let calls = Arc::new(AtomicUsize::new(0));
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        let transport =
            NostrTransport::with_client(config, Arc::new(CountingRelayClient(Arc::clone(&calls))));
        let delivery = transport.deliver(request());
        drop(delivery);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn preparation_is_no_io_redacted_consuming_and_bound_to_exact_config() {
        let calls = Arc::new(AtomicUsize::new(0));
        let profile = crate::profile::test_profile(
            crate::RelayProfileKind::Public,
            RelayUrlPolicy::Public,
            ["wss://one.example", "wss://two.example"],
        )
        .expect("profile");
        let config = Config::from_profile(profile.clone());
        let transport =
            NostrTransport::with_client(config, Arc::new(CountingRelayClient(Arc::clone(&calls))));
        let request = request();

        let prepared = transport
            .prepare_delivery(request.clone())
            .expect("prepared delivery");
        assert_eq!(prepared.request(), &request);
        assert_eq!(format!("{prepared:?}"), "PreparedDelivery([redacted])");
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let receipt = futures::executor::block_on(transport.execute_prepared_delivery(prepared))
            .expect("executed delivery");
        assert_eq!(receipt.request_id(), request.request_id());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let mismatch_calls = Arc::new(AtomicUsize::new(0));
        let mismatched = NostrTransport::with_client(
            Config::from_profile(profile)
                .with_timeouts(5_000, 20_000, 2_000)
                .expect("different bounded config"),
            Arc::new(CountingRelayClient(Arc::clone(&mismatch_calls))),
        );
        let prepared = transport
            .prepare_delivery(request)
            .expect("second prepared delivery");
        assert_eq!(
            futures::executor::block_on(mismatched.execute_prepared_delivery(prepared))
                .expect_err("prepared authority is config-bound")
                .code(),
            "invalid_transport_contract"
        );
        assert_eq!(mismatch_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn expired_delivery_deadline_performs_no_relay_work() {
        let calls = Arc::new(AtomicUsize::new(0));
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        let transport =
            NostrTransport::with_client(config, Arc::new(CountingRelayClient(Arc::clone(&calls))));
        let receipt = futures::executor::block_on(transport.deliver(request_with_deadline(1)))
            .expect("bounded timeout receipt");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(
            receipt
                .target_receipts()
                .iter()
                .all(|target| !target.was_attempted())
        );
    }

    #[derive(Debug)]
    struct ScriptedRelayClient(Vec<RelayPublishResult>);

    impl RelayClient for ScriptedRelayClient {
        fn publish<'a>(
            &'a self,
            _relays: Vec<RelayUrl>,
            _event: Event,
            _max_connections: usize,
            _connect_timeout: Duration,
            _operation_timeout: Duration,
        ) -> BoxFuture<'a, Vec<RelayPublishResult>> {
            Box::pin(async move { self.0.clone() })
        }
    }

    fn scripted(results: Vec<RelayPublishResult>) -> NostrTransport {
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        NostrTransport::with_client(config, Arc::new(ScriptedRelayClient(results)))
    }

    #[test]
    fn sink_handles_missing_duplicate_unexpected_and_denied_targets() {
        let one = RelayUrl::parse("wss://one.example", RelayUrlPolicy::Public).expect("one");
        let missing = futures::executor::block_on(
            scripted(vec![RelayPublishResult {
                relay: one.clone(),
                outcome: DeliveryOutcome::accepted(),
            }])
            .deliver(request()),
        )
        .expect("missing result receipt");
        assert_eq!(missing.target_receipts().len(), 2);

        let duplicate = scripted(vec![
            RelayPublishResult {
                relay: one.clone(),
                outcome: DeliveryOutcome::accepted(),
            },
            RelayPublishResult {
                relay: one,
                outcome: DeliveryOutcome::accepted(),
            },
        ]);
        assert_eq!(
            futures::executor::block_on(duplicate.deliver(request()))
                .expect_err("duplicate relay evidence")
                .code(),
            "invalid_transport_contract"
        );

        let other = RelayUrl::parse("wss://other.example", RelayUrlPolicy::Public).expect("other");
        let unexpected = scripted(vec![RelayPublishResult {
            relay: other,
            outcome: DeliveryOutcome::accepted(),
        }]);
        assert_eq!(
            futures::executor::block_on(unexpected.deliver(request()))
                .expect_err("unexpected relay evidence")
                .code(),
            "invalid_transport_contract"
        );

        let denied_request = DeliveryRequest::new(
            "denied",
            payload(),
            TargetSet::new(vec![
                Target::nostr_relay("wss://other.example").expect("other"),
            ])
            .expect("targets"),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            1_800_000_000_000,
        )
        .expect("request");
        let denied = futures::executor::block_on(scripted(vec![]).deliver(denied_request))
            .expect("denied receipt");
        assert!(!denied.target_receipts()[0].was_attempted());
        assert!(futures::executor::block_on(scripted(vec![]).status()).is_ok());
    }

    #[test]
    fn live_relay_client_accepts_an_empty_batch_without_io() {
        let client = LiveRelayClient::isolated();
        let results = futures::executor::block_on(client.publish(
            vec![],
            radroots_nostr::event::to_nostr(payload().event().envelope()).expect("nostr event"),
            1,
            Duration::from_millis(1),
            Duration::from_millis(1),
        ));
        assert!(results.is_empty());
    }
}
