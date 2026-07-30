use futures::executor::block_on;
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, EventSink, EventSource, FetchPage, FetchRequest,
    RadrootsTransportOutcome, RadrootsTransportOutcomeKind, RadrootsTransportPayload,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportTarget, RadrootsTransportTargetReceipt,
    RadrootsTransportTargetSet, SinkStatus, SourceStatus, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
};

struct SourceOnly;

impl EventSource for SourceOnly {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, radroots_transport::Error>> {
        Box::pin(async { Ok(source_status()) })
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> BoxFuture<'_, Result<FetchPage, radroots_transport::Error>> {
        Box::pin(async move { Ok(FetchPage::new(request.request_id, Vec::new(), 0)) })
    }
}

struct SinkOnly;

impl EventSink for SinkOnly {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, radroots_transport::Error>> {
        Box::pin(async { Ok(sink_status()) })
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::Error>> {
        Box::pin(async move {
            let receipts = request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| {
                    RadrootsTransportTargetReceipt::new(
                        target,
                        RadrootsTransportOutcome::new(RadrootsTransportOutcomeKind::Delivered),
                    )
                })
                .collect();
            DeliveryReceipt::for_request(&request, receipts)
        })
    }
}

struct Bidirectional {
    source: SourceOnly,
    sink: SinkOnly,
}

impl EventSource for Bidirectional {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, radroots_transport::Error>> {
        EventSource::status(&self.source)
    }

    fn fetch(
        &self,
        request: FetchRequest,
    ) -> BoxFuture<'_, Result<FetchPage, radroots_transport::Error>> {
        self.source.fetch(request)
    }
}

impl EventSink for Bidirectional {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, radroots_transport::Error>> {
        EventSink::status(&self.sink)
    }

    fn deliver(
        &self,
        request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::Error>> {
        self.sink.deliver(request)
    }
}

fn source_status() -> SourceStatus {
    SourceStatus::new(
        TransportId::LOCAL,
        true,
        Maturity::Stable,
        Availability::Available,
        SourceCapabilities::FETCH,
        "source ready",
    )
}

fn sink_status() -> SinkStatus {
    SinkStatus::new(
        TransportId::LOCAL,
        true,
        Maturity::Stable,
        Availability::Available,
        SinkCapabilities::DELIVER,
        "sink ready",
    )
}

fn target_set() -> RadrootsTransportTargetSet {
    RadrootsTransportTargetSet::new(vec![
        RadrootsTransportTarget::local("local:spi").expect("local target"),
    ])
    .expect("target set")
}

fn assert_source_dyn_compatible(_: &dyn EventSource) {}
fn assert_sink_dyn_compatible(_: &dyn EventSink) {}

#[test]
fn source_only_and_sink_only_implementations_are_independently_dispatchable() {
    let source = SourceOnly;
    let sink = SinkOnly;
    assert_source_dyn_compatible(&source);
    assert_sink_dyn_compatible(&sink);

    let source_status = block_on(EventSource::status(&source)).expect("source status");
    assert!(source_status.capabilities().can_fetch());
    let page =
        block_on(source.fetch(FetchRequest::new("fetch-1", target_set()))).expect("fetch page");
    assert_eq!(page.request_id, "fetch-1");

    let sink_status = block_on(EventSink::status(&sink)).expect("sink status");
    assert!(sink_status.capabilities().can_deliver());
    let receipt = block_on(
        sink.deliver(
            DeliveryRequest::new(
                "deliver-1",
                RadrootsTransportPayload::opaque_bytes("spi", [1]).expect("payload"),
                target_set(),
                RadrootsTransportSatisfactionPolicy::all_delivered(),
            )
            .expect("delivery request"),
        ),
    )
    .expect("delivery receipt");
    assert_eq!(receipt.request_id(), "deliver-1");
}

#[test]
fn a_bidirectional_adapter_exposes_both_dyn_contracts() {
    let adapter = Bidirectional {
        source: SourceOnly,
        sink: SinkOnly,
    };
    assert_source_dyn_compatible(&adapter);
    assert_sink_dyn_compatible(&adapter);

    assert!(
        block_on(EventSource::status(&adapter))
            .expect("source status")
            .capabilities()
            .can_fetch()
    );
    assert!(
        block_on(EventSink::status(&adapter))
            .expect("sink status")
            .capabilities()
            .can_deliver()
    );
}
