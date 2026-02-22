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
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read summary {}: {err}", path.display()))?;
    let parsed: LlvmCovSummaryRoot = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse summary {}: {err}", path.display()))?;
    let totals = parsed
        .data
        .first()
        .map(|entry| &entry.totals)
        .ok_or_else(|| format!("summary data is empty in {}", path.display()))?;

    Ok(CoverageSummary {
        functions_percent: totals.functions.percent,
        summary_lines_percent: totals.lines.percent,
        summary_regions_percent: totals.regions.percent,
    })
}

fn read_required_crates(path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read required crates {}: {err}", path.display()))?;
    let parsed: CoverageRequiredContract = toml::from_str(&raw)
        .map_err(|err| format!("failed to parse required crates {}: {err}", path.display()))?;
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
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    toml::from_str::<T>(&raw).map_err(|err| format!("failed to parse {}: {err}", path.display()))
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
    if let Some(test_threads) = resolved.test_threads {
        if test_threads == 0 {
            return Err(format!(
                "coverage profile for {crate_name} must set test_threads > 0"
            ));
        }
    }
    Ok(resolved)
}

pub fn read_lcov(path: &Path) -> Result<LcovCoverage, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read lcov {}: {err}", path.display()))?;

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
            let hit_count: u64 = hit.parse().map_err(|err| {
                format!("invalid DA hit count `{hit}` in {}: {err}", path.display())
            })?;
            da_total = da_total.saturating_add(1);
            if hit_count > 0 {
                da_covered = da_covered.saturating_add(1);
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("LF:") {
            let parsed: u64 = value.parse().map_err(|err| {
                format!("invalid LF value `{value}` in {}: {err}", path.display())
            })?;
            executable_total = executable_total.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("LH:") {
            let parsed: u64 = value.parse().map_err(|err| {
                format!("invalid LH value `{value}` in {}: {err}", path.display())
            })?;
            executable_covered = executable_covered.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRF:") {
            let parsed: u64 = value.parse().map_err(|err| {
                format!("invalid BRF value `{value}` in {}: {err}", path.display())
            })?;
            branch_total_lcov = branch_total_lcov.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRH:") {
            let parsed: u64 = value.parse().map_err(|err| {
                format!("invalid BRH value `{value}` in {}: {err}", path.display())
            })?;
            branch_covered_lcov = branch_covered_lcov.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRDA:") {
            let mut fields = value.split(',');
            let _line_no = fields
                .next()
                .ok_or_else(|| format!("invalid BRDA record in {}", path.display()))?;
            let _block_no = fields
                .next()
                .ok_or_else(|| format!("invalid BRDA record in {}", path.display()))?;
            let _branch_no = fields
                .next()
                .ok_or_else(|| format!("invalid BRDA record in {}", path.display()))?;
            let taken = fields
                .next()
                .ok_or_else(|| format!("invalid BRDA record in {}", path.display()))?;
            if fields.next().is_some() {
                return Err(format!("invalid BRDA record in {}", path.display()));
            }
            if taken == "-" {
                continue;
            }
            let hit_count: u64 = taken.parse().map_err(|err| {
                format!(
                    "invalid BRDA taken count `{taken}` in {}: {err}",
                    path.display()
                )
            })?;
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
    let branch_presence_ok = !thresholds.require_branches || lcov.branches_available;

    let mut branch_ok = true;
    if lcov.branches_available {
        if let Some(branch_percent) = lcov.branch_percent {
            branch_ok = branch_percent >= thresholds.fail_under_branches;
        }
    }

    let pass = exec_ok && functions_ok && branch_presence_ok && branch_ok;
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

    if thresholds.require_branches && !lcov.branches_available {
        fail_reasons.push("branches=unavailable".to_string());
    }

    if lcov.branches_available && !branch_ok {
        if let Some(branch_percent) = lcov.branch_percent {
            fail_reasons.push(format!(
                "branches={:.6} < {:.6}",
                branch_percent, thresholds.fail_under_branches
            ));
        }
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

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(crates_dir) = manifest_dir.parent() else {
        return Err("failed to resolve crates dir".to_string());
    };
    let Some(root) = crates_dir.parent() else {
        return Err("failed to resolve workspace root".to_string());
    };
    Ok(root.to_path_buf())
}

fn run_command(mut command: Command, name: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|err| format!("failed to run {name}: {err}"))?;
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

fn run_crate(args: &[String]) -> Result<(), String> {
    let crate_name = parse_string_arg(args, "crate")?;
    let workspace_root = workspace_root()?;
    let profile = read_coverage_profile(&workspace_root, &crate_name)?;
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

    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

    run_command(
        {
            let mut cmd = Command::new("rustup");
            cmd.arg("run")
                .arg("nightly")
                .arg("cargo")
                .arg("llvm-cov")
                .arg("clean")
                .arg("--workspace")
                .current_dir(&workspace_root);
            cmd
        },
        "cargo llvm-cov clean --workspace",
    )?;

    run_command(
        {
            let mut cmd = Command::new("rustup");
            cmd.arg("run").arg("nightly").arg("cargo").arg("llvm-cov");
            cmd.arg("-p").arg(&crate_name);
            apply_coverage_profile_flags(&mut cmd, &profile);
            cmd.arg("--no-report")
                .arg("--branch")
                .arg("--")
                .arg(format!("--test-threads={test_threads}"))
                .current_dir(&workspace_root);
            cmd
        },
        "cargo llvm-cov --no-report",
    )?;

    let summary_path = out_dir.join("coverage-summary.json");
    run_command(
        {
            let mut cmd = Command::new("rustup");
            cmd.arg("run").arg("nightly").arg("cargo").arg("llvm-cov");
            cmd.arg("report").arg("-p").arg(&crate_name);
            cmd.arg("--json")
                .arg("--summary-only")
                .arg("--branch")
                .arg("--output-path")
                .arg(&summary_path)
                .current_dir(&workspace_root);
            cmd
        },
        "cargo llvm-cov report --json --summary-only",
    )?;

    let lcov_path = out_dir.join("coverage-lcov.info");
    run_command(
        {
            let mut cmd = Command::new("rustup");
            cmd.arg("run").arg("nightly").arg("cargo").arg("llvm-cov");
            cmd.arg("report").arg("-p").arg(&crate_name);
            cmd.arg("--lcov")
                .arg("--branch")
                .arg("--output-path")
                .arg(&lcov_path)
                .current_dir(&workspace_root);
            cmd
        },
        "cargo llvm-cov report --lcov",
    )?;

    eprintln!("coverage summary: {}", summary_path.display());
    eprintln!("coverage lcov: {}", lcov_path.display());
    Ok(())
}

fn report_gate(args: &[String]) -> Result<(), String> {
    let scope = parse_string_arg(args, "scope")?;
    let summary_path = PathBuf::from(parse_string_arg(args, "summary")?);
    let lcov_path = PathBuf::from(parse_string_arg(args, "lcov")?);
    let out_path = PathBuf::from(parse_string_arg(args, "out")?);
    let thresholds = CoverageThresholds {
        fail_under_exec_lines: parse_f64_arg(args, "fail-under-exec-lines", 100.0)?,
        fail_under_functions: parse_f64_arg(args, "fail-under-functions", 100.0)?,
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
        .map_err(|err| format!("failed to encode coverage report json: {err}"))?;
    fs::write(&out_path, format!("{json}\n"))
        .map_err(|err| format!("failed to write {}: {err}", out_path.display()))?;

    if lcov.branches_available {
        eprintln!(
            "{} coverage: executable_lines={:.6} functions={:.6} branches={:.6}",
            scope,
            lcov.executable_percent,
            summary.functions_percent,
            lcov.branch_percent.unwrap_or(0.0)
        );
    } else {
        eprintln!(
            "{} coverage: executable_lines={:.6} functions={:.6} branches=unavailable",
            scope, lcov.executable_percent, summary.functions_percent
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

fn list_required_crates() -> Result<(), String> {
    let root = workspace_root()?;
    let required_path = root
        .join("contract")
        .join("coverage")
        .join("required-crates.toml");
    let crates = read_required_crates(&required_path)?;
    let mut stdout = std::io::stdout().lock();
    for crate_name in crates {
        writeln!(stdout, "{crate_name}")
            .map_err(|err| format!("failed to write required crates output: {err}"))?;
    }
    Ok(())
}

fn list_workspace_crates() -> Result<(), String> {
    let root = workspace_root()?;
    let crates = read_workspace_crates(&root)?;
    let mut stdout = std::io::stdout().lock();
    for crate_name in crates {
        writeln!(stdout, "{crate_name}")
            .map_err(|err| format!("failed to write workspace crates output: {err}"))?;
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
    fn reads_workspace_crates_and_contains_xtask() {
        let root = workspace_root().expect("workspace root");
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
            err.contains("empty feature value") || err.contains("test_threads > 0"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(root).expect("remove root");
    }
}
