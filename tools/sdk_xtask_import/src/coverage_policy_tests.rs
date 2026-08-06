use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    BranchKey, CoverageContract, LlvmCovFunction, LlvmCovMetric, LlvmCovReport,
    annotated_source_lines, brace_delta, branch_filename, branch_key, enforce_metric,
    evaluate_report, is_authored_function_line, is_ignorable_branch, is_ignorable_source_line,
    metric_percent, normalized_source_metrics, path_matches, region_filename, region_key,
    report_filename, validate_contract, validate_metric,
};

const CONTRACT: &str = r#"
[policy]
enforce = true
require_regions = true
require_functions = true
require_executable_lines = true
require_branches = true

[toolchain]
rust = "1.97.1"
coverage_rust = "nightly"
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
threshold = 90.0

[exclusions.generated]
paths = ["packages/*/src/generated/**"]
reason = "generated output is checked through reproducibility"
"#;

#[derive(Clone, Copy)]
struct Metrics {
    lines: (u64, u64, f64),
    functions: (u64, u64, f64),
    regions: (u64, u64, f64),
    branches: (u64, u64, f64),
}

fn covered() -> Metrics {
    Metrics {
        lines: (100, 100, 100.0),
        functions: (50, 50, 100.0),
        regions: (200, 200, 100.0),
        branches: (80, 80, 100.0),
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
        r#"{{"lines":{},"functions":{},"regions":{},"branches":{}}}"#,
        metric_json(metrics.lines),
        metric_json(metrics.functions),
        metric_json(metrics.regions),
        metric_json(metrics.branches)
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

fn write_report(root: &Path, raw: &str) -> PathBuf {
    let report_path = root.join("summary.json");
    fs::write(&report_path, raw).expect("write report");
    report_path
}

fn scope_file(root: &Path) -> String {
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
        ("rust = \"1.97.1\"", "rust = \" \"", "toolchain.rust"),
        (
            "coverage_rust = \"nightly\"",
            "coverage_rust = \" \"",
            "toolchain.coverage_rust",
        ),
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
            CONTRACT.replace("threshold = 90.0", "threshold = 101.0"),
            "scopes.xtask_policy.threshold",
        ),
        (
            CONTRACT.replace(
                "threshold = 90.0",
                "threshold = 90.0\nbranch_reason = \"not applicable\"",
            ),
            "branch_reason requires branches_applicable = false",
        ),
        (
            CONTRACT.replace(
                "threshold = 90.0",
                "threshold = 90.0\nbranches_applicable = false",
            ),
            "scopes.xtask_policy.branch_reason",
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
    undercovered.lines = (100, 89, 89.0);
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
    undercovered.branches = (80, 0, 0.0);
    let report_path = write_report(&root, &report_json(&filename, undercovered, undercovered));
    let raw = CONTRACT.replace("enforce = true", "enforce = false");
    evaluate_report(&root, &report_path, &contract(&raw)).expect("disabled policy passes");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn branch_inapplicability_is_explicit_and_rejects_measured_branches() {
    let root = test_root("branch_inapplicability");
    let filename = scope_file(&root);
    let raw = CONTRACT.replace(
        "threshold = 90.0",
        "threshold = 90.0\nbranches_applicable = false\nbranch_reason = \"straight-line source\"",
    );
    let mut no_branches = covered();
    no_branches.branches = (0, 0, 0.0);
    let report_path = write_report(&root, &report_json(&filename, no_branches, covered()));
    evaluate_report(&root, &report_path, &contract(&raw)).expect("no-branch scope passes");

    fs::write(&report_path, report_json(&filename, covered(), covered()))
        .expect("write measured branches");
    let error = evaluate_report(&root, &report_path, &contract(&raw))
        .expect_err("measured branch drift rejected");
    assert!(error.contains("declared branches inapplicable"), "{error}");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn duplicate_harness_variants_are_merged_and_test_modules_are_excluded() {
    let root = test_root("duplicate_harness_variants");
    let filename = scope_file(&root);
    let source_path = Path::new(&filename);
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source parent");
    fs::write(
        source_path,
        "pub fn production() -> bool { true }\n\n#[cfg(test)]\nmod tests {\n    fn helper() -> bool { false }\n}\n",
    )
    .expect("write source");
    let report = format!(
        r#"{{"data":[{{"files":[{{"filename":"{filename}","summary":{summary}}}],"functions":[{{"count":0,"filenames":["{filename}"],"regions":[[1,1,1,37,0,0,0,0]]}},{{"count":1,"filenames":["{filename}"],"regions":[[1,1,1,37,1,0,0,0]]}},{{"count":0,"filenames":["{filename}"],"regions":[[5,5,5,34,0,0,0,0]]}}],"totals":{summary}}}]}}"#,
        summary = summary_json(covered()),
    );
    let report_path = write_report(&root, &report);

    evaluate_report(&root, &report_path, &contract(CONTRACT))
        .expect("normalized production coverage passes");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn normalized_details_fail_closed_and_merge_every_supported_record_shape() {
    let root = test_root("normalized_record_shapes");
    let filename = scope_file(&root);
    let source_path = Path::new(&filename);
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("source parent");
    fs::write(
        source_path,
        "pub fn production(flag: bool) -> bool { if flag { true } else { false } }\nfn uncovered() {}\nconst VALUE: bool = true;\n#[cfg(test)]\nmod tests { fn ignored() {} }\n",
    )
    .expect("source");
    let outside = root.join("outside.rs").display().to_string();
    fs::write(&outside, "fn outside() {}\n").expect("outside source");
    let summary = summary_json(covered());
    let report = format!(
        r#"{{"data":[{{"files":[],"functions":[
          {{"count":0,"filenames":[],"regions":[],"branches":[]}},
          {{"count":0,"filenames":["{filename}"],"regions":[[1,1,1]],"branches":[]}},
          {{"count":0,"filenames":["{outside}"],"regions":[[1,1,1,16,0,0,0,0]],"branches":[]}},
          {{"count":0,"filenames":["{filename}"],"regions":[[2,1,2,18,0,0,0,0]],"branches":[]}},
          {{"count":1,"filenames":["{filename}"],"regions":[[3,1,3,25,1,0,0,0]],"branches":[]}},
          {{"count":1,"filenames":["{filename}","{outside}"],"regions":[[1,1,1,76,1,0,0,0],[1,1,5,36,1,0,0,0],[2,1,2,18,0,0,0,1],[4,1,4,13,0,0,0,0],[1,1,1,2,0,1,0,0],[1]],"branches":[[1,40,1,47,1,0,0,0,4],[2,1,2,3,0,1,0,0,4],[4,1,4,3,0,0,0,0,4],[99,1,99,3,0,0,0,0,4],[1,1,1,2,0,0,1,0,4],[1]]}},
          {{"count":0,"filenames":["{filename}","{outside}"],"regions":[[1,1,1,76,0,0,0,0],[2,1,2,18,1,0,0,0]],"branches":[[1,40,1,47,0,1,0,0,4],[2,1,2,3,1,0,0,0,4]]}}
        ],"totals":{summary}}}]}}"#,
    );
    let parsed = serde_json::from_str::<LlvmCovReport>(&report).expect("detail report");
    let policy = contract(CONTRACT);
    let scope = policy.scopes.get("xtask_policy").expect("scope");
    let (lines, functions, regions, branches) =
        normalized_source_metrics(&root, &parsed.data[0], scope).expect("normalized details");
    assert!(lines.count >= 2);
    assert_eq!(functions.count, 2);
    assert_eq!(functions.covered, 1);
    assert!(regions.count >= 2);
    assert_eq!(branches.count, 6);
    assert_eq!(branches.covered, 4);
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
            totals.branches = (0, 0, 0.0);
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

