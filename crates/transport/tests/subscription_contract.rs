use core::{future::Future, pin::Pin, task::Context};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use futures::{executor::block_on, future, task::noop_waker_ref};
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_transport::{
    BoxFuture, BoxSubscription, Error, EventSubscriber, EventSubscription, SubscriptionEnd,
    SubscriptionEndReason, SubscriptionEvent, SubscriptionNext, SubscriptionRequest, Target,
    TargetSet, TransportId,
    source::{
        EventProvenance, FetchCursor, FetchSelector, ObservedEvent, SUBSCRIPTION_MAX_EVENTS,
        SUBSCRIPTION_REQUEST_ID_MAX_BYTES, SubscriptionBounds, SubscriptionCheckpoint,
        SubscriptionRequestId,
    },
};

fn target(uri: &str) -> Target {
    Target::nostr_relay(uri).expect("nostr target")
}

fn signed_event() -> SignedEvent {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let wire = Nip01EventWire::parse_json(raw).expect("wire event");
    SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
}

fn request(targets: TargetSet, limit: u16) -> SubscriptionRequest {
    SubscriptionRequest::new(
        "subscription-request",
        targets,
        SubscriptionBounds::new(limit, 1_700_000_100_000).expect("bounds"),
    )
    .expect("request")
}

#[test]
fn subscription_identity_and_bounds_are_exact_and_bounded() {
    assert_eq!(
        SubscriptionRequestId::parse("").expect_err("empty id"),
        Error::EmptySubscriptionRequestId
    );
    for invalid in [" request", "request ", "request\nid"] {
        assert_eq!(
            SubscriptionRequestId::parse(invalid).expect_err("invalid id"),
            Error::InvalidSubscriptionRequestId
        );
    }
    assert_eq!(
        SubscriptionRequestId::parse("x".repeat(SUBSCRIPTION_REQUEST_ID_MAX_BYTES + 1))
            .expect_err("oversized id"),
        Error::InvalidSubscriptionRequestId
    );
    let maximum = SubscriptionRequestId::parse("x".repeat(SUBSCRIPTION_REQUEST_ID_MAX_BYTES))
        .expect("maximum id");
    assert_eq!(maximum.as_str().len(), SUBSCRIPTION_REQUEST_ID_MAX_BYTES);
    assert_eq!(maximum.to_string(), maximum.as_str());

    assert_eq!(
        SubscriptionBounds::new(0, 1).expect_err("zero limit"),
        Error::InvalidSubscriptionLimit
    );
    assert_eq!(
        SubscriptionBounds::new(SUBSCRIPTION_MAX_EVENTS + 1, 1).expect_err("oversized limit"),
        Error::InvalidSubscriptionLimit
    );
    assert_eq!(
        SubscriptionBounds::new(1, 0).expect_err("zero deadline"),
        Error::InvalidSubscriptionDeadline
    );
    let maximum =
        SubscriptionBounds::new(SUBSCRIPTION_MAX_EVENTS, u64::MAX).expect("maximum bounds");
    assert_eq!(maximum.event_limit(), SUBSCRIPTION_MAX_EVENTS);
    assert_eq!(maximum.deadline_unix_ms(), u64::MAX);
}

#[test]
fn checkpoints_are_bounded_unique_and_canonical_for_the_target_set() {
    let first = target("wss://one.example");
    let second = target("wss://two.example");
    let targets = TargetSet::new(vec![first.clone(), second.clone()]).expect("targets");
    let first_checkpoint = SubscriptionCheckpoint::new(
        first.fingerprint().clone(),
        FetchCursor::parse("first").expect("cursor"),
    );
    let second_checkpoint = SubscriptionCheckpoint::new(
        second.fingerprint().clone(),
        FetchCursor::parse("second").expect("cursor"),
    );
    let configured = request(targets.clone(), 2)
        .with_checkpoints([second_checkpoint.clone(), first_checkpoint.clone()])
        .expect("checkpoints");
    assert_eq!(
        configured.checkpoints(),
        &[first_checkpoint.clone(), second_checkpoint]
    );

    assert_eq!(
        request(targets.clone(), 2)
            .with_checkpoints([first_checkpoint.clone(), first_checkpoint.clone()])
            .expect_err("duplicate"),
        Error::DuplicateSubscriptionCheckpoint
    );
    let foreign = target("wss://foreign.example");
    assert_eq!(
        request(targets.clone(), 2)
            .with_checkpoints([SubscriptionCheckpoint::new(
                foreign.fingerprint().clone(),
                FetchCursor::parse("foreign").expect("cursor"),
            )])
            .expect_err("foreign"),
        Error::UnexpectedSubscriptionCheckpoint
    );
    assert_eq!(
        request(TargetSet::new(vec![first]).expect("targets"), 1)
            .with_checkpoints(core::iter::repeat(first_checkpoint))
            .expect_err("infinite iterator is bounded"),
        Error::SubscriptionCheckpointSetTooLarge
    );
}

