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
pub enum RadrootsCoreUnitDimension {
    Count,
    Mass,
    Volume,
}

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
        a.dimension() == b.dimension()
    }

    #[inline]
    pub fn dimension(&self) -> RadrootsCoreUnitDimension {
        match self {
            Self::Each => RadrootsCoreUnitDimension::Count,
            Self::MassKg | Self::MassG | Self::MassOz | Self::MassLb => {
                RadrootsCoreUnitDimension::Mass
            }
            Self::VolumeL | Self::VolumeMl => RadrootsCoreUnitDimension::Volume,
        }
    }

    #[inline]
    pub fn canonical_unit(&self) -> Self {
        match self.dimension() {
            RadrootsCoreUnitDimension::Count => Self::Each,
            RadrootsCoreUnitDimension::Mass => Self::MassG,
            RadrootsCoreUnitDimension::Volume => Self::VolumeMl,
        }
    }

    #[inline]
    pub fn is_volume(&self) -> bool {
        matches!(self, Self::VolumeL | Self::VolumeMl)
    }

    #[inline]
    pub fn is_mass(&self) -> bool {
        matches!(
            self,
            Self::MassKg | Self::MassG | Self::MassOz | Self::MassLb
        )
    }

    #[inline]
    pub fn is_count(&self) -> bool {
        matches!(self, Self::Each)
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
    NotAVolumeUnit,
}

impl fmt::Display for RadrootsCoreUnitParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnit => write!(f, "unknown unit string"),
            Self::NotAMassUnit => write!(f, "unit is not a mass unit"),
            Self::NotAVolumeUnit => write!(f, "unit is not a volume unit"),
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
    NotVolumeUnit {
        from: RadrootsCoreUnit,
        to: RadrootsCoreUnit,
    },
    NotConvertibleUnits {
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
            RadrootsCoreUnitConvertError::NotVolumeUnit { from, to } => {
                write!(f, "unit conversion requires volume units: {from} -> {to}")
            }
            RadrootsCoreUnitConvertError::NotConvertibleUnits { from, to } => {
                write!(f, "unit conversion requires matching dimensions: {from} -> {to}")
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
pub fn parse_volume_unit(s: &str) -> Result<RadrootsCoreUnit, RadrootsCoreUnitParseError> {
    let u: RadrootsCoreUnit = RadrootsCoreUnit::from_str(s)?;
    if u.is_volume() {
        Ok(u)
    } else {
        Err(RadrootsCoreUnitParseError::NotAVolumeUnit)
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
fn milliliters_factor_decimal(u: RadrootsCoreUnit) -> RadrootsCoreDecimal {
    match u {
        RadrootsCoreUnit::VolumeMl => RadrootsCoreDecimal::ONE,
        RadrootsCoreUnit::VolumeL => RadrootsCoreDecimal::from(1000u32),
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

#[inline]
pub fn convert_volume_decimal(
    amount: RadrootsCoreDecimal,
    from: RadrootsCoreUnit,
    to: RadrootsCoreUnit,
) -> Result<RadrootsCoreDecimal, RadrootsCoreUnitConvertError> {
    if !from.is_volume() || !to.is_volume() {
        return Err(RadrootsCoreUnitConvertError::NotVolumeUnit { from, to });
    }
    let amount_ml = amount * milliliters_factor_decimal(from);
    Ok(amount_ml / milliliters_factor_decimal(to))
}

#[inline]
pub fn convert_unit_decimal(
    amount: RadrootsCoreDecimal,
    from: RadrootsCoreUnit,
    to: RadrootsCoreUnit,
) -> Result<RadrootsCoreDecimal, RadrootsCoreUnitConvertError> {
    if from == to {
        return Ok(amount);
    }
    if !RadrootsCoreUnit::same_dimension(from, to) {
        return Err(RadrootsCoreUnitConvertError::NotConvertibleUnits { from, to });
    }
    if from.is_mass() {
        return convert_mass_decimal(amount, from, to);
    }
    if from.is_volume() {
        return convert_volume_decimal(amount, from, to);
    }
    Err(RadrootsCoreUnitConvertError::NotConvertibleUnits { from, to })
}
