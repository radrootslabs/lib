#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(unsafe_code)]
#![recursion_limit = "256"]

// These release-qualification modules are executable integration boundaries:
// their governed lanes invoke external toolchains, fuzzers, package builds,
// advisory scanners, SBOM generators, and target checks. They are exercised
// by their dedicated release gates and must not recursively execute inside
// xtask's unit-coverage process.
#[cfg_attr(coverage_nightly, coverage(off))]
mod api_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod architecture;
#[cfg_attr(coverage_nightly, coverage(off))]
mod catalog;
#[cfg_attr(coverage_nightly, coverage(off))]
mod consolidation;
#[cfg_attr(coverage_nightly, coverage(off))]
mod contract;
mod coverage;
#[cfg_attr(coverage_nightly, coverage(off))]
mod dto_roots;
#[cfg_attr(coverage_nightly, coverage(off))]
mod fuzz_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod generate;
#[cfg_attr(coverage_nightly, coverage(off))]
mod hygiene;
#[cfg_attr(coverage_nightly, coverage(off))]
mod portable_qualification;
mod release_graph;
#[cfg_attr(coverage_nightly, coverage(off))]
mod release_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod supply_chain_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod target_qualification;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask architecture");
    eprintln!("  cargo xtask architecture-ci");
    eprintln!("  cargo xtask check-api-boundaries");
    eprintln!("  cargo xtask check-dependency-boundaries");
    eprintln!("  cargo xtask catalog check|write");
    eprintln!("  cargo xtask contract validate");
    eprintln!("  cargo xtask contract event-contract-registry-v7 [--write]");
    eprintln!("  cargo xtask contract knowledge-manifest [--write]");
    eprintln!("  cargo xtask consolidation baseline");
    eprintln!("  cargo xtask consolidation history [--archive-root <absolute-directory>]");
    eprintln!("  cargo xtask consolidation history-rehearsal");
    eprintln!("  cargo xtask dto-roots --check|--write");
    eprintln!("  cargo xtask generate protocol --check|--write");
    eprintln!("  cargo xtask release preflight");
    eprintln!("  cargo xtask release qualify-features");
    eprintln!("  cargo xtask release qualify-graph");
    eprintln!("  cargo xtask release qualify-api");
    eprintln!("  cargo xtask release qualify-fuzz");
    eprintln!("  cargo xtask release qualify-portable");
    eprintln!("  cargo xtask release qualify-supply-chain");
    eprintln!("  cargo xtask release qualify-targets");
    eprintln!("  cargo xtask coverage run-crate --crate <crate> [--out <dir>]");
    eprintln!("  cargo xtask coverage required-crates");
    eprintln!("  cargo xtask coverage workspace-crates");
    eprintln!(
        "  cargo xtask coverage report --scope <scope> --summary <file> --lcov <file> --out <file> [--policy-gate | (--fail-under-exec-lines <pct> --fail-under-functions <pct> --fail-under-regions <pct> --fail-under-branches <pct> [--require-branches])]"
    );
    eprintln!(
        "  cargo xtask coverage report-missing --scope <scope> --out <file> --reason <reason>"
    );
    eprintln!(
        "  cargo xtask coverage refresh-summary [--reports-root <dir>] [--out <file>] [--status-out <file>]"
    );
    eprintln!("  cargo xtask hygiene forbidden-identifiers");
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

