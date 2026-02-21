#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

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

pub fn read_lcov(path: &Path) -> Result<LcovCoverage, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read lcov {}: {err}", path.display()))?;

    let mut da_total: u64 = 0;
    let mut da_covered: u64 = 0;
    let mut executable_total: u64 = 0;
    let mut executable_covered: u64 = 0;
    let mut branch_total: u64 = 0;
    let mut branch_covered: u64 = 0;

    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("DA:") {
            let Some((_, hit)) = value.split_once(',') else {
                return Err(format!("invalid DA record in {}", path.display()));
            };
            let hit_count: u64 = hit
                .parse()
                .map_err(|err| format!("invalid DA hit count `{hit}` in {}: {err}", path.display()))?;
            da_total = da_total.saturating_add(1);
            if hit_count > 0 {
                da_covered = da_covered.saturating_add(1);
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("LF:") {
            let parsed: u64 = value
                .parse()
                .map_err(|err| format!("invalid LF value `{value}` in {}: {err}", path.display()))?;
            executable_total = executable_total.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("LH:") {
            let parsed: u64 = value
                .parse()
                .map_err(|err| format!("invalid LH value `{value}` in {}: {err}", path.display()))?;
            executable_covered = executable_covered.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRF:") {
            let parsed: u64 = value
                .parse()
                .map_err(|err| format!("invalid BRF value `{value}` in {}: {err}", path.display()))?;
            branch_total = branch_total.saturating_add(parsed);
            continue;
        }
        if let Some(value) = line.strip_prefix("BRH:") {
            let parsed: u64 = value
                .parse()
                .map_err(|err| format!("invalid BRH value `{value}` in {}: {err}", path.display()))?;
            branch_covered = branch_covered.saturating_add(parsed);
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

fn parse_u32_arg(args: &[String], name: &str, default: u32) -> Result<u32, String> {
    if let Some(raw) = parse_optional_string_arg(args, name) {
        return raw
            .parse::<u32>()
            .map_err(|err| format!("invalid --{name} value `{raw}`: {err}"));
    }
    Ok(default)
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

fn run_crate(args: &[String]) -> Result<(), String> {
    let crate_name = parse_string_arg(args, "crate")?;
    let workspace_root = workspace_root()?;
    let out_dir = if let Some(raw) = parse_optional_string_arg(args, "out") {
        PathBuf::from(raw)
    } else {
        workspace_root
            .join("target")
            .join("coverage")
            .join(crate_name.replace('-', "_"))
    };
    let test_threads = parse_u32_arg(args, "test-threads", 1)?;

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
            cmd.arg("run")
                .arg("nightly")
                .arg("cargo")
                .arg("llvm-cov")
                .arg("-p")
                .arg(&crate_name)
                .arg("--no-report")
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
            cmd.arg("run")
                .arg("nightly")
                .arg("cargo")
                .arg("llvm-cov")
                .arg("report")
                .arg("-p")
                .arg(&crate_name)
                .arg("--json")
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
            cmd.arg("run")
                .arg("nightly")
                .arg("cargo")
                .arg("llvm-cov")
                .arg("report")
                .arg("-p")
                .arg(&crate_name)
                .arg("--lcov")
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

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") => Ok(()),
        Some("run-crate") => run_crate(&args[1..]),
        Some("report") => report_gate(&args[1..]),
        Some(_) => Err("unknown sdk coverage subcommand".to_string()),
        None => Err("missing sdk coverage subcommand".to_string()),
    }
}
