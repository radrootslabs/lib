use super::DeletionOperationExpectation;

pub(super) const DELETION_CONFORMANCE_VECTOR_RELATIVE: &str =
    "contracts/conformance/vectors/deletion/verified_profile.v1.json";

pub(super) const REQUIRED_DELETION_PUBLIC_TYPES: [&str; 18] = [
    "RadrootsNip01EventWireParts",
    "RadrootsEventEnvelope",
    "RadrootsSignatureVerifiedEvent",
    "RadrootsNip01Coordinate",
    "RadrootsNip01CoordinateParts",
    "RadrootsNip01CoordinateParseError",
    "RadrootsNip09DeletionError",
    "RadrootsNip09DeletionEventTarget",
    "RadrootsNip09DeletionAddressTarget",
    "RadrootsAuthoredNip09DeletionRequest",
    "RadrootsNip09DeletionDiagnostic",
    "RadrootsInboundNip09DeletionEventTarget",
    "RadrootsInboundNip09DeletionAddressTarget",
    "RadrootsInboundNip09DeletionKindAdvisory",
    "RadrootsInboundNip09DeletionProjection",
    "RadrootsNip09DeletionProjectionError",
    "RadrootsAdmittedNip09DeletionRequestEvent",
    "RadrootsNip09DeletionAdmissionError",
];

pub(super) const DELETION_OPERATION_EXPECTATIONS: [DeletionOperationExpectation; 3] = [
    DeletionOperationExpectation {
        key: "social_deletion_request_build_authored_draft",
        id: "social.deletion_request.build_authored_draft",
        inputs: &["RadrootsAuthoredNip09DeletionRequest"],
        outputs: &["RadrootsNip01EventWireParts"],
        error_class: "encode_error",
        signing: "none",
        rust_modules: &[
            "crates/event/src/deletion.rs",
            "crates/event_codec/src/deletion/authored.rs",
        ],
        rust_types: &[
            "radroots_event::deletion::RadrootsAuthoredNip09DeletionRequest",
            "radroots_event::deletion::RadrootsNip09DeletionAddressTarget",
            "radroots_event::deletion::RadrootsNip09DeletionError",
            "radroots_event::deletion::RadrootsNip09DeletionEventTarget",
            "radroots_event::ids::RadrootsNip01Coordinate",
            "radroots_event::wire::RadrootsNip01EventWireParts",
        ],
        case_kinds: &[
            "social.deletion_request.build_authored_draft.valid",
            "social.deletion_request.build_authored_draft.invalid",
        ],
    },
    DeletionOperationExpectation {
        key: "social_deletion_request_project_verified_event",
        id: "social.deletion_request.project_verified_event",
        inputs: &["RadrootsSignatureVerifiedEvent"],
        outputs: &["RadrootsInboundNip09DeletionProjection"],
        error_class: "parse_error",
        signing: "none",
        rust_modules: &[
            "crates/event_codec/src/deletion/inbound.rs",
            "crates/event_codec/src/verification.rs",
        ],
        rust_types: &[
            "radroots_event_codec::deletion::inbound::RadrootsInboundNip09DeletionAddressTarget",
            "radroots_event_codec::deletion::inbound::RadrootsInboundNip09DeletionEventTarget",
            "radroots_event_codec::deletion::inbound::RadrootsInboundNip09DeletionKindAdvisory",
            "radroots_event_codec::deletion::inbound::RadrootsInboundNip09DeletionProjection",
            "radroots_event_codec::deletion::inbound::RadrootsNip09DeletionDiagnostic",
            "radroots_event_codec::deletion::inbound::RadrootsNip09DeletionProjectionError",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.deletion_request.project_verified_event.valid",
            "social.deletion_request.project_verified_event.invalid",
        ],
    },
    DeletionOperationExpectation {
        key: "social_deletion_request_verify_and_admit_event",
        id: "social.deletion_request.verify_and_admit_event",
        inputs: &["RadrootsEventEnvelope"],
        outputs: &["RadrootsAdmittedNip09DeletionRequestEvent"],
        error_class: "admission_error",
        signing: "nip01",
        rust_modules: &[
            "crates/event_codec/src/deletion/admission.rs",
            "crates/event_codec/src/deletion/inbound.rs",
            "crates/event_codec/src/verification.rs",
        ],
        rust_types: &[
            "radroots_event::RadrootsEventEnvelope",
            "radroots_event_codec::deletion::admission::RadrootsAdmittedNip09DeletionRequestEvent",
            "radroots_event_codec::deletion::admission::RadrootsNip09DeletionAdmissionError",
            "radroots_event_codec::deletion::inbound::RadrootsInboundNip09DeletionProjection",
            "radroots_event_codec::deletion::inbound::RadrootsNip09DeletionProjectionError",
            "radroots_event_codec::verification::RadrootsSignatureVerifiedEvent",
        ],
        case_kinds: &[
            "social.deletion_request.verify_and_admit_event.valid",
            "social.deletion_request.verify_and_admit_event.invalid",
        ],
    },
];

pub(super) const DELETION_CASE_KINDS: [&str; 6] = [
    "social.deletion_request.build_authored_draft.valid",
    "social.deletion_request.build_authored_draft.invalid",
    "social.deletion_request.project_verified_event.valid",
    "social.deletion_request.project_verified_event.invalid",
    "social.deletion_request.verify_and_admit_event.valid",
    "social.deletion_request.verify_and_admit_event.invalid",
];

