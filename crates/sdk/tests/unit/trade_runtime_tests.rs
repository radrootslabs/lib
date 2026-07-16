use super::*;
use crate::{
    RadrootsClient, RadrootsSdkError, RadrootsSdkTimestamp, RadrootsSdkTradeErrorKind,
    SatisfactionPolicy, SdkIdempotencyKey, TargetPolicy,
};
use radroots_authority::{RadrootsActorContext, RadrootsLocalEventSigner};
use radroots_event::{
    contract::RadrootsActorRole,
    ids::{
        RadrootsAddressableCoordinate, RadrootsDTag, RadrootsEventId, RadrootsInventoryBinId,
        RadrootsPublicKey, RadrootsTradeId,
    },
    trade::{
        RADROOTS_TRADE_DECISION_CONTRACT_ID, RADROOTS_TRADE_PROPOSAL_CONTRACT_ID,
        RADROOTS_TRADE_SCHEMA_VERSION, RadrootsFulfillmentProfileV1,
        RadrootsSellerReservationAssertionV1, RadrootsSellerReservationLineV1,
        RadrootsTradeCancellationProfileV1, RadrootsTradeCandidateLineV1,
        RadrootsTradeCandidateTermsV1, RadrootsTradeDecisionV1, RadrootsTradeEconomicAdjustmentV1,
        RadrootsTradeEconomicsProfileV1, RadrootsTradeMutationBodyV1,
        RadrootsTradeMutationEnvelopeV1, canonical_trade_mutation_content,
    },
};
use radroots_nostr::prelude::{RadrootsNostrKeys, RadrootsNostrSecretKey};
use radroots_trade::workflow::RadrootsTradePrivateTermsStateV1;

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";

fn pubkey(value: &str) -> RadrootsPublicKey {
    RadrootsPublicKey::parse(value).expect("pubkey")
}

fn event_id(marker: char) -> RadrootsEventId {
    RadrootsEventId::parse(std::iter::repeat_n(marker, 64).collect::<String>()).expect("event id")
}

fn trade_id() -> RadrootsTradeId {
    RadrootsTradeId::parse("11111111111111111111111111111111").expect("trade id")
}

fn local_signer(secret_key_hex: &str) -> (String, RadrootsLocalEventSigner) {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let keys = RadrootsNostrKeys::new(secret_key);
    let pubkey = keys.public_key().to_hex();
    (
        pubkey,
        RadrootsLocalEventSigner::new(keys).expect("local event signer"),
    )
}

fn buyer_actor(buyer_pubkey: &str) -> RadrootsActorContext {
    RadrootsActorContext::test(buyer_pubkey, [RadrootsActorRole::Buyer]).expect("buyer")
}

fn seller_actor(seller_pubkey: &str) -> RadrootsActorContext {
    RadrootsActorContext::test(seller_pubkey, [RadrootsActorRole::Seller]).expect("seller")
}

fn candidate(buyer_pubkey: &str, seller_pubkey: &str) -> RadrootsTradeCandidateTermsV1 {
    RadrootsTradeCandidateTermsV1 {
        candidate_id: None,
        schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
        base_candidate_id: None,
        supersession_intent: None,
        buyer_pubkey: pubkey(buyer_pubkey),
        seller_pubkey: pubkey(seller_pubkey),
        farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
        lines: vec![RadrootsTradeCandidateLineV1 {
            line_id: RadrootsDTag::parse("line-1").expect("line id"),
            listing_addr: RadrootsAddressableCoordinate::parse(format!(
                "30402:{seller_pubkey}:listing-1"
            ))
            .expect("listing address"),
            listing_event_id: event_id('c'),
            listing_snapshot_sha256: "d".repeat(64),
            product_id: "carrots".to_owned(),
            option_id: None,
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            quantity_mantissa: "2".to_owned(),
            quantity_scale: 0,
            unit_code: "count".to_owned(),
            unit_profile: "mvp-count".to_owned(),
            unit_price_mantissa: "500".to_owned(),
            currency_code: "USD".to_owned(),
            line_subtotal_mantissa: "1000".to_owned(),
            replaces_line_id: None,
        }],
        line_tombstones: Vec::new(),
        economics: RadrootsTradeEconomicsProfileV1 {
            profile_id: "mvp-fixed".to_owned(),
            currency_code: "USD".to_owned(),
            currency_exponent: 2,
            rounding_profile: "half-even".to_owned(),
            subtotal_mantissa: "1000".to_owned(),
            discount_total_mantissa: "0".to_owned(),
            adjustment_total_mantissa: "0".to_owned(),
            total_mantissa: "1000".to_owned(),
            adjustments: Vec::<RadrootsTradeEconomicAdjustmentV1>::new(),
        },
        fulfillment: RadrootsFulfillmentProfileV1 {
            profile_id: "market-pickup".to_owned(),
            method: "pickup".to_owned(),
            starts_at_unix_s: 1_800_000_000,
            ends_at_unix_s: 1_800_003_600,
            timezone: "America/New_York".to_owned(),
            utc_offset_seconds: -18_000,
            fold: 0,
            location_class: "farmstand".to_owned(),
            requires_private_terms: true,
        },
        cancellation: RadrootsTradeCancellationProfileV1 {
            profile_id: "buyer-pre-agreement".to_owned(),
            buyer_pre_agreement: true,
            post_agreement_cutoff_unix_s: None,
        },
        private_terms: None,
        proposal_expires_at_unix_s: 1_799_999_000,
    }
}

