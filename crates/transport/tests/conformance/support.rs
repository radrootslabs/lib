use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use futures::future;
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, Error, EventSink, EventSource, FetchPage,
    FetchRequest, SinkFailure, SinkStatus, SourceStatus, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    outcome::{DeliveryOutcome, FetchTargetOutcome, FetchTargetState},
    sink::DeliveryTargetReceipt,
    source::NextPage,
};

use crate::suite::{NOW_UNIX_MS, SinkConformanceHarness, SourceConformanceHarness};

#[derive(Clone)]
enum Mode {
    Success,
    Fail(Error),
    Pending,
}

#[derive(Default)]
pub(crate) struct SourceState {
    request: Mutex<Option<FetchRequest>>,
    pub(crate) published: AtomicBool,
    pub(crate) cancelled_after_publish: AtomicBool,
}

impl SourceState {
    pub(crate) fn request(&self) -> Option<FetchRequest> {
        self.request.lock().expect("source request lock").clone()
    }
}

#[derive(Default)]
pub(crate) struct SinkState {
    request: Mutex<Option<DeliveryRequest>>,
    pub(crate) published: AtomicBool,
    pub(crate) cancelled_after_publish: AtomicBool,
}

impl SinkState {
    pub(crate) fn request(&self) -> Option<DeliveryRequest> {
        self.request.lock().expect("sink request lock").clone()
    }
}

pub(crate) struct MockSource {
    mode: Mode,
    state: Arc<SourceState>,
}

impl MockSource {
    pub(crate) fn successful() -> Self {
        Self::new(Mode::Success)
    }

    pub(crate) fn failing(error: Error) -> Self {
        Self::new(Mode::Fail(error))
    }

    pub(crate) fn pending() -> Self {
        Self::new(Mode::Pending)
    }

    fn new(mode: Mode) -> Self {
        Self {
            mode,
            state: Arc::new(SourceState::default()),
        }
    }
}

impl EventSource for MockSource {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>> {
        Box::pin(async { Ok(source_status()) })
    }

    fn fetch(&self, request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>> {
        let state = Arc::clone(&self.state);
        let mode = self.mode.clone();
        Box::pin(async move {
            *state.request.lock().expect("source request lock") = Some(request.clone());
            if request.bounds().deadline_unix_ms() <= NOW_UNIX_MS {
                return Err(Error::InvalidFetchDeadline);
            }
            match mode {
                Mode::Fail(error) => Err(error),
                Mode::Pending => {
                    state.published.store(true, Ordering::SeqCst);
                    let state_for_drop = Arc::clone(&state);
                    let _publish_guard = ScopeGuard::new(move || {
                        state_for_drop
                            .cancelled_after_publish
                            .store(true, Ordering::SeqCst);
                    });
                    future::pending().await
                }
                Mode::Success => {
                    let outcomes = request
                        .target_set()
                        .targets()
                        .iter()
                        .enumerate()
                        .map(|(index, target)| {
                            FetchTargetOutcome::new(
                                target.fingerprint().clone(),
                                if index == 0 {
                                    FetchTargetState::Complete
                                } else {
                                    FetchTargetState::FailedRetryable
                                },
                            )
                        })
                        .collect();
                    FetchPage::for_request(&request, Vec::new(), outcomes, NextPage::Complete)
                }
            }
        })
    }
}

impl SourceConformanceHarness for MockSource {
    fn source(&self) -> &dyn EventSource {
        self
    }

    fn expected_status(&self) -> SourceStatus {
        source_status()
    }

    fn target_set(&self) -> radroots_transport::TargetSet {
        target_set()
    }

    fn now_unix_ms(&self) -> u64 {
        NOW_UNIX_MS
    }

    fn captured_request(&self) -> Option<FetchRequest> {
        self.state.request()
    }

    fn published(&self) -> bool {
        self.state.published.load(Ordering::SeqCst)
    }

    fn cancelled_after_publish(&self) -> bool {
        self.state.cancelled_after_publish.load(Ordering::SeqCst)
    }
}

pub(crate) struct MockSink {
    mode: Mode,
    state: Arc<SinkState>,
}

impl MockSink {
    pub(crate) fn successful() -> Self {
        Self::new(Mode::Success)
    }

    pub(crate) fn failing(error: Error) -> Self {
        Self::new(Mode::Fail(error))
    }

    pub(crate) fn pending() -> Self {
        Self::new(Mode::Pending)
    }

