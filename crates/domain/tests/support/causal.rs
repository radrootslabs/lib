use super::agreement::{buyer, group, line, money, seller, terms};
use radroots_domain::{causal::*, *};

fn id(value: u64) -> RecordId {
    let mut out = [0; 32];
    out[24..].copy_from_slice(&value.to_be_bytes());
    RecordId::from_bytes(out)
}
fn src(value: u64) -> SourceId {
    SourceId::from_bytes(id(value).into_bytes())
}
fn order() -> OrderId {
    OrderId::from_bytes([2; 32])
}
fn thread() -> ThreadId {
    ThreadId::from_bytes([1; 32])
}
fn scope() -> CausalScope {
    CausalScope::new(thread(), order(), buyer(), seller()).unwrap()
}
fn make(author: PublicKey, sequence: u64, previous: Option<u64>, extra: &[u64]) -> CommerceContent {
    let mut parents = vec![src(9000)];
    parents.extend(previous.map(src));
    parents.extend(extra.iter().copied().map(src));
    parents.sort();
    parents.dedup();
    let header = CommerceHeader::new(
        thread(),
        Some(order()),
        author,
        buyer(),
        seller(),
        UnixSeconds::new(100).unwrap(),
        Sequence::new(sequence).unwrap(),
        previous.map(id),
        parents,
    )
    .unwrap();
    let reason = RecordReason::new(id(9000), "private reason").unwrap();
    let payload = if author == seller() {
        CommercePayload::Decline(reason)
    } else {
        CommercePayload::WithdrawalRequested(reason)
    };
    CommerceContent::new(header, payload).unwrap()
}
fn evaluate(
    values: &[(u64, CommerceContent)],
    external: &[u64],
    uninterpreted: &[u64],
) -> CausalAnalysis {
    analyze(
        scope(),
        &values
            .iter()
            .map(|(identity, content)| CausalRecord::new(id(*identity), content))
            .collect::<Vec<_>>(),
        &external.iter().copied().map(src).collect::<Vec<_>>(),
        &uninterpreted.iter().copied().map(src).collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn every_arrival_order_and_exact_duplicates_preserve_both_complete_author_heads() {
    let values = [
        (1, make(seller(), 1, None, &[])),
        (2, make(buyer(), 1, None, &[1])),
        (3, make(seller(), 2, Some(1), &[2])),
        (4, make(buyer(), 2, Some(2), &[3])),
    ];
    let expected = evaluate(&values, &[9000], &[]);
    assert_eq!(expected.readiness(), CausalReadiness::Complete);
    assert_eq!(expected.seller_head(), Some(id(3)));
    assert_eq!(expected.buyer_head(), Some(id(4)));
    assert_eq!(expected.quote_head(), None);
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let indices = [a, b, c, d];
                    if (0..4).any(|i| indices[..i].contains(&indices[i])) {
                        continue;
                    }
                    let shuffled = indices.map(|i| values[i].clone());
                    assert_eq!(evaluate(&shuffled, &[9000], &[]), expected);
                    let duplicated = [shuffled.as_slice(), &shuffled[..2]].concat();
                    assert_eq!(evaluate(&duplicated, &[9000], &[]), expected);
                }
            }
        }
    }
    assert!(!expected.has_cycle());
    assert!(expected.missing().is_empty());
    assert!(expected.conflicts().is_empty());
}

#[test]
fn exact_missing_predecessors_cannot_be_satisfied_by_generic_sources_or_empty_history() {
    let values = [(2, make(buyer(), 2, Some(1), &[5, 6]))];
    let result = evaluate(&values, &[9000, 1, 5], &[]);
    assert_eq!(result.readiness(), CausalReadiness::MissingHistory);
    assert_eq!(result.missing(), [src(1), src(6)]);
    assert_eq!(result.buyer_head(), None);
    assert!(!result.has_cycle());
    let result = evaluate(&values, &[9000], &[1]);
    assert_eq!(result.readiness(), CausalReadiness::Uninterpreted);
    assert!(result.missing().contains(&src(1)));
    let empty = evaluate(&[], &[], &[]);
    assert_eq!(empty.readiness(), CausalReadiness::Complete);
    assert_eq!(empty.buyer_head(), None);
    // Complete describes only supplied coverage; it cannot invent a request.
}