pub(super) const DELETION_AUTHORED_VALID_IDS: [&str; 14] = [
    "nip09_authored_event_target_min_kind_empty_content",
    "nip09_authored_event_target_max_kind_unicode_content",
    "nip09_authored_coordinate_kind_0_empty_identifier",
    "nip09_authored_coordinate_kind_3_empty_identifier",
    "nip09_authored_coordinate_kind_10000_empty_identifier",
    "nip09_authored_coordinate_kind_19999_empty_identifier",
    "nip09_authored_coordinate_kind_30000_empty_identifier",
    "nip09_authored_coordinate_kind_39999_opaque_identifier",
    "nip09_authored_mixed_targets_canonical_order",
    "nip09_authored_content_bytes_exact",
    "nip09_authored_tag_count_exact",
    "nip09_authored_tag_element_bytes_exact",
    "nip09_authored_tag_bytes_exact",
    "nip09_authored_event_wire_bytes_exact",
];

pub(super) const DELETION_AUTHORED_INVALID_IDS: [&str; 14] = [
    "nip09_authored_event_target_invalid",
    "nip09_authored_event_target_kind_out_of_range",
    "nip09_authored_address_target_invalid_format",
    "nip09_authored_address_target_invalid_pubkey",
    "nip09_authored_address_target_unsupported_kind",
    "nip09_authored_replaceable_identifier_nonempty",
    "nip09_authored_event_target_duplicate_normalized",
    "nip09_authored_address_target_duplicate_normalized",
    "nip09_authored_target_missing",
    "nip09_authored_content_bytes_overflow",
    "nip09_authored_tag_count_overflow_precedes_duplicate",
    "nip09_authored_tag_element_bytes_overflow",
    "nip09_authored_tag_bytes_overflow_precedes_duplicate",
    "nip09_authored_event_wire_bytes_overflow_precedes_duplicate",
];

pub(super) const DELETION_PROJECT_VALID_IDS: [&str; 18] = [
    "nip09_project_signed_event_target_without_k",
    "nip09_project_signed_address_replaceable_boundaries",
    "nip09_project_signed_addressable_boundaries",
    "nip09_project_signed_mixed_raw_retention",
    "nip09_project_signed_duplicate_targets_first_provenance",
    "nip09_project_signed_canonical_effect_sorting",
    "nip09_project_signed_kind_advisory_diagnostics",
    "nip09_project_signed_event_target_conflict_unprovable",
    "nip09_project_signed_trailing_kind_and_unknown_tags",
    "nip09_project_signed_unicode_whitespace_control_content",
    "nip09_project_signed_content_bytes_exact",
    "nip09_project_signed_tag_count_exact",
    "nip09_project_signed_tag_element_count_exact",
    "nip09_project_signed_tag_element_bytes_exact_multibyte",
    "nip09_project_signed_tag_bytes_exact",
    "nip09_project_signed_event_wire_bytes_exact_max_created_at",
    "nip09_project_signed_event_wire_short_created_at_width",
    "nip09_project_signed_kind_advisory_min_max",
];

pub(super) const DELETION_PROJECT_INVALID_IDS: [&str; 26] = [
    "nip09_project_signed_wrong_kind",
    "nip09_project_signed_content_bytes_overflow",
    "nip09_project_signed_tag_count_overflow",
    "nip09_project_signed_tag_element_count_overflow",
    "nip09_project_signed_tag_element_bytes_overflow",
    "nip09_project_signed_tag_bytes_overflow",
    "nip09_project_signed_event_wire_bytes_overflow",
    "nip09_project_signed_event_target_shape",
    "nip09_project_signed_event_target_empty",
    "nip09_project_signed_event_target_invalid",
    "nip09_project_signed_address_target_shape",
    "nip09_project_signed_address_target_missing_colon",
    "nip09_project_signed_address_target_invalid_pubkey",
    "nip09_project_signed_address_target_unsupported_kind",
    "nip09_project_signed_address_target_identifier_forbidden",
    "nip09_project_signed_target_missing",
    "nip09_project_signed_first_malformed_event_target",
    "nip09_project_signed_first_malformed_address_target",
    "nip09_project_signed_kind_precedes_content",
    "nip09_project_signed_content_precedes_tag_count",
    "nip09_project_signed_tag_count_precedes_element_count",
    "nip09_project_signed_element_count_precedes_element_size",
    "nip09_project_signed_element_size_precedes_tag_bytes",
    "nip09_project_signed_tag_bytes_precedes_wire",
    "nip09_project_signed_wire_precedes_target_parse",
    "nip09_project_signed_target_parse_precedes_missing_union",
];

pub(super) const DELETION_ADMIT_VALID_IDS: [&str; 3] = [
    "nip09_admit_signed_event_target",
    "nip09_admit_signed_address_target",
    "nip09_admit_signed_mixed_tolerant_projection",
];

pub(super) const DELETION_ADMIT_INVALID_IDS: [&str; 5] = [
    "nip09_admit_invalid_signature",
    "nip09_admit_id_mismatch",
    "nip09_admit_wrong_kind",
    "nip09_admit_invalid_target",
    "nip09_admit_target_missing",
];
