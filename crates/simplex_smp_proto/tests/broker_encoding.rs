use radroots_simplex_smp_proto::prelude::*;

fn broker(error: RadrootsSimplexSmpBrokerError) -> RadrootsSimplexSmpBrokerMessage {
    RadrootsSimplexSmpBrokerMessage::Err(RadrootsSimplexSmpError::Proxy(
        RadrootsSimplexSmpProxyError::Broker(error),
    ))
}

#[test]
fn oversized_response_is_a_typed_error() {
    for detail in ["p".repeat(256), "🍓".repeat(64)] {
        let message = broker(RadrootsSimplexSmpBrokerError::Response(detail));
        for version in [
            RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
            RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
        ] {
            assert_eq!(
                message.encode_for_version(version),
                Err(RadrootsSimplexSmpProtoError::InvalidShortFieldLength(256))
            );
        }
    }
}

#[test]
fn oversized_unexpected_is_a_typed_error() {
    for detail in ["p".repeat(256), "🍓".repeat(64)] {
        let message = broker(RadrootsSimplexSmpBrokerError::Unexpected(detail));
        assert_eq!(
            message.encode(),
            Err(RadrootsSimplexSmpProtoError::InvalidShortFieldLength(256))
        );
    }
}

#[test]
fn valid_byte_boundaries_round_trip_through_messages_and_transmissions() {
    for detail in [
        String::new(),
        "p".repeat(255),
        format!("{}abc", "🍓".repeat(63)),
    ] {
        for error in [
            RadrootsSimplexSmpBrokerError::Response(detail.clone()),
            RadrootsSimplexSmpBrokerError::Unexpected(detail.clone()),
        ] {
            let message = broker(error);
            for version in [
                RADROOTS_SIMPLEX_SMP_INITIAL_TRANSPORT_VERSION,
                RADROOTS_SIMPLEX_SMP_CURRENT_TRANSPORT_VERSION,
            ] {
                let encoded = message.encode_for_version(version).unwrap();
                assert_eq!(
                    RadrootsSimplexSmpBrokerMessage::decode_for_version(version, &encoded).unwrap(),
                    message
                );
            }
            let transmission = RadrootsSimplexSmpBrokerTransmission {
                authorization: Vec::new(),
                correlation_id: None,
                entity_id: Vec::new(),
                message,
            };
            let encoded = transmission.encode().unwrap();
            assert_eq!(
                RadrootsSimplexSmpBrokerTransmission::decode(&encoded).unwrap(),
                transmission
            );
        }
    }
}

#[test]
fn nested_oversized_errors_propagate_through_transmissions() {
    for error in [
        RadrootsSimplexSmpBrokerError::Response("p".repeat(256)),
        RadrootsSimplexSmpBrokerError::Unexpected("🍓".repeat(64)),
    ] {
        let mut nested =
            RadrootsSimplexSmpError::Proxy(RadrootsSimplexSmpProxyError::Broker(error));
        for _ in 0..3 {
            nested = RadrootsSimplexSmpError::Proxy(RadrootsSimplexSmpProxyError::Protocol(
                Box::new(nested),
            ));
        }
        let transmission = RadrootsSimplexSmpBrokerTransmission {
            authorization: Vec::new(),
            correlation_id: None,
            entity_id: Vec::new(),
            message: RadrootsSimplexSmpBrokerMessage::Err(nested),
        };
        assert_eq!(
            transmission.encode(),
            Err(RadrootsSimplexSmpProtoError::InvalidShortFieldLength(256))
        );
    }
}
