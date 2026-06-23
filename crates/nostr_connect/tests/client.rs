#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::nips::nip44::{self, Version};
use nostr::{
    Event, EventBuilder, Keys, Kind, PublicKey, RelayUrl, SecretKey, Tag, Timestamp, UnsignedEvent,
};
use radroots_nostr_connect::prelude::{
    RADROOTS_NOSTR_CONNECT_RPC_KIND, RadrootsNostrConnectClientEventOutcome,
    RadrootsNostrConnectClientProgress, RadrootsNostrConnectClientRequest,
    RadrootsNostrConnectClientTarget, RadrootsNostrConnectClientTransport,
    RadrootsNostrConnectClientTransportFuture, RadrootsNostrConnectError,
    RadrootsNostrConnectMethod, RadrootsNostrConnectRemoteSessionCapability,
    RadrootsNostrConnectRequest, RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
    build_request_event, execute_request_with_transport, parse_response_event,
};
use std::collections::VecDeque;
use test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB, FIXTURE_CAROL, RELAY_PRIMARY_WSS};

fn keys(secret_key_hex: &str) -> Keys {
    let secret_key = SecretKey::from_hex(secret_key_hex).expect("secret key");
    Keys::new(secret_key)
}

fn client_keys() -> Keys {
    keys(FIXTURE_ALICE.secret_key_hex)
}

fn remote_signer_keys() -> Keys {
    keys(FIXTURE_BOB.secret_key_hex)
}

fn other_keys() -> Keys {
    keys(FIXTURE_CAROL.secret_key_hex)
}

fn relay() -> RelayUrl {
    RelayUrl::parse(RELAY_PRIMARY_WSS).expect("relay")
}

fn target(remote_keys: &Keys) -> RadrootsNostrConnectClientTarget {
    RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), vec![relay()])
}

fn unsigned_event(pubkey: PublicKey) -> UnsignedEvent {
    EventBuilder::text_note("remote signing")
        .custom_created_at(Timestamp::from(1_714_078_911))
        .build(pubkey)
}

fn signed_event(keys: &Keys) -> Event {
    EventBuilder::text_note("signed remotely")
        .custom_created_at(Timestamp::from(1_714_078_911))
        .sign_with_keys(keys)
        .expect("signed event")
}

fn response_event(
    remote_keys: &Keys,
    client_public_key: PublicKey,
    request_id: &str,
    response: RadrootsNostrConnectResponse,
) -> Event {
    let envelope = response
        .into_envelope(request_id)
        .expect("response envelope");
    let payload = serde_json::to_string(&envelope).expect("response payload");
    let ciphertext = nip44::encrypt(
        remote_keys.secret_key(),
        &client_public_key,
        payload,
        Version::V2,
    )
    .expect("response ciphertext");

    EventBuilder::new(Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND), ciphertext)
        .tag(Tag::public_key(client_public_key))
        .sign_with_keys(remote_keys)
        .expect("response event")
}

fn untagged_response_event(
    remote_keys: &Keys,
    client_public_key: PublicKey,
    request_id: &str,
    response: RadrootsNostrConnectResponse,
) -> Event {
    let envelope = response
        .into_envelope(request_id)
        .expect("response envelope");
    let payload = serde_json::to_string(&envelope).expect("response payload");
    let ciphertext = nip44::encrypt(
        remote_keys.secret_key(),
        &client_public_key,
        payload,
        Version::V2,
    )
    .expect("response ciphertext");

    EventBuilder::new(Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND), ciphertext)
        .sign_with_keys(remote_keys)
        .expect("response event")
}

fn remote_session_capability(remote_keys: &Keys) -> RadrootsNostrConnectRemoteSessionCapability {
    RadrootsNostrConnectRemoteSessionCapability {
        user_public_key: remote_keys.public_key(),
        relays: vec![relay()],
        permissions: Vec::new().into(),
    }
}

struct MockTransport {
    published: Vec<Event>,
    inbound: VecDeque<Event>,
}

impl MockTransport {
    fn new(inbound: Vec<Event>) -> Self {
        Self {
            published: Vec::new(),
            inbound: inbound.into(),
        }
    }
}

