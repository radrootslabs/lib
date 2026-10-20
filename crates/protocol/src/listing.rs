//! Known public listing producer. Full monetary/media admission is separate;
//! unsupported features are never reduced to an invented quote or zero price.

use crate::nip01::{self, EventFields, WireError};
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;
use radroots_domain::{
    PublicKey, UnixSeconds,
    listing::{ListingId, ListingState, ListingTerms, Price},
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

mod command;
mod input;
mod plan;
pub use command::CapturedListingCommand;
pub use plan::PublicSigningPlan;

/// Assigned NIP-99 addressable classified-listing kind.
pub const LISTING_KIND: u32 = 30402;
/// Complete public event cap, including NIP-01 key, ID and signature framing.
pub const LISTING_EVENT_MAX_BYTES: usize = 65_536;
/// The closed public profile emits at most eight ordered two-field tags.
pub const LISTING_TAG_MAX_COUNT: usize = 8;
/// Both key and value count toward the closed public profile's field budget.
pub const LISTING_TAG_TOTAL_ELEMENT_MAX_COUNT: usize = 16;

/// Explicit immutable public-offer input; no IDs, time or authority are inferred.
#[derive(Clone, Serialize)]
pub struct PublishInput {
    id: ListingId,
    title: String,
    summary: String,
    description_markdown: String,
    published_at: UnixSeconds,
    #[serde(rename = "profile", serialize_with = "serialize_profile")]
    terms: ListingTerms,
}
impl PublishInput {
    /// Captures bounded text and validated quote-required terms. The human
    /// summary describes the selected default unit/basis. No price is inferred.
    pub fn new(
        id: ListingId,
        title: &str,
        summary: &str,
        description_markdown: &str,
        published_at: UnixSeconds,
        terms: ListingTerms,
    ) -> Result<Self, WireError> {
        for (value, limit, empty) in [
            (title, 160, false),
            (summary, 280, false),
            (description_markdown, 16_384, true),
        ] {
            if (!empty && value.is_empty())
                || value.len() > limit * 4
                || value.chars().count() > limit
            {
                return Err(WireError::InvalidInput);
            }
        }
        Ok(Self {
            id,
            title: title.into(),
            summary: summary.into(),
            description_markdown: description_markdown.into(),
            published_at,
            terms,
        })
    }
    /// Returns the independently selected stable coordinate discriminator.
    pub const fn id(&self) -> ListingId {
        self.id
    }
    /// Borrows the exact validated terms, without stock or publication claims.
    pub const fn terms(&self) -> &ListingTerms {
        &self.terms
    }
    /// Returns the captured original publication timestamp.
    pub const fn published_at(&self) -> UnixSeconds {
        self.published_at
    }
}
impl fmt::Debug for PublishInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublishInput(<redacted>)")
    }
}

fn profile_value(terms: &ListingTerms) -> Result<Value, WireError> {
    let mut value = serde_json::to_value(terms).map_err(|_| WireError::InvalidInput)?;
    let object = value.as_object_mut().ok_or(WireError::InvalidInput)?;
    if object.len() != 6
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "authority" | "default_option" | "fulfillment" | "options" | "product" | "state"
            )
        })
        || terms
            .options()
            .iter()
            .any(|option| !matches!(option.price(), Price::QuoteRequired))
    {
        return Err(WireError::InvalidInput);
    }
    object.insert("profile".into(), Value::String("radroots.listing".into()));
    object.insert("version".into(), Value::from(1_u8));
    // This producer supports no media. It never parses and silently strips
    // media supplied by another producer or coerces an unsupported price.
    object.insert("media".into(), Value::Array(Vec::new()));
    Ok(value)
}
fn serialize_profile<S: serde::Serializer>(
    terms: &ListingTerms,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    profile_value(terms)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}

