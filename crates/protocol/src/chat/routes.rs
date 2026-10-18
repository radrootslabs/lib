use crate::nip01::{EventFields, WireError};
use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};
use core::fmt;
use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};

const MAX_TAGS: usize = 1024;
const MAX_URI_BYTES: usize = 2048;

/// Bounded kind-10050 tag projection. Construction authenticates no signature,
/// route permission, transport availability or participant workspace authority.
/// The advertised list is distinct from the sixteen-relay active-session cap.
pub struct DmRelayAdvertisement {
    participant: PublicKey,
    created_at: UnixSeconds,
    relays: Vec<Box<str>>,
}
impl DmRelayAdvertisement {
    /// Projects exact relay tags after bounding their complete serialized size.
    /// Additional signed tags are preserved by the caller's original event.
    /// An empty list is explicit unavailable DM information, never a fallback.
    pub fn from_tags(
        participant: PublicKey,
        created_at: UnixSeconds,
        tags: &[Vec<String>],
    ) -> Result<Self, WireError> {
        if tags.len() > MAX_TAGS {
            return Err(WireError::LimitExceeded);
        }
        let fields = EventFields {
            author: participant,
            created_at: created_at.get(),
            kind: 10050,
            tags,
            content: "",
        };
        crate::nip01::signed_json_len(&fields, super::SIGNED_MAX_BYTES)?;
        let mut relays = Vec::new();
        let mut seen = BTreeSet::new();
        for tag in tags {
            if tag.first().is_none_or(|value| value != "relay") {
                continue;
            }
            if tag.len() != 2 {
                return Err(WireError::InvalidInput);
            }
            let value = tag[1].as_str();
            if value.len() > MAX_URI_BYTES {
                return Err(WireError::LimitExceeded);
            }
            if !(value.starts_with("wss://") || value.starts_with("ws://"))
                || value.trim() != value
                || value.chars().any(char::is_control)
            {
                return Err(WireError::InvalidInput);
            }
            if seen.insert(value) {
                relays.push(Box::from(value));
            }
        }
        Ok(Self {
            participant,
            created_at,
            relays,
        })
    }
    /// Returns the described participant, without proving ownership.
    pub const fn participant(&self) -> PublicKey {
        self.participant
    }
    /// Returns the author's claimed event time, not a trusted clock observation.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Borrows unique advertised strings in their original preference order.
    /// Actual URI syntax/network policy and session limits remain transport-owned.
    pub fn advertised_relays(&self) -> &[Box<str>] {
        &self.relays
    }
}
impl fmt::Debug for DmRelayAdvertisement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DmRelayAdvertisement(<redacted>)")
    }
}

/// Immutable captured route descriptors and original source bytes. This data
/// constructor verifies neither signature, source identity, target approval nor
/// transport availability. Execution must authenticate the source and enforce
/// its exact selected subset before dispatch, including after store recovery.
pub struct CapturedDmRoute {
    participant: PublicKey,
    source_id: EnvelopeEventId,
    raw_json: Box<str>,
    selected: Box<[Box<str>]>,
}
impl CapturedDmRoute {
    /// Bounds the original JSON and complete selected strings before copying.
    /// Session composition separately enforces its combined relay limit.
    pub fn from_observed_descriptor(
        participant: PublicKey,
        source_id: EnvelopeEventId,
        raw_json: &str,
        selected: &[String],
    ) -> Result<Self, WireError> {
        if raw_json.len() > super::SIGNED_MAX_BYTES || selected.len() > 16 {
            return Err(WireError::LimitExceeded);
        }
        if selected.is_empty() {
            return Err(WireError::InvalidInput);
        }
        let root: &serde_json::value::RawValue =
            serde_json::from_str(raw_json).map_err(|_| WireError::InvalidInput)?;
        if !root.get().starts_with('{') {
            return Err(WireError::InvalidInput);
        }
        let mut seen = BTreeSet::new();
        for value in selected {
            if value.len() > MAX_URI_BYTES {
                return Err(WireError::LimitExceeded);
            }
            if !(value.starts_with("wss://") || value.starts_with("ws://"))
                || value.trim() != value
                || value.chars().any(char::is_control)
                || !seen.insert(value.as_str())
            {
                return Err(WireError::InvalidInput);
            }
        }
        Ok(Self {
            participant,
            source_id,
            raw_json: Box::from(raw_json),
            selected: selected
                .iter()
                .map(|value| Box::from(value.as_str()))
                .collect(),
        })
    }
    /// Returns the described recipient.
    pub const fn participant(&self) -> PublicKey {
        self.participant
    }
    /// Returns the reported source event identity; this getter supplies no proof.
    pub const fn source_id(&self) -> EnvelopeEventId {
        self.source_id
    }
    /// Borrows exact original bytes, including insignificant whitespace.
    pub fn raw_json(&self) -> &str {
        &self.raw_json
    }
    /// Borrows the frozen selected strings in preference order.
    pub fn selected_relays(&self) -> &[Box<str>] {
        &self.selected
    }
}
impl fmt::Debug for CapturedDmRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapturedDmRoute(<redacted>)")
    }
}
