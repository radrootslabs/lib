#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::{Event, EventBuilder, JsonUtil, Keys, SecretKey, Timestamp, UnsignedEvent};
use radroots_nostr_connect::prelude::{
    CLIENT_URL_MAX_BYTES, ClientMetadata, Method, Permission, Permissions,
    RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR, RadrootsNostrConnectError,
    RadrootsNostrConnectPendingConnectionPollOutcome, RadrootsNostrConnectRequest,
    RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
    RadrootsNostrConnectResponseEnvelope, SignedEvent as ConnectSignedEvent,
    UnsignedEvent as ConnectUnsignedEvent, Uri,
};
use radroots_nostr_connect::uri::RelayUrl;
use serde_json::{Value, json};
use std::str::FromStr;
use test_fixtures::{
    APP_PRIMARY_HTTPS, CDN_PRIMARY_HTTPS, FIXTURE_ALICE, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
    RELAY_TERTIARY_WSS,
};

fn test_public_key() -> radroots_identity::PublicKey {
    radroots_identity::PublicKey::from_hex(FIXTURE_ALICE.public_key_hex).expect("public key")
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

fn unsigned_event() -> UnsignedEvent {
    serde_json::from_value(json!({
        "pubkey": test_public_key().to_hex(),
        "created_at": 1714078911u64,
        "kind": 1u16,
        "tags": [],
        "content": "hello"
    }))
    .expect("unsigned event")
}

fn signed_event() -> Event {
    EventBuilder::text_note("hello world")
        .custom_created_at(Timestamp::from(1_714_078_911))
        .sign_with_keys(&test_keys())
        .expect("sign event")
}

fn relay(value: &str) -> RelayUrl {
    RelayUrl::parse(value).expect("relay")
}

#[test]
fn error_method_and_permission_surfaces_cover_public_paths() {
    let json_error = serde_json::from_str::<Value>("{").expect_err("invalid json");
    assert!(matches!(
        RadrootsNostrConnectError::from(json_error),
        RadrootsNostrConnectError::Json(message) if !message.is_empty()
    ));

    let methods = [
        (Method::Connect, "connect"),
        (Method::GetPublicKey, "get_public_key"),
        (Method::GetSessionCapability, "get_session_capability"),
        (Method::SignEvent, "sign_event"),
        (Method::Nip04Encrypt, "nip04_encrypt"),
        (Method::Nip04Decrypt, "nip04_decrypt"),
        (Method::Nip44Encrypt, "nip44_encrypt"),
        (Method::Nip44Decrypt, "nip44_decrypt"),
        (Method::Ping, "ping"),
        (Method::SwitchRelays, "switch_relays"),
    ];
    for (method, raw) in methods {
        assert_eq!(method.as_str(), raw);
        assert_eq!(method.to_string(), raw);
        assert_eq!(Method::from_str(raw).expect("parse method"), method);
    }
    assert_eq!(
        Method::from_str("publish_note").expect("custom method"),
        Method::custom("publish_note").expect("valid custom NIP-46 method")
    );
    assert!(matches!(
        Method::from_str(" "),
        Err(RadrootsNostrConnectError::InvalidMethod(value)) if value == " "
    ));
    assert_eq!(
        serde_json::from_str::<Method>("\"do_work\"").expect("deserialize custom method"),
        Method::custom("do_work").expect("valid custom NIP-46 method")
    );
    assert!(
        serde_json::from_str::<Method>("123")
            .expect_err("non-string method")
            .to_string()
            .contains("invalid type")
    );
    assert!(
        serde_json::from_str::<Method>("\"\"")
            .expect_err("blank method")
            .to_string()
            .contains("invalid NIP-46 method")
    );

    let simple = Permission::new(Method::Ping);
    assert_eq!(simple.to_string(), "ping");
    let parameterized = Permission::with_parameter(Method::SignEvent, "1059");
    assert_eq!(parameterized.to_string(), "sign_event:1059");
    assert_eq!(
        Permission::from_str("sign_event:1059").expect("parse permission"),
        parameterized
    );
    assert!(matches!(
        Permission::from_str(" "),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == " "
    ));
    assert!(matches!(
        Permission::from_str("sign_event:"),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        Permission::from_str(" :kind"),
        Err(RadrootsNostrConnectError::InvalidMethod(_))
    ));

    let empty = Permissions::new();
    assert!(empty.is_empty());
    assert!(empty.as_slice().is_empty());
    assert!(empty.clone().into_vec().is_empty());
    assert_eq!(
        Permissions::from_str("  ").expect("empty permissions"),
        empty
    );

    let permissions = Permissions::from(vec![
        Permission::new(Method::Nip44Encrypt),
        Permission::with_parameter(Method::SignEvent, "13"),
    ]);
    assert_eq!(permissions.to_string(), "nip44_encrypt,sign_event:13");
    assert_eq!(
        serde_json::to_string(&permissions).expect("serialize permissions"),
        "\"nip44_encrypt,sign_event:13\""
    );
    assert_eq!(
        serde_json::from_str::<Permissions>("\"nip44_encrypt,sign_event:13\"")
            .expect("deserialize permissions"),
        permissions
    );
    assert!(
        serde_json::from_str::<Permissions>("123")
            .expect_err("non-string permissions")
            .to_string()
            .contains("invalid type")
    );
    assert!(matches!(
        Permissions::from_str("sign_event:,ping"),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));

    let all_sign_events = Permission::new(Method::SignEvent);
    assert!(all_sign_events.matches_sign_event_kind(30402));
    assert!(all_sign_events.matches_request(&Method::SignEvent, None));
    assert!(!all_sign_events.matches_request(&Method::Ping, None));

    let numeric_sign_event = Permission::with_parameter(Method::SignEvent, "30402");
    let kind_prefixed_sign_event = Permission::with_parameter(Method::SignEvent, "kind:30402");
    assert!(numeric_sign_event.matches_sign_event_kind(30402));
    assert!(kind_prefixed_sign_event.matches_sign_event_kind(30402));
    assert!(numeric_sign_event.matches_request(&Method::SignEvent, Some("kind:30402")));
    assert!(!numeric_sign_event.matches_sign_event_kind(3040));
    assert!(
        !Permission::with_parameter(Method::SignEvent, "130402").matches_sign_event_kind(30402)
    );
    assert!(
        !Permission::with_parameter(Method::SignEvent, "not-a-kind")
            .matches_request(&Method::SignEvent, Some("also-not-a-kind"))
    );
    assert!(!Permission::with_parameter(Method::SignEvent, "kind:").matches_sign_event_kind(30402));
    let encrypt_permission =
        Permission::with_parameter(Method::Nip44Encrypt, test_public_key().to_hex());
    assert!(
        encrypt_permission
            .matches_request(&Method::Nip44Encrypt, Some(&test_public_key().to_hex()))
    );
    assert!(!encrypt_permission.matches_request(&Method::Nip44Encrypt, None));

    let typed_permissions = Permissions::from(vec![
        Permission::new(Method::Ping),
        kind_prefixed_sign_event,
    ]);
    assert!(typed_permissions.allows_request(&Method::Ping, None));
    assert!(typed_permissions.allows_sign_event_kind(30402));
    assert!(!typed_permissions.allows_sign_event_kind(0));
}

#[test]
fn uri_surface_covers_rendering_ignored_queries_and_error_paths() {
    let bunker = Uri::parse(&format!(
        "bunker://{}?relay={}&foo=bar",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    ))
    .expect("parse bunker");
    let bunker_rendered = bunker.to_string();
    assert!(bunker_rendered.contains(&format!(
        "relay={}",
        encode_uri_component(RELAY_PRIMARY_WSS)
    )));
    assert!(!bunker_rendered.contains("secret="));

    let minimal_client: Uri = format!(
        "nostrconnect://{}?relay={}&secret=shared",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    )
    .parse()
    .expect("parse minimal client");
    let minimal_client_rendered = minimal_client.to_string();
    assert!(minimal_client_rendered.contains("secret=shared"));
    assert!(!minimal_client_rendered.contains("perms="));
    assert!(!minimal_client_rendered.contains("name="));
    assert!(!minimal_client_rendered.contains("url="));
    assert!(!minimal_client_rendered.contains("image="));

    let metadata_client = Uri::parse(&format!(
        "nostrconnect://{}?relay={}&secret=shared&perms=ping&name=myc&url={}&image={}&ignored=value",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
        encode_uri_component(APP_PRIMARY_HTTPS),
        encode_uri_component(&logo_url()),
    ))
    .expect("parse metadata client");
    let metadata_rendered = metadata_client.to_string();
    assert!(metadata_rendered.contains("perms=ping"));
    assert!(metadata_rendered.contains("name=myc"));
    assert!(metadata_rendered.contains(&format!(
        "url={}",
        encode_uri_component(&format!("{APP_PRIMARY_HTTPS}/"))
    )));
    assert!(metadata_rendered.contains(&format!("image={}", encode_uri_component(&logo_url()))));

    assert!(matches!(
        Uri::parse("not a uri"),
        Err(RadrootsNostrConnectError::InvalidUrl { .. })
    ));
    assert!(matches!(
        Uri::parse("nostrconnect:///path?relay=wss%3A%2F%2Frelay.example.com&secret=abc"),
        Err(RadrootsNostrConnectError::MissingPublicKey)
    ));
    assert!(matches!(
        Uri::parse(&format!("bunker://{}", FIXTURE_ALICE.public_key_hex)),
        Err(RadrootsNostrConnectError::MissingRelay)
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::MissingRelay)
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::MissingSecret)
    ));
    assert!(matches!(
        Uri::parse("https://example.com"),
        Err(RadrootsNostrConnectError::InvalidUriScheme(value)) if value == "https"
    ));
    assert!(matches!(
        Uri::parse("nostrconnect://bad-key?relay=wss%3A%2F%2Frelay.example.com&secret=abc"),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay=http%3A%2F%2Frelay.example.com&secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&url=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidClientMetadata { field: "url", .. })
    ));
    assert!(matches!(
        Uri::parse("bunker://bad-key?relay=wss%3A%2F%2Frelay.example.com"),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "bunker://{}?relay=http%3A%2F%2Frelay.example.com",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&perms=sign_event%3A",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&image=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidClientMetadata { field: "image", .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::MissingSecret)
    ));
}

