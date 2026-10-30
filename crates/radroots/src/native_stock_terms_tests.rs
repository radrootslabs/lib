//! Ordinary client ownership of an actual complete replacement installation.
use super::*;
use nostr::JsonUtil;
use radroots_domain::{
    AgreedFixedPrice, AgreementLine, AgreementPrice, CommerceContent, CommerceHeader,
    CommercePayload, RecordId, Sequence, SourceId, Terms, TermsAuthorization, TermsProposal,
};
use radroots_protocol::{
    command::CapturedTermsInstallation,
    commerce::{DecodedRecord, EncodedCommerceRumor, EncodedRecord},
    terms::EncodedTerms,
};
use radroots_signing::request::CancellationSignal;

async fn incoming(r: &Rig, content: &CommerceContent, scalar: u8) -> DecodedRecord {
    let encoded = EncodedRecord::capture(content).unwrap();
    let decoded = radroots_protocol::commerce::decode(encoded.protected_wire()).unwrap();
    let rumor = EncodedCommerceRumor::capture(content).unwrap();
    let keys = nostr::Keys::new(nostr::SecretKey::from_hex(&format!("{scalar:064x}")).unwrap());
    let receiver =
        nostr::PublicKey::from_hex(&r.enrollment.input().participant().to_hex()).unwrap();
    let wrap = nostr::EventBuilder::gift_wrap(
        &keys,
        &receiver,
        nostr::UnsignedEvent::from_json(rumor.protected_wire()).unwrap(),
        Vec::<nostr::Tag>::new(),
    )
    .await
    .unwrap();
    let provider =
        LocalSigner::with_clock(SecretKey::parse(&format!("{:064x}", 3)).unwrap(), || {
            Ok(150000)
        })
        .unwrap();
    let opening = radroots_nostr::commerce::open(
        &provider,
        ProviderCall::new(
            provider.public_key(),
            NonZeroU64::new(230000).unwrap(),
            CancellationSignal::new(),
        ),
        &wrap.as_json(),
    )
    .await
    .unwrap();
    let capture = radroots_storage_sqlite::workspace::StockSourceCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        &opening,
    )
    .unwrap();
    let (sources, _, _) = r.minima().await;
    r.native
        .retain_stock_source(
            r.scope.binding().clone(),
            sources,
            &capture,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    decoded
}

