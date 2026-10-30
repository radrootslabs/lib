//! Independent complete replacement claims; no source authentication or store.
use super::*;

fn time(value: u64) -> UnixSeconds {
    UnixSeconds::new(value).unwrap()
}
fn digest(value: u8) -> TermsDigest {
    TermsDigest::from_bytes([value; 32])
}
fn complete(different: bool) -> Terms {
    terms(
        vec![line(if different { 8 } else { 7 }, 9)],
        vec![group(9)],
        money("50"),
        money("700"),
    )
    .unwrap()
}
fn content(
    author: PublicKey,
    sequence: u64,
    previous: Option<u64>,
    extra: &[u64],
    payload: CommercePayload,
) -> CommerceContent {
    let mut parents = vec![SourceId::from_bytes([3; 32])];
    parents.extend(previous.map(src));
    parents.extend(extra.iter().copied().map(src));
    parents.sort();
    parents.dedup();
    CommerceContent::new(
        CommerceHeader::new(
            thread(),
            Some(order()),
            author,
            buyer(),
            seller(),
            time(100),
            Sequence::new(sequence).unwrap(),
            previous.map(id),
            parents,
        )
        .unwrap(),
        payload,
    )
    .unwrap()
}
fn proposal(base: Option<u64>, different: bool, expires: u64) -> CommercePayload {
    CommercePayload::TermsProposed(
        TermsProposal::new(
            base.map(id),
            complete(different),
            time(expires),
            "Private replacement",
        )
        .unwrap(),
    )
}
fn authorization(proposal: u64, declared: u8, expires: u64) -> CommercePayload {
    CommercePayload::TermsAuthorized(TermsAuthorization::new(
        id(proposal),
        digest(declared),
        time(expires),
    ))
}
fn fixture(buyer_proposes: bool) -> Vec<(u64, CommerceContent)> {
    let base = content(
        seller(),
        1,
        None,
        &[9000],
        CommercePayload::AgreementCommitted(
            AgreementCommitment::new(
                None,
                id(9000),
                digest(40),
                AuthorityRef::new(
                    ExecutorId::from_bytes([20; 32]),
                    Generation::new(1).unwrap(),
                ),
                Sequence::new(1).unwrap(),
            )
            .unwrap(),
        ),
    );
    let (proposed, authorized) = if buyer_proposes {
        (
            content(buyer(), 1, None, &[1], proposal(Some(1), false, 200)),
            content(seller(), 2, Some(1), &[2], authorization(2, 42, 190)),
        )
    } else {
        (
            content(seller(), 2, Some(1), &[], proposal(Some(1), false, 200)),
            content(buyer(), 1, None, &[2], authorization(2, 42, 190)),
        )
    };
    vec![(1, base), (2, proposed), (3, authorized)]
}
fn selection() -> ReplacementTermsSelection {
    ReplacementTermsSelection::new(id(1), id(2), id(3), digest(42))
}
fn claim_at(
    values: &[(u64, CommerceContent)],
    at: u64,
) -> Result<ReplacementTermsClaim<'_>, ReplacementTermsError> {
    prepare_replacement_terms(
        scope(),
        selection(),
        &values
            .iter()
            .map(|(id_value, value)| CausalRecord::new(id(*id_value), value))
            .collect::<Vec<_>>(),
        &[src(9000), SourceId::from_bytes([3; 32])],
        &[],
        time(at),
    )
}

#[test]
fn either_counterparty_can_propose_and_all_arrival_orders_preserve_exact_selected_claims() {
    for buyer_proposes in [false, true] {
        let values = fixture(buyer_proposes);
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let shuffled = order.map(|index| values[index].clone());
            let selected = claim_at(&shuffled, 150).unwrap();
            assert_eq!(selected.selection(), selection());
            assert_eq!(selected.base_commitment().seller_sequence().get(), 1);
            assert_eq!(selected.terms(), &complete(false));
            assert_eq!(selected.expires_at(), time(190));
            assert_eq!(
                format!("{selected:?}"),
                "ReplacementTermsClaim(<private data>)"
            );
        }
        let duplicated = [values.as_slice(), values.as_slice()].concat();
        assert!(claim_at(&duplicated, 150).is_ok());
    }
    assert_eq!(
        format!("{:?}", selection()),
        "ReplacementTermsSelection(<private identities>)"
    );
}

