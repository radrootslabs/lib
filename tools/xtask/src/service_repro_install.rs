use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::bounded_process::{self, ProcessOutput, ProcessRequest, ReplacementEnvironment};

const CONTRACT_RELATIVE: &str =
    "contracts/architecture/decisions/services_hardening_repro_install.v1.json";
const PLAN_SCHEMA: &str = "radroots.services-hardening.repro-install-plan.v1";
const RESULT_SCHEMA: &str = "radroots.services-hardening.repro-install-result.v1";
const WITNESS_SCHEMA: &str = "radroots.services-hardening.repro-install-phase-witness.v1";
const NORMALIZATION_KIND: &str = "identity_exact_bytes_v1";
const MAX_PLAN_BYTES: u64 = 1024 * 1024;
const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
const MAX_PROCESS_STREAM_BYTES: usize = 32 * 1024 * 1024;
const MAX_PROCESS_DEADLINE_SECONDS: u64 = 3600;
const MAX_ARTIFACT_FILES: usize = 65_536;
const MAX_ARTIFACT_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ARTIFACT_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_DIFF_ENTRIES: usize = 256;
const MAX_ARGV_TOKENS: usize = 128;
const MAX_ARGV_TOKEN_BYTES: usize = 4096;
const EMPTY_STATE: &str = "empty";
const SUPPORTED_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu"];
const BUILD_PLACEHOLDERS: [&str; 6] = [
    "{checkout}",
    "{store}",
    "{output}",
    "{source_date_epoch}",
    "{candidate_digest}",
    "{target}",
];
const PHASE_PLACEHOLDERS: [&str; 7] = [
    "{phase}",
    "{install_root}",
    "{artifact_root}",
    "{artifact_set_sha256}",
    "{source_date_epoch}",
    "{candidate_digest}",
    "{target}",
];
const PHASE_IDS: [&str; 8] = [
    "fresh_install_candidate",
    "fresh_health_candidate",
    "install_predecessor",
    "pre_upgrade_health",
    "upgrade_candidate",
    "post_upgrade_health",
    "rollback_predecessor",
    "post_rollback_health",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReproInstallError {
    InvalidContract,
    InvalidPlan,
    InvalidSource,
    DirtySource,
    InvalidTool,
    CheckoutFailure,
    BuildFailure,
    InvalidArtifacts,
    ReproducibilityMismatch,
    InstallFailure,
    InvalidWitness,
    InvalidOutput,
}

impl ReproInstallError {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidContract => "invalid_contract",
            Self::InvalidPlan => "invalid_plan",
            Self::InvalidSource => "invalid_source",
            Self::DirtySource => "dirty_source",
            Self::InvalidTool => "invalid_tool",
            Self::CheckoutFailure => "checkout_failure",
            Self::BuildFailure => "build_failure",
            Self::InvalidArtifacts => "invalid_artifacts",
            Self::ReproducibilityMismatch => "reproducibility_mismatch",
            Self::InstallFailure => "install_failure",
            Self::InvalidWitness => "invalid_witness",
            Self::InvalidOutput => "invalid_output",
        }
    }
}

