#![forbid(unsafe_code)]

use crate::{RadrootsActorContext, RadrootsAuthorityError, RadrootsEventSigner};
use radroots_event::contract::{RadrootsEventContract, event_contract};
use radroots_event::draft::{
    RadrootsDraftError, RadrootsEventDraft, RadrootsSignedEvent,
    validate_signed_nostr_event_matches_draft,
};
#[cfg(test)]
use radroots_event::wire::RadrootsNip01EventWire;

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
    draft: &RadrootsEventDraft,
) -> Result<&'static RadrootsEventContract, RadrootsAuthorityError> {
    let contract = event_contract(draft.contract_id()).ok_or_else(|| {
        RadrootsAuthorityError::UnknownContract {
            contract_id: draft.contract_id().to_owned(),
        }
    })?;
    if contract.kind != draft.kind_u32() {
        return Err(RadrootsAuthorityError::DraftKindMismatch {
            contract_id: draft.contract_id().to_owned(),
            expected_kind: contract.kind,
            actual_kind: draft.kind_u32(),
        });
    }
    authorize_actor_for_contract(actor, contract)?;
    if actor.pubkey().as_str() != draft.expected_pubkey_str() {
        return Err(RadrootsAuthorityError::ActorPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey_str().to_owned(),
            actor_pubkey: actor.pubkey().as_str().to_owned(),
        });
    }
    Ok(contract)
}

pub fn authorize_signer_for_draft<S>(
    signer: &S,
    draft: &RadrootsEventDraft,
) -> Result<(), RadrootsAuthorityError>
where
    S: RadrootsEventSigner + ?Sized,
{
    if signer.pubkey().as_str() == draft.expected_pubkey_str() {
        Ok(())
    } else {
        Err(RadrootsAuthorityError::SignerPubkeyMismatch {
            expected_pubkey: draft.expected_pubkey_str().to_owned(),
            signer_pubkey: signer.pubkey().as_str().to_owned(),
        })
    }
}

