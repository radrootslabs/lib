//! Closed NIP-17/44/59 chat profile data. No signer, route or grant is invoked.

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
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Complete unsigned-rumor byte limit, separate from plain chat text.
pub const RUMOR_MAX_BYTES: usize = 24_576;
/// Complete signed seal or gift-wrap limit.
pub const SIGNED_MAX_BYTES: usize = 65_536;
const MAX_TIME: u64 = 253_402_300_799;
const RANDOMIZED_PAST_SECONDS: u64 = 172_800;

mod plan;
pub use plan::{ChatSigningPlan, PLAN_MAX_BYTES};
mod routes;
pub use routes::{CapturedDmRoute, DmRelayAdvertisement};
mod gift_wrap;
pub use gift_wrap::GiftWrapBudget;

fn payload_shape(plaintext_bytes: usize) -> Result<(usize, usize), WireError> {
    // The qualified small-payload profile uses the two-byte length prefix.
    // This does not claim support for the newer extended NIP-44 prefix.
    if !(1..=65_535).contains(&plaintext_bytes) {
        return Err(WireError::InvalidInput);
    }
    let padded = if plaintext_bytes <= 32 {
        32
    } else {
        let power = plaintext_bytes.next_power_of_two();
        let chunk = if power <= 256 { 32 } else { power / 8 };
        plaintext_bytes.div_ceil(chunk) * chunk
    };
    let raw = padded + 67;
    Ok((raw.div_ceil(3) * 4, (3 - raw % 3) % 3))
}
/// Exact encoded size of the qualified two-byte-prefix NIP-44 v2 payload.
/// This is a size calculation, without encryption or a MAC-validity claim.
pub fn nip44_v2_payload_len(plaintext_bytes: usize) -> Result<usize, WireError> {
    Ok(payload_shape(plaintext_bytes)?.0)
}

/// Conservative complete nested sizes using the largest supported timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatBudget {
    rumor_ciphertext_bytes: usize,
    signed_seal_max_bytes: usize,
    envelope_ciphertext_max_bytes: usize,
    signed_envelope_max_bytes: usize,
}
impl ChatBudget {
    /// Measures both nesting layers before any business seal signing.
    pub fn for_rumor(rumor_bytes: usize, participant: PublicKey) -> Result<Self, WireError> {
        if rumor_bytes > RUMOR_MAX_BYTES {
            return Err(WireError::LimitExceeded);
        }
        let rumor_ciphertext_bytes = nip44_v2_payload_len(rumor_bytes)?;
        let fields = EventFields {
            author: participant,
            created_at: MAX_TIME,
            kind: 13,
            tags: &[],
            content: "",
        };
        let signed_seal_max_bytes =
            crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)? + rumor_ciphertext_bytes;
        let envelope_ciphertext_max_bytes = nip44_v2_payload_len(signed_seal_max_bytes)?;
        let tags = [vec![String::from("p"), participant.to_hex()]];
        let fields = EventFields {
            author: participant,
            created_at: MAX_TIME,
            kind: 1059,
            tags: &tags,
            content: "",
        };
        let signed_envelope_max_bytes = crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)?
            + envelope_ciphertext_max_bytes;
        if signed_seal_max_bytes > SIGNED_MAX_BYTES || signed_envelope_max_bytes > SIGNED_MAX_BYTES
        {
            return Err(WireError::LimitExceeded);
        }
        Ok(Self {
            rumor_ciphertext_bytes,
            signed_seal_max_bytes,
            envelope_ciphertext_max_bytes,
            signed_envelope_max_bytes,
        })
    }
    /// Exact encoded rumor ciphertext size.
    pub const fn rumor_ciphertext_bytes(self) -> usize {
        self.rumor_ciphertext_bytes
    }
    /// Maximum complete seal JSON size.
    pub const fn signed_seal_max_bytes(self) -> usize {
        self.signed_seal_max_bytes
    }
    /// Maximum outer encoded ciphertext size.
    pub const fn envelope_ciphertext_max_bytes(self) -> usize {
        self.envelope_ciphertext_max_bytes
    }
    /// Maximum complete signed gift-wrap size.
    pub const fn signed_envelope_max_bytes(self) -> usize {
        self.signed_envelope_max_bytes
    }
}