impl std::fmt::Display for ReproInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Debug)]
pub(crate) struct Arguments<'a> {
    pub(crate) plan: &'a Path,
    pub(crate) source_root: &'a Path,
    pub(crate) root_preimage_root: &'a Path,
    pub(crate) predecessor_artifact_root: &'a Path,
    pub(crate) git_executable: &'a Path,
    pub(crate) adapter_executable: &'a Path,
    pub(crate) output: &'a Path,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    candidate_digest: String,
    target: String,
    source_revision: String,
    source_tree: String,
    root_preimage_revision: String,
    source_date_epoch: u64,
    normalization: Normalization,
    git_executable_sha256: String,
    adapter_executable_sha256: String,
    build_argv: Vec<String>,
    phase: Vec<PhasePlan>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Normalization {
    kind: String,
    excluded_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhasePlan {
    id: String,
    argv: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PhaseWitness {
    schema: String,
    phase: String,
    candidate_digest: String,
    artifact_set_sha256: String,
    before_state_sha256: String,
    after_state_sha256: String,
    result: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ArtifactEntry {
    path: String,
    mode: u32,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug)]
struct ArtifactInventory {
    entries: Vec<ArtifactEntry>,
    sha256: String,
    total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct ArtifactDifference {
    path: String,
    first: Option<ArtifactEntry>,
    second: Option<ArtifactEntry>,
}

pub(crate) fn validate_contract(workspace_root: &Path) -> Result<(), String> {
    validate_contract_inner(workspace_root).map_err(|error| error.to_string())
}

fn validate_contract_inner(workspace_root: &Path) -> Result<(), ReproInstallError> {
    let bytes = read_regular_bounded(
        &workspace_root.join(CONTRACT_RELATIVE),
        MAX_PLAN_BYTES,
        ReproInstallError::InvalidContract,
    )?;
    let observed =
        serde_json::from_slice::<Value>(&bytes).map_err(|_| ReproInstallError::InvalidContract)?;
    if observed != expected_contract() {
        return Err(ReproInstallError::InvalidContract);
    }
    Ok(())
}

fn expected_contract() -> Value {
    json!({
        "schema": "radroots.services-hardening.repro-install-decisions.v1",
        "contract_version": 1,
        "decision_state": "active",
        "owner_step": 306,
        "command": "cargo xtask service-repro-install",
        "required_arguments": ["plan", "source_root", "root_preimage_root", "predecessor_artifact_root", "git_executable", "adapter_executable", "output"],
        "plan_schema": PLAN_SCHEMA,
        "result_schema": RESULT_SCHEMA,
        "phase_witness_schema": WITNESS_SCHEMA,
        "candidate_binding": "explicit_sha256_candidate_identity_digest",
        "source_binding": "exact_clean_git_revision_and_tree",
        "checkout_policy": "two_fresh_detached_no_local_no_checkout_clones",
        "checkout_count": 2,
        "store_policy": "one_distinct_empty_harness_owned_store_per_build",
        "source_date_epoch": "exact_root_preimage_commit_timestamp",
        "normalization": "identity_exact_bytes_v1_no_exclusions",
        "reproducibility_comparison": "exact_relative_path_mode_size_and_sha256_inventory",
        "tool_binding": "exact_regular_executable_bytes_sha256",
        "process_policy": "bounded_process_group_closed_stdin_replacement_environment",
        "result_write_policy": "create_new_regular_file_only_with_failure_diff_preserved",
        "install_roots": "distinct_fresh_and_lifecycle_harness_owned_roots",
        "phase_inventory": PHASE_IDS,
        "phase_state_policy": "canonical_witnesses_form_two_closed_state_chains_and_health_is_nonmutating",
        "maximums": {
            "plan_bytes": MAX_PLAN_BYTES,
            "result_bytes": MAX_RESULT_BYTES,
            "process_stream_bytes": MAX_PROCESS_STREAM_BYTES,
            "process_deadline_seconds": MAX_PROCESS_DEADLINE_SECONDS,
            "artifact_files": MAX_ARTIFACT_FILES,
            "artifact_total_bytes": MAX_ARTIFACT_TOTAL_BYTES,
            "artifact_file_bytes": MAX_ARTIFACT_FILE_BYTES,
            "diff_entries": MAX_DIFF_ENTRIES,
            "argv_tokens": MAX_ARGV_TOKENS,
            "argv_token_bytes": MAX_ARGV_TOKEN_BYTES
        },
        "required_negative_vectors": [
            "dirty_source", "wrong_source_tree", "wrong_root_preimage_epoch",
            "reused_checkout_or_store", "artifact_path_mode_size_or_digest_mismatch",
            "open_normalization", "missing_or_reordered_phase", "noncanonical_witness",
            "mutating_health_witness", "broken_upgrade_or_rollback_chain",
            "replaced_tool_executable", "preexisting_output"
        ],
        "negative_error_codes": [
            "invalid_contract", "invalid_plan", "invalid_source", "dirty_source",
            "invalid_tool", "checkout_failure", "build_failure", "invalid_artifacts",
            "reproducibility_mismatch", "install_failure", "invalid_witness", "invalid_output"
        ]
    })
}

pub(crate) fn run(workspace_root: &Path, arguments: Arguments<'_>) -> Result<(), String> {
    run_inner(workspace_root, arguments).map_err(|error| error.to_string())
}

fn run_inner(workspace_root: &Path, arguments: Arguments<'_>) -> Result<(), ReproInstallError> {
    validate_contract_inner(workspace_root)?;
    validate_output_target(arguments.output)?;
    let plan = load_plan(arguments.plan)?;
    validate_plan(&plan)?;
    validate_executable(arguments.git_executable, &plan.git_executable_sha256)?;
    validate_executable(
        arguments.adapter_executable,
        &plan.adapter_executable_sha256,
    )?;

    let source_root = canonical_directory(arguments.source_root, ReproInstallError::InvalidSource)?;
    let root_preimage_root = canonical_directory(
        arguments.root_preimage_root,
        ReproInstallError::InvalidSource,
    )?;
    let predecessor_root = canonical_directory(
        arguments.predecessor_artifact_root,
        ReproInstallError::InvalidArtifacts,
    )?;
    let temporary = tempfile::Builder::new()
        .prefix("radroots-repro-install-")
        .tempdir()
        .map_err(|_| ReproInstallError::InvalidOutput)?;
    let harness_root = temporary
        .path()
        .canonicalize()
        .map_err(|_| ReproInstallError::InvalidOutput)?;
    let home = create_owned_directory(&harness_root.join("home"))?;

    verify_source(
        arguments.git_executable,
        &source_root,
        &home,
        &plan.source_revision,
        &plan.source_tree,
    )?;
    verify_root_preimage_epoch(
        arguments.git_executable,
        &root_preimage_root,
        &home,
        &plan.root_preimage_revision,
        plan.source_date_epoch,
    )?;

    let checkout_one = harness_root.join("checkout-1");
    let checkout_two = harness_root.join("checkout-2");
    create_checkout(
        arguments.git_executable,
        &source_root,
        &checkout_one,
        &home,
        &plan,
    )?;
    create_checkout(
        arguments.git_executable,
        &source_root,
        &checkout_two,
        &home,
        &plan,
    )?;
    require_distinct(
        &checkout_one,
        &checkout_two,
        ReproInstallError::CheckoutFailure,
    )?;

    let store_one = create_owned_directory(&harness_root.join("store-1"))?;
    let store_two = create_owned_directory(&harness_root.join("store-2"))?;
    let output_one = create_owned_directory(&harness_root.join("output-1"))?;
    let output_two = create_owned_directory(&harness_root.join("output-2"))?;
    require_distinct(&store_one, &store_two, ReproInstallError::BuildFailure)?;
    require_distinct(&output_one, &output_two, ReproInstallError::BuildFailure)?;
    run_build(
        arguments.adapter_executable,
        &plan,
        &checkout_one,
        &store_one,
        &output_one,
        &home,
    )?;
    run_build(
        arguments.adapter_executable,
        &plan,
        &checkout_two,
        &store_two,
        &output_two,
        &home,
    )?;
    let first = artifact_inventory(&output_one)?;
    let second = artifact_inventory(&output_two)?;
    let differences = compare_inventories(&first, &second);
    if !differences.is_empty() {
        let result = failure_result(&plan, &first, &second, &differences);
        write_result(arguments.output, &result)?;
        return Err(ReproInstallError::ReproducibilityMismatch);
    }

    let predecessor = artifact_inventory(&predecessor_root)?;
    let fresh_root = create_owned_directory(&harness_root.join("install-fresh"))?;
    let lifecycle_root = create_owned_directory(&harness_root.join("install-lifecycle"))?;
    require_distinct(
        &fresh_root,
        &lifecycle_root,
        ReproInstallError::InstallFailure,
    )?;
    let witnesses = run_install_phases(
        arguments.adapter_executable,
        &plan,
        &output_one,
        &first.sha256,
        &predecessor_root,
        &predecessor.sha256,
        &fresh_root,
        &lifecycle_root,
        &home,
    )?;
    validate_phase_chain(
        &witnesses,
        &plan.candidate_digest,
        &first.sha256,
        &predecessor.sha256,
    )?;

    let result = json!({
        "schema": RESULT_SCHEMA,
        "candidate_digest": plan.candidate_digest,
        "target": plan.target,
        "source_revision": plan.source_revision,
        "source_tree": plan.source_tree,
        "root_preimage_revision": plan.root_preimage_revision,
        "source_date_epoch": plan.source_date_epoch,
        "normalization": {"kind": NORMALIZATION_KIND, "excluded_paths": []},
        "build": [
            build_result("build-1", "checkout-1", "store-1", &first),
            build_result("build-2", "checkout-2", "store-2", &second)
        ],
        "reproducibility": {
            "first_inventory_sha256": first.sha256,
            "second_inventory_sha256": second.sha256,
            "difference": [],
            "result": "pass"
        },
        "predecessor_artifact_set_sha256": predecessor.sha256,
        "install_phase": witnesses,
        "result": "pass"
    });
    write_result(arguments.output, &result)
}

fn load_plan(path: &Path) -> Result<Plan, ReproInstallError> {
    let bytes = read_regular_bounded(path, MAX_PLAN_BYTES, ReproInstallError::InvalidPlan)?;
    let value =
        serde_json::from_slice::<Value>(&bytes).map_err(|_| ReproInstallError::InvalidPlan)?;
    if canonical_json_line(&value).map_err(|_| ReproInstallError::InvalidPlan)? != bytes {
        return Err(ReproInstallError::InvalidPlan);
    }
    serde_json::from_value(value).map_err(|_| ReproInstallError::InvalidPlan)
}

fn validate_plan(plan: &Plan) -> Result<(), ReproInstallError> {
    if plan.schema != PLAN_SCHEMA
        || !valid_hex(&plan.candidate_digest, 64)
        || !SUPPORTED_TARGETS.contains(&plan.target.as_str())
        || !valid_hex(&plan.source_revision, 40)
        || !valid_hex(&plan.source_tree, 40)
        || !valid_hex(&plan.root_preimage_revision, 40)
        || plan.source_date_epoch == 0
        || plan.normalization.kind != NORMALIZATION_KIND
        || !plan.normalization.excluded_paths.is_empty()
        || !valid_hex(&plan.git_executable_sha256, 64)
        || !valid_hex(&plan.adapter_executable_sha256, 64)
        || plan.phase.len() != PHASE_IDS.len()
        || plan
            .phase
            .iter()
            .map(|phase| phase.id.as_str())
            .ne(PHASE_IDS)
    {
        return Err(ReproInstallError::InvalidPlan);
    }
    validate_argv_template(&plan.build_argv, &BUILD_PLACEHOLDERS)?;
    for phase in &plan.phase {
        validate_argv_template(&phase.argv, &PHASE_PLACEHOLDERS)?;
    }
    Ok(())
}

fn validate_argv_template(
    arguments: &[String],
    required: &[&str],
) -> Result<(), ReproInstallError> {
    if arguments.is_empty() || arguments.len() > MAX_ARGV_TOKENS {
        return Err(ReproInstallError::InvalidPlan);
    }
    let required = required.iter().copied().collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for argument in arguments {
        if argument.is_empty()
            || argument.len() > MAX_ARGV_TOKEN_BYTES
            || argument
                .bytes()
                .any(|byte| matches!(byte, 0 | b'\r' | b'\n'))
        {
            return Err(ReproInstallError::InvalidPlan);
        }
        if (argument.contains('{') || argument.contains('}'))
            && (!required.contains(argument.as_str()) || !observed.insert(argument.as_str()))
        {
            return Err(ReproInstallError::InvalidPlan);
        }
    }
    if observed != required {
        return Err(ReproInstallError::InvalidPlan);
    }
    Ok(())
}

fn verify_source(
    git: &Path,
    source_root: &Path,
    home: &Path,
    revision: &str,
    tree: &str,
) -> Result<(), ReproInstallError> {
    let status = git_output(
        git,
        source_root,
        home,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if !status.stdout().is_empty() {
        return Err(ReproInstallError::DirtySource);
    }
    if git_line(git, source_root, home, ["rev-parse", "HEAD"])? != revision
        || git_line(git, source_root, home, ["rev-parse", "HEAD^{tree}"])? != tree
    {
        return Err(ReproInstallError::InvalidSource);
    }
    Ok(())
}

fn verify_root_preimage_epoch(
    git: &Path,
    root: &Path,
    home: &Path,
    revision: &str,
    epoch: u64,
) -> Result<(), ReproInstallError> {
    let observed = git_line(git, root, home, ["show", "-s", "--format=%ct", revision])?
        .parse::<u64>()
        .map_err(|_| ReproInstallError::InvalidSource)?;
    if observed != epoch {
        return Err(ReproInstallError::InvalidSource);
    }
    Ok(())
}

fn create_checkout(
    git: &Path,
    source_root: &Path,
    checkout: &Path,
    home: &Path,
    plan: &Plan,
) -> Result<(), ReproInstallError> {
    let source = os_string(source_root);
    let destination = os_string(checkout);
    let clone_arguments = [
        OsString::from("-c"),
        OsString::from("core.hooksPath=/dev/null"),
        OsString::from("-c"),
        OsString::from("protocol.file.allow=always"),
        OsString::from("clone"),
        OsString::from("--no-local"),
        OsString::from("--no-checkout"),
        OsString::from("--no-tags"),
        source,
        destination,
    ];
    run_process(
        git,
        &clone_arguments,
        source_root,
        git_environment(home)?,
        60,
        ReproInstallError::CheckoutFailure,
    )?;
    let checkout = checkout
        .canonicalize()
        .map_err(|_| ReproInstallError::CheckoutFailure)?;
    let checkout_arguments = [
        OsString::from("-c"),
        OsString::from("core.hooksPath=/dev/null"),
        OsString::from("checkout"),
        OsString::from("--detach"),
        OsString::from(&plan.source_revision),
    ];
    run_process(
        git,
        &checkout_arguments,
        &checkout,
        git_environment(home)?,
        60,
        ReproInstallError::CheckoutFailure,
    )?;
    verify_source(
        git,
        &checkout,
        home,
        &plan.source_revision,
        &plan.source_tree,
    )
    .map_err(|_| ReproInstallError::CheckoutFailure)
}

fn run_build(
    adapter: &Path,
    plan: &Plan,
    checkout: &Path,
    store: &Path,
    output: &Path,
    home: &Path,
) -> Result<(), ReproInstallError> {
    require_empty_directory(store, ReproInstallError::BuildFailure)?;
    require_empty_directory(output, ReproInstallError::BuildFailure)?;
    let values = BTreeMap::from([
        ("{checkout}", os_string(checkout)),
        ("{store}", os_string(store)),
        ("{output}", os_string(output)),
        (
            "{source_date_epoch}",
            OsString::from(plan.source_date_epoch.to_string()),
        ),
        ("{candidate_digest}", OsString::from(&plan.candidate_digest)),
        ("{target}", OsString::from(&plan.target)),
    ]);
    let arguments = resolve_arguments(&plan.build_argv, &values)?;
    run_process(
        adapter,
        &arguments,
        checkout,
        harness_environment(home, plan, Some(store))?,
        MAX_PROCESS_DEADLINE_SECONDS,
        ReproInstallError::BuildFailure,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_install_phases(
    adapter: &Path,
    plan: &Plan,
    candidate_root: &Path,
    candidate_sha256: &str,
    predecessor_root: &Path,
    predecessor_sha256: &str,
    fresh_root: &Path,
    lifecycle_root: &Path,
    home: &Path,
) -> Result<Vec<PhaseWitness>, ReproInstallError> {
    let mut witnesses = Vec::with_capacity(PHASE_IDS.len());
    for phase in &plan.phase {
        let uses_candidate = matches!(
            phase.id.as_str(),
            "fresh_install_candidate"
                | "fresh_health_candidate"
                | "upgrade_candidate"
                | "post_upgrade_health"
        );
        let install_root = if phase.id.starts_with("fresh_") {
            fresh_root
        } else {
            lifecycle_root
        };
        let artifact_root = if uses_candidate {
            candidate_root
        } else {
            predecessor_root
        };
        let artifact_sha256 = if uses_candidate {
            candidate_sha256
        } else {
            predecessor_sha256
        };
        let values = BTreeMap::from([
            ("{phase}", OsString::from(&phase.id)),
            ("{install_root}", os_string(install_root)),
            ("{artifact_root}", os_string(artifact_root)),
            ("{artifact_set_sha256}", OsString::from(artifact_sha256)),
            (
                "{source_date_epoch}",
                OsString::from(plan.source_date_epoch.to_string()),
            ),
            ("{candidate_digest}", OsString::from(&plan.candidate_digest)),
            ("{target}", OsString::from(&plan.target)),
        ]);
        let arguments = resolve_arguments(&phase.argv, &values)?;
        let output = run_process(
            adapter,
            &arguments,
            install_root,
            harness_environment(home, plan, None)?,
            MAX_PROCESS_DEADLINE_SECONDS,
            ReproInstallError::InstallFailure,
        )?;
        let value = serde_json::from_slice::<Value>(output.stdout())
            .map_err(|_| ReproInstallError::InvalidWitness)?;
        if canonical_json_line(&value).map_err(|_| ReproInstallError::InvalidWitness)?
            != output.stdout()
        {
            return Err(ReproInstallError::InvalidWitness);
        }
        let witness = serde_json::from_value::<PhaseWitness>(value)
            .map_err(|_| ReproInstallError::InvalidWitness)?;
        if witness.schema != WITNESS_SCHEMA
            || witness.phase != phase.id
            || witness.candidate_digest != plan.candidate_digest
            || witness.artifact_set_sha256 != artifact_sha256
            || witness.result != "pass"
            || !valid_state(&witness.before_state_sha256)
            || !valid_hex(&witness.after_state_sha256, 64)
        {
            return Err(ReproInstallError::InvalidWitness);
        }
        witnesses.push(witness);
    }
    Ok(witnesses)
}

fn validate_phase_chain(
    witnesses: &[PhaseWitness],
    candidate_digest: &str,
    candidate_artifacts: &str,
    predecessor_artifacts: &str,
) -> Result<(), ReproInstallError> {
    if witnesses.len() != PHASE_IDS.len()
        || witnesses
            .iter()
            .map(|witness| witness.phase.as_str())
            .ne(PHASE_IDS)
        || witnesses.iter().any(|witness| {
            witness.schema != WITNESS_SCHEMA
                || witness.candidate_digest != candidate_digest
                || witness.result != "pass"
        })
    {
        return Err(ReproInstallError::InvalidWitness);
    }
    let expected_artifacts = [
        candidate_artifacts,
        candidate_artifacts,
        predecessor_artifacts,
        predecessor_artifacts,
        candidate_artifacts,
        candidate_artifacts,
        predecessor_artifacts,
        predecessor_artifacts,
    ];
    if witnesses
        .iter()
        .zip(expected_artifacts)
        .any(|(witness, expected)| witness.artifact_set_sha256 != expected)
        || witnesses[0].before_state_sha256 != EMPTY_STATE
        || witnesses[1].before_state_sha256 != witnesses[0].after_state_sha256
        || witnesses[1].after_state_sha256 != witnesses[0].after_state_sha256
        || witnesses[2].before_state_sha256 != EMPTY_STATE
        || witnesses[3].before_state_sha256 != witnesses[2].after_state_sha256
        || witnesses[3].after_state_sha256 != witnesses[2].after_state_sha256
        || witnesses[4].before_state_sha256 != witnesses[3].after_state_sha256
        || witnesses[5].before_state_sha256 != witnesses[4].after_state_sha256
        || witnesses[5].after_state_sha256 != witnesses[4].after_state_sha256
        || witnesses[6].before_state_sha256 != witnesses[5].after_state_sha256
        || witnesses[7].before_state_sha256 != witnesses[6].after_state_sha256
        || witnesses[7].after_state_sha256 != witnesses[6].after_state_sha256
    {
        return Err(ReproInstallError::InvalidWitness);
    }
    Ok(())
}

fn artifact_inventory(root: &Path) -> Result<ArtifactInventory, ReproInstallError> {
    let root = canonical_directory(root, ReproInstallError::InvalidArtifacts)?;
    let mut entries = Vec::new();
    let mut total_bytes = 0_u64;
    for entry in walkdir::WalkDir::new(&root).follow_links(false) {
        let entry = entry.map_err(|_| ReproInstallError::InvalidArtifacts)?;
        if entry.path() == root {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| ReproInstallError::InvalidArtifacts)?;
        if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(ReproInstallError::InvalidArtifacts);
        }
        if metadata.is_dir() {
            continue;
        }
        if metadata.len() > MAX_ARTIFACT_FILE_BYTES || entries.len() == MAX_ARTIFACT_FILES {
            return Err(ReproInstallError::InvalidArtifacts);
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .filter(|total| *total <= MAX_ARTIFACT_TOTAL_BYTES)
            .ok_or(ReproInstallError::InvalidArtifacts)?;
        let relative = entry
            .path()
            .strip_prefix(&root)
            .map_err(|_| ReproInstallError::InvalidArtifacts)?;
        let path = portable_relative_path(relative)?;
        entries.push(ArtifactEntry {
            path,
            mode: file_mode(&metadata),
            size_bytes: metadata.len(),
            sha256: sha256_reader(
                File::open(entry.path()).map_err(|_| ReproInstallError::InvalidArtifacts)?,
            )?,
        });
    }
    if entries.is_empty() {
        return Err(ReproInstallError::InvalidArtifacts);
    }
    entries.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    if entries.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(ReproInstallError::InvalidArtifacts);
    }
    let sha256 = sha256_bytes(
        &serde_json::to_vec(&entries).map_err(|_| ReproInstallError::InvalidArtifacts)?,
    );
    Ok(ArtifactInventory {
        entries,
        sha256,
        total_bytes,
    })
}

fn compare_inventories(
    first: &ArtifactInventory,
    second: &ArtifactInventory,
) -> Vec<ArtifactDifference> {
    let first = first
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let second = second
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    first
        .keys()
        .chain(second.keys())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let left = first.get(path).copied();
            let right = second.get(path).copied();
            (left != right).then(|| ArtifactDifference {
                path: path.to_owned(),
                first: left.cloned(),
                second: right.cloned(),
            })
        })
        .take(MAX_DIFF_ENTRIES)
        .collect()
}

fn build_result(
    id: &str,
    checkout_id: &str,
    store_id: &str,
    inventory: &ArtifactInventory,
) -> Value {
    json!({
        "id": id,
        "checkout_id": checkout_id,
        "store_id": store_id,
        "artifact_inventory_sha256": inventory.sha256,
        "artifact_file_count": inventory.entries.len(),
        "artifact_total_bytes": inventory.total_bytes,
        "result": "pass"
    })
}

fn failure_result(
    plan: &Plan,
    first: &ArtifactInventory,
    second: &ArtifactInventory,
    differences: &[ArtifactDifference],
) -> Value {
    json!({
        "schema": RESULT_SCHEMA,
        "candidate_digest": plan.candidate_digest,
        "target": plan.target,
        "source_revision": plan.source_revision,
        "source_tree": plan.source_tree,
        "root_preimage_revision": plan.root_preimage_revision,
        "source_date_epoch": plan.source_date_epoch,
        "normalization": {"kind": NORMALIZATION_KIND, "excluded_paths": []},
        "build": [
            build_result("build-1", "checkout-1", "store-1", first),
            build_result("build-2", "checkout-2", "store-2", second)
        ],
        "reproducibility": {
            "first_inventory_sha256": first.sha256,
            "second_inventory_sha256": second.sha256,
            "difference": differences,
            "result": "fail"
        },
        "predecessor_artifact_set_sha256": "not_evaluated",
        "install_phase": [],
        "result": "fail"
    })
}

fn write_result(path: &Path, value: &Value) -> Result<(), ReproInstallError> {
    let bytes = canonical_json_line(value).map_err(|_| ReproInstallError::InvalidOutput)?;
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(ReproInstallError::InvalidOutput);
    }
    let parent = path.parent().ok_or(ReproInstallError::InvalidOutput)?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| ReproInstallError::InvalidOutput)?;
    if canonical_parent != parent || path.exists() || path.is_symlink() {
        return Err(ReproInstallError::InvalidOutput);
    }
    #[cfg(unix)]
    let mut output = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| ReproInstallError::InvalidOutput)?
    };
    #[cfg(not(unix))]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ReproInstallError::InvalidOutput)?;
    output
        .write_all(&bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_| ReproInstallError::InvalidOutput)
}

