//! Strict publication allowlist for sealed Phase 1 artifacts.
//!
//! This authority is additive to event-contract registry v7. It answers only
//! whether a sealed authored artifact may enter the Phase 1 durable
//! publication lane. It does not broaden inbound admission, sign an event,
//! prove media readiness, publish to a relay, or grant product entitlement.

use core::fmt;

use radroots_event::{
    RadrootsEventTags,
    classified_listing::RadrootsClassifiedListingPartition,
    kinds::{
        KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_CLASSIFIED_LISTING, KIND_POST,
        KIND_PROFILE,
    },
};

use crate::{
    food_availability::inbound::{
        RadrootsFoodAvailabilityProjectionOutcome,
        registry_v7::project_inbound_food_availability_parts,
    },
    post::inbound::{RadrootsPostClassification, registry_v7::project_inbound_post_parts},
};

use super::{
    RadrootsPhase1PublicationArtifact, RadrootsPhase1PublicationArtifactError,
    RadrootsPhase1PublicationEventVariant, RadrootsPhase1PublicationSemanticVariant,
    validate_calendar_date, validate_calendar_time, validate_food_availability,
    validate_phase1_publication_artifact, validate_profile,
};

pub const RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_VERSION: u32 = 1;
pub const RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_CONTRACT_ID: &str =
    "radroots.phase1.publication_allowlist.v1";
pub const RADROOTS_PHASE1_PUBLICATION_ALLOWLIST_REGISTRY_VERSION: u32 = 7;

/// The exact seven authored leaves accepted by the Phase 1 publication lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsPhase1PublicationLeaf {
    Profile,
    Update,
    PhotoUpdate,
    Ask,
    EventDate,
    EventTime,
    FoodAvailability,
}

impl RadrootsPhase1PublicationLeaf {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Update => "update",
            Self::PhotoUpdate => "photo_update",
            Self::Ask => "ask",
            Self::EventDate => "event_date",
            Self::EventTime => "event_time",
            Self::FoodAvailability => "food_availability",
        }
    }

    pub const fn kind(self) -> u32 {
        self.semantic_variant().kind()
    }

    pub const fn authored_operation_id(self) -> &'static str {
        self.semantic_variant().authored_operation_id()
    }

    pub const fn event_contract_id(self) -> &'static str {
        self.semantic_variant().event_contract_id()
    }

    const fn from_semantic_variant(variant: RadrootsPhase1PublicationSemanticVariant) -> Self {
        match variant {
            RadrootsPhase1PublicationSemanticVariant::Profile => Self::Profile,
            RadrootsPhase1PublicationSemanticVariant::Update => Self::Update,
            RadrootsPhase1PublicationSemanticVariant::PhotoUpdate => Self::PhotoUpdate,
            RadrootsPhase1PublicationSemanticVariant::Ask => Self::Ask,
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Date,
            ) => Self::EventDate,
            RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Time,
            ) => Self::EventTime,
            RadrootsPhase1PublicationSemanticVariant::FoodAvailability => Self::FoodAvailability,
        }
    }

    const fn semantic_variant(self) -> RadrootsPhase1PublicationSemanticVariant {
        match self {
            Self::Profile => RadrootsPhase1PublicationSemanticVariant::Profile,
            Self::Update => RadrootsPhase1PublicationSemanticVariant::Update,
            Self::PhotoUpdate => RadrootsPhase1PublicationSemanticVariant::PhotoUpdate,
            Self::Ask => RadrootsPhase1PublicationSemanticVariant::Ask,
            Self::EventDate => RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Date,
            ),
            Self::EventTime => RadrootsPhase1PublicationSemanticVariant::Event(
                RadrootsPhase1PublicationEventVariant::Time,
            ),
            Self::FoodAvailability => RadrootsPhase1PublicationSemanticVariant::FoodAvailability,
        }
    }
}