#[test]
fn client_metadata_rejects_malformed_and_unsafe_display_fields() {
    let empty = ClientMetadata::default();
    assert!(empty.is_display_empty());
    let decoded: ClientMetadata = serde_json::from_value(json!({
        "requested_permissions": "ping",
        "name": " client ",
        "url": APP_PRIMARY_HTTPS,
        "image": logo_url(),
    }))
    .expect("deserialize and normalize client metadata");
    assert_eq!(decoded.name.as_deref(), Some("client"));
    assert_eq!(
        decoded.url.as_deref(),
        Some(format!("{APP_PRIMARY_HTTPS}/").as_str())
    );
    assert!(
        serde_json::from_value::<ClientMetadata>(json!({
            "name": "line\nbreak"
        }))
        .is_err()
    );
    for metadata in [
        ClientMetadata {
            name: Some("client".to_owned()),
            ..empty.clone()
        },
        ClientMetadata {
            url: Some(APP_PRIMARY_HTTPS.to_owned()),
            ..empty.clone()
        },
        ClientMetadata {
            image: Some(logo_url()),
            ..empty.clone()
        },
    ] {
        assert!(!metadata.is_display_empty());
    }

    assert!(matches!(
        ClientMetadata::from_connect_param("{"),
        Err(RadrootsNostrConnectError::InvalidClientMetadata {
            field: "payload",
            ..
        })
    ));

    for (value, field) in [
        ("x".repeat(CLIENT_URL_MAX_BYTES + 1), "url"),
        ("https://example.com/\n".to_owned(), "url"),
        ("https://user@example.com".to_owned(), "url"),
        ("https://:secret@example.com".to_owned(), "image"),
    ] {
        let metadata = ClientMetadata {
            url: (field == "url").then_some(value.clone()),
            image: (field == "image").then_some(value),
            ..empty.clone()
        };
        assert!(matches!(
            metadata.normalized(),
            Err(RadrootsNostrConnectError::InvalidClientMetadata {
                field: actual,
                ..
            }) if actual == field
        ));
    }
}