#[test]
fn detail_key_and_path_helpers_cover_valid_and_short_records() {
    assert!(region_key(&[1, 2, 3, 4, 5, 6, 7, 8]).is_some());
    assert!(region_key(&[1, 2, 3]).is_none());
    assert!(branch_key(&[1, 2, 3, 4, 5, 6, 7, 8, 9]).is_some());
    assert!(branch_key(&[1, 2, 3]).is_none());
    assert_eq!(brace_delta("fn value() { if true { 1 } else { 0 } }"), 0);
    assert_eq!(brace_delta("{{"), 2);
    assert_eq!(brace_delta("}"), -1);

    let root = Path::new("/workspace/sdk");
    assert_eq!(
        report_filename(root, "/workspace/sdk/crates/sdk/src/lib.rs"),
        "crates/sdk/src/lib.rs"
    );
    assert_eq!(
        report_filename(root, "/elsewhere/lib.rs"),
        "/elsewhere/lib.rs"
    );
    assert_eq!(
        report_filename(
            root,
            "/workspace/sdk/crates/sdk/src/adapters/../../tests/unit.rs"
        ),
        "crates/sdk/tests/unit.rs"
    );
}

#[test]
fn detail_filename_helpers_honor_record_indexes_and_safe_fallbacks() {
    let function = LlvmCovFunction {
        count: 1,
        filenames: vec!["primary.rs".to_owned(), "expanded.rs".to_owned()],
        regions: Vec::new(),
        branches: Vec::new(),
    };
    assert_eq!(
        region_filename(&function, &[1, 1, 1, 2, 1, 1, 0, 0]),
        Some("expanded.rs")
    );
    assert_eq!(
        region_filename(&function, &[1, 1, 1, 2, 1, 99, 0, 0]),
        Some("primary.rs")
    );
    assert_eq!(region_filename(&function, &[1]), Some("primary.rs"));
    assert_eq!(
        branch_filename(&function, &[1, 1, 1, 2, 1, 0, 1, 0, 4]),
        Some("expanded.rs")
    );
    assert_eq!(
        branch_filename(&function, &[1, 1, 1, 2, 1, 0, 99, 0, 4]),
        Some("primary.rs")
    );
    assert_eq!(branch_filename(&function, &[1]), Some("primary.rs"));

    let no_filenames = LlvmCovFunction {
        count: 0,
        filenames: Vec::new(),
        regions: Vec::new(),
        branches: Vec::new(),
    };
    assert_eq!(region_filename(&no_filenames, &[1]), None);
    assert_eq!(branch_filename(&no_filenames, &[1]), None);
}

