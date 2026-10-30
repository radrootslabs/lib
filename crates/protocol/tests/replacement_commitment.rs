#![cfg(feature = "commerce")]
use radroots_domain::{
    CommerceContent, CommerceHeader, CommercePayload, PublicKey, RecordId, Sequence, SourceId,
    TermsAuthorization, TermsProposal, UnixSeconds,
};
use radroots_protocol::{
    command::CapturedTermsInstallation,
    commerce::{self, DecodedRecord, EncodedRecord},
    terms::EncodedTerms,
};
use serde_json::{Value, json};

fn time(value: u64) -> UnixSeconds {
    UnixSeconds::new(value).unwrap()
}
fn decoded(content: &CommerceContent) -> DecodedRecord {
    commerce::decode(EncodedRecord::capture(content).unwrap().protected_wire()).unwrap()
}
struct Fixture {
    command: CapturedTermsInstallation,
    base: DecodedRecord,
    proposal: DecodedRecord,
    authorization: DecodedRecord,
    head: (RecordId, Sequence),
}
fn fixture(buyer_proposes: bool) -> Fixture {
    let original: Value =
        serde_json::from_str(include_str!("fixtures/commerce_signing.v1.json")).unwrap();
    let request = commerce::decode(original["request_wire"].as_str().unwrap().as_bytes()).unwrap();
    let base = commerce::decode(original["record_wire"].as_str().unwrap().as_bytes()).unwrap();
    let CommercePayload::Request(requested) = request.content().payload() else {
        panic!()
    };
    let terms = requested.terms();
    let header = base.content().header();
    let make = |author: PublicKey,
                previous: &DecodedRecord,
                sequence: u64,
                payload: CommercePayload,
                extra: &[RecordId]| {
        let mut parents: Vec<_> = terms
            .lines()
            .iter()
            .map(|line| SourceId::from_bytes(line.source().event_id().into_bytes()))
            .collect();
        parents.extend(
            [previous.record_id(), base.record_id()]
                .into_iter()
                .chain(extra.iter().copied())
                .map(|id| SourceId::from_bytes(id.into_bytes())),
        );
        parents.sort();
        parents.dedup();
        decoded(
            &CommerceContent::new(
                CommerceHeader::new(
                    header.thread_id(),
                    header.order_id(),
                    author,
                    header.buyer(),
                    header.seller(),
                    time(452),
                    Sequence::new(sequence).unwrap(),
                    Some(previous.record_id()),
                    parents,
                )
                .unwrap(),
                payload,
            )
            .unwrap(),
        )
    };
    let proposer = if buyer_proposes {
        header.buyer()
    } else {
        header.seller()
    };
    let authorizer = if buyer_proposes {
        header.seller()
    } else {
        header.buyer()
    };
    let proposal = make(
        proposer,
        if buyer_proposes { &request } else { &base },
        2,
        CommercePayload::TermsProposed(
            TermsProposal::new(
                Some(base.record_id()),
                terms.clone(),
                time(700),
                "complete replacement",
            )
            .unwrap(),
        ),
        &[],
    );
    let authorization = make(
        authorizer,
        if buyer_proposes { &base } else { &request },
        2,
        CommercePayload::TermsAuthorized(TermsAuthorization::new(
            proposal.record_id(),
            EncodedTerms::capture(terms).unwrap().digest(),
            time(650),
        )),
        &[proposal.record_id()],
    );
    let seller = if buyer_proposes {
        &authorization
    } else {
        &proposal
    };
    let buyer = if buyer_proposes {
        &proposal
    } else {
        &authorization
    };
    let head = (
        seller.record_id(),
        seller.content().header().author_sequence(),
    );
    let mut wire: Value = serde_json::from_str(original["command_wire"].as_str().unwrap()).unwrap();
    wire["type"] = json!("orders.install_terms");
    wire["input"] = json!({"proposal":proposal.record_id(),"authorization":authorization.record_id(),"terms_digest":EncodedTerms::capture(terms).unwrap().digest(),"allocation":{"type":"automatic"}});
    wire["context"]["approval"] = Value::Null;
    wire["context"]["not_after"] = json!("600");
    wire["context"]["expected"]["agreement"] = json!(base.record_id());
    wire["context"]["expected"]["seller_sequence"] = json!("1");
    wire["context"]["expected"]["buyer_head"] = json!(buyer.record_id());
    let command = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    Fixture {
        command,
        base,
        proposal,
        authorization,
        head,
    }
}