/// One exact unsigned kind-14 message shared by the complete peer/self set.
/// Selectors and codec output authenticate no parent, source or participant.
pub struct ChatRumor {
    event: UnsignedEvent,
    operation_id: OperationId,
    command_digest: CommandDigest,
    author: PublicKey,
    created_at: UnixSeconds,
    recipients: Vec<PublicKey>,
    budget: ChatBudget,
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
        let budget = ChatBudget::for_rumor(event.json().len(), author)?;
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
    pub const fn budget(&self) -> ChatBudget {
        self.budget
    }
}
impl fmt::Debug for ChatRumor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatRumor(<redacted>)")
    }
}

fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
fn ciphertext_shape(value: &str, plaintext_bytes: usize) -> Result<(), WireError> {
    let (size, padding) = payload_shape(plaintext_bytes)?;
    if value.len() != size {
        return Err(WireError::InvalidInput);
    }
    let bytes = value.as_bytes();
    let split = size - padding;
    if !bytes[..split].iter().all(|b| sextet(*b).is_some())
        || !bytes[split..].iter().all(|b| *b == b'=')
        || bytes[0] != b'A'
        || !(b'g'..=b'v').contains(&bytes[1])
    {
        return Err(WireError::InvalidInput);
    }
    let last = sextet(bytes[split - 1]).ok_or(WireError::InvalidInput)?;
    if (padding == 1 && last & 3 != 0) || (padding == 2 && last & 15 != 0) {
        return Err(WireError::InvalidInput);
    }
    Ok(())
}

/// Bounded exact kind-13 data for one recipient, with no signature/authority.
pub struct ChatSeal {
    operation_id: OperationId,
    command_digest: CommandDigest,
    message_id: MessageId,
    author: PublicKey,
    recipient: PublicKey,
    created_at: UnixSeconds,
    ciphertext: Zeroizing<String>,
    event_id: [u8; 32],
}
impl ChatSeal {
    /// Validates the recipient, randomized past timestamp, complete nesting and
    /// canonical encoded payload shape before copying ciphertext. MAC validity
    /// belongs to qualified cryptography, not this structural constructor.
    pub fn new(
        rumor: &ChatRumor,
        recipient: PublicKey,
        created_at: UnixSeconds,
        ciphertext: &str,
    ) -> Result<Self, WireError> {
        if !rumor.recipients.contains(&recipient)
            || created_at.get() == 0
            || created_at > rumor.created_at
            || rumor.created_at.get() - created_at.get() > RANDOMIZED_PAST_SECONDS
        {
            return Err(WireError::InvalidInput);
        }
        ciphertext_shape(ciphertext, rumor.json().len())?;
        let fields = EventFields {
            author: rumor.author,
            created_at: created_at.get(),
            kind: 13,
            tags: &[],
            content: ciphertext,
        };
        crate::nip01::signed_json_len(&fields, SIGNED_MAX_BYTES)?;
        let preimage = crate::nip01::bounded_preimage(&fields, SIGNED_MAX_BYTES)?;
        let event_id = Sha256::digest(preimage.as_bytes()).into();
        Ok(Self {
            operation_id: rumor.operation_id,
            command_digest: rumor.command_digest,
            message_id: rumor.message_id(),
            author: rumor.author,
            recipient,
            created_at,
            ciphertext: Zeroizing::new(String::from(ciphertext)),
            event_id,
        })
    }
    /// Borrows the exact immutable fields; seal tags are always empty.
    pub fn fields(&self) -> EventFields<'_> {
        EventFields {
            author: self.author,
            created_at: self.created_at.get(),
            kind: 13,
            tags: &[],
            content: &self.ciphertext,
        }
    }
    /// Returns the originating captured operation descriptor, without authority.
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    /// Returns the originating semantic command commitment.
    pub const fn command_digest(&self) -> CommandDigest {
        self.command_digest
    }
    /// Returns the exact unsigned rumor this seal was constructed to carry.
    /// The qualified encryption provider must separately bind its plaintext.
    pub const fn message_id(&self) -> MessageId {
        self.message_id
    }
    /// Returns the exact recipient, separate from public seal tags.
    pub const fn recipient(&self) -> PublicKey {
        self.recipient
    }
    /// Returns the exact expected signed event ID.
    pub const fn event_id(&self) -> [u8; 32] {
        self.event_id
    }
}
impl fmt::Debug for ChatSeal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChatSeal(<redacted>)")
    }
}
