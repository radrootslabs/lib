use core::fmt;
use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCoreMoney {
    pub amount: crate::RadrootsCoreDecimal,
    pub currency: crate::RadrootsCoreCurrency,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreMoneyInvariantError {
    NegativeAmount,
    NotWholeMinorUnits,
    AmountOverflow,
    CurrencyMismatch,
}

impl fmt::Display for RadrootsCoreMoneyInvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeAmount => write!(f, "money amount must be ≥ 0"),
            Self::NotWholeMinorUnits => write!(f, "money not a whole number of minor units"),
            Self::AmountOverflow => write!(f, "money minor-unit conversion overflow"),
            Self::CurrencyMismatch => write!(f, "money currency mismatch"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCoreMoneyInvariantError {}

impl RadrootsCoreMoney {
    #[inline]
    pub fn new(amount: crate::RadrootsCoreDecimal, currency: crate::RadrootsCoreCurrency) -> Self {
        Self { amount, currency }
    }

    #[inline]
    pub fn zero(currency: crate::RadrootsCoreCurrency) -> Self {
        Self {
            amount: crate::RadrootsCoreDecimal::ZERO,
            currency,
        }
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    #[inline]
    pub fn ensure_non_negative(&self) -> Result<(), RadrootsCoreMoneyInvariantError> {
        if self.amount.is_sign_negative() {
            return Err(RadrootsCoreMoneyInvariantError::NegativeAmount);
        }
        Ok(())
    }

    #[inline]
    pub fn quantize_to_currency(mut self) -> Self {
        let e = self.currency.minor_unit_exponent();
        self.amount.0 = self
            .amount
            .0
            .round_dp_with_strategy(e, RoundingStrategy::MidpointAwayFromZero);
        self
    }

    #[inline]
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.amount.rescale(scale);
        self
    }

    #[inline]
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, RadrootsCoreMoneyInvariantError> {
        if self.currency != rhs.currency {
            return Err(RadrootsCoreMoneyInvariantError::CurrencyMismatch);
        }
        Ok(Self::new(self.amount + rhs.amount, self.currency))
    }

    #[inline]
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, RadrootsCoreMoneyInvariantError> {
        if self.currency != rhs.currency {
            return Err(RadrootsCoreMoneyInvariantError::CurrencyMismatch);
        }
        Ok(Self::new(self.amount - rhs.amount, self.currency))
    }

    #[inline]
    pub fn mul_decimal(&self, factor: crate::RadrootsCoreDecimal) -> Self {
        Self::new(self.amount * factor, self.currency)
    }

    #[inline]
    pub fn div_decimal(&self, divisor: crate::RadrootsCoreDecimal) -> Self {
        Self::new(self.amount / divisor, self.currency)
    }

    #[inline]
    pub fn from_minor_units_u64(amount_minor: u64, currency: crate::RadrootsCoreCurrency) -> Self {
        let e = currency.minor_unit_exponent();
        let major = Decimal::from_i128_with_scale(amount_minor as i128, e);
        Self::new(crate::RadrootsCoreDecimal(major), currency)
    }

    #[inline]
    pub fn from_minor_units_u32(amount_minor: u32, currency: crate::RadrootsCoreCurrency) -> Self {
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
    pub fn to_minor_units_u64_exact(&self) -> Result<u64, RadrootsCoreMoneyInvariantError> {
        let e = self.currency.minor_unit_exponent();
        let as_minor = self.amount.0 * Self::pow10(e);

        if !as_minor.fract().is_zero() {
            return Err(RadrootsCoreMoneyInvariantError::NotWholeMinorUnits);
        }
        as_minor
            .to_u64()
            .ok_or(RadrootsCoreMoneyInvariantError::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u64_rounded(
        &self,
        strategy: RoundingStrategy,
    ) -> Result<u64, RadrootsCoreMoneyInvariantError> {
        let e = self.currency.minor_unit_exponent();
        let scaled = self.amount.0.round_dp_with_strategy(e, strategy);
        let as_minor = scaled * Self::pow10(e);
        if !as_minor.fract().is_zero() {
            return Err(RadrootsCoreMoneyInvariantError::NotWholeMinorUnits);
        }
        as_minor
            .to_u64()
            .ok_or(RadrootsCoreMoneyInvariantError::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u32_exact(&self) -> Result<u32, RadrootsCoreMoneyInvariantError> {
        let v = self.to_minor_units_u64_exact()?;
        u32::try_from(v).map_err(|_| RadrootsCoreMoneyInvariantError::AmountOverflow)
    }

    #[inline]
    pub fn to_minor_units_u32_rounded(
        &self,
        strategy: RoundingStrategy,
    ) -> Result<u32, RadrootsCoreMoneyInvariantError> {
        let v = self.to_minor_units_u64_rounded(strategy)?;
        u32::try_from(v).map_err(|_| RadrootsCoreMoneyInvariantError::AmountOverflow)
    }
}

impl fmt::Display for RadrootsCoreMoney {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

use core::ops::{Div, Mul};

impl Mul<crate::RadrootsCoreDecimal> for RadrootsCoreMoney {
    type Output = Self;
    fn mul(self, rhs: crate::RadrootsCoreDecimal) -> Self {
        self.mul_decimal(rhs)
    }
}

impl Div<crate::RadrootsCoreDecimal> for RadrootsCoreMoney {
    type Output = Self;
    fn div(self, rhs: crate::RadrootsCoreDecimal) -> Self {
        self.div_decimal(rhs)
    }
}
