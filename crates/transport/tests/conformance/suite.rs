use core::{future::Future, pin::Pin, task::Context};
use futures::{executor::block_on, task::noop_waker_ref};
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_transport::{
    DeliveryRequest, Error, EventSink, EventSource, FetchRequest, SinkStatus, SourceStatus, Target,
    TargetSet,
    outcome::FetchTargetState,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DELIVERY_REQUEST_ID_MAX_BYTES, DeliveryPayload},
    source::{FETCH_PAGE_MAX_EVENTS, FETCH_REQUEST_ID_MAX_BYTES, FetchBounds, FetchCursor},
};

pub(crate) const NOW_UNIX_MS: u64 = 1_700_000_000_000;

pub(crate) trait SourceConformanceHarness {
    fn source(&self) -> &dyn EventSource;
    fn expected_status(&self) -> SourceStatus;
    fn target_set(&self) -> TargetSet;
    fn now_unix_ms(&self) -> u64;
    fn captured_request(&self) -> Option<FetchRequest>;
    fn published(&self) -> bool;
    fn cancelled_after_publish(&self) -> bool;
}

pub(crate) trait SinkConformanceHarness {
    fn sink(&self) -> &dyn EventSink;
    fn expected_status(&self) -> SinkStatus;
    fn target_set(&self) -> TargetSet;
    fn now_unix_ms(&self) -> u64;
    fn captured_request(&self) -> Option<DeliveryRequest>;
    fn published(&self) -> bool;
    fn cancelled_after_publish(&self) -> bool;
}

fn fetch_request(id: &str, targets: TargetSet, deadline: u64) -> FetchRequest {
    FetchRequest::new(
        id,
        targets,
        FetchBounds::new(2, deadline).expect("fetch bounds"),
    )
    .expect("fetch request")
    .with_cursor(FetchCursor::parse("opaque-cursor").expect("cursor"))
}

fn delivery_request(id: &str, targets: TargetSet, deadline: u64) -> DeliveryRequest {
    DeliveryRequest::new(
        id,
        DeliveryPayload::new(signed_event()),
        targets,
        SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::any()),
        deadline,
    )
    .expect("delivery request")
}

pub(crate) fn assert_source_conformance(harness: &impl SourceConformanceHarness) {
    let source = harness.source();
    let status = block_on(source.status()).expect("source status");
    assert_eq!(status, harness.expected_status());
    assert!(status.is_configured());
    assert!(status.capabilities().can_fetch());
    assert!(!status.message().is_empty());

    let request = fetch_request(
        "source-conformance",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    );
    let page = block_on(source.fetch(request.clone())).expect("fetch page");
    page.validate_for_request(&request)
        .expect("request binding");
    assert_eq!(page.request_id().as_str(), request.request_id().as_str());
    assert!(page.events().len() <= usize::from(request.bounds().limit()));
    assert_eq!(page.target_outcomes().len(), request.target_set().len());
    assert_eq!(
        page.target_outcomes()[0].target(),
        request.target_set().targets()[0].fingerprint()
    );
    assert_eq!(
        page.target_outcomes()[0].state(),
        FetchTargetState::Complete
    );
    assert_eq!(
        page.target_outcomes()[1].target(),
        request.target_set().targets()[1].fingerprint()
    );
    assert!(page.target_outcomes()[1].state().is_retryable());
    assert_eq!(harness.captured_request().as_ref(), Some(&request));

    let expired = fetch_request(
        "source-expired",
        harness.target_set(),
        harness.now_unix_ms(),
    );
    assert_eq!(
        block_on(source.fetch(expired)).expect_err("expired fetch"),
        Error::InvalidFetchDeadline
    );
}

