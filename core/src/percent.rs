use core::fmt;
use core::str::FromStr;

use crate::RadrootsCoreDecimal;
use crate::money::RadrootsCoreMoney;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCorePercent {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_ext::decimal_str"))]
    pub value: RadrootsCoreDecimal,
}

impl RadrootsCorePercent {
    #[inline]
    pub fn new(value: RadrootsCoreDecimal) -> Self {
        Self { value }
    }

    #[inline]
    pub fn from_ratio(ratio_0_to_1: RadrootsCoreDecimal) -> Self {
        Self {
            value: ratio_0_to_1 * RadrootsCoreDecimal::from(100u32),
        }
    }

    #[inline]
    pub fn to_ratio(&self) -> RadrootsCoreDecimal {
        self.value / RadrootsCoreDecimal::from(100u32)
    }

    #[inline]
    pub fn of_money(&self, base: &RadrootsCoreMoney) -> RadrootsCoreMoney {
        base.mul_decimal(self.to_ratio())
    }

    #[inline]
    pub fn of_money_quantized(&self, base: &RadrootsCoreMoney) -> RadrootsCoreMoney {
        base.mul_decimal(self.to_ratio()).quantize_to_currency()
    }
}

impl fmt::Display for RadrootsCorePercent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.value.normalize())
    }
}

impl FromStr for RadrootsCorePercent {
    type Err = RadrootsCorePercentParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_end();
        let no_pct = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
        let dec = no_pct
            .parse::<RadrootsCoreDecimal>()
            .map_err(|_| RadrootsCorePercentParseError::InvalidNumber)?;
        Ok(RadrootsCorePercent::new(dec))
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