fn validate_output_target(path: &Path) -> Result<(), ReproInstallError> {
    if !path.is_absolute() || path.exists() || path.is_symlink() {
        return Err(ReproInstallError::InvalidOutput);
    }
    let parent = path.parent().ok_or(ReproInstallError::InvalidOutput)?;
    let canonical = parent
        .canonicalize()
        .map_err(|_| ReproInstallError::InvalidOutput)?;
    if canonical != parent {
        return Err(ReproInstallError::InvalidOutput);
    }
    Ok(())
}

fn validate_executable(path: &Path, expected_sha256: &str) -> Result<(), ReproInstallError> {
    if !path.is_absolute() || !valid_hex(expected_sha256, 64) {
        return Err(ReproInstallError::InvalidTool);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| ReproInstallError::InvalidTool)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || !executable_mode(&metadata) {
        return Err(ReproInstallError::InvalidTool);
    }
    let observed = sha256_reader(File::open(path).map_err(|_| ReproInstallError::InvalidTool)?)
        .map_err(|_| ReproInstallError::InvalidTool)?;
    if observed != expected_sha256 {
        return Err(ReproInstallError::InvalidTool);
    }
    Ok(())
}

fn read_regular_bounded(
    path: &Path,
    maximum: u64,
    error: ReproInstallError,
) -> Result<Vec<u8>, ReproInstallError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum {
        return Err(error);
    }
    fs::read(path).map_err(|_| error)
}

