//! Closed private NIP-01 data. This module verifies content identities only;
//! cryptographic carrier and recipient admission belongs to a concrete adapter.
use crate::{
    command::{self, CommandError},
    nip01::{self, EventFields, WireError},
    private_json::{self, RawArray},
    profile,
};
use alloc::{string::String, vec::Vec};
use core::{fmt, mem};
use radroots_domain::{PublicKey, RumorEventId};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

/// Source-redacted failures of closed unsigned data admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RumorWireError {
    InvalidInput,
    LimitExceeded,
    IdentityMismatch,
}
impl fmt::Display for RumorWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid rumor input",
            Self::LimitExceeded => "rumor byte limit exceeded",
            Self::IdentityMismatch => "rumor identity mismatch",
        })
    }
}
impl core::error::Error for RumorWireError {}
impl From<WireError> for RumorWireError {
    fn from(value: WireError) -> Self {
        match value {
            WireError::LimitExceeded => Self::LimitExceeded,
            _ => Self::InvalidInput,
        }
    }
}

/// Complete identity-checked rumor data, without a signature or read grant.
/// Original private bytes remain owned by the caller. No automatic private
/// serialization or cloning is implemented.
pub struct DecodedRumor {
    id: RumorEventId,
    author: PublicKey,
    created_at: u64,
    kind: u16,
    tags: Zeroizing<Vec<Vec<String>>>,
    content: Zeroizing<String>,
}
impl fmt::Debug for DecodedRumor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DecodedRumor(<redacted>)")
    }
}
impl DecodedRumor {
    /// Checks complete bytes/depth, exact closed fields, canonical identifiers,
    /// raw u16 kind and the original NIP-01 identity before returning data.
    pub fn decode(raw: &[u8]) -> Result<Self, RumorWireError> {
        if raw.len() > profile::UNSIGNED_RUMOR_MAX_BYTES {
            return Err(RumorWireError::LimitExceeded);
        }
        command::check_frame(raw).map_err(command_error)?;
        let value: RawRumor<'_> = command::parse_object(raw).map_err(command_error)?;
        if value.created_at > 253_402_300_799 {
            return Err(RumorWireError::InvalidInput);
        }
        let maximum = if value.kind == profile::COMMERCE_KIND {
            profile::PRIVATE_RECORD_MAX_BYTES
        } else {
            profile::UNSIGNED_RUMOR_MAX_BYTES
        };
        let content =
            private_json::decode_bytes(value.content.get(), maximum).map_err(command_error)?;
        let mut tags = Zeroizing::new(Vec::new());
        for tag in value.tags.0 {
            let fields: RawArray<'_, { profile::UNSIGNED_RUMOR_MAX_BYTES }> =
                serde_json::from_str(tag.get()).map_err(|_| RumorWireError::InvalidInput)?;
            if fields.0.is_empty() {
                return Err(RumorWireError::InvalidInput);
            }
            let mut strings = Zeroizing::new(Vec::new());
            for field in fields.0 {
                let mut text =
                    private_json::decode_bytes(field.get(), profile::UNSIGNED_RUMOR_MAX_BYTES)
                        .map_err(command_error)?;
                strings.push(mem::take(&mut *text));
            }
            if strings[0].is_empty() || strings[0].chars().any(char::is_control) {
                return Err(RumorWireError::InvalidInput);
            }
            tags.push(mem::take(&mut *strings));
        }
        let fields = EventFields {
            author: value.pubkey,
            created_at: value.created_at,
            kind: u32::from(value.kind),
            tags: &tags,
            content: &content,
        };
        let preimage = nip01::bounded_preimage(&fields, profile::UNSIGNED_RUMOR_MAX_BYTES)?;
        let computed = RumorEventId::from_bytes(Sha256::digest(preimage.as_bytes()).into());
        if computed != value.id {
            return Err(RumorWireError::IdentityMismatch);
        }
        Ok(Self {
            id: value.id,
            author: value.pubkey,
            created_at: value.created_at,
            kind: value.kind,
            tags,
            content,
        })
    }
    /// Exact unsigned event ID, distinct from a structured RecordId.
    pub const fn event_id(&self) -> RumorEventId {
        self.id
    }
    /// Declared canonical author; authenticated carrier binding remains separate.
    pub const fn author(&self) -> PublicKey {
        self.author
    }
    /// Exact supported NIP-01 author timestamp.
    pub const fn created_at(&self) -> u64 {
        self.created_at
    }
    /// Original range-checked wire kind, with no upstream narrowing.
    pub const fn kind(&self) -> u16 {
        self.kind
    }
    /// Whether any ordered tags were retained; no tags are interpreted here.
    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }
    /// Explicit private disclosure for protected profile interpretation.
    pub fn protected_content(&self) -> &str {
        &self.content
    }
    /// This owner's fixed value and all retained decoded capacities. Caller raw
    /// bytes, authenticated carrier evidence and decoded domain models are separate.
    pub fn retained_bytes(&self) -> usize {
        mem::size_of::<Self>()
            + self.content.capacity()
            + self.tags.capacity() * mem::size_of::<Vec<String>>()
            + self
                .tags
                .iter()
                .map(|tag| {
                    tag.capacity() * mem::size_of::<String>()
                        + tag.iter().map(String::capacity).sum::<usize>()
                })
                .sum::<usize>()
    }
}
fn command_error(error: CommandError) -> RumorWireError {
    match error {
        CommandError::LimitExceeded => RumorWireError::LimitExceeded,
        _ => RumorWireError::InvalidInput,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRumor<'a> {
    id: RumorEventId,
    #[serde(deserialize_with = "command::canonical_key")]
    pubkey: PublicKey,
    created_at: u64,
    kind: u16,
    #[serde(borrow)]
    tags: RawArray<'a, { profile::UNSIGNED_RUMOR_MAX_BYTES }>,
    #[serde(borrow)]
    content: &'a RawValue,
}
