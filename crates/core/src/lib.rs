//! Portable, deterministic value types for Radroots domain packages.
//!
//! Checked constructors and arithmetic preserve value invariants at trust
//! boundaries:
//!
//! ```
//! use radroots_core::{Currency, Decimal, Money};
//!
//! let price = Money::try_new("12.50".parse::<Decimal>()?, Currency::USD)?;
//! let total = price.checked_mul_decimal(Decimal::from(2_u32))?;
//! assert_eq!(total.amount().to_string(), "25");
//! # Ok::<(), radroots_core::Error>(())
//! ```
//!
//! Implementation-only serialization and code-generation modules are not
//! public API:
//!
//! ```compile_fail
//! use radroots_core::serde_ext;
//! ```
//!
//! ```compile_fail
//! use radroots_core::dto;
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(all(test, not(feature = "std")))]
extern crate std;

pub mod currency;
pub mod decimal;
mod discount;
#[cfg(all(test, feature = "std"))]
mod dto;
mod error;
pub mod money;
pub mod percent;
pub mod pricing;
pub mod quantity;
mod quantity_price;
#[cfg(feature = "serde")]
mod serde_ext;
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
