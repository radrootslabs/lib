//! Shared authentication of original signed DM route sources.
use super::OperationError;
use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};
use radroots_event::{
    SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};
use radroots_protocol::chat::CapturedDmRoute;
use radroots_transport::target::EndpointUri;
use std::fmt;

/// Authenticated kind-10050 source with its exact original representation.
/// This proves neither current access, target approval, availability nor a
/// dispatch right. Selected targets require their independent host policy.
pub struct SignedDmRelaySource {
    event: SignedEvent,
    participant: PublicKey,
    event_id: EnvelopeEventId,
    created_at: UnixSeconds,
}
impl SignedDmRelaySource {
    /// Bounds and verifies the actual NIP-01 ID/signature before exposing
    /// source fields. Additional signed non-relay tags remain intact.
    pub fn verify(raw: &str) -> Result<Self, OperationError> {
        let invalid = |_| OperationError::InvalidEvidence;
        let limits = EventWireLimits {
            max_raw_json_bytes: 65536,
            max_tag_element_bytes: 32768,
            max_total_tag_bytes: 65536,
            max_extra_fields: 0,
            max_total_extra_json_bytes: 0,
            ..Default::default()
        };
        let wire = Nip01EventWire::parse_json_exact_with_limits(raw, limits).map_err(invalid)?;
        let participant =
            PublicKey::from_hex(&wire.pubkey).map_err(|_| OperationError::InvalidEvidence)?;
        let event_id = wire
            .id
            .parse()
            .map_err(|_| OperationError::InvalidEvidence)?;
        if wire.kind != 10050
            || wire.pubkey != participant.to_hex()
            || wire.created_at == 0
            || wire.created_at > 253_402_300_799
        {
            return Err(OperationError::InvalidEvidence);
        }
        for tag in &wire.tags {
            if tag.first().is_some_and(|v| v == "relay") && (tag.len() != 2 || tag[1].len() > 2048)
            {
                return Err(OperationError::InvalidEvidence);
            }
        }
        let created_at =
            UnixSeconds::new(wire.created_at).map_err(|_| OperationError::InvalidEvidence)?;
        let event = SignedEvent::from_wire_verified_id(wire, raw)
            .map_err(|_| OperationError::InvalidEvidence)?;
        let checked = verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| OperationError::InvalidEvidence)?;
        verify::signature(checked, &Nip01SignatureVerifier)
            .map_err(|_| OperationError::InvalidEvidence)?;
        Ok(Self {
            event,
            participant,
            event_id,
            created_at,
        })
    }
    /// Actual signed author, without workspace or route authority.
    pub const fn participant(&self) -> PublicKey {
        self.participant
    }
    /// Actual signed event identity, separate from a logical chat message.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event_id
    }
    /// Author-claimed time; it is not an independent clock observation.
    pub const fn created_at(&self) -> UnixSeconds {
        self.created_at
    }
    /// Exact original signed source bytes.
    pub fn raw_json(&self) -> &str {
        self.event.raw_json()
    }
    /// Original relay-tag strings. Only actual transport matching/policy may
    /// select an active endpoint; unrelated signed tags authorize none.
    pub fn relay_uris(&self) -> impl Iterator<Item = &str> {
        self.event
            .wire()
            .tags
            .iter()
            .filter(|tag| tag.first().is_some_and(|value| value == "relay") && tag.len() == 2)
            .map(|tag| tag[1].as_str())
    }
    /// Checks a retained exact capture against its actual signed source. This
    /// does not approve another target or weaken the original target policy.
    pub fn validate_capture(&self, route: &CapturedDmRoute) -> Result<(), OperationError> {
        if route.participant() != self.participant
            || route.source_id() != self.event_id
            || route.raw_json() != self.raw_json()
        {
            return Err(OperationError::InvalidEvidence);
        }
        if route.selected_relays().is_empty() {
            return Err(OperationError::InvalidCommand);
        }
        for selected in route.selected_relays() {
            if !self.relay_uris().any(|value| value == selected.as_ref()) {
                return Err(OperationError::UnverifiedApproval);
            }
            let endpoint = EndpointUri::parse(selected.as_ref())
                .map_err(|_| OperationError::InvalidCommand)?;
            if endpoint.as_str().len() > 2048 {
                return Err(OperationError::InvalidCommand);
            }
        }
        Ok(())
    }
}
impl fmt::Debug for SignedDmRelaySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedDmRelaySource(<redacted>)")
    }
}
