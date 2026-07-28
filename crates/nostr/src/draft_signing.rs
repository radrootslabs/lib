#![forbid(unsafe_code)]

use crate::error::RadrootsNostrError;
use crate::events::radroots_nostr_build_event_unchecked;
use crate::types::{RadrootsNostrKeys, RadrootsNostrTimestamp};
use nostr::JsonUtil;
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_event::wire::RadrootsNip01EventWire;

pub fn radroots_nostr_sign_frozen_draft(
    keys: &RadrootsNostrKeys,
    draft: &RadrootsEventDraft,
) -> Result<RadrootsSignedEvent, RadrootsNostrError> {
    draft.validate_for_signing()?;
    let actual_pubkey = keys.public_key().to_hex();
    if actual_pubkey != draft.expected_pubkey().to_hex() {
        return Err(RadrootsNostrError::FrozenDraftPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey().to_hex().to_owned(),
            actual_pubkey,
        });
    }

    let event = radroots_nostr_build_event_unchecked(
        draft.kind_u32(),
        draft.content().to_owned(),
        draft.tags_as_vec(),
    )?
    .custom_created_at(RadrootsNostrTimestamp::from_secs(draft.created_at_u64()))
    .sign_with_keys(keys)?;
    let actual_event_id = event.id.to_hex();
    let expected_event_id = draft.expected_event_id_hex();
    if actual_event_id != expected_event_id {
        return Err(RadrootsNostrError::FrozenDraftEventIdMismatch {
            expected_event_id,
            actual_event_id,
        });
    }

    let raw_json = event.as_json();
    let wire = RadrootsNip01EventWire::parse_json(raw_json.as_str())?;
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::radroots_nostr_sign_frozen_draft;
    use crate::error::RadrootsNostrError;
    use crate::test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB};
    use crate::types::{RadrootsNostrKeys, RadrootsNostrSecretKey};
    use nostr::JsonUtil;
    use radroots_event::draft::RadrootsEventDraft;
    use radroots_event::kinds::KIND_GEOCHAT;

    fn fixture_keys(secret_key_hex: &str) -> RadrootsNostrKeys {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn generic_draft(expected_pubkey: &str) -> RadrootsEventDraft {
        RadrootsEventDraft::new(
            "radroots.social.geochat.v1",
            KIND_GEOCHAT,
            1_700_000_000,
            Vec::new(),
            "hello",
            expected_pubkey,
        )
        .expect("draft")
    }

    #[test]
    fn sign_frozen_draft_uses_fixed_created_at_and_expected_id() {
        let keys = fixture_keys(FIXTURE_ALICE.secret_key_hex);
        let draft = generic_draft(FIXTURE_ALICE.public_key_hex);
        let signed = radroots_nostr_sign_frozen_draft(&keys, &draft).expect("signed event");

        assert_eq!(signed.id_str(), draft.expected_event_id_hex());
        assert_eq!(signed.pubkey().to_hex(), draft.expected_pubkey().to_hex());
        assert_eq!(signed.created_at(), draft.created_at_u64());
        assert_eq!(signed.kind(), draft.kind_u32());
        assert_eq!(signed.tags_as_vec(), draft.tags_as_vec());
        assert_eq!(signed.content(), draft.content());

        let raw_event =
            crate::types::RadrootsNostrEvent::from_json(signed.raw_json()).expect("raw json");
        assert_eq!(raw_event.id.to_hex(), signed.id_str());
        assert_eq!(raw_event.created_at.as_secs(), draft.created_at_u64());
    }

    #[test]
    fn sign_frozen_draft_rejects_wrong_signer() {
        let keys = fixture_keys(FIXTURE_BOB.secret_key_hex);
        let draft = generic_draft(FIXTURE_ALICE.public_key_hex);
        let error = radroots_nostr_sign_frozen_draft(&keys, &draft).expect_err("wrong signer");

        assert!(matches!(
            error,
            RadrootsNostrError::FrozenDraftPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn frozen_draft_deserialization_rejects_event_id_mismatch() {
        let draft = generic_draft(FIXTURE_ALICE.public_key_hex);
        let mut raw = serde_json::to_value(&draft).expect("draft json");
        raw["expected_event_id"] = serde_json::Value::String("f".repeat(64));
        serde_json::from_value::<RadrootsEventDraft>(raw)
            .expect_err("tampered draft must fail before signing");
    }
}
