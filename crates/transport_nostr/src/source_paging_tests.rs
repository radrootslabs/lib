use super::*;
use crate::{Config, RelayUrlPolicy};
use radroots_transport::{Target, TargetSet, source::FetchBounds};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

struct CappedRelay {
    history: Vec<String>,
    calls: AtomicUsize,
}

impl RelaySourceClient for CappedRelay {
    fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
        Box::pin(async move {
            self.calls.fetch_add(1, AtomicOrdering::SeqCst);
            let events = self
                .history
                .iter()
                .filter(|raw| {
                    let event = radroots_event_codec::decode::signed_event(raw).unwrap();
                    query.selector.matches(&event)
                        && query
                            .until_unix_seconds
                            .is_none_or(|until| event.created_at() <= until)
                })
                .take(UPSTREAM_FETCH_LIMIT)
                .cloned()
                .collect::<Vec<_>>();
            query
                .relays
                .into_iter()
                .map(|relay| RelayFetchBatch {
                    relay,
                    result: RelayFetchResult::Complete(events.clone()),
                })
                .collect()
        })
    }
}

// Envelope IDs are canonical; signature verification remains in the shared
// ingest owner. This fixture exercises transport ordering only.
fn raw_event(id: usize, time: u64) -> String {
    let pubkey = "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
    let content = format!("page fixture {id}");
    let canonical = serde_json::json!([0, pubkey, time, 1, [], content]).to_string();
    let event_id = hex_encode(&Sha256::digest(canonical.as_bytes()));
    serde_json::json!({
        "id": event_id, "pubkey": pubkey,
        "created_at": time, "kind": 1, "tags": [], "content": content,
        "sig": "2".repeat(128)
    })
    .to_string()
}

fn fixture(
    ties: usize,
    time: u64,
    older: bool,
    duplicate_relay: bool,
) -> (NostrTransport, FetchRequest, Arc<CappedRelay>) {
    let mut history = (1..=ties)
        .rev()
        .map(|id| raw_event(id, time))
        .collect::<Vec<_>>();
    history.sort_by_cached_key(|raw| {
        std::cmp::Reverse(
            radroots_event_codec::decode::signed_event(raw)
                .unwrap()
                .id_str()
                .to_owned(),
        )
    });
    if older {
        history.push(raw_event(ties + 1, time - 1));
    }
    let source = Arc::new(CappedRelay {
        history,
        calls: AtomicUsize::new(0),
    });
    let mut urls = vec!["wss://one.example"];
    if duplicate_relay {
        urls.push("wss://two.example");
    }
    let config = Config::from_profile(
        crate::profile::test_profile(
            crate::RelayProfileKind::Public,
            RelayUrlPolicy::Public,
            urls.clone(),
        )
        .unwrap(),
    );
    let transport = NostrTransport::with_source_client(config, source.clone());
    let request = FetchRequest::new(
        "capped-page",
        TargetSet::new(
            urls.into_iter()
                .map(|url| Target::nostr_relay(url).unwrap())
                .collect(),
        )
        .unwrap(),
        FetchBounds::new(500, u64::MAX).unwrap(),
    )
    .unwrap();
    (transport, request, source)
}

#[tokio::test]
async fn ordinary_equal_time_pages_retain_all_peers_and_deduplicate_relays() {
    let (transport, request, source) = fixture(501, 100, true, true);
    let first = transport.fetch(request.clone()).await.unwrap();
    assert_eq!(first.events().len(), 500);
    let NextPage::Cursor(cursor) = first.next_page() else {
        panic!("next equal-time page")
    };
    let second = transport
        .fetch(request.with_cursor(cursor.clone()))
        .await
        .unwrap();
    assert_eq!(second.events().len(), 2);
    assert!(matches!(second.next_page(), NextPage::Complete));
    let ids = first
        .events()
        .iter()
        .chain(second.events())
        .map(|e| e.event().id_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 502);
    assert_eq!(source.calls.load(AtomicOrdering::SeqCst), 2);
}

