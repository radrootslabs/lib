#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod currency;
pub mod decimal;
pub mod discount;
#[cfg(feature = "dto-bindgen")]
pub mod dto;
pub mod money;
pub mod percent;
pub mod pricing;
pub mod quantity;
pub mod quantity_price;
#[cfg(feature = "serde")]
pub mod serde_ext;
pub mod unit;

pub use currency::{Currency, RadrootsCoreCurrencyParseError};
pub use decimal::Decimal;
pub use discount::{
    RadrootsCoreDiscount, RadrootsCoreDiscountScope, RadrootsCoreDiscountThreshold,
    RadrootsCoreDiscountValue,
};
pub use money::{Money, RadrootsCoreMoneyInvariantError};
pub use percent::{Percent, RadrootsCorePercentParseError};
pub use quantity::{Quantity, RadrootsCoreQuantityInvariantError};
pub use quantity_price::{
    QuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps,
};
pub use unit::{
    RadrootsCoreUnitConvertError, RadrootsCoreUnitParseError, Unit, convert_mass_decimal,
    convert_unit_decimal, convert_volume_decimal, parse_mass_unit, parse_volume_unit,
};

#[allow(deprecated)]
pub use currency::RadrootsCoreCurrency;
#[allow(deprecated)]
pub use decimal::RadrootsCoreDecimal;
#[allow(deprecated)]
pub use money::RadrootsCoreMoney;
#[allow(deprecated)]
pub use percent::RadrootsCorePercent;
#[allow(deprecated)]
pub use quantity::RadrootsCoreQuantity;
#[allow(deprecated)]
pub use quantity_price::RadrootsCoreQuantityPrice;
#[allow(deprecated)]
pub use unit::{RadrootsCoreUnit, RadrootsCoreUnitDimension};
