#![cfg(feature = "chat")]

use nostr::{JsonUtil, nips::nip44};
use radroots_domain::{
    ActorRef, ActorRole, AuthorityRef, CommandContextData, ExecutorId, Generation, GrantId,
    OperationId, PublicKey, RouteDigest, ThreadId, UnixSeconds, WorkspaceId,
};
use radroots_protocol::{
    chat::{ChatRumor, ChatSeal},
    command::{CapturedChatCommand, InquireInput},
};
use radroots_signing::{
    issuance::{ArtifactRole, OutcomeKind, SigningIntent, SigningOutcome},
    provider::{ProviderCall, ProviderError},
    request::CancellationSignal,
};
use std::num::NonZeroU64;

const NOW: u64 = 1_700_000_000;
fn keys(last: u8) -> nostr::Keys {
    // Published synthetic test identities, isolated from all host custody.
    let mut bytes = [0; 32];
    bytes[31] = last;
    nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap())
}
fn call(author: PublicKey) -> ProviderCall {
    ProviderCall::new(
        author,
        NonZeroU64::new((NOW + 120) * 1000).unwrap(),
        CancellationSignal::new(),
    )
}
fn command() -> CapturedChatCommand {
    let author = PublicKey::from_hex(&keys(1).public_key().to_hex()).unwrap();
    let peer = PublicKey::from_hex(&keys(2).public_key().to_hex()).unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([1; 32]),
        ActorRef::new(
            author,
            ActorRole::Buyer,
            None,
            Some(GrantId::from_bytes([2; 32])),
        ),
        WorkspaceId::from_bytes([3; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([4; 32]), Generation::new(1).unwrap()),
        RouteDigest::from_bytes([5; 32]),
        None,
        None,
        UnixSeconds::new(NOW + 120).unwrap(),
        None,
    )
    .unwrap();
    CapturedChatCommand::inquire(
        context,
        InquireInput::new(
            ThreadId::from_bytes([6; 32]),
            peer,
            None,
            "private 🥬 Å A\u{030a}\n",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn known_kind_conversion_signs_and_authenticates_the_same_rumor_for_peer_and_self() {
    let author = keys(1);
    let peer = keys(2);
    let command = command();
    let rumor = ChatRumor::from_command(&command, UnixSeconds::new(NOW).unwrap()).unwrap();
    let mut sealed = Vec::new();
    for recipient in rumor.recipients() {
        let receiver = if recipient == &rumor.author() {
            &author
        } else {
            &peer
        };
        let encrypted = nip44::encrypt(
            author.secret_key(),
            &receiver.public_key(),
            rumor.json(),
            nip44::Version::V2,
        )
        .unwrap();
        assert_eq!(encrypted.len(), rumor.budget().rumor_ciphertext_bytes());
        let seal = ChatSeal::new(
            &rumor,
            *recipient,
            UnixSeconds::new(NOW - 10).unwrap(),
            &encrypted,
        )
        .unwrap();
        let intent = SigningIntent::from_chat_seal(
            command.context().operation_id(),
            command.digest(),
            &seal,
            call(rumor.author()),
        )
        .unwrap();
        assert_eq!(intent.role(), ArtifactRole::BusinessSeal);
        assert_eq!(intent.recipient(), Some(*recipient));
        assert_eq!(intent.author(), rumor.author());
        assert_eq!(
            intent.binding().command_digest(),
            command.digest().as_bytes()
        );
        intent
            .check_before_effect(rumor.author(), NOW * 1000)
            .unwrap();
        let event = intent.with_plan(|plan| {
            nostr::UnsignedEvent {
                id: Some(nostr::EventId::from_slice(plan.expected_event_id().as_bytes()).unwrap()),
                pubkey: author.public_key(),
                created_at: nostr::Timestamp::from_secs(plan.created_at()),
                kind: nostr::Kind::Custom(plan.body().kind() as u16),
                tags: nostr::Tags::new(),
                content: plan.body().content().into(),
            }
            .sign_with_keys(&author)
            .unwrap()
        });
        event.verify().unwrap();
        let raw = event.as_json();
        assert_eq!(raw.len(), intent.signed_json_bytes());
        assert!(raw.len() <= rumor.budget().signed_seal_max_bytes());
        assert_eq!(event.id.as_bytes(), &seal.event_id());
        assert!(event.tags.is_empty());
        assert_eq!(
            nip44::decrypt(receiver.secret_key(), &author.public_key(), &event.content).unwrap(),
            rumor.json()
        );
        let captured = SigningOutcome::capture(&intent, raw, None);
        match captured.kind() {
            OutcomeKind::Obtained(artifact) => {
                assert_eq!(artifact.event_id().as_bytes(), &seal.event_id());
                assert_eq!(artifact.observed_at_unix_ms(), None);
                sealed.push(artifact.event_id());
            }
            _ => panic!("real exact seal must verify"),
        }
    }
    assert_eq!(sealed.len(), 2);
    assert_ne!(sealed[0], sealed[1]);
}

#[test]
fn known_seal_conversion_refuses_substituted_signer_identity() {
    let command = command();
    let author = keys(1);
    let peer = keys(2);
    let rumor = ChatRumor::from_command(&command, UnixSeconds::new(NOW).unwrap()).unwrap();
    let encrypted = nip44::encrypt(
        author.secret_key(),
        &peer.public_key(),
        rumor.json(),
        nip44::Version::V2,
    )
    .unwrap();
    let seal = ChatSeal::new(
        &rumor,
        command.peer(),
        UnixSeconds::new(NOW - 10).unwrap(),
        &encrypted,
    )
    .unwrap();
    assert!(matches!(
        SigningIntent::from_chat_seal(
            command.context().operation_id(),
            command.digest(),
            &seal,
            call(command.peer())
        ),
        Err(ProviderError::InvalidInput)
    ));
    assert_eq!(seal.message_id(), rumor.message_id());
    for (operation, digest) in [
        (OperationId::from_bytes([99; 32]), command.digest()),
        (
            command.context().operation_id(),
            radroots_domain::CommandDigest::from_bytes([99; 32]),
        ),
    ] {
        assert!(matches!(
            SigningIntent::from_chat_seal(operation, digest, &seal, call(rumor.author())),
            Err(ProviderError::InvalidInput)
        ));
    }
}
