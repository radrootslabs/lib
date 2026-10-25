use super::{EncodedRecord, RecordWireError};
use crate::{
    nip01::{EventFields, UnsignedEvent, WireError},
    profile,
};
use core::fmt;
use radroots_domain::{CommerceContent, PublicKey, RecordId, RumorEventId, UnixSeconds};

/// Exact private structured rumor data. Encoding grants no participant,
/// cryptographic, consent, stock or durable issuance authority.
pub struct EncodedCommerceRumor {
    event: UnsignedEvent,
    record_id: RecordId,
    author: PublicKey,
    created_at: UnixSeconds,
    recipients: [PublicKey; 2],
    budget: crate::nip59::PrivateEnvelopeBudget,
}
impl EncodedCommerceRumor {
    /// Binds the complete record's immutable final author/time and empty tags,
    /// checking16KiB content,24KiB complete rumor and both complete enclosing
    /// encryption/seal/wrap budgets before private authoring work.
    pub fn capture(value: &CommerceContent) -> Result<Self, RecordWireError> {
        let record = EncodedRecord::capture(value)?;
        let content = core::str::from_utf8(record.protected_wire())
            .map_err(|_| RecordWireError::InvalidInput)?;
        let header = value.header();
        let fields = EventFields {
            author: header.author(),
            created_at: header.created_at().get(),
            kind: u32::from(profile::COMMERCE_KIND),
            tags: &[],
            content,
        };
        let event =
            UnsignedEvent::encode(&fields, profile::UNSIGNED_RUMOR_MAX_BYTES).map_err(|error| {
                match error {
                    WireError::LimitExceeded => RecordWireError::LimitExceeded,
                    _ => RecordWireError::InvalidInput,
                }
            })?;
        let budget =
            crate::nip59::PrivateEnvelopeBudget::for_rumor(event.json().len(), header.author())
                .map_err(|error| match error {
                    WireError::LimitExceeded => RecordWireError::LimitExceeded,
                    _ => RecordWireError::InvalidInput,
                })?;
        let mut recipients = [header.buyer(), header.seller()];
        recipients.sort();
        Ok(Self {
            event,
            record_id: record.record_id(),
            author: header.author(),
            created_at: header.created_at(),
            recipients,
            budget,
        })
    }
    /// Exact private bytes for protected handling only.
    pub fn protected_wire(&self) -> &str {
        self.event.json()
    }
    /// Domain-separated complete record identity.
    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }
    /// NIP-01 identity of this exact unsigned representation.
    pub const fn event_id(&self) -> RumorEventId {
        RumorEventId::from_bytes(self.event.event_id())
    }
    /// Original record author claim, before participant/permission verification.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Original immutable record time; no wall-clock truth is established.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Complete sorted bilateral peer/self set, without route observations.
    pub const fn recipients(&self) -> &[PublicKey; 2] {
        &self.recipients
    }
    /// Both complete nesting layers, checked before any business signing.
    pub const fn budget(&self) -> crate::nip59::PrivateEnvelopeBudget {
        self.budget
    }
}
impl fmt::Debug for EncodedCommerceRumor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedCommerceRumor(<redacted>)")
    }
}
