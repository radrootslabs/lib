#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::{EventBuilder, JsonUtil, Keys, PublicKey, SecretKey, Timestamp, UnsignedEvent};
use radroots_nostr_connect::message::{
    PENDING_CONNECTION_ERROR, RemoteSessionCapability, RequestMessage, ResponseEnvelope,
    SignedEvent as ConnectSignedEvent, UnsignedEvent as ConnectUnsignedEvent,
};
use radroots_nostr_connect::permission::Permissions;
use radroots_nostr_connect::uri::{
    CLIENT_METADATA_JSON_MAX_BYTES, CLIENT_NAME_MAX_BYTES, ClientMetadata, ClientUri,
    RelayUrl as ConnectRelayUrl, Uri,
};
use radroots_nostr_connect::{Error, Method, Permission, Request, Response};
use serde_json::{Value, json};
use test_fixtures::{
    APP_PRIMARY_HTTPS, CDN_PRIMARY_HTTPS, FIXTURE_ALICE, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
    RELAY_TERTIARY_WSS,
};

fn test_public_key() -> PublicKey {
    PublicKey::parse(FIXTURE_ALICE.public_key_hex).expect("public key")
}

#[test]
fn constructs_client_uri_from_validated_values() {
    let relay = ConnectRelayUrl::parse(RELAY_PRIMARY_WSS).expect("relay");
    let metadata = ClientMetadata::new()
        .with_name("Radroots")
        .expect("metadata");
    let client = ClientUri::try_new(
        test_identity_public_key(),
        [relay.clone(), relay],
        "shared-secret",
        metadata,
    )
    .expect("client URI");

    assert_eq!(client.relays().len(), 1);
    assert_eq!(client.secret(), "shared-secret");
    assert_eq!(client.metadata().name(), Some("Radroots"));
    assert!(
        ClientUri::try_new(
            test_identity_public_key(),
            Vec::<ConnectRelayUrl>::new(),
            "shared-secret",
            ClientMetadata::new(),
        )
        .is_err()
    );
}

fn test_identity_public_key() -> radroots_identity::PublicKey {
    radroots_identity::PublicKey::from_hex(FIXTURE_ALICE.public_key_hex)
        .expect("identity public key")
}

fn test_keys() -> Keys {
    let secret_key = SecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).expect("secret key");
    Keys::new(secret_key)
}

fn encode_uri_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn logo_url() -> String {
    format!("{CDN_PRIMARY_HTTPS}/logo.png")
}

fn remote_session_capability() -> RemoteSessionCapability {
    RemoteSessionCapability {
        user_public_key: test_identity_public_key(),
        relays: vec![
            ConnectRelayUrl::parse(RELAY_PRIMARY_WSS).expect("relay 1"),
            ConnectRelayUrl::parse(RELAY_SECONDARY_WSS).expect("relay 2"),
        ],
        permissions: Permissions::from(vec![
            Permission::new(Method::Ping),
            Permission::with_parameter(Method::SignEvent, "kind:1"),
        ]),
    }
}

#[test]
fn parses_client_uri_with_current_spec_query_fields() {
    let uri = format!(
        "nostrconnect://{}?relay={}&relay={}&secret=0s8j2djs&perms=nip44_encrypt%2Csign_event%3A1059&name=My+Client&url={}&image={}",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_SECONDARY_WSS),
        encode_uri_component(RELAY_TERTIARY_WSS),
        encode_uri_component(APP_PRIMARY_HTTPS),
        encode_uri_component(&logo_url()),
    );
    let parsed = Uri::parse(&uri).expect("parse client uri");

    match parsed {
        Uri::Client(client) => {
            assert_eq!(client.client_public_key(), test_identity_public_key());
            assert_eq!(client.relays().len(), 2);
            assert_eq!(client.secret(), "0s8j2djs");
            assert_eq!(client.metadata().name.as_deref(), Some("My Client"));
            assert_eq!(
                client.metadata().requested_permissions,
                Permissions::from(vec![
                    Permission::new(Method::Nip44Encrypt,),
                    Permission::with_parameter(Method::SignEvent, "1059",),
                ])
            );
            assert_eq!(
                client.metadata().url.as_deref(),
                Some(format!("{APP_PRIMARY_HTTPS}/").as_str())
            );
            assert_eq!(
                client.metadata().image.as_deref(),
                Some(logo_url().as_str())
            );
        }
        other => panic!("expected client uri, got {other:?}"),
    }
}

