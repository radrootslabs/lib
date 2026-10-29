//! Closed value codecs. Successful decoding still proves no stock authority.
use super::{
    ObligationBacking, PhysicalBalances, PoolBalances, PoolSnapshot, PoolUnit, StockTicks,
};
use crate::{PoolId, Sequence, listing::MeasurementUnit};
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Clone, Copy)]
struct Scale(u8);
impl Serialize for Scale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Scale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = Scale;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("one canonical pool precision string from0 through6")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Scale, E> {
                match value.as_bytes() {
                    [byte @ b'0'..=b'6'] => Ok(Scale(byte - b'0')),
                    _ => Err(E::custom("invalid pool precision")),
                }
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Unit {
    unit: MeasurementUnit,
    scale: Scale,
}
impl Serialize for PoolUnit {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Unit {
            unit: self.unit(),
            scale: Scale(self.scale()),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PoolUnit {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire: Unit = object(deserializer)?;
        Self::new(wire.unit, wire.scale.0).map_err(de::Error::custom)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Physical {
    available: StockTicks,
    preparatory_hold: StockTicks,
    committed_hold: StockTicks,
    physical_quarantine: StockTicks,
    consumed: StockTicks,
    waste: StockTicks,
    removed: StockTicks,
}
impl Serialize for PhysicalBalances {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Physical {
            available: self.available,
            preparatory_hold: self.preparatory_hold,
            committed_hold: self.committed_hold,
            physical_quarantine: self.physical_quarantine,
            consumed: self.consumed,
            waste: self.waste,
            removed: self.removed,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PhysicalBalances {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v: Physical = object(deserializer)?;
        Ok(Self {
            available: v.available,
            preparatory_hold: v.preparatory_hold,
            committed_hold: v.committed_hold,
            physical_quarantine: v.physical_quarantine,
            consumed: v.consumed,
            waste: v.waste,
            removed: v.removed,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Balances {
    introduced: StockTicks,
    physical: PhysicalBalances,
}
impl Serialize for PoolBalances {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Balances {
            introduced: self.introduced(),
            physical: self.physical(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PoolBalances {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v: Balances = object(deserializer)?;
        Self::new(v.introduced, v.physical).map_err(de::Error::custom)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backing {
    remaining: StockTicks,
    backed: StockTicks,
    deficit: StockTicks,
}
impl Serialize for ObligationBacking {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Backing {
            remaining: self.remaining(),
            backed: self.backed(),
            deficit: self.deficit(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for ObligationBacking {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value: Backing = object(deserializer)?;
        Self::new(value.remaining, value.backed, value.deficit).map_err(de::Error::custom)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    id: PoolId,
    unit: PoolUnit,
    balances: PoolBalances,
    version: Sequence,
}
impl Serialize for PoolSnapshot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Snapshot {
            id: self.id(),
            unit: self.unit(),
            balances: self.balances(),
            version: self.version(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for PoolSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v: Snapshot = object(deserializer)?;
        Ok(Self::new(v.id, v.unit, v.balances, v.version))
    }
}

pub(super) fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    struct Visitor<T>(core::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a closed stock value object")
        }
        fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Visitor(core::marker::PhantomData))
}
