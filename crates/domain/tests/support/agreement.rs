use radroots_domain::{
    AgreedFixedPrice, AgreementLine, AgreementLineId, AgreementPrice, DiscountBps, EnvelopeEventId,
    FixedPrice, FulfillmentGroup, FulfillmentGroupId, FuturePromise, ListingOptionId, Money,
    MoneyError, PositiveDecimal, PriceTier, PricingError, PrivateFulfillment, PublicKey,
    SupplyCommitment, Terms, TermsError, TimeZoneId, UnixSeconds, VariableWeight, Window,
    listing::*,
};

pub(super) fn seller() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798").unwrap()
}
pub(super) fn buyer() -> PublicKey {
    PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5").unwrap()
}
pub(super) fn money(minor: &str) -> Money {
    Money::from_parts("CAD", minor, 2).unwrap()
}
fn source(author: PublicKey) -> ListingRevision {
    ListingRevision::new(
        ListingCoordinate::new(author, ListingId::from_bytes([1; 16])),
        EnvelopeEventId::from_bytes([3; 32]),
    )
}
pub(super) fn window() -> Window {
    Window::new(
        UnixSeconds::new(100).unwrap(),
        UnixSeconds::new(200).unwrap(),
        TimeZoneId::from_name("Etc/UTC").unwrap(),
    )
    .unwrap()
}
pub(super) fn group(id: u8) -> FulfillmentGroup {
    FulfillmentGroup::new(
        FulfillmentGroupId::from_bytes([id; 32]),
        PrivateFulfillment::new(
            FulfillmentMode::Pickup,
            window(),
            "Private destination",
            "Private instructions",
            Some("Private contact"),
        )
        .unwrap(),
    )
}
fn option() -> ListingOption {
    ListingOption::quote_required(
        ListingOptionId::from_bytes([2; 32]),
        "Original bunch",
        SaleUnit::package_unspecified("bunch").unwrap(),
        "1".parse().unwrap(),
        "1".parse().unwrap(),
        Supply::Estimated {
            amount: "0".parse().unwrap(),
            window: None,
        },
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap()
}
fn agreed(minor: &str) -> AgreementPrice {
    AgreementPrice::Fixed(AgreedFixedPrice::new(
        money(minor),
        "1".parse().unwrap(),
        DiscountBps::NONE,
        None,
    ))
}
#[allow(clippy::too_many_arguments)]
fn line_with(
    id: u8,
    source: ListingRevision,
    option_id: ListingOptionId,
    option: ListingOption,
    amount: &str,
    group: u8,
    label: Option<&str>,
    price: AgreementPrice,
    total: Money,
) -> Result<AgreementLine, TermsError> {
    AgreementLine::new(
        AgreementLineId::from_bytes([id; 32]),
        source,
        option_id,
        super::product(),
        option,
        amount.parse().unwrap(),
        FulfillmentGroupId::from_bytes([group; 32]),
        label,
        price,
        total,
    )
}
pub(super) fn line(id: u8, group: u8) -> AgreementLine {
    line_with(
        id,
        source(seller()),
        option().id(),
        option(),
        "2",
        group,
        Some("Private lot claim"),
        agreed("325"),
        money("650"),
    )
    .unwrap()
}
pub(super) fn terms(
    lines: Vec<AgreementLine>,
    groups: Vec<FulfillmentGroup>,
    fee: Money,
    total: Money,
) -> Result<Terms, TermsError> {
    Terms::new(
        buyer(),
        seller(),
        lines,
        groups,
        fee,
        total,
        SupplyCommitment::StockBacked,
    )
}

#[test]
fn complete_negotiated_quote_terms_preserve_snapshot_and_independently_recompute_fee_and_cap() {
    let line = line(7, 9);
    assert_eq!(line.id(), AgreementLineId::from_bytes([7; 32]));
    assert_eq!(line.source(), source(seller()));
    assert_eq!(line.option_id(), option().id());
    assert_eq!(line.product(), &super::product());
    assert_eq!(line.option(), &option());
    assert_eq!(line.option().price(), &Price::QuoteRequired);
    assert_eq!(line.amount(), "2".parse::<PositiveDecimal>().unwrap());
    assert_eq!(
        line.fulfillment_group(),
        FulfillmentGroupId::from_bytes([9; 32])
    );
    assert_eq!(line.source_lot_label(), Some("Private lot claim"));
    assert_eq!(line.price(), agreed("325"));
    assert_eq!(line.maximum_line_total(), money("650"));
    let value = terms(vec![line], vec![group(9)], money("50"), money("700")).unwrap();
    assert_eq!(value.buyer(), buyer());
    assert_eq!(value.seller(), seller());
    assert_eq!(value.maximum_total(), money("700"));
    assert_eq!(value.fulfillment_charge(), money("50"));
    assert_eq!(value.lines().len(), 1);
    assert_eq!(value.fulfillment(), [group(9)]);
    assert_eq!(
        value.fulfillment()[0].id(),
        FulfillmentGroupId::from_bytes([9; 32])
    );
    assert_eq!(
        value.fulfillment()[0].terms().address(),
        "Private destination"
    );
    assert_eq!(value.supply(), &SupplyCommitment::StockBacked);
    assert_eq!(value.cancellation(), "mutual_residual_v1");
    assert_eq!(value.rounding(), "half_even_net_line_v1");
    assert_eq!(value, value.clone());
    assert_eq!(format!("{value:?}"), "Terms(<redacted>)");
    assert_eq!(
        format!("{:?}", value.lines()[0]),
        "AgreementLine(<redacted>)"
    );
    assert_eq!(
        format!("{:?}", value.fulfillment()[0]),
        "FulfillmentGroup(<redacted>)"
    );
    assert_eq!(
        format!("{:?}", value.supply()),
        "SupplyCommitment(<redacted>)"
    );
}

#[test]
fn resolved_fixed_tiers_and_half_even_values_match_retained_independent_vectors_without_reselection()
 {
    let fixed = FixedPrice::new(
        money("325"),
        "1".parse().unwrap(),
        DiscountBps::new(1000).unwrap(),
        vec![PriceTier::new("2".parse().unwrap(), money("299"))],
    )
    .unwrap();
    let resolved = fixed.line("3.5".parse().unwrap()).unwrap();
    let price = AgreedFixedPrice::from_line(resolved);
    assert_eq!(price.rate(), money("299"));
    assert_eq!(price.basis(), "1".parse::<PositiveDecimal>().unwrap());
    assert_eq!(price.discount_bps(), DiscountBps::new(1000).unwrap());
    assert_eq!(price.selected_tier_from(), Some("2".parse().unwrap()));
    assert_eq!(price.total("3.5".parse().unwrap()).unwrap(), money("942"));
    assert_eq!(
        price.total("1".parse().unwrap()),
        Err(TermsError::InvalidSelectedTier)
    );
    assert_eq!(format!("{price:?}"), "AgreedFixedPrice(<redacted>)");
    for (rate, quantity, expected) in [
        ("325", "2.5", "812"),
        ("327", "2.5", "818"),
        ("0", "100", "0"),
    ] {
        let price =
            AgreedFixedPrice::new(money(rate), "1".parse().unwrap(), DiscountBps::NONE, None);
        assert_eq!(
            price.total(quantity.parse().unwrap()).unwrap(),
            money(expected)
        );
    }
    let full_discount = AgreedFixedPrice::new(
        money("325"),
        "1".parse().unwrap(),
        DiscountBps::FULL,
        Some("0".parse().unwrap()),
    );
    assert_eq!(
        full_discount.total("2".parse().unwrap()).unwrap(),
        money("0")
    );
    let original = ListingOption::new(
        option().id(),
        "Public fixed offering",
        SaleUnit::measured(MeasurementUnit::Kilogram),
        "0.5".parse().unwrap(),
        "0.5".parse().unwrap(),
        Price::Fixed(fixed),
        Supply::Unknown,
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    let line = line_with(
        1,
        source(seller()),
        original.id(),
        original.clone(),
        "3.5",
        1,
        None,
        AgreementPrice::Fixed(price),
        money("942"),
    )
    .unwrap();
    assert_eq!(line.option(), &original);
    assert_eq!(line.source_lot_label(), None);
    assert_eq!(
        terms(vec![line], vec![group(1)], money("0"), money("942"))
            .unwrap()
            .maximum_total(),
        money("942")
    );
}

#[test]
fn variable_agreement_bounds_are_explicit_absolute_line_values_and_never_auto_scaled() {
    let advertised = VariableWeight::new(
        MeasurementUnit::Kilogram,
        "0.8".parse().unwrap(),
        "1.2".parse().unwrap(),
        money("325"),
        "1".parse().unwrap(),
        money("400"),
    )
    .unwrap();
    let original = ListingOption::new(
        option().id(),
        "Estimated bag",
        option().unit().clone(),
        "1".parse().unwrap(),
        "1".parse().unwrap(),
        Price::VariableWeight(advertised),
        Supply::Unknown,
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    let absolute = VariableWeight::new(
        MeasurementUnit::Kilogram,
        "2".parse().unwrap(),
        "4".parse().unwrap(),
        money("325"),
        "1".parse().unwrap(),
        money("1000"),
    )
    .unwrap();
    let line = line_with(
        1,
        source(seller()),
        original.id(),
        original.clone(),
        "3",
        1,
        None,
        AgreementPrice::VariableWeight(absolute),
        money("1000"),
    )
    .unwrap();
    assert_eq!(line.option(), &original);
    assert_eq!(line.price(), AgreementPrice::VariableWeight(absolute));
    assert_eq!(line.maximum_line_total(), money("1000"));
    assert_eq!(
        absolute
            .total_at(
                radroots_domain::Quantity::new("3".parse().unwrap(), MeasurementUnit::Kilogram)
                    .unwrap()
            )
            .unwrap(),
        money("975")
    );
    assert_eq!(
        absolute.total_at(
            radroots_domain::Quantity::new("4".parse().unwrap(), MeasurementUnit::Kilogram)
                .unwrap()
        ),
        Err(PricingError::CapExceeded)
    );
    assert_eq!(
        terms(vec![line], vec![group(1)], money("10"), money("1010"))
            .unwrap()
            .maximum_total(),
        money("1010")
    );
    assert_eq!(
        format!("{:?}", AgreementPrice::VariableWeight(absolute)),
        "AgreementPrice(<redacted>)"
    );
    assert_eq!(
        line_with(
            1,
            source(seller()),
            original.id(),
            original,
            "3",
            1,
            None,
            AgreementPrice::VariableWeight(absolute),
            money("3000")
        )
        .unwrap_err(),
        TermsError::TotalMismatch
    );
}

#[test]
fn line_admission_refuses_snapshot_quantity_tier_text_and_claimed_total_mismatches() {
    assert_eq!(
        line_with(
            1,
            source(seller()),
            ListingOptionId::from_bytes([99; 32]),
            option(),
            "2",
            1,
            None,
            agreed("325"),
            money("650")
        )
        .unwrap_err(),
        TermsError::OptionMismatch
    );
    assert_eq!(
        line_with(
            1,
            source(seller()),
            option().id(),
            option(),
            "0.5",
            1,
            None,
            agreed("325"),
            money("0")
        )
        .unwrap_err(),
        TermsError::Purchase(ListingError::InvalidQuantity)
    );
    let minimum = ListingOption::quote_required(
        option().id(),
        "Even bunches",
        option().unit().clone(),
        "2".parse().unwrap(),
        "4".parse().unwrap(),
        Supply::Unknown,
        LeadSeconds::new(0).unwrap(),
    )
    .unwrap();
    for quantity in ["2", "5"] {
        assert_eq!(
            line_with(
                1,
                source(seller()),
                minimum.id(),
                minimum.clone(),
                quantity,
                1,
                None,
                agreed("0"),
                money("0")
            )
            .unwrap_err(),
            TermsError::Purchase(ListingError::InvalidQuantity)
        );
    }
    for label in ["x".repeat(121), "🥕".repeat(121), "x".repeat(481)] {
        assert_eq!(
            line_with(
                1,
                source(seller()),
                option().id(),
                option(),
                "2",
                1,
                Some(&label),
                agreed("325"),
                money("650")
            )
            .unwrap_err(),
            TermsError::InvalidText
        );
    }
    let label = "🥕".repeat(120);
    assert_eq!(
        line_with(
            1,
            source(seller()),
            option().id(),
            option(),
            "2",
            1,
            Some(&label),
            agreed("325"),
            money("650")
        )
        .unwrap()
        .source_lot_label(),
        Some(label.as_str())
    );
    assert_eq!(
        line_with(
            1,
            source(seller()),
            option().id(),
            option(),
            "2",
            1,
            Some(""),
            agreed("325"),
            money("650")
        )
        .unwrap()
        .source_lot_label(),
        Some("")
    );
    for total in [
        money("649"),
        money("651"),
        Money::from_parts("USD", "650", 2).unwrap(),
    ] {
        assert_eq!(
            line_with(
                1,
                source(seller()),
                option().id(),
                option(),
                "2",
                1,
                None,
                agreed("325"),
                total
            )
            .unwrap_err(),
            TermsError::TotalMismatch
        );
    }
}

#[test]
fn complete_terms_enforce_collection_identity_seller_and_group_closure_before_capture() {
    assert_eq!(
        Terms::new(
            seller(),
            seller(),
            vec![line(1, 1)],
            vec![group(1)],
            money("0"),
            money("650"),
            SupplyCommitment::StockBacked
        )
        .unwrap_err(),
        TermsError::PartiesCoincide
    );
    assert_eq!(
        terms(vec![], vec![group(1)], money("0"), money("0")).unwrap_err(),
        TermsError::InvalidLineCount
    );
    assert_eq!(
        terms(vec![line(1, 1); 33], vec![group(1)], money("0"), money("0")).unwrap_err(),
        TermsError::InvalidLineCount
    );
    assert_eq!(
        terms(vec![line(1, 1)], vec![], money("0"), money("650")).unwrap_err(),
        TermsError::InvalidFulfillmentCount
    );
    assert_eq!(
        terms(
            vec![line(1, 1)],
            vec![group(1); 9],
            money("0"),
            money("650")
        )
        .unwrap_err(),
        TermsError::InvalidFulfillmentCount
    );
    assert_eq!(
        terms(
            vec![line(1, 1); 2],
            vec![group(1)],
            money("0"),
            money("1300")
        )
        .unwrap_err(),
        TermsError::DuplicateLine
    );
    assert_eq!(
        terms(
            vec![line(1, 1)],
            vec![group(1); 2],
            money("0"),
            money("650")
        )
        .unwrap_err(),
        TermsError::DuplicateFulfillment
    );
    assert_eq!(
        terms(vec![line(1, 99)], vec![group(1)], money("0"), money("650")).unwrap_err(),
        TermsError::UnknownFulfillment
    );
    let wrong_source = line_with(
        1,
        source(buyer()),
        option().id(),
        option(),
        "2",
        1,
        None,
        agreed("325"),
        money("650"),
    )
    .unwrap();
    assert_eq!(
        terms(vec![wrong_source], vec![group(1)], money("0"), money("650")).unwrap_err(),
        TermsError::SellerMismatch
    );
    for total in [money("649"), Money::from_parts("USD", "650", 2).unwrap()] {
        assert_eq!(
            terms(vec![line(1, 1)], vec![group(1)], money("0"), total).unwrap_err(),
            TermsError::TotalMismatch
        );
    }
    assert_eq!(
        terms(
            vec![line(1, 1)],
            vec![group(1)],
            Money::from_parts("USD", "0", 2).unwrap(),
            money("650")
        )
        .unwrap_err(),
        TermsError::Pricing(PricingError::Money(MoneyError::CurrencyMismatch))
    );
}

#[test]
fn maximum_collections_preserve_line_order_and_check_aggregate_money_bounds() {
    let mut lines = Vec::with_capacity(4096);
    for id in 0..32 {
        lines.push(line(id, (id % 8) + 1));
    }
    let mut groups = Vec::with_capacity(4096);
    groups.extend((1..=8).map(group));
    let expected = lines.clone();
    let value = terms(lines, groups, money("1"), money("20801")).unwrap();
    assert_eq!(value.lines(), expected);
    assert_eq!(value.fulfillment().len(), 8);
    let maximum = line_with(
        1,
        source(seller()),
        option().id(),
        option(),
        "1",
        1,
        None,
        agreed("999999999999999"),
        money("999999999999999"),
    )
    .unwrap();
    assert_eq!(
        terms(
            vec![maximum],
            vec![group(1)],
            money("1"),
            money("999999999999999")
        )
        .unwrap_err(),
        TermsError::Pricing(PricingError::Money(MoneyError::MinorOutOfRange))
    );
}

#[test]
fn future_promise_requires_exact_bounded_risk_and_confers_no_physical_backing() {
    let risk = "🥕".repeat(512);
    let promise = FuturePromise::new(window(), &risk).unwrap();
    assert_eq!(promise.expected_window(), window());
    assert_eq!(promise.risk_disclosure(), risk);
    assert_eq!(promise, promise.clone());
    assert_eq!(format!("{promise:?}"), "FuturePromise(<redacted>)");
    for text in [
        String::new(),
        "x".repeat(513),
        "🥕".repeat(513),
        "x".repeat(2049),
    ] {
        assert_eq!(
            FuturePromise::new(window(), &text).unwrap_err(),
            TermsError::InvalidText
        );
    }
    let value = Terms::new(
        buyer(),
        seller(),
        vec![line(1, 1)],
        vec![group(1)],
        money("0"),
        money("650"),
        SupplyCommitment::FuturePromise(promise.clone()),
    )
    .unwrap();
    assert_eq!(value.supply(), &SupplyCommitment::FuturePromise(promise));
}

#[test]
fn exact_listing_source_references_retain_full_coordinate_and_signed_revision_identity() {
    let reference = source(seller());
    assert_eq!(reference.coordinate().kind(), 30402);
    assert_eq!(reference.coordinate().author(), seller());
    assert_eq!(reference.coordinate().d(), ListingId::from_bytes([1; 16]));
    assert_eq!(reference.event_id(), EnvelopeEventId::from_bytes([3; 32]));
    assert_eq!(format!("{reference:?}"), "ListingRevision(<redacted>)");
    assert_eq!(
        format!("{:?}", reference.coordinate()),
        "ListingCoordinate(<redacted>)"
    );
    let published = Listing::new(
        reference.coordinate().d(),
        seller(),
        reference.event_id(),
        UnixSeconds::new(101).unwrap(),
        UnixSeconds::new(100).unwrap(),
        ListingText::new("Carrots", "Public terms", "").unwrap(),
        ListingTerms::new(
            ListingState::Active,
            super::product(),
            option().id(),
            vec![option()],
            super::fulfillment(),
            None,
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(published.source(), reference);
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::to_value(reference).unwrap(),
        serde_json::json!({
        "coordinate":{"kind":30402,"author":seller().to_string(),"d":"01".repeat(16)},
        "event_id":"03".repeat(32)})
    );
}

#[test]
fn agreement_failures_remain_typed_bounded_and_input_free() {
    for error in [
        TermsError::PartiesCoincide,
        TermsError::InvalidLineCount,
        TermsError::InvalidFulfillmentCount,
        TermsError::DuplicateLine,
        TermsError::DuplicateFulfillment,
        TermsError::UnknownFulfillment,
        TermsError::OptionMismatch,
        TermsError::SellerMismatch,
        TermsError::InvalidText,
        TermsError::InvalidSelectedTier,
        TermsError::TotalMismatch,
        TermsError::Purchase(ListingError::InvalidQuantity),
        TermsError::Pricing(PricingError::Money(MoneyError::CurrencyMismatch)),
    ] {
        assert!(error.to_string().len() < 128);
        assert!(!format!("{error:?}").contains("Private"));
        let source = core::error::Error::source(&error);
        assert_eq!(
            source.is_some(),
            matches!(error, TermsError::Purchase(_) | TermsError::Pricing(_))
        );
    }
}
