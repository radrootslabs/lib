#![forbid(unsafe_code)]

use serde_json::Value;

const HOST_DECISION: &str =
    include_str!("../../../contracts/architecture/decisions/services_hardening_host.v1.json");

fn decision() -> Value {
    serde_json::from_str(HOST_DECISION).expect("host decision must be valid JSON")
}

#[test]
fn admin_routes_envelopes_and_exit_codes_are_unique_and_exact() {
    let value = decision();
    assert_eq!(
        value["schema"],
        "radroots.services-hardening.host-decisions.v1"
    );
    assert_eq!(value["decision_state"], "reserved_preimplementation");
    assert_eq!(
        value["local_admin"]["transport"],
        "http_1_1_over_unix_domain_socket"
    );
    assert_eq!(value["local_admin"]["base_path"], "/v1");
    assert_eq!(value["local_admin"]["tcp_admin"], false);

    assert_eq!(
        value["local_admin"]["mutation_request_envelope"],
        serde_json::json!({
            "required_fields": ["contract_version", "operation_id", "request"],
            "optional_fields": ["correlation_id"],
            "contract_version": 1,
            "operation_id_semantics": "caller_stable_idempotency_identity",
            "correlation_id_semantics": "caller_safe_trace_identity_or_daemon_generated_when_absent",
            "identical_operation_id_reuse": "return_original_committed_result",
            "different_request_operation_id_reuse": {
                "admin_error_code": "operation_id_conflict",
                "cli_exit": 5
            }
        })
    );
    assert_eq!(value["local_admin"]["cors"], false);
    assert_eq!(value["local_admin"]["browser_authentication"], false);
    assert_eq!(
        value["local_admin"]["unknown_major_version"],
        "unsupported_contract_version"
    );
    assert_eq!(value["local_admin"]["unknown_route"], "route_not_found");
    assert_eq!(value["local_admin"]["duplicate_json_fields_rejected"], true);
    assert_eq!(value["local_admin"]["unknown_json_fields_rejected"], true);
    assert_eq!(
        value["local_admin"]["success_response_envelope"],
        serde_json::json!({
            "required_fields": ["contract_version", "ok", "correlation_id", "result"],
            "contract_version": 1,
            "ok": true
        })
    );
    assert_eq!(
        value["local_admin"]["failure_response_envelope"],
        serde_json::json!({
            "required_fields": ["contract_version", "ok", "correlation_id", "error"],
            "error_required_fields": ["code", "message"],
            "contract_version": 1,
            "ok": false
        })
    );
    assert_eq!(
        value["local_admin"]["output_safety"],
        serde_json::json!({
            "request_body_max_utf8_bytes": 65_536,
            "response_body_max_utf8_bytes": 1_048_576,
            "operation_id_max_utf8_bytes": 128,
            "correlation_id_max_utf8_bytes": 128,
            "error_code_max_utf8_bytes": 64,
            "error_message_max_utf8_bytes": 256,
            "redaction_required": true,
            "forbidden_material": [
                "secret_or_credential_material",
                "private_identity_material",
                "decrypted_payload",
                "raw_absolute_or_resolved_path",
                "raw_sql_or_database_error",
                "raw_provider_error",
                "raw_relay_or_network_error",
                "source_error_chain"
            ]
        })
    );

    let routes = value["local_admin"]["common_route_suffixes"]
        .as_array()
        .expect("common routes");
    assert_eq!(
        routes,
        serde_json::json!([
            { "method": "GET", "path": "/status", "operation_suffix": "status.get", "request_model": "empty", "response_model": "service_status_v1" },
            { "method": "GET", "path": "/config/effective", "operation_suffix": "config.effective.get", "request_model": "empty", "response_model": "effective_config_v1" },
            { "method": "GET", "path": "/identity/status", "operation_suffix": "identity.status.get", "request_model": "identity_status_query_v1", "response_model": "identity_status_v1" },
            { "method": "POST", "path": "/identity/rekey", "operation_suffix": "identity.rekey", "request_model": "identity_rekey_request_v1", "response_model": "identity_mutation_receipt_v1" },
            { "method": "POST", "path": "/identity/replace", "operation_suffix": "identity.replace", "request_model": "identity_replace_request_v1", "response_model": "identity_mutation_receipt_v1" },
            { "method": "GET", "path": "/state/status", "operation_suffix": "state.status.get", "request_model": "empty", "response_model": "state_status_v1" },
            { "method": "POST", "path": "/state/backup", "operation_suffix": "state.backup.create", "request_model": "state_backup_request_v1", "response_model": "state_backup_receipt_v1" },
            { "method": "GET", "path": "/metrics/snapshot", "operation_suffix": "metrics.snapshot.get", "request_model": "empty", "response_model": "metrics_snapshot_v1" }
        ])
        .as_array()
        .unwrap()
    );

    assert_eq!(
        value["exit_codes"],
        serde_json::json!([
            { "code": 0, "name": "success", "meaning": "successful command or completed graceful first-signal shutdown" },
            { "code": 1, "name": "unexpected_internal", "meaning": "unexpected invariant, critical task, or internal failure" },
            { "code": 2, "name": "input_or_configuration", "meaning": "CLI, config, validation, or unsupported contract input" },
            { "code": 3, "name": "service_or_dependency_unavailable", "meaning": "daemon, required provider, relay, source, or local dependency unavailable" },
            { "code": 4, "name": "state_or_identity_unavailable", "meaning": "state, schema, lock, credential, or identity unavailable" },
            { "code": 5, "name": "operation_rejected_or_conflict", "meaning": "authorization rejection, idempotency conflict, stale generation, or domain conflict" },
            { "code": 6, "name": "doctor_required_check_failed", "meaning": "one or more required doctor checks failed or timed out" }
        ])
    );
}

