#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::{collections::BTreeMap, collections::BTreeSet, io::Write};

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CoverageSummary {
    pub functions_percent: f64,
    pub summary_lines_percent: f64,
    pub summary_regions_percent: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum ExecutableSource {
    Da,
    LfLh,
}

#[derive(Debug, Clone)]
pub struct LcovCoverage {
    pub executable_total: u64,
    pub executable_covered: u64,
    pub executable_percent: f64,
    pub executable_source: ExecutableSource,
    pub branch_total: u64,
    pub branch_covered: u64,
    pub branches_available: bool,
    pub branch_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct CoverageThresholds {
    pub fail_under_exec_lines: f64,
    pub fail_under_functions: f64,
    pub fail_under_regions: f64,
    pub fail_under_branches: f64,
    pub require_branches: bool,
}

#[derive(Debug, Clone)]
pub struct CoverageGateResult {
    pub pass: bool,
    pub fail_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CoverageGateReport {
    scope: String,
    thresholds: CoverageGateReportThresholds,
    measured: CoverageGateReportMeasured,
    counts: CoverageGateReportCounts,
    result: CoverageGateReportResult,
}

#[derive(Debug, Serialize)]
struct CoverageGateReportThresholds {
    executable_lines: f64,
    functions: f64,
    regions: f64,
    branches: f64,
    branches_required: bool,
}

#[derive(Debug, Serialize)]
struct CoverageGateReportMeasured {
    executable_lines_percent: f64,
    executable_lines_source: String,
    functions_percent: f64,
    branches_percent: Option<f64>,
    branches_available: bool,
    summary_lines_percent: f64,
    summary_regions_percent: f64,
}

#[derive(Debug, Serialize)]
struct CoverageGateReportCounts {
    executable_lines: CoverageCount,
    branches: CoverageCount,
}

#[derive(Debug, Serialize)]
struct CoverageCount {
    covered: u64,
    total: u64,
}

#[derive(Debug, Serialize)]
struct CoverageGateReportResult {
    pass: bool,
    fail_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovSummaryRoot {
    data: Vec<LlvmCovSummaryData>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovSummaryData {
    totals: LlvmCovSummaryTotals,
}

#[derive(Debug, Deserialize)]
struct LlvmCovSummaryTotals {
    functions: LlvmCovSummaryMetric,
    lines: LlvmCovSummaryMetric,
    regions: LlvmCovSummaryMetric,
}

#[derive(Debug, Deserialize)]
struct LlvmCovSummaryMetric {
    percent: f64,
}

#[derive(Debug, Deserialize)]
struct CoverageRequiredContract {
    required: CoverageRequiredList,
}

#[derive(Debug, Deserialize)]
struct CoverageRequiredList {
    crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceMembers,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembers {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PackageManifest {
    package: PackageSection,
}

#[derive(Debug, Deserialize)]
struct PackageSection {
    name: String,
}

#[derive(Debug, Deserialize, Default)]
struct CoverageProfilesFile {
    #[serde(default)]
    profiles: CoverageProfilesSection,
}

#[derive(Debug, Deserialize, Default)]
struct CoverageProfilesSection {
    #[serde(default)]
    default: CoverageProfileRaw,
    #[serde(default)]
    crates: BTreeMap<String, CoverageProfileRaw>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct CoverageProfileRaw {
    no_default_features: Option<bool>,
    features: Option<Vec<String>>,
    test_threads: Option<u32>,
}

#[derive(Debug, Clone)]
struct CoverageProfile {
    no_default_features: bool,
    features: Vec<String>,
    test_threads: Option<u32>,
}

pub fn read_summary(path: &Path) -> Result<CoverageSummary, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => return Err(format!("failed to read summary {}: {err}", path.display())),
    };
    let parsed: LlvmCovSummaryRoot = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => return Err(format!("failed to parse summary {}: {err}", path.display())),
    };
    let totals = match parsed.data.first() {
        Some(entry) => &entry.totals,
        None => return Err(format!("summary data is empty in {}", path.display())),
    };

    Ok(CoverageSummary {
        functions_percent: totals.functions.percent,
        summary_lines_percent: totals.lines.percent,
        summary_regions_percent: totals.regions.percent,
    })
}

fn read_required_crates(path: &Path) -> Result<Vec<String>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(format!(
                "failed to read required crates {}: {err}",
                path.display()
            ));
        }
    };
    let parsed: CoverageRequiredContract = match toml::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            return Err(format!(
                "failed to parse required crates {}: {err}",
                path.display()
            ));
        }
    };
    if parsed.required.crates.is_empty() {
        return Err("coverage required crates list must not be empty".to_string());
    }
    let mut seen = BTreeSet::new();
    for crate_name in &parsed.required.crates {
        if crate_name.trim().is_empty() {
            return Err("coverage required crates list includes an empty crate name".to_string());
        }
        if !seen.insert(crate_name.clone()) {
            return Err(format!(
                "coverage required crates list includes duplicate crate {crate_name}"
            ));
        }
    }
    Ok(parsed.required.crates)
}

fn read_workspace_crates(workspace_root: &Path) -> Result<Vec<String>, String> {
    let workspace_manifest = parse_toml::<WorkspaceManifest>(&workspace_root.join("Cargo.toml"))?;
    if workspace_manifest.workspace.members.is_empty() {
        return Err("workspace members list must not be empty".to_string());
    }
    let mut names = Vec::with_capacity(workspace_manifest.workspace.members.len());
    let mut seen = BTreeSet::new();
    for member in workspace_manifest.workspace.members {
        let package_manifest =
            parse_toml::<PackageManifest>(&workspace_root.join(member).join("Cargo.toml"))?;
        let package_name = package_manifest.package.name;
        if package_name.trim().is_empty() {
            return Err("workspace includes an empty package name".to_string());
        }
        if !seen.insert(package_name.clone()) {
            return Err(format!(
                "workspace includes duplicate package name {}",
                package_name
            ));
        }
        names.push(package_name);
    }
    Ok(names)
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => return Err(format!("failed to read {}: {err}", path.display())),
    };
    match toml::from_str::<T>(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(err) => Err(format!("failed to parse {}: {err}", path.display())),
    }
}

fn merge_coverage_profile(
    base: CoverageProfileRaw,
    overlay: CoverageProfileRaw,
) -> CoverageProfile {
    CoverageProfile {
        no_default_features: overlay
            .no_default_features
            .unwrap_or(base.no_default_features.unwrap_or(false)),
        features: overlay
            .features
            .unwrap_or_else(|| base.features.unwrap_or_default()),
        test_threads: overlay.test_threads.or(base.test_threads),
    }
}

fn read_coverage_profile(
    workspace_root: &Path,
    crate_name: &str,
) -> Result<CoverageProfile, String> {
    let path = workspace_root
        .join("contract")
        .join("coverage")
        .join("profiles.toml");
    if !path.exists() {
        return Ok(CoverageProfile {
            no_default_features: false,
            features: Vec::new(),
            test_threads: None,
        });
    }
    let parsed = parse_toml::<CoverageProfilesFile>(&path)?;
    let base = parsed.profiles.default;
    let overlay = parsed
        .profiles
        .crates
        .get(crate_name)
        .cloned()
        .unwrap_or_default();
    let resolved = merge_coverage_profile(base, overlay);
    if resolved
        .features
        .iter()
        .any(|feature| feature.trim().is_empty())
    {
        return Err(format!(
            "coverage profile for {crate_name} includes an empty feature value"
        ));
    }
    if resolved.test_threads == Some(0) {
        return Err(format!(
            "coverage profile for {crate_name} must set test_threads > 0"
        ));
    }
    Ok(resolved)
}

