#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOperationalListingSubtotal {
    pub price_amount: radroots_core::Money,
    pub price_currency: radroots_core::Currency,
    pub quantity_amount: radroots_core::Decimal,
    pub quantity_unit: radroots_core::Unit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsOperationalListingTotal {
    pub price_amount: radroots_core::Money,
    pub price_currency: radroots_core::Currency,
    pub quantity_amount: radroots_core::Decimal,
    pub quantity_unit: radroots_core::Unit,
}
