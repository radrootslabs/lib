//! Nostr implementation of the transport event source.

use crate::{NostrTransport, RelayUrl, status};
use core::cmp::Ordering;
use core::time::Duration;
use nostr_sdk::prelude::{Filter, JsonUtil, Kind, Timestamp};
use radroots_transport::{
    BoxFuture, EventSource, FetchPage, FetchRequest,
    outcome::{FetchTargetOutcome, FetchTargetState},
    source::{EventProvenance, FetchCursor, NextPage, ObservedEvent, SourceStatus},
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

const UPSTREAM_FETCH_LIMIT: usize = 1_000;
const CURSOR_PREFIX: &str = "nostr-v1";

#[derive(Clone, Debug)]
pub(crate) struct SourceQuery {
    relays: Vec<RelayUrl>,
    selector: radroots_transport::source::FetchSelector,
    until_unix_seconds: Option<u64>,
    connect_timeout: Duration,
    timeout: Duration,
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
    fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
        Box::pin(async move {
            let mut batches = Vec::with_capacity(query.relays.len());
            for relay in query.relays {
                let url = relay.as_str().to_owned();
                let result = async {
                    let kinds = query
                        .selector
                        .kinds()
                        .iter()
                        .filter_map(|kind| u16::try_from(*kind).ok())
                        .map(Kind::from)
                        .collect::<Vec<_>>();
                    if !query.selector.kinds().is_empty() && kinds.is_empty() {
                        return Ok(Vec::new());
                    }
                    let authors = query
                        .selector
                        .authors()
                        .iter()
                        .filter_map(|author| radroots_nostr::key::public_key_to_nostr(*author).ok())
                        .collect::<Vec<_>>();
                    if authors.len() != query.selector.authors().len() {
                        return Ok(Vec::new());
                    }
                    self.client.add_relay(url.as_str()).await?;
                    self.client
                        .try_connect_relay(url.as_str(), query.connect_timeout)
                        .await?;
                    let mut filter = Filter::new().limit(UPSTREAM_FETCH_LIMIT);
                    if !kinds.is_empty() {
                        filter = filter.kinds(kinds);
                    }
                    if !authors.is_empty() {
                        filter = filter.authors(authors);
                    }
                    if let Some(since) = query.selector.since_unix_seconds() {
                        filter = filter.since(Timestamp::from_secs(since));
                    }
                    if let Some(until) = query.until_unix_seconds {
                        filter = filter.until(Timestamp::from_secs(until));
                    }
                    self.client
                        .fetch_events_from([url.as_str()], filter, query.timeout)
                        .await
                        .map(|events| events.iter().map(JsonUtil::as_json).collect())
                }
                .await
                .map_err(|error: nostr_sdk::client::Error| error.to_string());
                batches.push(RelayFetchBatch { relay, result });
            }
            batches
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
        let configured = !self.config().relays().is_empty();
        Box::pin(async move { Ok(status::source_status(&self.status, configured)) })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> BoxFuture<'_, Result<FetchPage, radroots_transport::Error>> {
        Box::pin(async move {
            let cursor = request.cursor().map(parse_cursor).transpose()?.flatten();
            let selector_until = request.selector().until_unix_seconds();
            let now_ms = unix_time_ms();
            let remaining_ms = request.bounds().deadline_unix_ms().saturating_sub(now_ms);
            let timeout_ms = remaining_ms.min(self.config().request_timeout_ms());

            let mut targets = BTreeMap::new();
            let mut outcomes = Vec::new();
            for target in request.target_set().targets() {
                match RelayUrl::from_target(target, self.config().relay_url_policy()) {
                    Ok(relay) if self.config().relays().contains(&relay) => {
                        targets.insert(relay, target.clone());
                    }
                    _ => outcomes.push(
                        FetchTargetOutcome::new(
                            target.fingerprint().clone(),
                            FetchTargetState::FailedTerminal,
                        )
                        .with_message("target is not configured for this source"),
                    ),
                }
            }

            if timeout_ms == 0 {
                outcomes.extend(targets.values().map(|target| {
                    FetchTargetOutcome::new(
                        target.fingerprint().clone(),
                        FetchTargetState::FailedRetryable,
                    )
                    .with_message("fetch deadline elapsed before relay access")
                }));
                self.status.record_source(0, targets.len(), Some("timeout"));
                return FetchPage::for_request(&request, Vec::new(), outcomes, NextPage::Complete);
            }

            let batches = self
                .source_client
                .fetch(SourceQuery {
                    relays: targets.keys().cloned().collect(),
                    selector: request.selector().clone(),
                    until_unix_seconds: match (selector_until, cursor.as_ref()) {
                        (Some(until), Some(cursor)) => Some(until.min(cursor.created_at)),
                        (Some(until), None) => Some(until),
                        (None, Some(cursor)) => Some(cursor.created_at),
                        (None, None) => None,
                    },
                    connect_timeout: Duration::from_millis(
                        timeout_ms.min(self.config().connect_timeout_ms()),
                    ),
                    timeout: Duration::from_millis(timeout_ms),
                })
                .await;
            let mut candidates = Vec::new();
            let mut malformed_by_relay = BTreeMap::<RelayUrl, usize>::new();
            let mut reported = BTreeSet::new();
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            let mut diagnostic = None;
            for batch in batches {
                let Some(target) = targets.get(&batch.relay) else {
                    return Err(radroots_transport::Error::UnexpectedFetchTargetOutcome);
                };
                if !reported.insert(batch.relay.clone()) {
                    return Err(radroots_transport::Error::DuplicateFetchTargetOutcome);
                }
                match batch.result {
                    Ok(raw_events) => {
                        succeeded += 1;
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
                        failed += 1;
                        let (state, safe) = status::fetch_failure(message.as_str());
                        diagnostic.get_or_insert(safe);
                        outcomes.push(
                            FetchTargetOutcome::new(target.fingerprint().clone(), state)
                                .with_message(safe),
                        );
                    }
                }
            }
            for (relay, target) in &targets {
                if !reported.contains(relay) {
                    failed += 1;
                    diagnostic.get_or_insert("relay returned no fetch result");
                    outcomes.push(
                        FetchTargetOutcome::new(
                            target.fingerprint().clone(),
                            FetchTargetState::FailedRetryable,
                        )
                        .with_message("relay returned no fetch result"),
                    );
                }
            }
            self.status.record_source(succeeded, failed, diagnostic);

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
                    "{CURSOR_PREFIX}:{}:{}",
                    last.created_at, last.event_id
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

#[derive(Clone, Debug)]
struct CursorPosition {
    created_at: u64,
    event_id: String,
}

fn parse_cursor(cursor: &FetchCursor) -> Result<Option<CursorPosition>, radroots_transport::Error> {
    let mut parts = cursor.as_str().split(':');
    let valid = parts.next() == Some(CURSOR_PREFIX);
    let created_at = parts.next().and_then(|value| value.parse::<u64>().ok());
    let event_id = parts.next();
    if !valid || parts.next().is_some() {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    let (Some(created_at), Some(event_id)) = (created_at, event_id) else {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    };
    if event_id.len() != 64 || !event_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(radroots_transport::Error::InvalidFetchCursor);
    }
    Ok(Some(CursorPosition {
        created_at,
        event_id: event_id.to_ascii_lowercase(),
    }))
}

fn compare_candidate(left: &Candidate, right: &Candidate) -> Ordering {
    right
        .created_at
        .cmp(&left.created_at)
        .then_with(|| right.event_id.cmp(&left.event_id))
        .then_with(|| left.relay.cmp(&right.relay))
}

fn candidate_is_after_cursor(candidate: &Candidate, cursor: &CursorPosition) -> bool {
    candidate.created_at < cursor.created_at
        || candidate.created_at == cursor.created_at && candidate.event_id < cursor.event_id
}

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
        let config = Config::new(
            RelayUrlPolicy::Public,
            ["wss://one.example", "wss://two.example"],
        )
        .expect("config");
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
        let config = Config::new(RelayUrlPolicy::Public, ["wss://one.example"]).expect("config");
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
}