fn proposal(
    candidate: RadrootsTradeCandidateTermsV1,
    buyer_pubkey: &str,
    seller_pubkey: &str,
) -> RadrootsTradeMutationEnvelopeV1 {
    RadrootsTradeMutationEnvelopeV1 {
        mutation_id: None,
        contract_id: RADROOTS_TRADE_PROPOSAL_CONTRACT_ID.to_owned(),
        schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
        trade_id: trade_id(),
        root_mutation_id: None,
        buyer_pubkey: pubkey(buyer_pubkey),
        seller_pubkey: pubkey(seller_pubkey),
        farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
        parent_mutation_ids: Vec::new(),
        author_pubkey: pubkey(buyer_pubkey),
        counterparty_pubkey: pubkey(seller_pubkey),
        authored_at_unix_s: 1_799_000_000,
        body: RadrootsTradeMutationBodyV1::Proposal { candidate },
    }
}

fn reservation(
    candidate: &RadrootsTradeCandidateTermsV1,
    seller_pubkey: &str,
) -> RadrootsSellerReservationAssertionV1 {
    RadrootsSellerReservationAssertionV1 {
        reservation_id: RadrootsDTag::parse("reservation-1").expect("reservation id"),
        inventory_authority_id: pubkey(seller_pubkey),
        inventory_epoch: 42,
        candidate_id: candidate.candidate_id.clone().expect("candidate id"),
        commitments: candidate
            .lines
            .iter()
            .map(|line| RadrootsSellerReservationLineV1 {
                line_id: line.line_id.clone(),
                bin_id: line.bin_id.clone(),
                quantity_mantissa: line.quantity_mantissa.clone(),
                quantity_scale: line.quantity_scale,
                unit_code: line.unit_code.clone(),
            })
            .collect(),
        reservation_expires_at_unix_s: 1_800_000_000,
        assertion_commitment: "e".repeat(64),
    }
}

fn accepted_decision(
    proposal: &RadrootsTradeMutationEnvelopeV1,
    buyer_pubkey: &str,
    seller_pubkey: &str,
) -> RadrootsTradeMutationEnvelopeV1 {
    let proposal_id = proposal.mutation_id.clone().expect("proposal id");
    let candidate = match &proposal.body {
        RadrootsTradeMutationBodyV1::Proposal { candidate } => candidate.clone(),
        _ => unreachable!(),
    };
    RadrootsTradeMutationEnvelopeV1 {
        mutation_id: None,
        contract_id: RADROOTS_TRADE_DECISION_CONTRACT_ID.to_owned(),
        schema_version: RADROOTS_TRADE_SCHEMA_VERSION,
        trade_id: proposal.trade_id.clone(),
        root_mutation_id: Some(proposal_id.clone()),
        buyer_pubkey: pubkey(buyer_pubkey),
        seller_pubkey: pubkey(seller_pubkey),
        farm_id: RadrootsDTag::parse("farm-1").expect("farm id"),
        parent_mutation_ids: vec![proposal_id.clone()],
        author_pubkey: pubkey(seller_pubkey),
        counterparty_pubkey: pubkey(buyer_pubkey),
        authored_at_unix_s: 1_799_000_060,
        body: RadrootsTradeMutationBodyV1::Decision {
            proposal_mutation_id: proposal_id,
            candidate_id: candidate.candidate_id.clone().expect("candidate id"),
            decision: RadrootsTradeDecisionV1::Accepted {
                reservation_assertion: Some(reservation(&candidate, seller_pubkey)),
            },
        },
    }
}