impl fmt::Display for RadrootsPhase1PublicationLeaf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A sealed artifact that passed the additive Phase 1 publication allowlist.
///
/// Private fields prevent callers from pairing a leaf with a different
/// artifact. Construction is available only through
/// [`allow_phase1_publication_artifact`] and its strict canonical-reload adapter
/// [`allow_phase1_publication_canonical_json`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsPhase1AllowlistedPublicationArtifact {
    artifact: RadrootsPhase1PublicationArtifact,
    leaf: RadrootsPhase1PublicationLeaf,
}

impl RadrootsPhase1AllowlistedPublicationArtifact {
    pub const fn leaf(&self) -> RadrootsPhase1PublicationLeaf {
        self.leaf
    }

    pub fn artifact(&self) -> &RadrootsPhase1PublicationArtifact {
        &self.artifact
    }

    pub fn into_artifact(self) -> RadrootsPhase1PublicationArtifact {
        self.artifact
    }
}

/// Revalidates and reprojects a sealed artifact before granting Phase 1
/// publication-lane authority.
///
/// Kind `1` is classified with Ask-before-PhotoUpdate-before-Update
/// precedence. Calendar kinds must retain their strict date/time authored
/// profile, and kind `30402` is marker-partitioned before focused Food profile
/// validation. Every other kind and product family fails closed.
pub fn allow_phase1_publication_artifact(
    artifact: RadrootsPhase1PublicationArtifact,
) -> Result<RadrootsPhase1AllowlistedPublicationArtifact, RadrootsPhase1PublicationAllowlistError> {
    validate_phase1_publication_artifact(&artifact)
        .map_err(RadrootsPhase1PublicationAllowlistError::ArtifactInvalid)?;

    let claimed = RadrootsPhase1PublicationLeaf::from_semantic_variant(artifact.semantic_variant());
    let projected = reproject_publication_leaf(&artifact)?;
    if claimed != projected {
        return Err(
            RadrootsPhase1PublicationAllowlistError::SemanticVariantMismatch { claimed, projected },
        );
    }

    Ok(RadrootsPhase1AllowlistedPublicationArtifact {
        artifact,
        leaf: projected,
    })
}

/// Reloads exact canonical artifact bytes before applying the Phase 1
/// publication allowlist.
///
/// This is the durable-reload boundary for persisted `09a0a` artifacts. Raw
/// NIP-01 event JSON and malformed or semantically invalid artifact candidates
/// fail in the predecessor parser and are returned as
/// [`RadrootsPhase1PublicationAllowlistError::ArtifactInvalid`].
pub fn allow_phase1_publication_canonical_json(
    bytes: &[u8],
) -> Result<RadrootsPhase1AllowlistedPublicationArtifact, RadrootsPhase1PublicationAllowlistError> {
    let artifact = RadrootsPhase1PublicationArtifact::from_canonical_json(bytes)
        .map_err(RadrootsPhase1PublicationAllowlistError::ArtifactInvalid)?;
    allow_phase1_publication_artifact(artifact)
}

