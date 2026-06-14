#![forbid(unsafe_code)]

use crate::{RadrootsAuthorityError, RadrootsEventSigner, RadrootsSignerError};
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_events::ids::RadrootsPublicKey;
use radroots_nostr::prelude::{RadrootsNostrKeys, radroots_nostr_sign_frozen_draft};

pub struct RadrootsLocalEventSigner {
    keys: RadrootsNostrKeys,
    pubkey: RadrootsPublicKey,
}

impl RadrootsLocalEventSigner {
    pub fn new(keys: RadrootsNostrKeys) -> Result<Self, RadrootsAuthorityError> {
        let pubkey = RadrootsPublicKey::parse(keys.public_key().to_hex())
            .map_err(|_| RadrootsAuthorityError::InvalidSignerPubkey)?;
        Ok(Self { keys, pubkey })
    }
}

impl RadrootsEventSigner for RadrootsLocalEventSigner {
    fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::RadrootsNostrEvent;
    use radroots_events::kinds::KIND_POST;
    use radroots_nostr::prelude::{
        RadrootsNostrEventVerification, RadrootsNostrSecretKey, radroots_nostr_verify_event,
    };

    const FIXTURE_ALICE_SECRET_KEY_HEX: &str =
        "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
    const FIXTURE_ALICE_PUBLIC_KEY_HEX: &str =
        "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE_SECRET_KEY_HEX).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn post_draft() -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
            FIXTURE_ALICE_PUBLIC_KEY_HEX,
        )
        .expect("draft")
    }

    fn verification_event(signed: &RadrootsSignedNostrEvent) -> RadrootsNostrEvent {
        RadrootsNostrEvent {
            id: signed.id.clone(),
            author: signed.pubkey.clone(),
            created_at: signed.created_at,
            kind: signed.kind,
            tags: signed.tags.clone(),
            content: signed.content.clone(),
            sig: signed.sig.clone(),
        }
    }

    #[test]
    fn local_signer_reports_public_key() {
        let signer = RadrootsLocalEventSigner::new(fixture_keys()).expect("signer");

        assert_eq!(signer.pubkey().as_str(), FIXTURE_ALICE_PUBLIC_KEY_HEX);
    }

    #[test]
    fn local_signer_signs_and_verifies_frozen_drafts() {
        let signer = RadrootsLocalEventSigner::new(fixture_keys()).expect("signer");
        let draft = post_draft();

        let signed = signer.sign_frozen_draft(&draft).expect("signed");

        assert_eq!(signed.id, draft.expected_event_id);
        assert_eq!(signed.pubkey, draft.expected_pubkey);
        assert_eq!(
            radroots_nostr_verify_event(&verification_event(&signed)),
            RadrootsNostrEventVerification::Verified
        );
    }
}
