#![forbid(unsafe_code)]

use crate::{RadrootsAuthorityError, RadrootsSignerError};
#[cfg(test)]
use radroots_events::draft::RadrootsSignedNostrEventParts;
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};
use radroots_events::ids::RadrootsPublicKey;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSignerIdentity {
    pub pubkey: RadrootsPublicKey,
}

impl RadrootsSignerIdentity {
    pub fn new(pubkey: impl AsRef<str>) -> Result<Self, RadrootsAuthorityError> {
        let pubkey = RadrootsPublicKey::parse(pubkey.as_ref())
            .map_err(|_| RadrootsAuthorityError::InvalidSignerPubkey)?;
        Ok(Self { pubkey })
    }

    pub fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }
}

pub trait RadrootsEventSigner {
    fn pubkey(&self) -> &RadrootsPublicKey;

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_events::kinds::KIND_POST;

    fn hex_64(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        std::iter::repeat_n(character, 128).collect()
    }

    fn draft_for(pubkey: &str) -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.social.post.v1",
            KIND_POST,
            1_700_000_000,
            vec![vec!["t".to_owned(), "soil".to_owned()]],
            "hello",
            pubkey,
        )
        .expect("draft")
    }

    struct MockSigner {
        pubkey: RadrootsPublicKey,
        failure: Option<RadrootsSignerError>,
    }

    impl MockSigner {
        fn new(pubkey: &str) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                failure: None,
            }
        }

        fn failing(pubkey: &str, failure: RadrootsSignerError) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                failure: Some(failure),
            }
        }
    }

    impl RadrootsEventSigner for MockSigner {
        fn pubkey(&self) -> &RadrootsPublicKey {
            &self.pubkey
        }

        fn sign_frozen_draft(
            &self,
            draft: &RadrootsFrozenEventDraft,
        ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
            if let Some(failure) = &self.failure {
                return Err(match failure {
                    RadrootsSignerError::Unavailable => RadrootsSignerError::Unavailable,
                    RadrootsSignerError::Rejected => RadrootsSignerError::Rejected,
                    RadrootsSignerError::SigningFailed { message } => {
                        RadrootsSignerError::SigningFailed {
                            message: message.clone(),
                        }
                    }
                });
            }
            RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
                id: draft.expected_event_id.to_string(),
                pubkey: self.pubkey.to_string(),
                created_at: draft.created_at,
                kind: draft.kind,
                tags: draft.tags.clone(),
                content: draft.content.clone(),
                sig: hex_128('f'),
                raw_json: "{}".to_owned(),
            })
            .map_err(|error| RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            })
        }
    }

    #[test]
    fn mock_signer_reports_public_key() {
        let pubkey = hex_64('a');
        let signer = MockSigner::new(pubkey.as_str());

        assert_eq!(signer.pubkey().as_str(), pubkey);
    }

    #[test]
    fn signer_identity_validates_public_key() {
        let pubkey = hex_64('a');
        let identity = RadrootsSignerIdentity::new(pubkey.as_str()).expect("identity");
        assert_eq!(identity.pubkey().as_str(), pubkey);

        assert!(matches!(
            RadrootsSignerIdentity::new("bad-pubkey"),
            Err(RadrootsAuthorityError::InvalidSignerPubkey)
        ));
    }

    #[test]
    fn mock_signer_returns_signed_frozen_draft() {
        let pubkey = hex_64('a');
        let signer = MockSigner::new(pubkey.as_str());
        let draft = draft_for(pubkey.as_str());

        let signed = signer.sign_frozen_draft(&draft).expect("signed");

        assert_eq!(signed.id, draft.expected_event_id);
        assert_eq!(signed.pubkey, pubkey);
        assert_eq!(signed.kind, KIND_POST);
    }

    #[test]
    fn mock_signer_propagates_signing_errors() {
        let pubkey = hex_64('a');
        let draft = draft_for(pubkey.as_str());

        for failure in [
            RadrootsSignerError::Unavailable,
            RadrootsSignerError::Rejected,
            RadrootsSignerError::SigningFailed {
                message: "deterministic failure".to_owned(),
            },
        ] {
            let signer = MockSigner::failing(pubkey.as_str(), failure);
            let err = signer.sign_frozen_draft(&draft).expect_err("failure");

            match err {
                RadrootsSignerError::Unavailable => {}
                RadrootsSignerError::Rejected => {}
                RadrootsSignerError::SigningFailed { message } => {
                    assert_eq!(message, "deterministic failure");
                }
            }
        }
    }

    #[test]
    fn mock_signer_maps_invalid_signed_event_parts() {
        let pubkey = hex_64('a');
        let signer = MockSigner::new(pubkey.as_str());
        let mut draft = draft_for(pubkey.as_str());
        draft.expected_event_id = "bad-id".to_string();

        let err = signer.sign_frozen_draft(&draft).expect_err("failure");

        assert!(matches!(err, RadrootsSignerError::SigningFailed { .. }));
    }
}
