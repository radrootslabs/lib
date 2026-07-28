use super::*;

use nostr::{Keys, SECP256K1, secp256k1::Message};
use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeParts, wire::compute_canonical_nip01_event_id,
};

use crate::{
    deletion::admission::verify_and_admit_nip09_deletion_request_event,
    test_fixtures::{FIXTURE_ALICE_SECRET_KEY_HEX, FIXTURE_BOB_SECRET_KEY_HEX},
    verification::verify_nip01_event,
};

const TARGET_TIME: u64 = 1_800_100_100;

#[test]
fn nip09_evaluator_codes_and_opaque_evidence_are_stable() {
    let outcome_codes = [
        (RadrootsNip09SuppressionOutcome::Visible, "visible"),
        (RadrootsNip09SuppressionOutcome::Suppressed, "suppressed"),
    ];
    for (outcome, code) in outcome_codes {
        assert_eq!(outcome.code(), code);
    }

    let reason_codes = [
        (
            RadrootsNip09SuppressionReason::DeletionRequestImmune,
            "deletion_request_immune",
        ),
        (
            RadrootsNip09SuppressionReason::NoAuthorizedReference,
            "deletion_no_authorized_reference",
        ),
        (
            RadrootsNip09SuppressionReason::RequestAuthorMismatch,
            "deletion_request_author_mismatch",
        ),
        (
            RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
            "deletion_address_cutoff_precedes_target",
        ),
        (
            RadrootsNip09SuppressionReason::EventIdReference,
            "deletion_event_id_reference",
        ),
        (
            RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
            "deletion_address_reference",
        ),
        (
            RadrootsNip09SuppressionReason::EventIdAndAddressReference,
            "deletion_event_id_and_address_reference",
        ),
    ];
    for (reason, code) in reason_codes {
        assert_eq!(reason.code(), code);
    }
}

#[test]
fn nip09_evaluator_deletion_requests_are_immune() {
    let target = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        vec![event_reference("a")],
        "target",
    );
    let request = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 1,
        vec![event_reference(target.event().id_str())],
        "attempt",
    );

    let decision = evaluate_nip09_suppression(target.verified_event(), &[request]);

    assert_decision(
        &decision,
        RadrootsNip09SuppressionOutcome::Visible,
        RadrootsNip09SuppressionReason::DeletionRequestImmune,
    );
    assert!(decision.event_reference().is_none());
    assert!(decision.address_reference().is_none());
}

#[test]
fn nip09_evaluator_exact_event_reference_is_time_independent() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        1,
        Vec::new(),
        "target",
    );
    let request = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME - 1,
        vec![event_reference(target.event().id_str())],
        "exact",
    );
    let request_id = request.event().id().clone();

    let decision = evaluate_nip09_suppression(&target, &[request]);

    assert_decision(
        &decision,
        RadrootsNip09SuppressionOutcome::Suppressed,
        RadrootsNip09SuppressionReason::EventIdReference,
    );
    assert_eq!(
        decision
            .event_reference()
            .expect("event evidence")
            .request_id(),
        &request_id
    );
    assert!(decision.address_reference().is_none());
}

#[test]
fn nip09_evaluator_address_cutoff_is_inclusive_and_later_replacement_is_visible() {
    let at_cutoff = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        30_402,
        vec![d_tag("produce")],
        "at cutoff",
    );
    let coordinate = coordinate_for(&at_cutoff, "produce");
    let request = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        vec![address_reference(coordinate.as_str())],
        "address",
    );
    let request_id = request.event().id().clone();

    let suppressed = evaluate_nip09_suppression(&at_cutoff, core::slice::from_ref(&request));
    assert_decision(
        &suppressed,
        RadrootsNip09SuppressionOutcome::Suppressed,
        RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff,
    );
    let evidence = suppressed.address_reference().expect("address evidence");
    assert_eq!(evidence.coordinate(), &coordinate);
    assert_eq!(evidence.inclusive_cutoff(), TARGET_TIME);
    assert_eq!(evidence.request_id(), &request_id);

    let later = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 1,
        30_402,
        vec![d_tag("produce")],
        "later",
    );
    let visible = evaluate_nip09_suppression(&later, &[request]);
    assert_decision(
        &visible,
        RadrootsNip09SuppressionOutcome::Visible,
        RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
    );
    assert_eq!(
        visible
            .address_reference()
            .expect("stale address evidence")
            .inclusive_cutoff(),
        TARGET_TIME
    );
}

