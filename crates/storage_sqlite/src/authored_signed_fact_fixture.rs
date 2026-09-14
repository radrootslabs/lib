// Authentic public conformance vector generic_operational_listing_009.
// Source: contracts/conformance/vectors/event/authored_operations.v1.json
// Source SHA-256: 10a7fc63251e23bd7fb9f133a7754be90e6cdb907340ff190bae659d619ce65b
use radroots_event::{GenericEventDraft, SignedEvent, wire::v1::Nip01EventWire};
use radroots_event_codec::authoring::AuthoredEventPlan;
use radroots_storage::{
    atomic::AtomicCommitDigest,
    authored::{AuthoredArtifact, AuthoredArtifactId, AuthoredOperation, WorkClaim},
    authored_atomic::{AuthoredAtomicCommand, PrepareAuthoredOperation, RecordSignedArtifact},
    authored_delivery::{AuthoredDeliveryIntent, AuthoredDeliveryPlan, AuthoredDeliveryPlanId},
    journal::OperationInstanceId,
};
use radroots_transport::{
    Target, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
};
use std::num::NonZeroU64;

pub(crate) const RAW: &str = r###"{"id":"da14c35c4afe472a2ddef6d7298cc736782eaf09b55511c3b5774ad79468ada8","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1784347200,"kind":30402,"tags":[["d","AAAAAAAAAAAAAAAAAAAAAg"],["p","aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],["a","30340:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:AAAAAAAAAAAAAAAAAAAAAA"],["key","carrot-nantes"],["title","Nantes Carrots"],["category","produce"],["summary","Fresh bunches harvested in Saanich"],["published_at","1700000000"],["radroots:primary_bin","bunch"],["radroots:bin","bunch","1","each"],["radroots:price","bunch","4","CAD","1","each"],["price","4","CAD"],["inventory","24"],["status","active"],["delivery","pickup"],["location","Saanich Peninsula","Victoria","BC","CA"],["g","c28hr"]],"content":"# Nantes Carrots\n\nFresh bunches harvested in Saanich","sig":"07edaeab0b05807d4987346c95fd2441b46949be03f455bfbb3678c07b50fa95fb563b509b33f5d547e7cb74f09475c04e281c66362a8db206e57ee757d60dbe"}"###;

pub(crate) fn event(raw: &str) -> SignedEvent {
    let v: serde_json::Value = serde_json::from_str(raw).unwrap();
    let wire = Nip01EventWire {
        id: v["id"].as_str().unwrap().to_owned(),
        pubkey: v["pubkey"].as_str().unwrap().to_owned(),
        created_at: v["created_at"].as_u64().unwrap(),
        kind: u32::try_from(v["kind"].as_u64().unwrap()).unwrap(),
        tags: serde_json::from_value(v["tags"].clone()).unwrap(),
        content: v["content"].as_str().unwrap().to_owned(),
        sig: v["sig"].as_str().unwrap().to_owned(),
        extra: Default::default(),
    };
    SignedEvent::from_wire_verified_id(wire, raw.to_owned()).unwrap()
}

pub(crate) fn ids() -> (
    OperationInstanceId,
    AuthoredArtifactId,
    AuthoredDeliveryPlanId,
) {
    (
        OperationInstanceId::new([1; 16]).unwrap(),
        AuthoredArtifactId::new([2; 16]).unwrap(),
        AuthoredDeliveryPlanId::new([3; 16]).unwrap(),
    )
}

pub(crate) fn prepare() -> (AuthoredAtomicCommand, SignedEvent) {
    let event = event(RAW);
    let wire = event.wire();
    let plan = AuthoredEventPlan::from_generic(
        GenericEventDraft::new(
            "radroots.operational_listing.published.v1",
            wire.kind,
            wire.created_at,
            wire.tags.clone(),
            wire.content.clone(),
            wire.pubkey.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let (operation, artifact, delivery) = ids();
    let intent = AuthoredDeliveryIntent::new(
        "signed-fact",
        TargetSet::new(vec![Target::nostr_relay("wss://one.example").unwrap()]).unwrap(),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
        100,
    )
    .unwrap();
    let preparation = PrepareAuthoredOperation::new(
        AuthoredOperation::new(operation, vec![artifact], 10).unwrap(),
        vec![AuthoredArtifact::planned(artifact, operation, 0, &plan, 10).unwrap()],
        vec![AuthoredDeliveryPlan::new(delivery, artifact, intent, 10).unwrap()],
        AtomicCommitDigest::new([7; 32]),
        10,
    )
    .unwrap();
    (AuthoredAtomicCommand::Prepare(preparation), event)
}

pub(crate) fn claim(revision: NonZeroU64, token: u8, at: u64) -> WorkClaim {
    WorkClaim::new(
        [token; 16],
        format!("worker-{token}"),
        NonZeroU64::new(u64::from(token)).unwrap(),
        at,
        at + 20,
        revision,
    )
    .unwrap()
}

pub(crate) fn record(event: SignedEvent, claim: WorkClaim, at: u64) -> AuthoredAtomicCommand {
    AuthoredAtomicCommand::RecordSigned(
        RecordSignedArtifact::new(ids().0, ids().1, claim, event, at).unwrap(),
    )
}

// Authentic sibling vector typed_update_escaping_002, deliberately a different plan.
pub(crate) const OTHER_RAW: &str = r###"{"id":"11bcbaeab205194e26ae4d950190c03d18edb1b65f428048e589e4bbdcfa9d50","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1784347200,"kind":1,"tags":[],"content":"Farm update: \"ready\"\\\n🍓","sig":"a6a323774f1ce4aafa6c479e758eb350a7e5c4bdc55c7690beba2ccb1631839a1246a83e62f4b29e74f9afda62802794981570e10c7d4ad41c392dab4066b88c"}"###;
