use super::*;
use nostr::nips::nip44::{self, Version};
use nostr::{EventBuilder, JsonUtil, Kind, Tag};
use radroots_event::contract::AuthorRole;
use radroots_event::draft::EventDraft;
use radroots_event::envelope::kind::{
    KIND_CLASSIFIED_LISTING, KIND_COOP, KIND_FARM, TRADE_MUTATION_EVENT_KINDS,
};
use radroots_nostr::signing::sign_frozen_draft;
use radroots_nostr::{event::Event as RadrootsNostrEvent, signing::LocalSigner};
use radroots_nostr_connect::{
    Error as NostrConnectError, Request, Response,
    client::Target,
    message::{RPC_KIND, RequestMessage, SignedEvent as ConnectSignedEvent},
    uri::RelayUrl as ConnectRelayUrl,
};
use radroots_signing::actor::ActorSource;
use std::collections::VecDeque;
use std::future;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use uuid::Uuid;

static USER_KEYS: LazyLock<RadrootsNostrKeys> = LazyLock::new(RadrootsNostrKeys::generate);
static USER_PUBLIC_KEY: LazyLock<String> = LazyLock::new(|| USER_KEYS.public_key().to_hex());
static LOCAL_SIGNER: LazyLock<Arc<LocalSigner>> =
    LazyLock::new(|| Arc::new(LocalSigner::generate().expect("generated local signer")));
static LOCAL_SIGNER_PUBLIC_KEY: LazyLock<String> =
    LazyLock::new(|| LOCAL_SIGNER.public_key().to_hex());
static REMOTE_KEYS: LazyLock<RadrootsNostrKeys> = LazyLock::new(RadrootsNostrKeys::generate);

fn user_keys() -> RadrootsNostrKeys {
    USER_KEYS.clone()
}

fn connect_signed_event(event: RadrootsNostrEvent) -> ConnectSignedEvent {
    ConnectSignedEvent::from_json(&event.as_json()).expect("connect signed event")
}

fn user_pubkey() -> &'static str {
    USER_PUBLIC_KEY.as_str()
}

fn local_signer_pubkey() -> &'static str {
    LOCAL_SIGNER_PUBLIC_KEY.as_str()
}

fn local_sdk_signer() -> RadrootsSdkLocalKeySigner {
    let signer: Arc<RadrootsSdkLocalSignerCapability> = LOCAL_SIGNER.clone();
    RadrootsSdkLocalKeySigner::from_shared_signer(signer, LOCAL_SIGNER.public_key())
        .expect("sdk local signer")
}

fn remote_keys() -> RadrootsNostrKeys {
    REMOTE_KEYS.clone()
}

fn nip46_target(remote_public_key: nostr::PublicKey, relays: Vec<nostr::RelayUrl>) -> Target {
    let remote_public_key =
        radroots_nostr::key::public_key_from_nostr(remote_public_key).expect("identity public key");
    let relays = relays
        .into_iter()
        .map(|relay| ConnectRelayUrl::parse(relay.to_string().as_str()).expect("connect relay"))
        .collect();
    Target::try_new(remote_public_key, relays).expect("NIP-46 target")
}

fn client_keys() -> RadrootsSdkNip46ClientKey {
    RadrootsSdkNip46ClientKey::generate()
}

#[test]
fn nip46_client_key_debug_output_is_always_redacted() {
    assert_eq!(
        format!("{:?}", RadrootsSdkNip46ClientKey::generate()),
        "RadrootsSdkNip46ClientKey(\"[redacted]\")"
    );
}

