#[cfg(feature = "runtime")]
use crate::RadrootsSdk;
#[cfg(feature = "runtime")]
use core::marker::PhantomData;

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
    _sdk: PhantomData<&'sdk RadrootsSdk>,
}

#[cfg(feature = "runtime")]
impl<'sdk> OrdersClient<'sdk> {
    pub(crate) fn new(_sdk: &'sdk RadrootsSdk) -> Self {
        Self { _sdk: PhantomData }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy)]
pub struct SyncClient<'sdk> {
    _sdk: PhantomData<&'sdk RadrootsSdk>,
}

#[cfg(feature = "runtime")]
impl<'sdk> SyncClient<'sdk> {
    pub(crate) fn new(_sdk: &'sdk RadrootsSdk) -> Self {
        Self { _sdk: PhantomData }
    }
}
