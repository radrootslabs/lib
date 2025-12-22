use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};
use radroots_core::{
    RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreQuantityPrice,
    RadrootsCoreQuantityPriceError, RadrootsCoreQuantityPriceOps,
};
use radroots_events::listing::RadrootsListingQuantity;

pub trait AsCoreQuantityPrice {
    fn as_core_qp(&self) -> RadrootsCoreQuantityPrice;
}

pub trait ListingPricingExt {
    fn subtotal_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingSubtotal;
    fn total_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingTotal;
}

pub trait ListingPricingTryExt {
    fn try_subtotal_for(
        &self,
        qty: &RadrootsListingQuantity,
    ) -> Result<RadrootsTradeListingSubtotal, RadrootsCoreQuantityPriceError>;
    fn try_total_for(
        &self,
        qty: &RadrootsListingQuantity,
    ) -> Result<RadrootsTradeListingTotal, RadrootsCoreQuantityPriceError>;
}

#[inline]
fn effective_quantity(qty: &RadrootsListingQuantity) -> RadrootsCoreQuantity {
    let count = qty.count.unwrap_or(1);
    let amount = qty.value.amount * RadrootsCoreDecimal::from(count);
    RadrootsCoreQuantity::new(amount, qty.value.unit)
}

impl ListingPricingExt for RadrootsCoreQuantityPrice {
    fn subtotal_for(&self, qty: &RadrootsListingQuantity) -> RadrootsTradeListingSubtotal {
        let effective_qty = effective_quantity(qty);
        let money = self.cost_for_rounded(&effective_qty);
        let currency = money.currency;

        RadrootsTradeListingSubtotal {
            price_amount: money,
            price_currency: currency,
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

impl ListingPricingTryExt for RadrootsCoreQuantityPrice {
    fn try_subtotal_for(
        &self,
        qty: &RadrootsListingQuantity,
    ) -> Result<RadrootsTradeListingSubtotal, RadrootsCoreQuantityPriceError> {
        let effective_qty = effective_quantity(qty);
        let money = self.try_cost_for_rounded(&effective_qty)?;
        let currency = money.currency;

        Ok(RadrootsTradeListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
        })
    }

    fn try_total_for(
        &self,
        qty: &RadrootsListingQuantity,
    ) -> Result<RadrootsTradeListingTotal, RadrootsCoreQuantityPriceError> {
        let sub = self.try_subtotal_for(qty)?;
        Ok(RadrootsTradeListingTotal {
            price_amount: sub.price_amount,
            price_currency: sub.price_currency,
            quantity_amount: sub.quantity_amount,
            quantity_unit: sub.quantity_unit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ListingPricingTryExt;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreUnit,
    };
    use radroots_events::listing::RadrootsListingQuantity;

    #[test]
    fn try_subtotal_for_rejects_unit_mismatch() {
        let price = RadrootsCoreQuantityPrice::new(
            RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
            RadrootsCoreQuantity::new(RadrootsCoreDecimal::from(1u32), RadrootsCoreUnit::Each),
        );

        let qty = RadrootsListingQuantity {
            value: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1u32),
                RadrootsCoreUnit::MassG,
            ),
            label: None,
            count: None,
        };

        let err = price.try_subtotal_for(&qty).unwrap_err();
        assert_eq!(
            err,
            RadrootsCoreQuantityPriceError::UnitMismatch {
                have: RadrootsCoreUnit::MassG,
                want: RadrootsCoreUnit::Each,
            }
        );
    }
}
