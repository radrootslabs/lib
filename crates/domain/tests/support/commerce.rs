use super::agreement::{buyer, group, line, money, seller, terms, window};
use radroots_domain::*;

fn record(id: u8) -> RecordId {
    RecordId::from_bytes([id; 32])
}
fn source(id: u8) -> SourceId {
    SourceId::from_bytes([id; 32])
}
fn time(value: u64) -> UnixSeconds {
    UnixSeconds::new(value).unwrap()
}
fn seq(value: u64) -> Sequence {
    Sequence::new(value).unwrap()
}
fn authority() -> AuthorityRef {
    AuthorityRef::new(
        ExecutorId::from_bytes([20; 32]),
        Generation::new(2).unwrap(),
    )
}
fn digest() -> TermsDigest {
    TermsDigest::from_bytes([21; 32])
}
fn private_terms() -> Terms {
    terms(vec![line(7, 9)], vec![group(9)], money("50"), money("700")).unwrap()
}
fn quantities() -> Vec<LineQuantity> {
    vec![LineQuantity::new(
        AgreementLineId::from_bytes([7; 32]),
        "2".parse().unwrap(),
    )]
}
fn measurement() -> FinalMeasurement {
    FinalMeasurement::new(
        AgreementLineId::from_bytes([7; 32]),
        Quantity::new("1".parse().unwrap(), listing::MeasurementUnit::Kilogram).unwrap(),
        money("325"),
    )
}
fn parents() -> Vec<SourceId> {
    [3, 10, 11, 12, 13, 14, 15, 16].map(source).to_vec()
}
fn header(author: PublicKey, order: Option<OrderId>, parents: Vec<SourceId>) -> CommerceHeader {
    CommerceHeader::new(
        ThreadId::from_bytes([1; 32]),
        order,
        author,
        buyer(),
        seller(),
        time(100),
        seq(1),
        None,
        parents,
    )
    .unwrap()
}
fn order() -> Option<OrderId> {
    Some(OrderId::from_bytes([2; 32]))
}
fn issue() -> IssueReport {
    IssueReport::new(
        IssueCode::Shortage,
        "Private issue",
        quantities(),
        vec![record(10)],
    )
    .unwrap()
}
fn resolution() -> Resolution {
    Resolution::new(
        vec![record(11)],
        vec![record(12)],
        quantities(),
        "Private resolution",
    )
    .unwrap()
}