impl RadrootsNostrConnectClientTransport for MockTransport {
    fn publish_request_event<'a>(
        &'a mut self,
        event: Event,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, ()> {
        self.published.push(event);
        Box::pin(async { Ok(()) })
    }

    fn next_response_event<'a>(
        &'a mut self,
    ) -> RadrootsNostrConnectClientTransportFuture<'a, Event> {
        let next = self.inbound.pop_front();
        Box::pin(async move { next.ok_or(RadrootsNostrConnectError::RequestTimedOut) })
    }
}

#[tokio::test]
async fn executes_connect_request_and_secret_echo_response() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let mut transport = MockTransport::new(vec![response_event(
        &remote_keys,
        client_keys.public_key(),
        "req-connect",
        RadrootsNostrConnectResponse::ConnectSecretEcho("connect-secret".to_owned()),
    )]);

    let response = execute_request_with_transport(
        &client_keys,
        &target,
        RadrootsNostrConnectClientRequest::new(
            "req-connect",
            RadrootsNostrConnectRequest::Connect {
                remote_signer_public_key: remote_keys.public_key(),
                secret: Some("connect-secret".to_owned()),
                requested_permissions: Vec::new().into(),
            },
        ),
        &mut transport,
        |_| Ok(()),
    )
    .await
    .expect("connect response");

    assert_eq!(
        response,
        RadrootsNostrConnectResponse::ConnectSecretEcho("connect-secret".to_owned())
    );
    assert_eq!(transport.published.len(), 1);
}

#[tokio::test]
async fn executes_capability_request_and_typed_response() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let capability = remote_session_capability(&remote_keys);
    let mut transport = MockTransport::new(vec![response_event(
        &remote_keys,
        client_keys.public_key(),
        "req-capability",
        RadrootsNostrConnectResponse::RemoteSessionCapability(capability.clone()),
    )]);

    let response = execute_request_with_transport(
        &client_keys,
        &target,
        RadrootsNostrConnectClientRequest::new(
            "req-capability",
            RadrootsNostrConnectRequest::GetSessionCapability,
        ),
        &mut transport,
        |_| Ok(()),
    )
    .await
    .expect("capability response");

    assert_eq!(
        response,
        RadrootsNostrConnectResponse::RemoteSessionCapability(capability)
    );
    assert_eq!(transport.published.len(), 1);
}

#[tokio::test]
async fn reports_timeout_when_transport_has_no_matching_response() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let mut transport = MockTransport::new(Vec::new());

    let error = execute_request_with_transport(
        &client_keys,
        &target,
        RadrootsNostrConnectClientRequest::new("req-timeout", RadrootsNostrConnectRequest::Ping),
        &mut transport,
        |_| Ok(()),
    )
    .await
    .expect_err("timeout");

    assert_eq!(error, RadrootsNostrConnectError::RequestTimedOut);
    assert_eq!(transport.published.len(), 1);
}

#[test]
fn builds_encrypted_request_event_for_remote_signer() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let message =
        RadrootsNostrConnectRequestMessage::new("req-ping", RadrootsNostrConnectRequest::Ping);

    let event = build_request_event(&client_keys, &target, message.clone()).expect("event");

    assert_eq!(event.kind, Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND));
    assert_eq!(event.pubkey, client_keys.public_key());
    assert!(
        event
            .tags
            .public_keys()
            .any(|public_key| *public_key == remote_keys.public_key())
    );
    assert!(!event.content.contains("ping"));

    let decrypted = nip44::decrypt(
        remote_keys.secret_key(),
        &client_keys.public_key(),
        &event.content,
    )
    .expect("decrypt request");
    let decoded: RadrootsNostrConnectRequestMessage =
        serde_json::from_str(&decrypted).expect("decode request");
    assert_eq!(decoded, message);
}

#[test]
fn ignores_response_from_unexpected_signer_identity() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let other_keys = other_keys();
    let target = target(&remote_keys);
    let response = response_event(
        &other_keys,
        client_keys.public_key(),
        "req-ping",
        RadrootsNostrConnectResponse::Pong,
    );

    let outcome = parse_response_event(
        &client_keys,
        &target,
        "req-ping",
        &RadrootsNostrConnectMethod::Ping,
        &response,
    )
    .expect("parse response");

    assert_eq!(outcome, RadrootsNostrConnectClientEventOutcome::Ignore);
}

