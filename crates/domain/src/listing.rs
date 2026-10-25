//! Public offer values without protocol effects or private inventory backing.
//! Exact economics remain separate from execution and physical backing.

use crate::{
    AuthorityRef, ContentMeasure, Decimal, LinePrice, ListingOptionId, PositiveDecimal,
    PricingError, PublicKey, Sequence, Window,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{fmt, str::FromStr};

mod view;
pub use view::{Listing, ListingText};
mod reference;
pub use reference::{ListingCoordinate, ListingRevision};
mod price;
pub use price::Price;
mod media;
pub use media::Media;

/// A fixed, input-free public-offer value failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ListingError {
    /// An identifier has the wrong width or noncanonical hexadecimal.
    InvalidIdentifier,
    /// A descriptive field is empty where required or exceeds its bound.
    InvalidText,
    /// An amount violates its increment, minimum or count unit.
    InvalidQuantity,
    /// Choices are missing, excessive, duplicated or lack the selected default.
    InvalidOptions,
    /// Fulfillment modes are empty, duplicated or excessive.
    InvalidFulfillment,
    /// A duration exceeds the cross-host exact integer bound.
    InvalidDuration,
    /// Declared publication time follows the signed revision time.
    InvalidTime,
    /// The requested calculation requires a fixed-price option.
    NotFixedPrice,
    /// Exact price calculation cannot be represented within the approved bounds.
    Pricing(PricingError),
    /// A media digest is not exactly64 lowercase hexadecimal digits.
    InvalidMediaDigest,
    /// More than eight public object references were supplied.
    TooManyMedia,
}
impl fmt::Display for ListingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidIdentifier => "invalid listing identifier",
            Self::InvalidText => "invalid listing text",
            Self::InvalidQuantity => "invalid listing quantity",
            Self::InvalidOptions => "invalid listing options",
            Self::InvalidFulfillment => "invalid public fulfillment",
            Self::InvalidDuration => "invalid listing duration",
            Self::InvalidTime => "invalid listing time",
            Self::NotFixedPrice => "listing option has no fixed price",
            Self::Pricing(error) => return error.fmt(f),
            Self::InvalidMediaDigest => "invalid media digest",
            Self::TooManyMedia => "too many public media references",
        })
    }
}
impl core::error::Error for ListingError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Pricing(error) => Some(error),
            _ => None,
        }
    }
}

/// Stable128-bit coordinate discriminator. Generation requires host entropy;
/// parsing this descriptor never proves novelty, ownership or publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ListingId([u8; 16]);
impl ListingId {
    /// Captures exact bytes supplied by the execution owner's entropy boundary.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    /// Borrows the original bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}
impl FromStr for ListingId {
    type Err = ListingError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 32 {
            return Err(ListingError::InvalidIdentifier);
        }
        let digit = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(ListingError::InvalidIdentifier),
        };
        let mut bytes = [0; 16];
        let (pairs, _) = value.as_bytes().as_chunks::<2>();
        for (out, &[a, b]) in bytes.iter_mut().zip(pairs) {
            *out = digit(a)? * 16 + digit(b)?;
        }
        Ok(Self(bytes))
    }
}
impl fmt::Display for ListingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
#[cfg(feature = "serde")]
impl serde::Serialize for ListingId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ListingId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = ListingId;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("32 lowercase hexadecimal characters")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(E::custom)
            }
        }
        d.deserialize_str(Visitor)
    }
}

/// Exact lead time, serialized as a canonical decimal string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LeadSeconds(Sequence);
impl LeadSeconds {
    /// Checks the shared integer range; zero means no advertised lead time.
    pub fn new(value: u64) -> Result<Self, ListingError> {
        Sequence::new(value)
            .map(Self)
            .map_err(|_| ListingError::InvalidDuration)
    }
    /// Returns the exact duration without conversion to a timestamp.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Required public listing state; inactive states retain their coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ListingState {
    /// Available for new inquiries subject to current action readiness.
    Active,
    /// Temporarily unavailable, without a sold or completion claim.
    Paused,
    /// Withdrawn from new business, without cancelling existing obligations.
    Withdrawn,
}

