//! Fixed-precision decimal values, checked arithmetic, and exact conversions.

use core::fmt;
use core::str::FromStr;
use rust_decimal::Decimal as RustDecimal;
use rust_decimal::prelude::ToPrimitive;

#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Decimal(RustDecimal);

/// Errors produced while constructing or calculating with a [`Decimal`].
///
/// This type intentionally normalizes `rust_decimal` failures so dependency
/// implementation details do not become part of the Radroots public contract.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidFormat,
    OutOfRange,
    ScaleOutOfRange,
    PrecisionLoss,
    ArithmeticOverflow,
    DivisionByZero,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => f.write_str("invalid decimal string"),
            Self::OutOfRange => f.write_str("decimal is outside the supported range"),
            Self::ScaleOutOfRange => f.write_str("decimal scale is outside the supported range"),
            Self::PrecisionLoss => f.write_str("decimal operation would lose precision"),
            Self::ArithmeticOverflow => f.write_str("decimal arithmetic overflow"),
            Self::DivisionByZero => f.write_str("decimal division by zero"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

fn normalize_parse_error(error: rust_decimal::Error) -> Error {
    match error {
        rust_decimal::Error::ExceedsMaximumPossibleValue
        | rust_decimal::Error::LessThanMinimumPossibleValue
        | rust_decimal::Error::Underflow
        | rust_decimal::Error::ScaleExceedsMaximumPrecision(_)
        | rust_decimal::Error::ConversionTo(_) => Error::OutOfRange,
        rust_decimal::Error::ErrorString(_) => Error::InvalidFormat,
    }
}

#[inline]
fn canonicalize_zero(value: RustDecimal) -> RustDecimal {
    if value.is_zero() {
        RustDecimal::ZERO
    } else {
        value
    }
}

impl Decimal {
    pub const ZERO: Self = Self(RustDecimal::ZERO);
    pub const ONE: Self = Self(RustDecimal::ONE);
    pub const MAX: Self = Self(RustDecimal::MAX);
    pub const MIN: Self = Self(RustDecimal::MIN);
    pub const MAX_SCALE: u32 = RustDecimal::MAX_SCALE;

    #[inline]
    pub(crate) const fn from_parts(lo: u32, mid: u32, hi: u32, scale: u32) -> Self {
        Self(RustDecimal::from_parts(lo, mid, hi, false, scale))
    }

    #[inline]
    pub(crate) const fn from_backend(value: RustDecimal) -> Self {
        Self(value)
    }

    #[inline]
    pub(crate) const fn into_backend(self) -> RustDecimal {
        self.0
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    #[inline]
    pub fn is_sign_negative(&self) -> bool {
        self.0.is_sign_negative()
    }
    /// Rescales with deterministic midpoint-away-from-zero rounding when
    /// reducing precision. When increasing precision, the closest
    /// representable scale is used. Use [`Self::try_rescale_exact`] when
    /// either behavior would be ambiguous at a boundary.
    #[inline]
    pub fn rescale(&mut self, scale: u32) {
        self.0.rescale(scale);
    }

    /// Changes the scale only when the requested representation is exact.
    ///
    /// Unlike [`Self::rescale`], this method never rounds and never silently
    /// substitutes a smaller scale. The value is left unchanged on error.
    #[inline]
    pub fn try_rescale_exact(&mut self, scale: u32) -> Result<(), Error> {
        if scale > Self::MAX_SCALE {
            return Err(Error::ScaleOutOfRange);
        }

        let original = self.0;
        let mut candidate = original;
        candidate.rescale(scale);
        if candidate.scale() != scale {
            return Err(Error::ScaleOutOfRange);
        }
        if candidate != original {
            return Err(Error::PrecisionLoss);
        }

        self.0 = candidate;
        Ok(())
    }
    #[inline]
    pub fn normalize(&self) -> Self {
        Self(self.0.normalize())
    }

    #[inline]
    pub fn scale(&self) -> u32 {
        self.0.scale()
    }

    #[inline]
    pub fn from_str_exact(s: &str) -> Result<Self, Error> {
        RustDecimal::from_str_exact(s)
            .map(canonicalize_zero)
            .map(Self)
            .map_err(normalize_parse_error)
    }

    /// Converts the shortest deterministic 17-digit display of a finite
    /// `f64`. This is a decimal representation of the displayed float, not a
    /// promise to preserve the float's binary representation exactly.
    #[inline]
    pub fn from_f64_display(n: f64) -> Result<Self, Error> {
        if !n.is_finite() {
            return Err(Error::InvalidFormat);
        }
        let s = format!("{:.17}", n);
        s.parse()
    }
    #[inline]
    pub fn to_f64_lossy(&self) -> Option<f64> {
        self.0.to_f64()
    }

    #[inline]
    pub fn to_u64_exact(&self) -> Option<u64> {
        if self.0.fract().is_zero() {
            self.0.to_u64()
        } else {
            None
        }
    }

    #[inline]
    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        self.0
            .checked_add(rhs.0)
            .map(canonicalize_zero)
            .map(Self)
            .ok_or(Error::ArithmeticOverflow)
    }

    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Error> {
        self.0
            .checked_sub(rhs.0)
            .map(canonicalize_zero)
            .map(Self)
            .ok_or(Error::ArithmeticOverflow)
    }

    #[inline]
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Error> {
        self.0
            .checked_mul(rhs.0)
            .map(canonicalize_zero)
            .map(Self)
            .ok_or(Error::ArithmeticOverflow)
    }

    #[inline]
    pub fn checked_div(self, rhs: Self) -> Result<Self, Error> {
        if rhs.is_zero() {
            return Err(Error::DivisionByZero);
        }
        self.0
            .checked_div(rhs.0)
            .map(canonicalize_zero)
            .map(Self)
            .ok_or(Error::ArithmeticOverflow)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.normalize().to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<Decimal>().map_err(D::Error::custom)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.normalize().to_string())
    }
}

impl From<u32> for Decimal {
    fn from(v: u32) -> Self {
        Self(RustDecimal::from(v))
    }
}
impl From<i32> for Decimal {
    fn from(v: i32) -> Self {
        Self(RustDecimal::from(v))
    }
}
impl From<u64> for Decimal {
    fn from(v: u64) -> Self {
        Self(RustDecimal::from(v))
    }
}
impl From<i64> for Decimal {
    fn from(v: i64) -> Self {
        Self(RustDecimal::from(v))
    }
}

impl FromStr for Decimal {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RustDecimal::from_str(s)
            .map(canonicalize_zero)
            .map(Decimal)
            .map_err(normalize_parse_error)
    }
}