// Expected roles come from the operation contract, independently of the
// production role matcher: bit1 seller, bit2 buyer, both requires later
// authenticated parent-relative counterparty checks.
fn cases() -> Vec<(CommercePayload, &'static str, u8)> {
    let quote = TermsQuote::new(private_terms(), time(200), Some(source(10)));
    assert_eq!(quote.terms(), &private_terms());
    assert_eq!(quote.expires_at(), time(200));
    assert_eq!(quote.in_reply_to(), Some(source(10)));
    let request = TermsRequest::new(private_terms(), time(201), Some(record(10)));
    assert_eq!(request.terms(), &private_terms());
    assert_eq!(request.expires_at(), time(201));
    assert_eq!(request.quote(), Some(record(10)));
    let reason = RecordReason::new(record(10), "Private reason 🥕").unwrap();
    assert_eq!(reason.target(), record(10));
    assert_eq!(reason.reason(), "Private reason 🥕");
    let proposal = TermsProposal::new(
        Some(record(11)),
        private_terms(),
        time(202),
        "Private proposal",
    )
    .unwrap();
    assert_eq!(proposal.base_agreement(), Some(record(11)));
    assert_eq!(proposal.terms(), &private_terms());
    assert_eq!(proposal.expires_at(), time(202));
    assert_eq!(proposal.reason(), "Private proposal");
    let authorized = TermsAuthorization::new(record(10), digest(), time(203));
    assert_eq!(authorized.proposal(), record(10));
    assert_eq!(authorized.terms_digest(), digest());
    assert_eq!(authorized.expires_at(), time(203));
    let committed =
        AgreementCommitment::new(Some(record(11)), record(10), digest(), authority(), seq(7))
            .unwrap();
    assert_eq!(committed.base_agreement(), Some(record(11)));
    assert_eq!(committed.authorization(), record(10));
    assert_eq!(committed.terms_digest(), digest());
    assert_eq!(committed.authority(), authority());
    assert_eq!(committed.seller_sequence(), seq(7));
    let acknowledged = WithdrawalAcknowledgement::new(record(10), seq(8)).unwrap();
    assert_eq!(acknowledged.withdrawal(), record(10));
    assert_eq!(acknowledged.seller_sequence(), seq(8));
    let cancellation =
        CancellationProposal::new(record(11), quantities(), "Private cancellation", time(204))
            .unwrap();
    assert_eq!(cancellation.agreement(), record(11));
    assert_eq!(cancellation.residual(), quantities());
    assert_eq!(cancellation.reason(), "Private cancellation");
    assert_eq!(cancellation.expires_at(), time(204));
    let authorization = ProposalAuthorization::new(record(10), time(205));
    assert_eq!(authorization.proposal(), record(10));
    assert_eq!(authorization.expires_at(), time(205));
    let released = ResidualRelease::new(
        record(10),
        record(11),
        record(12),
        quantities(),
        authority(),
        seq(9),
    )
    .unwrap();
    assert_eq!(released.proposal(), record(10));
    assert_eq!(released.authorization(), record(11));
    assert_eq!(released.agreement(), record(12));
    assert_eq!(released.residual(), quantities());
    assert_eq!(released.authority(), authority());
    assert_eq!(released.seller_sequence(), seq(9));
    let ready = ReadinessClaim::new(
        record(10),
        FulfillmentGroupId::from_bytes([9; 32]),
        quantities(),
        window(),
    )
    .unwrap();
    assert_eq!(ready.agreement(), record(10));
    assert_eq!(
        ready.fulfillment_group(),
        FulfillmentGroupId::from_bytes([9; 32])
    );
    assert_eq!(ready.lines(), quantities());
    assert_eq!(ready.window(), window());
    let handover = HandoverClaim::new(
        record(10),
        FulfillmentGroupId::from_bytes([9; 32]),
        quantities(),
        vec![measurement()],
        time(90),
        authority(),
        seq(10),
    )
    .unwrap();
    assert_eq!(handover.agreement(), record(10));
    assert_eq!(
        handover.fulfillment_group(),
        FulfillmentGroupId::from_bytes([9; 32])
    );
    assert_eq!(handover.lines(), quantities());
    assert_eq!(handover.measurements(), [measurement()]);
    assert_eq!(handover.occurred_at(), time(90));
    assert_eq!(handover.authority(), authority());
    assert_eq!(handover.seller_sequence(), seq(10));
    let receipt = ReceiptObservation::new(
        record(10),
        quantities(),
        Some(record(11)),
        time(91),
        "Private receipt",
    )
    .unwrap();
    assert_eq!(receipt.handover(), record(10));
    assert_eq!(receipt.lines(), quantities());
    assert_eq!(receipt.supersedes(), Some(record(11)));
    assert_eq!(receipt.occurred_at(), time(91));
    assert_eq!(receipt.note(), "Private receipt");
    let issue = issue();
    assert_eq!(issue.code(), IssueCode::Shortage);
    assert_eq!(issue.description(), "Private issue");
    assert_eq!(issue.lines(), quantities());
    assert_eq!(issue.source_records(), [record(10)]);
    let resolution = resolution();
    assert_eq!(resolution.issues(), [record(11)]);
    assert_eq!(resolution.accepted_receipts(), [record(12)]);
    assert_eq!(resolution.release_residual(), quantities());
    assert_eq!(resolution.summary(), "Private resolution");
    let resolution_proposal = ResolutionProposal::new(record(10), resolution.clone(), time(206));
    assert_eq!(resolution_proposal.agreement(), record(10));
    assert_eq!(resolution_proposal.resolution(), &resolution);
    assert_eq!(resolution_proposal.expires_at(), time(206));
    let resolved = ResolutionCommitment::new(record(10), record(11), authority(), seq(11)).unwrap();
    assert_eq!(resolved.proposal(), record(10));
    assert_eq!(resolved.authorization(), record(11));
    assert_eq!(resolved.authority(), authority());
    assert_eq!(resolved.seller_sequence(), seq(11));
    vec![
        (CommercePayload::Quote(quote), "quote", 1),
        (CommercePayload::Request(request), "request", 2),
        (CommercePayload::Decline(reason.clone()), "decline", 1),
        (
            CommercePayload::TermsProposed(proposal),
            "terms_proposed",
            3,
        ),
        (
            CommercePayload::TermsAuthorized(authorized),
            "terms_authorized",
            3,
        ),
        (
            CommercePayload::TermsRejected(reason.clone()),
            "terms_rejected",
            3,
        ),
        (
            CommercePayload::AgreementCommitted(committed),
            "agreement_committed",
            1,
        ),
        (
            CommercePayload::WithdrawalRequested(reason),
            "withdrawal_requested",
            2,
        ),
        (
            CommercePayload::WithdrawalAcknowledged(acknowledged),
            "withdrawal_acknowledged",
            1,
        ),
        (
            CommercePayload::CancellationProposed(cancellation),
            "cancellation_proposed",
            3,
        ),
        (
            CommercePayload::CancellationAuthorized(authorization),
            "cancellation_authorized",
            3,
        ),
        (
            CommercePayload::ResidualReleased(released),
            "residual_released",
            1,
        ),
        (CommercePayload::Ready(ready), "ready", 1),
        (CommercePayload::Handover(handover), "handover", 1),
        (CommercePayload::Receipt(receipt), "receipt", 2),
        (CommercePayload::Issue(issue), "issue", 3),
        (
            CommercePayload::ResolutionProposed(resolution_proposal),
            "resolution_proposed",
            3,
        ),
        (
            CommercePayload::ResolutionAuthorized(authorization),
            "resolution_authorized",
            3,
        ),
        (
            CommercePayload::ResolutionCommitted(resolved),
            "resolution_committed",
            1,
        ),
    ]
}

