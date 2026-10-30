//! Actual authenticated replacement sources and native T0/T1/S/T2 effects.
use super::*;
use nostr::JsonUtil;

fn proof(r: &Rig) -> AttachmentProof {
    r.enrollment
        .proof()
        .with_secret(|bytes| AttachmentProof::from_protected_input(bytes))
        .unwrap()
}
use radroots_domain::{
    AgreementLine, CommerceContent, CommerceHeader, CommercePayload, RecordId, Sequence, SourceId,
    Terms, TermsAuthorization, TermsProposal,
};
use radroots_protocol::{
    command::CapturedTermsInstallation,
    commerce::{DecodedRecord, EncodedCommerceRumor, EncodedRecord},
    terms::EncodedTerms,
};

fn record(receipt: &StockAcceptanceReceipt) -> DecodedRecord {
    let rumor: serde_json::Value = serde_json::from_str(receipt.rumor().protected_wire()).unwrap();
    radroots_protocol::commerce::decode(rumor["content"].as_str().unwrap().as_bytes()).unwrap()
}
fn amount(original: &Terms, quantity: &str) -> Terms {
    assert_eq!(original.lines().len(), 1);
    let line = &original.lines()[0];
    let amount = quantity.parse().unwrap();
    let radroots_domain::AgreementPrice::Fixed(price) = line.price() else {
        panic!("fixed-price fixture")
    };
    let price = radroots_domain::AgreedFixedPrice::new(
        price.rate(),
        price.basis(),
        price.discount_bps(),
        None,
    );
    let total = price.total(amount).unwrap();
    let line = AgreementLine::new(
        line.id(),
        line.source(),
        line.option_id(),
        line.product().clone(),
        line.option().clone(),
        amount,
        line.fulfillment_group(),
        line.source_lot_label(),
        radroots_domain::AgreementPrice::Fixed(price),
        total,
    )
    .unwrap();
    Terms::new(
        original.buyer(),
        original.seller(),
        vec![line],
        original.fulfillment().to_vec(),
        original.fulfillment_charge(),
        original.fulfillment_charge().checked_add(total).unwrap(),
        original.supply().clone(),
    )
    .unwrap()
}
async fn retain_content(r: &Rig, content: &CommerceContent, scalar: u8) -> DecodedRecord {
    let encoded = EncodedRecord::capture(content).unwrap();
    let decoded = radroots_protocol::commerce::decode(encoded.protected_wire()).unwrap();
    let rumor = EncodedCommerceRumor::capture(content).unwrap();
    let keys = nostr::Keys::new(nostr::SecretKey::from_hex(&format!("{scalar:064x}")).unwrap());
    assert_eq!(
        keys.public_key().to_hex(),
        content.header().author().to_hex()
    );
    let receiver =
        nostr::PublicKey::from_hex(&r.enrollment.input().participant().to_hex()).unwrap();
    let unsigned = nostr::UnsignedEvent::from_json(rumor.protected_wire()).unwrap();
    let wrap = nostr::EventBuilder::gift_wrap(&keys, &receiver, unsigned, Vec::<nostr::Tag>::new())
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
    let capture = StockSourceCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        &opening,
    )
    .unwrap();
    assert_eq!(capture.profile(), StockSourceProfile::Known);
    let (minimum, _, _) = minima(r).await;
    r.store
        .retain_stock_source(r.binding.clone(), minimum, &capture, r.enrollment.proof())
        .await
        .unwrap();
    decoded
}
async fn proposal(
    r: &Rig,
    base: &StockAcceptanceReceipt,
    terms: Terms,
    buyer_proposes: bool,
) -> CapturedTermsInstallation {
    let base = record(base);
    let scope = scope();
    let snapshot = history(r).await;
    let proposer = if buyer_proposes {
        scope.buyer()
    } else {
        scope.seller()
    };
    let authorizer = if buyer_proposes {
        scope.seller()
    } else {
        scope.buyer()
    };
    let proposer_head = if buyer_proposes {
        snapshot.analysis().buyer_head()
    } else {
        snapshot.analysis().seller_head()
    }
    .unwrap();
    let authorizer_head = if buyer_proposes {
        snapshot.analysis().seller_head()
    } else {
        snapshot.analysis().buyer_head()
    }
    .unwrap();
    let make = |author, previous, sequence, payload, extra: &[RecordId]| {
        let mut parents: Vec<_> = terms
            .lines()
            .iter()
            .map(|line| SourceId::from_bytes(line.source().event_id().into_bytes()))
            .collect();
        parents.extend(
            [base.record_id(), previous]
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
    // These cases start from an initial agreement: both scoped author chains
    // have one record. The null-order quote remains a separate chain.
    let proposed = make(
        proposer,
        proposer_head,
        2,
        CommercePayload::TermsProposed(
            TermsProposal::new(
                Some(base.record_id()),
                terms.clone(),
                UnixSeconds::new(230).unwrap(),
                "complete updated amount",
            )
            .unwrap(),
        ),
        &[],
    );
    let proposed = retain_content(r, &proposed, if buyer_proposes { 2 } else { 3 }).await;
    let authorized = make(
        authorizer,
        authorizer_head,
        2,
        CommercePayload::TermsAuthorized(TermsAuthorization::new(
            proposed.record_id(),
            EncodedTerms::capture(&terms).unwrap().digest(),
            UnixSeconds::new(230).unwrap(),
        )),
        &[proposed.record_id()],
    );
    let authorized = retain_content(r, &authorized, if buyer_proposes { 3 } else { 2 }).await;
    let snapshot = history(r).await;
    let mut wire = serde_json::json!({"type":"orders.install_terms","context":context(r,110),"input":{"proposal":proposed.record_id(),"authorization":authorized.record_id(),"terms_digest":EncodedTerms::capture(&terms).unwrap().digest(),"allocation":{"type":"automatic"}}});
    wire["context"]["expected"] = serde_json::json!({"agreement":base.record_id(),"buyer_head":snapshot.analysis().buyer_head(),"checkpoint":snapshot.checkpoint().unwrap(),"order_id":scope.order_id(),"seller_sequence":"1"});
    let capture = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    wire["context"]["approval"] = serde_json::json!({"intent_digest":capture.approval_intent_digest(),"scope":"orders.install_terms","expires_at":"230"});
    CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap()
}
async fn prepare_terms(
    r: &Rig,
    command: &CapturedTermsInstallation,
) -> Result<StockAcceptanceReceipt, OperationError> {
    let capture = StockConsentCapture::prepare(
        SellerAuthoritySelection::from_attachment(r.enrollment.input()),
        r.enrollment.input().grant_id(),
        StockConsentMode::ExecuteAndRecover,
        StockResources::all_pools(),
        UnixSeconds::new(230).unwrap(),
        CapturedTermsInstallation::parse(command.protected_wire()).unwrap(),
    )
    .unwrap();
    r.store
        .issue_stock_consent(
            r.binding.clone(),
            inventory_minimum(r).await,
            &capture,
            r.enrollment.proof(),
        )
        .await?;
    let (sources, listings, minimum) = minima(r).await;
    let request = StockAcceptanceRequest::capture_terms(
        r.binding.clone(),
        minimum,
        sources,
        listings,
        command,
    )
    .unwrap();
    match r
        .store
        .prepare_stock_acceptance(request, r.enrollment.proof())
        .await?
    {
        StockAcceptanceOutcome::Prepared(value) => Ok(*value),
        _ => panic!("preparation must confirm"),
    }
}

#[tokio::test]
async fn positive_replacement_holds_only_the_delta_then_installs_through_original_signing_owner() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let original_terms = inspect(&r).await.terms().clone();
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let initial = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &initial, 42).await.unwrap());
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    balances(&r, 80, 7, 0, 3, 3).await;
    let line = &original_terms.lines()[0];
    physical(&r,serde_json::json!({"type":"stock.map_option","context":context(&r,91),"input":{"listing":line.source().coordinate(),"option_id":line.option_id(),"pool_id":pool(81),"expected_mapping_version":"1","per_sale_unit":{"amount":"1","unit":"each"}}})).await;
    let command = proposal(&r, &initial, amount(&original_terms, "5"), false).await;
    let amended = prepare_terms(&r, &command).await.unwrap();
    assert_eq!(amended.allocation_count(), 1);
    balances(&r, 80, 5, 2, 3, 4).await;
    bound(bind(&r, &amended, 43).await.unwrap());
    retain(&r, &amended, recipient).await;
    let replacement = installed(install(&r, &amended).await.unwrap());
    assert_ne!(replacement.record_id(), initial.rumor().record_id());
    balances(&r, 80, 5, 0, 5, 5).await;
    balances(&r, 81, 10, 0, 0, 1).await;
    let view = r
        .store
        .inspect_stock(
            r.binding.clone(),
            inventory_minimum(&r).await,
            pool(80),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.installed_obligations().remaining().get(), 5);
    assert_eq!(view.installed_obligations().backed().get(), 5);
    let revision = inventory_minimum(&r).await;
    assert_eq!(
        installed(install(&r, &amended).await.unwrap()).record_id(),
        replacement.record_id()
    );
    assert_eq!(inventory_minimum(&r).await, revision);
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn negative_replacement_keeps_old_obligations_while_pending_and_releases_only_at_t2() {
    let mut r = Rig::new().await;
    seed(&r, "10").await;
    let original_terms = inspect(&r).await.terms().clone();
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let initial = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &initial, 42).await.unwrap());
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    let command = proposal(&r, &initial, amount(&original_terms, "1"), true).await;
    let amended = prepare_terms(&r, &command).await.unwrap();
    assert_eq!(amended.allocation_count(), 0);
    balances(&r, 80, 7, 0, 3, 3).await;
    bound(bind(&r, &amended, 43).await.unwrap());
    retain(&r, &amended, recipient).await;
    installed(install(&r, &amended).await.unwrap());
    balances(&r, 80, 9, 0, 1, 4).await;
    let view = r
        .store
        .inspect_stock(
            r.binding.clone(),
            inventory_minimum(&r).await,
            pool(80),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.installed_obligations().remaining().get(), 1);
    assert_eq!(view.installed_obligations().backed().get(), 1);
    assert_eq!(view.installed_obligations().deficit().get(), 0);
    r.store.close().await.unwrap();
    r.store = NativeWorkspace::open_seller_stock(
        &r.path,
        r.enrollment.input().clone(),
        r.fixture.crypto(),
        r.clock.clone(),
    )
    .await
    .unwrap();
    r.binding = r
        .store
        .authenticate(r.enrollment.input().clone(), r.enrollment.proof())
        .await
        .unwrap();
    balances(&r, 80, 9, 0, 1, 4).await;
    let reopened = r
        .store
        .inspect_stock(
            r.binding.clone(),
            inventory_minimum(&r).await,
            pool(80),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reopened.installed_obligations().remaining().get(), 1);
    let mut raw = database(&r).await;
    sqlx::query("DELETE FROM stock_obligation_changes")
        .execute(&mut raw)
        .await
        .unwrap();
    assert_eq!(
        r.store
            .inspect_stock(
                r.binding.clone(),
                inventory_minimum(&r).await,
                pool(80),
                proof(&r)
            )
            .await
            .unwrap_err(),
        OperationError::RecoveryRequired
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn authorized_reduction_of_a_known_deficit_does_not_manufacture_available_stock() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let terms = inspect(&r).await.terms().clone();
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let initial = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &initial, 42).await.unwrap());
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    physical(&r,serde_json::json!({"type":"stock.spoil","context":context(&r,91),"input":{"pool_id":pool(80),"expected_version":"3","amount":"9","reason":"known loss before mutually authorized reduction"}})).await;
    balances(&r, 80, 0, 0, 1, 4).await;
    let command = proposal(&r, &initial, amount(&terms, "1"), false).await;
    let amended = prepare_terms(&r, &command).await.unwrap();
    bound(bind(&r, &amended, 43).await.unwrap());
    retain(&r, &amended, recipient).await;
    installed(install(&r, &amended).await.unwrap());
    balances(&r, 80, 0, 0, 1, 4).await;
    let view = r
        .store
        .inspect_stock(
            r.binding.clone(),
            inventory_minimum(&r).await,
            pool(80),
            proof(&r),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.installed_obligations().remaining().get(), 1);
    assert_eq!(view.installed_obligations().backed().get(), 1);
    assert_eq!(view.installed_obligations().deficit().get(), 0);
    let mut raw = database(&r).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_deficits")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM stock_obligation_changes")
            .fetch_one(&mut raw)
            .await
            .unwrap(),
        1
    );
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn changed_original_backing_before_first_signing_refuses_without_releasing_pending_claims() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let terms = inspect(&r).await.terms().clone();
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let initial = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &initial, 42).await.unwrap());
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    let command = proposal(&r, &initial, amount(&terms, "5"), false).await;
    let amended = prepare_terms(&r, &command).await.unwrap();
    physical(&r,serde_json::json!({"type":"stock.spoil","context":context(&r,91),"input":{"pool_id":pool(80),"expected_version":"4","amount":"6","reason":"loss while replacement remains unissued"}})).await;
    assert_eq!(
        bind(&r, &amended, 43).await.unwrap_err(),
        OperationError::StaleState
    );
    balances(&r, 80, 0, 2, 2, 5).await;
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn failed_negative_installation_rolls_back_all_changes_and_retains_original_issued_evidence()
{
    let r = Rig::new().await;
    seed(&r, "10").await;
    let terms = inspect(&r).await.terms().clone();
    let command = command(&r, 90, false).await;
    consent(&r, &command).await;
    let initial = prepare(&r, &command).await.unwrap();
    bound(bind(&r, &initial, 42).await.unwrap());
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    let command = proposal(&r, &initial, amount(&terms, "1"), false).await;
    let amended = prepare_terms(&r, &command).await.unwrap();
    bound(bind(&r, &amended, 43).await.unwrap());
    let evidence = retain(&r, &amended, recipient).await;
    let minimum = inventory_minimum(&r).await;
    let mut raw = database(&r).await;
    sqlx::raw_sql("CREATE TRIGGER fault BEFORE INSERT ON stock_obligation_changes BEGIN SELECT RAISE(ABORT,'synthetic fault'); END").execute(&mut raw).await.unwrap();
    assert_eq!(
        install(&r, &amended).await.unwrap_err(),
        OperationError::StorageFailure
    );
    assert_eq!(inventory_minimum(&r).await, minimum);
    balances(&r, 80, 7, 0, 3, 3).await;
    sqlx::raw_sql("DROP TRIGGER fault")
        .execute(&mut raw)
        .await
        .unwrap();
    let result = installed(install(&r, &amended).await.unwrap());
    assert_eq!(
        result
            .publication_work()
            .iter()
            .find(|work| work.recipient() == recipient)
            .unwrap()
            .evidence_digest(),
        Some(evidence.digest())
    );
    balances(&r, 80, 9, 0, 1, 4).await;
    raw.close().await.unwrap();
    r.store.close().await.unwrap();
}

