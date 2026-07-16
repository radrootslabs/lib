use super::*;
use nostr::nips::nip44::{self, Version};
use nostr::{EventBuilder, JsonUtil, Kind, Tag};
use radroots_authority::RadrootsLocalEventSigner;
use radroots_event::contract::RadrootsActorRole;
use radroots_event::draft::RadrootsEventDraft;
use radroots_event::kinds::{KIND_COOP, KIND_FARM, KIND_LISTING, TRADE_MUTATION_EVENT_KINDS};
use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrSecretKey};
use radroots_nostr_connect::prelude::{
    RADROOTS_NOSTR_CONNECT_RPC_KIND, RadrootsNostrConnectClientTarget, RadrootsNostrConnectError,
    RadrootsNostrConnectRequest, RadrootsNostrConnectRequestMessage, RadrootsNostrConnectResponse,
};
use std::collections::VecDeque;
use std::future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const USER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const USER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const REMOTE_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const CLIENT_SECRET_KEY_HEX: &str =
    "4d6c20fdd86857de77ff5cfa5c545751ba2efd126e0b6642dae9764d782d6509";

fn keys(secret_key_hex: &str) -> RadrootsNostrKeys {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

fn user_keys() -> RadrootsNostrKeys {
    keys(USER_SECRET_KEY_HEX)
}

fn local_sdk_signer(keys: RadrootsNostrKeys) -> RadrootsSdkLocalKeySigner {
    RadrootsSdkLocalKeySigner::from_event_signer(
        RadrootsLocalEventSigner::new(keys).expect("local event signer"),
    )
    .expect("sdk local signer")
}

fn remote_keys() -> RadrootsNostrKeys {
    keys(REMOTE_SECRET_KEY_HEX)
}

fn client_keys() -> RadrootsNostrKeys {
    keys(CLIENT_SECRET_KEY_HEX)
}

fn actor() -> RadrootsActorContext {
    RadrootsActorContext::test(USER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer]).expect("actor")
}

fn frozen_draft() -> RadrootsEventDraft {
    frozen_draft_with(
        "radroots.farm.profile.v1",
        USER_PUBLIC_KEY_HEX,
        KIND_FARM,
        1_700_000_000,
        vec![vec!["d".to_owned(), "sdk-signer".to_owned()]],
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
) -> RadrootsEventDraft {
    RadrootsEventDraft::new(
        contract_id,
        kind,
        u64::from(created_at),
        tags,
        content,
        pubkey,
    )
    .expect("frozen draft")
}

fn sign_event(keys: &RadrootsNostrKeys, draft: &RadrootsEventDraft) -> RadrootsNostrEvent {
    let signed =
        radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(keys, draft).expect("signed");
    RadrootsNostrEvent::from_json(signed.raw_json()).expect("event")
}

fn response_event(
    remote_keys: &RadrootsNostrKeys,
    client_public_key: nostr::PublicKey,
    request_id: &str,
    response: RadrootsNostrConnectResponse,
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
    EventBuilder::new(Kind::Custom(RADROOTS_NOSTR_CONNECT_RPC_KIND), ciphertext)
        .tag(Tag::public_key(client_public_key))
        .sign_with_keys(remote_keys)
        .expect("response event")
}

fn myc_signer_with_responses(
    responses: Vec<MockNip46Response>,
) -> (RadrootsSdkMycNip46Signer, Arc<MockNip46Transport>) {
    let remote_keys = remote_keys();
    let transport = Arc::new(MockNip46Transport::new(remote_keys.clone(), responses));
    let target = RadrootsNostrConnectClientTarget::new(
        remote_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com").expect("relay")],
    );
    let signer = RadrootsSdkMycNip46Signer::new(
        client_keys(),
        target,
        USER_PUBLIC_KEY_HEX,
        transport.clone(),
    )
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
    Respond(RadrootsNostrConnectResponse),
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

    fn published_request_messages(&self) -> Vec<RadrootsNostrConnectRequestMessage> {
        self.published()
            .iter()
            .map(|event| request_message_from_event(&self.remote_keys, event))
            .collect()
    }
}

fn request_message_from_event(
    remote_keys: &RadrootsNostrKeys,
    event: &RadrootsNostrEvent,
) -> RadrootsNostrConnectRequestMessage {
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
        Box::pin(async move { next.ok_or(RadrootsNostrConnectError::RequestTimedOut) })
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
            Result<RadrootsNostrEvent, RadrootsNostrConnectError>,
        >())
    }
}

