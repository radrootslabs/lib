#[path = "generated/dto_roots.rs"]
mod generated_roots;

pub use generated_roots::dto_bindgen_roots as dto_roots;

#[cfg(test)]
mod tests {
    use super::dto_roots;

    #[test]
    fn generated_core_roots_build_registry() {
        let roots = dto_roots();
        let registry = dto_bindgen::export::build_registry(roots.iter().copied());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), roots.len());
    }

    #[test]
    fn generated_core_roots_plan_typescript_bindings() {
        let roots = dto_roots();
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("dto_bindgen.toml");
        let report = dto_bindgen::export::plan_with_roots(
            dto_bindgen::export::ExportOptions::new(config_path),
            roots.iter().copied(),
        )
        .unwrap();

        assert!(report.diagnostics.is_empty());
        assert_eq!(report.registry.roots.len(), roots.len());
        assert!(report.files.iter().any(|path| path.ends_with("types.ts")));
        assert!(report.files.iter().any(|path| path.ends_with("index.ts")));
    }
}
