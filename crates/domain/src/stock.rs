//! Pure physical-stock values. A balanced snapshot proves no provenance,
//! availability, permission, selected executor or durable stock effect.

use crate::{PoolId, Quantity, Sequence, listing::MeasurementUnit};
use core::{fmt, str::FromStr};

/// A bounded, input-free stock value failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StockError {
    /// Integer input is not a canonical unsigned decimal string.
    InvalidFormat,
    /// Ticks exceed the supported maximum, including checked addition.
    OutOfRange,
    /// Subtraction would produce a negative balance.
    Underflow,
    /// A pool scale exceeds six fractional decimal places.
    InvalidScale,
    /// Each-count pools cannot have fractional precision.
    FractionalCount,
    /// The declared quantity and pool use different units.
    UnitMismatch,
    /// The quantity cannot be represented by integral pool ticks.
    PrecisionLoss,
    /// The seven physical categories do not sum to introduced stock.
    ConservationViolation,
}
impl fmt::Display for StockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFormat => "stock ticks must be a canonical unsigned integer",
            Self::OutOfRange => "stock ticks exceed the supported bound",
            Self::Underflow => "stock balance would be negative",
            Self::InvalidScale => "stock scale exceeds six decimal places",
            Self::FractionalCount => "each-count stock requires integral precision",
            Self::UnitMismatch => "stock quantity unit differs from pool unit",
            Self::PrecisionLoss => "stock quantity cannot be represented by integral ticks",
            Self::ConservationViolation => {
                "physical stock categories do not conserve introduced stock"
            }
        })
    }
}
impl core::error::Error for StockError {}

/// Exact integral pool ticks, including zero, bounded by10^18.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StockTicks(u64);
impl StockTicks {
    /// Largest admitted physical balance in ticks.
    pub const MAX: Self = Self(1_000_000_000_000_000_000);
    /// Exact zero ticks.
    pub const ZERO: Self = Self(0);
    /// Validates an integral Rust input; this grants no stock authority.
    pub const fn new(value: u64) -> Result<Self, StockError> {
        if value > Self::MAX.0 {
            return Err(StockError::OutOfRange);
        }
        Ok(Self(value))
    }
    /// Returns the exact integer; foreign numeric boundaries use integer strings.
    pub const fn get(self) -> u64 {
        self.0
    }
    /// Adds ticks without wrapping or exceeding the stock bound.
    pub fn checked_add(self, other: Self) -> Result<Self, StockError> {
        Self::new(self.0.checked_add(other.0).ok_or(StockError::OutOfRange)?)
    }
    /// Subtracts ticks without clamping a negative result to zero.
    pub fn checked_sub(self, other: Self) -> Result<Self, StockError> {
        Self::new(self.0.checked_sub(other.0).ok_or(StockError::Underflow)?)
    }
}
impl FromStr for StockTicks {
    type Err = StockError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || value.len() > 19
            || !value.bytes().all(|v| v.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(StockError::InvalidFormat);
        }
        Self::new(value.parse().map_err(|_| StockError::OutOfRange)?)
    }
}
impl fmt::Display for StockTicks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for StockTicks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StockTicks(<redacted>)")
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for StockTicks {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for StockTicks {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = StockTicks;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded stock ticks as a canonical integer string")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<StockTicks, E> {
                value.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

/// One explicit physical unit and fractional precision for an entire pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolUnit {
    unit: MeasurementUnit,
    scale: u8,
}
impl PoolUnit {
    /// Admits precision0–6, with scale zero for each counts.
    pub const fn new(unit: MeasurementUnit, scale: u8) -> Result<Self, StockError> {
        if scale > 6 {
            return Err(StockError::InvalidScale);
        }
        if matches!(unit, MeasurementUnit::Each) && scale != 0 {
            return Err(StockError::FractionalCount);
        }
        Ok(Self { unit, scale })
    }
    /// Returns the fixed pool unit, without converting another unit.
    pub const fn unit(self) -> MeasurementUnit {
        self.unit
    }
    /// Returns fractional decimal places represented by one tick.
    pub const fn scale(self) -> u8 {
        self.scale
    }
    /// Converts only an exactly matching declared unit and precision. Compatible
    /// unit conversion must be requested explicitly on `Quantity` beforehand.
    pub fn ticks_for(self, quantity: Quantity) -> Result<StockTicks, StockError> {
        if quantity.unit() != self.unit {
            return Err(StockError::UnitMismatch);
        }
        let amount = quantity.amount();
        let difference = self
            .scale
            .checked_sub(amount.scale())
            .ok_or(StockError::PrecisionLoss)?;
        let factor = 10_u64.pow(u32::from(difference));
        StockTicks::new(
            amount
                .coefficient()
                .checked_mul(factor)
                .ok_or(StockError::OutOfRange)?,
        )
    }
}

/// Seven disjoint claimed balances. Validate their total with `PoolBalances`.
/// Released quantities are movements back to available, never an eighth balance.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PhysicalBalances {
    /// Unheld physical stock.
    pub available: StockTicks,
    /// Physical stock held before commitment issuance is resolved.
    pub preparatory_hold: StockTicks,
    /// Remaining physical backing of installed commitments.
    pub committed_hold: StockTicks,
    /// Physical backing retained under unresolved risk.
    pub physical_quarantine: StockTicks,
    /// Cumulative physical stock consumed by handover.
    pub consumed: StockTicks,
    /// Cumulative physically present stock moved to waste.
    pub waste: StockTicks,
    /// Cumulative physical stock explicitly removed.
    pub removed: StockTicks,
}
impl PhysicalBalances {
    /// All seven categories are empty.
    pub const ZERO: Self = Self {
        available: StockTicks::ZERO,
        preparatory_hold: StockTicks::ZERO,
        committed_hold: StockTicks::ZERO,
        physical_quarantine: StockTicks::ZERO,
        consumed: StockTicks::ZERO,
        waste: StockTicks::ZERO,
        removed: StockTicks::ZERO,
    };
}
impl fmt::Debug for PhysicalBalances {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhysicalBalances(<redacted>)")
    }
}

