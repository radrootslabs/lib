//! Exact private option-to-pool claims; no store, stock hold or source authority.
use super::{PoolUnit, StockError, StockTicks};
use crate::{
    ListingOptionId, PoolId, PositiveDecimal, Quantity, QuantityError, Sequence,
    listing::ListingCoordinate,
};
use core::fmt;

/// The entire listing coordinate and stable option identity of a private mapping.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OptionMappingKey {
    listing: ListingCoordinate,
    option: ListingOptionId,
}
impl OptionMappingKey {
    /// Captures identity metadata without authenticating an owner or listing.
    pub const fn new(listing: ListingCoordinate, option: ListingOptionId) -> Self {
        Self { listing, option }
    }
    /// Returns the complete claimed listing coordinate.
    pub const fn listing(self) -> ListingCoordinate {
        self.listing
    }
    /// Returns the stable option identity within that listing.
    pub const fn option_id(self) -> ListingOptionId {
        self.option
    }
}
impl fmt::Debug for OptionMappingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OptionMappingKey(<private metadata>)")
    }
}

/// A validated private conversion claim at one positive mapping version.
/// This establishes neither an observed listing nor physical backing. The
/// executor checks current ownership, pool identity and versions separately.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OptionPoolMapping {
    key: OptionMappingKey,
    pool: PoolId,
    unit: PoolUnit,
    per_sale_unit: Quantity,
    per_sale_ticks: StockTicks,
    version: Sequence,
}
impl OptionPoolMapping {
    /// Resolves an explicit positive per-sale draw into the fixed pool precision.
    /// Only exact g/kg or mL/L conversion is performed. Package labels, estimated
    /// contents and mass/volume assumptions never supply a conversion here.
    pub fn new(
        key: OptionMappingKey,
        pool: PoolId,
        unit: PoolUnit,
        per_sale_unit: Quantity,
        version: Sequence,
    ) -> Result<Self, MappingError> {
        if version.get() == 0 {
            return Err(MappingError::InvalidVersion);
        }
        if per_sale_unit.amount().is_zero() {
            return Err(MappingError::ZeroDraw);
        }
        let normalized = per_sale_unit.convert(unit.unit())?;
        let per_sale_ticks = unit.ticks_for(normalized)?;
        if per_sale_ticks == StockTicks::ZERO {
            return Err(MappingError::ZeroDraw);
        }
        Ok(Self {
            key,
            pool,
            unit,
            per_sale_unit,
            per_sale_ticks,
            version,
        })
    }
    /// Returns the exact private option key.
    pub const fn key(self) -> OptionMappingKey {
        self.key
    }
    /// Returns the claimed pool identity.
    pub const fn pool_id(self) -> PoolId {
        self.pool
    }
    /// Returns the fixed unit/precision used to validate this mapping.
    pub const fn pool_unit(self) -> PoolUnit {
        self.unit
    }
    /// Retains the original explicit quantity, including its declared SI unit.
    pub const fn per_sale_unit(self) -> Quantity {
        self.per_sale_unit
    }
    /// Returns the exact integral pool ticks for one declared sale unit.
    pub const fn per_sale_ticks(self) -> StockTicks {
        self.per_sale_ticks
    }
    /// Returns a positive mapping version, without authenticating freshness.
    pub const fn version(self) -> Sequence {
        self.version
    }
    /// Multiplies a validated positive sale amount without rounding pool ticks.
    /// The caller must independently enforce the selected option's sale basis,
    /// including integral package purchases. This function reserves no stock.
    pub fn draw_for_sales(self, sales: PositiveDecimal) -> Result<StockTicks, MappingError> {
        let amount = sales.amount();
        let numerator = u128::from(self.per_sale_ticks.get())
            .checked_mul(u128::from(amount.coefficient()))
            .ok_or(StockError::OutOfRange)?;
        let denominator = 10_u128.pow(u32::from(amount.scale()));
        if !numerator.is_multiple_of(denominator) {
            return Err(StockError::PrecisionLoss.into());
        }
        let ticks = u64::try_from(numerator / denominator).map_err(|_| StockError::OutOfRange)?;
        StockTicks::new(ticks).map_err(MappingError::from)
    }
}
impl fmt::Debug for OptionPoolMapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OptionPoolMapping(<private metadata>)")
    }
}

/// Input-free exact mapping validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MappingError {
    /// A stored mapping version must be positive; version0 denotes absence.
    InvalidVersion,
    /// One sale unit cannot consume an explicit zero amount.
    ZeroDraw,
    /// Explicit quantity conversion is incompatible or inexact.
    Quantity(QuantityError),
    /// Exact ticks or multiplication exceed the physical value contract.
    Stock(StockError),
}
impl From<QuantityError> for MappingError {
    fn from(value: QuantityError) -> Self {
        Self::Quantity(value)
    }
}
impl From<StockError> for MappingError {
    fn from(value: StockError) -> Self {
        Self::Stock(value)
    }
}
impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion => f.write_str("mapping version must be positive"),
            Self::ZeroDraw => f.write_str("mapping draw must be positive"),
            Self::Quantity(error) => error.fmt(f),
            Self::Stock(error) => error.fmt(f),
        }
    }
}
impl core::error::Error for MappingError {}

#[cfg(feature = "serde")]
mod serialization {
    use super::*;
    use serde::{Deserialize, Serialize};
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        listing: ListingCoordinate,
        option_id: ListingOptionId,
        pool_id: PoolId,
        pool_unit: PoolUnit,
        per_sale_unit: Quantity,
        per_sale_ticks: StockTicks,
        version: Sequence,
    }
    impl Serialize for OptionPoolMapping {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            Wire {
                listing: self.key.listing,
                option_id: self.key.option,
                pool_id: self.pool,
                pool_unit: self.unit,
                per_sale_unit: self.per_sale_unit,
                per_sale_ticks: self.per_sale_ticks,
                version: self.version,
            }
            .serialize(serializer)
        }
    }
    impl<'de> Deserialize<'de> for OptionPoolMapping {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let wire: Wire = super::super::serialization::object(deserializer)?;
            let value = Self::new(
                OptionMappingKey::new(wire.listing, wire.option_id),
                wire.pool_id,
                wire.pool_unit,
                wire.per_sale_unit,
                wire.version,
            )
            .map_err(serde::de::Error::custom)?;
            if value.per_sale_ticks() != wire.per_sale_ticks {
                return Err(serde::de::Error::custom(
                    "mapping tick conversion does not match its declared quantity",
                ));
            }
            Ok(value)
        }
    }
}
