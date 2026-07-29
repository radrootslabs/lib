#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use core::fmt;

use radroots_event::{
    RadrootsEventEnvelope,
    contract::{
        RadrootsContractMatchError, RadrootsContractValidationError, RadrootsEventContract,
    },
    kinds::{
        KIND_CLASSIFIED_LISTING, KIND_COMMENT, KIND_DELETION_REQUEST, KIND_POST, KIND_PROFILE,
    },
};

use crate::{
    comment::admission::{
        RadrootsAdmittedNip22CommentEvent, RadrootsNip22CommentAdmissionError,
        admit_verified_nip22_comment_event,
    },
    deletion::admission::{
        RadrootsAdmittedNip09DeletionRequestEvent, RadrootsNip09DeletionAdmissionError,
        admit_verified_nip09_deletion_request_event,
    },
    food_availability::admission::{
        RadrootsAdmittedFoodAvailabilityEvent, RadrootsFoodAvailabilityAdmissionError,
        RadrootsFoodAvailabilityAdmissionOutcome, admit_verified_food_availability_event,
    },
    post::admission::{
        RadrootsAdmittedRootPostEvent, RadrootsPostAdmissionError, RadrootsPostAdmissionOutcome,
        admit_verified_post_event,
    },
    profile::admission::{
        RadrootsAdmittedProfileEvent, RadrootsProfileAdmissionError, admit_verified_profile_event,
    },
    reply::admission::{
        RadrootsAdmittedNip10ReplyEvent, RadrootsNip10ReplyAdmissionError,
        admit_thread_excluded_post_candidate,
    },
    verification::{
        RadrootsContractValidatedEvent, RadrootsSignatureVerifiedEvent, validate_event_contract,
    },
};

#[doc(hidden)]
pub mod registry_v7;
pub use registry_v7::{RadrootsRegistryV7AdmissionDecision, admit_verified_event_registry_v7};

/// A verified event admitted through its exact typed profile or full registry shape.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum RadrootsAdmittedEvent {
    Profile(RadrootsAdmittedProfileEvent),
    RootPost(RadrootsAdmittedRootPostEvent),
    Reply(RadrootsAdmittedNip10ReplyEvent),
    Comment(Box<RadrootsAdmittedNip22CommentEvent>),
    DeletionRequest(RadrootsAdmittedNip09DeletionRequestEvent),
    FoodAvailability(Box<RadrootsAdmittedFoodAvailabilityEvent>),
    ContractValidated(RadrootsContractValidatedEvent),
}

impl RadrootsAdmittedEvent {
    pub fn verified_event(&self) -> &RadrootsSignatureVerifiedEvent {
        match self {
            Self::Profile(event) => event.verified_event(),
            Self::RootPost(event) => event.verified_event(),
            Self::Reply(event) => event.verified_event(),
            Self::Comment(event) => event.verified_event(),
            Self::DeletionRequest(event) => event.verified_event(),
            Self::FoodAvailability(event) => event.verified_event(),
            Self::ContractValidated(event) => event.verified_event(),
        }
    }

    pub fn event(&self) -> &RadrootsEventEnvelope {
        self.verified_event().event()
    }

    pub fn contract(&self) -> &'static RadrootsEventContract {
        match self {
            Self::Profile(event) => event.contract(),
            Self::RootPost(event) => event.contract(),
            Self::Reply(event) => event.contract(),
            Self::Comment(event) => event.contract(),
            Self::DeletionRequest(event) => event.contract(),
            Self::FoodAvailability(event) => event.contract(),
            Self::ContractValidated(event) => event.contract(),
        }
    }

    pub fn contract_id(&self) -> &'static str {
        self.contract().id
    }

    pub fn into_verified_event(self) -> RadrootsSignatureVerifiedEvent {
        match self {
            Self::Profile(event) => event.into_parts().0,
            Self::RootPost(event) => event.into_parts().0,
            Self::Reply(event) => event.into_parts().0,
            Self::Comment(event) => event.into_parts().0,
            Self::DeletionRequest(event) => event.into_parts().0,
            Self::FoodAvailability(event) => event.into_parts().0,
            Self::ContractValidated(event) => event.into_verified_event(),
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventAdmissionError {
    ContractMatch(RadrootsContractMatchError),
    ContractValidation(RadrootsContractValidationError),
    Profile(RadrootsProfileAdmissionError),
    Post(RadrootsPostAdmissionError),
    Reply(RadrootsNip10ReplyAdmissionError),
    Comment(RadrootsNip22CommentAdmissionError),
    DeletionRequest(RadrootsNip09DeletionAdmissionError),
    FoodAvailability(RadrootsFoodAvailabilityAdmissionError),
}

impl RadrootsEventAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContractMatch(RadrootsContractMatchError::UnsupportedKind(_)) => {
                "unsupported_kind"
            }
            Self::ContractMatch(RadrootsContractMatchError::UnsupportedShape(_)) => {
                "unsupported_shape"
            }
            Self::ContractMatch(RadrootsContractMatchError::AmbiguousShape(_)) => "ambiguous_shape",
            Self::ContractValidation(error) => error.code(),
            Self::Profile(error) => error.code(),
            Self::Post(error) => error.code(),
            Self::Reply(error) => error.code(),
            Self::Comment(error) => error.code(),
            Self::DeletionRequest(error) => error.code(),
            Self::FoodAvailability(error) => error.code(),
        }
    }
}

