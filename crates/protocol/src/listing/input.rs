//! Closed known-profile input admission. Domain construction remains the sole
//! value invariant owner; no unknown field or unsupported feature is discarded.
use super::PublishInput;
use crate::command::{CommandError, check_frame, required_nullable};
use alloc::{string::String, vec::Vec};
use radroots_domain::{
    AuthorityRef, Decimal, ListingOptionId, PositiveDecimal, PublicKey, UnixSeconds, listing::*,
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
    #[serde(rename = "media")]
    _media: [(); 0],
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
    price: Quote,
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
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Contents {
    Unspecified,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Quote {
    QuoteRequired,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawSupply {
    Unknown,
    Available { amount: Decimal },
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

fn invalid<T>(value: Result<T, ListingError>) -> Result<T, CommandError> {
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
            let Quote::QuoteRequired = option.price;
            let unit = match option.unit {
                Unit::Measured { unit } => SaleUnit::measured(match unit {
                    Measure::Gram => MeasurementUnit::Gram,
                    Measure::Kilogram => MeasurementUnit::Kilogram,
                    Measure::Millilitre => MeasurementUnit::Millilitre,
                    Measure::Litre => MeasurementUnit::Litre,
                    Measure::Each => MeasurementUnit::Each,
                }),
                Unit::Package { label, contents } => {
                    let Contents::Unspecified = contents;
                    invalid(SaleUnit::package_unspecified(&label))?
                }
            };
            options.push(invalid(ListingOption::quote_required(
                option.id,
                &option.label,
                unit,
                option.increment,
                option.minimum,
                match option.supply {
                    RawSupply::Unknown => Supply::Unknown,
                    RawSupply::Available { amount } => Supply::Available { amount },
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
    /// Admits bounded closed quote-required input. Known unsupported features
    /// return an explicit refusal; they are never silently stripped or coerced.
    pub fn parse(raw: &[u8]) -> Result<Self, CommandError> {
        check_frame(raw)?;
        // This bounded peek only classifies unimplemented features. Acceptance
        // still requires the independent closed parser, including duplicates,
        // required nulls and all nested domain constructors.
        let value: serde_json::Value =
            serde_json::from_slice(raw).map_err(|_| CommandError::InvalidInput)?;
        if let Some(profile) = value.get("profile") {
            if profile
                .get("profile")
                .is_some_and(|v| v != "radroots.listing")
                || profile.get("version").is_some_and(|v| v != 1)
                || profile
                    .get("media")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|v| !v.is_empty())
            {
                return Err(CommandError::UnsupportedProfile);
            }
            if let Some(options) = profile.get("options").and_then(serde_json::Value::as_array) {
                for option in options {
                    if option
                        .pointer("/price/type")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|v| v != "quote_required")
                        || option
                            .pointer("/supply/type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|v| !matches!(v, "unknown" | "available"))
                        || option
                            .pointer("/unit/contents/type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|v| v != "unspecified")
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
