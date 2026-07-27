//! Non-negative monetary values and currency-aware quantization.

use core::fmt;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Money {
    pub amount: crate::Decimal,
    pub currency: crate::Currency,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NegativeAmount,
    NotWholeMinorUnits,
    AmountOverflow,
    CurrencyMismatch,
    ArithmeticOverflow,
    DivisionByZero,
    ScaleOutOfRange,
    PrecisionLoss,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeAmount => write!(f, "money amount must be ≥ 0"),
            Self::NotWholeMinorUnits => write!(f, "money not a whole number of minor units"),
            Self::AmountOverflow => write!(f, "money minor-unit conversion overflow"),
            Self::CurrencyMismatch => write!(f, "money currency mismatch"),
            Self::ArithmeticOverflow => write!(f, "money arithmetic overflow"),
            Self::DivisionByZero => write!(f, "money division by zero"),
            Self::ScaleOutOfRange => write!(f, "money scale is outside the supported range"),
            Self::PrecisionLoss => write!(f, "money operation would lose precision"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<crate::decimal::Error> for Error {
    fn from(error: crate::decimal::Error) -> Self {
        match error {
            crate::decimal::Error::DivisionByZero => Self::DivisionByZero,
            crate::decimal::Error::ScaleOutOfRange => Self::ScaleOutOfRange,
            crate::decimal::Error::PrecisionLoss => Self::PrecisionLoss,
            _ => Self::ArithmeticOverflow,
        }
    }
}

impl Money {
    /// Compatibility constructor for already-validated internal values.
    ///
    /// New boundary code should use [`Self::try_new`]. The unchecked shape is
    /// retained only until the scheduled first-party consumer migration.
    #[inline]
    pub fn new(amount: crate::Decimal, currency: crate::Currency) -> Self {
        Self { amount, currency }
    }

    #[inline]
    pub fn try_new(amount: crate::Decimal, currency: crate::Currency) -> Result<Self, Error> {
        if amount.is_sign_negative() && !amount.is_zero() {
            return Err(Error::NegativeAmount);
        }
        Ok(Self {
            amount: if amount.is_zero() {
                crate::Decimal::ZERO
            } else {
                amount
            },
            currency,
        })
    }

    #[inline]
    pub const fn amount(&self) -> crate::Decimal {
        self.amount
    }

    #[inline]
    pub const fn currency(&self) -> crate::Currency {
        self.currency
    }

    #[inline]
    pub fn zero(currency: crate::Currency) -> Self {
        Self {
            amount: crate::Decimal::ZERO,
            currency,
        }
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    #[inline]
    pub fn ensure_non_negative(&self) -> Result<(), Error> {
        if self.amount.is_sign_negative() && !self.amount.is_zero() {
            return Err(Error::NegativeAmount);
        }
        Ok(())
    }

    #[inline]
    pub fn quantize_to_currency(self) -> Self {
        self.quantize_to_currency_with_strategy(RoundingStrategy::MidpointAwayFromZero)
    }

    /// Rounds to the currency's minor-unit exponent with the supplied
    /// strategy. [`Self::quantize_to_currency`] uses
    /// [`RoundingStrategy::MidpointAwayFromZero`].
    #[inline]
    pub fn quantize_to_currency_with_strategy(mut self, strategy: RoundingStrategy) -> Self {
        let e = self.currency.minor_unit_exponent();
        self.amount.0 = self.amount.0.round_dp_with_strategy(e, strategy);
        self
    }

    #[inline]
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.amount.rescale(scale);
        self
    }

    /// Changes the amount scale without rounding or changing its value.
    #[inline]
    pub fn try_with_scale_exact(mut self, scale: u32) -> Result<Self, Error> {
        self.amount.try_rescale_exact(scale)?;
        Ok(self)
    }

    #[inline]
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, Error> {
        self.ensure_non_negative()?;
        rhs.ensure_non_negative()?;
        if self.currency != rhs.currency {
            return Err(Error::CurrencyMismatch);
        }
        Self::try_new(self.amount.checked_add(rhs.amount)?, self.currency)
    }

    #[inline]
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, Error> {
        self.ensure_non_negative()?;
        rhs.ensure_non_negative()?;
        if self.currency != rhs.currency {
            return Err(Error::CurrencyMismatch);
        }
        Self::try_new(self.amount.checked_sub(rhs.amount)?, self.currency)
    }

    #[inline]
    pub fn checked_mul_decimal(&self, factor: crate::Decimal) -> Result<Self, Error> {
        self.ensure_non_negative()?;
        Self::try_new(self.amount.checked_mul(factor)?, self.currency)
    }

    #[inline]
    pub fn checked_div_decimal(&self, divisor: crate::Decimal) -> Result<Self, Error> {
        self.ensure_non_negative()?;
        Self::try_new(self.amount.checked_div(divisor)?, self.currency)
    }

    #[inline]
    pub fn mul_decimal(&self, factor: crate::Decimal) -> Self {
        Self::new(self.amount * factor, self.currency)
    }

    #[inline]
    pub fn div_decimal(&self, divisor: crate::Decimal) -> Self {
        Self::new(self.amount / divisor, self.currency)
    }

    #[inline]
    pub fn from_minor_units_u64(amount_minor: u64, currency: crate::Currency) -> Self {
        let e = currency.minor_unit_exponent();
        let major = Decimal::from_i128_with_scale(amount_minor as i128, e);
        Self::new(crate::Decimal(major), currency)
    }

    #[inline]
    pub fn from_minor_units_u32(amount_minor: u32, currency: crate::Currency) -> Self {
        Self::from_minor_units_u64(amount_minor as u64, currency)
    }

    #[inline]
    fn pow10(e: u32) -> Decimal {
        match e {
            0 => Decimal::ONE,
            1 => Decimal::from(10u32),
            2 => Decimal::from(100u32),
            3 => Decimal::from(1_000u32),
            _ => {
                let p = 10u128.pow(e.min(38));
                Decimal::from(p)
            }
        }
    }

    #[inline]
    pub fn to_minor_units_u64_exact(&self) -> Result<u64, Error> {
        self.ensure_non_negative()?;
        let e = self.currency.minor_unit_exponent();
        let as_minor = self
            .amount
            .checked_mul(crate::Decimal(Self::pow10(e)))
            .map_err(|_| Error::AmountOverflow)?
            .0;

        if !as_minor.fract().is_zero() {
            return Err(Error::NotWholeMinorUnits);
        }
        as_minor.to_u64().ok_or(Error::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u64_rounded(&self, strategy: RoundingStrategy) -> Result<u64, Error> {
        self.ensure_non_negative()?;
        let e = self.currency.minor_unit_exponent();
        let scaled = self.amount.0.round_dp_with_strategy(e, strategy);
        let as_minor = scaled
            .checked_mul(Self::pow10(e))
            .ok_or(Error::AmountOverflow)?;
        debug_assert!(as_minor.fract().is_zero());
        as_minor.to_u64().ok_or(Error::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u32_exact(&self) -> Result<u32, Error> {
        let v = self.to_minor_units_u64_exact()?;
        u32::try_from(v).map_err(|_| Error::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u32_rounded(&self, strategy: RoundingStrategy) -> Result<u32, Error> {
        let v = self.to_minor_units_u64_rounded(strategy)?;
        u32::try_from(v).map_err(|_| Error::AmountOverflow)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

use core::ops::{Div, Mul};

impl Mul<crate::Decimal> for Money {
    type Output = Self;
    fn mul(self, rhs: crate::Decimal) -> Self {
        self.mul_decimal(rhs)
    }
}

impl Div<crate::Decimal> for Money {
    type Output = Self;
    fn div(self, rhs: crate::Decimal) -> Self {
        self.div_decimal(rhs)
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `Money`")]
pub use self::Money as RadrootsCoreMoney;

#[deprecated(since = "0.1.0", note = "renamed to `money::Error`")]
pub use self::Error as RadrootsCoreMoneyInvariantError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow10_internal_paths_cover_fallback_branches() {
        assert_eq!(Money::pow10(0), Decimal::ONE);
        assert_eq!(Money::pow10(1), Decimal::from(10u32));
        assert_eq!(Money::pow10(2), Decimal::from(100u32));
        assert_eq!(Money::pow10(3), Decimal::from(1_000u32));
        assert_eq!(Money::pow10(6), Decimal::from(1_000_000u32));
    }
}