#[test]
fn all_nineteen_complete_families_preserve_exact_data_and_declared_role_order_reference_rules() {
    let cases = cases();
    assert_eq!(cases.len(), 19);
    for (payload, token, roles) in cases {
        assert_eq!(payload.record_type().as_str(), token);
        assert_eq!(format!("{payload:?}"), "CommercePayload(<redacted>)");
        for (author, bit) in [(seller(), 1), (buyer(), 2)] {
            let original = header(author, order(), parents());
            let result = CommerceContent::new(original.clone(), payload.clone());
            if roles & bit == 0 {
                assert_eq!(result.unwrap_err(), CommerceError::InvalidAuthor, "{token}");
                continue;
            }
            let value = result.unwrap();
            assert_eq!(value.profile(), "radroots.commerce");
            assert_eq!(value.version(), 1);
            assert_eq!(value.record_type().as_str(), token);
            assert_eq!(value.header(), &original);
            assert_eq!(value.payload(), &payload);
            assert_eq!(value, value.clone());
            assert_eq!(format!("{value:?}"), "CommerceContent(<redacted>)");
            let no_order = CommerceContent::new(header(author, None, parents()), payload.clone());
            if token == "quote" {
                assert!(no_order.is_ok());
            } else {
                assert_eq!(
                    no_order.unwrap_err(),
                    CommerceError::InvalidOrder,
                    "{token}"
                );
            }
            assert_eq!(
                CommerceContent::new(header(author, order(), vec![]), payload.clone()).unwrap_err(),
                CommerceError::MissingParent,
                "{token}"
            );
            let required: &[u8] = match token {
                "quote" | "request" => &[3, 10],
                "terms_proposed" => &[3, 11],
                "agreement_committed" | "receipt" | "resolution_committed" => &[10, 11],
                "residual_released" | "resolution_proposed" => &[10, 11, 12],
                "cancellation_proposed" => &[11],
                "decline"
                | "terms_authorized"
                | "terms_rejected"
                | "withdrawal_requested"
                | "withdrawal_acknowledged"
                | "cancellation_authorized"
                | "ready"
                | "handover"
                | "issue"
                | "resolution_authorized" => &[10],
                _ => panic!("unknown fixture family"),
            };
            for missing in required {
                let mut remaining = parents();
                remaining.retain(|parent| *parent != source(*missing));
                assert_eq!(
                    CommerceContent::new(header(author, order(), remaining), payload.clone())
                        .unwrap_err(),
                    CommerceError::MissingParent,
                    "{token}: missing source{missing}"
                );
            }
        }
    }
}

