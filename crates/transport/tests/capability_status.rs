use radroots_transport::{
    SinkStatus, SourceStatus, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
};

#[test]
fn maturity_and_availability_round_trip_through_protocol_dtos() {
    for maturity in [Maturity::Experimental, Maturity::Preview, Maturity::Stable] {
        let protocol = radroots_protocol::capability::v1::Maturity::from(maturity);
        assert_eq!(Maturity::from(protocol), maturity);
    }

    for availability in [
        Availability::Available,
        Availability::Degraded,
        Availability::Unavailable,
    ] {
        let protocol = radroots_protocol::capability::v1::Availability::from(availability);
        assert_eq!(Availability::from(protocol), availability);
    }
}

#[test]
fn runtime_state_dimensions_are_independent() {
    let transport_id = TransportId::parse("future-mesh").expect("custom transport");
    let states = [
        (false, Maturity::Experimental, Availability::Available),
        (true, Maturity::Preview, Availability::Degraded),
        (true, Maturity::Stable, Availability::Unavailable),
    ];

    for (configured, maturity, availability) in states {
        let status = SourceStatus::new(
            transport_id,
            configured,
            maturity,
            availability,
            SourceCapabilities::NONE,
            "explicit state",
        );
        assert_eq!(status.transport_id(), transport_id);
        assert_eq!(status.is_configured(), configured);
        assert_eq!(status.maturity(), maturity);
        assert_eq!(status.availability(), availability);
        assert_eq!(status.message(), "explicit state");
    }
}

#[test]
fn source_and_sink_capabilities_are_separate_contracts() {
    let source = SourceStatus::new(
        TransportId::NOSTR,
        true,
        Maturity::Stable,
        Availability::Available,
        SourceCapabilities::FETCH.with_discovery(true),
        "source",
    );
    assert!(source.capabilities().can_fetch());
    assert!(source.capabilities().can_discover());
    assert_eq!(source.transport_id(), TransportId::NOSTR);

    let sink = SinkStatus::new(
        TransportId::NOSTR,
        true,
        Maturity::Stable,
        Availability::Available,
        SinkCapabilities::DELIVER
            .with_gateway_forwarding(true)
            .with_receipt_observation(true),
        "sink",
    );
    assert!(sink.capabilities().can_deliver());
    assert!(sink.capabilities().can_gateway_forward());
    assert!(sink.capabilities().can_observe_receipts());
    assert_eq!(sink.transport_id(), TransportId::NOSTR);
}

#[test]
fn protocol_descriptors_preserve_unknown_transports_and_split_capabilities() {
    use radroots_protocol::capability::v1::{
        Availability as ProtocolAvailability, Maturity as ProtocolMaturity, TransportDescriptor,
        TransportKind,
    };

    let descriptor = TransportDescriptor {
        kind: TransportKind::parse("future-mesh").expect("custom protocol transport"),
        maturity: ProtocolMaturity::Experimental,
        availability: ProtocolAvailability::Degraded,
        can_deliver: true,
        can_fetch: false,
        can_discover: true,
        can_gateway_forward: true,
        can_observe_receipts: false,
        required_for_v1: false,
    };

    let source = SourceStatus::from_descriptor(&descriptor, false, "source not configured");
    assert_eq!(source.transport_id().as_str(), "future-mesh");
    assert!(!source.is_configured());
    assert_eq!(source.maturity(), Maturity::Experimental);
    assert_eq!(source.availability(), Availability::Degraded);
    assert!(!source.capabilities().can_fetch());
    assert!(source.capabilities().can_discover());

    let sink = SinkStatus::from_descriptor(&descriptor, true, "sink configured");
    assert_eq!(sink.transport_id().as_str(), "future-mesh");
    assert!(sink.is_configured());
    assert!(sink.capabilities().can_deliver());
    assert!(sink.capabilities().can_gateway_forward());
    assert!(!sink.capabilities().can_observe_receipts());
}

#[cfg(feature = "serde")]
#[test]
fn experimental_maturity_has_a_stable_wire_spelling() {
    assert_eq!(
        serde_json::to_string(&Maturity::Experimental).expect("serialize maturity"),
        "\"experimental\""
    );
    assert_eq!(
        serde_json::from_str::<Maturity>("\"experimental\"").expect("deserialize maturity"),
        Maturity::Experimental
    );

    let status = SourceStatus::new(
        TransportId::parse("future-mesh").expect("custom transport"),
        true,
        Maturity::Experimental,
        Availability::Degraded,
        SourceCapabilities::FETCH,
        "degraded source",
    );
    let encoded = serde_json::to_string(&status).expect("serialize source status");
    assert!(encoded.contains("\"transport_id\":\"future-mesh\""));
    assert_eq!(
        serde_json::from_str::<SourceStatus>(&encoded).expect("deserialize source status"),
        status
    );
}