fn canonical_directory(
    path: &Path,
    error: ReproInstallError,
) -> Result<PathBuf, ReproInstallError> {
    if !path.is_absolute() {
        return Err(error);
    }
    let canonical = path.canonicalize().map_err(|_| error)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if canonical != path || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(error);
    }
    Ok(canonical)
}

fn create_owned_directory(path: &Path) -> Result<PathBuf, ReproInstallError> {
    fs::create_dir(path).map_err(|_| ReproInstallError::InvalidOutput)?;
    path.canonicalize()
        .map_err(|_| ReproInstallError::InvalidOutput)
}

fn require_empty_directory(path: &Path, error: ReproInstallError) -> Result<(), ReproInstallError> {
    let mut entries = fs::read_dir(path).map_err(|_| error)?;
    if entries.next().transpose().map_err(|_| error)?.is_some() {
        return Err(error);
    }
    Ok(())
}

fn require_distinct(
    left: &Path,
    right: &Path,
    error: ReproInstallError,
) -> Result<(), ReproInstallError> {
    if left == right {
        return Err(error);
    }
    let left_metadata = fs::metadata(left).map_err(|_| error)?;
    let right_metadata = fs::metadata(right).map_err(|_| error)?;
    if same_file(&left_metadata, &right_metadata) {
        return Err(error);
    }
    Ok(())
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn run_process(
    program: &Path,
    arguments: &[OsString],
    current_dir: &Path,
    environment: ReplacementEnvironment,
    deadline_seconds: u64,
    error: ReproInstallError,
) -> Result<ProcessOutput, ReproInstallError> {
    let mut request = ProcessRequest::new(program.as_os_str())
        .current_dir(current_dir)
        .environment(environment)
        .deadline(Duration::from_secs(deadline_seconds))
        .output_limits(MAX_PROCESS_STREAM_BYTES, MAX_PROCESS_STREAM_BYTES);
    for argument in arguments {
        request = request.arg(argument);
    }
    let output = bounded_process::run(&request).map_err(|_| error)?;
    if !output.status().success() {
        return Err(error);
    }
    Ok(output)
}

fn git_output<const N: usize>(
    git: &Path,
    current_dir: &Path,
    home: &Path,
    arguments: [&str; N],
) -> Result<ProcessOutput, ReproInstallError> {
    let arguments = arguments.map(OsString::from);
    run_process(
        git,
        &arguments,
        current_dir,
        git_environment(home)?,
        60,
        ReproInstallError::InvalidSource,
    )
}

fn git_line<const N: usize>(
    git: &Path,
    current_dir: &Path,
    home: &Path,
    arguments: [&str; N],
) -> Result<String, ReproInstallError> {
    let output = git_output(git, current_dir, home, arguments)?;
    let text =
        std::str::from_utf8(output.stdout()).map_err(|_| ReproInstallError::InvalidSource)?;
    let line = text
        .strip_suffix('\n')
        .ok_or(ReproInstallError::InvalidSource)?;
    if line.is_empty() || line.contains(['\r', '\n']) {
        return Err(ReproInstallError::InvalidSource);
    }
    Ok(line.to_owned())
}

fn git_environment(home: &Path) -> Result<ReplacementEnvironment, ReproInstallError> {
    let mut environment = ReplacementEnvironment::default();
    insert_environment(&mut environment, "HOME", home.as_os_str())?;
    insert_environment(&mut environment, "LC_ALL", "C")?;
    insert_environment(&mut environment, "TZ", "UTC")?;
    insert_environment(&mut environment, "GIT_CONFIG_NOSYSTEM", "1")?;
    Ok(environment)
}

fn harness_environment(
    home: &Path,
    plan: &Plan,
    store: Option<&Path>,
) -> Result<ReplacementEnvironment, ReproInstallError> {
    let mut environment = ReplacementEnvironment::default();
    insert_environment(&mut environment, "HOME", home.as_os_str())?;
    insert_environment(&mut environment, "LC_ALL", "C")?;
    insert_environment(&mut environment, "TZ", "UTC")?;
    insert_environment(
        &mut environment,
        "SOURCE_DATE_EPOCH",
        plan.source_date_epoch.to_string(),
    )?;
    insert_environment(
        &mut environment,
        "RSHR_CANDIDATE_DIGEST",
        &plan.candidate_digest,
    )?;
    if let Some(store) = store {
        insert_environment(&mut environment, "RSHR_BUILD_STORE", store.as_os_str())?;
    }
    Ok(environment)
}

fn insert_environment(
    environment: &mut ReplacementEnvironment,
    name: &str,
    value: impl Into<OsString>,
) -> Result<(), ReproInstallError> {
    environment
        .insert(name, value)
        .map_err(|_| ReproInstallError::InvalidPlan)
}

fn resolve_arguments(
    template: &[String],
    values: &BTreeMap<&str, OsString>,
) -> Result<Vec<OsString>, ReproInstallError> {
    template
        .iter()
        .map(|argument| {
            if argument.contains('{') || argument.contains('}') {
                values
                    .get(argument.as_str())
                    .cloned()
                    .ok_or(ReproInstallError::InvalidPlan)
            } else {
                Ok(OsString::from(argument))
            }
        })
        .collect()
}

fn portable_relative_path(path: &Path) -> Result<String, ReproInstallError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(ReproInstallError::InvalidArtifacts);
        };
        let part = part.to_str().ok_or(ReproInstallError::InvalidArtifacts)?;
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(['/', '\\', '\0', '\r', '\n'])
        {
            return Err(ReproInstallError::InvalidArtifacts);
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(ReproInstallError::InvalidArtifacts);
    }
    Ok(parts.join("/"))
}

