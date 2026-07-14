#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::str::FromStr;

use crate::types::{
    RadrootsNostrEvent as RadrootsNostrRawEvent, RadrootsNostrEventId, RadrootsNostrKind,
    RadrootsNostrPublicKey, RadrootsNostrTag, RadrootsNostrTimestamp,
};
use nostr::secp256k1::schnorr::Signature;
use radroots_event::RadrootsEventEnvelope;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsNostrEventVerification {
    Verified,
    IdVerified,
    IdMismatch,
    SignatureInvalid,
    MalformedEnvelope,
}

pub fn radroots_nostr_verify_event(
    event: &RadrootsEventEnvelope,
) -> RadrootsNostrEventVerification {
    let Some(raw_event) = raw_event_from_radroots(event) else {
        return RadrootsNostrEventVerification::MalformedEnvelope;
    };
    if !raw_event.verify_id() {
        return RadrootsNostrEventVerification::IdMismatch;
    }
    if !raw_event.verify_signature() {
        return RadrootsNostrEventVerification::SignatureInvalid;
    }
    RadrootsNostrEventVerification::Verified
}

pub fn radroots_nostr_verify_event_id(
    event: &RadrootsEventEnvelope,
) -> RadrootsNostrEventVerification {
    let Some(raw_event) = raw_event_from_radroots(event) else {
        return RadrootsNostrEventVerification::MalformedEnvelope;
    };
    if raw_event.verify_id() {
        RadrootsNostrEventVerification::IdVerified
    } else {
        RadrootsNostrEventVerification::IdMismatch
    }
}

fn raw_event_from_radroots(event: &RadrootsEventEnvelope) -> Option<RadrootsNostrRawEvent> {
    let id = RadrootsNostrEventId::from_hex(event.id_str()).ok()?;
    let public_key = RadrootsNostrPublicKey::from_hex(event.author_str()).ok()?;
    let kind_u16 = u16::try_from(event.kind_u32()).ok()?;
    let mut tags = Vec::with_capacity(event.tag_slices().len());
    for tag in event.tag_slices() {
        tags.push(RadrootsNostrTag::parse(tag.as_slice().to_vec()).ok()?);
    }
    let sig = Signature::from_str(event.sig_str()).ok()?;
    Some(RadrootsNostrRawEvent::new(
        id,
        public_key,
        RadrootsNostrTimestamp::from_secs(event.created_at_u64()),
        RadrootsNostrKind::Custom(kind_u16),
        tags,
        event.content().to_owned(),
        sig,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_convert::radroots_event_from_nostr;
    use crate::events::radroots_nostr_build_event;
    use crate::test_fixtures::FIXTURE_ALICE;
    use crate::types::{RadrootsNostrKeys, RadrootsNostrSecretKey};
    use radroots_event::{RadrootsEventEnvelopeParts, kinds::KIND_POST};

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn signed_event() -> RadrootsEventEnvelope {
        let raw_event = radroots_nostr_build_event(
            KIND_POST,
            "hello",
            vec![vec!["t".to_owned(), "soil".to_owned()]],
        )
        .expect("builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
        .sign_with_keys(&fixture_keys())
        .expect("signed event");
        radroots_event_from_nostr(&raw_event)
    }

    fn envelope_with(
        event: &RadrootsEventEnvelope,
        content: String,
        kind: u32,
        sig: String,
    ) -> RadrootsEventEnvelope {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: event.id_str().to_owned(),
            author: event.author_str().to_owned(),
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
            original.sig_str().to_owned(),
        );

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::IdMismatch
        );
    }

    #[test]
    fn reports_signature_invalid_for_valid_id_with_wrong_signature() {
        let original = signed_event();
        let mut sig = original.sig_str().to_owned();
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
    fn reports_malformed_envelope_for_unparseable_wire_fields() {
        let original = signed_event();
        let event = envelope_with(
            &original,
            original.content().to_owned(),
            u32::from(u16::MAX) + 1,
            original.sig_str().to_owned(),
        );

        assert_eq!(
            radroots_nostr_verify_event(&event),
            RadrootsNostrEventVerification::MalformedEnvelope
        );
    }
}
