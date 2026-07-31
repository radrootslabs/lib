#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::nips::nip44::{self, Version};
use nostr::{EventBuilder, Keys, Kind, PublicKey, SecretKey, Tag};
use radroots_nostr_connect::client::{
    CLIENT_EVENT_MAX_BYTES, CancellationPhase, CancellationToken, ClientEvent, Completion,
    EventOutcome, Progress, Receive, Target, Transport, TransportFuture,
};
use radroots_nostr_connect::message::{RPC_KIND, RequestId};
use radroots_nostr_connect::uri::RelayUrl;
use radroots_nostr_connect::{Client, Error, Request, Response};
use std::collections::VecDeque;
use test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB, RELAY_PRIMARY_WSS};

fn keys(secret_key_hex: &str) -> Keys {
    Keys::new(SecretKey::from_hex(secret_key_hex).expect("secret key"))
}

fn identity_public_key(public_key: PublicKey) -> radroots_identity::PublicKey {
    radroots_nostr::key::public_key_from_nostr(public_key).expect("identity public key")
}

fn client(remote_keys: &Keys) -> Client {
    Client::from_secret(
        FIXTURE_ALICE.secret_key_hex,
        Target::try_new(
            identity_public_key(remote_keys.public_key()),
            vec![RelayUrl::parse(RELAY_PRIMARY_WSS).expect("relay")],
        )
        .expect("target"),
    )
    .expect("client")
}

fn response_event(
    remote_keys: &Keys,
    client_public_key: radroots_identity::PublicKey,
    request_id: &str,
    response: Response,
) -> ClientEvent {
    let client_public_key =
        radroots_nostr::key::public_key_to_nostr(client_public_key).expect("client public key");
    let envelope = response
        .into_envelope(request_id)
        .expect("response envelope");
    let payload = serde_json::to_string(&envelope).expect("response JSON");
    let ciphertext = nip44::encrypt(
        remote_keys.secret_key(),
        &client_public_key,
        payload,
        Version::V2,
    )
    .expect("response encryption");
    let event = EventBuilder::new(Kind::Custom(RPC_KIND), ciphertext)
        .tag(Tag::public_key(client_public_key))
        .sign_with_keys(remote_keys)
        .expect("response event");
    ClientEvent::from_json(&serde_json::to_string(&event).expect("event JSON"))
        .expect("client event")
}

struct MockTransport {
    published: Vec<ClientEvent>,
    incoming: VecDeque<Receive>,
    cancel_on_publish: Option<CancellationToken>,
}

impl MockTransport {
    fn new(incoming: impl IntoIterator<Item = Receive>) -> Self {
        Self {
            published: Vec::new(),
            incoming: incoming.into_iter().collect(),
            cancel_on_publish: None,
        }
    }

    fn cancelling_on_publish(token: CancellationToken) -> Self {
        Self {
            published: Vec::new(),
            incoming: VecDeque::new(),
            cancel_on_publish: Some(token),
        }
    }
}

impl Transport for MockTransport {
    fn publish<'a>(&'a mut self, event: ClientEvent) -> TransportFuture<'a, ()> {
        self.published.push(event);
        if let Some(token) = self.cancel_on_publish.as_ref() {
            token.cancel();
        }
        Box::pin(async { Ok(()) })
    }

    fn receive<'a>(
        &'a mut self,
        cancellation: &'a CancellationToken,
    ) -> TransportFuture<'a, Receive> {
        let outcome = if cancellation.is_cancelled() {
            Receive::Cancelled
        } else {
            self.incoming.pop_front().unwrap_or(Receive::TimedOut)
        };
        Box::pin(async move { Ok(outcome) })
    }
}

#[tokio::test]
async fn client_completes_happy_path_after_one_publication() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);
    let response = response_event(
        &remote_keys,
        client.public_key().expect("client public key"),
        "request-happy",
        Response::Pong,
    );
    let mut transport = MockTransport::new([Receive::event(response)]);

    let completion = client
        .execute(
            RequestId::parse("request-happy").expect("request id"),
            Request::Ping,
            &mut transport,
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .await
        .expect("completion");

    assert_eq!(completion, Completion::response(Response::Pong));
    assert_eq!(transport.published.len(), 1);
    assert!(!transport.published[0].as_json().contains("request-happy"));
}

