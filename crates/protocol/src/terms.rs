//! Explicit private Terms v1 disclosure and digest. Encoding authenticates no
//! source, consent, participant, authority, reservation or installed agreement.

use alloc::string::String;
use core::fmt;
use radroots_domain::{
    AgreementPrice, Money, SupplyCommitment, Terms, TermsDigest, Window, listing::FulfillmentMode,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

mod writer;
use writer::Writer;
mod decode;
mod text;
pub use decode::decode;

/// Private content budget. A complete record additionally includes its header;
/// fitting this Terms object alone does not prove that a record or rumor fits.
pub const TERMS_MAX_BYTES: usize = 16_384;

/// Fixed failures without private caller data or dependency diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TermsWireError {
    /// A value cannot be represented by this exact canonical profile.
    InvalidInput,
    /// Complete escaped UTF-8 content exceeds its fixed byte limit.
    LimitExceeded,
    /// A future domain variant has no supported v1 wire representation.
    UnsupportedProfile,
}
impl fmt::Display for TermsWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid terms profile",
            Self::LimitExceeded => "terms byte limit exceeded",
            Self::UnsupportedProfile => "unsupported terms profile",
        })
    }
}
impl core::error::Error for TermsWireError {}

/// One explicit protected canonical disclosure. It has no automatic serialization
/// or cloning and supplies no authenticated evidence or permission to issue.
///
/// ```compile_fail
/// use radroots_protocol::terms::EncodedTerms;
/// fn automatic<T: serde::Serialize>() {}
/// automatic::<EncodedTerms>();
/// ```
///
/// ```compile_fail
/// use radroots_protocol::terms::EncodedTerms;
/// fn duplicate<T: Clone>() {}
/// duplicate::<EncodedTerms>();
/// ```
pub struct EncodedTerms {
    wire: Zeroizing<String>,
    digest: TermsDigest,
}
impl EncodedTerms {
    /// Counts the complete escaped representation before allocating its sole
    /// private output buffer, then computes the domain-separated Terms identity.
    pub fn capture(terms: &Terms) -> Result<Self, TermsWireError> {
        let mut count = Writer::counting();
        encode(terms, &mut count)?;
        let mut output = Writer::emitting(count.len());
        encode(terms, &mut output)?;
        if output.len() != count.len() {
            return Err(TermsWireError::InvalidInput);
        }
        let wire = output.finish()?;
        let mut hash = Sha256::new();
        hash.update(b"radroots.terms.v1\0");
        hash.update(wire.as_bytes());
        Ok(Self {
            wire,
            digest: TermsDigest::from_bytes(hash.finalize().into()),
        })
    }
    /// Explicitly borrows private complete canonical bytes for protected handling.
    pub fn protected_wire(&self) -> &[u8] {
        self.wire.as_bytes()
    }
    /// Returns the v1 content identity, without approval or signature evidence.
    pub const fn digest(&self) -> TermsDigest {
        self.digest
    }
    /// Returns this owner's fixed object plus retained private buffer capacity.
    /// Caller-owned domain values and outer record/rumor costs are separate.
    pub fn retained_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.wire.capacity()
    }
}
impl fmt::Debug for EncodedTerms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedTerms(<redacted>)")
    }
}

