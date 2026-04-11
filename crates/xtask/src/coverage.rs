#![forbid(unsafe_code)]

use std::ffi::OsString;
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
struct DetailedCoverageSummary {
    functions_percent: f64,
    regions_percent: f64,
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

#[derive(Debug, Serialize, Deserialize)]
struct CoverageGateReport {
    scope: String,
    thresholds: CoverageGateReportThresholds,
    measured: CoverageGateReportMeasured,
    counts: CoverageGateReportCounts,
    result: CoverageGateReportResult,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageGateReportThresholds {
    executable_lines: f64,
    functions: f64,
    regions: f64,
    branches: f64,
    branches_required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageGateReportMeasured {
    executable_lines_percent: f64,
    executable_lines_source: String,
    functions_percent: f64,
    branches_percent: Option<f64>,
    branches_available: bool,
    summary_lines_percent: f64,
    summary_regions_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageGateReportCounts {
    executable_lines: CoverageCount,
    branches: CoverageCount,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageCount {
    covered: u64,
    total: u64,
}

#[derive(Debug, Serialize, Deserialize)]
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
struct LlvmCovDetailsRoot {
    data: Vec<LlvmCovDetailsData>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovDetailsData {
    #[serde(default)]
    functions: Vec<LlvmCovFunction>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovFunction {
    count: u64,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    regions: Vec<[u64; 8]>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FunctionCoverageKey {
    filenames: Vec<String>,
    regions: Vec<RegionCoverageKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RegionCoverageKey {
    line_start: u64,
    column_start: u64,
    line_end: u64,
    column_end: u64,
    file_id: u64,
    expanded_file_id: u64,
    kind: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoveragePolicyFile {
    gate: CoveragePolicyGate,
    required: CoverageRequiredList,
    #[serde(default)]
    overrides: BTreeMap<String, CoveragePolicyOverride>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoveragePolicyGate {
    fail_under_exec_lines: f64,
    fail_under_functions: f64,
    fail_under_regions: f64,
    fail_under_branches: f64,
    require_branches: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageRequiredList {
    crates: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoveragePolicyOverride {
    fail_under_exec_lines: Option<f64>,
    fail_under_functions: Option<f64>,
    fail_under_regions: Option<f64>,
    fail_under_branches: Option<f64>,
    require_branches: Option<bool>,
    temporary: bool,
    reason: String,
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

#[cfg_attr(not(test), allow(dead_code))]
pub fn read_summary(path: &Path) -> Result<CoverageSummary, String> {
    read_summary_for_scope(path, None)
}

fn read_summary_for_scope(path: &Path, scope: Option<&str>) -> Result<CoverageSummary, String> {
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

    let mut summary = CoverageSummary {
        functions_percent: totals.functions.percent,
        summary_lines_percent: totals.lines.percent,
        summary_regions_percent: totals.regions.percent,
    };

    let details_path = coverage_details_path(path);
    if details_path.exists() {
        let normalized = read_detailed_summary(&details_path, scope)?;
        if (summary.functions_percent - 100.0).abs() < f64::EPSILON {
            summary.summary_regions_percent = normalized.regions_percent;
        }
    }

    Ok(summary)
}

fn coverage_details_path(summary_path: &Path) -> PathBuf {
    summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("coverage-details.json")
}

fn read_detailed_summary(
    path: &Path,
    scope: Option<&str>,
) -> Result<DetailedCoverageSummary, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(format!(
                "failed to read coverage details {}: {err}",
                path.display()
            ));
        }
    };
    let parsed: LlvmCovDetailsRoot = match serde_json::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            return Err(format!(
                "failed to parse coverage details {}: {err}",
                path.display()
            ));
        }
    };
    let Some(entry) = parsed.data.first() else {
        return Err(format!(
            "coverage details data is empty in {}",
            path.display()
        ));
    };

    let mut functions_by_key: BTreeMap<FunctionCoverageKey, Vec<&LlvmCovFunction>> =
        BTreeMap::new();
    for function in &entry.functions {
        if function.filenames.is_empty() || function.regions.is_empty() {
            continue;
        }
        let key = FunctionCoverageKey {
            filenames: function.filenames.clone(),
            regions: function
                .regions
                .iter()
                .map(|region| RegionCoverageKey {
                    line_start: region[0],
                    column_start: region[1],
                    line_end: region[2],
                    column_end: region[3],
                    file_id: region[5],
                    expanded_file_id: region[6],
                    kind: region[7],
                })
                .collect(),
        };
        functions_by_key.entry(key).or_default().push(function);
    }

    if functions_by_key.is_empty() {
        return Err(format!(
            "coverage details functions are empty in {}",
            path.display()
        ));
    }

    let mut regions_total = 0_u64;
    let mut regions_covered = 0_u64;
    let mut functions_total = 0_u64;
    let mut functions_covered = 0_u64;
    let mut source_cache: BTreeMap<String, Option<String>> = BTreeMap::new();
    let scope_filter = scope.map(scope_path_fragment);
    for variants in functions_by_key.values() {
        if !variants.iter().any(|function| function.count > 0) {
            continue;
        }
        if let Some(scope_filter) = scope_filter.as_deref() {
            if !variants.iter().any(|function| {
                function
                    .filenames
                    .iter()
                    .any(|filename| filename.contains(scope_filter))
            }) {
                continue;
            }
        }
        functions_total = functions_total.saturating_add(1);
        functions_covered = functions_covered.saturating_add(1);
        let mut group_regions: BTreeMap<RegionCoverageKey, bool> = BTreeMap::new();
        for function in variants {
            for region in &function.regions {
                let key = RegionCoverageKey {
                    line_start: region[0],
                    column_start: region[1],
                    line_end: region[2],
                    column_end: region[3],
                    file_id: region[5],
                    expanded_file_id: region[6],
                    kind: region[7],
                };
                let covered = region[4] > 0;
                group_regions
                    .entry(key)
                    .and_modify(|existing| *existing |= covered)
                    .or_insert(covered);
            }
        }
        let primary_filename = variants
            .first()
            .and_then(|function| function.filenames.first())
            .map(String::as_str);
        for (region, covered) in group_regions {
            if !covered
                && primary_filename.is_some_and(|filename| {
                    is_ignorable_synthetic_region(filename, &region, &mut source_cache)
                })
            {
                continue;
            }
            regions_total = regions_total.saturating_add(1);
            if covered {
                regions_covered = regions_covered.saturating_add(1);
            }
        }
    }

    Ok(DetailedCoverageSummary {
        functions_percent: percentage(functions_covered, functions_total),
        regions_percent: percentage(regions_covered, regions_total),
    })
}

fn scope_path_fragment(scope: &str) -> String {
    let crate_dir = scope.strip_prefix("radroots_").unwrap_or(scope);
    format!("/crates/{crate_dir}/")
}

fn percentage(covered: u64, total: u64) -> f64 {
    if total == 0 {
        100.0
    } else {
        (covered as f64 / total as f64) * 100.0
    }
}

fn is_ignorable_synthetic_region(
    filename: &str,
    region: &RegionCoverageKey,
    source_cache: &mut BTreeMap<String, Option<String>>,
) -> bool {
    if region.line_start != region.line_end {
        return false;
    }
    let source = source_cache
        .entry(filename.to_string())
        .or_insert_with(|| fs::read_to_string(filename).ok());
    let Some(source) = source.as_ref() else {
        return false;
    };
    let Some(line) = source
        .lines()
        .nth(region.line_start.saturating_sub(1) as usize)
    else {
        return false;
    };
    let start = region.column_start.saturating_sub(1) as usize;
    let end = region.column_end.saturating_sub(1) as usize;
    let slice = line.get(start..end);
    if region.column_end == region.column_start + 1 && slice == Some("?") {
        return true;
    }

    let is_unexpected_panic_fallback = filename.ends_with("/tests.rs")
        && line.contains("panic!(\"unexpected")
        && matches!(slice, Some("other") | Some("panic!"));
    is_unexpected_panic_fallback
}

impl CoveragePolicyFile {
    pub(crate) fn thresholds(&self) -> CoverageThresholds {
        CoverageThresholds {
            fail_under_exec_lines: self.gate.fail_under_exec_lines,
            fail_under_functions: self.gate.fail_under_functions,
            fail_under_regions: self.gate.fail_under_regions,
            fail_under_branches: self.gate.fail_under_branches,
            require_branches: self.gate.require_branches,
        }
    }

    pub(crate) fn thresholds_for_scope(&self, scope: &str) -> CoverageThresholds {
        let base = self.thresholds();
        let Some(override_policy) = self.overrides.get(scope) else {
            return base;
        };
        CoverageThresholds {
            fail_under_exec_lines: override_policy
                .fail_under_exec_lines
                .unwrap_or(base.fail_under_exec_lines),
            fail_under_functions: override_policy
                .fail_under_functions
                .unwrap_or(base.fail_under_functions),
            fail_under_regions: override_policy
                .fail_under_regions
                .unwrap_or(base.fail_under_regions),
            fail_under_branches: override_policy
                .fail_under_branches
                .unwrap_or(base.fail_under_branches),
            require_branches: override_policy
                .require_branches
                .unwrap_or(base.require_branches),
        }
    }

    pub(crate) fn required_crates(&self) -> Result<Vec<String>, String> {
        if self.required.crates.is_empty() {
            return Err("coverage required crates list must not be empty".to_string());
        }
        let mut seen = BTreeSet::new();
        for crate_name in &self.required.crates {
            if crate_name.trim().is_empty() {
                return Err(
                    "coverage required crates list includes an empty crate name".to_string()
                );
            }
            if !seen.insert(crate_name.clone()) {
                return Err(format!(
                    "coverage required crates list includes duplicate crate {crate_name}"
                ));
            }
        }
        Ok(self.required.crates.clone())
    }

    fn validate_overrides(&self) -> Result<(), String> {
        let required_crates = self.required_crates()?;
        let required_set: BTreeSet<_> = required_crates.into_iter().collect();
        let base = self.thresholds();
        for (crate_name, override_policy) in &self.overrides {
            if !required_set.contains(crate_name) {
                return Err(format!(
                    "coverage override {crate_name} must target a required crate"
                ));
            }
            if !override_policy.temporary {
                return Err(format!(
                    "coverage override {crate_name} must set temporary = true"
                ));
            }
            if override_policy.reason.trim().is_empty() {
                return Err(format!(
                    "coverage override {crate_name} must include a non-empty reason"
                ));
            }
            validate_override_threshold(
                crate_name,
                "fail_under_exec_lines",
                override_policy.fail_under_exec_lines,
                base.fail_under_exec_lines,
            )?;
            validate_override_threshold(
                crate_name,
                "fail_under_functions",
                override_policy.fail_under_functions,
                base.fail_under_functions,
            )?;
            validate_override_threshold(
                crate_name,
                "fail_under_regions",
                override_policy.fail_under_regions,
                base.fail_under_regions,
            )?;
            validate_override_threshold(
                crate_name,
                "fail_under_branches",
                override_policy.fail_under_branches,
                base.fail_under_branches,
            )?;
            if override_policy.require_branches == Some(true) && !base.require_branches {
                return Err(format!(
                    "coverage override {crate_name} require_branches cannot be stricter than the global gate"
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn required_crate_entries(&self) -> &[String] {
        &self.required.crates
    }
}

fn validate_override_threshold(
    crate_name: &str,
    label: &str,
    value: Option<f64>,
    global: f64,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_finite() {
        return Err(format!(
            "coverage override {crate_name} {label} must be finite"
        ));
    }
    if !(0.0..=100.0).contains(&value) {
        return Err(format!(
            "coverage override {crate_name} {label} must be within 0..=100"
        ));
    }
    if value > global {
        return Err(format!(
            "coverage override {crate_name} {label} must not exceed the global gate"
        ));
    }
    Ok(())
}

pub(crate) fn coverage_policy_path(root: &Path) -> PathBuf {
    root.join("policy").join("coverage").join("policy.toml")
}

pub(crate) fn read_coverage_policy(path: &Path) -> Result<CoveragePolicyFile, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(format!(
                "failed to read coverage policy {}: {err}",
                path.display()
            ));
        }
    };
    let parsed: CoveragePolicyFile = match toml::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            return Err(format!(
                "failed to parse coverage policy {}: {err}",
                path.display()
            ));
        }
    };
    let thresholds = parsed.thresholds();
    for (label, value) in [
        ("fail_under_exec_lines", thresholds.fail_under_exec_lines),
        ("fail_under_functions", thresholds.fail_under_functions),
        ("fail_under_regions", thresholds.fail_under_regions),
        ("fail_under_branches", thresholds.fail_under_branches),
    ] {
        if !value.is_finite() {
            return Err(format!("coverage policy {label} must be finite"));
        }
        if !(0.0..=100.0).contains(&value) {
            return Err(format!("coverage policy {label} must be within 0..=100"));
        }
    }
    parsed.required_crates()?;
    parsed.validate_overrides()?;
    Ok(parsed)
}

fn read_required_crates(path: &Path) -> Result<Vec<String>, String> {
    read_coverage_policy(path)?.required_crates()
}

fn read_workspace_crates(workspace_root: &Path) -> Result<Vec<String>, String> {
    let packages = read_workspace_packages(workspace_root)?;
    Ok(packages.into_iter().map(|(name, _)| name).collect())
}

fn read_workspace_packages(workspace_root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let workspace_manifest = parse_toml::<WorkspaceManifest>(&workspace_root.join("Cargo.toml"))?;
    if workspace_manifest.workspace.members.is_empty() {
        return Err("workspace members list must not be empty".to_string());
    }
    let mut packages = Vec::with_capacity(workspace_manifest.workspace.members.len());
    let mut seen = BTreeSet::new();
    for member in workspace_manifest.workspace.members {
        let package_manifest =
            parse_toml::<PackageManifest>(&workspace_root.join(&member).join("Cargo.toml"))?;
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
        packages.push((package_name, PathBuf::from(member)));
    }
    Ok(packages)
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
        .join("policy")
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

fn parse_optional_f64_arg(args: &[String], name: &str) -> Result<Option<f64>, String> {
    if let Some(raw) = parse_optional_string_arg(args, name) {
        let parsed = raw
            .parse::<f64>()
            .map_err(|err| format!("invalid --{name} value `{raw}`: {err}"))?;
        if !parsed.is_finite() {
            return Err(format!("invalid --{name} value `{raw}`: must be finite"));
        }
        return Ok(Some(parsed));
    }
    Ok(None)
}

#[cfg_attr(not(test), allow(dead_code))]
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

fn has_flag(args: &[String], name: &str) -> bool {
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

fn prepend_toolchain_bin_to_path(
    toolchain_bin: &Path,
    existing_path: Option<OsString>,
) -> OsString {
    match existing_path {
        Some(existing) => std::env::join_paths(
            std::iter::once(toolchain_bin.to_path_buf()).chain(std::env::split_paths(&existing)),
        )
        .expect("joining PATH entries for coverage toolchain should succeed"),
        None => OsString::from(toolchain_bin),
    }
}

fn configure_coverage_toolchain_env(command: &mut Command, toolchain_bin: &Path) {
    let joined_path = prepend_toolchain_bin_to_path(toolchain_bin, std::env::var_os("PATH"));
    command.env("PATH", joined_path);

    for (env_name, binary_name) in [
        ("RUSTC", "rustc"),
        ("RUSTDOC", "rustdoc"),
        ("LLVM_COV", "llvm-cov"),
        ("LLVM_PROFDATA", "llvm-profdata"),
    ] {
        let binary_path = toolchain_bin.join(binary_name);
        if binary_path.exists() {
            command.env(env_name, binary_path);
        }
    }
}

fn coverage_cargo_command_with_override(override_binary: Option<&str>) -> Command {
    if let Some(binary) = override_binary {
        let mut cmd = Command::new(binary);
        if let Some(toolchain_bin) = Path::new(binary).parent() {
            configure_coverage_toolchain_env(&mut cmd, toolchain_bin);
        }
        return cmd;
    }

    let mut cmd = Command::new("rustup");
    cmd.arg("run").arg("nightly").arg("cargo");
    cmd
}

fn normalized_coverage_cargo_override(raw: Option<String>) -> Option<String> {
    raw.map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn coverage_cargo_command() -> Command {
    let override_binary =
        normalized_coverage_cargo_override(std::env::var("RADROOTS_COVERAGE_CARGO").ok());
    coverage_cargo_command_with_override(override_binary.as_deref())
}

fn coverage_llvm_cov_command() -> Command {
    let mut cmd = coverage_cargo_command();
    cmd.arg("llvm-cov");
    cmd
}

const COVERAGE_EXTERNAL_IGNORE_FILENAME_REGEX: &str =
    r"(/\.cargo/registry/|/lib/rustlib/src/rust/)";

fn escape_regex_literal(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn coverage_ignore_filename_regex(
    workspace_root: &Path,
    crate_name: &str,
) -> Result<String, String> {
    let mut patterns = vec![COVERAGE_EXTERNAL_IGNORE_FILENAME_REGEX.to_string()];
    let mut found_target = false;

    for (package_name, member_path) in read_workspace_packages(workspace_root)? {
        let absolute_member = workspace_root.join(member_path);
        if package_name == crate_name {
            found_target = true;
            continue;
        }
        patterns.push(format!(
            "^{}/",
            escape_regex_literal(&absolute_member.display().to_string())
        ));
    }

    if !found_target {
        return Err(format!(
            "workspace coverage filters could not resolve crate directory for {crate_name}"
        ));
    }

    Ok(format!("({})", patterns.join("|")))
}

fn apply_coverage_report_filters(command: &mut Command, ignore_regex: &str) {
    command.arg("--ignore-filename-regex").arg(ignore_regex);
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
    let ignore_regex = coverage_ignore_filename_regex(workspace_root, &crate_name)?;

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
            apply_coverage_report_filters(&mut cmd, &ignore_regex);
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

    let details_path = out_dir.join("coverage-details.json");
    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("report").arg("-p").arg(&crate_name);
            apply_coverage_report_filters(&mut cmd, &ignore_regex);
            cmd.arg("--json")
                .arg("--branch")
                .arg("--output-path")
                .arg(&details_path)
                .current_dir(workspace_root);
            cmd
        },
        "cargo llvm-cov report --json",
    )?;

    let lcov_path = out_dir.join("coverage-lcov.info");
    runner(
        {
            let mut cmd = coverage_llvm_cov_command();
            cmd.arg("report").arg("-p").arg(&crate_name);
            apply_coverage_report_filters(&mut cmd, &ignore_regex);
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
    eprintln!("coverage details: {}", details_path.display());
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

fn report_gate_with_root(args: &[String], root: &Path) -> Result<(), String> {
    let scope = parse_string_arg(args, "scope")?;
    let summary_path = PathBuf::from(parse_string_arg(args, "summary")?);
    let lcov_path = PathBuf::from(parse_string_arg(args, "lcov")?);
    let out_path = PathBuf::from(parse_string_arg(args, "out")?);
    let policy_gate = parse_bool_flag(args, "policy-gate");
    let explicit_exec = parse_optional_f64_arg(args, "fail-under-exec-lines")?;
    let explicit_functions = parse_optional_f64_arg(args, "fail-under-functions")?;
    let explicit_regions = parse_optional_f64_arg(args, "fail-under-regions")?;
    let explicit_branches = parse_optional_f64_arg(args, "fail-under-branches")?;
    let explicit_require_branches = has_flag(args, "require-branches");
    let any_explicit_threshold = explicit_exec.is_some()
        || explicit_functions.is_some()
        || explicit_regions.is_some()
        || explicit_branches.is_some();
    let thresholds = if policy_gate {
        if any_explicit_threshold || explicit_require_branches {
            return Err(
                "--policy-gate cannot be combined with explicit threshold or branch flags"
                    .to_string(),
            );
        }
        let policy = read_coverage_policy(&coverage_policy_path(root))?;
        policy.thresholds_for_scope(&scope)
    } else {
        let Some(fail_under_exec_lines) = explicit_exec else {
            return Err(
                "missing coverage thresholds; pass --policy-gate or explicit --fail-under-* values"
                    .to_string(),
            );
        };
        let Some(fail_under_functions) = explicit_functions else {
            return Err(
                "missing coverage thresholds; pass --policy-gate or explicit --fail-under-* values"
                    .to_string(),
            );
        };
        let Some(fail_under_regions) = explicit_regions else {
            return Err(
                "missing coverage thresholds; pass --policy-gate or explicit --fail-under-* values"
                    .to_string(),
            );
        };
        let Some(fail_under_branches) = explicit_branches else {
            return Err(
                "missing coverage thresholds; pass --policy-gate or explicit --fail-under-* values"
                    .to_string(),
            );
        };
        CoverageThresholds {
            fail_under_exec_lines,
            fail_under_functions,
            fail_under_regions,
            fail_under_branches,
            require_branches: explicit_require_branches,
        }
    };

    let mut summary = read_summary_for_scope(&summary_path, Some(&scope))?;
    let lcov = read_lcov(&lcov_path)?;
    normalize_summary_for_gate(&scope, &summary_path, &lcov, &mut summary)?;
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
    write_gate_report(&out_path, &report)?;

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

fn normalize_summary_for_gate(
    scope: &str,
    summary_path: &Path,
    lcov: &LcovCoverage,
    summary: &mut CoverageSummary,
) -> Result<(), String> {
    if (lcov.executable_percent - 100.0).abs() >= f64::EPSILON {
        return Ok(());
    }
    let Some(branch_percent) = lcov.branch_percent else {
        return Ok(());
    };
    if (branch_percent - 100.0).abs() >= f64::EPSILON {
        return Ok(());
    }

    let details_path = coverage_details_path(summary_path);
    if !details_path.exists() {
        return Ok(());
    }

    let normalized = read_detailed_summary(&details_path, Some(scope))?;
    if (normalized.functions_percent - 100.0).abs() < f64::EPSILON {
        summary.functions_percent = normalized.functions_percent;
        summary.summary_regions_percent = normalized.regions_percent;
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn report_gate(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    report_gate_with_root(args, &root)
}

fn report_missing_gate_with_root(args: &[String], root: &Path) -> Result<(), String> {
    let scope = parse_string_arg(args, "scope")?;
    let out_path = PathBuf::from(parse_string_arg(args, "out")?);
    let reason = parse_string_arg(args, "reason")?;
    let policy = read_coverage_policy(&coverage_policy_path(root))?;
    let thresholds = policy.thresholds_for_scope(&scope);

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
            executable_lines_percent: 0.0,
            executable_lines_source: executable_source_label(ExecutableSource::Da).to_string(),
            functions_percent: 0.0,
            branches_percent: None,
            branches_available: false,
            summary_lines_percent: 0.0,
            summary_regions_percent: 0.0,
        },
        counts: CoverageGateReportCounts {
            executable_lines: CoverageCount {
                covered: 0,
                total: 0,
            },
            branches: CoverageCount {
                covered: 0,
                total: 0,
            },
        },
        result: CoverageGateReportResult {
            pass: false,
            fail_reasons: vec![reason.clone()],
        },
    };
    write_gate_report(&out_path, &report)?;
    eprintln!("{scope} gate fail: {reason}");
    Ok(())
}

fn write_gate_report(out_path: &Path, report: &CoverageGateReport) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report)
        .expect("serializing coverage gate report should succeed");
    if let Err(err) = fs::write(out_path, format!("{json}\n")) {
        return Err(format!("failed to write {}: {err}", out_path.display()));
    }
    Ok(())
}

fn coverage_report_path(reports_root: &Path, crate_name: &str) -> PathBuf {
    reports_root
        .join(crate_name.replace('-', "_"))
        .join("gate-report.json")
}

fn read_gate_report(path: &Path) -> Result<CoverageGateReport, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            return Err(format!(
                "failed to read gate report {}: {err}",
                path.display()
            ));
        }
    };
    match serde_json::from_str::<CoverageGateReport>(&raw) {
        Ok(report) => Ok(report),
        Err(err) => Err(format!(
            "failed to parse gate report {}: {err}",
            path.display()
        )),
    }
}

fn list_required_crates_with_root(root: &Path, writer: &mut dyn Write) -> Result<(), String> {
    let required_path = coverage_policy_path(root);
    let crates = read_required_crates(&required_path)?;
    write_crate_names_output(writer, crates, "required crates")
}

fn list_workspace_crates_with_root(root: &Path, writer: &mut dyn Write) -> Result<(), String> {
    let crates = read_workspace_crates(&root)?;
    write_crate_names_output(writer, crates, "workspace crates")
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

fn run_with_root(args: &[String], root: &Path) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("help") => Ok(()),
        Some("run-crate") => run_crate(&args[1..]),
        Some("report") => report_gate_with_root(&args[1..], root),
        Some("report-missing") => report_missing_gate_with_root(&args[1..], root),
        Some("refresh-summary") => {
            let reports_root = match parse_optional_string_arg(&args[1..], "reports-root") {
                Some(raw) => PathBuf::from(raw),
                None => PathBuf::from("target/coverage"),
            };
            let out_path = match parse_optional_string_arg(&args[1..], "out") {
                Some(raw) => PathBuf::from(raw),
                None => PathBuf::from("target/coverage/coverage-refresh.tsv"),
            };
            let status_out_path = match parse_optional_string_arg(&args[1..], "status-out") {
                Some(raw) => Some(PathBuf::from(raw)),
                None => None,
            };
            let required_crates = read_required_crates(&coverage_policy_path(root))?;

            let mut refresh_rows =
                String::from("crate\tstatus\texec\tfunc\tbranch\tregion\treport\n");
            let mut status_rows = String::from("crate\tstatus\n");

            for crate_name in required_crates {
                let report_path = coverage_report_path(&reports_root, &crate_name);
                let report = read_gate_report(&report_path)?;
                let status = if report.result.pass { "pass" } else { "fail" };
                let branch = report.measured.branches_percent.unwrap_or(0.0);
                refresh_rows.push_str(&format!(
                    "{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}\n",
                    crate_name,
                    status,
                    report.measured.executable_lines_percent,
                    report.measured.functions_percent,
                    branch,
                    report.measured.summary_regions_percent,
                    report_path.display()
                ));
                status_rows.push_str(&format!("{}\t{}\n", crate_name, status));
            }

            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        return Err(format!("failed to create {}: {err}", parent.display()));
                    }
                }
            }
            fs::write(&out_path, refresh_rows)
                .map_err(|err| format!("failed to write {}: {err}", out_path.display()))?;

            if let Some(status_out_path) = status_out_path {
                if let Some(parent) = status_out_path.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent).map_err(|err| {
                            format!("failed to create {}: {err}", parent.display())
                        })?;
                    }
                }
                fs::write(&status_out_path, status_rows).map_err(|err| {
                    format!("failed to write {}: {err}", status_out_path.display())
                })?;
            }

            Ok(())
        }
        Some("required-crates") => {
            let mut stdout = std::io::stdout().lock();
            list_required_crates_with_root(root, &mut stdout)
        }
        Some("workspace-crates") => {
            let mut stdout = std::io::stdout().lock();
            list_workspace_crates_with_root(root, &mut stdout)
        }
        Some(_) => Err("unknown sdk coverage subcommand".to_string()),
        None => Err("missing sdk coverage subcommand".to_string()),
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    run_with_root(args, &root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{self, Write};
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard, OnceLock};
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

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn recover_lock(lock: &'static Mutex<()>) -> MutexGuard<'static, ()> {
        match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock_cwd() -> MutexGuard<'static, ()> {
        recover_lock(cwd_lock())
    }

    fn collect_command_envs(cmd: &Command) -> BTreeMap<String, Option<String>> {
        let mut envs = BTreeMap::new();
        for (key, value) in cmd.get_envs() {
            envs.insert(
                key.to_string_lossy().to_string(),
                value.map(|raw| raw.to_string_lossy().to_string()),
            );
        }
        envs
    }

    fn ok_runner(_cmd: Command, _name: &str) -> Result<(), String> {
        Ok(())
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
    fn read_summary_normalizes_duplicate_generic_detail_records() {
        let root = temp_dir_path("summary_details_normalized");
        let summary_path = root.join("coverage-summary.json");
        write_file(
            &summary_path,
            r#"{
  "data": [
    {
      "totals": {
        "functions": {"percent": 100.0},
        "lines": {"percent": 88.5},
        "regions": {"percent": 22.0}
      }
    }
  ]
}"#,
        );
        write_file(
            &root.join("coverage-details.json"),
            r#"{
  "data": [
    {
      "functions": [
        {
          "count": 4,
          "filenames": ["/tmp/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 4, 0, 0, 0],
            [13, 1, 13, 8, 4, 0, 0, 0]
          ]
        },
        {
          "count": 0,
          "filenames": ["/tmp/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 0, 0, 0, 0],
            [13, 1, 13, 8, 0, 0, 0, 0]
          ]
        },
        {
          "count": 0,
          "filenames": ["/tmp/lib.rs"],
          "regions": [
            [20, 1, 20, 6, 0, 0, 0, 0]
          ]
        }
      ]
    }
  ]
}"#,
        );

        let summary = read_summary(&summary_path).expect("parse normalized summary");
        assert_eq!(summary.functions_percent, 100.0);
        assert_eq!(summary.summary_lines_percent, 88.5);
        assert_eq!(summary.summary_regions_percent, 100.0);

        fs::remove_dir_all(root).expect("remove summary details root");
    }

    #[test]
    fn read_summary_keeps_original_regions_when_functions_are_not_perfect() {
        let root = temp_dir_path("summary_details_not_applied");
        let summary_path = root.join("coverage-summary.json");
        write_file(
            &summary_path,
            r#"{
  "data": [
    {
      "totals": {
        "functions": {"percent": 95.0},
        "lines": {"percent": 88.5},
        "regions": {"percent": 22.0}
      }
    }
  ]
}"#,
        );
        write_file(
            &root.join("coverage-details.json"),
            r#"{
  "data": [
    {
      "functions": [
        {
          "count": 4,
          "filenames": ["/tmp/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 4, 0, 0, 0]
          ]
        }
      ]
    }
  ]
}"#,
        );

        let summary = read_summary(&summary_path).expect("parse preserved summary");
        assert_eq!(summary.functions_percent, 95.0);
        assert_eq!(summary.summary_regions_percent, 22.0);

        fs::remove_dir_all(root).expect("remove summary preserve root");
    }

    #[test]
    fn read_summary_for_scope_ignores_other_crate_detail_records() {
        let root = temp_dir_path("summary_details_scope_filtered");
        let summary_path = root.join("coverage-summary.json");
        write_file(
            &summary_path,
            r#"{
  "data": [
    {
      "totals": {
        "functions": {"percent": 100.0},
        "lines": {"percent": 88.5},
        "regions": {"percent": 22.0}
      }
    }
  ]
}"#,
        );
        write_file(
            &root.join("coverage-details.json"),
            r#"{
  "data": [
    {
      "functions": [
        {
          "count": 4,
          "filenames": ["/workspace/crates/a/src/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 4, 0, 0, 0]
          ]
        },
        {
          "count": 9,
          "filenames": ["/workspace/crates/b/src/lib.rs"],
          "regions": [
            [20, 1, 20, 6, 0, 0, 0, 0]
          ]
        }
      ]
    }
  ]
}"#,
        );