/// Explicit measurement identity; equal numbers do not imply conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum MeasurementUnit {
    /// Grams.
    #[cfg_attr(feature = "serde", serde(rename = "g"))]
    Gram,
    /// Kilograms.
    #[cfg_attr(feature = "serde", serde(rename = "kg"))]
    Kilogram,
    /// Millilitres.
    #[cfg_attr(feature = "serde", serde(rename = "mL"))]
    Millilitre,
    /// Litres.
    #[cfg_attr(feature = "serde", serde(rename = "L"))]
    Litre,
    /// Integral individual units.
    #[cfg_attr(feature = "serde", serde(rename = "each"))]
    Each,
}
impl MeasurementUnit {
    /// Exact public protocol symbol, without an implicit unit conversion.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Gram => "g",
            Self::Kilogram => "kg",
            Self::Millilitre => "mL",
            Self::Litre => "L",
            Self::Each => "each",
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for MeasurementUnit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = MeasurementUnit;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("one exact measurement unit symbol")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<MeasurementUnit, E> {
                match value {
                    "g" => Ok(MeasurementUnit::Gram),
                    "kg" => Ok(MeasurementUnit::Kilogram),
                    "mL" => Ok(MeasurementUnit::Millilitre),
                    "L" => Ok(MeasurementUnit::Litre),
                    "each" => Ok(MeasurementUnit::Each),
                    _ => Err(E::custom("invalid measurement unit")),
                }
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
enum UnitKind {
    Measured {
        unit: MeasurementUnit,
    },
    Package {
        label: String,
        contents: ContentMeasure,
    },
}

/// Validated sale unit. An unspecified package implies no mass/volume mapping.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct SaleUnit(UnitKind);
impl SaleUnit {
    /// Selects an explicit measurement unit.
    pub const fn measured(unit: MeasurementUnit) -> Self {
        Self(UnitKind::Measured { unit })
    }
    /// Selects a labelled package without inventing its physical contents.
    pub fn package_unspecified(label: &str) -> Result<Self, ListingError> {
        Self::package(label, ContentMeasure::Unspecified)
    }
    /// Captures declared contents without inferring physical conversion or stock.
    pub fn package(label: &str, contents: ContentMeasure) -> Result<Self, ListingError> {
        text(label, 80, false)?;
        Ok(Self(UnitKind::Package {
            label: label.into(),
            contents,
        }))
    }
    /// Returns package metadata; a measured sale unit has no package contents.
    pub const fn contents(&self) -> Option<ContentMeasure> {
        match self.0 {
            UnitKind::Package { contents, .. } => Some(contents),
            UnitKind::Measured { .. } => None,
        }
    }
    /// Borrows the explicit display label/symbol used by public projections.
    pub fn label(&self) -> &str {
        match &self.0 {
            UnitKind::Measured { unit } => unit.symbol(),
            UnitKind::Package { label, .. } => label,
        }
    }
    /// Whether sale quantities must be integral counts.
    pub const fn integral(&self) -> bool {
        matches!(
            self.0,
            UnitKind::Package { .. }
                | UnitKind::Measured {
                    unit: MeasurementUnit::Each
                }
        )
    }
}
impl fmt::Debug for SaleUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SaleUnit(<redacted>)")
    }
}

/// An advertised estimate/claim, never an authoritative stock balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
pub enum Supply {
    /// No quantity is advertised.
    Unknown,
    /// The seller advertises this nonnegative quantity in the sale unit.
    Available {
        /// Public advertised quantity, without backing authority.
        amount: Decimal,
    },
    /// An estimate and optional declared expected window, without reservation.
    Estimated {
        /// Exact nonnegative estimate in the selected sale unit.
        amount: Decimal,
        /// Explicit absent or validated absolute expected window.
        window: Option<Window>,
    },
}

/// Validated public product description; producer data grants no agency rights.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Product {
    name: String,
    description: String,
    producer: Option<PublicKey>,
    origin_summary: Option<String>,
}
impl Product {
    /// Captures explicitly public descriptive fields without hierarchy lookup.
    pub fn new(
        name: &str,
        description: &str,
        producer: Option<PublicKey>,
        origin_summary: Option<&str>,
    ) -> Result<Self, ListingError> {
        text(name, 160, false)?;
        text(description, 4096, true)?;
        if let Some(origin) = origin_summary {
            text(origin, 160, true)?;
        }
        Ok(Self {
            name: name.into(),
            description: description.into(),
            producer,
            origin_summary: origin_summary.map(String::from),
        })
    }
    /// Borrows the validated public product name.
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl fmt::Debug for Product {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Product(<redacted>)")
    }
}

