use core::{future::Future, pin::Pin, task::Context};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use futures::{future, task::noop_waker_ref};
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_identity::PublicKey;
use radroots_transport::{
    BoxFuture, Error, EventSource, FetchPage, FetchRequest, SourceStatus, Target, TargetSet,
    TransportId,
    capability::{Availability, Maturity, SourceCapabilities},
    outcome::{FetchTargetOutcome, FetchTargetState},
    source::{
        EventProvenance, FETCH_CURSOR_MAX_BYTES, FETCH_PAGE_MAX_EVENTS, FETCH_REQUEST_ID_MAX_BYTES,
        FETCH_SELECTOR_MAX_AUTHORS, FETCH_SELECTOR_MAX_KINDS, FetchBounds, FetchCursor,
        FetchSelector, NextPage, ObservedEvent,
    },
};

fn target(uri: &str) -> Target {
    Target::nostr_relay(uri).expect("nostr target")
}

#[test]
fn fetch_selector_is_bounded_canonical_and_request_bound() {
    let event = signed_event();
    let author = *event.pubkey();
    let selector = FetchSelector::all()
        .with_kinds(vec![1, 0])
        .expect("kind selector")
        .with_authors(vec![author])
        .expect("author selector")
        .with_since_unix_seconds(1_700_000_000)
        .expect("since")
        .with_until_unix_seconds(1_700_000_100)
        .expect("until");

    assert_eq!(selector.kinds(), &[0, 1]);
    assert_eq!(selector.authors(), &[author]);
    assert!(selector.matches(&event));
    assert_eq!(
        FetchSelector::all()
            .with_kinds(vec![1, 1])
            .expect_err("duplicate kind"),
        Error::DuplicateFetchKind
    );
    assert_eq!(
        FetchSelector::all()
            .with_authors(vec![author, author])
            .expect_err("duplicate author"),
        Error::DuplicateFetchAuthor
    );
    assert_eq!(
        FetchSelector::all()
            .with_kinds(vec![0; FETCH_SELECTOR_MAX_KINDS + 1])
            .expect_err("too many kinds"),
        Error::FetchSelectorTooLarge
    );
    assert_eq!(
        FetchSelector::all()
            .with_authors(vec![author; FETCH_SELECTOR_MAX_AUTHORS + 1])
            .expect_err("too many authors"),
        Error::FetchSelectorTooLarge
    );
    assert_eq!(
        FetchSelector::all()
            .with_since_unix_seconds(2)
            .and_then(|selector| selector.with_until_unix_seconds(1))
            .expect_err("reversed range"),
        Error::InvalidFetchTimeRange
    );

    let targets = TargetSet::new(vec![target("wss://one.example")]).expect("targets");
    let selected = request(targets.clone(), 1).with_selector(selector);
    let page = FetchPage::for_request(&selected, Vec::new(), Vec::new(), NextPage::Complete)
        .expect("selected page");
    assert_eq!(
        page.validate_for_request(&request(targets, 1))
            .expect_err("selector mismatch"),
        Error::FetchPageRequestMismatch
    );
}