pub fn read_lcov(path: &Path) -> Result<LcovCoverage, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => return Err(format!("failed to read lcov {}: {err}", path.display())),
    };

    let mut da_total: u64 = 0;
    let mut da_covered: u64 = 0;
    let mut executable_total: u64 = 0;
    let mut executable_covered: u64 = 0;
    let mut branch_total_lcov: u64 = 0;
    let mut branch_covered_lcov: u64 = 0;
    let mut branch_total_brda: u64 = 0;
    let mut branch_covered_brda: u64 = 0;

    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("DA:") {
            let Some((_, hit)) = value.split_once(',') else {
                return Err(format!("invalid DA record in {}", path.display()));
            };
            let hit_count: u64 = match hit.parse() {
                Ok(hit_count) => hit_count,
                Err(err) => {
                    return Err(format!(
                        "invalid DA hit count `{hit}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            da_total = da_total.saturating_add(1);
            if hit_count > 0 {
                da_covered = da_covered.saturating_add(1);
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("LF:") {
            let parsed: u64 = match value.parse() {
                Ok(parsed) => parsed,
                Err(err) => {
                    return Err(format!(
                        "invalid LF value `{value}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            executable_total = executable_total.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("LH:") {
            let parsed: u64 = match value.parse() {
                Ok(parsed) => parsed,
                Err(err) => {
                    return Err(format!(
                        "invalid LH value `{value}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            executable_covered = executable_covered.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRF:") {
            let parsed: u64 = match value.parse() {
                Ok(parsed) => parsed,
                Err(err) => {
                    return Err(format!(
                        "invalid BRF value `{value}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            branch_total_lcov = branch_total_lcov.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRH:") {
            let parsed: u64 = match value.parse() {
                Ok(parsed) => parsed,
                Err(err) => {
                    return Err(format!(
                        "invalid BRH value `{value}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            branch_covered_lcov = branch_covered_lcov.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRDA:") {
            let fields = value.split(',').collect::<Vec<_>>();
            if fields.len() != 4 {
                return Err(format!("invalid BRDA record in {}", path.display()));
            }
            let taken = fields[3];
            if taken == "-" {
                continue;
            }
            let hit_count: u64 = match taken.parse() {
                Ok(hit_count) => hit_count,
                Err(err) => {
                    return Err(format!(
                        "invalid BRDA taken count `{taken}` in {}: {err}",
                        path.display()
                    ));
                }
            };
            branch_total_brda = branch_total_brda.saturating_add(1);
            if hit_count > 0 {
                branch_covered_brda = branch_covered_brda.saturating_add(1);
            }
        }
    }

    let mut executable_source = ExecutableSource::Da;
    let mut executable_percent = 100.0_f64;

    if da_total > 0 {
        executable_total = da_total;
        executable_covered = da_covered;
        executable_percent = (da_covered as f64 / da_total as f64) * 100.0_f64;
    } else if executable_total > 0 {
        executable_source = ExecutableSource::LfLh;
        executable_percent = (executable_covered as f64 / executable_total as f64) * 100.0_f64;
    }

    let (branch_total, branch_covered) = if branch_total_brda > 0 {
        (branch_total_brda, branch_covered_brda)
    } else {
        (branch_total_lcov, branch_covered_lcov)
    };
    let branches_available = branch_total > 0;
    let branch_percent = if branches_available {
        Some((branch_covered as f64 / branch_total as f64) * 100.0_f64)
    } else {
        None
    };

    Ok(LcovCoverage {
        executable_total,
        executable_covered,
        executable_percent,
        executable_source,
        branch_total,
        branch_covered,
        branches_available,
        branch_percent,
    })
}

pub fn evaluate_gate(
    summary: &CoverageSummary,
    lcov: &LcovCoverage,
    thresholds: CoverageThresholds,
) -> CoverageGateResult {
    let exec_ok = lcov.executable_percent >= thresholds.fail_under_exec_lines;
    let functions_ok = summary.functions_percent >= thresholds.fail_under_functions;
    let regions_ok = summary.summary_regions_percent >= thresholds.fail_under_regions;
    let branch_presence_ok = !thresholds.require_branches || lcov.branches_available;
    let branch_ok = lcov
        .branch_percent
        .is_none_or(|branch_percent| branch_percent >= thresholds.fail_under_branches);

    let pass = [
        exec_ok,
        functions_ok,
        regions_ok,
        branch_presence_ok,
        branch_ok,
    ]
    .into_iter()
    .all(|flag| flag);
    let mut fail_reasons: Vec<String> = Vec::new();

    if !exec_ok {
        fail_reasons.push(format!(
            "executable_lines={:.6} < {:.6}",
            lcov.executable_percent, thresholds.fail_under_exec_lines
        ));
    }

    if !functions_ok {
        fail_reasons.push(format!(
            "functions={:.6} < {:.6}",
            summary.functions_percent, thresholds.fail_under_functions
        ));
    }

    if !regions_ok {
        fail_reasons.push(format!(
            "regions={:.6} < {:.6}",
            summary.summary_regions_percent, thresholds.fail_under_regions
        ));
    }

    if thresholds.require_branches && !lcov.branches_available {
        fail_reasons.push("branches=unavailable".to_string());
    }

    if lcov.branches_available && !branch_ok {
        fail_reasons.push(format!(
            "branches={:.6} < {:.6}",
            lcov.branch_percent.unwrap_or(0.0),
            thresholds.fail_under_branches
        ));
    }

    CoverageGateResult { pass, fail_reasons }
}

fn executable_source_label(source: ExecutableSource) -> &'static str {
    match source {
        ExecutableSource::Da => "da",
        ExecutableSource::LfLh => "lf_lh",
    }
}

fn parse_string_arg(args: &[String], name: &str) -> Result<String, String> {
    let flag = format!("--{name}");
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == flag {
            let Some(value) = args.get(index + 1) else {
                return Err(format!("missing value for --{name}"));
            };
            return Ok(value.clone());
        }
        index += 1;
    }
    Err(format!("missing --{name}"))
}

fn parse_optional_string_arg(args: &[String], name: &str) -> Option<String> {
    let flag = format!("--{name}");
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == flag {
            return args.get(index + 1).cloned();
        }
        index += 1;
    }
    None
}

fn parse_f64_arg(args: &[String], name: &str, default: f64) -> Result<f64, String> {
    if let Some(raw) = parse_optional_string_arg(args, name) {
        return raw
            .parse::<f64>()
            .map_err(|err| format!("invalid --{name} value `{raw}`: {err}"));
    }
    Ok(default)
}

fn parse_optional_u32_arg(args: &[String], name: &str) -> Result<Option<u32>, String> {
    if let Some(raw) = parse_optional_string_arg(args, name) {
        let parsed = raw
            .parse::<u32>()
            .map_err(|err| format!("invalid --{name} value `{raw}`: {err}"))?;
        return Ok(Some(parsed));
    }
    Ok(None)
}

fn parse_bool_flag(args: &[String], name: &str) -> bool {
    let flag = format!("--{name}");
    args.iter().any(|arg| arg == &flag)
}

fn workspace_root_with_override(override_root: Option<&str>) -> PathBuf {
    if let Some(raw) = override_root {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir.parent().unwrap_or(manifest_dir);
    let root = crates_dir.parent().unwrap_or(crates_dir);
    root.to_path_buf()
}

fn workspace_root() -> PathBuf {
    let override_root = std::env::var("RADROOTS_WORKSPACE_ROOT").ok();
    workspace_root_with_override(override_root.as_deref())
}

fn run_command(mut command: Command, name: &str) -> Result<(), String> {
    let status = match command.status() {
        Ok(status) => status,
        Err(err) => return Err(format!("failed to run {name}: {err}")),
    };
    if !status.success() {
        return Err(format!("{name} failed with status {status}"));
    }
    Ok(())
}

fn apply_coverage_profile_flags(command: &mut Command, profile: &CoverageProfile) {
    if profile.no_default_features {
        command.arg("--no-default-features");
    }
    if !profile.features.is_empty() {
        command.arg("--features").arg(profile.features.join(","));
    }
}

fn coverage_cargo_command_with_override(override_binary: Option<&str>) -> Command {
    if let Some(binary) = override_binary {
        return Command::new(binary);
    }

    let mut cmd = Command::new("rustup");
    cmd.arg("run").arg("nightly").arg("cargo");
    cmd
}

fn coverage_cargo_command() -> Command {
    let override_binary = std::env::var("RADROOTS_COVERAGE_CARGO")
        .ok()
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    coverage_cargo_command_with_override(override_binary.as_deref())
}

fn coverage_llvm_cov_command() -> Command {
    let mut cmd = coverage_cargo_command();
    cmd.arg("llvm-cov");
    cmd
}

fn run_crate_with_runner_at_root(
    args: &[String],
    workspace_root: &Path,
    runner: &mut dyn FnMut(Command, &str) -> Result<(), String>,
) -> Result<(), String> {
    let crate_name = parse_string_arg(args, "crate")?;
    let profile = read_coverage_profile(workspace_root, &crate_name)?;
    let out_dir = if let Some(raw) = parse_optional_string_arg(args, "out") {
        PathBuf::from(raw)
    } else {
        workspace_root
            .join("target")
            .join("coverage")
            .join(crate_name.replace('-', "_"))
    };
    let test_threads = parse_optional_u32_arg(args, "test-threads")?
        .or(profile.test_threads)
        .unwrap_or(1);

    if let Err(err) = fs::create_dir_all(&out_dir) {
        return Err(format!("failed to create {}: {err}", out_dir.display()));
    }

    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("clean")
                .arg("--workspace")
                .current_dir(workspace_root);
            cmd
        },
        "cargo llvm-cov clean --workspace",
    )?;

    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("-p").arg(&crate_name);
            apply_coverage_profile_flags(&mut cmd, &profile);
            cmd.arg("--no-report")
                .arg("--branch")
                .arg("--")
                .arg(format!("--test-threads={test_threads}"))
                .current_dir(workspace_root);
            cmd
        },
        "cargo llvm-cov --no-report",
    )?;

    let summary_path = out_dir.join("coverage-summary.json");
    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("report").arg("-p").arg(&crate_name);
            cmd.arg("--json")
                .arg("--summary-only")
                .arg("--branch")
                .arg("--output-path")
                .arg(&summary_path)
                .current_dir(workspace_root);
            cmd
        },
        "cargo llvm-cov report --json --summary-only",
    )?;

    let lcov_path = out_dir.join("coverage-lcov.info");
    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("report").arg("-p").arg(&crate_name);
            cmd.arg("--lcov")
                .arg("--branch")
                .arg("--output-path")
                .arg(&lcov_path)
                .current_dir(workspace_root);
            cmd
        },
        "cargo llvm-cov report --lcov",
    )?;

    eprintln!("coverage summary: {}", summary_path.display());
    eprintln!("coverage lcov: {}", lcov_path.display());
    Ok(())
}

fn run_crate_with_runner(
    args: &[String],
    runner: &mut dyn FnMut(Command, &str) -> Result<(), String>,
) -> Result<(), String> {
    let root = workspace_root();
    run_crate_with_runner_at_root(args, &root, runner)
}

fn run_crate(args: &[String]) -> Result<(), String> {
    let mut runner = run_command;
    run_crate_with_runner(args, &mut runner)
}

fn report_gate(args: &[String]) -> Result<(), String> {
    let scope = parse_string_arg(args, "scope")?;
    let summary_path = PathBuf::from(parse_string_arg(args, "summary")?);
    let lcov_path = PathBuf::from(parse_string_arg(args, "lcov")?);
    let out_path = PathBuf::from(parse_string_arg(args, "out")?);
    let thresholds = CoverageThresholds {
        fail_under_exec_lines: parse_f64_arg(args, "fail-under-exec-lines", 100.0)?,
        fail_under_functions: parse_f64_arg(args, "fail-under-functions", 100.0)?,
        fail_under_regions: parse_f64_arg(args, "fail-under-regions", 100.0)?,
        fail_under_branches: parse_f64_arg(args, "fail-under-branches", 100.0)?,
        require_branches: parse_bool_flag(args, "require-branches"),
    };

    let summary = read_summary(&summary_path)?;
    let lcov = read_lcov(&lcov_path)?;
    let gate = evaluate_gate(&summary, &lcov, thresholds);

    let report = CoverageGateReport {
        scope: scope.clone(),
        thresholds: CoverageGateReportThresholds {
            executable_lines: thresholds.fail_under_exec_lines,
            functions: thresholds.fail_under_functions,
            regions: thresholds.fail_under_regions,
            branches: thresholds.fail_under_branches,
            branches_required: thresholds.require_branches,
        },
        measured: CoverageGateReportMeasured {
            executable_lines_percent: lcov.executable_percent,
            executable_lines_source: executable_source_label(lcov.executable_source).to_string(),
            functions_percent: summary.functions_percent,
            branches_percent: lcov.branch_percent,
            branches_available: lcov.branches_available,
            summary_lines_percent: summary.summary_lines_percent,
            summary_regions_percent: summary.summary_regions_percent,
        },
        counts: CoverageGateReportCounts {
            executable_lines: CoverageCount {
                covered: lcov.executable_covered,
                total: lcov.executable_total,
            },
            branches: CoverageCount {
                covered: lcov.branch_covered,
                total: lcov.branch_total,
            },
        },
        result: CoverageGateReportResult {
            pass: gate.pass,
            fail_reasons: gate.fail_reasons.clone(),
        },
    };

    let json = serde_json::to_string_pretty(&report)
        .expect("serializing coverage gate report should succeed");
    if let Err(err) = fs::write(&out_path, format!("{json}\n")) {
        return Err(format!("failed to write {}: {err}", out_path.display()));
    }

    if lcov.branches_available {
        eprintln!(
            "{} coverage: executable_lines={:.6} functions={:.6} regions={:.6} branches={:.6}",
            scope,
            lcov.executable_percent,
            summary.functions_percent,
            summary.summary_regions_percent,
            lcov.branch_percent.unwrap_or(0.0)
        );
    } else {
        eprintln!(
            "{} coverage: executable_lines={:.6} functions={:.6} regions={:.6} branches=unavailable",
            scope,
            lcov.executable_percent,
            summary.functions_percent,
            summary.summary_regions_percent
        );
    }

    eprintln!(
        "{} summary (informational): lines={:.6} regions={:.6}",
        scope, summary.summary_lines_percent, summary.summary_regions_percent
    );

    if !gate.pass {
        for reason in &gate.fail_reasons {
            eprintln!("{scope} gate fail: {reason}");
        }
        return Err("coverage gate failed".to_string());
    }

    Ok(())
}

fn list_required_crates_with_root(root: &Path, writer: &mut dyn Write) -> Result<(), String> {
    let required_path = root
        .join("contract")
        .join("coverage")
        .join("required-crates.toml");
    let crates = read_required_crates(&required_path)?;
    write_crate_names_output(writer, crates, "required crates")
}

fn list_required_crates() -> Result<(), String> {
    let root = workspace_root();
    let mut stdout = std::io::stdout().lock();
    list_required_crates_with_root(&root, &mut stdout)
}

fn list_workspace_crates_with_root(root: &Path, writer: &mut dyn Write) -> Result<(), String> {
    let crates = read_workspace_crates(&root)?;
    write_crate_names_output(writer, crates, "workspace crates")
}

fn list_workspace_crates() -> Result<(), String> {
    let root = workspace_root();
    let mut stdout = std::io::stdout().lock();
    list_workspace_crates_with_root(&root, &mut stdout)
}

fn write_crate_names_output(
    writer: &mut dyn Write,
    crates: Vec<String>,
    label: &str,
) -> Result<(), String> {
    for crate_name in crates {
        if let Err(err) = writeln!(writer, "{crate_name}") {
            return Err(format!("failed to write {label} output: {err}"));
        }
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") => Ok(()),
        Some("run-crate") => run_crate(&args[1..]),
        Some("report") => report_gate(&args[1..]),
        Some("required-crates") => list_required_crates(),
        Some("workspace-crates") => list_workspace_crates(),
        Some(_) => Err("unknown sdk coverage subcommand".to_string()),
        None => Err("missing sdk coverage subcommand".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{self, Write};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_coverage_{prefix}_{ns}.tmp"))
    }

    fn temp_dir_path(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_coverage_{prefix}_{ns}"))
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("forced write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn reads_summary_totals_from_llvm_cov_json() {
        let path = temp_file_path("summary");
        fs::write(
            &path,
            r#"{
  "data": [
    {
      "totals": {
        "functions": {"percent": 91.25},
        "lines": {"percent": 88.5},
        "regions": {"percent": 86.75}
      }
    }
  ]
}"#,
        )
        .expect("write summary");

        let summary = read_summary(&path).expect("parse summary");
        assert_eq!(summary.functions_percent, 91.25);
        assert_eq!(summary.summary_lines_percent, 88.5);
        assert_eq!(summary.summary_regions_percent, 86.75);

        fs::remove_file(path).expect("remove summary");
    }

    #[test]
    fn read_summary_reports_read_and_parse_errors() {
        let missing = temp_file_path("summary_missing");
        let read_err = read_summary(&missing).expect_err("missing summary should fail");
        assert!(read_err.contains("failed to read summary"));

        let invalid = temp_file_path("summary_invalid");
        write_file(&invalid, "{not-json");
        let parse_err = read_summary(&invalid).expect_err("invalid summary should fail");
        assert!(parse_err.contains("failed to parse summary"));
        fs::remove_file(invalid).expect("remove invalid summary");
    }

    #[test]
    fn read_summary_reports_empty_data_error() {
        let path = temp_file_path("summary_empty_data");
        write_file(&path, r#"{"data":[]}"#);
        let err = read_summary(&path).expect_err("summary without data should fail");
        assert!(err.contains("summary data is empty"));
        fs::remove_file(path).expect("remove empty summary");
    }

    #[test]
    fn reads_lcov_da_and_branch_metrics() {
        let path = temp_file_path("lcov");
        fs::write(
            &path,
            "DA:1,1\nDA:2,0\nDA:3,1\nBRDA:1,0,0,1\nBRDA:1,0,1,0\nBRDA:2,0,0,3\nBRDA:2,0,1,-\n",
        )
        .expect("write lcov");

        let lcov = read_lcov(&path).expect("parse lcov");
        assert_eq!(lcov.executable_total, 3);
        assert_eq!(lcov.executable_covered, 2);
        assert!(lcov.branches_available);
        assert_eq!(lcov.branch_total, 3);
        assert_eq!(lcov.branch_covered, 2);
        assert_eq!(lcov.branch_percent, Some(66.66666666666666));

        fs::remove_file(path).expect("remove lcov");
    }

    #[test]
    fn reads_lcov_branch_metrics_from_brf_brh_when_brda_missing() {
        let path = temp_file_path("lcov_fallback");
        fs::write(&path, "DA:1,1\nDA:2,1\nBRF:4\nBRH:3\n").expect("write lcov");

        let lcov = read_lcov(&path).expect("parse lcov");
        assert!(lcov.branches_available);
        assert_eq!(lcov.branch_total, 4);
        assert_eq!(lcov.branch_covered, 3);
        assert_eq!(lcov.branch_percent, Some(75.0));

        fs::remove_file(path).expect("remove lcov");
    }

    #[test]
    fn gate_fails_when_branch_data_is_required_but_missing() {
        let summary = CoverageSummary {
            functions_percent: 100.0,
            summary_lines_percent: 100.0,
            summary_regions_percent: 100.0,
        };
        let lcov = LcovCoverage {
            executable_total: 10,
            executable_covered: 10,
            executable_percent: 100.0,
            executable_source: ExecutableSource::Da,
            branch_total: 0,
            branch_covered: 0,
            branches_available: false,
            branch_percent: None,
        };
        let thresholds = CoverageThresholds {
            fail_under_exec_lines: 100.0,
            fail_under_functions: 100.0,
            fail_under_regions: 100.0,
            fail_under_branches: 100.0,
            require_branches: true,
        };

        let gate = evaluate_gate(&summary, &lcov, thresholds);
        assert!(!gate.pass);
        assert!(
            gate.fail_reasons
                .iter()
                .any(|reason| reason == "branches=unavailable")
        );
    }

    #[test]
    fn reads_required_crates_and_rejects_duplicates() {
        let path = temp_file_path("required_crates");
        fs::write(&path, "[required]\ncrates = [\"a\", \"b\"]\n").expect("write required crates");
        let crates = read_required_crates(&path).expect("parse required crates");
        assert_eq!(crates, vec!["a".to_string(), "b".to_string()]);
        fs::remove_file(&path).expect("remove required crates");

        let dup_path = temp_file_path("required_crates_dup");
        fs::write(&dup_path, "[required]\ncrates = [\"a\", \"a\"]\n")
            .expect("write dup required crates");
        let err = read_required_crates(&dup_path).expect_err("duplicate required crates");
        assert!(err.contains("duplicate crate a"));
        fs::remove_file(dup_path).expect("remove dup required crates");
    }

    #[test]
    fn read_required_crates_reports_read_and_parse_errors() {
        let missing = temp_file_path("required_missing");
        let read_err = read_required_crates(&missing).expect_err("missing required file");
        assert!(read_err.contains("failed to read required crates"));

        let invalid = temp_file_path("required_invalid");
        write_file(&invalid, "not = [toml");
        let parse_err = read_required_crates(&invalid).expect_err("invalid required file");
        assert!(parse_err.contains("failed to parse required crates"));
        fs::remove_file(invalid).expect("remove invalid required file");
    }

    #[test]
    fn reads_workspace_crates_and_contains_xtask() {
        let root = workspace_root();
        let crates = read_workspace_crates(&root).expect("workspace crates");
        assert!(!crates.is_empty());
        assert!(crates.iter().any(|crate_name| crate_name == "xtask"));
    }

    #[test]
    fn coverage_profiles_default_when_contract_file_is_missing() {
        let root = temp_dir_path("profile_missing");
        fs::create_dir_all(&root).expect("create root");
        let profile = read_coverage_profile(&root, "radroots-app-core").expect("read profile");
        assert!(!profile.no_default_features);
        assert!(profile.features.is_empty());
        assert_eq!(profile.test_threads, None);
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_merge_defaults_and_crate_overrides() {
        let root = temp_dir_path("profile_merge");
        let coverage_dir = root.join("contract").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.default]
no_default_features = false
features = ["std"]
test_threads = 2

[profiles.crates."radroots-app-core"]
no_default_features = true
features = ["rt"]
"#,
        )
        .expect("write profiles");

        let app_profile = read_coverage_profile(&root, "radroots-app-core").expect("app profile");
        assert!(app_profile.no_default_features);
        assert_eq!(app_profile.features, vec!["rt".to_string()]);
        assert_eq!(app_profile.test_threads, Some(2));

        let other_profile = read_coverage_profile(&root, "radroots-types").expect("other profile");
        assert!(!other_profile.no_default_features);
        assert_eq!(other_profile.features, vec!["std".to_string()]);
        assert_eq!(other_profile.test_threads, Some(2));

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_accept_positive_test_threads() {
        let root = temp_dir_path("profile_positive_threads");
        let coverage_dir = root.join("contract").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots-app-core"]
test_threads = 4
"#,
        )
        .expect("write profiles");
        let profile = read_coverage_profile(&root, "radroots-app-core")
            .expect("valid positive thread profile");
        assert_eq!(profile.test_threads, Some(4));
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_invalid_feature_and_thread_values() {
        let root = temp_dir_path("profile_invalid");
        let coverage_dir = root.join("contract").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots-app-core"]
features = [""]
test_threads = 0
"#,
        )
        .expect("write profiles");

        let err = read_coverage_profile(&root, "radroots-app-core").expect_err("invalid profile");
        assert!(
            err.contains("empty feature value"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_invalid_toml() {
        let root = temp_dir_path("profile_invalid_toml");
        let coverage_dir = root.join("contract").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(coverage_dir.join("profiles.toml"), "[profiles.default\n")
            .expect("write invalid profiles");
        let err = read_coverage_profile(&root, "radroots-app-core").expect_err("invalid toml");
        assert!(err.contains("failed to parse"));
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_zero_test_threads_without_feature_error() {
        let root = temp_dir_path("profile_invalid_threads");
        let coverage_dir = root.join("contract").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots-app-core"]
test_threads = 0
"#,
        )
        .expect("write profiles");

        let err =
            read_coverage_profile(&root, "radroots-app-core").expect_err("invalid thread count");
        assert!(err.contains("test_threads > 0"));

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn parse_helpers_cover_success_and_error_paths() {
        let args = vec![
            "--scope".to_string(),
            "crate-a".to_string(),
            "--value".to_string(),
            "3.5".to_string(),
            "--threads".to_string(),
            "4".to_string(),
            "--flag".to_string(),
        ];
        assert_eq!(
            parse_string_arg(&args, "scope").expect("scope value"),
            "crate-a".to_string()
        );
        assert_eq!(
            parse_optional_string_arg(&args, "scope").expect("optional scope"),
            "crate-a".to_string()
        );
        assert_eq!(parse_f64_arg(&args, "value", 1.0).expect("f64 value"), 3.5);
        assert_eq!(
            parse_optional_u32_arg(&args, "threads").expect("u32 value"),
            Some(4)
        );
        assert!(parse_bool_flag(&args, "flag"));
        assert_eq!(parse_optional_string_arg(&args, "missing"), None);
        assert_eq!(
            parse_f64_arg(&args, "missing", 2.25).expect("default f64"),
            2.25
        );
        assert_eq!(
            parse_optional_u32_arg(&args, "missing").expect("missing u32"),
            None
        );

        let missing_err = parse_string_arg(&args, "absent").expect_err("missing arg");
        assert!(missing_err.contains("missing --absent"));

        let missing_value = vec!["--scope".to_string()];
        let missing_value_err =
            parse_string_arg(&missing_value, "scope").expect_err("missing arg value");
        assert!(missing_value_err.contains("missing value for --scope"));

        let invalid_f64 = vec!["--value".to_string(), "bad".to_string()];
        let invalid_f64_err = parse_f64_arg(&invalid_f64, "value", 1.0).expect_err("invalid f64");
        assert!(invalid_f64_err.contains("invalid --value value"));

        let invalid_u32 = vec!["--threads".to_string(), "bad".to_string()];
        let invalid_u32_err =
            parse_optional_u32_arg(&invalid_u32, "threads").expect_err("invalid u32");
        assert!(invalid_u32_err.contains("invalid --threads value"));
    }

    #[test]
    fn executable_source_labels_cover_all_variants() {
        assert_eq!(executable_source_label(ExecutableSource::Da), "da");
        assert_eq!(executable_source_label(ExecutableSource::LfLh), "lf_lh");
    }

    #[test]
    fn read_required_crates_rejects_empty_and_blank_entries() {
        let empty_path = temp_file_path("required_empty");
        write_file(&empty_path, "[required]\ncrates = []\n");
        let empty_err = read_required_crates(&empty_path).expect_err("empty required list");
        assert!(empty_err.contains("must not be empty"));
        fs::remove_file(&empty_path).expect("remove empty required file");

        let blank_path = temp_file_path("required_blank");
        write_file(&blank_path, "[required]\ncrates = [\"a\", \" \"]\n");
        let blank_err = read_required_crates(&blank_path).expect_err("blank crate name");
        assert!(blank_err.contains("empty crate name"));
        fs::remove_file(&blank_path).expect("remove blank required file");
    }

    #[test]
    fn read_workspace_crates_rejects_invalid_workspace_shapes() {
        let root_empty = temp_dir_path("workspace_empty_members");
        write_file(
            &root_empty.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        );
        let empty_err = read_workspace_crates(&root_empty).expect_err("empty workspace members");
        assert!(empty_err.contains("must not be empty"));
        fs::remove_dir_all(&root_empty).expect("remove empty members root");

        let root_blank = temp_dir_path("workspace_blank_package_name");
        write_file(
            &root_blank.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        write_file(
            &root_blank.join("crates").join("a").join("Cargo.toml"),
            "[package]\nname = \"\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let blank_err = read_workspace_crates(&root_blank).expect_err("blank package name");
        assert!(blank_err.contains("empty package name"));
        fs::remove_dir_all(&root_blank).expect("remove blank package root");

        let root_duplicate = temp_dir_path("workspace_duplicate_package");
        write_file(
            &root_duplicate.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
        );
        let package_manifest =
            "[package]\nname = \"duplicate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
        write_file(
            &root_duplicate.join("crates").join("a").join("Cargo.toml"),
            package_manifest,
        );
        write_file(
            &root_duplicate.join("crates").join("b").join("Cargo.toml"),
            package_manifest,
        );
        let dup_err = read_workspace_crates(&root_duplicate).expect_err("duplicate package names");
        assert!(dup_err.contains("duplicate package name"));
        fs::remove_dir_all(&root_duplicate).expect("remove duplicate package root");

        let root_parse = temp_dir_path("workspace_parse_error");
        write_file(
            &root_parse.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        write_file(
            &root_parse.join("crates").join("a").join("Cargo.toml"),
            "[package",
        );
        let parse_err = read_workspace_crates(&root_parse).expect_err("invalid package manifest");
        assert!(parse_err.contains("failed to parse"));
        fs::remove_dir_all(&root_parse).expect("remove parse package root");
    }

    #[test]
    fn parse_toml_reports_read_and_parse_errors() {
        let missing = temp_file_path("parse_toml_missing");
        let read_err =
            parse_toml::<CoverageRequiredContract>(&missing).expect_err("missing file should fail");
        assert!(read_err.contains("failed to read"));

        let invalid = temp_file_path("parse_toml_invalid");
        write_file(&invalid, "[required]\ncrates = [\n");
        let parse_err =
            parse_toml::<CoverageRequiredContract>(&invalid).expect_err("invalid toml should fail");
        assert!(parse_err.contains("failed to parse"));
        fs::remove_file(invalid).expect("remove invalid toml");
    }

    #[test]
    fn parse_toml_parses_valid_coverage_required_contract() {
        let valid = temp_file_path("parse_toml_valid");
        write_file(&valid, "[required]\ncrates = [\"radroots-core\"]\n");
        let parsed = parse_toml::<CoverageRequiredContract>(&valid).expect("valid toml");
        assert_eq!(parsed.required.crates, vec!["radroots-core".to_string()]);
        fs::remove_file(valid).expect("remove valid toml");
    }

    #[test]
    fn read_lcov_rejects_invalid_records() {
        let cases = vec![
            ("invalid_da_shape", "DA:1\n", "invalid DA record"),
            ("invalid_da_hits", "DA:1,bad\n", "invalid DA hit count"),
            ("invalid_lf", "LF:bad\n", "invalid LF value"),
            ("invalid_lh", "LH:bad\n", "invalid LH value"),
            ("invalid_brf", "BRF:bad\n", "invalid BRF value"),
            ("invalid_brh", "BRH:bad\n", "invalid BRH value"),
            ("invalid_brda_shape", "BRDA:1,0,0\n", "invalid BRDA record"),
            (
                "invalid_brda_taken",
                "BRDA:1,0,0,bad\n",
                "invalid BRDA taken count",
            ),
            (
                "invalid_brda_extra",
                "BRDA:1,0,0,1,extra\n",
                "invalid BRDA record",
            ),
        ];
        for (prefix, raw, expected) in cases {
            let path = temp_file_path(prefix);
            write_file(&path, raw);
            let err = read_lcov(&path).expect_err("invalid lcov record");
            assert!(
                err.contains(expected),
                "expected `{expected}` in `{err}` for case {prefix}"
            );
            fs::remove_file(path).expect("remove invalid lcov file");
        }
    }

    #[test]
    fn read_lcov_reports_read_error() {
        let missing = temp_file_path("lcov_missing");
        let err = read_lcov(&missing).expect_err("missing lcov should fail");
        assert!(err.contains("failed to read lcov"));
    }

    #[test]
    fn read_lcov_uses_lf_lh_when_da_is_missing_and_branches_absent() {
        let path = temp_file_path("lcov_lf_lh");
        fs::write(&path, "LF:4\nLH:3\n").expect("write lcov");
        let parsed = read_lcov(&path).expect("parse lcov");
        assert_eq!(executable_source_label(parsed.executable_source), "lf_lh");
        assert_eq!(parsed.executable_total, 4);
        assert_eq!(parsed.executable_covered, 3);
        assert_eq!(parsed.executable_percent, 75.0);
        assert!(!parsed.branches_available);
        assert_eq!(parsed.branch_percent, None);
        fs::remove_file(path).expect("remove lcov");
    }

    #[test]
    fn read_lcov_defaults_to_full_when_no_line_records_exist() {
        let path = temp_file_path("lcov_empty");
        write_file(&path, "TN:probe\n");
        let parsed = read_lcov(&path).expect("parse lcov");
        assert_eq!(parsed.executable_total, 0);
        assert_eq!(parsed.executable_covered, 0);
        assert_eq!(parsed.executable_percent, 100.0);
        assert!(!parsed.branches_available);
        assert_eq!(parsed.branch_percent, None);
        fs::remove_file(path).expect("remove lcov");
    }

    #[test]
    fn evaluate_gate_collects_all_failure_reasons() {
        let summary = CoverageSummary {
            functions_percent: 40.0,
            summary_lines_percent: 50.0,
            summary_regions_percent: 60.0,
        };
        let lcov = LcovCoverage {
            executable_total: 20,
            executable_covered: 10,
            executable_percent: 50.0,
            executable_source: ExecutableSource::Da,
            branch_total: 10,
            branch_covered: 3,
            branches_available: true,
            branch_percent: Some(30.0),
        };
        let thresholds = CoverageThresholds {
            fail_under_exec_lines: 90.0,
            fail_under_functions: 90.0,
            fail_under_regions: 90.0,
            fail_under_branches: 90.0,
            require_branches: true,
        };

        let gate = evaluate_gate(&summary, &lcov, thresholds);
        assert!(!gate.pass);
        assert!(
            gate.fail_reasons
                .iter()
                .any(|reason| reason.contains("executable_lines"))
        );
        assert!(
            gate.fail_reasons
                .iter()
                .any(|reason| reason.contains("functions"))
        );
        assert!(
            gate.fail_reasons
                .iter()
                .any(|reason| reason.contains("regions"))
        );
        assert!(
            gate.fail_reasons
                .iter()
                .any(|reason| reason.contains("branches"))
        );
    }

    #[test]
    fn run_command_covers_success_and_failure() {
        let mut ok = Command::new("sh");
        ok.arg("-c").arg("exit 0");
        run_command(ok, "shell ok").expect("run ok command");

        let mut fail = Command::new("sh");
        fail.arg("-c").arg("exit 9");
        let err = run_command(fail, "shell fail").expect_err("run failing command");
        assert!(err.contains("shell fail failed with status"));

        let missing = Command::new("/definitely/not/a/real/command");
        let err = run_command(missing, "shell missing").expect_err("missing command");
        assert!(err.contains("failed to run shell missing"));
    }

    #[test]
    fn apply_coverage_profile_flags_writes_expected_args() {
        let profile = CoverageProfile {
            no_default_features: true,
            features: vec!["std".to_string(), "serde".to_string()],
            test_threads: Some(2),
        };
        let mut command = Command::new("cargo");
        apply_coverage_profile_flags(&mut command, &profile);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "--no-default-features".to_string(),
                "--features".to_string(),
                "std,serde".to_string()
            ]
        );
    }

    #[test]
    fn run_crate_with_runner_builds_all_command_steps() {
        let out = temp_dir_path("run_crate_runner");
        let args = vec![
            "--crate".to_string(),
            "radroots-core".to_string(),
            "--out".to_string(),
            out.display().to_string(),
            "--test-threads".to_string(),
            "3".to_string(),
        ];
        let mut names = Vec::new();
        let mut runner = |cmd: Command, name: &str| {
            names.push(name.to_string());
            let rendered = cmd
                .get_args()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!rendered.is_empty());
            Ok(())
        };
        run_crate_with_runner(&args, &mut runner).expect("run crate with stub runner");
        assert_eq!(
            names,
            vec![
                "cargo llvm-cov clean --workspace".to_string(),
                "cargo llvm-cov --no-report".to_string(),
                "cargo llvm-cov report --json --summary-only".to_string(),
                "cargo llvm-cov report --lcov".to_string(),
            ]
        );
        fs::remove_dir_all(out).expect("remove run crate output dir");
    }

    #[test]
    fn coverage_cargo_command_defaults_to_rustup_nightly() {
        let cmd = coverage_cargo_command_with_override(None);
        let args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(cmd.get_program().to_string_lossy(), "rustup");
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "nightly".to_string(),
                "cargo".to_string()
            ]
        );
    }

    #[test]
    fn coverage_cargo_command_uses_override_binary_when_present() {
        let cmd = coverage_cargo_command_with_override(Some("/tmp/nightly-cargo"));
        let args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(cmd.get_program().to_string_lossy(), "/tmp/nightly-cargo");
        assert!(args.is_empty());
    }

    #[test]
    fn workspace_root_override_takes_precedence() {
        let root = workspace_root_with_override(Some("/tmp/radroots-coverage-root"));
        assert_eq!(root, PathBuf::from("/tmp/radroots-coverage-root"));

        let fallback = workspace_root_with_override(Some(""));
        assert!(fallback.join("Cargo.toml").exists());
    }

    #[test]
    fn run_crate_with_runner_uses_default_output_dir_when_out_is_missing() {
        let args = vec!["--crate".to_string(), "radroots-core".to_string()];
        let mut output_path_seen = false;
        let mut runner = |cmd: Command, _: &str| {
            let rendered = cmd
                .get_args()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            if rendered
                .iter()
                .any(|arg| arg.ends_with("coverage-summary.json"))
                || rendered
                    .iter()
                    .any(|arg| arg.ends_with("coverage-lcov.info"))
            {
                output_path_seen = true;
            }
            Ok(())
        };
        run_crate_with_runner(&args, &mut runner).expect("run crate with default out");
        assert!(output_path_seen);
    }

    #[test]
    fn run_crate_with_runner_propagates_runner_failures() {
        let out = temp_dir_path("run_crate_runner_fail");
        let args = vec![
            "--crate".to_string(),
            "radroots-core".to_string(),
            "--out".to_string(),
            out.display().to_string(),
        ];
        let mut runner = |_: Command, _: &str| Err("runner failed".to_string());
        let err =
            run_crate_with_runner(&args, &mut runner).expect_err("runner failure should bubble up");
        assert_eq!(err, "runner failed".to_string());
        fs::remove_dir_all(out).expect("remove run crate failure output dir");
        let root = temp_dir_path("run_crate_create_out_error");
        write_file(&root.join("blocker"), "x");
        let args = vec![
            "--crate".to_string(),
            "radroots-core".to_string(),
            "--out".to_string(),
            root.join("blocker").join("nested").display().to_string(),
        ];
        let mut runner = run_command;
        let err = run_crate_with_runner(&args, &mut runner)
            .expect_err("output dir create error should fail");
        assert!(err.contains("failed to create"));
        fs::remove_dir_all(root).expect("remove run crate create error root");
    }

    #[test]
    fn run_crate_wrapper_returns_missing_crate_error_without_running_commands() {
        let err = run_crate(&[]).expect_err("missing crate flag");
        assert!(err.contains("missing --crate"));
    }

    #[test]
    fn run_crate_with_runner_at_root_covers_profile_and_runner_error_paths() {
        let profile_root = temp_dir_path("run_crate_profile_invalid");
        write_file(
            &profile_root
                .join("contract")
                .join("coverage")
                .join("profiles.toml"),
            "[profiles.default]\nfeatures = [\"\"]\n",
        );
        let profile_args = vec![
            "--crate".to_string(),
            "radroots-core".to_string(),
            "--out".to_string(),
            profile_root.join("out").display().to_string(),
        ];
        let mut runner = run_command;
        let profile_err = run_crate_with_runner_at_root(&profile_args, &profile_root, &mut runner)
            .expect_err("invalid profile should fail");
        assert!(profile_err.contains("empty feature value"));
        fs::remove_dir_all(&profile_root).expect("remove profile root");

        let thread_root = temp_dir_path("run_crate_bad_threads");
        fs::create_dir_all(&thread_root).expect("create thread root");
        let thread_args = vec![
            "--crate".to_string(),
            "radroots-core".to_string(),
            "--out".to_string(),
            thread_root.join("out").display().to_string(),
            "--test-threads".to_string(),
            "bad".to_string(),
        ];
        let mut runner = run_command;
        let thread_err = run_crate_with_runner_at_root(&thread_args, &thread_root, &mut runner)
            .expect_err("invalid test threads should fail");
        assert!(thread_err.contains("invalid --test-threads value"));
        fs::remove_dir_all(&thread_root).expect("remove thread root");

        for fail_step in [2usize, 3usize, 4usize] {
            let step_root = temp_dir_path("run_crate_step_fail");
            let step_args = vec![
                "--crate".to_string(),
                "radroots-core".to_string(),
                "--out".to_string(),
                step_root.join("out").display().to_string(),
            ];
            let mut calls = 0usize;
            let mut runner = |_: Command, name: &str| {
                calls += 1;
                if calls == fail_step {
                    return Err(format!("runner failure at {name}"));
                }
                Ok(())
            };
            let err = run_crate_with_runner_at_root(&step_args, &step_root, &mut runner)
                .expect_err("runner should fail at selected step");
            assert!(err.contains("runner failure at"));
            fs::remove_dir_all(&step_root).expect("remove step root");
        }
    }

    #[test]
    fn report_gate_writes_report_file_on_success() {
        let root = temp_dir_path("report_gate_success");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");

        let args = vec![
            "--scope".to_string(),
            "crate-x".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--require-branches".to_string(),
        ];
        report_gate(&args).expect("report gate success");
        let report_raw = fs::read_to_string(&out_path).expect("read report");
        assert!(report_raw.contains("\"scope\": \"crate-x\""));
        assert!(report_raw.contains("\"regions\": 100.0"));
        assert!(report_raw.contains("\"pass\": true"));
        fs::remove_dir_all(root).expect("remove report gate success root");
    }

    #[test]
    fn report_gate_returns_error_on_failed_thresholds() {
        let root = temp_dir_path("report_gate_fail");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":10.0},"lines":{"percent":10.0},"regions":{"percent":10.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,0\nBRDA:1,0,0,0\n");

        let args = vec![
            "--scope".to_string(),
            "crate-y".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-exec-lines".to_string(),
            "100.0".to_string(),
            "--fail-under-functions".to_string(),
            "100.0".to_string(),
            "--fail-under-regions".to_string(),
            "100.0".to_string(),
            "--fail-under-branches".to_string(),
            "100.0".to_string(),
        ];
        let err = report_gate(&args).expect_err("report gate failure");
        assert!(err.contains("coverage gate failed"));
        fs::remove_dir_all(root).expect("remove report gate failure root");
    }

    #[test]
    fn report_gate_handles_nan_threshold_input() {
        let root = temp_dir_path("report_gate_nan");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");

        let args = vec![
            "--scope".to_string(),
            "crate-nan".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-functions".to_string(),
            "NaN".to_string(),
        ];
        let err = report_gate(&args).expect_err("nan threshold should fail coverage gate");
        assert!(err.contains("coverage gate failed"));
        fs::remove_dir_all(root).expect("remove report gate nan root");
    }

    #[test]
    fn report_gate_reports_write_failure() {
        let root = temp_dir_path("report_gate_write_fail");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");
        fs::create_dir_all(&out_path).expect("create directory at output path");

        let args = vec![
            "--scope".to_string(),
            "crate-write".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--require-branches".to_string(),
        ];
        let err = report_gate(&args).expect_err("writing report to directory should fail");
        assert!(err.contains("failed to write"));
        fs::remove_dir_all(root).expect("remove report gate write root");
    }

    #[test]
    fn report_gate_logs_branch_unavailable_path() {
        let root = temp_dir_path("report_gate_no_branches");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\n");

        let args = vec![
            "--scope".to_string(),
            "crate-no-branch".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ];
        report_gate(&args).expect("report gate no branches");
        let report_raw = fs::read_to_string(&out_path).expect("read report");
        assert!(report_raw.contains("\"branches_available\": false"));
        fs::remove_dir_all(root).expect("remove no branch report root");
    }

    #[test]
    fn report_gate_reports_argument_and_input_errors() {
        let missing_scope = report_gate(&[]).expect_err("missing scope");
        assert!(missing_scope.contains("missing --scope"));

        let missing_summary = report_gate(&["--scope".to_string(), "crate".to_string()])
            .expect_err("missing summary");
        assert!(missing_summary.contains("missing --summary"));

        let missing_lcov = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            "summary.json".to_string(),
        ])
        .expect_err("missing lcov");
        assert!(missing_lcov.contains("missing --lcov"));

        let missing_out = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            "summary.json".to_string(),
            "--lcov".to_string(),
            "coverage.info".to_string(),
        ])
        .expect_err("missing out");
        assert!(missing_out.contains("missing --out"));

        let root = temp_dir_path("report_gate_arg_errors");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");

        let invalid_functions = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-functions".to_string(),
            "bad".to_string(),
        ])
        .expect_err("invalid functions threshold");
        assert!(invalid_functions.contains("invalid --fail-under-functions value"));

        let invalid_exec = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-exec-lines".to_string(),
            "bad".to_string(),
        ])
        .expect_err("invalid executable threshold");
        assert!(invalid_exec.contains("invalid --fail-under-exec-lines value"));

        let invalid_regions = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-regions".to_string(),
            "bad".to_string(),
        ])
        .expect_err("invalid regions threshold");
        assert!(invalid_regions.contains("invalid --fail-under-regions value"));

        let invalid_branches = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-branches".to_string(),
            "bad".to_string(),
        ])
        .expect_err("invalid branches threshold");
        assert!(invalid_branches.contains("invalid --fail-under-branches value"));

        let missing_summary_file = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            root.join("missing-summary.json").display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect_err("missing summary file should fail");
        assert!(missing_summary_file.contains("failed to read summary"));

        let missing_lcov_file = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            root.join("missing-lcov.info").display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect_err("missing lcov file should fail");
        assert!(missing_lcov_file.contains("failed to read lcov"));

        fs::remove_dir_all(root).expect("remove report arg errors root");
    }

    #[test]
    fn run_dispatches_subcommands_and_errors() {
        run(&["help".to_string()]).expect("help subcommand");
        run(&["required-crates".to_string()]).expect("required crates subcommand");
        run(&["workspace-crates".to_string()]).expect("workspace crates subcommand");
        let run_crate_err = run(&["run-crate".to_string()]).expect_err("run crate missing args");
        assert!(run_crate_err.contains("missing --crate"));
        let unknown_err = run(&["unknown".to_string()]).expect_err("unknown subcommand");
        assert!(unknown_err.contains("unknown sdk coverage subcommand"));
        let missing_err = run(&[]).expect_err("missing subcommand");
        assert!(missing_err.contains("missing sdk coverage subcommand"));
    }

    #[test]
    fn list_root_helpers_report_missing_contract_files() {
        let root = temp_dir_path("list_helper_missing");
        fs::create_dir_all(&root).expect("create list helper root");
        let mut output = Vec::new();
        let required_err = list_required_crates_with_root(&root, &mut output)
            .expect_err("missing required crates file should fail");
        assert!(required_err.contains("failed to read required crates"));

        let workspace_err = list_workspace_crates_with_root(&root, &mut output)
            .expect_err("missing workspace manifest should fail");
        assert!(workspace_err.contains("failed to read"));

        fs::remove_dir_all(root).expect("remove list helper root");
    }

    #[test]
    fn write_crate_names_output_covers_success_and_error_paths() {
        let mut output = Vec::new();
        write_crate_names_output(
            &mut output,
            vec!["radroots-a".to_string(), "radroots-b".to_string()],
            "required crates",
        )
        .expect("write crate names");
        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("radroots-a"));
        assert!(rendered.contains("radroots-b"));

        let mut failing = FailingWriter;
        let err = write_crate_names_output(
            &mut failing,
            vec!["radroots-a".to_string()],
            "workspace crates",
        )
        .expect_err("writer failure");
        assert!(err.contains("failed to write workspace crates output"));
        failing.flush().expect("flush failing writer");
    }

    #[test]
    fn run_report_subcommand_dispatches_to_report_gate() {
        let root = temp_dir_path("run_dispatch_report");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");

        run(&[
            "report".to_string(),
            "--scope".to_string(),
            "dispatch".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--require-branches".to_string(),
        ])
        .expect("dispatch report");
        assert!(out_path.exists());
        fs::remove_dir_all(root).expect("remove report dispatch root");
    }
}