#[test]
fn parses_bunker_uri_and_roundtrips() {
    let source = format!(
        "bunker://{}?relay={}&secret=abcd",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    );
    let parsed = Uri::parse(&source).expect("parse bunker uri");
    let rendered = parsed.to_string();
    let reparsed = Uri::parse(&rendered).expect("reparse bunker uri");
    assert_eq!(parsed, reparsed);
}

#[test]
fn rejects_client_uri_without_required_secret() {
    let source = format!(
        "nostrconnect://{}?relay={}",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    );
    assert!(Uri::parse(&source).is_err());
}

#[test]
fn requested_permissions_roundtrip_as_csv() {
    let permissions = Permissions::from(vec![
        Permission::new(Method::Nip44Encrypt),
        Permission::with_parameter(Method::SignEvent, "13"),
    ]);

    let rendered = permissions.to_string();
    assert_eq!(rendered, "nip44_encrypt,sign_event:13");
    let reparsed: Permissions = rendered.parse().expect("parse permissions");
    assert_eq!(permissions, reparsed);
}

#[test]
fn connect_request_roundtrips_requested_permissions() {
    let request = Request::Connect {
        remote_signer_public_key: test_identity_public_key(),
        secret: Some("abcd".to_owned()),
        requested_permissions: Permissions::from(vec![
            Permission::new(Method::Nip44Encrypt),
            Permission::with_parameter(Method::SignEvent, "1059"),
        ]),
        client_metadata: None,
    };
    let message = RequestMessage::new("req-1", request);
    let encoded = serde_json::to_value(&message).expect("serialize request");
    assert_eq!(
        encoded,
        json!({
            "id": "req-1",
            "method": "connect",
            "params": [
                FIXTURE_ALICE.public_key_hex,
                "abcd",
                "nip44_encrypt,sign_event:1059"
            ]
        })
    );

    let decoded: RequestMessage = serde_json::from_value(encoded).expect("deserialize request");
    assert_eq!(decoded, message);
}

#[test]
fn connect_request_roundtrips_client_metadata_in_fourth_parameter() {
    let request = Request::Connect {
        remote_signer_public_key: test_identity_public_key(),
        secret: None,
        requested_permissions: Permissions::default(),
        client_metadata: Some(ClientMetadata {
            requested_permissions: Permissions::default(),
            name: Some(" My Client ".to_owned()),
            url: Some(APP_PRIMARY_HTTPS.to_owned()),
            image: Some(logo_url()),
        }),
    };
    let message = RequestMessage::new("req-metadata", request);
    let encoded = serde_json::to_value(&message).expect("serialize metadata request");
    assert_eq!(encoded["params"][1], "");
    assert_eq!(encoded["params"][2], "");
    let encoded_metadata: Value = serde_json::from_str(
        encoded["params"][3]
            .as_str()
            .expect("metadata parameter string"),
    )
    .expect("metadata parameter json");
    assert_eq!(
        encoded_metadata,
        json!({
            "name": "My Client",
            "url": format!("{APP_PRIMARY_HTTPS}/"),
            "image": logo_url(),
        })
    );

    let decoded: RequestMessage =
        serde_json::from_value(encoded.clone()).expect("deserialize metadata request");
    match &decoded.request {
        Request::Connect {
            client_metadata: Some(metadata),
            ..
        } => {
            assert_eq!(metadata.name.as_deref(), Some("My Client"));
            assert!(metadata.requested_permissions.is_empty());
        }
        other => panic!("expected connect metadata, got {other:?}"),
    }
    assert_eq!(
        serde_json::to_value(&decoded).expect("re-encode normalized metadata"),
        encoded
    );
}

