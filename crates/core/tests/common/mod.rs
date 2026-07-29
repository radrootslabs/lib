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
    Money::try_new(dec(amount), currency(code)).expect("valid money")
}

pub fn qty(amount: &str, unit: Unit) -> Quantity {
    Quantity::try_new(dec(amount), unit).expect("valid quantity")
}

pub fn percent(s: &str) -> Percent {
    Percent::from_str(s).expect("valid percent")
}
