#![forbid(unsafe_code)]

use serde_json::Value;
use sha2::{Digest as _, Sha256};

const DECISION: &str = include_str!(
    "../../../contracts/architecture/decisions/services_hardening_advisory_snapshots.v1.json"
);
const DECISION_SHA256: &str = "0eb19c747a160728f256329a36bd4000deca0fb433d4971f4472f6f792905212";

fn decision() -> Value {
    serde_json::from_str(DECISION).expect("advisory-snapshot decision must be valid JSON")
}

#[test]
fn authority_scope_and_complete_file_are_exact() {
    let value = decision();
    assert_eq!(value.as_object().map(serde_json::Map::len), Some(19));
    assert_eq!(
        hex::encode(Sha256::digest(DECISION.as_bytes())),
        DECISION_SHA256
    );
    assert_eq!(
        value["schema"],
        "radroots.services-hardening.advisory-snapshot-decisions.v1"
    );
    assert_eq!(value["contract_version"], 1);
    assert_eq!(value["decision_state"], "active");
    assert_eq!(value["owner"], "tools/xtask");
    assert_eq!(
        value["self_test_command"],
        "cargo xtask advisory-snapshot-self-test"
    );
    assert_eq!(
        value["platform_scope"],
        serde_json::json!({
            "implementation": ["macos_aarch64"],
            "source_gate": ["macos_aarch64"],
            "cross_platform_promotion_owner": "step-297"
        })
    );
    assert_eq!(
        value["scope"],
        serde_json::json!({
            "claims": [
                "closed_advisory_workload_inventory",
                "exact_step_287_tool_acquisition_binding",
                "one_external_immutable_rustsec_snapshot_per_candidate",
                "one_bounded_nvd_only_update_per_candidate",
                "offline_no_update_analysis",
                "digest_time_freshness",
                "owned_exact_expiring_suppressions",
                "descriptor_admitted_actual_scanner_outputs",
                "deterministic_byte_canonical_snapshot_archives"
            ],
            "nonclaims": [
                "live_rustsec_snapshot_acquisition",
                "live_nvd_update_or_provider_access_at_step_294",
                "scanner_or_provider_current_availability_before_step_297",
                "candidate_or_generation_instance_created",
                "step_314_advisory_qualification_or_pass_evidence",
                "oss_index_or_credential_authority",
                "cross_platform_runtime_promotion_before_step_297",
                "artifact_nix_oci_signing_notarization_publication_deployment_or_production_qualification"
            ]
        })
    );
}

