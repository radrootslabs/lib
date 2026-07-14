use super::{
    DvmProofMode, DvmTradeTransitionProofEnqueueRequest, DvmTradeTransitionProofPrepareRequest,
    DvmValidationReceiptIngestRequest, RadrootsTradeInventoryBinWitnessDto,
    dvm_trade_transition_proof_plan, sdk_timestamp_ms,
};
use crate::{
    NostrRelayUrlPolicy, RadrootsSdkError, RadrootsSdkTimestamp, SdkIdempotencyKey,
    SyncProjectionRefreshRequest, TargetPolicy,
};
use radroots_authority::RadrootsActorContext;
use radroots_event::{
    contract::RadrootsActorRole,
    draft::RadrootsSignedEvent,
    ids::{RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsPublicKey},
    kinds::KIND_TRADE_TRANSITION_PROOF_REQUEST,
    wire::RadrootsNip01EventWire,
};
use radroots_trade::validation_receipt::RadrootsValidationReceiptProofSystem;

const SERVICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SELLER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const WORKER: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const RELAY: &str = "wss://relay.radroots.test";

#[test]
fn trade_transition_proof_plan_builds_microstandard_wire_payload() {
    let request = proof_request(service_actor())
        .with_inventory_sequence(7)
        .with_previous_state_root(hash32('3'));
    let plan = dvm_trade_transition_proof_plan(
        request,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
    )
    .expect("plan");
    let payload: serde_json::Value =
        serde_json::from_str(plan.frozen_draft.content()).expect("payload json");

    assert_eq!(
        plan.frozen_draft.contract_id(),
        super::DVM_TRADE_TRANSITION_PROOF_REQUEST_CONTRACT_ID
    );
    assert_eq!(
        plan.frozen_draft.kind_u32(),
        KIND_TRADE_TRANSITION_PROOF_REQUEST
    );
    assert_eq!(plan.frozen_draft.expected_pubkey_str(), SERVICE);
    assert_eq!(plan.worker_pubkey.as_str(), WORKER);
    assert_eq!(plan.proof_mode, DvmProofMode::None);
    assert_eq!(
        plan.expected_receipt_proof_system,
        RadrootsValidationReceiptProofSystem::None
    );
    assert_eq!(payload["proof_mode"], "none");
    assert_eq!(payload["witness_version"], 1);
    assert_eq!(payload["proof_target"], "trade.order_acceptance.v1");
    assert_eq!(payload["radroots_protocol_version"], "radroots.trade.v1");
    assert_eq!(payload["inventory_sequence"], 7);
    assert_eq!(payload["previous_state_root"], hash32('3'));
    assert_eq!(payload["listing_event_id"], event_id('1').as_str());
    let tags = plan.frozen_draft.tags_as_vec();
    assert_eq!(tags[0], vec!["a", listing_addr().as_str()]);
    assert_eq!(
        tags[1],
        vec![
            "i",
            event_id('3').as_str(),
            "event",
            "radroots:order_decision_event"
        ]
    );
    assert_eq!(tags[2], vec!["p", WORKER]);
}

#[test]
fn trade_transition_proof_plan_rejects_non_service_actor_before_mutation() {
    let error = dvm_trade_transition_proof_plan(
        proof_request(buyer_actor()),
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
    )
    .expect_err("role error");

    assert!(matches!(error, RadrootsSdkError::UnauthorizedActor { .. }));
}

