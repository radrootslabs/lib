//! Non-negative monetary values and currency-aware quantization.

use core::fmt;
use rust_decimal::Decimal as BackendDecimal;
use rust_decimal::prelude::ToPrimitive;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Money {
    amount: crate::Decimal,
    currency: crate::Currency,
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
    pub(crate) fn ensure_non_negative(&self) -> Result<(), Error> {
        if self.amount.is_sign_negative() && !self.amount.is_zero() {
            return Err(Error::NegativeAmount);
        }
        Ok(())
    }

    #[inline]
    pub fn quantize_to_currency(self) -> Self {
        let mut value = self;
        let e = value.currency.minor_unit_exponent();
        value.amount = crate::Decimal::from_backend(
            value
                .amount
                .into_backend()
                .round_dp_with_strategy(e, rust_decimal::RoundingStrategy::MidpointAwayFromZero),
        );
        value
    }

    /// Changes the amount scale without rounding or changing its value.
    #[inline]
    pub fn try_with_scale_exact(mut self, scale: u32) -> Result<Self, Error> {
        self.amount.try_rescale_exact(scale)?;
        Ok(self)
    }

    #[inline]
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, Error> {
        if self.currency != rhs.currency {
            return Err(Error::CurrencyMismatch);
        }
        Self::try_new(self.amount.checked_add(rhs.amount)?, self.currency)
    }

    #[inline]
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, Error> {
        if self.currency != rhs.currency {
            return Err(Error::CurrencyMismatch);
        }
        Self::try_new(self.amount.checked_sub(rhs.amount)?, self.currency)
    }

    #[inline]
    pub fn checked_mul_decimal(&self, factor: crate::Decimal) -> Result<Self, Error> {
        Self::try_new(self.amount.checked_mul(factor)?, self.currency)
    }

    #[inline]
    pub fn checked_div_decimal(&self, divisor: crate::Decimal) -> Result<Self, Error> {
        Self::try_new(self.amount.checked_div(divisor)?, self.currency)
    }

    #[inline]
    pub fn from_minor_units_u64(amount_minor: u64, currency: crate::Currency) -> Self {
        let e = currency.minor_unit_exponent();
        let major = BackendDecimal::from_i128_with_scale(amount_minor as i128, e);
        Self {
            amount: crate::Decimal::from_backend(major),
            currency,
        }
    }

    #[inline]
    pub fn from_minor_units_u32(amount_minor: u32, currency: crate::Currency) -> Self {
        Self::from_minor_units_u64(amount_minor as u64, currency)
    }

    #[inline]
    fn pow10(e: u32) -> BackendDecimal {
        match e {
            0 => BackendDecimal::ONE,
            1 => BackendDecimal::from(10u32),
            2 => BackendDecimal::from(100u32),
            3 => BackendDecimal::from(1_000u32),
            _ => {
                let p = 10u128.pow(e.min(38));
                BackendDecimal::from(p)
            }
        }
    }

    #[inline]
    pub fn to_minor_units_u64_exact(&self) -> Result<u64, Error> {
        let e = self.currency.minor_unit_exponent();
        let as_minor = self
            .amount
            .checked_mul(crate::Decimal::from_backend(Self::pow10(e)))
            .map_err(|_| Error::AmountOverflow)?
            .into_backend();

        if !as_minor.fract().is_zero() {
            return Err(Error::NotWholeMinorUnits);
        }
        as_minor.to_u64().ok_or(Error::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u64_rounded(&self) -> Result<u64, Error> {
        let e = self.currency.minor_unit_exponent();
        let scaled = self
            .amount
            .into_backend()
            .round_dp_with_strategy(e, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
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
    pub fn to_minor_units_u32_rounded(&self) -> Result<u32, Error> {
        let v = self.to_minor_units_u64_rounded()?;
        u32::try_from(v).map_err(|_| Error::AmountOverflow)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Money {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Wire {
            amount: crate::Decimal,
            currency: crate::Currency,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.amount, wire.currency).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow10_internal_paths_cover_fallback_branches() {
        assert_eq!(Money::pow10(0), BackendDecimal::ONE);
        assert_eq!(Money::pow10(1), BackendDecimal::from(10u32));
        assert_eq!(Money::pow10(2), BackendDecimal::from(100u32));
        assert_eq!(Money::pow10(3), BackendDecimal::from(1_000u32));
        assert_eq!(Money::pow10(6), BackendDecimal::from(1_000_000u32));
    }
}