#[test]
fn incompatible_same_sequence_records_expose_all_forks_and_retain_only_the_uncontested_prefix() {
    let values = [
        (1, make(seller(), 1, None, &[])),
        (2, make(seller(), 2, Some(1), &[])),
        (3, make(seller(), 2, Some(1), &[20])),
        (4, make(seller(), 3, Some(2), &[])),
        (5, make(seller(), 2, Some(1), &[21])),
    ];
    let expected = evaluate(&values, &[9000, 20, 21], &[]);
    assert_eq!(expected.readiness(), CausalReadiness::Conflicted);
    assert_eq!(expected.seller_head(), Some(id(1)));
    assert_eq!(expected.conflicts(), [id(2), id(3), id(5)]);
    assert!(!expected.has_cycle());
    for rotation in 0..values.len() {
        let mut reversed = values.to_vec();
        reversed.rotate_left(rotation);
        reversed.reverse();
        assert_eq!(evaluate(&reversed, &[9000, 21, 20], &[]), expected);
    }
}

#[test]
fn predecessor_author_and_sequence_mismatches_block_contiguous_heads() {
    for values in [
        vec![
            (1, make(seller(), 1, None, &[])),
            (2, make(buyer(), 2, Some(1), &[])),
        ],
        vec![
            (1, make(seller(), 1, None, &[])),
            (2, make(seller(), 3, Some(1), &[])),
        ],
    ] {
        let result = evaluate(&values, &[9000], &[]);
        assert_eq!(result.readiness(), CausalReadiness::Conflicted);
        assert_eq!(result.conflicts(), [id(2)]);
        assert_eq!(result.buyer_head(), None);
        assert_eq!(result.seller_head(), Some(id(1)));
    }
}

#[test]
fn cross_author_cycles_and_descendants_are_bounded_and_never_timestamp_ranked() {
    let values = [
        (1, make(seller(), 1, None, &[2])),
        (2, make(buyer(), 1, None, &[1])),
        (3, make(buyer(), 2, Some(2), &[])),
    ];
    let result = evaluate(&values, &[9000], &[]);
    assert_eq!(result.readiness(), CausalReadiness::Conflicted);
    assert!(result.has_cycle());
    assert_eq!(result.conflicts(), [id(1), id(2), id(3)]);
    assert_eq!(result.buyer_head(), None);
    assert_eq!(result.seller_head(), None);
    let missing_only = evaluate(&[(1, make(seller(), 1, None, &[2]))], &[9000], &[]);
    assert!(!missing_only.has_cycle());
    assert_eq!(missing_only.readiness(), CausalReadiness::MissingHistory);
}

#[test]
fn authenticated_uninterpreted_evidence_blocks_reduction_even_when_not_referenced_by_known_nodes() {
    let values = [(1, make(seller(), 1, None, &[]))];
    let result = evaluate(&values, &[9000, 44], &[44, 43, 44]);
    assert_eq!(result.readiness(), CausalReadiness::Uninterpreted);
    assert_eq!(result.uninterpreted(), [src(43), src(44)]);
    assert!(result.missing().is_empty());
    assert_eq!(result.seller_head(), Some(id(1)));
}

fn quote(sequence: u64, previous: Option<u64>) -> CommerceContent {
    let mut parents = vec![SourceId::from_bytes([3; 32])];
    parents.extend(previous.map(src));
    parents.sort();
    let header = CommerceHeader::new(
        thread(),
        None,
        seller(),
        buyer(),
        seller(),
        UnixSeconds::new(100).unwrap(),
        Sequence::new(sequence).unwrap(),
        previous.map(id),
        parents,
    )
    .unwrap();
    let complete = terms(vec![line(7, 9)], vec![group(9)], money("50"), money("700")).unwrap();
    CommerceContent::new(
        header,
        CommercePayload::Quote(TermsQuote::new(
            complete,
            UnixSeconds::new(200).unwrap(),
            None,
        )),
    )
    .unwrap()
}
#[test]
fn null_order_quote_chains_remain_separate_from_the_same_seller_order_chain() {
    let values = [
        (1, quote(1, None)),
        (2, quote(2, Some(1))),
        (3, make(seller(), 1, None, &[2])),
    ];
    let external = [src(9000), SourceId::from_bytes([3; 32])];
    let records = values
        .iter()
        .map(|(i, c)| CausalRecord::new(id(*i), c))
        .collect::<Vec<_>>();
    let result = analyze(scope(), &records, &external, &[]).unwrap();
    assert_eq!(result.readiness(), CausalReadiness::Complete);
    assert_eq!(result.quote_head(), Some(id(2)));
    assert_eq!(result.seller_head(), Some(id(3)));
    let bad = make(seller(), 3, Some(2), &[]);
    let mut records = records;
    records.push(CausalRecord::new(id(4), &bad));
    let result = analyze(scope(), &records, &external, &[]).unwrap();
    assert_eq!(result.readiness(), CausalReadiness::Conflicted);
    assert!(result.conflicts().contains(&id(4)));
}

