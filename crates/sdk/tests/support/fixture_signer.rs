use radroots_authority::{RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity};
use radroots_events::draft::{
    RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts,
};

#[derive(Clone)]
pub struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

impl FixtureSigner {
    pub fn new(pubkey: &str) -> Self {
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey.as_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id,
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at,
            "kind": draft.kind,
            "tags": draft.tags,
            "content": draft.content,
            "sig": sig,
        })
        .to_string();
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}