impl fmt::Display for RadrootsEventAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContractMatch(RadrootsContractMatchError::UnsupportedKind(kind)) => {
                write!(formatter, "event kind {kind} has no registered contract")
            }
            Self::ContractMatch(RadrootsContractMatchError::UnsupportedShape(kind)) => {
                write!(
                    formatter,
                    "event kind {kind} has no supported contract shape"
                )
            }
            Self::ContractMatch(RadrootsContractMatchError::AmbiguousShape(kind)) => {
                write!(
                    formatter,
                    "event kind {kind} matches multiple contract shapes"
                )
            }
            Self::ContractValidation(error) => {
                write!(
                    formatter,
                    "event contract validation failed with code {}",
                    error.code()
                )
            }
            Self::Profile(error) => write!(formatter, "{error}"),
            Self::Post(error) => write!(formatter, "{error}"),
            Self::Reply(error) => write!(formatter, "{error}"),
            Self::Comment(error) => write!(formatter, "{error}"),
            Self::DeletionRequest(error) => write!(formatter, "{error}"),
            Self::FoodAvailability(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Profile(error) => Some(error),
            Self::Post(error) => Some(error),
            Self::Reply(error) => Some(error),
            Self::Comment(error) => Some(error),
            Self::DeletionRequest(error) => Some(error),
            Self::FoodAvailability(error) => Some(error),
            Self::ContractMatch(_) | Self::ContractValidation(_) => None,
        }
    }
}

/// Admits an already verified event through the exact typed or registry boundary.
pub fn admit_verified_event(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
    match event.event().kind_u32() {
        KIND_PROFILE => admit_profile(event),
        KIND_POST => admit_post_or_reply(event),
        KIND_COMMENT => admit_verified_nip22_comment_event(event)
            .map(|event| RadrootsAdmittedEvent::Comment(Box::new(event)))
            .map_err(RadrootsEventAdmissionError::Comment),
        KIND_DELETION_REQUEST => admit_verified_nip09_deletion_request_event(event)
            .map(RadrootsAdmittedEvent::DeletionRequest)
            .map_err(RadrootsEventAdmissionError::DeletionRequest),
        KIND_CLASSIFIED_LISTING => admit_food_or_registry(event),
        _ => admit_registry_contract(event),
    }
}

fn admit_profile(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
    admit_verified_profile_event(event)
        .map(RadrootsAdmittedEvent::Profile)
        .map_err(RadrootsEventAdmissionError::Profile)
}

fn admit_post_or_reply(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
    match admit_verified_post_event(event).map_err(RadrootsEventAdmissionError::Post)? {
        RadrootsPostAdmissionOutcome::Root(event) => Ok(RadrootsAdmittedEvent::RootPost(event)),
        RadrootsPostAdmissionOutcome::ThreadExcluded(candidate) => {
            admit_thread_excluded_post_candidate(candidate)
                .map(RadrootsAdmittedEvent::Reply)
                .map_err(RadrootsEventAdmissionError::Reply)
        }
    }
}

fn admit_food_or_registry(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
    match admit_verified_food_availability_event(event)
        .map_err(RadrootsEventAdmissionError::FoodAvailability)?
    {
        RadrootsFoodAvailabilityAdmissionOutcome::Admitted(event) => {
            Ok(RadrootsAdmittedEvent::FoodAvailability(event))
        }
        RadrootsFoodAvailabilityAdmissionOutcome::Excluded(candidate) => {
            admit_registry_contract(candidate.into_parts().0)
        }
    }
}

fn admit_registry_contract(
    event: RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
    validate_event_contract(event)
        .map(RadrootsAdmittedEvent::ContractValidated)
        .map_err(map_contract_validation)
}

