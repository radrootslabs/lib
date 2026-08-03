use dto_bindgen_backend_ts::{DtoTypesModule, TypeScriptImport, TypeScriptModule};
use sha2::{Digest, Sha256};

use crate::{
    dto_roots,
    manifest::manifest_relative_path,
    manifest::package_manifest,
    package_matrix::{PackageSpec, package_specs},
    ts::{generated_constants_file, generated_header, generated_kinds_file, generated_types_file},
};

pub struct PackageOutput {
    pub spec: PackageSpec,
    pub types_ts: Option<TsSource>,
    pub types_imports: Vec<TypeScriptImport>,
    pub constants_ts: Option<TsSource>,
    pub kinds_ts: Option<TsSource>,
}

pub struct GeneratedFile {
    pub relative_path: String,
    pub contents: String,
}

pub enum TsSource {
    DtoRegistry(DtoTypesModule),
    Module(TypeScriptModule),
    CanonicalNativeSnapshot { contents: &'static str },
}

impl TsSource {
    fn canonical_native_snapshot(
        contents: &'static str,
        sha256: &'static str,
        source_packages: &'static [&'static str],
    ) -> Result<Self, String> {
        let actual = format!("{:x}", Sha256::digest(contents.as_bytes()));
        if actual != sha256 {
            return Err(format!(
                "canonical native type snapshot drifted: expected {sha256}, found {actual}; regenerate from and review against {}",
                source_packages.join(", ")
            ));
        }
        Ok(Self::CanonicalNativeSnapshot { contents })
    }

    fn render(&self) -> String {
        match self {
            Self::DtoRegistry(module) => module.body_ts().to_owned(),
            Self::Module(module) => module.render_source(),
            Self::CanonicalNativeSnapshot { contents } => (*contents).to_owned(),
        }
    }

    fn imports(&self) -> Option<&str> {
        match self {
            Self::DtoRegistry(module) => module.imports_ts(),
            Self::Module(_) | Self::CanonicalNativeSnapshot { .. } => None,
        }
    }
}

const EVENT_BINDINGS_TYPES_TS: &str =
    include_str!("../../../packages/event-bindings/src/generated/types.ts");
const EVENT_BINDINGS_TYPES_SHA256: &str =
    "fe4496950852715c573ff2abc7df8edede7161de75d9c68c3f4d6836e67f829a";
const TRADE_BINDINGS_TYPES_TS: &str =
    include_str!("../../../packages/trade-bindings/src/generated/types.ts");
const TRADE_BINDINGS_TYPES_SHA256: &str =
    "49560c572206095bd7de5cfd377c234a7f136e4affc13d53516ae2c4b8998e87";
const EVENT_NATIVE_SOURCES: &[&str] = &["radroots_event", "radroots_core"];
const TRADE_NATIVE_SOURCES: &[&str] = &["radroots_trade", "radroots_event", "radroots_core"];

impl PackageOutput {
    pub fn files(&self) -> Vec<GeneratedFile> {
        let mut files = Vec::new();
        if let Some(types_ts) = &self.types_ts {
            let imports = combined_imports(
                structured_imports_ts(&self.types_imports).as_deref(),
                types_ts.imports(),
            );
            files.push(GeneratedFile {
                relative_path: format!("src/generated/{}", generated_types_file()),
                contents: render_ts(types_ts, imports.as_deref()),
            });
        }
        if let Some(constants_ts) = &self.constants_ts {
            files.push(GeneratedFile {
                relative_path: format!("src/generated/{}", generated_constants_file()),
                contents: render_ts(constants_ts, None),
            });
        }
        if let Some(kinds_ts) = &self.kinds_ts {
            files.push(GeneratedFile {
                relative_path: format!("src/generated/{}", generated_kinds_file()),
                contents: render_ts(kinds_ts, None),
            });
        }
        files
    }

    pub fn provenance_file(&self) -> GeneratedFile {
        GeneratedFile {
            relative_path: manifest_relative_path(self.spec),
            contents: render_manifest(self),
        }
    }
}

pub fn package_outputs() -> Result<Vec<PackageOutput>, String> {
    Ok(vec![
        PackageOutput {
            spec: spec_by_key("core"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::core_types_module()?)),
            types_imports: Vec::new(),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("event"),
            types_ts: Some(TsSource::canonical_native_snapshot(
                EVENT_BINDINGS_TYPES_TS,
                EVENT_BINDINGS_TYPES_SHA256,
                EVENT_NATIVE_SOURCES,
            )?),
            types_imports: Vec::new(),
            constants_ts: Some(TsSource::Module(radroots_event_bindings::constants_module())),
            kinds_ts: Some(TsSource::Module(radroots_event_bindings::kinds_module())),
        },
        PackageOutput {
            spec: spec_by_key("event_index"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::event_index_types_module()?)),
            types_imports: Vec::new(),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("identity"),
            types_ts: None,
            types_imports: Vec::new(),
            constants_ts: Some(TsSource::Module(
                radroots_identity_bindings::constants_module(),
            )),
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("replica_schema"),
            types_ts: Some(TsSource::DtoRegistry(
                dto_roots::replica_schema_types_module()?,
            )),
            types_imports: Vec::new(),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("trade"),
            types_ts: Some(TsSource::canonical_native_snapshot(
                TRADE_BINDINGS_TYPES_TS,
                TRADE_BINDINGS_TYPES_SHA256,
                TRADE_NATIVE_SOURCES,
            )?),
            types_imports: Vec::new(),
            constants_ts: None,
            kinds_ts: None,
        },
    ])
}

