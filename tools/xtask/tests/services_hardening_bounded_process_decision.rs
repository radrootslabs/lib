#![forbid(unsafe_code)]

use serde_json::Value;

const DECISION: &str = include_str!(
    "../../../contracts/architecture/decisions/services_hardening_bounded_process.v1.json"
);

fn decision() -> Value {
    serde_json::from_str(DECISION).expect("bounded-process decision must be valid JSON")
}

#[test]
fn process_and_resource_bounds_are_exact() {
    let value = decision();
    assert_eq!(
        value["schema"],
        "radroots.services-hardening.bounded-process-decisions.v1"
    );
    assert_eq!(value["contract_version"], 1);
    assert_eq!(value["decision_state"], "active");
    assert_eq!(value["owner"], "tools/xtask");
    assert_eq!(
        value["self_test_command"],
        "cargo xtask bounded-process-self-test"
    );
    assert_eq!(
        value["platform_scope"],
        serde_json::json!({
            "implementation": ["macos_aarch64", "linux_x86_64"],
            "source_gate": ["macos_aarch64"],
            "cross_platform_promotion_owner": "step-297"
        })
    );
    assert_eq!(
        value["process_model"],
        serde_json::json!({
            "child_process_group": "new_group_with_child_as_leader",
            "stdin": "closed_devnull",
            "stdout": "concurrently_drained_live_byte_cap",
            "stderr": "concurrently_drained_live_byte_cap",
            "deadline_clock": "monotonic",
            "normal_leader_exit": "clean_remaining_process_group",
            "failure_cleanup": "term_group_bounded_grace_then_kill_group_and_bounded_reap",
            "unsupported_platform": "fail_closed"
        })
    );
    assert_eq!(
        value["hard_maximums"],
        serde_json::json!({
            "deadline_seconds": 86_400,
            "termination_grace_seconds": 5,
            "stdout_bytes": 67_108_864,
            "stderr_bytes": 67_108_864,
            "environment_entries": 64,
            "environment_name_bytes": 128,
            "environment_value_bytes": 65_536
        })
    );
}

#[test]
fn environment_and_diagnostics_fail_closed() {
    let value = decision();
    assert_eq!(
        value["environment"],
        serde_json::json!({
            "inheritance": "replace_with_explicit_allowlist",
            "ambient_snapshot": false,
            "duplicate_name": "reject",
            "invalid_name_or_nul": "reject",
            "forbidden_names": [
                "CARGO_ENCODED_RUSTFLAGS",
                "DYLD_INSERT_LIBRARIES",
                "DYLD_LIBRARY_PATH",
                "LD_LIBRARY_PATH",
                "LD_PRELOAD",
                "NIX_CONFIG",
                "NIX_PATH",
                "NIXPKGS_ALLOW_BROKEN",
                "NIXPKGS_ALLOW_UNFREE",
                "RUSTFLAGS"
            ],
            "forbidden_name_patterns": [
                "*CREDENTIAL*",
                "*KEY*",
                "*PASSWORD*",
                "*SECRET*",
                "*TOKEN*"
            ],
            "equivalent_build_controls": [
                "CARGO_BUILD_RUSTFLAGS",
                "CARGO_TARGET_*_RUSTFLAGS",
                "DYLD_*",
                "LD_AUDIT",
                "LD_DEBUG",
                "LD_PROFILE",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "RUSTDOCFLAGS"
            ]
        })
    );
    assert_eq!(
        value["diagnostic_safety"],
        serde_json::json!({
            "program_argv_cwd": "redacted",
            "environment_values": "redacted",
            "captured_stream_bytes": "redacted",
            "operating_system_error_text": "redacted",
            "source_error_chain": "absent"
        })
    );
    assert_eq!(
        value["required_vectors"],
        serde_json::json!([
            "timeout",
            "orphan_child",
            "stdout_cap",
            "stderr_cap",
            "closed_stdin",
            "inherited_build_environment",
            "loader_injection",
            "redaction"
        ])
    );
}
