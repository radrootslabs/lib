#![forbid(unsafe_code)]

mod contract;
mod coverage;
mod export_ts;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask sdk export-ts [--out <dir>]");
    eprintln!("  cargo xtask sdk export-ts-crate --crate <crate> [--out <dir>]");
    eprintln!("  cargo xtask sdk export-ts-models [--out <dir>]");
    eprintln!("  cargo xtask sdk export-ts-constants [--out <dir>]");
    eprintln!("  cargo xtask sdk export-ts-wasm [--out <dir>]");
    eprintln!("  cargo xtask sdk export-manifest [--out <dir>]");
    eprintln!("  cargo xtask sdk validate");
    eprintln!("  cargo xtask sdk release preflight");
    eprintln!("  cargo xtask sdk coverage run-crate --crate <crate> [--out <dir>]");
    eprintln!("  cargo xtask sdk coverage required-crates");
    eprintln!("  cargo xtask sdk coverage workspace-crates");
    eprintln!(
        "  cargo xtask sdk coverage report --scope <scope> --summary <file> --lcov <file> --out <file> [--policy-gate | (--fail-under-exec-lines <pct> --fail-under-functions <pct> --fail-under-regions <pct> --fail-under-branches <pct> [--require-branches])]"
    );
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
    let override_root = env::var("RADROOTS_WORKSPACE_ROOT").ok();
    workspace_root_with_override(override_root.as_deref())
}

fn parse_out_dir(args: &[String], workspace_root: &Path) -> Result<PathBuf, String> {
    if args.is_empty() {
        return Ok(workspace_root.join("target").join("sdk-export"));
    }
    if args.len() == 2 && args[0] == "--out" {
        return Ok(PathBuf::from(&args[1]));
    }
    Err("invalid export args, expected --out <dir>".to_string())
}

fn parse_crate_out_dir(
    args: &[String],
    workspace_root: &Path,
) -> Result<(String, PathBuf), String> {
    let mut crate_selector = None;
    let mut out_dir = workspace_root.join("target").join("sdk-export");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--crate" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("invalid export args, expected --crate <crate>".to_string());
                };
                crate_selector = Some(value.clone());
                index += 2;
            }
            "--out" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("invalid export args, expected --out <dir>".to_string());
                };
                out_dir = PathBuf::from(value);
                index += 2;
            }
            _ => {
                return Err(
                    "invalid export args, expected --crate <crate> [--out <dir>]".to_string(),
                );
            }
        }
    }
    let crate_selector =
        crate_selector.ok_or_else(|| "missing required --crate <crate>".to_string())?;
    Ok((crate_selector, out_dir))
}

fn export_ts_models(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_models(&root, &out_dir)
}

fn export_ts_constants(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_constants(&root, &out_dir)
}

fn export_ts_wasm(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_wasm_artifacts(&root, &out_dir)
}

fn export_manifest(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::write_ts_export_manifest(&root, &out_dir).map(|_| ())
}

fn export_ts(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let out_dir = parse_out_dir(args, &root)?;
    export_ts::export_ts_bundle(&root, &out_dir).map(|_| ())
}

fn export_ts_crate(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let (crate_selector, out_dir) = parse_crate_out_dir(args, &root)?;
    export_ts::export_ts_bundle_for_crate(&root, &out_dir, &crate_selector).map(|_| ())
}

fn validate_contract() -> Result<(), String> {
    let root = workspace_root();
    contract::load_contract_bundle(&root)
        .and_then(|bundle| contract::validate_contract_bundle(&bundle))
}

fn release_preflight() -> Result<(), String> {
    contract::validate_release_preflight(&workspace_root())
}

fn run_release(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("preflight") => release_preflight(),
        _ => Err("unknown release subcommand".to_string()),
    }
}

fn run_sdk(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("export-ts") => export_ts(&args[1..]),
        Some("export-ts-crate") => export_ts_crate(&args[1..]),
        Some("export-ts-models") => export_ts_models(&args[1..]),
        Some("export-ts-constants") => export_ts_constants(&args[1..]),
        Some("export-ts-wasm") => export_ts_wasm(&args[1..]),
        Some("export-manifest") => export_manifest(&args[1..]),
        Some("validate") => validate_contract(),
        Some("release") => run_release(&args[1..]),
        Some("coverage") => coverage::run(&args[1..]),
        _ => Err("unknown sdk subcommand".to_string()),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("sdk") => run_sdk(&args[1..]),
        _ => Err("unknown command".to_string()),
    }
}