#[test]
fn logout_request_and_acknowledgement_roundtrip() {
    let message = RequestMessage::new("req-logout", Request::Logout);
    assert_eq!(
        serde_json::to_value(&message).expect("serialize logout"),
        json!({"id": "req-logout", "method": "logout", "params": []})
    );

    let response = Response::from_envelope(
        &Method::Logout,
        ResponseEnvelope {
            id: "req-logout".to_owned(),
            result: Some(Value::String("ack".to_owned())),
            error: None,
        },
    )
    .expect("parse logout acknowledgement");
    assert_eq!(response, Response::LogoutAcknowledged);
    assert_eq!(
        response
            .into_envelope("req-logout")
            .expect("encode logout acknowledgement")
            .result,
        Some(Value::String("ack".to_owned()))
    );
}

#[test]
fn rejects_invalid_client_metadata() {
    let invalid_name = json!({
        "id": "req-invalid-name",
        "method": "connect",
        "params": [
            FIXTURE_ALICE.public_key_hex,
            "",
            "",
            serde_json::to_string(&json!({"name": "line\nbreak"})).expect("metadata")
        ]
    });
    assert!(serde_json::from_value::<RequestMessage>(invalid_name).is_err());

    let invalid_scheme = format!(
        "nostrconnect://{}?relay={}&secret=secret&url={}",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
        encode_uri_component("file:///tmp/client"),
    );
    assert!(Uri::parse(&invalid_scheme).is_err());

    let oversized_name = ClientMetadata {
        requested_permissions: Permissions::default(),
        name: Some("a".repeat(CLIENT_NAME_MAX_BYTES + 1)),
        url: None,
        image: None,
    };
    assert!(matches!(
        oversized_name.to_connect_param(),
        Err(Error::InvalidClientMetadata { field: "name", .. })
    ));

    let oversized_payload = "x".repeat(CLIENT_METADATA_JSON_MAX_BYTES + 1);
    assert!(matches!(
        Request::from_parts(
            Method::Connect,
            vec![
                test_public_key().to_hex(),
                String::new(),
                String::new(),
                oversized_payload,
            ],
        ),
        Err(Error::ClientMetadataTooLarge { .. })
    ));
}

#[test]
fn sign_event_request_roundtrips_unsigned_event_payload() {
    let unsigned_event: UnsignedEvent = serde_json::from_value(json!({
        "pubkey": test_public_key().to_hex(),
        "created_at": 1714078911u64,
        "kind": 1u16,
        "tags": [],
        "content": "Hello, I'm signing remotely"
    }))
    .expect("unsigned event");

    let message = RequestMessage::new(
        "req-sign",
        Request::SignEvent(
            ConnectUnsignedEvent::from_json(&unsigned_event.as_json())
                .expect("unsigned event payload"),
        ),
    );
    let encoded = serde_json::to_value(&message).expect("serialize sign request");
    assert_eq!(encoded["method"], "sign_event");

    let decoded: RequestMessage =
        serde_json::from_value(encoded).expect("deserialize sign request");
    assert_eq!(decoded, message);
    assert_eq!(
        decoded.request,
        Request::SignEvent(
            ConnectUnsignedEvent::from_json(&unsigned_event.as_json())
                .expect("unsigned event payload"),
        )
    );
}

#[test]
fn switch_relays_response_accepts_array_or_null() {
    let relays_response = ResponseEnvelope {
        id: "req-switch".to_owned(),
        result: Some(json!([RELAY_SECONDARY_WSS, RELAY_TERTIARY_WSS])),
        error: None,
    };
    let parsed =
        Response::from_envelope(&Method::SwitchRelays, relays_response).expect("parse relay list");
    assert_eq!(
        parsed,
        Response::RelayList(vec![
            ConnectRelayUrl::parse(RELAY_SECONDARY_WSS).expect("relay 1"),
            ConnectRelayUrl::parse(RELAY_TERTIARY_WSS).expect("relay 2"),
        ])
    );

    let unchanged = Response::from_envelope(
        &Method::SwitchRelays,
        ResponseEnvelope {
            id: "req-switch".to_owned(),
            result: Some(Value::Null),
            error: None,
        },
    )
    .expect("parse null relay result");
    assert_eq!(unchanged, Response::RelayListUnchanged);
}