fn reproject_publication_leaf(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<RadrootsPhase1PublicationLeaf, RadrootsPhase1PublicationAllowlistError> {
    let draft = artifact.draft();
    match draft.kind() {
        KIND_PROFILE => {
            validate_profile(draft, artifact.media_references())
                .map_err(|_| RadrootsPhase1PublicationAllowlistError::ProfileInvalid)?;
            Ok(RadrootsPhase1PublicationLeaf::Profile)
        }
        KIND_POST => {
            let projection =
                project_inbound_post_parts(draft.kind(), draft.tags(), draft.content()).map_err(
                    |error| RadrootsPhase1PublicationAllowlistError::PostInvalid {
                        source_code: error.code(),
                    },
                )?;
            if !projection.diagnostics().is_empty() {
                return Err(RadrootsPhase1PublicationAllowlistError::PostInvalid {
                    source_code: "post_projection_diagnostic",
                });
            }
            match projection.classification() {
                RadrootsPostClassification::Ask => Ok(RadrootsPhase1PublicationLeaf::Ask),
                RadrootsPostClassification::PhotoUpdate => {
                    Ok(RadrootsPhase1PublicationLeaf::PhotoUpdate)
                }
                RadrootsPostClassification::Update => Ok(RadrootsPhase1PublicationLeaf::Update),
                RadrootsPostClassification::ThreadExcluded => {
                    Err(RadrootsPhase1PublicationAllowlistError::PostExcluded)
                }
            }
        }
        KIND_CALENDAR_DATE_EVENT => {
            validate_calendar_date(draft, artifact.media_references())
                .map_err(|_| RadrootsPhase1PublicationAllowlistError::EventInvalid)?;
            Ok(RadrootsPhase1PublicationLeaf::EventDate)
        }
        KIND_CALENDAR_TIME_EVENT => {
            validate_calendar_time(draft, artifact.media_references())
                .map_err(|_| RadrootsPhase1PublicationAllowlistError::EventInvalid)?;
            Ok(RadrootsPhase1PublicationLeaf::EventTime)
        }
        KIND_CLASSIFIED_LISTING => reproject_food_availability(artifact),
        kind => Err(RadrootsPhase1PublicationAllowlistError::UnsupportedKind { kind }),
    }
}

fn reproject_food_availability(
    artifact: &RadrootsPhase1PublicationArtifact,
) -> Result<RadrootsPhase1PublicationLeaf, RadrootsPhase1PublicationAllowlistError> {
    let draft = artifact.draft();
    let tags = RadrootsEventTags::new(draft.tags().to_vec())
        .map_err(|_| RadrootsPhase1PublicationAllowlistError::FoodInvalid)?;
    let projection = project_inbound_food_availability_parts(
        draft.kind(),
        draft.created_at(),
        &tags,
        draft.content(),
    )
    .map_err(
        |error| RadrootsPhase1PublicationAllowlistError::FoodInvalidWithSource {
            source_code: error.code(),
        },
    )?;
    match projection {
        RadrootsFoodAvailabilityProjectionOutcome::Focused(projection) => {
            if !projection.diagnostics().is_empty() {
                return Err(RadrootsPhase1PublicationAllowlistError::FoodInvalid);
            }
            validate_food_availability(draft, artifact.media_references())
                .map_err(|_| RadrootsPhase1PublicationAllowlistError::FoodInvalid)?;
            Ok(RadrootsPhase1PublicationLeaf::FoodAvailability)
        }
        RadrootsFoodAvailabilityProjectionOutcome::Excluded(partition) => {
            Err(RadrootsPhase1PublicationAllowlistError::FoodExcluded { partition })
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsPhase1PublicationAllowlistError {
    ArtifactInvalid(RadrootsPhase1PublicationArtifactError),
    UnsupportedKind {
        kind: u32,
    },
    ProfileInvalid,
    PostInvalid {
        source_code: &'static str,
    },
    PostExcluded,
    EventInvalid,
    FoodInvalid,
    FoodInvalidWithSource {
        source_code: &'static str,
    },
    FoodExcluded {
        partition: RadrootsClassifiedListingPartition,
    },
    SemanticVariantMismatch {
        claimed: RadrootsPhase1PublicationLeaf,
        projected: RadrootsPhase1PublicationLeaf,
    },
}

impl RadrootsPhase1PublicationAllowlistError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ArtifactInvalid(_) => "publication_allowlist_artifact_invalid",
            Self::UnsupportedKind { .. } => "publication_allowlist_kind_unsupported",
            Self::ProfileInvalid => "publication_allowlist_profile_invalid",
            Self::PostInvalid { .. } => "publication_allowlist_post_invalid",
            Self::PostExcluded => "publication_allowlist_post_excluded",
            Self::EventInvalid => "publication_allowlist_event_invalid",
            Self::FoodInvalid | Self::FoodInvalidWithSource { .. } => {
                "publication_allowlist_food_invalid"
            }
            Self::FoodExcluded { .. } => "publication_allowlist_food_excluded",
            Self::SemanticVariantMismatch { .. } => {
                "publication_allowlist_semantic_variant_mismatch"
            }
        }
    }

    pub const fn source_code(&self) -> Option<&'static str> {
        match self {
            Self::ArtifactInvalid(error) => Some(error.code()),
            Self::PostInvalid { source_code } => Some(*source_code),
            Self::FoodInvalidWithSource { source_code } => Some(*source_code),
            _ => None,
        }
    }
}

