#[cfg(feature = "runtime")]
use crate::RadrootsSdk;

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct ListingsClient<'sdk> {
    pub(crate) sdk: &'sdk RadrootsSdk,
}

#[cfg(feature = "runtime")]
impl<'sdk> ListingsClient<'sdk> {
    pub(crate) fn new(sdk: &'sdk RadrootsSdk) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct OrdersClient<'sdk> {
    pub(crate) sdk: &'sdk RadrootsSdk,
}

#[cfg(feature = "runtime")]
impl<'sdk> OrdersClient<'sdk> {
    pub(crate) fn new(sdk: &'sdk RadrootsSdk) -> Self {
        Self { sdk }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct SyncClient<'sdk> {
    pub(crate) sdk: &'sdk RadrootsSdk,
}

#[cfg(feature = "runtime")]
impl<'sdk> SyncClient<'sdk> {
    pub(crate) fn new(sdk: &'sdk RadrootsSdk) -> Self {
        Self { sdk }
    }
}
