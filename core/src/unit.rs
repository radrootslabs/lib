use core::fmt;
use core::str::FromStr;
use rust_decimal_macros::dec;

#[cfg(feature = "serde")]
#[cfg(feature = "std")]
use std::string::String;
#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

use crate::RadrootsCoreDecimal;

#[cfg_attr(feature = "typeshare", typeshare::typeshare)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RadrootsCoreUnit {
    Each,
    MassKg,
    MassG,
    MassOz,
    MassLb,
    VolumeL,
    VolumeMl,
}

impl RadrootsCoreUnit {
    #[inline]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Each => "each",
            Self::MassKg => "kg",
            Self::MassG => "g",
            Self::MassOz => "oz",
            Self::MassLb => "lb",
            Self::VolumeL => "l",
            Self::VolumeMl => "ml",
        }
    }

    pub fn same_dimension(a: Self, b: Self) -> bool {
        use RadrootsCoreUnit::*;
        matches!(
            (a, b),
            (Each, Each)
                | (MassKg, MassKg)
                | (MassKg, MassG)
                | (MassKg, MassOz)
                | (MassKg, MassLb)
                | (MassG, MassKg)
                | (MassG, MassG)
                | (MassG, MassOz)
                | (MassG, MassLb)
                | (MassOz, MassKg)
                | (MassOz, MassG)
                | (MassOz, MassOz)
                | (MassOz, MassLb)
                | (MassLb, MassKg)
                | (MassLb, MassG)
                | (MassLb, MassOz)
                | (MassLb, MassLb)
                | (VolumeL, VolumeL)
                | (VolumeL, VolumeMl)
                | (VolumeMl, VolumeL)
                | (VolumeMl, VolumeMl)
        )
    }

    #[inline]
    pub fn is_mass(&self) -> bool {
        matches!(
            self,
            Self::MassKg | Self::MassG | Self::MassOz | Self::MassLb
        )
    }
}

impl fmt::Display for RadrootsCoreUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreUnitParseError {
    UnknownUnit,
    NotAMassUnit,
}

impl fmt::Display for RadrootsCoreUnitParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnit => write!(f, "unknown unit string"),
            Self::NotAMassUnit => write!(f, "unit is not a mass unit"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCoreUnitParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreUnitConvertError {
    NotMassUnit {
        from: RadrootsCoreUnit,
        to: RadrootsCoreUnit,
    },
}

impl fmt::Display for RadrootsCoreUnitConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsCoreUnitConvertError::NotMassUnit { from, to } => {
                write!(f, "unit conversion requires mass units: {from} -> {to}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCoreUnitConvertError {}

impl FromStr for RadrootsCoreUnit {
    type Err = RadrootsCoreUnitParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "each" | "ea" | "count" => Ok(RadrootsCoreUnit::Each),
            "kg" | "kilogram" | "kilograms" => Ok(RadrootsCoreUnit::MassKg),
            "g" | "gram" | "grams" => Ok(RadrootsCoreUnit::MassG),
            "oz" | "ounce" | "ounces" => Ok(RadrootsCoreUnit::MassOz),
            "lb" | "pound" | "pounds" => Ok(RadrootsCoreUnit::MassLb),
            "l" | "liter" | "litre" | "liters" | "litres" => Ok(RadrootsCoreUnit::VolumeL),
            "ml" | "milliliter" | "millilitre" | "milliliters" | "millilitres" => {
                Ok(RadrootsCoreUnit::VolumeMl)
            }
            _ => Err(RadrootsCoreUnitParseError::UnknownUnit),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for RadrootsCoreUnit {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.code())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RadrootsCoreUnit {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[inline]
pub fn parse_mass_unit(s: &str) -> Result<RadrootsCoreUnit, RadrootsCoreUnitParseError> {
    let u: RadrootsCoreUnit = RadrootsCoreUnit::from_str(s)?;
    if u.is_mass() {
        Ok(u)
    } else {
        Err(RadrootsCoreUnitParseError::NotAMassUnit)
    }
}

#[inline]
fn grams_factor_decimal(u: RadrootsCoreUnit) -> RadrootsCoreDecimal {
    match u {
        RadrootsCoreUnit::MassG => RadrootsCoreDecimal::ONE,
        RadrootsCoreUnit::MassKg => RadrootsCoreDecimal::from(1000u32),
        RadrootsCoreUnit::MassOz => RadrootsCoreDecimal(dec!(28.349523125)),
        RadrootsCoreUnit::MassLb => RadrootsCoreDecimal(dec!(453.59237)),
        _ => RadrootsCoreDecimal::ONE,
    }
}

#[inline]
pub fn convert_mass_decimal(
    amount: RadrootsCoreDecimal,
    from: RadrootsCoreUnit,
    to: RadrootsCoreUnit,
) -> Result<RadrootsCoreDecimal, RadrootsCoreUnitConvertError> {
    if !from.is_mass() || !to.is_mass() {
        return Err(RadrootsCoreUnitConvertError::NotMassUnit { from, to });
    }
    let amount_g = amount * grams_factor_decimal(from);
    Ok(amount_g / grams_factor_decimal(to))
}
