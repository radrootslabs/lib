use serde_json::json;

use crate::sdk_generation::package_matrix::PackageSpec;

pub fn manifest_relative_path(spec: PackageSpec) -> String {
    format!("contracts/provenance/typescript/{}.json", spec.key)
}

pub fn package_manifest(spec: PackageSpec) -> serde_json::Value {
    json!({
        "package": spec.package_name,
        "crate": spec.crate_name,
        "version": "0.1.0",
        "license": "MIT OR Apache-2.0",
        "repository": "https://github.com/radrootslabs/sdk",
        "repository_directory": spec.package_dir,
        "generator": "radroots_lib_xtask",
        "generated": false
    })
}

#[cfg(test)]
mod tests {
    use crate::sdk_generation::{
        manifest::{manifest_relative_path, package_manifest},
        package_matrix::package_specs,
    };

    #[test]
    fn manifest_path_is_outside_package_source() {
        assert_eq!(
            manifest_relative_path(package_specs()[0]),
            "contracts/provenance/typescript/core.json"
        );
    }

    #[test]
    fn manifest_records_package_and_crate() {
        let manifest = package_manifest(package_specs()[0]);
        assert_eq!(manifest["package"], package_specs()[0].package_name);
        assert_eq!(manifest["crate"], package_specs()[0].crate_name);
        assert_eq!(manifest["version"], "0.1.0");
        assert_eq!(manifest["license"], "MIT OR Apache-2.0");
        assert_eq!(manifest["generator"], "radroots_lib_xtask");
        assert_eq!(
            manifest["repository"],
            "https://github.com/radrootslabs/sdk"
        );
        assert_eq!(
            manifest["repository_directory"],
            package_specs()[0].package_dir
        );
    }
}
