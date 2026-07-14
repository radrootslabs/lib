use radroots_authority::{RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity};
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent, RadrootsSignedEventParts};

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
    fn pubkey(&self) -> &radroots_event::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id_str(),
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at_u64(),
            "kind": draft.kind_u32(),
            "tags": draft.tags_as_vec(),
            "content": draft.content(),
            "sig": sig,
        })
        .to_string();
        RadrootsSignedEvent::new(RadrootsSignedEventParts {
            id: draft.expected_event_id_str().to_owned(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at_u64(),
            kind: draft.kind_u32(),
            tags: draft.tags_as_vec(),
            content: draft.content().to_owned(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}
