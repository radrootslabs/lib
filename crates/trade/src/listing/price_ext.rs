use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};
use radroots_core::{
    RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreQuantityPrice,
    RadrootsCoreQuantityPriceOps,
};
use radroots_events::listing::models::RadrootsListingQuantity;

pub trait AsCoreQuantityPrice {
    fn as_core_qp(&self) -> RadrootsCoreQuantityPrice;
}

pub trait ListingPricingExt {
    fn subtotal_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingSubtotal;
    fn total_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingTotal;
}

impl ListingPricingExt for RadrootsCoreQuantityPrice {
    fn subtotal_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingSubtotal {
        let count = qty.count.unwrap_or(1);
        let effective_qty = RadrootsCoreQuantity::new(
            qty.value.amount * RadrootsCoreDecimal::from(count as u32),
            qty.value.unit,
        );

        let money = self.cost_for_rounded(&effective_qty);

        RadrootsTradeListingSubtotal {
            price_amount: money.clone(),
            price_currency: self.amount.currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
        }
    }

    fn total_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingTotal {
        let sub = self.subtotal_for(qty);
        RadrootsTradeListingTotal {
            price_amount: sub.price_amount,
            price_currency: sub.price_currency,
            quantity_amount: sub.quantity_amount,
            quantity_unit: sub.quantity_unit,
        }
    }
}
