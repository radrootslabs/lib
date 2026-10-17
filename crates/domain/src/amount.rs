use core::{cmp::Ordering, fmt, str::FromStr};

/// A validated, canonical, nonnegative decimal with no floating representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decimal {
    coefficient: u64,
    scale: u8,
}

/// A validated decimal strictly greater than zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PositiveDecimal(Decimal);

/// A bounded value admission or exact-arithmetic failure; never contains input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AmountError {
    /// The input is not an unsigned ASCII decimal string.
    InvalidFormat,
    /// The spelling is not the canonical representation of its value.
    NonCanonical,
    /// The coefficient exceeds the supported twelve-digit bound.
    CoefficientOutOfRange,
    /// The scale exceeds six fractional digits.
    ScaleOutOfRange,
    /// A strictly positive amount was required.
    MustBePositive,
    /// Subtraction would produce a negative value.
    NegativeResult,
}

impl fmt::Display for AmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFormat => "invalid decimal format",
            Self::NonCanonical => "noncanonical decimal representation",
            Self::CoefficientOutOfRange => "decimal coefficient exceeds supported bound",
            Self::ScaleOutOfRange => "decimal scale exceeds supported bound",
            Self::MustBePositive => "decimal must be positive",
            Self::NegativeResult => "decimal subtraction would be negative",
        })
    }
}

impl core::error::Error for AmountError {}

impl Decimal {
    /// Largest supported canonical coefficient.
    pub const MAX_COEFFICIENT: u64 = 999_999_999_999;
    /// Largest supported scale.
    pub const MAX_SCALE: u8 = 6;
    /// Maximum syntax length, checked before examining untrusted string contents.
    pub const MAX_ENCODED_LEN: usize = 19;
    /// Canonical zero.
    pub const ZERO: Self = Self {
        coefficient: 0,
        scale: 0,
    };
    /// Canonical one.
    pub const ONE: Self = Self {
        coefficient: 1,
        scale: 0,
    };
    /// Largest supported value.
    pub const MAX: Self = Self {
        coefficient: Self::MAX_COEFFICIENT,
        scale: 0,
    };

    /// Validates bounded input parts, then removes fractional trailing zeros.
    ///
    /// Both input bounds apply before normalization. Zero always has scale zero.
    pub const fn from_parts(coefficient: u64, scale: u8) -> Result<Self, AmountError> {
        if scale > Self::MAX_SCALE {
            return Err(AmountError::ScaleOutOfRange);
        }
        if coefficient > Self::MAX_COEFFICIENT {
            return Err(AmountError::CoefficientOutOfRange);
        }
        Ok(Self::normalized(coefficient, scale))
    }

    const fn normalized(mut coefficient: u64, mut scale: u8) -> Self {
        if coefficient == 0 {
            return Self::ZERO;
        }
        while scale > 0 && coefficient.is_multiple_of(10) {
            coefficient /= 10;
            scale -= 1;
        }
        Self { coefficient, scale }
    }

    /// The canonical coefficient, without an implicit unit or currency.
    pub const fn coefficient(self) -> u64 {
        self.coefficient
    }
    /// The canonical number of fractional decimal digits.
    pub const fn scale(self) -> u8 {
        self.scale
    }
    /// Whether this amount is exactly zero.
    pub const fn is_zero(self) -> bool {
        self.coefficient == 0
    }

    // Validated coefficients times at most 10^6 fit in u64, including the
    // addition of two such aligned values. These helpers never round.
    fn aligned(self, scale: u8) -> u64 {
        self.coefficient * 10_u64.pow(u32::from(scale - self.scale))
    }

    fn from_result(coefficient: u64, scale: u8) -> Result<Self, AmountError> {
        let value = Self::normalized(coefficient, scale);
        Self::from_parts(value.coefficient, value.scale)
    }

