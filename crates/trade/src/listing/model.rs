#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingSubtotal {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
    pub price_amount: radroots_core::RadrootsCoreMoney,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreCurrency"))]
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDecimal"))]
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreUnit"))]
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsTradeListingTotal {
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreMoney"))]
    pub price_amount: radroots_core::RadrootsCoreMoney,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreCurrency"))]
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreDecimal"))]
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    #[cfg_attr(feature = "ts-rs", ts(type = "RadrootsCoreUnit"))]
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}
