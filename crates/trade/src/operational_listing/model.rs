#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOperationalListingSubtotal {
    pub price_amount: radroots_core::RadrootsCoreMoney,
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOperationalListingTotal {
    pub price_amount: radroots_core::RadrootsCoreMoney,
    pub price_currency: radroots_core::RadrootsCoreCurrency,
    pub quantity_amount: radroots_core::RadrootsCoreDecimal,
    pub quantity_unit: radroots_core::RadrootsCoreUnit,
}
