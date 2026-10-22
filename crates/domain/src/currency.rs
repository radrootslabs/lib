use core::{fmt, str::FromStr};

/// Immutable supported code/exponent data contract. No runtime update is implied.
pub const CURRENCY_REGISTRY_ID: &str = "radroots.currency_registry.v1";
/// SHA256 of the complete attributed source data used by this registry.
pub const CURRENCY_REGISTRY_SHA256: &str =
    "60d8282b2d7ddc27929b097d4de9e76a900a64f1de4a5aa7a4cb3308b55b82d0";

const DATA: &str = include_str!("../data/currency_registry_v1.csv");

/// Number of supported currencies/funds in the fixed registry.
pub const SUPPORTED_CURRENCY_COUNT: usize = {
    let bytes = DATA.as_bytes();
    let mut position = 0;
    let mut count = 0;
    let mut previous = [0_u8; 3];
    while position < bytes.len() {
        if bytes[position] == b'#' {
            while position < bytes.len() && bytes[position] != b'\n' {
                position += 1;
            }
            assert!(position < bytes.len(), "unterminated registry attribution");
            position += 1;
            continue;
        }
        assert!(position + 6 <= bytes.len(), "truncated currency row");
        let code = [bytes[position], bytes[position + 1], bytes[position + 2]];
        assert!(
            code[0].is_ascii_uppercase()
                && code[1].is_ascii_uppercase()
                && code[2].is_ascii_uppercase(),
            "noncanonical registry code"
        );
        assert!(
            bytes[position + 3] == b','
                && bytes[position + 4] >= b'0'
                && bytes[position + 4] <= b'4'
                && bytes[position + 5] == b'\n',
            "invalid registry exponent or row"
        );
        assert!(
            count == 0
                || code[0] > previous[0]
                || (code[0] == previous[0] && code[1] > previous[1])
                || (code[0] == previous[0] && code[1] == previous[1] && code[2] > previous[2]),
            "unsorted or duplicate registry code"
        );
        previous = code;
        count += 1;
        position += 6;
    }
    assert!(count == 165, "registry v1 membership changed");
    count
};

/// A validated supported currency/fund and its exact minor-unit exponent.
///
/// This represents the fixed registry, without exchange, cash/display rounding,
/// payment or present-day legal-tender claims.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Currency {
    code: &'static str,
    exponent: u8,
}

/// Input-free currency admission failure; no fallback exponent is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CurrencyError {
    /// The code is not exactly three uppercase ASCII letters.
    InvalidFormat,
    /// The code has no supported numeric exponent in this fixed registry.
    UnsupportedCode,
}

impl Currency {
    /// Admits exactly a supported uppercase ASCII code without allocation.
    pub fn from_code(code: &str) -> Result<Self, CurrencyError> {
        if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(CurrencyError::InvalidFormat);
        }
        Self::supported()
            .find(|currency| currency.code == code)
            .ok_or(CurrencyError::UnsupportedCode)
    }

    /// Iterates the fixed supported registry in canonical code order.
    pub fn supported() -> impl Iterator<Item = Self> + Clone {
        DATA.lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                // The compile-time data check proves every non-comment row is
                // six ASCII bytes, including its newline, with an exponent 0-4.
                Self {
                    code: &line[..3],
                    exponent: line.as_bytes()[4] - b'0',
                }
            })
    }

    /// Returns the canonical supported three-letter code.
    pub const fn code(self) -> &'static str {
        self.code
    }

    /// Returns the fixed exponent; this is not a presentation-rounding rule.
    pub const fn exponent(self) -> u8 {
        self.exponent
    }
}

impl FromStr for Currency {
    type Err = CurrencyError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_code(value)
    }
}

impl fmt::Debug for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Currency(<validated>)")
    }
}

impl fmt::Display for CurrencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFormat => "invalid currency code format",
            Self::UnsupportedCode => "unsupported currency code",
        })
    }
}

impl core::error::Error for CurrencyError {}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::Currency;
    use core::fmt;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    impl Serialize for Currency {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(self.code())
        }
    }

    impl<'de> Deserialize<'de> for Currency {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl de::Visitor<'_> for Visitor {
                type Value = Currency;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a supported uppercase currency code")
                }
                fn visit_str<E: de::Error>(self, value: &str) -> Result<Currency, E> {
                    Currency::from_code(value).map_err(E::custom)
                }
            }
            deserializer.deserialize_str(Visitor)
        }
    }
}
