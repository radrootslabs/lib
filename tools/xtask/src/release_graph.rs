use std::path::Path;

/// Proves the standalone lower-crate resolved graph. The SDK capsule owns the
/// aggregate 19-package ordering check because its staged metadata closure
/// resolves both repository surfaces.
pub fn run(workspace_root: &Path) -> Result<(), String> {
    crate::architecture::validate_dependency_boundaries(workspace_root)?;
    println!("resolved lower release graph passed all-kind dependency validation");
    Ok(())
}
