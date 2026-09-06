use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const STEP: u16 = 298;
const GATE_DIGEST: &str = "14c62391f40dcf5a2e166bac481c9eb8f50052ee65403ddbd63df5cc82ec6843";
const NIX_SHA256: &str = "a59ab70f97f6d571642d13c7506aafec0a4275520d53daee2d8451be7c495cd1";
const NIX_VERSION_SHA256: &str = "6db806391ffaea4cdb08ade0031feac399c0cd08474b3bfde8cb33f88a36c8e1";
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

const EXACT_SOURCES: &[(&str, &str)] = &[
    (
        "contracts/architecture/decisions/services_hardening_source_lock.v3.json",
        "3bc32c8ca2cecb06c8f8239ab1fe1fcfba93fe3ef0d60e9b078390347d08f817",
    ),
    (
        "contracts/release/lib-artifact-contract.v3.json",
        "bc352a132dd4c0e6f1d2ae7449998833efe1fdda2ab851e512bc9241c49edbf0",
    ),
    (
        "contracts/architecture/decisions/services_hardening_build_qualification.v3.json",
        "4f1bf59e6411c28c9b202c81ed9455c3446525fc39c8e96276a48e4223de1394",
    ),
    (
        "build/nix/service/systems.nix",
        "d16e21827022a2315234f4c5e4b485017a36ecd90a5559b01d23331cdd505e46",
    ),
    (
        "build/nix/service/fixture.nix",
        "d9f4ec24762b2cadb4aed45518f81e53308d4e1bf7ff2708aec74a1485269402",
    ),
    (
        "build/nix/service/oci.nix",
        "9111ce51465bbe45944b718e5e2477b43a08f8f7d9e29f43940a67e86e75daf1",
    ),
    (
        "contracts/releases/target_matrix.toml",
        "28583b0a163e51468d9688b463902ec2cd22b59c99061630596839baf9396527",
    ),
    (
        "tools/xtask/src/service_build_qualification.rs",
        "20dbae0f446bdd95e99f84d1c27ef4dec422eae8035ead5876adba047db20a9f",
    ),
    (
        "tools/xtask/src/target_qualification.rs",
        "0e5b9506c70f5175edeae7cf9b7fb0f55a0cb6abf465a3f708d4234b2069c585",
    ),
];

const BUILD_TESTS: &[&str] = &[
    "service_build_qualification::tests::checked_in_contract_and_fixture_are_exact",
    "service_build_qualification::tests::contract_inventory_is_literal_and_complete",
    "service_build_qualification::tests::contract_rejects_every_independent_governed_field_drift",
    "service_build_qualification::tests::errors_are_fixed_and_source_free",
    "service_build_qualification::tests::fixture_rejects_every_identity_and_lockfile_drift",
    "service_build_qualification::tests::fixture_rejects_every_independent_metadata_drift",
];

const TARGET_TESTS: &[&str] = &[
    "target_qualification::tests::current_contract_selects_exact_toolchains_targets_and_packages",
    "target_qualification::tests::unsupported_production_targets_are_rejected",
];

pub(crate) struct Arguments {
    pub(crate) step: u16,
    pub(crate) check_id: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree: String,
    pub(crate) candidate_digest: String,
    pub(crate) platform: String,
    pub(crate) execution_request_sha256: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask must remain under tools/xtask")
        .to_path_buf()
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn canonical(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|_| "Step 298 JSON encoding failed".to_owned())
}

fn bounded(command: &mut Command, label: &str) -> Result<Output, String> {
    let output = command
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|_| format!("{label} could not start"))?;
    if output.stdout.len() > MAX_OUTPUT_BYTES || output.stderr.len() > MAX_OUTPUT_BYTES {
        return Err(format!("{label} exceeded its output bound"));
    }
    if !output.status.success() {
        return Err(format!("{label} failed"));
    }
    Ok(output)
}

fn cargo(arguments: &[&str], label: &str) -> Result<Output, String> {
    bounded(Command::new("cargo").args(arguments), label)
}