/// A validated buyer-selectable sale choice, with no private pool/lot IDs.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ListingOption {
    id: ListingOptionId,
    label: String,
    unit: SaleUnit,
    increment: PositiveDecimal,
    minimum: PositiveDecimal,
    price: Price,
    supply: Supply,
    lead_seconds: LeadSeconds,
}
impl ListingOption {
    /// Captures quote-required terms. Minimum is an exact multiple of increment;
    /// count/package units and their advertised available supply are integral.
    pub fn quote_required(
        id: ListingOptionId,
        label: &str,
        unit: SaleUnit,
        increment: PositiveDecimal,
        minimum: PositiveDecimal,
        supply: Supply,
        lead_seconds: LeadSeconds,
    ) -> Result<Self, ListingError> {
        Self::new(
            id,
            label,
            unit,
            increment,
            minimum,
            Price::QuoteRequired,
            supply,
            lead_seconds,
        )
    }
    /// Captures complete explicit price and purchase rules without stock effects.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ListingOptionId,
        label: &str,
        unit: SaleUnit,
        increment: PositiveDecimal,
        minimum: PositiveDecimal,
        price: Price,
        supply: Supply,
        lead_seconds: LeadSeconds,
    ) -> Result<Self, ListingError> {
        text(label, 120, false)?;
        let inc = increment.amount();
        let min = minimum.amount();
        if !exact_multiple(min, inc)
            || (unit.integral()
                && (inc.scale() != 0
                    || min.scale() != 0
                    || matches!(supply, Supply::Available { amount } | Supply::Estimated { amount,.. } if amount.scale() != 0)))
        {
            return Err(ListingError::InvalidQuantity);
        }
        Ok(Self {
            id,
            label: label.into(),
            unit,
            increment,
            minimum,
            price,
            supply,
            lead_seconds,
        })
    }
    /// Returns the stable sale-choice identity.
    pub const fn id(&self) -> ListingOptionId {
        self.id
    }
    /// Borrows the exact sale unit.
    pub const fn unit(&self) -> &SaleUnit {
        &self.unit
    }
    /// Returns the exact positive purchase increment.
    pub const fn increment(&self) -> PositiveDecimal {
        self.increment
    }
    /// Returns the exact positive advertised minimum.
    pub const fn minimum(&self) -> PositiveDecimal {
        self.minimum
    }
    /// Returns the advertised supply claim without freshness or stock authority.
    pub const fn supply(&self) -> Supply {
        self.supply
    }
    /// Checks minimum, increment and integral count rules without claiming stock.
    /// Advertised supply is an estimate, not an implicit quantity ceiling.
    pub fn validate_amount(&self, amount: PositiveDecimal) -> Result<(), ListingError> {
        let amount = amount.amount();
        if amount < self.minimum.amount()
            || !exact_multiple(amount, self.increment.amount())
            || (self.unit.integral() && amount.scale() != 0)
        {
            return Err(ListingError::InvalidQuantity);
        }
        Ok(())
    }
    /// Borrows the complete validated mode without inventing a quoted amount.
    pub const fn price(&self) -> &Price {
        &self.price
    }
    /// Admits purchase rules before selecting a tier and pricing one fixed line.
    pub fn fixed_line(&self, amount: PositiveDecimal) -> Result<LinePrice, ListingError> {
        self.validate_amount(amount)?;
        match &self.price {
            Price::Fixed(value) => value.line(amount).map_err(ListingError::Pricing),
            Price::QuoteRequired | Price::VariableWeight(_) => Err(ListingError::NotFixedPrice),
        }
    }
    /// Borrows the public default-choice description.
    pub fn label(&self) -> &str {
        &self.label
    }
}
impl fmt::Debug for ListingOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingOption(<redacted>)")
    }
}

fn exact_multiple(value: Decimal, increment: Decimal) -> bool {
    let scale = value.scale().max(increment.scale());
    let align = |amount: Decimal| {
        i128::from(amount.coefficient())
            .checked_mul(10_i128.checked_pow(u32::from(scale - amount.scale()))?)
    };
    align(value).and_then(|value| value.checked_rem(align(increment)?)) == Some(0)
}

