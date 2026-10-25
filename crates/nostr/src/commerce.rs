//! Complete authenticated record admission. This supplies provenance, not a
//! business transition, current authority, agreement, inventory or physical fact.
use crate::private_ingress::{self, AuthenticatedCarrier, OpeningError};
use alloc::boxed::Box;
use radroots_protocol::commerce::{self, DecodedRecord, RecordWireError};
use radroots_protocol::profile;
use radroots_signing::provider::{ParticipantProvider, ProviderCall};

/// Only a fully admitted known profile can construct this record.
#[derive(Debug)]
pub struct AuthenticatedRecord {
    carrier: AuthenticatedCarrier,
    record: Box<DecodedRecord>,
}
impl AuthenticatedRecord {
    pub const fn carrier(&self) -> &AuthenticatedCarrier {
        &self.carrier
    }
    pub fn record(&self) -> &DecodedRecord {
        &self.record
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedReason {
    CarrierKind,
    Profile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectedReason {
    InvalidProfile,
    LimitExceeded,
    RecordIdentityMismatch,
    CarrierMismatch,
    RecipientMismatch,
}

/// Authenticated but non-actionable evidence, retained for protected storage.
#[derive(Debug)]
pub struct NonActionable<R> {
    carrier: AuthenticatedCarrier,
    reason: R,
}
impl<R: Copy> NonActionable<R> {
    pub const fn carrier(&self) -> &AuthenticatedCarrier {
        &self.carrier
    }
    pub const fn reason(&self) -> R {
        self.reason
    }
}
#[derive(Debug)]
pub enum RecordOpening {
    Known(AuthenticatedRecord),
    Unsupported(NonActionable<UnsupportedReason>),
    Rejected(NonActionable<RejectedReason>),
}

/// Interprets only already authenticated evidence. Future profiles retain
/// original protected bytes and cannot become a known record through fallback.
pub fn admit(carrier: AuthenticatedCarrier) -> RecordOpening {
    let unsupported =
        |carrier, reason| RecordOpening::Unsupported(NonActionable { carrier, reason });
    let rejected = |carrier, reason| RecordOpening::Rejected(NonActionable { carrier, reason });
    let rumor = carrier.rumor();
    if rumor.kind() != profile::COMMERCE_KIND {
        return unsupported(carrier, UnsupportedReason::CarrierKind);
    }
    let record = match commerce::decode(rumor.protected_content().as_bytes()) {
        Ok(value) => value,
        Err(RecordWireError::UnsupportedProfile) => {
            return unsupported(carrier, UnsupportedReason::Profile);
        }
        Err(error) => {
            let reason = match error {
                RecordWireError::LimitExceeded => RejectedReason::LimitExceeded,
                RecordWireError::IdentityMismatch => RejectedReason::RecordIdentityMismatch,
                RecordWireError::CarrierMismatch => RejectedReason::CarrierMismatch,
                _ => RejectedReason::InvalidProfile,
            };
            return rejected(carrier, reason);
        }
    };
    if rumor.has_tags()
        || record
            .check_rumor_fields(rumor.author(), rumor.created_at(), rumor.kind())
            .is_err()
    {
        return rejected(carrier, RejectedReason::CarrierMismatch);
    }
    let header = record.content().header();
    if carrier.recipient() != header.buyer() && carrier.recipient() != header.seller() {
        return rejected(carrier, RejectedReason::RecipientMismatch);
    }
    RecordOpening::Known(AuthenticatedRecord {
        carrier,
        record: Box::new(record),
    })
}

pub async fn open(
    provider: &dyn ParticipantProvider,
    call: ProviderCall,
    raw_outer: &str,
) -> Result<RecordOpening, OpeningError> {
    Ok(admit(
        private_ingress::open(provider, call, raw_outer).await?,
    ))
}
mod signing_plan;
pub use signing_plan::VerifiedCommerceSigningPlan;