#[test]
fn same_author_cannot_self_authorize_replacement_even_with_a_contiguous_author_chain() {
    for buyer_proposes in [false, true] {
        let mut values = fixture(buyer_proposes);
        values[2].1 = if buyer_proposes {
            content(buyer(), 2, Some(2), &[], authorization(2, 42, 190))
        } else {
            content(seller(), 3, Some(2), &[], authorization(2, 42, 190))
        };
        assert_eq!(
            claim_at(&values, 150).unwrap_err(),
            ReplacementTermsError::WrongAuthorization
        );
    }
}

#[test]
fn exact_base_proposal_authorization_and_declared_digest_are_separate_required_bindings() {
    let mut values = fixture(false);
    values[0].1 = content(
        seller(),
        1,
        None,
        &[9000],
        CommercePayload::Decline(RecordReason::new(id(9000), "declined").unwrap()),
    );
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::WrongBase
    );
    let mut values = fixture(false);
    values[1].1 = content(seller(), 2, Some(1), &[], proposal(None, false, 200));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::WrongProposal
    );
    let mut values = fixture(false);
    values[2].1 = content(buyer(), 1, None, &[1], authorization(1, 42, 190));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::WrongAuthorization
    );
    let mut values = fixture(false);
    values[2].1 = content(buyer(), 1, None, &[2], authorization(2, 43, 190));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::TermsDigestMismatch
    );
}

#[test]
fn exact_deadlines_and_future_claims_refuse_fresh_admission_without_erasing_history() {
    let values = fixture(false);
    assert!(claim_at(&values, 189).is_ok());
    assert_eq!(
        claim_at(&values, 190).unwrap_err(),
        ReplacementTermsError::Expired
    );
    assert_eq!(
        claim_at(&values, 99).unwrap_err(),
        ReplacementTermsError::ClockAhead
    );
    let mut values = values;
    values[1].1 = content(seller(), 2, Some(1), &[], proposal(Some(1), false, 140));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::Expired
    );
    assert_eq!(values.len(), 3);
}

#[test]
fn observed_withdrawal_and_counterparty_rejection_block_only_their_actual_selected_claims() {
    let mut values = fixture(false);
    values.push((
        4,
        content(
            buyer(),
            2,
            Some(3),
            &[],
            CommercePayload::WithdrawalRequested(RecordReason::new(id(3), "withdraw").unwrap()),
        ),
    ));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::Withdrawn
    );
    values.pop();
    values.push((
        4,
        content(
            buyer(),
            2,
            Some(3),
            &[2],
            CommercePayload::TermsRejected(RecordReason::new(id(2), "reject").unwrap()),
        ),
    ));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::Rejected
    );
    let mut values = fixture(false);
    values.push((
        4,
        content(
            seller(),
            3,
            Some(2),
            &[],
            CommercePayload::TermsRejected(
                RecordReason::new(id(2), "proposer is not counterparty").unwrap(),
            ),
        ),
    ));
    assert!(claim_at(&values, 150).is_ok());
    let mut values = fixture(true);
    values.push((
        4,
        content(
            buyer(),
            2,
            Some(2),
            &[3],
            CommercePayload::WithdrawalRequested(
                RecordReason::new(id(3), "not the author of this authorization").unwrap(),
            ),
        ),
    ));
    assert!(claim_at(&values, 150).is_ok());
}

fn competing(different: bool) -> Vec<(u64, CommerceContent)> {
    let mut values = fixture(false);
    values.push((
        4,
        content(
            seller(),
            3,
            Some(2),
            &[1],
            proposal(Some(1), different, 200),
        ),
    ));
    values.push((
        5,
        content(
            buyer(),
            2,
            Some(3),
            &[4],
            authorization(4, if different { 43 } else { 42 }, 190),
        ),
    ));
    values
}
#[test]
fn incompatible_live_authorizations_never_choose_by_arrival_time_but_compatible_claims_remain_usable()
 {
    let mut values = competing(true);
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::ConflictingAuthorizations
    );
    values.reverse();
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::ConflictingAuthorizations
    );
    values.reverse();
    values.push((
        6,
        content(
            buyer(),
            3,
            Some(5),
            &[],
            CommercePayload::WithdrawalRequested(
                RecordReason::new(id(5), "withdraw competing authorization").unwrap(),
            ),
        ),
    ));
    assert!(claim_at(&values, 150).is_ok());
    assert!(claim_at(&competing(false), 150).is_ok());
}