#[test]
fn mixed_scope_and_same_identity_changed_complete_payload_are_explicit_errors() {
    let first = make(seller(), 1, None, &[]);
    let second = make(seller(), 1, None, &[22]);
    assert_eq!(
        analyze(
            scope(),
            &[
                CausalRecord::new(id(1), &first),
                CausalRecord::new(id(1), &second)
            ],
            &[src(9000), src(22)],
            &[]
        )
        .unwrap_err(),
        CausalError::IdentityConflict
    );
    for different in [
        CausalScope::new(ThreadId::from_bytes([9; 32]), order(), buyer(), seller()).unwrap(),
        CausalScope::new(thread(), OrderId::from_bytes([9; 32]), buyer(), seller()).unwrap(),
        CausalScope::new(thread(), order(), seller(), buyer()).unwrap(),
    ] {
        assert_eq!(
            analyze(different, &[CausalRecord::new(id(1), &first)], &[], &[]).unwrap_err(),
            CausalError::InvalidScope
        );
    }
    assert_eq!(
        CausalScope::new(thread(), order(), seller(), seller()),
        Err(CausalError::InvalidScope)
    );
}

#[test]
fn maximum_reverse_dependency_chain_is_iterative_and_limits_never_silently_truncate() {
    assert_eq!(MAX_CAUSAL_RECORDS, 1000);
    assert_eq!(MAX_CAUSAL_SOURCES, 8000);
    let values = (1..=MAX_CAUSAL_RECORDS as u64)
        .map(|sequence| {
            (
                1001 - sequence,
                make(
                    seller(),
                    sequence,
                    (sequence > 1).then_some(1002 - sequence),
                    &[],
                ),
            )
        })
        .collect::<Vec<_>>();
    let result = evaluate(&values, &[9000], &[]);
    assert_eq!(result.readiness(), CausalReadiness::Complete);
    assert_eq!(result.seller_head(), Some(id(1)));
    let records = values
        .iter()
        .map(|(i, c)| CausalRecord::new(id(*i), c))
        .collect::<Vec<_>>();
    let mut excess = records.clone();
    excess.push(records[0]);
    assert_eq!(
        analyze(scope(), &excess, &[src(9000)], &[]).unwrap_err(),
        CausalError::LimitExceeded
    );
    for (external, uninterpreted) in [(vec![src(9); 8001], vec![]), (vec![], vec![src(9); 8001])] {
        assert_eq!(
            analyze(scope(), &records, &external, &uninterpreted).unwrap_err(),
            CausalError::LimitExceeded
        );
    }
}

#[test]
fn diagnostics_do_not_copy_private_contents_or_claim_permission() {
    let content = make(seller(), 1, None, &[]);
    let record = CausalRecord::new(id(1), &content);
    assert_eq!(format!("{record:?}"), "CausalRecord(<untrusted claim>)");
    let result = analyze(scope(), &[record], &[src(9000)], &[]).unwrap();
    assert_eq!(format!("{result:?}"), "CausalAnalysis(<private coverage>)");
    for error in [
        CausalError::InvalidScope,
        CausalError::IdentityConflict,
        CausalError::LimitExceeded,
    ] {
        assert!(error.to_string().len() < 64);
        assert!(core::error::Error::source(&error).is_none());
    }
}
