#!/usr/bin/env python3
"""Emit the source-bound RSHR-201 gate result for Lib Step 293."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import rshr_201_step_gate as step_292


ROOT = Path(__file__).resolve().parent.parent
AUTHORITY_PATH = ROOT / "contracts/rshr-201-step-293-gates.v1.json"
STEP = 293
GATE_DEFINITION = (
    "symlink, FIFO, replacement, archive-bomb, and "
    "missing-failed-skipped lane vectors"
)
GATE_DIGEST = "84d8022c579c03ed4192b0cd579bf105abf3f3e816cdafc3a53f53bc68460baf"
CHECK_ID = f"gate-01-{GATE_DIGEST}"
ASSERTION_ID = f"step_{STEP:03d}_gate_01_{GATE_DIGEST}"
EXPECTED_ARGV_TEMPLATE = [
    "cargo",
    "extbuild",
    "run",
    "--",
    "uv",
    "run",
    "--offline",
    "--no-project",
    "python3",
    "-B",
    "tools/rshr_201_step_293_gate.py",
    "--step={step}",
    "--check-id={check_id}",
    "--source-revision={source_revision}",
    "--source-tree={source_tree}",
    "--candidate-digest={candidate_digest}",
    "--platform=macos_aarch64",
    "--execution-request-sha256={execution_request_sha256}",
]
IMMUTABLE_STEP_292 = {
    "contracts/rshr-201-step-gates.v1.json": (
        "d787d9980e8fdcedbdf2fdf43e4eddf05220714598dfc44ec8d049acf05efd6c"
    ),
    "tools/rshr_201_step_gate.py": (
        "0ceefa6f116993106ceb94dc4ca9f64cc3c6de85b54a9a7aff327cef92e34b37"
    ),
}
EXPECTED_UNIT_TESTS = {
    "safe_artifact_io::tests": [
        "safe_artifact_io::tests::archive_rejects_duplicates_prefix_conflicts_and_concatenation",
        "safe_artifact_io::tests::bounded_read_hash_and_copy_are_streaming_and_exact",
        "safe_artifact_io::tests::diagnostics_are_redacted",
        "safe_artifact_io::tests::hard_maximums_reject_invalid_requests",
        "safe_artifact_io::tests::hardlinked_regular_inputs_are_rejected",
        "safe_artifact_io::tests::no_follow_admission_rejects_symlink_fifo_and_replacement",
        "safe_artifact_io::tests::tar_gzip_admission_is_parse_only_and_bounded",
        "safe_artifact_io::tests::traversal_enforces_type_count_byte_and_depth_bounds",
    ],
    "release_preflight::tests": [
        "release_preflight::tests::aggregate_diagnostics_are_static",
        "release_preflight::tests::every_nonpass_state_fails_closed_without_short_circuiting",
        "release_preflight::tests::exact_inventory_and_order_are_closed",
        "release_preflight::tests::missing_duplicate_and_unexpected_lanes_fail_closed",
    ],
    "service_release_artifacts::tests": [
        "service_release_artifacts::tests::absent_nix_material_is_preserved_without_invented_digest_evidence",
        "service_release_artifacts::tests::binary_archive_is_reproducible_and_metadata_is_fixed",
        "service_release_artifacts::tests::contract_matches_the_checked_in_decision",
        "service_release_artifacts::tests::contract_rejects_every_independent_governed_field_drift",
        "service_release_artifacts::tests::errors_are_stable_and_source_free",
        "service_release_artifacts::tests::exact_inventory_and_limits_are_literal",
        "service_release_artifacts::tests::file_admission_predicates_reject_each_independent_drift",
        "service_release_artifacts::tests::full_artifact_set_is_reproducible_immutable_and_verifiable",
        "service_release_artifacts::tests::identifier_predicates_reject_each_independent_boundary",
        "service_release_artifacts::tests::identifiers_and_sources_are_closed",
        "service_release_artifacts::tests::low_level_release_admission_and_comparison_branches_are_qualified",
        "service_release_artifacts::tests::output_name_is_one_bounded_component",
        "service_release_artifacts::tests::output_scope_and_target_admission_are_fail_closed",
        "service_release_artifacts::tests::output_scope_remote_and_inventory_reject_each_drift",
        "service_release_artifacts::tests::package_metadata_rejects_each_independent_field_drift",
        "service_release_artifacts::tests::private_or_incomplete_dependency_evidence_is_rejected",
        "service_release_artifacts::tests::protected_text_and_invalid_inventory_fail_closed",
        "service_release_artifacts::tests::release_metadata_and_text_admission_reject_each_field_drift",
        "service_release_artifacts::tests::release_service_and_workspace_binding_fail_closed",
        "service_release_artifacts::tests::remaining_release_boundaries_fail_closed",
        "service_release_artifacts::tests::secret_scanner_detects_a_pattern_across_chunk_boundaries",
        "service_release_artifacts::tests::snapshot_backed_inventory_rejects_file_and_root_mode_races",
        "service_release_artifacts::tests::source_lock_and_source_bundle_drift_fail_closed",
        "service_release_artifacts::tests::supply_chain_documents_are_deterministic_and_complete",
        "service_release_artifacts::tests::supply_chain_graph_rejects_each_independent_structural_drift",
    ],
}
EXPECTED_DECISION_TESTS = [
    "aggregate_inventory_nonclaims_and_vectors_are_exact",
    "filesystem_traversal_copy_and_archive_models_are_exact",
    "scope_platform_and_hard_maximums_are_exact",
]


def run_cargo(arguments: list[str], *, label: str) -> bytes:
    return step_292.run(
        [
            "cargo",
            "+1.97.1",
            *arguments,
        ],
        step_292.gate_environment(),
        label=label,
    )


def require_listed_tests(output: bytes, expected: list[str], *, label: str) -> None:
    try:
        lines = output.decode("utf-8", "strict").splitlines()
    except UnicodeError as error:
        raise step_292.GateError(f"{label} inventory is not UTF-8") from error
    observed = sorted(
        line.removesuffix(": test") for line in lines if line.endswith(": test")
    )
    if observed != sorted(expected):
        raise step_292.GateError(f"{label} inventory differs")


def run_unit_test_lane(test_filter: str, expected: list[str], *, label: str) -> None:
    base = [
        "test",
        "--offline",
        "--manifest-path",
        "tools/xtask/Cargo.toml",
        "--locked",
        "--bin",
        "xtask",
        test_filter,
    ]
    listed = run_cargo(
        [*base, "--", "--list", "--format=terse"],
        label=f"{label} inventory",
    )
    require_listed_tests(listed, expected, label=label)
    run_cargo(
        [*base, "--", "--test-threads=1"],
        label=label,
    )


def run_step() -> None:
    run_cargo(
        [
            "fmt",
            "--all",
            "--",
            "--check",
        ],
        label="Step 293 formatting check",
    )
    run_cargo(
        [
            "clippy",
            "--offline",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--locked",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        label="Step 293 lint check",
    )
    for test_filter, label in [
        ("safe_artifact_io::tests", "safe-artifact I/O vectors"),
        ("release_preflight::tests", "release-preflight aggregate vectors"),
        ("service_release_artifacts::tests", "release-artifact adoption vectors"),
    ]:
        run_unit_test_lane(
            test_filter,
            EXPECTED_UNIT_TESTS[test_filter],
            label=label,
        )
    run_cargo(
        [
            "run",
            "--offline",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--locked",
            "--",
            "safe-artifact-io-self-test",
        ],
        label="safe-artifact and aggregate self-test",
    )
    decision_base = [
        "test",
        "--offline",
        "--manifest-path",
        "tools/xtask/Cargo.toml",
        "--locked",
        "--test",
        "services_hardening_safe_artifact_io_decision",
    ]
    listed = run_cargo(
        [*decision_base, "--", "--list", "--format=terse"],
        label="safe-artifact I/O decision contract inventory",
    )
    require_listed_tests(
        listed,
        EXPECTED_DECISION_TESTS,
        label="safe-artifact I/O decision contract",
    )
    run_cargo(
        [*decision_base, "--", "--test-threads=1"],
        label="safe-artifact I/O decision contract",
    )
    if step_292.git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all"):
        raise step_292.GateError("verification changed the tracked or untracked source state")


def parse_arguments() -> argparse.Namespace:
    parser = step_292.RedactedArgumentParser(allow_abbrev=False)
    parser.add_argument("--step", type=int, required=True)
    parser.add_argument("--check-id")
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--source-tree", required=True)
    parser.add_argument("--candidate-digest")
    parser.add_argument("--platform", required=True)
    parser.add_argument("--execution-request-sha256", required=True)
    return parser.parse_args()


def expected_contract(verifier_digest: str) -> dict[str, object]:
    return {
        "argv_template": EXPECTED_ARGV_TEMPLATE,
        "assertion_id": [ASSERTION_ID],
        "check_id": CHECK_ID,
        "environment_authority": step_292.EXPECTED_ENVIRONMENT_AUTHORITY,
        "environment_names": step_292.EXPECTED_ENVIRONMENT_NAMES,
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["uv", "python3", "git"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/rshr_201_step_293_gate.py",
        "verifier_sha256": verifier_digest,
    }


def require_immutable_step_292() -> None:
    for relative, expected in IMMUTABLE_STEP_292.items():
        if step_292.sha256_bytes(step_292.read_regular(ROOT / relative)) != expected:
            raise step_292.GateError("Step 292 immutable authority differs")


def main() -> int:
    arguments = parse_arguments()
    if arguments.step != STEP:
        raise step_292.GateError("step is outside the Lib gate authority")
    step_292.validate_digest(arguments.source_revision, "source revision", 40)
    step_292.validate_digest(arguments.source_tree, "source tree", 40)
    step_292.validate_digest(arguments.execution_request_sha256, "execution request", 64)
    if step_292.sha256_bytes(GATE_DEFINITION.encode("utf-8")) != GATE_DIGEST:
        raise step_292.GateError("compiled gate definition digest differs")
    if arguments.check_id != CHECK_ID:
        raise step_292.GateError("check identity differs")
    if arguments.candidate_digest != "none" or arguments.platform != "macos_aarch64":
        raise step_292.GateError("candidate or platform scope differs")

    require_immutable_step_292()
    authority_bytes = step_292.read_regular(AUTHORITY_PATH, 256 * 1024)
    try:
        authority = json.loads(authority_bytes)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise step_292.GateError("gate authority is not canonical JSON") from error
    if step_292.canonical(authority) + b"\n" != authority_bytes:
        raise step_292.GateError("gate authority is not canonical JSON")
    if (
        not isinstance(authority, dict)
        or set(authority) != {"schema", "step", "gate_command_contract"}
        or authority.get("schema") != "radroots.lib.rshr-201-step-293-gates.v1"
        or authority.get("step") != [STEP]
    ):
        raise step_292.GateError("gate authority step inventory differs")
    contracts = authority.get("gate_command_contract")
    if not isinstance(contracts, list) or len(contracts) != 1:
        raise step_292.GateError("gate command authority is absent or duplicated")

    verifier_digest = step_292.sha256_bytes(step_292.read_regular(Path(__file__).resolve()))
    contract = contracts[0]
    if contract != expected_contract(verifier_digest):
        raise step_292.GateError("gate command authority differs from source bytes")

    step_292.require_source_state(arguments.source_revision, arguments.source_tree)
    run_step()
    assertions = [{"id": ASSERTION_ID, "result": "pass"}]
    result = {
        "schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "step": STEP,
        "check_id": CHECK_ID,
        "gate_definition_sha256": GATE_DIGEST,
        "source_revision": arguments.source_revision,
        "source_tree": arguments.source_tree,
        "candidate_generation": 0,
        "candidate_digest": "none",
        "command_contract_sha256": step_292.sha256_bytes(step_292.canonical(contract)),
        "verifier_sha256": verifier_digest,
        "execution_request": [
            {
                "platform": arguments.platform,
                "sha256": arguments.execution_request_sha256,
            }
        ],
        "assertion_inventory_sha256": step_292.sha256_bytes(
            step_292.canonical(assertions)
        ),
        "assertion": assertions,
        "result": "pass",
    }
    sys.stdout.buffer.write(step_292.canonical(result) + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except step_292.GateError as error:
        print(f"Lib RSHR-201 Step 293 gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
    except Exception:
        print("Lib RSHR-201 Step 293 gate failed safely", file=sys.stderr)
        raise SystemExit(1)