#[tokio::test]
async fn client_reports_auth_progress_before_completion() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);
    let client_public_key = client.public_key().expect("client public key");
    let mut transport = MockTransport::new([
        Receive::event(response_event(
            &remote_keys,
            client_public_key,
            "request-auth",
            Response::AuthUrl("https://auth.example.test/challenge".to_owned()),
        )),
        Receive::event(response_event(
            &remote_keys,
            client_public_key,
            "request-auth",
            Response::Pong,
        )),
    ]);
    let mut progress = Vec::new();

    let completion = client
        .execute(
            RequestId::parse("request-auth").expect("request id"),
            Request::Ping,
            &mut transport,
            &CancellationToken::new(),
            |event| {
                progress.push(event);
                Ok(())
            },
        )
        .await
        .expect("completion");

    assert_eq!(completion, Completion::response(Response::Pong));
    assert_eq!(
        progress,
        [Progress::AuthChallenge {
            url: "https://auth.example.test/challenge".to_owned(),
        }]
    );
}

#[tokio::test]
async fn transport_owns_timeout_handoff() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);
    let mut transport = MockTransport::new([Receive::TimedOut]);

    let error = client
        .execute(
            RequestId::parse("request-timeout").expect("request id"),
            Request::Ping,
            &mut transport,
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .await
        .expect_err("timeout");

    assert_eq!(error, Error::RequestTimedOut);
    assert_eq!(transport.published.len(), 1);
}

#[tokio::test]
async fn cancellation_distinguishes_before_and_after_publication() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);

    let before = CancellationToken::new();
    before.cancel();
    let mut before_transport = MockTransport::new([]);
    assert_eq!(
        client
            .execute(
                RequestId::parse("request-cancel-before").expect("request id"),
                Request::Ping,
                &mut before_transport,
                &before,
                |_| Ok(()),
            )
            .await
            .expect("before-publication cancellation"),
        Completion::Cancelled(CancellationPhase::BeforePublication)
    );
    assert!(before_transport.published.is_empty());

    let after = CancellationToken::new();
    let mut after_transport = MockTransport::cancelling_on_publish(after.clone());
    assert_eq!(
        client
            .execute(
                RequestId::parse("request-cancel-after").expect("request id"),
                Request::Ping,
                &mut after_transport,
                &after,
                |_| Ok(()),
            )
            .await
            .expect("after-publication cancellation"),
        Completion::Cancelled(CancellationPhase::AfterPublication)
    );
    assert_eq!(after_transport.published.len(), 1);
}

#[test]
fn state_machine_ignores_wrong_response_then_completes() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);
    let client_public_key = client.public_key().expect("client public key");
    let mut operation = client
        .prepare(
            RequestId::parse("request-select").expect("request id"),
            Request::Ping,
        )
        .expect("operation");
    operation.mark_published().expect("published");

    let wrong = response_event(
        &remote_keys,
        client_public_key,
        "other-request",
        Response::Pong,
    );
    assert_eq!(
        operation.select(&wrong).expect("wrong response"),
        EventOutcome::Ignore
    );
    let expected = response_event(
        &remote_keys,
        client_public_key,
        "request-select",
        Response::Pong,
    );
    assert_eq!(
        operation.select(&expected).expect("expected response"),
        EventOutcome::Complete(Box::new(Response::Pong))
    );
}

#[test]
fn state_machine_rejects_duplicate_response_event() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    let client = client(&remote_keys);
    let response = response_event(
        &remote_keys,
        client.public_key().expect("client public key"),
        "request-replay",
        Response::Pong,
    );
    let mut operation = client
        .prepare(
            RequestId::parse("request-replay").expect("request id"),
            Request::Ping,
        )
        .expect("operation");
    operation.mark_published().expect("published");
    assert_eq!(
        operation.select(&response).expect("first response"),
        EventOutcome::Complete(Box::new(Response::Pong))
    );
    assert_eq!(
        operation.select(&response).expect_err("duplicate response"),
        Error::ReplayedResponse
    );
}

#[test]
fn client_key_and_event_diagnostics_are_redacted() {
    let remote_keys = keys(FIXTURE_BOB.secret_key_hex);
    assert_eq!(
        Client::from_secret("not-a-secret", client(&remote_keys).target().clone())
            .expect_err("invalid key"),
        Error::InvalidClientKey
    );
    assert!(!format!("{:?}", client(&remote_keys)).contains(FIXTURE_ALICE.secret_key_hex));
    assert_eq!(
        ClientEvent::from_json("not an event").expect_err("invalid event"),
        Error::InvalidClientEvent
    );
    assert_eq!(
        ClientEvent::from_json(&"x".repeat(CLIENT_EVENT_MAX_BYTES + 1))
            .expect_err("oversized event"),
        Error::InvalidClientEvent
    );
    let progress = Progress::AuthChallenge {
        url: "https://auth.example.test/?token=do-not-log".to_owned(),
    };
    assert!(!format!("{progress:?}").contains("do-not-log"));
}
