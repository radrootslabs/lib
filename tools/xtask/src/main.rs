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
mod build_control;
mod build_output;
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod release_graph;
#[cfg_attr(coverage_nightly, coverage(off))]
mod release_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod safety_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod sdk_generation;
mod service_build_qualification;
mod service_release_artifacts;
mod service_source_lock;
mod service_source_lock_command;
#[cfg_attr(coverage_nightly, coverage(off))]
mod supply_chain_qualification;
#[cfg_attr(coverage_nightly, coverage(off))]
mod target_qualification;

use clap::{Parser, Subcommand, ValueEnum};
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "xtask", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: XtaskCommand,
}

#[derive(Debug, Subcommand)]
enum XtaskCommand {
    Architecture,
    ArchitectureCi,
    ArchitectureSourceExportCi,
    CheckApiBoundaries,
    CheckDependencyBoundaries,
    Check {
        #[arg(long)]
        group: String,
        #[arg(long, value_enum, default_value_t = GroupOperation::Check)]
        operation: GroupOperation,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        include_reserved: bool,
    },
    Catalog {
        #[command(subcommand)]
        command: CatalogCommand,
    },
    #[command(trailing_var_arg = true)]
    Contract {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(trailing_var_arg = true)]
    Consolidation {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(trailing_var_arg = true)]
    Coverage {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(name = "dto-roots", trailing_var_arg = true)]
    DtoRoots {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(trailing_var_arg = true)]
    Generate {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(trailing_var_arg = true)]
    Hygiene {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(trailing_var_arg = true)]
    Release {
        #[arg(allow_hyphen_values = true)]
        args: Vec<String>,
    },
    SourceLock {
        #[arg(long)]
        consumer_root: PathBuf,
    },
    ServiceSourceLock {
        #[arg(long, value_enum)]
        mode: ServiceSourceLockMode,
        #[arg(long)]
        service_root: PathBuf,
        #[arg(long)]
        source_archive: PathBuf,
    },
    ServiceReleaseArtifacts {
        #[arg(long, value_enum)]
        mode: ServiceReleaseArtifactMode,
        #[arg(long)]
        service_root: PathBuf,
        #[arg(long)]
        input_root: PathBuf,
        #[arg(long)]
        output_root: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        source_date_epoch: u32,
    },
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Artifact {
        #[arg(long, value_enum)]
        product: ArtifactProduct,
        #[arg(long, value_enum)]
        target: ArtifactTarget,
        #[arg(long, value_enum)]
        language: ArtifactLanguage,
        #[arg(long, value_enum)]
        mode: ArtifactMode,
        #[arg(long)]
        consumer_root: PathBuf,
        #[arg(long)]
        source_root: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        source_date_epoch: u64,
        #[arg(long)]
        builder_id: String,
        #[arg(long, value_delimiter = ',')]
        features: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Subcommand)]