#[test]
fn common_scope_claims_are_exact_bounded_and_have_no_implicit_genesis_or_source_coverage() {
    let thread = ThreadId::from_bytes([1; 32]);
    let make = |author, buyer, seller, sequence, previous, parents| {
        CommerceHeader::new(
            thread,
            order(),
            author,
            buyer,
            seller,
            time(100),
            sequence,
            previous,
            parents,
        )
    };
    let outsider =
        PublicKey::from_hex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
            .unwrap();
    assert_eq!(
        make(seller(), seller(), seller(), seq(1), None, vec![]).unwrap_err(),
        CommerceError::PartiesCoincide
    );
    assert_eq!(
        make(outsider, buyer(), seller(), seq(1), None, vec![]).unwrap_err(),
        CommerceError::InvalidAuthor
    );
    assert_eq!(
        make(seller(), buyer(), seller(), seq(0), None, vec![]).unwrap_err(),
        CommerceError::InvalidSequence
    );
    assert_eq!(
        make(
            seller(),
            buyer(),
            seller(),
            seq(1),
            Some(record(16)),
            parents()
        )
        .unwrap_err(),
        CommerceError::InvalidPrevious
    );
    assert_eq!(
        make(seller(), buyer(), seller(), seq(2), None, parents()).unwrap_err(),
        CommerceError::InvalidPrevious
    );
    assert_eq!(
        make(
            seller(),
            buyer(),
            seller(),
            seq(2),
            Some(record(16)),
            vec![]
        )
        .unwrap_err(),
        CommerceError::MissingParent
    );
    for invalid in [
        vec![source(2), source(1)],
        vec![source(1); 2],
        (0..9).map(source).collect(),
    ] {
        assert_eq!(
            make(seller(), buyer(), seller(), seq(1), None, invalid).unwrap_err(),
            CommerceError::InvalidParents
        );
    }
    let mut spacious = Vec::with_capacity(4096);
    spacious.extend(parents());
    let value = make(
        seller(),
        buyer(),
        seller(),
        seq(i64::MAX as u64),
        Some(record(16)),
        spacious,
    )
    .unwrap();
    assert_eq!(value.thread_id(), thread);
    assert_eq!(value.order_id(), order());
    assert_eq!(value.author(), seller());
    assert_eq!(value.buyer(), buyer());
    assert_eq!(value.seller(), seller());
    assert_eq!(value.created_at(), time(100));
    assert_eq!(value.author_sequence(), seq(i64::MAX as u64));
    assert_eq!(value.author_previous(), Some(record(16)));
    assert_eq!(value.parents(), parents());
    assert_eq!(value, value.clone());
    assert_eq!(format!("{value:?}"), "CommerceHeader(<redacted>)");
}

