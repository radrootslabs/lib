//! Nostr implementation of the transport event source.

use crate::{NostrTransport, RelayCursor, RelayUrl, status};
use core::cmp::Ordering;
use core::time::Duration;
use futures::{StreamExt, stream};
use nostr_sdk::prelude::{Filter, JsonUtil, Kind, Timestamp};
use radroots_transport::{
    BoxFuture, EventSource, FetchPage, FetchRequest,
    outcome::{FetchTargetOutcome, FetchTargetState},
    source::{EventProvenance, FetchCursor, NextPage, ObservedEvent, SourceStatus},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const UPSTREAM_FETCH_LIMIT: usize = 1_000;
const CURSOR_PREFIX: &str = "nostr-v2";
const CURSOR_SCOPE_DOMAIN: &[u8] = b"radroots.transport-nostr.fetch-cursor.v2\0";

#[derive(Clone, Debug)]
pub(crate) struct SourceQuery {
    relays: Vec<RelayUrl>,
    selector: radroots_transport::source::FetchSelector,
    until_unix_seconds: Option<u64>,
    connect_timeout: Duration,
    timeout: Duration,
    max_connections: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RelayFetchBatch {
    relay: RelayUrl,
    result: Result<Vec<String>, String>,
}

pub(crate) trait RelaySourceClient: Send + Sync {
    fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>>;
}

#[derive(Clone, Debug)]
pub(crate) struct LiveRelaySourceClient {
    client: nostr_sdk::Client,
}

impl LiveRelaySourceClient {
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

impl RelaySourceClient for LiveRelaySourceClient {
    // The live SDK loop requires external relays. Selection and normalized
    // result handling are covered through the injected source boundary.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
        Box::pin(async move {
            let SourceQuery {
                relays,
                selector,
                until_unix_seconds,
                connect_timeout,
                timeout,
                max_connections,
            } = query;
            stream::iter(relays.into_iter().map(|relay| {
                let selector = selector.clone();
                async move {
                    let url = relay.as_str().to_owned();
                    let result = async {
                        let kinds = selector
                            .kinds()
                            .iter()
                            .filter_map(|kind| u16::try_from(*kind).ok())
                            .map(Kind::from)
                            .collect::<Vec<_>>();
                        if !selector.kinds().is_empty() && kinds.is_empty() {
                            return Ok(Vec::new());
                        }
                        let authors = selector
                            .authors()
                            .iter()
                            .filter_map(|author| {
                                radroots_nostr::key::public_key_to_nostr(*author).ok()
                            })
                            .collect::<Vec<_>>();
                        if authors.len() != selector.authors().len() {
                            return Ok(Vec::new());
                        }
                        self.client.add_relay(url.as_str()).await?;
                        self.client
                            .try_connect_relay(url.as_str(), connect_timeout)
                            .await?;
                        let mut filter = Filter::new().limit(UPSTREAM_FETCH_LIMIT);
                        if !kinds.is_empty() {
                            filter = filter.kinds(kinds);
                        }
                        if !authors.is_empty() {
                            filter = filter.authors(authors);
                        }
                        if let Some(since) = selector.since_unix_seconds() {
                            filter = filter.since(Timestamp::from_secs(since));
                        }
                        if let Some(until) = until_unix_seconds {
                            filter = filter.until(Timestamp::from_secs(until));
                        }
                        self.client
                            .fetch_events_from([url.as_str()], filter, timeout)
                            .await
                            .map(|events| events.iter().map(JsonUtil::as_json).collect())
                    }
                    .await
                    .map_err(|error: nostr_sdk::client::Error| error.to_string());
                    RelayFetchBatch { relay, result }
                }
            }))
            .buffered(max_connections)
            .collect()
            .await
        })
    }
}

#[derive(Debug)]
struct Candidate {
    relay: RelayUrl,
    raw: String,
    created_at: u64,
    event_id: String,
}

impl EventSource for NostrTransport {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, radroots_transport::Error>> {
        Box::pin(async move { Ok(status::source_status(&self.status)) })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> BoxFuture<'_, Result<FetchPage, radroots_transport::Error>> {
        Box::pin(async move {
            let cursor_scope = request_scope(&request);
            let cursor = request
                .cursor()
                .map(|cursor| parse_cursor(cursor, cursor_scope.as_str()))
                .transpose()?;
            let selector_until = request.selector().until_unix_seconds();
            let now_ms = unix_time_ms();
            let remaining_ms = request.bounds().deadline_unix_ms().saturating_sub(now_ms);
            let timeout_ms = remaining_ms.min(self.config().request_timeout_ms());

            let mut targets = BTreeMap::new();
            let mut outcomes = Vec::new();
            for target in request.target_set().targets() {
                match self.config().endpoint_for_target(target) {
                    Some(endpoint) if endpoint.access().can_read() => {
                        let relay = endpoint.url().clone();
                        if self.status.may_read(&relay, now_ms) {
                            targets.insert(relay, target.clone());
                        } else {
                            outcomes.push(
                                FetchTargetOutcome::new(
                                    target.fingerprint().clone(),
                                    FetchTargetState::FailedRetryable,
                                )
                                .with_message("relay reconnect backoff is active"),
                            );
                        }
                    }
                    None | Some(_) => outcomes.push(
                        FetchTargetOutcome::new(
                            target.fingerprint().clone(),
                            FetchTargetState::FailedTerminal,
                        )
                        .with_message("target is not configured for this source"),
                    ),
                }
            }

            if timeout_ms == 0 {
                for relay in targets.keys() {
                    self.status.record_read(relay, false, true, now_ms);
                }
                outcomes.extend(targets.values().map(|target| {
                    FetchTargetOutcome::new(
                        target.fingerprint().clone(),
                        FetchTargetState::FailedRetryable,
                    )
                    .with_message("fetch deadline elapsed before relay access")
                }));
                return FetchPage::for_request(&request, Vec::new(), outcomes, NextPage::Complete);
            }

            for relay in targets.keys() {
                self.status.begin_read(relay, now_ms);
            }
            let batches = self
                .source_client
                .fetch(SourceQuery {
                    relays: targets.keys().cloned().collect(),
                    selector: request.selector().clone(),
                    until_unix_seconds: match (selector_until, cursor.as_ref()) {
                        (Some(until), Some(cursor)) => Some(until.min(cursor.created_at_unix_s())),
                        (Some(until), None) => Some(until),
                        (None, Some(cursor)) => Some(cursor.created_at_unix_s()),
                        (None, None) => None,
                    },
                    connect_timeout: Duration::from_millis(
                        timeout_ms.min(self.config().connect_timeout_ms()),
                    ),
                    timeout: Duration::from_millis(timeout_ms),
                    max_connections: self.config().max_connections(),
                })
                .await;
            let mut candidates = Vec::new();
            let mut malformed_by_relay = BTreeMap::<RelayUrl, usize>::new();
            let mut reported = BTreeSet::new();
            let observed_at_unix_ms = unix_time_ms().max(now_ms);
            for batch in batches {
                let Some(target) = targets.get(&batch.relay) else {
                    return Err(radroots_transport::Error::UnexpectedFetchTargetOutcome);
                };
                if !reported.insert(batch.relay.clone()) {
                    return Err(radroots_transport::Error::DuplicateFetchTargetOutcome);
                }
                match batch.result {
                    Ok(raw_events) => {
                        self.status
                            .record_read(&batch.relay, true, false, observed_at_unix_ms);
                        for raw in raw_events {
                            match radroots_event_codec::decode::signed_event(raw.as_str()) {
                                Ok(event) if request.selector().matches(&event) => {
                                    candidates.push(Candidate {
                                        relay: batch.relay.clone(),
                                        created_at: event.created_at(),
                                        event_id: event.id_str().to_owned(),
                                        raw,
                                    })
                                }
                                Ok(_) => {}
                                Err(_) => {
                                    *malformed_by_relay.entry(batch.relay.clone()).or_default() +=
                                        1;
                                }
                            }
                        }
                        let malformed = malformed_by_relay
                            .get(&batch.relay)
                            .copied()
                            .unwrap_or_default();
                        let outcome = if malformed == 0 {
                            FetchTargetOutcome::new(
                                target.fingerprint().clone(),
                                FetchTargetState::Complete,
                            )
                        } else {
                            FetchTargetOutcome::new(
                                target.fingerprint().clone(),
                                FetchTargetState::Partial,
                            )
                            .with_message(format!("ignored {malformed} malformed relay event(s)"))
                        };
                        outcomes.push(outcome);
                    }
                    Err(message) => {
                        let (state, safe) = status::fetch_failure(message.as_str());
                        self.status.record_read(
                            &batch.relay,
                            false,
                            state.is_retryable(),
                            observed_at_unix_ms,
                        );
                        outcomes.push(
                            FetchTargetOutcome::new(target.fingerprint().clone(), state)
                                .with_message(safe),
                        );
                    }
                }
            }
            for (relay, target) in &targets {
                if !reported.contains(relay) {
                    self.status
                        .record_read(relay, false, true, observed_at_unix_ms);
                    outcomes.push(
                        FetchTargetOutcome::new(
                            target.fingerprint().clone(),
                            FetchTargetState::FailedRetryable,
                        )
                        .with_message("relay returned no fetch result"),
                    );
                }
            }
            candidates.sort_by(compare_candidate);
            if let Some(cursor) = &cursor {
                candidates.retain(|candidate| candidate_is_after_cursor(candidate, cursor));
            }
            let mut seen = BTreeSet::new();
            candidates.retain(|candidate| seen.insert(candidate.event_id.clone()));

            let has_more = candidates.len() > usize::from(request.bounds().limit());
            candidates.truncate(usize::from(request.bounds().limit()));
            let next_page = if has_more {
                let last = candidates.last().expect("non-empty bounded page");
                NextPage::Cursor(FetchCursor::parse(format!(
                    "{CURSOR_PREFIX}:{}:{}:{cursor_scope}",
                    last.created_at, last.event_id,
                ))?)
            } else {
                NextPage::Complete
            };
            let observed_at = unix_time_ms().max(1);
            let mut events = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                let target = targets
                    .get(&candidate.relay)
                    .expect("candidate relay has requested target");
                let mut provenance = EventProvenance::new(
                    radroots_transport::TransportId::NOSTR,
                    target.fingerprint().clone(),
                    observed_at,
                )?;
                if let Some(request_cursor) = request.cursor().cloned() {
                    provenance = provenance.with_cursor(request_cursor);
                }
                let event = radroots_event_codec::decode::signed_event(candidate.raw.as_str())
                    .map_err(|_| radroots_transport::Error::UnexpectedFetchProvenance)?;
                events.push(ObservedEvent::new(event, provenance));
            }
            FetchPage::for_request(&request, events, outcomes, next_page)
        })
    }
}

fn parse_cursor(
    cursor: &FetchCursor,
    expected_scope: &str,
) -> Result<RelayCursor, radroots_transport::Error> {
    let mut parts = cursor.as_str().split(':');
    let valid = parts.next() == Some(CURSOR_PREFIX);
    let created_at = parts.next().and_then(|value| value.parse::<u64>().ok());
    let event_id = parts.next();
    let scope = parts.next();
    if !valid || parts.next().is_some() {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    let (Some(created_at), Some(event_id), Some(scope)) = (created_at, event_id, scope) else {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    };
    if scope != expected_scope {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    RelayCursor::new(created_at, event_id)
        .map_err(|_| radroots_transport::Error::InvalidFetchCursor)
}

fn compare_candidate(left: &Candidate, right: &Candidate) -> Ordering {
    right
        .created_at
        .cmp(&left.created_at)
        .then_with(|| right.event_id.cmp(&left.event_id))
        .then_with(|| left.relay.cmp(&right.relay))
}

fn candidate_is_after_cursor(candidate: &Candidate, cursor: &RelayCursor) -> bool {
    cursor.page_precedes(candidate.created_at, candidate.event_id.as_str())
}

fn request_scope(request: &FetchRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CURSOR_SCOPE_DOMAIN);
    for target in request.target_set().targets() {
        hasher.update(target.fingerprint().as_str().as_bytes());
        hasher.update([0]);
    }
    for kind in request.selector().kinds() {
        hasher.update(kind.to_be_bytes());
    }
    hasher.update([0]);
    for author in request.selector().authors() {
        hasher.update(author.as_bytes());
    }
    hasher.update([0]);
    hash_optional_u64(&mut hasher, request.selector().since_unix_seconds());
    hash_optional_u64(&mut hasher, request.selector().until_unix_seconds());
    hex_encode(&hasher.finalize())
}

fn hash_optional_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, RelayUrlPolicy};
    use radroots_transport::{
        FetchRequest, Target, TargetSet,
        source::{FetchBounds, FetchSelector, NextPage},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    };

    const FIRST: &str = r#"{"id":"762bee187e9e645b81ec26ade05a69b5e8398caf527be8de0d9a45311ed0c7a0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1800000100,"kind":0,"tags":[],"content":"{\"display_name\":\"Moss Street Farm\",\"bot\":false,\"website\":\"https://mossstreet.example\",\"picture\":42}","sig":"4290da0bb6422986647bc8cd5f63bd52d49f41e7b665d3b47105b8109183e8d596f322c531d4061df53e1d2b70fda12d5d1c14f3720d7a56d9d0a03746af5109"}"#;
    const SECOND: &str = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;

    #[derive(Debug)]
    struct MockSourceClient;

    impl RelaySourceClient for MockSourceClient {
        fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
            Box::pin(async move {
                query
                    .relays
                    .into_iter()
                    .map(|relay| RelayFetchBatch {
                        relay,
                        result: Ok(vec![FIRST.to_owned(), SECOND.to_owned(), "{".to_owned()]),
                    })
                    .collect()
            })
        }
    }

    fn transport() -> NostrTransport {
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        NostrTransport::with_source_client(config, Arc::new(MockSourceClient))
    }

    fn request(limit: u16) -> FetchRequest {
        FetchRequest::new(
            "nostr-fetch",
            TargetSet::new(vec![
                Target::nostr_relay("wss://one.example").expect("one"),
                Target::nostr_relay("wss://two.example").expect("two"),
            ])
            .expect("targets"),
            FetchBounds::new(limit, u64::MAX).expect("bounds"),
        )
        .expect("request")
    }

    #[test]
    fn source_deduplicates_relays_reports_malformed_and_paginates() {
        let transport = transport();
        let first_request = request(1);
        let first =
            futures::executor::block_on(transport.fetch(first_request)).expect("first page");
        assert_eq!(first.events().len(), 1);
        assert!(
            first
                .target_outcomes()
                .iter()
                .all(|outcome| outcome.state() == FetchTargetState::Partial)
        );
        let NextPage::Cursor(cursor) = first.next_page() else {
            panic!("cursor expected");
        };

        let second_request = request(2).with_cursor(cursor.clone());
        let second =
            futures::executor::block_on(transport.fetch(second_request)).expect("second page");
        assert_eq!(second.events().len(), 1);
        assert!(matches!(second.next_page(), NextPage::Complete));
        assert_ne!(
            first.events()[0].event().id(),
            second.events()[0].event().id()
        );
    }

    #[test]
    fn source_applies_kind_author_and_time_selectors_before_page_bounds() {
        let event = radroots_event_codec::decode::signed_event(FIRST).expect("fixture event");
        let selector = FetchSelector::all()
            .with_kinds(vec![0])
            .expect("kind")
            .with_authors(vec![*event.pubkey()])
            .expect("author")
            .with_since_unix_seconds(1_750_000_000)
            .expect("since");
        let selected =
            futures::executor::block_on(transport().fetch(request(10).with_selector(selector)))
                .expect("selected page");
        assert_eq!(selected.events().len(), 1);
        assert_eq!(selected.events()[0].event().id_str(), event.id_str());

        let excluded = FetchSelector::all()
            .with_kinds(vec![1])
            .expect("excluded kind");
        let selected =
            futures::executor::block_on(transport().fetch(request(10).with_selector(excluded)))
                .expect("empty selected page");
        assert!(selected.events().is_empty());
    }

    #[test]
    fn malformed_cursor_fails_before_relay_access() {
        let request = request(1).with_cursor(FetchCursor::parse("other:1:value").expect("opaque"));
        let error = futures::executor::block_on(transport().fetch(request)).expect_err("cursor");
        assert_eq!(error, radroots_transport::Error::InvalidFetchCursor);
    }

    #[test]
    fn dropping_an_unpolled_fetch_performs_no_relay_work() {
        #[derive(Debug)]
        struct CountingSourceClient(Arc<AtomicUsize>);

        impl RelaySourceClient for CountingSourceClient {
            fn fetch<'a>(&'a self, _query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
                self.0.fetch_add(1, AtomicOrdering::SeqCst);
                Box::pin(async { Vec::new() })
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example"],
            )
            .expect("profile"),
        );
        let transport = NostrTransport::with_source_client(
            config,
            Arc::new(CountingSourceClient(Arc::clone(&calls))),
        );
        let target_set = TargetSet::new(vec![
            Target::nostr_relay("wss://one.example").expect("target"),
        ])
        .expect("targets");
        let fetch = transport.fetch(
            FetchRequest::new(
                "cancel-before-poll",
                target_set,
                FetchBounds::new(1, u64::MAX).expect("bounds"),
            )
            .expect("request"),
        );
        drop(fetch);
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 0);
    }

    #[test]
    fn adapter_and_generic_page_limits_are_identical() {
        assert_eq!(UPSTREAM_FETCH_LIMIT, 1_000);
        assert_eq!(
            UPSTREAM_FETCH_LIMIT,
            usize::from(radroots_transport::source::FETCH_PAGE_MAX_EVENTS)
        );
        assert!(FetchBounds::new(1_001, u64::MAX).is_err());
    }

    #[derive(Debug)]
    struct ScriptedSourceClient(Vec<RelayFetchBatch>);

    impl RelaySourceClient for ScriptedSourceClient {
        fn fetch<'a>(&'a self, _query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
            Box::pin(async move { self.0.clone() })
        }
    }

    fn scripted(batches: Vec<RelayFetchBatch>) -> NostrTransport {
        let config = Config::from_profile(
            crate::profile::test_profile(
                crate::RelayProfileKind::Public,
                RelayUrlPolicy::Public,
                ["wss://one.example", "wss://two.example"],
            )
            .expect("profile"),
        );
        NostrTransport::with_source_client(config, Arc::new(ScriptedSourceClient(batches)))
    }

    #[test]
    fn source_handles_failed_missing_duplicate_and_unexpected_batches() {
        let one = RelayUrl::parse("wss://one.example", RelayUrlPolicy::Public).expect("one");
        let two = RelayUrl::parse("wss://two.example", RelayUrlPolicy::Public).expect("two");
        let failed = futures::executor::block_on(
            scripted(vec![RelayFetchBatch {
                relay: one.clone(),
                result: Err("connection timeout".into()),
            }])
            .fetch(request(10)),
        )
        .expect("failed page");
        assert_eq!(failed.target_outcomes().len(), 2);
        assert!(failed.events().is_empty());

        let duplicate = scripted(vec![
            RelayFetchBatch {
                relay: one.clone(),
                result: Ok(vec![]),
            },
            RelayFetchBatch {
                relay: one,
                result: Ok(vec![]),
            },
        ]);
        assert_eq!(
            futures::executor::block_on(duplicate.fetch(request(10))),
            Err(radroots_transport::Error::DuplicateFetchTargetOutcome)
        );

        let other = RelayUrl::parse("wss://other.example", RelayUrlPolicy::Public).expect("other");
        let unexpected = scripted(vec![RelayFetchBatch {
            relay: other,
            result: Ok(vec![]),
        }]);
        assert_eq!(
            futures::executor::block_on(unexpected.fetch(request(10))),
            Err(radroots_transport::Error::UnexpectedFetchTargetOutcome)
        );

        let complete = futures::executor::block_on(
            scripted(vec![RelayFetchBatch {
                relay: two,
                result: Ok(vec![]),
            }])
            .fetch(request(10)),
        )
        .expect("partial reporting");
        assert_eq!(complete.target_outcomes().len(), 2);
    }

    #[test]
    fn source_rejects_unconfigured_targets_and_expired_deadlines() {
        let target_set = TargetSet::new(vec![
            Target::nostr_relay("wss://other.example").expect("other"),
        ])
        .expect("targets");
        let expired = FetchRequest::new(
            "expired",
            target_set,
            FetchBounds::new(1, 1).expect("bounds"),
        )
        .expect("request");
        let page = futures::executor::block_on(transport().fetch(expired)).expect("page");
        assert!(page.events().is_empty());
        assert_eq!(page.target_outcomes().len(), 1);
        assert!(futures::executor::block_on(transport().status()).is_ok());
    }

    #[test]
    fn cursor_and_candidate_ordering_cover_boundaries() {
        for invalid in [
            "nostr-v1",
            "nostr-v2:not-a-time:id:scope",
            "nostr-v2:1",
            "nostr-v2:1:id:scope:extra",
            "nostr-v2:1:abc:scope",
            "nostr-v2:1:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg:scope",
        ] {
            let cursor = FetchCursor::parse(invalid).expect("opaque cursor");
            assert!(matches!(
                parse_cursor(&cursor, &"a".repeat(64)),
                Err(radroots_transport::Error::InvalidFetchCursor)
            ));
        }
        let cursor = RelayCursor::new(10, "b".repeat(64)).expect("cursor");
        let relay = RelayUrl::parse("wss://one.example", RelayUrlPolicy::Public).expect("relay");
        let older = Candidate {
            relay: relay.clone(),
            raw: String::new(),
            created_at: 9,
            event_id: "f".repeat(64),
        };
        let earlier_id = Candidate {
            relay: relay.clone(),
            raw: String::new(),
            created_at: 10,
            event_id: "a".repeat(64),
        };
        let later = Candidate {
            relay,
            raw: String::new(),
            created_at: 11,
            event_id: "0".repeat(64),
        };
        assert!(candidate_is_after_cursor(&older, &cursor));
        assert!(candidate_is_after_cursor(&earlier_id, &cursor));
        assert!(!candidate_is_after_cursor(&later, &cursor));
        assert_ne!(compare_candidate(&older, &later), Ordering::Equal);
    }

    #[test]
    fn continuation_cursor_is_bound_to_exact_targets_and_selector() {
        let first = futures::executor::block_on(transport().fetch(request(1))).expect("first page");
        let NextPage::Cursor(cursor) = first.next_page() else {
            panic!("cursor expected");
        };
        let different_selector = FetchSelector::all().with_kinds(vec![1]).expect("selector");
        let error = futures::executor::block_on(
            transport().fetch(
                request(1)
                    .with_selector(different_selector)
                    .with_cursor(cursor.clone()),
            ),
        )
        .expect_err("scope mismatch");
        assert_eq!(error, radroots_transport::Error::InvalidFetchCursor);

        let other_targets =
            TargetSet::new(vec![Target::nostr_relay("wss://one.example").expect("one")])
                .expect("targets");
        let error = futures::executor::block_on(
            transport().fetch(
                FetchRequest::new(
                    "different-targets",
                    other_targets,
                    FetchBounds::new(1, u64::MAX).expect("bounds"),
                )
                .expect("request")
                .with_cursor(cursor.clone()),
            ),
        )
        .expect_err("target scope mismatch");
        assert_eq!(error, radroots_transport::Error::InvalidFetchCursor);
    }

    #[test]
    fn live_source_short_circuits_selectors_that_cannot_be_encoded() {
        let client = LiveRelaySourceClient::isolated();
        let relay = RelayUrl::parse("wss://one.example", RelayUrlPolicy::Public).expect("relay");
        let selector = FetchSelector::all()
            .with_kinds(vec![u32::MAX])
            .expect("kind");
        let batches = futures::executor::block_on(client.fetch(SourceQuery {
            relays: vec![relay],
            selector,
            until_unix_seconds: None,
            connect_timeout: Duration::from_millis(1),
            timeout: Duration::from_millis(1),
            max_connections: 1,
        }));
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].result, Ok(vec![]));
    }
}
