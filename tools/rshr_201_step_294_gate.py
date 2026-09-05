#!/usr/bin/env python3
"""Emit the source-bound RSHR-201 gate result for Lib Step 294."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import rshr_201_step_gate as step_292


ROOT = Path(__file__).resolve().parent.parent
AUTHORITY_PATH = ROOT / "contracts/rshr-201-step-294-gates.v1.json"
STEP = 294
GATE_DEFINITION = (
    "known-vulnerable fixtures, missing inventory, unavailable, stale, timeout, "
    "and expired-suppression vectors"
)
GATE_DIGEST = "d61e7135b7f468f32bc765be844399ade531649d85a4f29e57c37c83eced8bcf"
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
    "tools/rshr_201_step_294_gate.py",
    "--step={step}",
    "--check-id={check_id}",
    "--source-revision={source_revision}",
    "--source-tree={source_tree}",
    "--candidate-digest={candidate_digest}",
    "--platform=macos_aarch64",
    "--execution-request-sha256={execution_request_sha256}",
]
IMMUTABLE_STEP_293 = {
    "contracts/rshr-201-step-293-gates.v1.json": (
        "456753ae166022205ee0f0f4722a4ea424adcf1f51f9852045749fd38b436c4e"
    ),
    "tools/rshr_201_step_293_gate.py": (
        "060ce66d435d7bb77bd7fb7c068e242c87bb4768819d85fbf45da324e1afac39"
    ),
}
EXPECTED_UNIT_TESTS = {
    "safe_artifact_io::step_294_tests": [
        "safe_artifact_io::step_294_tests::canonical_tar_gzip_is_exactly_reencoded_before_admission",
        "safe_artifact_io::step_294_tests::materialization_retains_exact_member_and_parent_bindings",
    ],
    "advisory_snapshot::tests": [
        "advisory_snapshot::tests::expired_suppression_is_rejected",
        "advisory_snapshot::tests::known_vulnerable_fixture",
        "advisory_snapshot::tests::missing_inventory_is_rejected",
        "advisory_snapshot::tests::stale_snapshot_is_rejected",
        "advisory_snapshot::tests::timed_out_operation_is_nonpass",
        "advisory_snapshot::tests::unavailable_provider_is_nonpass",
    ],
}
EXPECTED_DECISION_TESTS = [
    "authority_scope_and_complete_file_are_exact",
    "freshness_suppressions_results_and_required_vectors_are_exact",
    "immutable_archives_reports_and_execution_bounds_are_exact",
    "workload_inventory_and_tool_pins_are_exact",
]


def run_cargo(arguments: list[str], *, label: str) -> bytes:
    return step_292.run(
        ["cargo", "+1.97.1", *arguments],
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
    run_cargo([*base, "--", "--test-threads=1"], label=label)


def run_step() -> None:
    run_cargo(["fmt", "--all", "--", "--check"], label="Step 294 formatting check")
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
        label="Step 294 lint check",
    )
    for test_filter, label in [
        ("safe_artifact_io::step_294_tests", "deterministic archive vectors"),
        ("advisory_snapshot::tests", "advisory snapshot vectors"),
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
            "advisory-snapshot-self-test",
        ],
        label="advisory snapshot self-test",
    )
    decision_base = [
        "test",
        "--offline",
        "--manifest-path",
        "tools/xtask/Cargo.toml",
        "--locked",
        "--test",
        "services_hardening_advisory_snapshot_decision",
    ]
    listed = run_cargo(
        [*decision_base, "--", "--list", "--format=terse"],
        label="advisory snapshot decision contract inventory",
    )
    require_listed_tests(
        listed,
        EXPECTED_DECISION_TESTS,
        label="advisory snapshot decision contract",
    )
    run_cargo(
        [*decision_base, "--", "--test-threads=1"],
        label="advisory snapshot decision contract",
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
        "verifier_path": "tools/rshr_201_step_294_gate.py",
        "verifier_sha256": verifier_digest,
    }


def require_immutable_step_293() -> None:
    for relative, expected in IMMUTABLE_STEP_293.items():
        if step_292.sha256_bytes(step_292.read_regular(ROOT / relative)) != expected:
            raise step_292.GateError("Step 293 immutable authority differs")


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

    require_immutable_step_293()
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
        or authority.get("schema") != "radroots.lib.rshr-201-step-294-gates.v1"
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
            {"platform": arguments.platform, "sha256": arguments.execution_request_sha256}
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
        print(f"Lib RSHR-201 Step 294 gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
    except Exception:
        print("Lib RSHR-201 Step 294 gate failed safely", file=sys.stderr)
        raise SystemExit(1)