#[test]
fn complete_terms_sources_and_participants_are_required_without_conflating_references_with_proof() {
    let outsider =
        PublicKey::from_hex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
            .unwrap();
    for payload in [
        CommercePayload::Quote(TermsQuote::new(private_terms(), time(200), None)),
        CommercePayload::Request(TermsRequest::new(private_terms(), time(200), None)),
        CommercePayload::TermsProposed(
            TermsProposal::new(None, private_terms(), time(200), "").unwrap(),
        ),
    ] {
        let author = if payload.record_type() == CommerceType::Request {
            outsider
        } else {
            seller()
        };
        let different = CommerceHeader::new(
            ThreadId::from_bytes([1; 32]),
            order(),
            author,
            outsider,
            seller(),
            time(100),
            seq(1),
            None,
            parents(),
        )
        .unwrap();
        assert_eq!(
            CommerceContent::new(different, payload.clone()).unwrap_err(),
            CommerceError::TermsPartiesMismatch
        );
        let author = if payload.record_type() == CommerceType::Request {
            buyer()
        } else {
            outsider
        };
        let different_seller = CommerceHeader::new(
            ThreadId::from_bytes([1; 32]),
            order(),
            author,
            buyer(),
            outsider,
            time(100),
            seq(1),
            None,
            parents(),
        )
        .unwrap();
        assert_eq!(
            CommerceContent::new(different_seller, payload.clone()).unwrap_err(),
            CommerceError::TermsPartiesMismatch
        );
        let author = if payload.record_type() == CommerceType::Request {
            buyer()
        } else {
            seller()
        };
        assert!(
            CommerceContent::new(header(author, order(), vec![source(3)]), payload.clone()).is_ok()
        );
        assert_eq!(
            CommerceContent::new(header(author, order(), vec![source(10)]), payload).unwrap_err(),
            CommerceError::MissingParent
        );
    }
    let committed =
        AgreementCommitment::new(None, record(10), digest(), authority(), seq(1)).unwrap();
    assert!(
        CommerceContent::new(
            header(seller(), order(), vec![source(10)]),
            CommercePayload::AgreementCommitted(committed)
        )
        .is_ok()
    );
    let receipt = ReceiptObservation::new(record(10), quantities(), None, time(90), "").unwrap();
    assert!(
        CommerceContent::new(
            header(buyer(), order(), vec![source(10)]),
            CommercePayload::Receipt(receipt)
        )
        .is_ok()
    );
    // Payload reference maxima and the eight-parent envelope apply jointly.
    // References are refused rather than silently omitted or claimed covered.
    let issue =
        IssueReport::new(IssueCode::Other, "", vec![], (0..9).map(record).collect()).unwrap();
    assert_eq!(
        CommerceContent::new(
            header(seller(), order(), (0..8).map(source).collect()),
            CommercePayload::Issue(issue)
        )
        .unwrap_err(),
        CommerceError::MissingParent
    );
}

