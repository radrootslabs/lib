use super::{EncodedRecord, RecordWireError};
use crate::{
    nip01::{EventFields, UnsignedEvent, WireError},
    profile,
};
use core::fmt;
use radroots_domain::{CommerceContent, RecordId, RumorEventId};

/// Exact private structured rumor data. Encoding grants no participant,
/// cryptographic, consent, stock or durable issuance authority.
pub struct EncodedCommerceRumor {
    event: UnsignedEvent,
    record_id: RecordId,
}
impl EncodedCommerceRumor {
    /// Binds the complete record's immutable final author/time and empty tags,
    /// checking16KiB content and24KiB complete rumor before private allocation.
    /// Enclosing encryption/seal/wrap costs remain separately enforceable.
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
        Ok(Self {
            event,
            record_id: record.record_id(),
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
}
impl fmt::Debug for EncodedCommerceRumor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncodedCommerceRumor(<redacted>)")
    }
}
