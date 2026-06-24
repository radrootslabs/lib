use std::collections::{BTreeMap, BTreeSet};

use dto_bindgen_core::{
    BackendId, Config, EnumDef, EnumRepr, FieldDef, IntRepr, LargeIntPolicy, Primitive, Registry,
    StructDef, TypeDef, TypeId, TypeRef, VariantDef, VariantShape,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DtoTypesModule {
    imports_ts: String,
    body_ts: String,
}

impl DtoTypesModule {
    pub fn new(imports_ts: impl Into<String>, body_ts: impl Into<String>) -> Self {
        Self {
            imports_ts: imports_ts.into(),
            body_ts: body_ts.into(),
        }
    }

    pub fn imports_ts(&self) -> Option<&str> {
        (!self.imports_ts.is_empty()).then_some(self.imports_ts.as_str())
    }

    pub fn body_ts(&self) -> &str {
        self.body_ts.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DtoTypeImport {
    import_name: String,
    from: String,
}

impl DtoTypeImport {
    pub fn new(import_name: impl Into<String>, from: impl Into<String>) -> Self {
        Self {
            import_name: import_name.into(),
            from: from.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DtoRegistryRenderOptions {
    config: Config,
    external_imports: BTreeMap<TypeId, DtoTypeImport>,
    external_overrides: BTreeMap<String, DtoTypeImport>,
}

impl DtoRegistryRenderOptions {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            external_imports: BTreeMap::new(),
            external_overrides: BTreeMap::new(),
        }
    }

    pub fn with_external_type(
        mut self,
        type_id: TypeId,
        import_name: impl Into<String>,
        from: impl Into<String>,
    ) -> Self {
        self.external_imports
            .insert(type_id, DtoTypeImport::new(import_name, from));
        self
    }

    pub fn with_external_override(
        mut self,
        target_type: impl Into<String>,
        import_name: impl Into<String>,
        from: impl Into<String>,
    ) -> Self {
        self.external_overrides
            .insert(target_type.into(), DtoTypeImport::new(import_name, from));
        self
    }
}

impl Default for DtoRegistryRenderOptions {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

pub fn render_registry_types(
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
) -> Result<DtoTypesModule, String> {
    let mut imports = BTreeMap::<String, BTreeSet<String>>::new();
    let mut declarations = Vec::new();
    let mut type_defs = registry
        .types_by_id
        .iter()
        .filter(|(type_id, _)| !options.external_imports.contains_key(type_id))
        .collect::<Vec<_>>();

    type_defs.sort_by(|(_, left), (_, right)| type_name(left).cmp(type_name(right)));

    for (type_id, type_def) in type_defs {
        declarations.push(render_type_def(
            *type_id,
            type_def,
            registry,
            options,
            &mut imports,
        )?);
    }

    Ok(DtoTypesModule::new(
        render_imports(&imports),
        declarations.join("\n\n"),
    ))
}

fn render_type_def(
    type_id: TypeId,
    type_def: &TypeDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    match type_def {
        TypeDef::Struct(def) => render_struct(type_id, def, registry, options, imports),
        TypeDef::Enum(def) => render_enum(type_id, def, registry, options, imports),
    }
}

fn render_struct(
    _type_id: TypeId,
    def: &StructDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    Ok(format!(
        "export type {}{} = {};",
        struct_type_name(def),
        render_generic_params(def.generics.iter().map(|param| param.name.as_str())),
        render_object_fields(&def.fields, registry, options, imports)?
    ))
}

fn render_enum(
    _type_id: TypeId,
    def: &EnumDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    match &def.repr {
        EnumRepr::External => render_external_enum(def, registry, options, imports),
        EnumRepr::Internal { tag } => {
            render_tagged_enum(def, tag, None, registry, options, imports)
        }
        EnumRepr::Adjacent { tag, content } => {
            render_tagged_enum(def, tag, Some(content.as_str()), registry, options, imports)
        }
        EnumRepr::Untagged => render_untagged_enum(def, registry, options, imports),
    }
}

fn render_untagged_enum(
    def: &EnumDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let variants = def
        .variants
        .iter()
        .map(|variant| render_untagged_variant(def, variant, registry, options, imports))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "export type {} = {};",
        enum_type_name(def),
        render_union(variants)
    ))
}

fn render_untagged_variant(
    def: &EnumDef,
    variant: &VariantDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let rendered: Result<String, String> = match &variant.shape {
        VariantShape::Unit => {
            Err("untagged unit variants are unsupported for JSON DTO output".to_owned())
        }
        VariantShape::Newtype(ty) => render_type_ref(ty, None, registry, options, imports),
        VariantShape::Tuple(items) => {
            let rendered = items
                .iter()
                .map(|item| render_type_ref(item, None, registry, options, imports))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", rendered.join(", ")))
        }
        VariantShape::Struct(fields) => render_object_fields(fields, registry, options, imports),
    };

    rendered.map_err(|error| {
        format!(
            "{error} while rendering untagged enum {}.{}",
            enum_type_name(def),
            variant.rust_name
        )
    })
}

fn render_external_enum(
    def: &EnumDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let variants = def
        .variants
        .iter()
        .map(|variant| render_external_variant(def, variant, registry, options, imports))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "export type {} = {};",
        enum_type_name(def),
        render_union(variants)
    ))
}

fn render_external_variant(
    def: &EnumDef,
    variant: &VariantDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let rendered: Result<String, String> = match &variant.shape {
        VariantShape::Unit => Ok(quote_string(&variant.wire_name)),
        VariantShape::Newtype(ty) => Ok(format!(
            "{{ {}: {}, }}",
            render_property_name(&variant.wire_name),
            render_type_ref(ty, None, registry, options, imports)?
        )),
        VariantShape::Tuple(items) => {
            let rendered = items
                .iter()
                .map(|item| render_type_ref(item, None, registry, options, imports))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!(
                "{{ {}: [{}], }}",
                render_property_name(&variant.wire_name),
                rendered.join(", ")
            ))
        }
        VariantShape::Struct(fields) => Ok(format!(
            "{{ {}: {}, }}",
            render_property_name(&variant.wire_name),
            render_object_fields(fields, registry, options, imports)?
        )),
    };

    rendered.map_err(|error| {
        format!(
            "{error} while rendering external enum {}.{}",
            enum_type_name(def),
            variant.rust_name
        )
    })
}

