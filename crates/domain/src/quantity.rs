use crate::{AmountError, Decimal, listing::MeasurementUnit};
use core::fmt;

/// Validated declared physical amount; this establishes no stock authority.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Quantity {
    amount: Decimal,
    unit: MeasurementUnit,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            amount: Decimal,
            unit: MeasurementUnit,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Wire;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a closed exact quantity object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Wire, A::Error> {
                <Wire as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
            }
        }
        let wire = deserializer.deserialize_map(Visitor)?;
        Self::new(wire.amount, wire.unit).map_err(serde::de::Error::custom)
    }
}

impl Quantity {
    /// Admits exact values, including explicit zero; each counts are integral.
    pub fn new(amount: Decimal, unit: MeasurementUnit) -> Result<Self, QuantityError> {
        if unit == MeasurementUnit::Each && amount.scale() != 0 {
            return Err(QuantityError::FractionalCount);
        }
        Ok(Self { amount, unit })
    }

    /// Returns the exact normalized nonnegative amount.
    pub const fn amount(self) -> Decimal {
        self.amount
    }

    /// Returns the single existing measurement identity.
    pub const fn unit(self) -> MeasurementUnit {
        self.unit
    }

    /// Explicitly converts g/kg or mL/L without rounding or inferred dimensions.
    pub fn convert(self, target: MeasurementUnit) -> Result<Self, QuantityError> {
        if self.unit == target {
            return Ok(self);
        }
        let multiply = match (self.unit, target) {
            (MeasurementUnit::Kilogram, MeasurementUnit::Gram)
            | (MeasurementUnit::Litre, MeasurementUnit::Millilitre) => true,
            (MeasurementUnit::Gram, MeasurementUnit::Kilogram)
            | (MeasurementUnit::Millilitre, MeasurementUnit::Litre) => false,
            _ => return Err(QuantityError::IncompatibleDimension),
        };
        let mut coefficient = i128::from(self.amount.coefficient());
        let mut scale = self.amount.scale();
        if multiply && scale < 3 {
            let factor = 10_i128
                .checked_pow(u32::from(3 - scale))
                .ok_or(QuantityError::ArithmeticOutOfRange)?;
            coefficient = coefficient
                .checked_mul(factor)
                .ok_or(QuantityError::ArithmeticOutOfRange)?;
            scale = 0;
        } else if multiply {
            scale -= 3;
        } else {
            scale = scale
                .checked_add(3)
                .ok_or(QuantityError::ArithmeticOutOfRange)?;
        }
        while scale > 0 && coefficient.checked_rem(10) == Some(0) {
            coefficient = coefficient
                .checked_div(10)
                .ok_or(QuantityError::ArithmeticOutOfRange)?;
            scale -= 1;
        }
        if scale > Decimal::MAX_SCALE {
            return Err(QuantityError::PrecisionLoss);
        }
        let coefficient =
            u64::try_from(coefficient).map_err(|_| QuantityError::ArithmeticOutOfRange)?;
        Self::new(Decimal::from_parts(coefficient, scale)?, target)
    }

    /// Adds matching units exactly; even compatible units require conversion.
    pub fn checked_add(self, other: Self) -> Result<Self, QuantityError> {
        if self.unit != other.unit {
            return Err(QuantityError::UnitMismatch);
        }
        Self::new(self.amount.checked_add(other.amount)?, self.unit)
    }

    /// Subtracts matching units exactly; negative results remain errors.
    pub fn checked_sub(self, other: Self) -> Result<Self, QuantityError> {
        if self.unit != other.unit {
            return Err(QuantityError::UnitMismatch);
        }
        Self::new(self.amount.checked_sub(other.amount)?, self.unit)
    }
}

/// Declared package-content data, without physical backing or stock permission.
/// Estimated contents may inform display but never supply a fixed conversion.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum ContentMeasure {
    /// Explicit declared fixed quantity; authorization is established elsewhere.
    Fixed {
        /// Exact declared content and measurement unit.
        quantity: Quantity,
    },
    /// Display estimate, explicitly distinct from a guaranteed conversion.
    Estimated {
        /// Exact representation of the declared estimate.
        quantity: Quantity,
    },
    /// No quantity or universal physical conversion is implied.
    Unspecified,
}

impl ContentMeasure {
    /// Returns only explicitly fixed metadata; estimates never become fixed.
    pub const fn fixed_quantity(self) -> Option<Quantity> {
        match self {
            Self::Fixed { quantity } => Some(quantity),
            Self::Estimated { .. } | Self::Unspecified => None,
        }
    }
}

/// Input-free physical-value admission or exact-conversion failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum QuantityError {
    /// Each counts cannot have a fractional normalized amount.
    FractionalCount,
    /// Arithmetic operands have different explicit units.
    UnitMismatch,
    /// No supported dimension-preserving conversion exists.
    IncompatibleDimension,
    /// The exact converted value would exceed the supported fractional scale.
    PrecisionLoss,
    /// A checked conversion intermediate cannot be represented.
    ArithmeticOutOfRange,
    /// Exact decimal admission/arithmetic failed.
    Amount(AmountError),
}

impl From<AmountError> for QuantityError {
    fn from(value: AmountError) -> Self {
        Self::Amount(value)
    }
}

impl fmt::Display for QuantityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FractionalCount => "fractional each count",
            Self::UnitMismatch => "physical quantity units differ",
            Self::IncompatibleDimension => "incompatible physical measurement dimensions",
            Self::PrecisionLoss => "exact conversion exceeds supported precision",
            Self::ArithmeticOutOfRange => "quantity arithmetic exceeds supported bounds",
            Self::Amount(error) => return error.fmt(f),
        })
    }
}

impl core::error::Error for QuantityError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Amount(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Debug for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Quantity(<redacted>)")
    }
}

impl fmt::Debug for ContentMeasure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContentMeasure(<redacted>)")
    }
}
