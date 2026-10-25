//! Native storage tests use real synthetic NIP-44 data, never host custody.
#[path = "receipt_delivery.rs"]
mod delivery;
#[path = "receipt_installation.rs"]
mod installation;
#[path = "receipt_issuance.rs"]
mod issuance;
#[path = "receipt_snapshots.rs"]
mod snapshots;
use super::*;
use nostr::{JsonUtil, nips::nip44};
use radroots_domain::EnvelopeEventId;
use radroots_protocol::chat::{ChatRumor, ChatSeal, ChatSigningPlan};
use radroots_protocol::intent::SigningReplay;
use radroots_protocol::nip59::CapturedDmRoute;
use radroots_storage::command::{BoundSigningPlan, PlanBindingOutcome};
fn keys(last: u8) -> nostr::Keys {
    let mut bytes = [0; 32];
    bytes[31] = last;
    nostr::Keys::new(nostr::SecretKey::from_slice(&bytes).unwrap())
}
fn peer_request(binding: &WorkspaceBinding, id: u8) -> AdmissionRequest {
    let request = request(binding, id, "protected complete private plan", "original");
    let mut value: serde_json::Value =
        serde_json::from_slice(request.command().protected_wire()).unwrap();
    value["input"]["peer"] = keys(2).public_key().to_hex().into();
    AdmissionRequest::from_protected_wire(binding.clone(), &serde_json::to_vec(&value).unwrap())
        .unwrap()
}
fn plan(
    command: &radroots_protocol::command::CapturedChatCommand,
    created: u64,
    suffix: &str,
) -> ChatSigningPlan {
    plan_with_routes(command, created, suffix, None)
}
fn plan_with_routes(
    command: &radroots_protocol::command::CapturedChatCommand,
    created: u64,
    suffix: &str,
    selected: Option<&[String]>,
) -> ChatSigningPlan {
    let rumor = ChatRumor::from_command(command, UnixSeconds::new(created).unwrap()).unwrap();
    let author = keys(1);
    let mut seals = Vec::new();
    let mut routes = Vec::new();
    for recipient in rumor.recipients() {
        let target = nostr::PublicKey::from_slice(recipient.as_bytes()).unwrap();
        let ciphertext = nip44::encrypt(
            author.secret_key(),
            &target,
            rumor.json(),
            nip44::Version::V2,
        )
        .unwrap();
        seals.push(
            ChatSeal::new(
                &rumor,
                *recipient,
                UnixSeconds::new(created - 1).unwrap(),
                &ciphertext,
            )
            .unwrap(),
        );
        let signer = if recipient == &key() {
            keys(1)
        } else {
            keys(2)
        };
        let urls = selected.map_or_else(
            || vec![format!("wss://relay.example/{suffix}")],
            <[String]>::to_vec,
        );
        let source = nostr::UnsignedEvent {
            id: None,
            pubkey: signer.public_key(),
            created_at: nostr::Timestamp::from_secs(created - 1),
            kind: nostr::Kind::Custom(10050),
            tags: nostr::Tags::from_list(
                urls.iter()
                    .map(|url| nostr::Tag::parse(["relay", url]).unwrap())
                    .collect(),
            ),
            content: "public signed route provenance ".repeat(1000),
        }
        .sign_with_keys(&signer)
        .unwrap();
        source.verify().unwrap();
        routes.push(
            CapturedDmRoute::from_observed_descriptor(
                *recipient,
                EnvelopeEventId::from_bytes(*source.id.as_bytes()),
                &source.as_json(),
                &urls,
            )
            .unwrap(),
        );
    }
    ChatSigningPlan::new(
        command,
        UnixSeconds::new(created).unwrap(),
        seals,
        routes,
        SigningReplay::NonReplayable,
    )
    .unwrap()
}
fn bound(outcome: PlanBindingOutcome) -> BoundSigningPlan {
    match outcome {
        PlanBindingOutcome::Committed(bound) => bound,
        PlanBindingOutcome::Unknown(_) => panic!("fixture must confirm T1"),
    }
}