fn spec_by_key(key: &str) -> PackageSpec {
    package_specs()
        .iter()
        .copied()
        .find(|spec| spec.key == key)
        .unwrap_or_else(|| panic!("missing package spec for {key}"))
}

fn render_ts(source: &TsSource, imports: Option<&str>) -> String {
    if matches!(source, TsSource::CanonicalNativeSnapshot { .. }) {
        return source.render();
    }
    let body = source.render();
    let imports = imports.unwrap_or("");
    let mut rendered = format!("{}{}{}", generated_header(), imports, body.trim_start());
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

fn combined_imports(first: Option<&str>, second: Option<&str>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}{second}")),
        (Some(first), None) => Some(first.to_owned()),
        (None, Some(second)) => Some(second.to_owned()),
        (None, None) => None,
    }
}

fn structured_imports_ts(imports: &[TypeScriptImport]) -> Option<String> {
    if imports.is_empty() {
        return None;
    }
    Some(
        imports
            .iter()
            .cloned()
            .fold(TypeScriptModule::new("types.ts"), |module, import| {
                module.with_import(import)
            })
            .render_source(),
    )
}

fn render_manifest(output: &PackageOutput) -> String {
    let spec = output.spec;
    let mut value = package_manifest(spec);
    value["generated"] = serde_json::Value::Bool(true);
    value["outputs"] = serde_json::Value::Object(
        output
            .files()
            .into_iter()
            .map(|file| {
                (
                    file.relative_path,
                    serde_json::Value::String(format!(
                        "{:x}",
                        Sha256::digest(file.contents.as_bytes())
                    )),
                )
            })
            .collect(),
    );
    if matches!(spec.key, "event" | "trade") {
        let (sha256, source_packages) = match spec.key {
            "event" => (EVENT_BINDINGS_TYPES_SHA256, EVENT_NATIVE_SOURCES),
            "trade" => (TRADE_BINDINGS_TYPES_SHA256, TRADE_NATIVE_SOURCES),
            _ => unreachable!(),
        };
        value["types_source"] =
            serde_json::Value::String("canonical_native_type_snapshot".to_owned());
        value["types_sha256"] = serde_json::Value::String(sha256.to_owned());
        value["types_source_packages"] = serde_json::Value::Array(
            source_packages
                .iter()
                .map(|package| serde_json::Value::String((*package).to_owned()))
                .collect(),
        );
    }
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("manifest json serializes")
    )
}

#[cfg(test)]
mod tests {
    use super::{PackageOutput, TsSource, package_outputs, render_ts};
    use crate::package_matrix::package_specs;
    use dto_bindgen_backend_ts::{
        DtoTypesModule, TypeScriptDeclaration, TypeScriptImport, TypeScriptModule, TypeScriptType,
    };

    const TRADE_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/trade-bindings/src/generated/types.ts");
    const REPLICA_SCHEMA_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/replica-schema-bindings/src/generated/types.ts");
    const EVENT_BINDINGS_CONSTANTS_TS: &str =
        include_str!("../../../packages/event-bindings/src/generated/constants.ts");
    const EVENT_BINDINGS_KINDS_TS: &str =
        include_str!("../../../packages/event-bindings/src/generated/kinds.ts");
    const IDENTITY_BINDINGS_CONSTANTS_TS: &str =
        include_str!("../../../packages/identity-bindings/src/generated/constants.ts");

    #[test]
    fn renders_sdk_header() {
        let output = render_ts(&test_module(), None);
        assert!(output.starts_with("// @generated by cargo xtask generate ts"));
        assert!(output.contains("export type A = string;"));
    }

    #[test]
    fn renders_import_prelude_after_header() {
        let output = render_ts(&test_module(), Some("import type { B } from \"b\";\n\n"));
        assert!(output.starts_with(
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nimport type"
        ));
        assert!(output.contains("export type A = string;"));
    }

    #[test]
    fn renders_module_sources() {
        let output = render_ts(&test_module(), None);
        assert_eq!(
            output,
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nexport type A = string;\n"
        );
    }

    #[test]
    fn canonical_native_snapshot_rejects_digest_drift() {
        let error = match TsSource::canonical_native_snapshot(
            "drifted snapshot\n",
            super::EVENT_BINDINGS_TYPES_SHA256,
            super::EVENT_NATIVE_SOURCES,
        ) {
            Ok(_) => panic!("digest drift must fail closed"),
            Err(error) => error,
        };

        assert!(error.contains("canonical native type snapshot drifted"));
        assert!(error.contains(super::EVENT_BINDINGS_TYPES_SHA256));
        assert!(error.contains("radroots_event"));
    }

