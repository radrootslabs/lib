//! Exact structured-commerce seal data, without permission, crypto or retention.
use super::EncodedCommerceRumor;
use crate::{
    nip01::{EventFields, WireError},
    nip59::PrivateSeal,
};
use core::fmt;
use radroots_domain::{CommandDigest, OperationId, PublicKey, RecordId, RumorEventId, UnixSeconds};

/// One recipient's exact kind13 intent for an unchanged structured rumor.
/// Identity fields are data claims. A concrete executor must compare the entire
/// record/rumor and command against its actual retained T0 before binding T1.
pub struct CommerceSeal {
    operation: OperationId,
    command: CommandDigest,
    record: RecordId,
    rumor: RumorEventId,
    seal: PrivateSeal,
}
impl CommerceSeal {
    /// Requires an actual bilateral recipient and the shared complete NIP59
    /// structural limits. Encryption/MAC and current execution are separate.
    pub fn new(
        operation: OperationId,
        command: CommandDigest,
        rumor: &EncodedCommerceRumor,
        recipient: PublicKey,
        created_at: UnixSeconds,
        ciphertext: &str,
    ) -> Result<Self, WireError> {
        if !rumor.recipients().contains(&recipient) {
            return Err(WireError::InvalidInput);
        }
        let seal = PrivateSeal::new(
            rumor.author(),
            recipient,
            created_at,
            rumor.created_at(),
            rumor.protected_wire().len(),
            ciphertext,
        )?;
        Ok(Self {
            operation,
            command,
            record: rumor.record_id(),
            rumor: rumor.event_id(),
            seal,
        })
    }
    /// Exact declared captured operation identity; no store completion proof.
    pub const fn operation_id(&self) -> OperationId {
        self.operation
    }
    /// Exact declared command commitment; no approval proof.
    pub const fn command_digest(&self) -> CommandDigest {
        self.command
    }
    /// Original domain-separated structured record identity.
    pub const fn record_id(&self) -> RecordId {
        self.record
    }
    /// Exact unchanged inner NIP01 rumor identity.
    pub const fn rumor_id(&self) -> RumorEventId {
        self.rumor
    }
    /// Exact private recipient; never a public kind13 tag.
    pub const fn recipient(&self) -> PublicKey {
        self.seal.recipient()
    }
    /// Exact expected event identity; this provides no signature evidence.
    pub const fn event_id(&self) -> [u8; 32] {
        self.seal.event_id()
    }
    /// Original immutable NIP01 fields for qualified signing and comparison.
    pub fn fields(&self) -> EventFields<'_> {
        self.seal.fields()
    }
}
impl fmt::Debug for CommerceSeal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CommerceSeal(<redacted>)")
    }
}