#[test]
fn bounded_private_text_preserves_unicode_and_optional_empty_claims_without_diagnostic_disclosure()
{
    let maximum = "🥕".repeat(1024);
    assert_eq!(
        RecordReason::new(record(10), &maximum).unwrap().reason(),
        maximum
    );
    assert_eq!(RecordReason::new(record(10), "").unwrap().reason(), "");
    for invalid in ["x".repeat(1025), "🥕".repeat(1025), "x".repeat(4097)] {
        assert_eq!(
            RecordReason::new(record(10), &invalid).unwrap_err(),
            CommerceError::InvalidText
        );
        assert_eq!(
            TermsProposal::new(None, private_terms(), time(200), &invalid).unwrap_err(),
            CommerceError::InvalidText
        );
        assert_eq!(
            CancellationProposal::new(record(10), quantities(), &invalid, time(200)).unwrap_err(),
            CommerceError::InvalidText
        );
        assert_eq!(
            ReceiptObservation::new(record(10), quantities(), None, time(90), &invalid)
                .unwrap_err(),
            CommerceError::InvalidText
        );
    }
    let maximum = "🥕".repeat(2048);
    assert_eq!(
        IssueReport::new(IssueCode::Other, &maximum, vec![], vec![])
            .unwrap()
            .description(),
        maximum
    );
    assert_eq!(
        Resolution::new(vec![record(10)], vec![], vec![], &maximum)
            .unwrap()
            .summary(),
        maximum
    );
    for invalid in ["x".repeat(2049), "🥕".repeat(2049), "x".repeat(8193)] {
        assert_eq!(
            IssueReport::new(IssueCode::Other, &invalid, vec![], vec![]).unwrap_err(),
            CommerceError::InvalidText
        );
        assert_eq!(
            Resolution::new(vec![record(10)], vec![], vec![], &invalid).unwrap_err(),
            CommerceError::InvalidText
        );
    }
    for (code, token) in [
        (IssueCode::Shortage, "shortage"),
        (IssueCode::Quality, "quality"),
        (IssueCode::Substitution, "substitution"),
        (IssueCode::Collection, "collection"),
        (IssueCode::Excess, "excess"),
        (IssueCode::Other, "other"),
    ] {
        assert_eq!(code.as_str(), token);
    }
    assert_eq!(
        format!(
            "{:?}",
            RecordReason::new(record(10), "Private reason").unwrap()
        ),
        "RecordReason(<redacted>)"
    );
    assert_eq!(format!("{:?}", issue()), "IssueReport(<redacted>)");
    assert_eq!(format!("{:?}", resolution()), "Resolution(<redacted>)");
}

#[test]
fn quantity_reference_and_measurement_collections_reject_ambiguity_and_preserve_zero_observations()
{
    let maximum_lines: Vec<_> = (0..32)
        .map(|id| LineQuantity::new(AgreementLineId::from_bytes([id; 32]), "1".parse().unwrap()))
        .collect();
    let value = IssueReport::new(
        IssueCode::Other,
        "",
        maximum_lines.clone(),
        (0..32).map(record).collect(),
    )
    .unwrap();
    assert_eq!(value.lines(), maximum_lines);
    assert_eq!(value.source_records().len(), 32);
    assert!(
        Resolution::new(
            (0..32).map(record).collect(),
            (32..64).map(record).collect(),
            maximum_lines.clone(),
            ""
        )
        .is_ok()
    );
    for invalid in [vec![maximum_lines[0]; 2], vec![maximum_lines[0]; 33]] {
        assert_eq!(
            IssueReport::new(IssueCode::Other, "", invalid.clone(), vec![]).unwrap_err(),
            CommerceError::InvalidLines
        );
        assert_eq!(
            ReadinessClaim::new(
                record(10),
                FulfillmentGroupId::from_bytes([9; 32]),
                invalid,
                window()
            )
            .unwrap_err(),
            CommerceError::InvalidLines
        );
    }
    for invalid in [vec![record(1); 2], vec![record(1); 33]] {
        assert_eq!(
            IssueReport::new(IssueCode::Other, "", vec![], invalid.clone()).unwrap_err(),
            CommerceError::InvalidReferences
        );
        assert_eq!(
            Resolution::new(invalid, vec![], vec![], "").unwrap_err(),
            CommerceError::InvalidReferences
        );
    }
    assert_eq!(
        Resolution::new(vec![], vec![], vec![], "").unwrap_err(),
        CommerceError::InvalidReferences
    );
    assert_eq!(
        ReadinessClaim::new(
            record(10),
            FulfillmentGroupId::from_bytes([9; 32]),
            vec![],
            window()
        )
        .unwrap_err(),
        CommerceError::InvalidLines
    );
    assert_eq!(
        CancellationProposal::new(record(10), vec![], "", time(200)).unwrap_err(),
        CommerceError::InvalidLines
    );
    assert_eq!(
        ReceiptObservation::new(record(10), vec![], None, time(90), "").unwrap_err(),
        CommerceError::InvalidLines
    );
    let zero = LineQuantity::new(AgreementLineId::from_bytes([7; 32]), Decimal::ZERO);
    assert_eq!(zero.line_id(), AgreementLineId::from_bytes([7; 32]));
    assert_eq!(zero.amount(), Decimal::ZERO);
    assert_eq!(format!("{zero:?}"), "LineQuantity(<redacted>)");
    let observed =
        ReceiptObservation::new(record(10), vec![zero], None, time(90), "Nothing observed")
            .unwrap();
    assert_eq!(observed.lines(), [zero]);
    assert!(
        CommerceContent::new(
            header(buyer(), order(), vec![source(10)]),
            CommercePayload::Receipt(observed)
        )
        .is_ok()
    );
    let measured = measurement();
    assert_eq!(measured.line_id(), AgreementLineId::from_bytes([7; 32]));
    assert_eq!(
        measured.quantity(),
        Quantity::new("1".parse().unwrap(), listing::MeasurementUnit::Kilogram).unwrap()
    );
    assert_eq!(measured.line_total(), money("325"));
    assert_eq!(format!("{measured:?}"), "FinalMeasurement(<redacted>)");
    let unrelated = FinalMeasurement::new(
        AgreementLineId::from_bytes([8; 32]),
        measured.quantity(),
        money("325"),
    );
    for invalid in [vec![measured; 2], vec![measured; 33], vec![unrelated]] {
        assert_eq!(
            HandoverClaim::new(
                record(10),
                FulfillmentGroupId::from_bytes([9; 32]),
                quantities(),
                invalid,
                time(90),
                authority(),
                seq(1)
            )
            .unwrap_err(),
            CommerceError::InvalidMeasurements
        );
    }
    let no_measurements = HandoverClaim::new(
        record(10),
        FulfillmentGroupId::from_bytes([9; 32]),
        quantities(),
        vec![],
        time(90),
        authority(),
        seq(1),
    )
    .unwrap();
    assert!(no_measurements.measurements().is_empty());
}