/// Retained exact public event fields. This value contains no signature and
/// supplies no evidence of T0/T1, signer invocation, installation or delivery.
#[derive(Clone)]
pub struct ListingEvent {
    author: PublicKey,
    created_at: UnixSeconds,
    tags: Vec<Vec<String>>,
    content: String,
    id: [u8; 32],
}
impl ListingEvent {
    /// Builds one bounded profile/projection from immutable captured input.
    /// Inactive revisions retain kind30402 and the same `d`, visibly state
    /// non-availability, and omit a standard status rather than asserting sold.
    pub fn new(
        input: &PublishInput,
        author: PublicKey,
        created_at: UnixSeconds,
    ) -> Result<Self, WireError> {
        if input.published_at > created_at {
            return Err(WireError::InvalidInput);
        }
        let mut profile = String::new();
        canonical(&profile_value(&input.terms)?, &mut profile)?;
        let inactive = match input.terms.state() {
            ListingState::Active => None,
            ListingState::Paused => Some("Paused: unavailable"),
            ListingState::Withdrawn => Some("Withdrawn: unavailable"),
        };
        let summary = match inactive {
            Some(label) => format!("[{label}] {}", input.summary),
            None => input.summary.clone(),
        };
        let content = match inactive {
            Some(label) => format!("[{label}]\n\n{}", input.description_markdown),
            None => input.description_markdown.clone(),
        };
        let mut tags = vec![
            vec!["d".into(), input.id.to_string()],
            vec!["title".into(), input.title.clone()],
            vec!["summary".into(), summary],
            vec!["published_at".into(), input.published_at.to_string()],
            vec!["location".into(), input.terms.fulfillment().area().into()],
            vec![
                "radroots:price_unit".into(),
                input.terms.default_option().unit().label().into(),
            ],
        ];
        if inactive.is_none() {
            tags.push(vec!["status".into(), "active".into()]);
        }
        tags.push(vec!["radroots:listing".into(), profile]);
        let fields = EventFields {
            author,
            created_at: created_at.get(),
            kind: LISTING_KIND,
            tags: &tags,
            content: &content,
        };
        nip01::signed_json_len(&fields, LISTING_EVENT_MAX_BYTES)?;
        let id = Sha256::digest(nip01::canonical_preimage(&fields).as_bytes()).into();
        Ok(Self {
            author,
            created_at,
            tags,
            content,
            id,
        })
    }
    /// Borrows all original fields without sorting tags or reconstructing text.
    pub fn fields(&self) -> EventFields<'_> {
        EventFields {
            author: self.author,
            created_at: self.created_at.get(),
            kind: LISTING_KIND,
            tags: &self.tags,
            content: &self.content,
        }
    }
    /// Exact canonical unsigned NIP-01 event ID, not a signature or store receipt.
    pub const fn event_id(&self) -> &[u8; 32] {
        &self.id
    }
}
impl fmt::Debug for ListingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListingEvent(<redacted>)")
    }
}

// RFC8785 subset: field names are fixed ASCII, quantities/IDs/times are strings,
// numeric fields are exact safe unsigned integers, and Unicode is preserved.
// Explicit sorting remains correct if a downstream crate enables preserve_order.
pub(crate) fn canonical(value: &Value, out: &mut String) -> Result<(), WireError> {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<_> = fields.keys().collect();
            if keys.iter().any(|key| !key.is_ascii()) {
                return Err(WireError::InvalidInput);
            }
            keys.sort_unstable();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).map_err(|_| WireError::InvalidInput)?);
                out.push(':');
                canonical(&fields[key], out)?;
            }
            out.push('}');
        }
        Value::Array(values) => {
            out.push('[');
            for (i, value) in values.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                canonical(value, out)?;
            }
            out.push(']');
        }
        Value::Number(number) => {
            if number
                .as_u64()
                .is_none_or(|value| value > 9_007_199_254_740_991)
            {
                return Err(WireError::InvalidInput);
            }
            out.push_str(&number.to_string());
        }
        _ => out.push_str(&serde_json::to_string(value).map_err(|_| WireError::InvalidInput)?),
    }
    if out.len() > LISTING_EVENT_MAX_BYTES {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}
