use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

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
    require_executable_lines: bool,
    require_branches: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageToolchain {
    pub(crate) rust: String,
    pub(crate) coverage_rust: String,
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
    #[serde(default = "default_true")]
    branches_applicable: bool,
    branch_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageExclusion {
    paths: Vec<String>,
    reason: String,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct LlvmCovReport {
    data: Vec<LlvmCovData>,
}

#[derive(Debug, Deserialize)]
struct LlvmCovData {
    files: Vec<LlvmCovFile>,
    #[serde(default)]
    functions: Vec<LlvmCovFunction>,
    totals: LlvmCovSummary,
}

#[derive(Debug, Deserialize)]
struct LlvmCovFunction {
    count: u64,
    filenames: Vec<String>,
    regions: Vec<Vec<u64>>,
    #[serde(default)]
    branches: Vec<Vec<u64>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FunctionKey {
    filenames: Vec<String>,
    definition: RegionKey,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RegionKey {
    line_start: u64,
    column_start: u64,
    line_end: u64,
    column_end: u64,
    kind: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BranchKey {
    line_start: u64,
    column_start: u64,
    line_end: u64,
    column_end: u64,
    kind: u64,
}

#[derive(Debug)]
struct CoverageSource {
    raw: String,
    cfg_test_lines: Vec<bool>,
    coverage_off_lines: Vec<bool>,
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
    branches: LlvmCovMetric,
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
    branches: MetricAccumulator,
    matched_files: usize,
}

impl SummaryAccumulator {
    fn add(&mut self, summary: &LlvmCovSummary) {
        self.lines.add(&summary.lines);
        self.functions.add(&summary.functions);
        self.regions.add(&summary.regions);
        self.branches.add(&summary.branches);
        self.matched_files += 1;
    }

    fn summary(&self) -> LlvmCovSummary {
        LlvmCovSummary {
            lines: self.lines.metric(),
            functions: self.functions.metric(),
            regions: self.regions.metric(),
            branches: self.branches.metric(),
        }
    }
}

pub(crate) fn validate_contract(contract: &CoverageContract) -> Result<(), String> {
    validate_non_empty(&contract.toolchain.rust, "toolchain.rust")?;
    validate_non_empty(&contract.toolchain.coverage_rust, "toolchain.coverage_rust")?;
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
        if scope.branches_applicable {
            if scope.branch_reason.is_some() {
                return Err(format!(
                    "scopes.{name}.branch_reason requires branches_applicable = false"
                ));
            }
        } else {
            validate_non_empty(
                scope.branch_reason.as_deref().unwrap_or_default(),
                &format!("scopes.{name}.branch_reason"),
            )?;
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
        "total executable lines",
        &data.totals.lines,
        contract.policy.require_executable_lines,
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
    validate_metric(
        "total branches",
        &data.totals.branches,
        contract.policy.require_branches,
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
            "executable lines",
            &scope_summary.lines,
            scope.threshold,
            contract.policy.require_executable_lines,
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
        if scope.branches_applicable {
            collect_scope_metric_failure(
                &mut failures,
                scope_name,
                "branches",
                &scope_summary.branches,
                scope.threshold,
                contract.policy.require_branches,
            );
        } else if scope_summary.branches.count != 0 {
            failures.push(format!(
                "coverage scope {scope_name}: declared branches inapplicable but measured {} branch records",
                scope_summary.branches.count
            ));
        }
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
    let mut summary = accumulator.summary();
    if !data.functions.is_empty() {
        let (lines, functions, regions, branches) = normalized_source_metrics(root, data, scope)?;
        summary.lines = lines;
        summary.functions = functions;
        summary.regions = regions;
        if branches.count > 0 {
            summary.branches = branches;
        }
    }
    Ok(summary)
}

fn normalized_source_metrics(
    root: &Path,
    data: &LlvmCovData,
    scope: &CoverageScope,
) -> Result<(LlvmCovMetric, LlvmCovMetric, LlvmCovMetric, LlvmCovMetric), String> {
    let mut groups = BTreeMap::<FunctionKey, Vec<&LlvmCovFunction>>::new();
    for function in &data.functions {
        if function.filenames.is_empty() || function.regions.is_empty() {
            continue;
        }
        let Some(definition) = function
            .regions
            .first()
            .and_then(|region| region_key(region))
        else {
            continue;
        };
        groups
            .entry(FunctionKey {
                filenames: function.filenames.clone(),
                definition,
            })
            .or_default()
            .push(function);
    }
    let mut function_count = 0_u64;
    let mut function_covered = 0_u64;
    let mut all_regions = BTreeMap::<(String, RegionKey), bool>::new();
    let mut lines = BTreeMap::<(String, u64), bool>::new();
    let mut branches = BTreeMap::<(String, BranchKey), (bool, bool)>::new();
    let mut source_cache = BTreeMap::<PathBuf, Option<CoverageSource>>::new();
    for variants in groups.values() {
        let primary_definition = variants.iter().find_map(|function| {
            let region = function.regions.first()?;
            let filename = region_filename(function, region)?;
            let relative = report_filename(root, filename);
            if scope.paths.iter().any(|path| path_matches(path, &relative)) {
                Some((filename, region[0]))
            } else {
                None
            }
        });
        let Some((primary, definition_line)) = primary_definition else {
            continue;
        };
        if variants.iter().all(|function| {
            function.regions.first().is_some_and(|region| {
                region
                    .first()
                    .is_some_and(|line| is_ignorable_source_line(primary, *line, &mut source_cache))
            })
        }) {
            continue;
        }
        if is_authored_function_line(primary, definition_line, &mut source_cache) {
            function_count += 1;
            if variants.iter().any(|function| function.count > 0) {
                function_covered += 1;
            }
        }
        let mut regions = BTreeMap::<(String, RegionKey), bool>::new();
        for function in variants {
            for region in &function.regions {
                let filename = region_filename(function, region)
                    .expect("grouped coverage functions always retain a source filename");
                let relative = report_filename(root, filename);
                if !scope.paths.iter().any(|path| path_matches(path, &relative)) {
                    continue;
                }
                let Some(key) = region_key(region) else {
                    continue;
                };
                let covered = region.get(4).is_some_and(|count| *count > 0);
                regions
                    .entry((filename.to_owned(), key))
                    .and_modify(|existing| *existing |= covered)
                    .or_insert(covered);
            }
        }
        for ((filename, region), covered) in regions {
            if is_ignorable_source_line(&filename, region.line_start, &mut source_cache) {
                continue;
            }
            let filename = normalized_path_string(Path::new(&filename));
            all_regions
                .entry((filename.clone(), region.clone()))
                .and_modify(|existing| *existing |= covered)
                .or_insert(covered);
            if region.kind == 0 {
                for line in region.line_start..=region.line_end {
                    if !is_ignorable_source_line(&filename, line, &mut source_cache) {
                        lines
                            .entry((filename.clone(), line))
                            .and_modify(|existing| *existing |= covered)
                            .or_insert(covered);
                    }
                }
            }
        }
        for function in variants {
            for branch in &function.branches {
                let filename = branch_filename(function, branch)
                    .expect("grouped coverage functions always retain a source filename");
                let relative = report_filename(root, filename);
                if !scope.paths.iter().any(|path| path_matches(path, &relative)) {
                    continue;
                }
                let Some(key) = branch_key(branch) else {
                    continue;
                };
                if is_ignorable_branch(filename, &key, &mut source_cache) {
                    continue;
                }
                let true_covered = branch.get(4).is_some_and(|count| *count > 0);
                let false_covered = branch.get(5).is_some_and(|count| *count > 0);
                let filename = normalized_path_string(Path::new(filename));
                branches
                    .entry((filename, key))
                    .and_modify(|covered| {
                        covered.0 |= true_covered;
                        covered.1 |= false_covered;
                    })
                    .or_insert((true_covered, false_covered));
            }
        }
    }
    let line_count = lines.len() as u64;
    let line_covered = lines.values().filter(|covered| **covered).count() as u64;
    let branch_count = (branches.len() * 2) as u64;
    let branch_covered = branches
        .values()
        .map(|covered| u64::from(covered.0) + u64::from(covered.1))
        .sum::<u64>();
    let region_count = all_regions.len() as u64;
    let region_covered = all_regions.values().filter(|covered| **covered).count() as u64;
    Ok((
        LlvmCovMetric {
            count: line_count,
            covered: line_covered,
            percent: metric_percent(line_count, line_covered),
        },
        LlvmCovMetric {
            count: function_count,
            covered: function_covered,
            percent: metric_percent(function_count, function_covered),
        },
        LlvmCovMetric {
            count: region_count,
            covered: region_covered,
            percent: metric_percent(region_count, region_covered),
        },
        LlvmCovMetric {
            count: branch_count,
            covered: branch_covered,
            percent: metric_percent(branch_count, branch_covered),
        },
    ))
}

fn region_filename<'a>(function: &'a LlvmCovFunction, region: &[u64]) -> Option<&'a str> {
    region
        .get(5)
        .and_then(|index| function.filenames.get(*index as usize))
        .or_else(|| function.filenames.first())
        .map(String::as_str)
}

fn branch_filename<'a>(function: &'a LlvmCovFunction, branch: &[u64]) -> Option<&'a str> {
    branch
        .get(6)
        .and_then(|index| function.filenames.get(*index as usize))
        .or_else(|| function.filenames.first())
        .map(String::as_str)
}

fn is_authored_function_line(
    filename: &str,
    line: u64,
    cache: &mut BTreeMap<PathBuf, Option<CoverageSource>>,
) -> bool {
    let path = PathBuf::from(filename);
    if !cache.contains_key(&path) {
        let _ = is_ignorable_source_line(filename, line, cache);
    }
    cache
        .get(&path)
        .and_then(Option::as_ref)
        .and_then(|source| source.raw.lines().nth(line.saturating_sub(1) as usize))
        .is_some_and(|source_line| source_line.contains("fn "))
}

fn region_key(region: &[u64]) -> Option<RegionKey> {
    Some(RegionKey {
        line_start: *region.first()?,
        column_start: *region.get(1)?,
        line_end: *region.get(2)?,
        column_end: *region.get(3)?,
        kind: *region.get(7)?,
    })
}

fn branch_key(branch: &[u64]) -> Option<BranchKey> {
    Some(BranchKey {
        line_start: *branch.first()?,
        column_start: *branch.get(1)?,
        line_end: *branch.get(2)?,
        column_end: *branch.get(3)?,
        kind: *branch.get(8)?,
    })
}

fn is_ignorable_source_line(
    filename: &str,
    line: u64,
    cache: &mut BTreeMap<PathBuf, Option<CoverageSource>>,
) -> bool {
    let path = PathBuf::from(filename);
    let source = cache.entry(path.clone()).or_insert_with(|| {
        fs::read_to_string(path).ok().map(|raw| CoverageSource {
            raw: raw.clone(),
            cfg_test_lines: annotated_source_lines(&raw, "cfg(test)"),
            coverage_off_lines: annotated_source_lines(
                &raw,
                "cfg_attr(coverage_nightly, coverage(off))",
            ),
        })
    });
    let Some(source) = source else {
        return false;
    };
    line.checked_sub(1)
        .map(|index| {
            let index = index as usize;
            source.cfg_test_lines.get(index).copied().unwrap_or(false)
                || source
                    .coverage_off_lines
                    .get(index)
                    .copied()
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn is_ignorable_branch(
    filename: &str,
    branch: &BranchKey,
    cache: &mut BTreeMap<PathBuf, Option<CoverageSource>>,
) -> bool {
    if is_ignorable_source_line(filename, branch.line_start, cache) {
        return true;
    }
    if branch.line_start != branch.line_end {
        return false;
    }
    let path = PathBuf::from(filename);
    let Some(Some(source)) = cache.get(&path) else {
        return false;
    };
    let Some(line) = source
        .raw
        .lines()
        .nth(branch.line_start.saturating_sub(1) as usize)
    else {
        return false;
    };
    let start = branch.column_start.saturating_sub(1) as usize;
    let end = branch.column_end.saturating_sub(1) as usize;
    let slice = line.get(start..end);
    (branch.column_end == branch.column_start + 1 && slice == Some("?"))
        || line.contains("unreachable!()")
        || (line.contains("assert!(matches!(") && slice == Some("matches!"))
}

fn annotated_source_lines(source: &str, marker: &str) -> Vec<bool> {
    let mut pending = false;
    let mut depth = None;
    let mut lines = Vec::with_capacity(source.lines().count());
    for line in source.lines() {
        let trimmed = line.trim();
        let mut annotated = depth.is_some();
        let marker_matches = if marker == "cfg(test)" {
            trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test,")
        } else {
            trimmed.contains(marker)
        };
        if depth.is_none() && marker_matches {
            pending = true;
            annotated = true;
        } else if depth.is_none() && pending {
            annotated = true;
            let content =
                !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#[");
            if content {
                let delta = brace_delta(trimmed);
                if delta > 0 {
                    depth = Some(0_i64);
                    pending = false;
                } else if trimmed.contains('{') || trimmed.ends_with(';') {
                    pending = false;
                }
            }
        }
        lines.push(annotated);
        if let Some(current) = depth.as_mut() {
            *current += brace_delta(trimmed);
            if *current <= 0 {
                depth = None;
            }
        }
    }
    lines
}

fn brace_delta(line: &str) -> i64 {
    line.bytes().filter(|byte| *byte == b'{').count() as i64
        - line.bytes().filter(|byte| *byte == b'}').count() as i64
}

fn report_filename(root: &Path, filename: &str) -> String {
    let path = normalize_path(Path::new(filename));
    let root = normalize_path(root);
    let relative = path.strip_prefix(&root).unwrap_or(&path);
    normalized_path_string(relative)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn normalized_path_string(path: &Path) -> String {
    normalize_path(path).to_string_lossy().replace('\\', "/")
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
            "coverage {name} {:.3}% ({}/{}) is below required {:.1}%",
            metric.percent, metric.covered, metric.count, threshold
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
