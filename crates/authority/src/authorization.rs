#![forbid(unsafe_code)]

use crate::{RadrootsActorContext, RadrootsAuthorityError, RadrootsEventSigner};
use radroots_events::contract::{RadrootsEventContract, event_contract};
use radroots_events::draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent};

#[cfg(not(feature = "std"))]
use alloc::borrow::ToOwned;
#[cfg(feature = "std")]
use std::borrow::ToOwned;

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
    if actor.pubkey.as_str() != draft.expected_pubkey.as_str() {
        return Err(RadrootsAuthorityError::ActorPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            actor_pubkey: actor.pubkey.as_str().to_owned(),
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
    if signed_event.pubkey.as_str() != draft.expected_pubkey.as_str() {
        return Err(RadrootsAuthorityError::SignedEventPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey.clone(),
            actual_pubkey: signed_event.pubkey,
        });
    }
    if signed_event.id.as_str() != draft.expected_event_id.as_str() {
        return Err(RadrootsAuthorityError::SignedEventIdMismatch {
            expected_event_id: draft.expected_event_id.clone(),
            actual_event_id: signed_event.id,
        });
    }
    Ok(signed_event)
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
        RadrootsActorContext::with_roles(pubkey, [RadrootsActorRole::Seller]).expect("seller")
    }

    fn buyer_actor(pubkey: &str) -> RadrootsActorContext {
        RadrootsActorContext::with_roles(pubkey, [RadrootsActorRole::Buyer]).expect("buyer")
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

    struct StaticSigner {
        pubkey: RadrootsPublicKey,
        event_id: Option<String>,
    }

    impl StaticSigner {
        fn new(pubkey: &str) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                event_id: None,
            }
        }

        fn with_event_id(pubkey: &str, event_id: String) -> Self {
            Self {
                pubkey: RadrootsPublicKey::parse(pubkey).expect("pubkey"),
                event_id: Some(event_id),
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
            RadrootsSignedNostrEvent::new(
                self.event_id
                    .as_deref()
                    .unwrap_or(draft.expected_event_id.as_str()),
                self.pubkey.as_str(),
                draft.created_at,
                draft.kind,
                draft.tags.clone(),
                draft.content.as_str(),
                hex_128('f'),
                "{}",
            )
            .map_err(|error| RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            })
        }
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
