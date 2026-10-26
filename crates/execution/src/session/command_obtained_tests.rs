//! A refused storage descriptor must not discard authenticated original bytes.
use super::*;
use alloc::vec;
use radroots_domain::*;
use radroots_protocol::{
    command::CapturedOrderAcceptance,
    commerce::{self, CommerceSigningPlan, EncodedCommerceRumor},
    intent::SigningReplay,
    listing::{CapturedListingCommand, PublicSigningPlan, PublishInput},
    plan::SigningTarget,
};
use radroots_signing::{
    issuance::{OutcomeKind, SigningIntent, SigningOutcome},
    provider::ProviderCall,
    request::CancellationSignal,
};
use radroots_storage::{
    command::{AdmissionRequest, BoundSigningPlan, PreparedOperation},
    workspace::{AttachmentInput, WorkspaceBinding, WorkspaceScope},
};
use std::num::NonZeroU64;

fn foreign_attempt() -> SigningAttempt {
    let key =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .unwrap();
    let input = AttachmentInput::new(
        WorkspaceId::from_bytes([1; 32]),
        AuthorityRef::new(ExecutorId::from_bytes([2; 32]), Generation::new(1).unwrap()),
        key,
        "local://model",
        key,
        GrantId::from_bytes([3; 32]),
        CheckpointRef::new(
            CheckpointDigest::from_bytes([4; 32]),
            Sequence::new(0).unwrap(),
            Coverage::Partial,
            vec![],
        )
        .unwrap(),
        RouteDigest::from_bytes([5; 32]),
    )
    .unwrap();
    let binding = WorkspaceBinding::from_authenticated_host(
        input.clone(),
        WorkspaceScope::NewSellerOutbox,
        ActorRef::new(key, ActorRole::Seller, None, Some(input.grant_id())),
        "model",
        Sequence::new(1).unwrap(),
        UnixSeconds::new(200).unwrap(),
        UnixSeconds::new(100).unwrap(),
    )
    .unwrap();
    let context = CommandContextData::new(
        OperationId::from_bytes([8; 32]),
        binding.actor(),
        input.workspace_id(),
        input.authority(),
        input.route_digest(),
        None,
        None,
        UnixSeconds::new(150).unwrap(),
        None,
    )
    .unwrap();
    // Synthetic structural listing only; this descriptor has no store authority.
    let listing = PublishInput::parse(br#"{
        "id":"01010101010101010101010101010101","title":"Carrots",
        "summary":"Ask for terms","description_markdown":"Synthetic",
        "published_at":"100","profile":{"profile":"radroots.listing","version":1,
        "state":"active","product":{"name":"Carrots","description":"Synthetic","producer":null,"origin_summary":null},
        "default_option":"0202020202020202020202020202020202020202020202020202020202020202",
        "options":[{"id":"0202020202020202020202020202020202020202020202020202020202020202",
        "label":"Bunch","unit":{"type":"package","label":"bunch","contents":{"type":"unspecified"}},
        "minimum":"1","increment":"1","price":{"type":"quote_required"},
        "supply":{"type":"available","amount":"10"},"lead_seconds":"0"}],
        "fulfillment":{"modes":["pickup"],"area":"Synthetic","lead_seconds":"0"},
        "authority":null,"media":[]}}"#).unwrap();
    let command = CapturedListingCommand::capture(context, listing).unwrap();
    let now = UnixSeconds::new(100).unwrap();
    let plan = PublicSigningPlan::capture(
        &command,
        now,
        &["wss://synthetic.invalid".into()],
        SigningReplay::NonReplayable,
    )
    .unwrap();
    let prepared = PreparedOperation::from_committed_store(
        AdmissionRequest::new(binding, command).unwrap(),
        TransactionId::from_bytes([9; 32]),
        Sequence::new(1).unwrap(),
        IssuanceWindow::new(now, UnixSeconds::new(150).unwrap()).unwrap(),
    )
    .unwrap();
    let bound = BoundSigningPlan::from_committed_store(
        prepared,
        plan,
        TransactionId::from_bytes([10; 32]),
        now,
    )
    .unwrap();
    SigningAttempt::from_committed_store(
        bound,
        SigningTarget::PublicEvent,
        SigningAttemptId::from_bytes([11; 32]),
        TransactionId::from_bytes([12; 32]),
        now,
    )
    .unwrap()
}

#[test]
fn refused_descriptor_keeps_actual_obtained_bytes_and_original_observation() {
    let command = CapturedOrderAcceptance::parse(include_bytes!(
        "../../tests/fixtures/commerce_verification/command_wire.json"
    ))
    .unwrap();
    let record = commerce::decode(include_bytes!(
        "../../tests/fixtures/commerce_verification/record_wire.json"
    ))
    .unwrap();
    let rumor = EncodedCommerceRumor::capture(record.content()).unwrap();
    let plan = CommerceSigningPlan::parse(
        command.context().operation_id(),
        command.digest(),
        &rumor,
        include_bytes!("../../tests/fixtures/commerce_verification/plan_wire.bin"),
    )
    .unwrap();
    let intent = SigningIntent::from_commerce_seal(
        &plan.seals()[0],
        ProviderCall::new(
            rumor.author(),
            NonZeroU64::new(600000).unwrap(),
            CancellationSignal::new(),
        ),
    )
    .unwrap();
    let wire = include_str!("../../tests/fixtures/commerce_verification/seal_signed.json");
    let (_, OutcomeKind::Obtained(artifact)) =
        SigningOutcome::capture(&intent, wire.into(), None).into_parts()
    else {
        panic!("retained independent bytes must authenticate against the original intent")
    };
    let event = artifact.event_id();
    let retained = Obtained::new(foreign_attempt(), artifact);
    for _ in 0..2 {
        assert!(matches!(
            retained.evidence(),
            Err(OperationError::InvalidEvidence)
        ));
        assert_eq!(retained.operation(), OperationId::from_bytes([8; 32]));
        assert_eq!(retained.artifact.event_id(), event);
        assert_eq!(retained.artifact.observed_at_unix_ms(), None);
        retained.artifact.with_json(|raw| assert_eq!(raw, wire));
    }
}