fn validate_protocol_contracts() -> Result<(), String> {
    use radroots_protocol::{capability, event, runtime, schema};

    capability::v1::validate_catalog(capability::v1::CATALOG).map_err(|error| error.to_string())?;
    event::v1::validate_catalog(event::v1::CATALOG).map_err(|error| error.to_string())?;
    event::v1::validate_trade_state_vocabulary(event::v1::TRADE_STATE_VOCABULARY)
        .map_err(|error| error.to_string())?;
    runtime::v1::validate_catalog(runtime::v1::CATALOG).map_err(|error| error.to_string())?;
    schema::protocol_v1_registry()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn validate_contract() -> Result<(), String> {
    validate_protocol_contracts()?;
    let root = workspace_root();
    dto_roots::check(&root)?;
    generate::protocol::check(&root)?;
    contract::load_contract_bundle(&root)
        .and_then(|bundle| contract::validate_contract_bundle(&bundle))
        .and_then(|_| contract::validate_canonical_event_boundary(&root))
        .and_then(|_| contract::validate_artifact_contracts(&root))
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn release_preflight() -> Result<(), String> {
    let root = workspace_root();
    release_preflight_at(&root)
}

fn release_preflight_at(root: &Path) -> Result<(), String> {
    dto_roots::check(root)?;
    generate::protocol::check(root)?;
    contract::validate_artifact_contracts(root)?;
    contract::validate_release_preflight(root)
}

fn run_release(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("preflight") => release_preflight(),
        Some("qualify-features") => release_qualification::run_feature_matrix(&workspace_root()),
        Some("qualify-graph") => release_graph::run(&workspace_root()),
        Some("qualify-api") => api_qualification::run(&workspace_root()),
        Some("qualify-fuzz") => fuzz_qualification::run(&workspace_root()),
        Some("qualify-portable") => portable_qualification::run(&workspace_root()),
        Some("qualify-supply-chain") => supply_chain_qualification::run(&workspace_root()),
        Some("qualify-targets") => target_qualification::run(&workspace_root()),
        _ => Err("unknown release subcommand".to_string()),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_contract(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("validate") => validate_contract(),
        Some("event-contract-registry-v7") => match &args[1..] {
            [] => contract::validate_event_contract_registry_v7_inventory(&workspace_root()),
            [flag] if flag == "--write" => {
                contract::write_event_contract_registry_v7_inventory(&workspace_root())
            }
            _ => Err(
                "event-contract-registry-v7 accepts no arguments or exactly --write".to_string(),
            ),
        },
        Some("knowledge-manifest") => {
            if args.get(1).map(String::as_str) == Some("--write") {
                contract::write_knowledge_contract_manifest(&workspace_root())
            } else {
                contract::validate_knowledge_contract_manifest(&workspace_root())
            }
        }
        _ => Err("unknown contract subcommand".to_string()),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("architecture") if args.len() == 1 => architecture::validate(&workspace_root()),
        Some("architecture-ci") if args.len() == 1 => {
            catalog::check(&workspace_root())?;
            architecture::validate_ci(&workspace_root())?;
            validate_contract()
        }
        Some("check-api-boundaries") if args.len() == 1 => {
            architecture::validate_api_boundaries(&workspace_root())
        }
        Some("check-dependency-boundaries") if args.len() == 1 => {
            architecture::validate_dependency_boundaries(&workspace_root())
        }
        Some("catalog") => catalog::run(&args[1..], &workspace_root()),
        Some("contract") => run_contract(&args[1..]),
        Some("consolidation") => consolidation::run(&args[1..], &workspace_root()),
        Some("coverage") => coverage::run(&args[1..]),
        Some("dto-roots") => dto_roots::run(&args[1..], &workspace_root()),
        Some("generate") => generate::run(&args[1..], &workspace_root()),
        Some("hygiene") => hygiene::run(&args[1..], &workspace_root()),
        Some("release") => run_release(&args[1..]),
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

        let unknown_contract =
            run_contract(&["unknown".to_string()]).expect_err("unknown contract subcommand");
        assert!(unknown_contract.contains("unknown contract subcommand"));
        let invalid_registry = run_contract(&[
            "event-contract-registry-v7".to_string(),
            "--invalid".to_string(),
        ])
        .expect_err("invalid registry-v7 mode");
        assert!(invalid_registry.contains("exactly --write"));
        let unknown_root = run(&["unknown".to_string()]).expect_err("unknown command");
        assert!(unknown_root.contains("unknown command"));

        run(&["architecture".to_string()]).expect("architecture ledger validates");

        let invalid_dto_roots =
            run(&["dto-roots".to_string()]).expect_err("dto-roots requires an explicit mode");
        assert!(invalid_dto_roots.contains("--check|--write"));
        let invalid_generate =
            run(&["generate".to_string()]).expect_err("generate requires a target and mode");
        assert!(invalid_generate.contains("generate protocol --check|--write"));

        let removed_sdk = run(&["sdk".to_string(), "validate".to_string()])
            .expect_err("removed sdk command namespace");
        assert!(removed_sdk.contains("unknown command"));
    }

    #[test]
    fn release_preflight_checks_dto_root_authority_first() {
        let workspace = tempfile::TempDir::new().expect("create empty workspace");
        let error = release_preflight_at(workspace.path())
            .expect_err("missing DTO root authority must fail first");
        assert!(error.contains("DTO root authority"));
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
        let out_dir = unique_temp_dir("coverage_dispatch");
        fs::create_dir_all(&out_dir).expect("create out dir");

        run_contract(&["validate".to_string()]).expect("validate contract");
        run(&["dto-roots".to_string(), "--check".to_string()])
            .expect("validate DTO root freshness");
        run(&[
            "generate".to_string(),
            "protocol".to_string(),
            "--check".to_string(),
        ])
        .expect("validate protocol DTO inventory freshness");
        coverage::run(&["help".to_string()]).expect("coverage help");
        coverage::run(&["required-crates".to_string()]).expect("coverage required crates");
        coverage::run(&["workspace-crates".to_string()]).expect("coverage workspace crates");

        let summary_path = out_dir.join("summary.json");
        let lcov_path = out_dir.join("coverage.info");
        let gate_out = out_dir.join("gate-report.json");
        fs::write(
            &summary_path,
            r#"{"data":[{"totals":{"functions":{"percent":100.0},"lines":{"percent":100.0},"regions":{"percent":100.0}}}]}"#,
        )
        .expect("write summary");
        fs::write(&lcov_path, "DA:1,1\nBRDA:1,0,0,1\n").expect("write lcov");
        coverage::run(&[
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

        run(&["coverage".to_string(), "help".to_string()]).expect("root run coverage");
        run(&["hygiene".to_string(), "forbidden-identifiers".to_string()])
            .expect("hygiene forbidden identifiers");

        let _ = fs::remove_dir_all(out_dir);
    }

    #[test]
    fn usage_and_main_entrypoints_execute() {
        usage();
        let empty_code = main_with_args(Vec::new());
        assert_eq!(empty_code, ExitCode::from(2));
        let success_code = main_with_args(vec!["coverage".to_string(), "help".to_string()]);
        assert_eq!(success_code, ExitCode::SUCCESS);
        let failure_code = main_with_args(vec!["unknown".to_string()]);
        assert_eq!(failure_code, ExitCode::from(2));
        let _ = main();
    }

    #[test]
    fn run_contract_dispatches_validate_command() {
        let _guard = lock_workspace();
        validate_protocol_contracts().expect("final protocol contract catalogs");
        run_contract(&["validate".to_string()]).expect("contract validate");
        run_contract(&["event-contract-registry-v7".to_string()])
            .expect("contract registry-v7 inventory");
        run_contract(&["knowledge-manifest".to_string()]).expect("contract knowledge manifest");
    }
}