fn actor() -> Actor {
    Actor::from_public_key_hex(
        user_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("actor")
}

fn local_actor() -> Actor {
    Actor::from_public_key_hex(
        local_signer_pubkey(),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("local signer actor")
}

fn frozen_draft() -> EventDraft {
    frozen_draft_with(
        "radroots.farm.profile.v1",
        user_pubkey(),
        KIND_FARM,
        1_700_000_000,
        vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
        "{}",
    )
}

fn local_frozen_draft() -> EventDraft {
    frozen_draft_with(
        "radroots.farm.profile.v1",
        local_signer_pubkey(),
        KIND_FARM,
        1_700_000_000,
        vec![vec!["d".to_owned(), "sdk-local-signer".to_owned()]],
        "{}",
    )
}

fn frozen_draft_with(
    contract_id: &str,
    pubkey: &str,
    kind: u32,
    created_at: u32,
    tags: Vec<Vec<String>>,
    content: &str,
) -> EventDraft {
    EventDraft::new(
        contract_id,
        kind,
        u64::from(created_at),
        tags,
        content,
        pubkey,
    )
    .expect("frozen draft")
}

fn sign_event(keys: &RadrootsNostrKeys, draft: &EventDraft) -> RadrootsNostrEvent {
    let signed = sign_frozen_draft(keys, draft).expect("signed");
    RadrootsNostrEvent::from_json(signed.raw_json()).expect("event")
}

fn response_event(
    remote_keys: &RadrootsNostrKeys,
    client_public_key: nostr::PublicKey,
    request_id: &str,
    response: Response,
) -> RadrootsNostrEvent {
    let envelope = response
        .into_envelope(request_id)
        .expect("response envelope");
    let payload = serde_json::to_string(&envelope).expect("payload");
    let ciphertext = nip44::encrypt(
        remote_keys.secret_key(),
        &client_public_key,
        payload,
        Version::V2,
    )
    .expect("ciphertext");
    EventBuilder::new(Kind::Custom(RPC_KIND), ciphertext)
        .tag(Tag::public_key(client_public_key))
        .sign_with_keys(remote_keys)
        .expect("response event")
}

fn myc_signer_with_responses(
    responses: Vec<MockNip46Response>,
) -> (RadrootsSdkMycNip46Signer, Arc<MockNip46Transport>) {
    let remote_keys = remote_keys();
    let transport = Arc::new(MockNip46Transport::new(remote_keys.clone(), responses));
    let target = nip46_target(
        remote_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com").expect("relay")],
    );
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys(), target, user_pubkey(), transport.clone())
            .expect("signer");
    (signer, transport)
}

struct MockNip46Transport {
    remote_keys: RadrootsNostrKeys,
    responses: Mutex<VecDeque<MockNip46Response>>,
    published: Mutex<Vec<RadrootsNostrEvent>>,
    inbound: Mutex<VecDeque<RadrootsNostrEvent>>,
}

enum MockNip46Response {
    Respond(Response),
}

impl MockNip46Transport {
    fn new(remote_keys: RadrootsNostrKeys, responses: Vec<MockNip46Response>) -> Self {
        Self {
            remote_keys,
            responses: Mutex::new(responses.into()),
            published: Mutex::new(Vec::new()),
            inbound: Mutex::new(VecDeque::new()),
        }
    }

    fn published(&self) -> Vec<RadrootsNostrEvent> {
        self.published.lock().expect("published lock").clone()
    }

    fn published_request_messages(&self) -> Vec<RequestMessage> {
        self.published()
            .iter()
            .map(|event| request_message_from_event(&self.remote_keys, event))
            .collect()
    }
}

fn request_message_from_event(
    remote_keys: &RadrootsNostrKeys,
    event: &RadrootsNostrEvent,
) -> RequestMessage {
    let payload = nip44::decrypt(remote_keys.secret_key(), &event.pubkey, &event.content)
        .expect("request payload");
    serde_json::from_str(payload.as_str()).expect("request message")
}

impl RadrootsSdkNip46Transport for MockNip46Transport {
    fn publish_request_event<'a>(
        &'a self,
        event: RadrootsNostrEvent,
    ) -> RadrootsSdkNip46TransportFuture<'a, ()> {
        self.published.lock().expect("published lock").push(event);
        let response = self.responses.lock().expect("responses lock").pop_front();
        if let Some(MockNip46Response::Respond(response)) = response {
            let event = self
                .published
                .lock()
                .expect("published lock")
                .last()
                .cloned();
            let event = event.expect("published request event");
            let request = request_message_from_event(&self.remote_keys, &event);
            let response = response_event(&self.remote_keys, event.pubkey, &request.id, response);
            self.inbound
                .lock()
                .expect("inbound lock")
                .push_back(response);
        }
        Box::pin(async { Ok(()) })
    }

    fn next_response_event<'a>(
        &'a self,
    ) -> RadrootsSdkNip46TransportFuture<'a, RadrootsNostrEvent> {
        let next = self.inbound.lock().expect("inbound lock").pop_front();
        Box::pin(async move { next.ok_or(NostrConnectError::RequestTimedOut) })
    }
}

