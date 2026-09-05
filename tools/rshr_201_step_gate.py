#!/usr/bin/env python3
"""Emit the source-bound RSHR-201 gate result for Lib Step 292."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import selectors
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
AUTHORITY_PATH = ROOT / "contracts/rshr-201-step-gates.v1.json"
ORIGIN = "ssh://git@github.com/radrootslabs/lib.git"
BRANCH = "rshr/rcld-201"
STEP = 292
GATE_DEFINITION = (
    "timeout, orphan, output-cap, inherited-build-environment, "
    "loader-injection, and redaction vectors"
)
GATE_DIGEST = "f65be73a73a7ab8b0c8e02f0695ee5e910c73e0017572b82861c0c7f0d4fa454"
CHECK_ID = f"gate-01-{GATE_DIGEST}"
ASSERTION_ID = f"step_{STEP:03d}_gate_01_{GATE_DIGEST}"
MAX_SOURCE_BYTES = 64 * 1024 * 1024
MAX_STDOUT_BYTES = 16 * 1024 * 1024
MAX_STDERR_BYTES = 16 * 1024 * 1024
COMMAND_TIMEOUT_SECONDS = 3600.0
STREAM_CHUNK_BYTES = 64 * 1024
SELECT_INTERVAL_SECONDS = 0.1
TERM_GRACE_SECONDS = 0.25
KILL_GRACE_SECONDS = 0.5
EXPECTED_ENVIRONMENT_AUTHORITY = {
    "cache_policy_id": "rshr-200-step-287-cache-policy.v1",
    "cache_policy_sha256": (
        "3e81d178bce97b6c349dfbb00c68fd6f620ac00b1a1c8d37b12e9998f3c9eaaa"
    ),
    "cadence_policy_id": "rshr-200-step-287-cadence-policy.v1",
    "cadence_policy_sha256": (
        "d24903df8659ee3772297c84994911efe7d21cb8b988320ddc6ddce0431892a1"
    ),
    "isolation": "extbuild_host_constrained",
    "network": "disabled",
    "network_policy_id": "none",
    "network_policy_sha256": "none",
    "resource_policy_id": "rshr-200-step-287-resource-policy.v1",
    "resource_policy_sha256": (
        "05d3c7a89185d3c55678d97955193fce2ed92b1eee5af99083d77ea64c98d14e"
    ),
}
EXPECTED_ENVIRONMENT_NAMES = [
    "EXT_BUILD_CONFIG",
    "EXT_BUILD_MACHINE_CONFIG",
    "EXT_BUILD_ROOT",
    "HOME",
    "PATH",
    "RUSTUP_TOOLCHAIN",
    "TMPDIR",
]
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
    "tools/rshr_201_step_gate.py",
    "--step={step}",
    "--check-id={check_id}",
    "--source-revision={source_revision}",
    "--source-tree={source_tree}",
    "--candidate-digest={candidate_digest}",
    "--platform=macos_aarch64",
    "--execution-request-sha256={execution_request_sha256}",
]


class GateError(RuntimeError):
    """A fail-closed gate error whose message contains no protected data."""


class RedactedArgumentParser(argparse.ArgumentParser):
    """Reject malformed input without reflecting argument values."""

    def error(self, _message: str) -> None:
        raise GateError("arguments are invalid")


def canonical(value: object) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")


def sha256_bytes(contents: bytes) -> str:
    return hashlib.sha256(contents).hexdigest()


def read_regular(path: Path, maximum: int = MAX_SOURCE_BYTES) -> bytes:
    try:
        relative = path.relative_to(ROOT)
    except ValueError as error:
        raise GateError("source path is outside the repository") from error
    if maximum < 0 or path.is_symlink():
        raise GateError(f"source path is not a bounded regular file: {relative}")
    try:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    except OSError as error:
        raise GateError(f"source path cannot be opened safely: {relative}") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > maximum:
            raise GateError(f"source path is not a bounded regular file: {relative}")
        contents = bytearray()
        while len(contents) <= maximum:
            chunk = os.read(
                descriptor,
                min(STREAM_CHUNK_BYTES, maximum - len(contents) + 1),
            )
            if not chunk:
                break
            contents.extend(chunk)
        if len(contents) > maximum:
            raise GateError(f"source path exceeds its byte bound: {relative}")
        after = os.fstat(descriptor)
        if (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_size,
            metadata.st_mtime_ns,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
        ):
            raise GateError(f"source path changed during its bounded read: {relative}")
        return bytes(contents)
    except OSError as error:
        raise GateError(f"source path cannot be read safely: {relative}") from error
    finally:
        os.close(descriptor)


def _process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _signal_process_group(
    process: subprocess.Popen[bytes], process_signal: signal.Signals
) -> None:
    try:
        os.killpg(process.pid, process_signal)
    except ProcessLookupError:
        return
    except OSError:
        try:
            process.send_signal(process_signal)
        except OSError:
            return


def _bounded_reap(process: subprocess.Popen[bytes], timeout: float) -> None:
    if process.poll() is not None:
        return
    try:
        process.wait(timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        return


def _terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    """Apply bounded TERM/KILL cleanup to the isolated process group."""

    _signal_process_group(process, signal.SIGTERM)
    term_deadline = time.monotonic() + TERM_GRACE_SECONDS
    while _process_group_exists(process.pid) and time.monotonic() < term_deadline:
        _bounded_reap(
            process,
            min(SELECT_INTERVAL_SECONDS, max(0.001, term_deadline - time.monotonic())),
        )
        if process.poll() is not None:
            time.sleep(min(0.01, max(0.0, term_deadline - time.monotonic())))
    if _process_group_exists(process.pid):
        _signal_process_group(process, signal.SIGKILL)
    kill_deadline = time.monotonic() + KILL_GRACE_SECONDS
    while _process_group_exists(process.pid) and time.monotonic() < kill_deadline:
        _bounded_reap(
            process,
            min(SELECT_INTERVAL_SECONDS, max(0.001, kill_deadline - time.monotonic())),
        )
        if process.poll() is not None:
            time.sleep(min(0.01, max(0.0, kill_deadline - time.monotonic())))
    _bounded_reap(process, max(0.001, kill_deadline - time.monotonic()))
    if process.poll() is None or _process_group_exists(process.pid):
        raise GateError("bounded command cleanup did not reach a terminal state")


def run(
    arguments: list[str],
    environment: dict[str, str],
    *,
    label: str,
    maximum_stdout: int = MAX_STDOUT_BYTES,
    maximum_stderr: int = MAX_STDERR_BYTES,
    timeout_seconds: float = COMMAND_TIMEOUT_SECONDS,
) -> bytes:
    """Run one isolated command with live dual-stream caps and redacted errors."""

    if (
        not arguments
        or not label
        or maximum_stdout < 0
        or maximum_stderr < 0
        or timeout_seconds <= 0
        or any(
            not isinstance(argument, str)
            or not argument
            or "\x00" in argument
            or "\r" in argument
            or "\n" in argument
            for argument in arguments
        )
        or any(
            not isinstance(name, str)
            or not name
            or "=" in name
            or "\x00" in name
            or not isinstance(value, str)
            or "\x00" in value
            for name, value in environment.items()
        )
    ):
        raise GateError(f"{label} request is invalid")

    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    streams: list[object] = []
    try:
        try:
            process = subprocess.Popen(
                arguments,
                cwd=ROOT,
                env=dict(environment),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
                start_new_session=True,
            )
        except OSError as error:
            raise GateError(f"{label} could not start") from error

        assert process.stdout is not None
        assert process.stderr is not None
        streams.extend((process.stdout, process.stderr))
        destinations = {
            process.stdout: ("stdout", maximum_stdout, bytearray()),
            process.stderr: ("stderr", maximum_stderr, bytearray()),
        }
        for stream in destinations:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ)

        deadline = time.monotonic() + timeout_seconds
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GateError(f"{label} exceeded its deadline")
            if process.poll() is not None and _process_group_exists(process.pid):
                raise GateError(f"{label} left a running process group")
            events = selector.select(min(SELECT_INTERVAL_SECONDS, remaining))
            if not events:
                if process.poll() is not None and _process_group_exists(process.pid):
                    raise GateError(f"{label} left a running process group")
                continue
            for key, _mask in events:
                stream = key.fileobj
                stream_name, maximum, destination = destinations[stream]
                read_size = min(
                    STREAM_CHUNK_BYTES,
                    max(1, maximum - len(destination) + 1),
                )
                try:
                    chunk = os.read(stream.fileno(), read_size)
                except BlockingIOError:
                    continue
                except OSError as error:
                    raise GateError(f"{label} stream read failed") from error
                if not chunk:
                    selector.unregister(stream)
                    continue
                destination.extend(chunk)
                if len(destination) > maximum:
                    raise GateError(f"{label} {stream_name} exceeded its byte bound")

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise GateError(f"{label} exceeded its deadline")
        try:
            return_code = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            raise GateError(f"{label} exceeded its deadline") from error

        if _process_group_exists(process.pid):
            raise GateError(f"{label} left a running process group")
        if return_code != 0:
            raise GateError(f"{label} failed with exit status {return_code}")
        return bytes(destinations[process.stdout][2])
    except GateError:
        if process is not None:
            _terminate_process_group(process)
        raise
    except (OSError, subprocess.SubprocessError) as error:
        if process is not None:
            _terminate_process_group(process)
        raise GateError(f"{label} failed safely") from error
    except BaseException:
        if process is not None:
            _terminate_process_group(process)
        raise
    finally:
        selector.close()
        for stream in streams:
            try:
                stream.close()  # type: ignore[attr-defined]
            except OSError:
                pass


def git_environment() -> dict[str, str]:
    return {
        "GIT_ATTR_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_PAGER": "cat",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": os.environ.get("HOME", os.devnull),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.environ.get("PATH", os.defpath),
    }


def git_bytes(*arguments: str, maximum: int = MAX_SOURCE_BYTES) -> bytes:
    return run(
        ["git", "--no-pager", "--literal-pathspecs", *arguments],
        git_environment(),
        label="source identity inspection",
        maximum_stdout=maximum,
        maximum_stderr=0,
        timeout_seconds=60.0,
    )


def git(*arguments: str) -> str:
    try:
        return git_bytes(*arguments).decode("utf-8", "strict").strip()
    except UnicodeError as error:
        raise GateError("source identity is not UTF-8") from error


def require_source_state(source_revision: str, source_tree: str) -> None:
    if (
        git("rev-parse", "HEAD") != source_revision
        or git("rev-parse", "HEAD^{tree}") != source_tree
        or git("symbolic-ref", "--short", "HEAD") != BRANCH
        or git("remote", "get-url", "origin") != ORIGIN
        or git("rev-parse", f"refs/remotes/origin/{BRANCH}") != source_revision
        or git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all")
    ):
        raise GateError("Lib source is not clean and tracking-exact")

    tracked = git_bytes("ls-files", "-z").split(b"\0")
    if any(path == b".github" or path.startswith(b".github/") for path in tracked):
        raise GateError("forbidden .github surface is tracked")
    if os.path.lexists(ROOT / ".github"):
        raise GateError("forbidden .github surface is present")


def gate_environment() -> dict[str, str]:
    allowed = {
        "CARGO_HOME",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_TARGET_DIR",
        "EXT_BUILD_CONFIG",
        "EXT_BUILD_CARGO_TIMINGS_DIR",
        "EXT_BUILD_MACHINE_CONFIG",
        "EXT_BUILD_ROOT",
        "HOME",
        "LANG",
        "LC_ALL",
        "PATH",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "SCCACHE_DIR",
        "SDKROOT",
        "TMPDIR",
    }
    environment = {name: value for name, value in os.environ.items() if name in allowed}
    environment.update(
        {
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TERM_COLOR": "never",
            "LANG": "C",
            "LC_ALL": "C",
        }
    )
    if not environment.get("PATH"):
        environment["PATH"] = os.defpath
    extbuild_root_value = environment.get("EXT_BUILD_ROOT")
    routed_path_names = (
        "CARGO_TARGET_DIR",
        "EXT_BUILD_CARGO_TIMINGS_DIR",
        "SCCACHE_DIR",
    )
    routed_values = [environment[name] for name in routed_path_names if name in environment]
    if routed_values:
        if not extbuild_root_value:
            raise GateError("extbuild routing environment is incomplete")
        extbuild_root_input = Path(extbuild_root_value)
        if not extbuild_root_input.is_absolute():
            raise GateError("extbuild routing environment is invalid")
        extbuild_root = Path(os.path.abspath(extbuild_root_value))
        for value in routed_values:
            destination_input = Path(value)
            if not destination_input.is_absolute():
                raise GateError("extbuild routing environment is invalid")
            destination = Path(os.path.abspath(value))
            if (
                destination != extbuild_root and extbuild_root not in destination.parents
            ):
                raise GateError("extbuild routing environment is invalid")
    return environment


def run_step() -> None:
    environment = gate_environment()
    run(
        [
            "cargo",
            "+1.97.1",
            "test",
            "--offline",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--locked",
            "--bin",
            "xtask",
            "bounded_process",
            "--",
            "--test-threads=1",
        ],
        environment,
        label="bounded-process vector tests",
    )
    run(
        [
            "cargo",
            "+1.97.1",
            "run",
            "--offline",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--locked",
            "--",
            "bounded-process-self-test",
        ],
        environment,
        label="bounded-process self-test",
    )
    run(
        [
            "cargo",
            "+1.97.1",
            "test",
            "--offline",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--locked",
            "--test",
            "services_hardening_bounded_process_decision",
            "--",
            "--test-threads=1",
        ],
        environment,
        label="bounded-process decision contract",
    )
    if git_bytes("status", "--porcelain=v1", "-z", "--untracked-files=all"):
        raise GateError("verification changed the tracked or untracked source state")


def parse_arguments() -> argparse.Namespace:
    parser = RedactedArgumentParser(allow_abbrev=False)
    parser.add_argument("--step", type=int, required=True)
    parser.add_argument("--check-id")
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--source-tree", required=True)
    parser.add_argument("--candidate-digest")
    parser.add_argument("--platform", required=True)
    parser.add_argument("--execution-request-sha256", required=True)
    return parser.parse_args()


def validate_digest(value: str, label: str, length: int) -> None:
    if re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is None:
        raise GateError(f"{label} is not canonical")


def expected_contract(verifier_digest: str) -> dict[str, object]:
    return {
        "argv_template": EXPECTED_ARGV_TEMPLATE,
        "assertion_id": [ASSERTION_ID],
        "check_id": CHECK_ID,
        "environment_authority": EXPECTED_ENVIRONMENT_AUTHORITY,
        "environment_names": EXPECTED_ENVIRONMENT_NAMES,
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["uv", "python3", "git"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/rshr_201_step_gate.py",
        "verifier_sha256": verifier_digest,
    }


def main() -> int:
    arguments = parse_arguments()
    if arguments.step != STEP:
        raise GateError("step is outside the Lib gate authority")
    validate_digest(arguments.source_revision, "source revision", 40)
    validate_digest(arguments.source_tree, "source tree", 40)
    validate_digest(arguments.execution_request_sha256, "execution request", 64)
    if sha256_bytes(GATE_DEFINITION.encode("utf-8")) != GATE_DIGEST:
        raise GateError("compiled gate definition digest differs")
    if arguments.check_id != CHECK_ID:
        raise GateError("check identity differs")
    if arguments.candidate_digest != "none" or arguments.platform != "macos_aarch64":
        raise GateError("candidate or platform scope differs")

    authority_bytes = read_regular(AUTHORITY_PATH, 256 * 1024)
    try:
        authority = json.loads(authority_bytes)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise GateError("gate authority is not canonical JSON") from error
    if canonical(authority) + b"\n" != authority_bytes:
        raise GateError("gate authority is not canonical JSON")
    if (
        not isinstance(authority, dict)
        or set(authority) != {"schema", "step", "gate_command_contract"}
        or authority.get("schema") != "radroots.lib.rshr-201-step-gates.v1"
        or authority.get("step") != [STEP]
    ):
        raise GateError("gate authority step inventory differs")
    contracts = authority.get("gate_command_contract")
    if not isinstance(contracts, list) or len(contracts) != 1:
        raise GateError("gate command authority is absent or duplicated")

    verifier_digest = sha256_bytes(read_regular(Path(__file__).resolve()))
    contract = contracts[0]
    if contract != expected_contract(verifier_digest):
        raise GateError("gate command authority differs from source bytes")

    require_source_state(arguments.source_revision, arguments.source_tree)
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
        "command_contract_sha256": sha256_bytes(canonical(contract)),
        "verifier_sha256": verifier_digest,
        "execution_request": [
            {
                "platform": arguments.platform,
                "sha256": arguments.execution_request_sha256,
            }
        ],
        "assertion_inventory_sha256": sha256_bytes(canonical(assertions)),
        "assertion": assertions,
        "result": "pass",
    }
    sys.stdout.buffer.write(canonical(result) + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"Lib RSHR-201 gate failed: {error}", file=sys.stderr)
        raise SystemExit(1)
    except Exception:
        print("Lib RSHR-201 gate failed safely", file=sys.stderr)
        raise SystemExit(1)
