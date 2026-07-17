use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    CoverageContract, LlvmCovMetric, enforce_metric, evaluate_report, metric_percent, path_matches,
    validate_contract, validate_metric,
};

const CONTRACT: &str = r#"
[policy]
enforce = true
require_regions = true
require_functions = true
require_lines = true

[toolchain]
rust = "1.97.0"
wasm_target = "wasm32-unknown-unknown"

[report]
output = "target/sdk-coverage/summary.json"
ignore_filename_regex = "generated"

[generated]
typescript = "generated TypeScript is checked elsewhere"
binding_crates = "generated binding crates are checked elsewhere"
wasm_glue = "wasm glue is checked through package validation"

[scopes.xtask_policy]
paths = ["tools/xtask/src/coverage_policy.rs"]
threshold = 100.0

[exclusions.generated]
paths = ["packages/*/src/generated/**"]
reason = "generated output is checked through reproducibility"
"#;

#[derive(Clone, Copy)]
struct Metrics {
    lines: (u64, u64, f64),
    functions: (u64, u64, f64),
    regions: (u64, u64, f64),
}

fn covered() -> Metrics {
    Metrics {
        lines: (100, 100, 100.0),
        functions: (50, 50, 100.0),
        regions: (200, 200, 100.0),
    }
}

fn contract(raw: &str) -> CoverageContract {
    toml::from_str::<CoverageContract>(raw).expect("contract parses")
}

fn test_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "radroots_sdk_coverage_policy_{name}_{}_{}",
        std::process::id(),
        stamp
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    root
}

fn metric_json(metric: (u64, u64, f64)) -> String {
    format!(
        r#"{{"count":{},"covered":{},"percent":{}}}"#,
        metric.0, metric.1, metric.2
    )
}

fn summary_json(metrics: Metrics) -> String {
    format!(
        r#"{{"lines":{},"functions":{},"regions":{}}}"#,
        metric_json(metrics.lines),
        metric_json(metrics.functions),
        metric_json(metrics.regions)
    )
}

fn report_json(filename: &str, file_metrics: Metrics, totals: Metrics) -> String {
    format!(
        r#"{{"data":[{{"files":[{{"filename":"{}","summary":{}}}],"totals":{}}}]}}"#,
        filename,
        summary_json(file_metrics),
        summary_json(totals)
    )
}

fn write_report(root: &PathBuf, raw: &str) -> PathBuf {
    let report_path = root.join("summary.json");
    fs::write(&report_path, raw).expect("write report");
    report_path
}

fn scope_file(root: &PathBuf) -> String {
    root.join("tools/xtask/src/coverage_policy.rs")
        .display()
        .to_string()
}

#[test]
fn validates_contract_shape() {
    validate_contract(&contract(CONTRACT)).expect("contract validates");
}