struct HangingNip46Transport {
    published: Mutex<Vec<RadrootsNostrEvent>>,
}

impl HangingNip46Transport {
    fn new() -> Self {
        Self {
            published: Mutex::new(Vec::new()),
        }
    }
}

impl RadrootsSdkNip46Transport for HangingNip46Transport {
    fn publish_request_event<'a>(
        &'a self,
        event: RadrootsNostrEvent,
    ) -> RadrootsSdkNip46TransportFuture<'a, ()> {
        self.published.lock().expect("published lock").push(event);
        Box::pin(async { Ok(()) })
    }

    fn next_response_event<'a>(
        &'a self,
    ) -> RadrootsSdkNip46TransportFuture<'a, RadrootsNostrEvent> {
        Box::pin(future::pending::<
            Result<RadrootsNostrEvent, NostrConnectError>,
        >())
    }
}

#[tokio::test]
async fn local_key_provider_signs_authorized_frozen_draft() {
    let signer = local_sdk_signer();
    let provider = RadrootsSdkSignerProvider::LocalKey(signer.clone());
    let draft = local_frozen_draft();
    let actor = local_actor();
    let mut progress = Vec::new();

    let receipt = provider
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    progress.push(event);
                    Ok(())
                },
            ),
        )
        .await
        .expect("receipt");

    assert_eq!(provider.mode(), RadrootsSdkSignerMode::LocalKey);
    assert_eq!(provider.status(), signer.status());
    assert!(provider.capability().nip46_permissions.is_empty());
    assert_eq!(receipt.mode, RadrootsSdkSignerMode::LocalKey);
    assert_eq!(receipt.signer_pubkey, local_signer_pubkey());
    assert_eq!(receipt.signed_event_id, draft.expected_event_id_hex());
    assert_eq!(
        progress,
        vec![
            RadrootsSdkSignerProgress::RequestStarted {
                mode: RadrootsSdkSignerMode::LocalKey
            },
            RadrootsSdkSignerProgress::RequestCompleted {
                mode: RadrootsSdkSignerMode::LocalKey
            }
        ]
    );
}

#[tokio::test]
async fn local_key_provider_returns_progress_sink_errors_without_transport_state() {
    let signer = local_sdk_signer();
    let draft = local_frozen_draft();
    let actor = local_actor();
    let wrong_actor = Actor::from_public_key_hex(
        &"a".repeat(64),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("wrong actor");

    assert!(matches!(
        signer
            .sign(RadrootsSdkSignRequest::new(
                "farm.publish",
                &wrong_actor,
                &draft,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));

    let started_error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    assert!(matches!(
                        event,
                        RadrootsSdkSignerProgress::RequestStarted {
                            mode: RadrootsSdkSignerMode::LocalKey
                        }
                    ));
                    Err(RadrootsSdkError::InvalidRequest {
                        message: "local progress start refused".to_owned(),
                    })
                },
            ),
        )
        .await
        .expect_err("progress start error");
    assert!(matches!(
        started_error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "local progress start refused"
    ));

    let mut observed = Vec::new();
    let completed_error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    observed.push(event.clone());
                    if matches!(
                        event,
                        RadrootsSdkSignerProgress::RequestCompleted {
                            mode: RadrootsSdkSignerMode::LocalKey
                        }
                    ) {
                        return Err(RadrootsSdkError::InvalidRequest {
                            message: "local progress completion refused".to_owned(),
                        });
                    }
                    Ok(())
                },
            ),
        )
        .await
        .expect_err("progress completion error");
    assert!(matches!(
        completed_error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "local progress completion refused"
    ));
    assert_eq!(
        observed,
        vec![
            RadrootsSdkSignerProgress::RequestStarted {
                mode: RadrootsSdkSignerMode::LocalKey
            },
            RadrootsSdkSignerProgress::RequestCompleted {
                mode: RadrootsSdkSignerMode::LocalKey
            }
        ]
    );
}

