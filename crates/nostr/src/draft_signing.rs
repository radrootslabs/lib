#![forbid(unsafe_code)]

use crate::error::RadrootsNostrError;
use crate::event_convert::radroots_event_from_nostr;
use crate::events::radroots_nostr_build_event;
use crate::types::{RadrootsNostrKeys, RadrootsNostrTimestamp};
use nostr::JsonUtil;
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};

pub fn radroots_nostr_sign_frozen_draft(
    keys: &RadrootsNostrKeys,
    draft: &RadrootsFrozenEventDraft,
) -> Result<RadrootsSignedNostrEvent, RadrootsNostrError> {
    let actual_pubkey = keys.public_key().to_hex();
    if actual_pubkey != draft.expected_pubkey {
        return Err(RadrootsNostrError::FrozenDraftPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            actual_pubkey,
        });
    }

    let event = radroots_nostr_build_event(draft.kind, draft.content.clone(), draft.tags.clone())?
        .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(
            draft.created_at,
        )))
        .sign_with_keys(keys)?;
    let actual_event_id = event.id.to_hex();
    if actual_event_id != draft.expected_event_id {
        return Err(RadrootsNostrError::FrozenDraftEventIdMismatch {
            expected_event_id: draft.expected_event_id.clone(),
            actual_event_id,
        });
    }

    let raw_json = event.as_json();
    RadrootsSignedNostrEvent::from_event(radroots_event_from_nostr(&event), raw_json)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::radroots_nostr_sign_frozen_draft;
    use crate::error::RadrootsNostrError;
    use crate::test_fixtures::{FIXTURE_ALICE, FIXTURE_BOB};
    use crate::types::{RadrootsNostrKeys, RadrootsNostrSecretKey};
    use nostr::JsonUtil;
    use radroots_events::draft::RadrootsFrozenEventDraft;
    use radroots_events::kinds::KIND_POST;

    fn fixture_keys(secret_key_hex: &str) -> RadrootsNostrKeys {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn post_draft(expected_pubkey: &str) -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
            expected_pubkey,
        )
        .expect("draft")
    }

    #[test]
    fn sign_frozen_draft_uses_fixed_created_at_and_expected_id() {
        let keys = fixture_keys(FIXTURE_ALICE.secret_key_hex);
        let draft = post_draft(FIXTURE_ALICE.public_key_hex);
        let signed = radroots_nostr_sign_frozen_draft(&keys, &draft).expect("signed event");

        assert_eq!(signed.id, draft.expected_event_id);
        assert_eq!(signed.pubkey, draft.expected_pubkey);
        assert_eq!(signed.created_at, draft.created_at);
        assert_eq!(signed.kind, draft.kind);
        assert_eq!(signed.tags, draft.tags);
        assert_eq!(signed.content, draft.content);

        let raw_event = crate::types::RadrootsNostrEvent::from_json(signed.raw_json.as_str())
            .expect("raw json");
        assert_eq!(raw_event.id.to_hex(), signed.id);
        assert_eq!(raw_event.created_at.as_secs(), u64::from(draft.created_at));
    }

    #[test]
    fn sign_frozen_draft_rejects_wrong_signer() {
        let keys = fixture_keys(FIXTURE_BOB.secret_key_hex);
        let draft = post_draft(FIXTURE_ALICE.public_key_hex);
        let error = radroots_nostr_sign_frozen_draft(&keys, &draft).expect_err("wrong signer");

        assert!(matches!(
            error,
            RadrootsNostrError::FrozenDraftPubkeyMismatch { .. }
        ));
    }

    #[test]
    fn sign_frozen_draft_rejects_event_id_mismatch() {
        let keys = fixture_keys(FIXTURE_ALICE.secret_key_hex);
        let mut draft = post_draft(FIXTURE_ALICE.public_key_hex);
        draft.expected_event_id = "f".repeat(64);
        let error = radroots_nostr_sign_frozen_draft(&keys, &draft).expect_err("id mismatch");

        assert!(matches!(
            error,
            RadrootsNostrError::FrozenDraftEventIdMismatch { .. }
        ));
    }
}
