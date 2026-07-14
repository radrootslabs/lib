#![forbid(unsafe_code)]

use crate::{RadrootsAuthorityError, RadrootsSignerError};
use radroots_event::draft::{RadrootsEventDraft, RadrootsSignedEvent};
use radroots_event::ids::RadrootsPublicKey;
#[cfg(test)]
use radroots_event::wire::RadrootsNip01EventWire;

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
        draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::kinds::KIND_POST;

    fn hex_64(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        std::iter::repeat_n(character, 128).collect()
    }

    fn draft_for(pubkey: &str) -> RadrootsEventDraft {
        RadrootsEventDraft::new(
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
        event_id: Option<String>,
    }

    impl MockSigner {
        fn new(pubkey: &str) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                failure: None,
                event_id: None,
            }
        }

        fn failing(pubkey: &str, failure: RadrootsSignerError) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                failure: Some(failure),
                event_id: None,
            }
        }

        fn with_event_id(pubkey: &str, event_id: String) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                failure: None,
                event_id: Some(event_id),
            }
        }
    }

    impl RadrootsEventSigner for MockSigner {
        fn pubkey(&self) -> &RadrootsPublicKey {
            &self.pubkey
        }

        fn sign_frozen_draft(
            &self,
            draft: &RadrootsEventDraft,
        ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
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
            let id = self
                .event_id
                .as_deref()
                .unwrap_or(draft.expected_event_id_str())
                .to_owned();
            RadrootsSignedEvent::from_wire_unchecked(
                RadrootsNip01EventWire {
                    id,
                    pubkey: self.pubkey.to_string(),
                    created_at: draft.created_at_u64(),
                    kind: draft.kind_u32(),
                    tags: draft.tags_as_vec(),
                    content: draft.content().to_owned(),
                    sig: hex_128('f'),
                    extra: Default::default(),
                },
                "{}",
            )
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

        assert_eq!(signed.id_str(), draft.expected_event_id_str());
        assert_eq!(signed.pubkey_str(), pubkey);
        assert_eq!(signed.kind(), KIND_POST);
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
        let signer = MockSigner::with_event_id(pubkey.as_str(), "bad-id".to_string());
        let draft = draft_for(pubkey.as_str());

        let err = signer.sign_frozen_draft(&draft).expect_err("failure");

        assert!(matches!(err, RadrootsSignerError::SigningFailed { .. }));
    }
}