#[tokio::test]
async fn capped_eose_yields_partial_coverage_and_explicit_older_backfill() {
    for ties in [1000, 1001] {
        let (transport, request, source) = fixture(ties, 100, true, true);
        let first = transport.fetch(request.clone()).await.unwrap();
        assert_eq!(first.events().len(), 500);
        assert!(
            first
                .target_outcomes()
                .iter()
                .all(|outcome| outcome.state() == FetchTargetState::Partial)
        );
        let NextPage::Cursor(cursor) = first.next_page() else {
            panic!("collected peers remain")
        };
        let second = transport
            .fetch(request.clone().with_cursor(cursor.clone()))
            .await
            .unwrap();
        assert_eq!(second.events().len(), 500);
        let NextPage::Cancelled {
            resume_from: Some(older),
        } = second.next_page()
        else {
            panic!("a capped boundary must yield explicit older continuation")
        };
        let received = first
            .events()
            .iter()
            .chain(second.events())
            .map(|e| e.event().id_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(received.len(), 1000);
        assert_eq!(source.calls.load(AtomicOrdering::SeqCst), 2);
        let third = transport
            .fetch(request.with_cursor(older.clone()))
            .await
            .unwrap();
        assert_eq!(third.events().len(), 1);
        assert_eq!(third.events()[0].event().created_at(), 99);
        assert!(matches!(third.next_page(), NextPage::Complete));
        assert_eq!(source.calls.load(AtomicOrdering::SeqCst), 3);
        // The 1001st same-time peer is outside the capped discovery window;
        // partial evidence, not an invented completeness claim, describes it.
    }
}

#[tokio::test]
async fn zero_timestamp_yields_partial_without_fabricating_older_history() {
    let (transport, request, _) = fixture(1000, 0, false, false);
    let first = transport.fetch(request.clone()).await.unwrap();
    let NextPage::Cursor(cursor) = first.next_page() else {
        panic!("received peers")
    };
    let second = transport
        .fetch(request.with_cursor(cursor.clone()))
        .await
        .unwrap();
    assert_eq!(second.events().len(), 500);
    assert_eq!(
        second.target_outcomes()[0].state(),
        FetchTargetState::Partial
    );
    assert!(matches!(
        second.next_page(),
        NextPage::Cancelled { resume_from: None }
    ));
}

#[tokio::test]
async fn older_window_rejects_changed_scope_before_access() {
    let (transport, request, source) = fixture(1000, 100, false, false);
    let older = window::before_boundary(100, &request_scope(&request)).unwrap();
    for selector in [
        radroots_transport::source::FetchSelector::all()
            .with_kinds(vec![1])
            .unwrap(),
        radroots_transport::source::FetchSelector::all()
            .with_since_unix_seconds(1)
            .unwrap(),
        radroots_transport::source::FetchSelector::all()
            .with_until_unix_seconds(100)
            .unwrap(),
    ] {
        assert!(
            transport
                .fetch(
                    request
                        .clone()
                        .with_selector(selector)
                        .with_cursor(older.clone())
                )
                .await
                .is_err()
        );
    }
    let (_, different, _) = fixture(1, 100, false, true);
    assert!(transport.fetch(different.with_cursor(older)).await.is_err());
    assert_eq!(source.calls.load(AtomicOrdering::SeqCst), 0);
}

struct UnfilteredSource(Vec<String>);
impl RelaySourceClient for UnfilteredSource {
    fn fetch<'a>(&'a self, query: SourceQuery) -> BoxFuture<'a, Vec<RelayFetchBatch>> {
        Box::pin(async move {
            query
                .relays
                .into_iter()
                .map(|relay| RelayFetchBatch {
                    relay,
                    result: RelayFetchResult::Complete(self.0.clone()),
                })
                .collect()
        })
    }
}

#[tokio::test]
async fn malformed_or_out_of_bound_caps_cannot_fabricate_a_backward_boundary() {
    for raw in ["{".to_owned(), raw_event(1, 101)] {
        let (base, request, _) = fixture(1, 100, false, false);
        let transport = NostrTransport::with_source_client(
            base.config().clone(),
            Arc::new(UnfilteredSource(vec![raw; 1000])),
        );
        let cursor = window::before_boundary(101, &request_scope(&request)).unwrap();
        let page = transport.fetch(request.with_cursor(cursor)).await.unwrap();
        assert!(page.events().is_empty());
        assert_eq!(page.target_outcomes()[0].state(), FetchTargetState::Partial);
        assert!(matches!(
            page.next_page(),
            NextPage::Cancelled { resume_from: None }
        ));
    }
}