async fn exercise(quantity: &str, remaining: u64, available: u64) {
    let r = Rig::new().await;
    let request = r.request().await;
    let old = request.terms();
    let line = &old.lines()[0];
    let AgreementPrice::Fixed(old_price) = line.price() else {
        panic!("fixed fixture")
    };
    let price = AgreedFixedPrice::new(
        old_price.rate(),
        old_price.basis(),
        old_price.discount_bps(),
        None,
    );
    let quantity = quantity.parse().unwrap();
    let total = price.total(quantity).unwrap();
    let line = AgreementLine::new(
        line.id(),
        line.source(),
        line.option_id(),
        line.product().clone(),
        line.option().clone(),
        quantity,
        line.fulfillment_group(),
        line.source_lot_label(),
        AgreementPrice::Fixed(price),
        total,
    )
    .unwrap();
    let terms = Terms::new(
        old.buyer(),
        old.seller(),
        vec![line],
        old.fulfillment().to_vec(),
        old.fulfillment_charge(),
        old.fulfillment_charge().checked_add(total).unwrap(),
        old.supply().clone(),
    )
    .unwrap();
    let (_, initial) = r.accept().await;
    assert!(initial.installation().is_some());
    let scope = request.scope();
    let make = |author, previous, sequence, payload, extra: &[RecordId]| {
        let mut parents: Vec<_> = terms
            .lines()
            .iter()
            .map(|line| SourceId::from_bytes(line.source().event_id().into_bytes()))
            .collect();
        parents.extend(
            [initial.record_id(), previous]
                .into_iter()
                .chain(extra.iter().copied())
                .map(|id| SourceId::from_bytes(id.into_bytes())),
        );
        parents.sort();
        parents.dedup();
        CommerceContent::new(
            CommerceHeader::new(
                scope.thread_id(),
                Some(scope.order_id()),
                author,
                scope.buyer(),
                scope.seller(),
                UnixSeconds::new(150).unwrap(),
                Sequence::new(sequence).unwrap(),
                Some(previous),
                parents,
            )
            .unwrap(),
            payload,
        )
        .unwrap()
    };
    let proposal = make(
        scope.seller(),
        initial.record_id(),
        2,
        CommercePayload::TermsProposed(
            TermsProposal::new(
                Some(initial.record_id()),
                terms.clone(),
                UnixSeconds::new(230).unwrap(),
                "complete replacement",
            )
            .unwrap(),
        ),
        &[],
    );
    let proposal = incoming(&r, &proposal, 3).await;
    let authorization = make(
        scope.buyer(),
        request.request(),
        2,
        CommercePayload::TermsAuthorized(TermsAuthorization::new(
            proposal.record_id(),
            EncodedTerms::capture(&terms).unwrap().digest(),
            UnixSeconds::new(230).unwrap(),
        )),
        &[proposal.record_id()],
    );
    let authorization = incoming(&r, &authorization, 2).await;
    let (sources, listings, minimum) = r.minima().await;
    let history = r
        .native
        .stock_order_history(
            r.scope.binding().clone(),
            minimum,
            sources,
            listings,
            scope,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let mut wire = serde_json::json!({"type":"orders.install_terms","context":r.context(110),"input":{"proposal":proposal.record_id(),"authorization":authorization.record_id(),"terms_digest":EncodedTerms::capture(&terms).unwrap().digest(),"allocation":{"type":"automatic"}}});
    wire["context"]["expected"] = serde_json::json!({"agreement":initial.record_id(),"buyer_head":history.analysis().buyer_head(),"checkpoint":history.checkpoint().unwrap(),"order_id":scope.order_id(),"seller_sequence":"1"});
    let command = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    wire["context"]["approval"] = serde_json::json!({"intent_digest":command.approval_intent_digest(),"scope":"orders.install_terms","expires_at":"230"});
    let command = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    let context = r
        .scope
        .command_context(
            command.context().operation_id(),
            command.context().expected().cloned(),
            command.context().approval().cloned(),
            UnixSeconds::new(230).unwrap(),
        )
        .unwrap();
    let consent = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        r.enrollment.input().grant_id(),
        StockConsentMode::ExecuteAndRecover,
        StockResources::all_pools(),
        UnixSeconds::new(230).unwrap(),
        CapturedTermsInstallation::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    let (_, _, minimum) = r.minima().await;
    r.native
        .issue_stock_consent(
            r.scope.binding().clone(),
            minimum,
            &consent,
            r.enrollment.proof(),
        )
        .await
        .unwrap();
    let (sources, listings, minimum) = r.minima().await;
    let input = crate::orders::InstallTermsInput::new(
        proposal.record_id(),
        authorization.record_id(),
        command.input().terms_digest(),
        AllocationSelection::automatic(),
    );
    let installed = r
        .client
        .orders(&r.scope)
        .install_terms(context, minimum, sources, listings, input)
        .await
        .unwrap();
    assert!(installed.installation().is_some());
    assert_eq!(installed.attempt().command_digest(), command.digest());
    assert_ne!(installed.record_id(), initial.record_id());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 4);
    assert_eq!(r.io.sent.lock().unwrap().len(), 4);
    let (_, _, minimum) = r.minima().await;
    let current = r
        .client
        .stock(&r.scope)
        .inspect(minimum, PoolId::from_bytes([80; 32]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.installed_obligations().remaining().get(), remaining);
    assert_eq!(
        current
            .pool()
            .snapshot()
            .balances()
            .physical()
            .available
            .get(),
        available
    );
    let resumed = r
        .client
        .orders(&r.scope)
        .resume_acceptance(minimum, command.context().operation_id(), command.digest())
        .await
        .unwrap();
    assert_eq!(resumed.record_id(), installed.record_id());
    assert_eq!(r.control.signs.load(Ordering::SeqCst), 4);
    let (_, _, after) = r.minima().await;
    assert_eq!(after, minimum);
    r.client.close().await.unwrap();
}

#[tokio::test]
async fn ordinary_install_terms_uses_the_same_owner_and_resumes_without_extra_signatures_or_stock()
{
    exercise("5", 5, 5).await;
}

#[tokio::test]
async fn ordinary_negative_install_terms_finishes_self_copy_after_original_release() {
    exercise("1", 1, 9).await;
}