fn sha256_reader(mut reader: impl Read) -> Result<String, ReproInstallError> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| ReproInstallError::InvalidArtifacts)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn canonical_json_line(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn valid_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_state(value: &str) -> bool {
    value == EMPTY_STATE || valid_hex(value, 64)
}

fn os_string(path: &Path) -> OsString {
    path.as_os_str().to_owned()
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    if metadata.permissions().readonly() {
        0o444
    } else {
        0o666
    }
}

#[cfg(unix)]
fn executable_mode(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_mode(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn sample_plan_value() -> Value {
        json!({
            "schema": PLAN_SCHEMA,
            "candidate_digest": "a".repeat(64),
            "target": "aarch64-apple-darwin",
            "source_revision": "b".repeat(40),
            "source_tree": "c".repeat(40),
            "root_preimage_revision": "d".repeat(40),
            "source_date_epoch": 1_700_000_000_u64,
            "normalization": {"kind": NORMALIZATION_KIND, "excluded_paths": []},
            "git_executable_sha256": "e".repeat(64),
            "adapter_executable_sha256": "f".repeat(64),
            "build_argv": BUILD_PLACEHOLDERS,
            "phase": PHASE_IDS.map(|id| json!({"id": id, "argv": PHASE_PLACEHOLDERS}))
        })
    }

    fn sample_plan() -> Plan {
        serde_json::from_value(sample_plan_value()).expect("sample plan")
    }

    fn witness(phase: &str, artifacts: &str, before: &str, after: &str) -> PhaseWitness {
        PhaseWitness {
            schema: WITNESS_SCHEMA.to_owned(),
            phase: phase.to_owned(),
            candidate_digest: "a".repeat(64),
            artifact_set_sha256: artifacts.to_owned(),
            before_state_sha256: before.to_owned(),
            after_state_sha256: after.to_owned(),
            result: "pass".to_owned(),
        }
    }

    fn valid_witnesses() -> Vec<PhaseWitness> {
        let candidate = "1".repeat(64);
        let predecessor = "2".repeat(64);
        let fresh = "3".repeat(64);
        let installed = "4".repeat(64);
        let upgraded = "5".repeat(64);
        let rolled_back = "6".repeat(64);
        vec![
            witness(PHASE_IDS[0], &candidate, EMPTY_STATE, &fresh),
            witness(PHASE_IDS[1], &candidate, &fresh, &fresh),
            witness(PHASE_IDS[2], &predecessor, EMPTY_STATE, &installed),
            witness(PHASE_IDS[3], &predecessor, &installed, &installed),
            witness(PHASE_IDS[4], &candidate, &installed, &upgraded),
            witness(PHASE_IDS[5], &candidate, &upgraded, &upgraded),
            witness(PHASE_IDS[6], &predecessor, &upgraded, &rolled_back),
            witness(PHASE_IDS[7], &predecessor, &rolled_back, &rolled_back),
        ]
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(path, bytes).expect("file");
    }

    fn program_path(name: &str) -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
            .expect("program on PATH")
            .canonicalize()
            .expect("canonical program")
    }

    fn fixture_git(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new(program_path("git"))
            .args(arguments)
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("fixture git");
        assert!(
            output.status.success(),
            "fixture git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("fixture git output")
            .trim()
            .to_owned()
    }

    fn create_repository(path: &Path, filename: &str) {
        fs::create_dir(path).expect("repository directory");
        fixture_git(path, &["init", "--quiet", "--initial-branch=master"]);
        fixture_git(path, &["config", "user.name", "Radroots Test"]);
        fixture_git(path, &["config", "user.email", "test@radroots.invalid"]);
        write_file(&path.join(filename), b"tracked\n");
        fixture_git(path, &["add", filename]);
        fixture_git(path, &["commit", "--quiet", "-m", "fixture"]);
    }

    #[cfg(unix)]
    fn write_fixture_adapter(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let script = r#"#!/bin/sh
set -eu
if [ "$1" = "build" ]; then
    printf '%s\n' 'exact candidate artifact' > "$4/release.bin"
    exit 0
fi
if [ "$1" != "phase" ]; then
    exit 2
fi
phase="$2"
install_root="$3"
if [ -f "$install_root/state" ]; then
    IFS= read -r before < "$install_root/state"
else
    before=empty
fi
case "$phase" in
    fresh_install_candidate) after=3333333333333333333333333333333333333333333333333333333333333333 ;;
    fresh_health_candidate) after="$before" ;;
    install_predecessor) after=4444444444444444444444444444444444444444444444444444444444444444 ;;
    pre_upgrade_health) after="$before" ;;
    upgrade_candidate) after=5555555555555555555555555555555555555555555555555555555555555555 ;;
    post_upgrade_health) after="$before" ;;
    rollback_predecessor) after=6666666666666666666666666666666666666666666666666666666666666666 ;;
    post_rollback_health) after="$before" ;;
    *) exit 2 ;;
