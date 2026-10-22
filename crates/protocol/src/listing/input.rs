//! Closed known-profile input admission. Domain construction remains the sole
//! value invariant owner; no unknown field or unsupported feature is discarded.
use super::PublishInput;
use crate::command::{CommandError, check_frame, required_nullable};
use alloc::{string::String, vec::Vec};
use radroots_domain::{
    AuthorityRef, ContentMeasure, Decimal, DiscountBps, FixedPrice, ListingOptionId, Money,
    PositiveDecimal, PriceTier, PublicKey, Quantity, TimeZoneId, UnixSeconds, VariableWeight,
    Window, listing::*,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: ListingId,
    title: String,
    summary: String,
    description_markdown: String,
    published_at: UnixSeconds,
    profile: Profile,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    profile: String,
    version: u32,
    state: State,
    product: RawProduct,
    default_option: ListingOptionId,
    options: Vec<OptionInput>,
    fulfillment: Fulfillment,
    #[serde(deserialize_with = "required_nullable")]
    authority: Option<AuthorityRef>,
    media: Vec<RawMedia>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMedia {
    url: String,
    sha256: String,
    mime: String,
    alt: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Active,
    Paused,
    Withdrawn,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProduct {
    name: String,
    description: String,
    #[serde(deserialize_with = "required_nullable")]
    producer: Option<PublicKey>,
    #[serde(deserialize_with = "required_nullable")]
    origin_summary: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionInput {
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
    Unspecified,
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
    QuoteRequired,
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
            Self::QuoteRequired => Ok(Price::QuoteRequired),
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
    Unknown,
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
struct RawWindow {
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fulfillment {
    modes: Vec<Mode>,
    area: String,
    lead_seconds: LeadSeconds,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Pickup,
    Delivery,
}

fn invalid<T, E>(value: Result<T, E>) -> Result<T, CommandError> {
    value.map_err(|_| CommandError::InvalidInput)
}
impl Input {
    fn validate(self) -> Result<PublishInput, CommandError> {
        let profile = self.profile;
        if profile.profile != "radroots.listing" || profile.version != 1 {
            return Err(CommandError::UnsupportedProfile);
        }
        let product = invalid(Product::new(
            &profile.product.name,
            &profile.product.description,
            profile.product.producer,
            profile.product.origin_summary.as_deref(),
        ))?;
        if profile.options.is_empty() || profile.options.len() > 16 {
            return Err(CommandError::LimitExceeded);
        }
        let mut options = Vec::with_capacity(profile.options.len());
        for option in profile.options {
            let unit = match option.unit {
                Unit::Measured { unit } => SaleUnit::measured(unit.domain()),
                Unit::Package { label, contents } => {
                    let contents = match contents {
                        Contents::Fixed { quantity } => ContentMeasure::Fixed {
                            quantity: quantity.domain()?,
                        },
                        Contents::Estimated { quantity } => ContentMeasure::Estimated {
                            quantity: quantity.domain()?,
                        },
                        Contents::Unspecified => ContentMeasure::Unspecified,
                    };
                    invalid(SaleUnit::package(&label, contents))?
                }
            };
            options.push(invalid(ListingOption::new(
                option.id,
                &option.label,
                unit,
                option.increment,
                option.minimum,
                option.price.domain()?,
                match option.supply {
                    RawSupply::Unknown => Supply::Unknown,
                    RawSupply::Available { amount } => Supply::Available { amount },
                    RawSupply::Estimated { amount, window } => Supply::Estimated {
                        amount,
                        window: window.map(RawWindow::domain).transpose()?,
                    },
                },
                option.lead_seconds,
            ))?);
        }
        let fulfillment = invalid(PublicFulfillment::new(
            profile
                .fulfillment
                .modes
                .into_iter()
                .map(|mode| match mode {
                    Mode::Pickup => FulfillmentMode::Pickup,
                    Mode::Delivery => FulfillmentMode::Delivery,
                })
                .collect(),
            &profile.fulfillment.area,
            profile.fulfillment.lead_seconds,
        ))?;
        let terms = invalid(ListingTerms::new(
            match profile.state {
                State::Active => ListingState::Active,
                State::Paused => ListingState::Paused,
                State::Withdrawn => ListingState::Withdrawn,
            },
            product,
            profile.default_option,
            options,
            fulfillment,
            profile.authority,
            profile
                .media
                .into_iter()
                .map(|media| {
                    invalid(Media::from_hex_digest(
                        &media.url,
                        &media.sha256,
                        &media.mime,
                        &media.alt,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))?;
        PublishInput::new(
            self.id,
            &self.title,
            &self.summary,
            &self.description_markdown,
            self.published_at,
            terms,
        )
        .map_err(|_| CommandError::InvalidInput)
    }
}
impl PublishInput {
    /// Admits bounded closed sale-option input. Known unsupported features
    /// return an explicit refusal; they are never silently stripped or coerced.
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        // This bounded peek only classifies unimplemented features. Acceptance
        // still requires the independent closed parser, including duplicates,
        // required nulls and all nested domain constructors.
        let value: serde_json::Value =
            serde_json::from_slice(raw).map_err(|_| CommandError::InvalidInput)?;
        object_shapes(&value)?;
        if let Some(profile) = value.get("profile") {
            if profile
                .get("profile")
                .is_some_and(|v| v != "radroots.listing")
                || profile.get("version").is_some_and(|v| v != 1)
            {
                return Err(CommandError::UnsupportedProfile);
            }
            if let Some(options) = profile.get("options").and_then(serde_json::Value::as_array) {
                for option in options {
                    if option
                        .pointer("/price/type")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|v| {
                            !matches!(v, "quote_required" | "fixed" | "variable_weight")
                        })
                        || option
                            .pointer("/supply/type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|v| !matches!(v, "unknown" | "available" | "estimated"))
                        || option
                            .pointer("/unit/contents/type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|v| !matches!(v, "unspecified" | "fixed" | "estimated"))
                    {
                        return Err(CommandError::UnsupportedProfile);
                    }
                }
            }
        }
        let input: Input = serde_json::from_slice(raw).map_err(|_| CommandError::InvalidInput)?;
        input.validate()
    }
}

// Serde's derived struct decoder also accepts positional sequences. The wire
// profile requires JSON objects; this bounded structural pass never supplies
// acceptance without the independent duplicate/unknown-field-aware parser.
fn object(value: &serde_json::Value) -> Result<(), CommandError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(CommandError::InvalidInput)
    }
}
fn object_shapes(value: &serde_json::Value) -> Result<(), CommandError> {
    object(value)?;
    let profile = &value["profile"];
    object(profile)?;
    object(&profile["product"])?;
    object(&profile["fulfillment"])?;
    let media = profile["media"]
        .as_array()
        .ok_or(CommandError::InvalidInput)?;
    if media.len() > ListingTerms::MAX_MEDIA {
        return Err(CommandError::LimitExceeded);
    }
    for reference in media {
        object(reference)?;
    }
    if !profile["authority"].is_null() {
        object(&profile["authority"])?;
    }
    let options = profile["options"]
        .as_array()
        .ok_or(CommandError::InvalidInput)?;
    if options.is_empty() || options.len() > 16 {
        return Err(CommandError::LimitExceeded);
    }
    for option in options {
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
    }
    Ok(())
}