    /// Adds exactly, rejecting a result outside the canonical coefficient bound.
    pub fn checked_add(self, rhs: Self) -> Result<Self, AmountError> {
        let scale = self.scale.max(rhs.scale);
        let coefficient = self
            .aligned(scale)
            .checked_add(rhs.aligned(scale))
            .ok_or(AmountError::CoefficientOutOfRange)?;
        Self::from_result(coefficient, scale)
    }

    /// Subtracts exactly; negative or unrepresentable results are errors.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, AmountError> {
        let scale = self.scale.max(rhs.scale);
        let coefficient = self
            .aligned(scale)
            .checked_sub(rhs.aligned(scale))
            .ok_or(AmountError::NegativeResult)?;
        Self::from_result(coefficient, scale)
    }
}

impl Ord for Decimal {
    fn cmp(&self, rhs: &Self) -> Ordering {
        let scale = self.scale.max(rhs.scale);
        self.aligned(scale).cmp(&rhs.aligned(scale))
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl FromStr for Decimal {
    type Err = AmountError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() || input.len() > Self::MAX_ENCODED_LEN {
            return Err(AmountError::InvalidFormat);
        }
        let (whole, fraction) = match input.split_once('.') {
            Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
            Some(_) => return Err(AmountError::InvalidFormat),
            None => (input, ""),
        };
        if whole.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(AmountError::InvalidFormat);
        }
        if (whole.len() > 1 && whole.starts_with('0')) || fraction.ends_with('0') {
            return Err(AmountError::NonCanonical);
        }
        if fraction.len() > usize::from(Self::MAX_SCALE) {
            return Err(AmountError::ScaleOutOfRange);
        }
        let mut coefficient = 0_u64;
        for digit in whole.bytes().chain(fraction.bytes()) {
            coefficient = coefficient
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(digit - b'0')))
                .filter(|v| *v <= Self::MAX_COEFFICIENT)
                .ok_or(AmountError::CoefficientOutOfRange)?;
        }
        // The length bound above proves conversion fits in u8.
        Self::from_parts(coefficient, fraction.len() as u8)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.coefficient);
        }
        let divisor = 10_u64.pow(u32::from(self.scale));
        write!(
            f,
            "{}.{:0width$}",
            self.coefficient / divisor,
            self.coefficient % divisor,
            width = usize::from(self.scale)
        )
    }
}

impl PositiveDecimal {
    /// Requires an already validated nonzero value.
    pub const fn new(value: Decimal) -> Result<Self, AmountError> {
        if value.is_zero() {
            Err(AmountError::MustBePositive)
        } else {
            Ok(Self(value))
        }
    }
    /// Returns the validated nonnegative representation without losing precision.
    pub const fn amount(self) -> Decimal {
        self.0
    }
}

impl TryFrom<Decimal> for PositiveDecimal {
    type Error = AmountError;
    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PositiveDecimal> for Decimal {
    fn from(value: PositiveDecimal) -> Self {
        value.amount()
    }
}

impl FromStr for PositiveDecimal {
    type Err = AmountError;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input.parse()?)
    }
}

impl fmt::Display for PositiveDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::{Decimal, PositiveDecimal};
    use core::fmt;
    use serde::{
        Deserialize, Deserializer, Serialize, Serializer,
        de::{self, Visitor},
    };

    impl Serialize for Decimal {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_str(self)
        }
    }

    impl<'de> Deserialize<'de> for Decimal {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct DecimalVisitor;
            impl Visitor<'_> for DecimalVisitor {
                type Value = Decimal;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a canonical Radroots decimal string")
                }
                fn visit_str<E: de::Error>(self, value: &str) -> Result<Decimal, E> {
                    value.parse().map_err(E::custom)
                }
            }
            deserializer.deserialize_str(DecimalVisitor)
        }
    }

    impl Serialize for PositiveDecimal {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.amount().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for PositiveDecimal {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            Self::new(Decimal::deserialize(deserializer)?).map_err(de::Error::custom)
        }
    }
}