/// Public fulfillment mode, without exact private destination or time window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FulfillmentMode {
    /// Collection by the buyer.
    Pickup,
    /// Delivery, subject to separately agreed private terms.
    Delivery,
}

/// Bounded coarse public fulfillment; this is not an installed agreement.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PublicFulfillment {
    modes: Box<[FulfillmentMode]>,
    area: String,
    lead_seconds: LeadSeconds,
}
impl PublicFulfillment {
    /// Captures one or two distinct modes and an explicitly public coarse area.
    pub fn new(
        modes: Vec<FulfillmentMode>,
        area: &str,
        lead_seconds: LeadSeconds,
    ) -> Result<Self, ListingError> {
        if modes.is_empty() || modes.len() > 2 || (modes.len() == 2 && modes[0] == modes[1]) {
            return Err(ListingError::InvalidFulfillment);
        }
        text(area, 160, true)?;
        Ok(Self {
            modes: modes.into_boxed_slice(),
            area: area.into(),
            lead_seconds,
        })
    }
    /// Borrows the public coarse area; it grants no endpoint or address trust.
    pub fn area(&self) -> &str {
        &self.area
    }
}
impl fmt::Debug for PublicFulfillment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicFulfillment(<redacted>)")
    }
}

/// Immutable public-offer terms, with complete explicit sale-option prices.
/// These are data, without workspace authority, signing or private backing.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ListingTerms {
    state: ListingState,
    product: Product,
    default_option: ListingOptionId,
    options: Box<[ListingOption]>,
    fulfillment: PublicFulfillment,
    authority: Option<AuthorityRef>,
    media: Box<[Media]>,
    #[cfg_attr(feature = "serde", serde(skip))]
    default_index: usize,
}
impl ListingTerms {
    /// Largest number of public object references in one complete profile.
    pub const MAX_MEDIA: usize = 8;
    /// Checks bounded unique choices and exact default membership before capture.
    pub fn new(
        state: ListingState,
        product: Product,
        default_option: ListingOptionId,
        options: Vec<ListingOption>,
        fulfillment: PublicFulfillment,
        authority: Option<AuthorityRef>,
        media: Vec<Media>,
    ) -> Result<Self, ListingError> {
        if media.len() > Self::MAX_MEDIA {
            return Err(ListingError::TooManyMedia);
        }
        if options.is_empty()
            || options.len() > 16
            || options
                .iter()
                .enumerate()
                .any(|(i, o)| options[..i].iter().any(|p| p.id == o.id))
        {
            return Err(ListingError::InvalidOptions);
        }
        let default_index = options
            .iter()
            .position(|o| o.id == default_option)
            .ok_or(ListingError::InvalidOptions)?;
        Ok(Self {
            state,
            product,
            default_option,
            options: options.into_boxed_slice(),
            fulfillment,
            authority,
            media: media.into_boxed_slice(),
            default_index,
        })
    }
    /// Returns the advertised state, without determining action readiness.
    pub const fn state(&self) -> ListingState {
        self.state
    }
    /// Borrows the exact public product snapshot without any live source proof.
    pub const fn product(&self) -> &Product {
        &self.product
    }
    /// Borrows the bounded, unique public choices.
    pub fn options(&self) -> &[ListingOption] {
        &self.options
    }
    /// Borrows the previously validated default choice without a fallback.
    pub fn default_option(&self) -> &ListingOption {
        // Private immutable fields and the validating constructor ensure exact
        // membership. Returning a reference introduces no reconstructed choice.
        &self.options[self.default_index]
    }
    /// Borrows the coarse public fulfillment data.
    pub const fn fulfillment(&self) -> &PublicFulfillment {
        &self.fulfillment
    }
    /// Borrows bounded declared references without fetching or verifying files.
    pub fn media(&self) -> &[Media] {
        &self.media
    }
}
impl fmt::Debug for ListingTerms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingTerms(<redacted>)")
    }
}

fn text(value: &str, maximum: usize, allow_empty: bool) -> Result<(), ListingError> {
    if (!allow_empty && value.is_empty())
        || value.len() > maximum.saturating_mul(4)
        || value.chars().count() > maximum
    {
        return Err(ListingError::InvalidText);
    }
    Ok(())
}
