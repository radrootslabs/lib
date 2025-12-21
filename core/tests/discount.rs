mod common;

use radroots_core::{RadrootsCoreDiscount, RadrootsCoreDiscountValue, RadrootsCorePercent};

#[test]
fn is_non_negative_validates_all_discount_shapes() {
    let money_pos = common::money("1", "USD");
    let money_neg = common::money("-1", "USD");
    let qty_pos = common::qty("1", radroots_core::RadrootsCoreUnit::Each);
    let qty_neg = common::qty("-1", radroots_core::RadrootsCoreUnit::Each);
    let pct_pos = RadrootsCorePercent::new(common::dec("10"));
    let pct_neg = RadrootsCorePercent::new(common::dec("-5"));

    let d = RadrootsCoreDiscount::QuantityThreshold {
        ref_key: None,
        threshold: qty_pos.clone(),
        value: money_pos.clone(),
    };
    assert!(d.is_non_negative());

    let d = RadrootsCoreDiscount::QuantityThreshold {
        ref_key: None,
        threshold: qty_neg,
        value: money_pos.clone(),
    };
    assert!(!d.is_non_negative());

    let d = RadrootsCoreDiscount::MassThreshold {
        threshold: qty_pos.clone(),
        value: money_neg.clone(),
    };
    assert!(!d.is_non_negative());

    let d = RadrootsCoreDiscount::SubtotalThreshold {
        threshold: money_pos.clone(),
        value: RadrootsCoreDiscountValue::Percent(pct_pos.clone()),
    };
    assert!(d.is_non_negative());

    let d = RadrootsCoreDiscount::SubtotalThreshold {
        threshold: money_pos.clone(),
        value: RadrootsCoreDiscountValue::Percent(pct_neg),
    };
    assert!(!d.is_non_negative());

    let d = RadrootsCoreDiscount::TotalThreshold {
        total_min: money_pos,
        value: pct_pos,
    };
    assert!(d.is_non_negative());
}
