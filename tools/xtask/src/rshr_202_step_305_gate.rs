use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const STEP: u16 = 305;
const GATE_DIGEST: &str = "29633a173a2b7ee52d97d7a51feca377c1c182879fb73b9f3cf7524046b35148";
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
        "contracts/architecture/decisions/services_hardening_release_artifacts.v4.json",
        "6ed5bb06cf26565ac94d04f0b18a810e0a56372388e83e05c909c627da7ba7b4",
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
        "tools/xtask/src/exact_tree_archive.rs",
        "e176233167f99b783af4e041422eb619740a1d896138c77b51731400efb68ab2",
    ),
    (
        "tools/xtask/src/main.rs",
        "a4d543a4c690234203a88689ec7570aac0fcec53834ce6a7ef2970d83f34d216",
    ),
    (
        "tools/xtask/src/safe_artifact_io.rs",
        "3b361b22036ff8db5f5468e33341a7b2990d6f80aa6bdde4f791bd1320587e4d",
    ),
    (
        "tools/xtask/src/service_release_artifacts.rs",
        "52b5a014ce9ac58a0db849dfc105e7759732be5446b9cc16df4c80ceb303ec26",
    ),
    (
        "tools/xtask/src/service_source_lock_v3.rs",
        "8594897380149d864554a1cc0c463308a228091def7704215b8ca693fc61371c",
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
    serde_json::to_vec(value).map_err(|_| "Step 305 JSON encoding failed".to_owned())
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
            "-q", "-p", "xtask", "--", "rshr-step-305-gate", "--step={step}",
            "--check-id={check_id}", "--source-revision={source_revision}",
            "--source-tree={source_tree}", "--candidate-digest={candidate_digest}",
            "--platform=macos_aarch64", "--execution-request-sha256={execution_request_sha256}"
        ],
        "assertion_id": [format!("step_305_gate_01_{GATE_DIGEST}")],
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
        "verifier_path": "tools/xtask/src/rshr_202_step_305_gate.rs",
        "verifier_sha256": verifier_sha256
    })
}

pub(crate) fn run(arguments: Arguments) -> Result<(), String> {
    let check_id = format!("gate-01-{GATE_DIGEST}");
    if arguments.step != STEP
        || arguments.check_id != check_id
        || arguments.candidate_digest != "none"
        || arguments.platform != "macos_aarch64"
        || !valid_hex(&arguments.source_revision, 40)
        || !valid_hex(&arguments.source_tree, 40)
        || !valid_hex(&arguments.execution_request_sha256, 64)
    {
        return Err("Step 305 gate arguments differ".to_owned());
    }
    let root = root();
    if root.join(".github").exists() {
        return Err("forbidden .github surface is present".to_owned());
    }
    for (relative, expected) in EXACT_SOURCES {
        let observed = sha256(
            &fs::read(root.join(relative))
                .map_err(|_| "Step 305 governed source is unreadable".to_owned())?,
        );
        if observed != *expected {
            return Err(format!("Step 305 governed source bytes differ: {relative}"));
        }
    }
    let verifier_path = root.join("tools/xtask/src/rshr_202_step_305_gate.rs");
    let verifier_sha256 =
        sha256(&fs::read(verifier_path).map_err(|_| "Step 305 verifier is unreadable".to_owned())?);
    let authority_bytes = fs::read(root.join("contracts/rshr-202-step-305-gates.v1.json"))
        .map_err(|_| "Step 305 gate authority is unreadable".to_owned())?;
    let authority: Value = serde_json::from_slice(&authority_bytes)
        .map_err(|_| "Step 305 gate authority is invalid".to_owned())?;
    let mut canonical_authority = canonical(&authority)?;
    canonical_authority.push(b'\n');
    let contracts = authority
        .get("gate_command_contract")
        .and_then(Value::as_array)
        .ok_or_else(|| "Step 305 gate contract is absent".to_owned())?;
    if authority_bytes != canonical_authority
        || authority.get("schema") != Some(&json!("radroots.lib.rshr-202-step-305-gates.v1"))
        || authority.get("step") != Some(&json!([STEP]))
        || contracts.as_slice() != [expected_contract(&verifier_sha256)]
    {
        return Err("Step 305 gate authority differs".to_owned());
    }

    for (arguments, label) in [
        (
            vec!["+1.97.1", "fmt", "--all", "--", "--check"],
            "Step 305 formatting",
        ),
        (
            vec!["+1.97.1", "check", "--offline", "--locked", "-p", "xtask"],
            "Step 305 verifier check",
        ),
        (
            vec![
                "+1.97.1",
                "test",
                "--offline",
                "--locked",
                "-p",
                "xtask",
                "exact_tree_archive::tests",
            ],
            "Step 305 exact-tree archive tests",
        ),
        (
            vec![
                "+1.97.1",
                "test",
                "--offline",
                "--locked",
                "-p",
                "xtask",
                "service_source_lock_v3::tests",
            ],
            "Step 305 source-lock tests",
        ),
        (
            vec![
                "+1.97.1",
                "test",
                "--offline",
                "--locked",
                "-p",
                "xtask",
                "service_release_artifacts::tests",
            ],
            "Step 305 release-evidence and negative-vector tests",
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
        "Step 305 clippy",
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
        "Step 305 contracts",
    )?;

    let contract = &contracts[0];
    let assertion = json!([{ "id": format!("step_305_gate_01_{GATE_DIGEST}"), "result": "pass" }]);
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
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
        .map_err(|_| "Step 305 result write failed".to_owned())
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