fn render_tagged_enum(
    def: &EnumDef,
    tag: &str,
    content: Option<&str>,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let variants = def
        .variants
        .iter()
        .map(|variant| {
            render_tagged_variant(def, variant, tag, content, registry, options, imports)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "export type {} = {};",
        enum_type_name(def),
        render_union(variants)
    ))
}

fn render_tagged_variant(
    def: &EnumDef,
    variant: &VariantDef,
    tag: &str,
    content: Option<&str>,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let mut fields = vec![format!(
        "{}: {}",
        render_property_name(tag),
        quote_string(&variant.wire_name)
    )];

    match (&variant.shape, content) {
        (VariantShape::Unit, _) => {}
        (VariantShape::Struct(variant_fields), Some(content)) => {
            fields.push(format!(
                "{}: {}",
                render_property_name(content),
                render_object_fields(variant_fields, registry, options, imports)?
            ));
        }
        (VariantShape::Struct(variant_fields), None) => {
            fields.extend(render_object_field_list(
                variant_fields,
                registry,
                options,
                imports,
            )?);
        }
        (VariantShape::Newtype(ty), Some(content)) => {
            fields.push(format!(
                "{}: {}",
                render_property_name(content),
                render_type_ref(ty, None, registry, options, imports)?
            ));
        }
        (VariantShape::Tuple(items), Some(content)) => {
            let rendered = items
                .iter()
                .map(|item| render_type_ref(item, None, registry, options, imports))
                .collect::<Result<Vec<_>, _>>()?;
            fields.push(format!(
                "{}: [{}]",
                render_property_name(content),
                rendered.join(", ")
            ));
        }
        (VariantShape::Newtype(_) | VariantShape::Tuple(_), None) => {
            return Err(format!(
                "unsupported internally tagged variant shape for {}.{}",
                enum_type_name(def),
                variant.rust_name
            ));
        }
    }

    Ok(format!("{{ {}, }}", fields.join(", ")))
}

fn render_object_fields(
    fields: &[FieldDef],
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let rendered = render_object_field_list(fields, registry, options, imports)?;
    if rendered.is_empty() {
        return Ok("{}".to_owned());
    }
    Ok(format!("{{ {}, }}", rendered.join(", ")))
}