fn require_test_lane(filter: &str, expected: &[&str]) -> Result<(), String> {
    let listed = cargo(
        &[
            "+1.97.1",
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            filter,
            "--",
            "--list",
            "--format=terse",
        ],
        "Step 298 test inventory",
    )?;
    let text = std::str::from_utf8(&listed.stdout)
        .map_err(|_| "Step 298 test inventory is not UTF-8".to_owned())?;
    let mut observed = text
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect::<Vec<_>>();
    observed.sort_unstable();
    let mut required = expected.to_vec();
    required.sort_unstable();
    if observed != required {
        return Err("Step 298 test inventory differs".to_owned());
    }
    cargo(
        &[
            "+1.97.1",
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            filter,
            "--",
            "--test-threads=1",
        ],
        "Step 298 mutation lane",
    )?;
    Ok(())
}

fn resolve_nix() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("RSHR_NIX_EXECUTABLE") {
        return fs::canonicalize(explicit)
            .map_err(|_| "Step 298 Nix client is unavailable".to_owned());
    }
    let path = env::var_os("PATH").ok_or_else(|| "Step 298 PATH is absent".to_owned())?;
    env::split_paths(&path)
        .map(|directory| directory.join("nix"))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .ok_or_else(|| "Step 298 Nix client is unavailable".to_owned())
}

fn require_nix() -> Result<(), String> {
    let executable = resolve_nix()?;
    let bytes = fs::read(&executable).map_err(|_| "Step 298 Nix client is unreadable")?;
    if sha256(&bytes) != NIX_SHA256 {
        return Err("Step 298 Nix client identity differs".to_owned());
    }
    let version = bounded(
        Command::new(&executable).arg("--version"),
        "Step 298 Nix version",
    )?;
    if sha256(&version.stdout) != NIX_VERSION_SHA256 {
        return Err("Step 298 Nix version differs".to_owned());
    }
    let systems = bounded(
        Command::new(&executable).args([
            "--offline",
            "eval",
            "--json",
            "--file",
            "build/nix/service/systems.nix",
        ]),
        "Step 298 Nix systems",
    )?;
    if systems.stdout != b"[\"aarch64-darwin\",\"x86_64-linux\"]\n" {
        return Err("Step 298 Nix systems differ".to_owned());
    }
    bounded(
        Command::new(&executable).args([
            "--offline",
            "flake",
            "check",
            "--no-build",
            "--no-write-lock-file",
        ]),
        "Step 298 Nix flake evaluation",
    )?;
    Ok(())
}