    #[test]
    fn includes_core_and_schema_outputs() {
        let package_names = package_outputs()
            .expect("package outputs")
            .into_iter()
            .map(|output| output.spec.package_name)
            .collect::<Vec<_>>();
        assert!(package_names.contains(&"@radroots/core-bindings"));
        assert!(package_names.contains(&"@radroots/event-bindings"));
        assert!(package_names.contains(&"@radroots/event-index-bindings"));
        assert!(package_names.contains(&"@radroots/identity-bindings"));
        assert!(package_names.contains(&"@radroots/replica-schema-bindings"));
        assert!(package_names.contains(&"@radroots/trade-bindings"));
    }

    #[test]
    fn dto_registry_source_uses_generated_package_files() {
        let output = PackageOutput {
            spec: package_specs()[0],
            types_ts: Some(TsSource::DtoRegistry(DtoTypesModule::new(
                "import type { ExternalThing } from \"@radroots/external-bindings\";\n\n",
                "export type SyntheticThing = { external: ExternalThing, };",
            ))),
            types_imports: vec![TypeScriptImport::type_only(
                ["LocalPrelude"],
                "@radroots/local",
            )],
            constants_ts: None,
            kinds_ts: None,
        };
        let files = output.files();
        let types = files
            .iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");
        let manifest = output.provenance_file();

        assert_eq!(
            types.contents,
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nimport type { LocalPrelude } from \"@radroots/local\";\nimport type { ExternalThing } from \"@radroots/external-bindings\";\n\nexport type SyntheticThing = { external: ExternalThing, };\n"
        );
        assert_eq!(
            manifest.relative_path,
            "contracts/provenance/typescript/core.json"
        );
        assert!(manifest.contents.contains("\"generated\": true"));
        assert!(
            !files
                .iter()
                .any(|file| file.relative_path == "src/index.ts")
        );
    }

    #[test]
    fn package_outputs_do_not_generate_package_indices() {
        for output in package_outputs().expect("package outputs") {
            assert!(
                !output
                    .files()
                    .iter()
                    .any(|file| file.relative_path == "src/index.ts"),
                "{} index must remain handwritten source",
                output.spec.package_name
            );
        }
    }

    #[test]
    fn trade_output_uses_canonical_native_snapshot_and_matches_checked_in_types() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "trade")
            .expect("trade output");

        assert!(matches!(
            output.types_ts,
            Some(TsSource::CanonicalNativeSnapshot { .. })
        ));
        assert!(output.types_imports.is_empty());

        let types = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");

        assert_eq!(types.contents, TRADE_BINDINGS_TYPES_TS);
    }

    #[test]
    fn event_output_uses_canonical_native_snapshot_and_matches_checked_in_types() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "event")
            .expect("event output");

        assert!(matches!(
            output.types_ts,
            Some(TsSource::CanonicalNativeSnapshot { .. })
        ));
        let types = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");
        assert_eq!(types.contents, super::EVENT_BINDINGS_TYPES_TS);

        let manifest = output.provenance_file();
        assert!(manifest.contents.contains("canonical_native_type_snapshot"));
        assert!(manifest.contents.contains("radroots_event"));
        assert!(manifest.contents.contains("radroots_core"));
    }

    #[test]
    fn replica_schema_output_uses_dto_registry_and_matches_checked_in_types() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "replica_schema")
            .expect("replica_schema output");

        assert!(matches!(output.types_ts, Some(TsSource::DtoRegistry(_))));
        assert!(output.types_imports.is_empty());

        let types = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");

        assert_eq!(types.contents, REPLICA_SCHEMA_BINDINGS_TYPES_TS);
    }

    #[test]
    fn events_constants_and_kinds_use_modules_and_match_checked_in_files() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "event")
            .expect("event output");

        assert!(matches!(output.constants_ts, Some(TsSource::Module(_))));
        assert!(matches!(output.kinds_ts, Some(TsSource::Module(_))));

        let files = output.files();
        let constants = files
            .iter()
            .find(|file| file.relative_path == "src/generated/constants.ts")
            .expect("constants file");
        let kinds = files
            .iter()
            .find(|file| file.relative_path == "src/generated/kinds.ts")
            .expect("kinds file");

        assert_eq!(constants.contents, EVENT_BINDINGS_CONSTANTS_TS);
        assert_eq!(kinds.contents, EVENT_BINDINGS_KINDS_TS);
    }

    #[test]
    fn identity_constants_use_module_and_match_checked_in_file() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "identity")
            .expect("identity output");

        assert!(matches!(output.constants_ts, Some(TsSource::Module(_))));

        let constants = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/constants.ts")
            .expect("constants file");

        assert_eq!(constants.contents, IDENTITY_BINDINGS_CONSTANTS_TS);
    }

    fn test_module() -> TsSource {
        TsSource::Module(
            TypeScriptModule::new("src/generated/test.ts").with_declaration(
                TypeScriptDeclaration::type_alias("A", TypeScriptType::String),
            ),
        )
    }
}