        let summary =
            read_summary_for_scope(&summary_path, Some("radroots_a")).expect("parse scope summary");
        assert_eq!(summary.functions_percent, 100.0);
        assert_eq!(summary.summary_lines_percent, 88.5);
        assert_eq!(summary.summary_regions_percent, 100.0);

        fs::remove_dir_all(root).expect("remove summary scope root");
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
    fn read_summary_reports_detail_parse_errors() {
        let root = temp_dir_path("summary_invalid_details");
        let summary_path = root.join("coverage-summary.json");
        write_file(
            &summary_path,
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
        );
        write_file(&root.join("coverage-details.json"), "{not-json");

        let err = read_summary(&summary_path).expect_err("invalid details should fail");
        assert!(err.contains("failed to parse coverage details"));

        fs::remove_dir_all(root).expect("remove invalid details root");
    }

    #[test]
    fn ignorable_question_mark_regions_require_single_char_question_mark() {
        let path = temp_file_path("coverage_question_mark_region");
        write_file(&path, "let value = call()?;\nreturn Err(());\n");
        let mut cache = BTreeMap::new();

        let question_mark = RegionCoverageKey {
            line_start: 1,
            column_start: 19,
            line_end: 1,
            column_end: 20,
            file_id: 0,
            expanded_file_id: 0,
            kind: 0,
        };
        assert!(is_ignorable_synthetic_region(
            path.to_str().expect("utf-8 path"),
            &question_mark,
            &mut cache,
        ));

        let not_question_mark = RegionCoverageKey {
            line_start: 2,
            column_start: 8,
            line_end: 2,
            column_end: 15,
            file_id: 0,
            expanded_file_id: 0,
            kind: 0,
        };
        assert!(!is_ignorable_synthetic_region(
            path.to_str().expect("utf-8 path"),
            &not_question_mark,
            &mut cache,
        ));

        fs::remove_file(path).expect("remove question mark source");
    }

