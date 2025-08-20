#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod currency;
pub mod decimal;
pub mod discount;
pub mod money;
pub mod percent;
pub mod quantity;
pub mod quantity_price;
#[cfg(feature = "serde")]
pub mod serde_ext;
pub mod unit;

pub use currency::{RadrootsCoreCurrency, RadrootsCoreCurrencyParseError};
pub use decimal::RadrootsCoreDecimal;
pub use discount::{RadrootsCoreDiscount, RadrootsCoreDiscountValue};
pub use money::{RadrootsCoreMoney, RadrootsCoreMoneyInvariantError};
pub use percent::{RadrootsCorePercent, RadrootsCorePercentParseError};
pub use quantity::{RadrootsCoreQuantity, RadrootsCoreQuantityInvariantError};
pub use quantity_price::{RadrootsCoreQuantityPrice, RadrootsCoreQuantityPriceOps};
pub use unit::{RadrootsCoreUnit, RadrootsCoreUnitParseError};
