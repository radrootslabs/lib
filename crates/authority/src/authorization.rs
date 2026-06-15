#![forbid(unsafe_code)]

use crate::{RadrootsActorContext, RadrootsAuthorityError, RadrootsEventSigner};
use radroots_events::contract::{RadrootsEventContract, event_contract};
#[cfg(test)]
use radroots_events::draft::RadrootsSignedNostrEventParts;
use radroots_events::draft::{
    RadrootsDraftError, RadrootsFrozenEventDraft, RadrootsSignedNostrEvent,
    validate_signed_nostr_event_matches_draft,
};

#[cfg(not(feature = "std"))]
use alloc::{borrow::ToOwned, string::ToString};
#[cfg(feature = "std")]
use std::{borrow::ToOwned, string::ToString};

pub fn authorize_actor_for_contract(
    actor: &RadrootsActorContext,
    contract: &RadrootsEventContract,
) -> Result<(), RadrootsAuthorityError> {
    if actor.satisfies(contract.author_role) {
        Ok(())
    } else {
        Err(RadrootsAuthorityError::ActorRoleUnsatisfied {
            contract_id: contract.id.to_owned(),
            required_role: contract.author_role,
        })
    }
}

pub fn authorize_actor_for_draft(
    actor: &RadrootsActorContext,
    draft: &RadrootsFrozenEventDraft,
) -> Result<&'static RadrootsEventContract, RadrootsAuthorityError> {
    let contract = event_contract(draft.contract_id.as_str()).ok_or_else(|| {
        RadrootsAuthorityError::UnknownContract {
            contract_id: draft.contract_id.clone(),
        }
    })?;
    if contract.kind != draft.kind {
        return Err(RadrootsAuthorityError::DraftKindMismatch {
            contract_id: draft.contract_id.clone(),
            expected_kind: contract.kind,
            actual_kind: draft.kind,
        });
    }
    authorize_actor_for_contract(actor, contract)?;
    if actor.pubkey().as_str() != draft.expected_pubkey.as_str() {
        return Err(RadrootsAuthorityError::ActorPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            actor_pubkey: actor.pubkey().as_str().to_owned(),
        });
    }
    Ok(contract)
}

pub fn authorize_signer_for_draft<S>(
    signer: &S,
    draft: &RadrootsFrozenEventDraft,
) -> Result<(), RadrootsAuthorityError>
where
    S: RadrootsEventSigner + ?Sized,
{
    if signer.pubkey().as_str() == draft.expected_pubkey.as_str() {
        Ok(())
    } else {
        Err(RadrootsAuthorityError::SignerPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            signer_pubkey: signer.pubkey().as_str().to_owned(),
        })
    }
}

pub fn sign_authorized_draft<S>(
    actor: &RadrootsActorContext,
    signer: &S,
    draft: &RadrootsFrozenEventDraft,
) -> Result<RadrootsSignedNostrEvent, RadrootsAuthorityError>
where
    S: RadrootsEventSigner + ?Sized,
{
    authorize_actor_for_draft(actor, draft)?;
    authorize_signer_for_draft(signer, draft)?;
    let signed_event = signer.sign_frozen_draft(draft)?;
    validate_signed_event_matches_draft(&signed_event, draft)?;
    Ok(signed_event)
}

pub fn validate_signed_event_matches_draft(
    signed_event: &RadrootsSignedNostrEvent,
    draft: &RadrootsFrozenEventDraft,
) -> Result<(), RadrootsAuthorityError> {
    validate_signed_nostr_event_matches_draft(signed_event, draft)
        .map_err(authority_error_from_draft_validation)
}