#[test]
fn authored_and_annotated_source_helpers_cover_marker_shapes() {
    let root = test_root("source_helpers");
    let source = root.join("source.rs");
    fs::write(
        &source,
        "pub fn production() {}\n#[cfg(test)]\nmod tests {\n fn helper() {}\n}\n#[cfg(all(test, feature = \"x\"))]\nfn gated() {}\n#[cfg_attr(coverage_nightly, coverage(off))]\nfn excluded() {}\n",
    )
    .expect("source");
    let filename = source.display().to_string();
    let mut cache = std::collections::BTreeMap::new();

    assert!(is_authored_function_line(&filename, 1, &mut cache));
    assert!(!is_authored_function_line(&filename, 2, &mut cache));
    assert!(!is_ignorable_source_line(&filename, 1, &mut cache));
    assert!(is_ignorable_source_line(&filename, 2, &mut cache));
    assert!(is_ignorable_source_line(&filename, 4, &mut cache));
    assert!(is_ignorable_source_line(&filename, 7, &mut cache));
    assert!(is_ignorable_source_line(&filename, 9, &mut cache));
    assert!(!is_ignorable_source_line(&filename, 0, &mut cache));
    assert!(!is_ignorable_source_line(
        root.join("missing.rs").to_str().expect("path"),
        1,
        &mut cache
    ));

    let marked = annotated_source_lines(
        "#[cfg(test)]\n// note\nfn test() {}\nfn live() {}",
        "cfg(test)",
    );
    assert_eq!(marked, vec![true, true, true, false]);
    let coverage_off = annotated_source_lines(
        "#[cfg_attr(coverage_nightly, coverage(off))]\n#[inline]\nfn excluded() {\n if true {\n  work();\n }\n}\nfn live() {}",
        "cfg_attr(coverage_nightly, coverage(off))",
    );
    assert_eq!(
        coverage_off,
        vec![true, true, true, true, true, true, true, false]
    );
    assert_eq!(
        annotated_source_lines("#[cfg(test)]\nfn declaration();\nfn live() {}", "cfg(test)"),
        vec![true, true, false]
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn branch_filter_only_excludes_explicit_synthetic_constructs() {
    let root = test_root("branch_helpers");
    let source = root.join("source.rs");
    fs::write(
        &source,
        "fn value() {\n let _ = call()?;\n unreachable!();\n assert!(matches!(value, Some(_)));\n if live { work(); }\n}\n",
    )
    .expect("source");
    let filename = source.display().to_string();
    let mut cache = std::collections::BTreeMap::new();
    assert!(!is_ignorable_source_line(&filename, 1, &mut cache));

    let branch = |line_start, column_start, line_end, column_end| BranchKey {
        line_start,
        column_start,
        line_end,
        column_end,
        kind: 4,
    };
    assert!(is_ignorable_branch(
        &filename,
        &branch(2, 16, 2, 17),
        &mut cache
    ));
    assert!(is_ignorable_branch(
        &filename,
        &branch(3, 2, 3, 10),
        &mut cache
    ));
    assert!(is_ignorable_branch(
        &filename,
        &branch(4, 10, 4, 18),
        &mut cache
    ));
    assert!(!is_ignorable_branch(
        &filename,
        &branch(5, 2, 5, 9),
        &mut cache
    ));
    assert!(!is_ignorable_branch(
        &filename,
        &branch(5, 2, 6, 1),
        &mut cache
    ));
    assert!(!is_ignorable_branch(
        root.join("missing.rs").to_str().expect("path"),
        &branch(1, 1, 1, 2),
        &mut cache
    ));
    fs::remove_dir_all(root).expect("cleanup");
}
