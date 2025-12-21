use core::fmt;

use crate::RadrootsCoreDecimal;
use crate::unit::RadrootsCoreUnit;

#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCoreQuantity {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_ext::decimal_str"))]
    pub amount: RadrootsCoreDecimal,
    pub unit: RadrootsCoreUnit,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub label: Option<String>,
}

impl RadrootsCoreQuantity {
    #[inline]
    pub fn new(amount: RadrootsCoreDecimal, unit: RadrootsCoreUnit) -> Self {
        Self {
            amount,
            unit,
            label: None,
        }
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
    pub fn zero(unit: RadrootsCoreUnit) -> Self {
        Self {
            amount: RadrootsCoreDecimal::ZERO,
            unit,
            label: None,
        }
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    #[inline]
    pub fn ensure_non_negative(&self) -> Result<(), RadrootsCoreQuantityInvariantError> {
        if self.amount.is_sign_negative() {
            return Err(RadrootsCoreQuantityInvariantError::NegativeAmount);
        }
        Ok(())
    }

    #[inline]
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.amount.rescale(scale);
        self
    }

    #[inline]
    pub fn try_add(
        &self,
        rhs: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreQuantity, RadrootsCoreQuantityInvariantError> {
        if self.unit != rhs.unit {
            return Err(RadrootsCoreQuantityInvariantError::UnitMismatch);
        }
        Ok(RadrootsCoreQuantity {
            amount: self.amount + rhs.amount,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    #[inline]
    pub fn try_sub(
        &self,
        rhs: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreQuantity, RadrootsCoreQuantityInvariantError> {
        if self.unit != rhs.unit {
            return Err(RadrootsCoreQuantityInvariantError::UnitMismatch);
        }
        Ok(RadrootsCoreQuantity {
            amount: self.amount - rhs.amount,
            unit: self.unit,
            label: self.label.clone(),
        })
    }

    pub fn checked_add(&self, rhs: &RadrootsCoreQuantity) -> Option<RadrootsCoreQuantity> {
        if self.unit == rhs.unit {
            Some(RadrootsCoreQuantity {
                amount: self.amount + rhs.amount,
                unit: self.unit,
                label: self.label.clone(),
            })
        } else {
            None
        }
    }

    pub fn checked_sub(&self, rhs: &RadrootsCoreQuantity) -> Option<RadrootsCoreQuantity> {
        if self.unit == rhs.unit {
            Some(RadrootsCoreQuantity {
                amount: self.amount - rhs.amount,
                unit: self.unit,
                label: self.label.clone(),
            })
        } else {
            None
        }
    }

    #[inline]
    pub fn mul_decimal(&self, factor: RadrootsCoreDecimal) -> RadrootsCoreQuantity {
        RadrootsCoreQuantity {
            amount: self.amount * factor,
            unit: self.unit,
            label: self.label.clone(),
        }
    }

    #[inline]
    pub fn div_decimal(&self, divisor: RadrootsCoreDecimal) -> RadrootsCoreQuantity {
        RadrootsCoreQuantity {
            amount: self.amount / divisor,
            unit: self.unit,
            label: self.label.clone(),
        }
    }
}

impl fmt::Display for RadrootsCoreQuantity {
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
pub enum RadrootsCoreQuantityInvariantError {
    NegativeAmount,
    UnitMismatch,
}

impl fmt::Display for RadrootsCoreQuantityInvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsCoreQuantityInvariantError::NegativeAmount => {
                write!(f, "quantity amount must be ≥ 0")
            }
            RadrootsCoreQuantityInvariantError::UnitMismatch => {
                write!(f, "quantity unit mismatch")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCoreQuantityInvariantError {}

use core::ops::{Div, Mul};

impl Mul<RadrootsCoreDecimal> for RadrootsCoreQuantity {
    type Output = RadrootsCoreQuantity;
    fn mul(self, rhs: RadrootsCoreDecimal) -> RadrootsCoreQuantity {
        RadrootsCoreQuantity {
            amount: self.amount * rhs,
            unit: self.unit,
            label: self.label,
        }
    }
}

impl Div<RadrootsCoreDecimal> for RadrootsCoreQuantity {
    type Output = RadrootsCoreQuantity;
    fn div(self, rhs: RadrootsCoreDecimal) -> RadrootsCoreQuantity {
        RadrootsCoreQuantity {
            amount: self.amount / rhs,
            unit: self.unit,
            label: self.label,
        }
    }
}
