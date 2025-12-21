#![allow(dead_code)]

use core::str::FromStr;

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCorePercent,
    RadrootsCoreQuantity, RadrootsCoreUnit,
};

pub fn dec(s: &str) -> RadrootsCoreDecimal {
    RadrootsCoreDecimal::from_str(s).expect("valid decimal")
}

pub fn currency(code: &str) -> RadrootsCoreCurrency {
    RadrootsCoreCurrency::from_str(code).expect("valid currency")
}

pub fn money(amount: &str, code: &str) -> RadrootsCoreMoney {
    RadrootsCoreMoney::new(dec(amount), currency(code))
}

pub fn qty(amount: &str, unit: RadrootsCoreUnit) -> RadrootsCoreQuantity {
    RadrootsCoreQuantity::new(dec(amount), unit)
}

pub fn percent(s: &str) -> RadrootsCorePercent {
    RadrootsCorePercent::from_str(s).expect("valid percent")
}
