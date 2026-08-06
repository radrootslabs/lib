use std::path::Path;

use crate::build_control::Mode;
use crate::sdk_generation::{
    output::package_outputs, package_matrix::validate_package_matrix, package_metadata,
};

pub fn generate_all(source_root: &Path, consumer_root: &Path, mode: Mode) -> Result<(), String> {
    generate_ts(consumer_root, mode)?;
    generate_package_metadata(source_root, consumer_root, mode)
}

pub fn generate_ts(consumer_root: &Path, mode: Mode) -> Result<(), String> {
    validate_package_matrix()?;
    for output in package_outputs()? {
        for generated_file in output.files() {
            let path = consumer_root
                .join(output.spec.package_dir)
                .join(generated_file.relative_path);
            crate::sdk_generation::fs::write_or_check(&path, &generated_file.contents, mode)?;
        }
        let provenance_file = output.provenance_file();
        crate::sdk_generation::fs::write_or_check(
            &consumer_root.join(&provenance_file.relative_path),
            &provenance_file.contents,
            mode,
        )?;
        println!("generated TypeScript package {}", output.spec.package_name);
    }
    Ok(())
}

pub fn generate_package_metadata(
    source_root: &Path,
    consumer_root: &Path,
    mode: Mode,
) -> Result<(), String> {
    validate_package_matrix()?;
    package_metadata::generate_package_metadata(source_root, consumer_root, mode)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        build_control::Mode,
        sdk_generation::package_matrix::{package_specs, wasm_package_specs},
    };

    use super::generate_all;

    #[test]
    fn explicit_consumer_generation_round_trips_and_detects_drift() {
        let source = tempfile::TempDir::new().expect("source fixture");
        let consumer = tempfile::TempDir::new().expect("consumer fixture");
        fs::write(source.path().join("LICENSE-MIT"), "MIT fixture\n").expect("MIT license");
        fs::write(source.path().join("LICENSE-APACHE"), "Apache fixture\n")
            .expect("Apache license");
        for (package_name, package_dir) in package_specs()
            .iter()
            .map(|spec| (spec.package_name, spec.package_dir))
            .chain(
                wasm_package_specs()
                    .iter()
                    .map(|spec| (spec.package_name, spec.package_dir)),
            )
        {
            let directory = consumer.path().join(package_dir);
            fs::create_dir_all(&directory).expect("package directory");
            fs::write(
                directory.join("package.json"),
                format!(
                    "{{\n  \"name\": \"{package_name}\",\n  \"description\": \"Fixture package\"\n}}\n"
                ),
            )
            .expect("package manifest");
        }

        generate_all(source.path(), consumer.path(), Mode::Write).expect("write generation");
        generate_all(source.path(), consumer.path(), Mode::Check).expect("fresh generation");

        let generated = consumer
            .path()
            .join(package_specs()[0].package_dir)
            .join("src/generated/types.ts");
        fs::write(&generated, "drift\n").expect("drift generated output");
        let error = generate_all(source.path(), consumer.path(), Mode::Check)
            .expect_err("drift must fail check mode");
        assert!(error.contains("stale generated SDK output"));
    }
}