fn expected_contract(verifier_sha256: &str) -> Value {
    json!({
        "argv_template": [
            "cargo", "extbuild", "run", "--", "cargo", "run", "--offline", "--locked",
            "-q", "-p", "xtask", "--", "rshr-step-298-gate", "--step={step}",
            "--check-id={check_id}", "--source-revision={source_revision}",
            "--source-tree={source_tree}", "--candidate-digest={candidate_digest}",
            "--platform=macos_aarch64",
            "--execution-request-sha256={execution_request_sha256}"
        ],
        "assertion_id": [format!("step_298_gate_01_{GATE_DIGEST}")],
        "check_id": format!("gate-01-{GATE_DIGEST}"),
        "environment_authority": {
            "cache_policy_id": "rshr-200-step-287-cache-policy.v1",
            "cache_policy_sha256": "3e81d178bce97b6c349dfbb00c68fd6f620ac00b1a1c8d37b12e9998f3c9eaaa",
            "cadence_policy_id": "rshr-200-step-287-cadence-policy.v1",
            "cadence_policy_sha256": "d24903df8659ee3772297c84994911efe7d21cb8b988320ddc6ddce0431892a1",
            "isolation": "extbuild_host_constrained",
            "network": "disabled",
            "network_policy_id": "none",
            "network_policy_sha256": "none",
            "resource_policy_id": "rshr-200-step-287-resource-policy.v1",
            "resource_policy_sha256": "05d3c7a89185d3c55678d97955193fce2ed92b1eee5af99083d77ea64c98d14e"
        },
        "environment_names": [
            "EXT_BUILD_CONFIG", "EXT_BUILD_MACHINE_CONFIG", "EXT_BUILD_ROOT", "HOME", "PATH",
            "RUSTUP_TOOLCHAIN", "TMPDIR"
        ],
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["rustc"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/xtask/src/rshr_202_step_298_gate.rs",
        "verifier_sha256": verifier_sha256
    })
}

pub(crate) fn run(arguments: Arguments) -> Result<(), String> {
    let check_id = format!("gate-01-{GATE_DIGEST}");
    if arguments.step != STEP
        || arguments.check_id != check_id
        || arguments.candidate_digest != "none"
        || arguments.platform != "macos_aarch64"
        || arguments.source_revision.len() != 40
        || arguments.source_tree.len() != 40
        || arguments.execution_request_sha256.len() != 64
        || !arguments
            .source_revision
            .bytes()
            .chain(arguments.source_tree.bytes())
            .chain(arguments.execution_request_sha256.bytes())
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("Step 298 gate arguments differ".to_owned());
    }
    let root = root();
    if root.join(".github").exists() {
        return Err("forbidden .github surface is present".to_owned());
    }
    for (relative, expected) in EXACT_SOURCES {
        let bytes = fs::read(root.join(relative))
            .map_err(|_| "Step 298 governed source is unreadable".to_owned())?;
        if sha256(&bytes) != *expected {
            return Err("Step 298 governed source bytes differ".to_owned());
        }
    }

    let verifier_path = root.join("tools/xtask/src/rshr_202_step_298_gate.rs");
    let verifier_sha256 =
        sha256(&fs::read(verifier_path).map_err(|_| "Step 298 verifier is unreadable".to_owned())?);
    let authority_path = root.join("contracts/rshr-202-step-298-gates.v1.json");
    let authority_bytes =
        fs::read(authority_path).map_err(|_| "Step 298 gate authority is unreadable".to_owned())?;
    let authority: Value = serde_json::from_slice(&authority_bytes)
        .map_err(|_| "Step 298 gate authority is invalid".to_owned())?;
    let mut canonical_authority = canonical(&authority)?;
    canonical_authority.push(b'\n');
    let contracts = authority
        .get("gate_command_contract")
        .and_then(Value::as_array)
        .ok_or_else(|| "Step 298 gate contract is absent".to_owned())?;
    if authority_bytes != canonical_authority
        || authority.get("schema")
            != Some(&Value::String(
                "radroots.lib.rshr-202-step-298-gates.v1".to_owned(),
            ))
        || authority.get("step") != Some(&json!([STEP]))
        || contracts.as_slice() != [expected_contract(&verifier_sha256)]
    {
        return Err("Step 298 gate authority differs".to_owned());
    }

    cargo(
        &["+1.97.1", "fmt", "--all", "--", "--check"],
        "Step 298 formatting",
    )?;
    require_test_lane("service_build_qualification::tests", BUILD_TESTS)?;
    require_test_lane("target_qualification::tests", TARGET_TESTS)?;
    cargo(
        &[
            "+1.97.1",
            "run",
            "--offline",
            "--locked",
            "-q",
            "-p",
            "xtask",
            "--",
            "contract",
            "validate",
        ],
        "Step 298 contract validation",
    )?;
    require_nix()?;

    let contract = &contracts[0];
    let assertion = json!([{
        "id": format!("step_298_gate_01_{GATE_DIGEST}"),
        "result": "pass"
    }]);
    let result = json!({
        "schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "step": STEP,
        "check_id": check_id,
        "gate_definition_sha256": GATE_DIGEST,
        "source_revision": arguments.source_revision,
        "source_tree": arguments.source_tree,
        "candidate_generation": 0,
        "candidate_digest": "none",
        "command_contract_sha256": sha256(&canonical(contract)?),
        "verifier_sha256": verifier_sha256,
        "execution_request": [{
            "platform": arguments.platform,
            "sha256": arguments.execution_request_sha256
        }],
        "assertion_inventory_sha256": sha256(&canonical(&assertion)?),
        "assertion": assertion,
        "result": "pass"
    });
    let mut bytes = canonical(&result)?;
    bytes.push(b'\n');
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
        .map_err(|_| "Step 298 result write failed".to_owned())
}
