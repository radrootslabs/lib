use core::fmt;
use core::str::FromStr;

use crate::Decimal;
use crate::money::Money;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Percent {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_ext::decimal_str"))]
    #[cfg_attr(feature = "dto-bindgen", dto(as = "string"))]
    pub value: Decimal,
}

impl Percent {
    #[inline]
    pub fn new(value: Decimal) -> Self {
        Self { value }
    }

    #[inline]
    pub fn from_ratio(ratio_0_to_1: Decimal) -> Self {
        Self {
            value: ratio_0_to_1 * Decimal::from(100u32),
        }
    }

    #[inline]
    pub fn to_ratio(&self) -> Decimal {
        self.value / Decimal::from(100u32)
    }

    #[inline]
    pub fn of_money(&self, base: &Money) -> Money {
        base.mul_decimal(self.to_ratio())
    }

    #[inline]
    pub fn of_money_quantized(&self, base: &Money) -> Money {
        base.mul_decimal(self.to_ratio()).quantize_to_currency()
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.value.normalize())
    }
}

impl FromStr for Percent {
    type Err = RadrootsCorePercentParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_end();
        let no_pct = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
        let dec = no_pct
            .parse::<Decimal>()
            .map_err(|_| RadrootsCorePercentParseError::InvalidNumber)?;
        Ok(Percent::new(dec))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCorePercentParseError {
    InvalidNumber,
}

impl fmt::Display for RadrootsCorePercentParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsCorePercentParseError::InvalidNumber => write!(f, "invalid percent string"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCorePercentParseError {}

#[deprecated(since = "0.1.0", note = "renamed to `Percent`")]
pub use self::Percent as RadrootsCorePercent;