#[test]
fn incompatible_counterproposals_authorized_by_different_parties_still_require_reconciliation() {
    let mut values = fixture(false);
    values.push((
        4,
        content(buyer(), 2, Some(3), &[1], proposal(Some(1), true, 200)),
    ));
    values.push((
        5,
        content(seller(), 3, Some(2), &[4], authorization(4, 43, 190)),
    ));
    assert_eq!(
        claim_at(&values, 150).unwrap_err(),
        ReplacementTermsError::ConflictingAuthorizations
    );
}

#[test]
fn selected_originals_cannot_be_filled_by_generic_sources_and_unknown_or_forked_history_never_falls_back()
 {
    let values = fixture(false);
    let no_authorization = &values[..2];
    let records = no_authorization
        .iter()
        .map(|(i, v)| CausalRecord::new(id(*i), v))
        .collect::<Vec<_>>();
    assert_eq!(
        prepare_replacement_terms(
            scope(),
            selection(),
            &records,
            &[src(9000), SourceId::from_bytes([3; 32]), src(3)],
            &[],
            time(150)
        )
        .unwrap_err(),
        ReplacementTermsError::MissingHistory
    );
    let records = values
        .iter()
        .map(|(i, v)| CausalRecord::new(id(*i), v))
        .collect::<Vec<_>>();
    assert_eq!(
        prepare_replacement_terms(
            scope(),
            selection(),
            &records,
            &[src(9000), SourceId::from_bytes([3; 32])],
            &[src(8000)],
            time(150)
        )
        .unwrap_err(),
        ReplacementTermsError::Uninterpreted
    );
    let mut fork = values.clone();
    fork.push((
        4,
        content(seller(), 2, Some(1), &[], proposal(Some(1), true, 200)),
    ));
    assert_eq!(
        claim_at(&fork, 150).unwrap_err(),
        ReplacementTermsError::Conflicted
    );
    let mut missing = values.clone();
    missing.remove(0);
    assert_eq!(
        claim_at(&missing, 150).unwrap_err(),
        ReplacementTermsError::MissingHistory
    );
}

#[test]
fn record_quota_counts_duplicates_and_never_returns_a_claim_from_an_oversized_scope() {
    let values = fixture(false);
    let mut records = values
        .iter()
        .map(|(i, value)| CausalRecord::new(id(*i), value))
        .collect::<Vec<_>>();
    records.resize(MAX_CAUSAL_RECORDS + 1, records[2]);
    assert_eq!(
        prepare_replacement_terms(
            scope(),
            selection(),
            &records,
            &[src(9000), SourceId::from_bytes([3; 32])],
            &[],
            time(150),
        )
        .unwrap_err(),
        ReplacementTermsError::InvalidGraph
    );
}

#[test]
fn complete_scope_and_declared_digest_checks_do_not_authenticate_sources_or_mint_current_authority()
{
    let values = fixture(false);
    let records = values
        .iter()
        .map(|(i, v)| CausalRecord::new(id(*i), v))
        .collect::<Vec<_>>();
    let foreign =
        CausalScope::new(ThreadId::from_bytes([99; 32]), order(), buyer(), seller()).unwrap();
    assert_eq!(
        prepare_replacement_terms(foreign, selection(), &records, &[], &[], time(150)).unwrap_err(),
        ReplacementTermsError::InvalidGraph
    );
    // These are deliberately caller-declared IDs/digests, not a signed carrier
    // or a canonical hash proof. Native admission must independently prove both.
    let selected = claim_at(&values, 150).unwrap();
    assert_eq!(selected.selection().terms_digest(), digest(42));
    assert_eq!(selected.base_commitment().authority().generation().get(), 1);
    assert_eq!(selection().base_agreement(), id(1));
    assert_eq!(selection().proposal(), id(2));
    assert_eq!(selection().authorization(), id(3));
    for error in [
        ReplacementTermsError::InvalidGraph,
        ReplacementTermsError::MissingHistory,
        ReplacementTermsError::Conflicted,
        ReplacementTermsError::Uninterpreted,
        ReplacementTermsError::WrongBase,
        ReplacementTermsError::WrongProposal,
        ReplacementTermsError::WrongAuthorization,
        ReplacementTermsError::TermsDigestMismatch,
        ReplacementTermsError::Expired,
        ReplacementTermsError::ClockAhead,
        ReplacementTermsError::Withdrawn,
        ReplacementTermsError::Rejected,
        ReplacementTermsError::ConflictingAuthorizations,
    ] {
        assert!(error.to_string().len() < 80);
        assert!(core::error::Error::source(&error).is_none());
    }
}
