use crate::operational_listing::model::{
    RadrootsOperationalListingSubtotal, RadrootsOperationalListingTotal,
};
use radroots_core::pricing::{Error as PricingError, QuantityPriceOps};
use radroots_core::{Decimal, Quantity};
use radroots_event::operational_listing::RadrootsOperationalListingBin;

pub trait BinPricingExt {
    fn subtotal_for_count(&self, bin_count: u32) -> RadrootsOperationalListingSubtotal;
    fn total_for_count(&self, bin_count: u32) -> RadrootsOperationalListingTotal;
}

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
fn effective_quantity(bin: &RadrootsOperationalListingBin, bin_count: u32) -> Quantity {
    let amount = bin.quantity.amount * Decimal::from(bin_count);
    Quantity::new(amount, bin.quantity.unit)
}

impl BinPricingExt for RadrootsOperationalListingBin {
    fn subtotal_for_count(&self, bin_count: u32) -> RadrootsOperationalListingSubtotal {
        let effective_qty = effective_quantity(self, bin_count);
        let money = self
            .price_per_canonical_unit
            .cost_for_rounded(&effective_qty);
        let currency = money.currency;

        RadrootsOperationalListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
        }
    }

    fn total_for_count(&self, bin_count: u32) -> RadrootsOperationalListingTotal {
        let sub = self.subtotal_for_count(bin_count);
        RadrootsOperationalListingTotal {
            price_amount: sub.price_amount,
            price_currency: sub.price_currency,
            quantity_amount: sub.quantity_amount,
            quantity_unit: sub.quantity_unit,
        }
    }
}

impl BinPricingTryExt for RadrootsOperationalListingBin {
    fn try_subtotal_for_count(
        &self,
        bin_count: u32,
    ) -> Result<RadrootsOperationalListingSubtotal, PricingError> {
        let effective_qty = effective_quantity(self, bin_count);
        let money = self
            .price_per_canonical_unit
            .try_cost_for_rounded(&effective_qty)?;
        let currency = money.currency;

        Ok(RadrootsOperationalListingSubtotal {
            price_amount: money,
            price_currency: currency,
            quantity_amount: effective_qty.amount,
            quantity_unit: effective_qty.unit,
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

#[cfg(test)]
mod tests {
    use super::{BinPricingExt, BinPricingTryExt};
    use radroots_core::pricing::Error as PricingError;
    use radroots_core::{Currency, Decimal, Money, Quantity, QuantityPrice, Unit};
    use radroots_event::ids::RadrootsInventoryBinId;
    use radroots_event::operational_listing::RadrootsOperationalListingBin;

    fn bin_id(raw: &str) -> RadrootsInventoryBinId {
        RadrootsInventoryBinId::parse(raw).expect("bin id")
    }

    fn valid_bin() -> RadrootsOperationalListingBin {
        RadrootsOperationalListingBin {
            bin_id: bin_id("bin-1"),
            quantity: Quantity::new(Decimal::from(2u32), Unit::MassG),
            price_per_canonical_unit: QuantityPrice::new(
                Money::new(Decimal::from(5u32), Currency::USD),
                Quantity::new(Decimal::from(1u32), Unit::MassG),
            ),
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }
    }

    #[test]
    fn try_subtotal_for_rejects_unit_mismatch() {
        let bin = RadrootsOperationalListingBin {
            bin_id: bin_id("bin-1"),
            quantity: Quantity::new(Decimal::from(1u32), Unit::MassG),
            price_per_canonical_unit: QuantityPrice::new(
                Money::new(Decimal::from(10u32), Currency::USD),
                Quantity::new(Decimal::from(1u32), Unit::Each),
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
            PricingError::UnitMismatch {
                have: Unit::MassG,
                want: Unit::Each,
            }
        );
    }

    #[test]
    fn subtotal_and_total_for_count_follow_effective_quantity() {
        let bin = valid_bin();
        let subtotal = bin.subtotal_for_count(3);
        let total = bin.total_for_count(3);

        assert_eq!(subtotal.quantity_amount, Decimal::from(6u32));
        assert_eq!(subtotal.quantity_unit, Unit::MassG);
        assert_eq!(subtotal.price_amount.amount, Decimal::from(30u32));
        assert_eq!(subtotal.price_currency, Currency::USD);

        assert_eq!(total.quantity_amount, subtotal.quantity_amount);
        assert_eq!(total.quantity_unit, subtotal.quantity_unit);
        assert_eq!(total.price_amount, subtotal.price_amount);
        assert_eq!(total.price_currency, subtotal.price_currency);
    }

    #[test]
    fn try_subtotal_and_try_total_match_non_fallible_paths() {
        let bin = valid_bin();
        let subtotal = bin.try_subtotal_for_count(4).expect("subtotal");
        let total = bin.try_total_for_count(4).expect("total");

        assert_eq!(subtotal.quantity_amount, Decimal::from(8u32));
        assert_eq!(subtotal.price_amount.amount, Decimal::from(40u32));
        assert_eq!(total.quantity_amount, subtotal.quantity_amount);
        assert_eq!(total.price_amount, subtotal.price_amount);
    }

    #[test]
    fn try_total_for_count_propagates_subtotal_errors() {
        let bin = RadrootsOperationalListingBin {
            bin_id: bin_id("bin-1"),
            quantity: Quantity::new(Decimal::from(1u32), Unit::MassG),
            price_per_canonical_unit: QuantityPrice::new(
                Money::new(Decimal::from(10u32), Currency::USD),
                Quantity::new(Decimal::from(1u32), Unit::Each),
            ),
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        };

        let err = bin.try_total_for_count(1).unwrap_err();
        assert_eq!(
            err,
            PricingError::UnitMismatch {
                have: Unit::MassG,
                want: Unit::Each,
            }
        );
    }
}
