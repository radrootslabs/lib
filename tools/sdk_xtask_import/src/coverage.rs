use std::{collections::BTreeMap, fs, path::Path, process::Command};

use serde::Deserialize;

use crate::{check, fs::workspace_root, generate, wasm};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageContract {
    policy: CoveragePolicy,
    toolchain: CoverageToolchain,
    report: CoverageReport,
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
struct CoverageToolchain {
    rust: String,
    wasm_target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageReport {
    output: String,
    ignore_filename_regex: String,
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

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "run" => run_coverage(),
        [] => Err(usage()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: cargo xtask coverage run".to_owned()
}

fn run_coverage() -> Result<(), String> {
    let root = workspace_root()?;
    let contract = load_contract(&root)?;
    validate_contract(&contract)?;
    preflight(&contract)?;
    generate::generate_ts()?;
    wasm::generate(&[])?;
    check::check()?;
    clean_report_output(&root, &contract)?;
    run_llvm_cov(&root, &contract)?;
    evaluate_report(&root, &root.join(&contract.report.output), &contract)
}

fn load_contract(root: &Path) -> Result<CoverageContract, String> {
    let path = root.join("contracts").join("coverage.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate_contract(contract: &CoverageContract) -> Result<(), String> {
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

fn preflight(contract: &CoverageContract) -> Result<(), String> {
    require_command("rustup", &["--version"], "install rustup")?;
    require_command(
        "rustup",
        &[
            "run",
            &contract.toolchain.rust,
            "cargo",
            "llvm-cov",
            "--version",
        ],
        "install cargo-llvm-cov for the SDK Rust toolchain",
    )?;
    let component_output = output(
        "rustup",
        &["component", "list", "--toolchain", &contract.toolchain.rust],
    )
    .map_err(|error| format!("failed to inspect Rust components: {error}"))?;
    if !component_output
        .lines()
        .any(|line| line.starts_with("llvm-tools") && line.contains("(installed)"))
    {
        return Err(format!(
            "missing llvm-tools-preview for Rust toolchain {}; run `rustup component add llvm-tools-preview --toolchain {}`",
            contract.toolchain.rust, contract.toolchain.rust
        ));
    }
    let target_output = output(
        "rustup",
        &["target", "list", "--toolchain", &contract.toolchain.rust],
    )
    .map_err(|error| format!("failed to inspect Rust targets: {error}"))?;
    let expected_target = format!("{} (installed)", contract.toolchain.wasm_target);
    if !target_output.lines().any(|line| line == expected_target) {
        return Err(format!(
            "missing Rust target {}; run `rustup target add {} --toolchain {}`",
            contract.toolchain.wasm_target, contract.toolchain.wasm_target, contract.toolchain.rust
        ));
    }
    require_command("wasm-pack", &["--version"], "install wasm-pack")?;
    require_command("pnpm", &["--version"], "install pnpm")?;
    Ok(())
}

fn require_command(command: &str, args: &[&str], install_hint: &str) -> Result<(), String> {
    output(command, args)
        .map(|_| ())
        .map_err(|error| format!("missing {command}: {error}; {install_hint}"))
}

fn output(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn clean_report_output(root: &Path, contract: &CoverageContract) -> Result<(), String> {
    let output_path = root.join(&contract.report.output);
    if let Some(parent) = output_path.parent()
        && parent.exists()
    {
        fs::remove_dir_all(parent)
            .map_err(|error| format!("failed to remove {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn run_llvm_cov(root: &Path, contract: &CoverageContract) -> Result<(), String> {
    let output_path = root.join(&contract.report.output);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let status = Command::new("rustup")
        .current_dir(root)
        .args([
            "run",
            &contract.toolchain.rust,
            "cargo",
            "llvm-cov",
            "--workspace",
            "--all-features",
            "--summary-only",
            "--json",
            "--output-path",
            &contract.report.output,
            "--ignore-filename-regex",
            &contract.report.ignore_filename_regex,
            "--no-fail-fast",
        ])
        .status()
        .map_err(|error| format!("failed to run cargo llvm-cov: {error}"))?;
    if !status.success() {
        return Err(format!("cargo llvm-cov failed with status {status}"));
    }
    Ok(())
}

fn evaluate_report(
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
mod tests {
    use super::{
        CoverageContract, enforce_metric, evaluate_report, path_matches, validate_contract,
        validate_metric,
    };

    const CONTRACT: &str = r#"
[policy]
enforce = true
require_regions = true
require_functions = true
require_lines = true

[toolchain]
rust = "1.92.0"
wasm_target = "wasm32-unknown-unknown"

[report]
output = "target/sdk-coverage/summary.json"
ignore_filename_regex = "generated"

[generated]
typescript = "excluded because generated TypeScript is owned by Rust source generators"
binding_crates = "excluded because binding crates are generated source facades"
wasm_glue = "excluded because wasm-bindgen glue is verified through generated package checks"

[scopes.radroots_sdk]
paths = ["crates/sdk/src/**"]
threshold = 98.0

[scopes.xtask]
paths = ["tools/xtask/src/**"]
threshold = 100.0

[exclusions.generated]
paths = ["packages/*/src/generated/**"]
reason = "generated package output is checked through reproducibility"
"#;

    #[test]
    fn validates_contract_shape() {
        let contract = toml::from_str::<CoverageContract>(CONTRACT).expect("contract parses");
        validate_contract(&contract).expect("contract validates");
    }

    #[test]
    fn rejects_invalid_scope_thresholds() {
        let raw = CONTRACT.replace("threshold = 98.0", "threshold = 101.0");
        let contract = toml::from_str::<CoverageContract>(&raw).expect("contract parses");
        assert!(validate_contract(&contract).is_err());
    }

    #[test]
    fn matches_recursive_scope_paths() {
        assert!(path_matches(
            "crates/sdk/src/**",
            "crates/sdk/src/adapters/radrootsd.rs"
        ));
        assert!(path_matches("crates/sdk/src/**", "crates/sdk/src"));
        assert!(!path_matches(
            "crates/sdk/src/**",
            "crates/sql_wasm_runtime/src/lib.rs"
        ));
    }

    #[test]
    fn accepts_required_metric_counts() {
        let metric = super::LlvmCovMetric {
            count: 10,
            covered: 10,
            percent: 100.0,
        };
        validate_metric("lines", &metric, true).expect("metric validates");
        enforce_metric("lines", &metric, 100.0).expect("metric passes");
    }

    #[test]
    fn rejects_missing_required_metric_counts() {
        let metric = super::LlvmCovMetric {
            count: 0,
            covered: 0,
            percent: 0.0,
        };
        assert!(validate_metric("lines", &metric, true).is_err());
    }

    #[test]
    fn rejects_under_threshold_metric() {
        let metric = super::LlvmCovMetric {
            count: 10,
            covered: 9,
            percent: 90.0,
        };
        assert!(enforce_metric("lines", &metric, 100.0).is_err());
    }

    #[test]
    fn enforcement_rejects_undercovered_scope() {
        let dir = std::env::temp_dir().join(format!(
            "radroots_sdk_xtask_coverage_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let report_path = dir.join("summary.json");
        let filename = dir.join("crates/sdk/src/lib.rs");
        std::fs::write(
            &report_path,
            format!(
                r#"{{"data":[{{"files":[{{"filename":"{}","summary":{{"lines":{{"count":100,"covered":97,"percent":97.0}},"functions":{{"count":100,"covered":98,"percent":98.0}},"regions":{{"count":100,"covered":99,"percent":99.0}}}}}}],"totals":{{"lines":{{"count":100,"covered":97,"percent":97.0}},"functions":{{"count":100,"covered":98,"percent":98.0}},"regions":{{"count":100,"covered":99,"percent":99.0}}}}}}]}}"#,
                filename.display()
            ),
        )
        .expect("report");
        let contract = toml::from_str::<CoverageContract>(&CONTRACT.replace(
            r#"[scopes.xtask]
paths = ["tools/xtask/src/**"]
threshold = 100.0

"#,
            "",
        ))
        .expect("contract parses");
        assert!(evaluate_report(&dir, &report_path, &contract).is_err());
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn disabled_enforcement_accepts_measured_scope() {
        let dir = std::env::temp_dir().join(format!(
            "radroots_sdk_xtask_coverage_disabled_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let report_path = dir.join("summary.json");
        let filename = dir.join("crates/sdk/src/lib.rs");
        std::fs::write(
            &report_path,
            format!(
                r#"{{"data":[{{"files":[{{"filename":"{}","summary":{{"lines":{{"count":1,"covered":0,"percent":0.0}},"functions":{{"count":1,"covered":0,"percent":0.0}},"regions":{{"count":1,"covered":0,"percent":0.0}}}}}}],"totals":{{"lines":{{"count":1,"covered":0,"percent":0.0}},"functions":{{"count":1,"covered":0,"percent":0.0}},"regions":{{"count":1,"covered":0,"percent":0.0}}}}}}]}}"#,
                filename.display()
            ),
        )
        .expect("report");
        let raw = CONTRACT
            .replace("enforce = true", "enforce = false")
            .replace(
                r#"[scopes.xtask]
paths = ["tools/xtask/src/**"]
threshold = 100.0

"#,
                "",
            );
        let contract = toml::from_str::<CoverageContract>(&raw).expect("contract parses");
        evaluate_report(&dir, &report_path, &contract).expect("disabled report passes");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
