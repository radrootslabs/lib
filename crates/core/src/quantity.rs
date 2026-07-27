use core::fmt;

use crate::Decimal;
use crate::unit::{ConvertError, Unit, convert_unit_decimal};

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(feature = "std")]
use std::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quantity {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_ext::decimal_str"))]
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub amount: Decimal,
    pub unit: Unit,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

impl Quantity {
    /// Compatibility constructor for already-validated internal values.
    ///
    /// New boundary code should use [`Self::try_new`]. The unchecked shape is
    /// retained only until the scheduled first-party consumer migration.
    #[inline]
    pub fn new(amount: Decimal, unit: Unit) -> Self {
        Self {
            amount,
            unit,
            label: None,
        }
    }

    #[inline]
    pub fn try_new(amount: Decimal, unit: Unit) -> Result<Self, Error> {
        if amount.is_sign_negative() && !amount.is_zero() {
            return Err(Error::NegativeAmount);
        }
        Ok(Self {
            amount: if amount.is_zero() {
                Decimal::ZERO
            } else {
                amount
            },
            unit,
            label: None,
        })
    }

    #[inline]
    pub const fn amount(&self) -> Decimal {
        self.amount
    }

    #[inline]
    pub const fn unit(&self) -> Unit {
        self.unit
    }

    #[inline]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[inline]
    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = Some(label.into());
        self
    }

    #[inline]
    pub fn with_optional_label<S: Into<String>>(mut self, label: Option<S>) -> Self {
        self.label = label.map(|s| s.into());
        self
    }

    #[inline]
    pub fn clear_label(mut self) -> Self {
        self.label = None;
        self
    }

    #[inline]
    pub fn zero(unit: Unit) -> Self {
        Self {
            amount: Decimal::ZERO,
            unit,
            label: None,
        }
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    #[inline]
    pub fn is_canonical(&self) -> bool {
        self.unit == self.unit.canonical_unit()
    }

    #[inline]
    pub fn canonical_unit(&self) -> Unit {
        self.unit.canonical_unit()
    }

    #[inline]
    pub fn try_convert_to(&self, unit: Unit) -> Result<Quantity, ConvertError> {
        if self.unit == unit {
            return Ok(self.clone());
        }
        let amount = convert_unit_decimal(self.amount, self.unit, unit)?;
        Ok(Quantity {
            amount,
            unit,
            label: self.label.clone(),
        })
    }

    #[inline]
    pub fn to_canonical(&self) -> Result<Quantity, ConvertError> {
        self.try_convert_to(self.unit.canonical_unit())
    }

    #[inline]
    pub fn ensure_non_negative(&self) -> Result<(), Error> {
        if self.amount.is_sign_negative() && !self.amount.is_zero() {
            return Err(Error::NegativeAmount);
        }
        Ok(())
    }

    #[inline]
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.amount.rescale(scale);
        self
    }

    #[inline]
    pub fn try_add(&self, rhs: &Quantity) -> Result<Quantity, Error> {
        self.ensure_non_negative()?;
        rhs.ensure_non_negative()?;
        if self.unit != rhs.unit {
            return Err(Error::UnitMismatch);
        }
        Ok(Quantity {
            amount: self.amount.checked_add(rhs.amount)?,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    #[inline]
    pub fn try_sub(&self, rhs: &Quantity) -> Result<Quantity, Error> {
        self.ensure_non_negative()?;
        rhs.ensure_non_negative()?;
        if self.unit != rhs.unit {
            return Err(Error::UnitMismatch);
        }
        let amount = self.amount.checked_sub(rhs.amount)?;
        if amount.is_sign_negative() {
            return Err(Error::NegativeAmount);
        }
        Ok(Quantity {
            amount,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    pub fn checked_add(&self, rhs: &Quantity) -> Option<Quantity> {
        self.try_add(rhs).ok()
    }

    pub fn checked_sub(&self, rhs: &Quantity) -> Option<Quantity> {
        self.try_sub(rhs).ok()
    }

    #[inline]
    pub fn checked_mul_decimal(&self, factor: Decimal) -> Result<Quantity, Error> {
        self.ensure_non_negative()?;
        let amount = self.amount.checked_mul(factor)?;
        if amount.is_sign_negative() {
            return Err(Error::NegativeAmount);
        }
        Ok(Quantity {
            amount,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    #[inline]
    pub fn checked_div_decimal(&self, divisor: Decimal) -> Result<Quantity, Error> {
        self.ensure_non_negative()?;
        let amount = self.amount.checked_div(divisor)?;
        if amount.is_sign_negative() {
            return Err(Error::NegativeAmount);
        }
        Ok(Quantity {
            amount,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    #[inline]
    pub fn mul_decimal(&self, factor: Decimal) -> Quantity {
        Quantity {
            amount: self.amount * factor,
            unit: self.unit,
            label: self.label.clone(),
        }
    }

    #[inline]
    pub fn div_decimal(&self, divisor: Decimal) -> Quantity {
        Quantity {
            amount: self.amount / divisor,
            unit: self.unit,
            label: self.label.clone(),
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount.normalize(), self.unit)?;
        if let Some(label) = &self.label {
            write!(f, " ({label})")?;
        }
        Ok(())
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NegativeAmount,
    UnitMismatch,
    ArithmeticOverflow,
    DivisionByZero,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NegativeAmount => {
                write!(f, "quantity amount must be ≥ 0")
            }
            Error::UnitMismatch => {
                write!(f, "quantity unit mismatch")
            }
            Error::ArithmeticOverflow => write!(f, "quantity arithmetic overflow"),
            Error::DivisionByZero => write!(f, "quantity division by zero"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<crate::decimal::Error> for Error {
    fn from(error: crate::decimal::Error) -> Self {
        match error {
            crate::decimal::Error::DivisionByZero => Self::DivisionByZero,
            _ => Self::ArithmeticOverflow,
        }
    }
}

use core::ops::{Div, Mul};

impl Mul<Decimal> for Quantity {
    type Output = Quantity;
    fn mul(self, rhs: Decimal) -> Quantity {
        Quantity {
            amount: self.amount * rhs,
            unit: self.unit,
            label: self.label,
        }
    }
}

impl Div<Decimal> for Quantity {
    type Output = Quantity;
    fn div(self, rhs: Decimal) -> Quantity {
        Quantity {
            amount: self.amount / rhs,
            unit: self.unit,
            label: self.label,
        }
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `Quantity`")]
pub use self::Quantity as RadrootsCoreQuantity;

#[deprecated(since = "0.1.0", note = "renamed to `quantity::Error`")]
pub use self::Error as RadrootsCoreQuantityInvariantError;
