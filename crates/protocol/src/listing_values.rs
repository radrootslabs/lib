//! Shared public snapshot decoding. Private Terms never enter these JSON trees.
use crate::command::CommandError;
#[cfg(feature = "terms")]
use crate::command::parse_object;
use crate::command::required_nullable;
use alloc::{string::String, vec::Vec};
use radroots_domain::{
    ContentMeasure, Decimal, DiscountBps, FixedPrice, ListingOptionId, Money, PositiveDecimal,
    PriceTier, PublicKey, Quantity, TimeZoneId, UnixSeconds, VariableWeight, Window, listing::*,
};
use serde::Deserialize;
#[cfg(feature = "terms")]
use serde_json::value::RawValue;
fn invalid<T, E>(value: Result<T, E>) -> Result<T, CommandError> {
    value.map_err(|_| CommandError::InvalidInput)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProduct {
    name: String,
    description: String,
    #[serde(deserialize_with = "crate::command::nullable_canonical_key")]
    producer: Option<PublicKey>,
    #[serde(deserialize_with = "required_nullable")]
    origin_summary: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OptionInput {
    id: ListingOptionId,
    label: String,
    unit: Unit,
    increment: PositiveDecimal,
    minimum: PositiveDecimal,
    price: RawPrice,
    supply: RawSupply,
    lead_seconds: LeadSeconds,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Unit {
    Measured { unit: Measure },
    Package { label: String, contents: Contents },
}
#[derive(Deserialize)]
enum Measure {
    #[serde(rename = "g")]
    Gram,
    #[serde(rename = "kg")]
    Kilogram,
    #[serde(rename = "mL")]
    Millilitre,
    #[serde(rename = "L")]
    Litre,
    #[serde(rename = "each")]
    Each,
}
impl Measure {
    fn domain(self) -> MeasurementUnit {
        match self {
            Self::Gram => MeasurementUnit::Gram,
            Self::Kilogram => MeasurementUnit::Kilogram,
            Self::Millilitre => MeasurementUnit::Millilitre,
            Self::Litre => MeasurementUnit::Litre,
            Self::Each => MeasurementUnit::Each,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuantity {
    amount: Decimal,
    unit: Measure,
}
impl RawQuantity {
    fn domain(self) -> Result<Quantity, CommandError> {
        invalid(Quantity::new(self.amount, self.unit.domain()))
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Contents {
    Fixed { quantity: RawQuantity },
    Estimated { quantity: RawQuantity },
    Unspecified {},
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawPrice {
    Fixed {
        rate: Money,
        basis: PositiveDecimal,
        discount_bps: u16,
        tiers: Vec<RawTier>,
    },
    QuoteRequired {},
    VariableWeight {
        measurement_unit: Measure,
        minimum_per_sale_unit: PositiveDecimal,
        maximum_per_sale_unit: PositiveDecimal,
        rate: Money,
        basis: PositiveDecimal,
        maximum_per_sale_unit_total: Money,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier {
    from: Decimal,
    rate: Money,
}
impl RawPrice {
    fn domain(self) -> Result<Price, CommandError> {
        match self {
            Self::QuoteRequired {} => Ok(Price::QuoteRequired),
            Self::Fixed {
                rate,
                basis,
                discount_bps,
                tiers,
            } => Ok(Price::Fixed(invalid(FixedPrice::new(
                rate,
                basis,
                invalid(DiscountBps::new(discount_bps))?,
                tiers
                    .into_iter()
                    .map(|tier| PriceTier::new(tier.from, tier.rate))
                    .collect(),
            ))?)),
            Self::VariableWeight {
                measurement_unit,
                minimum_per_sale_unit,
                maximum_per_sale_unit,
                rate,
                basis,
                maximum_per_sale_unit_total,
            } => Ok(Price::VariableWeight(invalid(VariableWeight::new(
                measurement_unit.domain(),
                minimum_per_sale_unit,
                maximum_per_sale_unit,
                rate,
                basis,
                maximum_per_sale_unit_total,
            ))?)),
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawSupply {
    Unknown {},
    Available {
        amount: Decimal,
    },
    Estimated {
        amount: Decimal,
        #[serde(deserialize_with = "required_nullable")]
        window: Option<RawWindow>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWindow {
    start: UnixSeconds,
    end: UnixSeconds,
    timezone: String,
}
impl RawWindow {
    fn domain(self) -> Result<Window, CommandError> {
        invalid(Window::new(
            self.start,
            self.end,
            invalid(TimeZoneId::from_name(&self.timezone))?,
        ))
    }
}
impl RawProduct {
    pub(crate) fn domain(self) -> Result<Product, CommandError> {
        invalid(Product::new(
            &self.name,
            &self.description,
            self.producer,
            self.origin_summary.as_deref(),
        ))
    }
}
impl OptionInput {
    pub(crate) fn domain(self) -> Result<ListingOption, CommandError> {
        let unit = match self.unit {
            Unit::Measured { unit } => SaleUnit::measured(unit.domain()),
            Unit::Package { label, contents } => {
                let contents = match contents {
                    Contents::Fixed { quantity } => ContentMeasure::Fixed {
                        quantity: quantity.domain()?,
                    },
                    Contents::Estimated { quantity } => ContentMeasure::Estimated {
                        quantity: quantity.domain()?,
                    },
                    Contents::Unspecified {} => ContentMeasure::Unspecified,
                };
                invalid(SaleUnit::package(&label, contents))?
            }
        };
        invalid(ListingOption::new(
            self.id,
            &self.label,
            unit,
            self.increment,
            self.minimum,
            self.price.domain()?,
            match self.supply {
                RawSupply::Unknown {} => Supply::Unknown,
                RawSupply::Available { amount } => Supply::Available { amount },
                RawSupply::Estimated { amount, window } => Supply::Estimated {
                    amount,
                    window: window.map(RawWindow::domain).transpose()?,
                },
            },
            self.lead_seconds,
        ))
    }
}
pub(crate) fn object(value: &serde_json::Value) -> Result<(), CommandError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(CommandError::InvalidInput)
    }
}
pub(crate) fn option_shape(option: &serde_json::Value) -> Result<(), CommandError> {
    object(option)?;
    object(&option["unit"])?;
    object(&option["price"])?;
    object(&option["supply"])?;
    if let Some(window) = option["supply"].get("window")
        && !window.is_null()
    {
        object(window)?;
    }
    if let Some(contents) = option["unit"].get("contents") {
        object(contents)?;
        if let Some(quantity) = contents.get("quantity") {
            object(quantity)?;
        }
    }
    if let Some(tiers) = option["price"].get("tiers") {
        let tiers = tiers.as_array().ok_or(CommandError::InvalidInput)?;
        if tiers.len() > FixedPrice::MAX_TIERS {
            return Err(CommandError::LimitExceeded);
        }
        for tier in tiers {
            object(tier)?;
        }
    }
    Ok(())
}

#[cfg(feature = "terms")]
pub(crate) fn product(raw: &RawValue) -> Result<Product, CommandError> {
    parse_object::<RawProduct>(raw.get().as_bytes())?.domain()
}
#[cfg(feature = "terms")]
pub(crate) fn option(raw: &RawValue) -> Result<ListingOption, CommandError> {
    let value: serde_json::Value =
        serde_json::from_str(raw.get()).map_err(|_| CommandError::InvalidInput)?;
    option_shape(&value)?;
    for (path, supported) in [
        (
            "/price/type",
            &["quote_required", "fixed", "variable_weight"][..],
        ),
        ("/supply/type", &["unknown", "available", "estimated"][..]),
        ("/unit/type", &["measured", "package"][..]),
        (
            "/unit/contents/type",
            &["unspecified", "fixed", "estimated"][..],
        ),
    ] {
        if value
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|tag| !supported.contains(&tag))
        {
            return Err(CommandError::UnsupportedProfile);
        }
    }
    for path in [
        "/unit/unit",
        "/unit/contents/quantity/unit",
        "/price/measurement_unit",
    ] {
        if value
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|unit| !["g", "kg", "mL", "L", "each"].contains(&unit))
        {
            return Err(CommandError::UnsupportedProfile);
        }
    }
    parse_object::<OptionInput>(raw.get().as_bytes())?.domain()
}
