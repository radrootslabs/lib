#[cfg(feature = "std")]
use std::error::Error as _;

use radroots_core::{Error, Unit, currency, decimal, money, percent, pricing, quantity, unit};

#[test]
fn aggregate_error_accepts_every_scoped_error() {
    let errors = [
        Error::from(currency::ParseError::InvalidFormat),
        Error::from(decimal::Error::ArithmeticOverflow),
        Error::from(pricing::DiscountError::NegativeValue),
        Error::from(money::Error::CurrencyMismatch),
        Error::from(percent::ParseError::InvalidNumber),
        Error::from(pricing::Error::PerQuantityZero),
        Error::from(quantity::Error::UnitMismatch),
        Error::from(unit::ParseError::UnknownUnit),
        Error::from(unit::ConvertError::NotConvertibleUnits {
            from: Unit::Each,
            to: Unit::MassG,
        }),
    ];

    for error in errors {
        #[cfg(feature = "std")]
        assert!(error.source().is_some());
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn aggregate_error_display_identifies_the_owning_module() {
    assert_eq!(
        Error::from(decimal::Error::DivisionByZero).to_string(),
        "decimal: decimal division by zero"
    );
    assert_eq!(
        Error::from(unit::ParseError::UnknownUnit).to_string(),
        "unit parse: unknown unit string"
    );
}
