use dto_bindgen_core::{
    DescribeCtx, Dto, FieldDef, FieldPresence, IdentName, RootDescriptor, RustTypeId, SourceSpan,
    StructDef, TargetFieldNames, TypeDef, TypeRef, WireFieldNames,
};

use crate::{
    RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef,
    listing::{RadrootsListingImage, RadrootsListingImageSize, RadrootsListingProduct},
};

pub fn dto_roots() -> [RootDescriptor; 5] {
    [
        RootDescriptor::new::<RadrootsNostrEvent>(),
        RootDescriptor::new::<RadrootsNostrEventRef>(),
        RootDescriptor::new::<RadrootsNostrEventPtr>(),
        RootDescriptor::new::<RadrootsListingProduct>(),
        RootDescriptor::new::<RadrootsListingImage>(),
    ]
}

impl Dto for RadrootsNostrEvent {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEvent",
            "RadrootsNostrEvent",
            span("crates/events/src/lib.rs", 52),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            53,
        ))
        .with_field(field(
            "author",
            "author",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            54,
        ))
        .with_field(field(
            "created_at",
            "created_at",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            55,
        ))
        .with_field(field(
            "kind",
            "kind",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            56,
        ))
        .with_field(field(
            "tags",
            "tags",
            <Vec<Vec<String>> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            57,
        ))
        .with_field(field(
            "content",
            "content",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            58,
        ))
        .with_field(field(
            "sig",
            "sig",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            59,
        ));
        register(ctx, "RadrootsNostrEvent", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsNostrEventRef {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEventRef",
            "RadrootsNostrEventRef",
            span("crates/events/src/lib.rs", 64),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            65,
        ))
        .with_field(field(
            "author",
            "author",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            66,
        ))
        .with_field(field(
            "kind",
            "kind",
            u32::describe(ctx),
            "crates/events/src/lib.rs",
            67,
        ))
        .with_field(field(
            "d_tag",
            "d_tag",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            68,
        ))
        .with_field(field(
            "relays",
            "relays",
            <Option<Vec<String>> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            69,
        ));
        register(ctx, "RadrootsNostrEventRef", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsNostrEventPtr {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsNostrEventPtr",
            "RadrootsNostrEventPtr",
            span("crates/events/src/lib.rs", 74),
        )
        .with_field(field(
            "id",
            "id",
            String::describe(ctx),
            "crates/events/src/lib.rs",
            75,
        ))
        .with_field(field(
            "relays",
            "relays",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/lib.rs",
            76,
        ));
        register(ctx, "RadrootsNostrEventPtr", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingProduct {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingProduct",
            "RadrootsListingProduct",
            span("crates/events/src/listing.rs", 81),
        )
        .with_field(field(
            "key",
            "key",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            82,
        ))
        .with_field(field(
            "title",
            "title",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            83,
        ))
        .with_field(field(
            "category",
            "category",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            84,
        ))
        .with_field(nullable_field(
            "summary",
            "summary",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            85,
        ))
        .with_field(nullable_field(
            "process",
            "process",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            86,
        ))
        .with_field(nullable_field(
            "lot",
            "lot",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            87,
        ))
        .with_field(nullable_field(
            "location",
            "location",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            88,
        ))
        .with_field(nullable_field(
            "profile",
            "profile",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            89,
        ))
        .with_field(nullable_field(
            "year",
            "year",
            <Option<String> as Dto>::describe(ctx),
            "crates/events/src/listing.rs",
            90,
        ));
        register(ctx, "RadrootsListingProduct", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingImageSize {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let def = StructDef::new(
            "RadrootsListingImageSize",
            "RadrootsListingImageSize",
            span("crates/events/src/listing.rs", 133),
        )
        .with_field(field(
            "w",
            "w",
            u32::describe(ctx),
            "crates/events/src/listing.rs",
            134,
        ))
        .with_field(field(
            "h",
            "h",
            u32::describe(ctx),
            "crates/events/src/listing.rs",
            135,
        ));
        register(ctx, "RadrootsListingImageSize", TypeDef::Struct(def))
    }
}

impl Dto for RadrootsListingImage {
    fn describe(ctx: &mut DescribeCtx) -> TypeRef {
        let size = RadrootsListingImageSize::describe(ctx);
        let def = StructDef::new(
            "RadrootsListingImage",
            "RadrootsListingImage",
            span("crates/events/src/listing.rs", 126),
        )
        .with_field(field(
            "url",
            "url",
            String::describe(ctx),
            "crates/events/src/listing.rs",
            127,
        ))
        .with_field(nullable_field(
            "size",
            "size",
            TypeRef::option(size),
            "crates/events/src/listing.rs",
            128,
        ));
        register(ctx, "RadrootsListingImage", TypeDef::Struct(def))
    }
}

fn register(ctx: &mut DescribeCtx, rust_ident: &str, type_def: TypeDef) -> TypeRef {
    ctx.register_type(RustTypeId::new("radroots_events", rust_ident), type_def)
}

fn nullable_field(
    rust_name: &str,
    wire_name: &str,
    ty: TypeRef,
    file: &str,
    line: u32,
) -> FieldDef {
    field(rust_name, wire_name, ty, file, line).with_presence(FieldPresence::nullable_required())
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
    use dto_bindgen_core::{TypeDef, build_registry};

    use super::dto_roots;

    #[test]
    fn event_descriptor_roots_build_registry() {
        let registry = build_registry(dto_roots());

        assert!(!registry.has_errors());
        assert_eq!(registry.roots.len(), dto_roots().len());
        assert!(registry.types_by_id.values().any(
            |def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsNostrEvent")
        ));
        assert!(
            registry
                .types_by_id
                .values()
                .any(|def| matches!(def, TypeDef::Struct(def) if def.export_name == "RadrootsListingImageSize"))
        );
    }
}