#[tokio::test]
async fn local_key_provider_signs_authorized_frozen_draft() {
    let signer = local_sdk_signer(user_keys());
    let provider = RadrootsSdkSignerProvider::LocalKey(signer.clone());
    let draft = frozen_draft();
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

    assert_eq!(provider.mode(), RadrootsSdkSignerMode::LocalKey);
    assert_eq!(provider.status(), signer.status());
    assert!(provider.capability().nip46_permissions.is_empty());
    assert_eq!(receipt.mode, RadrootsSdkSignerMode::LocalKey);
    assert_eq!(receipt.signer_pubkey, USER_PUBLIC_KEY_HEX);
    assert_eq!(receipt.signed_event_id, draft.expected_event_id_str());
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
    let signer = local_sdk_signer(user_keys());
    let draft = frozen_draft();
    let actor = actor();
    let wrong_actor = RadrootsActorContext::test("a".repeat(64), [RadrootsActorRole::Farmer])
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
    let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), relays);
    let transport = Arc::new(MockNip46Transport::new(remote_keys.clone(), Vec::new()));
    let signer = RadrootsSdkMycNip46Signer::new(
        client_keys(),
        target,
        USER_PUBLIC_KEY_HEX,
        transport.clone(),
    )
    .expect("signer");
    let provider = RadrootsSdkSignerProvider::MycNip46(Box::new(signer));

    assert_eq!(provider.mode(), RadrootsSdkSignerMode::MycNip46);
    assert_eq!(
        provider.status(),
        RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::MycNip46,
            state: RadrootsSdkSignerState::Ready,
            signer_pubkey: USER_PUBLIC_KEY_HEX.to_owned(),
            remote_signer_pubkey: Some(remote_keys.public_key().to_hex()),
            relay_count: 2,
        }
    );
    assert_eq!(
        provider.capability(),
        RadrootsSdkSignerCapability {
            mode: RadrootsSdkSignerMode::MycNip46,
            signer_pubkey: USER_PUBLIC_KEY_HEX.to_owned(),
            remote_signer_pubkey: Some(remote_keys.public_key().to_hex()),
            relays: vec![
                "wss://relay-a.example.com".to_owned(),
                "wss://relay-b.example.com".to_owned(),
            ],
            can_sign_events: true,
            nip46_permissions: radroots_sdk_myc_nip46_product_permission_strings(),
        }
    );

    let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
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
    let pubkey = USER_PUBLIC_KEY_HEX.parse().expect("pubkey");
    let identity = RadrootsSdkSignerIdentityOnly { pubkey };
    assert_eq!(identity.pubkey().as_str(), USER_PUBLIC_KEY_HEX);
    assert!(matches!(
        identity.sign_frozen_draft(&frozen_draft()),
        Err(RadrootsSignerError::Unavailable)
    ));

    assert!(matches!(
        signed_event_from_nip46_response(
            "farm.publish",
            RadrootsNostrConnectResponse::Error {
                result: None,
                error: "operator rejected".to_owned(),
            },
        ),
        Err(RadrootsSdkError::SignerRequestRejected { ref mode, ref reason })
            if mode == "myc_nip46" && reason == "operator rejected"
    ));
    assert!(matches!(
        signed_event_from_nip46_response("farm.publish", RadrootsNostrConnectResponse::PendingConnection),
        Err(RadrootsSdkError::SignerAuthChallengePending { ref mode, auth_url: None })
            if mode == "myc_nip46"
    ));
    assert!(matches!(
        signed_event_from_nip46_response("farm.publish", RadrootsNostrConnectResponse::Pong),
        Err(RadrootsSdkError::SignerProtocol { ref mode, ref reason })
            if mode == "myc_nip46" && reason.contains("farm.publish")
    ));
    assert!(matches!(
        sdk_error_from_nip46_error(RadrootsNostrConnectError::Transport {
            reason: "relay offline".to_owned(),
        }),
        RadrootsSdkError::SignerTransport { ref mode, ref reason }
            if mode == "myc_nip46" && reason == "relay offline"
    ));
    assert!(matches!(
        sdk_error_from_nip46_error(RadrootsNostrConnectError::Json("bad json".to_owned())),
        RadrootsSdkError::SignerProtocol { ref mode, ref reason }
            if mode == "myc_nip46" && reason == "bad json"
    ));
    for error in [
        RadrootsNostrConnectError::Encrypt {
            reason: "encrypt failed".to_owned(),
        },
        RadrootsNostrConnectError::Decrypt {
            reason: "decrypt failed".to_owned(),
        },
        RadrootsNostrConnectError::Sign {
            reason: "sign failed".to_owned(),
        },
        RadrootsNostrConnectError::InvalidRequestPayload {
            method: "sign_event".to_owned(),
            reason: "request payload failed".to_owned(),
        },
        RadrootsNostrConnectError::InvalidResponsePayload {
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
        sdk_error_from_nip46_error(RadrootsNostrConnectError::InvalidMethod("ping".to_owned())),
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
    };

    adapter
        .publish_request_event(event)
        .await
        .expect("publish request");

    assert_eq!(transport.published().len(), 1);
    assert!(matches!(
        adapter.next_response_event().await,
        Err(RadrootsNostrConnectError::RequestTimedOut)
    ));
}