pub(crate) fn assert_sink_conformance(harness: &impl SinkConformanceHarness) {
    let sink = harness.sink();
    let status = block_on(sink.status()).expect("sink status");
    assert_eq!(status, harness.expected_status());
    assert!(status.is_configured());
    assert!(status.capabilities().can_deliver());
    assert!(!status.message().is_empty());

    let request = delivery_request(
        "sink-conformance",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    );
    let receipt = block_on(sink.deliver(request.clone())).expect("delivery receipt");
    receipt
        .validate_for_request(&request)
        .expect("request binding");
    assert_eq!(receipt.request_id().as_str(), request.request_id().as_str());
    assert_eq!(receipt.target_receipts().len(), request.target_set().len());
    for (target_receipt, requested_target) in receipt
        .target_receipts()
        .iter()
        .zip(request.target_set().targets())
    {
        assert_eq!(target_receipt.target(), requested_target);
    }
    assert!(
        receipt.target_receipts()[0]
            .outcome()
            .satisfies(SatisfactionClass::Delivered)
    );
    assert!(receipt.target_receipts()[1].outcome().is_retryable());
    assert!(receipt.is_satisfied(&request).expect("satisfaction"));
    assert_eq!(harness.captured_request().as_ref(), Some(&request));

    let expired = delivery_request("sink-expired", harness.target_set(), harness.now_unix_ms());
    let failure = block_on(sink.deliver(expired)).expect_err("expired delivery");
    assert_eq!(failure.code(), "invalid_transport_contract");
}

pub(crate) fn assert_request_boundaries() {
    assert_eq!(
        FetchBounds::new(0, 1).expect_err("zero fetch bound"),
        Error::InvalidFetchLimit
    );
    assert_eq!(
        FetchBounds::new(FETCH_PAGE_MAX_EVENTS + 1, 1).expect_err("oversized fetch bound"),
        Error::InvalidFetchLimit
    );
    assert_eq!(
        FetchRequest::new(
            "x".repeat(FETCH_REQUEST_ID_MAX_BYTES + 1),
            target_set(),
            FetchBounds::new(1, 1).expect("fetch bounds"),
        )
        .expect_err("oversized fetch request id"),
        Error::InvalidFetchRequestId
    );
    assert_eq!(
        DeliveryRequest::new(
            "x".repeat(DELIVERY_REQUEST_ID_MAX_BYTES + 1),
            DeliveryPayload::new(signed_event()),
            target_set(),
            SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all()),
            1,
        )
        .expect_err("oversized delivery request id"),
        Error::InvalidDeliveryRequestId
    );
}

pub(crate) fn assert_source_error(harness: &impl SourceConformanceHarness, expected: Error) {
    let request = fetch_request(
        "source-error",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    );
    assert_eq!(
        block_on(harness.source().fetch(request)).expect_err("source error"),
        expected
    );
}

pub(crate) fn assert_sink_error(harness: &impl SinkConformanceHarness, _expected: Error) {
    let request = delivery_request(
        "sink-error",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    );
    let failure = block_on(harness.sink().deliver(request)).expect_err("sink error");
    assert_eq!(failure.code(), "invalid_transport_contract");
    assert!(matches!(
        failure.retryability(),
        radroots_transport::outcome::Retryability::Terminal
    ));
}

pub(crate) fn assert_source_cancellation(harness: &impl SourceConformanceHarness) {
    let source = harness.source();
    let unpolled = source.fetch(fetch_request(
        "source-unpolled",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    ));
    drop(unpolled);
    assert!(!harness.published());
    assert!(!harness.cancelled_after_publish());

    let mut published = source.fetch(fetch_request(
        "source-published",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    ));
    let mut context = Context::from_waker(noop_waker_ref());
    assert!(Pin::new(&mut published).poll(&mut context).is_pending());
    assert!(harness.published());
    drop(published);
    assert!(harness.cancelled_after_publish());
}

pub(crate) fn assert_sink_cancellation(harness: &impl SinkConformanceHarness) {
    let sink = harness.sink();
    let unpolled = sink.deliver(delivery_request(
        "sink-unpolled",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    ));
    drop(unpolled);
    assert!(!harness.published());
    assert!(!harness.cancelled_after_publish());

    let mut published = sink.deliver(delivery_request(
        "sink-published",
        harness.target_set(),
        harness.now_unix_ms() + 100,
    ));
    let mut context = Context::from_waker(noop_waker_ref());
    assert!(Pin::new(&mut published).poll(&mut context).is_pending());
    assert!(harness.published());
    drop(published);
    assert!(harness.cancelled_after_publish());
}

fn target_set() -> TargetSet {
    TargetSet::new(vec![
        Target::local("local:conformance-a").expect("first target"),
        Target::local("local:conformance-b").expect("second target"),
    ])
    .expect("target set")
}

fn signed_event() -> SignedEvent {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let wire = Nip01EventWire::parse_json(raw).expect("wire event");
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}
