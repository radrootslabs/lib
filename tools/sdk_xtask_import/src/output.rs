use dto_bindgen_backend_ts::{DtoTypesModule, TypeScriptImport, TypeScriptModule};

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
}

impl TsSource {
    fn render(&self) -> String {
        match self {
            Self::DtoRegistry(module) => module.body_ts().to_owned(),
            Self::Module(module) => module.render_source(),
        }
    }

    fn imports(&self) -> Option<&str> {
        match self {
            Self::DtoRegistry(module) => module.imports_ts(),
            Self::Module(_) => None,
        }
    }
}

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
            contents: render_manifest(self.spec),
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
            spec: spec_by_key("events"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::events_types_module()?)),
            types_imports: Vec::new(),
            constants_ts: Some(TsSource::Module(
                radroots_events_bindings::constants_module(),
            )),
            kinds_ts: Some(TsSource::Module(radroots_events_bindings::kinds_module())),
        },
        PackageOutput {
            spec: spec_by_key("events_indexed"),
            types_ts: Some(TsSource::DtoRegistry(
                dto_roots::events_indexed_types_module()?,
            )),
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
            spec: spec_by_key("replica_db_schema"),
            types_ts: Some(TsSource::DtoRegistry(
                dto_roots::replica_db_schema_types_module()?,
            )),
            types_imports: vec![TypeScriptImport::type_only(
                ["IResult", "IResultList", "IResultPass"],
                "@radroots/types-bindings",
            )],
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("trade"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::trade_types_module()?)),
            types_imports: Vec::new(),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("types"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::types_types_module()?)),
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

fn render_manifest(spec: PackageSpec) -> String {
    let mut value = package_manifest(spec);
    value["generated"] = serde_json::Value::Bool(true);
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
    const REPLICA_DB_SCHEMA_BINDINGS_TYPES_TS: &str =
        include_str!("../../../packages/replica-db-schema-bindings/src/generated/types.ts");
    const EVENTS_BINDINGS_CONSTANTS_TS: &str =
        include_str!("../../../packages/events-bindings/src/generated/constants.ts");
    const EVENTS_BINDINGS_KINDS_TS: &str =
        include_str!("../../../packages/events-bindings/src/generated/kinds.ts");
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
    fn includes_core_and_types_outputs() {
        let package_names = package_outputs()
            .expect("package outputs")
            .into_iter()
            .map(|output| output.spec.package_name)
            .collect::<Vec<_>>();
        assert!(package_names.contains(&"@radroots/core-bindings"));
        assert!(package_names.contains(&"@radroots/events-bindings"));
        assert!(package_names.contains(&"@radroots/events-indexed-bindings"));
        assert!(package_names.contains(&"@radroots/identity-bindings"));
        assert!(package_names.contains(&"@radroots/replica-db-schema-bindings"));
        assert!(package_names.contains(&"@radroots/trade-bindings"));
        assert!(package_names.contains(&"@radroots/types-bindings"));
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
    fn trade_output_uses_dto_registry_and_matches_checked_in_types() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "trade")
            .expect("trade output");

        assert!(matches!(output.types_ts, Some(TsSource::DtoRegistry(_))));
        assert!(output.types_imports.is_empty());

        let types = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");

        assert_eq!(types.contents, TRADE_BINDINGS_TYPES_TS);
    }

    #[test]
    fn replica_db_schema_output_uses_dto_registry_and_matches_checked_in_types() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "replica_db_schema")
            .expect("replica_db_schema output");

        assert!(matches!(output.types_ts, Some(TsSource::DtoRegistry(_))));
        assert_eq!(output.types_imports.len(), 1);

        let types = output
            .files()
            .into_iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");

        assert_eq!(types.contents, REPLICA_DB_SCHEMA_BINDINGS_TYPES_TS);
    }

    #[test]
    fn events_constants_and_kinds_use_modules_and_match_checked_in_files() {
        let output = package_outputs()
            .expect("package outputs")
            .into_iter()
            .find(|output| output.spec.key == "events")
            .expect("events output");

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

        assert_eq!(constants.contents, EVENTS_BINDINGS_CONSTANTS_TS);
        assert_eq!(kinds.contents, EVENTS_BINDINGS_KINDS_TS);
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