#[test]
fn nip09_evaluator_author_mismatch_and_unrelated_requests_are_distinct() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        1,
        Vec::new(),
        "target",
    );
    let mismatch = admitted_request(
        FIXTURE_BOB_SECRET_KEY_HEX,
        TARGET_TIME + 1,
        vec![event_reference(target.event().id_str())],
        "wrong author",
    );
    let unrelated = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 1,
        vec![event_reference("f")],
        "unrelated",
    );

    let mismatch_decision = evaluate_nip09_suppression(&target, &[mismatch]);
    assert_decision(
        &mismatch_decision,
        RadrootsNip09SuppressionOutcome::Visible,
        RadrootsNip09SuppressionReason::RequestAuthorMismatch,
    );

    let unrelated_decision = evaluate_nip09_suppression(&target, &[unrelated]);
    assert_decision(
        &unrelated_decision,
        RadrootsNip09SuppressionOutcome::Visible,
        RadrootsNip09SuppressionReason::NoAuthorizedReference,
    );
}

#[test]
fn nip09_evaluator_authorized_stale_address_precedes_unauthorized_exact_event() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        0,
        Vec::new(),
        "profile",
    );
    let coordinate = coordinate_for(&target, "");
    let stale = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME - 1,
        vec![address_reference(coordinate.as_str())],
        "stale",
    );
    let mismatch = admitted_request(
        FIXTURE_BOB_SECRET_KEY_HEX,
        TARGET_TIME + 1,
        vec![event_reference(target.event().id_str())],
        "wrong author",
    );

    let decision = evaluate_nip09_suppression(&target, &[mismatch, stale]);

    assert_decision(
        &decision,
        RadrootsNip09SuppressionOutcome::Visible,
        RadrootsNip09SuppressionReason::AddressCutoffPrecedesTarget,
    );
}

#[test]
fn nip09_evaluator_exact_event_dominates_stale_address_reference() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        30_000,
        vec![d_tag("harvest")],
        "target",
    );
    let coordinate = coordinate_for(&target, "harvest");
    let exact = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME - 2,
        vec![event_reference(target.event().id_str())],
        "exact",
    );
    let stale = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME - 1,
        vec![address_reference(coordinate.as_str())],
        "stale",
    );

    let decision = evaluate_nip09_suppression(&target, &[stale, exact]);

    assert_decision(
        &decision,
        RadrootsNip09SuppressionOutcome::Suppressed,
        RadrootsNip09SuppressionReason::EventIdReference,
    );
    assert!(decision.event_reference().is_some());
    assert!(decision.address_reference().is_some());
}

#[test]
fn nip09_evaluator_reduction_is_order_and_repeat_invariant() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        39_999,
        vec![d_tag("market"), d_tag("ignored")],
        "target",
    );
    let coordinate = coordinate_for(&target, "market");
    let exact_a = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME - 10,
        vec![event_reference(target.event().id_str())],
        "exact a",
    );
    let exact_b = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 20,
        vec![event_reference(target.event().id_str())],
        "exact b",
    );
    let address_a = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 20,
        vec![address_reference(coordinate.as_str())],
        "address a",
    );
    let address_b = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 20,
        vec![address_reference(coordinate.as_str())],
        "address b",
    );
    let address_older = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME + 10,
        vec![address_reference(coordinate.as_str())],
        "address older",
    );
    let lower_exact_id = core::cmp::min(exact_a.event().id(), exact_b.event().id()).clone();
    let lower_address_id = core::cmp::min(address_a.event().id(), address_b.event().id()).clone();

    let forward = vec![
        exact_a.clone(),
        address_a.clone(),
        exact_b.clone(),
        address_b.clone(),
        address_older.clone(),
    ];
    let repeated_reverse = vec![
        address_b,
        address_older,
        exact_b,
        address_a,
        exact_a.clone(),
        exact_a,
    ];
    let expected = evaluate_nip09_suppression(&target, &forward);
    let actual = evaluate_nip09_suppression(&target, &repeated_reverse);

    assert_eq!(actual, expected);
    assert_decision(
        &actual,
        RadrootsNip09SuppressionOutcome::Suppressed,
        RadrootsNip09SuppressionReason::EventIdAndAddressReference,
    );
    assert_eq!(
        actual
            .event_reference()
            .expect("event evidence")
            .request_id(),
        &lower_exact_id
    );
    let address = actual.address_reference().expect("address evidence");
    assert_eq!(address.coordinate(), &coordinate);
    assert_eq!(address.inclusive_cutoff(), TARGET_TIME + 20);
    assert_eq!(address.request_id(), &lower_address_id);
}

