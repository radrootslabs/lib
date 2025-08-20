#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingSubtotal {
    pub price_amount: radroots_core::RadrootsCoreMoney,
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingTotal {
    pub price_amount: radroots_core::RadrootsCoreMoney,
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}
