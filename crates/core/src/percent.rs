use core::fmt;
use core::str::FromStr;

use crate::Decimal;
use crate::money::Money;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Percent {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_ext::decimal_str"))]
    #[cfg_attr(all(test, feature = "std"), dto(as = "string"))]
    pub value: Decimal,
}

impl Percent {
    /// Creates a percentage value.
    ///
    /// `Percent` is intentionally signed and unbounded because it is also
    /// used for deltas. Domain-specific non-negative rules belong to values
    /// such as [`crate::pricing::Discount`].
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
    pub fn try_from_ratio(ratio_0_to_1: Decimal) -> Result<Self, crate::decimal::Error> {
        Ok(Self {
            value: ratio_0_to_1.checked_mul(Decimal::from(100u32))?,
        })
    }

    #[inline]
    pub fn to_ratio(&self) -> Decimal {
        self.value / Decimal::from(100u32)
    }

    #[inline]
    pub fn try_to_ratio(&self) -> Result<Decimal, crate::decimal::Error> {
        self.value.checked_div(Decimal::from(100u32))
    }

    #[inline]
    pub fn of_money(&self, base: &Money) -> Money {
        base.mul_decimal(self.to_ratio())
    }

    #[inline]
    pub fn of_money_quantized(&self, base: &Money) -> Money {
        base.mul_decimal(self.to_ratio()).quantize_to_currency()
    }

    #[inline]
    pub fn try_of_money(&self, base: &Money) -> Result<Money, crate::money::Error> {
        base.checked_mul_decimal(self.try_to_ratio().map_err(crate::money::Error::from)?)
    }

    #[inline]
    pub fn try_of_money_quantized(&self, base: &Money) -> Result<Money, crate::money::Error> {
        Ok(self.try_of_money(base)?.quantize_to_currency())
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.value.normalize())
    }
}

impl FromStr for Percent {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_end();
        let no_pct = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
        let dec = no_pct
            .parse::<Decimal>()
            .map_err(|_| ParseError::InvalidNumber)?;
        Ok(Percent::new(dec))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidNumber,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidNumber => write!(f, "invalid percent string"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

#[deprecated(since = "0.1.0", note = "renamed to `Percent`")]
pub use self::Percent as RadrootsCorePercent;

#[deprecated(since = "0.1.0", note = "renamed to `percent::ParseError`")]
pub use self::ParseError as RadrootsCorePercentParseError;
