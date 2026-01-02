use crate::listing::model::{RadrootsTradeListingSubtotal, RadrootsTradeListingTotal};
use radroots_core::{
    RadrootsCoreDecimal, RadrootsCoreQuantity, RadrootsCoreQuantityPriceError,
    RadrootsCoreQuantityPriceOps,
};
use radroots_events::listing::RadrootsListingBin;

pub trait BinPricingExt {
    fn subtotal_for_count(&self, bin_count: u32) -> RadrootsTradeListingSubtotal;
    fn total_for_count(&self, bin_count: u32) -> RadrootsTradeListingTotal;
}

pub trait BinPricingTryExt {
    fn try_subtotal_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsTradeListingSubtotal, RadrootsCoreQuantityPriceError>;
    fn try_total_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsTradeListingTotal, RadrootsCoreQuantityPriceError>;
}

#[inline]
fn effective_quantity(bin: &RadrootsListingBin, bin_count: u32) -> RadrootsCoreQuantity {
    let amount = bin.quantity.amount * RadrootsCoreDecimal::from(bin_count);
    RadrootsCoreQuantity::new(amount, bin.quantity.unit)
}

impl BinPricingExt for RadrootsListingBin {
    fn subtotal_for_count(&self, bin_count: u32) -> RadrootsTradeListingSubtotal {
        let effective_qty = effective_quantity(self, bin_count);
        let money = self
            .price_per_canonical_unit
            .cost_for_rounded(&effective_qty);
        let currency = money.currency;

        RadrootsTradeListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
        }
    }

    fn total_for_count(&self, bin_count: u32) -> RadrootsTradeListingTotal {
        let sub = self.subtotal_for_count(bin_count);
        RadrootsTradeListingTotal {
            price_amount: sub.price_amount,
            price_currency: sub.price_currency,
            quantity_amount: sub.quantity_amount,
            quantity_unit: sub.quantity_unit,
        }
    }
}

impl BinPricingTryExt for RadrootsListingBin {
    fn try_subtotal_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsTradeListingSubtotal, RadrootsCoreQuantityPriceError> {
        let effective_qty = effective_quantity(self, bin_count);
        let money = self
            .price_per_canonical_unit
            .try_cost_for_rounded(&effective_qty)?;
        let currency = money.currency;

        Ok(RadrootsTradeListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
        })
    }

    fn try_total_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsTradeListingTotal, RadrootsCoreQuantityPriceError> {
        let sub = self.try_subtotal_for_count(bin_count)?;
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
    use super::BinPricingTryExt;
    use radroots_core::{
        RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
        RadrootsCoreQuantityPrice, RadrootsCoreQuantityPriceError, RadrootsCoreUnit,
    };
    use radroots_events::listing::RadrootsListingBin;

    #[test]
    fn try_subtotal_for_rejects_unit_mismatch() {
        let bin = RadrootsListingBin {
            bin_id: "bin-1".into(),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice::new(
                RadrootsCoreMoney::new(RadrootsCoreDecimal::from(10u32), RadrootsCoreCurrency::USD),
                RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::Each,
                ),
            ),
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        };

        let err = bin.try_subtotal_for_count(1).unwrap_err();
        assert_eq!(
            err,
            RadrootsCoreQuantityPriceError::UnitMismatch {
                have: RadrootsCoreUnit::MassG,
                want: RadrootsCoreUnit::Each,
            }
        );
    }
}
