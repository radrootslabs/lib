use radroots_sdk::runtime_contract_v1::{
    RuntimeOperationIdV1, TransportKindV1, operation_descriptor, validate_runtime_contract_v1,
};

#[test]
fn runtime_contract_v1_is_public_through_sdk_surface() {
    validate_runtime_contract_v1().expect("runtime contract validates");

    let descriptor = operation_descriptor(RuntimeOperationIdV1::TransportDeliveryRetry)
        .expect("delivery retry descriptor");

    assert!(
        descriptor
            .transport_capability
            .includes_transport(TransportKindV1::Nostr)
    );
    assert!(
        descriptor
            .transport_capability
            .includes_transport(TransportKindV1::Reticulum)
    );
    assert!(matches!(
        TransportKindV1::parse("reticulum"),
        Ok(TransportKindV1::Reticulum)
    ));
}