#[tokio::test]
async fn executes_request_through_transport_with_auth_progress() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let signed = signed_event(&remote_keys);
    let inbound = vec![
        response_event(
            &remote_keys,
            client_keys.public_key(),
            "other-request",
            RadrootsNostrConnectResponse::Pong,
        ),
        response_event(
            &remote_keys,
            client_keys.public_key(),
            "req-sign",
            RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned()),
        ),
        response_event(
            &remote_keys,
            client_keys.public_key(),
            "req-sign",
            RadrootsNostrConnectResponse::SignedEvent(signed.clone()),
        ),
    ];
    let mut transport = MockTransport::new(inbound);
    let mut progress = Vec::new();

    let response = execute_request_with_transport(
        &client_keys,
        &target,
        RadrootsNostrConnectClientRequest::new(
            "req-sign",
            RadrootsNostrConnectRequest::SignEvent(unsigned_event(remote_keys.public_key())),
        ),
        &mut transport,
        |event| {
            progress.push(event);
            Ok(())
        },
    )
    .await
    .expect("response");

    assert_eq!(transport.published.len(), 1);
    assert_eq!(
        progress,
        vec![RadrootsNostrConnectClientProgress::AuthChallenge {
            url: "https://auth.example.com/challenge".to_owned()
        }]
    );
    assert_eq!(response, RadrootsNostrConnectResponse::SignedEvent(signed));
}

#[tokio::test]
async fn ignores_events_not_addressed_by_expected_signer_and_client() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let other_keys = other_keys();
    let target = target(&remote_keys);
    let wrong_author = EventBuilder::new(
        Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND),
        "not encrypted for this client",
    )
    .tag(Tag::public_key(client_keys.public_key()))
    .sign_with_keys(&other_keys)
    .expect("wrong author event");
    let missing_client_tag = untagged_response_event(
        &remote_keys,
        client_keys.public_key(),
        "req-ping",
        RadrootsNostrConnectResponse::Pong,
    );
    let valid = response_event(
        &remote_keys,
        client_keys.public_key(),
        "req-ping",
        RadrootsNostrConnectResponse::Pong,
    );
    let mut transport = MockTransport::new(vec![wrong_author, missing_client_tag, valid]);

    let response = execute_request_with_transport(
        &client_keys,
        &target,
        RadrootsNostrConnectClientRequest::new("req-ping", RadrootsNostrConnectRequest::Ping),
        &mut transport,
        |_| Ok(()),
    )
    .await
    .expect("response");

    assert_eq!(response, RadrootsNostrConnectResponse::Pong);
    assert_eq!(transport.published.len(), 1);
}

#[test]
fn reports_decryption_failure_from_expected_signer() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let malformed = EventBuilder::new(
        Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND),
        "not nip44 ciphertext",
    )
    .tag(Tag::public_key(client_keys.public_key()))
    .sign_with_keys(&remote_keys)
    .expect("malformed response");

    let error = parse_response_event(
        &client_keys,
        &target,
        "req-ping",
        &RadrootsNostrConnectMethod::Ping,
        &malformed,
    )
    .expect_err("decrypt failure");

    assert!(matches!(
        error,
        RadrootsNostrConnectError::Decrypt { reason } if !reason.is_empty()
    ));
}

#[test]
fn parses_auth_challenge_as_progress_without_consuming_final_response() {
    let client_keys = client_keys();
    let remote_keys = remote_signer_keys();
    let target = target(&remote_keys);
    let auth = response_event(
        &remote_keys,
        client_keys.public_key(),
        "req-sign",
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/continue".to_owned()),
    );

    let outcome = parse_response_event(
        &client_keys,
        &target,
        "req-sign",
        &RadrootsNostrConnectMethod::SignEvent,
        &auth,
    )
    .expect("parse auth");

    assert_eq!(
        outcome,
        RadrootsNostrConnectClientEventOutcome::Progress(
            RadrootsNostrConnectClientProgress::AuthChallenge {
                url: "https://auth.example.com/continue".to_owned()
            }
        )
    );
}
