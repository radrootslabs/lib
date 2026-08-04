#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::{Event, EventBuilder, JsonUtil, Keys, SecretKey, Timestamp, UnsignedEvent};
use radroots_nostr_connect::client::{
    CancellationToken, Client, Completion, Progress, Receive, Target as ClientTarget,
};
use radroots_nostr_connect::message::{
    PENDING_CONNECTION_ERROR, PendingConnectionOutcome, REMOTE_CAPABILITY_RELAY_COUNT_MAX,
    REQUEST_ID_MAX_BYTES, REQUEST_PARAM_COUNT_MAX, REQUEST_PARAM_MAX_BYTES,
    REQUEST_PARAMS_MAX_BYTES, RESPONSE_ERROR_MAX_BYTES, RESPONSE_RESULT_MAX_BYTES,
    RemoteSessionCapability, RequestId, RequestMessage, ResponseEnvelope, ResponseValidator,
    SignedEvent as ConnectSignedEvent, UnsignedEvent as ConnectUnsignedEvent,
};
use radroots_nostr_connect::permission::{
    PERMISSION_PARAMETER_MAX_BYTES, PERMISSIONS_MAX_BYTES, Permissions,
};
use radroots_nostr_connect::uri::{
    CLIENT_URL_MAX_BYTES, ClientMetadata, RelayUrl, URI_MAX_BYTES, Uri,
};
use radroots_nostr_connect::{Error, Method, Permission, Request, Response};
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
        Error::from(json_error),
        Error::Json(message) if !message.is_empty()
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
        Err(Error::InvalidMethod(value)) if value == " "
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
        Err(Error::InvalidPermission(value)) if value == " "
    ));
    assert!(matches!(
        Permission::from_str("sign_event:"),
        Err(Error::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        Permission::from_str(" :kind"),
        Err(Error::InvalidMethod(_))
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
        Err(Error::InvalidPermission(value)) if value == "sign_event:"
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
        Err(Error::InvalidUrl { .. })
    ));
    assert!(matches!(
        Uri::parse("nostrconnect:///path?relay=wss%3A%2F%2Frelay.example.com&secret=abc"),
        Err(Error::MissingPublicKey)
    ));
    assert!(matches!(
        Uri::parse(&format!("bunker://{}", FIXTURE_ALICE.public_key_hex)),
        Err(Error::MissingRelay)
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(Error::MissingRelay)
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(Error::MissingSecret)
    ));
    assert!(matches!(
        Uri::parse("https://example.com"),
        Err(Error::InvalidUriScheme(value)) if value == "https"
    ));
    assert!(matches!(
        Uri::parse("nostrconnect://bad-key?relay=wss%3A%2F%2Frelay.example.com&secret=abc"),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay=http%3A%2F%2Frelay.example.com&secret=abc",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(Error::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&url=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(Error::InvalidClientMetadata { field: "url", .. })
    ));
    assert!(matches!(
        Uri::parse("bunker://bad-key?relay=wss%3A%2F%2Frelay.example.com"),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "bunker://{}?relay=http%3A%2F%2Frelay.example.com",
            FIXTURE_ALICE.public_key_hex
        )),
        Err(Error::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&perms=sign_event%3A",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(Error::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=abc&image=not-a-url",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(Error::InvalidClientMetadata { field: "image", .. })
    ));
    assert!(matches!(
        Uri::parse(&format!(
            "nostrconnect://{}?relay={}&secret=",
            FIXTURE_ALICE.public_key_hex,
            encode_uri_component(RELAY_PRIMARY_WSS),
        )),
        Err(Error::MissingSecret)
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
        Err(Error::InvalidClientMetadata {
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
            Err(Error::InvalidClientMetadata {
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
            Request::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: Permissions::default(),
                client_metadata: None,
            },
            Method::Connect,
            vec![test_public_key().to_hex()],
        ),
        (
            Request::Connect {
                remote_signer_public_key: test_public_key(),
                secret: None,
                requested_permissions: ping_permission.clone(),
                client_metadata: None,
            },
            Method::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        ),
        (Request::GetPublicKey, Method::GetPublicKey, Vec::new()),
        (
            Request::GetSessionCapability,
            Method::GetSessionCapability,
            Vec::new(),
        ),
        (
            Request::SignEvent(
                ConnectUnsignedEvent::from_json(&unsigned_event().as_json())
                    .expect("unsigned event payload"),
            ),
            Method::SignEvent,
            vec![serde_json::to_string(&unsigned_event()).expect("serialize unsigned event")],
        ),
        (
            Request::Nip04Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            Method::Nip04Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            Request::Nip04Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            Method::Nip04Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (
            Request::Nip44Encrypt {
                public_key: test_public_key(),
                plaintext: "hello".to_owned(),
            },
            Method::Nip44Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        ),
        (
            Request::Nip44Decrypt {
                public_key: test_public_key(),
                ciphertext: "cipher".to_owned(),
            },
            Method::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        ),
        (Request::Ping, Method::Ping, Vec::new()),
        (Request::SwitchRelays, Method::SwitchRelays, Vec::new()),
        (Request::Logout, Method::Logout, Vec::new()),
        (
            Request::Custom {
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
        Request::from_parts(Method::Connect, vec![test_public_key().to_hex()],)
            .expect("connect without secret or perms"),
        Request::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: Permissions::default(),
            client_metadata: None,
        }
    );
    assert_eq!(
        Request::from_parts(
            Method::Connect,
            vec![test_public_key().to_hex(), String::new(), "ping".to_owned()],
        )
        .expect("connect with empty secret"),
        Request::Connect {
            remote_signer_public_key: test_public_key(),
            secret: None,
            requested_permissions: Permissions::from(vec![Permission::new(Method::Ping),]),
            client_metadata: None,
        }
    );
    assert_eq!(
        Request::from_parts(Method::GetPublicKey, Vec::new(),).expect("get_public_key from parts"),
        Request::GetPublicKey
    );
    assert_eq!(
        Request::from_parts(Method::GetSessionCapability, Vec::new(),)
            .expect("get_session_capability from parts"),
        Request::GetSessionCapability
    );
    assert_eq!(
        Request::from_parts(
            Method::Nip04Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        )
        .expect("nip04 encrypt from parts"),
        Request::Nip04Encrypt {
            public_key: test_public_key(),
            plaintext: "hello".to_owned(),
        }
    );
    assert_eq!(
        Request::from_parts(
            Method::Nip04Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        )
        .expect("nip04 decrypt from parts"),
        Request::Nip04Decrypt {
            public_key: test_public_key(),
            ciphertext: "cipher".to_owned(),
        }
    );
    assert_eq!(
        Request::from_parts(
            Method::Nip44Encrypt,
            vec![test_public_key().to_hex(), "hello".to_owned()],
        )
        .expect("nip44 encrypt from parts"),
        Request::Nip44Encrypt {
            public_key: test_public_key(),
            plaintext: "hello".to_owned(),
        }
    );
    assert_eq!(
        Request::from_parts(
            Method::Nip44Decrypt,
            vec![test_public_key().to_hex(), "cipher".to_owned()],
        )
        .expect("nip44 decrypt from parts"),
        Request::Nip44Decrypt {
            public_key: test_public_key(),
            ciphertext: "cipher".to_owned(),
        }
    );
    assert_eq!(
        Request::from_parts(Method::Ping, Vec::new()).expect("ping from parts"),
        Request::Ping
    );
    assert_eq!(
        Request::from_parts(Method::SwitchRelays, Vec::new(),).expect("switch relays from parts"),
        Request::SwitchRelays
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
            Request::from_parts(method, params),
            Err(Error::InvalidParams { expected, .. }) if expected == expected_error
        ));
    }
    assert!(matches!(
        Request::from_parts(Method::Connect, Vec::new()),
        Err(Error::InvalidParams { expected, received, .. })
            if expected == "1 to 4 params" && received == 0
    ));
    assert!(matches!(
        Request::from_parts(Method::Connect, vec!["bad-key".to_owned()],),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Connect,
            vec![test_public_key().to_hex(), "secret".to_owned(), "sign_event:".to_owned()],
        ),
        Err(Error::InvalidPermission(value)) if value == "sign_event:"
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Connect,
            vec![
                test_public_key().to_hex(),
                "secret".to_owned(),
                "ping".to_owned(),
                "extra".to_owned(),
                "too-many".to_owned(),
            ],
        ),
        Err(Error::InvalidParams { expected, received, .. })
            if expected == "1 to 4 params" && received == 5
    ));
    assert!(matches!(
        Request::from_parts(Method::SignEvent, vec!["not-json".to_owned()],),
        Err(Error::InvalidRequestPayload { .. })
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Nip04Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Nip04Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Nip44Encrypt,
            vec!["bad-key".to_owned(), "hello".to_owned()],
        ),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Request::from_parts(
            Method::Nip44Decrypt,
            vec!["bad-key".to_owned(), "cipher".to_owned()],
        ),
        Err(Error::InvalidPublicKey { .. })
    ));

    let custom_message = RequestMessage::new(
        "req-custom",
        Request::Custom {
            method: Method::custom("publish_note").expect("valid custom NIP-46 method"),
            params: vec!["a".to_owned()],
        },
    );
    let encoded = serde_json::to_string(&custom_message).expect("serialize custom request");
    let decoded: RequestMessage =
        serde_json::from_str(&encoded).expect("deserialize custom request");
    assert_eq!(decoded, custom_message);
    assert!(
        serde_json::from_str::<RequestMessage>("{")
            .expect_err("invalid request message json")
            .to_string()
            .contains("EOF")
    );
    assert!(
        serde_json::from_str::<RequestMessage>(
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
    let remote_session_capability = RemoteSessionCapability {
        user_public_key: test_public_key(),
        relays: vec![relay(RELAY_PRIMARY_WSS), relay(RELAY_SECONDARY_WSS)],
        permissions: Permissions::from(vec![
            Permission::new(Method::Ping),
            Permission::with_parameter(Method::SignEvent, "kind:1"),
        ]),
    };
    let cases = vec![
        (
            Response::ConnectAcknowledged,
            Method::Connect,
            Response::ConnectAcknowledged,
        ),
        (
            Response::ConnectSecretEcho("secret".to_owned()),
            Method::Connect,
            Response::ConnectSecretEcho("secret".to_owned()),
        ),
        (
            Response::UserPublicKey(test_public_key()),
            Method::GetPublicKey,
            Response::UserPublicKey(test_public_key()),
        ),
        (
            Response::PendingConnection,
            Method::GetSessionCapability,
            Response::PendingConnection,
        ),
        (
            Response::RemoteSessionCapability(remote_session_capability.clone()),
            Method::GetSessionCapability,
            Response::RemoteSessionCapability(remote_session_capability.clone()),
        ),
        (
            Response::SignedEvent(
                ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload"),
            ),
            Method::SignEvent,
            Response::SignedEvent(
                ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload"),
            ),
        ),
        (Response::Pong, Method::Ping, Response::Pong),
        (
            Response::Nip04Encrypt("cipher".to_owned()),
            Method::Nip04Encrypt,
            Response::Nip04Encrypt("cipher".to_owned()),
        ),
        (
            Response::Nip04Decrypt("plain".to_owned()),
            Method::Nip04Decrypt,
            Response::Nip04Decrypt("plain".to_owned()),
        ),
        (
            Response::Nip44Encrypt("cipher".to_owned()),
            Method::Nip44Encrypt,
            Response::Nip44Encrypt("cipher".to_owned()),
        ),
        (
            Response::Nip44Decrypt("plain".to_owned()),
            Method::Nip44Decrypt,
            Response::Nip44Decrypt("plain".to_owned()),
        ),
        (
            Response::RelayList(vec![relay(RELAY_SECONDARY_WSS), relay(RELAY_TERTIARY_WSS)]),
            Method::SwitchRelays,
            Response::RelayList(vec![relay(RELAY_SECONDARY_WSS), relay(RELAY_TERTIARY_WSS)]),
        ),
        (
            Response::RelayListUnchanged,
            Method::SwitchRelays,
            Response::RelayListUnchanged,
        ),
    ];
    for (response, method, expected) in cases {
        let envelope = response.into_envelope("req").expect("serialize response");
        let parsed = Response::from_envelope(&method, envelope).expect("parse response");
        assert_eq!(parsed, expected);
    }

    let error_envelope = Response::Error {
        result: Some(json!("partial")),
        error: "denied".to_owned(),
    }
    .into_envelope("req-error")
    .expect("serialize error response");
    assert_eq!(error_envelope.error.as_deref(), Some("denied"));

    let custom_envelope = Response::Custom {
        result: Some(json!({"ok": true})),
        error: Some("warning".to_owned()),
    }
    .into_envelope("req-custom")
    .expect("serialize custom response");
    assert_eq!(custom_envelope.error.as_deref(), Some("warning"));

    let auth_envelope = Response::AuthUrl("https://auth.example.com/challenge".to_owned())
        .into_envelope("req-auth")
        .expect("serialize auth_url");
    assert_eq!(
        Response::from_envelope(&Method::SignEvent, auth_envelope,).expect("parse auth_url"),
        Response::AuthUrl("https://auth.example.com/challenge".to_owned())
    );

    assert_eq!(
        Response::from_envelope(
            &Method::custom("publish_note").expect("valid custom NIP-46 method"),
            ResponseEnvelope {
                id: "req-custom".to_owned(),
                result: Some(json!("ok")),
                error: None,
            },
        )
        .expect("parse custom response without error"),
        Response::Custom {
            result: Some(json!("ok")),
            error: None,
        }
    );
    assert_eq!(
        Response::from_envelope(
            &Method::custom("publish_note").expect("valid custom NIP-46 method"),
            ResponseEnvelope {
                id: "req-custom".to_owned(),
                result: Some(json!({"ok": true})),
                error: Some("warning".to_owned()),
            },
        )
        .expect("parse custom response"),
        Response::Custom {
            result: Some(json!({"ok": true})),
            error: Some("warning".to_owned()),
        }
    );
    assert_eq!(
        Response::from_envelope(
            &Method::GetPublicKey,
            ResponseEnvelope {
                id: "req-pending".to_owned(),
                result: None,
                error: Some(PENDING_CONNECTION_ERROR.to_owned()),
            },
        )
        .expect("parse typed pending response"),
        Response::PendingConnection
    );
    assert_eq!(
        Response::from_envelope(
            &Method::GetSessionCapability,
            ResponseEnvelope {
                id: "req-pending-capability".to_owned(),
                result: None,
                error: Some(PENDING_CONNECTION_ERROR.to_owned()),
            },
        )
        .expect("parse typed pending capability response"),
        Response::PendingConnection
    );
    assert_eq!(
        Response::from_envelope(
            &Method::GetPublicKey,
            ResponseEnvelope {
                id: "req-nonpending-public-key".to_owned(),
                result: None,
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse non-pending public key error"),
        Response::Error {
            result: None,
            error: "denied".to_owned(),
        }
    );
    assert_eq!(
        Response::from_envelope(
            &Method::GetSessionCapability,
            ResponseEnvelope {
                id: "req-capability-error-with-result".to_owned(),
                result: Some(json!({"code": "retry"})),
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse capability error with result"),
        Response::Error {
            result: Some(json!({"code": "retry"})),
            error: "denied".to_owned(),
        }
    );
    assert!(matches!(
        Response::from_envelope(
            &Method::GetSessionCapability,
            ResponseEnvelope {
                id: "req-capability-invalid-result".to_owned(),
                result: Some(json!({"permissions": "ping"})),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { method, .. })
            if method == "get_session_capability"
    ));
    assert_eq!(
        Response::from_envelope(
            &Method::GetSessionCapability,
            ResponseEnvelope {
                id: "req-capability-string-result".to_owned(),
                result: Some(json!(
                    serde_json::to_string(&remote_session_capability)
                        .expect("serialize remote session capability")
                )),
                error: None,
            },
        )
        .expect("parse stringified capability result"),
        Response::RemoteSessionCapability(remote_session_capability.clone(),)
    );
    assert!(matches!(
        Response::from_envelope(
            &Method::GetSessionCapability,
            ResponseEnvelope {
                id: "req-capability-invalid-string".to_owned(),
                result: Some(json!("{")),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { method, .. })
            if method == "get_session_capability"
    ));
    assert_eq!(
        Response::from_envelope(
            &Method::Ping,
            ResponseEnvelope {
                id: "req-error".to_owned(),
                result: Some(json!("partial")),
                error: Some("denied".to_owned()),
            },
        )
        .expect("parse error response"),
        Response::Error {
            result: Some(json!("partial")),
            error: "denied".to_owned(),
        }
    );
    assert_eq!(
        Response::from_envelope(
            &Method::SignEvent,
            ResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(serde_json::to_value(&event).expect("event value")),
                error: None,
            },
        )
        .expect("parse object event"),
        Response::SignedEvent(
            ConnectSignedEvent::from_json(&event.as_json()).expect("signed event payload")
        )
    );
    assert_eq!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!("null")),
                error: None,
            },
        )
        .expect("parse string null"),
        Response::RelayListUnchanged
    );
    assert_eq!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(format!("[\"{RELAY_SECONDARY_WSS}\"]"))),
                error: None,
            },
        )
        .expect("parse stringified relay list"),
        Response::RelayList(vec![relay(RELAY_SECONDARY_WSS)])
    );

    assert!(matches!(
        Response::AuthUrl("not-a-url".to_owned()).into_envelope("req"),
        Err(Error::InvalidUrl { value, .. }) if value == "[redacted auth URL]"
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SignEvent,
            ResponseEnvelope {
                id: "req-auth".to_owned(),
                result: Some(json!("auth_url")),
                error: Some("not-a-url".to_owned()),
            },
        ),
        Err(Error::InvalidUrl { value, .. }) if value == "[redacted auth URL]"
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::GetPublicKey,
            ResponseEnvelope {
                id: "req-key".to_owned(),
                result: Some(json!("bad-key")),
                error: None,
            },
        ),
        Err(Error::InvalidPublicKey { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Connect,
            ResponseEnvelope {
                id: "req-connect".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::GetPublicKey,
            ResponseEnvelope {
                id: "req-key".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Ping,
            ResponseEnvelope {
                id: "req-ping".to_owned(),
                result: Some(json!("nope")),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Ping,
            ResponseEnvelope {
                id: "req-ping".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Nip04Encrypt,
            ResponseEnvelope {
                id: "req-nip04".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Nip04Encrypt,
            ResponseEnvelope {
                id: "req-nip04".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SignEvent,
            ResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(json!("not-json")),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SignEvent,
            ResponseEnvelope {
                id: "req-event".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SignEvent,
            ResponseEnvelope {
                id: "req-event".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Nip04Decrypt,
            ResponseEnvelope {
                id: "req-nip04d".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Nip44Encrypt,
            ResponseEnvelope {
                id: "req-nip44e".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Nip44Decrypt,
            ResponseEnvelope {
                id: "req-nip44d".to_owned(),
                result: None,
                error: None,
            },
        ),
        Err(Error::MissingResult)
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!("[invalid")),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!([1])),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(["http://relay.example.com"])),
                error: None,
            },
        ),
        Err(Error::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::SwitchRelays,
            ResponseEnvelope {
                id: "req-switch".to_owned(),
                result: Some(json!(5)),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { .. })
    ));
    assert!(matches!(
        Response::from_envelope(
            &Method::Logout,
            ResponseEnvelope {
                id: "req-logout".to_owned(),
                result: Some(json!("not-ack")),
                error: None,
            },
        ),
        Err(Error::InvalidResponsePayload { method, .. })
            if method == "logout"
    ));
}

#[test]
fn pending_connection_poll_outcome_uses_typed_variants() {
    let remote_session_capability = RemoteSessionCapability {
        user_public_key: test_public_key(),
        relays: vec![relay(RELAY_PRIMARY_WSS), relay(RELAY_SECONDARY_WSS)],
        permissions: Permissions::from(vec![
            Permission::new(Method::Ping),
            Permission::with_parameter(Method::SignEvent, "kind:1"),
        ]),
    };

    assert_eq!(
        Response::PendingConnection.into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::PendingApproval
    );

    assert_eq!(
        Response::UserPublicKey(test_public_key()).into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::Approved(test_public_key())
    );
    assert_eq!(
        Response::RemoteSessionCapability(remote_session_capability.clone())
            .into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::ApprovedCapability(remote_session_capability)
    );

    assert_eq!(
        Response::Error {
            result: Some(json!("partial")),
            error: "rejected".to_owned(),
        }
        .into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::Rejected {
            message: "rejected".to_owned(),
        }
    );
    assert_eq!(
        Response::Error {
            result: None,
            error: PENDING_CONNECTION_ERROR.to_owned(),
        }
        .into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::PendingApproval
    );

    assert_eq!(
        Response::AuthUrl("https://auth.example.com/challenge".to_owned())
            .into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::AuthChallenge {
            url: "https://auth.example.com/challenge".to_owned(),
        }
    );

    assert!(matches!(
        Response::Pong.into_pending_connection_poll_outcome(),
        PendingConnectionOutcome::UnexpectedResponse { response }
            if response == "pong"
    ));
}

#[test]
fn client_and_message_wrappers_cover_redacted_debug_and_value_accessors() {
    let target = ClientTarget::try_new(
        test_public_key(),
        vec![relay(RELAY_PRIMARY_WSS), relay(RELAY_PRIMARY_WSS)],
    )
    .unwrap();
    assert_eq!(target.remote_signer_public_key(), test_public_key());
    assert_eq!(target.relays().len(), 1);
    let client = Client::generate(target.clone()).unwrap();
    assert_eq!(client.target(), &target);
    assert!(client.public_key().is_ok());
    assert!(format!("{client:?}").contains("<redacted>"));
    assert!(Client::from_secret("invalid", target).is_err());

    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
    token.cancel();
    assert!(token.is_cancelled());
    assert!(matches!(
        Completion::response(Response::Pong),
        Completion::Response(_)
    ));
    assert!(matches!(
        Receive::event(
            radroots_nostr_connect::client::ClientEvent::from_json(&signed_event().as_json())
                .unwrap()
        ),
        Receive::Event(_)
    ));
    assert!(
        format!(
            "{:?}",
            Progress::AuthChallenge {
                url: "secret".into()
            }
        )
        .contains("<redacted>")
    );

    let unsigned = ConnectUnsignedEvent::from_json(&unsigned_event().as_json()).unwrap();
    assert_eq!(unsigned.kind(), 1);
    assert!(format!("{unsigned:?}").contains("<redacted>"));
    let signed = ConnectSignedEvent::from_json(&signed_event().as_json()).unwrap();
    assert!(format!("{signed:?}").contains("<redacted>"));

    let envelope = ResponseEnvelope::try_new("request", Some(json!("pong")), None).unwrap();
    assert_eq!(envelope.result(), Some(&json!("pong")));
    assert_eq!(envelope.error(), None);
    assert!(format!("{envelope:?}").contains("has_result"));

    let capability = RemoteSessionCapability::try_new(
        test_public_key(),
        vec![relay(RELAY_PRIMARY_WSS)],
        Permissions::from(vec![Permission::new(Method::Ping)]),
    )
    .unwrap();
    assert_eq!(capability.user_public_key(), test_public_key());
    assert_eq!(capability.relays().len(), 1);
    assert!(capability.permissions().allows_request(&Method::Ping, None));

    let responses = [
        Response::ConnectAcknowledged,
        Response::ConnectSecretEcho("secret".into()),
        Response::LogoutAcknowledged,
        Response::PendingConnection,
        Response::UserPublicKey(test_public_key()),
        Response::RemoteSessionCapability(capability),
        Response::SignedEvent(signed),
        Response::Pong,
        Response::Nip04Encrypt("cipher".into()),
        Response::Nip04Decrypt("plain".into()),
        Response::Nip44Encrypt("cipher".into()),
        Response::Nip44Decrypt("plain".into()),
        Response::RelayList(vec![relay(RELAY_PRIMARY_WSS)]),
        Response::RelayListUnchanged,
        Response::AuthUrl("https://auth.example".into()),
        Response::Error {
            result: None,
            error: "rejected".into(),
        },
        Response::Custom {
            result: None,
            error: None,
        },
    ];
    for response in responses {
        let debug = format!("{response:?}");
        assert!(debug.contains("<redacted>"));
    }
}

#[test]
fn bounded_message_permission_and_uri_validators_cover_each_limit_branch() {
    for invalid_id in ["", " request", "line\nbreak"] {
        assert!(RequestId::parse(invalid_id).is_err());
    }
    assert!(RequestId::parse("x".repeat(REQUEST_ID_MAX_BYTES + 1)).is_err());

    for error in [
        "".to_string(),
        "line\nbreak".to_string(),
        "x".repeat(RESPONSE_ERROR_MAX_BYTES + 1),
    ] {
        assert!(ResponseEnvelope::try_new("request", None, Some(error)).is_err());
    }
    assert!(
        ResponseEnvelope::try_new(
            "request",
            Some(json!("x".repeat(RESPONSE_RESULT_MAX_BYTES + 1))),
            None,
        )
        .is_err()
    );

    let custom = Method::custom("vendor_action").unwrap();
    for params in [
        vec!["x".into(); REQUEST_PARAM_COUNT_MAX + 1],
        vec!["x".repeat(REQUEST_PARAM_MAX_BYTES + 1)],
        vec![
            "x".repeat(REQUEST_PARAM_MAX_BYTES);
            REQUEST_PARAMS_MAX_BYTES / REQUEST_PARAM_MAX_BYTES + 1
        ],
    ] {
        assert!(
            RequestMessage::try_new(
                "request",
                Request::Custom {
                    method: custom.clone(),
                    params
                },
            )
            .is_err()
        );
    }

    for parameter in [
        "".to_string(),
        " padded ".to_string(),
        "comma,value".to_string(),
        "line\nbreak".to_string(),
        "x".repeat(PERMISSION_PARAMETER_MAX_BYTES + 1),
    ] {
        assert!(
            Permissions::try_from_vec(vec![Permission::with_parameter(Method::Ping, parameter,)])
                .is_err()
        );
    }
    assert!(Permissions::from_str(&"p".repeat(PERMISSIONS_MAX_BYTES + 1)).is_err());

    let envelope = ResponseEnvelope::try_new("request", Some(json!("pong")), None).unwrap();
    let mut validator =
        ResponseValidator::new(RequestId::parse("request").unwrap(), test_public_key());
    for fingerprint in ["", "line\nbreak"] {
        assert!(
            validator
                .validate(test_public_key(), fingerprint, &envelope)
                .is_err()
        );
    }
    assert!(
        validator
            .validate(
                test_public_key(),
                "x".repeat(REQUEST_ID_MAX_BYTES + 1),
                &envelope,
            )
            .is_err()
    );

    assert!(Uri::parse(&"x".repeat(URI_MAX_BYTES + 1)).is_err());
    let duplicate_secret = format!(
        "nostrconnect://{}?relay={}&secret=one&secret=two",
        FIXTURE_ALICE.public_key_hex,
        encode_uri_component(RELAY_PRIMARY_WSS),
    );
    assert!(Uri::parse(&duplicate_secret).is_err());

    let too_many_relays = (0..=REMOTE_CAPABILITY_RELAY_COUNT_MAX)
        .map(|index| relay(&format!("wss://relay-{index}.example")))
        .collect::<Vec<_>>();
    assert!(
        RemoteSessionCapability::try_new(test_public_key(), too_many_relays, Permissions::new(),)
            .is_err()
    );
}
