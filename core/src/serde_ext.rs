#![cfg(feature = "serde")]

#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Deserializer, Serializer, de::Error as DeError};

pub mod decimal_str {
    use super::*;
    use crate::RadrootsCoreDecimal;
    use core::str::FromStr;

    pub fn serialize<S: Serializer>(
        value: &RadrootsCoreDecimal,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.normalize().to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<RadrootsCoreDecimal, D::Error> {
        let s = String::deserialize(deserializer)?;
        RadrootsCoreDecimal::from_str(&s).map_err(D::Error::custom)
    }
}