#[test]
fn get_session_capability_request_and_response_roundtrip() {
    let request_message = RequestMessage::new("req-cap", Request::GetSessionCapability);
    let encoded_request = serde_json::to_value(&request_message).expect("serialize request");
    let decoded_request: RequestMessage =
        serde_json::from_value(encoded_request).expect("deserialize request");
    assert_eq!(decoded_request, request_message);

    let capability = remote_session_capability();
    let response_envelope = Response::RemoteSessionCapability(capability.clone())
        .into_envelope("resp-cap")
        .expect("serialize response");
    let decoded_response =
        Response::from_envelope(&Method::GetSessionCapability, response_envelope)
            .expect("deserialize response");
    assert_eq!(
        decoded_response,
        Response::RemoteSessionCapability(capability)
    );
}

#[test]
fn auth_url_response_parses_from_result_and_error_fields() {
    let response = Response::from_envelope(
        &Method::SignEvent,
        ResponseEnvelope {
            id: "req-auth".to_owned(),
            result: Some(json!("auth_url")),
            error: Some("https://auth.example.com/challenge".to_owned()),
        },
    )
    .expect("parse auth challenge");

    assert_eq!(
        response,
        Response::AuthUrl("https://auth.example.com/challenge".to_owned())
    );
}

#[test]
fn get_public_key_pending_response_parses_as_typed_pending_connection() {
    let response = Response::from_envelope(
        &Method::GetPublicKey,
        ResponseEnvelope {
            id: "req-pending".to_owned(),
            result: None,
            error: Some(PENDING_CONNECTION_ERROR.to_owned()),
        },
    )
    .expect("parse pending get_public_key response");

    assert_eq!(response, Response::PendingConnection);
}

#[test]
fn sign_event_response_roundtrips_signed_event_json_string() {
    let keys = test_keys();
    let event = EventBuilder::text_note("hello world")
        .custom_created_at(Timestamp::from(1_714_078_911))
        .sign_with_keys(&keys)
        .expect("sign event");

    let envelope = Response::SignedEvent(
        ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload"),
    )
    .into_envelope("req-sign")
    .expect("serialize response");
    let parsed =
        Response::from_envelope(&Method::SignEvent, envelope).expect("parse signed event response");

    assert_eq!(
        parsed,
        Response::SignedEvent(
            ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload")
        )
    );
}