fn authority_error_from_draft_validation(error: RadrootsDraftError) -> RadrootsAuthorityError {
    match error {
        RadrootsDraftError::SignedEventPubkeyMismatch {
            expected_pubkey,
            actual_pubkey,
        } => RadrootsAuthorityError::SignedEventPubkeyMismatch {
            expected_pubkey,
            actual_pubkey,
        },
        RadrootsDraftError::SignedEventIdMismatch {
            expected_event_id,
            actual_event_id,
        } => RadrootsAuthorityError::SignedEventIdMismatch {
            expected_event_id,
            actual_event_id,
        },
        RadrootsDraftError::SignedEventCreatedAtMismatch {
            expected_created_at,
            actual_created_at,
        } => RadrootsAuthorityError::SignedEventCreatedAtMismatch {
            expected_created_at,
            actual_created_at,
        },
        RadrootsDraftError::SignedEventKindMismatch {
            expected_kind,
            actual_kind,
        } => RadrootsAuthorityError::SignedEventKindMismatch {
            expected_kind,
            actual_kind,
        },
        RadrootsDraftError::SignedEventTagsMismatch {
            expected_len,
            actual_len,
        } => RadrootsAuthorityError::SignedEventTagsMismatch {
            expected_len,
            actual_len,
        },
        RadrootsDraftError::SignedEventContentMismatch {
            expected_len,
            actual_len,
        } => RadrootsAuthorityError::SignedEventContentMismatch {
            expected_len,
            actual_len,
        },
        RadrootsDraftError::SignedEventComputedIdMismatch {
            expected_event_id,
            computed_event_id,
        } => RadrootsAuthorityError::SignedEventComputedIdMismatch {
            expected_event_id,
            computed_event_id,
        },
        error => RadrootsAuthorityError::SignedEventComputedIdInvalid {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RadrootsSignerError;
    use radroots_events::contract::{RadrootsActorRole, event_contract};
    use radroots_events::ids::RadrootsPublicKey;
    use radroots_events::kinds::{KIND_LISTING, KIND_ORDER_REQUEST, KIND_POST};

    fn hex_64(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        std::iter::repeat_n(character, 128).collect()
    }

    fn seller_actor(pubkey: &str) -> RadrootsActorContext {
        RadrootsActorContext::explicit_pubkey(pubkey, [RadrootsActorRole::Seller]).expect("seller")
    }

    fn buyer_actor(pubkey: &str) -> RadrootsActorContext {
        RadrootsActorContext::explicit_pubkey(pubkey, [RadrootsActorRole::Buyer]).expect("buyer")
    }

    fn listing_draft(pubkey: &str) -> RadrootsFrozenEventDraft {
        RadrootsFrozenEventDraft::new(
            "radroots.listing.published.v1",
            KIND_LISTING,
            1_700_000_000,
            vec![vec!["d".to_owned(), "listing-a".to_owned()]],
            "{}",
            pubkey,
        )
        .expect("listing draft")
    }

    #[derive(Default)]
    struct SignedEventOverrides {
        event_id: Option<String>,
        created_at: Option<u32>,
        kind: Option<u32>,
        tags: Option<Vec<Vec<String>>>,
        content: Option<String>,
    }

    struct StaticSigner {
        pubkey: RadrootsPublicKey,
        overrides: SignedEventOverrides,
    }

    impl StaticSigner {
        fn new(pubkey: &str) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                overrides: SignedEventOverrides::default(),
            }
        }

        fn with_event_id(pubkey: &str, event_id: String) -> Self {
            Self::with_overrides(
                pubkey,
                SignedEventOverrides {
                    event_id: Some(event_id),
                    ..SignedEventOverrides::default()
                },
            )
        }

        fn with_overrides(pubkey: &str, overrides: SignedEventOverrides) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                overrides,
            }
        }
    }

    impl RadrootsEventSigner for StaticSigner {
        fn pubkey(&self) -> &RadrootsPublicKey {
            &self.pubkey
        }

        fn sign_frozen_draft(
            &self,
            draft: &RadrootsFrozenEventDraft,
        ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
            RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
                id: self
                    .overrides
                    .event_id
                    .as_deref()
                    .unwrap_or(draft.expected_event_id.as_str())
                    .to_owned(),
                pubkey: self.pubkey.to_string(),
                created_at: self.overrides.created_at.unwrap_or(draft.created_at),
                kind: self.overrides.kind.unwrap_or(draft.kind),
                tags: self
                    .overrides
                    .tags
                    .clone()
                    .unwrap_or_else(|| draft.tags.clone()),
                content: self
                    .overrides
                    .content
                    .clone()
                    .unwrap_or_else(|| draft.content.clone()),
                sig: hex_128('f'),
                raw_json: "{}".to_owned(),
            })
            .map_err(|error| RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            })
        }
    }

    fn signed_event_from_draft(draft: &RadrootsFrozenEventDraft) -> RadrootsSignedNostrEvent {
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: draft.expected_pubkey.clone(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig: hex_128('f'),
            raw_json: "{}".to_owned(),
        })
        .expect("signed event")
    }

    #[test]
    fn buyer_and_seller_contract_roles_match_current_contracts() {
        let listing = event_contract("radroots.listing.published.v1").expect("listing contract");
        let order_request = event_contract("radroots.order.request.v1").expect("order contract");
        let seller = seller_actor(hex_64('a').as_str());
        let buyer = buyer_actor(hex_64('b').as_str());

        assert_eq!(listing.author_role, RadrootsActorRole::Seller);
        assert!(authorize_actor_for_contract(&seller, listing).is_ok());
        assert!(matches!(
            authorize_actor_for_contract(&buyer, listing),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
        assert!(authorize_actor_for_contract(&buyer, order_request).is_ok());
        assert!(matches!(
            authorize_actor_for_contract(&seller, order_request),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
    }

    #[test]
    fn actor_pubkey_mismatch_fails() {
        let draft = listing_draft(hex_64('a').as_str());
        let actor = seller_actor(hex_64('b').as_str());

        assert!(matches!(
            authorize_actor_for_draft(&actor, &draft),
            Err(RadrootsAuthorityError::ActorPubkeyMismatch { .. })
        ));
    }

    #[test]
    fn signer_pubkey_mismatch_fails() {
        let draft = listing_draft(hex_64('a').as_str());
        let signer = StaticSigner::new(hex_64('b').as_str());

        assert!(matches!(
            authorize_signer_for_draft(&signer, &draft),
            Err(RadrootsAuthorityError::SignerPubkeyMismatch { .. })
        ));
    }

    #[test]
    fn unknown_contract_and_kind_mismatch_fail() {
        let actor = seller_actor(hex_64('a').as_str());
        let unknown = RadrootsFrozenEventDraft {
            contract_id: "radroots.unknown.v1".to_owned(),
            contract_registry_version: 1,
            kind: KIND_LISTING,
            created_at: 1_700_000_000,
            tags: Vec::new(),
            content: "{}".to_owned(),
            expected_pubkey: hex_64('a'),
            expected_event_id: hex_64('e'),
        };
        assert!(matches!(
            authorize_actor_for_draft(&actor, &unknown),
            Err(RadrootsAuthorityError::UnknownContract { .. })
        ));

        let wrong_kind = RadrootsFrozenEventDraft {
            contract_id: "radroots.listing.published.v1".to_owned(),
            contract_registry_version: 1,
            kind: KIND_POST,
            created_at: 1_700_000_000,
            tags: Vec::new(),
            content: "{}".to_owned(),
            expected_pubkey: hex_64('a'),
            expected_event_id: hex_64('e'),
        };
        assert!(matches!(
            authorize_actor_for_draft(&actor, &wrong_kind),
            Err(RadrootsAuthorityError::DraftKindMismatch {
                expected_kind: KIND_LISTING,
                actual_kind: KIND_POST,
                ..
            })
        ));
    }

    #[test]
    fn signed_event_id_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_event_id(pubkey.as_str(), hex_64('e'));

        assert!(matches!(
            sign_authorized_draft(&actor, &signer, &draft),
            Err(RadrootsAuthorityError::SignedEventIdMismatch { .. })
        ));
    }

    #[test]
    fn signed_event_created_at_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_overrides(
            pubkey.as_str(),
            SignedEventOverrides {
                created_at: Some(draft.created_at + 1),
                ..SignedEventOverrides::default()
            },
        );

        assert!(matches!(
            sign_authorized_draft(&actor, &signer, &draft),
            Err(RadrootsAuthorityError::SignedEventCreatedAtMismatch { .. })
        ));
    }

    #[test]
    fn signed_event_kind_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_overrides(
            pubkey.as_str(),
            SignedEventOverrides {
                kind: Some(KIND_POST),
                ..SignedEventOverrides::default()
            },
        );

        assert!(matches!(
            sign_authorized_draft(&actor, &signer, &draft),
            Err(RadrootsAuthorityError::SignedEventKindMismatch {
                expected_kind: KIND_LISTING,
                actual_kind: KIND_POST
            })
        ));
    }

    #[test]
    fn signed_event_tags_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_overrides(
            pubkey.as_str(),
            SignedEventOverrides {
                tags: Some(vec![vec!["d".to_owned(), "listing-b".to_owned()]]),
                ..SignedEventOverrides::default()
            },
        );

        let error = sign_authorized_draft(&actor, &signer, &draft).unwrap_err();

        assert_eq!(
            error,
            RadrootsAuthorityError::SignedEventTagsMismatch {
                expected_len: 1,
                actual_len: 1
            }
        );
        assert!(!format!("{error:?}").contains("listing-b"));
        assert!(!error.to_string().contains("listing-b"));
    }

    #[test]
    fn signed_event_content_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_overrides(
            pubkey.as_str(),
            SignedEventOverrides {
                content: Some("{\"changed\":true}".to_owned()),
                ..SignedEventOverrides::default()
            },
        );

        let error = sign_authorized_draft(&actor, &signer, &draft).unwrap_err();

        assert_eq!(
            error,
            RadrootsAuthorityError::SignedEventContentMismatch {
                expected_len: 2,
                actual_len: 16
            }
        );
        assert!(!format!("{error:?}").contains("changed"));
        assert!(!error.to_string().contains("changed"));
    }

    #[test]
    fn signed_event_exactly_matching_draft_passes() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let signed = signed_event_from_draft(&draft);

        validate_signed_event_matches_draft(&signed, &draft).expect("signed event matches draft");
    }

    #[test]
    fn signed_event_computed_id_mismatch_fails() {
        let pubkey = hex_64('a');
        let inconsistent_draft = RadrootsFrozenEventDraft {
            contract_id: "radroots.listing.published.v1".to_owned(),
            contract_registry_version: 1,
            kind: KIND_LISTING,
            created_at: 1_700_000_000,
            tags: vec![vec!["d".to_owned(), "listing-a".to_owned()]],
            content: "{}".to_owned(),
            expected_pubkey: pubkey,
            expected_event_id: hex_64('e'),
        };
        let signed = signed_event_from_draft(&inconsistent_draft);

        assert!(matches!(
            validate_signed_event_matches_draft(&signed, &inconsistent_draft),
            Err(RadrootsAuthorityError::SignedEventComputedIdMismatch { .. })
        ));
    }

    #[test]
    fn sign_authorized_draft_calls_full_integrity_check() {
        let pubkey = hex_64('a');
        let inconsistent_draft = RadrootsFrozenEventDraft {
            contract_id: "radroots.listing.published.v1".to_owned(),
            contract_registry_version: 1,
            kind: KIND_LISTING,
            created_at: 1_700_000_000,
            tags: vec![vec!["d".to_owned(), "listing-a".to_owned()]],
            content: "{}".to_owned(),
            expected_pubkey: pubkey.clone(),
            expected_event_id: hex_64('e'),
        };
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::new(pubkey.as_str());

        assert!(matches!(
            sign_authorized_draft(&actor, &signer, &inconsistent_draft),
            Err(RadrootsAuthorityError::SignedEventComputedIdMismatch { .. })
        ));
    }

    #[test]
    fn authorized_actor_and_signer_return_signed_event() {
        let pubkey = hex_64('a');
        let draft = listing_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::new(pubkey.as_str());

        let signed = sign_authorized_draft(&actor, &signer, &draft).expect("signed");

        assert_eq!(signed.id, draft.expected_event_id);
        assert_eq!(signed.pubkey, draft.expected_pubkey);
        assert_eq!(signed.kind, KIND_LISTING);
    }

    #[test]
    fn order_request_draft_requires_buyer_role() {
        let pubkey = hex_64('a');
        let draft = RadrootsFrozenEventDraft::new(
            "radroots.order.request.v1",
            KIND_ORDER_REQUEST,
            1_700_000_000,
            Vec::new(),
            "{}",
            pubkey.as_str(),
        )
        .expect("order draft");
        let buyer = buyer_actor(pubkey.as_str());
        let seller = seller_actor(pubkey.as_str());

        assert!(authorize_actor_for_draft(&buyer, &draft).is_ok());
        assert!(matches!(
            authorize_actor_for_draft(&seller, &draft),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
    }
}