fn main_with_args(args: Vec<String>) -> ExitCode {
    if args.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    main_with_args(env::args().skip(1).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_main_{prefix}_{ns}"))
    }

    #[test]
    fn workspace_root_resolves_and_parse_helpers_cover_branches() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").exists());

        let default_out = parse_out_dir(&[], &root).expect("default out dir");
        assert_eq!(default_out, root.join("target").join("sdk-export"));

        let custom_out = parse_out_dir(&["--out".to_string(), "custom/out".to_string()], &root)
            .expect("custom out dir");
        assert_eq!(custom_out, PathBuf::from("custom/out"));

        let invalid_out = parse_out_dir(&["--bad".to_string()], &root).expect_err("invalid out");
        assert!(invalid_out.contains("invalid export args"));
        let invalid_out_pair =
            parse_out_dir(&["--bad".to_string(), "x".to_string()], &root).expect_err("invalid out");
        assert!(invalid_out_pair.contains("invalid export args"));

        let parsed = parse_crate_out_dir(
            &[
                "--crate".to_string(),
                "radroots-core".to_string(),
                "--out".to_string(),
                "my/out".to_string(),
            ],
            &root,
        )
        .expect("parsed crate out");
        assert_eq!(parsed.0, "radroots-core".to_string());
        assert_eq!(parsed.1, PathBuf::from("my/out"));

        let missing_crate = parse_crate_out_dir(&["--out".to_string(), "x".to_string()], &root)
            .expect_err("missing crate selector");
        assert!(missing_crate.contains("missing required --crate"));

        let invalid_crate_args = parse_crate_out_dir(
            &[
                "--crate".to_string(),
                "radroots-core".to_string(),
                "--bad".to_string(),
            ],
            &root,
        )
        .expect_err("invalid crate args");
        assert!(invalid_crate_args.contains("invalid export args"));

        let missing_crate_value =
            parse_crate_out_dir(&["--crate".to_string()], &root).expect_err("missing crate value");
        assert!(missing_crate_value.contains("expected --crate <crate>"));

        let missing_out_value = parse_crate_out_dir(
            &[
                "--crate".to_string(),
                "radroots-core".to_string(),
                "--out".to_string(),
            ],
            &root,
        )
        .expect_err("missing out value");
        assert!(missing_out_value.contains("expected --out <dir>"));
    }

    #[test]
    fn workspace_root_override_takes_precedence() {
        let root = workspace_root_with_override(Some("/tmp/radroots-test-root"));
        assert_eq!(root, PathBuf::from("/tmp/radroots-test-root"));

        let fallback = workspace_root_with_override(Some("   "));
        assert!(fallback.join("Cargo.toml").exists());
    }

    #[test]
    fn run_release_and_dispatchers_cover_error_paths() {
        let unknown_release =
            run_release(&["unknown".to_string()]).expect_err("unknown release subcommand");
        assert!(unknown_release.contains("unknown release subcommand"));

        let unknown_sdk = run_sdk(&["unknown".to_string()]).expect_err("unknown sdk subcommand");
        assert!(unknown_sdk.contains("unknown sdk subcommand"));

        let unknown_root = run(&["unknown".to_string()]).expect_err("unknown command");
        assert!(unknown_root.contains("unknown command"));
    }

    #[test]
    fn export_wrappers_cover_success_and_error_paths() {
        let _guard = workspace_lock().lock().expect("lock workspace");
        let root = workspace_root();
        let out_dir = unique_temp_dir("export_wrappers");
        fs::create_dir_all(&out_dir).expect("create out dir");

        let invalid_args = vec!["--bad".to_string()];
        assert!(export_ts_models(&invalid_args).is_err());
        assert!(export_ts_constants(&invalid_args).is_err());
        assert!(export_ts_wasm(&invalid_args).is_err());
        assert!(export_manifest(&invalid_args).is_err());
        assert!(export_ts(&invalid_args).is_err());
        assert!(export_ts_crate(&invalid_args).is_err());

        let ts_rs_root = root.join("target").join("ts-rs");
        fs::create_dir_all(ts_rs_root.join("core")).expect("create ts-rs core dir");
        fs::write(
            ts_rs_root.join("core").join("types.ts"),
            "export type CoreProbe = { id: string };\n",
        )
        .expect("write core types");

        let args = vec!["--out".to_string(), out_dir.display().to_string()];
        export_manifest(&args).expect("export manifest");
        export_ts_wasm(&args).expect("export wasm");
        export_ts_constants(&args).expect("export constants");
        export_ts_models(&args).expect("export models");

        let crate_args = vec![
            "--crate".to_string(),
            "core".to_string(),
            "--out".to_string(),
            out_dir.display().to_string(),
        ];
        export_ts_crate(&crate_args).expect("export ts crate");

        let bundle_args = vec!["--out".to_string(), out_dir.display().to_string()];
        export_ts(&bundle_args).expect("export ts bundle");

        assert!(out_dir.join("ts").exists());

        let _ = fs::remove_dir_all(out_dir);
    }

    #[test]
    fn contract_and_coverage_dispatchers_execute() {
        let _guard = workspace_lock().lock().expect("lock workspace");
        let root = workspace_root();
        let out_dir = unique_temp_dir("coverage_dispatch");
        fs::create_dir_all(&out_dir).expect("create out dir");

        let coverage_refresh_path = root
            .join("target")
            .join("coverage")
            .join("coverage-refresh.tsv");
        let parent = coverage_refresh_path.parent().expect("coverage parent");
        fs::create_dir_all(parent).expect("create coverage parent");
        fs::write(&coverage_refresh_path, "stale").expect("seed stale coverage refresh");
        fs::remove_file(&coverage_refresh_path).expect("remove existing coverage refresh");
        let parent = coverage_refresh_path.parent().expect("coverage parent");
        fs::create_dir_all(parent).expect("create coverage parent");
        let required_raw = fs::read_to_string(
            root.join("contract")
                .join("coverage")
                .join("policy.toml"),
        )
        .expect("read coverage policy contract");
        let required_toml =
            toml::from_str::<toml::Value>(&required_raw).expect("parse coverage policy contract");
        let required_crates = required_toml
            .get("required")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("crates"))
            .and_then(toml::Value::as_array)
            .expect("required crates array");
        let mut rows = String::from("crate\tstatus\texec\tfunc\tbranch\tregion\treport\n");
        for crate_name in required_crates {
            let crate_name = crate_name.as_str().expect("required crate name");
            rows.push_str(&format!(
                "{crate_name}\tpass\t100.0\t100.0\t100.0\t100.0\tfile\n"
            ));
        }
        fs::write(&coverage_refresh_path, rows).expect("write coverage refresh");

        validate_contract().expect("validate contract");
        release_preflight().expect("release preflight");
        run_sdk(&["coverage".to_string(), "help".to_string()]).expect("coverage help");
        run_sdk(&["coverage".to_string(), "required-crates".to_string()])
            .expect("coverage required crates");
        run_sdk(&["coverage".to_string(), "workspace-crates".to_string()])
            .expect("coverage workspace crates");

        let summary_path = out_dir.join("summary.json");
        let lcov_path = out_dir.join("coverage.info");
        let gate_out = out_dir.join("gate-report.json");
        fs::write(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        )
        .expect("write summary");
        fs::write(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n").expect("write lcov");
        run_sdk(&[
            "coverage".to_string(),
            "report".to_string(),
            "--scope".to_string(),
            "main-test".to_string(),
            "--summary".to_string(),
            summary_path.display().to_string(),
            "--lcov".to_string(),
            lcov_path.display().to_string(),
            "--out".to_string(),
            gate_out.display().to_string(),
            "--policy-gate".to_string(),
        ])
        .expect("coverage report");

        run_sdk(&["release".to_string(), "preflight".to_string()]).expect("sdk release preflight");

        run(&[
            "sdk".to_string(),
            "coverage".to_string(),
            "help".to_string(),
        ])
        .expect("root run sdk coverage");

        let _ = fs::remove_dir_all(out_dir);
    }

    #[test]
    fn usage_and_main_entrypoints_execute() {
        usage();
        let empty_code = main_with_args(Vec::new());
        assert_eq!(empty_code, ExitCode::from(2));
        let success_code = main_with_args(vec![
            "sdk".to_string(),
            "coverage".to_string(),
            "help".to_string(),
        ]);
        assert_eq!(success_code, ExitCode::SUCCESS);
        let failure_code = main_with_args(vec!["unknown".to_string()]);
        assert_eq!(failure_code, ExitCode::from(2));
        let _ = main();
    }

    #[test]
    fn run_sdk_dispatches_export_and_validate_commands() {
        let _guard = workspace_lock().lock().expect("lock workspace");
        assert!(run_sdk(&["export-ts".to_string(), "--bad".to_string()]).is_err());
        assert!(run_sdk(&["export-ts-crate".to_string(), "--bad".to_string()]).is_err());
        assert!(run_sdk(&["export-ts-models".to_string(), "--bad".to_string()]).is_err());
        assert!(run_sdk(&["export-ts-constants".to_string(), "--bad".to_string()]).is_err());
        assert!(run_sdk(&["export-ts-wasm".to_string(), "--bad".to_string()]).is_err());
        assert!(run_sdk(&["export-manifest".to_string(), "--bad".to_string()]).is_err());
        run_sdk(&["validate".to_string()]).expect("sdk validate");
    }
}