#[test]
fn either_participant_proposes_and_the_next_seller_record_binds_all_originals() {
    for buyer_proposes in [false, true] {
        let f = fixture(buyer_proposes);
        let rumor = commerce::replacement_commitment_rumor(
            &f.command,
            &f.base,
            &f.proposal,
            &f.authorization,
            f.head,
            time(453),
        )
        .unwrap();
        let wire: Value = serde_json::from_str(rumor.protected_wire()).unwrap();
        let record = commerce::decode(wire["content"].as_str().unwrap().as_bytes()).unwrap();
        let header = record.content().header();
        assert_eq!(header.author_sequence().get(), 3);
        assert_eq!(header.author_previous(), Some(f.head.0));
        assert_eq!(header.author(), f.base.content().header().seller());
        assert_eq!(header.created_at(), time(453));
        for id in [
            f.base.record_id(),
            f.proposal.record_id(),
            f.authorization.record_id(),
            f.head.0,
            f.command
                .context()
                .expected()
                .unwrap()
                .buyer_head()
                .unwrap(),
        ] {
            assert!(
                header
                    .parents()
                    .contains(&SourceId::from_bytes(id.into_bytes()))
            );
        }
        let CommercePayload::AgreementCommitted(commitment) = record.content().payload() else {
            panic!()
        };
        assert_eq!(commitment.base_agreement(), Some(f.base.record_id()));
        assert_eq!(commitment.authorization(), f.authorization.record_id());
        assert_eq!(commitment.terms_digest(), f.command.input().terms_digest());
        assert_eq!(commitment.seller_sequence().get(), 2);
        assert_eq!(commitment.authority(), f.command.context().authority());
        assert_eq!(record.record_id(), rumor.record_id());
        assert_eq!(
            commerce::replacement_commitment_rumor(
                &f.command,
                &f.base,
                &f.proposal,
                &f.authorization,
                f.head,
                time(453)
            )
            .unwrap()
            .protected_wire(),
            rumor.protected_wire()
        );
    }
}

#[test]
fn changed_base_sources_digest_sequence_authority_and_known_author_heads_refuse() {
    let f = fixture(false);
    let original: Value = serde_json::from_slice(f.command.protected_wire()).unwrap();
    for (path, value) in [
        ("/context/expected/agreement", json!("71".repeat(32))),
        ("/context/expected/agreement", Value::Null),
        ("/context/expected/seller_sequence", json!("2")),
        ("/context/expected/buyer_head", Value::Null),
        (
            "/context/expected/buyer_head",
            json!(f.proposal.record_id()),
        ),
        ("/context/authority/generation", json!("2")),
        ("/input/proposal", json!("72".repeat(32))),
        ("/input/authorization", json!("73".repeat(32))),
        ("/input/terms_digest", json!("74".repeat(32))),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        let command =
            CapturedTermsInstallation::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            commerce::replacement_commitment_rumor(
                &command,
                &f.base,
                &f.proposal,
                &f.authorization,
                f.head,
                time(453)
            )
            .is_err(),
            "{path}"
        );
    }
    for head in [
        (f.base.record_id(), Sequence::new(2).unwrap()),
        (f.authorization.record_id(), Sequence::new(2).unwrap()),
        (f.head.0, Sequence::new(1).unwrap()),
    ] {
        assert!(
            commerce::replacement_commitment_rumor(
                &f.command,
                &f.base,
                &f.proposal,
                &f.authorization,
                head,
                time(453)
            )
            .is_err()
        );
    }
    assert!(
        commerce::replacement_commitment_rumor(
            &f.command,
            &f.base,
            &f.authorization,
            &f.proposal,
            f.head,
            time(453)
        )
        .is_err()
    );
}

#[test]
fn no_deadline_or_future_author_claim_is_replaced_with_ambient_time() {
    let f = fixture(true);
    for at in [0, 451, 600, 650, 700] {
        assert!(
            commerce::replacement_commitment_rumor(
                &f.command,
                &f.base,
                &f.proposal,
                &f.authorization,
                f.head,
                time(at)
            )
            .is_err(),
            "{at}"
        );
    }
    assert!(
        commerce::replacement_commitment_rumor(
            &f.command,
            &f.base,
            &f.proposal,
            &f.authorization,
            f.head,
            time(599)
        )
        .is_ok()
    );
}