/// A conservation-checked physical balance claim, without authority or effects.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PoolBalances {
    introduced: StockTicks,
    physical: PhysicalBalances,
}
impl PoolBalances {
    /// Admits only an exact sum of the seven disjoint physical categories.
    /// Future promises and obligation deficits cannot be counted as stock.
    pub fn new(introduced: StockTicks, physical: PhysicalBalances) -> Result<Self, StockError> {
        let values = [
            physical.available,
            physical.preparatory_hold,
            physical.committed_hold,
            physical.physical_quarantine,
            physical.consumed,
            physical.waste,
            physical.removed,
        ];
        let sum = values
            .into_iter()
            .try_fold(StockTicks::ZERO, StockTicks::checked_add)?;
        if sum != introduced {
            return Err(StockError::ConservationViolation);
        }
        Ok(Self {
            introduced,
            physical,
        })
    }
    /// Returns cumulative introduced physical stock, including positive corrections.
    pub const fn introduced(self) -> StockTicks {
        self.introduced
    }
    /// Returns the seven conservation-checked balances.
    pub const fn physical(self) -> PhysicalBalances {
        self.physical
    }
}
impl fmt::Debug for PoolBalances {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PoolBalances(<redacted>)")
    }
}

/// A versioned claimed pool projection. Authentication and durable version
/// admission belong to the selected executor, never this value constructor.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PoolSnapshot {
    id: PoolId,
    unit: PoolUnit,
    balances: PoolBalances,
    version: Sequence,
}
impl PoolSnapshot {
    /// Combines already-validated values; version zero is an explicit genesis.
    pub const fn new(
        id: PoolId,
        unit: PoolUnit,
        balances: PoolBalances,
        version: Sequence,
    ) -> Self {
        Self {
            id,
            unit,
            balances,
            version,
        }
    }
    /// Returns the declared pool identity.
    pub const fn id(self) -> PoolId {
        self.id
    }
    /// Returns this pool's fixed unit/precision.
    pub const fn unit(self) -> PoolUnit {
        self.unit
    }
    /// Returns the validated physical balance claim.
    pub const fn balances(self) -> PoolBalances {
        self.balances
    }
    /// Returns the declared pool revision; this is not an authority generation.
    pub const fn version(self) -> Sequence {
        self.version
    }
}
impl fmt::Debug for PoolSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PoolSnapshot(<redacted>)")
    }
}
