use super::*;
use nostr::nips::nip44::{self, Version};
use nostr::{EventBuilder, JsonUtil, Kind, Tag};
use radroots_events::contract::RadrootsActorRole;
use radroots_events::kinds::{KIND_COOP, KIND_FARM};
use radroots_events_codec::wire::{WireEventParts, to_frozen_draft};
use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrSecretKey};
use radroots_nostr_connect::prelude::{
    RADROOTS_NOSTR_CONNECT_RPC_KIND, RadrootsNostrConnectClientTarget, RadrootsNostrConnectError,
    RadrootsNostrConnectResponse,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

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

fn remote_keys() -> RadrootsNostrKeys {
    keys(REMOTE_SECRET_KEY_HEX)
}

fn client_keys() -> RadrootsNostrKeys {
    keys(CLIENT_SECRET_KEY_HEX)
}

fn actor() -> RadrootsActorContext {
    RadrootsActorContext::test(USER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer]).expect("actor")
}

fn frozen_draft() -> RadrootsFrozenEventDraft {
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
) -> RadrootsFrozenEventDraft {
    to_frozen_draft(
        WireEventParts {
            kind,
            content: content.to_owned(),
            tags,
        },
        contract_id,
        pubkey,
        created_at,
    )
    .expect("frozen draft")
}

fn sign_event(keys: &RadrootsNostrKeys, draft: &RadrootsFrozenEventDraft) -> RadrootsNostrEvent {
    let signed =
        radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(keys, draft).expect("signed");
    RadrootsNostrEvent::from_json(signed.raw_json.as_str()).expect("event")
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

struct MockNip46Transport {
    published: Mutex<Vec<RadrootsNostrEvent>>,
    inbound: Mutex<VecDeque<RadrootsNostrEvent>>,
}

impl MockNip46Transport {
    fn new(inbound: Vec<RadrootsNostrEvent>) -> Self {
        Self {
            published: Mutex::new(Vec::new()),
            inbound: Mutex::new(inbound.into()),
        }
    }

    fn published(&self) -> Vec<RadrootsNostrEvent> {
        self.published.lock().expect("published lock").clone()
    }
}

impl RadrootsSdkNip46Transport for MockNip46Transport {
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
        let next = self.inbound.lock().expect("inbound lock").pop_front();
        Box::pin(async move { next.ok_or(RadrootsNostrConnectError::RequestTimedOut) })
    }
}

#[tokio::test]
async fn local_key_provider_signs_authorized_frozen_draft() {
    let signer = RadrootsSdkLocalKeySigner::new(user_keys()).expect("signer");
    let provider = RadrootsSdkSignerProvider::LocalKey(signer.clone());
    let draft = frozen_draft();
    let actor = actor();
    let mut progress = Vec::new();

    let receipt = provider
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event| {
                    progress.push(event);
                    Ok(())
                },
            ),
        )
        .await
        .expect("receipt");

    assert_eq!(provider.mode(), RadrootsSdkSignerMode::LocalKey);
    assert_eq!(provider.status(), signer.status());
    assert_eq!(receipt.mode, RadrootsSdkSignerMode::LocalKey);
    assert_eq!(receipt.signer_pubkey, USER_PUBLIC_KEY_HEX);
    assert_eq!(receipt.signed_event_id, draft.expected_event_id);
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
async fn myc_nip46_provider_signs_and_validates_remote_event() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let user_keys = user_keys();
    let draft = frozen_draft();
    let signed = radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(&user_keys, &draft)
        .expect("signed");
    let signed_event = RadrootsNostrEvent::from_json(signed.raw_json.as_str()).expect("event");
    let inbound = vec![response_event(
        &remote_keys,
        client_keys.public_key(),
        "radroots-sdk-myc-nip46-sign-1",
        RadrootsNostrConnectResponse::SignedEvent(signed_event),
    )];
    let transport = Arc::new(MockNip46Transport::new(inbound));
    let target = RadrootsNostrConnectClientTarget::new(
        remote_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com").expect("relay")],
    );
    let signer =
        RadrootsSdkMycNip46Signer::new(client_keys, target, USER_PUBLIC_KEY_HEX, transport.clone())
            .expect("signer");
    let provider = RadrootsSdkSignerProvider::MycNip46(signer);
    let actor = actor();
    let mut progress = Vec::new();

    let receipt = provider
        .sign(
            RadrootsSdkSignRequest::new("farm.publish", &actor, &draft).with_progress_sink(
                &mut |event| {
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
async fn myc_nip46_provider_reports_auth_challenge_progress_and_timeout() {
    let client_keys = client_keys();
    let remote_keys = remote_keys();
    let auth = response_event(
        &remote_keys,
        client_keys.public_key(),
        "radroots-sdk-myc-nip46-sign-1",
        RadrootsNostrConnectResponse::AuthUrl("https://auth.example.com/challenge".to_owned()),
    );
    let transport = Arc::new(MockNip46Transport::new(vec![auth]));
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
                &mut |event| {
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
        let transport = Arc::new(MockNip46Transport::new(vec![response_event(
            &remote_keys,
            client_keys.public_key(),
            "radroots-sdk-myc-nip46-sign-1",
            RadrootsNostrConnectResponse::SignedEvent(signed_event),
        )]));
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
    let signer = RadrootsSdkLocalKeySigner::new(user_keys()).expect("signer");
    let sdk = crate::RadrootsSdk::builder()
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(signer))
        .build()
        .await
        .expect("sdk");
    let draft = frozen_draft();

    assert!(sdk.configured_signer().is_some());
    assert!(matches!(
        sdk.signer_status(),
        Some(RadrootsSdkSignerStatus {
            mode: RadrootsSdkSignerMode::LocalKey,
            ..
        })
    ));
    let actor = actor();
    let receipt = sdk
        .sign_with_configured_signer(RadrootsSdkSignRequest::new("farm.publish", &actor, &draft))
        .await
        .expect("receipt");
    assert_eq!(receipt.signed_event_id, draft.expected_event_id);
}