#[test]
fn trade_transition_proof_plan_rejects_inventory_and_sp1_identity_edges() {
    let empty_bins = DvmTradeTransitionProofPrepareRequest::new(
        service_actor(),
        worker_pubkey(),
        listing_addr(),
        event_id('1'),
        event_id('2'),
        event_id('3'),
        Vec::new(),
    );
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            empty_bins,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let over_reserved = DvmTradeTransitionProofPrepareRequest::new(
        service_actor(),
        worker_pubkey(),
        listing_addr(),
        event_id('1'),
        event_id('2'),
        event_id('3'),
        vec![RadrootsTradeInventoryBinWitnessDto {
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            listing_capacity: 2,
            previous_reserved: 3,
        }],
    );
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            over_reserved,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let invalid_previous_state_root =
        proof_request(service_actor()).with_previous_state_root("not-a-hash");
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            invalid_previous_state_root,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let missing_sp1_identity = proof_request(service_actor()).with_proof_mode(DvmProofMode::Core);
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            missing_sp1_identity,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let none_with_sp1_identity =
        proof_request(service_actor()).with_sp1_identity(hash32('a'), hash32('b'));
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            none_with_sp1_identity,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let invalid_hash = proof_request(service_actor())
        .with_proof_mode(DvmProofMode::Core)
        .with_sp1_identity("0xABC", hash32('b'));
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            invalid_hash,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let invalid_previous_state = proof_request(service_actor()).with_previous_state_root("0xABC");
    assert!(matches!(
        dvm_trade_transition_proof_plan(
            invalid_previous_state,
            RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
}

#[test]
fn proof_modes_map_to_expected_receipt_proof_systems_and_labels() {
    let cases = [
        (
            DvmProofMode::None,
            "none",
            RadrootsValidationReceiptProofSystem::None,
        ),
        (
            DvmProofMode::Core,
            "core",
            RadrootsValidationReceiptProofSystem::Sp1Core,
        ),
        (
            DvmProofMode::Compressed,
            "compressed",
            RadrootsValidationReceiptProofSystem::Sp1Compressed,
        ),
        (
            DvmProofMode::Groth16,
            "groth16",
            RadrootsValidationReceiptProofSystem::Sp1Groth16,
        ),
        (
            DvmProofMode::Plonk,
            "plonk",
            RadrootsValidationReceiptProofSystem::Sp1Plonk,
        ),
    ];

    for (mode, label, proof_system) in cases {
        assert_eq!(mode.as_str(), label);
        assert_eq!(mode.proof_system(), proof_system);
    }

    let request = proof_request(service_actor())
        .with_proof_mode(DvmProofMode::Compressed)
        .with_sp1_identity(hash32('a'), hash32('b'));
    let plan = dvm_trade_transition_proof_plan(
        request,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000),
    )
    .expect("sp1 plan");

    assert_eq!(plan.payload.proof_mode, DvmProofMode::Compressed);
    assert_eq!(
        plan.expected_receipt_proof_system,
        RadrootsValidationReceiptProofSystem::Sp1Compressed
    );
}

#[test]
fn enqueue_request_builders_and_ingest_request_builders_are_deterministic() {
    let prepare = proof_request(service_actor())
        .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_010));
    let idempotency = SdkIdempotencyKey::new("dvm-proof-request").expect("idempotency");
    let idempotency_len = idempotency.as_str().len();
    let request = DvmTradeTransitionProofEnqueueRequest::from_prepare(
        prepare.clone(),
        TargetPolicy::default_profile(),
    )
    .try_with_nostr_targets([RELAY], NostrRelayUrlPolicy::Public)
    .expect("relays")
    .with_idempotency_key(idempotency)
    .with_inventory_sequence(11)
    .with_previous_state_root(hash32('1'));
    let serialized = serde_json::to_value(&request).expect("request json");

    assert_eq!(request.prepare.inventory_sequence, 11);
    assert_eq!(request.prepare.previous_state_root, Some(hash32('1')));
    assert_eq!(serialized["proof_mode"], "none");
    assert_eq!(serialized["target_policy"]["kind"], "explicit");
    assert_eq!(serialized["idempotency_key"]["value"], "<redacted>");
    assert_eq!(serialized["idempotency_key"]["len"], idempotency_len);

    let request = DvmTradeTransitionProofEnqueueRequest::new(
        service_actor(),
        worker_pubkey(),
        listing_addr(),
        event_id('1'),
        event_id('2'),
        event_id('3'),
        inventory_bins(),
        TargetPolicy::default_profile(),
    )
    .try_with_idempotency_key("dvm-proof-request-2")
    .expect("idempotency")
    .with_proof_mode(DvmProofMode::Core)
    .with_sp1_identity(hash32('a'), hash32('b'))
    .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_011));
    assert_eq!(request.prepare.proof_mode, DvmProofMode::Core);
    assert_eq!(request.prepare.sp1_program_hash, Some(hash32('a')));
    assert_eq!(request.prepare.sp1_verifying_key_hash, Some(hash32('b')));
    assert!(matches!(
        DvmTradeTransitionProofEnqueueRequest::from_prepare(
            prepare,
            TargetPolicy::default_profile(),
        )
        .try_with_nostr_targets(["ws://relay.example.com"], NostrRelayUrlPolicy::Public),
        Err(RadrootsSdkError::InvalidRelayUrl { .. })
    ));
    assert!(matches!(
        DvmTradeTransitionProofEnqueueRequest::from_prepare(
            proof_request(service_actor()),
            TargetPolicy::default_profile(),
        )
        .try_with_idempotency_key(""),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let ingest = DvmValidationReceiptIngestRequest::new(dummy_event())
        .with_projection_refresh(SyncProjectionRefreshRequest::new().with_limit(5));
    assert_eq!(ingest.projection_refresh.limit, 5);
}

#[test]
fn dvm_timestamp_edges_return_structured_errors() {
    let error = dvm_trade_transition_proof_plan(
        proof_request(service_actor()),
        RadrootsSdkTimestamp::from_unix_seconds(u64::from(u32::MAX) + 1),
    )
    .expect_err("nostr timestamp range");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::from(u32::MAX) + 1
    ));

    let error = sdk_timestamp_ms(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX))
        .expect_err("millisecond timestamp range");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::MAX
    ));
}