    #[test]
    fn ignorable_unexpected_panic_regions_require_test_fallback_lines() {
        let root = temp_dir_path("coverage_unexpected_panic_region");
        let path = root.join("tests.rs");
        write_file(
            &path,
            "match &err {\n    RuntimeProtectedFileError::Io { .. } => {}\n        other => panic!(\"unexpected io error: {other}\"),\n}\n",
        );
        let mut cache = BTreeMap::new();

        let other_region = RegionCoverageKey {
            line_start: 3,
            column_start: 9,
            line_end: 3,
            column_end: 14,
            file_id: 0,
            expanded_file_id: 0,
            kind: 0,
        };
        assert!(is_ignorable_synthetic_region(
            path.to_str().expect("utf-8 path"),
            &other_region,
            &mut cache,
        ));

        let panic_region = RegionCoverageKey {
            line_start: 3,
            column_start: 18,
            line_end: 3,
            column_end: 24,
            file_id: 0,
            expanded_file_id: 0,
            kind: 0,
        };
        assert!(is_ignorable_synthetic_region(
            path.to_str().expect("utf-8 path"),
            &panic_region,
            &mut cache,
        ));

        let non_test_path = root.join("source.rs");
        write_file(
            &non_test_path,
            "match &err {\n    RuntimeProtectedFileError::Io { .. } => {}\n        other => panic!(\"unexpected io error: {other}\"),\n}\n",
        );
        assert!(!is_ignorable_synthetic_region(
            non_test_path.to_str().expect("utf-8 path"),
            &other_region,
            &mut cache,
        ));

        fs::remove_dir_all(root).expect("remove unexpected panic source");
    }

