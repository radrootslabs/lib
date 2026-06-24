use crate::{
    dto_render::DtoTypesModule,
    dto_roots,
    manifest::manifest_file_name,
    manifest::package_manifest,
    package_matrix::{PackageSpec, package_specs},
    ts::{generated_constants_file, generated_header, generated_kinds_file, generated_types_file},
};
use radroots_sdk_binding_model::TsModule;

pub struct PackageOutput {
    pub spec: PackageSpec,
    pub types_ts: Option<TsSource>,
    pub types_imports_ts: Option<&'static str>,
    pub constants_ts: Option<TsSource>,
    pub kinds_ts: Option<TsSource>,
}

pub struct GeneratedFile {
    pub relative_path: String,
    pub contents: String,
}

#[allow(dead_code)]
pub enum TsSource {
    DtoRegistry(DtoTypesModule),
    Module(TsModule),
}

impl TsSource {
    fn render(&self) -> String {
        match self {
            Self::DtoRegistry(module) => module.body_ts().to_owned(),
            Self::Module(module) => module.render(),
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
            let imports = combined_imports(self.types_imports_ts, types_ts.imports());
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
        files.push(GeneratedFile {
            relative_path: format!("src/generated/{}", manifest_file_name()),
            contents: render_manifest(self.spec),
        });
        files.push(GeneratedFile {
            relative_path: "src/index.ts".to_owned(),
            contents: render_index(self),
        });
        files
    }
}

pub fn package_outputs() -> Result<Vec<PackageOutput>, String> {
    Ok(vec![
        PackageOutput {
            spec: spec_by_key("core"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::core_types_module()?)),
            types_imports_ts: None,
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("events"),
            types_ts: Some(TsSource::DtoRegistry(dto_roots::events_types_module()?)),
            types_imports_ts: None,
            constants_ts: Some(TsSource::Module(
                radroots_events_bindings::constants_module(),
            )),
            kinds_ts: Some(TsSource::Module(radroots_events_bindings::kinds_module())),
        },
        PackageOutput {
            spec: spec_by_key("events_indexed"),
            types_ts: Some(TsSource::Module(
                radroots_events_indexed_bindings::types_module(),
            )),
            types_imports_ts: None,
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("identity"),
            types_ts: None,
            types_imports_ts: None,
            constants_ts: Some(TsSource::Module(
                radroots_identity_bindings::constants_module(),
            )),
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("replica_db_schema"),
            types_ts: Some(TsSource::Module(
                radroots_replica_db_schema_bindings::types_module(),
            )),
            types_imports_ts: Some(REPLICA_DB_SCHEMA_TYPES_IMPORTS_TS),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("trade"),
            types_ts: Some(TsSource::Module(radroots_trade_bindings::types_module())),
            types_imports_ts: Some(TRADE_TYPES_IMPORTS_TS),
            constants_ts: None,
            kinds_ts: None,
        },
        PackageOutput {
            spec: spec_by_key("types"),
            types_ts: Some(TsSource::Module(radroots_types_bindings::types_module())),
            types_imports_ts: None,
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

const REPLICA_DB_SCHEMA_TYPES_IMPORTS_TS: &str = r#"import type {
    IResult,
    IResultList,
    IResultPass,
} from "@radroots/types-bindings";

"#;

const TRADE_TYPES_IMPORTS_TS: &str = r#"import type {
    RadrootsCoreCurrency,
    RadrootsCoreDecimal,
    RadrootsCoreDiscount,
    RadrootsCoreDiscountValue,
    RadrootsCoreMoney,
    RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice,
    RadrootsCoreUnit,
} from "@radroots/core-bindings";
import type {
    RadrootsListingImage,
    RadrootsNostrEventPtr,
    RadrootsPlotRef,
    RadrootsResourceAreaRef,
    RadrootsTradeMessagePayload,
    RadrootsTradeOrderEconomicLine,
    RadrootsTradeOrderItem,
} from "@radroots/events-bindings";

"#;

fn render_manifest(spec: PackageSpec) -> String {
    let mut value = package_manifest(spec);
    value["generated"] = serde_json::Value::Bool(true);
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("manifest json serializes")
    )
}

fn render_index(output: &PackageOutput) -> String {
    let mut lines = Vec::new();
    if output.types_ts.is_some() {
        lines.push("export * from \"./generated/types.js\";");
    }
    if output.constants_ts.is_some() {
        lines.push("export * from \"./generated/constants.js\";");
    }
    if output.kinds_ts.is_some() {
        lines.push("export * from \"./generated/kinds.js\";");
    }
    if lines.is_empty() {
        lines.push("export {};");
    }
    format!("{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{PackageOutput, TsSource, package_outputs, render_ts};
    use crate::{dto_render::DtoTypesModule, package_matrix::package_specs};
    use radroots_sdk_binding_model::{module, string, type_alias};

    #[test]
    fn renders_sdk_header() {
        let output = render_ts(
            &TsSource::Module(module(vec![type_alias("A", string())])),
            None,
        );
        assert!(output.starts_with("// @generated by cargo xtask generate ts"));
        assert!(output.contains("export type A = string;"));
    }

    #[test]
    fn renders_import_prelude_after_header() {
        let output = render_ts(
            &TsSource::Module(module(vec![type_alias("A", string())])),
            Some("import type { B } from \"b\";\n\n"),
        );
        assert!(output.starts_with(
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nimport type"
        ));
        assert!(output.contains("export type A = string;"));
    }

    #[test]
    fn renders_model_sources() {
        let output = render_ts(
            &TsSource::Module(module(vec![type_alias("A", string())])),
            None,
        );
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
    fn dto_registry_source_uses_package_shell() {
        let output = PackageOutput {
            spec: package_specs()[0],
            types_ts: Some(TsSource::DtoRegistry(DtoTypesModule::new(
                "import type { ExternalThing } from \"@radroots/external-bindings\";\n\n",
                "export type SyntheticThing = { external: ExternalThing, };",
            ))),
            types_imports_ts: Some("import type { LocalPrelude } from \"@radroots/local\";\n\n"),
            constants_ts: None,
            kinds_ts: None,
        };
        let files = output.files();
        let types = files
            .iter()
            .find(|file| file.relative_path == "src/generated/types.ts")
            .expect("types file");
        let manifest = files
            .iter()
            .find(|file| file.relative_path == "src/generated/sdk-manifest.json")
            .expect("manifest file");
        let index = files
            .iter()
            .find(|file| file.relative_path == "src/index.ts")
            .expect("index file");

        assert_eq!(
            types.contents,
            "// @generated by cargo xtask generate ts\n// Do not edit by hand.\nimport type { LocalPrelude } from \"@radroots/local\";\n\nimport type { ExternalThing } from \"@radroots/external-bindings\";\n\nexport type SyntheticThing = { external: ExternalThing, };\n"
        );
        assert!(manifest.contents.contains("\"generated\": true"));
        assert_eq!(index.contents, "export * from \"./generated/types.js\";\n");
    }
}
