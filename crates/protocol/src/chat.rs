//! Closed NIP-17/44/59 chat profile data. No signer, route or grant is invoked.

use crate::nip59::{PrivateEnvelopeBudget, PrivateSeal};
use crate::{
    command::CapturedChatCommand,
    nip01::{EventFields, UnsignedEvent, WireError},
};
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt;
use radroots_domain::{CommandDigest, MessageId, OperationId, PublicKey, UnixSeconds};

/// Complete unsigned-rumor byte limit, separate from plain chat text.
pub const RUMOR_MAX_BYTES: usize = crate::profile::UNSIGNED_RUMOR_MAX_BYTES;
/// Complete signed seal or gift-wrap limit.
pub const SIGNED_MAX_BYTES: usize = crate::profile::SIGNED_EVENT_MAX_BYTES;

mod plan;
pub use plan::{ChatSigningPlan, PLAN_MAX_BYTES};
mod gift_wrap;
pub use gift_wrap::GiftWrapBudget;

/// One exact unsigned kind-14 message shared by the complete peer/self set.
/// Selectors and codec output authenticate no parent, source or participant.
pub struct ChatRumor {
    event: UnsignedEvent,
    operation_id: OperationId,
    command_digest: CommandDigest,
    author: PublicKey,
    created_at: UnixSeconds,
    recipients: Vec<PublicKey>,
    budget: PrivateEnvelopeBudget,
}
impl ChatRumor {
    /// Encodes captured plain text and ordered profile tags, preserving Unicode.
    /// Authoritative execution must separately verify captured scope/history and
    /// bind this exact timestamp to its retained preparation.
    pub fn from_command(
        command: &CapturedChatCommand,
        created_at: UnixSeconds,
    ) -> Result<Self, WireError> {
        if created_at.get() == 0 {
            return Err(WireError::InvalidInput);
        }
        let author = command.context().actor().participant();
        let peer = command.peer();
        let mut tags = vec![
            vec![String::from("p"), peer.to_hex()],
            vec![
                String::from("radroots"),
                String::from("thread"),
                command.thread_id().to_string(),
            ],
            vec![
                String::from("radroots"),
                String::from("operation"),
                command.context().operation_id().to_string(),
            ],
        ];
        if let Some(source) = command.source() {
            tags.push(vec![
                String::from("radroots"),
                String::from("source"),
                String::from("30402"),
                source.author().to_hex(),
                String::from(source.d()),
                source.event_id().to_string(),
            ]);
        }
        if let Some(parent) = command.parent() {
            tags.push(vec![String::from("e"), parent.to_string()]);
        }
        let fields = EventFields {
            author,
            created_at: created_at.get(),
            kind: 14,
            tags: &tags,
            content: command.text(),
        };
        let event = UnsignedEvent::encode(&fields, RUMOR_MAX_BYTES)?;
        let budget = PrivateEnvelopeBudget::for_rumor(event.json().len(), author)?;
        let mut recipients = vec![author];
        if peer != author {
            recipients.push(peer);
        }
        recipients.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        Ok(Self {
            event,
            operation_id: command.context().operation_id(),
            command_digest: command.digest(),
            author,
            created_at,
            recipients,
            budget,
        })
    }
    /// Returns the logical message ID, independent of per-recipient wrapping.
    pub const fn message_id(&self) -> MessageId {
        MessageId::from_bytes(self.event.event_id())
    }
    /// Borrows exact unsigned JSON for protected retention and encryption.
    pub fn json(&self) -> &str {
        self.event.json()
    }
    /// Returns the captured author, without proving authority.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Returns the immutable rumor timestamp.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Borrows the complete sorted, unique peer/self recipient set.
    pub fn recipients(&self) -> &[PublicKey] {
        &self.recipients
    }
    /// Returns pre-signing complete nesting bounds.
    pub const fn budget(&self) -> PrivateEnvelopeBudget {
        self.budget
    }
}
impl fmt::Debug for ChatRumor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatRumor(<redacted>)")
    }
}

/// Bounded exact kind-13 data for one recipient, with no signature/authority.
pub struct ChatSeal {
    operation_id: OperationId,
    command_digest: CommandDigest,
    message_id: MessageId,
    seal: PrivateSeal,
}
impl ChatSeal {
    /// Binds the exact chat identity and complete recipient set to shared
    /// NIP59 structural checks. Encryption and actual retention remain separate.
    pub fn new(
        rumor: &ChatRumor,
        recipient: PublicKey,
        created_at: UnixSeconds,
        ciphertext: &str,
    ) -> Result<Self, WireError> {
        if !rumor.recipients.contains(&recipient) {
            return Err(WireError::InvalidInput);
        }
        Ok(Self {
            operation_id: rumor.operation_id,
            command_digest: rumor.command_digest,
            message_id: rumor.message_id(),
            seal: PrivateSeal::new(
                rumor.author,
                recipient,
                created_at,
                rumor.created_at,
                rumor.json().len(),
                ciphertext,
            )?,
        })
    }
    /// Borrows the original exact kind13 fields.
    pub fn fields(&self) -> EventFields<'_> {
        self.seal.fields()
    }
    /// Originating captured command identity; no execution permission.
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    /// Originating semantic command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    /// Exact original unsigned chat identity, distinct from this seal ID.
    pub const fn message_id(&self) -> MessageId {
        self.message_id
    }
    /// Exact private recipient.
    pub const fn recipient(&self) -> PublicKey {
        self.seal.recipient()
    }
    /// Exact expected signed event identity.
    pub const fn event_id(&self) -> [u8; 32] {
        self.seal.event_id()
    }
}
impl fmt::Debug for ChatSeal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatSeal(<redacted>)")
    }
}