#[test]
fn signer_provider_reports_myc_status_capability_and_constructor_errors() {
    assert_eq!(RadrootsSdkSignerMode::LocalKey.as_str(), "local_key");
    assert_eq!(RadrootsSdkSignerMode::MycNip46.as_str(), "myc_nip46");

    let remote_keys = remote_keys();
    let relays = vec![
        nostr::RelayUrl::parse("wss://relay-a.example.com").expect("relay a"),
        nostr::RelayUrl::parse("wss://relay-b.example.com").expect("relay b"),
    ];
    let target = nip46_target(remote_keys.public_key(), relays);
    let transport = Arc::new(MockNip46Transport::new(remote_keys.clone(), Vec::new()));
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys(), target, user_pubkey(), transport.clone())
            .expect("signer");
    let provider = RadrootsSdkSignerProvider::MycNip46(Box::new(signer));

    assert_eq!(provider.mode(), RadrootsSdkSignerMode::MycNip46);
    assert_eq!(
        provider.status(),
        RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::MycNip46,
            state: RadrootsSdkSignerState::Ready,
            signer_pubkey: user_pubkey().to_owned(),
            remote_signer_pubkey: Some(remote_keys.public_key().to_hex()),
            relay_count: 2,
        }
    );
    assert_eq!(
        provider.capability(),
        RadrootsSdkSignerCapability {
            mode: RadrootsSdkSignerMode::MycNip46,
            signer_pubkey: user_pubkey().to_owned(),
            remote_signer_pubkey: Some(remote_keys.public_key().to_hex()),
            relays: vec![
                "wss://relay-a.example.com".to_owned(),
                "wss://relay-b.example.com".to_owned(),
            ],
            can_sign_events: true,
            nip46_permissions: radroots_sdk_myc_nip46_product_permission_strings(),
        }
    );

    let target = nip46_target(remote_keys.public_key(), Vec::new());
    let error =
        match RadrootsSdkMycNip46Signer::new(client_keys(), target, "not-a-pubkey", transport) {
            Ok(_) => panic!("expected invalid pubkey"),
            Err(error) => error,
        };
    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message.contains("myc_nip46 user pubkey is invalid")
    ));
}

#[test]
fn nip46_private_helpers_map_identity_adapter_and_response_edges() {
    assert!(matches!(
        signed_event_from_nip46_response(
            "farm.publish",
            Response::Error {
                result: None,
                error: "operator rejected".to_owned(),
            },
        ),
        Err(RadrootsSdkError::SignerRequestRejected { ref mode, ref reason })
            if mode == "myc_nip46" && reason == "operator rejected"
    ));
    assert!(matches!(
        signed_event_from_nip46_response("farm.publish", Response::PendingConnection),
        Err(RadrootsSdkError::SignerAuthChallengePending { ref mode, auth_url: None })
            if mode == "myc_nip46"
    ));
    assert!(matches!(
        signed_event_from_nip46_response("farm.publish", Response::Pong),
        Err(RadrootsSdkError::SignerProtocol { ref mode, ref reason })
            if mode == "myc_nip46" && reason.contains("farm.publish")
    ));
    assert!(matches!(
        sdk_error_from_nip46_error(NostrConnectError::Transport {
            reason: "relay offline".to_owned(),
        }),
        RadrootsSdkError::SignerTransport { ref mode, ref reason }
            if mode == "myc_nip46" && reason == "relay offline"
    ));
    assert!(matches!(
        sdk_error_from_nip46_error(NostrConnectError::Json("bad json".to_owned())),
        RadrootsSdkError::SignerProtocol { ref mode, ref reason }
            if mode == "myc_nip46" && reason == "bad json"
    ));
    for error in [
        NostrConnectError::Encrypt {
            reason: "encrypt failed".to_owned(),
        },
        NostrConnectError::Decrypt {
            reason: "decrypt failed".to_owned(),
        },
        NostrConnectError::Sign {
            reason: "sign failed".to_owned(),
        },
        NostrConnectError::InvalidRequestPayload {
            method: "sign_event".to_owned(),
            reason: "request payload failed".to_owned(),
        },
        NostrConnectError::InvalidResponsePayload {
            method: "sign_event".to_owned(),
            reason: "response payload failed".to_owned(),
        },
    ] {
        assert!(matches!(
            sdk_error_from_nip46_error(error),
            RadrootsSdkError::SignerProtocol { ref mode, .. } if mode == "myc_nip46"
        ));
    }
    assert!(matches!(
        sdk_error_from_nip46_error(NostrConnectError::InvalidMethod("ping".to_owned())),
        RadrootsSdkError::SignerProtocol { ref mode, ref reason }
            if mode == "myc_nip46" && reason.contains("invalid NIP-46 method")
    ));
}

