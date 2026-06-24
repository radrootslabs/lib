use dto_bindgen_core::{
    BackendId, DescribeCtx, Dto, FieldDef, IdentName, RootDescriptor, RustTypeId, SourceSpan,
    StructDef, TargetFieldNames, TargetOverride, TypeDef, TypeRef, WireFieldNames,
};

use crate::{
    checkpoint::{RadrootsEventsIndexedIndexCheckpoint, RadrootsEventsIndexedShardCheckpoint},
    manifest::{RadrootsEventsIndexedManifest, RadrootsEventsIndexedShardMetadata},
    types::RadrootsEventsIndexedIdRange,
};

pub fn dto_roots() -> [RootDescriptor; 5] {
    [
        RootDescriptor::new::<RadrootsEventsIndexedIdRange>(),
        RootDescriptor::new::<RadrootsEventsIndexedShardMetadata>(),
        RootDescriptor::new::<RadrootsEventsIndexedManifest>(),
        RootDescriptor::new::<RadrootsEventsIndexedShardCheckpoint>(),
        RootDescriptor::new::<RadrootsEventsIndexedIndexCheckpoint>(),
    ]
}

impl Dto for RadrootsEventsIndexedShardCheckpoint {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsEventsIndexedShardCheckpoint",
            "RadrootsEventsIndexedShardCheckpoint",
            span("crates/events_indexed/src/checkpoint.rs", 10),
        )
        .with_field(field(
            "shard_id",
            "shard_id",
            shard_id_ref(),
            "crates/events_indexed/src/checkpoint.rs",
            11,
        ))
        .with_field(field(
            "last_created_at",
            "last_created_at",
            u32::describe(ctx),
            "crates/events_indexed/src/checkpoint.rs",
            16,
        ))
        .with_field(field(
            "last_event_id",
            "last_event_id",
            <Option<String> as Dto>::describe(ctx),
            "crates/events_indexed/src/checkpoint.rs",
            17,
        ))
        .with_field(field(
            "cursor",
            "cursor",
            <Option<String> as Dto>::describe(ctx),
            "crates/events_indexed/src/checkpoint.rs",
            18,
        ));
        register(
            ctx,
            "RadrootsEventsIndexedShardCheckpoint",
            TypeDef::Struct(def),
        )
    }
}

impl Dto for RadrootsEventsIndexedIndexCheckpoint {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsEventsIndexedIndexCheckpoint",
            "RadrootsEventsIndexedIndexCheckpoint",
            span("crates/events_indexed/src/checkpoint.rs", 24),
        )
        .with_field(field(
            "generated_at",
            "generated_at",
            u32::describe(ctx),
            "crates/events_indexed/src/checkpoint.rs",
            30,
        ))
        .with_field(field(
            "shards",
            "shards",
            <Vec<RadrootsEventsIndexedShardCheckpoint> as Dto>::describe(ctx),
            "crates/events_indexed/src/checkpoint.rs",
            31,
        ));
        register(
            ctx,
            "RadrootsEventsIndexedIndexCheckpoint",
            TypeDef::Struct(def),
        )
    }
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(
        RustTypeId::new("radroots_events_indexed", rust_ident),
        type_def,
    )
}

fn shard_id_ref() -> TypeRef {
    TypeRef::Override(TargetOverride::new(
        BackendId::TypeScript,
        "RadrootsEventsIndexedShardId",
    ))
}

fn field(rust_name: &str, wire_name: &str, ty: TypeRef, file: &str, line: u32) -> FieldDef {
    FieldDef::new(
        IdentName::new(rust_name),
        WireFieldNames::same(wire_name),
        TargetFieldNames::new(wire_name, rust_name),
        ty,
        span(file, line),
    )
}

fn span(file: &str, line: u32) -> SourceSpan {
    SourceSpan::new(file, line, 1)
}

#[cfg(test)]
mod tests {
    use dto_bindgen_core::{Primitive, TypeDef, TypeRef, build_registry};

    use super::dto_roots;

    #[test]
    fn indexed_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), dto_roots().len());
        assert!(registry.types_by_id.values().any(
            |def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsEventsIndexedManifest")
        ));
    }

    #[test]
    fn custom_epoch_second_fields_render_as_numbers() {
        let registry = build_registry(dto_roots());
        let checkpoint = registry
            .types_by_id
            .values()
            .find_map(|def| match def {
                TypeDef::Struct(def)
                    if def.export_name == "RadrootsEventsIndexedShardCheckpoint" =>
                {
                    Some(def)
                }
                _ => None,
            })
            .expect("checkpoint descriptor");
        let field = checkpoint
            .fields
            .iter()
            .find(|field| field.target.typescript == "last_created_at")
            .expect("last_created_at field");

        assert_eq!(field.ty, TypeRef::Primitive(Primitive::U32));
    }
}
