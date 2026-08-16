use radroots_transport::{
    BoxSubscription, DeliveryReceipt, DeliveryRequest, Error, EventSink, EventSource,
    EventSubscriber, FetchPage, FetchRequest, SubscriptionRequest, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    sink::SinkStatus,
    source::{BoxFuture, SourceStatus},
};

struct HostTransport;

impl EventSource for HostTransport {
    fn status(&self) -> BoxFuture<'_, Result<SourceStatus, Error>> {
        Box::pin(async {
            Ok(SourceStatus::new(
                TransportId::LOCAL,
                true,
                Maturity::Preview,
                Availability::Unavailable,
                SourceCapabilities::NONE,
                "host source is not connected",
            ))
        })
    }

    fn fetch(&self, _request: FetchRequest) -> BoxFuture<'_, Result<FetchPage, Error>> {
        Box::pin(async { Err(Error::UnsupportedOperation) })
    }
}

impl EventSubscriber for HostTransport {
    fn subscribe(
        &self,
        _request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<BoxSubscription, Error>> {
        Box::pin(async { Err(Error::UnsupportedOperation) })
    }
}

impl EventSink for HostTransport {
    fn status(&self) -> BoxFuture<'_, Result<SinkStatus, Error>> {
        Box::pin(async {
            Ok(SinkStatus::new(
                TransportId::LOCAL,
                true,
                Maturity::Preview,
                Availability::Unavailable,
                SinkCapabilities::NONE,
                "host sink is not connected",
            ))
        })
    }

    fn deliver(
        &self,
        _request: DeliveryRequest,
    ) -> BoxFuture<'_, Result<DeliveryReceipt, radroots_transport::SinkFailure>> {
        Box::pin(async move { Err(radroots_transport::SinkFailure::invalid_contract(&_request)) })
    }
}

fn main() {
    let transport = HostTransport;
    let source: &dyn EventSource = &transport;
    let subscriber: &dyn EventSubscriber = &transport;
    let sink: &dyn EventSink = &transport;

    let future = source.status();
    drop(future); // The composing host chooses and drives its async executor.
    let _ = subscriber;
    let future = sink.status();
    drop(future);
}
