#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use serde::Deserialize;

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

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") => Ok(()),
        Some(_) => Err("unknown sdk coverage subcommand".to_string()),
        None => Err("missing sdk coverage subcommand".to_string()),
    }
}
