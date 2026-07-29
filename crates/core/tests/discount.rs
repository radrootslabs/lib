mod common;

use radroots_core::{
    Percent,
    pricing::{Discount, DiscountError, DiscountScope, DiscountThreshold, DiscountValue},
};

#[test]
fn checked_constructor_rejects_negative_percent_values() {
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
    assert!(discount.is_non_negative());
}

#[test]
fn discount_error_messages_are_stable() {
    assert_eq!(
        DiscountError::NegativeThreshold.to_string(),
        "discount threshold must be non-negative"
    );
    assert_eq!(
        DiscountError::NegativeValue.to_string(),
        "discount value must be non-negative"
    );
}
