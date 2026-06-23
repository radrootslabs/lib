use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageContract {
    policy: CoveragePolicy,
    pub(crate) toolchain: CoverageToolchain,
    pub(crate) report: CoverageReport,
    generated: GeneratedCoveragePolicy,
    scopes: BTreeMap<String, CoverageScope>,
    exclusions: BTreeMap<String, CoverageExclusion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoveragePolicy {
    enforce: bool,
    require_regions: bool,
    require_functions: bool,
    require_lines: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageToolchain {
    pub(crate) rust: String,
    pub(crate) wasm_target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageReport {
    pub(crate) output: String,
    pub(crate) ignore_filename_regex: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedCoveragePolicy {
    typescript: String,
    binding_crates: String,
    wasm_glue: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageScope {
    paths: Vec<String>,
    threshold: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageExclusion {
    paths: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct LlvmCovReport {
    data: Vec<LlvmCovData>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovData {
    files: Vec<LlvmCovFile>,
    totals: LlvmCovSummary,
}

#[derive(Debug, Deserialize)]
struct LlvmCovFile {
    filename: String,
    summary: LlvmCovSummary,
}

#[derive(Debug, Deserialize)]
struct LlvmCovSummary {
    lines: LlvmCovMetric,
    functions: LlvmCovMetric,
    regions: LlvmCovMetric,
}

#[derive(Debug, Deserialize)]
struct LlvmCovMetric {
    count: u64,
    covered: u64,
    percent: f64,
}

#[derive(Debug, Default)]
struct MetricAccumulator {
    count: u64,
    covered: u64,
}

impl MetricAccumulator {
    fn add(&mut self, metric: &LlvmCovMetric) {
        self.count += metric.count;
        self.covered += metric.covered;
    }

    fn metric(&self) -> LlvmCovMetric {
        LlvmCovMetric {
            count: self.count,
            covered: self.covered,
            percent: metric_percent(self.count, self.covered),
        }
    }
}

#[derive(Debug, Default)]
struct SummaryAccumulator {
    lines: MetricAccumulator,
    functions: MetricAccumulator,
    regions: MetricAccumulator,
    matched_files: usize,
}

impl SummaryAccumulator {
    fn add(&mut self, summary: &LlvmCovSummary) {
        self.lines.add(&summary.lines);
        self.functions.add(&summary.functions);
        self.regions.add(&summary.regions);
        self.matched_files += 1;
    }

    fn summary(&self) -> LlvmCovSummary {
        LlvmCovSummary {
            lines: self.lines.metric(),
            functions: self.functions.metric(),
            regions: self.regions.metric(),
        }
    }
}

pub(crate) fn validate_contract(contract: &CoverageContract) -> Result<(), String> {
    validate_non_empty(&contract.toolchain.rust, "toolchain.rust")?;
    validate_non_empty(&contract.toolchain.wasm_target, "toolchain.wasm_target")?;
    validate_non_empty(&contract.report.output, "report.output")?;
    validate_non_empty(
        &contract.report.ignore_filename_regex,
        "report.ignore_filename_regex",
    )?;
    validate_non_empty(&contract.generated.typescript, "generated.typescript")?;
    validate_non_empty(
        &contract.generated.binding_crates,
        "generated.binding_crates",
    )?;
    validate_non_empty(&contract.generated.wasm_glue, "generated.wasm_glue")?;
    if contract.scopes.is_empty() {
        return Err("contracts/coverage.toml scopes must not be empty".to_owned());
    }
    for (name, scope) in &contract.scopes {
        validate_non_empty(name, "scope name")?;
        validate_threshold(scope.threshold, &format!("scopes.{name}.threshold"))?;
        if scope.paths.is_empty() {
            return Err(format!("scopes.{name}.paths must not be empty"));
        }
        for path in &scope.paths {
            validate_non_empty(path, &format!("scopes.{name}.paths entry"))?;
        }
    }
    if contract.exclusions.is_empty() {
        return Err("contracts/coverage.toml exclusions must not be empty".to_owned());
    }
    for (name, exclusion) in &contract.exclusions {
        validate_non_empty(name, "exclusion name")?;
        validate_non_empty(&exclusion.reason, &format!("exclusions.{name}.reason"))?;
        if exclusion.paths.is_empty() {
            return Err(format!("exclusions.{name}.paths must not be empty"));
        }
        for path in &exclusion.paths {
            validate_non_empty(path, &format!("exclusions.{name}.paths entry"))?;
        }
    }
    Ok(())
}

fn validate_threshold(threshold: f64, field: &str) -> Result<(), String> {
    if (0.0..=100.0).contains(&threshold) {
        Ok(())
    } else {
        Err(format!(
            "contracts/coverage.toml {field} must be between 0 and 100"
        ))
    }
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("contracts/coverage.toml {field} must not be empty"))
    } else {
        Ok(())
    }
}

pub(crate) fn evaluate_report(
    root: &Path,
    report_path: &Path,
    contract: &CoverageContract,
) -> Result<(), String> {
    let raw = fs::read_to_string(report_path)
        .map_err(|error| format!("failed to read {}: {error}", report_path.display()))?;
    let report = serde_json::from_str::<LlvmCovReport>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", report_path.display()))?;
    let data = report
        .data
        .first()
        .ok_or_else(|| format!("{} did not include coverage data", report_path.display()))?;
    validate_metric(
        "total lines",
        &data.totals.lines,
        contract.policy.require_lines,
    )?;
    validate_metric(
        "total functions",
        &data.totals.functions,
        contract.policy.require_functions,
    )?;
    validate_metric(
        "total regions",
        &data.totals.regions,
        contract.policy.require_regions,
    )?;
    let mut failures = Vec::new();
    for (scope_name, scope) in &contract.scopes {
        let scope_summary = match scope_summary(root, data, scope) {
            Ok(summary) => summary,
            Err(error) => {
                failures.push(format!("coverage scope {scope_name}: {error}"));
                continue;
            }
        };
        collect_scope_metric_failure(
            &mut failures,
            scope_name,
            "lines",
            &scope_summary.lines,
            scope.threshold,
            contract.policy.require_lines,
        );
        collect_scope_metric_failure(
            &mut failures,
            scope_name,
            "functions",
            &scope_summary.functions,
            scope.threshold,
            contract.policy.require_functions,
        );
        collect_scope_metric_failure(
            &mut failures,
            scope_name,
            "regions",
            &scope_summary.regions,
            scope.threshold,
            contract.policy.require_regions,
        );
    }
    if !contract.policy.enforce {
        println!(
            "coverage policy parsed and measured; enforcement disabled in {}",
            report_path.display()
        );
        return Ok(());
    }
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    println!("coverage policy passed using {}", report_path.display());
    Ok(())
}

fn scope_summary(
    root: &Path,
    data: &LlvmCovData,
    scope: &CoverageScope,
) -> Result<LlvmCovSummary, String> {
    let mut accumulator = SummaryAccumulator::default();
    for file in &data.files {
        let filename = report_filename(root, &file.filename);
        if scope
            .paths
            .iter()
            .any(|pattern| path_matches(pattern, &filename))
        {
            accumulator.add(&file.summary);
        }
    }
    if accumulator.matched_files == 0 {
        return Err(format!(
            "matched no report files for {}",
            scope.paths.join(", ")
        ));
    }
    Ok(accumulator.summary())
}

fn report_filename(root: &Path, filename: &str) -> String {
    let path = Path::new(filename);
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else {
        path == pattern
    }
}

fn collect_scope_metric_failure(
    failures: &mut Vec<String>,
    scope_name: &str,
    metric_name: &str,
    metric: &LlvmCovMetric,
    threshold: f64,
    required: bool,
) {
    if let Err(error) = validate_metric(metric_name, metric, required) {
        failures.push(format!("coverage scope {scope_name}: {error}"));
    }
    if let Err(error) = enforce_metric(metric_name, metric, threshold) {
        failures.push(format!("coverage scope {scope_name}: {error}"));
    }
}

fn validate_metric(name: &str, metric: &LlvmCovMetric, required: bool) -> Result<(), String> {
    if required && metric.count == 0 {
        return Err(format!(
            "coverage report did not include required {name} records"
        ));
    }
    if metric.covered > metric.count {
        return Err(format!("coverage report has invalid {name} counts"));
    }
    Ok(())
}

fn enforce_metric(name: &str, metric: &LlvmCovMetric, threshold: f64) -> Result<(), String> {
    if metric.percent < threshold {
        return Err(format!(
            "coverage {name} {:.3}% is below required {:.1}%",
            metric.percent, threshold
        ));
    }
    Ok(())
}

fn metric_percent(count: u64, covered: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        covered as f64 * 100.0 / count as f64
    }
}

#[cfg(test)]
#[path = "coverage_policy_tests.rs"]
mod tests;
