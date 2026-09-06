use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const STEP: u16 = 299;
const GATE_DIGEST: &str = "aaf288b5bf9102722f61bd8160188dc80bf84a62f48ddaef9023a4417654d8ba";
const NIX_SHA256: &str = "a59ab70f97f6d571642d13c7506aafec0a4275520d53daee2d8451be7c495cd1";
const NIX_VERSION_SHA256: &str = "6db806391ffaea4cdb08ade0031feac399c0cd08474b3bfde8cb33f88a36c8e1";
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

const EXACT_SOURCES: &[(&str, &str)] = &[
    (
        "flake.nix",
        "de3d2258f2c48ae8b1493b676ab3d1466cd5f1a59015f65283efbfe12447e501",
    ),
    (
        "build/nix/library.nix",
        "ff425fe6361bad78c105e36d3e900950ce22b0034e26add55e5ed287aa868765",
    ),
    (
        "build/nix/service/native-inputs.nix",
        "09aed688d6ad6c5c824aa771b871cc0c4857d8378d1330221c8d5a05509d9391",
    ),
    (
        "build/nix/service/nixos-module.nix",
        "20b42dd6972c59fcaaf5a282219644d83822ea03a2805bfe8e6e9b8f7b628622",
    ),
    (
        "build/nix/service/fixture.nix",
        "f3fa0cbef4f6a86feee9b2319165d97c51526236b1426ffa83ffb435ed90cd97",
    ),
    (
        "build/nix/service/systems.nix",
        "d16e21827022a2315234f4c5e4b485017a36ecd90a5559b01d23331cdd505e46",
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
    serde_json::to_vec(value).map_err(|_| "Step 299 JSON encoding failed".to_owned())
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
    if !output.status.success() {
        return Err(format!("{label} failed"));
    }
    Ok(output)
}

fn rejected(command: &mut Command, label: &str) -> Result<(), String> {
    if execute(command, label)?.status.success() {
        return Err(format!("{label} unexpectedly succeeded"));
    }
    Ok(())
}

fn resolve_nix() -> Result<PathBuf, String> {
    if let Some(explicit) = env::var_os("RSHR_NIX_EXECUTABLE") {
        return fs::canonicalize(explicit)
            .map_err(|_| "Step 299 Nix client is unavailable".to_owned());
    }
    let path = env::var_os("PATH").ok_or_else(|| "Step 299 PATH is absent".to_owned())?;
    env::split_paths(&path)
        .map(|directory| directory.join("nix"))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .ok_or_else(|| "Step 299 Nix client is unavailable".to_owned())
}

fn object_keys(value: &Value, label: &str) -> Result<Vec<String>, String> {
    let mut keys = value
        .as_object()
        .ok_or_else(|| format!("Step 299 {label} is not an object"))?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort_unstable();
    Ok(keys)
}

fn require_outputs(nix: &Path) -> Result<(), String> {
    let show = bounded(
        Command::new(nix).args([
            "--offline",
            "flake",
            "show",
            "--json",
            "--all-systems",
            "--no-write-lock-file",
        ]),
        "Step 299 Nix output inventory",
    )?;
    let inventory: Value = serde_json::from_slice(&show.stdout)
        .map_err(|_| "Step 299 Nix output inventory is invalid".to_owned())?;
    let systems = ["aarch64-darwin", "x86_64-linux"];
    for family in ["apps", "checks", "devShells", "formatter", "packages"] {
        if object_keys(&inventory[family], family)? != systems {
            return Err(format!("Step 299 {family} systems differ"));
        }
    }
    if inventory.pointer("/overlays/default/type") != Some(&json!("nixpkgs-overlay")) {
        return Err("Step 299 default overlay is absent".to_owned());
    }
    for system in systems {
        let packages = &inventory["packages"][system];
        if object_keys(packages, "packages")? != ["default", "xtask"]
            || packages["default"]["name"] != "radroots-lib-release-bundle-0.1.0-alpha"
        {
            return Err("Step 299 package inventory differs".to_owned());
        }
        if inventory["apps"][system]["default"]["description"]
            != "Inspect the installed Radroots Lib release bundle"
            || inventory["devShells"][system].get("default").is_none()
            || inventory["checks"][system].get("release-bundle").is_none()
        {
            return Err("Step 299 production output is absent".to_owned());
        }
        for family in ["apps", "devShells", "packages"] {
            if object_keys(&inventory[family][system], family)?
                .iter()
                .any(|name| name.contains("fixture"))
            {
                return Err("Step 299 fixture escaped its test-only surface".to_owned());
            }
        }
        let fixture_checks = object_keys(&inventory["checks"][system], "checks")?
            .into_iter()
            .filter(|name| name.contains("fixture"))
            .collect::<Vec<_>>();
        if fixture_checks.is_empty()
            || fixture_checks
                .iter()
                .any(|name| !name.starts_with("service-fixture-"))
        {
            return Err("Step 299 fixture check names differ".to_owned());
        }
    }

    let supported = bounded(
        Command::new(nix).args(["--offline", "eval", "--json", ".#lib.supportedSystems"]),
        "Step 299 shared-helper systems",
    )?;
    if supported.stdout != b"[\"aarch64-darwin\",\"x86_64-linux\"]\n" {
        return Err("Step 299 shared-helper systems differ".to_owned());
    }
    let helper_type = bounded(
        Command::new(nix).args([
            "--offline",
            "eval",
            "--raw",
            "--apply",
            "f: builtins.typeOf f",
            ".#lib.mkServiceHelpers",
        ]),
        "Step 299 shared-helper export",
    )?;
    if helper_type.stdout != b"lambda" {
        return Err("Step 299 shared-helper export differs".to_owned());
    }

    for system in ["x86_64-darwin", "aarch64-linux", "x86_64-windows"] {
        rejected(
            Command::new(nix).args([
                "--offline",
                "eval",
                "--raw",
                &format!(".#packages.{system}.default.name"),
            ]),
            "Step 299 excluded-system evaluation",
        )?;
    }
    Ok(())
}

fn require_nix() -> Result<(), String> {
    let executable = resolve_nix()?;
    if sha256(&fs::read(&executable).map_err(|_| "Step 299 Nix client is unreadable")?)
        != NIX_SHA256
    {
        return Err("Step 299 Nix client identity differs".to_owned());
    }
    let version = bounded(
        Command::new(&executable).arg("--version"),
        "Step 299 Nix version",
    )?;
    if sha256(&version.stdout) != NIX_VERSION_SHA256 {
        return Err("Step 299 Nix version differs".to_owned());
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
        "Step 299 Nix evaluation",
    )?;
    require_outputs(&executable)
}

fn expected_contract(verifier_sha256: &str) -> Value {
    json!({
        "argv_template": [
            "cargo", "extbuild", "run", "--", "cargo", "run", "--offline", "--locked",
            "-q", "-p", "xtask", "--", "rshr-step-299-gate", "--step={step}",
            "--check-id={check_id}", "--source-revision={source_revision}",
            "--source-tree={source_tree}", "--candidate-digest={candidate_digest}",
            "--platform=macos_aarch64",
            "--execution-request-sha256={execution_request_sha256}"
        ],
        "assertion_id": [format!("step_299_gate_01_{GATE_DIGEST}")],
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
        "verifier_path": "tools/xtask/src/rshr_202_step_299_gate.rs",
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
        return Err("Step 299 gate arguments differ".to_owned());
    }
    let root = root();
    if root.join(".github").exists() {
        return Err("forbidden .github surface is present".to_owned());
    }
    for (relative, expected) in EXACT_SOURCES {
        let bytes = fs::read(root.join(relative))
            .map_err(|_| "Step 299 governed source is unreadable".to_owned())?;
        if sha256(&bytes) != *expected {
            return Err("Step 299 governed source bytes differ".to_owned());
        }
    }

    let verifier_path = root.join("tools/xtask/src/rshr_202_step_299_gate.rs");
    let verifier_sha256 =
        sha256(&fs::read(verifier_path).map_err(|_| "Step 299 verifier is unreadable".to_owned())?);
    let authority_path = root.join("contracts/rshr-202-step-299-gates.v1.json");
    let authority_bytes =
        fs::read(authority_path).map_err(|_| "Step 299 gate authority is unreadable".to_owned())?;
    let authority: Value = serde_json::from_slice(&authority_bytes)
        .map_err(|_| "Step 299 gate authority is invalid".to_owned())?;
    let mut canonical_authority = canonical(&authority)?;
    canonical_authority.push(b'\n');
    let contracts = authority
        .get("gate_command_contract")
        .and_then(Value::as_array)
        .ok_or_else(|| "Step 299 gate contract is absent".to_owned())?;
    if authority_bytes != canonical_authority
        || authority.get("schema")
            != Some(&Value::String(
                "radroots.lib.rshr-202-step-299-gates.v1".to_owned(),
            ))
        || authority.get("step") != Some(&json!([STEP]))
        || contracts.as_slice() != [expected_contract(&verifier_sha256)]
    {
        return Err("Step 299 gate authority differs".to_owned());
    }

    bounded(
        Command::new("cargo").args(["+1.97.1", "fmt", "--all", "--", "--check"]),
        "Step 299 formatting",
    )?;
    bounded(
        Command::new("cargo").args(["+1.97.1", "check", "--offline", "--locked", "-p", "xtask"]),
        "Step 299 verifier check",
    )?;
    require_nix()?;

    let contract = &contracts[0];
    let assertion = json!([{
        "id": format!("step_299_gate_01_{GATE_DIGEST}"),
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
        .map_err(|_| "Step 299 result write failed".to_owned())
}
