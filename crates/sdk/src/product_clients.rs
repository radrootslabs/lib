#[cfg(feature = "runtime")]
use crate::RadrootsClient;

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct FarmsClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> FarmsClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct ListingsClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> ListingsClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct MarketClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> MarketClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct GeoNamesClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> GeoNamesClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradesClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradesClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }

    #[cfg(feature = "signer-adapters")]
    pub fn buyer(&self) -> TradeBuyerClient<'client> {
        TradeBuyerClient { sdk: self.sdk }
    }

    pub fn seller(&self) -> TradeSellerClient<'client> {
        TradeSellerClient { sdk: self.sdk }
    }

    pub fn resync(&self) -> TradeResyncClient<'client> {
        TradeResyncClient { sdk: self.sdk }
    }

    pub fn validation_receipts(&self) -> TradeValidationReceiptsClient<'client> {
        TradeValidationReceiptsClient { sdk: self.sdk }
    }
}

#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
#[derive(Clone, Copy)]
pub struct TradeBuyerClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeSellerClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeResyncClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeValidationReceiptsClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct SyncClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> SyncClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
}