#[test]
fn live_events_bind_selector_target_transport_and_checkpoint() {
    let requested = target("wss://one.example");
    let targets = TargetSet::new(vec![requested.clone()]).expect("targets");
    let request = request(targets, 2)
        .with_selector(FetchSelector::all().with_kinds(vec![0]).expect("selector"));
    let cursor = FetchCursor::parse("event-1").expect("cursor");
    let observed = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(
            TransportId::NOSTR,
            requested.fingerprint().clone(),
            1_700_000_000_001,
        )
        .expect("provenance")
        .with_cursor(cursor.clone()),
    );
    let event = SubscriptionEvent::for_request(
        &request,
        observed.clone(),
        SubscriptionCheckpoint::new(requested.fingerprint().clone(), cursor),
    )
    .expect("event");
    event
        .validate_for_request(&request)
        .expect("request binding");
    assert_eq!(event.request_id(), request.request_id());
    assert_eq!(event.observed().event().id_str(), signed_event().id_str());

    assert_eq!(
        SubscriptionEvent::for_request(
            &request,
            observed,
            SubscriptionCheckpoint::new(
                requested.fingerprint().clone(),
                FetchCursor::parse("different").expect("cursor"),
            ),
        )
        .expect_err("cursor mismatch"),
        Error::SubscriptionEventCheckpointMismatch
    );

    let filtered =
        request.with_selector(FetchSelector::all().with_kinds(vec![1]).expect("selector"));
    let observed = ObservedEvent::new(
        signed_event(),
        EventProvenance::new(TransportId::NOSTR, requested.fingerprint().clone(), 1)
            .expect("provenance")
            .with_cursor(FetchCursor::parse("event-2").expect("cursor")),
    );
    assert_eq!(
        SubscriptionEvent::for_request(
            &filtered,
            observed,
            SubscriptionCheckpoint::new(
                requested.fingerprint().clone(),
                FetchCursor::parse("event-2").expect("cursor"),
            ),
        )
        .expect_err("selector mismatch"),
        Error::UnexpectedSubscriptionEvent
    );
}

struct StableSubscription {
    request: SubscriptionRequest,
    terminal: SubscriptionEnd,
}

impl EventSubscription for StableSubscription {
    fn request(&self) -> &SubscriptionRequest {
        &self.request
    }

    fn next(&mut self) -> BoxFuture<'_, Result<SubscriptionNext, Error>> {
        let terminal = self.terminal.clone();
        Box::pin(async move { Ok(SubscriptionNext::End(terminal)) })
    }

    fn cancel(&mut self) -> BoxFuture<'_, Result<SubscriptionEnd, Error>> {
        let terminal = self.terminal.clone();
        Box::pin(async move { Ok(terminal) })
    }
}

struct StableSubscriber;

impl EventSubscriber for StableSubscriber {
    fn subscribe(
        &self,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<BoxSubscription, Error>> {
        Box::pin(async move {
            let terminal =
                SubscriptionEnd::for_request(&request, 0, [], SubscriptionEndReason::SourceClosed)?;
            Ok(Box::new(StableSubscription { request, terminal }) as BoxSubscription)
        })
    }
}

#[test]
fn subscription_spi_is_dyn_compatible_and_terminal_results_are_idempotent() {
    let subscriber: &dyn EventSubscriber = &StableSubscriber;
    let targets = TargetSet::new(vec![target("wss://one.example")]).expect("targets");
    let request = request(targets, 1);
    let mut subscription = block_on(subscriber.subscribe(request.clone())).expect("subscribe");
    assert_eq!(subscription.request(), &request);

    let first = block_on(subscription.next()).expect("next");
    let second = block_on(subscription.next()).expect("next again");
    let cancelled = block_on(subscription.cancel()).expect("cancel after end");
    assert_eq!(first, second);
    assert_eq!(first, SubscriptionNext::End(cancelled.clone()));
    assert_eq!(cancelled.reason(), SubscriptionEndReason::SourceClosed);
    assert_eq!(cancelled.event_count(), 0);
    assert!(cancelled.checkpoints().is_empty());
    cancelled
        .validate_for_request(&request)
        .expect("request-bound end");
    assert_eq!(cancelled.request(), &request);

    assert_eq!(
        SubscriptionEnd::for_request(&request, 2, [], SubscriptionEndReason::EventLimit,)
            .expect_err("event limit exceeded"),
        Error::SubscriptionEndLimitExceeded
    );
    assert_eq!(
        SubscriptionEnd::for_request(&request, 0, [], SubscriptionEndReason::EventLimit,)
            .expect_err("event limit reason requires the exact limit"),
        Error::InvalidSubscriptionEnd
    );

    let other = SubscriptionRequest::new(
        "other-request",
        request.target_set().clone(),
        request.bounds(),
    )
    .expect("other request");
    assert_eq!(
        cancelled
            .validate_for_request(&other)
            .expect_err("request mismatch"),
        Error::SubscriptionEndRequestMismatch
    );

    for (reason, event_count) in [
        (SubscriptionEndReason::EventLimit, 1),
        (SubscriptionEndReason::Deadline, 0),
        (SubscriptionEndReason::Cancelled, 0),
        (SubscriptionEndReason::SourceClosed, 0),
    ] {
        assert_eq!(
            SubscriptionEnd::for_request(&request, event_count, [], reason)
                .expect("terminal reason")
                .reason(),
            reason
        );
    }
}

struct CancellationGuard(Arc<AtomicBool>);

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct PendingSubscription {
    request: SubscriptionRequest,
    terminal: SubscriptionEnd,
    cancellation_observed: Arc<AtomicBool>,
}

impl EventSubscription for PendingSubscription {
    fn request(&self) -> &SubscriptionRequest {
        &self.request
    }