    #[test]
    fn read_coverage_policy_rejects_non_finite_and_out_of_range_thresholds() {
        let non_finite = temp_file_path("coverage_policy_non_finite");
        write_file(
            &non_finite,
            "[gate]\nfail_under_exec_lines = inf\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let non_finite_err =
            read_coverage_policy(&non_finite).expect_err("non-finite threshold should fail");
        assert!(non_finite_err.contains("must be finite"));
        fs::remove_file(non_finite).expect("remove non-finite policy");

        let out_of_range = temp_file_path("coverage_policy_out_of_range");
        write_file(
            &out_of_range,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 101.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let out_of_range_err =
            read_coverage_policy(&out_of_range).expect_err("out-of-range threshold should fail");
        assert!(out_of_range_err.contains("must be within 0..=100"));
        fs::remove_file(out_of_range).expect("remove out-of-range policy");
    }

    #[test]
    fn coverage_policy_resolves_scope_specific_temporary_overrides() {
        let path = temp_file_path("coverage_policy_override_scope");
        write_file(
            &path,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\", \"radroots_b\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 88.5\nfail_under_functions = 77.5\nfail_under_regions = 66.5\nfail_under_branches = 55.5\nrequire_branches = false\ntemporary = true\nreason = \"temporary publish unblocker\"\n",
        );
        let policy = read_coverage_policy(&path).expect("parse scoped override policy");
        let override_thresholds = policy.thresholds_for_scope("radroots_a");
        assert_eq!(override_thresholds.fail_under_exec_lines, 88.5);
        assert_eq!(override_thresholds.fail_under_functions, 77.5);
        assert_eq!(override_thresholds.fail_under_regions, 66.5);
        assert_eq!(override_thresholds.fail_under_branches, 55.5);
        assert!(!override_thresholds.require_branches);

        let default_thresholds = policy.thresholds_for_scope("radroots_b");
        assert_eq!(default_thresholds.fail_under_exec_lines, 100.0);
        assert_eq!(default_thresholds.fail_under_functions, 100.0);
        assert_eq!(default_thresholds.fail_under_regions, 100.0);
        assert_eq!(default_thresholds.fail_under_branches, 100.0);
        assert!(default_thresholds.require_branches);

        fs::remove_file(path).expect("remove override scope policy");
    }

    #[test]
    fn read_coverage_policy_rejects_invalid_override_shapes() {
        let non_required = temp_file_path("coverage_policy_override_non_required");
        write_file(
            &non_required,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_b]\nfail_under_exec_lines = 90.0\ntemporary = true\nreason = \"temporary publish unblocker\"\n",
        );
        let non_required_err =
            read_coverage_policy(&non_required).expect_err("non-required override should fail");
        assert!(non_required_err.contains("must target a required crate"));
        fs::remove_file(non_required).expect("remove non-required override policy");

        let missing_temporary = temp_file_path("coverage_policy_override_missing_temporary");
        write_file(
            &missing_temporary,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 90.0\ntemporary = false\nreason = \"temporary publish unblocker\"\n",
        );
        let missing_temporary_err = read_coverage_policy(&missing_temporary)
            .expect_err("override without temporary=true should fail");
        assert!(missing_temporary_err.contains("temporary = true"));
        fs::remove_file(missing_temporary).expect("remove temporary override policy");

        let missing_reason = temp_file_path("coverage_policy_override_missing_reason");
        write_file(
            &missing_reason,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 90.0\ntemporary = true\nreason = \"  \"\n",
        );
        let missing_reason_err =
            read_coverage_policy(&missing_reason).expect_err("blank override reason should fail");
        assert!(missing_reason_err.contains("non-empty reason"));
        fs::remove_file(missing_reason).expect("remove missing reason policy");

        let stricter = temp_file_path("coverage_policy_override_stricter");
        write_file(
            &stricter,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 100.1\ntemporary = true\nreason = \"temporary publish unblocker\"\n",
        );
        let stricter_err =
            read_coverage_policy(&stricter).expect_err("stricter override should fail");
        assert!(stricter_err.contains("must be within 0..=100"));
        fs::remove_file(stricter).expect("remove stricter override policy");
    }

    #[test]
    fn report_missing_gate_uses_policy_thresholds() {
        let root = temp_dir_path("report_missing_gate_root");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let out_path = root.join("gate-report.json");

        report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &root,
        )
        .expect("report missing gate");

        let report_raw = fs::read_to_string(&out_path).expect("read gate report");
        let report_json: serde_json::Value =
            serde_json::from_str(&report_raw).expect("parse gate report json");
        assert_eq!(
            report_json["thresholds"]["executable_lines"],
            serde_json::json!(100.0)
        );
        assert_eq!(
            report_json["thresholds"]["branches_required"],
            serde_json::json!(true)
        );
        assert_eq!(report_json["result"]["pass"], serde_json::json!(false));
        assert_eq!(
            report_json["result"]["fail_reasons"],
            serde_json::json!(["missing-coverage-artifacts"])
        );

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn report_missing_gate_uses_scope_specific_override_thresholds() {
        let root = temp_dir_path("report_missing_gate_override_root");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 88.5\nfail_under_functions = 77.5\nfail_under_regions = 66.5\nfail_under_branches = 55.5\nrequire_branches = false\ntemporary = true\nreason = \"temporary publish unblocker\"\n",
        );
        let out_path = root.join("gate-report.json");

        report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a".to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &root,
        )
        .expect("report missing gate with override");

