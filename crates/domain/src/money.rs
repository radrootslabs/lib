use crate::{Currency, CurrencyError};
use core::{fmt, str::FromStr};

/// Exact bounded nonnegative minor amount; JSON uses a canonical integer string.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MinorAmount(u64);

/// Validated money with one supported currency and its fixed registry exponent.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Money {
    currency: Currency,
    minor: MinorAmount,
}

/// Input-free monetary admission or checked-arithmetic failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MoneyError {
    /// The input is not an unsigned ASCII integer string.
    InvalidMinorFormat,
    /// A leading zero makes the spelling noncanonical.
    NonCanonicalMinor,
    /// The supported fifteen-digit minor amount bound is exceeded.
    MinorOutOfRange,
    /// The supplied exponent exceeds the supported range 0-4.
    InvalidExponent,
    /// The supplied exponent disagrees with the supported currency registry.
    ExponentMismatch,
    /// Arithmetic operands have different currencies.
    CurrencyMismatch,
    /// Subtraction would produce a negative monetary value.
    NegativeResult,
    /// The currency could not be admitted.
    Currency(CurrencyError),
}

impl MinorAmount {
    /// Largest supported exact minor amount.
    pub const MAX_VALUE: u64 = 999_999_999_999_999;
    /// Explicit valid zero; it does not imply a price mode or currency.
    pub const ZERO: Self = Self(0);
    /// Largest supported validated amount.
    pub const MAX: Self = Self(Self::MAX_VALUE);

    /// Checks the exact minor-unit bound without a floating conversion.
    pub const fn new(value: u64) -> Result<Self, MoneyError> {
        if value > Self::MAX_VALUE {
            Err(MoneyError::MinorOutOfRange)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the exact bounded integer representation.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for MinorAmount {
    type Error = MoneyError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl FromStr for MinorAmount {
    type Err = MoneyError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(MoneyError::InvalidMinorFormat);
        }
        if value.len() > 15 {
            return Err(MoneyError::MinorOutOfRange);
        }
        if !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(MoneyError::InvalidMinorFormat);
        }
        if value.len() > 1 && value.starts_with('0') {
            return Err(MoneyError::NonCanonicalMinor);
        }
        let mut result = 0_u64;
        for byte in value.bytes() {
            result = result
                .checked_mul(10)
                .and_then(|n| n.checked_add(u64::from(byte - b'0')))
                .ok_or(MoneyError::MinorOutOfRange)?;
        }
        Self::new(result)
    }
}

impl fmt::Display for MinorAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for MinorAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MinorAmount(<redacted>)")
    }
}

impl Money {
    /// Combines already validated values. Zero is explicit money, not a quote.
    pub const fn new(currency: Currency, minor: MinorAmount) -> Self {
        Self { currency, minor }
    }

    /// Admits wire/FFI parts and checks their explicit registry exponent.
    pub fn from_parts(currency: &str, minor: &str, exponent: u8) -> Result<Self, MoneyError> {
        let currency = Currency::from_code(currency)?;
        let minor = minor.parse()?;
        Self::from_wire(currency, minor, exponent)
    }

    fn from_wire(currency: Currency, minor: MinorAmount, exponent: u8) -> Result<Self, MoneyError> {
        if exponent > 4 {
            return Err(MoneyError::InvalidExponent);
        }
        if exponent != currency.exponent() {
            return Err(MoneyError::ExponentMismatch);
        }
        Ok(Self::new(currency, minor))
    }

    /// Returns the validated supported currency.
    pub const fn currency(self) -> Currency {
        self.currency
    }

    /// Returns the exact validated minor amount.
    pub const fn minor(self) -> MinorAmount {
        self.minor
    }

    /// Returns the currency's fixed minor exponent.
    pub const fn exponent(self) -> u8 {
        self.currency.exponent()
    }

    /// Adds in checked signed 128-bit arithmetic, retaining currency and bounds.
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        let result = i128::from(self.minor.get())
            .checked_add(i128::from(other.minor.get()))
            .ok_or(MoneyError::MinorOutOfRange)?;
        self.with_result(result)
    }

    /// Subtracts exactly; different currencies and negative values are errors.
    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch);
        }
        let result = i128::from(self.minor.get())
            .checked_sub(i128::from(other.minor.get()))
            .filter(|result| *result >= 0)
            .ok_or(MoneyError::NegativeResult)?;
        self.with_result(result)
    }

    fn with_result(self, value: i128) -> Result<Self, MoneyError> {
        let value = u64::try_from(value).map_err(|_| MoneyError::MinorOutOfRange)?;
        Ok(Self::new(self.currency, MinorAmount::new(value)?))
    }
}

impl From<CurrencyError> for MoneyError {
    fn from(value: CurrencyError) -> Self {
        Self::Currency(value)
    }
}

impl fmt::Debug for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Money(<redacted>)")
    }
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidMinorFormat => "invalid minor amount format",
            Self::NonCanonicalMinor => "noncanonical minor amount",
            Self::MinorOutOfRange => "minor amount exceeds supported bound",
            Self::InvalidExponent => "invalid currency exponent",
            Self::ExponentMismatch => "currency exponent mismatch",
            Self::CurrencyMismatch => "currency mismatch",
            Self::NegativeResult => "monetary subtraction would be negative",
            Self::Currency(error) => return error.fmt(f),
        })
    }
}

impl core::error::Error for MoneyError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Currency(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::{MinorAmount, Money};
    use crate::Currency;
    use core::fmt;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};

    impl Serialize for MinorAmount {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_str(self)
        }
    }

    impl<'de> Deserialize<'de> for MinorAmount {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl de::Visitor<'_> for Visitor {
                type Value = MinorAmount;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a bounded canonical minor amount string")
                }
                fn visit_str<E: de::Error>(self, value: &str) -> Result<MinorAmount, E> {
                    value.parse().map_err(E::custom)
                }
            }
            deserializer.deserialize_str(Visitor)
        }
    }

    impl Serialize for Money {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut state = serializer.serialize_struct("Money", 3)?;
            state.serialize_field("currency", &self.currency())?;
            state.serialize_field("minor", &self.minor())?;
            state.serialize_field("exponent", &self.exponent())?;
            state.end()
        }
    }

    impl<'de> Deserialize<'de> for Money {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                currency: Currency,
                minor: MinorAmount,
                exponent: u8,
            }
            struct Visitor;
            impl<'de> de::Visitor<'de> for Visitor {
                type Value = Money;

                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a money object with currency, minor and exponent")
                }

                fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Money, A::Error> {
                    let wire = Wire::deserialize(de::value::MapAccessDeserializer::new(map))?;
                    Money::from_wire(wire.currency, wire.minor, wire.exponent)
                        .map_err(de::Error::custom)
                }
            }
            deserializer.deserialize_map(Visitor)
        }
    }
}