#[tokio::test]
async fn nip46_transport_adapter_delegates_publish_and_response_poll() {
    let transport = Arc::new(MockNip46Transport::new(remote_keys(), Vec::new()));
    let event = sign_event(&user_keys(), &frozen_draft());
    let mut adapter = RadrootsSdkNip46TransportAdapter {
        transport: transport.as_ref(),
        request_timeout: Duration::from_millis(10),
    };

    adapter
        .publish(ClientEvent::from_json(event.as_json().as_str()).expect("client event"))
        .await
        .expect("publish request");

    assert_eq!(transport.published().len(), 1);
    assert!(matches!(
        adapter.receive(&CancellationToken::new()).await,
        Err(NostrConnectError::RequestTimedOut)
    ));
}

#[test]
fn myc_nip46_product_permissions_cover_sdk_write_event_kinds() {
    let permissions = radroots_sdk_myc_nip46_product_permissions();
    let rendered = radroots_sdk_myc_nip46_product_permission_strings();
    let mut expected_kinds = vec![KIND_FARM, KIND_CLASSIFIED_LISTING];
    expected_kinds.extend_from_slice(&TRADE_MUTATION_EVENT_KINDS);

    assert_eq!(permissions.as_slice().len(), expected_kinds.len());
    assert_eq!(
        RADROOTS_SDK_MYC_NIP46_PRODUCT_SIGN_EVENT_KINDS.as_slice(),
        expected_kinds.as_slice()
    );
    assert_eq!(rendered.len(), permissions.as_slice().len());
    for kind in RADROOTS_SDK_MYC_NIP46_PRODUCT_SIGN_EVENT_KINDS {
        assert!(rendered.contains(&format!("sign_event:{kind}")));
    }
    for old_trade_write_kind in [3422, 3423, 3432] {
        assert!(!rendered.contains(&format!("sign_event:{old_trade_write_kind}")));
    }
    assert!(!rendered.contains(&"sign_event:1".to_owned()));
}

#[tokio::test]
async fn myc_nip46_provider_signs_and_validates_remote_event() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let user_keys = user_keys();
    let draft = frozen_draft();
    let signed = sign_frozen_draft(&user_keys, &draft).expect("signed");
    let signed_event = RadrootsNostrEvent::from_json(signed.raw_json()).expect("event");
    let transport = Arc::new(MockNip46Transport::new(
        remote_keys.clone(),
        vec![MockNip46Response::Respond(Response::SignedEvent(
            connect_signed_event(signed_event),
        ))],
    ));
    let target = nip46_target(
        remote_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com").expect("relay")],
    );
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys, target, user_pubkey(), transport.clone())
            .expect("signer");
    let provider = RadrootsSdkSignerProvider::MycNip46(Box::new(signer));
    assert_eq!(
        provider.capability().nip46_permissions,
        radroots_sdk_myc_nip46_product_permission_strings()
    );
    let actor = actor();
    let mut progress = Vec::new();

    let receipt = provider
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    progress.push(event);
                    Ok(())
                },
            ),
        )
        .await
        .expect("receipt");

    assert_eq!(receipt.mode, RadrootsSdkSignerMode::MycNip46);
    assert_eq!(receipt.signer_pubkey, user_pubkey());
    assert_eq!(
        receipt.remote_signer_pubkey,
        Some(remote_keys.public_key().to_hex())
    );
    assert_eq!(receipt.signed_event, signed);
    assert_eq!(transport.published().len(), 1);
    let request_messages = transport.published_request_messages();
    let sign_event_request = match &request_messages[0].request {
        Request::SignEvent(unsigned_event) => unsigned_event,
        other => panic!("unexpected NIP-46 request: {other:?}"),
    };
    let sign_event_request: nostr::UnsignedEvent =
        serde_json::from_str(&sign_event_request.as_json()).expect("unsigned event payload");
    let request_tags = sign_event_request
        .tags
        .clone()
        .to_vec()
        .into_iter()
        .map(Tag::to_vec)
        .collect::<Vec<_>>();
    assert_eq!(
        sign_event_request.pubkey.to_hex(),
        draft.expected_pubkey().to_hex()
    );
    assert_eq!(
        sign_event_request.created_at.as_secs(),
        draft.created_at_u64()
    );
    assert_eq!(sign_event_request.kind.as_u16(), draft.kind_u32() as u16);
    assert_eq!(request_tags, draft.tags_as_vec());
    assert_eq!(sign_event_request.content, draft.content());
    let request_id = request_messages[0]
        .id
        .strip_prefix("radroots-sdk-myc-nip46-sign-")
        .expect("request id prefix");
    Uuid::parse_str(request_id).expect("uuid request id");
    assert_eq!(
        progress,
        vec![
            RadrootsSdkSignerProgress::RequestStarted {
                mode: RadrootsSdkSignerMode::MycNip46
            },
            RadrootsSdkSignerProgress::RequestCompleted {
                mode: RadrootsSdkSignerMode::MycNip46
            }
        ]
    );
}

