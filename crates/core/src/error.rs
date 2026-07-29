use core::fmt;

/// Crate-wide error for callers that prefer one Radroots-owned error type.
///
/// Focused module APIs retain their narrower errors; each converts into this
/// aggregate without exposing errors owned by implementation dependencies.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Currency(crate::currency::ParseError),
    Decimal(crate::decimal::Error),
    Discount(crate::pricing::DiscountError),
    Money(crate::money::Error),
    Percent(crate::percent::ParseError),
    Pricing(crate::pricing::Error),
    Quantity(crate::quantity::Error),
    UnitParse(crate::unit::ParseError),
    UnitConversion(crate::unit::ConvertError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Currency(error) => write!(f, "currency: {error}"),
            Self::Decimal(error) => write!(f, "decimal: {error}"),
            Self::Discount(error) => write!(f, "discount: {error}"),
            Self::Money(error) => write!(f, "money: {error}"),
            Self::Percent(error) => write!(f, "percent: {error}"),
            Self::Pricing(error) => write!(f, "pricing: {error}"),
            Self::Quantity(error) => write!(f, "quantity: {error}"),
            Self::UnitParse(error) => write!(f, "unit parse: {error}"),
            Self::UnitConversion(error) => write!(f, "unit conversion: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Currency(error) => Some(error),
            Self::Decimal(error) => Some(error),
            Self::Discount(error) => Some(error),
            Self::Money(error) => Some(error),
            Self::Percent(error) => Some(error),
            Self::Pricing(error) => Some(error),
            Self::Quantity(error) => Some(error),
            Self::UnitParse(error) => Some(error),
            Self::UnitConversion(error) => Some(error),
        }
    }
}

impl From<crate::currency::ParseError> for Error {
    fn from(error: crate::currency::ParseError) -> Self {
        Self::Currency(error)
    }
}

impl From<crate::decimal::Error> for Error {
    fn from(error: crate::decimal::Error) -> Self {
        Self::Decimal(error)
    }
}

impl From<crate::discount::Error> for Error {
    fn from(error: crate::discount::Error) -> Self {
        Self::Discount(error)
    }
}

impl From<crate::money::Error> for Error {
    fn from(error: crate::money::Error) -> Self {
        Self::Money(error)
    }
}

impl From<crate::percent::ParseError> for Error {
    fn from(error: crate::percent::ParseError) -> Self {
        Self::Percent(error)
    }
}

impl From<crate::quantity_price::Error> for Error {
    fn from(error: crate::quantity_price::Error) -> Self {
        Self::Pricing(error)
    }
}

impl From<crate::quantity::Error> for Error {
    fn from(error: crate::quantity::Error) -> Self {
        Self::Quantity(error)
    }
}

impl From<crate::unit::ParseError> for Error {
    fn from(error: crate::unit::ParseError) -> Self {
        Self::UnitParse(error)
    }
}

impl From<crate::unit::ConvertError> for Error {
    fn from(error: crate::unit::ConvertError) -> Self {
        Self::UnitConversion(error)
    }
}
