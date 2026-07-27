use crate::{Decimal, Money, Quantity, Unit};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityPrice {
    pub amount: Money,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsCoreQuantityPriceError {
    PerQuantityZero,
    UnitMismatch { have: Unit, want: Unit },
    NonConvertibleUnits { from: Unit, to: Unit },
}

pub trait RadrootsCoreQuantityPriceOps {
    #[must_use]
    fn cost_for(&self, qty: &Quantity) -> Money;

    #[must_use]
    fn cost_for_rounded(&self, qty: &Quantity) -> Money;

    #[must_use]
    fn cost_for_with_quantized_price(&self, qty: &Quantity) -> Money;

    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, RadrootsCoreQuantityPriceError>;

    fn try_cost_for_rounded(&self, qty: &Quantity)
    -> Result<Money, RadrootsCoreQuantityPriceError>;
}

impl QuantityPrice {
    #[inline]
    pub fn new(amount: Money, quantity: Quantity) -> Self {
        Self { amount, quantity }
    }

    #[inline]
    pub fn try_cost_for_amount_in(
        &self,
        amount: Decimal,
        unit: Unit,
    ) -> Result<Money, RadrootsCoreQuantityPriceError> {
        use crate::unit::convert_unit_decimal;

        let target = self.quantity.unit;

        let normalized = if unit == target {
            amount
        } else {
            convert_unit_decimal(amount, unit, target).map_err(|_| {
                RadrootsCoreQuantityPriceError::NonConvertibleUnits {
                    from: unit,
                    to: target,
                }
            })?
        };

        let qty = Quantity::new(normalized, target);
        self.try_cost_for_rounded(&qty)
    }

    #[inline]
    pub fn try_cost_for_quantity_in(
        &self,
        qty: &Quantity,
    ) -> Result<Money, RadrootsCoreQuantityPriceError> {
        self.try_cost_for_amount_in(qty.amount, qty.unit)
    }

    #[inline]
    pub fn is_price_per_canonical_unit(&self) -> bool {
        self.quantity.unit == self.quantity.unit.canonical_unit()
            && self.quantity.amount == Decimal::ONE
    }

    #[inline]
    pub fn try_to_unit_price(
        &self,
        unit: Unit,
    ) -> Result<QuantityPrice, RadrootsCoreQuantityPriceError> {
        use crate::unit::convert_unit_decimal;

        if self.quantity.amount.is_zero() {
            return Err(RadrootsCoreQuantityPriceError::PerQuantityZero);
        }

        let normalized = if self.quantity.unit == unit {
            self.quantity.amount
        } else {
            convert_unit_decimal(self.quantity.amount, self.quantity.unit, unit).map_err(|_| {
                RadrootsCoreQuantityPriceError::NonConvertibleUnits {
                    from: self.quantity.unit,
                    to: unit,
                }
            })?
        };

        if normalized.is_zero() {
            return Err(RadrootsCoreQuantityPriceError::PerQuantityZero);
        }

        let amount = self.amount.div_decimal(normalized);
        Ok(QuantityPrice {
            amount,
            quantity: Quantity::new(Decimal::ONE, unit),
        })
    }

    #[inline]
    pub fn try_to_canonical_unit_price(
        &self,
    ) -> Result<QuantityPrice, RadrootsCoreQuantityPriceError> {
        self.try_to_unit_price(self.quantity.unit.canonical_unit())
    }
}

impl RadrootsCoreQuantityPriceOps for QuantityPrice {
    #[inline]
    fn cost_for(&self, qty: &Quantity) -> Money {
        if qty.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if qty.unit != self.quantity.unit {
            return Money::zero(self.amount.currency);
        }

        let ratio = qty.amount / self.quantity.amount;
        self.amount.mul_decimal(ratio)
    }

    #[inline]
    fn cost_for_rounded(&self, qty: &Quantity) -> Money {
        self.cost_for(qty).quantize_to_currency()
    }

    #[inline]
    fn cost_for_with_quantized_price(&self, qty: &Quantity) -> Money {
        if qty.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if self.quantity.amount.is_zero() {
            return Money::zero(self.amount.currency);
        }
        if qty.unit != self.quantity.unit {
            return Money::zero(self.amount.currency);
        }
        let unit_price_q = self.amount.clone().quantize_to_currency();
        unit_price_q.mul_decimal(qty.amount / self.quantity.amount)
    }

    #[inline]
    fn try_cost_for(&self, qty: &Quantity) -> Result<Money, RadrootsCoreQuantityPriceError> {
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
        qty: &Quantity,
    ) -> Result<Money, RadrootsCoreQuantityPriceError> {
        Ok(self.try_cost_for(qty)?.quantize_to_currency())
    }
}

#[deprecated(since = "0.1.0", note = "renamed to `QuantityPrice`")]
pub use self::QuantityPrice as RadrootsCoreQuantityPrice;