#[tokio::test]
async fn myc_nip46_provider_reports_preflight_and_progress_sink_edges() {
    let draft = frozen_draft();
    let actor = actor();
    let (signer, transport) = myc_signer_with_responses(Vec::new());

    let started_error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    assert!(matches!(
                        event,
                        RadrootsSdkSignerProgress::RequestStarted {
                            mode: RadrootsSdkSignerMode::MycNip46
                        }
                    ));
                    Err(RadrootsSdkError::InvalidRequest {
                        message: "myc progress start refused".to_owned(),
                    })
                },
            ),
        )
        .await
        .expect_err("progress start error");
    assert!(matches!(
        started_error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "myc progress start refused"
    ));
    assert!(transport.published().is_empty());

    let wrong_actor = Actor::from_public_key_hex(
        &"a".repeat(64),
        ActorSource::ExplicitPublicKey,
        [AuthorRole::Farmer],
    )
    .expect("wrong actor");
    let actor_error = signer
        .sign(RadrootsSdkSignRequest::new(
            "farm.publish",
            &wrong_actor,
            &draft,
        ))
        .await
        .expect_err("actor mismatch");
    assert!(matches!(
        actor_error,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));
    assert!(transport.published().is_empty());

    let remote_keys = remote_keys();
    let mismatch_transport = Arc::new(MockNip46Transport::new(remote_keys.clone(), Vec::new()));
    let mismatch_target = nip46_target(remote_keys.public_key(), Vec::new());
    let mismatch_signer = RadrootsSdkMycNip46Signer::new(
        client_keys(),
        mismatch_target,
        remote_keys.public_key().to_hex(),
        mismatch_transport.clone(),
    )
    .expect("mismatch signer");
    let signer_error = mismatch_signer
        .sign(RadrootsSdkSignRequest::new("farm.publish", &actor, &draft))
        .await
        .expect_err("signer mismatch");
    assert!(matches!(
        signer_error,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));
    assert!(mismatch_transport.published().is_empty());
}

#[tokio::test]
async fn myc_nip46_provider_returns_completion_progress_errors_after_remote_sign() {
    let user_keys = user_keys();
    let draft = frozen_draft();
    let signed = sign_frozen_draft(&user_keys, &draft).expect("signed");
    let signed_event = RadrootsNostrEvent::from_json(signed.raw_json()).expect("event");
    let (signer, transport) = myc_signer_with_responses(vec![MockNip46Response::Respond(
        Response::SignedEvent(connect_signed_event(signed_event)),
    )]);
    let actor = actor();
    let mut observed = Vec::new();

    let error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    observed.push(event.clone());
                    if matches!(
                        event,
                        RadrootsSdkSignerProgress::RequestCompleted {
                            mode: RadrootsSdkSignerMode::MycNip46
                        }
                    ) {
                        return Err(RadrootsSdkError::InvalidRequest {
                            message: "myc progress completion refused".to_owned(),
                        });
                    }
                    Ok(())
                },
            ),
        )
        .await
        .expect_err("completion progress error");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "myc progress completion refused"
    ));
    assert_eq!(transport.published().len(), 1);
    assert_eq!(
        observed,
        vec![
            RadrootsSdkSignerProgress::RequestStarted {
                mode: RadrootsSdkSignerMode::MycNip46
            },
            RadrootsSdkSignerProgress::RequestCompleted {
                mode: RadrootsSdkSignerMode::MycNip46
            }
        ]
    );
}

