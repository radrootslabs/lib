use futures::executor::block_on;
use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
use radroots_transport::{
    BoxFuture, DeliveryReceipt, DeliveryRequest, EventSink, EventSource, FetchPage, FetchRequest,
    SinkStatus, SourceStatus, Target, TargetSet, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    outcome::DeliveryOutcome,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::{DeliveryPayload, DeliveryTargetReceipt},
    source::{FetchBounds, NextPage},
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
        Box::pin(async move {
            FetchPage::for_request(&request, Vec::new(), Vec::new(), NextPage::Complete)
        })
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
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::SinkFailure>> {
        Box::pin(async move {
            let receipts = request
                .target_set()
                .targets()
                .iter()
                .cloned()
                .map(|target| {
                    DeliveryTargetReceipt::attempted(target, DeliveryOutcome::delivered())
                })
                .collect();
            DeliveryReceipt::for_request(&request, receipts)
                .map_err(|_| radroots_transport::SinkFailure::invalid_contract(&request))
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
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::SinkFailure>> {
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

fn target_set() -> TargetSet {
    TargetSet::new(vec![Target::local("local:spi").expect("local target")]).expect("target set")
}

fn delivery_payload() -> DeliveryPayload {
    let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
    let wire = Nip01EventWire::parse_json(raw).expect("wire event");
    DeliveryPayload::new(
        SignedEvent::from_wire_verified_id(wire, raw).expect("signed delivery event"),
    )
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
    let request = FetchRequest::new(
        "fetch-1",
        target_set(),
        FetchBounds::new(10, 1_700_000_000_000).expect("fetch bounds"),
    )
    .expect("fetch request");
    let page = block_on(source.fetch(request)).expect("fetch page");
    assert_eq!(page.request_id().as_str(), "fetch-1");

    let sink_status = block_on(EventSink::status(&sink)).expect("sink status");
    assert!(sink_status.capabilities().can_deliver());
    let receipt = block_on(
        sink.deliver(
            DeliveryRequest::new(
                "deliver-1",
                delivery_payload(),
                target_set(),
                SatisfactionPolicy::new(SatisfactionClass::Delivered, TargetPolicy::all()),
                1_700_000_100_000,
            )
            .expect("delivery request"),
        ),
    )
    .expect("delivery receipt");
    assert_eq!(receipt.request_id().as_str(), "deliver-1");
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
