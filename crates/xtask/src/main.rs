#![forbid(unsafe_code)]

mod contract;
mod coverage;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask sdk validate");
    eprintln!("  cargo xtask sdk release preflight");
    eprintln!("  cargo xtask sdk coverage run-crate --crate <crate> [--out <dir>]");
    eprintln!("  cargo xtask sdk coverage required-crates");
    eprintln!("  cargo xtask sdk coverage workspace-crates");
    eprintln!(
        "  cargo xtask sdk coverage report --scope <scope> --summary <file> --lcov <file> --out <file> [--policy-gate | (--fail-under-exec-lines <pct> --fail-under-functions <pct> --fail-under-regions <pct> --fail-under-branches <pct> [--require-branches])]"
    );
    eprintln!(
        "  cargo xtask sdk coverage report-missing --scope <scope> --out <file> --reason <reason>"
    );
    eprintln!(
        "  cargo xtask sdk coverage refresh-summary [--reports-root <dir>] [--out <file>] [--status-out <file>]"
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
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_workspace() -> MutexGuard<'static, ()> {
        match workspace_lock().lock() {
            Ok(guard) => guard,
            Err(poison) => poison.into_inner(),
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("radroots_xtask_main_{prefix}_{ns}"))
    }

    fn write_file(path: &Path, content: &str) {
        let _ = fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
        fs::write(path, content).expect("write file");
    }

    fn release_preflight_with_override(release_policy_path: Option<&Path>) -> Result<(), String> {
        contract::validate_release_preflight_with_override(
            &workspace_root(),
            release_policy_path.map(PathBuf::from),
        )
    }

    fn run_sdk_with_release_policy_override(
        args: &[String],
        release_policy_path: Option<&Path>,
    ) -> Result<(), String> {
        match args.first().map(String::as_str) {
            Some("release") => match args.get(1).map(String::as_str) {
                Some("preflight") => release_preflight_with_override(release_policy_path),
                Some(other) => Err(format!("unknown release subcommand: {other}")),
                None => Err("missing release subcommand".to_string()),
            },
            _ => run_sdk(args),
        }
    }

    #[test]
    fn workspace_root_resolves() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn workspace_root_override_takes_precedence() {
        let root = workspace_root_with_override(Some("/tmp/radroots-test-root"));
        assert_eq!(root, PathBuf::from("/tmp/radroots-test-root"));

        let fallback = workspace_root_with_override(Some("   "));
        assert!(fallback.join("Cargo.toml").exists());

        let default_root = workspace_root_with_override(None);
        assert!(default_root.join("Cargo.toml").exists());
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
    fn lock_workspace_recovers_from_poisoned_mutex() {
        let handle = std::thread::spawn(|| {
            let _guard = workspace_lock().lock().expect("lock workspace");
            panic!("poison workspace lock");
        });
        assert!(handle.join().is_err());

        let _guard = lock_workspace();
    }

    #[test]
    fn contract_and_coverage_dispatchers_execute() {
        let _guard = lock_workspace();
        let root = workspace_root();
        let out_dir = unique_temp_dir("coverage_dispatch");
        fs::create_dir_all(&out_dir).expect("create out dir");
        let release_policy_path = out_dir.join("publish-policy.toml");
        let release_policy = contract::synthetic_release_policy_for_workspace(&root)
            .expect("synthetic release policy");
        write_file(&release_policy_path, &release_policy);

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
        let required_raw =
            fs::read_to_string(root.join("policy").join("coverage").join("policy.toml"))
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
        release_preflight_with_override(Some(&release_policy_path)).expect("release preflight");
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

        run_sdk_with_release_policy_override(
            &["release".to_string(), "preflight".to_string()],
            Some(&release_policy_path),
        )
        .expect("sdk release preflight");

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
    fn run_sdk_dispatches_validate_command() {
        let _guard = lock_workspace();
        run_sdk(&["validate".to_string()]).expect("sdk validate");
    }
}
