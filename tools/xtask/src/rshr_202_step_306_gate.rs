use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const STEP: u16 = 306;
const GATE_DIGEST: &str = "9d88a1ac22999cbb6ec8d2537a0c94614841cae1df303dc49dde045745eb7bf2";
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
        "contracts/architecture/decisions/services_hardening_repro_install.v1.json",
        "69844a67fdc35345fd8d44f95516d238f868a14dcee3d5338aabeb17ca006903",
    ),
    (
        "tools/xtask/Cargo.toml",
        "bf5442895085225a571bf8aa45b2316e732a5c9925911de26147a924466f8ba0",
    ),
    (
        "tools/xtask/src/bounded_process.rs",
        "56b8de0c3c34b7481f0f8c792e37dfb0d9fc12a5b990287c979e89fd6989202e",
    ),
    (
        "tools/xtask/src/main.rs",
        "1e2101e74985fe6a37762d4d32002221d659b7d5307f3788b6171683b46bf48a",
    ),
    (
        "tools/xtask/src/service_repro_install.rs",
        "55fba278db8b4eec333181a307c70036a0b0fde077fc7254af386ed940dc117e",
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
    serde_json::to_vec(value).map_err(|_| "Step 306 JSON encoding failed".to_owned())
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
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!("{label} failed"))
    }
}

fn expected_contract(verifier_sha256: &str) -> Value {
    json!({
        "argv_template": [
            "cargo", "extbuild", "run", "--", "cargo", "run", "--offline", "--locked",
            "-q", "-p", "xtask", "--", "rshr-step-306-gate", "--step={step}",
            "--check-id={check_id}", "--source-revision={source_revision}",
            "--source-tree={source_tree}", "--candidate-digest={candidate_digest}",
            "--platform=macos_aarch64", "--execution-request-sha256={execution_request_sha256}"
        ],
        "assertion_id": [format!("step_306_gate_01_{GATE_DIGEST}")],
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
        "environment_names": ["EXT_BUILD_CONFIG", "EXT_BUILD_MACHINE_CONFIG", "EXT_BUILD_ROOT", "HOME", "PATH", "RUSTUP_TOOLCHAIN", "TMPDIR"],
        "gate_definition_sha256": GATE_DIGEST,
        "required_platforms": ["macos_aarch64"],
        "required_tools": ["git", "rustc"],
        "result_schema": "radroots.services-hardening.rshr-200-step-check-result.v1",
        "schema": "radroots.services-hardening.rshr-200-step-check-command.v1",
        "step": STEP,
        "verifier_path": "tools/xtask/src/rshr_202_step_306_gate.rs",
        "verifier_sha256": verifier_sha256
    })
}

fn validate_arguments(arguments: &Arguments) -> Result<String, String> {
    let check_id = format!("gate-01-{GATE_DIGEST}");
    if arguments.step != STEP
        || arguments.check_id != check_id
        || arguments.candidate_digest != "none"
        || arguments.platform != "macos_aarch64"
        || !valid_hex(&arguments.source_revision, 40)
        || !valid_hex(&arguments.source_tree, 40)
        || !valid_hex(&arguments.execution_request_sha256, 64)
    {
        return Err("Step 306 gate arguments differ".to_owned());
    }
    Ok(check_id)
}

pub(crate) fn run(arguments: Arguments) -> Result<(), String> {
    let check_id = validate_arguments(&arguments)?;
    let root = root();
    if root.join(".github").exists() {
        return Err("forbidden .github surface is present".to_owned());
    }
    for (relative, expected) in EXACT_SOURCES {
        let observed = sha256(
            &fs::read(root.join(relative))
                .map_err(|_| "Step 306 governed source is unreadable".to_owned())?,
        );
        if observed != *expected {
            return Err(format!("Step 306 governed source bytes differ: {relative}"));
        }
    }
    let verifier_path = root.join("tools/xtask/src/rshr_202_step_306_gate.rs");
    let verifier_sha256 =
        sha256(&fs::read(verifier_path).map_err(|_| "Step 306 verifier is unreadable".to_owned())?);
    let authority_bytes = fs::read(root.join("contracts/rshr-202-step-306-gates.v1.json"))
        .map_err(|_| "Step 306 gate authority is unreadable".to_owned())?;
    let contract = validate_authority(&authority_bytes, &verifier_sha256)?;

    for (arguments, label) in [
        (
            vec!["+1.97.1", "fmt", "--all", "--", "--check"],
            "Step 306 formatting",
        ),
        (
            vec!["+1.97.1", "check", "--offline", "--locked", "-p", "xtask"],
            "Step 306 verifier check",
        ),
        (
            vec![
                "+1.97.1",
                "test",
                "--offline",
                "--locked",
                "-p",
                "xtask",
                "service_repro_install::tests",
            ],
            "Step 306 reproducibility and install tests",
        ),
        (
            vec![
                "+1.97.1",
                "test",
                "--offline",
                "--locked",
                "-p",
                "xtask",
                "tests::typed_build_control_cli_requires_explicit_modes_and_known_values",
            ],
            "Step 306 typed command test",
        ),
    ] {
        bounded(Command::new("cargo").args(arguments), label)?;
    }
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
        "Step 306 clippy",
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
        "Step 306 contracts",
    )?;

    let bytes = result_bytes(&arguments, &check_id, &verifier_sha256, &contract)?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
        .map_err(|_| "Step 306 result write failed".to_owned())
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_authority(authority_bytes: &[u8], verifier_sha256: &str) -> Result<Value, String> {
    let authority: Value = serde_json::from_slice(authority_bytes)
        .map_err(|_| "Step 306 gate authority is invalid".to_owned())?;
    let mut canonical_authority = canonical(&authority)?;
    canonical_authority.push(b'\n');
    let contracts = authority
        .get("gate_command_contract")
        .and_then(Value::as_array)
        .ok_or_else(|| "Step 306 gate contract is absent".to_owned())?;
    if authority_bytes != canonical_authority
        || authority.get("schema") != Some(&json!("radroots.lib.rshr-202-step-306-gates.v1"))
        || authority.get("step") != Some(&json!([STEP]))
        || contracts.as_slice() != [expected_contract(verifier_sha256)]
    {
        return Err("Step 306 gate authority differs".to_owned());
    }
    Ok(contracts[0].clone())
}

