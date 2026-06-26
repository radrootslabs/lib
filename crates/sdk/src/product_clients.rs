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
    pub(crate) _sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> MarketClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { _sdk: sdk }
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
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct DvmClient<'client> {
    pub(crate) _sdk: &'client RadrootsClient,
}

#[cfg(feature = "runtime")]
impl<'client> DvmClient<'client> {
    pub(crate) fn new(sdk: &'client RadrootsClient) -> Self {
        Self { _sdk: sdk }
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
