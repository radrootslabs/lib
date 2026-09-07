use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const STEP: u16 = 304;
const GATE_DIGEST: &str = "054db63c65f921ecba1c1bf416d680ee7ef880bdcf90cf6dc152677a03dd129a";
const NIX_SHA256: &str = "a59ab70f97f6d571642d13c7506aafec0a4275520d53daee2d8451be7c495cd1";
const NIX_VERSION_SHA256: &str = "6db806391ffaea4cdb08ade0031feac399c0cd08474b3bfde8cb33f88a36c8e1";
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const EXACT_SOURCES: &[(&str, &str)] = &[
    (
        "Cargo.lock",
        "a8edafae2d2b26465b2b99038ce55d80fa603481ebb5ebebbd810708ba4a894a",
    ),
    (
        "Cargo.toml",
        "322b975e60004df42de5b1b9460bf84255d2210e97f4f1cf1ede84a068b7519e",
    ),
    (
        "build/nix/service/fixture-service/Cargo.toml",
        "a1f0053f34606669c2e91c69a01c51d5db0e2af290ede801df6bf4ed45f14202",
    ),
    (
        "build/nix/service/fixture.nix",
        "1b18450ad14bfe74faf2372e2d0a396bdd5e79c257acf653ad5c45018daa573a",
    ),
    (
        "build/nix/service/oci.nix",
        "70458d0a988098b5343da00daad1d4d4d4a7cc95646928a938e28af3dd9ccd16",
    ),
    (
        "build/nix/service/package.nix",
        "cb6f32977bad84c3d8250d2ed806a8f8cc8b9b3d1d9d40f2b74db205a0abef00",
    ),
    (
        "contracts/architecture/decisions/services_hardening_artifact_admission.v1.json",
        "3b8d2bc57efb4dcce27248a6300c32ac84e39869cd73850cc4f468d911276a1d",
    ),
    (
        "contracts/architecture/decisions/services_hardening_release_artifacts.v3.json",
        "51979f32271b5a9851ad37a5da763fd8e2cba3fe8b4c36d35fac8b28e3fe4732",
    ),
    (
        "tools/xtask/Cargo.toml",
        "bf5442895085225a571bf8aa45b2316e732a5c9925911de26147a924466f8ba0",
    ),
    (
        "tools/xtask/src/artifact_admission.rs",
        "87b538d5ca80dfbc3de232ded40320cc1b0ec555fa48056af79eb365cc74ea44",
    ),
    (
        "tools/xtask/src/safe_artifact_io.rs",
        "3b361b22036ff8db5f5468e33341a7b2990d6f80aa6bdde4f791bd1320587e4d",
    ),
    (
        "tools/xtask/src/service_release_artifacts.rs",
        "866dc107182e35c4c1492936fddc70252405abcdf5a80f19fb161e07e361dfba",
    ),
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
    serde_json::to_vec(value).map_err(|_| "Step 304 JSON encoding failed".to_owned())
}
fn execute(command: &mut Command, label: &str) -> Result<Output, String> {
    let output = command
        .current_dir(root())
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|_| format!("{label} could not start"))?;
    if output.stdout.len() > MAX_OUTPUT_BYTES || output.stderr.len() > MAX_OUTPUT_BYTES {
        return Err(format!("{label} exceeded its output bound"));
    }
    Ok(output)
}
fn bounded(command: &mut Command, label: &str) -> Result<Output, String> {
    let output = execute(command, label)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!("{label} failed"))
    }
}
fn resolve_nix() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("RSHR_NIX_EXECUTABLE") {
        return fs::canonicalize(explicit)
            .map_err(|_| "Step 304 Nix client is unavailable".to_owned());
    }
    let path = env::var_os("PATH").ok_or_else(|| "Step 304 PATH is absent".to_owned())?;
    env::split_paths(&path)
        .map(|directory| directory.join("nix"))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .ok_or_else(|| "Step 304 Nix client is unavailable".to_owned())
}

fn require_nix() -> Result<(), String> {
    let executable = resolve_nix()?;
    if sha256(&fs::read(&executable).map_err(|_| "Step 304 Nix client is unreadable")?)
        != NIX_SHA256
    {
        return Err("Step 304 Nix client identity differs".to_owned());
    }
    let version = bounded(
        Command::new(&executable).arg("--version"),
        "Step 304 Nix version",
    )?;
    if sha256(&version.stdout) != NIX_VERSION_SHA256 {
        return Err("Step 304 Nix version differs".to_owned());
    }
    bounded(
        Command::new(&executable).args([
            "--offline",
            "flake",
            "check",
            "--all-systems",
            "--no-build",
            "--no-write-lock-file",
        ]),
        "Step 304 Nix evaluation",
    )?;
    bounded(
        Command::new(&executable).args([
            "--offline",
            "eval",
            "--raw",
            ".#checks.x86_64-linux.service-fixture-oci-image.drvPath",
            "--no-write-lock-file",
        ]),
        "Step 304 OCI derivation evaluation",
    )?;
    Ok(())
}

