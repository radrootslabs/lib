use crate::{
    DiscountBps, Money, MoneyError, PositiveDecimal, PricingError, Quantity,
    listing::MeasurementUnit, pricing::net_line_minor,
};
use core::fmt;

/// Bounded measurement-price data, without agreement installation or stock.
/// The enclosing offer or agreement determines whether bounds describe one
/// sale unit or one complete line; this value performs no implicit scaling.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct VariableWeight {
    measurement_unit: MeasurementUnit,
    minimum: PositiveDecimal,
    maximum: PositiveDecimal,
    rate: Money,
    basis: PositiveDecimal,
    maximum_total: Money,
}

impl VariableWeight {
    /// Checks inclusive positive bounds, count units, currency and a feasible cap.
    /// The cap may restrict the range's more expensive measurements; the minimum
    /// must remain admissible. Zero rate/cap are explicit, not quote-required.
    pub fn new(
        measurement_unit: MeasurementUnit,
        minimum: PositiveDecimal,
        maximum: PositiveDecimal,
        rate: Money,
        basis: PositiveDecimal,
        maximum_total: Money,
    ) -> Result<Self, PricingError> {
        if minimum > maximum
            || Quantity::new(minimum.amount(), measurement_unit).is_err()
            || Quantity::new(maximum.amount(), measurement_unit).is_err()
        {
            return Err(PricingError::InvalidMeasurementRange);
        }
        if rate.currency() != maximum_total.currency() {
            return Err(MoneyError::CurrencyMismatch.into());
        }
        let value = Self {
            measurement_unit,
            minimum,
            maximum,
            rate,
            basis,
            maximum_total,
        };
        value.total(minimum)?;
        Ok(value)
    }

    /// Returns the exact authorized measurement identity.
    pub const fn measurement_unit(self) -> MeasurementUnit {
        self.measurement_unit
    }
    /// Returns the inclusive positive lower bound.
    pub const fn minimum(self) -> PositiveDecimal {
        self.minimum
    }
    /// Returns the inclusive positive upper bound.
    pub const fn maximum(self) -> PositiveDecimal {
        self.maximum
    }
    /// Returns the exact currency/rate used for final measurement pricing.
    pub const fn rate(self) -> Money {
        self.rate
    }
    /// Returns the strictly positive measurement price basis.
    pub const fn basis(self) -> PositiveDecimal {
        self.basis
    }
    /// Returns the explicit monetary authorization ceiling.
    pub const fn maximum_total(self) -> Money {
        self.maximum_total
    }

    /// Prices an exact same-unit in-range measurement and enforces the cap.
    /// No implicit conversion, discount, amendment or physical consumption occurs.
    pub fn total_at(self, measurement: Quantity) -> Result<Money, PricingError> {
        if measurement.unit() != self.measurement_unit {
            return Err(PricingError::MeasurementUnitMismatch);
        }
        if measurement.amount() < self.minimum.amount()
            || measurement.amount() > self.maximum.amount()
        {
            return Err(PricingError::MeasurementOutOfRange);
        }
        let amount = PositiveDecimal::new(measurement.amount())
            .map_err(|_| PricingError::MeasurementOutOfRange)?;
        self.total(amount)
    }

    fn total(self, amount: PositiveDecimal) -> Result<Money, PricingError> {
        let minor = net_line_minor(self.rate.minor(), amount, self.basis, DiscountBps::NONE)?;
        if minor > self.maximum_total.minor() {
            return Err(PricingError::CapExceeded);
        }
        Ok(Money::new(self.rate.currency(), minor))
    }
}

impl fmt::Debug for VariableWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VariableWeight(<redacted>)")
    }
}