pub fn sign_authorized_draft<S>(
    actor: &RadrootsActorContext,
    signer: &S,
    draft: &RadrootsEventDraft,
) -> Result<RadrootsSignedEvent, RadrootsAuthorityError>
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
    signed_event: &RadrootsSignedEvent,
    draft: &RadrootsEventDraft,
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
    use radroots_event::contract::{RadrootsActorRole, event_contract};
    use radroots_event::ids::RadrootsPublicKey;
    use radroots_event::kinds::{KIND_LISTING, KIND_POST, KIND_TRADE_PROPOSAL};

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

    fn listing_event_draft(pubkey: &str) -> RadrootsEventDraft {
        RadrootsEventDraft::new(
            "radroots.listing.published.v1",
            KIND_LISTING,
            1_700_000_000,
            vec![vec!["d".to_owned(), "listing-a".to_owned()]],
            "{}",
            pubkey,
        )
        .expect("listing event draft")
    }

    #[derive(Default)]
    struct SignedEventOverrides {
        event_id: Option<String>,
        created_at: Option<u64>,
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
            draft: &RadrootsEventDraft,
        ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
            let wire = RadrootsNip01EventWire {
                id: self
                    .overrides
                    .event_id
                    .as_deref()
                    .unwrap_or(draft.expected_event_id_str())
                    .to_owned(),
                pubkey: self.pubkey.to_string(),
                created_at: self.overrides.created_at.unwrap_or(draft.created_at_u64()),
                kind: self.overrides.kind.unwrap_or(draft.kind_u32()),
                tags: self
                    .overrides
                    .tags
                    .clone()
                    .unwrap_or_else(|| draft.tags_as_vec()),
                content: self
                    .overrides
                    .content
                    .clone()
                    .unwrap_or_else(|| draft.content().to_owned()),
                sig: hex_128('f'),
                extra: Default::default(),
            };
            let mut wire = wire;
            if self.overrides.event_id.is_none() {
                wire.id = wire
                    .computed_event_id()
                    .map_err(|error| RadrootsSignerError::SigningFailed {
                        message: error.to_string(),
                    })?
                    .into_string();
            }
            let raw_json = raw_json_for_wire(&wire);
            RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).map_err(|error| {
                RadrootsSignerError::SigningFailed {
                    message: error.to_string(),
                }
            })
        }
    }

    fn signed_event_from_draft(draft: &RadrootsEventDraft) -> RadrootsSignedEvent {
        signed_event_from_parts(
            draft.expected_pubkey_str().to_owned(),
            draft.created_at_u64(),
            draft.kind_u32(),
            draft.tags_as_vec(),
            draft.content().to_owned(),
        )
    }

    fn signed_event_from_parts(
        pubkey: String,
        created_at: u64,
        kind: u32,
        tags: Vec<Vec<String>>,
        content: String,
    ) -> RadrootsSignedEvent {
        let mut wire = RadrootsNip01EventWire {
            id: String::new(),
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig: hex_128('f'),
            extra: Default::default(),
        };
        wire.id = wire.computed_event_id().expect("event id").into_string();
        let raw_json = raw_json_for_wire(&wire);
        RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
    }

    fn raw_json_for_wire(wire: &RadrootsNip01EventWire) -> String {
        serde_json::json!({
            "id": wire.id,
            "pubkey": wire.pubkey,
            "created_at": wire.created_at,
            "kind": wire.kind,
            "tags": wire.tags,
            "content": wire.content,
            "sig": wire.sig,
        })
        .to_string()
    }

    #[test]
    fn buyer_and_seller_contract_roles_match_current_contracts() {
        let listing = event_contract("radroots.listing.published.v1").expect("listing contract");
        let trade_proposal =
            event_contract("radroots.trade.proposal.v1").expect("trade proposal contract");
        let trade_decision =
            event_contract("radroots.trade.decision.v1").expect("trade decision contract");
        let seller = seller_actor(hex_64('a').as_str());
        let buyer = buyer_actor(hex_64('b').as_str());

        assert_eq!(listing.author_role, RadrootsActorRole::Seller);
        assert!(authorize_actor_for_contract(&seller, listing).is_ok());
        assert!(matches!(
            authorize_actor_for_contract(&buyer, listing),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
        assert!(authorize_actor_for_contract(&buyer, trade_proposal).is_ok());
        assert!(matches!(
            authorize_actor_for_contract(&seller, trade_proposal),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
        assert_eq!(trade_decision.author_role, RadrootsActorRole::Seller);
        assert!(authorize_actor_for_contract(&seller, trade_decision).is_ok());
        assert!(matches!(
            authorize_actor_for_contract(&buyer, trade_decision),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
    }

    #[test]
    fn actor_pubkey_mismatch_fails() {
        let draft = listing_event_draft(hex_64('a').as_str());
        let actor = seller_actor(hex_64('b').as_str());

        assert!(matches!(
            authorize_actor_for_draft(&actor, &draft),
            Err(RadrootsAuthorityError::ActorPubkeyMismatch { .. })
        ));
    }

    #[test]
    fn signer_pubkey_mismatch_fails() {
        let draft = listing_event_draft(hex_64('a').as_str());
        let signer = StaticSigner::new(hex_64('b').as_str());

        assert!(matches!(
            authorize_signer_for_draft(&signer, &draft),
            Err(RadrootsAuthorityError::SignerPubkeyMismatch { .. })
        ));
    }

    #[test]
    fn signer_explicit_id_mismatch_fails_before_authorized_signing() {
        let pubkey = hex_64('a');
        let draft = listing_event_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_event_id(pubkey.as_str(), hex_64('e'));

        assert!(matches!(
            sign_authorized_draft(&actor, &signer, &draft),
            Err(RadrootsAuthorityError::Signer(
                RadrootsSignerError::SigningFailed { .. }
            ))
        ));
    }

    #[test]
    fn signed_event_created_at_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_event_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::with_overrides(
            pubkey.as_str(),
            SignedEventOverrides {
                created_at: Some(draft.created_at_u64() + 1),
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
        let draft = listing_event_draft(pubkey.as_str());
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
        let draft = listing_event_draft(pubkey.as_str());
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
        let draft = listing_event_draft(pubkey.as_str());
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
        let draft = listing_event_draft(pubkey.as_str());
        let signed = signed_event_from_draft(&draft);

        validate_signed_event_matches_draft(&signed, &draft).expect("signed event matches draft");
    }

    #[test]
    fn signed_event_pubkey_mismatch_fails() {
        let pubkey = hex_64('a');
        let draft = listing_event_draft(pubkey.as_str());
        let signed = signed_event_from_parts(
            hex_64('b'),
            draft.created_at_u64(),
            draft.kind_u32(),
            draft.tags_as_vec(),
            draft.content().to_owned(),
        );

        assert!(matches!(
            validate_signed_event_matches_draft(&signed, &draft),
            Err(RadrootsAuthorityError::SignedEventPubkeyMismatch { .. })
        ));
    }

    #[test]
    fn draft_validation_fallback_errors_map_to_computed_id_invalid() {
        let error = authority_error_from_draft_validation(RadrootsDraftError::UnknownContract(
            "radroots.unknown.v1".to_owned(),
        ));

        assert!(matches!(
            error,
            RadrootsAuthorityError::SignedEventComputedIdInvalid { .. }
        ));
    }

    #[test]
    fn static_signer_maps_invalid_signed_event_parts() {
        let pubkey = hex_64('a');
        let draft = listing_event_draft(pubkey.as_str());
        let signer = StaticSigner::with_event_id(pubkey.as_str(), "bad-id".to_owned());

        assert!(matches!(
            signer.sign_frozen_draft(&draft),
            Err(RadrootsSignerError::SigningFailed { .. })
        ));
    }

    #[test]
    fn authorized_actor_and_signer_return_signed_event() {
        let pubkey = hex_64('a');
        let draft = listing_event_draft(pubkey.as_str());
        let actor = seller_actor(pubkey.as_str());
        let signer = StaticSigner::new(pubkey.as_str());

        let signed = sign_authorized_draft(&actor, &signer, &draft).expect("signed");

        assert_eq!(signed.id_str(), draft.expected_event_id_str());
        assert_eq!(signed.pubkey_str(), draft.expected_pubkey_str());
        assert_eq!(signed.kind(), KIND_LISTING);
    }

    #[test]
    fn trade_proposal_draft_requires_buyer_role() {
        let pubkey = hex_64('a');
        let draft = RadrootsEventDraft::new(
            "radroots.trade.proposal.v1",
            KIND_TRADE_PROPOSAL,
            1_700_000_000,
            vec![
                vec![
                    "contract".to_owned(),
                    "radroots.trade.proposal.v1".to_owned(),
                ],
                vec![
                    "d".to_owned(),
                    "11111111111111111111111111111111".to_owned(),
                ],
                vec!["p".to_owned(), pubkey.clone()],
            ],
            r#"{"contract_id":"radroots.trade.proposal.v1"}"#,
            pubkey.as_str(),
        )
        .expect("trade proposal draft");
        let buyer = buyer_actor(pubkey.as_str());
        let seller = seller_actor(pubkey.as_str());

        assert!(authorize_actor_for_draft(&buyer, &draft).is_ok());
        assert!(matches!(
            authorize_actor_for_draft(&seller, &draft),
            Err(RadrootsAuthorityError::ActorRoleUnsatisfied { .. })
        ));
    }
}