fn expected_contract(verifier_sha256: &str) -> Value {
    json!({
        "argv_template": [
            "cargo", "extbuild", "run", "--", "cargo", "run", "--offline", "--locked",
            "-q", "-p", "xtask", "--", "rshr-step-304-gate", "--step={step}",
            "--check-id={check_id}", "--source-revision={source_revision}",
            "--source-tree={source_tree}", "--candidate-digest={candidate_digest}",
            "--platform=macos_aarch64", "--execution-request-sha256={execution_request_sha256}"
        ],
        "assertion_id": [format!("step_304_gate_01_{GATE_DIGEST}")],
        "check_id": format!("gate-01-{GATE_DIGEST}"),
        "environment_authority": {
            "cache_policy_id": "rshr-200-step-287-cache-policy.v1",
            "cache_policy_sha256": "3e81d178bce97b6c349dfbb00c68fd6f620ac00b1a1c8d37b12e9998f3c9eaaa",
            "cadence_policy_id": "rshr-200-step-287-cadence-policy.v1",
            "cadence_policy_sha256": "d24903df8659ee3772297c84994911efe7d21cb8b988320ddc6ddce0431892a1",
            "isolation": "extbuild_host_constrained", "network": "disabled",
            "network_policy_id": "none", "network_policy_sha256": "none",
            "resource_policy_id": "rshr-200-step-287-resource-policy.v1",
            "resource_policy_sha256": "05d3c7a89185d3c55678d97955193fce2ed92b1eee5af99083d77ea64c98d14e"
        },
        "environment_names": ["EXT_BUILD_CONFIG", "EXT_BUILD_MACHINE_CONFIG", "EXT_BUILD_ROOT", "HOME", "PATH", "RUSTUP_TOOLCHAIN", "TMPDIR"],
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["nix", "rustc"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/xtask/src/rshr_202_step_304_gate.rs",
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
        return Err("Step 304 gate arguments differ".to_owned());
    }
    let root = root();
    if root.join(".github").exists() {
        return Err("forbidden .github surface is present".to_owned());
    }
    for (relative, expected) in EXACT_SOURCES {
        if sha256(
            &fs::read(root.join(relative)).map_err(|_| "Step 304 governed source is unreadable")?,
        ) != *expected
        {
            return Err("Step 304 governed source bytes differ".to_owned());
        }
    }
    let verifier_path = root.join("tools/xtask/src/rshr_202_step_304_gate.rs");
    let verifier_sha256 =
        sha256(&fs::read(verifier_path).map_err(|_| "Step 304 verifier is unreadable")?);
    let authority_bytes = fs::read(root.join("contracts/rshr-202-step-304-gates.v1.json"))
        .map_err(|_| "Step 304 gate authority is unreadable".to_owned())?;
    let authority: Value = serde_json::from_slice(&authority_bytes)
        .map_err(|_| "Step 304 gate authority is invalid".to_owned())?;
    let mut canonical_authority = canonical(&authority)?;
    canonical_authority.push(b'\n');
    let contracts = authority
        .get("gate_command_contract")
        .and_then(Value::as_array)
        .ok_or_else(|| "Step 304 gate contract is absent".to_owned())?;
    if authority_bytes != canonical_authority
        || authority.get("schema") != Some(&json!("radroots.lib.rshr-202-step-304-gates.v1"))
        || authority.get("step") != Some(&json!([STEP]))
        || contracts.as_slice() != [expected_contract(&verifier_sha256)]
    {
        return Err("Step 304 gate authority differs".to_owned());
    }
    bounded(
        Command::new("cargo").args(["+1.97.1", "fmt", "--all", "--", "--check"]),
        "Step 304 formatting",
    )?;
    bounded(
        Command::new("cargo").args(["+1.97.1", "check", "--offline", "--locked", "-p", "xtask"]),
        "Step 304 verifier check",
    )?;
    bounded(
        Command::new("cargo").args([
            "+1.97.1",
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "artifact_admission::tests",
        ]),
        "Step 304 adversarial admission tests",
    )?;
    bounded(
        Command::new("cargo").args([
            "+1.97.1",
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "service_release_artifacts::tests",
        ]),
        "Step 304 release integration tests",
    )?;
    bounded(
        Command::new("cargo").args([
            "+1.97.1",
            "clippy",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
        "Step 304 clippy",
    )?;
    bounded(
        Command::new("cargo").args([
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
        ]),
        "Step 304 contracts",
    )?;
    require_nix()?;
    let contract = &contracts[0];
    let assertion = json!([{ "id": format!("step_304_gate_01_{GATE_DIGEST}"), "result": "pass" }]);
    let result = json!({
        "schema": "radroots.services-hardening.rshr-200-step-check-result.v1", "step": STEP,
        "check_id": check_id, "gate_definition_sha256": GATE_DIGEST,
        "source_revision": arguments.source_revision, "source_tree": arguments.source_tree,
        "candidate_generation": 0, "candidate_digest": "none",
        "command_contract_sha256": sha256(&canonical(contract)?), "verifier_sha256": verifier_sha256,
        "execution_request": [{"platform": arguments.platform, "sha256": arguments.execution_request_sha256}],
        "assertion_inventory_sha256": sha256(&canonical(&assertion)?), "assertion": assertion, "result": "pass"
    });
    let mut bytes = canonical(&result)?;
    bytes.push(b'\n');
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
        .map_err(|_| "Step 304 result write failed".to_owned())
}