        let report_raw = fs::read_to_string(&out_path).expect("read gate report");
        let report_json: serde_json::Value =
            serde_json::from_str(&report_raw).expect("parse gate report json");
        assert_eq!(
            report_json["thresholds"]["executable_lines"],
            serde_json::json!(88.5)
        );
        assert_eq!(
            report_json["thresholds"]["functions"],
            serde_json::json!(77.5)
        );
        assert_eq!(
            report_json["thresholds"]["regions"],
            serde_json::json!(66.5)
        );
        assert_eq!(
            report_json["thresholds"]["branches"],
            serde_json::json!(55.5)
        );
        assert_eq!(
            report_json["thresholds"]["branches_required"],
            serde_json::json!(false)
        );

        fs::remove_dir_all(root).expect("remove override root");
    }

    #[test]
    fn report_missing_gate_reports_argument_policy_and_write_errors() {
        let root = temp_dir_path("report_missing_gate_error_root");
        let missing_scope =
            report_missing_gate_with_root(&[], &root).expect_err("missing scope should fail");
        assert!(missing_scope.contains("missing --scope"));

        let missing_out = report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &root,
        )
        .expect_err("missing out should fail");
        assert!(missing_out.contains("missing --out"));

        let missing_reason = report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--out".to_string(),
                root.join("missing-gate.json").display().to_string(),
            ],
            &root,
        )
        .expect_err("missing reason should fail");
        assert!(missing_reason.contains("missing --reason"));

        let policy_err = report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--out".to_string(),
                root.join("missing-gate.json").display().to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &root,
        )
        .expect_err("missing policy should fail");
        assert!(policy_err.contains("failed to read coverage policy"));

        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let out_path = root.join("gate-report.json");
        fs::create_dir_all(&out_path).expect("create blocking output dir");
        let write_err = report_missing_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &root,
        )
        .expect_err("directory output should fail");
        assert!(write_err.contains("failed to write"));

        fs::remove_dir_all(root).expect("remove report missing gate error root");
    }

    #[test]
    fn refresh_summary_uses_measured_gate_report_values() {
        let root = temp_dir_path("refresh_summary_root");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );

        let reports_root = root.join("target").join("coverage");
        let crate_dir = reports_root.join("radroots_a");
        fs::create_dir_all(&crate_dir).expect("create crate dir");
        write_file(
            &crate_dir.join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 97.5
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": true,
    "fail_reasons": []
  }
}"#,
        );

        let refresh_out = reports_root.join("coverage-refresh.tsv");
        let status_out = reports_root.join("coverage-refresh-status.tsv");
        run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                reports_root.display().to_string(),
                "--out".to_string(),
                refresh_out.display().to_string(),
                "--status-out".to_string(),
                status_out.display().to_string(),
            ],
            &root,
        )
        .expect("write refresh summary");

        let refresh = fs::read_to_string(&refresh_out).expect("read refresh summary");
        assert!(refresh.contains("crate\tstatus\texec\tfunc\tbranch\tregion\treport"));
        assert!(
            refresh.contains("radroots_a\tpass\t100.000000\t100.000000\t100.000000\t97.500000\t")
        );

        let status = fs::read_to_string(&status_out).expect("read status summary");
        assert_eq!(status, "crate\tstatus\nradroots_a\tpass\n");

        fs::remove_dir_all(root).expect("remove root");

        let defaults_root = temp_dir_path("refresh_summary_defaults_root");
        let defaults_coverage_dir = defaults_root.join("policy").join("coverage");
        fs::create_dir_all(&defaults_coverage_dir).expect("create defaults coverage dir");
        write_file(
            &defaults_coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        write_file(
            &defaults_root
                .join("target")
                .join("coverage")
                .join("radroots_a")
                .join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 100.0
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": false,
    "fail_reasons": ["synthetic-fail"]
  }
}"#,
        );

        let _guard = lock_cwd();
        let previous_dir = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(&defaults_root).expect("set current dir");
        run_with_root(&["refresh-summary".to_string()], &defaults_root)
            .expect("write refresh summary defaults");
        let defaults_refresh = fs::read_to_string(
            defaults_root
                .join("target")
                .join("coverage")
                .join("coverage-refresh.tsv"),
        )
        .expect("read defaults refresh summary");
        assert!(
            defaults_refresh
                .contains("radroots_a\tfail\t100.000000\t100.000000\t100.000000\t100.000000\t")
        );

        let dispatch_root = temp_dir_path("refresh_summary_parentless_root");
        let dispatch_coverage_dir = dispatch_root.join("policy").join("coverage");
        fs::create_dir_all(&dispatch_coverage_dir).expect("create dispatch coverage dir");
        write_file(
            &dispatch_coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        write_file(
            &dispatch_root.join("Cargo.toml"),
            "[workspace]\nmembers = []\nresolver = \"2\"\n",
        );
        write_file(
            &dispatch_root
                .join("target")
                .join("coverage")
                .join("radroots_a")
                .join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 100.0
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": true,
    "fail_reasons": []
  }
}"#,
        );
        std::env::set_current_dir(&dispatch_root).expect("set dispatch current dir");
        run_with_root(
            &[
                "report-missing".to_string(),
                "--scope".to_string(),
                "radroots_a_blocking".to_string(),
                "--out".to_string(),
                "missing-gate.json".to_string(),
                "--reason".to_string(),
                "missing-coverage-artifacts".to_string(),
            ],
            &dispatch_root,
        )
        .expect("dispatch report-missing");
        run_with_root(
            &[
                "refresh-summary".to_string(),
                "--out".to_string(),
                "coverage-refresh.tsv".to_string(),
                "--status-out".to_string(),
                "coverage-refresh-status.tsv".to_string(),
            ],
            &dispatch_root,
        )
        .expect("dispatch refresh-summary");
        std::env::set_current_dir(previous_dir).expect("restore current dir");

        assert!(dispatch_root.join("missing-gate.json").exists());
        assert!(dispatch_root.join("coverage-refresh.tsv").exists());
        assert!(dispatch_root.join("coverage-refresh-status.tsv").exists());

        fs::remove_dir_all(defaults_root).expect("remove defaults root");
        fs::remove_dir_all(dispatch_root).expect("remove dispatch root");
    }

    #[test]
    fn refresh_summary_rejects_empty_output_paths() {
        let root = temp_dir_path("refresh_summary_empty_paths_root");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("radroots_a")
                .join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 100.0
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": true,
    "fail_reasons": []
  }
}"#,
        );

        let out_err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
                "--out".to_string(),
                String::new(),
            ],
            &root,
        )
        .expect_err("empty out path should fail");
        assert!(out_err.contains("failed to write"));

        let status_err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
                "--out".to_string(),
                root.join("target")
                    .join("coverage")
                    .join("coverage-refresh.tsv")
                    .display()
                    .to_string(),
                "--status-out".to_string(),
                String::new(),
            ],
            &root,
        )
        .expect_err("empty status out path should fail");
        assert!(status_err.contains("failed to write"));

        fs::remove_dir_all(root).expect("remove empty path root");
    }

    #[test]
    fn refresh_summary_reports_output_parent_creation_failure() {
        let root = temp_dir_path("refresh_summary_out_parent_fail");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("radroots_a")
                .join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 100.0
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": true,
    "fail_reasons": []
  }
}"#,
        );
        write_file(&root.join("out-blocker"), "x");

        let err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
                "--out".to_string(),
                root.join("out-blocker")
                    .join("nested")
                    .join("coverage-refresh.tsv")
                    .display()
                    .to_string(),
            ],
            &root,
        )
        .expect_err("out parent create failure should bubble up");
        assert!(err.contains("failed to create"));

        fs::remove_dir_all(root).expect("remove out parent fail root");
    }

    #[test]
    fn refresh_summary_reports_status_output_parent_creation_failure() {
        let root = temp_dir_path("refresh_summary_status_parent_fail");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        write_file(
            &root
                .join("target")
                .join("coverage")
                .join("radroots_a")
                .join("gate-report.json"),
            r#"{
  "scope": "radroots_a",
  "thresholds": {
    "executable_lines": 100.0,
    "functions": 100.0,
    "regions": 100.0,
    "branches": 100.0,
    "branches_required": true
  },
  "measured": {
    "executable_lines_percent": 100.0,
    "executable_lines_source": "da",
    "functions_percent": 100.0,
    "branches_percent": 100.0,
    "branches_available": true,
    "summary_lines_percent": 100.0,
    "summary_regions_percent": 100.0
  },
  "counts": {
    "executable_lines": {
      "covered": 4,
      "total": 4
    },
    "branches": {
      "covered": 2,
      "total": 2
    }
  },
  "result": {
    "pass": true,
    "fail_reasons": []
  }
}"#,
        );
        write_file(&root.join("status-blocker"), "x");

        let err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
                "--out".to_string(),
                root.join("target")
                    .join("coverage")
                    .join("coverage-refresh.tsv")
                    .display()
                    .to_string(),
                "--status-out".to_string(),
                root.join("status-blocker")
                    .join("nested")
                    .join("coverage-refresh-status.tsv")
                    .display()
                    .to_string(),
            ],
            &root,
        )
        .expect_err("status-out parent create failure should bubble up");
        assert!(err.contains("failed to create"));

        fs::remove_dir_all(root).expect("remove status parent fail root");
    }

    #[test]
    fn refresh_summary_reports_policy_and_gate_report_errors() {
        let root = temp_dir_path("refresh_summary_error_root");
        let policy_err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
            ],
            &root,
        )
        .expect_err("missing policy should fail");
        assert!(policy_err.contains("failed to read coverage policy"));

        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n",
        );
        let gate_err = run_with_root(
            &[
                "refresh-summary".to_string(),
                "--reports-root".to_string(),
                root.join("target").join("coverage").display().to_string(),
            ],
            &root,
        )
        .expect_err("missing gate report should fail");
        assert!(gate_err.contains("failed to read gate report"));

        fs::remove_dir_all(root).expect("remove refresh summary error root");
    }

    #[test]
    fn recover_lock_covers_ok_and_poisoned_paths() {
        let ok_lock: &'static Mutex<()> = Box::leak(Box::new(Mutex::new(())));
        let _ok_guard = recover_lock(ok_lock);

        let poisoned_lock: &'static Mutex<()> = Box::leak(Box::new(Mutex::new(())));
        let handle = std::thread::spawn(move || {
            let _guard = poisoned_lock.lock().expect("lock poisoned mutex");
            panic!("poison test mutex");
        });
        assert!(handle.join().is_err());

        let _poisoned_guard = recover_lock(poisoned_lock);
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
        fs::write(
            &path,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"a\", \"b\"]\n",
        )
        .expect("write required crates");
        let crates = read_required_crates(&path).expect("parse required crates");
        assert_eq!(crates, vec!["a".to_string(), "b".to_string()]);
        fs::remove_file(&path).expect("remove required crates");

        let dup_path = temp_file_path("required_crates_dup");
        fs::write(
            &dup_path,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"a\", \"a\"]\n",
        )
        .expect("write dup required crates");
        let err = read_required_crates(&dup_path).expect_err("duplicate required crates");
        assert!(err.contains("duplicate crate a"));
        fs::remove_file(dup_path).expect("remove dup required crates");
    }

    #[test]
    fn read_required_crates_reports_read_and_parse_errors() {
        let missing = temp_file_path("required_missing");
        let read_err = read_required_crates(&missing).expect_err("missing required file");
        assert!(read_err.contains("failed to read coverage policy"));

        let invalid = temp_file_path("required_invalid");
        write_file(&invalid, "not = [toml");
        let parse_err = read_required_crates(&invalid).expect_err("invalid required file");
        assert!(parse_err.contains("failed to parse coverage policy"));
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
        let profile = read_coverage_profile(&root, "radroots_log").expect("read profile");
        assert!(!profile.no_default_features);
        assert!(profile.features.is_empty());
        assert_eq!(profile.test_threads, None);
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_merge_defaults_and_crate_overrides() {
        let root = temp_dir_path("profile_merge");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.default]
no_default_features = false
features = ["std"]
test_threads = 2

[profiles.crates."radroots_log"]
no_default_features = true
features = ["rt"]
"#,
        )
        .expect("write profiles");

        let app_profile = read_coverage_profile(&root, "radroots_log").expect("app profile");
        assert!(app_profile.no_default_features);
        assert_eq!(app_profile.features, vec!["rt".to_string()]);
        assert_eq!(app_profile.test_threads, Some(2));

        let other_profile = read_coverage_profile(&root, "radroots_types").expect("other profile");
        assert!(!other_profile.no_default_features);
        assert_eq!(other_profile.features, vec!["std".to_string()]);
        assert_eq!(other_profile.test_threads, Some(2));

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_accept_positive_test_threads() {
        let root = temp_dir_path("profile_positive_threads");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots_log"]
test_threads = 4
"#,
        )
        .expect("write profiles");
        let profile =
            read_coverage_profile(&root, "radroots_log").expect("valid positive thread profile");
        assert_eq!(profile.test_threads, Some(4));
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_invalid_feature_and_thread_values() {
        let root = temp_dir_path("profile_invalid");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots_log"]
features = [""]
test_threads = 0
"#,
        )
        .expect("write profiles");

        let err = read_coverage_profile(&root, "radroots_log").expect_err("invalid profile");
        assert!(
            err.contains("empty feature value"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_invalid_toml() {
        let root = temp_dir_path("profile_invalid_toml");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(coverage_dir.join("profiles.toml"), "[profiles.default\n")
            .expect("write invalid profiles");
        let err = read_coverage_profile(&root, "radroots_log").expect_err("invalid toml");
        assert!(err.contains("failed to parse"));
        fs::remove_dir_all(root).expect("remove root");
    }

    #[test]
    fn coverage_profiles_reject_zero_test_threads_without_feature_error() {
        let root = temp_dir_path("profile_invalid_threads");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        fs::write(
            coverage_dir.join("profiles.toml"),
            r#"[profiles.crates."radroots_log"]
test_threads = 0
"#,
        )
        .expect("write profiles");

        let err = read_coverage_profile(&root, "radroots_log").expect_err("invalid thread count");
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
        write_file(
            &empty_path,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = []\n",
        );
        let empty_err = read_required_crates(&empty_path).expect_err("empty required list");
        assert!(empty_err.contains("must not be empty"));
        fs::remove_file(&empty_path).expect("remove empty required file");

        let blank_path = temp_file_path("required_blank");
        write_file(
            &blank_path,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"a\", \" \"]\n",
        );
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
            parse_toml::<CoveragePolicyFile>(&missing).expect_err("missing file should fail");
        assert!(read_err.contains("failed to read"));

        let invalid = temp_file_path("parse_toml_invalid");
        write_file(&invalid, "[gate]\nfail_under_exec_lines = 100.0\n");
        let parse_err =
            parse_toml::<CoveragePolicyFile>(&invalid).expect_err("invalid toml should fail");
        assert!(parse_err.contains("failed to parse"));
        fs::remove_file(invalid).expect("remove invalid toml");

        let workspace_missing = temp_file_path("parse_toml_workspace_missing");
        let workspace_read_err = parse_toml::<WorkspaceManifest>(&workspace_missing)
            .expect_err("missing workspace manifest should fail");
        assert!(workspace_read_err.contains("failed to read"));

        let workspace_invalid = temp_file_path("parse_toml_workspace_invalid");
        write_file(&workspace_invalid, "[workspace");
        let workspace_parse_err = parse_toml::<WorkspaceManifest>(&workspace_invalid)
            .expect_err("invalid workspace manifest should fail");
        assert!(workspace_parse_err.contains("failed to parse"));
        fs::remove_file(workspace_invalid).expect("remove invalid workspace manifest");

        let package_missing = temp_file_path("parse_toml_package_missing");
        let package_read_err = parse_toml::<PackageManifest>(&package_missing)
            .expect_err("missing package manifest should fail");
        assert!(package_read_err.contains("failed to read"));

        let package_invalid = temp_file_path("parse_toml_package_invalid");
        write_file(&package_invalid, "[package");
        let package_parse_err = parse_toml::<PackageManifest>(&package_invalid)
            .expect_err("invalid package manifest should fail");
        assert!(package_parse_err.contains("failed to parse"));
        fs::remove_file(package_invalid).expect("remove invalid package manifest");

        let profiles_missing = temp_file_path("parse_toml_profiles_missing");
        let profiles_read_err = parse_toml::<CoverageProfilesFile>(&profiles_missing)
            .expect_err("missing coverage profiles should fail");
        assert!(profiles_read_err.contains("failed to read"));

        let profiles_invalid = temp_file_path("parse_toml_profiles_invalid");
        write_file(&profiles_invalid, "[profiles.default");
        let profiles_parse_err = parse_toml::<CoverageProfilesFile>(&profiles_invalid)
            .expect_err("invalid coverage profiles should fail");
        assert!(profiles_parse_err.contains("failed to parse"));
        fs::remove_file(profiles_invalid).expect("remove invalid coverage profiles");
    }

    #[test]
    fn parse_toml_parses_valid_coverage_required_contract() {
        let valid = temp_file_path("parse_toml_valid");
        write_file(
            &valid,
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_core\"]\n",
        );
        let parsed = parse_toml::<CoveragePolicyFile>(&valid).expect("valid toml");
        assert_eq!(parsed.required.crates, vec!["radroots_core".to_string()]);
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
            "radroots_core".to_string(),
            "--out".to_string(),
            out.display().to_string(),
            "--test-threads".to_string(),
            "3".to_string(),
        ];
        let mut names = Vec::new();
        let mut rendered_commands = Vec::new();
        let mut runner = |cmd: Command, name: &str| {
            names.push(name.to_string());
            let rendered = cmd
                .get_args()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!rendered.is_empty());
            rendered_commands.push(rendered);
            Ok(())
        };
        run_crate_with_runner(&args, &mut runner).expect("run crate with stub runner");
        assert_eq!(
            names,
            vec![
                "cargo llvm-cov clean --workspace".to_string(),
                "cargo llvm-cov --no-report".to_string(),
                "cargo llvm-cov report --json --summary-only".to_string(),
                "cargo llvm-cov report --json".to_string(),
                "cargo llvm-cov report --lcov".to_string(),
            ]
        );
        assert!(
            rendered_commands
                .iter()
                .filter(|rendered| rendered.contains("report -p radroots_core"))
                .all(|rendered| rendered.contains("--ignore-filename-regex"))
        );
        assert!(
            rendered_commands
                .iter()
                .filter(|rendered| rendered.contains("report -p radroots_core"))
                .all(|rendered| rendered.contains(COVERAGE_EXTERNAL_IGNORE_FILENAME_REGEX))
        );
        fs::remove_dir_all(out).expect("remove run crate output dir");
    }

    #[test]
    fn coverage_ignore_filename_regex_excludes_external_and_sibling_workspace_paths() {
        let root = workspace_root();
        let ignore_regex =
            coverage_ignore_filename_regex(&root, "radroots_core").expect("build ignore regex");
        assert!(ignore_regex.contains(COVERAGE_EXTERNAL_IGNORE_FILENAME_REGEX));
        assert!(ignore_regex.contains("crates/identity"));
        assert!(!ignore_regex.contains("crates/core/"));
    }

    #[test]
    fn escape_regex_literal_escapes_regex_metacharacters() {
        let escaped = escape_regex_literal(r"\.+*?()|[]{}^$");
        assert_eq!(escaped, r"\\\.\+\*\?\(\)\|\[\]\{\}\^\$");
    }

    #[test]
    fn coverage_cargo_command_defaults_to_rustup_nightly() {
        let cmd = coverage_cargo_command_with_override(None);
        let mut args = Vec::new();
        for arg in cmd.get_args() {
            args.push(arg.to_string_lossy().to_string());
        }

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
    fn normalized_coverage_cargo_override_trims_and_filters_values() {
        assert_eq!(
            normalized_coverage_cargo_override(Some("  /tmp/cargo  ".to_string())),
            Some("/tmp/cargo".to_string())
        );
        assert_eq!(
            normalized_coverage_cargo_override(Some("   ".to_string())),
            None
        );
        assert_eq!(normalized_coverage_cargo_override(None), None);
    }

    fn assert_coverage_command_shapes(
        cargo_cmd: Command,
        llvm_cov_cmd: Command,
        override_binary: Option<&str>,
    ) {
        match override_binary {
            Some(binary) => assert_eq!(cargo_cmd.get_program().to_string_lossy(), binary),
            None => assert_eq!(cargo_cmd.get_program().to_string_lossy(), "rustup"),
        }

        let llvm_args = llvm_cov_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        match override_binary {
            Some(_) => assert_eq!(llvm_args, vec!["llvm-cov".to_string()]),
            None => assert_eq!(
                llvm_args,
                vec![
                    "run".to_string(),
                    "nightly".to_string(),
                    "cargo".to_string(),
                    "llvm-cov".to_string()
                ]
            ),
        }
    }

    #[test]
    fn coverage_public_command_helpers_match_current_env_resolution() {
        let mut default_llvm_cov_cmd = coverage_cargo_command_with_override(None);
        default_llvm_cov_cmd.arg("llvm-cov");
        assert_coverage_command_shapes(
            coverage_cargo_command_with_override(None),
            default_llvm_cov_cmd,
            None,
        );

        let explicit_binary = temp_dir_path("coverage_command_override")
            .join("nightly-cargo")
            .to_string_lossy()
            .to_string();
        let mut explicit_llvm_cov_cmd =
            coverage_cargo_command_with_override(Some(&explicit_binary));
        explicit_llvm_cov_cmd.arg("llvm-cov");
        assert_coverage_command_shapes(
            coverage_cargo_command_with_override(Some(&explicit_binary)),
            explicit_llvm_cov_cmd,
            Some(explicit_binary.as_str()),
        );

        let override_binary =
            normalized_coverage_cargo_override(std::env::var("RADROOTS_COVERAGE_CARGO").ok());
        assert_coverage_command_shapes(
            coverage_cargo_command(),
            coverage_llvm_cov_command(),
            override_binary.as_deref(),
        );
    }

    #[test]
    fn configure_coverage_toolchain_env_sets_existing_binary_envs() {
        let toolchain_dir = temp_dir_path("coverage_toolchain_env");
        fs::create_dir_all(&toolchain_dir).expect("create toolchain env dir");
        for binary in ["rustc", "rustdoc", "llvm-cov", "llvm-profdata"] {
            write_file(&toolchain_dir.join(binary), "");
        }

        let mut cmd = Command::new("cargo");
        configure_coverage_toolchain_env(&mut cmd, &toolchain_dir);
        let envs = collect_command_envs(&cmd);
        assert_eq!(
            envs.get("RUSTC"),
            Some(&Some(
                toolchain_dir.join("rustc").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("RUSTDOC"),
            Some(&Some(
                toolchain_dir.join("rustdoc").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("LLVM_COV"),
            Some(&Some(
                toolchain_dir.join("llvm-cov").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("LLVM_PROFDATA"),
            Some(&Some(
                toolchain_dir
                    .join("llvm-profdata")
                    .to_string_lossy()
                    .to_string()
            ))
        );

        fs::remove_dir_all(toolchain_dir).expect("remove toolchain env dir");
    }

    #[test]
    fn configure_coverage_toolchain_env_skips_missing_binary_envs() {
        let toolchain_dir = temp_dir_path("coverage_toolchain_missing_env");
        fs::create_dir_all(&toolchain_dir).expect("create missing env dir");

        let mut cmd = Command::new("cargo");
        configure_coverage_toolchain_env(&mut cmd, &toolchain_dir);
        let envs = collect_command_envs(&cmd);
        assert!(!envs.contains_key("RUSTC"));
        assert!(!envs.contains_key("RUSTDOC"));
        assert!(!envs.contains_key("LLVM_COV"));
        assert!(!envs.contains_key("LLVM_PROFDATA"));

        fs::remove_dir_all(toolchain_dir).expect("remove missing env dir");
    }

    #[test]
    fn coverage_cargo_command_override_variants_cover_parented_and_parentless_paths() {
        let toolchain_dir = temp_dir_path("coverage_toolchain_override");
        fs::create_dir_all(&toolchain_dir).expect("create toolchain dir");
        for binary in [
            "nightly-cargo",
            "rustc",
            "rustdoc",
            "llvm-cov",
            "llvm-profdata",
        ] {
            write_file(&toolchain_dir.join(binary), "");
        }

        let default_cmd = coverage_cargo_command_with_override(None);
        let mut args = Vec::new();
        for arg in default_cmd.get_args() {
            args.push(arg.to_string_lossy().to_string());
        }
        assert_eq!(default_cmd.get_program().to_string_lossy(), "rustup");
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "nightly".to_string(),
                "cargo".to_string()
            ]
        );

        let override_binary = toolchain_dir.join("nightly-cargo");
        let cmd = coverage_cargo_command_with_override(Some(
            override_binary
                .to_str()
                .expect("override path should be utf-8"),
        ));

        assert_eq!(
            cmd.get_program().to_string_lossy(),
            override_binary.to_string_lossy()
        );
        assert!(cmd.get_args().next().is_none());
        let mut envs = collect_command_envs(&cmd);
        envs.insert("MISSING".to_string(), None);
        assert_eq!(
            envs.get("RUSTC"),
            Some(&Some(
                toolchain_dir.join("rustc").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("RUSTDOC"),
            Some(&Some(
                toolchain_dir.join("rustdoc").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("LLVM_COV"),
            Some(&Some(
                toolchain_dir.join("llvm-cov").to_string_lossy().to_string()
            ))
        );
        assert_eq!(
            envs.get("LLVM_PROFDATA"),
            Some(&Some(
                toolchain_dir
                    .join("llvm-profdata")
                    .to_string_lossy()
                    .to_string()
            ))
        );
        let path_env = envs
            .get("PATH")
            .and_then(|value| value.as_ref())
            .expect("override binary should prepend PATH");
        assert!(path_env.starts_with(toolchain_dir.to_string_lossy().as_ref()));
        let mut cmd = coverage_cargo_command_with_override(Some("/"));
        cmd.env_remove("RUSTC");
        cmd.env_remove("LLVM_COV");
        assert_eq!(cmd.get_program().to_string_lossy(), "/");
        let envs = collect_command_envs(&cmd);
        assert_eq!(envs.get("RUSTC"), Some(&None));
        assert_eq!(envs.get("LLVM_COV"), Some(&None));

        fs::remove_dir_all(toolchain_dir).expect("remove toolchain dir");
    }

    #[test]
    fn workspace_root_override_takes_precedence() {
        let root = workspace_root_with_override(Some("/tmp/radroots-coverage-root"));
        assert_eq!(root, PathBuf::from("/tmp/radroots-coverage-root"));

        let fallback = workspace_root_with_override(Some(""));
        assert!(fallback.join("Cargo.toml").exists());

        let default_root = workspace_root_with_override(None);
        assert!(default_root.join("Cargo.toml").exists());
    }

    #[test]
    fn prepend_toolchain_bin_to_path_covers_missing_and_existing_path_inputs() {
        let toolchain_dir = PathBuf::from("/tmp/radroots-coverage-toolchain");
        let no_path = prepend_toolchain_bin_to_path(&toolchain_dir, None);
        assert_eq!(no_path, OsString::from(&toolchain_dir));

        let joined =
            prepend_toolchain_bin_to_path(&toolchain_dir, Some(OsString::from("/usr/bin:/bin")));
        let joined = joined.to_string_lossy().to_string();
        assert!(joined.starts_with("/tmp/radroots-coverage-toolchain"));
        assert!(joined.contains("/usr/bin"));
    }

    #[test]
    fn collect_command_envs_cover_helper_paths() {
        let mut cmd = Command::new("sh");
        cmd.env("PRESENT", "value");
        cmd.env_remove("REMOVED");
        let envs = collect_command_envs(&cmd);
        assert_eq!(envs.get("PRESENT"), Some(&Some("value".to_string())));
        assert_eq!(envs.get("REMOVED"), Some(&None));
    }

    #[test]
    fn ok_runner_helper_returns_success() {
        let cmd = Command::new("true");
        assert!(ok_runner(cmd, "noop").is_ok());
    }

    #[test]
    fn run_crate_with_runner_uses_default_output_dir_when_out_is_missing() {
        let args = vec!["--crate".to_string(), "radroots_core".to_string()];
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
                    .any(|arg| arg.ends_with("coverage-details.json"))
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
            "radroots_core".to_string(),
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
            "radroots_core".to_string(),
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
        let write_minimal_workspace = |root: &Path| {
            write_file(
                &root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/core\"]\n",
            );
            write_file(
                &root.join("crates").join("core").join("Cargo.toml"),
                "[package]\nname = \"radroots_core\"\nversion = \"0.1.0-alpha.1\"\nedition = \"2024\"\n",
            );
        };

        let profile_root = temp_dir_path("run_crate_profile_invalid");
        write_minimal_workspace(&profile_root);
        write_file(
            &profile_root
                .join("policy")
                .join("coverage")
                .join("profiles.toml"),
            "[profiles.default]\nfeatures = [\"\"]\n",
        );
        let profile_args = vec![
            "--crate".to_string(),
            "radroots_core".to_string(),
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
        write_minimal_workspace(&thread_root);
        let thread_args = vec![
            "--crate".to_string(),
            "radroots_core".to_string(),
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
            write_minimal_workspace(&step_root);
            let step_args = vec![
                "--crate".to_string(),
                "radroots_core".to_string(),
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
            "--policy-gate".to_string(),
        ];
        report_gate(&args).expect("report gate success");
        let report_raw = fs::read_to_string(&out_path).expect("read report");
        assert!(report_raw.contains("\"scope\": \"crate-x\""));
        assert!(report_raw.contains("\"regions\": 100.0"));
        assert!(report_raw.contains("\"pass\": true"));
        fs::remove_dir_all(root).expect("remove report gate success root");
    }

    #[test]
    fn report_gate_normalizes_duplicate_generic_records_after_perfect_lcov() {
        let root = temp_dir_path("report_gate_normalized_generics");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{
  "data": [
    {
      "totals": {
        "functions": {"percent": 96.0},
        "lines": {"percent": 99.0},
        "regions": {"percent": 22.0}
      }
    }
  ]
}"#,
        );
        write_file(
            &root.join("coverage-details.json"),
            r#"{
  "data": [
    {
      "functions": [
        {
          "count": 4,
          "filenames": ["/tmp/crates/runtime_manager/src/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 4, 0, 0, 0],
            [13, 1, 13, 8, 4, 0, 0, 0]
          ]
        },
        {
          "count": 0,
          "filenames": ["/tmp/crates/runtime_manager/src/lib.rs"],
          "regions": [
            [10, 1, 12, 2, 0, 0, 0, 0],
            [13, 1, 13, 8, 0, 0, 0, 0]
          ]
        },
        {
          "count": 0,
          "filenames": ["/tmp/crates/runtime_manager/src/lib.rs"],
          "regions": [
            [20, 1, 20, 6, 0, 0, 0, 0]
          ]
        }
      ]
    }
  ]
}"#,
        );
        write_file(&lcov_path, "DA:1,1\nLF:1\nLH:1\nBRDA:1,0,0,1\n");

        let args = vec![
            "--scope".to_string(),
            "radroots_runtime_manager".to_string(),
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
        report_gate(&args).expect("normalized report gate success");

        let report_raw = fs::read_to_string(&out_path).expect("read normalized report");
        assert!(report_raw.contains("\"functions_percent\": 100.0"));
        assert!(report_raw.contains("\"summary_regions_percent\": 100.0"));
        assert!(report_raw.contains("\"pass\": true"));

        fs::remove_dir_all(root).expect("remove normalized report gate root");
    }

    #[test]
    fn report_gate_with_root_uses_scope_specific_override_thresholds() {
        let root = temp_dir_path("report_gate_override_success");
        let coverage_dir = root.join("policy").join("coverage");
        fs::create_dir_all(&coverage_dir).expect("create coverage dir");
        write_file(
            &coverage_dir.join("policy.toml"),
            "[gate]\nfail_under_exec_lines = 100.0\nfail_under_functions = 100.0\nfail_under_regions = 100.0\nfail_under_branches = 100.0\nrequire_branches = true\n\n[required]\ncrates = [\"radroots_a\"]\n\n[overrides.radroots_a]\nfail_under_exec_lines = 88.5\nfail_under_functions = 77.5\nfail_under_regions = 66.5\nfail_under_branches = 55.5\nrequire_branches = false\ntemporary = true\nreason = \"temporary publish unblocker\"\n",
        );

        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":80.0},"lines":{"percent":88.5},"regions":{"percent":70.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nLF:1\nLH:1\nBRDA:1,0,0,1\n");

        report_gate_with_root(
            &[
                "--scope".to_string(),
                "radroots_a".to_string(),
                "--summary".to_string(),
                summary_path.display().to_string(),
                "--lcov".to_string(),
                lcov_path.display().to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
                "--policy-gate".to_string(),
            ],
            &root,
        )
        .expect("report gate should honor override");

        let report_raw = fs::read_to_string(&out_path).expect("read override report");
        assert!(report_raw.contains("\"functions\": 77.5"));
        assert!(report_raw.contains("\"regions\": 66.5"));
        assert!(report_raw.contains("\"branches_required\": false"));
        assert!(report_raw.contains("\"pass\": true"));

        fs::remove_dir_all(root).expect("remove report gate override root");
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
        assert!(err.contains("invalid --fail-under-functions value"));
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
            "--policy-gate".to_string(),
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
            "--fail-under-exec-lines".to_string(),
            "100.0".to_string(),
            "--fail-under-functions".to_string(),
            "100.0".to_string(),
            "--fail-under-regions".to_string(),
            "100.0".to_string(),
            "--fail-under-branches".to_string(),
            "100.0".to_string(),
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

        let missing_thresholds = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect_err("missing thresholds");
        assert!(missing_thresholds.contains("missing coverage thresholds"));

        let missing_functions = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-exec-lines".to_string(),
            "100".to_string(),
        ])
        .expect_err("missing functions threshold");
        assert!(missing_functions.contains("missing coverage thresholds"));

        let missing_regions = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-exec-lines".to_string(),
            "100".to_string(),
            "--fail-under-functions".to_string(),
            "100".to_string(),
        ])
        .expect_err("missing regions threshold");
        assert!(missing_regions.contains("missing coverage thresholds"));

        let missing_branches = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--fail-under-exec-lines".to_string(),
            "100".to_string(),
            "--fail-under-functions".to_string(),
            "100".to_string(),
            "--fail-under-regions".to_string(),
            "100".to_string(),
        ])
        .expect_err("missing branches threshold");
        assert!(missing_branches.contains("missing coverage thresholds"));

        let missing_summary_file = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            root.join("missing-summary.json").display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--policy-gate".to_string(),
        ])
        .expect_err("missing summary file should fail");
        assert!(missing_summary_file.contains("failed to read summary"));

        let missing_gate_report = read_gate_report(&root.join("missing-gate-report.json"))
            .expect_err("missing gate report should fail");
        assert!(missing_gate_report.contains("failed to read gate report"));

        write_file(&out_path, "{not-json");
        let invalid_gate_report = read_gate_report(&out_path).expect_err("invalid gate report");
        assert!(invalid_gate_report.contains("failed to parse gate report"));

        let missing_lcov_file = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            root.join("missing-lcov.info").display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--policy-gate".to_string(),
        ])
        .expect_err("missing lcov file should fail");
        assert!(missing_lcov_file.contains("failed to read lcov"));

        let mixed_policy_gate = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--policy-gate".to_string(),
            "--fail-under-functions".to_string(),
            "100.0".to_string(),
        ])
        .expect_err("policy gate mixed with explicit thresholds");
        assert!(mixed_policy_gate.contains("cannot be combined"));

        let mixed_policy_gate_regions = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--policy-gate".to_string(),
            "--fail-under-regions".to_string(),
            "100.0".to_string(),
        ])
        .expect_err("policy gate mixed with regions threshold");
        assert!(mixed_policy_gate_regions.contains("cannot be combined"));

        let mixed_policy_gate_branches_flag = report_gate(&[
            "--scope".to_string(),
            "crate".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
            "--policy-gate".to_string(),
            "--require-branches".to_string(),
        ])
        .expect_err("policy gate mixed with require-branches");
        assert!(mixed_policy_gate_branches_flag.contains("cannot be combined"));

        fs::remove_dir_all(root).expect("remove report arg errors root");
    }

    #[test]
    fn coverage_ignore_filename_regex_reports_unknown_crate() {
        let root = temp_dir_path("coverage_unknown_crate_root");
        write_file(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/core\"]\n",
        );
        write_file(
            &root.join("crates").join("core").join("Cargo.toml"),
            "[package]\nname = \"radroots_core\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );

        let err = coverage_ignore_filename_regex(&root, "radroots_missing")
            .expect_err("unknown crate should fail");
        assert!(err.contains("could not resolve crate directory"));

        fs::remove_dir_all(root).expect("remove unknown crate root");
    }

    #[test]
    fn coverage_ignore_filename_regex_reports_workspace_manifest_errors() {
        let root = temp_dir_path("coverage_regex_workspace_error_root");
        let read_err = coverage_ignore_filename_regex(&root, "radroots_core")
            .expect_err("missing workspace manifest should fail");
        assert!(read_err.contains("failed to read"));

        write_file(&root.join("Cargo.toml"), "[workspace");
        let parse_err = coverage_ignore_filename_regex(&root, "radroots_core")
            .expect_err("invalid workspace manifest should fail");
        assert!(parse_err.contains("failed to parse"));

        fs::remove_dir_all(root).expect("remove workspace error root");
    }

    #[test]
    fn run_crate_with_runner_at_root_reports_ignore_filter_errors() {
        let root = temp_dir_path("run_crate_ignore_filter_error");
        write_file(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/other\"]\n",
        );
        write_file(
            &root.join("crates").join("other").join("Cargo.toml"),
            "[package]\nname = \"radroots_other\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let args = vec![
            "--crate".to_string(),
            "radroots_core".to_string(),
            "--out".to_string(),
            root.join("target").join("coverage").display().to_string(),
        ];
        let mut runner = ok_runner;
        let err = run_crate_with_runner_at_root(&args, &root, &mut runner)
            .expect_err("missing crate coverage filter should fail");
        assert!(err.contains("could not resolve crate directory"));

        fs::remove_dir_all(root).expect("remove run crate ignore filter root");
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
        assert!(required_err.contains("failed to read coverage policy"));

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
            vec!["radroots_a".to_string(), "radroots_b".to_string()],
            "required crates",
        )
        .expect("write crate names");
        let rendered = String::from_utf8(output).expect("utf8");
        assert!(rendered.contains("radroots_a"));
        assert!(rendered.contains("radroots_b"));

        let mut failing = FailingWriter;
        let err = write_crate_names_output(
            &mut failing,
            vec!["radroots_a".to_string()],
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
            "--policy-gate".to_string(),
        ])
        .expect("dispatch report");
        assert!(out_path.exists());
        fs::remove_dir_all(root).expect("remove report dispatch root");
    }

    #[test]
    fn report_gate_with_root_reports_policy_read_errors() {
        let root = temp_dir_path("report_gate_policy_root_error");
        let summary_path = root.join("summary.json");
        let lcov_path = root.join("coverage.info");
        let out_path = root.join("gate-report.json");
        write_file(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        );
        write_file(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n");

        let err = report_gate_with_root(
            &[
                "--scope".to_string(),
                "crate-x".to_string(),
                "--summary".to_string(),
                summary_path.display().to_string(),
                "--lcov".to_string(),
                lcov_path.display().to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
                "--policy-gate".to_string(),
            ],
            &root,
        )
        .expect_err("missing policy should fail");
        assert!(err.contains("failed to read coverage policy"));

        fs::remove_dir_all(root).expect("remove report gate policy error root");
    }
}
