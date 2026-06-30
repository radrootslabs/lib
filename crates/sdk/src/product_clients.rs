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

    pub fn buyer(&self) -> TradeBuyerClient<'client> {
        TradeBuyerClient { sdk: self.sdk }
    }

    pub fn seller(&self) -> TradeSellerClient<'client> {
        TradeSellerClient { sdk: self.sdk }
    }

    pub fn validation(&self) -> TradeValidationClient<'client> {
        TradeValidationClient { sdk: self.sdk }
    }

    pub fn status_client(&self) -> TradeStatusClient<'client> {
        TradeStatusClient { sdk: self.sdk }
    }

    pub fn resync(&self) -> TradeResyncClient<'client> {
        TradeResyncClient { sdk: self.sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeBuyerClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeBuyerClient<'client> {
    pub fn root(&self) -> &'client RadrootsClient {
        self.sdk
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeSellerClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeSellerClient<'client> {
    pub fn root(&self) -> &'client RadrootsClient {
        self.sdk
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeValidationClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeValidationClient<'client> {
    pub fn root(&self) -> &'client RadrootsClient {
        self.sdk
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeStatusClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeStatusClient<'client> {
    pub fn root(&self) -> &'client RadrootsClient {
        self.sdk
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct TradeResyncClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> TradeResyncClient<'client> {
    pub fn root(&self) -> &'client RadrootsClient {
        self.sdk
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct DvmClient<'client> {
    pub(crate) sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> DvmClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { sdk }
    }
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
