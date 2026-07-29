//! Unit codes, dimensions, parsing, and checked deterministic conversion.

use core::fmt;
use core::str::FromStr;

#[cfg(all(feature = "serde", not(feature = "std")))]
use alloc::string::String;
#[cfg(feature = "serde")]
#[cfg(feature = "std")]
use std::string::String;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

use crate::Decimal;

const GRAMS_PER_OUNCE: Decimal = Decimal::from_parts(2_579_719_349, 6, 0, 9);
const GRAMS_PER_POUND: Decimal = Decimal::from_parts(45_359_237, 0, 0, 5);

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(as = "string_enum"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitDimension {
    #[cfg_attr(all(test, feature = "std"), dto(rename = "count"))]
    Count,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "mass"))]
    Mass,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "volume"))]
    Volume,
}

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(all(test, feature = "std"), dto(as = "string_enum"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    #[cfg_attr(all(test, feature = "std"), dto(rename = "each"))]
    Each,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "kg"))]
    MassKg,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "g"))]
    MassG,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "oz"))]
    MassOz,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "lb"))]
    MassLb,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "l"))]
    VolumeL,
    #[cfg_attr(all(test, feature = "std"), dto(rename = "ml"))]
    VolumeMl,
}

impl Unit {
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
    pub fn dimension(&self) -> UnitDimension {
        match self {
            Self::Each => UnitDimension::Count,
            Self::MassKg | Self::MassG | Self::MassOz | Self::MassLb => UnitDimension::Mass,
            Self::VolumeL | Self::VolumeMl => UnitDimension::Volume,
        }
    }