#[tokio::test]
async fn changing_an_original_multi_pool_split_requires_explicit_complete_allocation() {
    let r = Rig::new().await;
    seed(&r, "10").await;
    let terms = inspect(&r).await.terms().clone();
    let initial = ready(&r).await;
    let recipient = peer(&r, &initial);
    retain(&r, &initial, recipient).await;
    installed(install(&r, &initial).await.unwrap());
    let command = proposal(&r, &initial, amount(&terms, "5"), false).await;
    assert_eq!(
        prepare_terms(&r, &command).await.unwrap_err(),
        OperationError::UnsupportedOperation
    );
    balances(&r, 80, 9, 0, 1, 3).await;
    balances(&r, 81, 8, 0, 2, 3).await;
    let mut wire: serde_json::Value = serde_json::from_slice(command.protected_wire()).unwrap();
    wire["context"]["operation_id"] = serde_json::json!(OperationId::from_bytes([111; 32]));
    wire["context"]["approval"] = serde_json::Value::Null;
    let line = terms.lines()[0].id();
    wire["input"]["allocation"] = serde_json::json!({"type":"explicit","allocations":[{"line_id":line,"pool_id":pool(80),"quantity":{"amount":"2","unit":"each"},"expected_pool_version":"3"},{"line_id":line,"pool_id":pool(81),"quantity":{"amount":"3","unit":"each"},"expected_pool_version":"3"}]});
    let command = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    wire["context"]["approval"] = serde_json::json!({"intent_digest":command.approval_intent_digest(),"scope":"orders.install_terms","expires_at":"230"});
    let command = CapturedTermsInstallation::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
    let amended = prepare_terms(&r, &command).await.unwrap();
    assert_eq!(amended.allocation_count(), 2);
    balances(&r, 80, 8, 1, 1, 4).await;
    balances(&r, 81, 7, 1, 2, 4).await;
    bound(bind(&r, &amended, 43).await.unwrap());
    retain(&r, &amended, recipient).await;
    installed(install(&r, &amended).await.unwrap());
    balances(&r, 80, 8, 0, 2, 5).await;
    balances(&r, 81, 7, 0, 3, 5).await;
    r.store.close().await.unwrap();
}
