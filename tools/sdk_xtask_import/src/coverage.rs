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
    exclusions: BTreeMap<String, CoverageExclusion>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoveragePolicy {
    threshold: f64,
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
    totals: LlvmCovSummary,
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
    run_llvm_cov(&root, &contract)?;
    evaluate_report(&root.join(&contract.report.output), &contract)
}

fn load_contract(root: &Path) -> Result<CoverageContract, String> {
    let path = root.join("contracts").join("coverage.toml");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn validate_contract(contract: &CoverageContract) -> Result<(), String> {
    if !(0.0..=100.0).contains(&contract.policy.threshold) {
        return Err(
            "contracts/coverage.toml policy.threshold must be between 0 and 100".to_owned(),
        );
    }
    if contract.policy.threshold != 100.0 {
        return Err("contracts/coverage.toml policy.threshold must be 100.0".to_owned());
    }
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
            "--no-clean",
            "--no-fail-fast",
        ])
        .status()
        .map_err(|error| format!("failed to run cargo llvm-cov: {error}"))?;
    if !status.success() {
        return Err(format!("cargo llvm-cov failed with status {status}"));
    }
    Ok(())
}

fn evaluate_report(report_path: &Path, contract: &CoverageContract) -> Result<(), String> {
    let raw = fs::read_to_string(report_path)
        .map_err(|error| format!("failed to read {}: {error}", report_path.display()))?;
    let report = serde_json::from_str::<LlvmCovReport>(&raw)
        .map_err(|error| format!("failed to parse {}: {error}", report_path.display()))?;
    let data = report
        .data
        .first()
        .ok_or_else(|| format!("{} did not include coverage data", report_path.display()))?;
    validate_metric("lines", &data.totals.lines, contract.policy.require_lines)?;
    validate_metric(
        "functions",
        &data.totals.functions,
        contract.policy.require_functions,
    )?;
    validate_metric(
        "regions",
        &data.totals.regions,
        contract.policy.require_regions,
    )?;
    if !contract.policy.enforce {
        println!(
            "coverage policy parsed and measured; enforcement pending final hardening gate at {}",
            report_path.display()
        );
        return Ok(());
    }
    enforce_metric("lines", &data.totals.lines, contract.policy.threshold)?;
    enforce_metric(
        "functions",
        &data.totals.functions,
        contract.policy.threshold,
    )?;
    enforce_metric("regions", &data.totals.regions, contract.policy.threshold)?;
    println!(
        "coverage policy passed at {:.1}% using {}",
        contract.policy.threshold,
        report_path.display()
    );
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{
        CoverageContract, enforce_metric, evaluate_report, validate_contract, validate_metric,
    };

    const CONTRACT: &str = r#"
[policy]
threshold = 100.0
enforce = false
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
    fn rejects_non_100_thresholds() {
        let raw = CONTRACT.replace("threshold = 100.0", "threshold = 99.0");
        let contract = toml::from_str::<CoverageContract>(&raw).expect("contract parses");
        assert!(validate_contract(&contract).is_err());
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
    fn pending_enforcement_accepts_measured_report() {
        let dir = std::env::temp_dir().join(format!(
            "radroots_sdk_xtask_coverage_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let report_path = dir.join("summary.json");
        std::fs::write(
            &report_path,
            r#"{"data":[{"totals":{"lines":{"count":1,"covered":0,"percent":0.0},"functions":{"count":1,"covered":0,"percent":0.0},"regions":{"count":1,"covered":0,"percent":0.0}}}]}"#,
        )
        .expect("report");
        let contract = toml::from_str::<CoverageContract>(CONTRACT).expect("contract parses");
        evaluate_report(&report_path, &contract).expect("pending report passes");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
