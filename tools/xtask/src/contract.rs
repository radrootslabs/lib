#![forbid(unsafe_code)]

mod admission_authority;
mod artifact_bundle;
mod comment_authority;
mod deletion_authority;
mod registry_v7;
pub(crate) use registry_v7::{
    validate_event_contract_registry_v7_inventory, write_event_contract_registry_v7_inventory,
};

use crate::coverage::{CoveragePolicyFile, CoverageThresholds, read_coverage_policy};
use admission_authority::validate_admission_operation_authority;
use artifact_bundle::{
    GeneratedArtifact, read_regular_file, validate_canonical_json_artifact,
    validate_sha256_artifact, with_artifact_bundle_transaction,
};
use comment_authority::{
    COMMENT_CASE_KINDS, COMMENT_CONFORMANCE_VECTOR_RELATIVE, COMMENT_OPERATION_EXPECTATIONS,
    COMMENT_VECTOR_EXPECTATIONS, REQUIRED_COMMENT_PUBLIC_TYPES,
};
use deletion_authority::{
    DELETION_ADMIT_INVALID_IDS, DELETION_ADMIT_VALID_IDS, DELETION_AUTHORED_INVALID_IDS,
    DELETION_AUTHORED_VALID_IDS, DELETION_CASE_KINDS, DELETION_CONFORMANCE_VECTOR_RELATIVE,
    DELETION_OPERATION_EXPECTATIONS, DELETION_PROJECT_INVALID_IDS, DELETION_PROJECT_VALID_IDS,
    DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE, DELETION_SUPPRESSION_VALID_IDS,
    REQUIRED_DELETION_PUBLIC_TYPES,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn validate_artifact_contracts(workspace_root: &Path) -> Result<(), String> {
    validate_event_contract_registry_v7_inventory(workspace_root)?;
    validate_knowledge_contract_manifest(workspace_root)
}

const CONFORMANCE_ROOT_RELATIVE: &str = "contracts/conformance";
const CONFORMANCE_SCHEMA_RELATIVE: &str = "contracts/conformance/schema/vector.schema.json";
const KNOWLEDGE_MANIFEST_RELATIVE: &str =
    "contracts/knowledge/knowledge_event_contract_manifest.v2.json";
const KNOWLEDGE_MANIFEST_SHA256_RELATIVE: &str =
    "contracts/knowledge/knowledge_event_contract_manifest.v2.sha256";
const KNOWLEDGE_MANIFEST_WRITE_COMMAND: &str = "cargo xtask contract knowledge-manifest --write";
const KNOWLEDGE_MANIFEST_AND_DECODE_RELATIVE: &str =
    "contracts/conformance/vectors/knowledge/manifest_and_decode.v1.json";
const KNOWLEDGE_PUBLIC_SURFACE_RELATIVE: &str =
    "contracts/conformance/vectors/knowledge/public_surface.v1.json";
const POST_CONFORMANCE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/post/verified_profiles.v1.json";
const FOOD_AVAILABILITY_CONFORMANCE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/food_availability/profile.v1.json";
const RELEASES_ROOT_RELATIVE: &str = "contracts/releases";
const RELEASE_POLICY_RELATIVE: &str = "contracts/releases/publish_policy.toml";
const SQLITE_RUNTIME_CONTRACT_RELATIVE: &str = "contracts/releases/sqlite_runtime.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const REPLICA_CONTRACT_RELATIVE: &str = "contracts/replica.toml";
const REPLICA_CONTRACT_NAME: &str = "radroots_replica_contract";
const REPLICA_TRANSFER_CONSTANT: &str = "RADROOTS_REPLICA_TRANSFER_VERSION";
const REPLICA_TRANSFER_VERSION: u32 = 2;
const CONFORMANCE_VECTOR_MIRRORS: [(&str, &str); 20] = [
    (
        "contracts/conformance/vectors/blossom/bud11_claims.v1.json",
        "crates/blossom/tests/fixtures/bud11_claims.v1.json",
    ),
    (
        "contracts/conformance/vectors/blossom/hash_path_and_descriptor.v1.json",
        "crates/blossom/tests/fixtures/hash_path_and_descriptor.v1.json",
    ),
    (
        "contracts/conformance/vectors/blossom/bud11_nostr_adapter.v1.json",
        "crates/nostr/tests/fixtures/bud11_nostr_adapter.v1.json",
    ),
    (
        "contracts/conformance/vectors/nip17/adapter.v1.json",
        "crates/nostr/tests/fixtures/nip17_adapter.v1.json",
    ),
    (
        "contracts/conformance/vectors/calendar/nip52_baseline.v1.json",
        "crates/event_codec/tests/fixtures/calendar_nip52_baseline.v1.json",
    ),
    (
        "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
        "crates/event_codec/tests/fixtures/calendar_radroots_profile.v1.json",
    ),
    (
        "contracts/conformance/vectors/comment/verified_profile.v1.json",
        "crates/event_codec/tests/fixtures/comment_verified_profile.v1.json",
    ),
    (
        "contracts/conformance/vectors/deletion/verified_profile.v1.json",
        "crates/event_codec/tests/fixtures/deletion_verified_profile.v1.json",
    ),
    (
        "contracts/conformance/vectors/deletion/suppression.v1.json",
        "crates/event_codec/tests/fixtures/deletion_suppression.v1.json",
    ),
    (
        "contracts/conformance/vectors/event/verified_admission.v1.json",
        "crates/event_codec/tests/fixtures/verified_admission.v1.json",
    ),
    (
        "contracts/conformance/vectors/events/operational_listing_tags_full.v1.json",
        "crates/event_codec/tests/fixtures/operational_listing_tags_full.v1.json",
    ),
    (
        "contracts/conformance/vectors/food_availability/profile.v1.json",
        "crates/event_codec/tests/fixtures/food_availability_profile.v1.json",
    ),
    (
        "contracts/conformance/vectors/operational_listing/build_draft.v1.json",
        "crates/event_codec/tests/fixtures/operational_listing_build_draft.v1.json",
    ),
    (
        "contracts/conformance/vectors/operational_listing/build_tags.v1.json",
        "crates/event_codec/tests/fixtures/operational_listing_build_tags.v1.json",
    ),
    (
        "contracts/conformance/vectors/operational_listing/parse_event.v1.json",
        "crates/event_codec/tests/fixtures/operational_listing_parse_event.v1.json",
    ),
    (
        "contracts/conformance/vectors/profile/metadata.v1.json",
        "crates/event_codec/tests/fixtures/profile_metadata.v1.json",
    ),
    (
        "contracts/conformance/vectors/profile/verified_event.v1.json",
        "crates/event_codec/tests/fixtures/profile_verified_event.v1.json",
    ),
    (
        "contracts/conformance/vectors/post/verified_profiles.v1.json",
        "crates/event_codec/tests/fixtures/post_verified_profiles.v1.json",
    ),
    (
        "contracts/conformance/vectors/trade/parse_classified_listing_address.v1.json",
        "crates/trade/tests/fixtures/parse_classified_listing_address.v1.json",
    ),
    (
        "contracts/conformance/vectors/trade_validation/validate_operational_listing_event.v1.json",
        "crates/trade/tests/fixtures/validate_operational_listing_event.v1.json",
    ),
];
const KNOWLEDGE_MVP_SUPPORT_CONTRACT_IDS: [&str; 8] = [
    "radroots.wiki.article.v1",
    "radroots.wiki.redirect.v1",
    "radroots.wiki.merge_request.v1",
    "radroots.knowledge.source.v1",
    "radroots.knowledge.claim.v1",
    "radroots.knowledge.relation.v1",
    "radroots.knowledge.review.v1",
    "radroots.knowledge.field_report.v1",
];
const KNOWLEDGE_BETA_CONTRACT_IDS: [&str; 3] = [
    "radroots.knowledge.evidence_bounty.v1",
    "radroots.knowledge.change_proposal.v1",
    "radroots.knowledge.contribution_attestation.v1",
];
const EVENT_BOUNDARY_MATRIX_ENV: &str = "RADROOTS_EVENT_BOUNDARY_MATRIX";
const COVERAGE_REQUIRED_THRESHOLD: f64 = 90.0;
const COVERAGE_REQUIRED_THRESHOLD_LABEL: &str = "90/90/90/90";
const COVERAGE_REPORT_EPSILON: f64 = 0.000_001;
const DTO_TOOLING_DEPENDENCIES: [&str; 4] = [
    "dto_bindgen",
    "dto_bindgen_backend_ts",
    "dto_bindgen_core",
    "dto_bindgen_macros",
];
const RETIRED_OPERATION_EVENT_NAMES: [&str; 15] = [
    "WireEventParts",
    "RadrootsFrozenEventDraft",
    "RadrootsNostrEvent",
    "RadrootsNostrEventRef",
    "RadrootsNostrEventPtr",
    "RadrootsCalendarDateEvent",
    "RadrootsCalendarTimeEvent",
    "RadrootsCalendarDateValue",
    "RadrootsInboundCalendarDateEvent",
    "RadrootsInboundCalendarTimeEvent",
    "RadrootsCalendar",
    "RadrootsCalendarEventRsvp",
    "RadrootsCalendarRsvp",
    "RadrootsComment",
    "RadrootsNip10RelayHint",
];
const REQUIRED_CALENDAR_PUBLIC_TYPES: [&str; 34] = [
    "Nip01EventWireParts",
    "BlobUrl",
    "AuthoredImage",
    "AuthoredImageError",
    "IanaTimeZoneId",
    "CalendarUri",
    "CalendarRequest",
    "CalendarParticipant",
    "CalendarEventError",
    "CalendarDate",
    "AuthoredCalendarDateEvent",
    "AuthoredCalendarTimeEvent",
    "ParsedNip52CalendarCommon",
    "ParsedNip52CalendarCommonParts",
    "ParsedNip52CalendarDateEvent",
    "ObservedUtcDay",
    "ParsedNip52CalendarTimeEvent",
    "CalendarAdmissionError",
    "AdmittedCalendarDateEvent",
    "AdmittedCalendarTimeEvent",
    "CalendarUid",
    "CalendarEventReference",
    "CalendarEventRevisionReference",
    "CalendarEventAuthorReference",
    "AuthoredCalendar",
    "ParsedNip52CalendarParts",
    "ParsedNip52Calendar",
    "AdmittedCalendar",
    "CalendarEventRsvpStatus",
    "CalendarEventFreeBusy",
    "AuthoredCalendarEventRsvp",
    "ParsedNip52CalendarEventRsvpParts",
    "ParsedNip52CalendarEventRsvp",
    "AdmittedCalendarEventRsvp",
];
const REQUIRED_POST_PUBLIC_TYPES: [&str; 34] = [
    "ApprovedBlobUrl",
    "ByteVerifiedDescriptor",
    "Nip01EventWireParts",
    "EventEnvelope",
    "RadrootsSignatureVerifiedEvent",
    "AuthoredImage",
    "Post",
    "AuthoredPostError",
    "PostImageDimensions",
    "AuthoredPostImage",
    "AuthoredUpdate",
    "AuthoredPhotoUpdate",
    "AuthoredAsk",
    "Nip10ReplyError",
    "NostrRelayHint",
    "Nip10ReplyReference",
    "AuthoredNip10Reply",
    "RadrootsPostDiagnostic",
    "RadrootsPostClassification",
    "RadrootsInboundPostImeta",
    "RadrootsInboundPostProjection",
    "RadrootsPostProjectionError",
    "RadrootsAdmittedRootPostEvent",
    "RadrootsThreadExcludedPostCandidate",
    "RadrootsPostAdmissionOutcome",
    "RadrootsPostAdmissionError",
    "RadrootsNip10ReplyStyle",
    "RadrootsNip10ReplyDiagnostic",
    "RadrootsInboundNip10EventReference",
    "RadrootsInboundNip10Participant",
    "RadrootsInboundNip10ReplyProjection",
    "RadrootsNip10ReplyProjectionError",
    "RadrootsAdmittedNip10ReplyEvent",
    "RadrootsNip10ReplyAdmissionError",
];
const REQUIRED_FOOD_AVAILABILITY_PUBLIC_TYPES: [&str; 31] = [
    "ByteVerifiedDescriptor",
    "Nip01EventWireParts",
    "EventEnvelope",
    "RadrootsSignatureVerifiedEvent",
    "ClassifiedListingPartition",
    "AuthoredImage",
    "FoodAvailabilityError",
    "FoodContent",
    "FoodIdentifier",
    "FoodText",
    "FoodPublishedAt",
    "FoodCurrency",
    "FoodUnit",
    "FoodPrice",
    "FoodQuantity",
    "FoodAvailabilityStatus",
    "FoodImageDimensions",
    "FoodAvailabilityImage",
    "FoodAvailabilityDetailsParts",
    "FoodAvailabilityDetails",
    "RadrootsFoodAvailabilityEncodeError",
    "RadrootsFoodAvailabilityImageDiagnostic",
    "RadrootsInboundFoodAvailabilityImage",
    "RadrootsInboundFoodAvailabilityProjection",
    "RadrootsFoodAvailabilityProjectionOutcome",
    "RadrootsFoodAvailabilityProjectionError",
    "RadrootsAdmittedFoodAvailabilityEvent",
    "RadrootsExcludedClassifiedListingCandidate",
    "RadrootsFoodAvailabilityAdmissionOutcome",
    "RadrootsFoodAvailabilityAdmissionError",
    "RadrootsFoodAvailabilityRevisionError",
];
const CALENDAR_OPERATION_EXPECTATIONS: [CalendarOperationExpectation; 12] = [
    CalendarOperationExpectation {
        key: "social_calendar_date_event_build_authored_draft",
        id: "social.calendar_date_event.build_authored_draft",
        inputs: &["AuthoredCalendarDateEvent"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event/src/media.rs",
            "crates/event_codec/src/calendar/encode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AuthoredCalendarDateEvent",
            "radroots_event::calendar::CalendarDate",
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::media::AuthoredImage",
            "radroots_event::wire::Nip01EventWireParts",
            "radroots_event_codec::error::EventEncodeError",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_date_event_parse_nip52",
        id: "social.calendar_date_event.parse_nip52",
        inputs: &["u32", "NostrTags", "String"],
        outputs: &["ParsedNip52CalendarDateEvent"],
        error_class: "parse_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::CalendarDate",
            "radroots_event::calendar::CalendarRequest",
            "radroots_event::calendar::CalendarUri",
            "radroots_event::calendar::ParsedNip52CalendarCommon",
            "radroots_event::calendar::ParsedNip52CalendarCommonParts",
            "radroots_event::calendar::ParsedNip52CalendarDateEvent",
            "radroots_event_codec::error::EventParseError",
        ],
        vector: "contracts/conformance/vectors/calendar/nip52_baseline.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_date_event_admit_radroots_profile",
        id: "social.calendar_date_event.admit_radroots_profile",
        inputs: &["ParsedNip52CalendarDateEvent"],
        outputs: &["AdmittedCalendarDateEvent"],
        error_class: "admission_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AdmittedCalendarDateEvent",
            "radroots_event::calendar::CalendarAdmissionError",
            "radroots_event::calendar::ParsedNip52CalendarDateEvent",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_time_event_build_authored_draft",
        id: "social.calendar_time_event.build_authored_draft",
        inputs: &["AuthoredCalendarTimeEvent"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event/src/media.rs",
            "crates/event_codec/src/calendar/encode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AuthoredCalendarTimeEvent",
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::calendar::IanaTimeZoneId",
            "radroots_event::media::AuthoredImage",
            "radroots_event::wire::Nip01EventWireParts",
            "radroots_event_codec::error::EventEncodeError",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_time_event_parse_nip52",
        id: "social.calendar_time_event.parse_nip52",
        inputs: &["u32", "NostrTags", "String"],
        outputs: &["ParsedNip52CalendarTimeEvent"],
        error_class: "parse_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::CalendarRequest",
            "radroots_event::calendar::CalendarUri",
            "radroots_event::calendar::IanaTimeZoneId",
            "radroots_event::calendar::ObservedUtcDay",
            "radroots_event::calendar::ParsedNip52CalendarCommon",
            "radroots_event::calendar::ParsedNip52CalendarCommonParts",
            "radroots_event::calendar::ParsedNip52CalendarTimeEvent",
            "radroots_event_codec::error::EventParseError",
        ],
        vector: "contracts/conformance/vectors/calendar/nip52_baseline.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_time_event_admit_radroots_profile",
        id: "social.calendar_time_event.admit_radroots_profile",
        inputs: &["ParsedNip52CalendarTimeEvent"],
        outputs: &["AdmittedCalendarTimeEvent"],
        error_class: "admission_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AdmittedCalendarTimeEvent",
            "radroots_event::calendar::CalendarAdmissionError",
            "radroots_event::calendar::ParsedNip52CalendarTimeEvent",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_build_authored_draft",
        id: "social.calendar.build_authored_draft",
        inputs: &["AuthoredCalendar"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event/src/media.rs",
            "crates/event_codec/src/calendar/encode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AuthoredCalendar",
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarUid",
            "radroots_event::media::AuthoredImage",
            "radroots_event::wire::Nip01EventWireParts",
            "radroots_event_codec::error::EventEncodeError",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_parse_nip52",
        id: "social.calendar.parse_nip52",
        inputs: &["u32", "NostrTags", "String"],
        outputs: &["ParsedNip52Calendar"],
        error_class: "parse_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarUri",
            "radroots_event::calendar::ParsedNip52Calendar",
            "radroots_event::calendar::ParsedNip52CalendarParts",
            "radroots_event_codec::error::EventParseError",
        ],
        vector: "contracts/conformance/vectors/calendar/nip52_baseline.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_admit_radroots_profile",
        id: "social.calendar.admit_radroots_profile",
        inputs: &["ParsedNip52Calendar"],
        outputs: &["AdmittedCalendar"],
        error_class: "admission_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_blossom::BlobUrl",
            "radroots_event::calendar::AdmittedCalendar",
            "radroots_event::calendar::CalendarAdmissionError",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarUid",
            "radroots_event::calendar::ParsedNip52Calendar",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_rsvp_build_authored_draft",
        id: "social.calendar_rsvp.build_authored_draft",
        inputs: &["AuthoredCalendarEventRsvp"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/encode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AuthoredCalendarEventRsvp",
            "radroots_event::calendar::CalendarEventAuthorReference",
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarEventRevisionReference",
            "radroots_event::calendar::CalendarUid",
            "radroots_event::calendar::CalendarEventFreeBusy",
            "radroots_event::calendar::CalendarEventRsvpStatus",
            "radroots_event::wire::Nip01EventWireParts",
            "radroots_event_codec::error::EventEncodeError",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_rsvp_parse_nip52",
        id: "social.calendar_rsvp.parse_nip52",
        inputs: &["u32", "NostrTags", "String"],
        outputs: &["ParsedNip52CalendarEventRsvp"],
        error_class: "parse_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::CalendarEventAuthorReference",
            "radroots_event::calendar::CalendarEventError",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarEventRevisionReference",
            "radroots_event::calendar::ParsedNip52CalendarEventRsvp",
            "radroots_event::calendar::ParsedNip52CalendarEventRsvpParts",
            "radroots_event::calendar::CalendarEventFreeBusy",
            "radroots_event::calendar::CalendarEventRsvpStatus",
            "radroots_event_codec::error::EventParseError",
        ],
        vector: "contracts/conformance/vectors/calendar/nip52_baseline.v1.json",
    },
    CalendarOperationExpectation {
        key: "social_calendar_rsvp_admit_radroots_profile",
        id: "social.calendar_rsvp.admit_radroots_profile",
        inputs: &["ParsedNip52CalendarEventRsvp"],
        outputs: &["AdmittedCalendarEventRsvp"],
        error_class: "admission_error",
        rust_modules: &[
            "crates/event/src/calendar.rs",
            "crates/event_codec/src/calendar/decode.rs",
        ],
        rust_types: &[
            "radroots_event::calendar::AdmittedCalendarEventRsvp",
            "radroots_event::calendar::CalendarAdmissionError",
            "radroots_event::calendar::CalendarEventAuthorReference",
            "radroots_event::calendar::CalendarEventReference",
            "radroots_event::calendar::CalendarEventRevisionReference",
            "radroots_event::calendar::CalendarUid",
            "radroots_event::calendar::ParsedNip52CalendarEventRsvp",
            "radroots_event::calendar::CalendarEventFreeBusy",
            "radroots_event::calendar::CalendarEventRsvpStatus",
        ],
        vector: "contracts/conformance/vectors/calendar/radroots_profile.v1.json",
    },
];
const POST_OPERATION_EXPECTATIONS: [PostOperationExpectation; 8] = [
    PostOperationExpectation {
        key: "social_update_build_authored_draft",
        id: "social.update.build_authored_draft",
        inputs: &["AuthoredUpdate"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/post.rs",
            "crates/event_codec/src/post/authored.rs",
        ],
        rust_types: &[
            "radroots_event::post::AuthoredPostError",
            "radroots_event::post::AuthoredUpdate",
        ],
        case_kinds: &[
            "social.update.build_authored_draft.valid",
            "social.update.build_authored_draft.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_photo_update_build_authored_draft",
        id: "social.photo_update.build_authored_draft",
        inputs: &["AuthoredPhotoUpdate"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/post.rs",
            "crates/event_codec/src/post/authored.rs",
        ],
        rust_types: &[
            "radroots_blossom::ApprovedBlobUrl",
            "radroots_blossom::ByteVerifiedDescriptor",
            "radroots_event::media::AuthoredImage",
            "radroots_event::post::AuthoredPhotoUpdate",
            "radroots_event::post::AuthoredPostError",
            "radroots_event::post::AuthoredPostImage",
            "radroots_event::post::PostImageDimensions",
        ],
        case_kinds: &[
            "social.photo_update.build_authored_draft.valid",
            "social.photo_update.build_authored_draft.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_ask_build_authored_draft",
        id: "social.ask.build_authored_draft",
        inputs: &["AuthoredAsk"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/post.rs",
            "crates/event_codec/src/post/authored.rs",
        ],
        rust_types: &[
            "radroots_event::post::AuthoredAsk",
            "radroots_event::post::AuthoredPostError",
            "radroots_event::post::AuthoredPostImage",
        ],
        case_kinds: &[
            "social.ask.build_authored_draft.valid",
            "social.ask.build_authored_draft.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_reply_build_authored_draft",
        id: "social.reply.build_authored_draft",
        inputs: &["AuthoredNip10Reply"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/relay_hint.rs",
            "crates/event/src/reply.rs",
            "crates/event_codec/src/reply/authored.rs",
        ],
        rust_types: &[
            "radroots_event::post::reply::AuthoredNip10Reply",
            "radroots_event::post::reply::Nip10ReplyError",
            "radroots_event::tag::relay_hint::NostrRelayHint",
            "radroots_event::post::reply::Nip10ReplyReference",
        ],
        case_kinds: &[
            "social.reply.build_authored_draft.valid",
            "social.reply.build_authored_draft.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_reply_project_verified_event",
        id: "social.reply.project_verified_event",
        inputs: &["RadrootsSignatureVerifiedEvent"],
        outputs: &["RadrootsInboundNip10ReplyProjection"],
        error_class: "parse_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/relay_hint.rs",
            "crates/event_codec/src/reply/inbound.rs",
        ],
        rust_types: &[
            "radroots_event::tag::relay_hint::NostrRelayHint",
            "radroots_event_codec::reply::inbound::RadrootsInboundNip10EventReference",
            "radroots_event_codec::reply::inbound::RadrootsInboundNip10Participant",
            "radroots_event_codec::reply::inbound::RadrootsInboundNip10ReplyProjection",
            "radroots_event_codec::reply::inbound::RadrootsNip10ReplyDiagnostic",
            "radroots_event_codec::reply::inbound::RadrootsNip10ReplyProjectionError",
            "radroots_event_codec::reply::inbound::RadrootsNip10ReplyStyle",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.reply.project_verified_event.valid",
            "social.reply.project_verified_event.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_reply_verify_and_admit_event",
        id: "social.reply.verify_and_admit_event",
        inputs: &["EventEnvelope"],
        outputs: &["RadrootsAdmittedNip10ReplyEvent"],
        error_class: "admission_error",
        signing: "nip01",
        rust_modules: &[
            "crates/event_codec/src/reply/admission.rs",
            "crates/event_codec/src/reply/inbound.rs",
            "crates/event_codec/src/verification.rs",
        ],
        rust_types: &[
            "radroots_event::envelope::EventEnvelope",
            "radroots_event_codec::reply::admission::RadrootsAdmittedNip10ReplyEvent",
            "radroots_event_codec::reply::admission::RadrootsNip10ReplyAdmissionError",
            "radroots_event_codec::reply::inbound::RadrootsInboundNip10ReplyProjection",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.reply.verify_and_admit_event.valid",
            "social.reply.verify_and_admit_event.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_post_project_verified_event",
        id: "social.post.project_verified_event",
        inputs: &["RadrootsSignatureVerifiedEvent"],
        outputs: &["RadrootsInboundPostProjection"],
        error_class: "parse_error",
        signing: "none",
        rust_modules: &["crates/event_codec/src/post/inbound.rs"],
        rust_types: &[
            "radroots_event_codec::post::inbound::RadrootsInboundPostImeta",
            "radroots_event_codec::post::inbound::RadrootsInboundPostProjection",
            "radroots_event_codec::post::inbound::RadrootsPostClassification",
            "radroots_event_codec::post::inbound::RadrootsPostDiagnostic",
            "radroots_event_codec::post::inbound::RadrootsPostProjectionError",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.post.project_verified_event.valid",
            "social.post.project_verified_event.invalid",
        ],
    },
    PostOperationExpectation {
        key: "social_post_verify_and_admit_event",
        id: "social.post.verify_and_admit_event",
        inputs: &["EventEnvelope"],
        outputs: &["RadrootsPostAdmissionOutcome"],
        error_class: "admission_error",
        signing: "nip01",
        rust_modules: &[
            "crates/event_codec/src/post/admission.rs",
            "crates/event_codec/src/post/inbound.rs",
            "crates/event_codec/src/verification.rs",
        ],
        rust_types: &[
            "radroots_event::envelope::EventEnvelope",
            "radroots_event_codec::post::admission::RadrootsAdmittedRootPostEvent",
            "radroots_event_codec::post::admission::RadrootsPostAdmissionError",
            "radroots_event_codec::post::admission::RadrootsPostAdmissionOutcome",
            "radroots_event_codec::post::admission::RadrootsThreadExcludedPostCandidate",
            "radroots_event_codec::post::inbound::RadrootsInboundPostProjection",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.post.verify_and_admit_event.valid",
            "social.post.verify_and_admit_event.invalid",
        ],
    },
];
const POST_OPERATION_KEY_PREFIXES: [&str; 5] = [
    "social_update_",
    "social_photo_update_",
    "social_ask_",
    "social_reply_",
    "social_post_",
];
const POST_OPERATION_ID_PREFIXES: [&str; 5] = [
    "social.update.",
    "social.photo_update.",
    "social.ask.",
    "social.reply.",
    "social.post.",
];
const POST_VECTOR_EXPECTATIONS: [(&str, &str); 64] = [
    (
        "authored_update_wire",
        "social.update.build_authored_draft.valid",
    ),
    (
        "authored_update_blank",
        "social.update.build_authored_draft.invalid",
    ),
    (
        "authored_photo_update_wire",
        "social.photo_update.build_authored_draft.valid",
    ),
    (
        "authored_photo_update_mime_underscore",
        "social.photo_update.build_authored_draft.invalid",
    ),
    ("authored_ask_wire", "social.ask.build_authored_draft.valid"),
    (
        "authored_ask_blank",
        "social.ask.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_direct_wire",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_nip10_nested_wire",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_nip10_canonical_ipv6_relay",
        "social.reply.build_authored_draft.valid",
    ),
    (
        "authored_nip10_ambiguous_parent",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_event_id",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_percent_host",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_bad_percent_host",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_ipv4_overflow",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_ipvfuture",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_empty_port",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_zero_port",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "authored_nip10_invalid_relay_port_overflow",
        "social.reply.build_authored_draft.invalid",
    ),
    (
        "project_signed_nip10_marked_direct",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_with_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_with_malformed_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_marked_nested_reordered",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_direct",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_direct_with_author_hint",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_many",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_many_with_author_hints",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_positional_malformed_middle_citation",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_precedes_ask_and_media",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_relay",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_canonical_relay_authorities",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_malformed_relay_authorities",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_ambiguous_same_reference",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_missing_author",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_event_id",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_author_hint_mismatch",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_blank_content",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_author_hint_tolerated",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_invalid_participant_tolerated",
        "social.reply.project_verified_event.valid",
    ),
    (
        "project_signed_nip10_lone_reply_marker",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "project_signed_nip10_unknown_marker",
        "social.reply.project_verified_event.invalid",
    ),
    (
        "admit_signed_nip10_marked_direct",
        "social.reply.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_nip10_positional_direct",
        "social.reply.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_nip10_invalid_signature",
        "social.reply.verify_and_admit_event.invalid",
    ),
    (
        "project_signed_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_empty_inbound_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_structural_photo",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_photo_preserves_fallbacks_and_unknown_fields",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_normalized_ask_precedes_malformed_media",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_malformed_imeta_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_duplicate_singleton_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_mixed_imeta_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_thread_candidate_precedes_ask_and_media",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_malformed_ask_marker_is_update",
        "social.post.project_verified_event.valid",
    ),
    (
        "project_signed_duplicate_normalized_ask_marker",
        "social.post.project_verified_event.invalid",
    ),
    (
        "project_signed_kind_20_is_not_photo_update",
        "social.post.project_verified_event.invalid",
    ),
    (
        "admit_signed_update",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_structural_photo",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_normalized_ask_precedes_malformed_media",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_thread_candidate_precedes_ask_and_media",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_empty_e_thread_candidate",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_empty_e_value_thread_candidate",
        "social.post.verify_and_admit_event.valid",
    ),
    (
        "admit_signed_duplicate_normalized_ask_marker",
        "social.post.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_kind_20_is_not_photo_update",
        "social.post.verify_and_admit_event.invalid",
    ),
    (
        "admit_signed_invalid_signature",
        "social.post.verify_and_admit_event.invalid",
    ),
];
const FOOD_AVAILABILITY_OPERATION_EXPECTATIONS: [FoodAvailabilityOperationExpectation; 4] = [
    FoodAvailabilityOperationExpectation {
        key: "food_availability_build_authored_draft",
        id: "food_availability.build_authored_draft",
        inputs: &["FoodAvailabilityDetails", "u64"],
        outputs: &["Nip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/food_availability.rs",
            "crates/event_codec/src/food_availability/authored.rs",
        ],
        rust_types: &[
            "radroots_blossom::ByteVerifiedDescriptor",
            "radroots_event::food::availability::FoodAvailabilityDetails",
            "radroots_event::food::availability::FoodAvailabilityDetailsParts",
            "radroots_event::food::availability::FoodAvailabilityError",
            "radroots_event::food::availability::FoodAvailabilityImage",
            "radroots_event::media::AuthoredImage",
            "radroots_event::wire::Nip01EventWireParts",
            "radroots_event_codec::food_availability::authored::RadrootsFoodAvailabilityEncodeError",
        ],
        case_kinds: &[
            "food_availability.build_authored_draft.valid",
            "food_availability.build_authored_draft.invalid",
        ],
    },
    FoodAvailabilityOperationExpectation {
        key: "food_availability_project_verified_event",
        id: "food_availability.project_verified_event",
        inputs: &["RadrootsSignatureVerifiedEvent"],
        outputs: &["RadrootsFoodAvailabilityProjectionOutcome"],
        error_class: "parse_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/classified_listing.rs",
            "crates/event/src/food_availability.rs",
            "crates/event_codec/src/food_availability/inbound.rs",
        ],
        rust_types: &[
            "radroots_event::listing::classified::ClassifiedListingPartition",
            "radroots_event::food::availability::FoodAvailabilityError",
            "radroots_event_codec::food_availability::inbound::RadrootsFoodAvailabilityImageDiagnostic",
            "radroots_event_codec::food_availability::inbound::RadrootsFoodAvailabilityProjectionError",
            "radroots_event_codec::food_availability::inbound::RadrootsFoodAvailabilityProjectionOutcome",
            "radroots_event_codec::food_availability::inbound::RadrootsInboundFoodAvailabilityImage",
            "radroots_event_codec::food_availability::inbound::RadrootsInboundFoodAvailabilityProjection",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "food_availability.project_verified_event.valid",
            "food_availability.project_verified_event.invalid",
        ],
    },
    FoodAvailabilityOperationExpectation {
        key: "food_availability_verify_and_admit_event",
        id: "food_availability.verify_and_admit_event",
        inputs: &["EventEnvelope"],
        outputs: &["RadrootsFoodAvailabilityAdmissionOutcome"],
        error_class: "admission_error",
        signing: "nip01",
        rust_modules: &[
            "crates/event_codec/src/food_availability/admission.rs",
            "crates/event_codec/src/food_availability/inbound.rs",
            "crates/event_codec/src/verification.rs",
        ],
        rust_types: &[
            "radroots_event::envelope::EventEnvelope",
            "radroots_event_codec::food_availability::admission::RadrootsAdmittedFoodAvailabilityEvent",
            "radroots_event_codec::food_availability::admission::RadrootsExcludedClassifiedListingCandidate",
            "radroots_event_codec::food_availability::admission::RadrootsFoodAvailabilityAdmissionError",
            "radroots_event_codec::food_availability::admission::RadrootsFoodAvailabilityAdmissionOutcome",
            "radroots_event_codec::food_availability::inbound::RadrootsInboundFoodAvailabilityProjection",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "food_availability.verify_and_admit_event.valid",
            "food_availability.verify_and_admit_event.invalid",
        ],
    },
    FoodAvailabilityOperationExpectation {
        key: "food_availability_validate_revision",
        id: "food_availability.validate_revision",
        inputs: &["RadrootsSignatureVerifiedEvent"],
        outputs: &["Unit"],
        error_class: "validation_error",
        signing: "none",
        rust_modules: &[
            "crates/event_codec/src/food_availability/inbound.rs",
            "crates/event_codec/src/food_availability/revision.rs",
        ],
        rust_types: &[
            "radroots_event_codec::food_availability::inbound::RadrootsFoodAvailabilityProjectionError",
            "radroots_event_codec::food_availability::revision::RadrootsFoodAvailabilityRevisionError",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "food_availability.validate_revision.valid",
            "food_availability.validate_revision.invalid",
        ],
    },
];
const FOOD_AVAILABILITY_CASE_KINDS: [&str; 8] = [
    "food_availability.build_authored_draft.valid",
    "food_availability.build_authored_draft.invalid",
    "food_availability.project_verified_event.valid",
    "food_availability.project_verified_event.invalid",
    "food_availability.verify_and_admit_event.valid",
    "food_availability.verify_and_admit_event.invalid",
    "food_availability.validate_revision.valid",
    "food_availability.validate_revision.invalid",
];
const FOOD_AVAILABILITY_VECTOR_EXPECTATIONS: [(&str, &str); 40] = [
    (
        "food_authored_unit_g_001",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_kg_002",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_lb_003",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_oz_004",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_each_005",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_dozen_006",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_bunch_007",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_punnet_008",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_bag_009",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_unit_basket_010",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_wire_budget_ascii_max_011",
        "food_availability.build_authored_draft.valid",
    ),
    (
        "food_authored_wire_budget_escaped_overflow_012",
        "food_availability.build_authored_draft.invalid",
    ),
    (
        "food_authored_future_published_at_013",
        "food_availability.build_authored_draft.invalid",
    ),
    (
        "food_admission_normalizes_decimal_currency_014",
        "food_availability.project_verified_event.valid",
    ),
    (
        "food_admission_optional_standard_tags_015",
        "food_availability.verify_and_admit_event.valid",
    ),
    (
        "food_admission_excludes_operational_before_validation_016",
        "food_availability.project_verified_event.valid",
    ),
    (
        "food_admission_excludes_generic_nip99_017",
        "food_availability.project_verified_event.valid",
    ),
    (
        "food_admission_rejects_ambiguous_markers_018",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_wrong_kind_019",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_core_tag_shape_020",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_prohibited_delivery_021",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_price_frequency_022",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_requires_price_unit_023",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_bounds_raw_decimal_digits_024",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_malformed_price_unit_025",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_quantity_unit_mismatch_026",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_status_027",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_rejects_future_published_at_028",
        "food_availability.project_verified_event.invalid",
    ),
    (
        "food_admission_preserves_ordered_image_diagnostics_029",
        "food_availability.project_verified_event.valid",
    ),
    (
        "food_admission_bounds_image_projection_030",
        "food_availability.project_verified_event.valid",
    ),
    (
        "food_admission_rejects_invalid_signature_031",
        "food_availability.verify_and_admit_event.invalid",
    ),
    (
        "food_revision_accepts_later_created_at_032",
        "food_availability.validate_revision.valid",
    ),
    (
        "food_revision_rejects_invalid_previous_033",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_rejects_invalid_current_034",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_rejects_identifier_coordinate_change_035",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_rejects_author_coordinate_change_036",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_rejects_published_at_change_037",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_rejects_older_created_at_038",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_equal_time_a_current_039",
        "food_availability.validate_revision.invalid",
    ),
    (
        "food_revision_equal_time_b_current_040",
        "food_availability.validate_revision.valid",
    ),
];
const EVENT_BOUNDARY_MATRIX_RELATIVES: [&str; 2] = [
    "contracts/event_boundary_matrix.md",
    "docs/platform/canonical/open_source/radroots_v1_spec/02_public_contract_and_runtime/08_event_boundary_matrix.md",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractManifest {
    pub contract: ManifestContract,
    pub surface: Surface,
    pub policy: Policy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestContract {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    pub model_crates: Vec<String>,
    pub algorithm_crates: Vec<String>,
    pub rust_crate_tiers: Option<RustCrateTiers>,
    pub internal_replica_crates: Option<InternalReplicaCrates>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustCrateTiers {
    pub advanced_substrate: Vec<String>,
    pub published_support: Vec<String>,
    pub deferred_publication: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InternalReplicaCrates {
    pub schema: String,
    pub storage: String,
    pub sync: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub exclude_internal_workspace_crates: bool,
    pub require_reproducible_exports: bool,
    pub require_conformance_vectors: bool,
    pub replica: Option<ReplicaPolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaPolicy {
    pub forbid_legacy_alias_identifiers: bool,
    pub require_transport_agnostic_sync_contract: bool,
    pub require_deterministic_emit_ingest: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaContractManifest {
    pub schema_version: u32,
    pub contract: ReplicaContractMetadata,
    pub crate_family: ReplicaContractCrateFamily,
    pub policy: ReplicaContractPolicy,
    pub transfer: ReplicaTransferContract,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaContractMetadata {
    pub name: String,
    pub version: String,
    pub purpose: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaContractCrateFamily {
    pub schema: String,
    pub storage: String,
    pub sync: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaContractPolicy {
    pub transport_agnostic_sync_core: bool,
    pub deterministic_emit_and_ingest: bool,
    pub forbid_legacy_alias_identifiers: bool,
    pub profile_event_emission: String,
    pub unknown_sync_request_fields: String,
    pub classified_listing_signature_verification: String,
    pub classified_listing_head_selection: String,
    pub classified_listing_operational_projection: String,
    pub classified_listing_excluded_or_rejected_head: String,
    pub classified_listing_head_only_ingest: String,
    pub legacy_bare_envelope_ingest: String,
    pub legacy_ingest_feature: String,
    pub phase_1_ingest_replacement: String,
    pub future_product_ingest_input: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplicaTransferContract {
    pub version: u32,
    pub source: String,
    pub constant: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationsContractManifest {
    pub contract: ManifestContract,
    pub public: PublicContract,
    pub shared_types: SharedTypesContract,
    pub errors: ErrorClassesContract,
    pub operations: BTreeMap<String, PublicOperationContract>,
    pub implementation_provenance: Option<ImplementationProvenance>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicContract {
    pub domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedTypesContract {
    pub public: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorClassesContract {
    pub classes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementationProvenance {
    pub model_crates: Vec<String>,
    pub algorithm_crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicOperationContract {
    pub domain: String,
    pub id: String,
    pub stability: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub error_class: String,
    #[allow(dead_code)]
    pub deterministic: bool,
    pub signing: String,
    pub transport: String,
    pub implementation: PublicOperationImplementation,
    pub conformance: PublicOperationConformance,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicOperationImplementation {
    pub rust_modules: Vec<String>,
    pub rust_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicOperationConformance {
    pub vector: String,
    #[serde(default)]
    pub case_kinds: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeletionConformanceRawEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[derive(Clone, Copy)]
struct CalendarOperationExpectation {
    key: &'static str,
    id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    rust_modules: &'static [&'static str],
    rust_types: &'static [&'static str],
    vector: &'static str,
}

#[derive(Clone, Copy)]
struct PostOperationExpectation {
    key: &'static str,
    id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    signing: &'static str,
    rust_modules: &'static [&'static str],
    rust_types: &'static [&'static str],
    case_kinds: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct CommentOperationExpectation {
    key: &'static str,
    id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    signing: &'static str,
    rust_modules: &'static [&'static str],
    rust_types: &'static [&'static str],
    case_kinds: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct DeletionOperationExpectation {
    key: &'static str,
    id: &'static str,
    vector: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    signing: &'static str,
    rust_modules: &'static [&'static str],
    rust_types: &'static [&'static str],
    case_kinds: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct FoodAvailabilityOperationExpectation {
    key: &'static str,
    id: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    error_class: &'static str,
    signing: &'static str,
    rust_modules: &'static [&'static str],
    rust_types: &'static [&'static str],
    case_kinds: &'static [&'static str],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionPolicy {
    pub contract: VersionContract,
    pub semver: SemverRules,
    pub release_integrity: ReleaseIntegrityRules,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionContract {
    pub version: String,
    pub stability: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemverRules {
    pub major_on: Vec<String>,
    pub minor_on: Vec<String>,
    pub patch_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseIntegrityRules {
    pub requires_conformance_pass: bool,
    pub requires_contract_manifest_diff: bool,
    pub requires_release_notes: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRecord {
    schema_version: u32,
    release: ReleaseRecordMetadata,
    artifacts: ReleaseRecordArtifacts,
    changes: Vec<ReleaseRecordChange>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRecordMetadata {
    version: String,
    previous_version: String,
    contract_base_version: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRecordArtifacts {
    changelog: String,
    manifest: String,
    operations: String,
    replica: String,
    conformance: String,
    publish_policy: String,
    #[serde(default)]
    sqlite_runtime: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SqliteRuntimeContract {
    schema_version: u32,
    package: SqliteRuntimePackage,
    activation: SqliteRuntimeActivation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SqliteRuntimePackage {
    name: String,
    version: String,
    source: String,
    checksum: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SqliteRuntimeActivation {
    route: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRecordChange {
    id: String,
    classification: String,
    semver_impacts: Vec<String>,
    summary: String,
}

#[derive(Debug)]
pub struct ContractBundle {
    pub root: PathBuf,
    pub manifest: ContractManifest,
    pub version: VersionPolicy,
    pub replica: ReplicaContractManifest,
    pub operations_manifest: OperationsContractManifest,
}

#[derive(Debug, Deserialize)]
struct WorkspaceCargoManifest {
    workspace: WorkspaceSection,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSection {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceVersionCargoManifest {
    workspace: WorkspaceVersionSection,
}

#[derive(Debug, Deserialize)]
struct WorkspaceVersionSection {
    members: Vec<String>,
    package: WorkspacePackageVersion,
    dependencies: BTreeMap<String, WorkspaceDependencyVersion>,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackageVersion {
    version: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceDependencyVersion {
    path: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VersionedPackageCargoManifest {
    package: VersionedPackageSection,
}

#[derive(Debug, Deserialize)]
struct VersionedPackageSection {
    name: String,
    version: PackageVersionSource,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PackageVersionSource {
    Literal(String),
    Workspace { workspace: bool },
}

#[derive(Debug, Deserialize)]
struct CargoLockManifest {
    package: Vec<CargoLockPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PackageCargoManifest {
    package: PackageSection,
}

#[derive(Debug, Deserialize)]
struct PackageSection {
    name: String,
    publish: Option<PackagePublish>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum PackagePublish {
    Bool(bool),
    Registries(Vec<String>),
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredFile {
    required: CoverageRequiredSection,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Deserialize)]
struct CoverageRequiredSection {
    crates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EventBoundaryRow {
    domain: String,
    kind: String,
    radroots_type: String,
    rpc_methods: BTreeSet<String>,
}

#[derive(Clone, Copy)]
struct EventBoundarySourceWitness {
    relative_path: &'static str,
    required_fragments: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct EventBoundaryExpectation {
    domain: &'static str,
    kind: &'static str,
    radroots_type: &'static str,
    rpc_methods: &'static [&'static str],
    witnesses: &'static [EventBoundarySourceWitness],
}

const PROFILE_WITNESSES: [EventBoundarySourceWitness; 5] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/profile.rs",
        required_fragments: &["pub struct AuthoredProfile"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/profile/inbound/registry_v7.rs",
        required_fragments: &["pub struct RadrootsInboundProfileMetadata"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/profile/admission.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedProfileEvent",
            "pub fn verify_and_admit_profile_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/verification/v1.rs",
        required_fragments: &[
            "pub struct RadrootsSignatureVerifiedEvent",
            "pub fn verify_nip01_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_PROFILE: u32 = 0;"],
    },
];

const FOLLOW_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/follow.rs",
        required_fragments: &["pub struct Follow"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_FOLLOW: u32 = 3;"],
    },
];

const POST_WITNESSES: [EventBoundarySourceWitness; 5] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/post.rs",
        required_fragments: &[
            "pub struct Post",
            "pub struct AuthoredUpdate",
            "pub struct AuthoredPhotoUpdate",
            "pub struct AuthoredAsk",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/post/authored.rs",
        required_fragments: &[
            "pub fn authored_update_to_wire_parts",
            "pub fn authored_photo_update_to_wire_parts",
            "pub fn authored_ask_to_wire_parts",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/post/inbound/registry_v7.rs",
        required_fragments: &[
            "pub struct RadrootsInboundPostProjection",
            "pub fn project_verified_post_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/post/admission.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedRootPostEvent",
            "pub struct RadrootsThreadExcludedPostCandidate",
            "pub enum RadrootsPostAdmissionOutcome",
            "pub fn verify_and_admit_post_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_POST: u32 = 1;"],
    },
];

const REPLY_WITNESSES: [EventBoundarySourceWitness; 5] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/relay_hint.rs",
        required_fragments: &["pub struct NostrRelayHint"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/reply.rs",
        required_fragments: &[
            "pub struct Nip10ReplyReference",
            "pub struct AuthoredNip10Reply",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/reply/inbound/registry_v7.rs",
        required_fragments: &[
            "pub struct RadrootsInboundNip10ReplyProjection",
            "pub fn project_verified_nip10_reply_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/reply/admission.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedNip10ReplyEvent",
            "pub fn verify_and_admit_nip10_reply_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/events/reply.rs",
        required_fragments: &[
            "pub struct Nip10ReplyBuilder",
            "pub fn build_nip10_reply_event",
        ],
    },
];

const COMMENT_WITNESSES: [EventBoundarySourceWitness; 7] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/relay_hint.rs",
        required_fragments: &["pub struct NostrRelayHint"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/comment.rs",
        required_fragments: &[
            "pub enum Nip22CommentRoot",
            "pub enum Nip22CommentPosition",
            "pub struct AuthoredNip22Comment",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/comment/authored.rs",
        required_fragments: &["pub fn authored_nip22_comment_to_wire_parts"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/comment/inbound/registry_v7.rs",
        required_fragments: &[
            "pub struct RadrootsInboundNip22CommentProjection",
            "pub fn project_verified_nip22_comment_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/comment/admission.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedNip22CommentEvent",
            "pub fn verify_and_admit_nip22_comment_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/events/comment.rs",
        required_fragments: &[
            "pub struct Nip22CommentBuilder",
            "pub fn build_nip22_comment_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_COMMENT: u32 = 1111;"],
    },
];

const DELETION_WITNESSES: [EventBoundarySourceWitness; 7] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/deletion.rs",
        required_fragments: &["pub struct AuthoredNip09DeletionRequest"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/deletion/authored.rs",
        required_fragments: &["pub fn authored_nip09_deletion_request_to_wire_parts"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        required_fragments: &[
            "pub struct RadrootsInboundNip09DeletionProjection",
            "pub fn project_verified_nip09_deletion_request_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedNip09DeletionRequestEvent",
            "pub fn verify_and_admit_nip09_deletion_request_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/deletion/reconciliation_v1.rs",
        required_fragments: &[
            "pub enum RadrootsNip09SuppressionOutcome",
            "pub enum RadrootsNip09SuppressionReason",
            "pub struct RadrootsNip09EventReferenceEvidence",
            "pub struct RadrootsNip09AddressReferenceEvidence",
            "pub struct RadrootsNip09SuppressionDecision",
            "pub fn evaluate_nip09_suppression",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/events/deletion.rs",
        required_fragments: &[
            "pub struct Nip09DeletionRequestBuilder",
            "pub fn build_nip09_deletion_request_event",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_DELETION_REQUEST: u32 = 5;"],
    },
];

const REACTION_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/reaction.rs",
        required_fragments: &["pub struct Reaction"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_REACTION: u32 = 7;"],
    },
];

const REPOST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/repost.rs",
        required_fragments: &["pub struct Repost"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_REPOST: u32 = 6;"],
    },
];

const GENERIC_REPOST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/repost.rs",
        required_fragments: &["pub struct GenericRepost"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_GENERIC_REPOST: u32 = 16;"],
    },
];

const SEAL_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/seal.rs",
        required_fragments: &["pub struct Seal"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_SEAL: u32 = 13;"],
    },
];

const MESSAGE_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/message.rs",
        required_fragments: &["pub struct Message"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_MESSAGE: u32 = 14;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &["pub async fn wrap_message<T>(", "KIND_MESSAGE =>"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["pub mod nip17;"],
    },
];

const MESSAGE_FILE_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/message_file.rs",
        required_fragments: &["pub struct MessageFile"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_MESSAGE_FILE: u32 = 15;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &["pub async fn wrap_message_file<T>(", "KIND_MESSAGE_FILE =>"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["pub mod nip17;"],
    },
];

const GIFT_WRAP_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/gift_wrap.rs",
        required_fragments: &["pub struct GiftWrap"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_GIFT_WRAP: u32 = 1059;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/nip17.rs",
        required_fragments: &["pub async fn unwrap_gift_wrap<T>("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/lib.rs",
        required_fragments: &["pub mod nip17;"],
    },
];

const PUBLIC_FILE_METADATA_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/file_metadata.rs",
        required_fragments: &["pub struct FileMetadata"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_PUBLIC_FILE_METADATA: u32 = KIND_FILE_METADATA;"],
    },
];

const REPORT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/report.rs",
        required_fragments: &["pub struct Report"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_REPORT: u32 = 1984;"],
    },
];

const LIST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/list.rs",
        required_fragments: &["pub struct List"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_LIST_MUTE: u32 = 10000;",
            "pub const KIND_LIST_GOOD_WIKI_RELAYS: u32 = 10102;",
        ],
    },
];

const RELAY_LIST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/list.rs",
        required_fragments: &["pub struct List"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_LIST_READ_WRITE_RELAYS: u32 = 10002;"],
    },
];

const LIST_SET_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/list_set.rs",
        required_fragments: &["pub struct ListSet"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_LIST_SET_FOLLOW: u32 = 30000;",
            "pub const KIND_LIST_SET_MEDIA_STARTER_PACK: u32 = 39092;",
        ],
    },
];

const ARTICLE_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/article.rs",
        required_fragments: &["pub struct Article"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_ARTICLE: u32 = 30023;"],
    },
];

const KNOWLEDGE_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/knowledge.rs",
        required_fragments: &[
            "pub struct WikiArticle",
            "pub struct KnowledgeClaim",
            "pub struct KnowledgeFieldReport",
            "pub struct EvidenceBounty",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_WIKI_MERGE_REQUEST: u32 = 818;",
            "pub const KIND_KNOWLEDGE_CLAIM: u32 = 3460;",
            "pub const KIND_KNOWLEDGE_SOURCE: u32 = 30450;",
            "pub const KIND_WIKI_ARTICLE: u32 = 30818;",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/contract/registry_v7.rs",
        required_fragments: &[
            "Reducer::KnowledgeProjection",
            "\"radroots.wiki.article.v1\"",
            "\"radroots.knowledge.claim.v1\"",
            "pub fn validate_event_contract_shape",
        ],
    },
];

const APP_DATA_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/app_data.rs",
        required_fragments: &["pub struct AppData"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_APP_DATA: u32 = 30078;"],
    },
];

const APP_HANDLER_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_APPLICATION_HANDLER: u32 = 31990;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/nostr/src/events/application_handler.rs",
        required_fragments: &["pub fn build_application_handler_event("],
    },
];

const CALENDAR_DATE_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/calendar.rs",
        required_fragments: &[
            "pub struct AuthoredCalendarDateEvent",
            "pub struct ParsedNip52CalendarDateEvent",
            "pub struct AdmittedCalendarDateEvent",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/encode.rs",
        required_fragments: &["pub fn date_to_wire_parts("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/decode.rs",
        required_fragments: &[
            "pub fn parse_nip52_calendar_date_event(",
            "pub fn admit_radroots_calendar_date_event(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CALENDAR_DATE_EVENT: u32 = 31922;"],
    },
];

const CALENDAR_TIME_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/calendar.rs",
        required_fragments: &[
            "pub struct AuthoredCalendarTimeEvent",
            "pub struct ParsedNip52CalendarTimeEvent",
            "pub struct AdmittedCalendarTimeEvent",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/encode.rs",
        required_fragments: &["pub fn time_to_wire_parts("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/decode.rs",
        required_fragments: &[
            "pub fn parse_nip52_calendar_time_event(",
            "pub fn admit_radroots_calendar_time_event(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CALENDAR_TIME_EVENT: u32 = 31923;"],
    },
];

const CALENDAR_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/calendar.rs",
        required_fragments: &[
            "pub struct AuthoredCalendar {",
            "pub struct ParsedNip52Calendar {",
            "pub struct AdmittedCalendar {",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/encode.rs",
        required_fragments: &["pub fn calendar_to_wire_parts("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/decode.rs",
        required_fragments: &[
            "pub fn parse_nip52_calendar(",
            "pub fn admit_radroots_calendar(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CALENDAR: u32 = KIND_LIST_SET_CALENDAR;"],
    },
];

const CALENDAR_RSVP_WITNESSES: [EventBoundarySourceWitness; 4] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/calendar.rs",
        required_fragments: &[
            "pub struct AuthoredCalendarEventRsvp {",
            "pub struct ParsedNip52CalendarEventRsvp {",
            "pub struct AdmittedCalendarEventRsvp {",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/encode.rs",
        required_fragments: &["pub fn rsvp_to_wire_parts("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/calendar/decode.rs",
        required_fragments: &[
            "pub fn parse_nip52_calendar_event_rsvp(",
            "pub fn admit_radroots_calendar_event_rsvp(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CALENDAR_EVENT_RSVP: u32 = 31925;"],
    },
];

const FARM_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/farm.rs",
        required_fragments: &["pub struct Farm"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_FARM: u32 = 30340;"],
    },
];

const PLOT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/plot.rs",
        required_fragments: &["pub struct Plot"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_PLOT: u32 = 30350;"],
    },
];

const COOP_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/coop.rs",
        required_fragments: &["pub struct Coop"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_COOP: u32 = 30360;"],
    },
];

const DOCUMENT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/document.rs",
        required_fragments: &["pub struct Document"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_DOCUMENT: u32 = 30361;"],
    },
];

const RESOURCE_AREA_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/resource_area.rs",
        required_fragments: &["pub struct ResourceArea"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_RESOURCE_AREA: u32 = 30370;"],
    },
];

const RESOURCE_CAP_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/resource_cap.rs",
        required_fragments: &["pub struct ResourceHarvestCap"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_RESOURCE_HARVEST_CAP: u32 = 30371;"],
    },
];

const OPERATIONAL_LISTING_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/operational_listing.rs",
        required_fragments: &["pub struct OperationalListing"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CLASSIFIED_LISTING: u32 = 30402;"],
    },
];

const FOOD_AVAILABILITY_WITNESSES: [EventBoundarySourceWitness; 6] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/food_availability.rs",
        required_fragments: &[
            "pub const RADROOTS_FOOD_AVAILABILITY_CONTRACT_ID: &str",
            "pub struct FoodAvailabilityDetails",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_CLASSIFIED_LISTING: u32 = 30402;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/food_availability/authored.rs",
        required_fragments: &["pub fn authored_food_availability_to_wire_parts("],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/food_availability/inbound/registry_v7.rs",
        required_fragments: &[
            "pub struct RadrootsInboundFoodAvailabilityProjection",
            "pub fn project_verified_food_availability_event(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/food_availability/admission.rs",
        required_fragments: &[
            "pub struct RadrootsAdmittedFoodAvailabilityEvent",
            "pub fn verify_and_admit_food_availability_event(",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/food_availability/revision.rs",
        required_fragments: &["pub fn validate_food_availability_revision("],
    },
];

const DVM_REQUEST_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/job_request.rs",
        required_fragments: &["pub struct JobRequest"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_JOB_REQUEST_MIN: u32 = 5000;",
            "pub const KIND_JOB_REQUEST_MAX: u32 = 5999;",
        ],
    },
];

const DVM_RESULT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/job_result.rs",
        required_fragments: &["pub struct JobResult"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &[
            "pub const KIND_JOB_RESULT_MIN: u32 = 6000;",
            "pub const KIND_JOB_RESULT_MAX: u32 = 6999;",
        ],
    },
];

const DVM_FEEDBACK_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/job_feedback.rs",
        required_fragments: &["pub struct JobFeedback"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_JOB_FEEDBACK: u32 = 7000;"],
    },
];

const TRADE_PROPOSAL_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_PROPOSAL: u32 = 3470;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/trade.rs",
        required_fragments: &[
            "pub const RADROOTS_TRADE_PROPOSAL_CONTRACT_ID: &str",
            "Self::Proposal => KIND_TRADE_PROPOSAL",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/trade/mod.rs",
        required_fragments: &[
            "pub fn trade_mutation_event_build",
            "pub fn trade_mutation_from_event",
        ],
    },
];

const TRADE_DECISION_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_DECISION: u32 = 3471;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/trade.rs",
        required_fragments: &[
            "pub const RADROOTS_TRADE_DECISION_CONTRACT_ID: &str",
            "Self::Decision => KIND_TRADE_DECISION",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/trade/mod.rs",
        required_fragments: &[
            "pub fn trade_mutation_event_build",
            "pub fn trade_mutation_from_event",
        ],
    },
];

const TRADE_REVISION_PROPOSAL_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_REVISION_PROPOSAL: u32 = 3472;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/trade.rs",
        required_fragments: &[
            "pub const RADROOTS_TRADE_REVISION_PROPOSAL_CONTRACT_ID: &str",
            "Self::RevisionProposal => KIND_TRADE_REVISION_PROPOSAL",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/trade/mod.rs",
        required_fragments: &[
            "pub fn trade_mutation_event_build",
            "pub fn trade_mutation_from_event",
        ],
    },
];

const TRADE_REVISION_DECISION_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_REVISION_DECISION: u32 = 3473;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/trade.rs",
        required_fragments: &[
            "pub const RADROOTS_TRADE_REVISION_DECISION_CONTRACT_ID: &str",
            "Self::RevisionDecision => KIND_TRADE_REVISION_DECISION",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/trade/mod.rs",
        required_fragments: &[
            "pub fn trade_mutation_event_build",
            "pub fn trade_mutation_from_event",
        ],
    },
];

const TRADE_CANCELLATION_WITNESSES: [EventBoundarySourceWitness; 3] = [
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_CANCELLATION: u32 = 3474;"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/trade.rs",
        required_fragments: &[
            "pub const RADROOTS_TRADE_CANCELLATION_CONTRACT_ID: &str",
            "Self::Cancellation => KIND_TRADE_CANCELLATION",
        ],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event_codec/src/trade/mod.rs",
        required_fragments: &[
            "pub fn trade_mutation_event_build",
            "pub fn trade_mutation_from_event",
        ],
    },
];

const TRADE_VALIDATION_RECEIPT_WITNESSES: [EventBoundarySourceWitness; 2] = [
    EventBoundarySourceWitness {
        relative_path: "crates/trade/src/validation_receipt.rs",
        required_fragments: &["pub struct RadrootsTradeValidationReceipt"],
    },
    EventBoundarySourceWitness {
        relative_path: "crates/event/src/kinds.rs",
        required_fragments: &["pub const KIND_TRADE_VALIDATION_RECEIPT: u32 = 3440;"],
    },
];

const RELAY_DOC_WITNESSES: [EventBoundarySourceWitness; 1] = [EventBoundarySourceWitness {
    relative_path: "crates/event/src/relay_document.rs",
    required_fragments: &["pub struct RelayDocument"],
}];

const CANONICAL_EVENT_BOUNDARY_EXPECTATIONS: [EventBoundaryExpectation; 44] = [
    EventBoundaryExpectation {
        domain: "profile",
        kind: "0",
        radroots_type: "AuthoredProfile / RadrootsInboundProfileMetadata",
        rpc_methods: &[
            "events.profile.publish",
            "events.profile.list",
            "events.profile.get",
        ],
        witnesses: &PROFILE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "follow",
        kind: "3",
        radroots_type: "Follow",
        rpc_methods: &[
            "events.follow.publish",
            "events.follow.list",
            "events.follow.get",
        ],
        witnesses: &FOLLOW_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "post",
        kind: "1",
        radroots_type: "AuthoredUpdate / AuthoredPhotoUpdate / AuthoredAsk / RadrootsInboundPostProjection",
        rpc_methods: &["events.post.publish", "events.post.list", "events.post.get"],
        witnesses: &POST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "reply",
        kind: "1",
        radroots_type: "AuthoredNip10Reply / RadrootsInboundNip10ReplyProjection / RadrootsAdmittedNip10ReplyEvent / Nip10ReplyBuilder",
        rpc_methods: &[
            "social.reply.build_authored_draft",
            "social.reply.project_verified_event",
            "social.reply.verify_and_admit_event",
        ],
        witnesses: &REPLY_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "comment",
        kind: "1111",
        radroots_type: "AuthoredNip22Comment / RadrootsInboundNip22CommentProjection / RadrootsAdmittedNip22CommentEvent / Nip22CommentBuilder",
        rpc_methods: &[
            "social.comment.build_authored_draft",
            "social.comment.project_verified_event",
            "social.comment.verify_and_admit_event",
        ],
        witnesses: &COMMENT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "deletion_request",
        kind: "5",
        radroots_type: "AuthoredNip09DeletionRequest / RadrootsInboundNip09DeletionProjection / RadrootsAdmittedNip09DeletionRequestEvent / RadrootsNip09SuppressionDecision / Nip09DeletionRequestBuilder",
        rpc_methods: &[
            "social.deletion_request.build_authored_draft",
            "social.deletion_request.project_verified_event",
            "social.deletion_request.verify_and_admit_event",
            "social.deletion_request.evaluate_suppression",
        ],
        witnesses: &DELETION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "reaction",
        kind: "7",
        radroots_type: "Reaction",
        rpc_methods: &[
            "events.reaction.publish",
            "events.reaction.list",
            "events.reaction.get",
        ],
        witnesses: &REACTION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "repost",
        kind: "6",
        radroots_type: "Repost",
        rpc_methods: &[
            "events.repost.publish",
            "events.repost.list",
            "events.repost.get",
        ],
        witnesses: &REPOST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "generic_repost",
        kind: "16",
        radroots_type: "GenericRepost",
        rpc_methods: &[
            "events.generic_repost.publish",
            "events.generic_repost.list",
            "events.generic_repost.get",
        ],
        witnesses: &GENERIC_REPOST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "seal",
        kind: "13",
        radroots_type: "Seal",
        rpc_methods: &["events.seal.encode", "events.seal.decode"],
        witnesses: &SEAL_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "message",
        kind: "14",
        radroots_type: "Message",
        rpc_methods: &[
            "events.message.publish",
            "events.message.list",
            "events.message.get",
        ],
        witnesses: &MESSAGE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "message_file",
        kind: "15",
        radroots_type: "MessageFile",
        rpc_methods: &[
            "events.message_file.publish",
            "events.message_file.list",
            "events.message_file.get",
        ],
        witnesses: &MESSAGE_FILE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "gift_wrap",
        kind: "1059",
        radroots_type: "GiftWrap",
        rpc_methods: &[
            "events.gift_wrap.publish",
            "events.gift_wrap.list",
            "events.gift_wrap.get",
        ],
        witnesses: &GIFT_WRAP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "public_file_metadata",
        kind: "1063",
        radroots_type: "FileMetadata",
        rpc_methods: &[
            "events.public_file_metadata.publish",
            "events.public_file_metadata.list",
            "events.public_file_metadata.get",
        ],
        witnesses: &PUBLIC_FILE_METADATA_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "report",
        kind: "1984",
        radroots_type: "Report",
        rpc_methods: &[
            "events.report.publish",
            "events.report.list",
            "events.report.get",
        ],
        witnesses: &REPORT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "list",
        kind: "10000..10102",
        radroots_type: "List",
        rpc_methods: &["events.list.publish", "events.list.list", "events.list.get"],
        witnesses: &LIST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "relay_list",
        kind: "10002",
        radroots_type: "List",
        rpc_methods: &[
            "events.relay_list.publish",
            "events.relay_list.list",
            "events.relay_list.get",
        ],
        witnesses: &RELAY_LIST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "list_set",
        kind: "30000..30007, 30015, 30030, 30063, 30267, 39089, 39092",
        radroots_type: "ListSet",
        rpc_methods: &[
            "events.list_set.publish",
            "events.list_set.list",
            "events.list_set.get",
        ],
        witnesses: &LIST_SET_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "article",
        kind: "30023",
        radroots_type: "Article",
        rpc_methods: &[
            "events.article.publish",
            "events.article.list",
            "events.article.get",
        ],
        witnesses: &ARTICLE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "knowledge",
        kind: "818, 3460..3465, 30450..30451, 30818..30819",
        radroots_type: "RadrootsKnowledgeEvent",
        rpc_methods: &[
            "events.knowledge.publish",
            "events.knowledge.list",
            "events.knowledge.get",
        ],
        witnesses: &KNOWLEDGE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "app_data",
        kind: "30078",
        radroots_type: "AppData",
        rpc_methods: &[
            "events.app_data.publish",
            "events.app_data.list",
            "events.app_data.get",
        ],
        witnesses: &APP_DATA_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "app_handler",
        kind: "31990",
        radroots_type: "KIND_APPLICATION_HANDLER",
        rpc_methods: &[
            "events.app_handler.publish",
            "events.app_handler.list",
            "events.app_handler.get",
        ],
        witnesses: &APP_HANDLER_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "calendar_date",
        kind: "31922",
        radroots_type: "AuthoredCalendarDateEvent / ParsedNip52CalendarDateEvent / AdmittedCalendarDateEvent",
        rpc_methods: &[
            "events.calendar_date.publish",
            "events.calendar_date.list",
            "events.calendar_date.get",
        ],
        witnesses: &CALENDAR_DATE_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "calendar_time",
        kind: "31923",
        radroots_type: "AuthoredCalendarTimeEvent / ParsedNip52CalendarTimeEvent / AdmittedCalendarTimeEvent",
        rpc_methods: &[
            "events.calendar_time.publish",
            "events.calendar_time.list",
            "events.calendar_time.get",
        ],
        witnesses: &CALENDAR_TIME_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "calendar",
        kind: "31924",
        radroots_type: "AuthoredCalendar / ParsedNip52Calendar / AdmittedCalendar",
        rpc_methods: &[
            "events.calendar.publish",
            "events.calendar.list",
            "events.calendar.get",
        ],
        witnesses: &CALENDAR_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "calendar_rsvp",
        kind: "31925",
        radroots_type: "AuthoredCalendarEventRsvp / ParsedNip52CalendarEventRsvp / AdmittedCalendarEventRsvp",
        rpc_methods: &[
            "events.calendar_rsvp.publish",
            "events.calendar_rsvp.list",
            "events.calendar_rsvp.get",
        ],
        witnesses: &CALENDAR_RSVP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "farm",
        kind: "30340",
        radroots_type: "Farm",
        rpc_methods: &["events.farm.publish", "events.farm.list", "events.farm.get"],
        witnesses: &FARM_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "plot",
        kind: "30350",
        radroots_type: "Plot",
        rpc_methods: &["events.plot.publish", "events.plot.list", "events.plot.get"],
        witnesses: &PLOT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "coop",
        kind: "30360",
        radroots_type: "Coop",
        rpc_methods: &["events.coop.publish", "events.coop.list", "events.coop.get"],
        witnesses: &COOP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "document",
        kind: "30361",
        radroots_type: "Document",
        rpc_methods: &[
            "events.document.publish",
            "events.document.list",
            "events.document.get",
        ],
        witnesses: &DOCUMENT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "resource_area",
        kind: "30370",
        radroots_type: "ResourceArea",
        rpc_methods: &[
            "events.resource_area.publish",
            "events.resource_area.list",
            "events.resource_area.get",
        ],
        witnesses: &RESOURCE_AREA_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "resource_cap",
        kind: "30371",
        radroots_type: "ResourceHarvestCap",
        rpc_methods: &[
            "events.resource_cap.publish",
            "events.resource_cap.list",
            "events.resource_cap.get",
        ],
        witnesses: &RESOURCE_CAP_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "food_availability",
        kind: "30402",
        radroots_type: "FoodAvailabilityDetails / RadrootsInboundFoodAvailabilityProjection / RadrootsAdmittedFoodAvailabilityEvent / FoodAvailabilityBuilder",
        rpc_methods: &[
            "food_availability.build_authored_draft",
            "food_availability.project_verified_event",
            "food_availability.verify_and_admit_event",
            "food_availability.validate_revision",
        ],
        witnesses: &FOOD_AVAILABILITY_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "operational_listing",
        kind: "30402",
        radroots_type: "OperationalListing",
        rpc_methods: &[
            "events.operational_listing.publish",
            "events.operational_listing.list",
            "events.operational_listing.get",
        ],
        witnesses: &OPERATIONAL_LISTING_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_request",
        kind: "5000-5999",
        radroots_type: "JobRequest",
        rpc_methods: &[
            "events.dvm_request.publish",
            "events.dvm_request.list",
            "events.dvm_request.get",
        ],
        witnesses: &DVM_REQUEST_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_result",
        kind: "6000-6999",
        radroots_type: "JobResult",
        rpc_methods: &[
            "events.dvm_result.publish",
            "events.dvm_result.list",
            "events.dvm_result.get",
        ],
        witnesses: &DVM_RESULT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "dvm_feedback",
        kind: "7000",
        radroots_type: "JobFeedback",
        rpc_methods: &[
            "events.dvm_feedback.publish",
            "events.dvm_feedback.list",
            "events.dvm_feedback.get",
        ],
        witnesses: &DVM_FEEDBACK_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:proposal",
        kind: "3470",
        radroots_type: "TradeMutationEnvelopeV1",
        rpc_methods: &[
            "trade.get_trade",
            "trade.list_trades",
            "trade.submit_proposal",
        ],
        witnesses: &TRADE_PROPOSAL_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:decision",
        kind: "3471",
        radroots_type: "TradeMutationEnvelopeV1",
        rpc_methods: &[
            "trade.decide_candidate",
            "trade.get_trade",
            "trade.list_trades",
        ],
        witnesses: &TRADE_DECISION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:revision_proposal",
        kind: "3472",
        radroots_type: "TradeMutationEnvelopeV1",
        rpc_methods: &[
            "trade.get_trade",
            "trade.list_trades",
            "trade.propose_revision",
        ],
        witnesses: &TRADE_REVISION_PROPOSAL_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:revision_decision",
        kind: "3473",
        radroots_type: "TradeMutationEnvelopeV1",
        rpc_methods: &[
            "trade.decide_candidate",
            "trade.get_trade",
            "trade.list_trades",
        ],
        witnesses: &TRADE_REVISION_DECISION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:cancellation",
        kind: "3474",
        radroots_type: "TradeMutationEnvelopeV1",
        rpc_methods: &["trade.cancel_trade", "trade.get_trade", "trade.list_trades"],
        witnesses: &TRADE_CANCELLATION_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "trade:validation_receipt",
        kind: "3440",
        radroots_type: "RadrootsTradeValidationReceipt",
        rpc_methods: &[
            "domains.trade.validation_receipt.get",
            "domains.trade.validation_receipt.list",
            "domains.trade.validation_receipt.verify",
        ],
        witnesses: &TRADE_VALIDATION_RECEIPT_WITNESSES,
    },
    EventBoundaryExpectation {
        domain: "relay_doc",
        kind: "N/A",
        radroots_type: "RelayDocument",
        rpc_methods: &["system.relay_doc.get"],
        witnesses: &RELAY_DOC_WITNESSES,
    },
];

#[derive(Debug, Deserialize)]
struct ReleaseContractFile {
    release: ReleaseSection,
    #[serde(default)]
    publication: Option<PublicationControl>,
    #[serde(default)]
    workspace_classification: Option<WorkspaceReleaseClassification>,
    #[serde(default)]
    classification: ReleaseClassification,
    #[serde(default)]
    publish: Option<ReleaseCrateSet>,
    #[serde(default)]
    internal: Option<ReleaseCrateSet>,
    publish_order: ReleaseCrateSet,
}

#[derive(Debug, Default, Deserialize)]
struct ReleaseClassification {
    #[serde(default)]
    public: Vec<String>,
    #[serde(default)]
    internal: Vec<String>,
    #[serde(default)]
    deferred: Vec<String>,
    #[serde(default)]
    retired: Vec<String>,
    #[serde(default)]
    yank_only: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseSection {
    version: String,
}

#[derive(Debug, Deserialize)]
struct PublicationControl {
    frozen: bool,
    registry: String,
    final_enablement_step: u16,
    #[serde(default)]
    spec_id: String,
    #[serde(default)]
    approved_packages: Vec<String>,
    #[serde(default)]
    local_packages: Vec<String>,
    #[serde(default)]
    external_packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceReleaseClassification {
    #[serde(default)]
    private: Vec<String>,
    #[serde(default)]
    build_codegen: Vec<String>,
    #[serde(default)]
    test_support: Vec<String>,
    #[serde(default)]
    preview: Vec<String>,
    #[serde(default)]
    retired: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseArchitecture {
    spec_id: String,
    package_count: usize,
    repositories: CratesReleaseRepositories,
    package: Vec<CratesReleasePackage>,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseRepositories {
    lib: CratesReleaseRepository,
    sdk: CratesReleaseRepository,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseRepository {
    version: String,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CratesReleasePackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseCrateSet {
    crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceVectorFile {
    suite: String,
    contract_version: String,
    vectors: Vec<ConformanceVectorEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceVectorEntry {
    id: String,
    kind: String,
    input: Value,
    expected: Option<Value>,
    expected_error_contains: Option<String>,
}

impl ConformanceVectorEntry {
    fn expected_value(&self) -> Result<&Value, String> {
        self.expected.as_ref().ok_or_else(|| {
            format!(
                "conformance vector {} does not define an expected output",
                self.id
            )
        })
    }
}

impl ReleaseContractFile {
    fn uses_classification(&self) -> bool {
        !self.classification.public.is_empty()
            || !self.classification.internal.is_empty()
            || !self.classification.deferred.is_empty()
            || !self.classification.retired.is_empty()
            || !self.classification.yank_only.is_empty()
    }

    fn public_crates(&self) -> Vec<String> {
        if let Some(publication) = &self.publication
            && !publication.local_packages.is_empty()
        {
            return publication.local_packages.clone();
        }
        if self.uses_classification() {
            return self.classification.public.clone();
        }
        self.publish
            .as_ref()
            .map(|set| set.crates.clone())
            .unwrap_or_default()
    }

    fn internal_crates(&self) -> Vec<String> {
        if self.uses_classification() {
            return self.classification.internal.clone();
        }
        self.internal
            .as_ref()
            .map(|set| set.crates.clone())
            .unwrap_or_default()
    }

    fn deferred_crates(&self) -> Vec<String> {
        self.classification.deferred.clone()
    }

    fn retired_crates(&self) -> Vec<String> {
        self.classification.retired.clone()
    }

    fn yank_only_crates(&self) -> Vec<String> {
        self.classification.yank_only.clone()
    }
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match toml::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("parse {}: {e}", path.display())),
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match serde_json::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("parse {}: {e}", path.display())),
    }
}

fn resolve_event_boundary_matrix_path_with_override(
    workspace_root: &Path,
    event_boundary_override: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(path) = event_boundary_override {
        if !path.is_file() {
            return Err(format!(
                "{EVENT_BOUNDARY_MATRIX_ENV} points to a missing canonical event matrix file: {}",
                path.display()
            ));
        }
        return Ok(path);
    }

    for ancestor in workspace_root.ancestors() {
        for relative in EVENT_BOUNDARY_MATRIX_RELATIVES {
            let candidate = ancestor.join(relative);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    resolve_missing_event_boundary_matrix_path(workspace_root)
}

fn missing_event_boundary_matrix_error() -> String {
    format!(
        "canonical event matrix not found; set {EVENT_BOUNDARY_MATRIX_ENV} or provide one of: {}",
        EVENT_BOUNDARY_MATRIX_RELATIVES.join(", ")
    )
}

#[cfg(not(test))]
fn resolve_missing_event_boundary_matrix_path(_workspace_root: &Path) -> Result<PathBuf, String> {
    Err(missing_event_boundary_matrix_error())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn resolve_missing_event_boundary_matrix_path(workspace_root: &Path) -> Result<PathBuf, String> {
    if !should_synthesize_owner_contracts_for_tests(workspace_root) {
        return Err(missing_event_boundary_matrix_error());
    }
    let path = std::env::temp_dir().join(format!(
        "radroots_xtask_event_boundary_{}.md",
        std::process::id()
    ));
    fs::write(&path, synthetic_event_boundary_matrix())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn synthetic_event_boundary_matrix() -> String {
    let mut raw = String::from(
        "# Event boundary matrix\n\n## Coverage matrix\n\n| Domain | Kind | Radroots Type | RPC Methods | Notes |\n| --- | --- | --- | --- | --- |\n",
    );
    for expectation in CANONICAL_EVENT_BOUNDARY_EXPECTATIONS {
        raw.push_str(&format!(
            "| {} | {} | {} | {} | synthetic test matrix |\n",
            expectation.domain,
            expectation.kind,
            expectation.radroots_type,
            expectation.rpc_methods.join(", ")
        ));
    }
    raw.push('\n');
    raw
}

fn parse_event_boundary_matrix(path: &Path) -> Result<BTreeMap<String, EventBoundaryRow>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut rows = BTreeMap::new();
    let mut in_table = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "| Domain | Kind | Radroots Type | RPC Methods | Notes |" {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        if trimmed == "| --- | --- | --- | --- | --- |" {
            continue;
        }
        if !trimmed.starts_with('|') {
            break;
        }
        let columns = trimmed
            .trim_matches('|')
            .split('|')
            .map(|part| part.trim())
            .collect::<Vec<_>>();
        if columns.len() != 5 {
            return Err(format!(
                "canonical event matrix row in {} must have exactly 5 columns: {}",
                path.display(),
                trimmed
            ));
        }
        let domain = columns[0].to_string();
        if domain.is_empty() {
            return Err(format!(
                "canonical event matrix row in {} must define a non-empty domain",
                path.display()
            ));
        }
        let rpc_methods = columns[3]
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect::<BTreeSet<_>>();
        if rpc_methods.is_empty() {
            return Err(format!(
                "canonical event matrix row {} in {} must define rpc methods",
                domain,
                path.display()
            ));
        }
        let row = EventBoundaryRow {
            domain: domain.clone(),
            kind: columns[1].to_string(),
            radroots_type: columns[2].to_string(),
            rpc_methods,
        };
        if rows.insert(domain.clone(), row).is_some() {
            return Err(format!(
                "canonical event matrix {} has duplicate domain row {}",
                path.display(),
                domain
            ));
        }
    }

    if rows.is_empty() {
        return Err(format!(
            "canonical event matrix {} does not contain the coverage table",
            path.display()
        ));
    }

    Ok(rows)
}

fn validate_event_boundary_source_witness(
    workspace_root: &Path,
    domain: &str,
    witness: &EventBoundarySourceWitness,
) -> Result<(), String> {
    let path = workspace_root.join(witness.relative_path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    for fragment in witness.required_fragments {
        if !source.contains(fragment) {
            return Err(format!(
                "canonical event row {} is missing required implementation fragment {} in {}",
                domain,
                fragment,
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_canonical_event_boundary_with_override(
    workspace_root: &Path,
    event_boundary_override: Option<PathBuf>,
) -> Result<(), String> {
    let matrix_path =
        resolve_event_boundary_matrix_path_with_override(workspace_root, event_boundary_override)?;
    let rows = parse_event_boundary_matrix(&matrix_path)?;
    let expected_domains = CANONICAL_EVENT_BOUNDARY_EXPECTATIONS
        .iter()
        .map(|row| row.domain.to_string())
        .collect::<BTreeSet<_>>();
    let actual_domains = rows.keys().cloned().collect::<BTreeSet<_>>();
    if actual_domains != expected_domains {
        let missing = expected_domains
            .difference(&actual_domains)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = actual_domains
            .difference(&expected_domains)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "canonical event matrix {} is missing rows: {}; and includes unexpected rows: {}",
            matrix_path.display(),
            join_set(&missing),
            join_set(&extra)
        ));
    }

    for expectation in CANONICAL_EVENT_BOUNDARY_EXPECTATIONS {
        let row = rows.get(expectation.domain).ok_or_else(|| {
            format!(
                "canonical event matrix {} is missing required row {}",
                matrix_path.display(),
                expectation.domain
            )
        })?;
        if row.kind != expectation.kind {
            return Err(format!(
                "canonical event row {} kind drift: expected {}, got {}",
                expectation.domain, expectation.kind, row.kind
            ));
        }
        if row.radroots_type != expectation.radroots_type {
            return Err(format!(
                "canonical event row {} type drift: expected {}, got {}",
                expectation.domain, expectation.radroots_type, row.radroots_type
            ));
        }
        let expected_methods = expectation
            .rpc_methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<BTreeSet<_>>();
        if row.rpc_methods != expected_methods {
            return Err(format!(
                "canonical event row {} rpc drift: expected {}, got {}",
                expectation.domain,
                join_set(&expected_methods),
                join_set(&row.rpc_methods)
            ));
        }
        for witness in expectation.witnesses {
            validate_event_boundary_source_witness(workspace_root, expectation.domain, witness)?;
        }
    }

    Ok(())
}

pub fn validate_canonical_event_boundary(workspace_root: &Path) -> Result<(), String> {
    validate_canonical_event_boundary_with_override(workspace_root, None)
}

fn contract_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("contracts")
}

fn conformance_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(CONFORMANCE_ROOT_RELATIVE)
}

fn conformance_schema_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(CONFORMANCE_SCHEMA_RELATIVE)
}

fn required_field_set(value: &Value, field: &str, path: &Path) -> Result<BTreeSet<String>, String> {
    let required = value
        .as_array()
        .ok_or_else(|| format!("{field} in {} must be an array", path.display()))?;
    let mut names = BTreeSet::new();
    for item in required {
        let name = item
            .as_str()
            .ok_or_else(|| format!("{field} in {} must contain strings", path.display()))?;
        if name.trim().is_empty() {
            return Err(format!(
                "{field} in {} must not contain empty names",
                path.display()
            ));
        }
        names.insert(name.to_string());
    }
    Ok(names)
}

fn validate_string_schema_property(
    property: &Value,
    field: &str,
    path: &Path,
    min_length: Option<u64>,
    pattern: Option<&str>,
) -> Result<(), String> {
    let property = property
        .as_object()
        .ok_or_else(|| format!("{field} schema in {} must be an object", path.display()))?;
    let kind = property
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} schema in {} must declare type", path.display()))?;
    if kind != "string" {
        return Err(format!(
            "{field} schema in {} must use type=string",
            path.display()
        ));
    }
    if let Some(expected) = min_length {
        let actual = property
            .get("minLength")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{field} schema in {} must set minLength", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{field} schema in {} must set minLength={expected}",
                path.display()
            ));
        }
    }
    if let Some(expected) = pattern {
        let actual = property
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{field} schema in {} must set pattern", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{field} schema in {} must set pattern {}",
                path.display(),
                expected
            ));
        }
    }
    Ok(())
}

fn validate_conformance_schema(workspace_root: &Path) -> Result<(), String> {
    let path = conformance_schema_path(workspace_root);
    let schema = parse_json::<Value>(&path)?;
    let schema_obj = schema.as_object().ok_or_else(|| {
        format!(
            "conformance schema {} must be a JSON object",
            path.display()
        )
    })?;
    let schema_type = schema_obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("conformance schema {} must declare type", path.display()))?;
    if schema_type != "object" {
        return Err(format!(
            "conformance schema {} must use type=object",
            path.display()
        ));
    }
    let additional = schema_obj
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "conformance schema {} must declare additionalProperties",
                path.display()
            )
        })?;
    if additional {
        return Err(format!(
            "conformance schema {} must disallow additionalProperties",
            path.display()
        ));
    }
    let root_required = required_field_set(
        schema_obj.get("required").ok_or_else(|| {
            format!(
                "conformance schema {} missing required list",
                path.display()
            )
        })?,
        "required",
        &path,
    )?;
    let expected_root_required = BTreeSet::from([
        "suite".to_string(),
        "contract_version".to_string(),
        "vectors".to_string(),
    ]);
    if root_required != expected_root_required {
        return Err(format!(
            "conformance schema {} must require suite, contract_version, and vectors",
            path.display()
        ));
    }
    let properties = schema_obj
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "conformance schema {} missing properties map",
                path.display()
            )
        })?;
    validate_string_schema_property(
        properties.get("suite").ok_or_else(|| {
            format!(
                "conformance schema {} missing suite property",
                path.display()
            )
        })?,
        "suite",
        &path,
        Some(1),
        None,
    )?;
    validate_string_schema_property(
        properties.get("contract_version").ok_or_else(|| {
            format!(
                "conformance schema {} missing contract_version property",
                path.display()
            )
        })?,
        "contract_version",
        &path,
        None,
        Some("^[0-9]+\\.[0-9]+\\.[0-9]+$"),
    )?;
    let vectors = properties
        .get("vectors")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "conformance schema {} missing vectors property",
                path.display()
            )
        })?;
    let vectors_type = vectors
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vectors schema in {} must declare type", path.display()))?;
    if vectors_type != "array" {
        return Err(format!(
            "vectors schema in {} must use type=array",
            path.display()
        ));
    }
    let items = vectors
        .get("items")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("vectors schema in {} must define items", path.display()))?;
    let items_type = items
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("vector item schema in {} must declare type", path.display()))?;
    if items_type != "object" {
        return Err(format!(
            "vector item schema in {} must use type=object",
            path.display()
        ));
    }
    let items_additional = items
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            format!(
                "vector item schema in {} must declare additionalProperties",
                path.display()
            )
        })?;
    if items_additional {
        return Err(format!(
            "vector item schema in {} must disallow additionalProperties",
            path.display()
        ));
    }
    let item_required = required_field_set(
        items.get("required").ok_or_else(|| {
            format!(
                "vector item schema in {} missing required list",
                path.display()
            )
        })?,
        "required",
        &path,
    )?;
    let expected_item_required =
        BTreeSet::from(["id".to_string(), "input".to_string(), "kind".to_string()]);
    if item_required != expected_item_required {
        return Err(format!(
            "vector item schema in {} must require id, kind, and input",
            path.display()
        ));
    }
    let expected_one_of = serde_json::json!([
        {
            "required": ["expected"],
            "not": {"required": ["expected_error_contains"]}
        },
        {
            "required": ["expected_error_contains"],
            "not": {"required": ["expected"]}
        }
    ]);
    if items.get("oneOf") != Some(&expected_one_of) {
        return Err(format!(
            "vector item schema in {} must require exactly one of expected or expected_error_contains",
            path.display()
        ));
    }
    let item_properties = items
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "vector item schema in {} missing properties",
                path.display()
            )
        })?;
    validate_string_schema_property(
        item_properties.get("id").ok_or_else(|| {
            format!(
                "vector item schema in {} missing id property",
                path.display()
            )
        })?,
        "id",
        &path,
        Some(1),
        None,
    )?;
    validate_string_schema_property(
        item_properties.get("kind").ok_or_else(|| {
            format!(
                "vector item schema in {} missing kind property",
                path.display()
            )
        })?,
        "kind",
        &path,
        Some(1),
        None,
    )?;
    for field in ["input", "expected"] {
        let property = item_properties.get(field).ok_or_else(|| {
            format!(
                "vector item schema in {} missing {} property",
                path.display(),
                field
            )
        })?;
        if !property.is_object() {
            return Err(format!(
                "vector item schema in {} must define {} as an object schema",
                path.display(),
                field
            ));
        }
    }
    validate_string_schema_property(
        item_properties
            .get("expected_error_contains")
            .ok_or_else(|| {
                format!(
                    "vector item schema in {} missing expected_error_contains property",
                    path.display()
                )
            })?,
        "expected_error_contains",
        &path,
        Some(1),
        None,
    )?;
    Ok(())
}

fn base_contract_version(version: &str) -> &str {
    version.split_once('-').map_or(version, |(base, _)| base)
}

fn parse_semver_version(version: &str) -> Result<Version, String> {
    Version::parse(version)
        .map_err(|error| format!("version {version} is not valid SemVer: {error}"))
}

fn validate_contract_version_lockstep(bundle: &ContractBundle) -> Result<(), String> {
    let contract_version = bundle.manifest.contract.version.as_str();
    parse_semver_version(contract_version)?;
    if bundle.version.contract.version != contract_version {
        return Err(format!(
            "version contract {} must match manifest contract version {}",
            bundle.version.contract.version, contract_version
        ));
    }
    if bundle.operations_manifest.contract.version != contract_version {
        return Err(format!(
            "operations contract version {} must match manifest contract version {}",
            bundle.operations_manifest.contract.version, contract_version
        ));
    }
    Ok(())
}

fn validate_workspace_version_lockstep(
    workspace_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let workspace_manifest =
        parse_toml::<WorkspaceVersionCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let architecture_path = workspace_root.join("docs/specs/radroots_crates_release_v1.toml");
    let governed_version = if architecture_path.is_file() {
        parse_toml::<CratesReleaseArchitecture>(&architecture_path)?
            .repositories
            .lib
            .version
    } else {
        contract_version.to_owned()
    };
    if workspace_manifest.workspace.package.version != governed_version {
        return Err(format!(
            "workspace.package.version {} must match library repository version {}",
            workspace_manifest.workspace.package.version, governed_version
        ));
    }
    let mut governed_packages = BTreeMap::new();
    for member in &workspace_manifest.workspace.members {
        let package_path = workspace_root.join(member).join("Cargo.toml");
        let package = parse_toml::<VersionedPackageCargoManifest>(&package_path)?;
        let expected_version = governed_version.as_str();
        match package.package.version {
            PackageVersionSource::Literal(ref version) if version == expected_version => {}
            PackageVersionSource::Literal(version) => {
                return Err(format!(
                    "workspace member {member} package version {version} must match governed version {expected_version}"
                ));
            }
            PackageVersionSource::Workspace { workspace } => {
                return Err(format!(
                    "workspace member {member} must set an explicit package version {expected_version}, not version.workspace = {workspace}, so mounted path consumers preserve the governed package version"
                ));
            }
        }
        governed_packages.insert(
            member.clone(),
            (package.package.name.clone(), expected_version.to_owned()),
        );

        if package.package.name.starts_with("radroots_") {
            let exact_requirement = format!("={expected_version}");
            let dependency = workspace_manifest
                .workspace
                .dependencies
                .get(&package.package.name)
                .ok_or_else(|| {
                    format!(
                        "workspace dependency {} is required for member {member}",
                        package.package.name
                    )
                })?;
            if dependency.path.as_deref() != Some(member.as_str()) {
                return Err(format!(
                    "workspace dependency {} path must be {member}",
                    package.package.name
                ));
            }
            if dependency.version.as_deref() != Some(exact_requirement.as_str()) {
                return Err(format!(
                    "workspace dependency {} version must be the exact requirement {}",
                    package.package.name, exact_requirement
                ));
            }
        }
    }

    for (dependency_name, dependency) in &workspace_manifest.workspace.dependencies {
        let Some(path) = dependency.path.as_deref() else {
            continue;
        };
        let Some((_, expected_version)) = governed_packages.get(path) else {
            continue;
        };
        let exact_requirement = format!("={expected_version}");
        if dependency.version.as_deref() != Some(exact_requirement.as_str()) {
            return Err(format!(
                "workspace path dependency {dependency_name} version must be the exact requirement {exact_requirement}"
            ));
        }
    }

    validate_cargo_lock_version_lockstep(workspace_root, &governed_packages)
}

fn validate_cargo_lock_version_lockstep(
    workspace_root: &Path,
    governed_packages: &BTreeMap<String, (String, String)>,
) -> Result<(), String> {
    let lock = parse_toml::<CargoLockManifest>(&workspace_root.join("Cargo.lock"))?;
    for (member, (package_name, expected_version)) in governed_packages {
        let workspace_entries = lock
            .package
            .iter()
            .filter(|package| package.name == *package_name && package.source.is_none())
            .collect::<Vec<_>>();
        if workspace_entries.len() != 1 {
            return Err(format!(
                "Cargo.lock must contain exactly one source-free entry for workspace member {member} ({package_name})"
            ));
        }
        if workspace_entries[0].version != *expected_version {
            return Err(format!(
                "Cargo.lock package {package_name} version {} must match governed version {expected_version}",
                workspace_entries[0].version
            ));
        }
    }
    Ok(())
}

fn valid_release_change_id(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validate_release_record(
    workspace_root: &Path,
    contract_version: &str,
    semver: &SemverRules,
) -> Result<(), String> {
    let current_version = parse_semver_version(contract_version)?;
    let releases_root = workspace_root.join(RELEASES_ROOT_RELATIVE);
    if !releases_root.is_dir() {
        return Err(format!(
            "release records directory {RELEASES_ROOT_RELATIVE} is required"
        ));
    }
    let record_relative = format!("{RELEASES_ROOT_RELATIVE}/{contract_version}.toml");
    let record = parse_toml::<ReleaseRecord>(&workspace_root.join(&record_relative))?;
    if record.schema_version != 1 {
        return Err(format!(
            "release record {record_relative} schema_version must be 1"
        ));
    }
    if record.release.version != contract_version {
        return Err(format!(
            "release record version {} must match contract version {contract_version}",
            record.release.version
        ));
    }
    let previous_version = parse_semver_version(&record.release.previous_version)?;
    if record.release.previous_version == contract_version {
        return Err("release.previous_version must differ from release.version".to_string());
    }
    if current_version <= previous_version {
        return Err(format!(
            "release version {contract_version} must be greater than previous version {}",
            record.release.previous_version
        ));
    }
    let contract_base_version = format!(
        "{}.{}.{}",
        current_version.major, current_version.minor, current_version.patch
    );
    if record.release.contract_base_version != contract_base_version {
        return Err(format!(
            "release.contract_base_version {} must match contract base version {}",
            record.release.contract_base_version, contract_base_version
        ));
    }
    if !matches!(
        record.release.status.as_str(),
        "unreleased" | "released" | "yanked"
    ) {
        return Err(format!(
            "release.status {} must be unreleased, released, or yanked",
            record.release.status
        ));
    }

    let mut expected_artifacts = vec![
        (
            record.artifacts.changelog.as_str(),
            CHANGELOG_RELATIVE,
            false,
        ),
        (
            record.artifacts.manifest.as_str(),
            "contracts/manifest.toml",
            false,
        ),
        (
            record.artifacts.operations.as_str(),
            "contracts/operations.toml",
            false,
        ),
        (
            record.artifacts.replica.as_str(),
            REPLICA_CONTRACT_RELATIVE,
            false,
        ),
        (
            record.artifacts.conformance.as_str(),
            CONFORMANCE_ROOT_RELATIVE,
            true,
        ),
        (
            record.artifacts.publish_policy.as_str(),
            RELEASE_POLICY_RELATIVE,
            false,
        ),
    ];
    if let Some(sqlite_runtime) = record.artifacts.sqlite_runtime.as_deref() {
        expected_artifacts.push((sqlite_runtime, SQLITE_RUNTIME_CONTRACT_RELATIVE, false));
    }
    for (actual, expected, directory) in expected_artifacts {
        if actual != expected {
            return Err(format!(
                "release artifact path {actual} must use canonical path {expected}"
            ));
        }
        let path = workspace_root.join(actual);
        if directory && !path.is_dir() || !directory && !path.is_file() {
            return Err(format!("release artifact {actual} does not exist"));
        }
    }

    if record.changes.is_empty() {
        return Err("release record must contain at least one change".to_string());
    }
    let mut change_ids = BTreeSet::new();
    let mut has_breaking_change = false;
    let major_impacts = semver
        .major_on
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let minor_impacts = semver
        .minor_on
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let patch_impacts = semver
        .patch_on
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for change in &record.changes {
        if !valid_release_change_id(&change.id) {
            return Err(format!(
                "release change id {} must use lowercase kebab-case",
                change.id
            ));
        }
        if !change_ids.insert(change.id.as_str()) {
            return Err(format!(
                "release record has duplicate change id {}",
                change.id
            ));
        }
        if !matches!(
            change.classification.as_str(),
            "breaking" | "feature" | "fix" | "deprecation" | "security" | "docs"
        ) {
            return Err(format!(
                "release change {} has unsupported classification {}",
                change.id, change.classification
            ));
        }
        has_breaking_change |= change.classification == "breaking";
        if change.semver_impacts.is_empty() {
            return Err(format!(
                "release change {} must declare at least one exact semver impact",
                change.id
            ));
        }
        let mut change_impacts = BTreeSet::new();
        let mut has_major_impact = false;
        let mut has_minor_impact = false;
        let mut has_patch_impact = false;
        for impact in &change.semver_impacts {
            if !change_impacts.insert(impact.as_str()) {
                return Err(format!(
                    "release change {} has duplicate semver impact {impact}",
                    change.id
                ));
            }
            if major_impacts.contains(impact.as_str()) {
                has_major_impact = true;
            } else if minor_impacts.contains(impact.as_str()) {
                has_minor_impact = true;
            } else if patch_impacts.contains(impact.as_str()) {
                has_patch_impact = true;
            } else {
                return Err(format!(
                    "release change {} semver impact {impact} is not governed by contracts/version.toml",
                    change.id
                ));
            }
        }
        let classification_matches = if has_major_impact {
            change.classification == "breaking"
        } else if has_minor_impact {
            matches!(change.classification.as_str(), "feature" | "deprecation")
        } else if has_patch_impact {
            matches!(change.classification.as_str(), "fix" | "security" | "docs")
        } else {
            false
        };
        if !classification_matches {
            return Err(format!(
                "release change {} classification {} does not match its governed semver impacts",
                change.id, change.classification
            ));
        }
        if change.summary.trim().is_empty() {
            return Err(format!(
                "release change {} summary must not be empty",
                change.id
            ));
        }
    }
    if current_version.major != previous_version.major && !has_breaking_change {
        return Err("a major version transition requires a breaking release change".to_string());
    }

    let declares_registry_sqlite = change_ids.contains("registry-sqlite-provenance");
    if declares_registry_sqlite != record.artifacts.sqlite_runtime.is_some() {
        return Err(format!(
            "release change registry-sqlite-provenance and artifact {SQLITE_RUNTIME_CONTRACT_RELATIVE} must be declared together"
        ));
    }
    if declares_registry_sqlite {
        validate_sqlite_runtime_contract(workspace_root)?;
    }

    validate_changelog_release_notes(workspace_root, contract_version)
}

fn validate_sqlite_runtime_contract(workspace_root: &Path) -> Result<(), String> {
    const PACKAGE_NAME: &str = "libsqlite3-sys";
    const PACKAGE_VERSION: &str = "0.37.0";
    const PACKAGE_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
    const ACTIVATION_ROUTE: [&str; 3] = [
        "radroots_event_store/sqlite",
        "sqlx/sqlite-bundled",
        "libsqlite3-sys/bundled",
    ];

    let contract = parse_toml::<SqliteRuntimeContract>(
        &workspace_root.join(SQLITE_RUNTIME_CONTRACT_RELATIVE),
    )?;
    if contract.schema_version != 1
        || contract.package.name != PACKAGE_NAME
        || contract.package.version != PACKAGE_VERSION
        || contract.package.source != PACKAGE_SOURCE
        || contract.activation.route != ACTIVATION_ROUTE
    {
        return Err(format!(
            "{SQLITE_RUNTIME_CONTRACT_RELATIVE} must govern the exact crates.io bundled SQLite runtime identity and activation route"
        ));
    }
    if contract.package.checksum.len() != 64
        || !contract
            .package
            .checksum
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{SQLITE_RUNTIME_CONTRACT_RELATIVE} package.checksum must be a lowercase SHA-256 digest"
        ));
    }

    let lock = parse_toml::<CargoLockManifest>(&workspace_root.join("Cargo.lock"))?;
    let matches = lock
        .package
        .iter()
        .filter(|package| {
            package.name == contract.package.name
                && package.version == contract.package.version
                && package.source.as_deref() == Some(contract.package.source.as_str())
                && package.checksum.as_deref() == Some(contract.package.checksum.as_str())
        })
        .count();
    if matches != 1 {
        return Err(format!(
            "Cargo.lock must contain exactly one package matching {SQLITE_RUNTIME_CONTRACT_RELATIVE}"
        ));
    }

    let workspace = parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    for member in workspace.workspace.members {
        let package =
            parse_toml::<PackageCargoManifest>(&workspace_root.join(&member).join("Cargo.toml"))?;
        if package.package.name == PACKAGE_NAME {
            return Err(format!(
                "workspace member {member} must not vendor registry package {PACKAGE_NAME}"
            ));
        }
    }

    let storage_sqlite =
        fs::read_to_string(workspace_root.join("crates/storage_sqlite/Cargo.toml"))
            .map_err(|error| format!("read crates/storage_sqlite/Cargo.toml: {error}"))?;
    let storage_sqlite: toml::Value = toml::from_str(&storage_sqlite)
        .map_err(|error| format!("parse crates/storage_sqlite/Cargo.toml: {error}"))?;
    let sqlite_features = storage_sqlite
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("sqlx"))
        .and_then(|sqlx| sqlx.get("features"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            "crates/storage_sqlite/Cargo.toml dependencies.sqlx.features is required".to_string()
        })?;
    if !sqlite_features
        .iter()
        .any(|feature| feature.as_str() == Some("sqlite-bundled"))
    {
        return Err(
            "crates/storage_sqlite/Cargo.toml dependencies.sqlx.features must activate sqlite-bundled"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_changelog_release_notes(
    workspace_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let path = workspace_root.join(CHANGELOG_RELATIVE);
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let heading = format!("## [{contract_version}]");
    let mut in_release = false;
    let mut has_release_note = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == heading {
            if in_release {
                return Err(format!(
                    "{CHANGELOG_RELATIVE} contains duplicate heading {heading}"
                ));
            }
            in_release = true;
            continue;
        }
        if in_release && trimmed.starts_with("## [") {
            break;
        }
        if in_release && trimmed.starts_with("- ") && trimmed.len() > 2 {
            has_release_note = true;
        }
    }
    if !in_release {
        return Err(format!("{CHANGELOG_RELATIVE} is missing heading {heading}"));
    }
    if !has_release_note {
        return Err(format!(
            "{CHANGELOG_RELATIVE} release {contract_version} must contain at least one note"
        ));
    }
    Ok(())
}

fn validate_conformance_vector_mirrors(workspace_root: &Path) -> Result<(), String> {
    for (canonical_relative, mirror_relative) in CONFORMANCE_VECTOR_MIRRORS {
        let canonical = fs::read(workspace_root.join(canonical_relative))
            .map_err(|error| format!("read {canonical_relative}: {error}"))?;
        let mirror = fs::read(workspace_root.join(mirror_relative))
            .map_err(|error| format!("read {mirror_relative}: {error}"))?;
        if canonical != mirror {
            return Err(format!(
                "packaged conformance mirror {mirror_relative} must exactly match {canonical_relative}"
            ));
        }
    }
    Ok(())
}

fn validate_version_governance(
    bundle: &ContractBundle,
    workspace_root: &Path,
) -> Result<(), String> {
    validate_contract_version_lockstep(bundle)?;
    let version = bundle.manifest.contract.version.as_str();
    validate_workspace_version_lockstep(workspace_root, version)?;
    validate_release_record(workspace_root, version, &bundle.version.semver)?;
    validate_conformance_vector_mirrors(workspace_root)
}

fn collect_conformance_vector_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(e) => return Err(format!("read dir {}: {e}", dir.display())),
    };
    let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_conformance_vector_paths(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    Ok(())
}

fn validate_conformance_vector_file(
    path: &Path,
    contract_version: &str,
) -> Result<ConformanceVectorFile, String> {
    let vector = parse_json::<ConformanceVectorFile>(path)?;
    if vector.suite.trim().is_empty() {
        return Err(format!(
            "conformance vector {} suite must not be empty",
            path.display()
        ));
    }
    if vector.vectors.is_empty() {
        return Err(format!(
            "conformance vector {} must contain at least one vector",
            path.display()
        ));
    }
    if vector.contract_version != base_contract_version(contract_version) {
        return Err(format!(
            "conformance vector {} version {} must match contract version {}",
            path.display(),
            vector.contract_version,
            base_contract_version(contract_version)
        ));
    }
    let mut ids = BTreeSet::new();
    for entry in &vector.vectors {
        if entry.id.trim().is_empty() || entry.kind.trim().is_empty() {
            return Err(format!(
                "conformance vector {} entries must define non-empty id and kind",
                path.display()
            ));
        }
        if !ids.insert(entry.id.clone()) {
            return Err(format!(
                "conformance vector {} has duplicate vector id {}",
                path.display(),
                entry.id
            ));
        }
        match (&entry.expected, &entry.expected_error_contains) {
            (Some(_), None) => {}
            (None, Some(fragment)) if !fragment.trim().is_empty() => {}
            (None, Some(_)) => {
                return Err(format!(
                    "conformance vector {} entry {} expected_error_contains must not be blank",
                    path.display(),
                    entry.id
                ));
            }
            _ => {
                return Err(format!(
                    "conformance vector {} entry {} must define exactly one of expected or expected_error_contains",
                    path.display(),
                    entry.id
                ));
            }
        }
    }
    Ok(vector)
}

fn validate_all_conformance_vectors(
    workspace_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let vectors_dir = conformance_root(workspace_root).join("vectors");
    if !vectors_dir.is_dir() {
        return validate_missing_conformance_vectors(workspace_root, &vectors_dir);
    }
    let mut paths = Vec::new();
    collect_conformance_vector_paths(&vectors_dir, &mut paths)?;
    if paths.is_empty() {
        return Err(format!(
            "conformance vectors directory {} must contain JSON vectors",
            vectors_dir.display()
        ));
    }
    let canonical_comment_path = workspace_root.join(COMMENT_CONFORMANCE_VECTOR_RELATIVE);
    let canonical_deletion_path = workspace_root.join(DELETION_CONFORMANCE_VECTOR_RELATIVE);
    let canonical_deletion_suppression_path =
        workspace_root.join(DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE);
    for path in paths {
        let vector = validate_conformance_vector_file(&path, contract_version)?;
        validate_comment_vector_namespace(&path, &canonical_comment_path, &vector)?;
        validate_deletion_vector_namespace(
            &path,
            &canonical_deletion_path,
            &canonical_deletion_suppression_path,
            &vector,
        )?;
    }
    Ok(())
}

fn validate_deletion_vector_namespace(
    path: &Path,
    canonical_request_path: &Path,
    canonical_suppression_path: &Path,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    if path == canonical_request_path || path == canonical_suppression_path {
        return Ok(());
    }
    if let Some(entry) = vector
        .vectors
        .iter()
        .find(|entry| entry.kind.starts_with("social.deletion_request."))
    {
        return Err(format!(
            "deletion conformance case kind {} in {} is outside canonical vectors {} and {}",
            entry.kind,
            path.display(),
            canonical_request_path.display(),
            canonical_suppression_path.display()
        ));
    }
    Ok(())
}

fn validate_comment_vector_namespace(
    path: &Path,
    canonical_path: &Path,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    if path == canonical_path {
        return Ok(());
    }
    if let Some(entry) = vector
        .vectors
        .iter()
        .find(|entry| entry.kind.starts_with("social.comment."))
    {
        return Err(format!(
            "comment conformance case kind {} in {} is outside canonical vector {}",
            entry.kind,
            path.display(),
            canonical_path.display()
        ));
    }
    Ok(())
}

pub fn write_knowledge_contract_manifest(workspace_root: &Path) -> Result<(), String> {
    let manifest_json = write_knowledge_contract_manifest_artifacts(workspace_root)?;
    validate_knowledge_contract_manifest_context(workspace_root, &manifest_json)
}

pub fn validate_knowledge_contract_manifest(workspace_root: &Path) -> Result<(), String> {
    let manifest_json = validate_knowledge_contract_manifest_artifacts(workspace_root)?;
    validate_knowledge_contract_manifest_context(workspace_root, &manifest_json)
}

fn write_knowledge_contract_manifest_artifacts(workspace_root: &Path) -> Result<String, String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        let manifest_json = expected_knowledge_contract_manifest_json()?;
        let manifest_sha256 = expected_knowledge_contract_manifest_sha256()?;
        transaction.write(vec![
            GeneratedArtifact {
                relative: KNOWLEDGE_MANIFEST_RELATIVE,
                contents: manifest_json.into_bytes(),
            },
            GeneratedArtifact {
                relative: KNOWLEDGE_MANIFEST_SHA256_RELATIVE,
                contents: format!("{manifest_sha256}\n").into_bytes(),
            },
        ])?;
        validate_knowledge_contract_manifest_artifacts_under_lock(workspace_root)
    })
}

fn validate_knowledge_contract_manifest_artifacts(workspace_root: &Path) -> Result<String, String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_knowledge_contract_manifest_artifacts_under_lock(workspace_root)
    })
}

fn validate_knowledge_contract_manifest_artifacts_under_lock(
    workspace_root: &Path,
) -> Result<String, String> {
    let expected_json = expected_knowledge_contract_manifest_json()?;
    let expected_sha256 = expected_knowledge_contract_manifest_sha256()?;
    let actual_json = read_regular_file(workspace_root, KNOWLEDGE_MANIFEST_RELATIVE)?;
    let actual_sha256 = read_regular_file(workspace_root, KNOWLEDGE_MANIFEST_SHA256_RELATIVE)?;
    let actual_json_text = std::str::from_utf8(&actual_json)
        .map_err(|error| format!("{KNOWLEDGE_MANIFEST_RELATIVE} must be UTF-8 JSON: {error}"))?;
    let parsed =
        radroots_event_codec::manifest::parse_knowledge_contract_manifest_json(actual_json_text)
            .map_err(|error| format!("parse {KNOWLEDGE_MANIFEST_RELATIVE}: {error}"))?;

    validate_knowledge_contract_manifest_shape(&parsed)?;
    validate_canonical_json_artifact(KNOWLEDGE_MANIFEST_RELATIVE, &actual_json)?;
    validate_sha256_artifact(KNOWLEDGE_MANIFEST_SHA256_RELATIVE, &actual_sha256)?;

    if actual_json != expected_json.as_bytes() {
        return Err(stale_knowledge_manifest_error(KNOWLEDGE_MANIFEST_RELATIVE));
    }
    if actual_sha256 != format!("{expected_sha256}\n").as_bytes() {
        return Err(stale_knowledge_manifest_error(
            KNOWLEDGE_MANIFEST_SHA256_RELATIVE,
        ));
    }
    if parsed != radroots_event_codec::manifest::knowledge_contract_manifest() {
        return Err(stale_knowledge_manifest_error(KNOWLEDGE_MANIFEST_RELATIVE));
    }

    Ok(actual_json_text.to_owned())
}

fn expected_knowledge_contract_manifest_json() -> Result<String, String> {
    let expected_json = radroots_event_codec::manifest::contract_manifest_json()
        .map_err(|error| format!("serialize knowledge contract manifest: {error}"))?;
    Ok(expected_json)
}

fn expected_knowledge_contract_manifest_sha256() -> Result<String, String> {
    radroots_event_codec::manifest::contract_manifest_sha256()
        .map_err(|error| format!("hash knowledge contract manifest: {error}"))
}

fn validate_knowledge_contract_manifest_shape(
    manifest: &radroots_event_codec::manifest::RadrootsKnowledgeContractManifest,
) -> Result<(), String> {
    if manifest.schema_version
        != radroots_event_codec::manifest::RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION
    {
        return Err(format!(
            "{KNOWLEDGE_MANIFEST_RELATIVE} schema_version must be {}",
            radroots_event_codec::manifest::RADROOTS_KNOWLEDGE_CONTRACT_MANIFEST_SCHEMA_VERSION
        ));
    }
    if manifest.registry_version
        != radroots_event_codec::manifest::registry_v7::RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION
    {
        return Err(format!(
            "{KNOWLEDGE_MANIFEST_RELATIVE} registry_version must be {}",
            radroots_event_codec::manifest::registry_v7::RADROOTS_EVENT_CONTRACT_REGISTRY_V7_VERSION
        ));
    }
    if manifest.contract_count != manifest.contracts.len() {
        return Err(format!(
            "{KNOWLEDGE_MANIFEST_RELATIVE} contract_count must match its contract inventory"
        ));
    }
    if manifest
        .contracts
        .windows(2)
        .any(|pair| pair[0].contract_id >= pair[1].contract_id)
    {
        return Err(format!(
            "{KNOWLEDGE_MANIFEST_RELATIVE} contract IDs must be unique and strictly sorted"
        ));
    }
    Ok(())
}

fn stale_knowledge_manifest_error(relative: &str) -> String {
    format!("{relative} is stale; run `{KNOWLEDGE_MANIFEST_WRITE_COMMAND}`")
}

fn validate_knowledge_contract_manifest_context(
    workspace_root: &Path,
    manifest_json: &str,
) -> Result<(), String> {
    validate_knowledge_manifest_witnesses(workspace_root, manifest_json)?;
    validate_knowledge_conformance_vector_inventory(workspace_root)?;

    let bundle = load_contract_bundle(workspace_root)?;
    let knowledge_manifest_vector = validate_conformance_vector_file(
        &workspace_root.join(KNOWLEDGE_MANIFEST_AND_DECODE_RELATIVE),
        &bundle.manifest.contract.version,
    )?;
    validate_knowledge_manifest_vector_semantics(manifest_json, &knowledge_manifest_vector)?;
    validate_conformance_vector_file(
        &workspace_root.join(KNOWLEDGE_PUBLIC_SURFACE_RELATIVE),
        &bundle.manifest.contract.version,
    )?;
    Ok(())
}

fn validate_knowledge_manifest_vector_semantics(
    manifest_json: &str,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    let manifest = serde_json::from_str::<Value>(manifest_json)
        .map_err(|error| format!("parse knowledge manifest JSON: {error}"))?;
    let schema_version = manifest
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "knowledge manifest schema_version must be an unsigned integer".to_string()
        })?;
    let registry_version = manifest
        .get("registry_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "knowledge manifest registry_version must be an unsigned integer".to_string()
        })?;
    let case = vector
        .vectors
        .iter()
        .find(|entry| entry.id == "knowledge_manifest_fields_valid_001")
        .ok_or_else(|| {
            "knowledge manifest conformance must define knowledge_manifest_fields_valid_001"
                .to_string()
        })?;
    if case.kind != "knowledge.contract_manifest_json.valid" {
        return Err(format!(
            "knowledge manifest conformance case kind drift: expected knowledge.contract_manifest_json.valid, got {}",
            case.kind
        ));
    }

    let expected_registry_marker = format!("radroots_event_contract_registry_v{registry_version}");
    let actual_registry_marker = case.input.get("registry").and_then(Value::as_str);
    if actual_registry_marker != Some(expected_registry_marker.as_str()) {
        return Err(format!(
            "knowledge manifest conformance registry marker drift: expected {expected_registry_marker}, got {}",
            actual_registry_marker.unwrap_or("<missing-or-non-string>")
        ));
    }

    for (field, expected) in [
        ("schema_version", schema_version),
        ("registry_version", registry_version),
    ] {
        let actual = case.expected_value()?.get(field).and_then(Value::as_u64);
        if actual != Some(expected) {
            return Err(format!(
                "knowledge manifest conformance expected {field} drift: expected {expected}, got {}",
                actual
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "<missing-or-non-integer>".to_string())
            ));
        }
    }
    Ok(())
}

fn validate_knowledge_conformance_vector_inventory(workspace_root: &Path) -> Result<(), String> {
    let expected = BTreeSet::from([
        KNOWLEDGE_MANIFEST_AND_DECODE_RELATIVE.to_owned(),
        KNOWLEDGE_PUBLIC_SURFACE_RELATIVE.to_owned(),
    ]);
    let mut paths = Vec::new();
    collect_conformance_vector_paths(
        &workspace_root.join("contracts/conformance/vectors/knowledge"),
        &mut paths,
    )?;
    let mut actual = BTreeSet::new();
    for path in paths {
        let relative = path.strip_prefix(workspace_root).map_err(|error| {
            format!(
                "knowledge conformance vector {} is outside workspace root {}: {error}",
                path.display(),
                workspace_root.display()
            )
        })?;
        actual.insert(relative.to_string_lossy().replace('\\', "/"));
    }
    if actual != expected {
        return Err(format!(
            "knowledge conformance vector inventory mismatch: expected {:?}, found {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_knowledge_manifest_witnesses(
    workspace_root: &Path,
    actual_json: &str,
) -> Result<(), String> {
    for relative in legacy_knowledge_manifest_relatives() {
        let path = workspace_root.join(&relative);
        if path.exists() {
            return Err(format!(
                "stale knowledge manifest artifact remains at {}",
                relative
            ));
        }
    }

    let value = serde_json::from_str::<Value>(actual_json)
        .map_err(|error| format!("parse knowledge manifest JSON: {error}"))?;
    if value.get("schema_version").and_then(Value::as_u64) != Some(2) {
        return Err("knowledge manifest schema_version must be 2".to_string());
    }
    let contracts = value
        .get("contracts")
        .and_then(Value::as_array)
        .ok_or_else(|| "knowledge manifest contracts must be an array".to_string())?;
    let mut previous_id: Option<String> = None;
    let mut ids = BTreeSet::new();

    for contract in contracts {
        let contract_id = contract
            .get("contract_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "knowledge manifest entry missing contract_id".to_string())?;
        if let Some(previous) = previous_id.as_deref()
            && previous > contract_id
        {
            return Err("knowledge manifest contracts must be sorted by contract_id".to_string());
        }
        previous_id = Some(contract_id.to_string());
        if !ids.insert(contract_id.to_string()) {
            return Err(format!(
                "knowledge manifest has duplicate contract id {contract_id}"
            ));
        }

        if contract.get("stability").and_then(Value::as_str) != Some("experimental") {
            return Err(format!(
                "knowledge manifest contract {contract_id} must be experimental"
            ));
        }
        if contract_id == "radroots.wiki.merge_request.v1"
            && contract.get("content_schema").and_then(Value::as_str) != Some("plain_text")
        {
            return Err(
                "wiki merge request manifest content_schema must be plain_text".to_string(),
            );
        }

        let sdk_builder_support = manifest_bool_field(contract, "sdk_builder_support")?;
        let sdk_draft_support = manifest_bool_field(contract, "sdk_draft_support")?;
        let wasm_tag_builder_support = manifest_bool_field(contract, "wasm_tag_builder_support")?;
        let wasm_verified_decode_support =
            manifest_bool_field(contract, "wasm_verified_decode_support")?;

        if KNOWLEDGE_MVP_SUPPORT_CONTRACT_IDS.contains(&contract_id)
            && (!sdk_builder_support || !sdk_draft_support || !wasm_tag_builder_support)
        {
            return Err(format!(
                "knowledge manifest MVP contract {contract_id} must report SDK and WASM tag support"
            ));
        }
        if KNOWLEDGE_BETA_CONTRACT_IDS.contains(&contract_id)
            && (sdk_builder_support || sdk_draft_support || wasm_tag_builder_support)
        {
            return Err(format!(
                "knowledge manifest beta contract {contract_id} must not overclaim builder support"
            ));
        }
        if !wasm_verified_decode_support {
            return Err(format!(
                "knowledge manifest contract {contract_id} must report WASM verified decode support"
            ));
        }
    }

    for contract_id in KNOWLEDGE_MVP_SUPPORT_CONTRACT_IDS
        .iter()
        .chain(KNOWLEDGE_BETA_CONTRACT_IDS.iter())
    {
        if !ids.contains(*contract_id) {
            return Err(format!(
                "knowledge manifest missing required contract {contract_id}"
            ));
        }
    }

    Ok(())
}

fn legacy_knowledge_manifest_relatives() -> [String; 2] {
    [
        KNOWLEDGE_MANIFEST_RELATIVE.replace(".v2.", ".v1."),
        KNOWLEDGE_MANIFEST_SHA256_RELATIVE.replace(".v2.", ".v1."),
    ]
}

fn manifest_bool_field(contract: &Value, field: &str) -> Result<bool, String> {
    contract
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("knowledge manifest entry missing boolean field {field}"))
}

#[cfg(not(test))]
fn validate_missing_conformance_vectors(
    _workspace_root: &Path,
    vectors_dir: &Path,
) -> Result<(), String> {
    Err(format!(
        "conformance vectors directory {} must exist",
        vectors_dir.display()
    ))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn validate_missing_conformance_vectors(
    _workspace_root: &Path,
    _vectors_dir: &Path,
) -> Result<(), String> {
    Ok(())
}

#[derive(Debug)]
struct WorkspacePackageRecord {
    name: String,
    #[cfg_attr(not(test), allow(dead_code))]
    manifest_path: PathBuf,
    publish_enabled: bool,
    publish: Option<PackagePublish>,
    manifest_value: toml::Value,
}

fn workspace_package_records(workspace_root: &Path) -> Result<Vec<WorkspacePackageRecord>, String> {
    let workspace_manifest =
        parse_toml::<WorkspaceCargoManifest>(&workspace_root.join("Cargo.toml"))?;
    let mut records = Vec::with_capacity(workspace_manifest.workspace.members.len());
    for member in workspace_manifest.workspace.members {
        let manifest_path = workspace_root.join(&member).join("Cargo.toml");
        let raw = match fs::read_to_string(&manifest_path) {
            Ok(raw) => raw,
            Err(e) => return Err(format!("read {}: {e}", manifest_path.display())),
        };
        let manifest_value = match toml::from_str::<toml::Value>(&raw) {
            Ok(value) => value,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let package_manifest = match toml::from_str::<PackageCargoManifest>(&raw) {
            Ok(manifest) => manifest,
            Err(e) => return Err(format!("parse {}: {e}", manifest_path.display())),
        };
        let name = package_manifest.package.name;
        let publish_enabled = package_publish_enabled(package_manifest.package.publish.as_ref());
        let publish = package_manifest.package.publish.clone();
        records.push(WorkspacePackageRecord {
            name,
            manifest_path,
            publish_enabled,
            publish,
            manifest_value,
        });
    }
    Ok(records)
}

fn workspace_package_names(workspace_root: &Path) -> Result<Vec<String>, String> {
    Ok(workspace_package_records(workspace_root)?
        .into_iter()
        .map(|record| record.name)
        .collect())
}

fn coverage_required_workspace_crates(workspace_root: &Path) -> Result<BTreeSet<String>, String> {
    let names = workspace_package_names(workspace_root)?
        .into_iter()
        .filter(|crate_name| !coverage_policy_excludes_workspace_crate(crate_name))
        .collect::<Vec<_>>();
    collect_unique_set(&names, "workspace coverage crates")
}

fn coverage_policy_excludes_workspace_crate(crate_name: &str) -> bool {
    crate_name.contains("_simplex_") || crate_name.starts_with("simplex_")
}

#[cfg_attr(not(test), allow(dead_code))]
fn workspace_package_manifests(workspace_root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut manifests = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if manifests
            .insert(record.name, record.manifest_path)
            .is_some()
        {
            return Err("duplicate workspace package name in manifest map".to_string());
        }
    }
    Ok(manifests)
}

fn load_coverage_policy(
    contract_root: &Path,
) -> Result<crate::coverage::CoveragePolicyFile, String> {
    read_coverage_policy(&coverage_root(contract_root).join("coverage.toml"))
}

fn coverage_root(contract_root: &Path) -> PathBuf {
    contract_root.to_path_buf()
}

fn release_contract_path(workspace_root: &Path, _contract_version: &str) -> PathBuf {
    workspace_root.join(RELEASE_POLICY_RELATIVE)
}

#[cfg(test)]
fn root_release_policy_path(workspace_root: &Path) -> PathBuf {
    release_contract_path(workspace_root, "1.0.0")
}

fn resolve_release_contract_path_with_override(
    workspace_root: &Path,
    contract_version: &str,
    release_policy_override: Option<PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(path) = release_policy_override {
        if !path.is_file() {
            return Err(format!(
                "release policy override points to a missing file: {}",
                path.display()
            ));
        }
        return Ok(path);
    }

    let path = release_contract_path(workspace_root, contract_version);
    if !path.is_file() {
        return Err(format!(
            "release publish policy not found; expected {}",
            path.display()
        ));
    }

    Ok(path)
}

#[cfg(test)]
fn load_release_contract(
    workspace_root: &Path,
    contract_version: &str,
) -> Result<ReleaseContractFile, String> {
    load_release_contract_with_override(workspace_root, contract_version, None)
}

fn load_release_contract_with_override(
    workspace_root: &Path,
    contract_version: &str,
    release_policy_override: Option<PathBuf>,
) -> Result<ReleaseContractFile, String> {
    let path = resolve_release_contract_path_with_override(
        workspace_root,
        contract_version,
        release_policy_override,
    )?;
    parse_toml::<ReleaseContractFile>(&path)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn should_synthesize_owner_contracts_for_tests(workspace_root: &Path) -> bool {
    workspace_root
        .join("crates")
        .join("core")
        .join("Cargo.toml")
        .is_file()
        && workspace_root
            .join("crates")
            .join("event_codec")
            .join("Cargo.toml")
            .is_file()
        && workspace_root
            .join("crates")
            .join("trade")
            .join("Cargo.toml")
            .is_file()
        && workspace_root
            .join("contracts")
            .join("manifest.toml")
            .is_file()
        && workspace_root
            .join("contracts")
            .join("coverage.toml")
            .is_file()
}

fn package_publish_enabled(publish: Option<&PackagePublish>) -> bool {
    match publish {
        None => true,
        Some(PackagePublish::Bool(flag)) => *flag,
        Some(PackagePublish::Registries(registries)) => !registries.is_empty(),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn workspace_package_publish_flags(
    workspace_root: &Path,
) -> Result<BTreeMap<String, bool>, String> {
    let mut flags = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if flags
            .insert(record.name.clone(), record.publish_enabled)
            .is_some()
        {
            return Err(format!("duplicate workspace package name {}", record.name));
        }
    }
    Ok(flags)
}

fn workspace_package_publish_configs(
    workspace_root: &Path,
) -> Result<BTreeMap<String, Option<PackagePublish>>, String> {
    let mut configs = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if configs
            .insert(record.name.clone(), record.publish.clone())
            .is_some()
        {
            return Err(format!("duplicate workspace package name {}", record.name));
        }
    }
    Ok(configs)
}

fn read_workspace_package_dependencies(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let package_records = workspace_package_records(workspace_root)?;
    let workspace_names = package_records
        .iter()
        .map(|record| record.name.clone())
        .collect::<BTreeSet<_>>();

    let mut deps = BTreeMap::new();
    for record in package_records {
        let mut package_deps = BTreeSet::new();
        for section in ["dependencies", "build-dependencies"] {
            let Some(table) = record
                .manifest_value
                .get(section)
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            for dep_name in table.keys() {
                if workspace_names.contains(dep_name) {
                    package_deps.insert(dep_name.clone());
                }
            }
        }
        deps.insert(record.name, package_deps);
    }

    Ok(deps)
}

fn validate_publishable_dto_tooling_sources(
    workspace_root: &Path,
    public_crates: &BTreeSet<String>,
) -> Result<(), String> {
    let workspace_manifest_value = parse_toml::<toml::Value>(&workspace_root.join("Cargo.toml"))?;
    let package_records = workspace_package_records(workspace_root)?;

    for record in package_records {
        if !public_crates.contains(&record.name) {
            continue;
        }
        for section in ["dependencies", "build-dependencies"] {
            let Some(dependencies) = record
                .manifest_value
                .get(section)
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            for dependency_name in DTO_TOOLING_DEPENDENCIES {
                let Some(dependency_value) = dependencies.get(dependency_name) else {
                    continue;
                };
                validate_publishable_dto_dependency_source(
                    workspace_manifest_value.as_table(),
                    record.name.as_str(),
                    section,
                    dependency_name,
                    dependency_value,
                )?;
            }
        }
    }

    Ok(())
}

fn validate_publishable_dto_dependency_source(
    workspace_manifest: Option<&toml::value::Table>,
    crate_name: &str,
    section: &str,
    dependency_name: &str,
    dependency_value: &toml::Value,
) -> Result<(), String> {
    let resolved =
        resolve_workspace_dependency_source(workspace_manifest, dependency_name, dependency_value)
            .unwrap_or(dependency_value);
    if dependency_has_source_key(resolved, "git") {
        return Err(format!(
            "public crate {crate_name} {section}.{dependency_name} must use a crates.io DTO tooling dependency, not a git source"
        ));
    }
    if dependency_has_source_key(resolved, "path") {
        return Err(format!(
            "public crate {crate_name} {section}.{dependency_name} must use a crates.io DTO tooling dependency, not a path source"
        ));
    }
    Ok(())
}

fn resolve_workspace_dependency_source<'a>(
    workspace_manifest: Option<&'a toml::value::Table>,
    dependency_name: &str,
    dependency_value: &toml::Value,
) -> Option<&'a toml::Value> {
    if !dependency_has_workspace_true(dependency_value) {
        return None;
    }
    workspace_manifest?
        .get("workspace")?
        .as_table()?
        .get("dependencies")?
        .as_table()?
        .get(dependency_name)
}

fn dependency_has_workspace_true(value: &toml::Value) -> bool {
    value
        .as_table()
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

fn dependency_has_source_key(value: &toml::Value, key: &str) -> bool {
    value.as_table().and_then(|table| table.get(key)).is_some()
}

fn join_set(items: &BTreeSet<String>) -> String {
    items.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn collect_unique_set(items: &[String], field: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} contains an empty crate name"));
        }
        if !set.insert(item.clone()) {
            return Err(format!("{field} has duplicate crate {}", item));
        }
    }
    Ok(set)
}

fn collect_non_empty_set(items: &[String], field: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} contains an empty value"));
        }
        if !set.insert(item.clone()) {
            return Err(format!("{field} has duplicate value {}", item));
        }
    }
    Ok(set)
}

fn validate_crate_identifier(value: &str, field: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed != value
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed == "radroots_sdk"
    {
        return Err(format!("{field} must be a crate identifier"));
    }
    Ok(())
}

fn validate_surface_metadata(surface: &Surface) -> Result<(), String> {
    if let Some(tiers) = &surface.rust_crate_tiers {
        let mut tier_crates = BTreeSet::new();
        for (field, crates) in [
            (
                "surface.rust_crate_tiers.advanced_substrate",
                &tiers.advanced_substrate,
            ),
            (
                "surface.rust_crate_tiers.published_support",
                &tiers.published_support,
            ),
            (
                "surface.rust_crate_tiers.deferred_publication",
                &tiers.deferred_publication,
            ),
        ] {
            let entries = collect_unique_set(crates, field)?;
            if entries.is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            for crate_name in entries {
                if !tier_crates.insert(crate_name.clone()) {
                    return Err(format!(
                        "surface.rust_crate_tiers has duplicate crate {crate_name}"
                    ));
                }
            }
        }
    }

    if let Some(replica) = &surface.internal_replica_crates {
        validate_crate_identifier(&replica.schema, "surface.internal_replica_crates.schema")?;
        validate_crate_identifier(&replica.storage, "surface.internal_replica_crates.storage")?;
        validate_crate_identifier(&replica.sync, "surface.internal_replica_crates.sync")?;
    }

    Ok(())
}

fn validate_policy_metadata(policy: &Policy) -> Result<(), String> {
    if !policy.exclude_internal_workspace_crates
        || !policy.require_reproducible_exports
        || !policy.require_conformance_vectors
    {
        return Err("contract policy flags must all be true".to_string());
    }
    if let Some(replica) = &policy.replica
        && (!replica.forbid_legacy_alias_identifiers
            || !replica.require_transport_agnostic_sync_contract
            || !replica.require_deterministic_emit_ingest)
    {
        return Err("contract replica policy flags must all be true".to_string());
    }
    Ok(())
}

fn parse_replica_transfer_constant(path: &Path, name: &str) -> Result<u32, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let declaration_prefix = format!("pub const {name}: u32 =");
    let mut value = None;
    for line in source.lines() {
        let Some(raw_value) = line.trim().strip_prefix(&declaration_prefix) else {
            continue;
        };
        let raw_value = raw_value.trim().strip_suffix(';').ok_or_else(|| {
            format!(
                "replica transfer constant {name} in {} must terminate with a semicolon",
                path.display()
            )
        })?;
        let parsed = raw_value.parse::<u32>().map_err(|error| {
            format!(
                "replica transfer constant {name} in {} must be a u32 literal: {error}",
                path.display()
            )
        })?;
        if value.replace(parsed).is_some() {
            return Err(format!(
                "replica transfer constant {name} must be declared exactly once in {}",
                path.display()
            ));
        }
    }
    value.ok_or_else(|| {
        format!(
            "replica transfer constant {name} is missing from {}",
            path.display()
        )
    })
}

fn has_exact_legacy_ingest_cfg(attributes: &[syn::Attribute]) -> bool {
    let mut cfg_attributes = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"));
    let Some(attribute) = cfg_attributes.next() else {
        return false;
    };
    if cfg_attributes.next().is_some() {
        return false;
    }
    let syn::Meta::List(arguments) = &attribute.meta else {
        return false;
    };
    let Ok(predicate) = arguments.parse_args::<syn::Meta>() else {
        return false;
    };
    let syn::Meta::NameValue(feature) = predicate else {
        return false;
    };
    if !feature.path.is_ident("feature") {
        return false;
    }
    matches!(
        feature.value,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(value),
            ..
        }) if value.value() == "legacy-ingest"
    )
}

fn replica_use_tree_references_ingest(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            path.ident == "ingest" || replica_use_tree_references_ingest(&path.tree)
        }
        syn::UseTree::Name(name) => name.ident == "ingest",
        syn::UseTree::Rename(rename) => rename.ident == "ingest",
        syn::UseTree::Group(group) => group.items.iter().any(replica_use_tree_references_ingest),
        syn::UseTree::Glob(_) => false,
    }
}

fn collect_public_replica_ingest_exports<'a>(
    items: &'a [syn::Item],
    exports: &mut Vec<&'a syn::ItemUse>,
) {
    for item in items {
        match item {
            syn::Item::Use(export)
                if matches!(&export.vis, syn::Visibility::Public(_))
                    && replica_use_tree_references_ingest(&export.tree) =>
            {
                exports.push(export);
            }
            syn::Item::Mod(module) if matches!(&module.vis, syn::Visibility::Public(_)) => {
                if let Some((_, nested_items)) = &module.content {
                    collect_public_replica_ingest_exports(nested_items, exports);
                }
            }
            _ => {}
        }
    }
}

fn validate_replica_legacy_ingest_exports(lib_path: &Path, source: &str) -> Result<(), String> {
    let syntax = syn::parse_file(source)
        .map_err(|error| format!("parse replica sync source {}: {error}", lib_path.display()))?;
    let ingest_modules = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "ingest" => Some(module),
            _ => None,
        })
        .collect::<Vec<_>>();
    if ingest_modules.len() != 1 {
        return Err(format!(
            "replica legacy ingest source {} must declare exactly one ingest module",
            lib_path.display()
        ));
    }
    let ingest_module = ingest_modules[0];
    if !matches!(&ingest_module.vis, syn::Visibility::Public(_)) {
        return Err(format!(
            "replica legacy ingest module in {} must remain public",
            lib_path.display()
        ));
    }
    if !has_exact_legacy_ingest_cfg(&ingest_module.attrs) {
        return Err(format!(
            "replica legacy ingest module in {} must be guarded by exact #[cfg(feature = \"legacy-ingest\")]",
            lib_path.display()
        ));
    }

    let mut ingest_exports = Vec::new();
    collect_public_replica_ingest_exports(&syntax.items, &mut ingest_exports);
    if ingest_exports.is_empty() {
        return Err(format!(
            "replica legacy ingest source {} must publicly re-export the ingest API",
            lib_path.display()
        ));
    }
    if ingest_exports
        .iter()
        .any(|export| !has_exact_legacy_ingest_cfg(&export.attrs))
    {
        return Err(format!(
            "every public replica ingest re-export in {} must be guarded by exact #[cfg(feature = \"legacy-ingest\")]",
            lib_path.display()
        ));
    }
    Ok(())
}

fn validate_replica_policy_source_witnesses(sync_root: &Path) -> Result<(), String> {
    let cargo_path = sync_root.join("Cargo.toml");
    let cargo_source = fs::read_to_string(&cargo_path)
        .map_err(|error| format!("read {}: {error}", cargo_path.display()))?;
    let cargo: toml::Value = toml::from_str(&cargo_source)
        .map_err(|error| format!("parse {}: {error}", cargo_path.display()))?;
    let features = cargo
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("replica sync {} must define features", cargo_path.display()))?;
    let default_features = features
        .get("default")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            format!(
                "replica sync {} must define default features",
                cargo_path.display()
            )
        })?;
    let mut pending_default_features = default_features
        .iter()
        .filter_map(toml::Value::as_str)
        .collect::<Vec<_>>();
    let mut visited_default_features = BTreeSet::new();
    while let Some(feature) = pending_default_features.pop() {
        if !visited_default_features.insert(feature) {
            continue;
        }
        if feature == "legacy-ingest" {
            return Err(format!(
                "replica legacy-ingest must not be enabled by default features in {}",
                cargo_path.display()
            ));
        }
        if let Some(members) = features.get(feature).and_then(toml::Value::as_array) {
            pending_default_features.extend(
                members
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .filter(|member| features.contains_key(*member)),
            );
        }
    }
    let legacy_features = features
        .get("legacy-ingest")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            format!(
                "replica sync {} must define the explicit legacy-ingest feature",
                cargo_path.display()
            )
        })?;
    if !legacy_features
        .iter()
        .filter_map(toml::Value::as_str)
        .any(|feature| feature == "std")
    {
        return Err(format!(
            "replica legacy-ingest feature in {} must enable std",
            cargo_path.display()
        ));
    }

    let lib_path = sync_root.join("src/lib.rs");
    let lib_source = fs::read_to_string(&lib_path)
        .map_err(|error| format!("read {}: {error}", lib_path.display()))?;
    validate_replica_legacy_ingest_exports(&lib_path, &lib_source)?;

    let types_path = sync_root.join("src/types.rs");
    let types_source = fs::read_to_string(&types_path)
        .map_err(|error| format!("read {}: {error}", types_path.display()))?;
    for type_name in [
        "RadrootsReplicaFarmSelector",
        "RadrootsReplicaSyncOptions",
        "RadrootsReplicaSyncRequest",
    ] {
        let witness = format!("#[serde(deny_unknown_fields)]\npub struct {type_name}");
        if !types_source.contains(&witness) {
            return Err(format!(
                "replica request type {type_name} must place #[serde(deny_unknown_fields)] immediately before its public struct declaration in {}",
                types_path.display()
            ));
        }
    }
    if types_source.contains("include_profiles") {
        return Err(format!(
            "retired replica request identifier include_profiles is forbidden in {}",
            types_path.display()
        ));
    }

    let emit_path = sync_root.join("src/emit.rs");
    let emit_source = fs::read_to_string(&emit_path)
        .map_err(|error| format!("read {}: {error}", emit_path.display()))?;
    let test_module_marker = "#[cfg(test)]\nmod tests {";
    let test_module_start = emit_source.rfind(test_module_marker).ok_or_else(|| {
        format!(
            "replica emit source {} must keep its bottom test module behind #[cfg(test)]",
            emit_path.display()
        )
    })?;
    let production_source = &emit_source[..test_module_start];
    if !production_source.contains("pub fn radroots_replica_sync_all_with_options(") {
        return Err(format!(
            "replica emit source {} is missing radroots_replica_sync_all_with_options",
            emit_path.display()
        ));
    }
    let production_code = production_source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for identifier in production_code
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|identifier| !identifier.is_empty())
    {
        if identifier.to_ascii_lowercase().contains("profile") {
            return Err(format!(
                "replica emit production source {} must not contain Profile-related identifier {identifier}",
                emit_path.display()
            ));
        }
    }
    let compact_production = production_code
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if compact_production.contains("kind:0") {
        return Err(format!(
            "replica emit production source {} must not construct a literal kind-0 event",
            emit_path.display()
        ));
    }

    Ok(())
}

fn validate_replica_contract(bundle: &ContractBundle, workspace_root: &Path) -> Result<(), String> {
    let replica = &bundle.replica;
    if replica.schema_version != 1 {
        return Err("replica contract schema_version must be 1".to_string());
    }
    if replica.contract.name != REPLICA_CONTRACT_NAME {
        return Err(format!(
            "replica contract name must be {REPLICA_CONTRACT_NAME}"
        ));
    }
    if replica.contract.version != bundle.manifest.contract.version {
        return Err(format!(
            "replica contract version {} must match manifest contract version {}",
            replica.contract.version, bundle.manifest.contract.version
        ));
    }
    if replica.contract.purpose.trim().is_empty() {
        return Err("replica contract purpose is required".to_string());
    }

    let manifest_family = bundle
        .manifest
        .surface
        .internal_replica_crates
        .as_ref()
        .ok_or_else(|| "surface.internal_replica_crates is required".to_string())?;
    for (field, actual, expected) in [
        (
            "schema",
            replica.crate_family.schema.as_str(),
            manifest_family.schema.as_str(),
        ),
        (
            "storage",
            replica.crate_family.storage.as_str(),
            manifest_family.storage.as_str(),
        ),
        (
            "sync",
            replica.crate_family.sync.as_str(),
            manifest_family.sync.as_str(),
        ),
    ] {
        validate_crate_identifier(actual, &format!("replica.crate_family.{field}"))?;
        if actual != expected {
            return Err(format!(
                "replica crate_family.{field} {actual} must match surface.internal_replica_crates.{field} {expected}"
            ));
        }
    }

    let package_manifests = workspace_package_manifests(workspace_root)?;
    for (field, crate_name) in [
        ("schema", replica.crate_family.schema.as_str()),
        ("storage", replica.crate_family.storage.as_str()),
        ("sync", replica.crate_family.sync.as_str()),
    ] {
        if !package_manifests.contains_key(crate_name) {
            return Err(format!(
                "replica crate_family.{field} {crate_name} must name a workspace package"
            ));
        }
    }

    let manifest_policy = bundle
        .manifest
        .policy
        .replica
        .as_ref()
        .ok_or_else(|| "policy.replica is required".to_string())?;
    let policy_parity = [
        (
            "transport_agnostic_sync_core",
            replica.policy.transport_agnostic_sync_core,
            manifest_policy.require_transport_agnostic_sync_contract,
        ),
        (
            "deterministic_emit_and_ingest",
            replica.policy.deterministic_emit_and_ingest,
            manifest_policy.require_deterministic_emit_ingest,
        ),
        (
            "forbid_legacy_alias_identifiers",
            replica.policy.forbid_legacy_alias_identifiers,
            manifest_policy.forbid_legacy_alias_identifiers,
        ),
    ];
    for (field, actual, expected) in policy_parity {
        if !actual || actual != expected {
            return Err(format!(
                "replica policy.{field} must be true and match manifest policy.replica"
            ));
        }
    }
    if replica.policy.profile_event_emission != "excluded" {
        return Err("replica policy.profile_event_emission must be excluded".to_string());
    }
    if replica.policy.unknown_sync_request_fields != "reject" {
        return Err("replica policy.unknown_sync_request_fields must be reject".to_string());
    }
    for (field, actual, expected) in [
        (
            "classified_listing_signature_verification",
            replica
                .policy
                .classified_listing_signature_verification
                .as_str(),
            "required_before_state",
        ),
        (
            "classified_listing_head_selection",
            replica.policy.classified_listing_head_selection.as_str(),
            "raw_before_profile",
        ),
        (
            "classified_listing_operational_projection",
            replica
                .policy
                .classified_listing_operational_projection
                .as_str(),
            "operational_partition_only",
        ),
        (
            "classified_listing_excluded_or_rejected_head",
            replica
                .policy
                .classified_listing_excluded_or_rejected_head
                .as_str(),
            "remove_projection_and_advance",
        ),
        (
            "classified_listing_head_only_ingest",
            replica.policy.classified_listing_head_only_ingest.as_str(),
            "reject_require_profile_aware",
        ),
        (
            "legacy_bare_envelope_ingest",
            replica.policy.legacy_bare_envelope_ingest.as_str(),
            "explicit_non_default_feature_only",
        ),
        (
            "legacy_ingest_feature",
            replica.policy.legacy_ingest_feature.as_str(),
            "legacy-ingest",
        ),
        (
            "phase_1_ingest_replacement",
            replica.policy.phase_1_ingest_replacement.as_str(),
            "none",
        ),
        (
            "future_product_ingest_input",
            replica.policy.future_product_ingest_input.as_str(),
            "store_produced_verified_valid_visible_admission",
        ),
    ] {
        if actual != expected {
            return Err(format!("replica policy.{field} must be {expected}"));
        }
    }

    if replica.transfer.version != REPLICA_TRANSFER_VERSION {
        return Err(format!(
            "replica transfer.version must be {REPLICA_TRANSFER_VERSION}"
        ));
    }
    if replica.transfer.constant != REPLICA_TRANSFER_CONSTANT {
        return Err(format!(
            "replica transfer.constant must be {REPLICA_TRANSFER_CONSTANT}"
        ));
    }
    let sync_manifest = package_manifests
        .get(&replica.crate_family.sync)
        .expect("replica sync workspace package was validated");
    let sync_root = sync_manifest
        .parent()
        .expect("workspace package manifest has a parent");
    validate_replica_policy_source_witnesses(sync_root)?;
    let expected_source = sync_root
        .strip_prefix(workspace_root)
        .expect("workspace package lives under the workspace root")
        .join("src/types.rs");
    if Path::new(&replica.transfer.source) != expected_source {
        return Err(format!(
            "replica transfer.source {} must be {}",
            replica.transfer.source,
            expected_source.display()
        ));
    }
    let source_path = workspace_root.join(&replica.transfer.source);
    let source_version = parse_replica_transfer_constant(&source_path, &replica.transfer.constant)?;
    if source_version != replica.transfer.version {
        return Err(format!(
            "replica transfer source constant {} value {} must match contract version {}",
            replica.transfer.constant, source_version, replica.transfer.version
        ));
    }

    Ok(())
}

fn validate_operations_contract(
    bundle: &ContractBundle,
    operations_manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    validate_conformance_schema(workspace_root)?;
    let conformance_root = conformance_root(workspace_root);
    if operations_manifest.contract.name.trim().is_empty() {
        return Err("operations contract name is required".to_string());
    }
    if operations_manifest.contract.version.trim().is_empty() {
        return Err("operations contract version is required".to_string());
    }
    if operations_manifest.contract.source.trim().is_empty() {
        return Err("operations contract source is required".to_string());
    }
    if operations_manifest.contract.name != bundle.manifest.contract.name {
        return Err("operations contract name must match manifest contract name".to_string());
    }
    if operations_manifest.contract.version != bundle.manifest.contract.version {
        return Err("operations contract version must match manifest contract version".to_string());
    }
    if operations_manifest.contract.source != bundle.manifest.contract.source {
        return Err("operations contract source must match manifest contract source".to_string());
    }

    let domains = collect_non_empty_set(&operations_manifest.public.domains, "public.domains")?;
    if domains.is_empty() {
        return Err("public.domains must not be empty".to_string());
    }
    let shared_types = collect_non_empty_set(
        &operations_manifest.shared_types.public,
        "shared_types.public",
    )?;
    if shared_types.is_empty() {
        return Err("shared_types.public must not be empty".to_string());
    }
    validate_no_retired_operation_event_names(
        &operations_manifest.shared_types.public,
        "shared_types.public",
    )?;
    let error_classes =
        collect_non_empty_set(&operations_manifest.errors.classes, "errors.classes")?;
    if error_classes.is_empty() {
        return Err("errors.classes must not be empty".to_string());
    }
    if operations_manifest.operations.is_empty() {
        return Err("operations map must not be empty".to_string());
    }

    if let Some(provenance) = &operations_manifest.implementation_provenance {
        let manifest_models = collect_unique_set(
            &bundle.manifest.surface.model_crates,
            "surface.model_crates",
        )?;
        let manifest_algorithms = collect_unique_set(
            &bundle.manifest.surface.algorithm_crates,
            "surface.algorithm_crates",
        )?;
        let provenance_models = collect_unique_set(
            &provenance.model_crates,
            "implementation_provenance.model_crates",
        )?;
        let provenance_algorithms = collect_unique_set(
            &provenance.algorithm_crates,
            "implementation_provenance.algorithm_crates",
        )?;
        if provenance_models != manifest_models || provenance_algorithms != manifest_algorithms {
            return Err(
                "operations implementation_provenance must match manifest surface crates"
                    .to_string(),
            );
        }
    }

    let mut operation_ids = BTreeSet::new();
    for (operation_key, operation) in &operations_manifest.operations {
        if operation_key.trim().is_empty() {
            return Err("operations map contains an empty key".to_string());
        }
        if operation.domain.trim().is_empty() {
            return Err(format!("operation {} domain is required", operation_key));
        }
        if !domains.contains(&operation.domain) {
            return Err(format!(
                "operation {} references unknown domain {}",
                operation_key, operation.domain
            ));
        }
        if operation.id.trim().is_empty() {
            return Err(format!("operation {} id is required", operation_key));
        }
        if !operation_ids.insert(operation.id.clone()) {
            return Err(format!("operations has duplicate id {}", operation.id));
        }
        if operation.stability.trim().is_empty() {
            return Err(format!("operation {} stability is required", operation.id));
        }
        if !operation.deterministic {
            return Err(format!(
                "operation {} deterministic must be true for the public contract",
                operation.id
            ));
        }
        if operation.inputs.is_empty() {
            return Err(format!(
                "operation {} inputs must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.inputs,
            &format!("operation {} inputs", operation.id),
        )?;
        validate_no_retired_operation_event_names(
            &operation.inputs,
            &format!("operation {} inputs", operation.id),
        )?;
        if operation.outputs.is_empty() {
            return Err(format!(
                "operation {} outputs must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.outputs,
            &format!("operation {} outputs", operation.id),
        )?;
        validate_no_retired_operation_event_names(
            &operation.outputs,
            &format!("operation {} outputs", operation.id),
        )?;
        if !error_classes.contains(&operation.error_class) {
            return Err(format!(
                "operation {} references unknown error class {}",
                operation.id, operation.error_class
            ));
        }
        if operation.signing.trim().is_empty() {
            return Err(format!("operation {} signing is required", operation.id));
        }
        if operation.transport.trim().is_empty() {
            return Err(format!("operation {} transport is required", operation.id));
        }
        if operation.implementation.rust_modules.is_empty() {
            return Err(format!(
                "operation {} implementation.rust_modules must not be empty",
                operation.id
            ));
        }
        let _ = collect_non_empty_set(
            &operation.implementation.rust_types,
            &format!("operation {} implementation.rust_types", operation.id),
        )?;
        validate_no_retired_operation_event_names(
            &operation.implementation.rust_types,
            &format!("operation {} implementation.rust_types", operation.id),
        )?;
        for rust_module in &operation.implementation.rust_modules {
            if rust_module.trim().is_empty() {
                return Err(format!(
                    "operation {} implementation.rust_modules contains an empty value",
                    operation.id
                ));
            }
            let path = workspace_root.join(rust_module);
            if !path.is_file() {
                return Err(format!(
                    "operation {} references missing rust module {}",
                    operation.id, rust_module
                ));
            }
        }
        if operation.conformance.vector.trim().is_empty() {
            return Err(format!(
                "operation {} conformance.vector is required",
                operation.id
            ));
        }
        if !operation
            .conformance
            .vector
            .starts_with("contracts/conformance/")
        {
            return Err(format!(
                "operation {} conformance.vector must live under contracts/conformance/",
                operation.id
            ));
        }
        let vector_path = workspace_root.join(&operation.conformance.vector);
        if !vector_path.starts_with(&conformance_root) {
            return Err(format!(
                "operation {} conformance.vector must resolve under {}",
                operation.id,
                conformance_root.display()
            ));
        }
        let vector =
            validate_conformance_vector_file(&vector_path, &operations_manifest.contract.version)?;
        validate_operation_case_kinds(operation, &vector)?;
    }

    Ok(())
}

fn validate_capsule_operation_authority(
    operations_manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &operations_manifest.shared_types.public,
        "shared_types.public",
    )?;
    validate_comment_operation_authority(operations_manifest, workspace_root)?;
    validate_deletion_operation_authority(operations_manifest, workspace_root)?;
    validate_admission_operation_authority(operations_manifest, workspace_root)?;
    validate_post_operation_authority(operations_manifest, workspace_root)?;
    validate_calendar_operation_authority(operations_manifest, &shared_types)?;
    validate_food_availability_operation_authority(operations_manifest, workspace_root)
}

fn validate_operation_case_kinds(
    operation: &PublicOperationContract,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    if operation.conformance.case_kinds.is_empty() {
        return Ok(());
    }
    let case_kinds = collect_non_empty_set(
        &operation.conformance.case_kinds,
        &format!("operation {} conformance.case_kinds", operation.id),
    )?;
    if case_kinds.len() != operation.conformance.case_kinds.len() {
        return Err(format!(
            "operation {} conformance.case_kinds must not contain duplicates",
            operation.id
        ));
    }
    let prefix = format!("{}.", operation.id);
    let vector_kinds = vector
        .vectors
        .iter()
        .map(|entry| entry.kind.as_str())
        .collect::<BTreeSet<_>>();
    for case_kind in case_kinds {
        if !case_kind.starts_with(&prefix) {
            return Err(format!(
                "operation {} conformance case kind {} must start with {}",
                operation.id, case_kind, prefix
            ));
        }
        if !vector_kinds.contains(case_kind.as_str()) {
            return Err(format!(
                "operation {} conformance case kind {} is absent from {}",
                operation.id, case_kind, operation.conformance.vector
            ));
        }
    }
    Ok(())
}

fn validate_post_operation_authority(
    manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let vector = validate_conformance_vector_file(
        &workspace_root.join(POST_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    validate_post_operation_inventory(manifest, &vector)
}

fn validate_post_operation_inventory(
    manifest: &OperationsContractManifest,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &manifest.shared_types.public,
        "post operation shared_types.public",
    )?;
    for required in REQUIRED_POST_PUBLIC_TYPES {
        if !shared_types.contains(required) {
            return Err(format!(
                "post operation authority requires shared public type {required}"
            ));
        }
    }

    let expected_keys = POST_OPERATION_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.key.to_string())
        .collect::<BTreeSet<_>>();
    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            operation.conformance.vector == POST_CONFORMANCE_VECTOR_RELATIVE
                || POST_OPERATION_KEY_PREFIXES
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
                || POST_OPERATION_ID_PREFIXES
                    .iter()
                    .any(|prefix| operation.id.starts_with(prefix))
        })
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        let missing = expected_keys
            .difference(&actual_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexpected = actual_keys
            .difference(&expected_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "post operation authority drift: missing {}; unexpected {}",
            join_set(&missing),
            join_set(&unexpected)
        ));
    }

    let mut owners = BTreeMap::new();
    for expected in POST_OPERATION_EXPECTATIONS {
        let operation = manifest
            .operations
            .get(expected.key)
            .ok_or_else(|| format!("post operation {} is required", expected.key))?;
        validate_post_operation_scalar(expected.key, "domain", &operation.domain, "social")?;
        validate_post_operation_scalar(expected.key, "id", &operation.id, expected.id)?;
        validate_post_operation_scalar(expected.key, "stability", &operation.stability, "beta")?;
        validate_post_operation_scalar(
            expected.key,
            "error_class",
            &operation.error_class,
            expected.error_class,
        )?;
        validate_post_operation_scalar(
            expected.key,
            "signing",
            &operation.signing,
            expected.signing,
        )?;
        validate_post_operation_scalar(expected.key, "transport", &operation.transport, "none")?;
        if !operation.deterministic {
            return Err(format!(
                "post operation {} deterministic drift: expected true, got false",
                expected.key
            ));
        }
        validate_post_operation_sequence(
            expected.key,
            "inputs",
            &operation.inputs,
            expected.inputs,
        )?;
        validate_post_operation_sequence(
            expected.key,
            "outputs",
            &operation.outputs,
            expected.outputs,
        )?;
        validate_post_operation_sequence(
            expected.key,
            "implementation.rust_modules",
            &operation.implementation.rust_modules,
            expected.rust_modules,
        )?;
        validate_post_operation_sequence(
            expected.key,
            "implementation.rust_types",
            &operation.implementation.rust_types,
            expected.rust_types,
        )?;
        validate_post_operation_scalar(
            expected.key,
            "conformance.vector",
            &operation.conformance.vector,
            POST_CONFORMANCE_VECTOR_RELATIVE,
        )?;
        validate_operation_case_kinds(operation, vector)?;
        if !operation
            .conformance
            .case_kinds
            .iter()
            .map(String::as_str)
            .eq(expected.case_kinds.iter().copied())
        {
            return Err(format!(
                "post operation {} conformance.case_kinds drift: expected {:?}, got {:?}",
                expected.key, expected.case_kinds, operation.conformance.case_kinds
            ));
        }
        for case_kind in &operation.conformance.case_kinds {
            if let Some(previous) = owners.insert(case_kind.as_str(), expected.key) {
                return Err(format!(
                    "post conformance case kind {case_kind} is multiply claimed by {previous} and {}",
                    expected.key
                ));
            }
        }
    }

    let mut actual_inventory = BTreeMap::new();
    for entry in &vector.vectors {
        if actual_inventory
            .insert(entry.id.as_str(), entry.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "post conformance vector inventory has duplicate id {}",
                entry.id
            ));
        }
    }
    for kind in actual_inventory.values() {
        if !owners.contains_key(kind) {
            return Err(format!(
                "post conformance vector kind {kind} is not claimed by exactly one operation"
            ));
        }
    }
    let expected_inventory = POST_VECTOR_EXPECTATIONS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    if actual_inventory != expected_inventory {
        return Err(format!(
            "post conformance vector inventory drift: expected {:?}, got {:?}",
            expected_inventory, actual_inventory
        ));
    }
    Ok(())
}

fn validate_post_operation_scalar(
    operation_key: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "post operation {operation_key} {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn validate_post_operation_sequence(
    operation_key: &str,
    field: &str,
    actual: &[String],
    expected: &[&str],
) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "post operation {operation_key} {field} drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_comment_operation_authority(
    manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let vector = validate_conformance_vector_file(
        &workspace_root.join(COMMENT_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    validate_comment_operation_inventory(manifest, &vector)
}

fn validate_comment_operation_inventory(
    manifest: &OperationsContractManifest,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &manifest.shared_types.public,
        "comment operation shared_types.public",
    )?;
    for required in REQUIRED_COMMENT_PUBLIC_TYPES {
        if !shared_types.contains(required) {
            return Err(format!(
                "comment operation authority requires shared public type {required}"
            ));
        }
    }

    let expected_keys = COMMENT_OPERATION_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.key.to_string())
        .collect::<BTreeSet<_>>();
    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            operation.conformance.vector == COMMENT_CONFORMANCE_VECTOR_RELATIVE
                || key.starts_with("social_comment_")
                || operation.id.starts_with("social.comment.")
        })
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        let missing = expected_keys
            .difference(&actual_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexpected = actual_keys
            .difference(&expected_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "comment operation authority drift: missing {}; unexpected {}",
            join_set(&missing),
            join_set(&unexpected)
        ));
    }

    let mut owners = BTreeMap::new();
    for expected in COMMENT_OPERATION_EXPECTATIONS {
        let operation = manifest
            .operations
            .get(expected.key)
            .ok_or_else(|| format!("comment operation {} is required", expected.key))?;
        validate_comment_operation_scalar(expected.key, "domain", &operation.domain, "social")?;
        validate_comment_operation_scalar(expected.key, "id", &operation.id, expected.id)?;
        validate_comment_operation_scalar(expected.key, "stability", &operation.stability, "beta")?;
        validate_comment_operation_scalar(
            expected.key,
            "error_class",
            &operation.error_class,
            expected.error_class,
        )?;
        validate_comment_operation_scalar(
            expected.key,
            "signing",
            &operation.signing,
            expected.signing,
        )?;
        validate_comment_operation_scalar(expected.key, "transport", &operation.transport, "none")?;
        if !operation.deterministic {
            return Err(format!(
                "comment operation {} deterministic drift: expected true, got false",
                expected.key
            ));
        }
        validate_comment_operation_sequence(
            expected.key,
            "inputs",
            &operation.inputs,
            expected.inputs,
        )?;
        validate_comment_operation_sequence(
            expected.key,
            "outputs",
            &operation.outputs,
            expected.outputs,
        )?;
        validate_comment_operation_sequence(
            expected.key,
            "implementation.rust_modules",
            &operation.implementation.rust_modules,
            expected.rust_modules,
        )?;
        validate_comment_operation_sequence(
            expected.key,
            "implementation.rust_types",
            &operation.implementation.rust_types,
            expected.rust_types,
        )?;
        validate_comment_operation_scalar(
            expected.key,
            "conformance.vector",
            &operation.conformance.vector,
            COMMENT_CONFORMANCE_VECTOR_RELATIVE,
        )?;
        validate_operation_case_kinds(operation, vector)?;
        if !operation
            .conformance
            .case_kinds
            .iter()
            .map(String::as_str)
            .eq(expected.case_kinds.iter().copied())
        {
            return Err(format!(
                "comment operation {} conformance.case_kinds drift: expected {:?}, got {:?}",
                expected.key, expected.case_kinds, operation.conformance.case_kinds
            ));
        }
        for case_kind in &operation.conformance.case_kinds {
            if let Some(previous) = owners.insert(case_kind.as_str(), expected.key) {
                return Err(format!(
                    "comment conformance case kind {case_kind} is multiply claimed by {previous} and {}",
                    expected.key
                ));
            }
        }
    }

    let expected_case_kinds = COMMENT_CASE_KINDS.into_iter().collect::<BTreeSet<_>>();
    let actual_case_kinds = owners.keys().copied().collect::<BTreeSet<_>>();
    if actual_case_kinds != expected_case_kinds {
        return Err(format!(
            "comment conformance case-kind authority drift: expected {:?}, got {:?}",
            expected_case_kinds, actual_case_kinds
        ));
    }

    let mut actual_inventory = BTreeMap::new();
    for entry in &vector.vectors {
        if actual_inventory
            .insert(entry.id.as_str(), entry.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "comment conformance vector inventory has duplicate id {}",
                entry.id
            ));
        }
        if !owners.contains_key(entry.kind.as_str()) {
            return Err(format!(
                "comment conformance vector kind {} is not claimed by exactly one operation",
                entry.kind
            ));
        }
    }
    let expected_inventory = COMMENT_VECTOR_EXPECTATIONS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    if actual_inventory != expected_inventory {
        return Err(format!(
            "comment conformance vector inventory drift: expected {:?}, got {:?}",
            expected_inventory, actual_inventory
        ));
    }

    Ok(())
}

fn validate_comment_operation_scalar(
    operation_key: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "comment operation {operation_key} {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn validate_comment_operation_sequence(
    operation_key: &str,
    field: &str,
    actual: &[String],
    expected: &[&str],
) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "comment operation {operation_key} {field} drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_deletion_operation_authority(
    manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let request_vector = validate_conformance_vector_file(
        &workspace_root.join(DELETION_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    let suppression_vector = validate_conformance_vector_file(
        &workspace_root.join(DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    validate_deletion_operation_inventory(manifest, &request_vector, &suppression_vector)
}

fn validate_deletion_operation_inventory(
    manifest: &OperationsContractManifest,
    request_vector: &ConformanceVectorFile,
    suppression_vector: &ConformanceVectorFile,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &manifest.shared_types.public,
        "deletion operation shared_types.public",
    )?;
    let expected_public_types = REQUIRED_DELETION_PUBLIC_TYPES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual_public_types = shared_types
        .iter()
        .map(String::as_str)
        .filter(|name| {
            name.contains("Nip09Deletion")
                || name.starts_with("RadrootsNip09")
                || name.starts_with("Nip01Coordinate")
                || matches!(
                    *name,
                    "Nip01EventWireParts" | "EventEnvelope" | "RadrootsSignatureVerifiedEvent"
                )
        })
        .collect::<BTreeSet<_>>();
    if actual_public_types != expected_public_types {
        return Err(format!(
            "deletion operation public-type authority drift: expected {:?}, got {:?}",
            expected_public_types, actual_public_types
        ));
    }

    let expected_keys = DELETION_OPERATION_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.key.to_string())
        .collect::<BTreeSet<_>>();
    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            operation.conformance.vector == DELETION_CONFORMANCE_VECTOR_RELATIVE
                || operation.conformance.vector == DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE
                || key.starts_with("social_deletion_request_")
                || operation.id.starts_with("social.deletion_request.")
        })
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        let missing = expected_keys
            .difference(&actual_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexpected = actual_keys
            .difference(&expected_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "deletion operation authority drift: missing {}; unexpected {}",
            join_set(&missing),
            join_set(&unexpected)
        ));
    }

    let mut owners = BTreeMap::new();
    for expected in DELETION_OPERATION_EXPECTATIONS {
        let vector = match expected.vector {
            DELETION_CONFORMANCE_VECTOR_RELATIVE => request_vector,
            DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE => suppression_vector,
            unexpected => {
                return Err(format!(
                    "deletion operation authority contains unsupported vector {unexpected}"
                ));
            }
        };
        let operation = manifest
            .operations
            .get(expected.key)
            .ok_or_else(|| format!("deletion operation {} is required", expected.key))?;
        validate_deletion_operation_scalar(expected.key, "domain", &operation.domain, "social")?;
        validate_deletion_operation_scalar(expected.key, "id", &operation.id, expected.id)?;
        validate_deletion_operation_scalar(
            expected.key,
            "stability",
            &operation.stability,
            "beta",
        )?;
        validate_deletion_operation_scalar(
            expected.key,
            "error_class",
            &operation.error_class,
            expected.error_class,
        )?;
        validate_deletion_operation_scalar(
            expected.key,
            "signing",
            &operation.signing,
            expected.signing,
        )?;
        validate_deletion_operation_scalar(
            expected.key,
            "transport",
            &operation.transport,
            "none",
        )?;
        if !operation.deterministic {
            return Err(format!(
                "deletion operation {} deterministic drift: expected true, got false",
                expected.key
            ));
        }
        validate_deletion_operation_sequence(
            expected.key,
            "inputs",
            &operation.inputs,
            expected.inputs,
        )?;
        validate_deletion_operation_sequence(
            expected.key,
            "outputs",
            &operation.outputs,
            expected.outputs,
        )?;
        validate_deletion_operation_sequence(
            expected.key,
            "implementation.rust_modules",
            &operation.implementation.rust_modules,
            expected.rust_modules,
        )?;
        validate_deletion_operation_sequence(
            expected.key,
            "implementation.rust_types",
            &operation.implementation.rust_types,
            expected.rust_types,
        )?;
        validate_deletion_operation_scalar(
            expected.key,
            "conformance.vector",
            &operation.conformance.vector,
            expected.vector,
        )?;
        validate_operation_case_kinds(operation, vector)?;
        if !operation
            .conformance
            .case_kinds
            .iter()
            .map(String::as_str)
            .eq(expected.case_kinds.iter().copied())
        {
            return Err(format!(
                "deletion operation {} conformance.case_kinds drift: expected {:?}, got {:?}",
                expected.key, expected.case_kinds, operation.conformance.case_kinds
            ));
        }
        for case_kind in &operation.conformance.case_kinds {
            if let Some(previous) = owners.insert(case_kind.as_str(), expected.key) {
                return Err(format!(
                    "deletion conformance case kind {case_kind} is multiply claimed by {previous} and {}",
                    expected.key
                ));
            }
        }
    }

    let expected_case_kinds = DELETION_CASE_KINDS.into_iter().collect::<BTreeSet<_>>();
    let actual_case_kinds = owners.keys().copied().collect::<BTreeSet<_>>();
    if actual_case_kinds != expected_case_kinds {
        return Err(format!(
            "deletion conformance case-kind authority drift: expected {:?}, got {:?}",
            expected_case_kinds, actual_case_kinds
        ));
    }

    let mut actual_inventory = BTreeMap::new();
    for entry in &request_vector.vectors {
        validate_deletion_vector_shape(entry)?;
        if actual_inventory
            .insert(entry.id.as_str(), entry.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "deletion conformance vector inventory has duplicate id {}",
                entry.id
            ));
        }
        if !entry.id.starts_with("nip09_") {
            return Err(format!(
                "deletion conformance vector id {} must use the nip09_ prefix",
                entry.id
            ));
        }
        if !owners.contains_key(entry.kind.as_str()) {
            return Err(format!(
                "deletion conformance vector kind {} is not claimed by exactly one operation",
                entry.kind
            ));
        }
    }
    let mut expected_inventory = BTreeMap::new();
    for (ids, kind) in [
        (
            DELETION_AUTHORED_VALID_IDS.as_slice(),
            "social.deletion_request.build_authored_draft.valid",
        ),
        (
            DELETION_AUTHORED_INVALID_IDS.as_slice(),
            "social.deletion_request.build_authored_draft.invalid",
        ),
        (
            DELETION_PROJECT_VALID_IDS.as_slice(),
            "social.deletion_request.project_verified_event.valid",
        ),
        (
            DELETION_PROJECT_INVALID_IDS.as_slice(),
            "social.deletion_request.project_verified_event.invalid",
        ),
        (
            DELETION_ADMIT_VALID_IDS.as_slice(),
            "social.deletion_request.verify_and_admit_event.valid",
        ),
        (
            DELETION_ADMIT_INVALID_IDS.as_slice(),
            "social.deletion_request.verify_and_admit_event.invalid",
        ),
    ] {
        for id in ids {
            if expected_inventory.insert(*id, kind).is_some() {
                return Err(format!(
                    "deletion authority contains duplicate expected vector id {id}"
                ));
            }
        }
    }
    if actual_inventory != expected_inventory {
        return Err(format!(
            "deletion conformance vector inventory drift: expected {:?}, got {:?}",
            expected_inventory, actual_inventory
        ));
    }

    validate_deletion_suppression_vector_inventory(suppression_vector, &owners)
}

fn validate_deletion_suppression_vector_inventory(
    vector: &ConformanceVectorFile,
    owners: &BTreeMap<&str, &str>,
) -> Result<(), String> {
    if vector.suite != "nip09_suppression_evaluator" {
        return Err(format!(
            "deletion suppression conformance suite drift: expected nip09_suppression_evaluator, got {}",
            vector.suite
        ));
    }

    let expected_kind = "social.deletion_request.evaluate_suppression.valid";
    if owners.get(expected_kind).copied() != Some("social_deletion_request_evaluate_suppression") {
        return Err(format!(
            "deletion suppression conformance kind {expected_kind} is not owned by the evaluator operation"
        ));
    }

    let expected_inventory = DELETION_SUPPRESSION_VALID_IDS
        .into_iter()
        .map(|id| (id, expected_kind))
        .collect::<BTreeMap<_, _>>();
    let mut actual_inventory = BTreeMap::new();
    for entry in &vector.vectors {
        validate_deletion_suppression_vector_shape(entry)?;
        if actual_inventory
            .insert(entry.id.as_str(), entry.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "deletion suppression conformance vector inventory has duplicate id {}",
                entry.id
            ));
        }
        if !entry.id.starts_with("nip09_suppress_") {
            return Err(format!(
                "deletion suppression conformance vector id {} must use the nip09_suppress_ prefix",
                entry.id
            ));
        }
        if entry.kind != expected_kind {
            return Err(format!(
                "deletion suppression conformance vector {} kind drift: expected {expected_kind}, got {}",
                entry.id, entry.kind
            ));
        }
    }
    if actual_inventory != expected_inventory {
        return Err(format!(
            "deletion suppression conformance vector inventory drift: expected {:?}, got {:?}",
            expected_inventory, actual_inventory
        ));
    }

    Ok(())
}

fn validate_deletion_suppression_vector_shape(
    entry: &ConformanceVectorEntry,
) -> Result<(), String> {
    validate_deletion_suppression_forbidden_material(&entry.input, &format!("{}.input", entry.id))?;
    let expected_value = entry.expected_value()?;
    validate_deletion_suppression_forbidden_material(
        expected_value,
        &format!("{}.expected", entry.id),
    )?;

    let input = deletion_object(&entry.input, &format!("{} input", entry.id))?;
    validate_deletion_object_keys(
        input,
        &format!("{} input", entry.id),
        &["request_event_jsons", "target_event_json"],
    )?;
    let target_event_json = input
        .get("target_event_json")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "deletion suppression vector {} input.target_event_json must be a string",
                entry.id
            )
        })?;
    validate_deletion_suppression_fixed_event(
        target_event_json,
        &format!("{} target_event_json", entry.id),
        None,
    )?;

    let request_event_jsons = input
        .get("request_event_jsons")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "deletion suppression vector {} input.request_event_jsons must be an array",
                entry.id
            )
        })?;
    let mut request_ids = BTreeSet::new();
    for (index, value) in request_event_jsons.iter().enumerate() {
        let event_json = value.as_str().ok_or_else(|| {
            format!(
                "deletion suppression vector {} input.request_event_jsons[{index}] must be a string",
                entry.id
            )
        })?;
        let event = validate_deletion_suppression_fixed_event(
            event_json,
            &format!("{} request_event_jsons[{index}]", entry.id),
            Some(5),
        )?;
        request_ids.insert(event.id);
    }

    let expected = deletion_object(expected_value, &format!("{} expected", entry.id))?;
    validate_deletion_object_keys(
        expected,
        &format!("{} expected", entry.id),
        &["address_reference", "event_reference", "outcome", "reason"],
    )?;
    let outcome = expected
        .get("outcome")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "deletion suppression vector {} expected.outcome must be a string",
                entry.id
            )
        })?;
    if !matches!(outcome, "visible" | "suppressed") {
        return Err(format!(
            "deletion suppression vector {} expected.outcome is unsupported: {outcome}",
            entry.id
        ));
    }
    let reason = expected
        .get("reason")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "deletion suppression vector {} expected.reason must be a string",
                entry.id
            )
        })?;
    if !matches!(
        reason,
        "deletion_request_immune"
            | "deletion_no_authorized_reference"
            | "deletion_request_author_mismatch"
            | "deletion_address_cutoff_precedes_target"
            | "deletion_event_id_reference"
            | "deletion_address_reference"
            | "deletion_event_id_and_address_reference"
    ) {
        return Err(format!(
            "deletion suppression vector {} expected.reason is unsupported: {reason}",
            entry.id
        ));
    }

    let event_reference = validate_deletion_suppression_event_reference(
        expected
            .get("event_reference")
            .expect("exact expected keys contain event_reference"),
        &entry.id,
        &request_ids,
    )?;
    let address_reference = validate_deletion_suppression_address_reference(
        expected
            .get("address_reference")
            .expect("exact expected keys contain address_reference"),
        &entry.id,
        &request_ids,
    )?;
    let decision_shape_matches = match reason {
        "deletion_request_immune"
        | "deletion_no_authorized_reference"
        | "deletion_request_author_mismatch" => {
            outcome == "visible" && !event_reference && !address_reference
        }
        "deletion_address_cutoff_precedes_target" => {
            outcome == "visible" && !event_reference && address_reference
        }
        "deletion_address_reference" => {
            outcome == "suppressed" && !event_reference && address_reference
        }
        "deletion_event_id_and_address_reference" => {
            outcome == "suppressed" && event_reference && address_reference
        }
        "deletion_event_id_reference" => outcome == "suppressed" && event_reference,
        _ => unreachable!("supported reason matched above"),
    };
    if !decision_shape_matches {
        return Err(format!(
            "deletion suppression vector {} expected decision shape is inconsistent with reason {reason}",
            entry.id
        ));
    }

    Ok(())
}

fn validate_deletion_suppression_fixed_event(
    event_json: &str,
    label: &str,
    expected_kind: Option<u32>,
) -> Result<DeletionConformanceRawEvent, String> {
    if contains_nsec_material(event_json) {
        return Err(format!(
            "deletion suppression vector {label} contains forbidden nsec material"
        ));
    }
    if contains_approved_fixture_secret(event_json) {
        return Err(format!(
            "deletion suppression vector {label} contains forbidden approved fixture secret material"
        ));
    }
    let raw = serde_json::from_str::<DeletionConformanceRawEvent>(event_json).map_err(|error| {
        format!("deletion suppression vector {label} has invalid fixed event shape: {error}")
    })?;
    if expected_kind.is_some_and(|kind| raw.kind != kind) {
        return Err(format!(
            "deletion suppression vector {label} must contain a kind-5 deletion request"
        ));
    }
    let canonical = serde_json::to_string(&raw).map_err(|error| {
        format!("deletion suppression vector {label} cannot be reserialized: {error}")
    })?;
    if canonical != event_json {
        return Err(format!(
            "deletion suppression vector {label} must be compact canonical JSON"
        ));
    }
    Ok(raw)
}

fn validate_deletion_suppression_event_reference(
    value: &Value,
    id: &str,
    request_ids: &BTreeSet<String>,
) -> Result<bool, String> {
    if value.is_null() {
        return Ok(false);
    }
    let reference = deletion_object(value, &format!("{id} expected.event_reference"))?;
    validate_deletion_object_keys(
        reference,
        &format!("{id} expected.event_reference"),
        &["request_id"],
    )?;
    validate_deletion_suppression_request_id(
        reference.get("request_id"),
        id,
        "event_reference.request_id",
        request_ids,
    )?;
    Ok(true)
}

fn validate_deletion_suppression_address_reference(
    value: &Value,
    id: &str,
    request_ids: &BTreeSet<String>,
) -> Result<bool, String> {
    if value.is_null() {
        return Ok(false);
    }
    let reference = deletion_object(value, &format!("{id} expected.address_reference"))?;
    validate_deletion_object_keys(
        reference,
        &format!("{id} expected.address_reference"),
        &["coordinate", "inclusive_cutoff", "request_id"],
    )?;
    let coordinate = reference
        .get("coordinate")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "deletion suppression vector {id} expected.address_reference.coordinate must be a string"
            )
        })?;
    validate_deletion_suppression_coordinate(coordinate, id)?;
    if !reference
        .get("inclusive_cutoff")
        .is_some_and(|value| value.as_u64().is_some())
    {
        return Err(format!(
            "deletion suppression vector {id} expected.address_reference.inclusive_cutoff must be an unsigned integer"
        ));
    }
    validate_deletion_suppression_request_id(
        reference.get("request_id"),
        id,
        "address_reference.request_id",
        request_ids,
    )?;
    Ok(true)
}

fn validate_deletion_suppression_request_id(
    value: Option<&Value>,
    id: &str,
    field: &str,
    request_ids: &BTreeSet<String>,
) -> Result<(), String> {
    let request_id = value.and_then(Value::as_str).ok_or_else(|| {
        format!("deletion suppression vector {id} expected.{field} must be a string")
    })?;
    if request_id.len() != 64
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "deletion suppression vector {id} expected.{field} must be lowercase 64-character hex"
        ));
    }
    if !request_ids.contains(request_id) {
        return Err(format!(
            "deletion suppression vector {id} expected.{field} must identify an input request"
        ));
    }
    Ok(())
}

fn validate_deletion_suppression_coordinate(coordinate: &str, id: &str) -> Result<(), String> {
    let mut parts = coordinate.splitn(3, ':');
    let kind_text = parts.next().unwrap_or_default();
    let pubkey = parts.next().unwrap_or_default();
    let identifier = parts.next().ok_or_else(|| {
        format!(
            "deletion suppression vector {id} expected.address_reference.coordinate has invalid format"
        )
    })?;
    let kind = kind_text.parse::<u32>().map_err(|_| {
        format!(
            "deletion suppression vector {id} expected.address_reference.coordinate kind is invalid"
        )
    })?;
    if kind.to_string() != kind_text
        || pubkey.len() != 64
        || !pubkey
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || (!matches!(kind, 0 | 3)
            && !(10_000..=19_999).contains(&kind)
            && !(30_000..=39_999).contains(&kind))
        || ((matches!(kind, 0 | 3) || (10_000..=19_999).contains(&kind)) && !identifier.is_empty())
    {
        return Err(format!(
            "deletion suppression vector {id} expected.address_reference.coordinate is not canonical"
        ));
    }
    Ok(())
}

fn validate_deletion_suppression_forbidden_material(
    value: &Value,
    path: &str,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let normalized = key.to_ascii_lowercase();
                if matches!(normalized.as_str(), "base" | "mutation")
                    || normalized.contains("seed")
                    || normalized.contains("generator")
                    || normalized.contains("recipe")
                    || normalized.contains("secret_key")
                    || normalized.contains("private_key")
                    || normalized.contains("signing_key")
                    || normalized.contains("boundary")
                {
                    return Err(format!(
                        "deletion suppression vector contains forbidden metadata key {path}.{key}"
                    ));
                }
                validate_deletion_suppression_forbidden_material(child, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_deletion_suppression_forbidden_material(
                    child,
                    &format!("{path}[{index}]"),
                )?;
            }
        }
        Value::String(string) => {
            if contains_nsec_material(string) {
                return Err(format!(
                    "deletion suppression vector contains forbidden nsec material at {path}"
                ));
            }
            if contains_approved_fixture_secret(string) {
                return Err(format!(
                    "deletion suppression vector contains forbidden approved fixture secret material at {path}"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_deletion_vector_shape(entry: &ConformanceVectorEntry) -> Result<(), String> {
    validate_deletion_forbidden_metadata(&entry.input, &format!("{}.input", entry.id))?;
    let expected_value = entry.expected_value()?;
    validate_deletion_forbidden_metadata(expected_value, &format!("{}.expected", entry.id))?;

    let input = deletion_object(&entry.input, &format!("{} input", entry.id))?;
    let is_authored = entry
        .kind
        .starts_with("social.deletion_request.build_authored_draft.");
    let is_valid = entry.kind.ends_with(".valid");
    if is_authored {
        validate_deletion_object_keys(
            input,
            &format!("{} input", entry.id),
            &["address_targets", "content", "event_targets"],
        )?;
        if !input.get("content").is_some_and(Value::is_string) {
            return Err(format!(
                "deletion vector {} input.content must be a string",
                entry.id
            ));
        }
        let event_targets = input
            .get("event_targets")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                format!(
                    "deletion vector {} input.event_targets must be an array",
                    entry.id
                )
            })?;
        for (index, target) in event_targets.iter().enumerate() {
            let target = deletion_object(
                target,
                &format!("{} input.event_targets[{index}]", entry.id),
            )?;
            validate_deletion_object_keys(
                target,
                &format!("{} input.event_targets[{index}]", entry.id),
                &["event_id", "kind"],
            )?;
            if !target.get("event_id").is_some_and(Value::is_string)
                || !target.get("kind").is_some_and(Value::is_u64)
            {
                return Err(format!(
                    "deletion vector {} input.event_targets[{index}] must contain string event_id and unsigned kind",
                    entry.id
                ));
            }
        }
        let address_targets = input
            .get("address_targets")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                format!(
                    "deletion vector {} input.address_targets must be an array",
                    entry.id
                )
            })?;
        if !address_targets.iter().all(Value::is_string) {
            return Err(format!(
                "deletion vector {} input.address_targets must contain only strings",
                entry.id
            ));
        }
    } else {
        validate_deletion_object_keys(input, &format!("{} input", entry.id), &["event_json"])?;
        let event_json = input
            .get("event_json")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "deletion vector {} input.event_json must be a string",
                    entry.id
                )
            })?;
        if contains_nsec_material(event_json) {
            return Err(format!(
                "deletion vector {} input.event_json contains forbidden nsec material",
                entry.id
            ));
        }
        let raw =
            serde_json::from_str::<DeletionConformanceRawEvent>(event_json).map_err(|error| {
                format!(
                    "deletion vector {} input.event_json has invalid fixed event shape: {error}",
                    entry.id
                )
            })?;
        let canonical = serde_json::to_string(&raw).map_err(|error| {
            format!(
                "deletion vector {} input.event_json cannot be reserialized: {error}",
                entry.id
            )
        })?;
        if canonical != event_json {
            return Err(format!(
                "deletion vector {} input.event_json must be compact canonical JSON",
                entry.id
            ));
        }
    }

    let expected = deletion_object(expected_value, &format!("{} expected", entry.id))?;
    if !is_valid {
        validate_deletion_object_keys(expected, &format!("{} expected", entry.id), &["error"])?;
        if !expected
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| !error.is_empty())
        {
            return Err(format!(
                "deletion vector {} expected.error must be a non-empty string",
                entry.id
            ));
        }
    } else if is_authored {
        validate_deletion_object_keys(
            expected,
            &format!("{} expected", entry.id),
            &["content", "kind", "tags"],
        )?;
        if expected.get("kind").and_then(Value::as_u64) != Some(5)
            || !expected.get("content").is_some_and(Value::is_string)
            || !expected.get("tags").is_some_and(Value::is_array)
        {
            return Err(format!(
                "deletion vector {} authored expected output must contain kind 5, string content, and tags",
                entry.id
            ));
        }
    } else {
        validate_deletion_object_keys(
            expected,
            &format!("{} expected", entry.id),
            &[
                "address_targets",
                "contract_id",
                "diagnostics",
                "event_targets",
                "kind_advisories",
                "raw_tags",
            ],
        )?;
        if expected.get("contract_id").and_then(Value::as_str)
            != Some("radroots.social.deletion_request.v1")
        {
            return Err(format!(
                "deletion vector {} expected.contract_id drifted",
                entry.id
            ));
        }
        for field in [
            "address_targets",
            "diagnostics",
            "event_targets",
            "kind_advisories",
            "raw_tags",
        ] {
            if !expected.get(field).is_some_and(Value::is_array) {
                return Err(format!(
                    "deletion vector {} expected.{field} must be an array",
                    entry.id
                ));
            }
        }
    }

    Ok(())
}

const APPROVED_FIXTURE_SECRET_TEXT_SHA256: [&str; 4] = [
    "abd5b64bb0a9a0b9b2e928edb278d0f4d442d16e620ac56570c354a040f4e01a",
    "619b2fc89e98c17205800071802b3f06e12b05b79401da800b8b13aa8597d240",
    "82e759d54455fbfa5b9c58367b37f1fbc3d54becc097dcdf71fa48ee0af6b2a6",
    "510b06b0d391a517860b8e406cdc827b0481f2e449c9817b73adad07f4ff02a7",
];

fn validate_deletion_forbidden_metadata(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_deletion_forbidden_metadata_key(key) {
                    return Err(format!(
                        "deletion vector contains forbidden metadata key {path}.{key}"
                    ));
                }
                validate_deletion_forbidden_metadata(child, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_deletion_forbidden_metadata(child, &format!("{path}[{index}]"))?;
            }
        }
        Value::String(string) => {
            if contains_nsec_material(string) {
                return Err(format!(
                    "deletion vector contains forbidden nsec material at {path}"
                ));
            }
            if contains_approved_fixture_secret(string) {
                return Err(format!(
                    "deletion vector contains forbidden approved fixture secret material at {path}"
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_deletion_forbidden_metadata_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    matches!(normalized.as_str(), "base" | "mutation")
        || normalized.contains("seed")
        || normalized.contains("generator")
        || normalized.contains("recipe")
        || normalized.contains("secret_key")
        || normalized.contains("private_key")
        || normalized.contains("signing_key")
        || normalized.contains("boundary")
        || normalized.contains("authorization")
        || normalized.contains("authorized")
        || normalized.contains("cutoff")
        || normalized.contains("evaluator")
        || normalized.contains("store_mutation")
        || normalized.contains("suppression")
        || normalized.contains("suppressed")
        || normalized == "effect"
        || normalized.ends_with("_effect")
        || normalized == "effects"
}

fn contains_nsec_material(value: &str) -> bool {
    value.to_ascii_lowercase().contains("nsec1")
}

fn contains_approved_fixture_secret(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .as_bytes()
        .windows(64)
        .filter(|window| window.iter().all(u8::is_ascii_hexdigit))
        .any(|window| {
            let digest = hex::encode(Sha256::digest(window));
            APPROVED_FIXTURE_SECRET_TEXT_SHA256.contains(&digest.as_str())
        })
}

fn deletion_object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("deletion vector {label} must be an object"))
}

fn validate_deletion_object_keys(
    object: &serde_json::Map<String, Value>,
    label: &str,
    expected: &[&str],
) -> Result<(), String> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "deletion vector {label} keys drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_deletion_operation_scalar(
    operation_key: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "deletion operation {operation_key} {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn validate_deletion_operation_sequence(
    operation_key: &str,
    field: &str,
    actual: &[String],
    expected: &[&str],
) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "deletion operation {operation_key} {field} drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_food_availability_operation_authority(
    manifest: &OperationsContractManifest,
    workspace_root: &Path,
) -> Result<(), String> {
    let vector = validate_conformance_vector_file(
        &workspace_root.join(FOOD_AVAILABILITY_CONFORMANCE_VECTOR_RELATIVE),
        &manifest.contract.version,
    )?;
    validate_food_availability_operation_inventory(manifest, &vector)
}

fn validate_food_availability_operation_inventory(
    manifest: &OperationsContractManifest,
    vector: &ConformanceVectorFile,
) -> Result<(), String> {
    let shared_types = collect_non_empty_set(
        &manifest.shared_types.public,
        "food availability operation shared_types.public",
    )?;
    for required in REQUIRED_FOOD_AVAILABILITY_PUBLIC_TYPES {
        if !shared_types.contains(required) {
            return Err(format!(
                "food availability operation authority requires shared public type {required}"
            ));
        }
    }

    let expected_keys = FOOD_AVAILABILITY_OPERATION_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.key.to_string())
        .collect::<BTreeSet<_>>();
    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            operation.domain == "food_availability"
                || operation.conformance.vector == FOOD_AVAILABILITY_CONFORMANCE_VECTOR_RELATIVE
                || key.starts_with("food_availability_")
                || operation.id.starts_with("food_availability.")
        })
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        let missing = expected_keys
            .difference(&actual_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexpected = actual_keys
            .difference(&expected_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "food availability operation authority drift: missing {}; unexpected {}",
            join_set(&missing),
            join_set(&unexpected)
        ));
    }

    let mut owners = BTreeMap::new();
    for expected in FOOD_AVAILABILITY_OPERATION_EXPECTATIONS {
        let operation = manifest
            .operations
            .get(expected.key)
            .ok_or_else(|| format!("food availability operation {} is required", expected.key))?;
        validate_food_availability_operation_scalar(
            expected.key,
            "domain",
            &operation.domain,
            "food_availability",
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "id",
            &operation.id,
            expected.id,
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "stability",
            &operation.stability,
            "beta",
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "error_class",
            &operation.error_class,
            expected.error_class,
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "signing",
            &operation.signing,
            expected.signing,
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "transport",
            &operation.transport,
            "none",
        )?;
        if !operation.deterministic {
            return Err(format!(
                "food availability operation {} deterministic drift: expected true, got false",
                expected.key
            ));
        }
        validate_food_availability_operation_sequence(
            expected.key,
            "inputs",
            &operation.inputs,
            expected.inputs,
        )?;
        validate_food_availability_operation_sequence(
            expected.key,
            "outputs",
            &operation.outputs,
            expected.outputs,
        )?;
        validate_food_availability_operation_sequence(
            expected.key,
            "implementation.rust_modules",
            &operation.implementation.rust_modules,
            expected.rust_modules,
        )?;
        validate_food_availability_operation_sequence(
            expected.key,
            "implementation.rust_types",
            &operation.implementation.rust_types,
            expected.rust_types,
        )?;
        validate_food_availability_operation_scalar(
            expected.key,
            "conformance.vector",
            &operation.conformance.vector,
            FOOD_AVAILABILITY_CONFORMANCE_VECTOR_RELATIVE,
        )?;
        validate_operation_case_kinds(operation, vector)?;
        if !operation
            .conformance
            .case_kinds
            .iter()
            .map(String::as_str)
            .eq(expected.case_kinds.iter().copied())
        {
            return Err(format!(
                "food availability operation {} conformance.case_kinds drift: expected {:?}, got {:?}",
                expected.key, expected.case_kinds, operation.conformance.case_kinds
            ));
        }
        for case_kind in &operation.conformance.case_kinds {
            if let Some(previous) = owners.insert(case_kind.as_str(), expected.key) {
                return Err(format!(
                    "food availability conformance case kind {case_kind} is multiply claimed by {previous} and {}",
                    expected.key
                ));
            }
        }
    }

    let expected_case_kinds = FOOD_AVAILABILITY_CASE_KINDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual_case_kinds = owners.keys().copied().collect::<BTreeSet<_>>();
    if actual_case_kinds != expected_case_kinds {
        return Err(format!(
            "food availability conformance case-kind authority drift: expected {:?}, got {:?}",
            expected_case_kinds, actual_case_kinds
        ));
    }

    let mut actual_inventory = BTreeMap::new();
    for entry in &vector.vectors {
        if actual_inventory
            .insert(entry.id.as_str(), entry.kind.as_str())
            .is_some()
        {
            return Err(format!(
                "food availability conformance vector inventory has duplicate id {}",
                entry.id
            ));
        }
    }
    for kind in actual_inventory.values() {
        if !owners.contains_key(kind) {
            return Err(format!(
                "food availability conformance vector kind {kind} is not claimed by exactly one operation"
            ));
        }
    }
    let expected_inventory = FOOD_AVAILABILITY_VECTOR_EXPECTATIONS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    if actual_inventory != expected_inventory {
        return Err(format!(
            "food availability conformance vector inventory drift: expected {:?}, got {:?}",
            expected_inventory, actual_inventory
        ));
    }
    Ok(())
}

fn validate_food_availability_operation_scalar(
    operation_key: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "food availability operation {operation_key} {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn validate_food_availability_operation_sequence(
    operation_key: &str,
    field: &str,
    actual: &[String],
    expected: &[&str],
) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "food availability operation {operation_key} {field} drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_calendar_operation_authority(
    manifest: &OperationsContractManifest,
    shared_types: &BTreeSet<String>,
) -> Result<(), String> {
    for required in REQUIRED_CALENDAR_PUBLIC_TYPES {
        if !shared_types.contains(required) {
            return Err(format!(
                "calendar operation authority requires shared public type {required}"
            ));
        }
    }

    let expected_keys = CALENDAR_OPERATION_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.key.to_string())
        .collect::<BTreeSet<_>>();
    let actual_keys = manifest
        .operations
        .iter()
        .filter(|(key, operation)| {
            key.starts_with("social_calendar_") || operation.id.starts_with("social.calendar")
        })
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        let missing = expected_keys
            .difference(&actual_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexpected = actual_keys
            .difference(&expected_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "calendar operation authority drift: missing {}; unexpected {}",
            join_set(&missing),
            join_set(&unexpected)
        ));
    }

    for expected in CALENDAR_OPERATION_EXPECTATIONS {
        let operation = manifest
            .operations
            .get(expected.key)
            .ok_or_else(|| format!("calendar operation {} is required", expected.key))?;
        validate_calendar_operation_scalar(expected.key, "domain", &operation.domain, "social")?;
        validate_calendar_operation_scalar(expected.key, "id", &operation.id, expected.id)?;
        validate_calendar_operation_scalar(
            expected.key,
            "stability",
            &operation.stability,
            "beta",
        )?;
        validate_calendar_operation_scalar(
            expected.key,
            "error_class",
            &operation.error_class,
            expected.error_class,
        )?;
        validate_calendar_operation_scalar(expected.key, "signing", &operation.signing, "none")?;
        validate_calendar_operation_scalar(
            expected.key,
            "transport",
            &operation.transport,
            "none",
        )?;
        if !operation.deterministic {
            return Err(format!(
                "calendar operation {} deterministic drift: expected true, got false",
                expected.key
            ));
        }
        validate_calendar_operation_sequence(
            expected.key,
            "inputs",
            &operation.inputs,
            expected.inputs,
        )?;
        validate_calendar_operation_sequence(
            expected.key,
            "outputs",
            &operation.outputs,
            expected.outputs,
        )?;
        validate_calendar_operation_sequence(
            expected.key,
            "implementation.rust_modules",
            &operation.implementation.rust_modules,
            expected.rust_modules,
        )?;
        validate_calendar_operation_sequence(
            expected.key,
            "implementation.rust_types",
            &operation.implementation.rust_types,
            expected.rust_types,
        )?;
        validate_calendar_operation_scalar(
            expected.key,
            "conformance.vector",
            &operation.conformance.vector,
            expected.vector,
        )?;
    }

    Ok(())
}

fn validate_calendar_operation_scalar(
    operation_key: &str,
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "calendar operation {operation_key} {field} drift: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn validate_calendar_operation_sequence(
    operation_key: &str,
    field: &str,
    actual: &[String],
    expected: &[&str],
) -> Result<(), String> {
    if !actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Err(format!(
            "calendar operation {operation_key} {field} drift: expected {:?}, got {:?}",
            expected, actual
        ));
    }
    Ok(())
}

fn validate_no_retired_operation_event_names(
    values: &[String],
    context: &str,
) -> Result<(), String> {
    for value in values {
        for retired in RETIRED_OPERATION_EVENT_NAMES {
            if value == retired || value.ends_with(&format!("::{retired}")) {
                return Err(format!(
                    "{context} uses retired event type {retired}; use target-state event and wire names"
                ));
            }
        }
    }
    Ok(())
}

fn package_field_configured(table: &toml::value::Table, field: &str) -> bool {
    let Some(value) = table.get(field) else {
        return false;
    };
    match value {
        toml::Value::String(raw) => !raw.trim().is_empty(),
        toml::Value::Array(values) => !values.is_empty(),
        toml::Value::Table(inner) => inner
            .get("workspace")
            .and_then(toml::Value::as_bool)
            .is_some_and(|configured| configured),
        _ => false,
    }
}

fn package_string_array<'a>(
    package: &'a toml::value::Table,
    crate_name: &str,
    field: &str,
) -> Result<Vec<&'a str>, String> {
    let values = package
        .get(field)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("publish crate {crate_name} must define package.{field}"))?;
    if values.is_empty() {
        return Err(format!(
            "publish crate {crate_name} package.{field} must not be empty"
        ));
    }
    let mut resolved = Vec::with_capacity(values.len());
    let mut unique = BTreeSet::new();
    for value in values {
        let value = value.as_str().ok_or_else(|| {
            format!("publish crate {crate_name} package.{field} entries must be strings")
        })?;
        if value.trim().is_empty() {
            return Err(format!(
                "publish crate {crate_name} package.{field} entries must not be empty"
            ));
        }
        if !unique.insert(value) {
            return Err(format!(
                "publish crate {crate_name} package.{field} contains duplicate {value}"
            ));
        }
        resolved.push(value);
    }
    Ok(resolved)
}

fn validate_package_file_matches(
    workspace_root: &Path,
    package_root: &Path,
    crate_name: &str,
    relative: &str,
) -> Result<(), String> {
    let package_path = package_root.join(relative);
    let root_path = workspace_root.join(relative);
    let package_bytes = fs::read(&package_path)
        .map_err(|error| format!("publish crate {crate_name} must include {relative}: {error}"))?;
    let root_bytes =
        fs::read(&root_path).map_err(|error| format!("read {}: {error}", root_path.display()))?;
    if package_bytes != root_bytes {
        return Err(format!(
            "publish crate {crate_name} {relative} must match the workspace license"
        ));
    }
    Ok(())
}

fn validate_publish_package_metadata(
    workspace_root: &Path,
    publish_crates: &BTreeSet<String>,
) -> Result<(), String> {
    let mut package_records = BTreeMap::new();
    for record in workspace_package_records(workspace_root)? {
        if package_records
            .insert(record.name.clone(), record)
            .is_some()
        {
            return Err("duplicate workspace package name in package metadata map".to_string());
        }
    }
    for crate_name in publish_crates {
        let record = match package_records.get(crate_name) {
            Some(record) => record,
            None => {
                return Err(format!(
                    "publish crate {} has no workspace manifest",
                    crate_name
                ));
            }
        };
        let package = record
            .manifest_value
            .get("package")
            .and_then(toml::Value::as_table)
            .expect("workspace package records include [package] table");

        if !package_field_configured(package, "description") {
            return Err(format!(
                "publish crate {} must define a non-empty package.description",
                crate_name
            ));
        }
        for field in [
            "authors",
            "version",
            "edition",
            "rust-version",
            "license",
            "repository",
            "homepage",
            "documentation",
            "readme",
        ] {
            if !package_field_configured(package, field) {
                return Err(format!(
                    "publish crate {} must configure package.{}",
                    crate_name, field
                ));
            }
        }

        let expected_documentation = format!("https://docs.rs/{crate_name}");
        if package.get("documentation").and_then(toml::Value::as_str)
            != Some(expected_documentation.as_str())
        {
            return Err(format!(
                "publish crate {crate_name} package.documentation must be {expected_documentation}"
            ));
        }
        if package.get("license-file").is_some() {
            return Err(format!(
                "publish crate {crate_name} must use the workspace SPDX license expression"
            ));
        }

        let keywords = package_string_array(package, crate_name, "keywords")?;
        if keywords.len() > 5 {
            return Err(format!(
                "publish crate {crate_name} package.keywords exceeds the crates.io limit of 5"
            ));
        }
        let categories = package_string_array(package, crate_name, "categories")?;
        if categories.len() > 5 {
            return Err(format!(
                "publish crate {crate_name} package.categories exceeds the crates.io limit of 5"
            ));
        }

        let include = package_string_array(package, crate_name, "include")?;
        for required in [
            "src/**",
            "tests/**",
            "README.md",
            "LICENSE-APACHE",
            "LICENSE-MIT",
        ] {
            if !include.contains(&required) {
                return Err(format!(
                    "publish crate {crate_name} package.include must contain {required}"
                ));
            }
        }

        let package_root = record
            .manifest_path
            .parent()
            .expect("workspace member manifest has a parent");
        if !package_root.join("README.md").is_file() {
            return Err(format!(
                "publish crate {crate_name} must include a package-local README.md"
            ));
        }
        validate_package_file_matches(workspace_root, package_root, crate_name, "LICENSE-APACHE")?;
        validate_package_file_matches(workspace_root, package_root, crate_name, "LICENSE-MIT")?;

        let docs_rs = package
            .get("metadata")
            .and_then(toml::Value::as_table)
            .and_then(|metadata| metadata.get("docs"))
            .and_then(toml::Value::as_table)
            .and_then(|docs| docs.get("rs"))
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                format!("publish crate {crate_name} must define [package.metadata.docs.rs]")
            })?;
        if docs_rs
            .get("all-features")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!(
                "publish crate {crate_name} docs.rs must use an intentional feature set"
            ));
        }
        let docs_features = docs_rs
            .get("features")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("publish crate {crate_name} docs.rs must define features"))?;
        let declared_features = record
            .manifest_value
            .get("features")
            .and_then(toml::Value::as_table);
        for feature in docs_features {
            let feature = feature.as_str().ok_or_else(|| {
                format!("publish crate {crate_name} docs.rs features must be strings")
            })?;
            if !declared_features.is_some_and(|features| features.contains_key(feature)) {
                return Err(format!(
                    "publish crate {crate_name} docs.rs selects unknown feature {feature}"
                ));
            }
        }
    }
    Ok(())
}

fn parse_coverage_percent(raw: &str, field: &str, crate_name: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("parse {} for {}: {e}", field, crate_name)),
    }
}

fn parse_branch_coverage_percent(raw: &str, crate_name: &str) -> Result<Option<f64>, String> {
    if raw == "unavailable" {
        return Ok(None);
    }
    parse_coverage_percent(raw, "branch", crate_name).map(Some)
}

fn branch_coverage_fails(branch: Option<f64>, thresholds: CoverageThresholds) -> bool {
    match branch {
        Some(value) => value < thresholds.fail_under_branches,
        None => thresholds.require_branches,
    }
}

fn branch_coverage_display(branch: Option<f64>) -> String {
    branch
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

#[derive(Debug)]
struct CoverageRefreshRow {
    status: String,
    exec: f64,
    func: f64,
    branch: Option<f64>,
    region: f64,
    report_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CoverageGateReportForValidation {
    scope: String,
    thresholds: CoverageGateReportThresholdsForValidation,
    measured: CoverageGateReportMeasuredForValidation,
    result: CoverageGateReportResultForValidation,
}

#[derive(Debug, Deserialize)]
struct CoverageGateReportThresholdsForValidation {
    executable_lines: f64,
    functions: f64,
    regions: f64,
    branches: f64,
    branches_required: bool,
}

#[derive(Debug, Deserialize)]
struct CoverageGateReportMeasuredForValidation {
    executable_lines_percent: f64,
    functions_percent: f64,
    branches_percent: Option<f64>,
    branches_available: bool,
    summary_regions_percent: f64,
}

#[derive(Debug, Deserialize)]
struct CoverageGateReportResultForValidation {
    pass: bool,
}

type CoverageRefreshRows = BTreeMap<String, CoverageRefreshRow>;

fn coverage_refresh_report_path(
    workspace_root: &Path,
    report_path: &Path,
    raw_report_path: &str,
    crate_name: &str,
) -> Result<PathBuf, String> {
    let trimmed = raw_report_path.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "coverage row for crate {} in {} must include a report path",
            crate_name,
            report_path.display()
        ));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(workspace_root.join(path))
    }
}

fn load_coverage_refresh_rows(workspace_root: &Path) -> Result<CoverageRefreshRows, String> {
    let report_path = workspace_root
        .join("target")
        .join("coverage")
        .join("coverage-refresh.tsv");
    let raw = match fs::read_to_string(&report_path) {
        Ok(raw) => raw,
        Err(e) => return Err(format!("read {}: {e}", report_path.display())),
    };
    let mut rows = BTreeMap::new();
    for line in raw.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split('\t').collect::<Vec<_>>();
        if parts.len() < 7 {
            return Err(format!(
                "coverage row must have at least 7 columns in {}: {}",
                report_path.display(),
                trimmed
            ));
        }
        let crate_name = parts[0].to_string();
        let status = parts[1].to_string();
        let exec = parse_coverage_percent(parts[2], "exec", &crate_name)?;
        let func = parse_coverage_percent(parts[3], "func", &crate_name)?;
        let branch = parse_branch_coverage_percent(parts[4], &crate_name)?;
        let region = parse_coverage_percent(parts[5], "region", &crate_name)?;
        let row_report_path =
            coverage_refresh_report_path(workspace_root, &report_path, parts[6], &crate_name)?;
        if rows
            .insert(
                crate_name.clone(),
                CoverageRefreshRow {
                    status,
                    exec,
                    func,
                    branch,
                    region,
                    report_path: row_report_path,
                },
            )
            .is_some()
        {
            return Err(format!(
                "duplicate coverage row for crate {} in {}",
                crate_name,
                report_path.display()
            ));
        }
    }
    Ok(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn validate_required_coverage_summary(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
    thresholds: CoverageThresholds,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let row = rows.get(crate_name).ok_or_else(|| {
            format!(
                "required coverage crate {} missing from coverage-refresh.tsv",
                crate_name
            )
        })?;
        if row.status != "pass" {
            return Err(format!(
                "required coverage crate {} has non-pass status {}",
                crate_name, row.status
            ));
        }
        if row.exec < thresholds.fail_under_exec_lines
            || row.func < thresholds.fail_under_functions
            || branch_coverage_fails(row.branch, thresholds)
            || row.region < thresholds.fail_under_regions
        {
            return Err(format!(
                "required coverage crate {} must satisfy coverage policy {},{},{},{}, found {}/{}/{}/{}",
                crate_name,
                thresholds.fail_under_exec_lines,
                thresholds.fail_under_functions,
                thresholds.fail_under_branches,
                thresholds.fail_under_regions,
                row.exec,
                row.func,
                branch_coverage_display(row.branch),
                row.region
            ));
        }
    }
    Ok(())
}

fn read_coverage_gate_report(
    path: &Path,
    crate_name: &str,
) -> Result<CoverageGateReportForValidation, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            return Err(format!(
                "read coverage gate report for {} at {}: {e}",
                crate_name,
                path.display()
            ));
        }
    };
    serde_json::from_str::<CoverageGateReportForValidation>(&raw).map_err(|e| {
        format!(
            "parse coverage gate report for {} at {}: {e}",
            crate_name,
            path.display()
        )
    })
}

fn coverage_percent_matches(left: f64, right: f64) -> bool {
    (left - right).abs() <= COVERAGE_REPORT_EPSILON
}

fn coverage_branch_percent_matches(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => coverage_percent_matches(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn coverage_gate_report_thresholds_match(
    report: &CoverageGateReportThresholdsForValidation,
    thresholds: CoverageThresholds,
) -> bool {
    coverage_percent_matches(report.executable_lines, thresholds.fail_under_exec_lines)
        && coverage_percent_matches(report.functions, thresholds.fail_under_functions)
        && coverage_percent_matches(report.regions, thresholds.fail_under_regions)
        && coverage_percent_matches(report.branches, thresholds.fail_under_branches)
        && report.branches_required == thresholds.require_branches
}

fn validate_coverage_gate_report_for_row(
    crate_name: &str,
    row: &CoverageRefreshRow,
    thresholds: CoverageThresholds,
) -> Result<(), String> {
    let report = read_coverage_gate_report(&row.report_path, crate_name)?;
    if report.scope != crate_name {
        return Err(format!(
            "coverage gate report {} has scope {}, expected {}",
            row.report_path.display(),
            report.scope,
            crate_name
        ));
    }
    if !coverage_gate_report_thresholds_match(&report.thresholds, thresholds) {
        return Err(format!(
            "coverage gate report {} for {} thresholds do not match policy",
            row.report_path.display(),
            crate_name
        ));
    }
    if !report.result.pass {
        return Err(format!(
            "coverage gate report {} for {} has non-pass result",
            row.report_path.display(),
            crate_name
        ));
    }
    if report.measured.branches_available != report.measured.branches_percent.is_some() {
        return Err(format!(
            "coverage gate report {} for {} has inconsistent branch measurement",
            row.report_path.display(),
            crate_name
        ));
    }
    if !coverage_percent_matches(row.exec, report.measured.executable_lines_percent)
        || !coverage_percent_matches(row.func, report.measured.functions_percent)
        || !coverage_branch_percent_matches(row.branch, report.measured.branches_percent)
        || !coverage_percent_matches(row.region, report.measured.summary_regions_percent)
    {
        return Err(format!(
            "coverage row for {} does not match coverage gate report {}",
            crate_name,
            row.report_path.display()
        ));
    }
    Ok(())
}

fn validate_required_coverage_summary_with_policy(
    workspace_root: &Path,
    required_crates: &BTreeSet<String>,
    policy: &CoveragePolicyFile,
) -> Result<(), String> {
    let rows = load_coverage_refresh_rows(workspace_root)?;
    for crate_name in required_crates {
        let row = rows.get(crate_name).ok_or_else(|| {
            format!(
                "required coverage crate {} missing from coverage-refresh.tsv",
                crate_name
            )
        })?;
        if row.status != "pass" {
            return Err(format!(
                "required coverage crate {} has non-pass status {}",
                crate_name, row.status
            ));
        }
        let thresholds = policy.thresholds_for_scope(crate_name);
        validate_coverage_gate_report_for_row(crate_name, row, thresholds)?;
        if row.exec < thresholds.fail_under_exec_lines
            || row.func < thresholds.fail_under_functions
            || branch_coverage_fails(row.branch, thresholds)
            || row.region < thresholds.fail_under_regions
        {
            return Err(format!(
                "required coverage crate {} must satisfy coverage policy {},{},{},{}, found {}/{}/{}/{}",
                crate_name,
                thresholds.fail_under_exec_lines,
                thresholds.fail_under_functions,
                thresholds.fail_under_branches,
                thresholds.fail_under_regions,
                row.exec,
                row.func,
                branch_coverage_display(row.branch),
                row.region
            ));
        }
    }
    Ok(())
}

const CORE_UNIT_DIMENSION_ENUM: &str = "UnitDimension";
const CORE_UNIT_DIMENSION_ORDER: [&str; 3] = ["Count", "Mass", "Volume"];

fn extract_enum_body<'a>(source: &'a str, enum_name: &str) -> Result<&'a str, String> {
    let marker = format!("pub enum {enum_name}");
    let enum_start = match source.find(&marker) {
        Some(index) => index,
        None => return Err(format!("missing enum {enum_name}")),
    };
    let after_start = &source[enum_start..];
    let open_rel = match after_start.find('{') {
        Some(index) => index,
        None => return Err(format!("missing opening brace for enum {enum_name}")),
    };
    let open_idx = enum_start + open_rel;
    let mut depth = 0usize;
    for (offset, ch) in source[open_idx..].char_indices() {
        if ch == '{' {
            depth += 1;
            continue;
        }
        if ch != '}' {
            continue;
        }
        depth = depth.saturating_sub(1);
        if depth == 0 {
            let close_idx = open_idx + offset;
            return Ok(&source[(open_idx + 1)..close_idx]);
        }
    }
    Err(format!("missing closing brace for enum {enum_name}"))
}

fn parse_enum_variants(enum_body: &str) -> Vec<String> {
    enum_body
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                return None;
            }
            let before_comma = trimmed
                .split_once(',')
                .map_or(trimmed, |(head, _)| head)
                .trim();
            if before_comma.is_empty() {
                return None;
            }
            let before_discriminant = before_comma
                .split_once('=')
                .map_or(before_comma, |(head, _)| head)
                .trim();
            if before_discriminant.is_empty() {
                return None;
            }
            let ident = before_discriminant
                .split_whitespace()
                .next()
                .unwrap_or_default();
            Some(ident.to_string())
        })
        .collect()
}

fn validate_core_unit_dimension_variant_order(workspace_root: &Path) -> Result<(), String> {
    let source_path = workspace_root
        .join("crates")
        .join("core")
        .join("src")
        .join("unit.rs");
    let source = match fs::read_to_string(&source_path) {
        Ok(source) => source,
        Err(e) => return Err(format!("read {}: {e}", source_path.display())),
    };
    let enum_body = extract_enum_body(&source, CORE_UNIT_DIMENSION_ENUM)?;
    let variants = parse_enum_variants(enum_body);
    let expected = CORE_UNIT_DIMENSION_ORDER
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    if variants != expected {
        return Err(format!(
            "core unit dimension variant order must be {} but was {}",
            CORE_UNIT_DIMENSION_ORDER.join(", "),
            variants.join(", ")
        ));
    }
    Ok(())
}

fn validate_coverage_policy_parity(
    workspace_root: &Path,
    contract_root: &Path,
) -> Result<(), String> {
    let policy = load_coverage_policy(contract_root)?;
    let thresholds = policy.thresholds();
    if thresholds.fail_under_exec_lines != COVERAGE_REQUIRED_THRESHOLD
        || thresholds.fail_under_functions != COVERAGE_REQUIRED_THRESHOLD
        || thresholds.fail_under_regions != COVERAGE_REQUIRED_THRESHOLD
        || thresholds.fail_under_branches != COVERAGE_REQUIRED_THRESHOLD
        || !thresholds.require_branches
    {
        return Err(format!(
            "coverage policy must enforce {COVERAGE_REQUIRED_THRESHOLD_LABEL} with required branches"
        ));
    }

    let required_packages = policy
        .required_crate_entries()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for package in &required_packages {
        let scoped = policy.thresholds_for_scope(package);
        if scoped.fail_under_exec_lines < COVERAGE_REQUIRED_THRESHOLD
            || scoped.fail_under_functions < COVERAGE_REQUIRED_THRESHOLD
            || scoped.fail_under_regions < COVERAGE_REQUIRED_THRESHOLD
            || scoped.fail_under_branches < COVERAGE_REQUIRED_THRESHOLD
        {
            return Err(format!(
                "coverage policy scope {package} must enforce at least {COVERAGE_REQUIRED_THRESHOLD_LABEL}"
            ));
        }
    }
    let expected_packages = coverage_required_workspace_crates(workspace_root)?;
    if expected_packages != required_packages {
        let missing = expected_packages
            .difference(&required_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = required_packages
            .difference(&expected_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "coverage policy missing workspace crates: {}; coverage policy includes excluded or unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    Ok(())
}

fn publish_config_is_public(publish: Option<&PackagePublish>) -> bool {
    matches!(
        publish,
        Some(PackagePublish::Registries(registries))
            if registries.len() == 1 && registries[0] == "crates-io"
    )
}

fn publish_config_is_non_public(publish: Option<&PackagePublish>) -> bool {
    matches!(publish, Some(PackagePublish::Bool(false)))
}

fn validate_publication_control(
    release: &ReleaseContractFile,
    publish_configs: &BTreeMap<String, Option<PackagePublish>>,
    require_control: bool,
) -> Result<bool, String> {
    let Some(control) = release.publication.as_ref() else {
        if require_control {
            return Err("publication control is required".to_string());
        }
        return Ok(false);
    };
    if control.registry != "crates-io" {
        return Err("publication.registry must be crates-io".to_string());
    }
    if control.final_enablement_step != 305 {
        return Err("publication.final_enablement_step must be 305".to_string());
    }
    if !control.frozen {
        return Ok(false);
    }
    for (crate_name, publish) in publish_configs {
        if !publish_config_is_non_public(publish.as_ref()) {
            return Err(format!(
                "publication freeze requires workspace crate {} to set publish = false",
                crate_name
            ));
        }
    }
    Ok(true)
}

fn validate_v1_release_policy(
    workspace_root: &Path,
    release: &ReleaseContractFile,
    workspace_packages: &BTreeSet<String>,
    publish_configs: &BTreeMap<String, Option<PackagePublish>>,
    require_v1: bool,
) -> Result<Option<BTreeSet<String>>, String> {
    let Some(control) = release.publication.as_ref() else {
        if require_v1 {
            return Err("publication control is required".to_string());
        }
        return Ok(None);
    };
    let declares_v1 = !control.spec_id.is_empty()
        || !control.approved_packages.is_empty()
        || !control.local_packages.is_empty()
        || !control.external_packages.is_empty()
        || release.workspace_classification.is_some();
    if !declares_v1 {
        if require_v1 {
            return Err("publication must define the v1 approved package authority".to_string());
        }
        return Ok(None);
    }

    let architecture_path = workspace_root.join("docs/specs/radroots_crates_release_v1.toml");
    let architecture = parse_toml::<CratesReleaseArchitecture>(&architecture_path)?;
    let expected_approved = collect_unique_set(
        &architecture
            .package
            .iter()
            .map(|package| package.name.clone())
            .collect::<Vec<_>>(),
        "architecture.package.name",
    )?;
    if architecture.package_count != expected_approved.len() || architecture.package_count != 19 {
        return Err(format!(
            "release architecture must define exactly 19 unique packages, found package_count {} and {} unique package records",
            architecture.package_count,
            expected_approved.len()
        ));
    }
    if control.spec_id != architecture.spec_id || control.spec_id != "radroots.crates.release.v1" {
        return Err(format!(
            "publication.spec_id {} must match architecture id {}",
            control.spec_id, architecture.spec_id
        ));
    }

    let approved = collect_unique_set(&control.approved_packages, "publication.approved_packages")?;
    let local = collect_unique_set(&control.local_packages, "publication.local_packages")?;
    let external = collect_unique_set(&control.external_packages, "publication.external_packages")?;
    let expected_local = collect_unique_set(
        &architecture.repositories.lib.packages,
        "architecture.repositories.lib.packages",
    )?;
    let expected_external = collect_unique_set(
        &architecture.repositories.sdk.packages,
        "architecture.repositories.sdk.packages",
    )?;
    for (field, actual, expected) in [
        (
            "publication.approved_packages",
            &approved,
            &expected_approved,
        ),
        ("publication.local_packages", &local, &expected_local),
        (
            "publication.external_packages",
            &external,
            &expected_external,
        ),
    ] {
        if actual != expected {
            let missing = expected
                .difference(actual)
                .cloned()
                .collect::<BTreeSet<_>>();
            let extra = actual
                .difference(expected)
                .cloned()
                .collect::<BTreeSet<_>>();
            return Err(format!(
                "{field} is missing approved packages: {}; {field} has unapproved packages: {}",
                join_set(&missing),
                join_set(&extra)
            ));
        }
    }
    let ownership_overlap = local
        .intersection(&external)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !ownership_overlap.is_empty() {
        return Err(format!(
            "local and external approved package ownership overlaps: {}",
            join_set(&ownership_overlap)
        ));
    }
    let mut owned = local.clone();
    owned.extend(external.iter().cloned());
    if owned != approved {
        return Err(
            "local and external package ownership must partition approved packages".to_string(),
        );
    }
    let external_in_workspace = external
        .intersection(workspace_packages)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !external_in_workspace.is_empty() {
        return Err(format!(
            "externally owned approved packages must not be workspace members: {}",
            join_set(&external_in_workspace)
        ));
    }

    let classification = release.workspace_classification.as_ref().ok_or_else(|| {
        "workspace_classification is required for the v1 release policy".to_string()
    })?;
    let private = collect_unique_set(&classification.private, "workspace_classification.private")?;
    let build_codegen = collect_unique_set(
        &classification.build_codegen,
        "workspace_classification.build_codegen",
    )?;
    let test_support = collect_unique_set(
        &classification.test_support,
        "workspace_classification.test_support",
    )?;
    let preview = collect_unique_set(&classification.preview, "workspace_classification.preview")?;
    let retired = collect_unique_set(&classification.retired, "workspace_classification.retired")?;
    let classes = [
        ("private", &private),
        ("build-codegen", &build_codegen),
        ("test-support", &test_support),
        ("preview", &preview),
        ("retired", &retired),
    ];
    for index in 0..classes.len() {
        for other_index in (index + 1)..classes.len() {
            let overlap = classes[index]
                .1
                .intersection(classes[other_index].1)
                .cloned()
                .collect::<BTreeSet<_>>();
            if !overlap.is_empty() {
                return Err(format!(
                    "workspace classification overlap is not allowed between {} and {}: {}",
                    classes[index].0,
                    classes[other_index].0,
                    join_set(&overlap)
                ));
            }
        }
    }
    let mut classified = BTreeSet::new();
    for (_, entries) in classes {
        classified.extend(entries.iter().cloned());
    }
    let local_workspace_packages = local
        .intersection(workspace_packages)
        .cloned()
        .collect::<BTreeSet<_>>();
    let public_classification_overlap = classified
        .intersection(&local_workspace_packages)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !public_classification_overlap.is_empty() {
        return Err(format!(
            "approved local packages must not be classified as private workspace packages: {}",
            join_set(&public_classification_overlap)
        ));
    }
    let mut accounted = classified.clone();
    accounted.extend(local_workspace_packages.iter().cloned());
    if accounted != *workspace_packages {
        let missing = workspace_packages
            .difference(&accounted)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = accounted
            .difference(workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "workspace classification is missing packages: {}; workspace classification has unknown packages: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if control.registry != "crates-io" {
        return Err("publication.registry must be crates-io".to_string());
    }
    if control.final_enablement_step != 305 {
        return Err("publication.final_enablement_step must be 305".to_string());
    }
    let publish_order = collect_unique_set(&release.publish_order.crates, "publish_order.crates")?;
    if control.frozen {
        if !publish_order.is_empty() {
            return Err(
                "publish_order.crates must remain empty while publication is frozen".to_string(),
            );
        }
        for (crate_name, publish) in publish_configs {
            if !publish_config_is_non_public(publish.as_ref()) {
                return Err(format!(
                    "publication freeze requires workspace crate {} to set publish = false",
                    crate_name
                ));
            }
        }
        return Ok(Some(BTreeSet::new()));
    }

    if local_workspace_packages != local {
        let missing = local
            .difference(&local_workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publication enablement is missing approved local workspace packages: {}",
            join_set(&missing)
        ));
    }
    if publish_order != local {
        return Err("publish_order.crates must contain exactly the approved local packages when publication is enabled".to_string());
    }
    for (crate_name, publish) in publish_configs {
        if local.contains(crate_name) {
            if !publish_config_is_public(publish.as_ref()) {
                return Err(format!(
                    "approved local crate {} must set publish = [\"crates-io\"]",
                    crate_name
                ));
            }
        } else if !publish_config_is_non_public(publish.as_ref()) {
            return Err(format!(
                "private workspace crate {} must set publish = false",
                crate_name
            ));
        }
    }
    Ok(Some(local))
}

#[cfg(test)]
fn validate_release_publish_policy(
    workspace_root: &Path,
    _contract_root: &Path,
    contract_version: &str,
) -> Result<(), String> {
    let release = load_release_contract(workspace_root, contract_version)?;
    if release.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_string());
    }
    if release.release.version != contract_version {
        return Err(format!(
            "release.version {} must match contract version {}",
            release.release.version, contract_version
        ));
    }

    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let publish_configs = workspace_package_publish_configs(workspace_root)
        .expect("workspace publish configs are stable");
    if validate_v1_release_policy(
        workspace_root,
        &release,
        &workspace_packages,
        &publish_configs,
        false,
    )?
    .is_some()
    {
        return Ok(());
    }
    let uses_classification = release.uses_classification();
    let public_field = if uses_classification {
        "classification.public"
    } else {
        "publish.crates"
    };
    let internal_field = if uses_classification {
        "classification.internal"
    } else {
        "internal.crates"
    };

    let public_set = collect_unique_set(&release.public_crates(), public_field)?;
    let internal_set = collect_unique_set(&release.internal_crates(), internal_field)?;
    let deferred_set = collect_unique_set(&release.deferred_crates(), "classification.deferred")?;
    let retired_set = collect_unique_set(&release.retired_crates(), "classification.retired")?;
    let yank_only_set =
        collect_unique_set(&release.yank_only_crates(), "classification.yank_only")?;
    let publish_order = &release.publish_order.crates;
    let publish_order_set = collect_unique_set(publish_order, "publish_order.crates")?;

    let class_sets = [
        ("public", &public_set),
        ("internal", &internal_set),
        ("deferred", &deferred_set),
        ("retired", &retired_set),
        ("yank-only", &yank_only_set),
    ];
    for idx in 0..class_sets.len() {
        for other_idx in (idx + 1)..class_sets.len() {
            let overlap = class_sets[idx]
                .1
                .intersection(class_sets[other_idx].1)
                .cloned()
                .collect::<BTreeSet<_>>();
            if !overlap.is_empty() {
                return Err(format!(
                    "release classification overlap is not allowed between {} and {}: {}",
                    class_sets[idx].0,
                    class_sets[other_idx].0,
                    join_set(&overlap)
                ));
            }
        }
    }

    let mut combined = public_set.clone();
    combined.extend(internal_set.iter().cloned());
    combined.extend(deferred_set.iter().cloned());
    combined.extend(retired_set.iter().cloned());
    combined.extend(yank_only_set.iter().cloned());
    if combined != workspace_packages {
        let missing = workspace_packages
            .difference(&combined)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = combined
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "release classification sets are missing workspace crates: {}; release classification sets include unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if publish_order_set != public_set {
        let missing = public_set
            .difference(&publish_order_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = publish_order_set
            .difference(&public_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publish_order.crates is missing publish crates: {}; publish_order.crates has non-publish crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)
        .expect("workspace package manifests were already parsed");
    for crate_name in &public_set {
        let crate_deps = &dependencies[crate_name];
        let crate_order = order_index[crate_name];
        for dep in crate_deps {
            if !public_set.contains(dep) {
                continue;
            }
            let dep_order = order_index[dep];
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    if validate_publication_control(&release, &publish_configs, false)? {
        return Ok(());
    }
    for crate_name in &public_set {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_public(publish) {
            return Err(format!(
                "public crate {} must set publish = [\"crates-io\"]",
                crate_name
            ));
        }
    }
    for crate_name in internal_set
        .iter()
        .chain(deferred_set.iter())
        .chain(retired_set.iter())
        .chain(yank_only_set.iter())
    {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_non_public(publish) {
            return Err(format!(
                "non-public crate {} must set publish = false",
                crate_name
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum OperationAuthorityProfile {
    CapsuleCanonical,
    #[cfg(test)]
    Generic,
}

pub fn validate_release_preflight(workspace_root: &Path) -> Result<(), String> {
    validate_release_preflight_with_override(workspace_root, None)
}

pub fn validate_release_preflight_with_override(
    workspace_root: &Path,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    validate_release_preflight_with_override_and_profile(
        workspace_root,
        release_policy_override,
        OperationAuthorityProfile::CapsuleCanonical,
    )
}

fn validate_release_preflight_with_override_and_profile(
    workspace_root: &Path,
    release_policy_override: Option<PathBuf>,
    authority_profile: OperationAuthorityProfile,
) -> Result<(), String> {
    let bundle = load_contract_bundle(workspace_root)?;
    validate_contract_bundle_with_release_policy_override_and_profile(
        &bundle,
        release_policy_override.clone(),
        authority_profile,
    )?;
    let release = load_release_contract_with_override(
        workspace_root,
        bundle.version.contract.version.as_str(),
        release_policy_override,
    )?;
    let policy =
        load_coverage_policy(&bundle.root).expect("validated contract includes coverage policy");
    let publish_crates = collect_unique_set(
        &release.public_crates(),
        if release.uses_classification() {
            "classification.public"
        } else {
            "publish.crates"
        },
    )
    .expect("validated contract enforces unique public crates");
    let required_crate_list = policy
        .required_crates()
        .expect("validated contract includes required crates");
    let required_crates = collect_unique_set(&required_crate_list, "required.crates")
        .expect("validated contract enforces unique required.crates");
    validate_publishable_dto_tooling_sources(workspace_root, &publish_crates)?;
    validate_publish_package_metadata(workspace_root, &publish_crates)?;
    validate_required_coverage_summary_with_policy(workspace_root, &required_crates, &policy)?;
    Ok(())
}

fn validate_contract_bundle_with_release_policy_override(
    bundle: &ContractBundle,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    validate_contract_bundle_with_release_policy_override_and_profile(
        bundle,
        release_policy_override,
        OperationAuthorityProfile::CapsuleCanonical,
    )
}

fn validate_contract_bundle_with_release_policy_override_and_profile(
    bundle: &ContractBundle,
    release_policy_override: Option<PathBuf>,
    authority_profile: OperationAuthorityProfile,
) -> Result<(), String> {
    if bundle.manifest.contract.name.trim().is_empty() {
        return Err("contract name is required".to_string());
    }
    if bundle.manifest.contract.version.trim().is_empty() {
        return Err("contract version is required".to_string());
    }
    if bundle.manifest.contract.source.trim().is_empty() {
        return Err("contract source is required".to_string());
    }
    if bundle.manifest.surface.model_crates.is_empty() {
        return Err("contract surface.model_crates must not be empty".to_string());
    }
    if bundle.manifest.surface.algorithm_crates.is_empty() {
        return Err("contract surface.algorithm_crates must not be empty".to_string());
    }
    validate_surface_metadata(&bundle.manifest.surface)?;
    if bundle.version.contract.version.trim().is_empty() {
        return Err("version.contract.version is required".to_string());
    }
    if bundle.version.contract.stability.trim().is_empty() {
        return Err("version.contract.stability is required".to_string());
    }
    if bundle.version.semver.major_on.is_empty()
        || bundle.version.semver.minor_on.is_empty()
        || bundle.version.semver.patch_on.is_empty()
    {
        return Err("version.semver rules must all be non-empty".to_string());
    }
    if !bundle.version.release_integrity.requires_conformance_pass {
        return Err("release_integrity.requires_conformance_pass must be true".to_string());
    }
    if !bundle
        .version
        .release_integrity
        .requires_contract_manifest_diff
    {
        return Err("release_integrity.requires_contract_manifest_diff must be true".to_string());
    }
    if !bundle.version.release_integrity.requires_release_notes {
        return Err("release_integrity.requires_release_notes must be true".to_string());
    }
    validate_policy_metadata(&bundle.manifest.policy)?;
    let workspace_root = bundle
        .root
        .parent()
        .expect("contract root must have a workspace parent");
    validate_replica_contract(bundle, workspace_root)?;
    validate_operations_contract(bundle, &bundle.operations_manifest, workspace_root)?;
    if matches!(
        authority_profile,
        OperationAuthorityProfile::CapsuleCanonical
    ) {
        validate_capsule_operation_authority(&bundle.operations_manifest, workspace_root)?;
    }
    validate_all_conformance_vectors(workspace_root, &bundle.manifest.contract.version)?;
    validate_core_unit_dimension_variant_order(workspace_root)?;
    validate_coverage_policy_parity(workspace_root, &bundle.root)?;
    validate_version_governance(bundle, workspace_root)?;
    if matches!(
        authority_profile,
        OperationAuthorityProfile::CapsuleCanonical
    ) {
        crate::architecture::validate(workspace_root)?;
    }
    validate_release_publish_policy_with_override_and_control(
        workspace_root,
        &bundle.root,
        bundle.version.contract.version.as_str(),
        release_policy_override,
        matches!(
            authority_profile,
            OperationAuthorityProfile::CapsuleCanonical
        ),
        matches!(
            authority_profile,
            OperationAuthorityProfile::CapsuleCanonical
        ),
    )?;
    Ok(())
}

#[cfg(test)]
fn validate_release_publish_policy_with_override(
    workspace_root: &Path,
    contract_root: &Path,
    contract_version: &str,
    release_policy_override: Option<PathBuf>,
) -> Result<(), String> {
    validate_release_publish_policy_with_override_and_control(
        workspace_root,
        contract_root,
        contract_version,
        release_policy_override,
        true,
        false,
    )
}

fn validate_release_publish_policy_with_override_and_control(
    workspace_root: &Path,
    _contract_root: &Path,
    contract_version: &str,
    release_policy_override: Option<PathBuf>,
    require_publication_control: bool,
    require_v1_policy: bool,
) -> Result<(), String> {
    let release = load_release_contract_with_override(
        workspace_root,
        contract_version,
        release_policy_override,
    )?;
    if release.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_string());
    }
    if release.release.version != contract_version {
        return Err(format!(
            "release.version {} must match contract version {}",
            release.release.version, contract_version
        ));
    }

    let workspace_packages = workspace_package_names(workspace_root)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let publish_configs = workspace_package_publish_configs(workspace_root)
        .expect("workspace publish configs are stable");
    if validate_v1_release_policy(
        workspace_root,
        &release,
        &workspace_packages,
        &publish_configs,
        require_v1_policy,
    )?
    .is_some()
    {
        return Ok(());
    }
    let uses_classification = release.uses_classification();
    let public_field = if uses_classification {
        "classification.public"
    } else {
        "publish.crates"
    };
    let internal_field = if uses_classification {
        "classification.internal"
    } else {
        "internal.crates"
    };

    let public_set = collect_unique_set(&release.public_crates(), public_field)?;
    let internal_set = collect_unique_set(&release.internal_crates(), internal_field)?;
    let deferred_set = collect_unique_set(&release.deferred_crates(), "classification.deferred")?;
    let retired_set = collect_unique_set(&release.retired_crates(), "classification.retired")?;
    let yank_only_set =
        collect_unique_set(&release.yank_only_crates(), "classification.yank_only")?;
    let publish_order = &release.publish_order.crates;
    let publish_order_set = collect_unique_set(publish_order, "publish_order.crates")?;

    let class_sets = [
        ("public", &public_set),
        ("internal", &internal_set),
        ("deferred", &deferred_set),
        ("retired", &retired_set),
        ("yank-only", &yank_only_set),
    ];
    for idx in 0..class_sets.len() {
        for other_idx in (idx + 1)..class_sets.len() {
            let overlap = class_sets[idx]
                .1
                .intersection(class_sets[other_idx].1)
                .cloned()
                .collect::<BTreeSet<_>>();
            if !overlap.is_empty() {
                return Err(format!(
                    "release classification overlap is not allowed between {} and {}: {}",
                    class_sets[idx].0,
                    class_sets[other_idx].0,
                    join_set(&overlap)
                ));
            }
        }
    }

    let mut combined = public_set.clone();
    combined.extend(internal_set.iter().cloned());
    combined.extend(deferred_set.iter().cloned());
    combined.extend(retired_set.iter().cloned());
    combined.extend(yank_only_set.iter().cloned());
    if combined != workspace_packages {
        let missing = workspace_packages
            .difference(&combined)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = combined
            .difference(&workspace_packages)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "release classification sets are missing workspace crates: {}; release classification sets include unknown crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    if publish_order_set != public_set {
        let missing = public_set
            .difference(&publish_order_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = publish_order_set
            .difference(&public_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "publish_order.crates is missing publish crates: {}; publish_order.crates has non-publish crates: {}",
            join_set(&missing),
            join_set(&extra)
        ));
    }

    let order_index = publish_order
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let dependencies = read_workspace_package_dependencies(workspace_root)
        .expect("workspace package manifests were already parsed");
    for crate_name in &public_set {
        let crate_deps = &dependencies[crate_name];
        let crate_order = order_index[crate_name];
        for dep in crate_deps {
            if !public_set.contains(dep) {
                continue;
            }
            let dep_order = order_index[dep];
            if dep_order >= crate_order {
                return Err(format!(
                    "publish order must place dependency {} before {}",
                    dep, crate_name
                ));
            }
        }
    }

    if validate_publication_control(&release, &publish_configs, require_publication_control)? {
        return Ok(());
    }
    for crate_name in &public_set {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_public(publish) {
            return Err(format!(
                "public crate {} must set publish = [\"crates-io\"]",
                crate_name
            ));
        }
    }
    for crate_name in internal_set
        .iter()
        .chain(deferred_set.iter())
        .chain(retired_set.iter())
        .chain(yank_only_set.iter())
    {
        let publish = publish_configs[crate_name].as_ref();
        if !publish_config_is_non_public(publish) {
            return Err(format!(
                "non-public crate {} must set publish = false",
                crate_name
            ));
        }
    }

    Ok(())
}

pub fn load_contract_bundle(workspace_root: &Path) -> Result<ContractBundle, String> {
    reject_legacy_contract_roots(workspace_root)?;
    let root = contract_root(workspace_root);
    let manifest = parse_toml::<ContractManifest>(&root.join("manifest.toml"))?;
    let version = parse_toml::<VersionPolicy>(&root.join("version.toml"))?;
    let replica =
        parse_toml::<ReplicaContractManifest>(&workspace_root.join(REPLICA_CONTRACT_RELATIVE))?;
    let operations_manifest =
        parse_toml::<OperationsContractManifest>(&root.join("operations.toml"))?;
    Ok(ContractBundle {
        root,
        manifest,
        version,
        replica,
        operations_manifest,
    })
}

fn reject_legacy_contract_roots(workspace_root: &Path) -> Result<(), String> {
    for relative in ["spec", "policy"] {
        let legacy_root = workspace_root.join(relative);
        if legacy_root.exists() {
            return Err(format!(
                "legacy contract root {} is forbidden; use contracts/",
                legacy_root.display()
            ));
        }
    }
    Ok(())
}

pub fn validate_contract_bundle(bundle: &ContractBundle) -> Result<(), String> {
    validate_contract_bundle_with_release_policy_override(bundle, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SYNTHETIC_CONFORMANCE_VECTOR: &str = r#"{
  "suite": "synthetic",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "synthetic_vector_001",
      "kind": "synthetic.operation",
      "input": {},
      "expected": {}
    }
  ]
}
"#;

    fn workspace_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .join("../..")
            .canonicalize()
            .expect("canonical workspace root")
    }

    fn validate_generic_contract_bundle(bundle: &ContractBundle) -> Result<(), String> {
        validate_contract_bundle_with_release_policy_override_and_profile(
            bundle,
            None,
            OperationAuthorityProfile::Generic,
        )
    }

    fn validate_generic_release_preflight(workspace_root: &Path) -> Result<(), String> {
        validate_release_preflight_with_override_and_profile(
            workspace_root,
            None,
            OperationAuthorityProfile::Generic,
        )
    }

    fn current_post_authority() -> (OperationsContractManifest, ConformanceVectorFile) {
        let root = workspace_root();
        let manifest =
            parse_toml::<OperationsContractManifest>(&root.join("contracts/operations.toml"))
                .expect("current operations manifest");
        let vector =
            parse_json::<ConformanceVectorFile>(&root.join(POST_CONFORMANCE_VECTOR_RELATIVE))
                .expect("current post conformance vector");
        (manifest, vector)
    }

    fn current_admission_authority() -> (OperationsContractManifest, ConformanceVectorFile) {
        let root = workspace_root();
        let manifest =
            parse_toml::<OperationsContractManifest>(&root.join("contracts/operations.toml"))
                .expect("current operations manifest");
        let vector = parse_json::<ConformanceVectorFile>(
            &root.join(admission_authority::ADMISSION_CONFORMANCE_VECTOR_RELATIVE),
        )
        .expect("current verified admission conformance vector");
        (manifest, vector)
    }

    fn current_comment_authority() -> (OperationsContractManifest, ConformanceVectorFile) {
        let root = workspace_root();
        let manifest =
            parse_toml::<OperationsContractManifest>(&root.join("contracts/operations.toml"))
                .expect("current operations manifest");
        let vector =
            parse_json::<ConformanceVectorFile>(&root.join(COMMENT_CONFORMANCE_VECTOR_RELATIVE))
                .expect("current Comment conformance vector");
        (manifest, vector)
    }

    fn current_deletion_authority() -> (
        OperationsContractManifest,
        ConformanceVectorFile,
        ConformanceVectorFile,
    ) {
        let root = workspace_root();
        let manifest =
            parse_toml::<OperationsContractManifest>(&root.join("contracts/operations.toml"))
                .expect("current operations manifest");
        let request_vector =
            parse_json::<ConformanceVectorFile>(&root.join(DELETION_CONFORMANCE_VECTOR_RELATIVE))
                .expect("current deletion conformance vector");
        let suppression_vector = parse_json::<ConformanceVectorFile>(
            &root.join(DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE),
        )
        .expect("current deletion suppression conformance vector");
        (manifest, request_vector, suppression_vector)
    }

    fn current_food_availability_authority() -> (OperationsContractManifest, ConformanceVectorFile)
    {
        let root = workspace_root();
        let manifest =
            parse_toml::<OperationsContractManifest>(&root.join("contracts/operations.toml"))
                .expect("current operations manifest");
        let vector = parse_json::<ConformanceVectorFile>(
            &root.join(FOOD_AVAILABILITY_CONFORMANCE_VECTOR_RELATIVE),
        )
        .expect("current FoodAvailability conformance vector");
        (manifest, vector)
    }

    fn current_knowledge_manifest_authority() -> (String, ConformanceVectorFile) {
        let root = workspace_root();
        let manifest = fs::read_to_string(root.join(KNOWLEDGE_MANIFEST_RELATIVE))
            .expect("current knowledge manifest");
        let vector =
            parse_json::<ConformanceVectorFile>(&root.join(KNOWLEDGE_MANIFEST_AND_DECODE_RELATIVE))
                .expect("current knowledge manifest conformance vector");
        (manifest, vector)
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("radroots_xtask_{prefix}_{nanos}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    fn required_thresholds() -> CoverageThresholds {
        CoverageThresholds {
            fail_under_exec_lines: COVERAGE_REQUIRED_THRESHOLD,
            fail_under_functions: COVERAGE_REQUIRED_THRESHOLD,
            fail_under_regions: COVERAGE_REQUIRED_THRESHOLD,
            fail_under_branches: COVERAGE_REQUIRED_THRESHOLD,
            require_branches: true,
        }
    }

    fn coverage_thresholds(value: f64, require_branches: bool) -> CoverageThresholds {
        CoverageThresholds {
            fail_under_exec_lines: value,
            fail_under_functions: value,
            fail_under_regions: value,
            fail_under_branches: value,
            require_branches,
        }
    }

    struct TestCoverageRefreshRow<'a> {
        crate_name: &'a str,
        status: &'a str,
        thresholds: CoverageThresholds,
        exec: f64,
        func: f64,
        branch: Option<f64>,
        region: f64,
        report_pass: bool,
    }

    fn passing_coverage_row(crate_name: &str) -> TestCoverageRefreshRow<'_> {
        TestCoverageRefreshRow {
            crate_name,
            status: "pass",
            thresholds: coverage_thresholds(COVERAGE_REQUIRED_THRESHOLD, true),
            exec: 100.0,
            func: 100.0,
            branch: Some(100.0),
            region: 100.0,
            report_pass: true,
        }
    }

    fn coverage_refresh_branch_value(branch: Option<f64>) -> String {
        branch
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    }

    fn write_test_coverage_gate_report(root: &Path, row: &TestCoverageRefreshRow<'_>) -> String {
        let report_relative = format!("target/coverage/{}/gate-report.json", row.crate_name);
        let report_path = root.join(&report_relative);
        let fail_reasons = if row.report_pass {
            Vec::<&str>::new()
        } else {
            vec!["policy gate failed"]
        };
        let report = serde_json::json!({
            "scope": row.crate_name,
            "thresholds": {
                "executable_lines": row.thresholds.fail_under_exec_lines,
                "functions": row.thresholds.fail_under_functions,
                "regions": row.thresholds.fail_under_regions,
                "branches": row.thresholds.fail_under_branches,
                "branches_required": row.thresholds.require_branches
            },
            "measured": {
                "executable_lines_percent": row.exec,
                "executable_lines_source": "da",
                "functions_percent": row.func,
                "branches_percent": row.branch,
                "branches_available": row.branch.is_some(),
                "summary_lines_percent": row.exec,
                "summary_regions_percent": row.region
            },
            "counts": {
                "executable_lines": {
                    "covered": 1,
                    "total": 1
                },
                "branches": {
                    "covered": if row.branch.is_some() { 1 } else { 0 },
                    "total": if row.branch.is_some() { 1 } else { 0 }
                }
            },
            "result": {
                "pass": row.report_pass,
                "fail_reasons": fail_reasons
            }
        });
        let json =
            serde_json::to_string_pretty(&report).expect("serialize test coverage gate report");
        write_file(&report_path, &format!("{json}\n"));
        report_relative
    }

    fn write_test_coverage_refresh(root: &Path, rows: &[TestCoverageRefreshRow<'_>]) {
        let mut refresh_rows = String::from("crate\tstatus\texec\tfunc\tbranch\tregion\treport\n");
        for row in rows {
            let report_relative = write_test_coverage_gate_report(root, row);
            refresh_rows.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                row.crate_name,
                row.status,
                row.exec,
                row.func,
                coverage_refresh_branch_value(row.branch),
                row.region,
                report_relative
            ));
        }
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            &refresh_rows,
        );
    }

    fn create_synthetic_workspace(prefix: &str) -> PathBuf {
        let root = temp_root(prefix);
        write_file(
            &root.join("docs/specs/radroots_crates_release_v1.toml"),
            r#"spec_id = "radroots.crates.release.v1"
package_count = 2

[repositories.lib]
version = "1.0.0"
packages = ["radroots_a", "radroots_b"]

[repositories.sdk]
version = "0.1.0"
packages = []

[[package]]
name = "radroots_a"

[[package]]
name = "radroots_b"
"#,
        );
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
resolver = "2"

[workspace.package]
version = "1.0.0"

[workspace.dependencies]
radroots_a = { path = "crates/a", version = "=1.0.0" }
radroots_b = { path = "crates/b", version = "=1.0.0" }
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
authors = ["Radroots Test"]
rust-version = "1.97"
license = "MIT OR Apache-2.0"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.rs/radroots_a"
readme = "README.md"
keywords = ["radroots"]
categories = ["data-structures"]
include = ["src/**", "tests/**", "README.md", "LICENSE-APACHE", "LICENSE-MIT"]

[package.metadata.docs.rs]
features = []
"#,
        );
        write_file(&root.join("LICENSE-APACHE"), "Apache license\n");
        write_file(&root.join("LICENSE-MIT"), "MIT license\n");
        for relative in ["README.md", "LICENSE-APACHE", "LICENSE-MIT"] {
            let contents = if relative == "README.md" {
                "# radroots_a\n".to_owned()
            } else {
                fs::read_to_string(root.join(relative)).expect("read synthetic package metadata")
            };
            write_file(&root.join("crates").join("a").join(relative), &contents);
        }
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "1.0.0"
edition = "2024"
publish = false

[features]
default = ["std"]
std = []
legacy-ingest = ["std"]
"#,
        );
        write_file(
            &root.join("crates").join("b").join("src").join("lib.rs"),
            r#"#[cfg(feature = "legacy-ingest")]
pub mod ingest;

#[cfg(feature = "legacy-ingest")]
pub use ingest::{radroots_replica_ingest_event, RadrootsReplicaIngestOutcome};
"#,
        );
        write_file(
            &root.join("crates").join("b").join("src").join("types.rs"),
            r#"use serde::{Deserialize, Serialize};

pub const RADROOTS_REPLICA_TRANSFER_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsReplicaFarmSelector;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsReplicaSyncOptions;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadrootsReplicaSyncRequest;
"#,
        );
        write_file(
            &root.join("crates").join("b").join("src").join("emit.rs"),
            r#"pub fn radroots_replica_sync_all_with_options() {}

#[cfg(test)]
mod tests {}
"#,
        );
        write_file(
            &root.join("Cargo.lock"),
            r#"version = 4

[[package]]
name = "radroots_a"
version = "1.0.0"

[[package]]
name = "radroots_b"
version = "1.0.0"
"#,
        );
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum UnitDimension {
    Count,
    Mass,
    Volume,
}
"#,
        );

        write_file(
            &root.join("contracts").join("manifest.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]

[surface.internal_replica_crates]
schema = "radroots_a"
storage = "radroots_b"
sync = "radroots_b"

[policy]
exclude_internal_workspace_crates = true
require_reproducible_exports = true
require_conformance_vectors = true

[policy.replica]
forbid_legacy_alias_identifiers = true
require_transport_agnostic_sync_contract = true
require_deterministic_emit_ingest = true
"#,
        );
        write_file(
            &root.join("contracts").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[release_integrity]
requires_conformance_pass = true
requires_contract_manifest_diff = true
requires_release_notes = true
"#,
        );
        write_file(
            &root.join("contracts").join("replica.toml"),
            r#"schema_version = 1

[contract]
name = "radroots_replica_contract"
version = "1.0.0"
purpose = "synthetic deterministic replica sync"

[crate_family]
schema = "radroots_a"
storage = "radroots_b"
sync = "radroots_b"

[policy]
transport_agnostic_sync_core = true
deterministic_emit_and_ingest = true
forbid_legacy_alias_identifiers = true
profile_event_emission = "excluded"
unknown_sync_request_fields = "reject"
classified_listing_signature_verification = "required_before_state"
classified_listing_head_selection = "raw_before_profile"
classified_listing_operational_projection = "operational_partition_only"
classified_listing_excluded_or_rejected_head = "remove_projection_and_advance"
classified_listing_head_only_ingest = "reject_require_profile_aware"
legacy_bare_envelope_ingest = "explicit_non_default_feature_only"
legacy_ingest_feature = "legacy-ingest"
phase_1_ingest_replacement = "none"
future_product_ingest_input = "store_produced_verified_valid_visible_admission"

[transfer]
version = 2
source = "crates/b/src/types.rs"
constant = "RADROOTS_REPLICA_TRANSFER_VERSION"
"#,
        );
        write_file(
            &root.join("contracts").join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        write_file(
            &root.join(CHANGELOG_RELATIVE),
            "# Changelog\n\n## [1.0.0]\n\n- Synthetic breaking release.\n",
        );
        for (canonical_relative, mirror_relative) in CONFORMANCE_VECTOR_MIRRORS {
            write_file(&root.join(canonical_relative), SYNTHETIC_CONFORMANCE_VECTOR);
            write_file(&root.join(mirror_relative), SYNTHETIC_CONFORMANCE_VECTOR);
        }
        write_file(
            &root.join(RELEASES_ROOT_RELATIVE).join("1.0.0.toml"),
            r#"schema_version = 1

[release]
version = "1.0.0"
previous_version = "0.1.0-alpha.2"
contract_base_version = "1.0.0"
status = "unreleased"

[artifacts]
changelog = "CHANGELOG.md"
manifest = "contracts/manifest.toml"
operations = "contracts/operations.toml"
replica = "contracts/replica.toml"
conformance = "contracts/conformance"
publish_policy = "contracts/releases/publish_policy.toml"

[[changes]]
id = "synthetic-major-release"
classification = "breaking"
semver_impacts = ["breaking"]
summary = "Exercise synthetic major release governance."
"#,
        );
        write_file(
            &root_release_policy_path(&root),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_test_coverage_refresh(
            &root,
            &[
                passing_coverage_row("radroots_a"),
                passing_coverage_row("radroots_b"),
            ],
        );
        add_operation_contract_files(&root);
        root
    }

    fn add_operation_contract_files(root: &Path) {
        write_file(
            &root.join("contracts").join("operations.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[public]
domains = ["profile", "farm", "operational_listing", "trade"]

[shared_types]
public = [
  "Nip01EventWireParts",
  "EventDraft",
  "SignedEvent",
  "EventEnvelope",
  "EventRef",
  "EventPtr",
  "ClassifiedListingAddress",
  "AuthoredProfile",
  "RadrootsInboundProfileMetadata",
  "Farm",
  "OperationalListing",
]

[errors]
classes = ["encode_error", "parse_error", "validation_error", "address_error"]

[implementation_provenance]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]

[operations.profile_build_authored_draft]
domain = "profile"
id = "profile.build_authored_draft"
stability = "beta"
inputs = ["AuthoredProfile"]
outputs = ["Nip01EventWireParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.profile_build_authored_draft.implementation]
rust_modules = ["crates/core/src/unit.rs"]
rust_types = ["radroots_event::profile::AuthoredProfile"]

[operations.profile_build_authored_draft.conformance]
vector = "contracts/conformance/vectors/profile/metadata.v1.json"

[operations.operational_listing_build_draft]
domain = "operational_listing"
id = "operational_listing.build_draft"
stability = "beta"
inputs = ["OperationalListing"]
outputs = ["Nip01EventWireParts"]
error_class = "encode_error"
deterministic = true
signing = "native"
transport = "native"

[operations.operational_listing_build_draft.implementation]
rust_modules = ["crates/core/src/unit.rs"]
rust_types = ["radroots_event::listing::operational::OperationalListing"]

[operations.operational_listing_build_draft.conformance]
vector = "contracts/conformance/vectors/operational_listing/build_draft.v1.json"
"#,
        );
        write_file(
            &root
                .join("contracts")
                .join("conformance")
                .join("schema")
                .join("vector.schema.json"),
            r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://radroots.org/core/conformance/vector.schema.json",
  "title": "radroots core conformance vector",
  "type": "object",
  "required": ["suite", "contract_version", "vectors"],
  "properties": {
    "suite": {
      "type": "string",
      "minLength": 1
    },
    "contract_version": {
      "type": "string",
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$"
    },
    "vectors": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "kind", "input"],
        "properties": {
          "id": {
            "type": "string",
            "minLength": 1
          },
          "kind": {
            "type": "string",
            "minLength": 1
          },
          "input": {},
          "expected": {},
          "expected_error_contains": {
            "type": "string",
            "minLength": 1
          }
        },
        "oneOf": [
          {
            "required": ["expected"],
            "not": {"required": ["expected_error_contains"]}
          },
          {
            "required": ["expected_error_contains"],
            "not": {"required": ["expected"]}
          }
        ],
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
"#,
        );
        write_file(
            &root
                .join("contracts")
                .join("conformance")
                .join("vectors")
                .join("profile")
                .join("metadata.v1.json"),
            SYNTHETIC_CONFORMANCE_VECTOR,
        );
        let operational_listing_vector = r#"{
  "suite": "operational_listing",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "operational_listing_build_draft_minimal_001",
      "kind": "operational_listing.build_draft",
      "input": {},
      "expected": {}
    }
  ]
}
"#;
        write_file(
            &root
                .join("contracts")
                .join("conformance")
                .join("vectors")
                .join("operational_listing")
                .join("build_draft.v1.json"),
            operational_listing_vector,
        );
        write_file(
            &root
                .join("crates")
                .join("event_codec")
                .join("tests")
                .join("fixtures")
                .join("operational_listing_build_draft.v1.json"),
            operational_listing_vector,
        );
    }

    fn write_root_release_policy(root: &Path, raw: &str) {
        write_file(&root_release_policy_path(root), raw);
    }

    fn configure_root_release_policy_workspace(root: &Path) {
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b", "crates/c", "crates/d", "crates/e"]
resolver = "2"

[workspace.package]
version = "1.0.0"

[workspace.dependencies]
radroots_a = { path = "crates/a", version = "=1.0.0" }
radroots_b = { path = "crates/b", version = "=1.0.0" }
radroots_c = { path = "crates/c", version = "=1.0.0" }
radroots_d = { path = "crates/d", version = "=1.0.0" }
radroots_e = { path = "crates/e", version = "=1.0.0" }
"#,
        );
        for crate_name in ["c", "d", "e"] {
            write_file(
                &root.join("crates").join(crate_name).join("Cargo.toml"),
                &format!(
                    r#"[package]
name = "radroots_{crate_name}"
version = "1.0.0"
edition = "2024"
publish = false
"#
                ),
            );
        }
        write_file(
            &root.join("Cargo.lock"),
            r#"version = 4

[[package]]
name = "radroots_a"
version = "1.0.0"

[[package]]
name = "radroots_b"
version = "1.0.0"

[[package]]
name = "radroots_c"
version = "1.0.0"

[[package]]
name = "radroots_d"
version = "1.0.0"

[[package]]
name = "radroots_e"
version = "1.0.0"
"#,
        );
        write_file(
            &root.join("contracts").join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b", "radroots_c", "radroots_d", "radroots_e"]
"#,
        );
        write_test_coverage_refresh(
            root,
            &[
                passing_coverage_row("radroots_a"),
                passing_coverage_row("radroots_b"),
                passing_coverage_row("radroots_c"),
                passing_coverage_row("radroots_d"),
                passing_coverage_row("radroots_e"),
            ],
        );
        let _ = fs::remove_file(root_release_policy_path(root));
    }

    #[test]
    fn validate_current_contract_bundle() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load contract");
        validate_contract_bundle(&bundle).expect("validate contract");
    }

    #[test]
    fn knowledge_manifest_vector_authority_rejects_registry_drift() {
        let (manifest, vector) = current_knowledge_manifest_authority();
        validate_knowledge_manifest_vector_semantics(&manifest, &vector)
            .expect("current knowledge manifest vector authority");

        let (manifest, mut vector) = current_knowledge_manifest_authority();
        let case = vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "knowledge_manifest_fields_valid_001")
            .expect("knowledge manifest case");
        case.input
            .as_object_mut()
            .expect("knowledge manifest input")
            .insert(
                "registry".to_string(),
                Value::String("radroots_event_contract_registry_v1".to_string()),
            );
        let error = validate_knowledge_manifest_vector_semantics(&manifest, &vector)
            .expect_err("stale registry marker must fail");
        assert!(error.contains("registry marker drift"), "{error}");

        let (manifest, mut vector) = current_knowledge_manifest_authority();
        let case = vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "knowledge_manifest_fields_valid_001")
            .expect("knowledge manifest case");
        case.expected
            .as_mut()
            .expect("knowledge manifest expected output")
            .as_object_mut()
            .expect("knowledge manifest expected output object")
            .insert("registry_version".to_string(), Value::from(1_u64));
        let error = validate_knowledge_manifest_vector_semantics(&manifest, &vector)
            .expect_err("stale expected registry version must fail");
        assert!(error.contains("expected registry_version drift"), "{error}");
    }

    #[test]
    fn knowledge_manifest_artifacts_are_atomic_fresh_and_shape_checked() {
        let root = temp_root("knowledge_manifest_artifacts");
        let generated =
            write_knowledge_contract_manifest_artifacts(&root).expect("write manifest artifacts");
        assert_eq!(
            generated,
            expected_knowledge_contract_manifest_json().expect("expected manifest")
        );
        assert_eq!(
            validate_knowledge_contract_manifest_artifacts(&root)
                .expect("fresh manifest artifacts"),
            generated
        );

        let manifest_path = root.join(KNOWLEDGE_MANIFEST_RELATIVE);
        let mut extra_lf = fs::read_to_string(&manifest_path).expect("read manifest");
        extra_lf.push('\n');
        fs::write(&manifest_path, extra_lf).expect("write noncanonical manifest");
        let error = validate_knowledge_contract_manifest_artifacts(&root)
            .expect_err("noncanonical manifest must fail");
        assert!(error.contains("exactly one LF"), "{error}");

        write_knowledge_contract_manifest_artifacts(&root).expect("restore manifest artifacts");
        let mut value: Value =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read restored manifest"))
                .expect("parse restored manifest");
        value["contract_count"] = Value::from(0_u64);
        let mut mismatched = serde_json::to_string_pretty(&value).expect("serialize mismatch");
        mismatched.push('\n');
        fs::write(&manifest_path, mismatched).expect("write count mismatch");
        let error = validate_knowledge_contract_manifest_artifacts(&root)
            .expect_err("count mismatch must fail");
        assert!(error.contains("contract_count"), "{error}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn strict_post_release_record_governs_public_boundary_breaks() {
        let root = workspace_root();
        let bundle = load_contract_bundle(&root).expect("load current contract bundle");
        let major_impacts = bundle
            .version
            .semver
            .major_on
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for impact in [
            "add_exported_field",
            "change_exported_function_signature",
            "change_exported_constant_value",
        ] {
            assert!(
                major_impacts.contains(impact),
                "version policy must govern {impact} as a major impact"
            );
        }
        assert!(
            bundle
                .version
                .semver
                .minor_on
                .iter()
                .any(|impact| impact == "add_exported_constant"),
            "version policy must govern add_exported_constant as a minor impact"
        );

        let release =
            parse_toml::<ReleaseRecord>(&root.join("contracts/releases/1.0.0-alpha.1.toml"))
                .expect("current release record");
        let change = release
            .changes
            .iter()
            .find(|change| change.id == "strict-kind-one-product-profiles")
            .expect("strict kind-one release change");
        let impacts = change
            .semver_impacts
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for impact in [
            "remove_exported_type",
            "add_exported_constant",
            "add_exported_field",
            "change_exported_function_signature",
            "change_exported_enum_variant",
            "change_exported_constant_value",
        ] {
            assert!(
                impacts.contains(impact),
                "strict kind-one release change must declare {impact}"
            );
        }
    }

    #[test]
    fn post_operation_authority_rejects_manifest_and_inventory_drift() {
        let (manifest, vector) = current_post_authority();
        validate_post_operation_inventory(&manifest, &vector).expect("current post authority");

        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .remove("social_update_build_authored_draft");
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("missing post operation must fail");
        assert!(error.contains("post operation authority drift"));

        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation")
            .id = "social.update.wrong".to_string();
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("wrong post operation ID must fail");
        assert!(error.contains("id drift"));

        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation")
            .conformance
            .vector = "contracts/conformance/vectors/profile/metadata.v1.json".to_string();
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("wrong post vector path must fail");
        assert!(error.contains("conformance.vector drift"));

        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation")
            .conformance
            .case_kinds[0] = "social.ask.build_authored_draft.valid".to_string();
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("wrong post case prefix must fail");
        assert!(error.contains("must start with social.update.build_authored_draft."));

        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation")
            .conformance
            .case_kinds
            .pop();
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("missing post case kind must fail");
        assert!(error.contains("conformance.case_kinds drift"));

        let (mut manifest, vector) = current_post_authority();
        let operation = manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation");
        operation.conformance.case_kinds[1] = operation.conformance.case_kinds[0].clone();
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("duplicate post case kind must fail");
        assert!(error.contains("duplicate value"), "{error}");

        let (manifest, mut vector) = current_post_authority();
        vector.vectors.push(ConformanceVectorEntry {
            id: "unclaimed_post_case".to_string(),
            kind: "social.post.unclaimed.valid".to_string(),
            input: Value::Object(Default::default()),
            expected: Some(Value::Object(Default::default())),
            expected_error_contains: None,
        });
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("unclaimed post vector kind must fail");
        assert!(error.contains("is not claimed by exactly one operation"));

        let (manifest, mut vector) = current_post_authority();
        vector.vectors.remove(
            vector
                .vectors
                .iter()
                .position(|entry| entry.kind == "social.post.project_verified_event.valid")
                .expect("project valid case"),
        );
        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("post vector count drift must fail");
        assert!(error.contains("post conformance vector inventory drift"));
    }

    #[test]
    fn verified_admission_authority_rejects_manifest_fixture_and_secret_drift() {
        let (manifest, vector) = current_admission_authority();
        admission_authority::validate_admission_operation_inventory(&manifest, &vector)
            .expect("current verified admission authority");

        let (mut manifest, vector) = current_admission_authority();
        manifest.operations.remove("event_admit_verified");
        let error = admission_authority::validate_admission_operation_inventory(&manifest, &vector)
            .expect_err("missing central admission operation must fail");
        assert!(error.contains("operation authority drift"), "{error}");

        let (mut manifest, vector) = current_admission_authority();
        manifest
            .shared_types
            .public
            .retain(|value| value != "RadrootsEventAdmissionError");
        let error = admission_authority::validate_admission_operation_inventory(&manifest, &vector)
            .expect_err("missing central admission public type must fail");
        assert!(error.contains("requires shared public type"), "{error}");

        let (manifest, mut vector) = current_admission_authority();
        vector.vectors[0]
            .input
            .as_object_mut()
            .expect("admission input")
            .insert(
                "secret_key".to_string(),
                Value::String("forbidden".to_string()),
            );
        let error = admission_authority::validate_admission_operation_inventory(&manifest, &vector)
            .expect_err("fixture secret material must fail exact input inventory");
        assert!(error.contains("field inventory drift"), "{error}");

        let (manifest, mut vector) = current_admission_authority();
        vector.vectors[0].id = "renamed_admission_case".to_string();
        let error = admission_authority::validate_admission_operation_inventory(&manifest, &vector)
            .expect_err("renamed admission vector must fail exact inventory");
        assert!(error.contains("unexpected id"), "{error}");
    }

    #[test]
    fn post_operation_authority_rejects_another_vector_namespace_operation() {
        let (mut manifest, vector) = current_post_authority();
        let mut unexpected = manifest
            .operations
            .remove("social_reaction_build_tags")
            .expect("unrelated social operation");
        unexpected.id = "social.update.shadow".to_string();
        manifest
            .operations
            .insert("social_update_shadow".to_string(), unexpected);

        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("another-vector post namespace operation must fail");
        assert!(error.contains("post operation authority drift"), "{error}");
        assert!(error.contains("social_update_shadow"), "{error}");
    }

    #[test]
    fn post_operation_authority_rejects_metadata_drift() {
        let (mut manifest, vector) = current_post_authority();
        manifest
            .operations
            .get_mut("social_update_build_authored_draft")
            .expect("Update operation")
            .stability = "stable".to_string();

        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("post operation metadata drift must fail");
        assert!(error.contains("stability drift"), "{error}");
    }

    #[test]
    fn post_operation_authority_rejects_required_public_type_removal() {
        let (mut manifest, vector) = current_post_authority();
        manifest
            .shared_types
            .public
            .retain(|value| value != "RadrootsPostAdmissionOutcome");

        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("required post public type removal must fail");
        assert!(
            error.contains(
                "post operation authority requires shared public type RadrootsPostAdmissionOutcome"
            ),
            "{error}"
        );
    }

    #[test]
    fn post_operation_authority_rejects_same_count_vector_id_replacement() {
        let (manifest, mut vector) = current_post_authority();
        vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "authored_update_wire")
            .expect("authored Update case")
            .id = "authored_update_wire_replacement".to_string();

        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("same-count post vector ID replacement must fail");
        assert!(error.contains("post conformance vector inventory drift"));
    }

    #[test]
    fn post_operation_authority_rejects_vector_id_kind_drift() {
        let (manifest, mut vector) = current_post_authority();
        let update_position = vector
            .vectors
            .iter()
            .position(|entry| entry.id == "authored_update_wire")
            .expect("authored Update case");
        let ask_position = vector
            .vectors
            .iter()
            .position(|entry| entry.id == "authored_ask_wire")
            .expect("authored Ask case");
        let update_kind = vector.vectors[update_position].kind.clone();
        vector.vectors[update_position].kind = vector.vectors[ask_position].kind.clone();
        vector.vectors[ask_position].kind = update_kind;

        let error = validate_post_operation_inventory(&manifest, &vector)
            .expect_err("post vector ID-to-kind drift must fail");
        assert!(error.contains("post conformance vector inventory drift"));
    }

    #[test]
    fn comment_operation_authority_rejects_contract_drift() {
        let (manifest, vector) = current_comment_authority();
        validate_comment_operation_inventory(&manifest, &vector)
            .expect("current Comment operation authority");

        let (mut manifest, vector) = current_comment_authority();
        manifest
            .operations
            .remove("social_comment_build_authored_draft");
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("missing Comment operation must fail");
        assert!(
            error.contains("comment operation authority drift"),
            "{error}"
        );

        let (mut manifest, vector) = current_comment_authority();
        let renamed = manifest
            .operations
            .remove("social_comment_project_verified_event")
            .expect("Comment projection operation");
        manifest
            .operations
            .insert("social_comment_project_event".to_string(), renamed);
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("renamed Comment operation must fail");
        assert!(
            error.contains("comment operation authority drift"),
            "{error}"
        );
        assert!(error.contains("social_comment_project_event"), "{error}");

        let (mut manifest, vector) = current_comment_authority();
        manifest
            .operations
            .get_mut("social_comment_verify_and_admit_event")
            .expect("Comment admission operation")
            .signing = "none".to_string();
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("Comment operation metadata drift must fail");
        assert!(error.contains("signing drift"), "{error}");

        let (mut manifest, vector) = current_comment_authority();
        manifest
            .shared_types
            .public
            .retain(|value| value != "RadrootsInboundNip22TopLevelEventReference");
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("required Comment public type removal must fail");
        assert!(
            error.contains(
                "comment operation authority requires shared public type RadrootsInboundNip22TopLevelEventReference"
            ),
            "{error}"
        );
    }

    #[test]
    fn comment_operation_authority_rejects_vector_inventory_drift() {
        let (manifest, mut vector) = current_comment_authority();
        vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "authored_top_event_listing")
            .expect("authored top-level event case")
            .id = "authored_top_event_listing_replacement".to_string();
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("same-count Comment vector ID replacement must fail");
        assert!(
            error.contains("comment conformance vector inventory drift"),
            "{error}"
        );

        let (manifest, mut vector) = current_comment_authority();
        vector.vectors.push(ConformanceVectorEntry {
            id: "unclaimed_comment_case".to_string(),
            kind: "social.comment.project_verified_event.shadow".to_string(),
            input: Value::Object(Default::default()),
            expected: Some(Value::Object(Default::default())),
            expected_error_contains: None,
        });
        let error = validate_comment_operation_inventory(&manifest, &vector)
            .expect_err("unclaimed Comment vector kind must fail");
        assert!(
            error.contains("is not claimed by exactly one operation"),
            "{error}"
        );
    }

    #[test]
    fn comment_vector_namespace_rejects_legacy_owners() {
        let canonical = PathBuf::from(COMMENT_CONFORMANCE_VECTOR_RELATIVE);
        let legacy = PathBuf::from("contracts/conformance/vectors/social/mvp.v1.json");
        let vector = ConformanceVectorFile {
            suite: "legacy".to_string(),
            contract_version: "1.0.0".to_string(),
            vectors: vec![ConformanceVectorEntry {
                id: "legacy_comment".to_string(),
                kind: "social.comment.build_tags".to_string(),
                input: Value::Object(Default::default()),
                expected: Some(Value::Object(Default::default())),
                expected_error_contains: None,
            }],
        };

        let error = validate_comment_vector_namespace(&legacy, &canonical, &vector)
            .expect_err("legacy Comment vector namespace must fail");
        assert!(error.contains("outside canonical vector"), "{error}");
        validate_comment_vector_namespace(&canonical, &canonical, &vector)
            .expect("canonical Comment vector owns the namespace");
    }

    #[test]
    fn deletion_operation_authority_rejects_contract_drift() {
        let (manifest, request_vector, suppression_vector) = current_deletion_authority();
        validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
            .expect("current deletion operation authority");

        let (mut manifest, request_vector, suppression_vector) = current_deletion_authority();
        manifest
            .operations
            .remove("social_deletion_request_project_verified_event");
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("missing deletion operation must fail");
        assert!(
            error.contains("deletion operation authority drift"),
            "{error}"
        );

        let (mut manifest, request_vector, suppression_vector) = current_deletion_authority();
        manifest
            .shared_types
            .public
            .push("RadrootsNip09DeletionUnauthorizedEffect".to_string());
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("unexpected deletion public type must fail");
        assert!(
            error.contains("deletion operation public-type authority drift"),
            "{error}"
        );

        let (mut manifest, request_vector, suppression_vector) = current_deletion_authority();
        manifest
            .operations
            .get_mut("social_deletion_request_verify_and_admit_event")
            .expect("admission operation")
            .conformance
            .case_kinds
            .pop();
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("missing deletion case kind must fail");
        assert!(error.contains("conformance.case_kinds drift"), "{error}");

        let (mut manifest, request_vector, suppression_vector) = current_deletion_authority();
        manifest
            .operations
            .get_mut("social_deletion_request_evaluate_suppression")
            .expect("suppression operation")
            .conformance
            .vector = DELETION_CONFORMANCE_VECTOR_RELATIVE.to_string();
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("suppression vector ownership drift must fail");
        assert!(error.contains("conformance.vector drift"), "{error}");
    }

    #[test]
    fn deletion_operation_authority_rejects_vector_inventory_drift() {
        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_authored_event_target_min_kind_empty_content")
            .expect("authored deletion vector")
            .id = "nip09_authored_event_target_min_kind_replacement".to_string();
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("same-count deletion vector ID replacement must fail");
        assert!(
            error.contains("deletion conformance vector inventory drift"),
            "{error}"
        );

        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        let source = request_vector
            .vectors
            .iter()
            .find(|entry| entry.id == "nip09_project_signed_event_target_without_k")
            .expect("valid signed deletion vector");
        let input = source.input.clone();
        let expected = source.expected.clone();
        request_vector.vectors.push(ConformanceVectorEntry {
            id: "nip09_unclaimed_case".to_string(),
            kind: "social.deletion_request.unclaimed.valid".to_string(),
            input,
            expected,
            expected_error_contains: None,
        });
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("unclaimed deletion vector kind must fail");
        assert!(
            error.contains("is not claimed by exactly one operation"),
            "{error}"
        );

        let (manifest, request_vector, mut suppression_vector) = current_deletion_authority();
        suppression_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_suppress_no_requests_visible")
            .expect("visible suppression vector")
            .id = "nip09_suppress_replacement_visible".to_string();
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("same-count suppression vector ID replacement must fail");
        assert!(
            error.contains("deletion suppression conformance vector inventory drift"),
            "{error}"
        );
    }

    #[test]
    fn deletion_operation_authority_rejects_generation_and_effect_metadata() {
        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_authored_event_target_min_kind_empty_content")
            .expect("authored deletion vector")
            .input
            .as_object_mut()
            .expect("authored input")
            .insert("SeEd".to_string(), Value::from(7_u64));
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("generation seed metadata must fail");
        assert!(error.contains("forbidden metadata key"), "{error}");

        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_project_signed_event_target_without_k")
            .expect("signed deletion vector")
            .input
            .as_object_mut()
            .expect("signed input")
            .insert("trace".to_string(), Value::Bool(true));
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("extra signed input key must fail");
        assert!(error.contains("input keys drift"), "{error}");

        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_project_signed_event_target_without_k")
            .expect("signed deletion vector")
            .expected
            .as_mut()
            .expect("projection expected")
            .as_object_mut()
            .expect("projection expected object")
            .insert("AuThOrIzAtIoN".to_string(), Value::Bool(true));
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("effect-authority output must fail");
        assert!(error.contains("forbidden metadata key"), "{error}");

        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_admit_signed_event_target")
            .expect("admission deletion vector")
            .input
            .as_object_mut()
            .expect("signed input")
            .insert(
                "event_json".to_string(),
                Value::String(r#"{"content":"NSEC1FORBIDDEN"}"#.to_string()),
            );
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("nsec material must fail");
        assert!(error.contains("forbidden nsec material"), "{error}");

        let (manifest, mut request_vector, suppression_vector) = current_deletion_authority();
        request_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_admit_signed_event_target")
            .expect("admission deletion vector")
            .input
            .as_object_mut()
            .expect("signed input")
            .insert(
                "event_json".to_string(),
                Value::String(
                    r#"{"content":"prefix10C5304D6C9AE3A1A16F7860F1CC8F5E3A76225A2663B3A989A0D775919B7DF5suffix"}"#
                        .to_string(),
                ),
            );
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("approved fixture secret material must fail");
        assert!(
            error.contains("forbidden approved fixture secret material"),
            "{error}"
        );
    }

    #[test]
    fn deletion_suppression_authority_rejects_shape_and_material_drift() {
        let (manifest, request_vector, mut suppression_vector) = current_deletion_authority();
        suppression_vector.suite = "nip09_suppression_shadow".to_string();
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("suppression suite drift must fail");
        assert!(
            error.contains("deletion suppression conformance suite drift"),
            "{error}"
        );

        let (manifest, request_vector, mut suppression_vector) = current_deletion_authority();
        suppression_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_suppress_no_requests_visible")
            .expect("visible suppression vector")
            .input
            .as_object_mut()
            .expect("suppression input")
            .insert("seed".to_string(), Value::from(7_u64));
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("suppression generation metadata must fail");
        assert!(error.contains("forbidden metadata key"), "{error}");

        let (manifest, request_vector, mut suppression_vector) = current_deletion_authority();
        suppression_vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "nip09_suppress_same_author_event_reference")
            .expect("event-reference suppression vector")
            .expected
            .as_mut()
            .expect("suppression expected")
            .as_object_mut()
            .expect("suppression expected object")
            .get_mut("event_reference")
            .expect("event-reference evidence")
            .as_object_mut()
            .expect("event-reference object")
            .insert("request_id".to_string(), Value::String("0".repeat(64)));
        let error =
            validate_deletion_operation_inventory(&manifest, &request_vector, &suppression_vector)
                .expect_err("evidence not bound to an input request must fail");
        assert!(error.contains("must identify an input request"), "{error}");
    }

    #[test]
    fn deletion_vector_namespace_rejects_alternate_owners() {
        let canonical_request = PathBuf::from(DELETION_CONFORMANCE_VECTOR_RELATIVE);
        let canonical_suppression = PathBuf::from(DELETION_SUPPRESSION_CONFORMANCE_VECTOR_RELATIVE);
        let alternate = PathBuf::from("contracts/conformance/vectors/social/mvp.v1.json");
        let vector = ConformanceVectorFile {
            suite: "alternate".to_string(),
            contract_version: "1.0.0".to_string(),
            vectors: vec![ConformanceVectorEntry {
                id: "nip09_alternate".to_string(),
                kind: "social.deletion_request.build_authored_draft.valid".to_string(),
                input: Value::Object(Default::default()),
                expected: Some(Value::Object(Default::default())),
                expected_error_contains: None,
            }],
        };

        let error = validate_deletion_vector_namespace(
            &alternate,
            &canonical_request,
            &canonical_suppression,
            &vector,
        )
        .expect_err("alternate deletion vector namespace must fail");
        assert!(error.contains("outside canonical vectors"), "{error}");
        validate_deletion_vector_namespace(
            &canonical_request,
            &canonical_request,
            &canonical_suppression,
            &vector,
        )
        .expect("canonical deletion vector owns the namespace");
        validate_deletion_vector_namespace(
            &canonical_suppression,
            &canonical_request,
            &canonical_suppression,
            &vector,
        )
        .expect("canonical deletion suppression vector owns the namespace");
    }

    #[test]
    fn food_availability_operation_authority_rejects_contract_drift() {
        let (manifest, vector) = current_food_availability_authority();
        validate_food_availability_operation_inventory(&manifest, &vector)
            .expect("current FoodAvailability operation authority");

        let (mut manifest, vector) = current_food_availability_authority();
        manifest
            .operations
            .remove("food_availability_build_authored_draft");
        let error = validate_food_availability_operation_inventory(&manifest, &vector)
            .expect_err("missing FoodAvailability operation must fail");
        assert!(
            error.contains("food availability operation authority drift"),
            "{error}"
        );

        let (mut manifest, vector) = current_food_availability_authority();
        manifest
            .shared_types
            .public
            .retain(|value| value != "RadrootsFoodAvailabilityRevisionError");
        let error = validate_food_availability_operation_inventory(&manifest, &vector)
            .expect_err("missing FoodAvailability public type must fail");
        assert!(
            error.contains(
                "food availability operation authority requires shared public type RadrootsFoodAvailabilityRevisionError"
            ),
            "{error}"
        );

        let (mut manifest, vector) = current_food_availability_authority();
        manifest
            .operations
            .get_mut("food_availability_validate_revision")
            .expect("revision operation")
            .conformance
            .case_kinds
            .pop();
        let error = validate_food_availability_operation_inventory(&manifest, &vector)
            .expect_err("missing FoodAvailability case kind must fail");
        assert!(error.contains("conformance.case_kinds drift"), "{error}");
    }

    #[test]
    fn food_availability_operation_authority_rejects_vector_inventory_drift() {
        let (manifest, mut vector) = current_food_availability_authority();
        vector
            .vectors
            .iter_mut()
            .find(|entry| entry.id == "food_admission_normalizes_decimal_currency_014")
            .expect("normalization vector")
            .id = "food_admission_normalizes_decimal_currency_replacement".to_string();
        let error = validate_food_availability_operation_inventory(&manifest, &vector)
            .expect_err("same-count FoodAvailability vector replacement must fail");
        assert!(
            error.contains("food availability conformance vector inventory drift"),
            "{error}"
        );

        let (manifest, mut vector) = current_food_availability_authority();
        vector.vectors.push(ConformanceVectorEntry {
            id: "food_unclaimed_case".to_string(),
            kind: "food_availability.unclaimed.valid".to_string(),
            input: Value::Object(Default::default()),
            expected: Some(Value::Object(Default::default())),
            expected_error_contains: None,
        });
        let error = validate_food_availability_operation_inventory(&manifest, &vector)
            .expect_err("unclaimed FoodAvailability vector kind must fail");
        assert!(
            error.contains("is not claimed by exactly one operation"),
            "{error}"
        );
    }

    #[test]
    fn version_governance_rejects_contract_workspace_and_lock_drift() {
        let root = create_synthetic_workspace("version_governance_drift");
        let mut bundle = load_contract_bundle(&root).expect("load contract");
        bundle.version.contract.version = "1.0.1".to_string();
        let contract_error = validate_contract_version_lockstep(&bundle)
            .expect_err("contract header drift must fail");
        assert!(contract_error.contains("must match manifest contract version"));

        let member_path = root.join("crates/a/Cargo.toml");
        let member = fs::read_to_string(&member_path).expect("read member manifest");
        write_file(
            &member_path,
            &member.replace("version = \"1.0.0\"", "version.workspace = true"),
        );
        let member_error = validate_workspace_version_lockstep(&root, "1.0.0")
            .expect_err("inherited member version must fail");
        assert!(member_error.contains("must set an explicit package version"));
        write_file(&member_path, &member);

        let workspace_path = root.join("Cargo.toml");
        let workspace = fs::read_to_string(&workspace_path).expect("read workspace manifest");
        write_file(
            &workspace_path,
            &workspace.replacen(
                "radroots_a = { path = \"crates/a\", version = \"=1.0.0\" }",
                "radroots_a = { path = \"crates/a\", version = \"1.0.0\" }",
                1,
            ),
        );
        let requirement_error = validate_workspace_version_lockstep(&root, "1.0.0")
            .expect_err("non-exact internal dependency must fail");
        assert!(requirement_error.contains("exact requirement =1.0.0"));
        write_file(&workspace_path, &workspace);

        let lock_path = root.join("Cargo.lock");
        let lock = fs::read_to_string(&lock_path).expect("read Cargo.lock");
        write_file(&lock_path, &lock.replacen("1.0.0", "1.0.1", 1));
        let lock_error = validate_workspace_version_lockstep(&root, "1.0.0")
            .expect_err("lockfile version drift must fail");
        assert!(lock_error.contains("Cargo.lock package radroots_a version"));

        write_file(
            &root.join("docs/specs/radroots_crates_release_v1.toml"),
            r#"spec_id = "radroots.crates.release.v1"
package_count = 0
package = []

[repositories.lib]
version = "0.1.0-alpha"
packages = []

[repositories.sdk]
version = "0.1.0"
packages = []
"#,
        );
        let architecture_error = validate_workspace_version_lockstep(&root, "1.0.0")
            .expect_err("repository version authority must override protocol version");
        assert!(architecture_error.contains("must match library repository version 0.1.0-alpha"));

        assert!(parse_semver_version("01.0.0").is_err());
        assert!(parse_semver_version("1.0").is_err());
        assert!(parse_semver_version("1.0.0-alpha_1").is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replica_contract_governance_rejects_metadata_policy_and_transfer_drift() {
        let root = create_synthetic_workspace("replica_contract_drift");
        let bundle = load_contract_bundle(&root).expect("load synthetic contract");
        validate_replica_contract(&bundle, &root).expect("validate replica contract");

        let assert_replica_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load synthetic contract");
            mutator(&mut bundle);
            let error = validate_replica_contract(&bundle, &root)
                .expect_err("replica contract drift must fail");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        };

        assert_replica_error("schema_version must be 1", |bundle| {
            bundle.replica.schema_version = 2;
        });
        assert_replica_error("name must be radroots_replica_contract", |bundle| {
            bundle.replica.contract.name = "replica".to_string();
        });
        assert_replica_error("must match manifest contract version", |bundle| {
            bundle.replica.contract.version = "1.0.1".to_string();
        });
        assert_replica_error("purpose is required", |bundle| {
            bundle.replica.contract.purpose.clear();
        });
        assert_replica_error("crate_family.schema", |bundle| {
            bundle.replica.crate_family.schema = "radroots_b".to_string();
        });
        assert_replica_error("must name a workspace package", |bundle| {
            bundle.replica.crate_family.schema = "radroots_missing".to_string();
            bundle
                .manifest
                .surface
                .internal_replica_crates
                .as_mut()
                .expect("replica family")
                .schema = "radroots_missing".to_string();
        });
        assert_replica_error("policy.replica is required", |bundle| {
            bundle.manifest.policy.replica = None;
        });
        assert_replica_error("transport_agnostic_sync_core", |bundle| {
            bundle.replica.policy.transport_agnostic_sync_core = false;
        });
        assert_replica_error("profile_event_emission must be excluded", |bundle| {
            bundle.replica.policy.profile_event_emission = "included".to_string();
        });
        assert_replica_error("unknown_sync_request_fields must be reject", |bundle| {
            bundle.replica.policy.unknown_sync_request_fields = "ignore".to_string();
        });
        assert_replica_error(
            "classified_listing_signature_verification must be required_before_state",
            |bundle| {
                bundle
                    .replica
                    .policy
                    .classified_listing_signature_verification = "unchecked".to_string();
            },
        );
        assert_replica_error(
            "classified_listing_head_selection must be raw_before_profile",
            |bundle| {
                bundle.replica.policy.classified_listing_head_selection =
                    "profile_before_raw".to_string();
            },
        );
        assert_replica_error(
            "classified_listing_operational_projection must be operational_partition_only",
            |bundle| {
                bundle
                    .replica
                    .policy
                    .classified_listing_operational_projection = "all_partitions".to_string();
            },
        );
        assert_replica_error(
            "classified_listing_excluded_or_rejected_head must be remove_projection_and_advance",
            |bundle| {
                bundle
                    .replica
                    .policy
                    .classified_listing_excluded_or_rejected_head = "retain_projection".to_string();
            },
        );
        assert_replica_error(
            "classified_listing_head_only_ingest must be reject_require_profile_aware",
            |bundle| {
                bundle.replica.policy.classified_listing_head_only_ingest =
                    "allow_head_only".to_string();
            },
        );
        assert_replica_error(
            "legacy_bare_envelope_ingest must be explicit_non_default_feature_only",
            |bundle| {
                bundle.replica.policy.legacy_bare_envelope_ingest = "default".to_string();
            },
        );
        assert_replica_error("legacy_ingest_feature must be legacy-ingest", |bundle| {
            bundle.replica.policy.legacy_ingest_feature = "std".to_string();
        });
        assert_replica_error("phase_1_ingest_replacement must be none", |bundle| {
            bundle.replica.policy.phase_1_ingest_replacement = "legacy".to_string();
        });
        assert_replica_error(
            "future_product_ingest_input must be store_produced_verified_valid_visible_admission",
            |bundle| {
                bundle.replica.policy.future_product_ingest_input = "bare_envelope".to_string();
            },
        );
        assert_replica_error("transfer.version must be 2", |bundle| {
            bundle.replica.transfer.version = 1;
        });
        assert_replica_error("transfer.constant", |bundle| {
            bundle.replica.transfer.constant = "REPLICA_VERSION".to_string();
        });
        assert_replica_error("transfer.source", |bundle| {
            bundle.replica.transfer.source = "crates/a/src/types.rs".to_string();
        });

        let source_path = root.join("crates/b/src/types.rs");
        let source = fs::read_to_string(&source_path).expect("read replica types source");
        write_file(
            &source_path,
            &source.replace(
                "pub const RADROOTS_REPLICA_TRANSFER_VERSION: u32 = 2;",
                "pub const RADROOTS_REPLICA_TRANSFER_VERSION: u32 = 1;",
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load source-drift contract");
        let source_error = validate_replica_contract(&bundle, &root)
            .expect_err("source constant version drift must fail");
        assert!(source_error.contains("source constant"));
        assert!(source_error.contains("must match contract version 2"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replica_policy_source_witnesses_reject_runtime_drift() {
        let root = create_synthetic_workspace("replica_policy_source_drift");
        let cargo_path = root.join("crates/b/Cargo.toml");
        let lib_path = root.join("crates/b/src/lib.rs");
        let types_path = root.join("crates/b/src/types.rs");
        let emit_path = root.join("crates/b/src/emit.rs");
        let cargo = fs::read_to_string(&cargo_path).expect("read replica cargo manifest");
        let lib = fs::read_to_string(&lib_path).expect("read replica lib source");
        let types = fs::read_to_string(&types_path).expect("read replica types source");
        let emit = fs::read_to_string(&emit_path).expect("read replica emit source");

        write_file(
            &cargo_path,
            &cargo.replace("std = []", "std = [\"legacy-ingest\"]"),
        );
        let bundle =
            load_contract_bundle(&root).expect("load transitive default-feature drift contract");
        let default_feature_error = validate_replica_contract(&bundle, &root)
            .expect_err("transitively default legacy ingest feature must fail");
        assert!(default_feature_error.contains("must not be enabled by default features"));

        write_file(&cargo_path, &cargo);
        write_file(
            &lib_path,
            &format!(
                "{}\n/*\n#[cfg(feature = \"legacy-ingest\")]\npub mod ingest;\n*/\n",
                lib.replace(
                    "#[cfg(feature = \"legacy-ingest\")]\npub mod ingest;",
                    "#[cfg(feature = \"std\")]\npub mod ingest;",
                )
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load ingest-module drift contract");
        let module_error = validate_replica_contract(&bundle, &root)
            .expect_err("comment-only legacy guard witness must fail");
        assert!(module_error.contains("must be guarded by exact"));

        write_file(
            &lib_path,
            &format!("{lib}\npub use ingest::RadrootsReplicaIngestOutcome;\n"),
        );
        let bundle = load_contract_bundle(&root).expect("load second-reexport drift contract");
        let reexport_error = validate_replica_contract(&bundle, &root)
            .expect_err("ungated second ingest re-export must fail");
        assert!(reexport_error.contains("every public replica ingest re-export"));

        write_file(
            &lib_path,
            &lib.replacen(
                "#[cfg(feature = \"legacy-ingest\")]\npub use ingest::{",
                "#[cfg(any(feature = \"legacy-ingest\", feature = \"std\"))]\npub use ingest::{",
                1,
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load broadened-reexport contract");
        let broadened_error = validate_replica_contract(&bundle, &root)
            .expect_err("broadened ingest re-export guard must fail");
        assert!(broadened_error.contains("every public replica ingest re-export"));

        write_file(
            &lib_path,
            &format!(
                "{lib}\npub mod nested {{\n    pub use super::ingest::RadrootsReplicaIngestOutcome;\n}}\n"
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load nested-reexport drift contract");
        let nested_error = validate_replica_contract(&bundle, &root)
            .expect_err("ungated nested public ingest re-export must fail");
        assert!(nested_error.contains("every public replica ingest re-export"));

        write_file(&lib_path, &lib);

        write_file(
            &types_path,
            &types.replace(
                "#[serde(deny_unknown_fields)]\npub struct RadrootsReplicaSyncOptions",
                "pub struct RadrootsReplicaSyncOptions",
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load attribute-drift contract");
        let attribute_error = validate_replica_contract(&bundle, &root)
            .expect_err("missing fail-closed request attribute must fail");
        assert!(attribute_error.contains("RadrootsReplicaSyncOptions"));
        assert!(attribute_error.contains("immediately before"));

        write_file(
            &types_path,
            &format!("{types}\npub struct include_profiles;\n"),
        );
        let bundle = load_contract_bundle(&root).expect("load retired-option contract");
        let retired_error = validate_replica_contract(&bundle, &root)
            .expect_err("retired include_profiles identifier must fail");
        assert!(retired_error.contains("include_profiles is forbidden"));

        write_file(&types_path, &types);
        write_file(
            &emit_path,
            &emit.replace(
                "#[cfg(test)]\nmod tests {}",
                "fn emit_profile_event() {}\n\n#[cfg(test)]\nmod tests {}",
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load profile-emitter contract");
        let profile_error = validate_replica_contract(&bundle, &root)
            .expect_err("Profile-related production emitter must fail");
        assert!(profile_error.contains("Profile-related identifier emit_profile_event"));

        write_file(
            &emit_path,
            &emit.replace(
                "#[cfg(test)]\nmod tests {}",
                "fn emit_kind_zero() { let _event = Event { kind: 0 }; }\n\n#[cfg(test)]\nmod tests {}",
            ),
        );
        let bundle = load_contract_bundle(&root).expect("load literal-kind-zero contract");
        let kind_error = validate_replica_contract(&bundle, &root)
            .expect_err("literal kind-0 production emitter must fail");
        assert!(kind_error.contains("must not construct a literal kind-0 event"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replica_contract_is_required_and_rejects_unknown_fields() {
        let missing_root = create_synthetic_workspace("replica_contract_missing");
        fs::remove_file(missing_root.join(REPLICA_CONTRACT_RELATIVE))
            .expect("remove replica contract");
        let missing_error = load_contract_bundle(&missing_root)
            .expect_err("missing replica contract must fail bundle loading");
        assert!(missing_error.contains(REPLICA_CONTRACT_RELATIVE));
        let _ = fs::remove_dir_all(missing_root);

        let unknown_root = create_synthetic_workspace("replica_contract_unknown_field");
        let replica_path = unknown_root.join(REPLICA_CONTRACT_RELATIVE);
        let replica = fs::read_to_string(&replica_path).expect("read replica contract");
        write_file(&replica_path, &format!("{replica}unexpected = true\n"));
        let unknown_error = load_contract_bundle(&unknown_root)
            .expect_err("unknown replica contract field must fail bundle loading");
        assert!(unknown_error.contains(REPLICA_CONTRACT_RELATIVE));
        assert!(unknown_error.contains("unexpected"));
        let _ = fs::remove_dir_all(unknown_root);
    }

    #[test]
    fn conformance_vector_mirrors_are_required_even_when_parent_is_deleted() {
        let root = create_synthetic_workspace("required_conformance_mirrors");
        validate_conformance_vector_mirrors(&root).expect("validate synthetic mirrors");

        let (_, mirror_relative) = CONFORMANCE_VECTOR_MIRRORS[0];
        let mirror_path = root.join(mirror_relative);
        fs::remove_file(&mirror_path).expect("remove required mirror");
        let missing_file_error = validate_conformance_vector_mirrors(&root)
            .expect_err("missing required mirror file must fail");
        assert!(missing_file_error.contains(&format!("read {mirror_relative}")));

        write_file(&mirror_path, SYNTHETIC_CONFORMANCE_VECTOR);
        fs::remove_dir_all(mirror_path.parent().expect("mirror parent"))
            .expect("remove required mirror parent");
        let missing_parent_error = validate_conformance_vector_mirrors(&root)
            .expect_err("missing required mirror parent must fail");
        assert!(missing_parent_error.contains(&format!("read {mirror_relative}")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_record_and_conformance_mirror_validation_reject_drift() {
        let root = create_synthetic_workspace("release_record_drift");
        let bundle = load_contract_bundle(&root).expect("load synthetic contract");
        validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect("validate release record");

        let record_path = root.join("contracts/releases/1.0.0.toml");
        let record = fs::read_to_string(&record_path).expect("read release record");
        write_file(
            &record_path,
            &record.replace("status = \"unreleased\"", "status = \"pending\""),
        );
        let status_error = validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect_err("unsupported release status must fail");
        assert!(status_error.contains("must be unreleased, released, or yanked"));

        write_file(
            &record_path,
            &record.replace(
                "replica = \"contracts/replica.toml\"",
                "replica = \"contracts/replica-v1.toml\"",
            ),
        );
        let replica_artifact_error =
            validate_release_record(&root, "1.0.0", &bundle.version.semver)
                .expect_err("noncanonical replica artifact must fail");
        assert!(replica_artifact_error.contains(
            "release artifact path contracts/replica-v1.toml must use canonical path contracts/replica.toml"
        ));

        write_file(
            &record_path,
            &record.replace(
                "operations = \"contracts/operations.toml\"",
                "operations = \"contracts/operations-v1.toml\"",
            ),
        );
        let operations_artifact_error =
            validate_release_record(&root, "1.0.0", &bundle.version.semver)
                .expect_err("noncanonical operations artifact must fail");
        assert!(operations_artifact_error.contains(
            "release artifact path contracts/operations-v1.toml must use canonical path contracts/operations.toml"
        ));

        write_file(&record_path, &record);
        let operations_path = root.join("contracts").join("operations.toml");
        let operations = fs::read_to_string(&operations_path).expect("read operations manifest");
        fs::remove_file(&operations_path).expect("remove operations manifest");
        let missing_operations_error =
            validate_release_record(&root, "1.0.0", &bundle.version.semver)
                .expect_err("missing operations artifact must fail");
        assert!(
            missing_operations_error
                .contains("release artifact contracts/operations.toml does not exist")
        );
        write_file(&operations_path, &operations);

        write_file(
            &record_path,
            &record.replace("id = \"synthetic-major-release\"", "id = \"Bad_Id\""),
        );
        let id_error = validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect_err("invalid release change id must fail");
        assert!(id_error.contains("lowercase kebab-case"));
        write_file(&record_path, &record);

        write_file(
            &record_path,
            &record.replace(
                "semver_impacts = [\"breaking\"]",
                "semver_impacts = [\"fix\"]",
            ),
        );
        let classification_error = validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect_err("classification and semver impact mismatch must fail");
        assert!(classification_error.contains("does not match its governed semver impacts"));

        write_file(
            &record_path,
            &record.replace(
                "semver_impacts = [\"breaking\"]",
                "semver_impacts = [\"unknown-impact\"]",
            ),
        );
        let impact_error = validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect_err("unknown semver impact must fail");
        assert!(impact_error.contains("is not governed by contracts/version.toml"));
        write_file(&record_path, &record);

        write_file(&root.join(CHANGELOG_RELATIVE), "# Changelog\n");
        let notes_error = validate_release_record(&root, "1.0.0", &bundle.version.semver)
            .expect_err("missing release notes must fail");
        assert!(notes_error.contains("missing heading"));

        let (canonical_relative, mirror_relative) = CONFORMANCE_VECTOR_MIRRORS[0];
        write_file(&root.join(canonical_relative), "canonical\n");
        write_file(&root.join(mirror_relative), "drifted\n");
        let mirror_error = validate_conformance_vector_mirrors(&root)
            .expect_err("packaged conformance drift must fail");
        assert!(mirror_error.contains("must exactly match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn calendar_operation_authority_reports_exact_signature_drift() {
        let root = workspace_root();
        let mut bundle = load_contract_bundle(&root).expect("load contract");
        let manifest = &mut bundle.operations_manifest;
        let shared_types =
            collect_non_empty_set(&manifest.shared_types.public, "shared_types.public")
                .expect("shared public types");
        manifest
            .operations
            .get_mut("social_calendar_date_event_build_authored_draft")
            .expect("calendar authored operation")
            .outputs = vec!["NostrTags".to_string()];

        let error = validate_calendar_operation_authority(manifest, &shared_types)
            .expect_err("calendar operation signature drift");
        assert!(error.contains("outputs drift"));
    }

    #[test]
    fn calendar_operation_authority_reports_obsolete_operation_drift() {
        let root = workspace_root();
        let mut bundle = load_contract_bundle(&root).expect("load contract");
        let manifest = &mut bundle.operations_manifest;
        let shared_types =
            collect_non_empty_set(&manifest.shared_types.public, "shared_types.public")
                .expect("shared public types");
        let obsolete = manifest
            .operations
            .remove("social_calendar_date_event_build_authored_draft")
            .expect("calendar authored operation");
        manifest.operations.insert(
            "social_calendar_date_event_build_tags".to_string(),
            obsolete,
        );

        let error = validate_calendar_operation_authority(manifest, &shared_types)
            .expect_err("obsolete calendar operation drift");
        assert!(error.contains("calendar operation authority drift"));
        assert!(error.contains("social_calendar_date_event_build_tags"));
    }

    #[test]
    fn calendar_operation_authority_reports_obsolete_rsvp_operation_drift() {
        let root = workspace_root();
        let mut bundle = load_contract_bundle(&root).expect("load contract");
        let manifest = &mut bundle.operations_manifest;
        let shared_types =
            collect_non_empty_set(&manifest.shared_types.public, "shared_types.public")
                .expect("shared public types");
        let obsolete = manifest
            .operations
            .remove("social_calendar_rsvp_build_authored_draft")
            .expect("calendar RSVP authored operation");
        manifest
            .operations
            .insert("social_calendar_rsvp_build_tags".to_string(), obsolete);

        let error = validate_calendar_operation_authority(manifest, &shared_types)
            .expect_err("obsolete calendar RSVP operation drift");
        assert!(error.contains("calendar operation authority drift"));
        assert!(error.contains("social_calendar_rsvp_build_tags"));
    }

    #[test]
    fn validate_current_canonical_event_boundary() {
        let root = workspace_root();
        validate_canonical_event_boundary(&root).expect("validate canonical event boundary");
    }

    #[test]
    fn canonical_event_boundary_reports_row_drift() {
        let root = workspace_root();
        let matrix_path =
            resolve_event_boundary_matrix_path_with_override(&root, None).expect("matrix path");
        let raw = fs::read_to_string(&matrix_path).expect("read matrix");
        let drifted = raw.replacen(
            "| message | 14 | Message |",
            "| message | 999 | Message |",
            1,
        );
        let temp = temp_root("event_boundary_drift");
        let override_path = temp.join("spec-coverage.md");
        write_file(&override_path, &drifted);

        let err = validate_canonical_event_boundary_with_override(&root, Some(override_path))
            .expect_err("message kind drift should fail");
        assert!(err.contains("message kind drift"));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn canonical_event_boundary_rejects_deletion_operation_drift() {
        let root = workspace_root();
        let matrix_path =
            resolve_event_boundary_matrix_path_with_override(&root, None).expect("matrix path");
        let raw = fs::read_to_string(&matrix_path).expect("read matrix");
        let (preamble, table) = raw
            .split_once("## Coverage matrix")
            .expect("coverage matrix marker");
        let drifted = format!(
            "{preamble}## Coverage matrix{}",
            table.replacen(
                "social.deletion_request.project_verified_event",
                "social.deletion_request.project_unverified_event",
                1,
            )
        );
        let temp = temp_root("deletion_event_boundary_drift");
        let override_path = temp.join("spec-coverage.md");
        write_file(&override_path, &drifted);

        let error = validate_canonical_event_boundary_with_override(&root, Some(override_path))
            .expect_err("deletion operation drift must fail");
        assert!(error.contains("deletion_request rpc drift"), "{error}");

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn validate_synthetic_operation_contract_bundle() {
        let root = create_synthetic_workspace("operation_contract_bundle");
        add_operation_contract_files(&root);
        let bundle = load_contract_bundle(&root).expect("load contract");
        validate_generic_contract_bundle(&bundle).expect("validate contract");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_enum_variants_in_declared_order() {
        let source = r#"
pub enum UnitDimension {
    Count,
    Mass,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "UnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        assert_eq!(variants, vec!["Count", "Mass", "Volume"]);
    }

    #[test]
    fn fails_when_enum_order_does_not_match_contract() {
        let source = r#"
pub enum UnitDimension {
    Mass,
    Count,
    Volume,
}
"#;
        let enum_body = extract_enum_body(source, "UnitDimension").expect("enum body");
        let variants = parse_enum_variants(enum_body);
        let expected = CORE_UNIT_DIMENSION_ORDER
            .iter()
            .map(|item| (*item).to_string())
            .collect::<Vec<_>>();
        assert_ne!(variants, expected);
    }

    #[test]
    fn coverage_policy_matches_non_simplex_workspace_crates() {
        let root = workspace_root();
        let expected_names =
            coverage_required_workspace_crates(&root).expect("workspace coverage crates");
        let policy = load_coverage_policy(&root.join("contracts")).expect("coverage policy");
        let required_names = policy
            .required_crates()
            .expect("required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(expected_names, required_names);
        assert!(
            required_names
                .iter()
                .all(|crate_name| !coverage_policy_excludes_workspace_crate(crate_name))
        );
    }

    #[test]
    fn coverage_required_workspace_crates_excludes_non_policy_packages() {
        let root = temp_root("coverage_required_workspace_simplex");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/radroots_simplex_probe", "crates/simplex_probe"]
resolver = "2"
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
"#,
        );
        write_file(
            &root
                .join("crates")
                .join("radroots_simplex_probe")
                .join("Cargo.toml"),
            r#"[package]
name = "radroots_simplex_probe"
version = "1.0.0"
edition = "2024"
"#,
        );
        write_file(
            &root.join("crates").join("simplex_probe").join("Cargo.toml"),
            r#"[package]
name = "simplex_probe"
version = "1.0.0"
edition = "2024"
"#,
        );

        let required =
            coverage_required_workspace_crates(&root).expect("workspace coverage crates");
        assert_eq!(
            required,
            ["radroots_a".to_string()]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert!(coverage_policy_excludes_workspace_crate(
            "radroots_simplex_probe"
        ));
        assert!(coverage_policy_excludes_workspace_crate("simplex_probe"));
        assert!(!coverage_policy_excludes_workspace_crate("radroots_a"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_required_crates_match_policy_required_status() {
        let root = workspace_root();
        let contract_root = root.join("contracts");
        let policy = load_coverage_policy(&contract_root).expect("coverage policy");
        let required = CoverageRequiredFile {
            required: CoverageRequiredSection {
                crates: policy.required_crates().expect("coverage required"),
            },
        };
        let required_names = required
            .required
            .crates
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_required = policy
            .required_crates()
            .expect("policy required crates")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(required_names, policy_required);
    }

    #[test]
    fn coverage_policy_required_crates_report_policy_errors() {
        let missing_root = temp_root("load_coverage_required_missing_policy");
        let missing_err =
            load_coverage_policy(&missing_root).expect_err("missing policy should fail");
        assert!(missing_err.contains("coverage.toml"));
        let _ = fs::remove_dir_all(&missing_root);

        let duplicate_root =
            create_synthetic_workspace("load_coverage_required_duplicate_required");
        let contract_root = duplicate_root.join("contracts");
        let coverage_root = coverage_root(&contract_root);
        write_file(
            &coverage_root.join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\", \"radroots_a\"]\n",
        );
        let duplicate_err =
            load_coverage_policy(&contract_root).expect_err("duplicate required crates");
        assert!(duplicate_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_root);
    }

    #[test]
    fn package_field_configured_accepts_workspace_table() {
        let mut package = toml::value::Table::new();
        let mut repository = toml::value::Table::new();
        repository.insert("workspace".to_string(), toml::Value::Boolean(true));
        package.insert("repository".to_string(), toml::Value::Table(repository));
        assert!(package_field_configured(&package, "repository"));
    }

    #[test]
    fn validate_required_coverage_summary_enforces_required_threshold() {
        let root = temp_root("coverage_summary");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t90.0\t90.0\t90.0\t90.0\tfile\n",
        )
        .expect("write coverage file");
        let required = ["radroots_core".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        validate_required_coverage_summary(&root, &required, required_thresholds())
            .expect("coverage summary");

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t90.0\t89.9\t90.0\t90.0\tfile\n",
        )
        .expect("write function coverage file");
        let func_err = validate_required_coverage_summary(&root, &required, required_thresholds())
            .expect_err("function coverage below 90");
        assert!(func_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t90.0\t90.0\t89.9\t90.0\tfile\n",
        )
        .expect("write branch coverage file");
        let branch_err =
            validate_required_coverage_summary(&root, &required, required_thresholds())
                .expect_err("branch coverage below 90");
        assert!(branch_err.contains("must satisfy coverage policy"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t90.0\t90.0\tunavailable\t90.0\tfile\n",
        )
        .expect("write unavailable branch coverage file");
        let missing_branch_err =
            validate_required_coverage_summary(&root, &required, required_thresholds())
                .expect_err("branch coverage missing under strict policy");
        assert!(missing_branch_err.contains("unavailable"));

        fs::write(
            coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_core\tpass\t90.0\t90.0\t90.0\t89.9\tfile\n",
        )
        .expect("write region coverage file");
        let region_err =
            validate_required_coverage_summary(&root, &required, required_thresholds())
                .expect_err("region coverage below 90");
        assert!(region_err.contains("must satisfy coverage policy"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_required_coverage_summary_with_policy_honors_scope_override() {
        let root = temp_root("coverage_summary_override");
        write_test_coverage_refresh(
            &root,
            &[
                TestCoverageRefreshRow {
                    crate_name: "radroots_event_codec",
                    status: "pass",
                    thresholds: CoverageThresholds {
                        fail_under_exec_lines: 100.0,
                        fail_under_functions: 100.0,
                        fail_under_regions: 99.946,
                        fail_under_branches: 100.0,
                        require_branches: true,
                    },
                    exec: 100.0,
                    func: 100.0,
                    branch: Some(100.0),
                    region: 99.946385,
                    report_pass: true,
                },
                TestCoverageRefreshRow {
                    crate_name: "radroots_log",
                    status: "pass",
                    thresholds: coverage_thresholds(100.0, false),
                    exec: 100.0,
                    func: 100.0,
                    branch: None,
                    region: 100.0,
                    report_pass: true,
                },
            ],
        );
        let policy_dir = root.join("contracts");
        fs::create_dir_all(&policy_dir).expect("create policy dir");
        fs::write(
            policy_dir.join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[overrides.radroots_event_codec]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 99.946\nfail_under_branches = 100.0\ntemporary = true\nreason = \"publish 0.1.0-alpha temporary coverage override\"\n\n[overrides.radroots_log]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = false\ntemporary = true\nreason = \"branch coverage is not applicable while the crate has no measured branch records\"\n\n[required]\ncrates = [\"radroots_event_codec\", \"radroots_log\"]\n",
        )
        .expect("write coverage policy");
        let required = [
            "radroots_event_codec".to_string(),
            "radroots_log".to_string(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let policy = read_coverage_policy(&policy_dir.join("coverage.toml"))
            .expect("parse override coverage policy");
        validate_required_coverage_summary_with_policy(&root, &required, &policy)
            .expect("coverage summary should honor override");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_required_coverage_summary_with_policy_rejects_synthetic_report_path() {
        let root = temp_root("coverage_summary_synthetic_report_path");
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n",
        );
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_dir = root.join("contracts");
        write_file(
            &policy_dir.join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let policy =
            read_coverage_policy(&policy_dir.join("coverage.toml")).expect("parse coverage policy");
        let err = validate_required_coverage_summary_with_policy(&root, &required, &policy)
            .expect_err("synthetic report path should fail");
        assert!(err.contains("coverage gate report"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_required_coverage_summary_with_policy_rejects_stale_gate_report_thresholds() {
        let root = temp_root("coverage_summary_stale_gate_report_thresholds");
        let row = TestCoverageRefreshRow {
            crate_name: "radroots_a",
            status: "pass",
            thresholds: coverage_thresholds(90.0, true),
            exec: 100.0,
            func: 100.0,
            branch: Some(100.0),
            region: 100.0,
            report_pass: true,
        };
        write_test_coverage_refresh(&root, &[row]);
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_dir = root.join("contracts");
        write_file(
            &policy_dir.join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let policy =
            read_coverage_policy(&policy_dir.join("coverage.toml")).expect("parse coverage policy");
        let err = validate_required_coverage_summary_with_policy(&root, &required, &policy)
            .expect_err("stale threshold report should fail");
        assert!(err.contains("thresholds do not match policy"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_required_coverage_summary_with_policy_rejects_row_report_mismatch() {
        let root = temp_root("coverage_summary_row_report_mismatch");
        let row = TestCoverageRefreshRow {
            crate_name: "radroots_a",
            status: "pass",
            thresholds: coverage_thresholds(100.0, true),
            exec: 99.0,
            func: 100.0,
            branch: Some(100.0),
            region: 100.0,
            report_pass: true,
        };
        let report_relative = write_test_coverage_gate_report(&root, &row);
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            &format!(
                "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100.0\t100.0\t100.0\t100.0\t{report_relative}\n"
            ),
        );
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let policy_dir = root.join("contracts");
        write_file(
            &policy_dir.join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let policy =
            read_coverage_policy(&policy_dir.join("coverage.toml")).expect("parse coverage policy");
        let err = validate_required_coverage_summary_with_policy(&root, &required, &policy)
            .expect_err("row and report mismatch should fail");
        assert!(err.contains("does not match coverage gate report"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_publish_package_metadata_requires_description() {
        let root = temp_root("publish_metadata");
        fs::create_dir_all(root.join("crates").join("a")).expect("create crate dir");
        fs::write(
            root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
repository = { workspace = true }
homepage = { workspace = true }
documentation = "https://docs.rs/radroots_a"
readme = { workspace = true }
"#,
        )
        .expect("write package manifest");
        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let err =
            validate_publish_package_metadata(&root, &publish).expect_err("missing description");
        assert!(err.contains("package.description"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn synthetic_workspace_validates_contract_and_release_preflight() {
        let root = create_synthetic_workspace("synthetic_valid");
        let bundle = load_contract_bundle(&root).expect("load synthetic bundle");
        validate_generic_contract_bundle(&bundle).expect("validate synthetic bundle");
        validate_generic_release_preflight(&root).expect("validate synthetic preflight");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn helper_functions_cover_error_paths() {
        let empty = collect_unique_set(&["".to_string()], "field").expect_err("empty value");
        assert!(empty.contains("field contains an empty crate name"));
        let duplicate = collect_unique_set(&["a".to_string(), "a".to_string()], "field")
            .expect_err("duplicate value");
        assert!(duplicate.contains("field has duplicate crate a"));

        let values = ["b".to_string(), "a".to_string()];
        let set = collect_unique_set(&values, "field").expect("unique values");
        assert_eq!(join_set(&set), "a, b".to_string());

        assert!(package_publish_enabled(None));
        assert!(package_publish_enabled(Some(&PackagePublish::Bool(true))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Bool(false))));
        assert!(package_publish_enabled(Some(&PackagePublish::Registries(
            vec!["crates-io".to_string(),]
        ))));
        assert!(!package_publish_enabled(Some(&PackagePublish::Registries(
            Vec::new()
        ))));

        let mut package = toml::value::Table::new();
        package.insert("description".to_string(), toml::Value::Integer(42));
        assert!(!package_field_configured(&package, "description"));

        assert!(!publish_config_is_public(None));
        assert!(!publish_config_is_public(Some(&PackagePublish::Bool(true))));
        assert!(publish_config_is_public(Some(&PackagePublish::Registries(
            vec!["crates-io".to_string(),]
        ))));
        assert!(!publish_config_is_public(Some(
            &PackagePublish::Registries(vec!["crates-io".to_string(), "mirror".to_string(),])
        )));
        assert!(!publish_config_is_public(Some(
            &PackagePublish::Registries(vec!["mirror".to_string(),])
        )));

        assert!(!publish_config_is_non_public(None));
        assert!(!publish_config_is_non_public(Some(&PackagePublish::Bool(
            true
        ))));
        assert!(publish_config_is_non_public(Some(&PackagePublish::Bool(
            false
        ))));
        assert!(!publish_config_is_non_public(Some(
            &PackagePublish::Registries(vec!["crates-io".to_string(),])
        )));
    }

    #[test]
    fn release_contract_helpers_cover_classification_and_env_override_paths() {
        let release = ReleaseSection {
            version: "1.0.0".to_string(),
        };
        let empty_order = ReleaseCrateSet { crates: Vec::new() };

        let legacy = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification::default(),
            publish: Some(ReleaseCrateSet {
                crates: vec!["radroots_public".to_string()],
            }),
            internal: Some(ReleaseCrateSet {
                crates: vec!["radroots_internal".to_string()],
            }),
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(!legacy.uses_classification());
        assert_eq!(legacy.public_crates(), vec!["radroots_public".to_string()]);
        assert_eq!(
            legacy.internal_crates(),
            vec!["radroots_internal".to_string()]
        );

        let empty_legacy = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification::default(),
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(!empty_legacy.uses_classification());
        assert_eq!(empty_legacy.public_crates(), Vec::<String>::new());
        assert_eq!(empty_legacy.internal_crates(), Vec::<String>::new());

        let internal = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification {
                internal: vec!["radroots_internal_only".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(internal.uses_classification());

        let deferred = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification {
                deferred: vec!["radroots_deferred".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(deferred.uses_classification());
        assert_eq!(
            deferred.deferred_crates(),
            vec!["radroots_deferred".to_string()]
        );

        let retired = ReleaseContractFile {
            release: ReleaseSection {
                version: release.version.clone(),
            },
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification {
                retired: vec!["radroots_retired".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: ReleaseCrateSet {
                crates: empty_order.crates.clone(),
            },
        };
        assert!(retired.uses_classification());
        assert_eq!(
            retired.retired_crates(),
            vec!["radroots_retired".to_string()]
        );

        let yank_only = ReleaseContractFile {
            release,
            publication: None,
            workspace_classification: None,
            classification: ReleaseClassification {
                yank_only: vec!["radroots_yank_only".to_string()],
                ..ReleaseClassification::default()
            },
            publish: None,
            internal: None,
            publish_order: empty_order,
        };
        assert!(yank_only.uses_classification());
        assert_eq!(
            yank_only.yank_only_crates(),
            vec!["radroots_yank_only".to_string()]
        );

        let root = create_synthetic_workspace("release_contract_env_override");
        let policy_path = root_release_policy_path(&root);
        let resolved =
            resolve_release_contract_path_with_override(&root, "1.0.0", Some(policy_path.clone()))
                .expect("existing override policy should resolve");
        assert_eq!(resolved, policy_path);

        let missing_policy = root.join("missing-release-policy.toml");
        let err = resolve_release_contract_path_with_override(
            &root,
            "1.0.0",
            Some(missing_policy.clone()),
        )
        .expect_err("missing fixture policy should fail");
        assert!(err.contains("release policy override points to a missing file"));
        assert!(err.contains(&missing_policy.display().to_string()));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_manifests_reject_duplicate_package_names() {
        let root = temp_root("workspace_manifest_duplicates");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let package_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            package_manifest,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            package_manifest,
        );
        let err = workspace_package_manifests(&root)
            .expect_err("duplicate package names in manifest map");
        assert!(err.contains("duplicate workspace package name in manifest map"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_refresh_parsing_and_summary_errors_are_reported() {
        let root = temp_root("coverage_refresh_errors");
        let coverage_dir = root.join("target").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nbad-row\n",
        );
        let bad_row = load_coverage_refresh_rows(&root).expect_err("invalid coverage row");
        assert!(bad_row.contains("at least 7 columns"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\tnot-a-number\t100\t100\t100\tfile\n",
        );
        let bad_percent = load_coverage_refresh_rows(&root).expect_err("invalid coverage percent");
        assert!(bad_percent.contains("parse exec"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\t100\tnot-a-number\tfile\n",
        );
        let bad_region =
            load_coverage_refresh_rows(&root).expect_err("invalid region coverage percent");
        assert!(bad_region.contains("parse region"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\t100\t100\tfile\nradroots_a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let duplicate_row = load_coverage_refresh_rows(&root).expect_err("duplicate coverage row");
        assert!(duplicate_row.contains("duplicate coverage row"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tfail\t100\t100\t100\t100\tfile\n",
        );
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let non_pass = validate_required_coverage_summary(&root, &required, required_thresholds())
            .expect_err("non-pass status");
        assert!(non_pass.contains("non-pass status"));

        write_file(
            &coverage_dir.join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t89.9\t90\t90\t90\tfile\n",
        );
        let below_90 = validate_required_coverage_summary(&root, &required, required_thresholds())
            .expect_err("coverage below 90");
        assert!(below_90.contains("must satisfy coverage policy"));

        let missing = ["missing".to_string()].into_iter().collect::<BTreeSet<_>>();
        let missing_err =
            validate_required_coverage_summary(&root, &missing, required_thresholds())
                .expect_err("missing required row");
        assert!(missing_err.contains("missing from coverage-refresh.tsv"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn enum_extract_and_parse_error_paths_are_reported() {
        let missing =
            extract_enum_body("pub struct X;", "UnitDimension").expect_err("missing enum");
        assert!(missing.contains("missing enum"));

        let missing_brace = extract_enum_body("pub enum UnitDimension", "UnitDimension")
            .expect_err("missing opening brace");
        assert!(missing_brace.contains("missing opening brace"));

        let missing_close =
            extract_enum_body("pub enum UnitDimension { Count, Mass", "UnitDimension")
                .expect_err("missing closing brace");
        assert!(missing_close.contains("missing closing brace"));

        let variants = parse_enum_variants(
            r#"
            ,
            = 1,
            // skip
            #![cfg(test)]
            Count,
            "#,
        );
        assert_eq!(variants, vec!["Count".to_string()]);

        let nested = extract_enum_body(
            "pub enum UnitDimension { Count = { 1 }, Mass = 2 }",
            "UnitDimension",
        )
        .expect("nested braces in enum body");
        assert!(nested.contains("Count"));
    }

    #[test]
    fn coverage_policy_parity_reports_contract_errors() {
        let root = create_synthetic_workspace("coverage_policy_errors");
        let contract_root = root.join("contracts");
        let coverage_root = coverage_root(&contract_root);

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = []
"#,
        );
        let empty_required =
            validate_coverage_policy_parity(&root, &contract_root).expect_err("empty required");
        assert!(empty_required.contains("required crates list must not be empty"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 89.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let invalid_gate = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid policy thresholds");
        assert!(invalid_gate.contains("90/90/90/90"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 89.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let invalid_functions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid function threshold");
        assert!(invalid_functions.contains("90/90/90/90"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 89.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let invalid_regions = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid region threshold");
        assert!(invalid_regions.contains("90/90/90/90"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 89.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let invalid_branches = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("invalid branch threshold");
        assert!(invalid_branches.contains("90/90/90/90"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_a"]
"#,
        );
        let duplicate_required = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("duplicate required crate");
        assert!(duplicate_required.contains("duplicate crate"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = false

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let branches_optional = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("branches must be required");
        assert!(branches_optional.contains("required branches"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[overrides.radroots_a]
fail_under_exec_lines = 89.9
temporary = true
reason = "invalid override below the active development baseline"

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let below_minimum_override = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("numeric override below the active baseline");
        assert!(below_minimum_override.contains("scope radroots_a"));
        assert!(below_minimum_override.contains("at least 90/90/90/90"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_b"]
"#,
        );
        let missing_workspace = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("missing workspace crate in policy");
        assert!(missing_workspace.contains("missing workspace crates"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["unknown"]
"#,
        );
        let required_unknown = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("unknown required crate");
        assert!(required_unknown.contains("includes excluded or unknown crates"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_publish_policy_reports_contract_errors() {
        let root = create_synthetic_workspace("release_policy_errors");
        let contract_root = root.join("contracts");
        let release_policy_path = root_release_policy_path(&root);

        write_file(
            &release_policy_path,
            r#"[release]
version = ""

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let empty_version = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("empty release version");
        assert!(empty_version.contains("must not be empty"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "2.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let version_mismatch = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release version mismatch");
        assert!(version_mismatch.contains("must match contract version"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_a"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let overlap = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish/internal overlap");
        assert!(overlap.contains("overlap is not allowed"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let missing_workspace = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing workspace crate");
        assert!(missing_workspace.contains("missing workspace crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = []
"#,
        );
        let missing_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("missing publish order entries");
        assert!(missing_publish_order.contains("missing publish crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let extra_publish_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("extra publish order entries");
        assert!(extra_publish_order.contains("non-publish crates"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
authors = ["Radroots Test"]
rust-version = "1.97"
license = "MIT OR Apache-2.0"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.rs/radroots_a"
readme = "README.md"
keywords = ["radroots"]
categories = ["data-structures"]
include = ["src/**", "tests/**", "README.md", "LICENSE-APACHE", "LICENSE-MIT"]

[package.metadata.docs.rs]
features = []

[dependencies]
radroots_b = { path = "../b" }
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "1.0.0"
edition = "2024"
description = "crate b"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README"
"#,
        );
        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_b"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let dependency_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("dependency order violation");
        assert!(dependency_order.contains("must place dependency"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "1.0.0"
edition = "2024"
publish = false
"#,
        );
        validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect("internal dependency should be ignored in publish ordering");

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let publish_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish crate must be publishable");
        assert!(publish_flag.contains("must set publish = [\"crates-io\"]"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README"
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "1.0.0"
edition = "2024"
"#,
        );
        let internal_flag = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("internal crate must be non-publishable");
        assert!(internal_flag.contains("non-public crate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn publication_freeze_requires_every_workspace_package_to_be_private() {
        let root = create_synthetic_workspace("publication_freeze");
        let contract_root = root.join("contracts");
        let release_policy_path = root_release_policy_path(&root);
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
publish = false
"#,
        );
        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publication]
frozen = true
registry = "crates-io"
final_enablement_step = 305

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        validate_release_publish_policy_with_override(
            &root,
            &contract_root,
            "1.0.0",
            Some(release_policy_path.clone()),
        )
        .expect("fully private workspace should satisfy publication freeze");

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
publish = ["crates-io"]
"#,
        );
        let publishable = validate_release_publish_policy_with_override(
            &root,
            &contract_root,
            "1.0.0",
            Some(release_policy_path.clone()),
        )
        .expect_err("publication freeze must reject a publishable package");
        assert!(publishable.contains("publication freeze requires workspace crate radroots_a"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let missing_control = validate_release_publish_policy_with_override(
            &root,
            &contract_root,
            "1.0.0",
            Some(release_policy_path),
        )
        .expect_err("release policy must carry explicit publication control");
        assert!(missing_control.contains("publication control is required"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v1_release_policy_covers_approved_unapproved_unclassified_and_private_fixtures() {
        let root = create_synthetic_workspace("v1_release_policy");
        let contract_root = root.join("contracts");
        let release_policy_path = root_release_policy_path(&root);
        for member in ["a", "b"] {
            write_file(
                &root.join("crates").join(member).join("Cargo.toml"),
                &format!(
                    "[package]\nname = \"radroots_{member}\"\nversion = \"1.0.0\"\nedition = \"2024\"\npublish = false\n"
                ),
            );
        }

        let approved = (1..=19)
            .map(|index| format!("package-{index:02}"))
            .collect::<Vec<_>>();
        let approved_toml = approved
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let external_toml = approved[1..]
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut architecture = format!(
            "spec_id = \"radroots.crates.release.v1\"\npackage_count = 19\n\n[repositories.lib]\nversion = \"0.1.0-alpha\"\npackages = [\"package-01\"]\n\n[repositories.sdk]\nversion = \"0.1.0\"\npackages = [{external_toml}]\n"
        );
        for name in &approved {
            architecture.push_str(&format!("\n[[package]]\nname = \"{name}\"\n"));
        }
        write_file(
            &root.join("docs/specs/radroots_crates_release_v1.toml"),
            &architecture,
        );

        let policy = |approved_packages: &str, test_support: &str| {
            format!(
                r#"[release]
version = "1.0.0"

[publication]
frozen = true
registry = "crates-io"
final_enablement_step = 305
spec_id = "radroots.crates.release.v1"
approved_packages = [{approved_packages}]
local_packages = ["package-01"]
external_packages = [{external_toml}]

[workspace_classification]
private = ["radroots_a"]
build_codegen = []
test_support = [{test_support}]
preview = []
retired = []

[publish_order]
crates = []
"#
            )
        };

        write_file(
            &release_policy_path,
            &policy(&approved_toml, "\"radroots_b\""),
        );
        validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect("approved and exhaustively classified fixture must pass");

        let unapproved = format!("{approved_toml}, \"unapproved-public\"");
        write_file(&release_policy_path, &policy(&unapproved, "\"radroots_b\""));
        let unapproved_error = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("unapproved public package must fail");
        assert!(unapproved_error.contains("unapproved packages: unapproved-public"));

        write_file(&release_policy_path, &policy(&approved_toml, ""));
        let unclassified = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("unclassified workspace package must fail");
        assert!(unclassified.contains("workspace classification is missing packages: radroots_b"));

        write_file(
            &release_policy_path,
            &policy(&approved_toml, "\"radroots_b\""),
        );
        write_file(
            &root.join("crates/a/Cargo.toml"),
            "[package]\nname = \"radroots_a\"\nversion = \"1.0.0\"\nedition = \"2024\"\npublish = [\"crates-io\"]\n",
        );
        let private = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("private package must remain non-publishable");
        assert!(private.contains("publication freeze requires workspace crate radroots_a"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_preflight_rejects_public_dto_tooling_git_or_path_sources() {
        let root = create_synthetic_workspace("release_policy_dto_tooling_sources");

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
authors = ["Radroots Test"]
rust-version = "1.97"
license = "MIT OR Apache-2.0"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.rs/radroots_a"
readme = "README.md"
keywords = ["radroots"]
categories = ["data-structures"]
include = ["src/**", "tests/**", "README.md", "LICENSE-APACHE", "LICENSE-MIT"]

[package.metadata.docs.rs]
features = []

[dependencies]
dto_bindgen_core = { path = "../../dto_bindgen_core", version = "0.1.0", optional = true }
"#,
        );
        let path_err =
            validate_generic_release_preflight(&root).expect_err("public path DTO dependency");
        assert!(path_err.contains("radroots_a dependencies.dto_bindgen_core"));
        assert!(path_err.contains("not a path source"));

        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
resolver = "2"

[workspace.package]
version = "1.0.0"

[workspace.dependencies]
dto_bindgen = { version = "0.1.0", git = "https://example.com/dto_bindgen", rev = "abc123" }
radroots_a = { path = "crates/a", version = "=1.0.0" }
radroots_b = { path = "crates/b", version = "=1.0.0" }
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.example.com/a"
readme = "README"

[dependencies]
dto_bindgen = { workspace = true, optional = true }
"#,
        );
        let git_err = validate_generic_release_preflight(&root)
            .expect_err("public workspace git DTO dependency");
        assert!(git_err.contains("radroots_a dependencies.dto_bindgen"));
        assert!(git_err.contains("not a git source"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
authors = ["Radroots Test"]
rust-version = "1.97"
license = "MIT OR Apache-2.0"
description = "crate a"
repository = "https://example.com/a"
homepage = "https://example.com/a"
documentation = "https://docs.rs/radroots_a"
readme = "README.md"
keywords = ["radroots"]
categories = ["data-structures"]
include = ["src/**", "tests/**", "README.md", "LICENSE-APACHE", "LICENSE-MIT"]

[package.metadata.docs.rs]
features = []
"#,
        );
        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_b"
version = "1.0.0"
edition = "2024"
publish = false

[dependencies]
dto_bindgen = { workspace = true, optional = true }

[features]
default = ["std"]
std = []
legacy-ingest = ["std"]
"#,
        );
        validate_generic_release_preflight(&root)
            .expect("internal DTO tooling source does not block public publish policy");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_reports_required_field_errors() {
        let root = create_synthetic_workspace("contract_bundle_errors");

        let assert_bundle_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load bundle");
            mutator(&mut bundle);
            let err = match validate_generic_contract_bundle(&bundle) {
                Ok(()) => panic!("expected bundle validation error: {expected}"),
                Err(err) => err,
            };
            assert!(err.contains(expected), "expected `{expected}` in `{err}`");
        };

        assert_bundle_error("contract name is required", |bundle| {
            bundle.manifest.contract.name.clear();
        });
        assert_bundle_error("contract version is required", |bundle| {
            bundle.manifest.contract.version.clear();
        });
        assert_bundle_error("contract source is required", |bundle| {
            bundle.manifest.contract.source.clear();
        });
        assert_bundle_error("surface.model_crates must not be empty", |bundle| {
            bundle.manifest.surface.model_crates.clear();
        });
        assert_bundle_error("surface.algorithm_crates must not be empty", |bundle| {
            bundle.manifest.surface.algorithm_crates.clear();
        });
        assert_bundle_error(
            "surface.internal_replica_crates.storage must be a crate identifier",
            |bundle| {
                bundle.manifest.surface.internal_replica_crates = Some(InternalReplicaCrates {
                    schema: "radroots_replica_schema".to_string(),
                    storage: "crates/replica_store".to_string(),
                    sync: "radroots_replica_sync".to_string(),
                });
            },
        );
        assert_bundle_error("version.contract.version is required", |bundle| {
            bundle.version.contract.version.clear();
        });
        assert_bundle_error("version.contract.stability is required", |bundle| {
            bundle.version.contract.stability.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.major_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.minor_on.clear();
        });
        assert_bundle_error("version.semver rules must all be non-empty", |bundle| {
            bundle.version.semver.patch_on.clear();
        });
        assert_bundle_error(
            "release_integrity.requires_conformance_pass must be true",
            |bundle| {
                bundle.version.release_integrity.requires_conformance_pass = false;
            },
        );
        assert_bundle_error(
            "release_integrity.requires_contract_manifest_diff must be true",
            |bundle| {
                bundle
                    .version
                    .release_integrity
                    .requires_contract_manifest_diff = false;
            },
        );
        assert_bundle_error(
            "release_integrity.requires_release_notes must be true",
            |bundle| {
                bundle.version.release_integrity.requires_release_notes = false;
            },
        );
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.exclude_internal_workspace_crates = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_reproducible_exports = false;
        });
        assert_bundle_error("contract policy flags must all be true", |bundle| {
            bundle.manifest.policy.require_conformance_vectors = false;
        });
        assert_bundle_error("contract replica policy flags must all be true", |bundle| {
            bundle.manifest.policy.replica = Some(ReplicaPolicy {
                forbid_legacy_alias_identifiers: false,
                require_transport_agnostic_sync_contract: true,
                require_deterministic_emit_ingest: true,
            });
        });

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_contract_bundle_rejects_stale_consumer_sdk_tables() {
        let stale_manifest_root = create_synthetic_workspace("stale_manifest_consumer_sdk");
        let manifest_path = stale_manifest_root.join("contracts").join("manifest.toml");
        let mut manifest = fs::read_to_string(&manifest_path).expect("manifest");
        manifest.push_str(
            r#"
[consumer_sdk]
rust_package = "radroots_sdk"
"#,
        );
        write_file(&manifest_path, &manifest);
        let manifest_err =
            load_contract_bundle(&stale_manifest_root).expect_err("stale manifest table");
        assert!(manifest_err.contains("manifest.toml"));
        assert!(manifest_err.contains("consumer_sdk"));
        let _ = fs::remove_dir_all(stale_manifest_root);

        let stale_operations_root = create_synthetic_workspace("stale_operations_consumer_sdk");
        add_operation_contract_files(&stale_operations_root);
        let operations_path = stale_operations_root
            .join("contracts")
            .join("operations.toml");
        let mut operations = fs::read_to_string(&operations_path).expect("operations");
        operations.push_str(
            r#"
[consumer_sdk]
rust_package = "radroots_sdk"
"#,
        );
        write_file(&operations_path, &operations);
        let operations_err =
            load_contract_bundle(&stale_operations_root).expect_err("stale operations table");
        assert!(operations_err.contains("operations.toml"));
        assert!(operations_err.contains("consumer_sdk"));
        let _ = fs::remove_dir_all(stale_operations_root);
    }

    #[test]
    fn load_contract_bundle_rejects_legacy_contract_roots() {
        let stale_spec_root = create_synthetic_workspace("stale_spec_root");
        fs::create_dir_all(stale_spec_root.join("spec")).expect("create spec root");
        let spec_err = load_contract_bundle(&stale_spec_root).expect_err("stale spec root");
        assert!(spec_err.contains("legacy contract root"));
        assert!(spec_err.contains("spec"));
        let _ = fs::remove_dir_all(stale_spec_root);

        let stale_policy_root = create_synthetic_workspace("stale_policy_root");
        fs::create_dir_all(stale_policy_root.join("policy")).expect("create policy root");
        let policy_err = load_contract_bundle(&stale_policy_root).expect_err("stale policy root");
        assert!(policy_err.contains("legacy contract root"));
        assert!(policy_err.contains("policy"));
        let _ = fs::remove_dir_all(stale_policy_root);
    }

    #[test]
    fn load_contract_bundle_requires_operations_manifest() {
        let root = create_synthetic_workspace("missing_operations_manifest");
        fs::remove_file(root.join("contracts").join("operations.toml"))
            .expect("remove operations manifest");

        let error = load_contract_bundle(&root).expect_err("missing operations manifest");
        assert!(error.contains("operations.toml"), "{error}");
        assert!(error.contains("read"), "{error}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn capsule_operation_authority_cannot_be_disabled_by_domain_removal() {
        let root = workspace_root();

        let mut social_bundle = load_contract_bundle(&root).expect("load current contract");
        social_bundle
            .operations_manifest
            .public
            .domains
            .retain(|domain| domain != "social");
        social_bundle
            .operations_manifest
            .operations
            .retain(|_, operation| operation.domain != "social");
        let social_error = validate_contract_bundle(&social_bundle)
            .expect_err("removing social authority must fail");
        assert!(
            social_error.contains("comment operation authority drift"),
            "{social_error}"
        );

        let mut food_bundle = load_contract_bundle(&root).expect("load current contract");
        food_bundle
            .operations_manifest
            .public
            .domains
            .retain(|domain| domain != "food_availability");
        food_bundle
            .operations_manifest
            .operations
            .retain(|_, operation| operation.domain != "food_availability");
        let food_error = validate_contract_bundle(&food_bundle)
            .expect_err("removing FoodAvailability authority must fail");
        assert!(
            food_error.contains("food availability operation authority drift"),
            "{food_error}"
        );
    }

    #[test]
    fn validate_contract_bundle_reports_operation_contract_errors() {
        let root = create_synthetic_workspace("operation_contract_bundle_errors");
        add_operation_contract_files(&root);

        let assert_bundle_error = |expected: &str, mutator: fn(&mut ContractBundle)| {
            let mut bundle = load_contract_bundle(&root).expect("load bundle");
            mutator(&mut bundle);
            let err =
                validate_generic_contract_bundle(&bundle).expect_err("bundle validation error");
            assert!(err.contains(expected), "expected `{expected}` in `{err}`");
        };

        assert_bundle_error("public.domains must not be empty", |bundle| {
            bundle.operations_manifest.public.domains.clear();
        });
        assert_bundle_error(
            "shared_types.public uses retired event type RadrootsNostrEvent",
            |bundle| {
                bundle
                    .operations_manifest
                    .shared_types
                    .public
                    .push("RadrootsNostrEvent".to_string());
            },
        );
        assert_bundle_error(
            "shared_types.public uses retired event type RadrootsInboundCalendarDateEvent",
            |bundle| {
                bundle
                    .operations_manifest
                    .shared_types
                    .public
                    .push("RadrootsInboundCalendarDateEvent".to_string());
            },
        );
        assert_bundle_error(
            "shared_types.public uses retired event type RadrootsCalendar",
            |bundle| {
                bundle
                    .operations_manifest
                    .shared_types
                    .public
                    .push("RadrootsCalendar".to_string());
            },
        );
        assert_bundle_error(
            "shared_types.public uses retired event type RadrootsCalendarEventRsvp",
            |bundle| {
                bundle
                    .operations_manifest
                    .shared_types
                    .public
                    .push("RadrootsCalendarEventRsvp".to_string());
            },
        );
        assert_bundle_error(
            "shared_types.public uses retired event type RadrootsCalendarRsvp",
            |bundle| {
                bundle
                    .operations_manifest
                    .shared_types
                    .public
                    .push("RadrootsCalendarRsvp".to_string());
            },
        );
        assert_bundle_error(
            "operation profile.build_authored_draft inputs uses retired event type RadrootsNostrEvent",
            |bundle| {
                bundle
                    .operations_manifest
                    .operations
                    .get_mut("profile_build_authored_draft")
                    .expect("profile operation")
                    .inputs
                    .push("RadrootsNostrEvent".to_string());
            },
        );
        assert_bundle_error(
            "operation profile.build_authored_draft outputs uses retired event type WireEventParts",
            |bundle| {
                bundle
                    .operations_manifest
                    .operations
                    .get_mut("profile_build_authored_draft")
                    .expect("profile operation")
                    .outputs
                    .push("WireEventParts".to_string());
            },
        );
        assert_bundle_error(
            "operation profile.build_authored_draft implementation.rust_types uses retired event type RadrootsNostrEventPtr",
            |bundle| {
                bundle
                    .operations_manifest
                    .operations
                    .get_mut("profile_build_authored_draft")
                    .expect("profile operation")
                    .implementation
                    .rust_types
                    .push("radroots_event::RadrootsNostrEventPtr".to_string());
            },
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_contract_bundle_requires_real_conformance_assets() {
        let missing_schema_root = create_synthetic_workspace("operation_contract_missing_schema");
        add_operation_contract_files(&missing_schema_root);
        let _ = fs::remove_file(conformance_schema_path(&missing_schema_root));
        let bundle = load_contract_bundle(&missing_schema_root).expect("load bundle");
        let err =
            validate_generic_contract_bundle(&bundle).expect_err("missing schema should fail");
        assert!(err.contains("vector.schema.json"));
        let _ = fs::remove_dir_all(&missing_schema_root);

        let invalid_vector_root = create_synthetic_workspace("operation_contract_invalid_vector");
        add_operation_contract_files(&invalid_vector_root);
        let invalid_vector_path = invalid_vector_root
            .join("contracts")
            .join("conformance")
            .join("vectors")
            .join("profile")
            .join("metadata.v1.json");
        write_file(
            &invalid_vector_path,
            r#"{
  "suite": "profile",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "profile_build_authored_draft_minimal_001",
      "kind": "profile.build_authored_draft",
      "input": {}
    }
  ]
}
"#,
        );
        let bundle = load_contract_bundle(&invalid_vector_root).expect("load bundle");
        let err =
            validate_generic_contract_bundle(&bundle).expect_err("invalid vector should fail");
        assert!(err.contains("metadata.v1.json"));
        assert!(err.contains("exactly one of expected or expected_error_contains"));

        write_file(
            &invalid_vector_path,
            r#"{
  "suite": "profile",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "profile_build_authored_draft_minimal_001",
      "kind": "profile.build_authored_draft",
      "input": {},
      "expected": {},
      "expected_error_contains": "invalid"
    }
  ]
}
"#,
        );
        let err = validate_generic_contract_bundle(&bundle)
            .expect_err("vector with two result authorities should fail");
        assert!(err.contains("exactly one of expected or expected_error_contains"));

        write_file(
            &invalid_vector_path,
            r#"{
  "suite": "profile",
  "contract_version": "1.0.0",
  "vectors": [
    {
      "id": "profile_build_authored_draft_minimal_001",
      "kind": "profile.build_authored_draft",
      "input": {},
      "expected_error_contains": "   "
    }
  ]
}
"#,
        );
        let err = validate_generic_contract_bundle(&bundle)
            .expect_err("blank expected error fragment should fail");
        assert!(err.contains("expected_error_contains must not be blank"));
        let _ = fs::remove_dir_all(&invalid_vector_root);

        let root = create_synthetic_workspace("operation_contract_vector_path");
        add_operation_contract_files(&root);
        let mut bundle = load_contract_bundle(&root).expect("load bundle");
        bundle
            .operations_manifest
            .operations
            .get_mut("profile_build_authored_draft")
            .expect("profile operation")
            .conformance
            .vector = "conformance/vectors/profile/metadata.v1.json".to_string();
        let err = validate_generic_contract_bundle(&bundle).expect_err("legacy path should fail");
        assert!(err.contains("must live under contracts/conformance/"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_toml_and_publish_flags_report_failures() {
        let missing = temp_root("parse_toml_missing");
        let read_err =
            parse_toml::<WorkspaceCargoManifest>(&missing.join("Cargo.toml")).expect_err("missing");
        assert!(read_err.contains("read"));
        let _ = fs::remove_dir_all(&missing);

        let invalid = temp_root("parse_toml_invalid");
        write_file(&invalid.join("Cargo.toml"), "[workspace]\nmembers = [");
        let parse_err = parse_toml::<WorkspaceCargoManifest>(&invalid.join("Cargo.toml"))
            .expect_err("invalid manifest");
        assert!(parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&invalid);

        let contract_manifest_missing = temp_root("parse_contract_manifest_missing");
        let contract_manifest_read_err =
            parse_toml::<ContractManifest>(&contract_manifest_missing.join("manifest.toml"))
                .expect_err("missing contract manifest");
        assert!(contract_manifest_read_err.contains("read"));
        let _ = fs::remove_dir_all(&contract_manifest_missing);

        let contract_manifest_invalid = temp_root("parse_contract_manifest_invalid");
        write_file(
            &contract_manifest_invalid.join("manifest.toml"),
            "[contract",
        );
        let contract_manifest_parse_err =
            parse_toml::<ContractManifest>(&contract_manifest_invalid.join("manifest.toml"))
                .expect_err("invalid contract manifest");
        assert!(contract_manifest_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&contract_manifest_invalid);

        let version_missing = temp_root("parse_version_policy_missing");
        let version_read_err = parse_toml::<VersionPolicy>(&version_missing.join("version.toml"))
            .expect_err("missing version policy");
        assert!(version_read_err.contains("read"));
        let _ = fs::remove_dir_all(&version_missing);

        let version_invalid = temp_root("parse_version_policy_invalid");
        write_file(&version_invalid.join("version.toml"), "[version");
        let version_parse_err = parse_toml::<VersionPolicy>(&version_invalid.join("version.toml"))
            .expect_err("invalid version policy");
        assert!(version_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&version_invalid);

        let release_missing = temp_root("parse_release_contract_missing");
        let release_read_err =
            parse_toml::<ReleaseContractFile>(&release_missing.join("publish-set.toml"))
                .expect_err("missing release contract");
        assert!(release_read_err.contains("read"));
        let _ = fs::remove_dir_all(&release_missing);

        let release_invalid = temp_root("parse_release_contract_invalid");
        write_file(&release_invalid.join("publish-set.toml"), "[release");
        let release_parse_err =
            parse_toml::<ReleaseContractFile>(&release_invalid.join("publish-set.toml"))
                .expect_err("invalid release contract");
        assert!(release_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&release_invalid);

        let operations_missing = temp_root("parse_operations_manifest_missing");
        let operations_read_err =
            parse_toml::<OperationsContractManifest>(&operations_missing.join("operations.toml"))
                .expect_err("missing operations manifest");
        assert!(operations_read_err.contains("read"));
        let _ = fs::remove_dir_all(&operations_missing);

        let operations_invalid = temp_root("parse_operations_manifest_invalid");
        write_file(&operations_invalid.join("operations.toml"), "[operations");
        let operations_parse_err =
            parse_toml::<OperationsContractManifest>(&operations_invalid.join("operations.toml"))
                .expect_err("invalid operations manifest");
        assert!(operations_parse_err.contains("parse"));
        let _ = fs::remove_dir_all(&operations_invalid);

        let dup = temp_root("publish_flags_duplicate");
        write_file(
            &dup.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a", "crates/b"]
"#,
        );
        let member_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &dup.join("crates").join("a").join("Cargo.toml"),
            member_manifest,
        );
        write_file(
            &dup.join("crates").join("b").join("Cargo.toml"),
            member_manifest,
        );
        let dup_err = workspace_package_publish_flags(&dup).expect_err("duplicate publish flags");
        assert!(dup_err.contains("duplicate workspace package name"));
        let _ = fs::remove_dir_all(&dup);
    }

    #[test]
    fn workspace_package_records_and_callers_report_member_manifest_errors() {
        let root = temp_root("workspace_package_record_errors");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );

        let read_err =
            workspace_package_records(&root).expect_err("missing member manifest should fail");
        assert!(read_err.contains("read"));

        let names_err = workspace_package_names(&root).expect_err("names should fail");
        assert!(names_err.contains("read"));
        let manifests_err = workspace_package_manifests(&root).expect_err("manifests should fail");
        assert!(manifests_err.contains("read"));
        let flags_err = workspace_package_publish_flags(&root).expect_err("flags should fail");
        assert!(flags_err.contains("read"));
        let deps_err = read_workspace_package_dependencies(&root).expect_err("deps should fail");
        assert!(deps_err.contains("read"));

        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let publish_err =
            validate_publish_package_metadata(&root, &publish).expect_err("publish metadata");
        assert!(publish_err.contains("read"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let parse_value_err =
            workspace_package_records(&root).expect_err("invalid toml should fail");
        assert!(parse_value_err.contains("parse"));

        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[workspace]
resolver = "2"
"#,
        );
        let parse_package_err =
            workspace_package_records(&root).expect_err("missing package table should fail");
        assert!(parse_package_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_manifests_success_and_publish_metadata_duplicate_names() {
        let root = create_synthetic_workspace("workspace_manifest_success");
        let manifests = workspace_package_manifests(&root).expect("workspace manifests");
        assert_eq!(manifests.len(), 2);
        assert!(manifests.contains_key("radroots_a"));
        assert!(manifests.contains_key("radroots_b"));

        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
description = "crate b duplicate name"
repository = "https://example.com/b"
homepage = "https://example.com/b"
documentation = "https://docs.example.com/b"
readme = "README"
publish = false
"#,
        );
        let publish = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let duplicate_err =
            validate_publish_package_metadata(&root, &publish).expect_err("duplicate package map");
        assert!(duplicate_err.contains("duplicate workspace package name"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_publish_configs_cover_success_and_duplicate_names() {
        let root = create_synthetic_workspace("workspace_publish_configs");
        let flags = workspace_package_publish_flags(&root).expect("publish flags");
        assert!(flags["radroots_a"]);
        assert!(!flags["radroots_b"]);

        let configs = workspace_package_publish_configs(&root).expect("publish configs");
        assert_eq!(
            configs["radroots_a"],
            Some(PackagePublish::Registries(vec!["crates-io".to_string()]))
        );
        assert_eq!(configs["radroots_b"], Some(PackagePublish::Bool(false)));

        write_file(
            &root.join("crates").join("b").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
publish = false
"#,
        );
        let duplicate_err = workspace_package_publish_configs(&root)
            .expect_err("duplicate package name in publish configs");
        assert!(duplicate_err.contains("duplicate workspace package name"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_package_publish_configs_report_workspace_record_errors() {
        let root = temp_root("workspace_publish_configs_errors");
        let err = workspace_package_publish_configs(&root)
            .expect_err("missing workspace manifest should fail");
        assert!(err.contains("Cargo.toml"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_release_and_bundle_loaders_report_parse_and_read_errors() {
        let root = create_synthetic_workspace("coverage_release_loader_errors");
        let contract_root = root.join("contracts");
        let coverage_root = coverage_root(&contract_root);
        let release_policy_path = root_release_policy_path(&root);

        let missing_workspace = temp_root("coverage_missing_workspace_manifest");
        let policy_workspace_err =
            validate_coverage_policy_parity(&missing_workspace, &contract_root)
                .expect_err("coverage workspace lookup error");
        assert!(policy_workspace_err.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(&missing_workspace);

        let _ = fs::remove_file(coverage_root.join("coverage.toml"));
        let policy_load_err = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage policy read error");
        assert!(policy_load_err.contains("coverage.toml"));
        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 100.0
fail_under_functions = 100.0
fail_under_regions = 100.0
fail_under_branches = 100.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );

        let missing_release = temp_root("release_missing_workspace_manifest");
        write_root_release_policy(
            &missing_release,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let release_workspace_err =
            validate_release_publish_policy(&missing_release, &contract_root, "1.0.0")
                .expect_err("release workspace read error");
        assert!(release_workspace_err.contains("Cargo.toml"));
        let _ = fs::remove_dir_all(&missing_release);

        let _ = fs::remove_file(&release_policy_path);
        let release_load_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release contract read error");
        assert!(release_load_err.contains(RELEASE_POLICY_RELATIVE));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_publish = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish crates");
        assert!(duplicate_publish.contains("publish.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b", "radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_internal = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate internal crates");
        assert!(duplicate_internal.contains("internal.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_a"]
"#,
        );
        let duplicate_order = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("duplicate publish order");
        assert!(duplicate_order.contains("publish_order.crates has duplicate crate"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let dependency_err = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("workspace dependency parse error");
        assert!(dependency_err.contains("parse"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_release_contract_with_override_reports_override_and_missing_policy_errors() {
        let root = create_synthetic_workspace("release_contract_loader_errors");

        let missing_override = root.join("missing-release-policy.toml");
        let override_err =
            load_release_contract_with_override(&root, "1.0.0", Some(missing_override.clone()))
                .expect_err("missing override should fail");
        assert!(override_err.contains("release policy override points to a missing file"));

        let _ = fs::remove_file(root_release_policy_path(&root));
        let missing_policy_err = load_release_contract_with_override(&root, "1.0.0", None)
            .expect_err("missing release policy should fail");
        assert!(missing_policy_err.contains("release publish policy not found"));
        assert!(missing_policy_err.contains(RELEASE_POLICY_RELATIVE));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn release_contract_discovery_does_not_search_parent_directories() {
        let parent = temp_root("release_contract_parent_isolation");
        write_file(
            &release_contract_path(&parent, "1.0.0"),
            "[release]\nversion = \"1.0.0\"\n\n[publish_order]\ncrates = []\n",
        );
        let capsule = parent.join("capsule");
        fs::create_dir_all(&capsule).expect("create isolated capsule root");

        let err = resolve_release_contract_path_with_override(&capsule, "1.0.0", None)
            .expect_err("parent release contract must be ignored");
        assert!(err.contains(&capsule.display().to_string()));
        assert!(
            !err.contains(
                &release_contract_path(&parent, "1.0.0")
                    .display()
                    .to_string()
            )
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn root_release_policy_preflight_covers_classification_variants() {
        let root = create_synthetic_workspace("root_release_policy_classifications");
        configure_root_release_policy_workspace(&root);
        write_root_release_policy(
            &root,
            r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
        );

        let bundle = load_contract_bundle(&root).expect("load root release policy bundle");
        validate_generic_contract_bundle(&bundle).expect("validate root release policy bundle");
        validate_generic_release_preflight(&root).expect("validate root release policy preflight");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn root_release_policy_reports_deferred_retired_and_yank_only_errors() {
        for (label, policy_body, expected) in [
            (
                "deferred",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c", "radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.deferred has duplicate crate radroots_c",
            ),
            (
                "retired",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = [""]
yank_only = ["radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.retired contains an empty crate name",
            ),
            (
                "yank_only",
                r#"[release]
version = "1.0.0"

[classification]
public = ["radroots_a"]
internal = ["radroots_b"]
deferred = ["radroots_c"]
retired = ["radroots_d"]
yank_only = ["radroots_e", "radroots_e"]

[publish_order]
crates = ["radroots_a"]
"#,
                "classification.yank_only has duplicate crate radroots_e",
            ),
        ] {
            let root = create_synthetic_workspace(&format!("root_release_policy_{label}_error"));
            configure_root_release_policy_workspace(&root);
            write_root_release_policy(&root, policy_body);

            let err = validate_release_publish_policy(&root, &root.join("contracts"), "1.0.0")
                .expect_err("invalid non-public classification should fail");
            assert!(err.contains(expected), "{label} err: {err}");

            let _ = fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn validate_release_preflight_reports_each_stage_error() {
        let missing_contract_root = temp_root("preflight_missing_contract");
        let missing_contract_err = validate_generic_release_preflight(&missing_contract_root)
            .expect_err("missing contract");
        assert!(missing_contract_err.contains("manifest.toml"));
        let _ = fs::remove_dir_all(&missing_contract_root);

        let invalid_bundle = create_synthetic_workspace("preflight_invalid_bundle");
        write_file(
            &invalid_bundle.join("contracts").join("manifest.toml"),
            r#"[contract]
name = "radroots_contract"
version = "1.0.0"
source = "synthetic"

[surface]
model_crates = ["radroots_a"]
algorithm_crates = ["radroots_b"]

[policy]
exclude_internal_workspace_crates = false
require_reproducible_exports = true
require_conformance_vectors = true
"#,
        );
        let invalid_bundle_err =
            validate_generic_release_preflight(&invalid_bundle).expect_err("bundle validation");
        assert!(invalid_bundle_err.contains("contract policy flags must all be true"));
        let _ = fs::remove_dir_all(&invalid_bundle);

        let missing_release = create_synthetic_workspace("preflight_missing_release");
        let _ = fs::remove_file(root_release_policy_path(&missing_release));
        let missing_release_err =
            validate_generic_release_preflight(&missing_release).expect_err("missing release");
        assert!(missing_release_err.contains(RELEASE_POLICY_RELATIVE));
        let _ = fs::remove_dir_all(&missing_release);

        let missing_required = create_synthetic_workspace("preflight_missing_required");
        let _ = fs::remove_file(missing_required.join("contracts").join("coverage.toml"));
        let missing_required_err = validate_generic_release_preflight(&missing_required)
            .expect_err("missing required list");
        assert!(missing_required_err.contains("coverage.toml"));
        let _ = fs::remove_dir_all(&missing_required);

        let duplicate_publish = create_synthetic_workspace("preflight_duplicate_publish");
        write_file(
            &root_release_policy_path(&duplicate_publish),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a"]
"#,
        );
        let duplicate_publish_err = validate_generic_release_preflight(&duplicate_publish)
            .expect_err("duplicate publish crates");
        assert!(duplicate_publish_err.contains("publish.crates has duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_publish);

        let duplicate_required = create_synthetic_workspace("preflight_duplicate_required");
        write_file(
            &duplicate_required.join("contracts").join("coverage.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\", \"radroots_a\"]\n",
        );
        let duplicate_required_err = validate_generic_release_preflight(&duplicate_required)
            .expect_err("duplicate required crates");
        assert!(duplicate_required_err.contains("duplicate crate"));
        let _ = fs::remove_dir_all(&duplicate_required);

        let publish_metadata = create_synthetic_workspace("preflight_publish_metadata");
        write_file(
            &publish_metadata.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
publish = ["crates-io"]
version = "1.0.0"
edition = "2024"
"#,
        );
        let publish_metadata_err = validate_generic_release_preflight(&publish_metadata)
            .expect_err("publish metadata validation");
        assert!(publish_metadata_err.contains("must define a non-empty package.description"));
        let _ = fs::remove_dir_all(&publish_metadata);

        let missing_coverage_row = create_synthetic_workspace("preflight_missing_coverage_row");
        write_file(
            &missing_coverage_row
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n",
        );
        let missing_coverage_row_err = validate_generic_release_preflight(&missing_coverage_row)
            .expect_err("required coverage refresh row missing");
        assert!(missing_coverage_row_err.contains("missing from coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_coverage_row);
    }

    #[test]
    fn load_contract_bundle_and_validation_report_version_core_and_coverage_errors() {
        let root = create_synthetic_workspace("bundle_version_core_and_coverage_errors");
        write_file(&root.join("contracts").join("version.toml"), "[contract");
        let version_parse_err = load_contract_bundle(&root).expect_err("invalid version file");
        assert!(version_parse_err.contains("version.toml"));

        write_file(
            &root.join("contracts").join("version.toml"),
            r#"[contract]
version = "1.0.0"
stability = "alpha"

[semver]
major_on = ["breaking"]
minor_on = ["feature"]
patch_on = ["fix"]

[release_integrity]
requires_conformance_pass = true
requires_contract_manifest_diff = true
requires_release_notes = true
"#,
        );
        let bundle = load_contract_bundle(&root).expect("load bundle");
        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum UnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let core_err = validate_generic_contract_bundle(&bundle).expect_err("core unit mismatch");
        assert!(core_err.contains("variant order must be"));

        write_file(
            &root.join("crates").join("core").join("src").join("unit.rs"),
            r#"pub enum UnitDimension {
Count,
Mass,
Volume,
}
"#,
        );
        write_file(
            &root.join("contracts").join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = false

[required]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let policy_err =
            validate_generic_contract_bundle(&bundle).expect_err("coverage policy validation");
        assert!(policy_err.contains("90/90/90/90"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_summary_and_core_enum_additional_error_paths() {
        let coverage_root = temp_root("coverage_summary_additional_errors");
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\tbad\t100\t100\tfile\n",
        );
        let func_err = load_coverage_refresh_rows(&coverage_root).expect_err("func parse error");
        assert!(func_err.contains("parse func"));
        write_file(
            &coverage_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\nradroots_a\tpass\t100\t100\tbad\t100\tfile\n",
        );
        let branch_err =
            load_coverage_refresh_rows(&coverage_root).expect_err("branch parse error");
        assert!(branch_err.contains("parse branch"));
        let _ = fs::remove_dir_all(&coverage_root);

        let missing_refresh_root = temp_root("coverage_summary_missing_refresh");
        let required = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_refresh_err = validate_required_coverage_summary(
            &missing_refresh_root,
            &required,
            required_thresholds(),
        )
        .expect_err("missing refresh should fail");
        assert!(missing_refresh_err.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&missing_refresh_root);

        let enum_root = temp_root("core_unit_missing_enum");
        write_file(
            &enum_root
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            "pub struct NotTheEnum;",
        );
        let enum_err =
            validate_core_unit_dimension_variant_order(&enum_root).expect_err("missing enum");
        assert!(enum_err.contains("missing enum"));
        let _ = fs::remove_dir_all(&enum_root);
    }

    #[test]
    fn publish_metadata_and_coverage_refresh_report_missing_paths() {
        let root = temp_root("publish_missing_manifest");
        write_file(
            &root.join("Cargo.toml"),
            r#"[workspace]
members = ["crates/a"]
"#,
        );
        write_file(
            &root.join("crates").join("a").join("Cargo.toml"),
            r#"[package]
name = "radroots_a"
version = "1.0.0"
edition = "2024"
authors = ["Radroots Test"]
rust-version = "1.97"
license = "MIT OR Apache-2.0"
description = "crate a"
repository = { workspace = true }
homepage = { workspace = true }
readme = { workspace = true }
"#,
        );
        let missing_manifest = ["radroots_b".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let missing_err = validate_publish_package_metadata(&root, &missing_manifest)
            .expect_err("missing workspace manifest");
        assert!(missing_err.contains("has no workspace manifest"));

        let missing_field = ["radroots_a".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let field_err = validate_publish_package_metadata(&root, &missing_field)
            .expect_err("missing configured field");
        assert!(field_err.contains("must configure package.documentation"));

        let refresh_missing =
            load_coverage_refresh_rows(&root).expect_err("missing coverage-refresh.tsv");
        assert!(refresh_missing.contains("coverage-refresh.tsv"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn coverage_refresh_parser_skips_blank_lines() {
        let root = temp_root("coverage_refresh_blank_lines");
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
            "crate\tstatus\texec\tfunc\tbranch\tregion\treport\n\nradroots_a\tpass\t100\t100\t100\t100\tfile\n",
        );
        let rows = load_coverage_refresh_rows(&root).expect("rows");
        assert_eq!(rows.len(), 1);
        assert!(rows.contains_key("radroots_a"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn core_unit_dimension_validation_reports_missing_and_mismatch() {
        let missing = temp_root("core_unit_missing");
        let missing_err = validate_core_unit_dimension_variant_order(&missing)
            .expect_err("missing unit file should fail");
        assert!(missing_err.contains("unit.rs"));
        let _ = fs::remove_dir_all(&missing);

        let mismatch = temp_root("core_unit_mismatch");
        write_file(
            &mismatch
                .join("crates")
                .join("core")
                .join("src")
                .join("unit.rs"),
            r#"pub enum UnitDimension {
Mass,
Count,
Volume,
}
"#,
        );
        let mismatch_err = validate_core_unit_dimension_variant_order(&mismatch)
            .expect_err("mismatched enum order should fail");
        assert!(mismatch_err.contains("variant order must be"));
        let _ = fs::remove_dir_all(&mismatch);
    }

    #[test]
    fn coverage_and_release_additional_error_branches_are_reported() {
        let root = create_synthetic_workspace("coverage_release_extra_errors");
        let contract_root = root.join("contracts");
        let coverage_root = coverage_root(&contract_root);
        let release_policy_path = root_release_policy_path(&root);

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_a", "radroots_b", "radroots_extra"]
"#,
        );
        let coverage_extra = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("coverage unknown crate");
        assert!(coverage_extra.contains("includes excluded or unknown crates"));

        write_file(
            &coverage_root.join("coverage.toml"),
            r#"[gate]
fail_under_exec_lines = 90.0
fail_under_functions = 90.0
fail_under_regions = 90.0
fail_under_branches = 90.0
require_branches = true

[required]
crates = ["radroots_b"]
"#,
        );
        let required_list_mismatch = validate_coverage_policy_parity(&root, &contract_root)
            .expect_err("required list must match workspace crates");
        assert!(required_list_mismatch.contains("missing workspace crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a", "radroots_b", "radroots_extra"]

[internal]
crates = []

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let release_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("release extra crate");
        assert!(release_extra.contains("include unknown crates"));

        write_file(
            &release_policy_path,
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = ["radroots_a", "radroots_b"]
"#,
        );
        let publish_order_extra = validate_release_publish_policy(&root, &contract_root, "1.0.0")
            .expect_err("publish order non-publish crate");
        assert!(publish_order_extra.contains("non-publish crates"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_contract_bundle_reports_release_policy_errors() {
        let release_error_root = create_synthetic_workspace("bundle_release_policy_error");
        write_file(
            &root_release_policy_path(&release_error_root),
            r#"[release]
version = "1.0.0"

[publish]
crates = ["radroots_a"]

[internal]
crates = ["radroots_b"]

[publish_order]
crates = []
"#,
        );
        let bundle = load_contract_bundle(&release_error_root).expect("load release error bundle");
        let release_err =
            validate_generic_contract_bundle(&bundle).expect_err("release policy failure");
        assert!(release_err.contains("publish_order.crates is missing publish crates"));
        let _ = fs::remove_dir_all(&release_error_root);
    }
}