#[test]
fn readiness_peer_authorization_and_native_support_fail_closed() {
    let value = decision();
    assert_eq!(
        value["tcp_operations"]["routes"],
        serde_json::json!([
            { "method": "GET", "path": "/livez", "source": "cached_supervisor_state" },
            { "method": "GET", "path": "/readyz", "source": "cached_readiness_state" },
            { "method": "GET", "path": "/metrics", "source": "cached_bounded_metrics_snapshot" }
        ])
    );
    assert_eq!(value["tcp_operations"]["active_probe_per_request"], false);
    assert_eq!(value["tcp_operations"]["additional_routes"], false);
    assert_eq!(value["systemd"]["sd_notify_v1"], false);
    assert_eq!(value["systemd"]["service_type"], "simple");
    assert_eq!(
        value["systemd"]["readiness_authority"],
        "cached_http_readyz"
    );
    assert_eq!(
        value["systemd"]["process_running_does_not_imply_ready"],
        true
    );
    assert_eq!(
        value["peer_authorization"]["linux_service_host"],
        serde_json::json!({
            "credential_api": "SO_PEERCRED",
            "required": true,
            "allow": ["peer_uid_equals_daemon_euid", "peer_primary_gid_equals_configured_admin_gid"],
            "credential_unavailable": "deny",
            "parent_mode_without_admin_gid": "0700",
            "socket_mode_without_admin_gid": "0600",
            "parent_mode_with_admin_gid": "0750",
            "socket_mode_with_admin_gid": "0660"
        })
    );
    assert_eq!(
        value["peer_authorization"]["macos_interactive"],
        serde_json::json!({
            "credential_api": "none_v1",
            "required": false,
            "authority": "filesystem_owner_permissions_only",
            "parent_mode": "0700",
            "socket_mode": "0600",
            "peer_credential_equivalence_claim": false
        })
    );
    assert_eq!(
        value["peer_authorization"]["other_platforms"],
        serde_json::json!({ "admin_support": "unsupported_v1" })
    );
    assert_eq!(
        value["doctor"],
        serde_json::json!({
            "schema": "radroots.service.doctor.v1",
            "contract_version": 1,
            "required_fields": ["contract_version", "service", "instance", "status", "checks"],
            "check_required_fields": ["id", "status", "required", "deadline_ms", "summary", "remediation_code"],
            "statuses": ["pass", "fail", "timeout", "skipped"],
            "required_skipped": "forbidden",
            "aggregate_statuses": ["pass", "degraded", "fail"],
            "aggregation": {
                "required_fail_or_timeout": "fail",
                "optional_fail_timeout_or_skipped": "degraded",
                "otherwise": "pass"
            },
            "optional_nonpass_exit": 0,
            "summary_max_utf8_bytes": 256,
            "raw_error_or_path_allowed": false,
            "required_fail_or_timeout_exit": 6
        })
    );
    assert_eq!(
        value["forced_signal_exit"],
        "operating_system_128_plus_signal_not_remapped"
    );
    assert_eq!(
        value["bare_rust_linux"]["qualification_base"],
        "debian_bookworm_slim_digest_pinned_per_receipt"
    );
    assert_eq!(
        value["bare_rust_linux"]["architectures"],
        serde_json::json!(["x86_64", "aarch64"])
    );
    assert_eq!(
        value["bare_rust_linux"]["rust_install"],
        "rustup_profile_minimal_exact_repository_toolchain"
    );
    assert_eq!(
        value["bare_rust_linux"]["apt_packages"],
        serde_json::json!(["build-essential", "ca-certificates", "git"])
    );
    assert_eq!(
        value["bare_rust_linux"]["not_required_by_final_graph"],
        serde_json::json!([
            "clang",
            "libclang-dev",
            "libsodium-dev",
            "libsqlite3-dev",
            "libssl-dev",
            "pkg-config"
        ])
    );
    assert_eq!(value["bare_rust_linux"]["sqlite"], "bundled");
    assert_eq!(value["bare_rust_linux"]["tls"], "rustls");
    assert_eq!(
        value["bare_rust_linux"]["proof"],
        serde_json::json!([
            "fresh_digest_pinned_base_for_each_architecture",
            "install_only_declared_apt_packages_and_exact_rust_toolchain",
            "locked_format_check_test_clippy_rustdoc_release_build",
            "repeat_with_network_disabled_from_governed_vendor_bundle",
            "fail_if_undeclared_native_package_is_installed_or_linked"
        ])
    );
}