fn result_bytes(
    arguments: &Arguments,
    check_id: &str,
    verifier_sha256: &str,
    contract: &Value,
) -> Result<Vec<u8>, String> {
    let assertion = json!([{ "id": format!("step_306_gate_01_{GATE_DIGEST}"), "result": "pass" }]);
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
        "execution_request": [{"platform": arguments.platform, "sha256": arguments.execution_request_sha256}],
        "assertion_inventory_sha256": sha256(&canonical(&assertion)?),
        "assertion": assertion,
        "result": "pass"
    });
    let mut bytes = canonical(&result)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_capture_preserves_status_streams_and_enforces_both_output_limits() {
        let output = bounded(
            Command::new("/bin/sh").args(["-c", "printf output; printf diagnostic >&2"]),
            "fixture",
        )
        .unwrap();
        assert_eq!(output.stdout, b"output");
        assert_eq!(output.stderr, b"diagnostic");
        assert_eq!(
            bounded(Command::new("/bin/sh").args(["-c", "exit 7"]), "fixture").unwrap_err(),
            "fixture failed"
        );
        let missing = tempfile::TempDir::new().unwrap();
        assert_eq!(
            bounded(&mut Command::new(missing.path().join("absent")), "fixture").unwrap_err(),
            "fixture could not start"
        );
        for script in ["head -c \"$1\" /dev/zero", "head -c \"$1\" /dev/zero >&2"] {
            let maximum = MAX_OUTPUT_BYTES.to_string();
            let output = bounded(
                Command::new("/bin/sh").args(["-c", script, "fixture", &maximum]),
                "fixture",
            )
            .unwrap();
            assert_eq!(output.stdout.len() + output.stderr.len(), MAX_OUTPUT_BYTES);
            let oversized = (MAX_OUTPUT_BYTES + 1).to_string();
            assert_eq!(
                bounded(
                    Command::new("/bin/sh").args(["-c", script, "fixture", &oversized]),
                    "fixture"
                )
                .unwrap_err(),
                "fixture exceeded its output bound"
            );
        }
    }

    fn arguments() -> Arguments {
        Arguments {
            step: STEP,
            check_id: format!("gate-01-{GATE_DIGEST}"),
            source_revision: "a".repeat(40),
            source_tree: "0".repeat(40),
            candidate_digest: "none".into(),
            platform: "macos_aarch64".into(),
            execution_request_sha256: "1".repeat(64),
        }
    }

    #[test]
    fn invalid_gate_arguments_are_rejected_before_external_work() {
        assert_eq!(
            validate_arguments(&arguments()).unwrap(),
            format!("gate-01-{GATE_DIGEST}")
        );
        for field in 0..8 {
            let mut invalid = arguments();
            match field {
                0 => invalid.step = 0,
                1 => invalid.check_id.clear(),
                2 => invalid.candidate_digest = "unbound".into(),
                3 => invalid.platform = "linux".into(),
                4 => invalid.source_revision.clear(),
                5 => invalid.source_tree.clear(),
                6 => invalid.execution_request_sha256.clear(),
                _ => invalid.source_revision = "A".repeat(40),
            }
            assert_eq!(run(invalid).unwrap_err(), "Step 306 gate arguments differ");
        }
        let mut invalid = arguments();
        invalid.source_tree = "g".repeat(40);
        assert!(validate_arguments(&invalid).is_err());
    }

    #[test]
    fn authority_requires_canonical_bytes_and_exact_retained_bindings() {
        let raw = include_bytes!("../../../contracts/rshr-202-step-306-gates.v1.json");
        let authority: Value = serde_json::from_slice(raw).unwrap();
        let verifier = authority["gate_command_contract"][0]["verifier_sha256"]
            .as_str()
            .unwrap();
        let contract = validate_authority(raw, verifier).unwrap();
        assert!(validate_authority(raw, &"f".repeat(64)).is_err());
        assert!(validate_authority(b"invalid", verifier).is_err());
        assert!(validate_authority(b"{}\n", verifier).is_err());
        assert!(
            validate_authority(&serde_json::to_vec_pretty(&authority).unwrap(), verifier).is_err()
        );
        for (pointer, value) in [
            ("/schema", json!("other")),
            ("/step", json!([0])),
            ("/gate_command_contract", json!([])),
        ] {
            let mut changed = authority.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            let mut bytes = canonical(&changed).unwrap();
            bytes.push(b'\n');
            assert!(validate_authority(&bytes, verifier).is_err());
        }
        // Encoding fixtures is not a historical gate execution or qualification.
        let args = arguments();
        let bytes = result_bytes(&args, &args.check_id, verifier, &contract).unwrap();
        let result: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(bytes.ends_with(b"\n"));
        assert_eq!(result["source_revision"], args.source_revision);
        assert_eq!(result["source_tree"], args.source_tree);
        assert_eq!(
            result["execution_request"][0]["sha256"],
            args.execution_request_sha256
        );
        assert_eq!(
            result["command_contract_sha256"],
            sha256(&canonical(&contract).unwrap())
        );
        assert_eq!(
            result["assertion_inventory_sha256"],
            sha256(&canonical(&result["assertion"]).unwrap())
        );
    }
}
