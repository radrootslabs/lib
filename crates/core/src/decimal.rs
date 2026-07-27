use core::fmt;
use core::ops::{Add, Div, Mul, Sub};
use core::str::FromStr;
use rust_decimal::Decimal as RustDecimal;
use rust_decimal::prelude::ToPrimitive;

#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Decimal(pub RustDecimal);

impl Decimal {
    pub const ZERO: Self = Self(RustDecimal::ZERO);
    pub const ONE: Self = Self(RustDecimal::ONE);

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    #[inline]
    pub fn is_sign_negative(&self) -> bool {
        self.0.is_sign_negative()
    }
    #[inline]
    pub fn rescale(&mut self, scale: u32) {
        self.0.rescale(scale);
    }
    #[inline]
    pub fn normalize(&self) -> RustDecimal {
        self.0.normalize()
    }

    #[inline]
    pub fn scale(&self) -> u32 {
        self.0.scale()
    }

    #[inline]
    pub fn from_str_exact(s: &str) -> Result<Self, rust_decimal::Error> {
        RustDecimal::from_str_exact(s).map(Self)
    }

    #[inline]
    pub fn from_f64_display(n: f64) -> Result<Self, rust_decimal::Error> {
        let s = format!("{:.17}", n);
        RustDecimal::from_str(&s).map(Self)
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
}

#[cfg(feature = "serde")]
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.normalize().to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        RustDecimal::from_str(&s)
            .map(Decimal)
            .map_err(D::Error::custom)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalize().to_string())
    }
}

impl From<RustDecimal> for Decimal {
    fn from(d: RustDecimal) -> Self {
        Self(d)
    }
}
impl From<Decimal> for RustDecimal {
    fn from(d: Decimal) -> Self {
        d.0
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

impl Add for Decimal {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl Sub for Decimal {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl Mul for Decimal {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}
impl Div for Decimal {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl FromStr for Decimal {
    type Err = rust_decimal::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RustDecimal::from_str(s).map(Decimal)
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `Decimal`")]
pub use self::Decimal as RadrootsCoreDecimal;