#[test]
fn checked_in_current_session_vectors_match_protocol_behavior() {
    let vectors =
        include_str!("../../../contracts/conformance/vectors/nip46/current_session.v1.json");
    let document: Value = serde_json::from_str(vectors).expect("NIP-46 vector JSON");
    assert_eq!(document["suite"], "nip46_current_session");
    assert_eq!(document["contract_version"], "1.0.0");
    let entries = document["vectors"].as_array().expect("NIP-46 vectors");

    for entry in entries {
        let id = entry["id"].as_str().expect("vector id");
        let kind = entry["kind"].as_str().expect("vector kind");
        let input = &entry["input"];
        let expected = &entry["expected"];

        match kind {
            "nip46.request.valid" => {
                let message: RequestMessage = serde_json::from_value(input["message"].clone())
                    .unwrap_or_else(|error| panic!("{id}: parse request: {error}"));
                let normalized = serde_json::to_value(message)
                    .unwrap_or_else(|error| panic!("{id}: serialize request: {error}"));
                assert_eq!(normalized, expected["normalized_message"], "{id}");
            }
            "nip46.request.invalid" => {
                let error = serde_json::from_value::<RequestMessage>(input["message"].clone())
                    .expect_err("invalid request vector");
                assert_vector_error(id, expected, error);
            }
            "nip46.metadata.invalid" => {
                let count = input["count"].as_u64().expect("metadata repeat count") as usize;
                let repeat = input["repeat"].as_str().expect("metadata repeat value");
                let metadata = ClientMetadata {
                    name: Some(repeat.repeat(count)),
                    ..ClientMetadata::default()
                };
                let error = metadata.normalized().expect_err("invalid metadata vector");
                assert_vector_error(id, expected, error);
            }
            "nip46.uri.valid" => {
                let uri = input["uri"].as_str().expect("NIP-46 URI");
                let parsed =
                    Uri::parse(uri).unwrap_or_else(|error| panic!("{id}: parse URI: {error}"));
                assert_uri_vector(id, parsed, expected);
            }
            "nip46.uri.invalid" => {
                let uri = input["uri"].as_str().expect("NIP-46 URI");
                let error = Uri::parse(uri).expect_err("invalid URI vector");
                assert_vector_error(id, expected, error);
            }
            "nip46.response.valid" => {
                let method = input["method"]
                    .as_str()
                    .expect("response method")
                    .parse::<Method>()
                    .expect("typed response method");
                let envelope: ResponseEnvelope = serde_json::from_value(input["envelope"].clone())
                    .unwrap_or_else(|error| panic!("{id}: parse envelope: {error}"));
                let request_id = envelope.id.clone();
                let response = Response::from_envelope(&method, envelope)
                    .unwrap_or_else(|error| panic!("{id}: parse response: {error}"));
                let normalized = response
                    .into_envelope(request_id)
                    .unwrap_or_else(|error| panic!("{id}: serialize response: {error}"));
                let normalized = serde_json::to_value(normalized)
                    .unwrap_or_else(|error| panic!("{id}: serialize envelope: {error}"));
                assert_eq!(normalized, expected["normalized_envelope"], "{id}");
            }
            "nip46.response.invalid" => {
                let method = input["method"]
                    .as_str()
                    .expect("response method")
                    .parse::<Method>()
                    .expect("typed response method");
                let envelope: ResponseEnvelope = serde_json::from_value(input["envelope"].clone())
                    .unwrap_or_else(|error| panic!("{id}: parse envelope: {error}"));
                let error = Response::from_envelope(&method, envelope)
                    .expect_err("invalid response vector");
                assert_vector_error(id, expected, error);
            }
            other => panic!("{id}: unknown NIP-46 vector kind {other}"),
        }
    }
}

fn assert_uri_vector(id: &str, parsed: Uri, expected: &Value) {
    let expected_relays = expected["relays"]
        .as_array()
        .expect("expected relays")
        .iter()
        .map(|relay| relay.as_str().expect("expected relay"))
        .collect::<Vec<_>>();

    match parsed {
        Uri::Bunker(uri) => {
            assert_eq!(expected["variant"], "bunker", "{id}");
            let relays = uri
                .relays()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            assert_eq!(relays, expected_relays, "{id}");
            assert_eq!(uri.secret(), expected["secret"].as_str(), "{id}");
        }
        Uri::Client(uri) => {
            assert_eq!(expected["variant"], "nostrconnect", "{id}");
            let relays = uri
                .relays()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            assert_eq!(relays, expected_relays, "{id}");
            assert_eq!(uri.secret(), expected["secret"].as_str().expect("secret"));
            assert_eq!(
                uri.metadata().name.as_deref(),
                expected["metadata"]["name"].as_str(),
                "{id}"
            );
            assert_eq!(
                uri.metadata().url.as_deref(),
                expected["metadata"]["url"].as_str(),
                "{id}"
            );
            assert_eq!(
                uri.metadata().image.as_deref(),
                expected["metadata"]["image"].as_str(),
                "{id}"
            );
            assert_eq!(
                uri.metadata().requested_permissions.to_string(),
                expected["metadata"]["permissions"]
                    .as_str()
                    .expect("permissions"),
                "{id}"
            );
        }
    }
}

fn assert_vector_error(id: &str, expected: &Value, error: impl ToString) {
    let class = expected["error"].as_str().expect("expected error class");
    let needle = match class {
        "invalid_client_metadata" => "invalid NIP-46 client metadata",
        "invalid_params" => "invalid parameter count",
        "invalid_response_payload" => "invalid response payload",
        "missing_result" => "missing response result",
        "missing_secret" => "missing secret",
        other => panic!("{id}: unknown expected error class {other}"),
    };
    let message = error.to_string();
    assert!(
        message.contains(needle),
        "{id}: expected {class}, got {message}"
    );
}
