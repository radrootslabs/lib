use std::ffi::OsStr;

pub(crate) fn is_build_output_directory(name: &OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(".build" | ".gradle" | ".kotlin" | "build" | "node_modules" | "out" | "target")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_exact_build_output_directory_names() {
        for name in [
            ".build",
            ".gradle",
            ".kotlin",
            "build",
            "node_modules",
            "out",
            "target",
        ] {
            assert!(is_build_output_directory(OsStr::new(name)), "{name}");
        }
        for name in [
            ".builder",
            ".build-output",
            "builds",
            "node_module",
            "output",
            "targets",
        ] {
            assert!(!is_build_output_directory(OsStr::new(name)), "{name}");
        }
    }
}
