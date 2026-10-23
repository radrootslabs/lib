//! Closed data decoding. No source, participant, authority or consent is admitted.
use super::{TERMS_MAX_BYTES, TermsWireError};
use crate::{
    command::{CommandError, check_frame, object, parse_object, required_nullable},
    listing_values,
    private_json::{self as text, RawArray},
};
use alloc::{string::String, vec::Vec};
use radroots_domain::{
    AgreedFixedPrice, AgreementLine, AgreementLineId, AgreementPrice, Decimal, DiscountBps,
    EnvelopeEventId, FulfillmentGroup, FulfillmentGroupId, FuturePromise, ListingOptionId, Money,
    PositiveDecimal, PrivateFulfillment, PublicKey, SupplyCommitment, Terms, TimeZoneId,
    UnixSeconds, VariableWeight, Window,
    listing::{FulfillmentMode, ListingCoordinate, ListingId, ListingRevision, MeasurementUnit},
};
use serde::Deserialize;
use serde_json::value::RawValue;

/// Decodes bounded closed Terms claims and checks every domain constructor.
/// Original private bytes belong to the caller. No signature, approval, current
/// source availability, stock backing or application permission is established.
pub fn decode(raw: &[u8]) -> Result<Terms, TermsWireError> {
    if raw.len() > TERMS_MAX_BYTES {
        return Err(TermsWireError::LimitExceeded);
    }
    check_frame(raw).map_err(command_error)?;
    let value: RawTerms<'_> = parse_object(raw).map_err(command_error)?;
    if value.cancellation != "mutual_residual_v1" || value.rounding != "half_even_net_line_v1" {
        return Err(TermsWireError::UnsupportedProfile);
    }
    let lines = value
        .lines
        .0
        .into_iter()
        .map(line)
        .collect::<Result<Vec<_>, _>>()?;
    let groups = value
        .fulfillment
        .0
        .into_iter()
        .map(group)
        .collect::<Result<Vec<_>, _>>()?;
    invalid(Terms::new(
        value.buyer,
        value.seller,
        lines,
        groups,
        money(value.fulfillment_charge)?,
        money(value.maximum_total)?,
        supply(value.supply)?,
    ))
}
fn invalid<T, E>(result: Result<T, E>) -> Result<T, TermsWireError> {
    result.map_err(|_| TermsWireError::InvalidInput)
}
fn command_error(error: CommandError) -> TermsWireError {
    match error {
        CommandError::LimitExceeded => TermsWireError::LimitExceeded,
        CommandError::UnsupportedProfile => TermsWireError::UnsupportedProfile,
        _ => TermsWireError::InvalidInput,
    }
}
fn parse<'a, T: Deserialize<'a>>(raw: &'a RawValue) -> Result<T, TermsWireError> {
    parse_object(raw.get().as_bytes()).map_err(command_error)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTerms<'a> {
    #[serde(deserialize_with = "crate::command::canonical_key")]
    buyer: PublicKey,
    #[serde(deserialize_with = "crate::command::canonical_key")]
    seller: PublicKey,
    #[serde(borrow)]
    lines: RawArray<'a, 32>,
    #[serde(borrow)]
    fulfillment: RawArray<'a, 8>,
    #[serde(borrow)]
    fulfillment_charge: &'a RawValue,
    #[serde(borrow)]
    maximum_total: &'a RawValue,
    #[serde(borrow)]
    supply: &'a RawValue,
    cancellation: String,
    rounding: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLine<'a> {
    id: AgreementLineId,
    #[serde(borrow)]
    source: &'a RawValue,
    option_id: ListingOptionId,
    #[serde(borrow)]
    product: &'a RawValue,
    #[serde(borrow)]
    option: &'a RawValue,
    amount: PositiveDecimal,
    fulfillment_group: FulfillmentGroupId,
    #[serde(borrow, deserialize_with = "required_nullable")]
    source_lot_label: Option<&'a RawValue>,
    #[serde(borrow)]
    price: &'a RawValue,
    #[serde(borrow)]
    maximum_line_total: &'a RawValue,
}
fn line(raw: &RawValue) -> Result<AgreementLine, TermsWireError> {
    let value: RawLine<'_> = parse(raw)?;
    let lot = value
        .source_lot_label
        .map(|v| text::decode(v.get(), 120).map_err(command_error))
        .transpose()?;
    invalid(AgreementLine::new(
        value.id,
        source(value.source)?,
        value.option_id,
        listing_values::product(value.product).map_err(command_error)?,
        listing_values::option(value.option).map_err(command_error)?,
        value.amount,
        value.fulfillment_group,
        lot.as_deref().map(String::as_str),
        price(value.price)?,
        money(value.maximum_line_total)?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    #[serde(deserialize_with = "object")]
    coordinate: RawCoordinate,
    event_id: EnvelopeEventId,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCoordinate {
    kind: u16,
    #[serde(deserialize_with = "crate::command::canonical_key")]
    author: PublicKey,
    d: ListingId,
}
fn source(raw: &RawValue) -> Result<ListingRevision, TermsWireError> {
    let value: RawSource = parse(raw)?;
    if value.coordinate.kind != 30402 {
        return Err(TermsWireError::InvalidInput);
    }
    Ok(ListingRevision::new(
        ListingCoordinate::new(value.coordinate.author, value.coordinate.d),
        value.event_id,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMoney<'a> {
    #[serde(borrow)]
    currency: &'a RawValue,
    #[serde(borrow)]
    minor: &'a RawValue,
    exponent: u8,
}
pub(crate) fn money(raw: &RawValue) -> Result<Money, TermsWireError> {
    let value: RawMoney<'_> = parse(raw)?;
    let code = text::decode(value.currency.get(), 3).map_err(command_error)?;
    let minor = text::decode(value.minor.get(), 15).map_err(command_error)?;
    invalid(Money::from_parts(&code, &minor, value.exponent))
}
#[derive(Deserialize)]
struct Tag {
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFixed<'a> {
    #[serde(rename = "type")]
    kind: String,
    #[serde(borrow)]
    rate: &'a RawValue,
    basis: PositiveDecimal,
    discount_bps: u16,
    #[serde(deserialize_with = "required_nullable")]
    selected_tier_from: Option<Decimal>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVariable<'a> {
    #[serde(rename = "type")]
    kind: String,
    measurement_unit: String,
    minimum: PositiveDecimal,
    maximum: PositiveDecimal,
    #[serde(borrow)]
    rate: &'a RawValue,
    basis: PositiveDecimal,
    #[serde(borrow)]
    maximum_total: &'a RawValue,
}
fn measurement_unit(value: &str) -> Result<MeasurementUnit, TermsWireError> {
    match value {
        "g" => Ok(MeasurementUnit::Gram),
        "kg" => Ok(MeasurementUnit::Kilogram),
        "mL" => Ok(MeasurementUnit::Millilitre),
        "L" => Ok(MeasurementUnit::Litre),
        "each" => Ok(MeasurementUnit::Each),
        _ => Err(TermsWireError::UnsupportedProfile),
    }
}
fn price(raw: &RawValue) -> Result<AgreementPrice, TermsWireError> {
    let tag: Tag = parse(raw)?;
    match tag.kind.as_str() {
        "fixed" => {
            let value: RawFixed<'_> = parse(raw)?;
            if value.kind != "fixed" {
                return Err(TermsWireError::InvalidInput);
            }
            Ok(AgreementPrice::Fixed(AgreedFixedPrice::new(
                money(value.rate)?,
                value.basis,
                invalid(DiscountBps::new(value.discount_bps))?,
                value.selected_tier_from,
            )))
        }
        "variable_weight" => {
            let value: RawVariable<'_> = parse(raw)?;
            if value.kind != "variable_weight" {
                return Err(TermsWireError::InvalidInput);
            }
            Ok(AgreementPrice::VariableWeight(invalid(
                VariableWeight::new(
                    measurement_unit(&value.measurement_unit)?,
                    value.minimum,
                    value.maximum,
                    money(value.rate)?,
                    value.basis,
                    money(value.maximum_total)?,
                ),
            )?))
        }
        _ => Err(TermsWireError::UnsupportedProfile),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroup<'a> {
    id: FulfillmentGroupId,
    #[serde(borrow)]
    terms: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFulfillment<'a> {
    mode: String,
    #[serde(borrow)]
    window: &'a RawValue,
    #[serde(borrow)]
    address: &'a RawValue,
    #[serde(borrow)]
    instructions: &'a RawValue,
    #[serde(borrow, deserialize_with = "required_nullable")]
    contact: Option<&'a RawValue>,
}
fn group(raw: &RawValue) -> Result<FulfillmentGroup, TermsWireError> {
    let value: RawGroup<'_> = parse(raw)?;
    let terms: RawFulfillment<'_> = parse(value.terms)?;
    let address = text::decode(terms.address.get(), 2048).map_err(command_error)?;
    let instructions = text::decode(terms.instructions.get(), 2048).map_err(command_error)?;
    let contact = terms
        .contact
        .map(|v| text::decode(v.get(), 256).map_err(command_error))
        .transpose()?;
    Ok(FulfillmentGroup::new(
        value.id,
        invalid(PrivateFulfillment::new(
            match terms.mode.as_str() {
                "pickup" => FulfillmentMode::Pickup,
                "delivery" => FulfillmentMode::Delivery,
                _ => return Err(TermsWireError::UnsupportedProfile),
            },
            window(terms.window)?,
            &address,
            &instructions,
            contact.as_deref().map(String::as_str),
        ))?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow<'a> {
    start: UnixSeconds,
    end: UnixSeconds,
    #[serde(borrow)]
    timezone: &'a RawValue,
}
pub(crate) fn window(raw: &RawValue) -> Result<Window, TermsWireError> {
    let value: RawWindow<'_> = parse(raw)?;
    let zone = text::decode(value.timezone.get(), 80).map_err(command_error)?;
    invalid(Window::new(
        value.start,
        value.end,
        invalid(TimeZoneId::from_name(&zone))?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stock {
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Future<'a> {
    #[serde(rename = "type")]
    kind: String,
    #[serde(borrow)]
    expected_window: &'a RawValue,
    #[serde(borrow)]
    risk_disclosure: &'a RawValue,
}
fn supply(raw: &RawValue) -> Result<SupplyCommitment, TermsWireError> {
    let tag: Tag = parse(raw)?;
    match tag.kind.as_str() {
        "stock_backed" => {
            let value: Stock = parse(raw)?;
            if value.kind != "stock_backed" {
                return Err(TermsWireError::InvalidInput);
            }
            Ok(SupplyCommitment::StockBacked)
        }
        "future_promise" => {
            let value: Future<'_> = parse(raw)?;
            if value.kind != "future_promise" {
                return Err(TermsWireError::InvalidInput);
            }
            let risk = text::decode(value.risk_disclosure.get(), 512).map_err(command_error)?;
            Ok(SupplyCommitment::FuturePromise(invalid(
                FuturePromise::new(window(value.expected_window)?, &risk),
            )?))
        }
        _ => Err(TermsWireError::UnsupportedProfile),
    }
}