    fn next(&mut self) -> BoxFuture<'_, Result<SubscriptionNext, Error>> {
        let cancellation_observed = Arc::clone(&self.cancellation_observed);
        Box::pin(async move {
            let _guard = CancellationGuard(cancellation_observed);
            future::pending::<Result<SubscriptionNext, Error>>().await
        })
    }

    fn cancel(&mut self) -> BoxFuture<'_, Result<SubscriptionEnd, Error>> {
        let terminal = self.terminal.clone();
        Box::pin(async move { Ok(terminal) })
    }
}

#[test]
fn dropping_a_polled_subscription_future_requests_cancellation() {
    let targets = TargetSet::new(vec![target("wss://one.example")]).expect("targets");
    let request = request(targets, 1);
    let terminal = SubscriptionEnd::for_request(&request, 0, [], SubscriptionEndReason::Cancelled)
        .expect("terminal");
    let cancellation_observed = Arc::new(AtomicBool::new(false));
    let mut subscription = PendingSubscription {
        request,
        terminal,
        cancellation_observed: Arc::clone(&cancellation_observed),
    };

    let unpolled = subscription.next();
    drop(unpolled);
    assert!(!cancellation_observed.load(Ordering::SeqCst));

    let mut pending = subscription.next();
    let mut context = Context::from_waker(noop_waker_ref());
    assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
    drop(pending);
    assert!(cancellation_observed.load(Ordering::SeqCst));
    assert_eq!(
        block_on(subscription.cancel()).expect("cancel").reason(),
        SubscriptionEndReason::Cancelled
    );
}

#[cfg(feature = "serde")]
#[test]
fn subscription_wire_models_revalidate_bounds_and_request_binding() {
    let requested = target("wss://one.example");
    let request = request(TargetSet::new(vec![requested.clone()]).expect("targets"), 1);
    let cursor = FetchCursor::parse("event-1").expect("cursor");
    let event = SubscriptionEvent::for_request(
        &request,
        ObservedEvent::new(
            signed_event(),
            EventProvenance::new(TransportId::NOSTR, requested.fingerprint().clone(), 1)
                .expect("provenance")
                .with_cursor(cursor.clone()),
        ),
        SubscriptionCheckpoint::new(requested.fingerprint().clone(), cursor),
    )
    .expect("event");
    let encoded = serde_json::to_string(&SubscriptionNext::Event(Box::new(event.clone())))
        .expect("serialize event");
    assert_eq!(
        serde_json::from_str::<SubscriptionNext>(&encoded).expect("deserialize event"),
        SubscriptionNext::Event(Box::new(event))
    );

    let mut invalid = serde_json::to_value(&request).expect("request value");
    invalid["bounds"]["event_limit"] = 0.into();
    assert!(serde_json::from_value::<SubscriptionRequest>(invalid).is_err());

    let checkpoint = serde_json::to_value(SubscriptionCheckpoint::new(
        requested.fingerprint().clone(),
        FetchCursor::parse("checkpoint").expect("cursor"),
    ))
    .expect("checkpoint value");
    let mut oversized = serde_json::to_value(&request).expect("request value");
    oversized["checkpoints"] = serde_json::Value::Array(vec![
        checkpoint;
        radroots_transport::TARGET_SET_MAX_ITEMS
            + 1
    ]);
    assert!(serde_json::from_value::<SubscriptionRequest>(oversized).is_err());

    let terminal = SubscriptionEnd::for_request(&request, 1, [], SubscriptionEndReason::Deadline)
        .expect("terminal");
    let mut unknown = serde_json::to_value(&terminal).expect("terminal value");
    unknown["unknown"] = true.into();
    assert!(serde_json::from_value::<SubscriptionEnd>(unknown).is_err());
}
