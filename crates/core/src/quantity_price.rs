use core::fmt;

use crate::{Decimal, Money, Quantity, Unit};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(dto_bindgen::Dto))]
#[cfg_attr(test, dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityPrice {
    pub amount: Money,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    PerQuantityZero,
    PerQuantityNegative,
    NegativePrice,
    NegativeRequestedQuantity,
    UnitMismatch { have: Unit, want: Unit },
    NonConvertibleUnits { from: Unit, to: Unit },
    ArithmeticOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PerQuantityZero => f.write_str("price quantity must be greater than zero"),
            Self::PerQuantityNegative => f.write_str("price quantity must not be negative"),
            Self::NegativePrice => f.write_str("price amount must not be negative"),
            Self::NegativeRequestedQuantity => {
                f.write_str("requested quantity must not be negative")
            }
            Self::UnitMismatch { have, want } => {
                write!(f, "price quantity unit mismatch: have {have}, want {want}")
            }
            Self::NonConvertibleUnits { from, to } => {
                write!(f, "price units are not convertible: {from} -> {to}")
            }
            Self::ArithmeticOverflow => f.write_str("price arithmetic overflow"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

fn map_conversion_error(error: crate::unit::ConvertError, from: Unit, to: Unit) -> Error {
    match error {
        crate::unit::ConvertError::ArithmeticOverflow { .. } => Error::ArithmeticOverflow,
        _ => Error::NonConvertibleUnits { from, to },
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for super::QuantityPrice {}
}

/// Sealed pricing operations implemented by [`QuantityPrice`].
///
/// The `try_*` methods are the canonical checked surface. The infallible
/// methods remain temporarily for first-party source compatibility and return
/// zero when the corresponding checked operation fails.
pub trait QuantityPriceOps: sealed::Sealed {
    /// Compatibility operation that returns zero on invalid input.
    #[must_use]
    fn cost_for(&self, qty: &Quantity) -> Money;

    /// Calculates first, then rounds the final result to the currency exponent
    /// with midpoint-away-from-zero rounding.
    #[must_use]
    fn cost_for_rounded(&self, qty: &Quantity) -> Money;

    /// Rounds the price first, then calculates the requested cost without a
    /// second quantization.
    #[must_use]
    fn cost_for_with_quantized_price(&self, qty: &Quantity) -> Money;

    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, Error>;

    fn try_cost_for_rounded(&self, qty: &Quantity) -> Result<Money, Error>;
}

impl QuantityPrice {
    /// Compatibility constructor for already-validated internal values.
    ///
    /// New boundary code should use [`Self::try_new`].
    #[inline]
    pub fn new(amount: Money, quantity: Quantity) -> Self {
        Self { amount, quantity }
    }

    #[inline]
    pub fn try_new(amount: Money, quantity: Quantity) -> Result<Self, Error> {
        amount
            .ensure_non_negative()
            .map_err(|_| Error::NegativePrice)?;
        if quantity.amount.is_sign_negative() {
            return Err(Error::PerQuantityNegative);
        }
        if quantity.amount.is_zero() {
            return Err(Error::PerQuantityZero);
        }
        Ok(Self { amount, quantity })
    }

    #[inline]
    pub fn amount(&self) -> &Money {
        &self.amount
    }

    #[inline]
    pub fn quantity(&self) -> &Quantity {
        &self.quantity
    }

    #[inline]
    pub fn validate(&self) -> Result<(), Error> {
        Self::try_new(self.amount.clone(), self.quantity.clone()).map(|_| ())
    }

    #[inline]
    pub fn try_cost_for_amount_in(&self, amount: Decimal, unit: Unit) -> Result<Money, Error> {
        use crate::unit::convert_unit_decimal;

        let target = self.quantity.unit;

        let normalized = if unit == target {
            amount
        } else {
            convert_unit_decimal(amount, unit, target)
                .map_err(|error| map_conversion_error(error, unit, target))?
        };

        let qty =
            Quantity::try_new(normalized, target).map_err(|_| Error::NegativeRequestedQuantity)?;
        self.try_cost_for_rounded(&qty)
    }

    #[inline]
    pub fn try_cost_for_quantity_in(&self, qty: &Quantity) -> Result<Money, Error> {
        self.try_cost_for_amount_in(qty.amount, qty.unit)
    }

    #[inline]
    pub fn is_price_per_canonical_unit(&self) -> bool {
        self.quantity.unit == self.quantity.unit.canonical_unit()
            && self.quantity.amount == Decimal::ONE
    }

    #[inline]
    pub fn try_to_unit_price(&self, unit: Unit) -> Result<QuantityPrice, Error> {
        use crate::unit::convert_unit_decimal;

        self.validate()?;

        let normalized = if self.quantity.unit == unit {
            self.quantity.amount
        } else {
            convert_unit_decimal(self.quantity.amount, self.quantity.unit, unit)
                .map_err(|error| map_conversion_error(error, self.quantity.unit, unit))?
        };

        if normalized.is_zero() {
            return Err(Error::PerQuantityZero);
        }

        let amount = self
            .amount
            .checked_div_decimal(normalized)
            .map_err(|_| Error::ArithmeticOverflow)?;
        Self::try_new(amount, Quantity::new(Decimal::ONE, unit))
    }

    #[inline]
    pub fn try_to_canonical_unit_price(&self) -> Result<QuantityPrice, Error> {
        self.try_to_unit_price(self.quantity.unit.canonical_unit())
    }
}

impl QuantityPriceOps for QuantityPrice {
    #[inline]
    fn cost_for(&self, qty: &Quantity) -> Money {
        if qty.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if qty.unit != self.quantity.unit {
            return Money::zero(self.amount.currency);
        }

        let ratio = qty.amount / self.quantity.amount;
        self.amount.mul_decimal(ratio)
    }

    #[inline]
    fn cost_for_rounded(&self, qty: &Quantity) -> Money {
        self.cost_for(qty).quantize_to_currency()
    }

    #[inline]
    fn cost_for_with_quantized_price(&self, qty: &Quantity) -> Money {
        if qty.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if qty.unit != self.quantity.unit {
            return Money::zero(self.amount.currency);
        }
        let unit_price_q = self.amount.clone().quantize_to_currency();
        unit_price_q.mul_decimal(qty.amount / self.quantity.amount)
    }

    #[inline]
    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, Error> {
        self.validate()?;
        qty.ensure_non_negative()
            .map_err(|_| Error::NegativeRequestedQuantity)?;
        if qty.unit != self.quantity.unit {
            return Err(Error::UnitMismatch {
                have: qty.unit,
                want: self.quantity.unit,
            });
        }
        let ratio = qty
            .amount
            .checked_div(self.quantity.amount)
            .map_err(|_| Error::ArithmeticOverflow)?;
        self.amount
            .checked_mul_decimal(ratio)
            .map_err(|_| Error::ArithmeticOverflow)
    }

    #[inline]
    fn try_cost_for_rounded(&self, qty: &Quantity) -> Result<Money, Error> {
        Ok(self.try_cost_for(qty)?.quantize_to_currency())
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `QuantityPrice`")]
pub use self::QuantityPrice as RadrootsCoreQuantityPrice;

#[deprecated(since = "0.1.0", note = "renamed to `pricing::Error`")]
pub use self::Error as RadrootsCoreQuantityPriceError;

#[deprecated(since = "0.1.0", note = "renamed to `pricing::QuantityPriceOps`")]
pub use self::QuantityPriceOps as RadrootsCoreQuantityPriceOps;
