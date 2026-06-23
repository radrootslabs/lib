use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageSpec {
    pub key: &'static str,
    pub crate_name: &'static str,
    pub crate_dir: &'static str,
    pub package_name: &'static str,
    pub package_dir: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WasmPackageSpec {
    pub key: &'static str,
    pub crate_name: &'static str,
    pub crate_dir: &'static str,
    pub package_name: &'static str,
    pub package_dir: &'static str,
    pub out_name: &'static str,
    pub out_dir: &'static str,
}

pub const PACKAGE_SPECS: [PackageSpec; 7] = [
    PackageSpec {
        key: "core",
        crate_name: "radroots_core_bindings",
        crate_dir: "crates/core_bindings",
        package_name: "@radroots/core-bindings",
        package_dir: "packages/core-bindings",
    },
    PackageSpec {
        key: "events",
        crate_name: "radroots_events_bindings",
        crate_dir: "crates/events_bindings",
        package_name: "@radroots/events-bindings",
        package_dir: "packages/events-bindings",
    },
    PackageSpec {
        key: "events_indexed",
        crate_name: "radroots_events_indexed_bindings",
        crate_dir: "crates/events_indexed_bindings",
        package_name: "@radroots/events-indexed-bindings",
        package_dir: "packages/events-indexed-bindings",
    },
    PackageSpec {
        key: "identity",
        crate_name: "radroots_identity_bindings",
        crate_dir: "crates/identity_bindings",
        package_name: "@radroots/identity-bindings",
        package_dir: "packages/identity-bindings",
    },
    PackageSpec {
        key: "replica_db_schema",
        crate_name: "radroots_replica_db_schema_bindings",
        crate_dir: "crates/replica_db_schema_bindings",
        package_name: "@radroots/replica-db-schema-bindings",
        package_dir: "packages/replica-db-schema-bindings",
    },
    PackageSpec {
        key: "trade",
        crate_name: "radroots_trade_bindings",
        crate_dir: "crates/trade_bindings",
        package_name: "@radroots/trade-bindings",
        package_dir: "packages/trade-bindings",
    },
    PackageSpec {
        key: "types",
        crate_name: "radroots_types_bindings",
        crate_dir: "crates/types_bindings",
        package_name: "@radroots/types-bindings",
        package_dir: "packages/types-bindings",
    },
];

pub const WASM_PACKAGE_SPECS: [WasmPackageSpec; 3] = [
    WasmPackageSpec {
        key: "events_codec",
        crate_name: "radroots_events_codec_wasm",
        crate_dir: "crates/events_codec_wasm",
        package_name: "@radroots/events-codec-wasm",
        package_dir: "packages/events-codec-wasm",
        out_name: "radroots_events_codec_wasm",
        out_dir: "../../packages/events-codec-wasm/dist",
    },
    WasmPackageSpec {
        key: "replica_db",
        crate_name: "radroots_replica_db_wasm",
        crate_dir: "crates/replica_db_wasm",
        package_name: "@radroots/replica-db-wasm",
        package_dir: "packages/replica-db-wasm",
        out_name: "radroots_replica_db_wasm",
        out_dir: "../../packages/replica-db-wasm/dist",
    },
    WasmPackageSpec {
        key: "replica_sync",
        crate_name: "radroots_replica_sync_wasm",
        crate_dir: "crates/replica_sync_wasm",
        package_name: "@radroots/replica-sync-wasm",
        package_dir: "packages/replica-sync-wasm",
        out_name: "radroots_replica_sync_wasm",
        out_dir: "../../packages/replica-sync-wasm/dist",
    },
];

pub const FORBIDDEN_PACKAGE_NAMES: [&str; 2] =
    ["@radroots/tangle-db-schema-bindings", "@radroots/contracts"];

pub fn package_specs() -> &'static [PackageSpec] {
    &PACKAGE_SPECS
}

pub fn wasm_package_specs() -> &'static [WasmPackageSpec] {
    &WASM_PACKAGE_SPECS
}

pub fn validate_package_matrix() -> Result<(), String> {
    let mut crate_names = BTreeSet::new();
    let mut package_names = BTreeSet::new();
    let mut package_dirs = BTreeSet::new();
    for spec in package_specs() {
        if FORBIDDEN_PACKAGE_NAMES.contains(&spec.package_name) {
            return Err(format!(
                "forbidden package in matrix: {}",
                spec.package_name
            ));
        }
        if !crate_names.insert(spec.crate_name) {
            return Err(format!("duplicate crate in matrix: {}", spec.crate_name));
        }
        if !package_names.insert(spec.package_name) {
            return Err(format!(
                "duplicate package in matrix: {}",
                spec.package_name
            ));
        }
        if !package_dirs.insert(spec.package_dir) {
            return Err(format!(
                "duplicate package directory in matrix: {}",
                spec.package_dir
            ));
        }
    }
    for spec in wasm_package_specs() {
        if FORBIDDEN_PACKAGE_NAMES.contains(&spec.package_name) {
            return Err(format!(
                "forbidden package in matrix: {}",
                spec.package_name
            ));
        }
        if !crate_names.insert(spec.crate_name) {
            return Err(format!("duplicate crate in matrix: {}", spec.crate_name));
        }
        if !package_names.insert(spec.package_name) {
            return Err(format!(
                "duplicate package in matrix: {}",
                spec.package_name
            ));
        }
        if !package_dirs.insert(spec.package_dir) {
            return Err(format!(
                "duplicate package directory in matrix: {}",
                spec.package_dir
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        FORBIDDEN_PACKAGE_NAMES, package_specs, validate_package_matrix, wasm_package_specs,
    };

    #[test]
    fn package_matrix_is_valid() {
        validate_package_matrix().expect("package matrix is valid");
    }

    #[test]
    fn approved_package_count_is_stable() {
        assert_eq!(package_specs().len(), 7);
        assert_eq!(wasm_package_specs().len(), 3);
    }

    #[test]
    fn forbidden_names_are_absent() {
        for spec in package_specs() {
            assert!(!FORBIDDEN_PACKAGE_NAMES.contains(&spec.package_name));
        }
        for spec in wasm_package_specs() {
            assert!(!FORBIDDEN_PACKAGE_NAMES.contains(&spec.package_name));
        }
    }

    #[test]
    fn replica_schema_package_uses_current_name() {
        assert!(
            package_specs()
                .iter()
                .any(|spec| spec.package_name == "@radroots/replica-db-schema-bindings")
        );
    }

    #[test]
    fn wasm_packages_use_sdk_package_names() {
        assert!(
            wasm_package_specs()
                .iter()
                .any(|spec| spec.package_name == "@radroots/replica-db-wasm")
        );
    }
}
