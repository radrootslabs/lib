//! Bounded inventory permission claims. A qualified host must authenticate the
//! protected grant and its current principal, expiry and revocation separately.
use crate::workspace::WorkspaceError;
use radroots_domain::PoolId;

/// One explicit inventory capability; no signer or grant-administration right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum StockAction {
    ReadPools,
    ReadOperations,
    CreatePool,
    AddStock,
    RemoveStock,
    SpoilStock,
}
impl StockAction {
    const ALL: [Self; 6] = [
        Self::ReadPools,
        Self::ReadOperations,
        Self::CreatePool,
        Self::AddStock,
        Self::RemoveStock,
        Self::SpoilStock,
    ];
    const fn bit(self) -> u8 {
        match self {
            Self::ReadPools => 1,
            Self::ReadOperations => 2,
            Self::CreatePool => 4,
            Self::AddStock => 8,
            Self::RemoveStock => 16,
            Self::SpoilStock => 32,
        }
    }
}
/// Explicit all-pool selection or at most64 sorted unique private pool IDs.
#[derive(Clone, Eq, PartialEq)]
pub struct StockResources(Option<Box<[PoolId]>>);
impl StockResources {
    /// Claims every pool in this selected seller namespace, without authority.
    pub const fn all_pools() -> Self {
        Self(None)
    }
    /// Requires canonical ascending unique IDs; empty never means all pools.
    pub fn pools(pools: Vec<PoolId>) -> Result<Self, WorkspaceError> {
        if pools.is_empty() || pools.len() > 64 || pools.windows(2).any(|v| v[0] >= v[1]) {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self(Some(pools.into_boxed_slice())))
    }
    /// Tests only the declared resource set, never authenticated permission.
    pub fn contains(&self, pool: PoolId) -> bool {
        self.0
            .as_ref()
            .is_none_or(|ids| ids.binary_search(&pool).is_ok())
    }
    /// None denotes the explicit all-pools variant; a selected set is nonempty.
    pub fn selected_pools(&self) -> Option<&[PoolId]> {
        self.0.as_deref()
    }
}
impl core::fmt::Debug for StockResources {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockResources(<redacted>)")
    }
}
/// Action/resource claims for one namespace. Write scopes explicitly include
/// pool and operation reads needed for checked effects and original recovery.
/// This constructor grants no permission and cannot create or update a grant.
#[derive(Clone, Eq, PartialEq)]
pub struct StockGrantScope {
    resources: StockResources,
    mask: u8,
}
impl StockGrantScope {
    /// Requires nonempty actions in the declared enum order, without duplicates.
    pub fn new(
        resources: StockResources,
        actions: Vec<StockAction>,
    ) -> Result<Self, WorkspaceError> {
        if actions.is_empty()
            || actions.len() > 6
            || actions.windows(2).any(|v| v[0].bit() >= v[1].bit())
        {
            return Err(WorkspaceError::InvalidInput);
        }
        let mask = actions
            .into_iter()
            .fold(0, |mask, action| mask | action.bit());
        if mask & !3 != 0 && mask & 3 != 3 {
            return Err(WorkspaceError::InvalidInput);
        }
        Ok(Self { resources, mask })
    }
    /// Explicit full inventory claim; it includes no grant control or signing.
    pub const fn full_inventory() -> Self {
        Self {
            resources: StockResources::all_pools(),
            mask: 63,
        }
    }
    /// Canonical bounded declared actions.
    pub fn actions(&self) -> Vec<StockAction> {
        StockAction::ALL
            .into_iter()
            .filter(|action| self.allows_action(*action))
            .collect()
    }
    pub fn resources(&self) -> &StockResources {
        &self.resources
    }
    /// Evaluates only a declared action, without grant authentication.
    pub const fn allows_action(&self, action: StockAction) -> bool {
        self.mask & action.bit() != 0
    }
    /// Evaluates the complete declared action/resource pair.
    pub fn allows(&self, action: StockAction, pool: PoolId) -> bool {
        self.allows_action(action) && self.resources.contains(pool)
    }
}
impl core::fmt::Debug for StockGrantScope {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StockGrantScope(<untrusted descriptor>)")
    }
}

#[cfg(feature = "serde")]
mod serialization {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};
    impl<'de> Deserialize<'de> for StockAction {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl de::Visitor<'_> for Visitor {
                type Value = StockAction;
                fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str("one exact stock action string")
                }
                fn visit_str<E: de::Error>(self, v: &str) -> Result<StockAction, E> {
                    match v {
                        "read_pools" => Ok(StockAction::ReadPools),
                        "read_operations" => Ok(StockAction::ReadOperations),
                        "create_pool" => Ok(StockAction::CreatePool),
                        "add_stock" => Ok(StockAction::AddStock),
                        "remove_stock" => Ok(StockAction::RemoveStock),
                        "spoil_stock" => Ok(StockAction::SpoilStock),
                        _ => Err(E::custom("invalid stock action")),
                    }
                }
            }
            d.deserialize_str(Visitor)
        }
    }
    fn bounded<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
        d: D,
    ) -> Result<Vec<T>, D::Error> {
        struct Visitor<T, const N: usize>(core::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> de::Visitor<'de> for Visitor<T, N> {
            type Value = Vec<T>;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a bounded scope array")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if values.len() == N {
                        return Err(de::Error::custom("scope array limit exceeded"));
                    }
                    values.push(value);
                }
                Ok(values)
            }
        }
        d.deserialize_seq(Visitor::<T, N>(core::marker::PhantomData))
    }
    fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
        struct Visitor<T>(core::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
            type Value = T;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a closed stock scope object")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
                T::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }
        d.deserialize_map(Visitor(core::marker::PhantomData))
    }
    impl Serialize for StockResources {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut wire =
                s.serialize_struct("StockResources", if self.0.is_some() { 2 } else { 1 })?;
            if let Some(ids) = &self.0 {
                wire.serialize_field("ids", ids)?;
            }
            wire.serialize_field(
                "type",
                if self.0.is_some() {
                    "pools"
                } else {
                    "all_pools"
                },
            )?;
            wire.end()
        }
    }
    impl<'de> Deserialize<'de> for StockResources {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            fn ids<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<PoolId>>, D::Error> {
                bounded::<D, PoolId, 64>(d).map(Some)
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                #[serde(rename = "type")]
                kind: String,
                #[serde(default, deserialize_with = "ids")]
                ids: Option<Vec<PoolId>>,
            }
            let wire: Wire = object(d)?;
            match (wire.kind.as_str(), wire.ids) {
                ("all_pools", None) => Ok(Self::all_pools()),
                ("pools", Some(ids)) => Self::pools(ids).map_err(de::Error::custom),
                _ => Err(de::Error::custom("invalid stock resources")),
            }
        }
    }
    impl Serialize for StockGrantScope {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let mut wire = s.serialize_struct("StockGrantScope", 2)?;
            wire.serialize_field("actions", &self.actions())?;
            wire.serialize_field("resources", &self.resources)?;
            wire.end()
        }
    }
    impl<'de> Deserialize<'de> for StockGrantScope {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            fn actions<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<StockAction>, D::Error> {
                bounded::<D, StockAction, 6>(d)
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire {
                resources: StockResources,
                #[serde(deserialize_with = "actions")]
                actions: Vec<StockAction>,
            }
            let wire: Wire = object(d)?;
            Self::new(wire.resources, wire.actions).map_err(de::Error::custom)
        }
    }
}