esac
printf '%s\n' "$after" > "$install_root/state"
printf '{"after_state_sha256":"%s","artifact_set_sha256":"%s","before_state_sha256":"%s","candidate_digest":"%s","phase":"%s","result":"pass","schema":"radroots.services-hardening.repro-install-phase-witness.v1"}\n' "$after" "$5" "$before" "$7" "$phase"
"#;
        fs::write(path, script).expect("adapter");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("adapter mode");
    }

    #[test]
    fn plan_and_argument_admission_rejects_each_independent_policy_violation() {
        for (pointer, replacement) in [
            ("/schema", json!("unknown")),
            ("/candidate_digest", json!("A".repeat(64))),
            ("/target", json!("unknown")),
            ("/source_revision", json!("invalid")),
            ("/source_tree", json!("invalid")),
            ("/root_preimage_revision", json!("invalid")),
            ("/source_date_epoch", json!(0)),
            ("/normalization/kind", json!("ignore_timestamps")),
            ("/normalization/excluded_paths", json!(["secret"])),
            ("/git_executable_sha256", json!("invalid")),
            ("/adapter_executable_sha256", json!("invalid")),
            ("/phase", json!([])),
            ("/phase/0/id", json!("upgrade_candidate")),
            ("/phase/0/argv", json!([])),
        ] {
            let mut value = sample_plan_value();
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert_eq!(
                validate_plan(&serde_json::from_value(value).unwrap()),
                Err(ReproInstallError::InvalidPlan),
                "{pointer}"
            );
        }
        for arguments in [
            vec![],
            vec!["x".to_owned(); MAX_ARGV_TOKENS + 1],
            vec![String::new()],
            vec!["x".repeat(MAX_ARGV_TOKEN_BYTES + 1)],
            vec!["a\nb".to_owned()],
            vec!["a\0b".to_owned()],
            vec!["{unknown}".to_owned()],
            vec!["trailing}".to_owned()],
            vec!["{checkout}".to_owned(), "{checkout}".to_owned()],
            vec!["literal".to_owned()],
        ] {
            assert_eq!(
                validate_argv_template(&arguments, &["{checkout}"]),
                Err(ReproInstallError::InvalidPlan)
            );
        }
        let values = BTreeMap::from([("{checkout}", OsString::from("/fixture"))]);
        assert_eq!(
            resolve_arguments(&["literal".to_owned(), "{checkout}".to_owned()], &values).unwrap(),
            [OsString::from("literal"), OsString::from("/fixture")]
        );
        assert_eq!(
            resolve_arguments(&["{unknown}".to_owned()], &values),
            Err(ReproInstallError::InvalidPlan)
        );
        assert_eq!(
            resolve_arguments(&["trailing}".to_owned()], &values),
            Err(ReproInstallError::InvalidPlan)
        );
    }

    #[test]
    fn phase_chain_rejects_each_broken_binding_transition_and_health_mutation() {
        let candidate = "a".repeat(64);
        let artifacts = "1".repeat(64);
        let predecessor = "2".repeat(64);
        let validate = |rows: &[PhaseWitness]| {
            validate_phase_chain(rows, &candidate, &artifacts, &predecessor)
        };
        validate(&valid_witnesses()).unwrap();
        assert_eq!(validate(&[]), Err(ReproInstallError::InvalidWitness));
        let mut reordered = valid_witnesses();
        reordered.swap(0, 1);
        assert_eq!(validate(&reordered), Err(ReproInstallError::InvalidWitness));
        for index in 0..PHASE_IDS.len() {
            for field in [
                "schema",
                "candidate_digest",
                "artifact_set_sha256",
                "before_state_sha256",
                "result",
            ] {
                let mut value = serde_json::to_value(valid_witnesses()).unwrap();
                value[index][field] = json!("mismatch");
                let rows: Vec<PhaseWitness> = serde_json::from_value(value).unwrap();
                assert_eq!(
                    validate(&rows),
                    Err(ReproInstallError::InvalidWitness),
                    "{index} {field}"
                );
            }
        }
        for index in [1, 3, 5, 7] {
            let mut rows = valid_witnesses();
            rows[index].after_state_sha256 = "7".repeat(64);
            assert_eq!(validate(&rows), Err(ReproInstallError::InvalidWitness));
        }
    }

    #[cfg(unix)]
    #[test]
    fn install_receipts_reject_noncanonical_bytes_and_unbound_adapter_claims() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let adapter = root.join("adapter");
        let witness = serde_json::to_value(&valid_witnesses()[0]).unwrap();
        let mut invalid_outputs = vec![
            b"not json".to_vec(),
            serde_json::to_vec_pretty(&witness).unwrap(),
            b"{}\n".to_vec(),
        ];
        for field in [
            "schema",
            "phase",
            "candidate_digest",
            "artifact_set_sha256",
            "result",
            "before_state_sha256",
            "after_state_sha256",
        ] {
            let mut changed = witness.clone();
            changed[field] = json!("unbound");
            invalid_outputs.push(canonical_json_line(&changed).unwrap());
        }
        for bytes in invalid_outputs {
            let text = String::from_utf8(bytes).unwrap();
            assert!(!text.contains('\''));
            fs::write(&adapter, format!("#!/bin/sh\nprintf '%s' '{text}'\n")).unwrap();
            fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(matches!(
                run_install_phases(
                    &adapter,
                    &sample_plan(),
                    &root,
                    &"1".repeat(64),
                    &root,
                    &"2".repeat(64),
                    &root,
                    &root,
                    &root
                ),
                Err(ReproInstallError::InvalidWitness)
            ));
        }
        fs::write(&adapter, b"#!/bin/sh\nexit 3\n").unwrap();
        assert!(matches!(
            run_install_phases(
                &adapter,
                &sample_plan(),
                &root,
                &"1".repeat(64),
                &root,
                &"2".repeat(64),
                &root,
                &root,
                &root
            ),
            Err(ReproInstallError::InstallFailure)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_and_source_admission_preserves_existing_outputs() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temporary = TempDir::new().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let file = root.join("file");
        fs::write(&file, b"preserve").unwrap();
        let dangling = root.join("dangling");
        symlink(root.join("missing"), &dangling).unwrap();
        let linked = root.join("linked");
        symlink(&root, &linked).unwrap();
        for path in [
            Path::new("relative"),
            &file,
            &dangling,
            &linked.join("result"),
            &root.join("missing/result"),
        ] {
            assert_eq!(
                validate_output_target(path),
                Err(ReproInstallError::InvalidOutput)
            );
        }
        assert_eq!(
            write_result(&dangling, &json!({})),
            Err(ReproInstallError::InvalidOutput)
        );
        assert_eq!(
            write_result(&linked.join("result"), &json!({})),
            Err(ReproInstallError::InvalidOutput)
        );
        assert_eq!(
            write_result(
                &root.join("too-large"),
                &json!("x".repeat(MAX_RESULT_BYTES))
            ),
            Err(ReproInstallError::InvalidOutput)
        );
        for path in [Path::new("relative"), &file, &linked] {
            assert!(canonical_directory(path, ReproInstallError::InvalidArtifacts).is_err());
        }
        assert_eq!(
            validate_executable(Path::new("relative"), &"0".repeat(64)),
            Err(ReproInstallError::InvalidTool)
        );
        assert_eq!(
            validate_executable(&file, "invalid"),
            Err(ReproInstallError::InvalidTool)
        );
        assert_eq!(
            validate_executable(&root, &"0".repeat(64)),
            Err(ReproInstallError::InvalidTool)
        );
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            validate_executable(&file, &sha256_bytes(b"preserve")),
            Err(ReproInstallError::InvalidTool)
        );
        for path in [&root, &linked, &file] {
            assert_eq!(
                read_regular_bounded(path, 2, ReproInstallError::InvalidPlan),
                Err(ReproInstallError::InvalidPlan)
            );
        }
        assert_eq!(
            require_empty_directory(&root, ReproInstallError::BuildFailure),
            Err(ReproInstallError::BuildFailure)
        );
        assert_eq!(
            require_distinct(&root, &linked, ReproInstallError::BuildFailure),
            Err(ReproInstallError::BuildFailure)
        );
        for path in ["", "/absolute", "../escape", ".", "a\\b", "a\nb", "a\0b"] {
            assert_eq!(
                portable_relative_path(Path::new(path)),
                Err(ReproInstallError::InvalidArtifacts)
            );
        }
        let empty = root.join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(matches!(
            artifact_inventory(&empty),
            Err(ReproInstallError::InvalidArtifacts)
        ));
        let source = root.join("source");
        create_repository(&source, "source.txt");
        let git = program_path("git");
        let revision = fixture_git(&source, &["rev-parse", "HEAD"]);
        let tree = fixture_git(&source, &["rev-parse", "HEAD^{tree}"]);
        assert_eq!(
            verify_source(&git, &source, &root, &"0".repeat(40), &tree),
            Err(ReproInstallError::InvalidSource)
        );
        assert_eq!(
            verify_source(&git, &source, &root, &revision, &"0".repeat(40)),
            Err(ReproInstallError::InvalidSource)
        );
        assert_eq!(
            verify_root_preimage_epoch(&git, &source, &root, &revision, 1),
            Err(ReproInstallError::InvalidSource)
        );
        assert_eq!(fs::read(&file).unwrap(), b"preserve");
    }

    #[test]
    fn decision_contract_is_exact() {
        validate_contract_inner(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("decision contract");
        assert_eq!(
            [
                ReproInstallError::InvalidContract,
                ReproInstallError::InvalidPlan,
                ReproInstallError::InvalidSource,
                ReproInstallError::DirtySource,
                ReproInstallError::InvalidTool,
                ReproInstallError::CheckoutFailure,
                ReproInstallError::BuildFailure,
                ReproInstallError::InvalidArtifacts,
                ReproInstallError::ReproducibilityMismatch,
                ReproInstallError::InstallFailure,
                ReproInstallError::InvalidWitness,
                ReproInstallError::InvalidOutput,
            ]
            .map(ReproInstallError::code),
            [
                "invalid_contract",
                "invalid_plan",
                "invalid_source",
                "dirty_source",
                "invalid_tool",
                "checkout_failure",
                "build_failure",
                "invalid_artifacts",
                "reproducibility_mismatch",
                "install_failure",
                "invalid_witness",
                "invalid_output",
            ]
        );
    }

    #[test]
    fn plan_requires_closed_normalization_tools_and_ordered_phases() {
        validate_plan(&sample_plan()).expect("valid plan");
        let mut open = sample_plan_value();
        open["normalization"]["excluded_paths"] = json!(["build-id"]);
        assert_eq!(
            validate_plan(&serde_json::from_value(open).expect("open plan")),
            Err(ReproInstallError::InvalidPlan)
        );
        let mut reordered = sample_plan_value();
        reordered["phase"]
            .as_array_mut()
            .expect("phases")
            .swap(0, 1);
        assert_eq!(
            validate_plan(&serde_json::from_value(reordered).expect("reordered plan")),
            Err(ReproInstallError::InvalidPlan)
        );
        let mut missing_placeholder = sample_plan_value();
        missing_placeholder["build_argv"] = json!(&BUILD_PLACEHOLDERS[1..]);
        assert_eq!(
            validate_plan(
                &serde_json::from_value(missing_placeholder).expect("missing placeholder")
            ),
            Err(ReproInstallError::InvalidPlan)
        );
    }

    #[test]
    fn canonical_plan_and_witness_reject_noncanonical_bytes() {
        let temporary = TempDir::new().expect("temporary");
        let plan_path = temporary.path().join("plan.json");
        let canonical = canonical_json_line(&sample_plan_value()).expect("canonical plan");
        fs::write(&plan_path, &canonical).expect("plan");
        load_plan(&plan_path).expect("canonical accepted");
        let mut pretty = serde_json::to_vec_pretty(&sample_plan_value()).expect("pretty");
        pretty.push(b'\n');
        fs::write(&plan_path, pretty).expect("pretty plan");
        assert!(matches!(
            load_plan(&plan_path),
            Err(ReproInstallError::InvalidPlan)
        ));

        let value = serde_json::to_value(&valid_witnesses()[0]).expect("witness value");
        let canonical = canonical_json_line(&value).expect("canonical witness");
        assert_eq!(canonical.last(), Some(&b'\n'));
        assert_ne!(
            serde_json::to_vec_pretty(&value).expect("pretty witness"),
            canonical
        );
    }

    #[test]
    fn exact_artifact_inventory_reports_path_mode_size_and_digest_differences() {
        let temporary = TempDir::new().expect("temporary");
        let first_root = temporary.path().join("first");
        let second_root = temporary.path().join("second");
        write_file(&first_root.join("bin/service"), b"same");
        write_file(&second_root.join("bin/service"), b"same");
        let first = artifact_inventory(&first_root.canonicalize().expect("first root"))
            .expect("first inventory");
        let second = artifact_inventory(&second_root.canonicalize().expect("second root"))
            .expect("second inventory");
        assert!(compare_inventories(&first, &second).is_empty());

        write_file(&second_root.join("bin/service"), b"changed");
        write_file(&second_root.join("extra"), b"extra");
        let second = artifact_inventory(&second_root.canonicalize().expect("second root"))
            .expect("changed inventory");
        let differences = compare_inventories(&first, &second);
        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.as_str())
                .collect::<Vec<_>>(),
            ["bin/service", "extra"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn artifact_inventory_rejects_symlinks_and_binds_mode() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary = TempDir::new().expect("temporary");
        let root = temporary.path().join("artifacts");
        write_file(&root.join("service"), b"service");
        fs::set_permissions(root.join("service"), fs::Permissions::from_mode(0o755)).expect("mode");
        let executable =
            artifact_inventory(&root.canonicalize().expect("root")).expect("executable inventory");
        assert_eq!(executable.entries[0].mode, 0o755);
        symlink("service", root.join("linked")).expect("symlink");
        assert!(matches!(
            artifact_inventory(&root.canonicalize().expect("root")),
            Err(ReproInstallError::InvalidArtifacts)
        ));
    }

    #[test]
    fn install_upgrade_and_rollback_witnesses_form_exact_chains() {
        let candidate = "1".repeat(64);
        let predecessor = "2".repeat(64);
        let witnesses = valid_witnesses();
        validate_phase_chain(&witnesses, &"a".repeat(64), &candidate, &predecessor)
            .expect("valid phase chain");

        let mut mutating_health = valid_witnesses();
        mutating_health[5].after_state_sha256 = "7".repeat(64);
        assert_eq!(
            validate_phase_chain(&mutating_health, &"a".repeat(64), &candidate, &predecessor),
            Err(ReproInstallError::InvalidWitness)
        );
        let mut broken_rollback = valid_witnesses();
        broken_rollback[6].before_state_sha256 = "8".repeat(64);
        assert_eq!(
            validate_phase_chain(&broken_rollback, &"a".repeat(64), &candidate, &predecessor),
            Err(ReproInstallError::InvalidWitness)
        );
    }

    #[cfg(unix)]
    #[test]
    fn exact_tool_bytes_and_create_new_output_fail_closed() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary = TempDir::new().expect("temporary");
        let root = temporary.path().canonicalize().expect("root");
        let tool = root.join("tool");
        fs::write(&tool, b"tool").expect("tool");
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).expect("mode");
        validate_executable(&tool, &sha256_bytes(b"tool")).expect("valid tool");
        assert_eq!(
            validate_executable(&tool, &sha256_bytes(b"replacement")),
            Err(ReproInstallError::InvalidTool)
        );
        let link = root.join("tool-link");
        symlink(&tool, &link).expect("tool symlink");
        assert_eq!(
            validate_executable(&link, &sha256_bytes(b"tool")),
            Err(ReproInstallError::InvalidTool)
        );

        let output = root.join("result.json");
        write_result(&output, &json!({"result": "pass"})).expect("first result");
        assert_eq!(
            write_result(&output, &json!({"result": "pass"})),
            Err(ReproInstallError::InvalidOutput)
        );
    }

    #[test]
    fn source_epoch_and_distinct_store_invariants_are_exact() {
        let temporary = TempDir::new().expect("temporary");
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        fs::create_dir(&first).expect("first");
        fs::create_dir(&second).expect("second");
        require_distinct(&first, &second, ReproInstallError::BuildFailure).expect("distinct roots");
        assert_eq!(
            require_distinct(&first, &first, ReproInstallError::BuildFailure),
            Err(ReproInstallError::BuildFailure)
        );
        let plan = sample_plan();
        assert_eq!(plan.source_date_epoch, 1_700_000_000);
        assert_ne!(plan.root_preimage_revision, plan.source_revision);
    }

    #[cfg(unix)]
    #[test]
    fn full_harness_uses_two_clones_root_epoch_and_all_install_phases() {
        let temporary = TempDir::new().expect("temporary");
        let fixture_root = temporary.path().canonicalize().expect("fixture root");
        let source = fixture_root.join("source");
        let root_preimage = fixture_root.join("root-preimage");
        create_repository(&source, "source.txt");
        create_repository(&root_preimage, "root.txt");
        let source_revision = fixture_git(&source, &["rev-parse", "HEAD"]);
        let source_tree = fixture_git(&source, &["rev-parse", "HEAD^{tree}"]);
        let root_revision = fixture_git(&root_preimage, &["rev-parse", "HEAD"]);
        let source_date_epoch = fixture_git(
            &root_preimage,
            &["show", "-s", "--format=%ct", &root_revision],
        )
        .parse::<u64>()
        .expect("root epoch");
        let predecessor = fixture_root.join("predecessor");
        write_file(
            &predecessor.join("release.bin"),
            b"exact predecessor artifact\n",
        );
        let predecessor = predecessor.canonicalize().expect("predecessor");
        let git = program_path("git");
        let adapter = fixture_root.join("adapter");
        write_fixture_adapter(&adapter);

        let mut plan_value = sample_plan_value();
        plan_value["source_revision"] = json!(source_revision);
        plan_value["source_tree"] = json!(source_tree);
        plan_value["root_preimage_revision"] = json!(root_revision);
        plan_value["source_date_epoch"] = json!(source_date_epoch);
        plan_value["git_executable_sha256"] =
            json!(sha256_reader(File::open(&git).expect("git")).expect("git hash"));
        plan_value["adapter_executable_sha256"] =
            json!(sha256_bytes(&fs::read(&adapter).expect("adapter bytes")));
        plan_value["build_argv"] = json!([
            "build",
            "{checkout}",
            "{store}",
            "{output}",
            "{source_date_epoch}",
            "{candidate_digest}",
            "{target}"
        ]);
        plan_value["phase"] = json!(PHASE_IDS.map(|id| json!({
            "id": id,
            "argv": [
                "phase",
                "{phase}",
                "{install_root}",
                "{artifact_root}",
                "{artifact_set_sha256}",
                "{source_date_epoch}",
                "{candidate_digest}",
                "{target}"
            ]
        })));
        let plan_path = fixture_root.join("plan.json");
        fs::write(
            &plan_path,
            canonical_json_line(&plan_value).expect("plan bytes"),
        )
        .expect("plan");
        let output = fixture_root.join("result.json");
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        run_inner(
            &workspace_root,
            Arguments {
                plan: &plan_path,
                source_root: &source,
                root_preimage_root: &root_preimage,
                predecessor_artifact_root: &predecessor,
                git_executable: &git,
                adapter_executable: &adapter,
                output: &output,
            },
        )
        .expect("full harness");
        let result: Value =
            serde_json::from_slice(&fs::read(&output).expect("result")).expect("result JSON");
        assert_eq!(result["result"], "pass");
        assert_eq!(result["build"].as_array().expect("builds").len(), 2);
        assert_eq!(
            result["build"][0]["artifact_inventory_sha256"],
            result["build"][1]["artifact_inventory_sha256"]
        );
        assert_eq!(
            result["install_phase"].as_array().expect("phases").len(),
            PHASE_IDS.len()
        );

        write_file(&source.join("untracked.txt"), b"dirty\n");
        let dirty_output = fixture_root.join("dirty-result.json");
        assert_eq!(
            run_inner(
                &workspace_root,
                Arguments {
                    plan: &plan_path,
                    source_root: &source,
                    root_preimage_root: &root_preimage,
                    predecessor_artifact_root: &predecessor,
                    git_executable: &git,
                    adapter_executable: &adapter,
                    output: &dirty_output,
                },
            ),
            Err(ReproInstallError::DirtySource)
        );
    }
}