impl fmt::Display for RadrootsPhase1PublicationAllowlistError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind { kind } => {
                write!(
                    formatter,
                    "event kind {kind} is outside the Phase 1 publication allowlist"
                )
            }
            Self::FoodExcluded { partition } => write!(
                formatter,
                "classified-listing partition {partition:?} is outside the Phase 1 publication allowlist"
            ),
            Self::SemanticVariantMismatch { claimed, projected } => write!(
                formatter,
                "publication artifact claims {claimed} but reprojects as {projected}"
            ),
            error => formatter.write_str(error.code()),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsPhase1PublicationAllowlistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ArtifactInvalid(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const PREDECESSOR_VECTOR: &str =
        include_str!("../../../tests/fixtures/phase1_publication_artifact.v1.json");

    #[test]
    fn publication_allowlist_admits_exactly_the_seven_sealed_leaves() {
        let fixtures = [
            ("profile", RadrootsPhase1PublicationLeaf::Profile),
            ("update", RadrootsPhase1PublicationLeaf::Update),
            ("photo_update", RadrootsPhase1PublicationLeaf::PhotoUpdate),
            ("ask", RadrootsPhase1PublicationLeaf::Ask),
            ("event_date", RadrootsPhase1PublicationLeaf::EventDate),
            ("event_time", RadrootsPhase1PublicationLeaf::EventTime),
            (
                "food_availability",
                RadrootsPhase1PublicationLeaf::FoodAvailability,
            ),
        ];

        for (fixture, expected_leaf) in fixtures {
            let artifact = artifact_fixture(fixture);
            let canonical = artifact.to_canonical_json();
            let allowed = allow_phase1_publication_artifact(artifact).unwrap();
            assert_eq!(allowed.leaf(), expected_leaf);
            assert_eq!(allowed.leaf().as_str(), fixture);
            assert_eq!(allowed.leaf().kind(), allowed.artifact().draft().kind());
            assert_eq!(
                allowed.leaf().authored_operation_id(),
                allowed.artifact().authored_operation_id()
            );
            assert_eq!(
                allowed.leaf().event_contract_id(),
                allowed.artifact().event_contract_id()
            );
            assert_eq!(allowed.into_artifact().to_canonical_json(), canonical);
        }
    }

    #[test]
    fn publication_allowlist_canonical_reload_uses_the_same_authority() {
        for fixture in [
            "profile",
            "update",
            "photo_update",
            "ask",
            "event_date",
            "event_time",
            "food_availability",
        ] {
            let artifact = artifact_fixture(fixture);
            let expected = allow_phase1_publication_artifact(artifact.clone()).unwrap();
            let actual =
                allow_phase1_publication_canonical_json(&artifact.to_canonical_json()).unwrap();
            assert_eq!(actual, expected);
        }

        let raw_event = br#"{"id":"00","pubkey":"00","created_at":1,"kind":1,"tags":[],"content":"raw","sig":"00"}"#;
        assert_eq!(
            allow_phase1_publication_canonical_json(raw_event)
                .unwrap_err()
                .source_code(),
            Some("publication_artifact_invalid_json")
        );
    }

    #[test]
    fn publication_allowlist_reprojects_kind_one_with_exact_precedence() {
        let update = artifact_fixture("update");
        assert_eq!(
            reproject_publication_leaf(&update).unwrap(),
            RadrootsPhase1PublicationLeaf::Update
        );

        let photo = artifact_fixture("photo_update");
        assert_eq!(
            reproject_publication_leaf(&photo).unwrap(),
            RadrootsPhase1PublicationLeaf::PhotoUpdate
        );

        let ask = artifact_fixture("ask");
        assert_eq!(
            reproject_publication_leaf(&ask).unwrap(),
            RadrootsPhase1PublicationLeaf::Ask
        );

        let mut ask_over_photo = photo;
        ask_over_photo
            .draft
            .tags
            .insert(0, vec!["t".to_owned(), "radroots-ask".to_owned()]);
        assert_eq!(
            reproject_publication_leaf(&ask_over_photo).unwrap(),
            RadrootsPhase1PublicationLeaf::Ask
        );

        let mut reply = update;
        reply.draft.tags = vec![vec!["e".to_owned(), "00".repeat(32)]];
        assert_eq!(
            reproject_publication_leaf(&reply).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::PostExcluded
        );
    }

    #[test]
    fn publication_allowlist_partitions_food_markers_before_profile_validation() {
        let focused = artifact_fixture("food_availability");
        assert_eq!(
            reproject_publication_leaf(&focused).unwrap(),
            RadrootsPhase1PublicationLeaf::FoodAvailability
        );

        let mut generic = focused.clone();
        generic.draft.tags.retain(|tag| {
            !matches!(
                tag.first().map(String::as_str),
                Some("radroots:price_unit" | "radroots:quantity")
            )
        });
        assert_eq!(
            reproject_publication_leaf(&generic).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::FoodExcluded {
                partition: RadrootsClassifiedListingPartition::GenericNip99,
            }
        );

        let mut operational = generic;
        operational.draft.tags.push(vec![
            "radroots:price".to_owned(),
            "3".to_owned(),
            "CAD".to_owned(),
        ]);
        assert_eq!(
            reproject_publication_leaf(&operational).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::FoodExcluded {
                partition: RadrootsClassifiedListingPartition::OperationalListing,
            }
        );

        let mut mixed = focused;
        mixed.draft.tags.push(vec![
            "radroots:price".to_owned(),
            "3".to_owned(),
            "CAD".to_owned(),
        ]);
        assert_eq!(
            reproject_publication_leaf(&mixed).unwrap_err().code(),
            "publication_allowlist_food_invalid"
        );
    }

    #[test]
    fn publication_allowlist_rejects_generic_calendar_and_unsupported_kinds() {
        let mut generic_calendar = artifact_fixture("event_date");
        generic_calendar.draft.tags = vec![
            vec!["d".to_owned(), "generic-calendar".to_owned()],
            vec!["title".to_owned(), "Generic calendar event".to_owned()],
        ];
        assert_eq!(
            reproject_publication_leaf(&generic_calendar).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::EventInvalid
        );

        let mut bud11 = artifact_fixture("update");
        bud11.draft.kind = 24_242;
        assert_eq!(
            reproject_publication_leaf(&bud11).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::UnsupportedKind { kind: 24_242 }
        );

        let mut ephemeral = artifact_fixture("update");
        ephemeral.draft.kind = 20_001;
        assert_eq!(
            reproject_publication_leaf(&ephemeral).unwrap_err(),
            RadrootsPhase1PublicationAllowlistError::UnsupportedKind { kind: 20_001 }
        );
    }

    fn artifact_fixture(name: &str) -> RadrootsPhase1PublicationArtifact {
        let suite: Value = serde_json::from_str(PREDECESSOR_VECTOR).unwrap();
        let vector = suite["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|vector| vector["expected"]["semantic_variant"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("missing predecessor fixture {name}"));
        let canonical = vector["expected"]["canonical_json"]
            .as_str()
            .unwrap_or_else(|| panic!("missing canonical predecessor fixture {name}"));
        RadrootsPhase1PublicationArtifact::from_canonical_json(canonical.as_bytes()).unwrap()
    }
}