#[test]
fn every_seller_decision_requires_positive_sequence_without_minting_authority() {
    assert_eq!(
        AgreementCommitment::new(None, record(10), digest(), authority(), seq(0)).unwrap_err(),
        CommerceError::InvalidSequence
    );
    assert_eq!(
        WithdrawalAcknowledgement::new(record(10), seq(0)).unwrap_err(),
        CommerceError::InvalidSequence
    );
    assert_eq!(
        ResidualRelease::new(
            record(10),
            record(11),
            record(12),
            quantities(),
            authority(),
            seq(0)
        )
        .unwrap_err(),
        CommerceError::InvalidSequence
    );
    assert_eq!(
        HandoverClaim::new(
            record(10),
            FulfillmentGroupId::from_bytes([9; 32]),
            quantities(),
            vec![],
            time(90),
            authority(),
            seq(0)
        )
        .unwrap_err(),
        CommerceError::InvalidSequence
    );
    assert_eq!(
        ResolutionCommitment::new(record(10), record(11), authority(), seq(0)).unwrap_err(),
        CommerceError::InvalidSequence
    );
}

#[test]
fn record_errors_are_input_free_and_bounded() {
    for error in [
        CommerceError::PartiesCoincide,
        CommerceError::InvalidAuthor,
        CommerceError::InvalidOrder,
        CommerceError::InvalidSequence,
        CommerceError::InvalidPrevious,
        CommerceError::InvalidParents,
        CommerceError::MissingParent,
        CommerceError::TermsPartiesMismatch,
        CommerceError::InvalidText,
        CommerceError::InvalidLines,
        CommerceError::InvalidMeasurements,
        CommerceError::InvalidReferences,
    ] {
        assert!(error.to_string().len() < 96);
        assert!(core::error::Error::source(&error).is_none());
        assert!(!format!("{error:?}").contains("Private reason"));
    }
}