#[test]
fn workload_inventory_and_tool_pins_are_exact() {
    let value = decision();
    let cargo_ids = value["workload_inventory"]["cargo"]
        .as_array()
        .expect("Cargo inventory")
        .iter()
        .map(|row| row["id"].as_str().expect("Cargo workload id"))
        .collect::<Vec<_>>();
    assert_eq!(
        cargo_ids,
        [
            "cli",
            "harvest_core",
            "harvest_xtask",
            "ios_ffi",
            "lib",
            "myc",
            "radrootsd",
            "rhi",
            "root",
            "sdk"
        ]
    );
    let gradle_rows = value["workload_inventory"]["gradle_kotlin"]
        .as_array()
        .expect("Gradle inventory");
    assert_eq!(
        gradle_rows
            .iter()
            .map(|row| (
                row["id"].as_str().expect("Gradle workload id"),
                row["build_root"].as_str().expect("Gradle build root"),
                row["project_path"].as_str().expect("Gradle project path"),
                row["configuration"].as_str().expect("Gradle configuration")
            ))
            .collect::<Vec<_>>(),
        [
            (
                "app_design_system",
                ".",
                ":app:design_system",
                "desktopRuntimeClasspath"
            ),
            ("app_desktop", ".", ":app:desktop", "runtimeClasspath"),
            ("app_shared", ".", ":app:shared", "desktopRuntimeClasspath"),
            (
                "tools_design_catalog",
                ".",
                ":tools:design_catalog",
                "desktopRuntimeClasspath"
            ),
            (
                "build_logic_contracts",
                "build-logic",
                ":contracts",
                "runtimeClasspath"
            ),
            (
                "build_logic_plugins",
                "build-logic",
                ":plugins",
                "runtimeClasspath"
            )
        ]
    );
    for (id, version, executable, source) in [
        (
            "cargo_audit",
            "0.22.1",
            "c5cd7c0da8a9d0dff338aa1a2a30b0c723fde8201c23481f49e75be0bb77fe74",
            "2f4e27b0ab2d116c87c29db159ad42565cdcdccf77eb62ef0486ddd017a02da6",
        ),
        (
            "owasp_dependency_check",
            "12.2.2",
            "d683a49ec335eeca93d8707f3e8ce21d7ba63a1e619a325c6518f89c25efcdc4",
            "bf07fefd81af3094c5f6850423b014df44db62ce2dbad0f79079a90df675e44a",
        ),
        (
            "java",
            "21.0.12.1",
            "9be1d0a740ff6502df1a762145e62860f5de4b7e17658d9cb9498da3acf9d16c",
            "575bb8d9d604821d8f350325b28a35e49bcffd7ec33727b41edc8d709537dada",
        ),
        (
            "gradle_wrapper",
            "9.5.0",
            "ab5c0cad16305af2e619c159c1f58dd68d07fab9c11e36701e109c0277407f7a",
            "553c78f50dafcd54d65b9a444649057857469edf836431389695608536d6b746",
        ),
    ] {
        let row = &value["tool_acquisition"][id]["platform"][0];
        assert_eq!(row["normalized_version"], version);
        assert_eq!(
            row["package_receipt_projection"]["selected_executable"]["exact_bytes_sha256"],
            executable
        );
        assert_eq!(row["source_sha256"], source);
    }
}

#[test]
fn immutable_archives_reports_and_execution_bounds_are_exact() {
    let value = decision();
    assert_eq!(
        value["snapshot_model"]["archive_reencoding_backend"],
        serde_json::json!({
            "compression_level": 6,
            "flate2": "1.1.9",
            "flate2_checksum": "843fba2746e448b37e26a819579957415c8cef339bf08564fe8b7ddbd959573c",
            "miniz_oxide": "0.8.9",
            "miniz_oxide_checksum": "1fa76a2c86f704bdb222d66965fb3d63269ce38518b83cb0575fca855ebb6316",
            "golden_fixture_byte_length": 147,
            "golden_fixture_sha256": "6073c70a98ff9ab610085ed74b7886811da3fe64e9ec2771464300a94befd0fb"
        })
    );
    assert_eq!(
        value["owasp_dependency_check_nvd"]["report_template_binding"],
        serde_json::json!({
            "distribution_package_receipt_sha256": "83d9a668179fb5e9ad6fd5c625a9ab58b8371a6d32b59808fc8d2973957f54a2",
            "dependency_check_core_jar_relative_identity": "dependency-check-core-12.2.2.jar",
            "dependency_check_core_jar_sha256": "1ebe55e542b2f4d2727380395843922381447dc8b9a4f1633e77096f47ebfb48",
            "template_member": "templates/jsonReport.vsl",
            "template_member_byte_length": 34639,
            "template_member_sha256": "35881409102304d5a91d08566c1d0fda6fa5f9ec8810994fcdd5106aed3729b5",
            "parser": "strict_exact_schema_for_report_scan_project_dependency_evidence_identifier_related_dependency_vulnerability_cvss_reference_and_vulnerable_software_objects_with_unknown_fields_rejected"
        })
    );
    assert_eq!(
        value["bounded_execution"],
        serde_json::json!({
            "runner": "step_292_bounded_process_runner",
            "environment": "replace_with_explicit_allowlist",
            "stdin": "closed",
            "deadline_clock": "monotonic",
            "maximum_concurrent_top_level_operations": 1,
            "required_process_receipts": 23,
            "maximum_command_seconds": 3600,
            "maximum_stdout_bytes": 67108864,
            "maximum_stderr_bytes": 67108864,
            "timeout_unavailable_interrupted_or_unparseable": "nonpass_without_fallback",
            "exit_status_interpretation": "provider_specific_and_never_substitutes_for_complete_report_parsing",
            "working_directories": "unique_opaque_identity_per_operation_with_kind_logical_uri_pre_and_post_entry_counts_and_tree_digests_cargo_audit_empty_config_free_gradle_retained_candidate_source_nvd_update_fresh_then_nonempty_and_odc_analysis_fresh_empty",
            "environment_evidence": "exact_ordered_name_and_logical_role_allowlist_with_sha256_of_each_actual_replacement_value_fixed_C_and_UTC_values_shared_pinned_java_and_path_values_and_unique_private_roots_no_ambient_inheritance",
            "authority_byte_limits": {
                "snapshot_manifest": 67108864,
                "producer_request": 1048576,
                "tool_manifest": 16777216,
                "tool_observation": 4194304,
                "provider_execution_evidence": 67108864,
                "raw_scanner_output_each": 33554432
            }
        })
    );
}

