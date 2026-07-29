#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(feature = "std")]
use std::string::String;

use serde::{Deserialize, Deserializer, Serializer, de::Error as DeError};

pub mod decimal_str {
    use super::*;
    use crate::Decimal;
    use core::str::FromStr;

    pub fn serialize<S: Serializer>(value: &Decimal, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.normalize().to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Decimal, D::Error> {
        let s = String::deserialize(deserializer)?;
        Decimal::from_str(&s).map_err(D::Error::custom)
    }
}
