use crate::{fs::workspace_root, output::package_outputs, package_matrix::validate_package_matrix};

pub fn generate_ts() -> Result<(), String> {
    validate_package_matrix()?;
    let root = workspace_root()?;
    for output in package_outputs() {
        for generated_file in output.files() {
            let path = root
                .join(output.spec.package_dir)
                .join(generated_file.relative_path);
            crate::fs::write_if_changed(&path, &generated_file.contents)?;
        }
        println!("generated TypeScript package {}", output.spec.package_name);
    }
    Ok(())
}