#[test]
fn freshness_suppressions_results_and_required_vectors_are_exact() {
    let value = decision();
    assert_eq!(
        value["freshness"],
        serde_json::json!({
            "clock": "caller_supplied_positive_utc_epoch_seconds",
            "snapshot_time": "provider_digest_finalization_epoch_bound_to_the_accepted_snapshot_digest",
            "future_snapshot_time": "reject",
            "age_arithmetic": "checked_nonnegative_seconds",
            "qualification_maximum_age_seconds": 86400,
            "development_reuse_maximum_age_seconds": 604800,
            "boundary": "fresh_when_age_is_less_than_or_equal_to_the_selected_maximum",
            "providers_evaluated_independently": true,
            "stale_or_missing_provider": "nonpass"
        })
    );
    assert_eq!(
        value["suppressions"],
        serde_json::json!({
            "inventory": "canonical_ordered_exact_rows_bound_into_the_candidate_advisory_input_digest",
            "required_fields": [
                "id", "provider", "advisory_id", "workload_id", "package_ecosystem",
                "package_namespace", "package_name", "package_version", "owner", "rationale",
                "created_at_epoch", "expires_at_epoch"
            ],
            "matching": "one_row_matches_one_finding_only_when_provider_advisory_id_workload_id_package_ecosystem_package_namespace_package_name_and_package_version_are_all_exact",
            "wildcards_ranges_regex_and_lexical_patterns": "forbidden",
            "owner_and_rationale": "nonempty_bounded_safe_text",
            "expiry": "active_only_when_evaluation_epoch_is_strictly_less_than_expires_at_epoch",
            "expiry_equality": "expired",
            "future_creation_time": "reject",
            "unused_duplicate_ambiguous_or_unknown_rows": "nonpass",
            "non_waivable_advisory_ids": ["RUSTSEC-2026-0253"],
            "non_waivable_match": "reject_even_when_a_suppression_row_is_otherwise_exact"
        })
    );
    assert_eq!(
        value["required_vectors"],
        serde_json::json!([
            "known_vulnerable_fixture",
            "missing_inventory",
            "unavailable",
            "stale",
            "timeout",
            "expired_suppression"
        ])
    );
    assert_eq!(
        value["result_model"]["pass"],
        "both_provider_snapshots_fresh_all_sixteen_workloads_present_in_exact_order_all_scans_completed_and_every_finding_is_absent_or_exactly_suppressed"
    );
    assert_eq!(value["result_model"]["unavailable"], "nonpass");
    assert_eq!(value["result_model"]["timeout"], "nonpass");
    assert_eq!(value["result_model"]["stale"], "nonpass");
}
