use crate::listing::operational_listing::model::{
    RadrootsOperationalListingSubtotal, RadrootsOperationalListingTotal,
};
use radroots_core::pricing::{Error as PricingError, QuantityPriceOps};
use radroots_core::{Decimal, Quantity};
use radroots_event::listing::operational::OperationalListingBin;

pub trait BinPricingTryExt {
    fn try_subtotal_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsOperationalListingSubtotal, PricingError>;
    fn try_total_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsOperationalListingTotal, PricingError>;
}

#[inline]
fn effective_quantity(
    bin: &OperationalListingBin,
    bin_count: u32,
) -> Result<Quantity, PricingError> {
    let amount = bin
        .quantity
        .amount()
        .checked_mul(Decimal::from(bin_count))
        .map_err(|_| PricingError::ArithmeticOverflow)?;
    Quantity::try_new(amount, bin.quantity.unit()).map_err(|_| PricingError::ArithmeticOverflow)
}

impl BinPricingTryExt for OperationalListingBin {
    fn try_subtotal_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsOperationalListingSubtotal, PricingError> {
        let effective_qty = effective_quantity(self, bin_count)?;
        let money = self
            .price_per_canonical_unit
            .try_cost_for_rounded(&effective_qty)?;
        let currency = money.currency();

        Ok(RadrootsOperationalListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount(),
            quantity_unit: effective_qty.unit(),
        })
    }

    fn try_total_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsOperationalListingTotal, PricingError> {
        let sub = self.try_subtotal_for_count(bin_count)?;
        Ok(RadrootsOperationalListingTotal {
            price_amount: sub.price_amount,
            price_currency: sub.price_currency,
            quantity_amount: sub.quantity_amount,
            quantity_unit: sub.quantity_unit,
        })
    }
}