fn signed_event() -> SignedEvent {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let wire = Nip01EventWire::parse_json(raw).expect("wire event");
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn request(targets: TargetSet, limit: u16) -> FetchRequest {
    FetchRequest::new(
        "fetch-request",
        targets,
        FetchBounds::new(limit, 1_700_000_100_000).expect("bounds"),
    )
    .expect("request")
}

#[test]
fn fetch_bounds_request_ids_and_cursors_fail_closed() {
    assert_eq!(
        FetchBounds::new(0, 1).expect_err("zero limit"),
        Error::InvalidFetchLimit
    );
    assert_eq!(
        FetchBounds::new(FETCH_PAGE_MAX_EVENTS + 1, 1).expect_err("oversized limit"),
        Error::InvalidFetchLimit
    );
    assert_eq!(
        FetchBounds::new(1, 0).expect_err("zero deadline"),
        Error::InvalidFetchDeadline
    );
    assert_eq!(
        FetchRequest::new(
            "",
            TargetSet::new(vec![target("wss://one.example")]).expect("targets"),
            FetchBounds::new(1, 1).expect("bounds"),
        )
        .expect_err("empty request id"),
        Error::EmptyFetchRequestId
    );
    assert_eq!(
        FetchRequest::new(
            "x".repeat(FETCH_REQUEST_ID_MAX_BYTES + 1),
            TargetSet::new(vec![target("wss://one.example")]).expect("targets"),
            FetchBounds::new(1, 1).expect("bounds"),
        )
        .expect_err("oversized request id"),
        Error::InvalidFetchRequestId
    );
    assert_eq!(
        FetchCursor::parse("").expect_err("empty cursor"),
        Error::EmptyFetchCursor
    );
    assert_eq!(
        FetchCursor::parse("x".repeat(FETCH_CURSOR_MAX_BYTES + 1)).expect_err("oversized cursor"),
        Error::InvalidFetchCursor
    );
    for invalid in [" request", "request ", "request\nid"] {
        assert_eq!(
            FetchRequest::new(
                invalid,
                TargetSet::new(vec![target("wss://one.example")]).expect("targets"),
                FetchBounds::new(1, 1).expect("bounds"),
            )
            .expect_err("invalid request id"),
            Error::InvalidFetchRequestId
        );
    }
    for invalid in [" cursor", "cursor ", "cursor\nid"] {
        assert_eq!(
            FetchCursor::parse(invalid).expect_err("invalid cursor"),
            Error::InvalidFetchCursor
        );
    }

    let bounds = FetchBounds::new(FETCH_PAGE_MAX_EVENTS, u64::MAX).expect("maximum bounds");
    assert_eq!(bounds.limit(), FETCH_PAGE_MAX_EVENTS);
    assert_eq!(bounds.deadline_unix_ms(), u64::MAX);
    let cursor = FetchCursor::parse("cursor").expect("cursor");
    assert_eq!(cursor.as_str(), "cursor");
    assert_eq!(cursor.to_string(), "cursor");
}

#[test]
fn selectors_expose_bounds_and_reject_each_nonmatching_dimension() {
    let event = signed_event();
    let other_author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("other author");

    let reversed_since = FetchSelector::all()
        .with_until_unix_seconds(1)
        .expect("until")
        .with_since_unix_seconds(2)
        .expect_err("reversed since");
    assert_eq!(reversed_since, Error::InvalidFetchTimeRange);

    for selector in [
        FetchSelector::all().with_kinds(vec![1]).expect("kinds"),
        FetchSelector::all()
            .with_authors(vec![other_author])
            .expect("authors"),
        FetchSelector::all()
            .with_since_unix_seconds(event.created_at() + 1)
            .expect("since"),
        FetchSelector::all()
            .with_until_unix_seconds(event.created_at() - 1)
            .expect("until"),
    ] {
        assert!(!selector.matches(&event));
    }

    let selector = FetchSelector::all()
        .with_since_unix_seconds(event.created_at())
        .expect("since")
        .with_until_unix_seconds(event.created_at())
        .expect("until");
    assert_eq!(selector.since_unix_seconds(), Some(event.created_at()));
    assert_eq!(selector.until_unix_seconds(), Some(event.created_at()));
    assert!(selector.matches(&event));
}

#[test]
fn page_preserves_cursor_provenance_and_partial_target_outcomes() {
    let first = target("wss://one.example");
    let second = target("wss://two.example");
    let targets = TargetSet::new(vec![first.clone(), second.clone()]).expect("targets");
    let request = request(targets, 2).with_cursor(FetchCursor::parse("page-1").expect("cursor"));
    let provenance = EventProvenance::new(
        TransportId::NOSTR,
        first.fingerprint().clone(),
        1_700_000_000_001,
    )
    .expect("provenance")
    .with_cursor(FetchCursor::parse("event-1").expect("event cursor"));
    let observed = ObservedEvent::new(signed_event(), provenance);
    let outcomes = vec![
        FetchTargetOutcome::new(first.fingerprint().clone(), FetchTargetState::Complete),
        FetchTargetOutcome::new(
            second.fingerprint().clone(),
            FetchTargetState::FailedRetryable,
        )
        .with_message("relay unavailable"),
    ];
    let page = FetchPage::for_request(
        &request,
        vec![observed],
        outcomes,
        NextPage::Cursor(FetchCursor::parse("page-2").expect("next cursor")),
    )
    .expect("page");

    page.validate_for_request(&request)
        .expect("request binding");
    assert_eq!(page.events()[0].event().id_str(), signed_event().id_str());
    assert_eq!(page.events()[0].provenance().target(), first.fingerprint());
    assert_eq!(page.target_outcomes().len(), 2);
    assert!(page.target_outcomes()[1].state().is_retryable());
    assert_eq!(
        page.target_outcomes()[1].message(),
        Some("relay unavailable")
    );
    assert!(matches!(page.next_page(), NextPage::Cursor(cursor) if cursor.as_str() == "page-2"));

    #[cfg(feature = "serde")]
    {
        let encoded = serde_json::to_string(&page).expect("serialize page");
        assert!(!encoded.contains("admission"));
        assert!(!encoded.contains("storage"));
        assert_eq!(
            serde_json::from_str::<FetchPage>(&encoded).expect("deserialize page"),
            page
        );
        let mut invalid_time = serde_json::to_value(&page).expect("page value");
        invalid_time["events"][0]["provenance"]["observed_at_unix_ms"] = 0.into();
        assert!(serde_json::from_value::<FetchPage>(invalid_time).is_err());
    }
}

#[test]
fn pages_reject_oversize_unrequested_and_duplicate_evidence() {
    let requested = target("wss://one.example");
    let foreign = target("wss://foreign.example");
    let request = request(TargetSet::new(vec![requested.clone()]).expect("targets"), 1);
    let observed = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(TransportId::NOSTR, requested.fingerprint().clone(), 1)
            .expect("provenance"),
    );
    assert_eq!(
        FetchPage::for_request(
            &request,
            vec![observed.clone(), observed],
            Vec::new(),
            NextPage::Complete,
        )
        .expect_err("oversized page"),
        Error::FetchPageLimitExceeded
    );

    let foreign_observation = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(TransportId::NOSTR, foreign.fingerprint().clone(), 1)
            .expect("foreign provenance"),
    );
    assert_eq!(
        FetchPage::for_request(
            &request,
            vec![foreign_observation],
            Vec::new(),
            NextPage::Complete,
        )
        .expect_err("foreign provenance"),
        Error::UnexpectedFetchProvenance
    );

    let wrong_transport = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(
            TransportId::parse("future-mesh").expect("custom transport"),
            requested.fingerprint().clone(),
            1,
        )
        .expect("wrong transport provenance"),
    );
    assert_eq!(
        FetchPage::for_request(
            &request,
            vec![wrong_transport],
            Vec::new(),
            NextPage::Complete,
        )
        .expect_err("transport mismatch"),
        Error::UnexpectedFetchProvenance
    );

    let duplicate =
        FetchTargetOutcome::new(requested.fingerprint().clone(), FetchTargetState::Partial);
    assert_eq!(
        FetchPage::for_request(
            &request,
            Vec::new(),
            vec![duplicate.clone(), duplicate],
            NextPage::Cancelled {
                resume_from: Some(FetchCursor::parse("resume").expect("resume cursor")),
            },
        )
        .expect_err("duplicate outcome"),
        Error::DuplicateFetchTargetOutcome
    );
    assert_eq!(
        FetchPage::for_request(
            &request,
            Vec::new(),
            vec![FetchTargetOutcome::new(
                foreign.fingerprint().clone(),
                FetchTargetState::Unavailable,
            )],
            NextPage::Complete,
        )
        .expect_err("foreign outcome"),
        Error::UnexpectedFetchTargetOutcome
    );

    let page = FetchPage::for_request(&request, Vec::new(), Vec::new(), NextPage::Complete)
        .expect("empty page");
    let other_request = FetchRequest::new(
        "other-request",
        request.target_set().clone(),
        request.bounds(),
    )
    .expect("other request");
    assert_eq!(
        page.validate_for_request(&other_request)
            .expect_err("request mismatch"),
        Error::FetchPageRequestMismatch
    );

    let filtered = FetchRequest::new(
        "filtered-request",
        request.target_set().clone(),
        request.bounds(),
    )
    .expect("filtered request")
    .with_selector(
        FetchSelector::all()
            .with_kinds(vec![1])
            .expect("filtered selector"),
    );
    let unexpected = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(TransportId::NOSTR, requested.fingerprint().clone(), 1)
            .expect("provenance"),
    );
    assert_eq!(
        FetchPage::for_request(&filtered, vec![unexpected], Vec::new(), NextPage::Complete)
            .expect_err("selector mismatch"),
        Error::UnexpectedFetchEvent
    );

    assert_eq!(
        EventProvenance::new(TransportId::NOSTR, requested.fingerprint().clone(), 0)
            .expect_err("zero observation time"),
        Error::InvalidObservedAt
    );
}

