use nostr::{Event, EventBuilder, Keys, PublicKey, RelayUrl, SecretKey, Timestamp, UnsignedEvent};
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectError, RadrootsNostrConnectMethod, RadrootsNostrConnectPermission,
    RadrootsNostrConnectPermissions, RadrootsNostrConnectRequest,
    RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
    RadrootsNostrConnectResponseEnvelope, RadrootsNostrConnectUri,
};
use radroots_test_fixtures::{
    APP_PRIMARY_HTTPS, CDN_PRIMARY_HTTPS, FIXTURE_ALICE, RELAY_PRIMARY_WSS, RELAY_SECONDARY_WSS,
    RELAY_TERTIARY_WSS,
};
use serde_json::{Value, json};
use std::str::FromStr;

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
        (RadrootsNostrConnectMethod::Connect, "connect"),
        (RadrootsNostrConnectMethod::GetPublicKey, "get_public_key"),
        (RadrootsNostrConnectMethod::SignEvent, "sign_event"),
        (RadrootsNostrConnectMethod::Nip04Encrypt, "nip04_encrypt"),
        (RadrootsNostrConnectMethod::Nip04Decrypt, "nip04_decrypt"),
        (RadrootsNostrConnectMethod::Nip44Encrypt, "nip44_encrypt"),
        (RadrootsNostrConnectMethod::Nip44Decrypt, "nip44_decrypt"),
        (RadrootsNostrConnectMethod::Ping, "ping"),
        (RadrootsNostrConnectMethod::SwitchRelays, "switch_relays"),
    ];
    for (method, raw) in methods {
        assert_eq!(method.as_str(), raw);
        assert_eq!(method.to_string(), raw);
        assert_eq!(
            RadrootsNostrConnectMethod::from_str(raw).expect("parse method"),
            method
        );
    }
    assert_eq!(
        RadrootsNostrConnectMethod::from_str("publish_note").expect("custom method"),
        RadrootsNostrConnectMethod::Custom("publish_note".to_owned())
    );
    assert!(matches!(
        RadrootsNostrConnectMethod::from_str(" "),
        Err(RadrootsNostrConnectError::InvalidMethod(value)) if value == " "
    ));
    assert_eq!(
        serde_json::from_str::<RadrootsNostrConnectMethod>("\"do_work\"")
            .expect("deserialize custom method"),
        RadrootsNostrConnectMethod::Custom("do_work".to_owned())
    );
    assert!(
        serde_json::from_str::<RadrootsNostrConnectMethod>("123")
            .expect_err("non-string method")
            .to_string()
            .contains("invalid type")
    );
    assert!(
        serde_json::from_str::<RadrootsNostrConnectMethod>("\"\"")
            .expect_err("blank method")
            .to_string()
            .contains("invalid NIP-46 method")
    );

    let simple = RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Ping);
    assert_eq!(simple.to_string(), "ping");
    let parameterized = RadrootsNostrConnectPermission::with_parameter(
        RadrootsNostrConnectMethod::SignEvent,
        "1059",
    );
    assert_eq!(parameterized.to_string(), "sign_event:1059");
    assert_eq!(
        RadrootsNostrConnectPermission::from_str("sign_event:1059").expect("parse permission"),
        parameterized
    );
    assert!(matches!(
        RadrootsNostrConnectPermission::from_str(" "),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == " "
    ));
    assert!(matches!(
        RadrootsNostrConnectPermission::from_str("sign_event:"),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        RadrootsNostrConnectPermission::from_str(" :kind"),
        Err(RadrootsNostrConnectError::InvalidMethod(_))
    ));

    let empty = RadrootsNostrConnectPermissions::new();
    assert!(empty.is_empty());
    assert!(empty.as_slice().is_empty());
    assert!(empty.clone().into_vec().is_empty());
    assert_eq!(
        RadrootsNostrConnectPermissions::from_str("  ").expect("empty permissions"),
        empty
    );

    let permissions = RadrootsNostrConnectPermissions::from(vec![
        RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Nip44Encrypt),
        RadrootsNostrConnectPermission::with_parameter(RadrootsNostrConnectMethod::SignEvent, "13"),
    ]);
    assert_eq!(permissions.to_string(), "nip44_encrypt,sign_event:13");
    assert_eq!(
        serde_json::to_string(&permissions).expect("serialize permissions"),
        "\"nip44_encrypt,sign_event:13\""
    );
    assert_eq!(
        serde_json::from_str::<RadrootsNostrConnectPermissions>("\"nip44_encrypt,sign_event:13\"")
            .expect("deserialize permissions"),
        permissions
    );
    assert!(
        serde_json::from_str::<RadrootsNostrConnectPermissions>("123")
            .expect_err("non-string permissions")
            .to_string()
            .contains("invalid type")
    );
    assert!(matches!(
        RadrootsNostrConnectPermissions::from_str("sign_event:,ping"),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
}

#[test]
fn uri_surface_covers_rendering_ignored_queries_and_error_paths() {
    let bunker = RadrootsNostrConnectUri::parse(&format!(
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

    let minimal_client: RadrootsNostrConnectUri = format!(
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

    let metadata_client = RadrootsNostrConnectUri::parse(&format!(
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
        RadrootsNostrConnectUri::parse("not a uri"),
        Err(RadrootsNostrConnectError::InvalidUrl { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(
            "nostrconnect:///path?relay=wss%3A%2F%2Frelay.example.com&secret=abc"
        ),
        Err(RadrootsNostrConnectError::MissingPublicKey)
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!("bunker://{}", FIXTURE_ALICE.public_key_hex)),
        Err(RadrootsNostrConnectError::MissingRelay)
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::MissingRelay)
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?relay={}",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::MissingSecret)
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse("https://example.com"),
        Err(RadrootsNostrConnectError::InvalidUriScheme(value)) if value == "https"
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(
            "nostrconnect://bad-key?relay=wss%3A%2F%2Frelay.example.com&secret=abc"
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?relay=http%3A%2F%2Frelay.example.com&secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&url=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "not-a-url"
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse("bunker://bad-key?relay=wss%3A%2F%2Frelay.example.com"),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "bunker://{}?relay=http%3A%2F%2Frelay.example.com",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(RadrootsNostrConnectError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&perms=sign_event%3A",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        RadrootsNostrConnectUri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&image=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "not-a-url"
    ));
}

#[test]
fn request_surface_covers_variant_methods_serialization_and_validation() {
    let ping_permission =
        RadrootsNostrConnectPermissions::from(vec![RadrootsNostrConnectPermission::new(
            RadrootsNostrConnectMethod::Ping,
        )]);

    let requests = vec![
        (
            RadrootsNostrConnectRequest::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: RadrootsNostrConnectPermissions::default(),
            },
            RadrootsNostrConnectMethod::Connect,
            vec![test_public_key().to_hex()],
        ),
        (
            RadrootsNostrConnectRequest::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: ping_permission.clone(),
            },
            RadrootsNostrConnectMethod::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::GetPublicKey,
            RadrootsNostrConnectMethod::GetPublicKey,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::SignEvent(unsigned_event()),
            RadrootsNostrConnectMethod::SignEvent,
            vec![serde_json::to_string(&unsigned_event()).expect("serialize unsigned event")],
        ),
        (
            RadrootsNostrConnectRequest::Nip04Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            RadrootsNostrConnectMethod::Nip04Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip04Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            RadrootsNostrConnectMethod::Nip04Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip44Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            RadrootsNostrConnectMethod::Nip44Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Nip44Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            RadrootsNostrConnectMethod::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (
            RadrootsNostrConnectRequest::Ping,
            RadrootsNostrConnectMethod::Ping,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::SwitchRelays,
            RadrootsNostrConnectMethod::SwitchRelays,
            Vec::new(),
        ),
        (
            RadrootsNostrConnectRequest::Custom {
                method: RadrootsNostrConnectMethod::Custom("publish_note".to_owned()),
                params: vec!["one".to_owned(), "two".to_owned()],
            },
            RadrootsNostrConnectMethod::Custom("publish_note".to_owned()),
            vec!["one".to_owned(), "two".to_owned()],
        ),
    ];
    for (request, method, params) in requests {
        assert_eq!(request.method(), method);
        assert_eq!(request.to_params(), params);
    }

    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Connect,
            vec![test_public_key().to_hex()],
        )
        .expect("connect without secret or perms"),
        RadrootsNostrConnectRequest::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: RadrootsNostrConnectPermissions::default(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        )
        .expect("connect with empty secret"),
        RadrootsNostrConnectRequest::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: RadrootsNostrConnectPermissions::from(vec![
                RadrootsNostrConnectPermission::new(RadrootsNostrConnectMethod::Ping),
            ]),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::GetPublicKey,
            Vec::new(),
        )
        .expect("get_public_key from parts"),
        RadrootsNostrConnectRequest::GetPublicKey
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Nip04Encrypt,
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
            RadrootsNostrConnectMethod::Nip04Decrypt,
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
            RadrootsNostrConnectMethod::Nip44Encrypt,
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
            RadrootsNostrConnectMethod::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        )
        .expect("nip44 decrypt from parts"),
        RadrootsNostrConnectRequest::Nip44Decrypt {
            public_key: test_public_key(),
            ciphertext: "cipher".to_owned(),
        }
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(RadrootsNostrConnectMethod::Ping, Vec::new())
            .expect("ping from parts"),
        RadrootsNostrConnectRequest::Ping
    );
    assert_eq!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::SwitchRelays,
            Vec::new(),
        )
        .expect("switch relays from parts"),
        RadrootsNostrConnectRequest::SwitchRelays
    );

    for (method, params, expected_error) in [
        (
            RadrootsNostrConnectMethod::GetPublicKey,
            vec!["oops".to_owned()],
            "no params",
        ),
        (
            RadrootsNostrConnectMethod::SignEvent,
            Vec::new(),
            "exactly 1 param",
        ),
        (
            RadrootsNostrConnectMethod::Nip04Encrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            RadrootsNostrConnectMethod::Nip04Decrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            RadrootsNostrConnectMethod::Nip44Encrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            RadrootsNostrConnectMethod::Nip44Decrypt,
            vec!["only-one".to_owned()],
            "exactly 2 params",
        ),
        (
            RadrootsNostrConnectMethod::Ping,
            vec!["oops".to_owned()],
            "no params",
        ),
        (
            RadrootsNostrConnectMethod::SwitchRelays,
            vec!["oops".to_owned()],
            "no params",
        ),
    ] {
        assert!(matches!(
            RadrootsNostrConnectRequest::from_parts(method, params),
            Err(RadrootsNostrConnectError::InvalidParams { expected, .. }) if expected == expected_error
        ));
    }
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(RadrootsNostrConnectMethod::Connect, Vec::new()),
        Err(RadrootsNostrConnectError::InvalidParams { expected, received, .. })
            if expected == "1 to 3 params" && received == 0
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Connect,
            vec!["bad-key".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Connect,
            vec![test_public_key().to_hex(), "secret".to_owned(), "sign_event:".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Connect,
            vec![
                test_public_key().to_hex(),
                "secret".to_owned(),
                "ping".to_owned(),
                "extra".to_owned(),
            ],
        ),
        Err(RadrootsNostrConnectError::InvalidParams { expected, received, .. })
            if expected == "1 to 3 params" && received == 4
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::SignEvent,
            vec!["not-json".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidRequestPayload { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Nip04Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Nip04Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Nip44Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        RadrootsNostrConnectRequest::from_parts(
            RadrootsNostrConnectMethod::Nip44Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(RadrootsNostrConnectError::InvalidPublicKey { .. })
    ));

    let custom_message = RadrootsNostrConnectRequestMessage::new(
        "req-custom",
        RadrootsNostrConnectRequest::Custom {
            method: RadrootsNostrConnectMethod::Custom("publish_note".to_owned()),
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
    let cases = vec![
        (
            RadrootsNostrConnectResponse::ConnectAcknowledged,
            RadrootsNostrConnectMethod::Connect,
            RadrootsNostrConnectResponse::ConnectAcknowledged,
        ),
        (
            RadrootsNostrConnectResponse::ConnectSecretEcho("secret".to_owned()),
            RadrootsNostrConnectMethod::Connect,
            RadrootsNostrConnectResponse::ConnectSecretEcho("secret".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::UserPublicKey(test_public_key()),
            RadrootsNostrConnectMethod::GetPublicKey,
            RadrootsNostrConnectResponse::UserPublicKey(test_public_key()),
        ),
        (
            RadrootsNostrConnectResponse::SignedEvent(event.clone()),
            RadrootsNostrConnectMethod::SignEvent,
            RadrootsNostrConnectResponse::SignedEvent(event.clone()),
        ),
        (
            RadrootsNostrConnectResponse::Pong,
            RadrootsNostrConnectMethod::Ping,
            RadrootsNostrConnectResponse::Pong,
        ),
        (
            RadrootsNostrConnectResponse::Nip04Encrypt("cipher".to_owned()),
            RadrootsNostrConnectMethod::Nip04Encrypt,
            RadrootsNostrConnectResponse::Nip04Encrypt("cipher".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip04Decrypt("plain".to_owned()),
            RadrootsNostrConnectMethod::Nip04Decrypt,
            RadrootsNostrConnectResponse::Nip04Decrypt("plain".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip44Encrypt("cipher".to_owned()),
            RadrootsNostrConnectMethod::Nip44Encrypt,
            RadrootsNostrConnectResponse::Nip44Encrypt("cipher".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::Nip44Decrypt("plain".to_owned()),
            RadrootsNostrConnectMethod::Nip44Decrypt,
            RadrootsNostrConnectResponse::Nip44Decrypt("plain".to_owned()),
        ),
        (
            RadrootsNostrConnectResponse::RelayList(vec![
                relay(RELAY_SECONDARY_WSS),
                relay(RELAY_TERTIARY_WSS),
            ]),
            RadrootsNostrConnectMethod::SwitchRelays,
            RadrootsNostrConnectResponse::RelayList(vec![
                relay(RELAY_SECONDARY_WSS),
                relay(RELAY_TERTIARY_WSS),
            ]),
        ),
        (
            RadrootsNostrConnectResponse::RelayListUnchanged,
            RadrootsNostrConnectMethod::SwitchRelays,
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
        RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::SignEvent,
            auth_envelope,
        )
        .expect("parse auth_url"),
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned())
    );

    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::Custom("publish_note".to_owned()),
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
            &RadrootsNostrConnectMethod::Custom("publish_note".to_owned()),
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
            &RadrootsNostrConnectMethod::Ping,
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
            &RadrootsNostrConnectMethod::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(serde_json::to_value(&event).expect("event value")),
                error: None,
            },
        )
        .expect("parse object event"),
        RadrootsNostrConnectResponse::SignedEvent(event)
    );
    assert_eq!(
        RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::SwitchRelays,
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
            &RadrootsNostrConnectMethod::SwitchRelays,
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
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "not-a-url"
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::SignEvent,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-auth".to_owned(),
                result: Some(json!("auth_url")),
                error: Some("not-a-url".to_owned()),
            },
        ),
        Err(RadrootsNostrConnectError::InvalidUrl { value, .. }) if value == "not-a-url"
    ));
    assert!(matches!(
        RadrootsNostrConnectResponse::from_envelope(
            &RadrootsNostrConnectMethod::GetPublicKey,
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
            &RadrootsNostrConnectMethod::Connect,
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
            &RadrootsNostrConnectMethod::GetPublicKey,
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
            &RadrootsNostrConnectMethod::Ping,
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
            &RadrootsNostrConnectMethod::Ping,
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
            &RadrootsNostrConnectMethod::Nip04Encrypt,
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
            &RadrootsNostrConnectMethod::Nip04Encrypt,
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
            &RadrootsNostrConnectMethod::SignEvent,
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
            &RadrootsNostrConnectMethod::SignEvent,
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
            &RadrootsNostrConnectMethod::SignEvent,
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
            &RadrootsNostrConnectMethod::Nip04Decrypt,
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
            &RadrootsNostrConnectMethod::Nip44Encrypt,
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
            &RadrootsNostrConnectMethod::Nip44Decrypt,
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
            &RadrootsNostrConnectMethod::SwitchRelays,
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
            &RadrootsNostrConnectMethod::SwitchRelays,
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
            &RadrootsNostrConnectMethod::SwitchRelays,
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
            &RadrootsNostrConnectMethod::SwitchRelays,
            RadrootsNostrConnectResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(RadrootsNostrConnectError::InvalidResponsePayload { .. })
    ));
}