#[test]
fn myc_nip46_product_permissions_cover_sdk_write_event_kinds() {
    let permissions = radroots_sdk_myc_nip46_product_permissions();
    let rendered = radroots_sdk_myc_nip46_product_permission_strings();
    let mut expected_kinds = vec![KIND_FARM, KIND_LISTING];
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
    let signed = radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(&user_keys, &draft)
        .expect("signed");
    let signed_event = RadrootsNostrEvent::from_json(signed.raw_json()).expect("event");
    let transport = Arc::new(MockNip46Transport::new(
        remote_keys.clone(),
        vec![MockNip46Response::Respond(
            RadrootsNostrConnectResponse::SignedEvent(signed_event),
        )],
    ));
    let target = RadrootsNostrConnectClientTarget::new(
        remote_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com").expect("relay")],
    );
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys, target, USER_PUBLIC_KEY_HEX, transport.clone())
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
    assert_eq!(receipt.signer_pubkey, USER_PUBLIC_KEY_HEX);
    assert_eq!(
        receipt.remote_signer_pubkey,
        Some(remote_keys.public_key().to_hex())
    );
    assert_eq!(receipt.signed_event, signed);
    assert_eq!(transport.published().len(), 1);
    let request_messages = transport.published_request_messages();
    let sign_event_request = match &request_messages[0].request {
        RadrootsNostrConnectRequest::SignEvent(unsigned_event) => unsigned_event,
        other => panic!("unexpected NIP-46 request: {other:?}"),
    };
    let request_tags = sign_event_request
        .tags
        .clone()
        .to_vec()
        .into_iter()
        .map(Tag::to_vec)
        .collect::<Vec<_>>();
    assert_eq!(
        sign_event_request.pubkey.to_hex(),
        draft.expected_pubkey_str()
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

    let wrong_actor = RadrootsActorContext::test("a".repeat(64), [RadrootsActorRole::Farmer])
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
    let mismatch_target =
        RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
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
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(mismatch_transport.published().is_empty());
}

#[tokio::test]
async fn myc_nip46_provider_returns_completion_progress_errors_after_remote_sign() {
    let user_keys = user_keys();
    let draft = frozen_draft();
    let signed = radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(&user_keys, &draft)
        .expect("signed");
    let signed_event = RadrootsNostrEvent::from_json(signed.raw_json()).expect("event");
    let (signer, transport) = myc_signer_with_responses(vec![MockNip46Response::Respond(
        RadrootsNostrConnectResponse::SignedEvent(signed_event),
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
        vec![MockNip46Response::Respond(
            RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned()),
        )],
    ));
    let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys, target, USER_PUBLIC_KEY_HEX, transport)
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
        vec![MockNip46Response::Respond(
            RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned()),
        )],
    ));
    let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys, target, USER_PUBLIC_KEY_HEX, transport)
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

    let target = RadrootsNostrConnectClientTarget::new(remote_keys().public_key(), Vec::new());
    let transport = Arc::new(MockNip46Transport::new(remote_keys(), Vec::new()));
    let constructor_error = match RadrootsSdkMycNip46Signer::new_with_request_policy(
        client_keys(),
        target,
        USER_PUBLIC_KEY_HEX,
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
    let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
    let transport = Arc::new(HangingNip46Transport::new());
    let policy = RadrootsSdkMycNip46RequestPolicy::new(Duration::from_millis(5)).expect("policy");
    let signer = RadrootsSdkMycNip46Signer::new_with_request_policy(
        client_keys,
        target,
        USER_PUBLIC_KEY_HEX,
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
                USER_PUBLIC_KEY_HEX,
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
                USER_PUBLIC_KEY_HEX,
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
                USER_PUBLIC_KEY_HEX,
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
                USER_PUBLIC_KEY_HEX,
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
                USER_PUBLIC_KEY_HEX,
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
            vec![MockNip46Response::Respond(
                RadrootsNostrConnectResponse::SignedEvent(signed_event),
            )],
        ));
        let target = RadrootsNostrConnectClientTarget::new(remote_keys.public_key(), Vec::new());
        let signer =
            RadrootsSdkMycNip46Signer::new(client_keys, target, USER_PUBLIC_KEY_HEX, transport)
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
    let draft = frozen_draft();
    let signer_actor = actor();
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

    let signer = local_sdk_signer(user_keys());
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
    assert_eq!(receipt.signed_event_id, draft.expected_event_id_str());
}