struct CancellationSource {
    published: Arc<AtomicBool>,
    cancelled_after_publish: Arc<AtomicBool>,
}

struct PublicationGuard(Arc<AtomicBool>);

impl Drop for PublicationGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl EventSource for CancellationSource {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>> {
        Box::pin(async {
            Ok(SourceStatus::new(
                TransportId::NOSTR,
                true,
                Maturity::Stable,
                Availability::Available,
                SourceCapabilities::FETCH,
                "ready",
            ))
        })
    }

    fn fetch(&self, _request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>> {
        let published = Arc::clone(&self.published);
        let cancelled = Arc::clone(&self.cancelled_after_publish);
        Box::pin(async move {
            published.store(true, Ordering::SeqCst);
            let _guard = PublicationGuard(cancelled);
            future::pending::<Result<FetchPage, Error>>().await
        })
    }
}

#[test]
fn dropping_fetch_futures_respects_before_and_after_publication_boundaries() {
    let source = CancellationSource {
        published: Arc::new(AtomicBool::new(false)),
        cancelled_after_publish: Arc::new(AtomicBool::new(false)),
    };
    let targets = TargetSet::new(vec![target("wss://one.example")]).expect("targets");

    let unpolled = source.fetch(request(targets.clone(), 1));
    drop(unpolled);
    assert!(!source.published.load(Ordering::SeqCst));
    assert!(!source.cancelled_after_publish.load(Ordering::SeqCst));

    let mut published = source.fetch(request(targets, 1));
    let mut context = Context::from_waker(noop_waker_ref());
    assert!(Pin::new(&mut published).poll(&mut context).is_pending());
    assert!(source.published.load(Ordering::SeqCst));
    drop(published);
    assert!(source.cancelled_after_publish.load(Ordering::SeqCst));
}
