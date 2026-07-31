use nostr::{Keys, SecretKey};
use radroots_identity::PublicKey as IdentityPublicKey;
use radroots_nostr_connect::message::{
    PendingConnectionOutcome, REMOTE_CAPABILITY_RELAY_COUNT_MAX, REQUEST_ID_MAX_BYTES,
    RESPONSE_ERROR_MAX_BYTES, RemoteSessionCapability, RequestId, RequestMessage, ResponseEnvelope,
    ResponseValidator,
};
use radroots_nostr_connect::uri::RelayUrl;
use radroots_nostr_connect::{Error, Method, Request, Response};
use serde_json::{Value, json};
use std::str::FromStr;

fn keys(secret_hex: &str) -> Keys {
    Keys::new(SecretKey::from_hex(secret_hex).expect("secret key"))
}

fn identity_key(secret_hex: &str) -> IdentityPublicKey {
    radroots_nostr::key::public_key_from_nostr(keys(secret_hex).public_key())
        .expect("identity public key")
}

#[test]
fn request_and_response_round_trip_with_bounded_correlation() {
    let request = RequestMessage::try_new("request-1", Request::Ping).expect("valid request");
    let encoded = serde_json::to_string(&request).expect("serialize request");
    assert_eq!(
        serde_json::from_str::<RequestMessage>(&encoded).expect("deserialize request"),
        request
    );

    let response = Response::Pong
        .into_envelope("request-1")
        .expect("valid response");
    assert_eq!(
        request.correlate(response).expect("correlated response"),
        Response::Pong
    );

    assert!(matches!(
        RequestId::parse(""),
        Err(Error::InvalidRequestId { .. })
    ));
    assert!(matches!(
        RequestId::parse(" request-1"),
        Err(Error::InvalidRequestId { .. })
    ));
    assert!(matches!(
        RequestId::parse("x".repeat(REQUEST_ID_MAX_BYTES + 1)),
        Err(Error::InvalidRequestId { .. })
    ));
}

#[test]
fn correlation_rejects_wrong_id_wrong_signer_and_replay() {
    const SIGNER: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const OTHER: &str = "0000000000000000000000000000000000000000000000000000000000000002";
    let request = RequestMessage::try_new("request-2", Request::Ping).expect("request");
    let wrong_id = Response::Pong.into_envelope("request-3").expect("response");
    assert_eq!(
        request.correlate(wrong_id).expect_err("wrong id"),
        Error::WrongRequestId
    );

    let envelope = Response::Pong.into_envelope("request-2").expect("response");
    let mut validator = ResponseValidator::new(
        RequestId::parse("request-2").expect("request id"),
        identity_key(SIGNER),
    );
    assert_eq!(
        validator
            .validate(identity_key(OTHER), "event-1", &envelope)
            .expect_err("wrong signer"),
        Error::WrongResponseSigner
    );
    validator
        .validate(identity_key(SIGNER), "event-1", &envelope)
        .expect("first response");
    assert_eq!(
        validator
            .validate(identity_key(SIGNER), "event-1", &envelope)
            .expect_err("replay"),
        Error::ReplayedResponse
    );
}

#[test]
fn malformed_envelopes_and_unsafe_auth_challenges_fail_closed() {
    assert!(matches!(
        ResponseEnvelope::try_new("request-3", None, Some(String::new())),
        Err(Error::InvalidResponseEnvelope { .. })
    ));
    assert!(matches!(
        ResponseEnvelope::try_new(
            "request-3",
            None,
            Some("x".repeat(RESPONSE_ERROR_MAX_BYTES + 1)),
        ),
        Err(Error::InvalidResponseEnvelope { .. })
    ));
    assert!(
        serde_json::from_value::<ResponseEnvelope>(json!({
            "id": "request-3",
            "result": "pong",
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(matches!(
        Response::AuthUrl("file:///tmp/approval".to_owned()).into_envelope("request-3"),
        Err(Error::InvalidUrl { value, .. }) if value == "[redacted auth URL]"
    ));
}

#[test]
fn unknown_methods_round_trip_only_when_canonical() {
    let method = Method::from_str("vendor_action").expect("canonical extension method");
    let request = RequestMessage::try_new(
        "request-custom",
        Request::Custom {
            method: method.clone(),
            params: vec!["alpha".to_owned()],
        },
    )
    .expect("custom request");
    let encoded = serde_json::to_value(&request).expect("serialize custom request");
    assert_eq!(encoded["method"], Value::String("vendor_action".to_owned()));
    assert_eq!(
        serde_json::from_value::<RequestMessage>(encoded)
            .expect("deserialize custom request")
            .payload()
            .method(),
        method
    );
    assert!(Method::from_str("Vendor-Action").is_err());
}

#[test]
fn remote_capabilities_and_pending_outcomes_are_bounded_and_typed() {
    let relay = RelayUrl::parse("wss://relay.example.test").expect("relay");
    let capability = RemoteSessionCapability::try_new(
        identity_key("0000000000000000000000000000000000000000000000000000000000000003"),
        vec![relay.clone()],
        Default::default(),
    )
    .expect("capability");
    let response = Response::RemoteSessionCapability(capability.clone());
    assert_eq!(
        response.into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::ApprovedCapability(capability)
    );
    assert!(matches!(
        RemoteSessionCapability::try_new(
            identity_key("0000000000000000000000000000000000000000000000000000000000000003"),
            vec![relay; REMOTE_CAPABILITY_RELAY_COUNT_MAX + 1],
            Default::default(),
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
}

#[test]
fn diagnostics_redact_protocol_payloads() {
    const SECRET: &str = "do-not-log-connect-secret";
    let request = Request::Connect {
        remote_signer_public_key: identity_key(
            "0000000000000000000000000000000000000000000000000000000000000004",
        ),
        secret: Some(SECRET.to_owned()),
        requested_permissions: Default::default(),
        client_metadata: None,
    };
    assert!(!format!("{request:?}").contains(SECRET));

    let envelope = ResponseEnvelope::try_new(
        "request-redacted",
        Some(Value::String(SECRET.to_owned())),
        None,
    )
    .expect("response envelope");
    assert!(!format!("{envelope:?}").contains(SECRET));

    let response = Response::ConnectSecretEcho(SECRET.to_owned());
    assert!(!format!("{response:?}").contains(SECRET));
    assert_eq!(
        response.into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::UnexpectedResponse {
            response: "connect_secret_echo".to_owned(),
        }
    );
}