    fn new(mode: Mode) -> Self {
        Self {
            mode,
            state: Arc::new(SinkState::default()),
        }
    }
}

impl EventSink for MockSink {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>> {
        Box::pin(async { Ok(sink_status()) })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        let state = Arc::clone(&self.state);
        let mode = self.mode.clone();
        Box::pin(async move {
            *state.request.lock().expect("sink request lock") = Some(request.clone());
            if request.deadline_unix_ms() <= NOW_UNIX_MS {
                return Err(SinkFailure::invalid_contract(&request));
            }
            match mode {
                Mode::Fail(_) => Err(SinkFailure::invalid_contract(&request)),
                Mode::Pending => {
                    state.published.store(true, Ordering::SeqCst);
                    let state_for_drop = Arc::clone(&state);
                    let _publish_guard = ScopeGuard::new(move || {
                        state_for_drop
                            .cancelled_after_publish
                            .store(true, Ordering::SeqCst);
                    });
                    future::pending().await
                }
                Mode::Success => {
                    let receipts = request
                        .target_set()
                        .targets()
                        .iter()
                        .enumerate()
                        .map(|(index, target)| {
                            DeliveryTargetReceipt::attempted(
                                target.clone(),
                                if index == 0 {
                                    DeliveryOutcome::delivered()
                                } else {
                                    DeliveryOutcome::unavailable()
                                },
                            )
                        })
                        .collect();
                    DeliveryReceipt::for_request(&request, receipts)
                        .map_err(|_| SinkFailure::invalid_contract(&request))
                }
            }
        })
    }
}

impl SinkConformanceHarness for MockSink {
    fn sink(&self) -> &dyn EventSink {
        self
    }

    fn expected_status(&self) -> SinkStatus {
        sink_status()
    }

    fn target_set(&self) -> radroots_transport::TargetSet {
        target_set()
    }

    fn now_unix_ms(&self) -> u64 {
        NOW_UNIX_MS
    }

    fn captured_request(&self) -> Option<DeliveryRequest> {
        self.state.request()
    }

    fn published(&self) -> bool {
        self.state.published.load(Ordering::SeqCst)
    }

    fn cancelled_after_publish(&self) -> bool {
        self.state.cancelled_after_publish.load(Ordering::SeqCst)
    }
}

struct ScopeGuard<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> ScopeGuard<F> {
    fn new(callback: F) -> Self {
        Self(Some(callback))
    }
}

impl<F: FnOnce()> Drop for ScopeGuard<F> {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

pub(crate) struct CombinedAdapter {
    source: MockSource,
    sink: MockSink,
}

impl CombinedAdapter {
    pub(crate) fn successful() -> Self {
        Self {
            source: MockSource::successful(),
            sink: MockSink::successful(),
        }
    }
}

impl EventSource for CombinedAdapter {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>> {
        EventSource::status(&self.source)
    }

    fn fetch(&self, request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>> {
        self.source.fetch(request)
    }
}

impl EventSink for CombinedAdapter {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>> {
        EventSink::status(&self.sink)
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, SinkFailure>> {
        self.sink.deliver(request)
    }
}

impl SourceConformanceHarness for CombinedAdapter {
    fn source(&self) -> &dyn EventSource {
        self
    }

    fn expected_status(&self) -> SourceStatus {
        source_status()
    }

    fn target_set(&self) -> radroots_transport::TargetSet {
        target_set()
    }

    fn now_unix_ms(&self) -> u64 {
        NOW_UNIX_MS
    }

    fn captured_request(&self) -> Option<FetchRequest> {
        self.source.state.request()
    }

    fn published(&self) -> bool {
        self.source.state.published.load(Ordering::SeqCst)
    }

    fn cancelled_after_publish(&self) -> bool {
        self.source
            .state
            .cancelled_after_publish
            .load(Ordering::SeqCst)
    }
}

impl SinkConformanceHarness for CombinedAdapter {
    fn sink(&self) -> &dyn EventSink {
        self
    }

    fn expected_status(&self) -> SinkStatus {
        sink_status()
    }

    fn target_set(&self) -> radroots_transport::TargetSet {
        target_set()
    }

    fn now_unix_ms(&self) -> u64 {
        NOW_UNIX_MS
    }

    fn captured_request(&self) -> Option<DeliveryRequest> {
        self.sink.state.request()
    }

    fn published(&self) -> bool {
        self.sink.state.published.load(Ordering::SeqCst)
    }

    fn cancelled_after_publish(&self) -> bool {
        self.sink
            .state
            .cancelled_after_publish
            .load(Ordering::SeqCst)
    }
}

fn target_set() -> radroots_transport::TargetSet {
    radroots_transport::TargetSet::new(vec![
        radroots_transport::Target::local("local:conformance-a").expect("first target"),
        radroots_transport::Target::local("local:conformance-b").expect("second target"),
    ])
    .expect("target set")
}

fn source_status() -> SourceStatus {
    SourceStatus::new(
        TransportId::LOCAL,
        true,
        Maturity::Stable,
        Availability::Available,
        SourceCapabilities::FETCH,
        "mock source ready",
    )
}

fn sink_status() -> SinkStatus {
    SinkStatus::new(
        TransportId::LOCAL,
        true,
        Maturity::Stable,
        Availability::Available,
        SinkCapabilities::DELIVER,
        "mock sink ready",
    )
}