    #[inline]
    pub fn canonical_unit(&self) -> Self {
        match self.dimension() {
            UnitDimension::Count => Self::Each,
            UnitDimension::Mass => Self::MassG,
            UnitDimension::Volume => Self::VolumeMl,
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

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    UnknownUnit,
    NotAMassUnit,
    NotAVolumeUnit,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnit => write!(f, "unknown unit string"),
            Self::NotAMassUnit => write!(f, "unit is not a mass unit"),
            Self::NotAVolumeUnit => write!(f, "unit is not a volume unit"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    NotMassUnit { from: Unit, to: Unit },
    NotVolumeUnit { from: Unit, to: Unit },
    NotConvertibleUnits { from: Unit, to: Unit },
    ArithmeticOverflow { from: Unit, to: Unit },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::NotMassUnit { from, to } => {
                write!(f, "unit conversion requires mass units: {from} -> {to}")
            }
            ConvertError::NotVolumeUnit { from, to } => {
                write!(f, "unit conversion requires volume units: {from} -> {to}")
            }
            ConvertError::NotConvertibleUnits { from, to } => {
                write!(
                    f,
                    "unit conversion requires matching dimensions: {from} -> {to}"
                )
            }
            ConvertError::ArithmeticOverflow { from, to } => {
                write!(f, "unit conversion arithmetic overflow: {from} -> {to}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConvertError {}

impl FromStr for Unit {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        match s.as_str() {
            "each" | "ea" | "count" => Ok(Unit::Each),
            "kg" | "kilogram" | "kilograms" => Ok(Unit::MassKg),
            "g" | "gram" | "grams" => Ok(Unit::MassG),
            "oz" | "ounce" | "ounces" => Ok(Unit::MassOz),
            "lb" | "pound" | "pounds" => Ok(Unit::MassLb),
            "l" | "liter" | "litre" | "liters" | "litres" => Ok(Unit::VolumeL),
            "ml" | "milliliter" | "millilitre" | "milliliters" | "millilitres" => {
                Ok(Unit::VolumeMl)
            }
            _ => Err(ParseError::UnknownUnit),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for Unit {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.code())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        s.parse().map_err(D::Error::custom)
    }
}

#[inline]
pub fn parse_mass_unit(s: &str) -> Result<Unit, ParseError> {
    let u: Unit = Unit::from_str(s)?;
    if u.is_mass() {
        Ok(u)
    } else {
        Err(ParseError::NotAMassUnit)
    }
}

#[inline]
pub fn parse_volume_unit(s: &str) -> Result<Unit, ParseError> {
    let u: Unit = Unit::from_str(s)?;
    if u.is_volume() {
        Ok(u)
    } else {
        Err(ParseError::NotAVolumeUnit)
    }
}

#[inline]
/// Converts mass using exact decimal factors expressed in grams.
///
/// The pound and ounce factors are exact definitions. Arithmetic is checked;
/// division uses the decimal backend's deterministic precision and performs no
/// additional application-level rounding.
pub fn convert_mass_decimal(
    amount: Decimal,
    from: Unit,
    to: Unit,
) -> Result<Decimal, ConvertError> {
    let arithmetic_error = || ConvertError::ArithmeticOverflow { from, to };
    let amount_g = match from {
        Unit::MassG => amount,
        Unit::MassKg => amount
            .checked_mul(Decimal::from(1000u32))
            .map_err(|_| arithmetic_error())?,
        Unit::MassOz => amount
            .checked_mul(GRAMS_PER_OUNCE)
            .map_err(|_| arithmetic_error())?,
        Unit::MassLb => amount
            .checked_mul(GRAMS_PER_POUND)
            .map_err(|_| arithmetic_error())?,
        _ => {
            return Err(ConvertError::NotMassUnit { from, to });
        }
    };

    let to_factor = match to {
        Unit::MassG => Decimal::ONE,
        Unit::MassKg => Decimal::from(1000u32),
        Unit::MassOz => GRAMS_PER_OUNCE,
        Unit::MassLb => GRAMS_PER_POUND,
        _ => {
            return Err(ConvertError::NotMassUnit { from, to });
        }
    };

    amount_g
        .checked_div(to_factor)
        .map_err(|_| arithmetic_error())
}

#[inline]
/// Converts volume using the exact relation `1 L = 1000 mL`.
///
/// Arithmetic is checked and no application-level rounding is applied.
pub fn convert_volume_decimal(
    amount: Decimal,
    from: Unit,
    to: Unit,
) -> Result<Decimal, ConvertError> {
    let arithmetic_error = || ConvertError::ArithmeticOverflow { from, to };
    let amount_ml = match from {
        Unit::VolumeMl => amount,
        Unit::VolumeL => amount
            .checked_mul(Decimal::from(1000u32))
            .map_err(|_| arithmetic_error())?,
        _ => {
            return Err(ConvertError::NotVolumeUnit { from, to });
        }
    };

    let to_factor = match to {
        Unit::VolumeMl => Decimal::ONE,
        Unit::VolumeL => Decimal::from(1000u32),
        _ => {
            return Err(ConvertError::NotVolumeUnit { from, to });
        }
    };

    amount_ml
        .checked_div(to_factor)
        .map_err(|_| arithmetic_error())
}

#[inline]
pub fn convert_unit_decimal(
    amount: Decimal,
    from: Unit,
    to: Unit,
) -> Result<Decimal, ConvertError> {
    if !Unit::same_dimension(from, to) {
        return Err(ConvertError::NotConvertibleUnits { from, to });
    }
    match from.dimension() {
        UnitDimension::Count => Ok(amount),
        UnitDimension::Mass => convert_mass_decimal(amount, from, to),
        UnitDimension::Volume => convert_volume_decimal(amount, from, to),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_paths_cover_unit_branches() {
        assert_eq!(
            convert_mass_decimal(Decimal::ONE, Unit::Each, Unit::MassG),
            Err(ConvertError::NotMassUnit {
                from: Unit::Each,
                to: Unit::MassG
            })
        );
        assert_eq!(
            convert_volume_decimal(Decimal::ONE, Unit::Each, Unit::VolumeMl),
            Err(ConvertError::NotVolumeUnit {
                from: Unit::Each,
                to: Unit::VolumeMl
            })
        );
    }
}
