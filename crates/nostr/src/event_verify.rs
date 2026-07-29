#![forbid(unsafe_code)]

use radroots_event::{
    admission::{Error as VerificationError, RawEvent, SignatureVerifier},
    envelope::EventEnvelope,
};
use radroots_event_codec::verify;

/// Deterministic BIP-340 verification supplied by the Nostr adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct NostrSignatureVerifier;

impl SignatureVerifier for NostrSignatureVerifier {
    fn verify_signature(&self, event: &EventEnvelope) -> Result<(), VerificationError> {
        validate_nostr_kind(event)?;
        let public_key = nostr::secp256k1::XOnlyPublicKey::from_slice(event.author().as_bytes())
            .map_err(|_| VerificationError::MalformedEnvelope)?;
        let signature = nostr::secp256k1::schnorr::Signature::from_slice(event.sig().as_bytes())
            .map_err(|_| VerificationError::MalformedEnvelope)?;
        let message = nostr::secp256k1::Message::from_digest(*event.id().as_bytes());
        nostr::SECP256K1
            .verify_schnorr(&signature, &message, &public_key)
            .map_err(|_| VerificationError::SignatureInvalid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsNostrEventVerification {
    Verified,
    IdVerified,
    IdMismatch,
    SignatureInvalid,
    MalformedEnvelope,
}

pub fn radroots_nostr_verify_event(event: &EventEnvelope) -> RadrootsNostrEventVerification {
    let result = validate_nostr_kind(event)
        .and_then(|()| verify::id(RawEvent::new(event.clone())))
        .and_then(|event| verify::signature(event, &NostrSignatureVerifier));
    match result {
        Ok(_) => RadrootsNostrEventVerification::Verified,
        Err(error) => verification_error_status(&error),
    }
}

pub fn radroots_nostr_verify_event_id(event: &EventEnvelope) -> RadrootsNostrEventVerification {
    let result = validate_nostr_kind(event).and_then(|()| verify::id(RawEvent::new(event.clone())));
    match result {
        Ok(_) => RadrootsNostrEventVerification::IdVerified,
        Err(error) => verification_error_status(&error),
    }
}

fn validate_nostr_kind(event: &EventEnvelope) -> Result<(), VerificationError> {
    u16::try_from(event.kind_u32())
        .map(|_| ())
        .map_err(|_| VerificationError::MalformedEnvelope)
}

fn verification_error_status(error: &VerificationError) -> RadrootsNostrEventVerification {
    match error {
        VerificationError::IdMismatch { .. } => RadrootsNostrEventVerification::IdMismatch,
        VerificationError::SignatureInvalid => RadrootsNostrEventVerification::SignatureInvalid,
        VerificationError::MalformedEnvelope | VerificationError::ContractValidation(_) => {
            RadrootsNostrEventVerification::MalformedEnvelope
        }
        _ => RadrootsNostrEventVerification::MalformedEnvelope,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_convert::radroots_event_from_nostr;
    use crate::events::radroots_nostr_build_event_unchecked;
    use crate::test_fixtures::FIXTURE_ALICE;
    use crate::types::{RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp};
    use radroots_event::{
        envelope::EventEnvelopeParts, envelope::kind::KIND_POST,
        wire::compute_canonical_nip01_event_id,
    };

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn signed_event() -> EventEnvelope {
        let raw_event = radroots_nostr_build_event_unchecked(
            KIND_POST,
            "hello",
            vec![vec!["t".to_owned(), "soil".to_owned()]],
        )
        .expect("builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed event");
        radroots_event_from_nostr(&raw_event).expect("Radroots event envelope")
    }

    fn envelope_with(
        event: &EventEnvelope,
        content: String,
        kind: u32,
        sig: String,
    ) -> EventEnvelope {
        EventEnvelope::new(EventEnvelopeParts {
            id: event.id_hex(),
            author: event.author().to_hex().to_owned(),
            created_at: event.created_at_u64(),
            kind,
            tags: event.tags_as_vec(),
            content,
            sig,
        })
        .expect("envelope")
    }

    #[test]
    fn verifies_signed_event_id_and_signature() {
        let event = signed_event();

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::Verified
        );
        assert_eq!(
            radroots_nostr_verify_event_id(&event),
            RadrootsNostrEventVerification::IdVerified
        );
    }

    #[test]
    fn reports_id_mismatch_before_signature_checks() {
        let original = signed_event();
        let event = envelope_with(
            &original,
            "tampered".to_owned(),
            original.kind_u32(),
            original.signature_hex(),
        );

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::IdMismatch
        );
    }

    #[test]
    fn reports_signature_invalid_for_valid_id_with_wrong_signature() {
        let original = signed_event();
        let mut sig = original.signature_hex();
        let replacement = if sig.starts_with('0') { "1" } else { "0" };
        sig.replace_range(0..1, replacement);
        let event = envelope_with(
            &original,
            original.content().to_owned(),
            original.kind_u32(),
            sig,
        );

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::SignatureInvalid
        );
    }

    #[test]
    fn out_of_range_kind_precedes_id_mismatch() {
        let original = signed_event();
        let event = envelope_with(
            &original,
            original.content().to_owned(),
            u32::from(u16::MAX) + 1,
            original.signature_hex(),
        );

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::MalformedEnvelope
        );
        assert_eq!(
            radroots_nostr_verify_event_id(&event),
            RadrootsNostrEventVerification::MalformedEnvelope
        );
    }

    #[test]
    fn reports_malformed_envelope_for_id_valid_out_of_range_kind() {
        let original = signed_event();
        let kind = u32::from(u16::MAX) + 1;
        let id = compute_canonical_nip01_event_id(
            &original.author().to_hex(),
            original.created_at_u64(),
            kind,
            &original.tags_as_vec(),
            original.content(),
        )
        .expect("canonical id")
        .into_string();
        let event = EventEnvelope::new(EventEnvelopeParts {
            id,
            author: original.author().to_hex().to_owned(),
            created_at: original.created_at_u64(),
            kind,
            tags: original.tags_as_vec(),
            content: original.content().to_owned(),
            sig: original.signature_hex(),
        })
        .expect("envelope");

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::MalformedEnvelope
        );
        assert_eq!(
            radroots_nostr_verify_event_id(&event),
            RadrootsNostrEventVerification::MalformedEnvelope
        );
    }
}