#[test]
fn request_surface_covers_variant_methods_serialization_and_validation() {
    let ping_permission = Permissions::from(vec![Permission::new(Method::Ping)]);

    let requests = vec![
        (
            RadrootsNostrConnectRequest::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: Permissions::default(),
                client_metadata: None,
            },
            Method::Connect,
            vec![test_public_key().to_hex()],
        ),
        (
            RadrootsNostrConnectRequest::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: ping_permission.clone(),
                client_metadata: None,
            },
            Method::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::GetPublicKey,
            Method::GetPublicKey,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::GetSessionCapability,
            Method::GetSessionCapability,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::SignEvent(
                ConnectUnsignedEvent::from_json(&unsigned_event().as_json())
                    .expect("unsigned event payload"),
            ),
            Method::SignEvent,
            vec![serde_json::to_string(&unsigned_event()).expect("serialize unsigned event")],
        ),
        (
            RadrootsNostrConnectRequest::Nip04Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            Method::Nip04Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip04Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            Method::Nip04Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip44Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            Method::Nip44Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip44Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            Method::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (RadrootsNostrConnectRequest::Ping, Method::Ping, Vec::new()),
        (
            RadrootsNostrConnectRequest::SwitchRelays,
            Method::SwitchRelays,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::Logout,
            Method::Logout,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::Custom {
                method: Method::custom("publish_note").expect("valid custom NIP-46 method"),
                params: vec!["one".to_owned(), "two".to_owned()],
            },
            Method::custom("publish_note").expect("valid custom NIP-46 method"),
            vec!["one".to_owned(), "two".to_owned()],
        ),
    ];
    for (request, method, params) in requests {
        assert_eq!(request.method(), method);
        assert_eq!(request.to_params().expect("request params"), params);
    }

    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(Method::Connect, vec![test_public_key().to_hex()],)
            .expect("connect without secret or perms"),
        RadrootsNostrConnectRequest::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: Permissions::default(),
            client_metadata: None,
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        )
        .expect("connect with empty secret"),
        RadrootsNostrConnectRequest::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: Permissions::from(vec![Permission::new(Method::Ping),]),
            client_metadata: None,
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(Method::GetPublicKey, Vec::new(),)
            .expect("get_public_key from parts"),
        RadrootsNostrConnectRequest::GetPublicKey
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(Method::GetSessionCapability, Vec::new(),)
            .expect("get_session_capability from parts"),
        RadrootsNostrConnectRequest::GetSessionCapability
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip04Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        )
        .expect("nip04 encrypt from parts"),
        RadrootsNostrConnectRequest::Nip04Encrypt {
            public_key: test_public_key(),
            plaintext: "hello".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip04Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        )
        .expect("nip04 decrypt from parts"),
        RadrootsNostrConnectRequest::Nip04Decrypt {
            public_key: test_public_key(),
            ciphertext: "cipher".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip44Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        )
        .expect("nip44 encrypt from parts"),
        RadrootsNostrConnectRequest::Nip44Encrypt {
            public_key: test_public_key(),
            plaintext: "hello".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        )
        .expect("nip44 decrypt from parts"),
        RadrootsNostrConnectRequest::Nip44Decrypt {
            public_key: test_public_key(),
            ciphertext: "cipher".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(Method::Ping, Vec::new()).expect("ping from parts"),
        RadrootsNostrConnectRequest::Ping
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(Method::SwitchRelays, Vec::new(),)
            .expect("switch relays from parts"),
        RadrootsNostrConnectRequest::SwitchRelays
    );

    for (method, params, expected_error) in [
        (Method::GetPublicKey, vec!["oops".to_owned()], "no params"),
        (
            Method::GetSessionCapability,
            vec!["oops".to_owned()],
            "no params",
        ),
        (Method::SignEvent, Vec::new(), "exactly 1 param"),
        (
            Method::Nip04Encrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            Method::Nip04Decrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            Method::Nip44Encrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            Method::Nip44Decrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (Method::Ping, vec!["oops".to_owned()], "no params"),
        (Method::SwitchRelays, vec!["oops".to_owned()], "no params"),
        (Method::Logout, vec!["oops".to_owned()], "no params"),
    ] {
        assert!(matches!(
            RadrootsNostrConnectRequest::from_parts(method, params),
            Err(RadrootsNostrConnectError::InvalidParams { expected, .. }) if expected == expected_error
        ));
    }
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(Method::Connect, Vec::new()),
        Err(RadrootsNostrConnectError::InvalidParams { expected, received, .. })
            if expected == "1 to 4 params" && received == 0
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(Method::Connect, vec!["bad-key".to_owned()],),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Connect,
            vec![test_public_key().to_hex(), "secret".to_owned(), "sign_event:".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Connect,
            vec![
                test_public_key().to_hex(),
                "secret".to_owned(),
                "ping".to_owned(),
                "extra".to_owned(),
                "too-many".to_owned(),
            ],
        ),
        Err(RadrootsNostrConnectError::InvalidParams { expected, received, .. })
            if expected == "1 to 4 params" && received == 5
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(Method::SignEvent, vec!["not-json".to_owned()],),
        Err(RadrootsNostrConnectError::InvalidRequestPayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip04Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip04Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip44Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            Method::Nip44Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));

    let custom_message = RadrootsNostrConnectRequestMessage::new(
        "req-custom",
        RadrootsNostrConnectRequest::Custom {
            method: Method::custom("publish_note").expect("valid custom NIP-46 method"),
            params: vec!["a".to_owned()],
        },
    );
    let encoded = serde_json::to_string(&custom_message).expect("serialize custom request");
    let decoded: RadrootsNostrConnectRequestMessage =
        serde_json::from_str(&encoded).expect("deserialize custom request");
    assert_eq!(decoded, custom_message);
    assert!(
        serde_json::from_str::<RadrootsNostrConnectRequestMessage>("{")
            .expect_err("invalid request message json")
            .to_string()
            .contains("EOF")
    );
    assert!(
        serde_json::from_str::<RadrootsNostrConnectRequestMessage>(
            "{\"id\":\"req\",\"method\":\"get_public_key\",\"params\":[\"oops\"]}",
        )
        .expect_err("invalid request params")
        .to_string()
        .contains("invalid parameter count")
    );
}

