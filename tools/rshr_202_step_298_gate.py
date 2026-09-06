#!/usr/bin/env python3
"""Emit the source-bound RSHR-202 gate result for Lib Step 298."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

import rshr_201_step_gate as shared


ROOT = Path(__file__).resolve().parent.parent
AUTHORITY_PATH = ROOT / "contracts/rshr-202-step-298-gates.v1.json"
ORIGIN = "ssh://git@github.com/radrootslabs/lib.git"
BRANCH = "rshr/rcld-202"
STEP = 298
GATE_DEFINITION = (
    "contract, transition, source-lock, Windows, macOS-x86_64, Linux-aarch64, "
    "and undeclared-system mutation vectors"
)
GATE_DIGEST = hashlib.sha256(GATE_DEFINITION.encode("utf-8")).hexdigest()
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
    "tools/rshr_202_step_298_gate.py",
    "--step={step}",
    "--check-id={check_id}",
    "--source-revision={source_revision}",
    "--source-tree={source_tree}",
    "--candidate-digest={candidate_digest}",
    "--platform=macos_aarch64",
    "--execution-request-sha256={execution_request_sha256}",
]
EXPECTED_FILES = {
    "contracts/architecture/decisions/services_hardening_source_lock.v3.json": (
        "3bc32c8ca2cecb06c8f8239ab1fe1fcfba93fe3ef0d60e9b078390347d08f817"
    ),
    "contracts/release/lib-artifact-contract.v3.json": (
        "bc352a132dd4c0e6f1d2ae7449998833efe1fdda2ab851e512bc9241c49edbf0"
    ),
    "contracts/architecture/decisions/services_hardening_build_qualification.v3.json": (
        "4f1bf59e6411c28c9b202c81ed9455c3446525fc39c8e96276a48e4223de1394"
    ),
    "build/nix/service/systems.nix": (
        "d16e21827022a2315234f4c5e4b485017a36ecd90a5559b01d23331cdd505e46"
    ),
    "contracts/releases/target_matrix.toml": (
        "28583b0a163e51468d9688b463902ec2cd22b59c99061630596839baf9396527"
    ),
    "tools/xtask/src/service_build_qualification.rs": (
        "20dbae0f446bdd95e99f84d1c27ef4dec422eae8035ead5876adba047db20a9f"
    ),
    "tools/xtask/src/target_qualification.rs": (
        "0e5b9506c70f5175edeae7cf9b7fb0f55a0cb6abf465a3f708d4234b2069c585"
    ),
}
EXPECTED_TESTS = {
    "service_build_qualification::tests": [
        "service_build_qualification::tests::checked_in_contract_and_fixture_are_exact",
        "service_build_qualification::tests::contract_inventory_is_literal_and_complete",
        "service_build_qualification::tests::contract_rejects_every_independent_governed_field_drift",
        "service_build_qualification::tests::errors_are_fixed_and_source_free",
        "service_build_qualification::tests::fixture_rejects_every_identity_and_lockfile_drift",
        "service_build_qualification::tests::fixture_rejects_every_independent_metadata_drift",
    ],
    "target_qualification::tests": [
        "target_qualification::tests::current_contract_selects_exact_toolchains_targets_and_packages",
        "target_qualification::tests::unsupported_production_targets_are_rejected",
    ],
}
EXPECTED_NIX_SHA256 = (
    "a59ab70f97f6d571642d13c7506aafec0a4275520d53daee2d8451be7c495cd1"
)
EXPECTED_NIX_VERSION_SHA256 = (
    "6db806391ffaea4cdb08ade0031feac399c0cd08474b3bfde8cb33f88a36c8e1"
)


def run_cargo(arguments: list[str], *, label: str) -> bytes:
    return shared.run(
        ["cargo", "+1.97.1", *arguments], shared.gate_environment(), label=label
    )


def require_listed_tests(output: bytes, expected: list[str], *, label: str) -> None:
    try:
        observed = sorted(
            line.removesuffix(": test")
            for line in output.decode("utf-8", "strict").splitlines()
            if line.endswith(": test")
        )
    except UnicodeError as error:
        raise shared.GateError(f"{label} inventory is not UTF-8") from error
    if observed != sorted(expected):
        raise shared.GateError(f"{label} inventory differs")


def require_source_state(source_revision: str, source_tree: str) -> None:
    if (
        shared.git("rev-parse", "HEAD") != source_revision
        or shared.git("rev-parse", "HEAD^{tree}") != source_tree
        or shared.git("symbolic-ref", "--short", "HEAD") != BRANCH
        or shared.git("remote", "get-url", "origin") != ORIGIN
        or shared.git("rev-parse", f"refs/remotes/origin/{BRANCH}") != source_revision
        or shared.git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all")
    ):
        raise shared.GateError("Lib source is not clean and tracking-exact")
    tracked = shared.git_bytes("ls-files", "-z").split(b"\0")
    if any(path == b".github" or path.startswith(b".github/") for path in tracked):
        raise shared.GateError("forbidden .github surface is tracked")
    if os.path.lexists(ROOT / ".github"):
        raise shared.GateError("forbidden .github surface is present")


def require_exact_sources() -> None:
    for relative, expected in EXPECTED_FILES.items():
        contents = shared.read_regular(ROOT / relative)
        if shared.sha256_bytes(contents) != expected:
            raise shared.GateError("Step 298 governed source bytes differ")


def run_test_lane(test_filter: str, expected: list[str]) -> None:
    base = [
        "test",
        "--offline",
        "--locked",
        "-p",
        "xtask",
        test_filter,
    ]
    listed = run_cargo(
        [*base, "--", "--list", "--format=terse"],
        label=f"Step 298 {test_filter} inventory",
    )
    require_listed_tests(listed, expected, label=test_filter)
    run_cargo(
        [*base, "--", "--test-threads=1"], label=f"Step 298 {test_filter}"
    )


def run_nix_lane() -> None:
    executable_name = os.environ.get("RSHR_NIX_EXECUTABLE", "nix")
    selected = Path(executable_name)
    if not selected.is_absolute():
        selected = Path(shutil.which(executable_name) or "")
    try:
        executable = selected.resolve(strict=True)
    except OSError as error:
        raise shared.GateError("Step 298 Nix client is unavailable") from error
    if (
        not executable.is_file()
        or shared.sha256_bytes(executable.read_bytes()) != EXPECTED_NIX_SHA256
    ):
        raise shared.GateError("Step 298 Nix client identity differs")
    environment = shared.gate_environment()
    version = shared.run(
        [os.fspath(executable), "--version"], environment, label="Step 298 Nix version"
    )
    if shared.sha256_bytes(version) != EXPECTED_NIX_VERSION_SHA256:
        raise shared.GateError("Step 298 Nix version differs")
    systems = shared.run(
        [
            os.fspath(executable),
            "--offline",
            "eval",
            "--json",
            "--file",
            "build/nix/service/systems.nix",
        ],
        environment,
        label="Step 298 Nix system evaluation",
    )
    if systems != b'["aarch64-darwin","x86_64-linux"]\n':
        raise shared.GateError("Step 298 Nix systems differ")
    shared.run(
        [
            os.fspath(executable),
            "--offline",
            "flake",
            "check",
            "--no-build",
            "--no-write-lock-file",
        ],
        environment,
        label="Step 298 Nix flake evaluation",
    )


def run_step() -> None:
    require_exact_sources()
    run_cargo(["fmt", "--all", "--", "--check"], label="Step 298 formatting")
    for test_filter, expected in EXPECTED_TESTS.items():
        run_test_lane(test_filter, expected)
    run_cargo(
        ["run", "--offline", "--locked", "-q", "-p", "xtask", "--", "contract", "validate"],
        label="Step 298 contract validation",
    )
    run_nix_lane()
    if shared.git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all"):
        raise shared.GateError("verification changed the source state")


def parse_arguments() -> argparse.Namespace:
    parser = shared.RedactedArgumentParser(allow_abbrev=False)
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
        "environment_authority": shared.EXPECTED_ENVIRONMENT_AUTHORITY,
        "environment_names": shared.EXPECTED_ENVIRONMENT_NAMES,
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["uv", "python3", "git", "perl"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/rshr_202_step_298_gate.py",
        "verifier_sha256": verifier_digest,
    }


def main() -> int:
    arguments = parse_arguments()
    if arguments.step != STEP:
        raise shared.GateError("step is outside the Lib gate authority")
    shared.validate_digest(arguments.source_revision, "source revision", 40)
    shared.validate_digest(arguments.source_tree, "source tree", 40)
    shared.validate_digest(arguments.execution_request_sha256, "execution request", 64)
    if arguments.check_id != CHECK_ID:
        raise shared.GateError("check identity differs")
    if arguments.candidate_digest != "none" or arguments.platform != "macos_aarch64":
        raise shared.GateError("candidate or platform scope differs")

    authority_bytes = shared.read_regular(AUTHORITY_PATH, 256 * 1024)
    try:
        authority = json.loads(authority_bytes)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise shared.GateError("gate authority is not canonical JSON") from error
    if shared.canonical(authority) + b"\n" != authority_bytes:
        raise shared.GateError("gate authority is not canonical JSON")
    verifier_digest = shared.sha256_bytes(shared.read_regular(Path(__file__).resolve()))
    contracts = authority.get("gate_command_contract")
    if (
        not isinstance(authority, dict)
        or set(authority) != {"schema", "step", "gate_command_contract"}
        or authority.get("schema") != "radroots.lib.rshr-202-step-298-gates.v1"
        or authority.get("step") != [STEP]
        or not isinstance(contracts, list)
        or len(contracts) != 1
        or contracts[0] != expected_contract(verifier_digest)
    ):
        raise shared.GateError("gate command authority differs from source bytes")

    require_source_state(arguments.source_revision, arguments.source_tree)
    run_step()
    contract = contracts[0]
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
        "command_contract_sha256": shared.sha256_bytes(shared.canonical(contract)),
        "verifier_sha256": verifier_digest,
        "execution_request": [
            {"platform": arguments.platform, "sha256": arguments.execution_request_sha256}
        ],
        "assertion_inventory_sha256": shared.sha256_bytes(shared.canonical(assertions)),
        "assertion": assertions,
        "result": "pass",
    }
    sys.stdout.buffer.write(shared.canonical(result) + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except shared.GateError as error:
        print(f"Lib RSHR-202 Step 298 gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
    except Exception:
        print("Lib RSHR-202 Step 298 gate failed safely", file=sys.stderr)
        raise SystemExit(1)