fn map_contract_validation(error: RadrootsContractValidationError) -> RadrootsEventAdmissionError {
    match error {
        RadrootsContractValidationError::ContractMatch { error } => {
            RadrootsEventAdmissionError::ContractMatch(error)
        }
        error => RadrootsEventAdmissionError::ContractValidation(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::contract::{RadrootsEventDiscriminator, all_event_contracts};

    #[test]
    fn covers_the_exact_admission_only_registry_inventory() {
        let mut actual = all_event_contracts()
            .iter()
            .filter(|contract| {
                matches!(
                    contract.discriminator,
                    RadrootsEventDiscriminator::AdmissionOnly
                )
            })
            .map(|contract| contract.id);

        for expected in [
            "radroots.social.update.v1",
            "radroots.social.photo_update.v1",
            "radroots.social.ask.v1",
            "radroots.social.reply.v1",
            "radroots.social.deletion_request.v1",
            "radroots.social.comment.v1",
            "radroots.food.availability.v1",
        ] {
            assert_eq!(actual.next(), Some(expected));
        }
        assert_eq!(actual.next(), None);
    }

    #[test]
    fn contract_match_error_codes_are_stable_and_distinct() {
        for (error, code) in [
            (
                RadrootsContractMatchError::UnsupportedKind(65_535),
                "unsupported_kind",
            ),
            (
                RadrootsContractMatchError::UnsupportedShape(KIND_CLASSIFIED_LISTING),
                "unsupported_shape",
            ),
            (
                RadrootsContractMatchError::AmbiguousShape(KIND_CLASSIFIED_LISTING),
                "ambiguous_shape",
            ),
        ] {
            let error = RadrootsEventAdmissionError::ContractMatch(error);
            assert_eq!(error.code(), code);
            assert!(!error.to_string().is_empty());
        }
    }

    #[cfg(feature = "nostr")]
    mod signed {
        use super::*;
        use crate::{
            test_fixtures::{FIXTURE_ALICE_PUBLIC_KEY_HEX, FIXTURE_ALICE_SECRET_KEY_HEX},
            verification::verify_nip01_event,
        };
        use nostr::secp256k1::Message;
        use nostr::{Keys, SECP256K1};
        use radroots_event::{
            RadrootsEventEnvelopeParts, kinds::KIND_FOLLOW, wire::compute_canonical_nip01_event_id,
        };

        #[test]
        fn routes_every_typed_profile_and_preserves_the_verified_envelope() {
            let profile = admitted(100, KIND_PROFILE, vec![], "{}");
            assert!(matches!(&profile, RadrootsAdmittedEvent::Profile(_)));
            assert_admitted(profile, "radroots.profile.metadata.v1");

            let post = admitted(101, KIND_POST, vec![], "Harvest update");
            assert!(matches!(&post, RadrootsAdmittedEvent::RootPost(_)));
            assert_admitted(post, "radroots.social.update.v1");

            let reply = admitted(
                102,
                KIND_POST,
                vec![vec![
                    "e".into(),
                    "a".repeat(64),
                    String::new(),
                    "root".into(),
                ]],
                "Reply",
            );
            assert!(matches!(&reply, RadrootsAdmittedEvent::Reply(_)));
            assert_admitted(reply, "radroots.social.reply.v1");

            let comment = admitted(103, KIND_COMMENT, comment_tags(), "Comment");
            assert!(matches!(&comment, RadrootsAdmittedEvent::Comment(_)));
            assert_admitted(comment, "radroots.social.comment.v1");

            let deletion = admitted(
                104,
                KIND_DELETION_REQUEST,
                vec![vec!["e".into(), "a".repeat(64)]],
                "Superseded",
            );
            assert!(matches!(
                &deletion,
                RadrootsAdmittedEvent::DeletionRequest(_)
            ));
            assert_admitted(deletion, "radroots.social.deletion_request.v1");

            let food = admitted(
                200,
                KIND_CLASSIFIED_LISTING,
                food_tags(),
                "Carrots available this week.",
            );
            assert!(matches!(&food, RadrootsAdmittedEvent::FoodAvailability(_)));
            assert_admitted(food, "radroots.food.availability.v1");
        }

        #[test]
        fn generic_fallback_and_invalid_outcomes_remain_distinct() {
            let generic = admitted(300, KIND_FOLLOW, vec![], "{}");
            assert!(matches!(
                &generic,
                RadrootsAdmittedEvent::ContractValidated(_)
            ));
            assert_admitted(generic, "radroots.social.follow_list.v1");

            let unsupported = admit(301, u32::from(u16::MAX), vec![], "unsupported")
                .expect_err("unregistered kind must remain unsupported");
            assert!(matches!(
                unsupported,
                RadrootsEventAdmissionError::ContractMatch(
                    RadrootsContractMatchError::UnsupportedKind(_)
                )
            ));

            let unsupported_listing =
                admit(302, KIND_CLASSIFIED_LISTING, vec![], "generic listing")
                    .expect_err("generic NIP-99 shape must remain unsupported");
            assert!(matches!(
                unsupported_listing,
                RadrootsEventAdmissionError::ContractMatch(
                    RadrootsContractMatchError::UnsupportedShape(KIND_CLASSIFIED_LISTING)
                )
            ));

            let tolerant_profile = admitted(
                303,
                KIND_PROFILE,
                vec![vec!["p".into(), "invalid".into()]],
                "{}",
            );
            assert!(matches!(
                &tolerant_profile,
                RadrootsAdmittedEvent::Profile(_)
            ));
            assert_admitted(tolerant_profile, "radroots.profile.metadata.v1");

            let invalid_profile = admit(306, KIND_PROFILE, vec![], "not JSON")
                .expect_err("invalid Profile metadata must fail at the typed boundary");
            assert!(matches!(
                invalid_profile,
                RadrootsEventAdmissionError::Profile(_)
            ));

            let invalid_reply = admit(
                304,
                KIND_POST,
                vec![vec![
                    "e".into(),
                    "invalid".into(),
                    String::new(),
                    "root".into(),
                ]],
                "Reply",
            )
            .expect_err("thread candidate must fail at the Reply boundary");
            assert!(matches!(
                invalid_reply,
                RadrootsEventAdmissionError::Reply(_)
            ));

            let mut mixed_tags = food_tags();
            mixed_tags.push(vec!["radroots:bin".into(), "bin-1".into()]);
            let mixed = admit(305, KIND_CLASSIFIED_LISTING, mixed_tags, "Mixed listing")
                .expect_err("mixed classified-listing markers must fail typed admission");
            assert!(matches!(
                &mixed,
                RadrootsEventAdmissionError::FoodAvailability(_)
            ));
            assert_eq!(mixed.code(), "food_profile_ambiguous");
        }

        fn admitted(
            created_at: u64,
            kind: u32,
            tags: Vec<Vec<String>>,
            content: &str,
        ) -> RadrootsAdmittedEvent {
            admit(created_at, kind, tags, content).expect("event must be admitted")
        }

        fn admit(
            created_at: u64,
            kind: u32,
            tags: Vec<Vec<String>>,
            content: &str,
        ) -> Result<RadrootsAdmittedEvent, RadrootsEventAdmissionError> {
            let verified = verify_nip01_event(signed_event(created_at, kind, tags, content))
                .expect("fixed signed event must verify");
            admit_verified_event(verified)
        }

        fn assert_admitted(event: RadrootsAdmittedEvent, expected_contract_id: &str) {
            let expected_event = event.event().clone();
            assert_eq!(event.contract_id(), expected_contract_id);
            assert_eq!(event.verified_event().event(), &expected_event);
            assert_eq!(event.into_verified_event().into_event(), expected_event);
        }

        fn signed_event(
            created_at: u64,
            kind: u32,
            tags: Vec<Vec<String>>,
            content: &str,
        ) -> RadrootsEventEnvelope {
            let keys = Keys::parse(FIXTURE_ALICE_SECRET_KEY_HEX)
                .expect("fixed fixture secret key must parse");
            let author = keys.public_key().to_string();
            let id = compute_canonical_nip01_event_id(&author, created_at, kind, &tags, content)
                .expect("canonical event id");
            let nostr_id = nostr::EventId::from_hex(id.as_str()).expect("Nostr event id");
            let message = Message::from_digest(nostr_id.to_bytes());
            let signature = SECP256K1.sign_schnorr_no_aux_rand(&message, keys.key_pair(SECP256K1));

            RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
                id: id.into_string(),
                author,
                created_at,
                kind,
                tags,
                content: content.into(),
                sig: signature.to_string(),
            })
            .expect("valid signed event envelope")
        }

        fn comment_tags() -> Vec<Vec<String>> {
            vec![
                vec![
                    "E".into(),
                    "a".repeat(64),
                    String::new(),
                    FIXTURE_ALICE_PUBLIC_KEY_HEX.into(),
                ],
                vec!["K".into(), KIND_CLASSIFIED_LISTING.to_string()],
                vec!["P".into(), FIXTURE_ALICE_PUBLIC_KEY_HEX.into()],
                vec![
                    "e".into(),
                    "a".repeat(64),
                    String::new(),
                    FIXTURE_ALICE_PUBLIC_KEY_HEX.into(),
                ],
                vec!["k".into(), KIND_CLASSIFIED_LISTING.to_string()],
                vec!["p".into(), FIXTURE_ALICE_PUBLIC_KEY_HEX.into()],
            ]
        }

        fn food_tags() -> Vec<Vec<String>> {
            vec![
                vec!["d".into(), "nantes-carrots".into()],
                vec!["title".into(), "Nantes Carrots".into()],
                vec!["summary".into(), "Fresh bunches".into()],
                vec!["published_at".into(), "100".into()],
                vec!["location".into(), "Central Saanich, BC".into()],
                vec!["price".into(), "3".into(), "CAD".into()],
                vec!["radroots:price_unit".into(), "lb".into()],
                vec!["radroots:quantity".into(), "24".into(), "lb".into()],
                vec!["status".into(), "active".into()],
            ]
        }
    }
}