#[test]
fn response_surface_covers_success_and_error_paths() {
    let event = signed_event();
    let remote_session_capability =
        radroots_nostr_connect::prelude::RadrootsNostrConnectRemoteSessionCapability {
            user_public_key: test_public_key(),
            relays: vec![relay(RELAY_PRIMARY_WSS), relay(RELAY_SECONDARY_WSS)],
            permissions: Permissions::from(vec![
                Permission::new(Method::Ping),
                Permission::with_parameter(Method::SignEvent, "kind:1"),
            ]),
        };
    let cases = vec![
        (
            RadrootsNostrConnectResponse::ConnectAcknowledged,
            Method::Connect,
            RadrootsNostrConnectResponse::ConnectAcknowledged,
        ),
        (
            RadrootsNostrConnectResponse::ConnectSecretEcho("secret".to_owned()),
            Method::Connect,
            RadrootsNostrConnectResponse::ConnectSecretEcho("secret".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::UserPublicKey(test_public_key()),
            Method::GetPublicKey,
            RadrootsNostrConnectResponse::UserPublicKey(test_public_key()),
        ),
        (
            RadrootsNostrConnectResponse::PendingConnection,
            Method::GetSessionCapability,
            RadrootsNostrConnectResponse::PendingConnection,
        ),
        (
            RadrootsNostrConnectResponse::RemoteSessionCapability(
                remote_session_capability.clone(),
            ),
            Method::GetSessionCapability,
            RadrootsNostrConnectResponse::RemoteSessionCapability(
                remote_session_capability.clone(),
            ),
        ),
        (
            RadrootsNostrConnectResponse::SignedEvent(
                ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload"),
            ),
            Method::SignEvent,
            RadrootsNostrConnectResponse::SignedEvent(
                ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload"),
            ),
        ),
        (
            RadrootsNostrConnectResponse::Pong,
            Method::Ping,
            RadrootsNostrConnectResponse::Pong,
        ),
        (
            RadrootsNostrConnectResponse::Nip04Encrypt("cipher".to_owned()),
            Method::Nip04Encrypt,
            RadrootsNostrConnectResponse::Nip04Encrypt("cipher".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip04Decrypt("plain".to_owned()),
            Method::Nip04Decrypt,
            RadrootsNostrConnectResponse::Nip04Decrypt("plain".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip44Encrypt("cipher".to_owned()),
            Method::Nip44Encrypt,
            RadrootsNostrConnectResponse::Nip44Encrypt("cipher".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip44Decrypt("plain".to_owned()),
            Method::Nip44Decrypt,
            RadrootsNostrConnectResponse::Nip44Decrypt("plain".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::RelayList(vec![
                relay(RELAY_SECONDARY_WSS),
                relay(RELAY_TERTIARY_WSS),
            ]),
            Method::SwitchRelays,
            RadrootsNostrConnectResponse::RelayList(vec![
                relay(RELAY_SECONDARY_WSS),
                relay(RELAY_TERTIARY_WSS),
            ]),
        ),
        (
            RadrootsNostrConnectResponse::RelayListUnchanged,
            Method::SwitchRelays,
            RadrootsNostrConnectResponse::RelayListUnchanged,
        ),
    ];
    for (response, method, expected) in cases {
        let envelope = response.into_envelope("req").expect("serialize response");
        let parsed =
            RadrootsNostrConnectResponse::from_envelope(&method, envelope).expect("parse response");
        assert_eq!(parsed, expected);
    }

    let error_envelope = RadrootsNostrConnectResponse::Error {
        result: Some(json!("partial")),
        error: "denied".to_owned(),
    }
    .into_envelope("req-error")
    .expect("serialize error response");
    assert_eq!(error_envelope.error.as_deref(), Some("denied"));

    let custom_envelope = RadrootsNostrConnectResponse::Custom {
        result: Some(json!({"ok": true})),
        error: Some("warning".to_owned()),
    }
    .into_envelope("req-custom")
    .expect("serialize custom response");
    assert_eq!(custom_envelope.error.as_deref(), Some("warning"));

    let auth_envelope =
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned())
            .into_envelope("req-auth")
            .expect("serialize auth_url");
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(&Method::SignEvent, auth_envelope,)
            .expect("parse auth_url"),
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned())
    );

    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::custom("publish_note").expect("valid custom NIP-46 method"),
            RadrootsNostrConnectResponseEnvelope {
                id: "req-custom".to_owned(),
                result: Some(json!("ok")),
                error: None,
            },
        )
        .expect("parse custom response without error"),
        RadrootsNostrConnectResponse::Custom {
            result: Some(json!("ok")),
            error: None,
        }
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::custom("publish_note").expect("valid custom NIP-46 method"),
            RadrootsNostrConnectResponseEnvelope {
                id: "req-custom".to_owned(),
                result: Some(json!({"ok": true})),
                error: Some("warning".to_owned()),
            },
        )
        .expect("parse custom response"),
        RadrootsNostrConnectResponse::Custom {
            result: Some(json!({"ok": true})),
            error: Some("warning".to_owned()),
        }
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetPublicKey,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-pending".to_owned(),
                result: None,
                error: Some(RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR.to_owned()),
            },
        )
        .expect("parse typed pending response"),
        RadrootsNostrConnectResponse::PendingConnection
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetSessionCapability,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-pending-capability".to_owned(),
                result: None,
                error: Some(RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR.to_owned()),
            },
        )
        .expect("parse typed pending capability response"),
        RadrootsNostrConnectResponse::PendingConnection
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetPublicKey,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nonpending-public-key".to_owned(),
                result: None,
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse non-pending public key error"),
        RadrootsNostrConnectResponse::Error {
            result: None,
            error: "denied".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetSessionCapability,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-capability-error-with-result".to_owned(),
                result: Some(json!({"code": "retry"})),
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse capability error with result"),
        RadrootsNostrConnectResponse::Error {
            result: Some(json!({"code": "retry"})),
            error: "denied".to_owned(),
        }
    );
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetSessionCapability,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-capability-invalid-result".to_owned(),
                result: Some(json!({"permissions": "ping"})),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { method, .. })
            if method == "get_session_capability"
    ));
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetSessionCapability,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-capability-string-result".to_owned(),
                result: Some(json!(
                    serde_json::to_string(&remote_session_capability)
                        .expect("serialize remote session capability")
                )),
                error: None,
            },
        )
        .expect("parse stringified capability result"),
        RadrootsNostrConnectResponse::RemoteSessionCapability(remote_session_capability.clone(),)
    );
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetSessionCapability,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-capability-invalid-string".to_owned(),
                result: Some(json!("{")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { method, .. })
            if method == "get_session_capability"
    ));
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Ping,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-error".to_owned(),
                result: Some(json!("partial")),
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse error response"),
        RadrootsNostrConnectResponse::Error {
            result: Some(json!("partial")),
            error: "denied".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(serde_json::to_value(&event).expect("event value")),
                error: None,
            },
        )
        .expect("parse object event"),
        RadrootsNostrConnectResponse::SignedEvent(
            ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload")
        )
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!("null")),
                error: None,
            },
        )
        .expect("parse string null"),
        RadrootsNostrConnectResponse::RelayListUnchanged
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(format!("[\"{RELAY_SECONDARY_WSS}\"]"))),
                error: None,
            },
        )
        .expect("parse stringified relay list"),
        RadrootsNostrConnectResponse::RelayList(vec![relay(RELAY_SECONDARY_WSS)])
    );

    assert!(matches!(
        RadrootsNostrConnectResponse::AuthUrl("not-a-url".to_owned()).into_envelope("req"),
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "[redacted auth URL]"
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-auth".to_owned(),
                result: Some(json!("auth_url")),
                error: Some("not-a-url".to_owned()),
            },
        ),
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "[redacted auth URL]"
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetPublicKey,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-key".to_owned(),
                result: Some(json!("bad-key")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Connect,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-connect".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::GetPublicKey,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-key".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Ping,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-ping".to_owned(),
                result: Some(json!("nope")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Ping,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-ping".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Nip04Encrypt,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nip04".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Nip04Encrypt,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nip04".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(json!("not-json")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-event".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Nip04Decrypt,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nip04d".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Nip44Encrypt,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nip44e".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Nip44Decrypt,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-nip44d".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::MissingResult)
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!("[invalid")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!([1])),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(["http://relay.example.com"])),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &Method::Logout,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-logout".to_owned(),
                result: Some(json!("not-ack")),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { method, .. })
            if method == "logout"
    ));
}

