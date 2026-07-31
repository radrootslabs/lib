#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use std::str::FromStr;

use radroots_nostr_connect::{
    BunkerUri, ClientUri, Method, Permission,
    permission::{PERMISSION_COUNT_MAX, PERMISSION_PARAMETER_MAX_BYTES, Permissions},
    uri::{ClientMetadata, Uri},
};

use test_fixtures::{FIXTURE_ALICE, RELAY_PRIMARY_WSS};

#[test]
fn methods_and_permissions_are_bounded_and_canonical() {
    assert_eq!(
        Method::custom("publish_note").expect("custom method"),
        Method::from_str("publish_note").expect("parsed custom method")
    );
    for invalid in [
        "",
        "PublishNote",
        "publish-note",
        &"x".repeat(radroots_nostr_connect::method::METHOD_MAX_BYTES + 1),
    ] {
        assert!(Method::from_str(invalid).is_err(), "accepted `{invalid}`");
    }
    assert!(Method::custom("not canonical").is_err());

    let sign_event =
        Permission::try_with_parameter(Method::SignEvent, "kind:1").expect("bounded permission");
    assert_eq!(sign_event.method(), &Method::SignEvent);
    assert_eq!(sign_event.parameter(), Some("kind:1"));
    assert!(
        Permission::try_with_parameter(
            Method::SignEvent,
            "x".repeat(PERMISSION_PARAMETER_MAX_BYTES + 1),
        )
        .is_err()
    );

    let permissions = Permissions::try_from_vec(vec![
        sign_event.clone(),
        Permission::new(Method::Ping),
        sign_event,
    ])
    .expect("canonical permissions");
    assert_eq!(permissions.to_string(), "ping,sign_event:kind:1");
    assert_eq!(permissions.as_slice().len(), 2);
    assert!(
        Permissions::try_from_vec(
            (0..=PERMISSION_COUNT_MAX)
                .map(|index| {
                    Permission::try_with_parameter(Method::SignEvent, index.to_string())
                        .expect("bounded parameter")
                })
                .collect(),
        )
        .is_err()
    );
}

#[test]
fn uri_keys_are_identity_owned_and_secret_diagnostics_are_redacted() {
    let encoded_relay: String =
        url::form_urlencoded::byte_serialize(RELAY_PRIMARY_WSS.as_bytes()).collect();
    let source = format!(
        "bunker://{}?relay={encoded_relay}&secret=do-not-log",
        FIXTURE_ALICE.npub
    );
    let uri = Uri::parse(&source).expect("bunker URI");
    let Uri::Bunker(bunker) = &uri else {
        panic!("expected bunker URI");
    };

    let key: radroots_identity::PublicKey = bunker.remote_signer_public_key();
    assert_eq!(key.to_hex(), FIXTURE_ALICE.public_key_hex);
    assert_eq!(bunker.relays().len(), 1);
    assert_eq!(bunker.secret(), Some("do-not-log"));
    assert!(!format!("{bunker:?}").contains("do-not-log"));
    assert!(!format!("{uri:?}").contains("do-not-log"));

    let canonical = uri.to_string();
    assert!(canonical.starts_with(&format!("bunker://{}?", FIXTURE_ALICE.public_key_hex)));
    let serialized = serde_json::to_string(&uri).expect("serialize URI");
    let decoded: Uri = serde_json::from_str(&serialized).expect("deserialize URI");
    assert_eq!(decoded, uri);

    let duplicate = format!(
        "bunker://{}?relay={encoded_relay}&secret=one&secret=two",
        FIXTURE_ALICE.public_key_hex
    );
    assert!(Uri::parse(&duplicate).is_err());
    let duplicate_relay = format!(
        "bunker://{}?relay={encoded_relay}&relay={encoded_relay}",
        FIXTURE_ALICE.public_key_hex
    );
    let Uri::Bunker(deduplicated) = Uri::parse(&duplicate_relay).expect("deduplicated relay")
    else {
        panic!("expected bunker URI");
    };
    assert_eq!(deduplicated.relays().len(), 1);
    assert!(
        Uri::parse(&format!(
            "bunker://{}?relay={encoded_relay}&secret={}",
            FIXTURE_ALICE.public_key_hex,
            "x".repeat(radroots_nostr_connect::uri::SECRET_MAX_BYTES + 1)
        ))
        .is_err()
    );
    let malformed = "not a uri?secret=do-not-log";
    let error = Uri::parse(malformed).expect_err("malformed URI");
    assert!(!error.to_string().contains("do-not-log"));
    assert!(!format!("{error:?}").contains("do-not-log"));

    let _: &BunkerUri = bunker;
    let _client_type: Option<&ClientUri> = None;
}

#[test]
fn client_metadata_serialization_revalidates_public_fields() {
    let metadata = ClientMetadata::new()
        .with_name("  My Client  ")
        .expect("name")
        .with_url("https://client.example.com")
        .expect("URL")
        .with_requested_permissions(
            Permissions::try_from_vec(vec![Permission::new(Method::Ping)]).expect("permissions"),
        );
    assert_eq!(metadata.name(), Some("My Client"));
    assert_eq!(metadata.url(), Some("https://client.example.com/"));
    assert!(metadata.image().is_none());
    assert_eq!(metadata.requested_permissions().to_string(), "ping");

    let invalid = ClientMetadata {
        name: Some("x".repeat(radroots_nostr_connect::uri::CLIENT_NAME_MAX_BYTES + 1)),
        ..ClientMetadata::default()
    };
    assert!(serde_json::to_string(&invalid).is_err());
}
