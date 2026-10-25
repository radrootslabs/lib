use crate::provider::ProviderError;
#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::fmt;
use radroots_domain::{EnvelopeEventId, PublicKey, UnixSeconds};
use radroots_event::{
    SignedEvent,
    wire::{EventWireLimits, Nip01EventWire},
};
use radroots_event_codec::verify::{self, Nip01SignatureVerifier, RawEvent};
use radroots_protocol::nip59::DmRelayAdvertisement;

/// Actual ID/signature-verified kind-10050 observation. This verifies the
/// participant's advertisement only, not its global freshness, routability,
/// transport configuration, workspace grant or permission to issue a message.
/// No public field constructor, serialization or automatic payload Debug exists.
pub struct VerifiedDmRelayAdvertisement {
    event_id: EnvelopeEventId,
    advertisement: DmRelayAdvertisement,
    raw_json: String,
}
impl VerifiedDmRelayAdvertisement {
    /// Verifies bounded exact NIP-01 fields, event ID, expected participant and
    /// Schnorr signature before returning retained routing data. The complete
    /// raw budget is checked before any decoder-owned copy.
    /// The caller retains unsupported/unverified input for its bounded evidence
    /// policy; this borrowed-input validator never erases that original input.
    pub fn verify(raw_json: &str, expected_participant: PublicKey) -> Result<Self, ProviderError> {
        if raw_json.len() > super::SIGNED_EVENT_MAX_BYTES {
            return Err(ProviderError::LimitExceeded);
        }
        let limits = EventWireLimits {
            max_raw_json_bytes: super::SIGNED_EVENT_MAX_BYTES,
            max_extra_fields: 0,
            max_total_extra_json_bytes: 0,
            ..EventWireLimits::default()
        };
        let wire = Nip01EventWire::parse_json_exact_with_limits(raw_json, limits)
            .map_err(|_| ProviderError::AuthenticationFailed)?;
        let event = SignedEvent::from_wire_verified_id(wire, raw_json)
            .map_err(|_| ProviderError::AuthenticationFailed)?;
        let verified = verify::id(RawEvent::new(event.envelope().clone()))
            .map_err(|_| ProviderError::AuthenticationFailed)?;
        verify::signature(verified, &Nip01SignatureVerifier)
            .map_err(|_| ProviderError::AuthenticationFailed)?;
        if event.wire().pubkey != expected_participant.to_hex() {
            return Err(ProviderError::AuthenticationFailed);
        }
        if event.wire().kind != 10050 {
            return Err(ProviderError::UnsupportedVersion);
        }
        let created_at =
            UnixSeconds::new(event.wire().created_at).map_err(|_| ProviderError::InvalidInput)?;
        let advertisement =
            DmRelayAdvertisement::from_tags(expected_participant, created_at, &event.wire().tags)
                .map_err(|error| match error {
                radroots_protocol::nip01::WireError::LimitExceeded => ProviderError::LimitExceeded,
                radroots_protocol::nip01::WireError::InvalidInput => ProviderError::InvalidInput,
            })?;
        Ok(Self {
            event_id: EnvelopeEventId::from_bytes(*event.id().as_bytes()),
            advertisement,
            raw_json: String::from(raw_json),
        })
    }
    /// Returns the authenticated NIP-01 source identity.
    pub const fn event_id(&self) -> EnvelopeEventId {
        self.event_id
    }
    /// Borrows the bounded authenticated tag projection.
    pub const fn advertisement(&self) -> &DmRelayAdvertisement {
        &self.advertisement
    }
    /// Borrows the exact original source bytes for retained plan evidence.
    pub fn raw_json(&self) -> &str {
        &self.raw_json
    }
    /// Freezes an explicit subset of this actual authenticated advertisement.
    /// Native URI/network policy and runtime availability are still required;
    /// this value supplies no workspace or signing permission.
    pub fn capture_route(
        &self,
        selected: &[String],
    ) -> Result<radroots_protocol::nip59::CapturedDmRoute, ProviderError> {
        if selected.iter().any(|target| {
            !self
                .advertisement
                .advertised_relays()
                .iter()
                .any(|advertised| advertised.as_ref() == target)
        }) {
            return Err(ProviderError::InvalidInput);
        }
        radroots_protocol::nip59::CapturedDmRoute::from_observed_descriptor(
            self.advertisement.participant(),
            self.event_id,
            &self.raw_json,
            selected,
        )
        .map_err(|error| match error {
            radroots_protocol::nip01::WireError::LimitExceeded => ProviderError::LimitExceeded,
            radroots_protocol::nip01::WireError::InvalidInput => ProviderError::InvalidInput,
        })
    }
}
impl fmt::Debug for VerifiedDmRelayAdvertisement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerifiedDmRelayAdvertisement(<redacted>)")
    }
}
