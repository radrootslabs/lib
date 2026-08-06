mod bindings;
mod contracts;
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

use std::path::Path;

use crate::build_control::Mode;

pub fn artifact(
    source_root: &Path,
    consumer_root: &Path,
    target: &str,
    language: &str,
    mode: Mode,
) -> Result<(), String> {
    match (target, language) {
        ("typescript", "typescript") => {
            generate::generate_all(source_root, consumer_root, mode)?;
        }
        ("wasm", "javascript") => {
            wasm::generate(source_root, consumer_root, &[], mode)?;
            generate::generate_package_metadata(source_root, consumer_root, mode)?;
        }
        ("ffi", "swift" | "kotlin") => {
            bindings::generate(source_root, consumer_root, language, mode)?;
            bindings::check(source_root, consumer_root)?;
        }
        _ => {
            return Err(format!(
                "unsupported SDK artifact route {target}/{language}"
            ));
        }
    }
    contracts::validate_sdk_contracts(consumer_root)
}
