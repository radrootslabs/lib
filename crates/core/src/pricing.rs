//! Deterministic quantity pricing, discounts, and checked cost operations.
//!
//! Import [`QuantityPriceOps`] to calculate costs. Its `try_*` methods reject
//! invalid prices, negative quantities, unit mismatches, and arithmetic
//! overflow without producing a partial result.

pub use crate::discount::{
    Discount, DiscountScope, DiscountThreshold, DiscountValue, Error as DiscountError,
};
pub use crate::quantity_price::{Error, QuantityPrice, QuantityPriceOps};
