#![doc = include_str!("../README.md")]
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
