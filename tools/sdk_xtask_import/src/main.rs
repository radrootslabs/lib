mod check;
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
mod ts;
mod wasm;
mod wasm_declarations;

enum CommandAction<'a> {
    GenerateTs,
    GenerateWasm(&'a [String]),
    GeneratePackageMetadata,
    Coverage(&'a [String]),
    Check,
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
        CommandAction::GenerateTs => generate::generate_ts(),
        CommandAction::GenerateWasm(rest) => wasm::generate(rest),
        CommandAction::GeneratePackageMetadata => generate::generate_package_metadata(),
        CommandAction::Coverage(rest) => coverage::run(rest),
        CommandAction::Check => check::check(),
    }
}

fn command_action(args: &[String]) -> Result<CommandAction<'_>, String> {
    match args {
        [command, target] if command == "generate" && target == "ts" => {
            Ok(CommandAction::GenerateTs)
        }
        [command, target, rest @ ..] if command == "generate" && target == "wasm" => {
            Ok(CommandAction::GenerateWasm(rest))
        }
        [command, target] if command == "generate" && target == "package-metadata" => {
            Ok(CommandAction::GeneratePackageMetadata)
        }
        [command, rest @ ..] if command == "coverage" => Ok(CommandAction::Coverage(rest)),
        [command] if command == "check" => Ok(CommandAction::Check),
        [] => Err(usage()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: cargo xtask generate ts | cargo xtask generate wasm [--package <key>] | cargo xtask generate package-metadata | cargo xtask check | cargo xtask coverage run"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{CommandAction, command_action};

    #[test]
    fn accepts_generate_ts() {
        let args = ["generate".to_owned(), "ts".to_owned()];
        assert!(matches!(
            command_action(&args).expect("action"),
            CommandAction::GenerateTs
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