#[tokio::test]
async fn myc_nip46_provider_reports_auth_challenge_progress_and_timeout() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let transport = Arc::new(MockNip46Transport::new(
        remote_keys.clone(),
        vec![MockNip46Response::Respond(Response::AuthUrl(
            "https://auth.example.com/challenge".to_owned(),
        ))],
    ));
    let target = nip46_target(remote_keys.public_key(), Vec::new());
    let signer = RadrootsSdkMycNip46Signer::new(client_keys, target, user_pubkey(), transport)
        .expect("signer");
    let mut progress = Vec::new();
    let draft = frozen_draft();
    let actor = actor();

    let error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    progress.push(event);
                    Ok(())
                },
            ),
        )
        .await
        .expect_err("timeout");

    assert!(matches!(
        error,
        RadrootsSdkError::SignerRequestTimedOut { ref mode } if mode == "myc_nip46"
    ));
    assert_eq!(
        progress,
        vec![
            RadrootsSdkSignerProgress::RequestStarted {
                mode: RadrootsSdkSignerMode::MycNip46
            },
            RadrootsSdkSignerProgress::AuthChallenge {
                mode: RadrootsSdkSignerMode::MycNip46,
                url: "https://auth.example.com/challenge".to_owned()
            }
        ]
    );
}

#[tokio::test]
async fn myc_nip46_provider_returns_progress_sink_errors_from_auth_challenge() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let transport = Arc::new(MockNip46Transport::new(
        remote_keys.clone(),
        vec![MockNip46Response::Respond(Response::AuthUrl(
            "https://auth.example.com/challenge".to_owned(),
        ))],
    ));
    let target = nip46_target(remote_keys.public_key(), Vec::new());
    let signer = RadrootsSdkMycNip46Signer::new(client_keys, target, user_pubkey(), transport)
        .expect("signer");
    let draft = frozen_draft();
    let actor = actor();
    let mut observed = Vec::new();

    let error = signer
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event: RadrootsSdkSignerProgress| {
                    observed.push(event.clone());
                    if matches!(
                        event,
                        RadrootsSdkSignerProgress::AuthChallenge {
                            mode: RadrootsSdkSignerMode::MycNip46,
                            ..
                        }
                    ) {
                        return Err(RadrootsSdkError::InvalidRequest {
                            message: "progress sink refused auth challenge".to_owned(),
                        });
                    }
                    Ok(())
                },
            ),
        )
        .await
        .expect_err("progress sink error");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "progress sink refused auth challenge"
    ));
    assert_eq!(observed.len(), 2);
}

#[tokio::test]
async fn myc_nip46_provider_rejects_zero_timeout_policy() {
    let error = RadrootsSdkMycNip46RequestPolicy::new(Duration::ZERO).expect_err("zero timeout");

    assert!(matches!(
        error,
        RadrootsSdkError::SignerUnavailable { ref mode, ref reason }
            if mode == "myc_nip46" && reason.contains("timeout")
    ));

    let target = nip46_target(remote_keys().public_key(), Vec::new());
    let transport = Arc::new(MockNip46Transport::new(remote_keys(), Vec::new()));
    let constructor_error = match RadrootsSdkMycNip46Signer::new_with_request_policy(
        client_keys(),
        target,
        user_pubkey(),
        transport,
        RadrootsSdkMycNip46RequestPolicy {
            request_timeout: Duration::ZERO,
        },
    ) {
        Ok(_) => panic!("expected zero timeout constructor error"),
        Err(error) => error,
    };
    assert!(matches!(
        constructor_error,
        RadrootsSdkError::SignerUnavailable { ref mode, ref reason }
            if mode == "myc_nip46" && reason.contains("timeout")
    ));
}

#[tokio::test]
async fn myc_nip46_provider_times_out_hanging_transport() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let target = nip46_target(remote_keys.public_key(), Vec::new());
    let transport = Arc::new(HangingNip46Transport::new());
    let policy = RadrootsSdkMycNip46RequestPolicy::new(Duration::from_millis(5)).expect("policy");
    let signer = RadrootsSdkMycNip46Signer::new_with_request_policy(
        client_keys,
        target,
        user_pubkey(),
        transport,
        policy,
    )
    .expect("signer");
    let draft = frozen_draft();
    let actor = actor();

    let error = signer
        .sign(RadrootsSdkSignRequest::new("farm.publish", &actor, &draft))
        .await
        .expect_err("timeout");

    assert!(matches!(
        error,
        RadrootsSdkError::SignerRequestTimedOut { ref mode } if mode == "myc_nip46"
    ));
}

