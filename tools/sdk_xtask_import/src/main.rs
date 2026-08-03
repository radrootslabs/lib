mod api_qualification;
mod architecture;
mod bindings;
mod check;
mod cli_host;
mod contracts;
mod coverage;
mod coverage_policy;
#[allow(dead_code)]
mod dto_roots;
mod fs;
mod generate;
mod manifest;
mod output;
mod package_matrix;
mod package_metadata;
mod portable_qualification;
mod release_qualification;
mod smoke;
mod target_qualification;
mod ts;
mod wasm;
mod wasm_declarations;

enum CommandAction<'a> {
    Architecture,
    ArchitectureCi,
    CheckApiBoundaries,
    CheckDependencyBoundaries,
    GenerateAll,
    GenerateBindings(&'a [String]),
    GenerateTs,
    GenerateWasm(&'a [String]),
    GeneratePackageMetadata,
    Coverage(&'a [String]),
    QualifyFeatures,
    QualifyApi,
    QualifyPortable,
    QualifyTargets,
    Check,
    Smoke(&'a [String]),
}

fn main() {
    if let Err(error) = run(std::env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let args = args.into_iter().collect::<Vec<_>>();
    match command_action(&args)? {
        CommandAction::Architecture => architecture::validate(&fs::workspace_root()?),
        CommandAction::ArchitectureCi => {
            let root = fs::workspace_root()?;
            architecture::validate_ci(&root)?;
            check::architecture_ci(&root)
        }
        CommandAction::CheckApiBoundaries => {
            architecture::validate_api_boundaries(&fs::workspace_root()?)
        }
        CommandAction::CheckDependencyBoundaries => {
            architecture::validate_dependency_boundaries(&fs::workspace_root()?)
        }
        CommandAction::GenerateAll => generate::generate_all(),
        CommandAction::GenerateBindings(rest) => bindings::generate(rest),
        CommandAction::GenerateTs => generate::generate_ts(),
        CommandAction::GenerateWasm(rest) => wasm::generate(rest),
        CommandAction::GeneratePackageMetadata => generate::generate_package_metadata(),
        CommandAction::Coverage(rest) => coverage::run(rest),
        CommandAction::QualifyFeatures => {
            release_qualification::run_feature_matrix(&fs::workspace_root()?)
        }
        CommandAction::QualifyApi => api_qualification::run(&fs::workspace_root()?),
        CommandAction::QualifyPortable => portable_qualification::run(&fs::workspace_root()?),
        CommandAction::QualifyTargets => target_qualification::run(&fs::workspace_root()?),
        CommandAction::Check => check::check(),
        CommandAction::Smoke(rest) => smoke::run(rest),
    }
}

fn command_action(args: &[String]) -> Result<CommandAction<'_>, String> {
    match args {
        [command] if command == "architecture" => Ok(CommandAction::Architecture),
        [command] if command == "architecture-ci" => Ok(CommandAction::ArchitectureCi),
        [command] if command == "check-api-boundaries" => Ok(CommandAction::CheckApiBoundaries),
        [command] if command == "check-dependency-boundaries" => {
            Ok(CommandAction::CheckDependencyBoundaries)
        }
        [command] if command == "generate" => Ok(CommandAction::GenerateAll),
        [command, target] if command == "generate" && target == "ts" => {
            Ok(CommandAction::GenerateTs)
        }
        [command, target, rest @ ..] if command == "generate" && target == "wasm" => {
            Ok(CommandAction::GenerateWasm(rest))
        }
        [command, target, rest @ ..] if command == "generate" && target == "bindings" => {
            Ok(CommandAction::GenerateBindings(rest))
        }
        [command, target] if command == "generate" && target == "package-metadata" => {
            Ok(CommandAction::GeneratePackageMetadata)
        }
        [command, rest @ ..] if command == "coverage" => Ok(CommandAction::Coverage(rest)),
        [command, target] if command == "release" && target == "qualify-features" => {
            Ok(CommandAction::QualifyFeatures)
        }
        [command, target] if command == "release" && target == "qualify-api" => {
            Ok(CommandAction::QualifyApi)
        }
        [command, target] if command == "release" && target == "qualify-portable" => {
            Ok(CommandAction::QualifyPortable)
        }
        [command, target] if command == "release" && target == "qualify-targets" => {
            Ok(CommandAction::QualifyTargets)
        }
        [command] if command == "check" => Ok(CommandAction::Check),
        [command, rest @ ..] if command == "smoke" => Ok(CommandAction::Smoke(rest)),
        [] => Err(usage()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: cargo xtask architecture | cargo xtask architecture-ci | cargo xtask check-api-boundaries | cargo xtask check-dependency-boundaries | cargo xtask generate | cargo xtask generate ts | cargo xtask generate wasm [--package <key>] | cargo xtask generate bindings <swift|kotlin> | cargo xtask generate package-metadata | cargo xtask check | cargo xtask smoke facade-rust-local | sdk-rust-local | front-doors-rust-local | cargo xtask coverage run | cargo xtask release qualify-features | cargo xtask release qualify-api | cargo xtask release qualify-portable | cargo xtask release qualify-targets"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{CommandAction, command_action};

    #[test]
    fn accepts_architecture() {
        let args = ["architecture".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::Architecture
        ));
    }

    #[test]
    fn accepts_architecture_ci() {
        let args = ["architecture-ci".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::ArchitectureCi
        ));
    }

    #[test]
    fn accepts_api_boundary_check() {
        let args = ["check-api-boundaries".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::CheckApiBoundaries
        ));
    }

    #[test]
    fn accepts_dependency_boundary_check() {
        let args = ["check-dependency-boundaries".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::CheckDependencyBoundaries
        ));
    }

    #[test]
    fn accepts_generate_ts() {
        let args = ["generate".to_owned(), "ts".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GenerateTs
        ));
    }

    #[test]
    fn accepts_generate_all() {
        let args = ["generate".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GenerateAll
        ));
    }

    #[test]
    fn accepts_generate_wasm() {
        let args = ["generate".to_owned(), "wasm".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GenerateWasm(rest) if rest.is_empty()
        ));
    }

    #[test]
    fn accepts_generate_bindings() {
        let args = [
            "generate".to_owned(),
            "bindings".to_owned(),
            "swift".to_owned(),
        ];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GenerateBindings(rest) if rest == ["swift"]
        ));
    }

    #[test]
    fn accepts_generate_package_metadata() {
        let args = ["generate".to_owned(), "package-metadata".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GeneratePackageMetadata
        ));
    }

    #[test]
    fn accepts_check() {
        let args = ["check".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::Check
        ));
    }

    #[test]
    fn accepts_smoke() {
        let args = ["smoke".to_owned(), "sdk-rust-local".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::Smoke(rest) if rest == ["sdk-rust-local"]
        ));
    }

    #[test]
    fn accepts_coverage_run() {
        let args = ["coverage".to_owned(), "run".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::Coverage(rest) if rest == ["run"]
        ));
    }

    #[test]
    fn rejects_unknown_command() {
        let args = ["generate".to_owned(), "swift".to_owned()];
        assert!(command_action(&args).is_err());
    }
}
