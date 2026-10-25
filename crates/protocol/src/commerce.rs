//! Complete private record data and canonical identities. No authenticated
//! consent, current authority, issuance or stock capability is created here.
use crate::{
    profile,
    terms::{TermsWireError, writer::Writer},
};
use alloc::string::String;
use core::fmt;
use radroots_domain::{CommerceContent, PublicKey, RecordId};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

mod coverage;
mod decode;
pub use coverage::CapturedCausalCoverage;
mod seal;
pub use seal::CommerceSeal;
mod signing_plan;
pub use signing_plan::{CommerceSigningPlan, SIGNING_PLAN_MAX_BYTES};
mod initial_commitment;
pub use initial_commitment::initial_commitment_rumor;
mod payload;
pub use decode::{DecodedRecord, decode};
mod rumor;
pub use rumor::EncodedCommerceRumor;

/// The complete private content budget, including record ID, header and payload.
pub const RECORD_MAX_BYTES: usize = profile::PRIVATE_RECORD_MAX_BYTES;

/// Input-free record codec failures. None contains private source data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecordWireError {
    /// A closed field or complete domain invariant is invalid.
    InvalidInput,
    /// Complete content or a nested profile exceeds a supported bound.
    LimitExceeded,
    /// A future profile or vocabulary has no supported representation.
    UnsupportedProfile,
    /// The supplied record identity differs from complete canonical content.
    IdentityMismatch,
    /// Final rumor kind, author or timestamp differs from the content claims.
    CarrierMismatch,
}
impl fmt::Display for RecordWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid record profile",
            Self::LimitExceeded => "record byte limit exceeded",
            Self::UnsupportedProfile => "unsupported record profile",
            Self::IdentityMismatch => "record identity mismatch",
            Self::CarrierMismatch => "record carrier mismatch",
        })
    }
}
impl core::error::Error for RecordWireError {}
impl From<TermsWireError> for RecordWireError {
    fn from(value: TermsWireError) -> Self {
        match value {
            TermsWireError::LimitExceeded => Self::LimitExceeded,
            TermsWireError::UnsupportedProfile => Self::UnsupportedProfile,
            _ => Self::InvalidInput,
        }
    }
}

/// Explicit canonical private disclosure, with no automatic serialization,
/// cloning or permission to issue a structured rumor.
///
/// ```compile_fail
/// use radroots_protocol::commerce::EncodedRecord;
/// fn automatic<T: serde::Serialize>() {}
/// automatic::<EncodedRecord>();
/// ```
/// ```compile_fail
/// use radroots_protocol::commerce::EncodedRecord;
/// fn duplicate<T: Clone>() {}
/// duplicate::<EncodedRecord>();
/// ```
pub struct EncodedRecord {
    wire: Zeroizing<String>,
    identity: RecordId,
    author: PublicKey,
    created_at: u64,
}
impl EncodedRecord {
    /// Checks the complete final size before private output allocation. The
    /// zeroizing hash preimage is dropped before allocating the final wire.
    pub fn capture(content: &CommerceContent) -> Result<Self, RecordWireError> {
        if content.profile() != profile::COMMERCE_PROFILE
            || content.version() != profile::PROFILE_VERSION
        {
            return Err(RecordWireError::UnsupportedProfile);
        }
        let mut complete = Writer::counting();
        encode(content, Some(RecordId::from_bytes([0; 32])), &mut complete)?;
        if complete.len() > RECORD_MAX_BYTES {
            return Err(RecordWireError::LimitExceeded);
        }
        let mut count = Writer::counting();
        encode(content, None, &mut count)?;
        let mut preimage = Writer::emitting(count.len());
        encode(content, None, &mut preimage)?;
        if preimage.len() != count.len() {
            return Err(RecordWireError::InvalidInput);
        }
        let bytes = preimage.finish()?;
        let mut hash = Sha256::new();
        hash.update(b"radroots.record.v1\0");
        hash.update(bytes.as_bytes());
        let identity = RecordId::from_bytes(hash.finalize().into());
        drop(bytes);
        let mut output = Writer::emitting(complete.len());
        encode(content, Some(identity), &mut output)?;
        if output.len() != complete.len() {
            return Err(RecordWireError::InvalidInput);
        }
        Ok(Self {
            wire: output.finish()?,
            identity,
            author: content.header().author(),
            created_at: content.header().created_at().get(),
        })
    }
    /// Borrows complete private content for explicitly protected handling.
    pub fn protected_wire(&self) -> &[u8] {
        self.wire.as_bytes()
    }
    /// Returns the content identity, distinct from a rumor or envelope ID.
    pub const fn record_id(&self) -> RecordId {
        self.identity
    }
    /// Reports this owner's object and retained private buffer capacity.
    pub fn retained_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.wire.capacity()
    }
    /// Checks equality with final rumor claims. The caller must separately
    /// authenticate the original wrap/seal/rumor and bind its recipient; this
    /// comparison creates no authenticated evidence or permission.
    pub fn check_rumor_fields(
        &self,
        author: PublicKey,
        created_at: u64,
        kind: u16,
    ) -> Result<(), RecordWireError> {
        if kind != profile::COMMERCE_KIND || author != self.author || created_at != self.created_at
        {
            return Err(RecordWireError::CarrierMismatch);
        }
        Ok(())
    }
}
impl fmt::Debug for EncodedRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedRecord(<redacted>)")
    }
}
fn optional<T: fmt::Display>(value: Option<T>, out: &mut Writer) -> Result<(), TermsWireError> {
    match value {
        Some(value) => out.exact_string(value),
        None => out.raw("null"),
    }
}
fn ids<T: fmt::Display>(values: &[T], out: &mut Writer) -> Result<(), TermsWireError> {
    out.raw("[")?;
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.raw(",")?;
        }
        out.exact_string(value)?;
    }
    out.raw("]")
}
fn encode(
    value: &CommerceContent,
    identity: Option<RecordId>,
    out: &mut Writer,
) -> Result<(), TermsWireError> {
    let header = value.header();
    out.raw("{\"author\":")?;
    out.exact_string(header.author())?;
    out.raw(",\"author_previous\":")?;
    optional(header.author_previous(), out)?;
    out.raw(",\"author_sequence\":")?;
    out.exact_string(header.author_sequence())?;
    out.raw(",\"buyer\":")?;
    out.exact_string(header.buyer())?;
    out.raw(",\"created_at\":")?;
    out.exact_string(header.created_at())?;
    out.raw(",\"order_id\":")?;
    optional(header.order_id(), out)?;
    out.raw(",\"parents\":")?;
    ids(header.parents(), out)?;
    out.raw(",\"payload\":")?;
    payload::encode(value.payload(), out)?;
    out.raw(",\"profile\":")?;
    out.string(value.profile())?;
    if let Some(identity) = identity {
        out.raw(",\"record_id\":")?;
        out.exact_string(identity)?;
    }
    out.raw(",\"seller\":")?;
    out.exact_string(header.seller())?;
    out.raw(",\"thread_id\":")?;
    out.exact_string(header.thread_id())?;
    out.raw(",\"type\":")?;
    out.string(value.record_type().as_str())?;
    out.raw(",\"version\":1}")
}