fn render_object_field_list(
    fields: &[FieldDef],
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, String> {
    fields
        .iter()
        .filter(|field| field.presence.is_serialized())
        .map(|field| render_object_field(field, registry, options, imports))
        .collect()
}

fn render_object_field(
    field: &FieldDef,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    let optional = if field.presence.required_on_deserialize {
        ""
    } else {
        "?"
    };
    let mut value = render_type_ref(&field.ty, field.int_repr, registry, options, imports)
        .map_err(|error| {
            format!(
                "{error} while rendering field {} at {}",
                field.target.typescript, field.source
            )
        })?;
    if field.presence.nullable {
        value = render_nullable(value);
    }
    Ok(format!(
        "{}{}: {}",
        render_property_name(&field.target.typescript),
        optional,
        value
    ))
}

fn render_type_ref(
    ty: &TypeRef,
    int_repr: Option<IntRepr>,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    match ty {
        TypeRef::Primitive(primitive) => render_primitive(*primitive, int_repr, options),
        TypeRef::String => Ok("string".to_owned()),
        TypeRef::Bytes(_) => Ok("Uint8Array".to_owned()),
        TypeRef::Option(inner) => Ok(render_nullable(render_type_ref(
            inner, int_repr, registry, options, imports,
        )?)),
        TypeRef::Vec(inner) => Ok(format!(
            "Array<{}>",
            render_type_ref(inner, int_repr, registry, options, imports)?
        )),
        TypeRef::Array { item, len } => {
            let item = render_type_ref(item, int_repr, registry, options, imports)?;
            Ok(format!("[{}]", vec![item; *len].join(", ")))
        }
        TypeRef::Map { key, value } => {
            if !matches!(key.as_ref(), TypeRef::String) {
                return Err("non-string map keys are unsupported".to_owned());
            }
            Ok(format!(
                "Record<string, {}>",
                render_type_ref(value, int_repr, registry, options, imports)?
            ))
        }
        TypeRef::Named(type_id) => render_named_type(*type_id, registry, options, imports),
        TypeRef::GenericParam(name) => Ok(name.clone()),
        TypeRef::Override(target) if target.backend == BackendId::TypeScript => {
            if let Some(import) = options.external_overrides.get(&target.target_type) {
                imports
                    .entry(import.from.clone())
                    .or_default()
                    .insert(import.import_name.clone());
                return Ok(import.import_name.clone());
            }
            Ok(target.target_type.clone())
        }
        TypeRef::Override(_) => Err("target override is for a different backend".to_owned()),
    }
}

fn render_named_type(
    type_id: TypeId,
    registry: &Registry,
    options: &DtoRegistryRenderOptions,
    imports: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<String, String> {
    if let Some(import) = options.external_imports.get(&type_id) {
        imports
            .entry(import.from.clone())
            .or_default()
            .insert(import.import_name.clone());
        return Ok(import.import_name.clone());
    }

    let type_def = registry
        .type_def(type_id)
        .ok_or_else(|| format!("missing named type reference {type_id}"))?;
    Ok(type_name(type_def).to_owned())
}

fn render_primitive(
    primitive: Primitive,
    int_repr: Option<IntRepr>,
    options: &DtoRegistryRenderOptions,
) -> Result<String, String> {
    if primitive.requires_explicit_integer_policy() {
        return match int_repr {
            Some(IntRepr::JsonString) => Ok("string".to_owned()),
            Some(IntRepr::JsonNumberUnsafe) => Ok("number".to_owned()),
            Some(IntRepr::NonJsonBigint) => Ok("bigint".to_owned()),
            None => match options.config.numeric.large_int_policy {
                LargeIntPolicy::RequireExplicit => {
                    Err("large integer field requires explicit numeric policy".to_owned())
                }
                LargeIntPolicy::JsonString => Ok("string".to_owned()),
                LargeIntPolicy::JsonNumberUnsafe => Ok("number".to_owned()),
                LargeIntPolicy::NonJsonBigint => Ok("bigint".to_owned()),
            },
        };
    }

    match primitive {
        Primitive::Bool => Ok("boolean".to_owned()),
        primitive if primitive.is_integer() || primitive.is_float() => Ok("number".to_owned()),
        _ => unreachable!("all primitive variants are covered by bool, integer, or float"),
    }
}

fn render_imports(imports: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut rendered = String::new();
    for (from, names) in imports {
        if names.len() == 1 {
            rendered.push_str("import type { ");
            rendered.push_str(names.iter().next().expect("single import name"));
            rendered.push_str(" } from ");
            rendered.push_str(&quote_string(from));
            rendered.push_str(";\n");
        } else {
            rendered.push_str("import type {\n");
            for name in names {
                rendered.push_str("    ");
                rendered.push_str(name);
                rendered.push_str(",\n");
            }
            rendered.push_str("} from ");
            rendered.push_str(&quote_string(from));
            rendered.push_str(";\n");
        }
    }
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

fn render_nullable(value: String) -> String {
    if value.split(" | ").any(|part| part == "null") {
        value
    } else {
        format!("{value} | null")
    }
}

fn render_union(items: Vec<String>) -> String {
    if items.is_empty() {
        return "never".to_owned();
    }

    let mut seen = BTreeSet::new();
    let mut rendered = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            rendered.push(item);
        }
    }
    rendered.join(" | ")
}

fn render_generic_params<'a>(params: impl Iterator<Item = &'a str>) -> String {
    let params = params.collect::<Vec<_>>();
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn type_name(type_def: &TypeDef) -> &str {
    match type_def {
        TypeDef::Struct(def) => struct_type_name(def),
        TypeDef::Enum(def) => enum_type_name(def),
    }
}

fn struct_type_name(def: &StructDef) -> &str {
    def.attrs.ts_name.as_deref().unwrap_or(&def.export_name)
}

fn enum_type_name(def: &EnumDef) -> &str {
    def.attrs.ts_name.as_deref().unwrap_or(&def.export_name)
}

fn render_property_name(value: &str) -> String {
    if is_identifier(value) {
        value.to_owned()
    } else {
        quote_string(value)
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first == '_' || first == '$' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn quote_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::{DtoRegistryRenderOptions, render_registry_types};
    use dto_bindgen_core::{
        BackendId, EnumDef, EnumRepr, FieldDef, FieldPresence, GenericParam, IdentName, IntRepr,
        Primitive, Registry, RustTypeId, SourceSpan, StructDef, TargetFieldNames, TargetOverride,
        TypeDef, TypeRef, VariantDef, VariantShape, WireFieldNames,
    };

    fn span() -> SourceSpan {
        SourceSpan::new("src/dto.rs", 1, 1)
    }

    fn field(name: &str, target: &str, ty: TypeRef) -> FieldDef {
        FieldDef::new(
            IdentName::new(name),
            WireFieldNames::same(target),
            TargetFieldNames::new(target, name),
            ty,
            span(),
        )
    }

    #[test]
    fn renders_synthetic_registry_as_package_level_types() {
        let mut registry = Registry::new();
        let external_id = registry.register_type(
            RustTypeId::new("external", "ExternalThing"),
            TypeDef::Struct(StructDef::new("ExternalThing", "ExternalThing", span())),
        );
        let envelope = TypeDef::Struct(StructDef {
            generics: vec![GenericParam::new("T")],
            ..StructDef::new("Envelope", "Envelope", span()).with_field(field(
                "value",
                "value",
                TypeRef::GenericParam("T".to_owned()),
            ))
        });
        registry.register_type(RustTypeId::new("sdk", "Envelope"), envelope);
        let thing = TypeDef::Struct(
            StructDef::new("SyntheticThing", "SyntheticThing", span())
                .with_field(field("external", "external", TypeRef::Named(external_id)))
                .with_field(
                    field(
                        "maybe_count",
                        "maybeCount",
                        TypeRef::Primitive(Primitive::I64),
                    )
                    .with_presence(FieldPresence::optional_nullable())
                    .with_int_repr(IntRepr::JsonString),
                )
                .with_field(field(
                    "point",
                    "point",
                    TypeRef::array(TypeRef::Primitive(Primitive::F64), 2),
                ))
                .with_field(field(
                    "envelope",
                    "envelope",
                    TypeRef::Override(TargetOverride::new(
                        BackendId::TypeScript,
                        "Envelope<ExternalThing>",
                    )),
                )),
        );
        registry.register_type(RustTypeId::new("sdk", "SyntheticThing"), thing);
        let mode = TypeDef::Enum(
            EnumDef::new("SyntheticMode", "SyntheticMode", EnumRepr::External, span())
                .with_variant(VariantDef::new(
                    "Ready",
                    "ready",
                    VariantShape::Unit,
                    span(),
                ))
                .with_variant(VariantDef::new("Done", "done", VariantShape::Unit, span())),
        );
        registry.register_type(RustTypeId::new("sdk", "SyntheticMode"), mode);
        let event = TypeDef::Enum(
            EnumDef::new(
                "SyntheticEvent",
                "SyntheticEvent",
                EnumRepr::Adjacent {
                    tag: "type".to_owned(),
                    content: "payload".to_owned(),
                },
                span(),
            )
            .with_variant(VariantDef::new(
                "Created",
                "created",
                VariantShape::Struct(vec![field("id", "id", TypeRef::String)]),
                span(),
            ))
            .with_variant(VariantDef::new(
                "Archived",
                "archived",
                VariantShape::Struct(vec![
                    field("reason", "reason", TypeRef::option(TypeRef::String))
                        .with_presence(FieldPresence::optional_nullable_skip_if_none()),
                ]),
                span(),
            )),
        );
        registry.register_type(RustTypeId::new("sdk", "SyntheticEvent"), event);

        let rendered = render_registry_types(
            &registry,
            &DtoRegistryRenderOptions::default().with_external_type(
                external_id,
                "ExternalThing",
                "@radroots/external-bindings",
            ),
        )
        .expect("registry renders");

        assert_eq!(
            rendered.imports_ts(),
            Some("import type { ExternalThing } from \"@radroots/external-bindings\";\n\n")
        );
        assert_eq!(
            rendered.body_ts(),
            "export type Envelope<T> = { value: T, };\n\nexport type SyntheticEvent = { type: \"created\", payload: { id: string, }, } | { type: \"archived\", payload: { reason?: string | null, }, };\n\nexport type SyntheticMode = \"ready\" | \"done\";\n\nexport type SyntheticThing = { external: ExternalThing, maybeCount?: string | null, point: [number, number], envelope: Envelope<ExternalThing>, };"
        );
    }

    #[test]
    fn imports_typescript_overrides_when_configured() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "SyntheticThing"),
            TypeDef::Struct(
                StructDef::new("SyntheticThing", "SyntheticThing", span()).with_field(field(
                    "external",
                    "external",
                    TypeRef::Override(TargetOverride::new(BackendId::TypeScript, "ExternalAlias")),
                )),
            ),
        );

        let rendered = render_registry_types(
            &registry,
            &DtoRegistryRenderOptions::default().with_external_override(
                "ExternalAlias",
                "ExternalAlias",
                "@radroots/external-bindings",
            ),
        )
        .expect("registry renders");

        assert_eq!(
            rendered.imports_ts(),
            Some("import type { ExternalAlias } from \"@radroots/external-bindings\";\n\n")
        );
        assert_eq!(
            rendered.body_ts(),
            "export type SyntheticThing = { external: ExternalAlias, };"
        );
    }

    #[test]
    fn renders_untagged_object_unions() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "Query"),
            TypeDef::Enum(
                EnumDef::new("Query", "Query", EnumRepr::Untagged, span())
                    .with_variant(VariantDef::new(
                        "ById",
                        "byId",
                        VariantShape::Struct(vec![field("id", "id", TypeRef::String)]),
                        span(),
                    ))
                    .with_variant(VariantDef::new(
                        "BySlug",
                        "bySlug",
                        VariantShape::Struct(vec![field("slug", "slug", TypeRef::String)]),
                        span(),
                    )),
            ),
        );

        let rendered = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect("registry renders");

        assert_eq!(
            rendered.body_ts(),
            "export type Query = { id: string, } | { slug: string, };"
        );
    }

    #[test]
    fn renders_untagged_newtype_aliases() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "FindOneResolve"),
            TypeDef::Enum(
                EnumDef::new(
                    "FindOneResolve",
                    "FindOneResolve",
                    EnumRepr::Untagged,
                    span(),
                )
                .with_variant(VariantDef::new(
                    "Alias",
                    "alias",
                    VariantShape::Newtype(TypeRef::Override(TargetOverride::new(
                        BackendId::TypeScript,
                        "IResult<Farm | null>",
                    ))),
                    span(),
                )),
            ),
        );

        let rendered = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect("registry renders");

        assert_eq!(
            rendered.body_ts(),
            "export type FindOneResolve = IResult<Farm | null>;"
        );
    }

    #[test]
    fn rejects_untagged_unit_variants() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "MaybeReady"),
            TypeDef::Enum(
                EnumDef::new("MaybeReady", "MaybeReady", EnumRepr::Untagged, span()).with_variant(
                    VariantDef::new("Ready", "ready", VariantShape::Unit, span()),
                ),
            ),
        );

        let error = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect_err("untagged unit variant blocks render");

        assert_eq!(
            error,
            "untagged unit variants are unsupported for JSON DTO output while rendering untagged enum MaybeReady.Ready"
        );
    }

    #[test]
    fn requires_explicit_large_integer_policy() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "Counter"),
            TypeDef::Struct(
                StructDef::new("Counter", "Counter", span()).with_field(field(
                    "value",
                    "value",
                    TypeRef::Primitive(Primitive::U64),
                )),
            ),
        );

        let error = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect_err("missing policy blocks render");

        assert_eq!(
            error,
            "large integer field requires explicit numeric policy while rendering field value at src/dto.rs:1:1"
        );
    }

    #[test]
    fn propagates_integer_policy_through_transparent_containers_only() {
        let mut registry = Registry::new();
        let counter_id = registry.register_type(
            RustTypeId::new("sdk", "Counter"),
            TypeDef::Struct(
                StructDef::new("Counter", "Counter", span()).with_field(
                    field("value", "value", TypeRef::Primitive(Primitive::U64))
                        .with_int_repr(IntRepr::JsonNumberUnsafe),
                ),
            ),
        );
        registry.register_type(
            RustTypeId::new("sdk", "TransparentCounters"),
            TypeDef::Struct(
                StructDef::new("TransparentCounters", "TransparentCounters", span())
                    .with_field(
                        field(
                            "maybe_count",
                            "maybeCount",
                            TypeRef::option(TypeRef::Primitive(Primitive::U64)),
                        )
                        .with_presence(FieldPresence::optional_nullable())
                        .with_int_repr(IntRepr::JsonString),
                    )
                    .with_field(
                        field(
                            "count_list",
                            "countList",
                            TypeRef::vec(TypeRef::Primitive(Primitive::U64)),
                        )
                        .with_int_repr(IntRepr::JsonString),
                    )
                    .with_field(
                        field(
                            "fixed_counts",
                            "fixedCounts",
                            TypeRef::array(TypeRef::Primitive(Primitive::U64), 2),
                        )
                        .with_int_repr(IntRepr::JsonString),
                    )
                    .with_field(
                        field(
                            "by_key",
                            "byKey",
                            TypeRef::Map {
                                key: Box::new(TypeRef::String),
                                value: Box::new(TypeRef::Primitive(Primitive::U64)),
                            },
                        )
                        .with_int_repr(IntRepr::JsonString),
                    )
                    .with_field(
                        field("named_counter", "namedCounter", TypeRef::Named(counter_id))
                            .with_int_repr(IntRepr::JsonString),
                    ),
            ),
        );

        let rendered = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect("registry renders");

        assert_eq!(
            rendered.body_ts(),
            "export type Counter = { value: number, };\n\nexport type TransparentCounters = { maybeCount?: string | null, countList: Array<string>, fixedCounts: [string, string], byKey: Record<string, string>, namedCounter: Counter, };"
        );
    }

    #[test]
    fn renders_external_data_enums() {
        let mut registry = Registry::new();
        registry.register_type(
            RustTypeId::new("sdk", "ParseError"),
            TypeDef::Enum(
                EnumDef::new("ParseError", "ParseError", EnumRepr::External, span())
                    .with_variant(VariantDef::new(
                        "InvalidKind",
                        "InvalidKind",
                        VariantShape::Newtype(TypeRef::Primitive(Primitive::U32)),
                        span(),
                    ))
                    .with_variant(VariantDef::new(
                        "InvalidUnit",
                        "InvalidUnit",
                        VariantShape::Unit,
                        span(),
                    )),
            ),
        );

        let rendered = render_registry_types(&registry, &DtoRegistryRenderOptions::default())
            .expect("registry renders");

        assert_eq!(
            rendered.body_ts(),
            "export type ParseError = { InvalidKind: number, } | \"InvalidUnit\";"
        );
    }
}