#[tokio::test]
async fn myc_nip46_provider_rejects_returned_event_drift() {
    let draft = frozen_draft();
    let wrong_user_keys = remote_keys();
    let wrong_pubkey = wrong_user_keys.public_key().to_hex();
    let cases = vec![
        (
            "pubkey",
            wrong_user_keys,
            frozen_draft_with(
                "radroots.farm.profile.v1",
                &wrong_pubkey,
                KIND_FARM,
                1_700_000_000,
                vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
                "{}",
            ),
        ),
        (
            "id",
            user_keys(),
            frozen_draft_with(
                "radroots.farm.profile.v1",
                user_pubkey(),
                KIND_FARM,
                1_700_000_000,
                vec![vec!["d".to_owned(), "sdk-signer-id-drift".to_owned()]],
                "{}",
            ),
        ),
        (
            "created_at",
            user_keys(),
            frozen_draft_with(
                "radroots.farm.profile.v1",
                user_pubkey(),
                KIND_FARM,
                1_700_000_001,
                vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
                "{}",
            ),
        ),
        (
            "kind",
            user_keys(),
            frozen_draft_with(
                "radroots.farm.coop.v1",
                user_pubkey(),
                KIND_COOP,
                1_700_000_000,
                vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
                "{}",
            ),
        ),
        (
            "tags",
            user_keys(),
            frozen_draft_with(
                "radroots.farm.profile.v1",
                user_pubkey(),
                KIND_FARM,
                1_700_000_000,
                vec![vec!["d".to_owned(), "sdk-signer-tags-drift".to_owned()]],
                "{}",
            ),
        ),
        (
            "content",
            user_keys(),
            frozen_draft_with(
                "radroots.farm.profile.v1",
                user_pubkey(),
                KIND_FARM,
                1_700_000_000,
                vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
                "{\"drift\":true}",
            ),
        ),
    ];

    for (drift_kind, signing_keys, drifted_draft) in cases {
        let client_keys = client_keys();
        let remote_keys = remote_keys();
        let signed_event = sign_event(&signing_keys, &drifted_draft);
        let transport = Arc::new(MockNip46Transport::new(
            remote_keys.clone(),
            vec![MockNip46Response::Respond(Response::SignedEvent(
                connect_signed_event(signed_event),
            ))],
        ));
        let target = nip46_target(remote_keys.public_key(), Vec::new());
        let signer = RadrootsSdkMycNip46Signer::new(client_keys, target, user_pubkey(), transport)
            .expect("signer");
        let actor = actor();

        let error = signer
            .sign(RadrootsSdkSignRequest::new("farm.publish", &actor, &draft))
            .await
            .expect_err(drift_kind);

        assert!(matches!(
            error,
            RadrootsSdkError::SignerReturnedEventDrift { ref operation, .. }
                if operation == "farm.publish"
        ));
    }
}

#[tokio::test]
async fn sdk_builder_installs_configured_signer_provider() {
    let empty_sdk = crate::RadrootsClient::builder()
        .build()
        .await
        .expect("empty sdk");
    let draft = local_frozen_draft();
    let signer_actor = local_actor();
    let error = empty_sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new(
            "farm.publish",
            &signer_actor,
            &draft,
        ))
        .await
        .expect_err("missing configured signer");
    assert!(matches!(
        error,
        RadrootsSdkError::SignerUnavailable { ref mode, ref reason }
            if mode == "configured" && reason.contains("no SDK signer provider")
    ));

    let signer = local_sdk_signer();
    let sdk = crate::RadrootsClient::builder()
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(signer))
        .build()
        .await
        .expect("sdk");

    assert!(sdk.configured_signer().is_some());
    assert!(matches!(
        sdk.signer_status(),
        Some(RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::LocalKey,
            ..
        })
    ));
    let receipt = sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new(
            "farm.publish",
            &signer_actor,
            &draft,
        ))
        .await
        .expect("receipt");
    assert_eq!(receipt.signed_event_id, draft.expected_event_id_hex());
}
