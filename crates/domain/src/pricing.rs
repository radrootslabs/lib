use crate::{Decimal, MinorAmount, Money, MoneyError, PositiveDecimal};
use alloc::{boxed::Box, vec::Vec};
use core::fmt;

/// Fixed calculation rule: discount first, half-even once per net line.
pub const PRICING_PROFILE: &str = "half_even_net_line_v1";
/// Largest supported number of rounded lines in one aggregate.
pub const MAX_PRICED_LINES: usize = 32;

/// Validated discount in integer basis points, without an implicit default.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct DiscountBps(u16);

impl DiscountBps {
    /// Explicit absence of a discount.
    pub const NONE: Self = Self(0);
    /// Explicit full discount; the resulting price is free, not quote-required.
    pub const FULL: Self = Self(10_000);

    /// Admits exactly 0 through 10000 basis points.
    pub const fn new(value: u16) -> Result<Self, PricingError> {
        if value > 10_000 {
            Err(PricingError::InvalidDiscount)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the exact bounded integer basis points.
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// One threshold/rate pair, expressed in its enclosing price's sale-unit basis.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PriceTier {
    from: Decimal,
    rate: Money,
}

impl PriceTier {
    /// Captures validated values; the enclosing price checks order and currency.
    pub const fn new(from: Decimal, rate: Money) -> Self {
        Self { from, rate }
    }
    /// Returns the inclusive threshold; an explicit zero threshold is valid.
    pub const fn from(self) -> Decimal {
        self.from
    }
    /// Returns the exact tier rate, without currency conversion.
    pub const fn rate(self) -> Money {
        self.rate
    }
}

/// Validated fixed-price calculation inputs for one selected sale unit.
///
/// Amounts, basis and thresholds share that sale unit. This type performs no
/// physical conversion, option/increment admission, agreement installation or
/// stock operation. JSON serialization alone does not create a listing profile.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FixedPrice {
    rate: Money,
    basis: PositiveDecimal,
    discount_bps: DiscountBps,
    tiers: Box<[PriceTier]>,
}

impl FixedPrice {
    /// Maximum number of explicitly ordered price tiers.
    pub const MAX_TIERS: usize = 8;

    /// Checks tier count, strict threshold order and exact shared currency.
    pub fn new(
        rate: Money,
        basis: PositiveDecimal,
        discount_bps: DiscountBps,
        tiers: Vec<PriceTier>,
    ) -> Result<Self, PricingError> {
        if tiers.len() > Self::MAX_TIERS {
            return Err(PricingError::TooManyTiers);
        }
        let mut previous = None;
        for tier in &tiers {
            if tier.rate.currency() != rate.currency() {
                return Err(MoneyError::CurrencyMismatch.into());
            }
            if previous.is_some_and(|value| tier.from <= value) {
                return Err(PricingError::InvalidTierOrder);
            }
            previous = Some(tier.from);
        }
        Ok(Self {
            rate,
            basis,
            discount_bps,
            tiers: tiers.into_boxed_slice(),
        })
    }

    /// Returns the base rate, used below the first applicable tier.
    pub const fn rate(&self) -> Money {
        self.rate
    }
    /// Returns the strictly positive common price basis.
    pub const fn basis(&self) -> PositiveDecimal {
        self.basis
    }
    /// Returns the single discount applied before line rounding.
    pub const fn discount_bps(&self) -> DiscountBps {
        self.discount_bps
    }
    /// Borrows the exact, strictly increasing tier sequence.
    pub fn tiers(&self) -> &[PriceTier] {
        &self.tiers
    }

    /// Resolves one tier and computes one exact discounted, half-even net line.
    pub fn line(&self, amount: PositiveDecimal) -> Result<LinePrice, PricingError> {
        let selected = self
            .tiers
            .iter()
            .rev()
            .find(|tier| tier.from <= amount.amount());
        let rate = selected.map_or(self.rate, |tier| tier.rate);
        let total = Money::new(
            rate.currency(),
            net_line_minor(rate.minor(), amount, self.basis, self.discount_bps)?,
        );
        Ok(LinePrice {
            rate,
            basis: self.basis,
            discount_bps: self.discount_bps,
            selected_tier_from: selected.map(|tier| tier.from),
            total,
        })
    }
}

/// Resolved numeric calculation, without authenticated terms or installation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct LinePrice {
    rate: Money,
    basis: PositiveDecimal,
    discount_bps: DiscountBps,
    selected_tier_from: Option<Decimal>,
    total: Money,
}

impl LinePrice {
    /// Returns the base or selected tier rate actually used.
    pub const fn rate(self) -> Money {
        self.rate
    }
    /// Returns the exact common basis used for this line.
    pub const fn basis(self) -> PositiveDecimal {
        self.basis
    }
    /// Returns the single applied discount.
    pub const fn discount_bps(self) -> DiscountBps {
        self.discount_bps
    }
    /// Returns the selected inclusive threshold, or None for the base rate.
    pub const fn selected_tier_from(self) -> Option<Decimal> {
        self.selected_tier_from
    }
    /// Returns the once-rounded minor amount within the supported money cap.
    pub const fn total(self) -> Money {
        self.total
    }
}

