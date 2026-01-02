mod common;

use radroots_core::{
    RadrootsCoreDiscount, RadrootsCoreDiscountScope, RadrootsCoreDiscountThreshold,
    RadrootsCoreDiscountValue, RadrootsCorePercent, RadrootsCoreUnit,
};

#[test]
fn is_non_negative_validates_discount_shapes() {
    let money_pos = common::money("1", "USD");
    let money_neg = common::money("-1", "USD");
    let qty_pos = common::qty("1", RadrootsCoreUnit::Each);
    let qty_neg = common::qty("-1", RadrootsCoreUnit::Each);
    let pct_pos = RadrootsCorePercent::new(common::dec("10"));
    let pct_neg = RadrootsCorePercent::new(common::dec("-5"));

    let d = RadrootsCoreDiscount {
        scope: RadrootsCoreDiscountScope::Bin,
        threshold: RadrootsCoreDiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 2,
        },
        value: RadrootsCoreDiscountValue::MoneyPerBin(money_pos.clone()),
    };
    assert!(d.is_non_negative());

    let d = RadrootsCoreDiscount {
        scope: RadrootsCoreDiscountScope::Bin,
        threshold: RadrootsCoreDiscountThreshold::BinCount {
            bin_id: "bin-1".to_string(),
            min: 2,
        },
        value: RadrootsCoreDiscountValue::MoneyPerBin(money_neg),
    };
    assert!(!d.is_non_negative());

    let d = RadrootsCoreDiscount {
        scope: RadrootsCoreDiscountScope::OrderTotal,
        threshold: RadrootsCoreDiscountThreshold::OrderQuantity { min: qty_pos.clone() },
        value: RadrootsCoreDiscountValue::Percent(pct_pos.clone()),
    };
    assert!(d.is_non_negative());

    let d = RadrootsCoreDiscount {
        scope: RadrootsCoreDiscountScope::OrderTotal,
        threshold: RadrootsCoreDiscountThreshold::OrderQuantity { min: qty_neg },
        value: RadrootsCoreDiscountValue::Percent(pct_pos),
    };
    assert!(!d.is_non_negative());

    let d = RadrootsCoreDiscount {
        scope: RadrootsCoreDiscountScope::OrderTotal,
        threshold: RadrootsCoreDiscountThreshold::OrderQuantity { min: qty_pos },
        value: RadrootsCoreDiscountValue::Percent(pct_neg),
    };
    assert!(!d.is_non_negative());
}