#[test]
fn rejects_blank_contract_fields() {
    let cases = [
        ("rust = \"1.97.0\"", "rust = \" \"", "toolchain.rust"),
        (
            "wasm_target = \"wasm32-unknown-unknown\"",
            "wasm_target = \" \"",
            "toolchain.wasm_target",
        ),
        (
            "output = \"target/sdk-coverage/summary.json\"",
            "output = \" \"",
            "report.output",
        ),
        (
            "ignore_filename_regex = \"generated\"",
            "ignore_filename_regex = \" \"",
            "report.ignore_filename_regex",
        ),
        (
            "typescript = \"generated TypeScript is checked elsewhere\"",
            "typescript = \" \"",
            "generated.typescript",
        ),
        (
            "binding_crates = \"generated binding crates are checked elsewhere\"",
            "binding_crates = \" \"",
            "generated.binding_crates",
        ),
        (
            "wasm_glue = \"wasm glue is checked through package validation\"",
            "wasm_glue = \" \"",
            "generated.wasm_glue",
        ),
        (
            "paths = [\"tools/xtask/src/coverage_policy.rs\"]",
            "paths = [\" \"]",
            "scopes.xtask_policy.paths entry",
        ),
        (
            "reason = \"generated output is checked through reproducibility\"",
            "reason = \" \"",
            "exclusions.generated.reason",
        ),
        (
            "paths = [\"packages/*/src/generated/**\"]",
            "paths = [\" \"]",
            "exclusions.generated.paths entry",
        ),
    ];

    for (from, to, expected) in cases {
        let raw = CONTRACT.replace(from, to);
        let error = validate_contract(&contract(&raw)).expect_err("invalid contract");
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn rejects_contract_collection_errors() {
    let mut no_scopes = contract(CONTRACT);
    no_scopes.scopes.clear();
    assert_eq!(
        validate_contract(&no_scopes).unwrap_err(),
        "contracts/coverage.toml scopes must not be empty"
    );

    let mut no_exclusions = contract(CONTRACT);
    no_exclusions.exclusions.clear();
    assert_eq!(
        validate_contract(&no_exclusions).unwrap_err(),
        "contracts/coverage.toml exclusions must not be empty"
    );

    let cases = [
        (
            CONTRACT.replace("[scopes.xtask_policy]", "[scopes.\"\"]"),
            "scope name",
        ),
        (
            CONTRACT.replace("[exclusions.generated]", "[exclusions.\"\"]"),
            "exclusion name",
        ),
        (
            CONTRACT.replace("threshold = 100.0", "threshold = 101.0"),
            "scopes.xtask_policy.threshold",
        ),
        (
            CONTRACT.replace(
                "paths = [\"tools/xtask/src/coverage_policy.rs\"]",
                "paths = []",
            ),
            "scopes.xtask_policy.paths must not be empty",
        ),
        (
            CONTRACT.replace("paths = [\"packages/*/src/generated/**\"]", "paths = []"),
            "exclusions.generated.paths must not be empty",
        ),
    ];

    for (raw, expected) in cases {
        let error = validate_contract(&contract(&raw)).expect_err("invalid contract");
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn matches_recursive_scope_paths() {
    assert!(path_matches(
        "crates/sdk/src/**",
        "crates/sdk/src/adapters/radrootsd.rs"
    ));
    assert!(path_matches("crates/sdk/src/**", "crates/sdk/src"));
    assert!(path_matches(
        "tools/xtask/src/coverage_policy.rs",
        "tools/xtask/src/coverage_policy.rs"
    ));
    assert!(!path_matches(
        "crates/sdk/src/**",
        "crates/sql_wasm_runtime/src/lib.rs"
    ));
}

#[test]
fn accepts_passing_reports_and_rejects_undercovered_scopes() {
    let root = test_root("passing_and_undercovered");
    let filename = scope_file(&root);
    let passing_report = report_json(&filename, covered(), covered());
    let report_path = write_report(&root, &passing_report);
    evaluate_report(&root, &report_path, &contract(CONTRACT)).expect("passing report");

    let mut undercovered = covered();
    undercovered.lines = (100, 99, 99.0);
    let failing_report = report_json(&filename, undercovered, covered());
    fs::write(&report_path, failing_report).expect("write failing report");
    let error = evaluate_report(&root, &report_path, &contract(CONTRACT))
        .expect_err("undercovered report rejected");
    assert!(error.contains("coverage scope xtask_policy"), "{error}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn disabled_enforcement_accepts_measured_undercoverage() {
    let root = test_root("disabled");
    let filename = scope_file(&root);
    let mut undercovered = covered();
    undercovered.lines = (100, 0, 0.0);
    undercovered.functions = (50, 0, 0.0);
    undercovered.regions = (200, 0, 0.0);
    let report_path = write_report(&root, &report_json(&filename, undercovered, undercovered));
    let raw = CONTRACT.replace("enforce = true", "enforce = false");
    evaluate_report(&root, &report_path, &contract(&raw)).expect("disabled policy passes");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_unreadable_malformed_and_empty_reports() {
    let root = test_root("bad_reports");
    let missing = root.join("missing.json");
    assert!(evaluate_report(&root, &missing, &contract(CONTRACT)).is_err());

    let malformed = write_report(&root, "{");
    assert!(evaluate_report(&root, &malformed, &contract(CONTRACT)).is_err());

    fs::write(&malformed, r#"{"data":[]}"#).expect("write empty report");
    assert!(evaluate_report(&root, &malformed, &contract(CONTRACT)).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_required_total_metric_failures() {
    let root = test_root("total_metrics");
    let filename = scope_file(&root);
    let mut totals = covered();
    let cases = [
        Metrics {
            lines: (0, 0, 0.0),
            ..totals
        },
        {
            totals = covered();
            totals.functions = (0, 0, 0.0);
            totals
        },
        {
            totals = covered();
            totals.regions = (0, 0, 0.0);
            totals
        },
        {
            totals = covered();
            totals.lines = (1, 2, 200.0);
            totals
        },
    ];

    let report_path = root.join("summary.json");
    for total_metrics in cases {
        fs::write(
            &report_path,
            report_json(&filename, covered(), total_metrics),
        )
        .expect("write report");
        assert!(evaluate_report(&root, &report_path, &contract(CONTRACT)).is_err());
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_scope_metric_validation_and_missing_scope_files() {
    let root = test_root("scope_metrics");
    let filename = scope_file(&root);
    let other_filename = root
        .join("tools/xtask/src/coverage.rs")
        .display()
        .to_string();
    let report_path = root.join("summary.json");

    fs::write(
        &report_path,
        report_json(&other_filename, covered(), covered()),
    )
    .expect("write unmatched report");
    let error = evaluate_report(&root, &report_path, &contract(CONTRACT))
        .expect_err("unmatched scope rejected");
    assert!(error.contains("matched no report files"), "{error}");

    let mut invalid = covered();
    invalid.lines = (0, 0, 0.0);
    fs::write(&report_path, report_json(&filename, invalid, covered())).expect("write report");
    assert!(evaluate_report(&root, &report_path, &contract(CONTRACT)).is_err());

    invalid = covered();
    invalid.functions = (1, 2, 200.0);
    fs::write(&report_path, report_json(&filename, invalid, covered())).expect("write report");
    assert!(evaluate_report(&root, &report_path, &contract(CONTRACT)).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn metric_helpers_cover_edges() {
    let valid = LlvmCovMetric {
        count: 10,
        covered: 10,
        percent: 100.0,
    };
    validate_metric("lines", &valid, true).expect("metric validates");
    enforce_metric("lines", &valid, 100.0).expect("metric passes");

    let missing = LlvmCovMetric {
        count: 0,
        covered: 0,
        percent: 0.0,
    };
    assert!(validate_metric("lines", &missing, true).is_err());
    assert!(enforce_metric("lines", &missing, 100.0).is_err());

    let invalid = LlvmCovMetric {
        count: 1,
        covered: 2,
        percent: 200.0,
    };
    assert!(validate_metric("lines", &invalid, true).is_err());
    assert_eq!(metric_percent(0, 0), 0.0);
    assert_eq!(metric_percent(4, 2), 50.0);
}