fn money(value: Money, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{\"currency\":")?;
    out.string(value.currency().code())?;
    out.raw(",\"exponent\":")?;
    out.integer(u16::from(value.currency().exponent()))?;
    out.raw(",\"minor\":")?;
    out.exact_string(value.minor())?;
    out.raw("}")
}
fn window(value: Window, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{\"end\":")?;
    out.exact_string(value.end())?;
    out.raw(",\"start\":")?;
    out.exact_string(value.start())?;
    out.raw(",\"timezone\":")?;
    out.string(value.timezone().name())?;
    out.raw("}")
}
fn encode(value: &Terms, out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("{\"buyer\":")?;
    out.string(&value.buyer().to_hex())?;
    out.raw(",\"cancellation\":")?;
    out.string(value.cancellation())?;
    out.raw(",\"fulfillment\":[")?;
    for (index, group) in value.fulfillment().iter().enumerate() {
        if index != 0 {
            out.raw(",")?;
        }
        out.raw("{\"id\":")?;
        out.exact_string(group.id())?;
        out.raw(",\"terms\":{\"address\":")?;
        out.string(group.terms().address())?;
        out.raw(",\"contact\":")?;
        out.optional_string(group.terms().contact())?;
        out.raw(",\"instructions\":")?;
        out.string(group.terms().instructions())?;
        out.raw(",\"mode\":")?;
        out.string(match group.terms().mode() {
            FulfillmentMode::Pickup => "pickup",
            FulfillmentMode::Delivery => "delivery",
        })?;
        out.raw(",\"window\":")?;
        window(group.terms().window(), out)?;
        out.raw("}}")?;
    }
    out.raw("],\"fulfillment_charge\":")?;
    money(value.fulfillment_charge(), out)?;
    out.raw(",\"lines\":[")?;
    for (index, line) in value.lines().iter().enumerate() {
        if index != 0 {
            out.raw(",")?;
        }
        out.raw("{\"amount\":")?;
        out.exact_string(line.amount())?;
        out.raw(",\"fulfillment_group\":")?;
        out.exact_string(line.fulfillment_group())?;
        out.raw(",\"id\":")?;
        out.exact_string(line.id())?;
        out.raw(",\"maximum_line_total\":")?;
        money(line.maximum_line_total(), out)?;
        out.raw(",\"option\":")?;
        out.public_snapshot(line.option())?;
        out.raw(",\"option_id\":")?;
        out.exact_string(line.option_id())?;
        out.raw(",\"price\":")?;
        match line.price() {
            AgreementPrice::Fixed(price) => {
                out.raw("{\"basis\":")?;
                out.exact_string(price.basis())?;
                out.raw(",\"discount_bps\":")?;
                out.integer(price.discount_bps().get())?;
                out.raw(",\"rate\":")?;
                money(price.rate(), out)?;
                out.raw(",\"selected_tier_from\":")?;
                match price.selected_tier_from() {
                    Some(value) => out.exact_string(value)?,
                    None => out.raw("null")?,
                }
                out.raw(",\"type\":\"fixed\"}")?;
            }
            AgreementPrice::VariableWeight(price) => {
                out.raw("{\"basis\":")?;
                out.exact_string(price.basis())?;
                out.raw(",\"maximum\":")?;
                out.exact_string(price.maximum())?;
                out.raw(",\"maximum_total\":")?;
                money(price.maximum_total(), out)?;
                out.raw(",\"measurement_unit\":")?;
                out.string(price.measurement_unit().symbol())?;
                out.raw(",\"minimum\":")?;
                out.exact_string(price.minimum())?;
                out.raw(",\"rate\":")?;
                money(price.rate(), out)?;
                out.raw(",\"type\":\"variable_weight\"}")?;
            }
            _ => return Err(TermsWireError::UnsupportedProfile),
        }
        out.raw(",\"product\":")?;
        out.public_snapshot(line.product())?;
        out.raw(",\"source\":")?;
        out.public_snapshot(&line.source())?;
        out.raw(",\"source_lot_label\":")?;
        out.optional_string(line.source_lot_label())?;
        out.raw("}")?;
    }
    out.raw("],\"maximum_total\":")?;
    money(value.maximum_total(), out)?;
    out.raw(",\"rounding\":")?;
    out.string(value.rounding())?;
    out.raw(",\"seller\":")?;
    out.string(&value.seller().to_hex())?;
    out.raw(",\"supply\":")?;
    match value.supply() {
        SupplyCommitment::StockBacked => out.raw("{\"type\":\"stock_backed\"}")?,
        SupplyCommitment::FuturePromise(promise) => {
            out.raw("{\"expected_window\":")?;
            window(promise.expected_window(), out)?;
            out.raw(",\"risk_disclosure\":")?;
            out.string(promise.risk_disclosure())?;
            out.raw(",\"type\":\"future_promise\"}")?;
        }
        _ => return Err(TermsWireError::UnsupportedProfile),
    }
    out.raw("}")
}