#[tokio::test]
async fn trade_commands_query_and_private_terms_are_release_product_v1() {
    let (buyer_pubkey, buyer_signer) = local_signer(BUYER_SECRET_KEY_HEX);
    let (seller_pubkey, seller_signer) = local_signer(SELLER_SECRET_KEY_HEX);
    let sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_799_000_100))
        .build()
        .await
        .expect("sdk");
    let sealed = sdk
        .trades()
        .seal_private_artifact(TradePrivateArtifactSealRequest::binding_terms(
            "terms-1",
            trade_id(),
            "radroots.trade.binding_terms.v1",
            b"{\"pickup\":\"south gate\"}".to_vec(),
        ))
        .await
        .expect("seal private terms");
    let mut candidate = candidate(&buyer_pubkey, &seller_pubkey);
    candidate.private_terms = sealed.private_terms_ref.clone();
    let proposal = proposal(candidate, &buyer_pubkey, &seller_pubkey);
    let submit = SubmitProposalRequest::new(
        buyer_actor(&buyer_pubkey),
        proposal.clone(),
        TargetPolicy::LocalOnly,
    )
    .with_satisfaction_policy(SatisfactionPolicy::NoWait)
    .with_idempotency_key(
        SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000501").expect("idempotency key"),
    );
    let receipt = sdk
        .trades()
        .commands()
        .submit_proposal_with_explicit_signer(submit, &buyer_signer)
        .await
        .expect("submit proposal");
    let canonical_proposal = canonical_trade_mutation_content(proposal)
        .expect("canonical proposal")
        .envelope;

    assert_eq!(receipt.operation_kind, TRADE_SUBMIT_PROPOSAL_OPERATION_KIND);
    assert_eq!(receipt.trade_id, trade_id());
    assert_eq!(
        sdk.trades()
            .open_private_artifact(TradePrivateArtifactOpenRequest::new("terms-1"))
            .await
            .expect("open private artifact")
            .expect("private artifact")
            .plaintext,
        b"{\"pickup\":\"south gate\"}".to_vec()
    );

    let status = sdk
        .trades()
        .queries()
        .get_trade(GetTradeRequest::new(trade_id()))
        .await
        .expect("trade status");
    assert_eq!(status.source_event_count, 1);
    assert_eq!(status.private_terms.len(), 1);
    assert_eq!(
        status.private_terms[0].state,
        RadrootsTradePrivateTermsStateV1::AvailableVerified
    );

    let evidence = sdk
        .trades()
        .queries()
        .inspect_evidence(InspectEvidenceRequest::new(trade_id()))
        .await
        .expect("evidence");
    assert_eq!(evidence.items.len(), 1);
    assert_eq!(evidence.items[0].artifact_id, "terms-1");

    let listed = sdk
        .trades()
        .queries()
        .list_trades(ListTradesRequest::new())
        .await
        .expect("list trades");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].trade_id, trade_id());

    let decision = accepted_decision(&canonical_proposal, &buyer_pubkey, &seller_pubkey);
    let error = sdk
        .trades()
        .commands()
        .decide_candidate_with_explicit_signer(
            DecideCandidateRequest::new(
                seller_actor(&seller_pubkey),
                decision,
                TargetPolicy::LocalOnly,
            )
            .with_satisfaction_policy(SatisfactionPolicy::NoWait)
            .with_idempotency_key(
                SdkIdempotencyKey::new("01890f0e-6c00-7000-8000-000000000502")
                    .expect("idempotency key"),
            ),
            &seller_signer,
        )
        .await
        .expect_err("private terms acknowledgement required");
    assert!(matches!(
        error,
        RadrootsSdkError::Trade {
            kind: RadrootsSdkTradeErrorKind::PrivateArtifactAcknowledgementMissing,
            ..
        }
    ));
}
