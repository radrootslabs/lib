#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod amount;

pub use amount::{AmountError, Decimal, PositiveDecimal};
