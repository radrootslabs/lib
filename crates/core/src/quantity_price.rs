use core::fmt;

use crate::{Decimal, Money, Quantity, Unit};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityPrice {
    amount: Money,
    quantity: Quantity,
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
/// Downstream crates may call this trait but cannot implement it:
///
/// ```compile_fail
/// use radroots_core::{Money, Quantity};
/// use radroots_core::pricing::{Error, QuantityPriceOps};
///
/// struct ForeignPrice;
///
/// impl QuantityPriceOps for ForeignPrice {
///     fn try_cost_for(&self, _: &Quantity) -> Result<Money, Error> { panic!() }
///     fn try_cost_for_rounded(&self, _: &Quantity) -> Result<Money, Error> { panic!() }
/// }
/// ```
pub trait QuantityPriceOps: sealed::Sealed {
    /// Calculates the requested cost without silently converting invalid
    /// quantities, unit mismatches, or arithmetic failures into zero.
    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, Error>;

    /// Calculates first, then rounds the final result to the currency exponent
    /// with deterministic midpoint-away-from-zero rounding.
    fn try_cost_for_rounded(&self, qty: &Quantity) -> Result<Money, Error>;
}

impl QuantityPrice {
    #[inline]
    pub fn try_new(amount: Money, quantity: Quantity) -> Result<Self, Error> {
        amount
            .ensure_non_negative()
            .map_err(|_| Error::NegativePrice)?;
        if quantity.amount().is_sign_negative() {
            return Err(Error::PerQuantityNegative);
        }
        if quantity.amount().is_zero() {
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

        let target = self.quantity.unit();

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
        self.try_cost_for_amount_in(qty.amount(), qty.unit())
    }

    #[inline]
    pub fn is_price_per_canonical_unit(&self) -> bool {
        self.quantity.unit() == self.quantity.unit().canonical_unit()
            && self.quantity.amount() == Decimal::ONE
    }

    #[inline]
    pub fn try_to_unit_price(&self, unit: Unit) -> Result<QuantityPrice, Error> {
        use crate::unit::convert_unit_decimal;

        self.validate()?;

        let normalized = if self.quantity.unit() == unit {
            self.quantity.amount()
        } else {
            convert_unit_decimal(self.quantity.amount(), self.quantity.unit(), unit)
                .map_err(|error| map_conversion_error(error, self.quantity.unit(), unit))?
        };

        if normalized.is_zero() {
            return Err(Error::PerQuantityZero);
        }

        let amount = self
            .amount
            .checked_div_decimal(normalized)
            .map_err(|_| Error::ArithmeticOverflow)?;
        let quantity =
            Quantity::try_new(Decimal::ONE, unit).map_err(|_| Error::ArithmeticOverflow)?;
        Self::try_new(amount, quantity)
    }

    #[inline]
    pub fn try_to_canonical_unit_price(&self) -> Result<QuantityPrice, Error> {
        self.try_to_unit_price(self.quantity.unit().canonical_unit())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for QuantityPrice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Wire {
            amount: Money,
            quantity: Quantity,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.amount, wire.quantity).map_err(serde::de::Error::custom)
    }
}

impl QuantityPriceOps for QuantityPrice {
    #[inline]
    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, Error> {
        self.validate()?;
        qty.ensure_non_negative()
            .map_err(|_| Error::NegativeRequestedQuantity)?;
        if qty.unit() != self.quantity.unit() {
            return Err(Error::UnitMismatch {
                have: qty.unit(),
                want: self.quantity.unit(),
            });
        }
        let ratio = qty
            .amount()
            .checked_div(self.quantity.amount())
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