#[tokio::test]
async fn dvm_client_prepare_reports_default_clock_errors() {
    let sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");

    let error = sdk
        .dvm()
        .prepare_trade_transition_proof_request(proof_request(service_actor()))
        .expect_err("clock error");

    assert!(matches!(error, RadrootsSdkError::ClockBeforeUnixEpoch));
}

fn proof_request(actor: RadrootsActorContext) -> DvmTradeTransitionProofPrepareRequest {
    DvmTradeTransitionProofPrepareRequest::new(
        actor,
        worker_pubkey(),
        listing_addr(),
        event_id('1'),
        event_id('2'),
        event_id('3'),
        vec![RadrootsTradeInventoryBinWitnessDto {
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            listing_capacity: 5,
            previous_reserved: 1,
        }],
    )
}

fn inventory_bins() -> Vec<RadrootsTradeInventoryBinWitnessDto> {
    vec![RadrootsTradeInventoryBinWitnessDto {
        bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
        listing_capacity: 5,
        previous_reserved: 1,
    }]
}

fn service_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SERVICE, [RadrootsActorRole::Service]).expect("service actor")
}

fn buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER, [RadrootsActorRole::Buyer]).expect("buyer actor")
}

fn worker_pubkey() -> RadrootsPublicKey {
    RadrootsPublicKey::parse(WORKER).expect("worker pubkey")
}

fn listing_addr() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!("30402:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"))
        .expect("listing addr")
}

fn event_id(ch: char) -> RadrootsEventId {
    RadrootsEventId::parse(ch.to_string().repeat(64)).expect("event id")
}

fn hash32(ch: char) -> String {
    format!("0x{}", ch.to_string().repeat(64))
}

fn dummy_event() -> RadrootsSignedEvent {
    let mut wire = RadrootsNip01EventWire {
        id: String::new(),
        pubkey: event_id('a').into_string(),
        created_at: 1,
        kind: 3440,
        tags: Vec::new(),
        content: "{}".to_owned(),
        sig: "b".repeat(128),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().expect("event id").into_string();
    let raw_json = serde_json::to_string(&wire).expect("raw event json");
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}