enum CatalogCommand {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GroupOperation {
    Check,
    Test,
    Clippy,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ArtifactMode {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SourceMode {
    Prefetch,
    Offline,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ServiceSourceLockMode {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ServiceReleaseArtifactMode {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ArtifactProduct {
    Sdk,
    Mobile,
}

impl ArtifactProduct {
    fn as_str(self) -> &'static str {
        match self {
            Self::Sdk => "sdk",
            Self::Mobile => "mobile",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ArtifactTarget {
    Typescript,
    Wasm,
    Ffi,
    Ios,
    Android,
    Linux,
    Macos,
    Windows,
}

impl ArtifactTarget {
    fn as_str(self) -> &'static str {
        match self {
            Self::Typescript => "typescript",
            Self::Wasm => "wasm",
            Self::Ffi => "ffi",
            Self::Ios => "ios",
            Self::Android => "android",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ArtifactLanguage {
    Typescript,
    Swift,
    Kotlin,
    Javascript,
}

impl ArtifactLanguage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Typescript => "typescript",
            Self::Swift => "swift",
            Self::Kotlin => "kotlin",
            Self::Javascript => "javascript",
        }
    }
}

#[derive(Debug, Subcommand)]
enum SourceCommand {
    Materialize {
        #[arg(long)]
        consumer_root: PathBuf,
        #[arg(long)]
        cache_root: PathBuf,
        #[arg(long, value_enum)]
        mode: SourceMode,
    },
    ArchiveVerify {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        sha256: String,
    },
    ArchiveCreate {
        #[arg(long)]
        source_root: PathBuf,
        #[arg(long)]
        revision: String,
        #[arg(long)]
        output: PathBuf,
    },
}

fn usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask architecture");
    eprintln!("  cargo xtask architecture-ci");
    eprintln!("  cargo xtask architecture-source-export-ci");
    eprintln!("  cargo xtask check-api-boundaries");
    eprintln!("  cargo xtask check-dependency-boundaries");
    eprintln!("  cargo xtask check --group <group> [--operation check|test|clippy] [--execute]");
    eprintln!("  cargo xtask catalog check|write");
    eprintln!("  cargo xtask contract validate");
    eprintln!("  cargo xtask contract event-contract-registry-v7 [--write]");
    eprintln!("  cargo xtask contract knowledge-manifest [--write]");
    eprintln!("  cargo xtask consolidation baseline");
    eprintln!("  cargo xtask consolidation history [--archive-root <absolute-directory>]");
    eprintln!("  cargo xtask consolidation history-rehearsal");
    eprintln!(
        "  cargo xtask consolidation import-verify --source <id> --source-root <absolute-directory> --filtered-root <absolute-directory> --mode <check|write>"
    );
    eprintln!("  cargo xtask dto-roots --check|--write");
    eprintln!("  cargo xtask generate protocol --check|--write");
    eprintln!("  cargo xtask release preflight");
    eprintln!("  cargo xtask release qualify-features");
    eprintln!("  cargo xtask release qualify-graph");
    eprintln!("  cargo xtask release qualify-api");
    eprintln!("  cargo xtask release qualify-fuzz");
    eprintln!("  cargo xtask release qualify-portable");
    eprintln!("  cargo xtask release qualify-safety");
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
    eprintln!(
        "  cargo xtask hygiene prototype-contracts [--config <repo-relative-path>] [--strict|--report-only]"
    );
    eprintln!("  cargo xtask source-lock --consumer-root <absolute-directory>");
    eprintln!(
        "  cargo xtask service-source-lock --mode <check|write> --service-root <absolute-directory> --source-archive <absolute-bundle>"
    );
    eprintln!(
        "  cargo xtask service-release-artifacts --mode <check|write> --service-root <absolute-directory> --input-root <absolute-directory> --output-root <absolute-directory> --target <rust-target> --source-date-epoch <seconds>"
    );
    eprintln!(
        "  cargo xtask source materialize --consumer-root <absolute-directory> --cache-root <absolute-directory> --mode <prefetch|offline>"
    );
    eprintln!("  cargo xtask source archive-verify --archive <bundle> --sha256 <digest>");
    eprintln!(
        "  cargo xtask source archive-create --source-root <absolute-directory> --revision <full-sha> --output <absolute-bundle>"
    );
    eprintln!(
        "  cargo xtask artifact --product <sdk|mobile> --target <target> --language <language> --mode <check|write> --consumer-root <absolute-directory> --source-root <absolute-directory> --output <relative-path> --source-date-epoch <seconds> --builder-id <id>"
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
    service_source_lock::validate_contract(&root)?;
    service_build_qualification::validate_contract(&root)?;
    service_release_artifacts::validate_contract(&root)?;
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
    catalog::check(root)?;
    service_source_lock::validate_contract(root)?;
    service_build_qualification::validate_contract(root)?;
    service_release_artifacts::validate_contract(root)?;
    for group in ["public_native", "preview", "tools"] {
        build_control::group_plan(root, group, build_control::Operation::Check, false)?;
    }
    dto_roots::check(root)?;
    generate::protocol::check(root)?;
    contract::validate_artifact_contracts(root)?;
    contract::validate_release_preflight(root)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run_release(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("preflight") => release_preflight(),
        Some("qualify-features") => release_qualification::run_feature_matrix(&workspace_root()),
        Some("qualify-graph") => release_graph::run(&workspace_root()),
        Some("qualify-api") => api_qualification::run(&workspace_root()),
        Some("qualify-fuzz") => fuzz_qualification::run(&workspace_root()),
        Some("qualify-portable") => portable_qualification::run(&workspace_root()),
        Some("qualify-safety") => safety_qualification::run(&workspace_root()),
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
    let cli = Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().map(String::as_str)))
        .map_err(|error| error.to_string())?;
    match cli.command {
        XtaskCommand::Architecture => architecture::validate(&workspace_root()),
        XtaskCommand::ArchitectureCi => {
            catalog::check(&workspace_root())?;
            architecture::validate_ci(&workspace_root())?;
            validate_contract()
        }
        XtaskCommand::ArchitectureSourceExportCi => {
            catalog::check_source_export(&workspace_root())?;
            architecture::validate_ci(&workspace_root())?;
            validate_contract()
        }
        XtaskCommand::CheckApiBoundaries => {
            architecture::validate_api_boundaries(&workspace_root())
        }
        XtaskCommand::CheckDependencyBoundaries => {
            architecture::validate_dependency_boundaries(&workspace_root())
        }
        XtaskCommand::Check {
            group,
            operation,
            execute,
            include_reserved,
        } => {
            let operation = match operation {
                GroupOperation::Check => build_control::Operation::Check,
                GroupOperation::Test => build_control::Operation::Test,
                GroupOperation::Clippy => build_control::Operation::Clippy,
            };
            let root = workspace_root();
            let plan = build_control::group_plan(&root, &group, operation, include_reserved)?;
            if execute {
                build_control::execute_group_plan(&root, &plan)
            } else {
                build_control::print_plan(&plan);
                Ok(())
            }
        }
        XtaskCommand::Catalog { command } => match command {
            CatalogCommand::Check => catalog::run(&["check".to_owned()], &workspace_root()),
            CatalogCommand::Write => catalog::run(&["write".to_owned()], &workspace_root()),
        },
        XtaskCommand::Contract { args } => run_contract(&args),
        XtaskCommand::Consolidation { args } => consolidation::run(&args, &workspace_root()),
        XtaskCommand::Coverage { args } => coverage::run(&args),
        XtaskCommand::DtoRoots { args } => dto_roots::run(&args, &workspace_root()),
        XtaskCommand::Generate { args } => generate::run(&args, &workspace_root()),
        XtaskCommand::Hygiene { args } => hygiene::run(&args, &workspace_root()),
        XtaskCommand::Release { args } => run_release(&args),
        XtaskCommand::SourceLock { consumer_root } => {
            build_control::validate_consumer(&consumer_root).map(|_| ())
        }
        XtaskCommand::ServiceSourceLock {
            mode,
            service_root,
            source_archive,
        } => service_source_lock_command::run(
            match mode {
                ServiceSourceLockMode::Check => service_source_lock_command::CommandMode::Check,
                ServiceSourceLockMode::Write => service_source_lock_command::CommandMode::Write,
            },
            &service_root,
            &source_archive,
        ),
        XtaskCommand::ServiceReleaseArtifacts {
            mode,
            service_root,
            input_root,
            output_root,
            target,
            source_date_epoch,
        } => service_release_artifacts::run(
            match mode {
                ServiceReleaseArtifactMode::Check => service_release_artifacts::CommandMode::Check,
                ServiceReleaseArtifactMode::Write => service_release_artifacts::CommandMode::Write,
            },
            &service_root,
            &input_root,
            &output_root,
            &target,
            source_date_epoch,
        ),
        XtaskCommand::Source { command } => match command {
            SourceCommand::Materialize {
                consumer_root,
                cache_root,
                mode,
            } => build_control::materialize(
                &consumer_root,
                &cache_root,
                matches!(mode, SourceMode::Offline),
            )
            .map(|path| {
                println!("{}", path.display());
            }),
            SourceCommand::ArchiveVerify { archive, sha256 } => {
                build_control::verify_source_archive(&archive, &sha256)
            }
            SourceCommand::ArchiveCreate {
                source_root,
                revision,
                output,
            } => build_control::create_source_archive(&source_root, &revision, &output).map(
                |digest| {
                    println!("{digest}");
                },
            ),
        },
        XtaskCommand::Artifact {
            product,
            target,
            language,
            mode,
            consumer_root,
            source_root,
            output,
            source_date_epoch,
            builder_id,
            features,
        } => {
            let product = product.as_str();
            let target = target.as_str();
            let language = language.as_str();
            let mode = match mode {
                ArtifactMode::Check => build_control::Mode::Check,
                ArtifactMode::Write => build_control::Mode::Write,
            };
            build_control::validate_generation_roots(
                product,
                target,
                language,
                &consumer_root,
                &source_root,
            )?;
            if product == "sdk" {
                sdk_generation::artifact(&source_root, &consumer_root, target, language, mode)?;
            }
            build_control::artifact(
                product,
                target,
                language,
                mode,
                &consumer_root,
                &source_root,
                &output,
                source_date_epoch,
                &builder_id,
                &features,
            )
        }
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
    fn typed_build_control_cli_requires_explicit_modes_and_known_values() {
        let source_args = [
            "xtask",
            "source",
            "materialize",
            "--consumer-root",
            "/tmp/consumer",
            "--cache-root",
            "/tmp/cache",
        ];
        assert!(Cli::try_parse_from(source_args).is_err());
        assert!(Cli::try_parse_from(source_args.into_iter().chain(["--mode", "prefetch"])).is_ok());

        assert!(
            Cli::try_parse_from([
                "xtask",
                "service-source-lock",
                "--mode",
                "check",
                "--service-root",
                "/tmp/service",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "xtask",
                "service-source-lock",
                "--mode",
                "write",
                "--service-root",
                "/tmp/service",
                "--source-archive",
                "/tmp/lib.bundle",
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "xtask",
                "service-release-artifacts",
                "--mode",
                "write",
                "--service-root",
                "/tmp/service",
                "--input-root",
                "/tmp/input",
                "--output-root",
                "/tmp/output",
                "--target",
                "x86_64-unknown-linux-gnu",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "xtask",
                "service-release-artifacts",
                "--mode",
                "check",
                "--service-root",
                "/tmp/service",
                "--input-root",
                "/tmp/input",
                "--output-root",
                "/tmp/output",
                "--target",
                "x86_64-unknown-linux-gnu",
                "--source-date-epoch",
                "1",
            ])
            .is_ok()
        );

        assert!(
            Cli::try_parse_from([
                "xtask",
                "artifact",
                "--product",
                "unknown",
                "--target",
                "wasm",
                "--language",
                "javascript",
                "--mode",
                "check",
                "--consumer-root",
                "/tmp/consumer",
                "--source-root",
                "/tmp/source",
                "--output",
                "generated/manifest.json",
                "--source-date-epoch",
                "1",
                "--builder-id",
                "fixture",
            ])
            .is_err()
        );
    }

    #[test]
    fn artifact_cli_values_preserve_every_governed_identifier() {
        assert_eq!(
            [
                ArtifactProduct::Sdk.as_str(),
                ArtifactProduct::Mobile.as_str(),
            ],
            ["sdk", "mobile"]
        );
        assert_eq!(
            [
                ArtifactTarget::Typescript.as_str(),
                ArtifactTarget::Wasm.as_str(),
                ArtifactTarget::Ffi.as_str(),
                ArtifactTarget::Ios.as_str(),
                ArtifactTarget::Android.as_str(),
                ArtifactTarget::Linux.as_str(),
                ArtifactTarget::Macos.as_str(),
                ArtifactTarget::Windows.as_str(),
            ],
            [
                "typescript",
                "wasm",
                "ffi",
                "ios",
                "android",
                "linux",
                "macos",
                "windows",
            ]
        );
        assert_eq!(
            [
                ArtifactLanguage::Typescript.as_str(),
                ArtifactLanguage::Swift.as_str(),
                ArtifactLanguage::Kotlin.as_str(),
                ArtifactLanguage::Javascript.as_str(),
            ],
            ["typescript", "swift", "kotlin", "javascript"]
        );
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
        assert!(unknown_root.contains("unrecognized subcommand"));

        run(&["architecture".to_string()]).expect("architecture ledger validates");

        let invalid_dto_roots =
            run(&["dto-roots".to_string()]).expect_err("dto-roots requires an explicit mode");
        assert!(invalid_dto_roots.contains("--check|--write"));
        let invalid_generate =
            run(&["generate".to_string()]).expect_err("generate requires a target and mode");
        assert!(invalid_generate.contains("generate protocol --check|--write"));

        let removed_sdk = run(&["sdk".to_string(), "validate".to_string()])
            .expect_err("removed sdk command namespace");
        assert!(removed_sdk.contains("unrecognized subcommand"));
    }

    #[test]
    fn release_preflight_checks_catalog_authority_first() {
        let workspace = tempfile::TempDir::new().expect("create empty workspace");
        let error = release_preflight_at(workspace.path())
            .expect_err("missing catalog authority must fail first");
        assert!(error.contains("inspect artifact path") && error.contains("contracts"));
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
        run(&["hygiene".to_string(), "prototype-contracts".to_string()])
            .expect("report prototype contracts");

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
