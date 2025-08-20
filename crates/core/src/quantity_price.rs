use crate::{RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity, RadrootsCoreUnit};

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCoreQuantityPrice {
    #[cfg_attr(feature = "serde", serde(alias = "money", alias = "price"))]
    pub amount: RadrootsCoreMoney,
    #[cfg_attr(feature = "serde", serde(alias = "per", alias = "quantity"))]
    pub quantity: RadrootsCoreQuantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreQuantityPriceError {
    PerQuantityZero,
    UnitMismatch {
        have: RadrootsCoreUnit,
        want: RadrootsCoreUnit,
    },
    NonConvertibleUnits {
        from: RadrootsCoreUnit,
        to: RadrootsCoreUnit,
    },
}

pub trait RadrootsCoreQuantityPriceOps {
    #[must_use]
    fn cost_for(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney;

    #[must_use]
    fn cost_for_rounded(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney;

    #[must_use]
    fn cost_for_with_quantized_price(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney;

    fn try_cost_for(
        &self,
        qty: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreMoney, RadrootsCoreQuantityPriceError>;

    fn try_cost_for_rounded(
        &self,
        qty: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreMoney, RadrootsCoreQuantityPriceError>;
}

impl RadrootsCoreQuantityPrice {
    #[inline]
    pub fn new(amount: RadrootsCoreMoney, quantity: RadrootsCoreQuantity) -> Self {
        Self { amount, quantity }
    }

    #[inline]
    pub fn try_cost_for_amount_in(
        &self,
        amount: RadrootsCoreDecimal,
        unit: RadrootsCoreUnit,
    ) -> Result<RadrootsCoreMoney, RadrootsCoreQuantityPriceError> {
        use crate::unit::convert_mass_decimal;

        let target = self.quantity.unit;

        let normalized = if unit == target {
            amount
        } else if unit.is_mass() && target.is_mass() {
            convert_mass_decimal(amount, unit, target)
        } else {
            return Err(RadrootsCoreQuantityPriceError::NonConvertibleUnits {
                from: unit,
                to: target,
            });
        };

        let qty = RadrootsCoreQuantity::new(normalized, target);
        self.try_cost_for_rounded(&qty)
    }
}

impl RadrootsCoreQuantityPriceOps for RadrootsCoreQuantityPrice {
    #[inline]
    fn cost_for(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney {
        if qty.amount.is_zero() {
            return RadrootsCoreMoney::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return RadrootsCoreMoney::zero(self.amount.currency);
        }

        let ratio = qty.amount / self.quantity.amount;
        self.amount.mul_decimal(ratio)
    }

    #[inline]
    fn cost_for_rounded(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney {
        self.cost_for(qty).quantize_to_currency()
    }

    #[inline]
    fn cost_for_with_quantized_price(&self, qty: &RadrootsCoreQuantity) -> RadrootsCoreMoney {
        if qty.amount.is_zero() {
            return RadrootsCoreMoney::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return RadrootsCoreMoney::zero(self.amount.currency);
        }
        let unit_price_q = self.amount.clone().quantize_to_currency();
        unit_price_q.mul_decimal(qty.amount / self.quantity.amount)
    }

    #[inline]
    fn try_cost_for(
        &self,
        qty: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreMoney, RadrootsCoreQuantityPriceError> {
        if self.quantity.amount.is_zero() {
            return Err(RadrootsCoreQuantityPriceError::PerQuantityZero);
        }
        if qty.unit != self.quantity.unit {
            return Err(RadrootsCoreQuantityPriceError::UnitMismatch {
                have: qty.unit,
                want: self.quantity.unit,
            });
        }
        let ratio = qty.amount / self.quantity.amount;
        Ok(self.amount.mul_decimal(ratio))
    }

    #[inline]
    fn try_cost_for_rounded(
        &self,
        qty: &RadrootsCoreQuantity,
    ) -> Result<RadrootsCoreMoney, RadrootsCoreQuantityPriceError> {
        Ok(self.try_cost_for(qty)?.quantize_to_currency())
    }
}
