#[cfg(feature = "runtime")]
use std::collections::BTreeMap;

#[cfg(feature = "runtime")]
use radroots_event::ids::{RadrootsEventId, RadrootsOrderId};
#[cfg(feature = "runtime")]
use radroots_trade::{
    identity::{RadrootsTradeId, RadrootsTradeLocator, RadrootsTradeLocatorCandidate},
    workflow::RadrootsTradeWorkflowState,
};

#[cfg(feature = "runtime")]
pub const SDK_TRADE_PROJECTION_CACHE_VERSION: u32 = 1;

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct SdkTradeProjectionCacheKey {
    pub order_id: RadrootsOrderId,
    pub root_event_id: RadrootsEventId,
    pub projection_version: u32,
}

#[cfg(feature = "runtime")]
impl SdkTradeProjectionCacheKey {
    pub fn new(order_id: RadrootsOrderId, root_event_id: RadrootsEventId) -> Self {
        Self {
            order_id,
            root_event_id,
            projection_version: SDK_TRADE_PROJECTION_CACHE_VERSION,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SdkTradeProjectionCacheRecord {
    pub key: SdkTradeProjectionCacheKey,
    pub locator: RadrootsTradeLocator,
    pub status: RadrootsTradeWorkflowState,
    pub updated_at_ms: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdkTradeProjectionCache {
    records: BTreeMap<SdkTradeProjectionCacheKey, SdkTradeProjectionCacheRecord>,
}

#[cfg(feature = "runtime")]
impl SdkTradeProjectionCache {
    pub fn upsert(&mut self, record: SdkTradeProjectionCacheRecord) {
        self.records.insert(record.key.clone(), record);
    }

    pub fn get(&self, key: &SdkTradeProjectionCacheKey) -> Option<&SdkTradeProjectionCacheRecord> {
        self.records.get(key)
    }

    pub fn ambiguity_candidates(
        &self,
        order_id: &RadrootsOrderId,
    ) -> Vec<RadrootsTradeLocatorCandidate> {
        self.records
            .values()
            .filter(|record| record.key.order_id == *order_id)
            .filter_map(|record| {
                Some(RadrootsTradeLocatorCandidate {
                    trade_id: RadrootsTradeId::from(record.key.order_id.clone()),
                    root_event_id: record.key.root_event_id.clone(),
                    listing_addr: record.locator.listing_addr.clone()?,
                    buyer_pubkey: record.locator.buyer_pubkey.clone()?,
                    seller_pubkey: record.locator.seller_pubkey.clone()?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[cfg(feature = "runtime")]
mod tests {
    use super::{
        SDK_TRADE_PROJECTION_CACHE_VERSION, SdkTradeProjectionCache, SdkTradeProjectionCacheKey,
        SdkTradeProjectionCacheRecord,
    };
    use radroots_event::ids::{
        RadrootsEventId, RadrootsListingAddress, RadrootsOrderId, RadrootsPublicKey,
    };
    use radroots_event::kinds::KIND_LISTING;
    use radroots_trade::{identity::RadrootsTradeLocator, workflow::RadrootsTradeWorkflowState};

    const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BUYER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn order_id() -> RadrootsOrderId {
        RadrootsOrderId::parse("order-1").expect("order id")
    }

    fn event_id(raw: u8) -> RadrootsEventId {
        RadrootsEventId::parse(format!("{raw:064x}")).expect("event id")
    }

    fn listing_addr() -> RadrootsListingAddress {
        RadrootsListingAddress::parse(format!("{KIND_LISTING}:{SELLER}:AAAAAAAAAAAAAAAAAAAAAg"))
            .expect("listing address")
    }

    fn locator(root: RadrootsEventId) -> RadrootsTradeLocator {
        RadrootsTradeLocator::from_order_id(order_id())
            .with_root_event_id(root)
            .with_listing_addr(listing_addr())
            .with_buyer_pubkey(RadrootsPublicKey::parse(BUYER).expect("buyer"))
            .with_seller_pubkey(RadrootsPublicKey::parse(SELLER).expect("seller"))
    }

    #[test]
    fn projection_cache_key_includes_root_and_version() {
        let first = SdkTradeProjectionCacheKey::new(order_id(), event_id(1));
        let second = SdkTradeProjectionCacheKey::new(order_id(), event_id(2));

        assert_ne!(first, second);
        assert_eq!(first.projection_version, SDK_TRADE_PROJECTION_CACHE_VERSION);
    }

    #[test]
    fn projection_cache_returns_root_ambiguity_candidates() {
        let mut cache = SdkTradeProjectionCache::default();
        for root in [event_id(1), event_id(2)] {
            let record = SdkTradeProjectionCacheRecord {
                key: SdkTradeProjectionCacheKey::new(order_id(), root.clone()),
                locator: locator(root),
                status: RadrootsTradeWorkflowState::Requested,
                updated_at_ms: 1,
            };
            cache.upsert(record);
        }

        let candidates = cache.ambiguity_candidates(&order_id());

        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.root_event_id == event_id(1))
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.root_event_id == event_id(2))
        );
    }
}
