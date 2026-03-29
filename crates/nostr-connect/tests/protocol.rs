use nostr::{EventBuilder, Keys, PublicKey, RelayUrl, SecretKey, Timestamp, UnsignedEvent};
use radroots_nostr_connect::prelude::{
    RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR,
    RadrootsNostrConnectMethod, RadrootsNostrConnectPermission, RadrootsNostrConnectPermissions,
    RadrootsNostrConnectRequest, RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
    RadrootsNostrConnectResponseEnvelope, RadrootsNostrConnectUri,
};
use radroots_test_fixtures::{
    APP_PRIMARY_HTTPS, CDN_PRIMARY_HTTPS, FIXTURE_ALICE, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
    RELAY_TERTIARY_WSS,
};
use serde_json::{Value, json};

fn test_public_key() -> PublicKey {
    PublicKey::parse(FIXTURE_ALICE.public_key_hex).expect("public key")
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
    let parsed = RadrootsNostrConnectUri::parse(&uri).expect("parse client uri");

    match parsed {
        RadrootsNostrConnectUri::Client(client) => {
            assert_eq!(client.client_public_key, test_public_key());
            assert_eq!(client.relays.len(), 2);
            assert_eq!(client.secret, "0s8j2djs");
            assert_eq!(client.metadata.name.as_deref(), Some("My Client"));
            assert_eq!(
                client.metadata.requested_permissions,
                RadrootsNostrConnectPermissions::from(vec![
                    RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt,),
                    RadrootsNostrConnectPermission::with_parameter(
                        RadrootsNostrConnectMethod::SignEvent,
                        "1059",
                    ),
                ])
            );
            assert_eq!(
                client.metadata.url.as_deref(),
                Some(format!("{APP_PRIMARY_HTTPS}/").as_str())
            );
            assert_eq!(client.metadata.image.as_deref(), Some(logo_url().as_str()));
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
    let parsed = RadrootsNostrConnectUri::parse(&source).expect("parse bunker uri");
    let rendered = parsed.to_string();
    let reparsed = RadrootsNostrConnectUri::parse(&rendered).expect("reparse bunker uri");
    assert_eq!(parsed, reparsed);
}

#[test]
fn rejects_client_uri_without_required_secret() {
    let source = format!(
        "nostrconnect://{}?relay={}",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    );
    assert!(RadrootsNostrConnectUri::parse(&source).is_err());
}

#[test]
fn requested_permissions_roundtrip_as_csv() {
    let permissions = RadrootsNostrConnectPermissions::from(vec![
        RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt),
        RadrootsNostrConnectPermission::with_parameter(RadrootsNostrConnectMethod::SignEvent, "13"),
    ]);

    let rendered = permissions.to_string();
    assert_eq!(rendered, "nip44_encrypt,sign_event:13");
    let reparsed: RadrootsNostrConnectPermissions = rendered.parse().expect("parse permissions");
    assert_eq!(permissions, reparsed);
}

#[test]
fn connect_request_roundtrips_requested_permissions() {
    let request = RadrootsNostrConnectRequest::Connect {
        remote_signer_public_key: test_public_key(),
        secret: Some("abcd".to_owned()),
        requested_permissions: RadrootsNostrConnectPermissions::from(vec![
            RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt),
            RadrootsNostrConnectPermission::with_parameter(
                RadrootsNostrConnectMethod::SignEvent,
                "1059",
            ),
        ]),
    };
    let message = RadrootsNostrConnectRequestMessage::new("req-1", request);
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

    let decoded: RadrootsNostrConnectRequestMessage =
        serde_json::from_value(encoded).expect("deserialize request");
    assert_eq!(decoded, message);
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

    let message = RadrootsNostrConnectRequestMessage::new(
        "req-sign",
        RadrootsNostrConnectRequest::SignEvent(unsigned_event.clone()),
    );
    let encoded = serde_json::to_value(&message).expect("serialize sign request");
    assert_eq!(encoded["method"], "sign_event");

    let decoded: RadrootsNostrConnectRequestMessage =
        serde_json::from_value(encoded).expect("deserialize sign request");
    assert_eq!(decoded, message);
    assert_eq!(
        decoded.request,
        RadrootsNostrConnectRequest::SignEvent(unsigned_event)
    );
}

#[test]
fn switch_relays_response_accepts_array_or_null() {
    let relays_response = RadrootsNostrConnectResponseEnvelope {
        id: "req-switch".to_owned(),
        result: Some(json!([RELAY_SECONDARY_WSS, RELAY_TERTIARY_WSS])),
        error: None,
    };
    let parsed = RadrootsNostrConnectResponse::from_envelope(
        &RadrootsNostrConnectMethod::SwitchRelays,
        relays_response,
    )
    .expect("parse relay list");
    assert_eq!(
        parsed,
        RadrootsNostrConnectResponse::RelayList(vec![
            RelayUrl::parse(RELAY_SECONDARY_WSS).expect("relay 1"),
            RelayUrl::parse(RELAY_TERTIARY_WSS).expect("relay 2"),
        ])
    );

    let unchanged = RadrootsNostrConnectResponse::from_envelope(
        &RadrootsNostrConnectMethod::SwitchRelays,
        RadrootsNostrConnectResponseEnvelope {
            id: "req-switch".to_owned(),
            result: Some(Value::Null),
            error: None,
        },
    )
    .expect("parse null relay result");
    assert_eq!(unchanged, RadrootsNostrConnectResponse::RelayListUnchanged);
}

#[test]
fn auth_url_response_parses_from_result_and_error_fields() {
    let response = RadrootsNostrConnectResponse::from_envelope(
        &RadrootsNostrConnectMethod::SignEvent,
        RadrootsNostrConnectResponseEnvelope {
            id: "req-auth".to_owned(),
            result: Some(json!("auth_url")),
            error: Some("https://auth.example.com/challenge".to_owned()),
        },
    )
    .expect("parse auth challenge");

    assert_eq!(
        response,
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned())
    );
}

#[test]
fn get_public_key_pending_response_parses_as_typed_pending_connection() {
    let response = RadrootsNostrConnectResponse::from_envelope(
        &RadrootsNostrConnectMethod::GetPublicKey,
        RadrootsNostrConnectResponseEnvelope {
            id: "req-pending".to_owned(),
            result: None,
            error: Some(RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR.to_owned()),
        },
    )
    .expect("parse pending get_public_key response");

    assert_eq!(response, RadrootsNostrConnectResponse::PendingConnection);
}

#[test]
fn sign_event_response_roundtrips_signed_event_json_string() {
    let keys = test_keys();
    let event = EventBuilder::text_note("hello world")
        .custom_created_at(Timestamp::from(1_714_078_911))
        .sign_with_keys(&keys)
        .expect("sign event");

    let envelope = RadrootsNostrConnectResponse::SignedEvent(event.clone())
        .into_envelope("req-sign")
        .expect("serialize response");
    let parsed = RadrootsNostrConnectResponse::from_envelope(
        &RadrootsNostrConnectMethod::SignEvent,
        envelope,
    )
    .expect("parse signed event response");

    assert_eq!(parsed, RadrootsNostrConnectResponse::SignedEvent(event));
}