/// Sums one through 32 already-rounded net lines and an explicit fulfillment fee.
/// All operands must share one currency. Received totals still require the
/// enclosing agreement's independent recomputation and admission.
pub fn checked_order_total(
    lines: &[Money],
    fulfillment_charge: Money,
) -> Result<Money, PricingError> {
    if lines.is_empty() || lines.len() > MAX_PRICED_LINES {
        return Err(PricingError::InvalidLineCount);
    }
    let mut total = fulfillment_charge;
    for line in lines {
        total = total.checked_add(*line)?;
    }
    Ok(total)
}

pub(crate) fn net_line_minor(
    rate: MinorAmount,
    amount: PositiveDecimal,
    basis: PositiveDecimal,
    discount: DiscountBps,
) -> Result<MinorAmount, PricingError> {
    let amount = amount.amount();
    let basis = basis.amount();
    let scale = |digits: u8| {
        10_i128
            .checked_pow(u32::from(digits))
            .ok_or(PricingError::ArithmeticOutOfRange)
    };
    // Validated factors bound the numerator below 10^37 and denominator below
    // 10^22, inside signed i128. Keep every operation checked independently.
    let basis_scale = scale(basis.scale())?;
    let amount_scale = scale(amount.scale())?;
    let numerator = i128::from(rate.get())
        .checked_mul(i128::from(amount.coefficient()))
        .and_then(|value| value.checked_mul(basis_scale))
        .and_then(|value| value.checked_mul(i128::from(10_000 - discount.get())))
        .ok_or(PricingError::ArithmeticOutOfRange)?;
    let denominator = i128::from(basis.coefficient())
        .checked_mul(amount_scale)
        .and_then(|value| value.checked_mul(10_000))
        .ok_or(PricingError::ArithmeticOutOfRange)?;
    let quotient = numerator
        .checked_div(denominator)
        .ok_or(PricingError::ArithmeticOutOfRange)?;
    let twice_remainder = numerator
        .checked_rem(denominator)
        .and_then(|value| value.checked_mul(2))
        .ok_or(PricingError::ArithmeticOutOfRange)?;
    let odd = quotient
        .checked_rem(2)
        .ok_or(PricingError::ArithmeticOutOfRange)?
        == 1;
    let increment = twice_remainder > denominator || (twice_remainder == denominator && odd);
    let rounded = quotient
        .checked_add(i128::from(increment))
        .ok_or(PricingError::ArithmeticOutOfRange)?;
    let rounded = u64::try_from(rounded).map_err(|_| MoneyError::MinorOutOfRange)?;
    MinorAmount::new(rounded).map_err(Into::into)
}

/// Input-free fixed-price admission or exact-arithmetic failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PricingError {
    /// Basis points exceed the inclusive 0-10000 bound.
    InvalidDiscount,
    /// More than eight tiers were supplied.
    TooManyTiers,
    /// Thresholds are duplicate or are not strictly increasing.
    InvalidTierOrder,
    /// An aggregate has no lines or exceeds 32 lines.
    InvalidLineCount,
    /// Measurement bounds are inverted or contain fractional each counts.
    InvalidMeasurementRange,
    /// The final measurement uses a different explicit unit.
    MeasurementUnitMismatch,
    /// A final measurement is outside the authorized inclusive range.
    MeasurementOutOfRange,
    /// The exact recomputed price exceeds the explicit monetary cap.
    CapExceeded,
    /// A checked intermediate operation could not be represented.
    ArithmeticOutOfRange,
    /// The monetary bound or exact currency invariant was violated.
    Money(MoneyError),
}

impl From<MoneyError> for PricingError {
    fn from(value: MoneyError) -> Self {
        Self::Money(value)
    }
}

impl fmt::Display for PricingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDiscount => "invalid discount basis points",
            Self::TooManyTiers => "too many price tiers",
            Self::InvalidTierOrder => "price thresholds must strictly increase",
            Self::InvalidLineCount => "invalid priced line count",
            Self::InvalidMeasurementRange => "invalid measurement range",
            Self::MeasurementUnitMismatch => "measurement unit differs from authorized unit",
            Self::MeasurementOutOfRange => "measurement is outside authorized range",
            Self::CapExceeded => "measurement price exceeds authorized cap",
            Self::ArithmeticOutOfRange => "pricing arithmetic exceeds supported bounds",
            Self::Money(error) => return error.fmt(f),
        })
    }
}

impl core::error::Error for PricingError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Money(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Debug for DiscountBps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DiscountBps(<redacted>)")
    }
}
impl fmt::Debug for PriceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PriceTier(<redacted>)")
    }
}
impl fmt::Debug for FixedPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FixedPrice(<redacted>)")
    }
}
impl fmt::Debug for LinePrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LinePrice(<redacted>)")
    }
}
