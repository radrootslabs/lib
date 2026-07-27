mod common;

use radroots_core::{
    Percent, Unit,
    pricing::{Discount, DiscountError, DiscountScope, DiscountThreshold, DiscountValue},
};

#[test]
fn is_non_negative_validates_discount_shapes() {
    let money_pos = common::money("1", "USD");
    let money_neg = common::money("-1", "USD");
    let qty_pos = common::qty("1", Unit::Each);
    let qty_neg = common::qty("-1", Unit::Each);
    let pct_pos = Percent::new(common::dec("10"));
    let pct_neg = Percent::new(common::dec("-5"));

    let d = Discount {
        scope: DiscountScope::Bin,
        threshold: DiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 2,
        },
        value: DiscountValue::MoneyPerBin(money_pos.clone()),
    };
    assert!(d.is_non_negative());

    let d = Discount {
        scope: DiscountScope::Bin,
        threshold: DiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 2,
        },
        value: DiscountValue::MoneyPerBin(money_neg),
    };
    assert!(!d.is_non_negative());

    let d = Discount {
        scope: DiscountScope::OrderTotal,
        threshold: DiscountThreshold::OrderQuantity {
            min: qty_pos.clone(),
        },
        value: DiscountValue::Percent(pct_pos.clone()),
    };
    assert!(d.is_non_negative());

    let d = Discount {
        scope: DiscountScope::OrderTotal,
        threshold: DiscountThreshold::OrderQuantity { min: qty_neg },
        value: DiscountValue::Percent(pct_pos),
    };
    assert!(!d.is_non_negative());

    let d = Discount {
        scope: DiscountScope::OrderTotal,
        threshold: DiscountThreshold::OrderQuantity { min: qty_pos },
        value: DiscountValue::Percent(pct_neg),
    };
    assert!(!d.is_non_negative());
}

#[test]
fn checked_constructor_rejects_negative_thresholds_and_values() {
    let negative_threshold = Discount::try_new(
        DiscountScope::OrderTotal,
        DiscountThreshold::OrderQuantity {
            min: common::qty("-1", Unit::Each),
        },
        DiscountValue::Percent(Percent::new(common::dec("10"))),
    );
    assert_eq!(negative_threshold, Err(DiscountError::NegativeThreshold));

    let negative_value = Discount::try_new(
        DiscountScope::Bin,
        DiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 1,
        },
        DiscountValue::Percent(Percent::new(common::dec("-0.01"))),
    );
    assert_eq!(negative_value, Err(DiscountError::NegativeValue));
}

#[test]
fn checked_constructor_and_accessors_preserve_valid_shape() {
    let discount = Discount::try_new(
        DiscountScope::Bin,
        DiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 2,
        },
        DiscountValue::MoneyPerBin(common::money("1.25", "USD")),
    )
    .unwrap();
    assert_eq!(discount.scope(), &DiscountScope::Bin);
    assert!(matches!(
        discount.threshold(),
        DiscountThreshold::BinCount { min: 2, .. }
    ));
    assert!(matches!(discount.value(), DiscountValue::MoneyPerBin(_)));
    assert_eq!(discount.validate(), Ok(()));
    assert_eq!(
        DiscountError::NegativeThreshold.to_string(),
        "discount threshold must be non-negative"
    );
    assert_eq!(
        DiscountError::NegativeValue.to_string(),
        "discount value must be non-negative"
    );
}
