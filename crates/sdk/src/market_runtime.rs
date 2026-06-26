#[cfg(feature = "runtime")]
use crate::sync_runtime::refresh_product_projections_for_sdk;
#[cfg(feature = "runtime")]
use crate::{
    MarketClient, RadrootsSdkError, SyncProjectionRefreshReceipt, SyncProjectionRefreshRequest,
};
#[cfg(feature = "runtime")]
use radroots_events::ids::{RadrootsEventId, RadrootsListingAddress, RadrootsPublicKey};
#[cfg(feature = "runtime")]
use radroots_trade::projection::{
    RadrootsListingProjectionRow, RadrootsListingSearchRequest, search_listing_projection,
};

#[cfg(feature = "runtime")]
pub const MARKET_SEARCH_DEFAULT_LIMIT: u32 = 50;

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct MarketSearchRequest {
    pub query: String,
    pub limit: u32,
    pub projection_refresh: SyncProjectionRefreshRequest,
}

#[cfg(feature = "runtime")]
impl MarketSearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: MARKET_SEARCH_DEFAULT_LIMIT,
            projection_refresh: SyncProjectionRefreshRequest::new(),
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_projection_refresh(mut self, refresh: SyncProjectionRefreshRequest) -> Self {
        self.projection_refresh = refresh;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MarketSearchReceipt {
    pub source: MarketSearchSource,
    pub refresh: SyncProjectionRefreshReceipt,
    pub listings: Vec<MarketListingSearchRow>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MarketSearchSource {
    LocalProjectionFts,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MarketListingSearchRow {
    pub listing_addr: RadrootsListingAddress,
    pub listing_event_id: RadrootsEventId,
    pub seller_pubkey: RadrootsPublicKey,
    pub title: String,
    pub description: String,
    pub product_type: String,
    pub price_amount: String,
    pub price_currency: String,
    pub inventory_available: String,
    pub delivery_method: String,
    pub locality_primary: String,
    pub locality_city: Option<String>,
    pub locality_region: Option<String>,
    pub locality_country: Option<String>,
    pub geohash5: String,
    pub updated_at_ms: i64,
}

#[cfg(feature = "runtime")]
impl<'sdk> MarketClient<'sdk> {
    pub async fn search(
        &self,
        request: MarketSearchRequest,
    ) -> Result<MarketSearchReceipt, RadrootsSdkError> {
        let refresh =
            refresh_product_projections_for_sdk(self.sdk, request.projection_refresh).await?;
        let rows = search_listing_projection(
            &self.sdk._event_store,
            &RadrootsListingSearchRequest::new(request.query).with_limit(request.limit),
        )
        .await?;
        let listings = rows
            .into_iter()
            .map(MarketListingSearchRow::try_from_projection_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MarketSearchReceipt {
            source: MarketSearchSource::LocalProjectionFts,
            refresh,
            listings,
        })
    }
}

#[cfg(feature = "runtime")]
impl MarketListingSearchRow {
    fn try_from_projection_row(
        row: RadrootsListingProjectionRow,
    ) -> Result<Self, RadrootsSdkError> {
        let listing_addr = row.listing_addr;
        let listing_event_id =
            RadrootsEventId::parse(row.listing_event_id.as_str()).map_err(|source| {
                RadrootsSdkError::Projection {
                    message: format!(
                        "stored listing projection event id `{}` is invalid: {source}",
                        row.listing_event_id
                    ),
                }
            })?;
        let seller_pubkey =
            RadrootsPublicKey::parse(row.seller_pubkey.as_str()).map_err(|source| {
                RadrootsSdkError::Projection {
                    message: format!(
                        "stored listing projection seller pubkey `{}` is invalid: {source}",
                        row.seller_pubkey
                    ),
                }
            })?;
        Ok(Self {
            listing_addr,
            listing_event_id,
            seller_pubkey,
            title: row.title,
            description: row.description,
            product_type: row.product_type,
            price_amount: row.price_amount,
            price_currency: row.price_currency,
            inventory_available: row.inventory_available,
            delivery_method: row.delivery_method,
            locality_primary: row.locality_primary,
            locality_city: row.locality_city,
            locality_region: row.locality_region,
            locality_country: row.locality_country,
            geohash5: row.geohash5,
            updated_at_ms: row.updated_at_ms,
        })
    }
}

#[cfg(test)]
#[path = "../tests/unit/market_runtime_tests.rs"]
mod tests;
