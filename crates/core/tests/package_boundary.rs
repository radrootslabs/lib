use radroots_core::unit::UnitDimension;
#[allow(unused_imports)]
use radroots_core::{
    Currency, Decimal, Error, Money, Percent, Quantity, QuantityPrice, Unit, currency as _,
    decimal as _, money as _, percent as _, pricing as _, quantity as _, unit as _,
};

#[test]
fn approved_module_skeleton_is_public() {}

#[test]
fn canonical_value_paths_are_public() {
    let _ = core::mem::size_of::<Currency>();
    let _ = core::mem::size_of::<Decimal>();
    let _ = core::mem::size_of::<Money>();
    let _ = core::mem::size_of::<Percent>();
    let _ = core::mem::size_of::<Quantity>();
    let _ = core::mem::size_of::<QuantityPrice>();
    let _ = core::mem::size_of::<Unit>();
    let _ = core::mem::size_of::<Error>();
    let _ = UnitDimension::Count;
    let _ = core::mem::size_of::<radroots_core::pricing::Discount>();
    let _ = core::mem::size_of::<radroots_core::pricing::DiscountError>();
    let _ = core::mem::size_of::<radroots_core::pricing::Error>();
}

#[test]
fn canonical_values_use_checked_construction() {
    let money = Money::try_new(Decimal::ONE, Currency::USD).expect("valid money");
    let quantity = Quantity::try_new(Decimal::ONE, Unit::Each).expect("valid quantity");
    let price = QuantityPrice::try_new(money, quantity).expect("valid price");
    assert_eq!(price.quantity().unit(), Unit::Each);
}
