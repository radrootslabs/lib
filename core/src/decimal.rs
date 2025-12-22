use core::fmt;
use core::ops::{Add, Div, Mul, Sub};
use core::str::FromStr;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString};
#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RadrootsCoreDecimal(pub Decimal);

impl RadrootsCoreDecimal {
    pub const ZERO: Self = Self(Decimal::ZERO);
    pub const ONE: Self = Self(Decimal::ONE);

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
    pub fn normalize(&self) -> Decimal {
        self.0.normalize()
    }

    #[inline]
    pub fn scale(&self) -> u32 {
        self.0.scale()
    }

    #[inline]
    pub fn from_str_exact(s: &str) -> Result<Self, rust_decimal::Error> {
        Decimal::from_str_exact(s).map(Self)
    }

    #[inline]
    pub fn from_f64_display(n: f64) -> Result<Self, rust_decimal::Error> {
        let s = format!("{:.17}", n);
        Decimal::from_str(&s).map(Self)
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
impl Serialize for RadrootsCoreDecimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.normalize().to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RadrootsCoreDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Decimal::from_str(&s)
            .map(RadrootsCoreDecimal)
            .map_err(D::Error::custom)
    }
}

impl fmt::Display for RadrootsCoreDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalize().to_string())
    }
}

impl From<Decimal> for RadrootsCoreDecimal {
    fn from(d: Decimal) -> Self {
        Self(d)
    }
}
impl From<RadrootsCoreDecimal> for Decimal {
    fn from(d: RadrootsCoreDecimal) -> Self {
        d.0
    }
}
impl From<u32> for RadrootsCoreDecimal {
    fn from(v: u32) -> Self {
        Self(Decimal::from(v))
    }
}
impl From<i32> for RadrootsCoreDecimal {
    fn from(v: i32) -> Self {
        Self(Decimal::from(v))
    }
}
impl From<u64> for RadrootsCoreDecimal {
    fn from(v: u64) -> Self {
        Self(Decimal::from(v))
    }
}
impl From<i64> for RadrootsCoreDecimal {
    fn from(v: i64) -> Self {
        Self(Decimal::from(v))
    }
}

impl Add for RadrootsCoreDecimal {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl Sub for RadrootsCoreDecimal {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl Mul for RadrootsCoreDecimal {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}
impl Div for RadrootsCoreDecimal {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl FromStr for RadrootsCoreDecimal {
    type Err = rust_decimal::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Decimal::from_str(s).map(RadrootsCoreDecimal)
    }
}
