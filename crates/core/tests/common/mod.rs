#![allow(dead_code)]

use core::str::FromStr;

use radroots_core::{Currency, Decimal, Money, Percent, Quantity, Unit};

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal")
}

pub fn currency(code: &str) -> Currency {
    Currency::from_str(code).expect("valid currency")
}

pub fn money(amount: &str, code: &str) -> Money {
    Money::new(dec(amount), currency(code))
}

pub fn qty(amount: &str, unit: Unit) -> Quantity {
    Quantity::new(dec(amount), unit)
}

pub fn percent(s: &str) -> Percent {
    Percent::from_str(s).expect("valid percent")
}