#[test]
fn nip09_borrowed_request_iterator_matches_slice_evaluation() {
    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        30_402,
        vec![d_tag("borrowed")],
        "target",
    );
    let coordinate = coordinate_for(&target, "borrowed");
    let requests = vec![
        admitted_request(
            FIXTURE_ALICE_SECRET_KEY_HEX,
            TARGET_TIME + 1,
            vec![event_reference("f")],
            "unrelated",
        ),
        admitted_request(
            FIXTURE_ALICE_SECRET_KEY_HEX,
            TARGET_TIME - 1,
            vec![event_reference(target.event().id_str())],
            "exact",
        ),
        admitted_request(
            FIXTURE_BOB_SECRET_KEY_HEX,
            TARGET_TIME + 2,
            vec![address_reference(coordinate.as_str())],
            "wrong author",
        ),
        admitted_request(
            FIXTURE_ALICE_SECRET_KEY_HEX,
            TARGET_TIME + 3,
            vec![
                event_reference(target.event().id_str()),
                address_reference(coordinate.as_str()),
            ],
            "exact and address",
        ),
    ];
    let expected = evaluate_nip09_suppression_v1(&target, &requests);
    let actual = evaluate_nip09_suppression_from_borrowed_requests_v1(
        &target,
        requests.iter().rev().filter(|request| {
            request
                .projection()
                .event_targets()
                .iter()
                .any(|reference| reference.event_id() == target.event().id())
                || request
                    .projection()
                    .address_targets()
                    .iter()
                    .any(|reference| reference.coordinate() == &coordinate)
        }),
    );

    assert_eq!(actual, expected);
    assert_decision(
        &actual,
        RadrootsNip09SuppressionOutcome::Suppressed,
        RadrootsNip09SuppressionReason::EventIdAndAddressReference,
    );
}

#[test]
fn nip09_evaluator_uses_generic_coordinates_and_rejects_malformed_first_d() {
    for kind in [0, 3, 10_000, 19_999] {
        let target = verified_event(
            FIXTURE_ALICE_SECRET_KEY_HEX,
            TARGET_TIME,
            kind,
            Vec::new(),
            "replaceable",
        );
        let coordinate = coordinate_for(&target, "");
        let request = admitted_request(
            FIXTURE_ALICE_SECRET_KEY_HEX,
            TARGET_TIME,
            vec![address_reference(coordinate.as_str())],
            "replaceable",
        );
        assert_eq!(
            evaluate_nip09_suppression(&target, &[request]).reason(),
            RadrootsNip09SuppressionReason::AddressReferenceAtOrBeforeCutoff
        );
    }

    let target = verified_event(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        30_000,
        vec![vec!["d".to_string()], d_tag("later")],
        "malformed",
    );
    let later_coordinate = coordinate_for(&target, "later");
    let request = admitted_request(
        FIXTURE_ALICE_SECRET_KEY_HEX,
        TARGET_TIME,
        vec![address_reference(later_coordinate.as_str())],
        "must not match",
    );
    assert_eq!(
        evaluate_nip09_suppression(&target, &[request]).reason(),
        RadrootsNip09SuppressionReason::NoAuthorizedReference
    );
}

fn assert_decision(
    decision: &RadrootsNip09SuppressionDecision,
    outcome: RadrootsNip09SuppressionOutcome,
    reason: RadrootsNip09SuppressionReason,
) {
    assert_eq!(decision.outcome(), outcome);
    assert_eq!(decision.reason(), reason);
}

fn event_reference(value: &str) -> Vec<String> {
    vec!["e".to_string(), normalize_event_id(value)]
}

fn address_reference(value: &str) -> Vec<String> {
    vec!["a".to_string(), value.to_string()]
}

fn d_tag(value: &str) -> Vec<String> {
    vec!["d".to_string(), value.to_string()]
}

fn normalize_event_id(value: &str) -> String {
    if value.len() == 1 {
        value.repeat(64)
    } else {
        value.to_string()
    }
}

fn coordinate_for(
    target: &RadrootsSignatureVerifiedEvent,
    identifier: &str,
) -> RadrootsNip01Coordinate {
    RadrootsNip01Coordinate::parse(format!(
        "{}:{}:{identifier}",
        target.event().kind_u32(),
        target.event().author().to_hex()
    ))
    .expect("target coordinate")
}

fn admitted_request(
    secret_key_hex: &str,
    created_at: u64,
    tags: Vec<Vec<String>>,
    content: &str,
) -> RadrootsAdmittedNip09DeletionRequestEventV1 {
    verify_and_admit_nip09_deletion_request_event(signed_event(
        secret_key_hex,
        created_at,
        KIND_DELETION_REQUEST,
        tags,
        content,
    ))
    .expect("valid admitted deletion request")
}

fn verified_event(
    secret_key_hex: &str,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: &str,
) -> RadrootsSignatureVerifiedEvent {
    verify_nip01_event(signed_event(
        secret_key_hex,
        created_at,
        kind,
        tags,
        content,
    ))
    .expect("valid verified event")
}

fn signed_event(
    secret_key_hex: &str,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: &str,
) -> RadrootsEventEnvelope {
    let keys = Keys::parse(secret_key_hex).expect("fixed fixture secret key must parse");
    let author = keys.public_key().to_string();
    let id = compute_canonical_nip01_event_id(author.as_str(), created_at, kind, &tags, content)
        .expect("canonical event id");
    let nostr_id = nostr::EventId::from_hex(id.as_str()).expect("Nostr event id");
    let message = Message::from_digest(nostr_id.to_bytes());
    let signature = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));

    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: id.into_string(),
        author,
        created_at,
        kind,
        tags,
        content: content.to_string(),
        sig: signature.to_string(),
    })
    .expect("valid event envelope")
}
