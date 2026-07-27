#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod currency;
pub mod decimal;
mod discount;
#[cfg(test)]
mod dto;
mod error;
pub mod money;
pub mod percent;
pub mod pricing;
pub mod quantity;
mod quantity_price;
#[cfg(feature = "serde")]
pub mod serde_ext;
pub mod unit;

pub use currency::Currency;
pub use decimal::Decimal;
pub use error::Error;
pub use money::Money;
pub use percent::Percent;
pub use pricing::QuantityPrice;
pub use quantity::Quantity;
pub use unit::Unit;

#[allow(deprecated)]
pub use currency::RadrootsCoreCurrencyParseError;
#[allow(deprecated)]
pub use discount::{
    RadrootsCoreDiscount, RadrootsCoreDiscountScope, RadrootsCoreDiscountThreshold,
    RadrootsCoreDiscountValue,
};
#[allow(deprecated)]
pub use money::RadrootsCoreMoneyInvariantError;
#[allow(deprecated)]
pub use percent::RadrootsCorePercentParseError;
#[allow(deprecated)]
pub use quantity::RadrootsCoreQuantityInvariantError;
#[allow(deprecated)]
pub use quantity_price::{RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps};
#[allow(deprecated)]
pub use unit::{RadrootsCoreUnitConvertError, RadrootsCoreUnitParseError};

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