#[test]
fn pending_connection_poll_outcome_uses_typed_variants() {
    let remote_session_capability =
        radroots_nostr_connect::prelude::RadrootsNostrConnectRemoteSessionCapability {
            user_public_key: test_public_key(),
            relays: vec![relay(RELAY_PRIMARY_WSS), relay(RELAY_SECONDARY_WSS)],
            permissions: Permissions::from(vec![
                Permission::new(Method::Ping),
                Permission::with_parameter(Method::SignEvent, "kind:1"),
            ]),
        };

    assert_eq!(
        RadrootsNostrConnectResponse::PendingConnection.into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::PendingApproval
    );

    assert_eq!(
        RadrootsNostrConnectResponse::UserPublicKey(test_public_key())
            .into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::Approved(test_public_key())
    );
    assert_eq!(
        RadrootsNostrConnectResponse::RemoteSessionCapability(remote_session_capability.clone())
            .into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::ApprovedCapability(
            remote_session_capability
        )
    );

    assert_eq!(
        RadrootsNostrConnectResponse::Error {
            result: Some(json!("partial")),
            error: "rejected".to_owned(),
        }
        .into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::Rejected {
            message: "rejected".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectResponse::Error {
            result: None,
            error: RADROOTS_NOSTR_CONNECT_PENDING_CONNECTION_ERROR.to_owned(),
        }
        .into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::PendingApproval
    );

    assert_eq!(
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned())
            .into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::AuthChallenge {
            url: "https://auth.example.com/challenge".to_owned(),
        }
    );

    assert!(matches!(
        RadrootsNostrConnectResponse::Pong.into_pending_connection_poll_outcome(),
        RadrootsNostrConnectPendingConnectionPollOutcome::UnexpectedResponse { response }
            if response == "pong"
    ));
}
