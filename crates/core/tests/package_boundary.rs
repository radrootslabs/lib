use radroots_core::unit::UnitDimension;
#[allow(unused_imports)]
use radroots_core::{
    Currency, Decimal, Money, Percent, Quantity, QuantityPrice, Unit, currency as _, decimal as _,
    money as _, percent as _, pricing as _, quantity as _, unit as _,
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
    let _ = UnitDimension::Count;
}

#[allow(deprecated)]
#[test]
fn temporary_legacy_aliases_remain_source_compatible() {
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCorePercent,
        RadrootsCoreQuantity, RadrootsCoreQuantityPrice, RadrootsCoreUnit,
        RadrootsCoreUnitDimension,
    };

    let decimal = RadrootsCoreDecimal(rust_decimal::Decimal::ONE);
    let currency = RadrootsCoreCurrency::USD;
    let money = RadrootsCoreMoney::new(decimal, currency);
    let quantity = RadrootsCoreQuantity::new(decimal, RadrootsCoreUnit::Each);
    let _ = RadrootsCorePercent::new(decimal);
    let _ = RadrootsCoreQuantityPrice::new(money, quantity);
    let _ = RadrootsCoreUnitDimension::Count;
}
