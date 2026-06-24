use dto_bindgen_core::{
    BackendId, DefaultKind, EnumDef, EnumRepr, FieldDef, FieldPresence, IdentName, Registry,
    RustTypeId, SerializePresence, SourceSpan, StructDef, TargetFieldNames, TargetOverride,
    TypeDef, TypeRef, VariantDef, VariantShape, WireFieldNames,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeSpec {
    Object {
        name: &'static str,
        fields: &'static [FieldSpec],
    },
    Union {
        name: &'static str,
        variants: &'static [VariantSpec],
    },
    Alias {
        name: &'static str,
        target: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VariantSpec {
    Object(&'static [FieldSpec]),
    Ref(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FieldSpec {
    name: &'static str,
    target: &'static str,
    optional: bool,
    nullable: bool,
}

impl FieldSpec {
    const fn required(name: &'static str, target: &'static str) -> Self {
        Self {
            name,
            target,
            optional: false,
            nullable: false,
        }
    }

    const fn nullable(name: &'static str, target: &'static str) -> Self {
        Self {
            name,
            target,
            optional: false,
            nullable: true,
        }
    }

    const fn optional(name: &'static str, target: &'static str) -> Self {
        Self {
            name,
            target,
            optional: true,
            nullable: false,
        }
    }

    const fn optional_nullable(name: &'static str, target: &'static str) -> Self {
        Self {
            name,
            target,
            optional: true,
            nullable: true,
        }
    }
}

pub fn dto_registry() -> Registry {
    let mut registry = Registry::new();
    for spec in TYPE_SPECS {
        let name = spec.name();
        let type_id = registry.register_type(
            RustTypeId::new(env!("CARGO_PKG_NAME"), name),
            spec.type_def(),
        );
        registry.mark_root(type_id);
    }
    registry
}

pub fn type_inventory() -> Vec<&'static str> {
    TYPE_SPECS.iter().map(TypeSpec::name).collect()
}

impl TypeSpec {
    fn name(&self) -> &'static str {
        match self {
            Self::Object { name, .. } | Self::Union { name, .. } | Self::Alias { name, .. } => name,
        }
    }

    fn type_def(&self) -> TypeDef {
        match self {
            Self::Object { name, fields } => TypeDef::Struct(object_def(name, fields)),
            Self::Union { name, variants } => TypeDef::Enum(union_def(name, variants)),
            Self::Alias { name, target } => TypeDef::Enum(alias_def(name, target)),
        }
    }
}

fn object_def(name: &str, fields: &[FieldSpec]) -> StructDef {
    let mut def = StructDef::new(name, name, source_span());
    for field in fields {
        def = def.with_field(field_def(field));
    }
    def
}

fn union_def(name: &str, variants: &[VariantSpec]) -> EnumDef {
    let mut def = EnumDef::new(name, name, EnumRepr::Untagged, source_span());
    for (index, variant) in variants.iter().enumerate() {
        def = def.with_variant(match variant {
            VariantSpec::Object(fields) => VariantDef::new(
                format!("Variant{index}"),
                format!("variant{index}"),
                VariantShape::Struct(fields.iter().map(field_def).collect()),
                source_span(),
            ),
            VariantSpec::Ref(target) => VariantDef::new(
                format!("Variant{index}"),
                format!("variant{index}"),
                VariantShape::Newtype(ts_ref(target)),
                source_span(),
            ),
        });
    }
    def
}

fn alias_def(name: &str, target: &str) -> EnumDef {
    EnumDef::new(name, name, EnumRepr::Untagged, source_span()).with_variant(VariantDef::new(
        "Alias",
        "alias",
        VariantShape::Newtype(ts_ref(target)),
        source_span(),
    ))
}

fn field_def(field: &FieldSpec) -> FieldDef {
    FieldDef::new(
        IdentName::new(field.name),
        WireFieldNames::same(field.name),
        TargetFieldNames::new(field.name, field.name),
        ts_ref(field.target),
        source_span(),
    )
    .with_presence(field_presence(field.optional, field.nullable))
}

fn field_presence(optional: bool, nullable: bool) -> FieldPresence {
    match (optional, nullable) {
        (false, false) => FieldPresence::required(),
        (false, true) => FieldPresence::nullable_required(),
        (true, true) => FieldPresence::optional_nullable(),
        (true, false) => FieldPresence {
            nullable: false,
            required_on_deserialize: false,
            default: Some(DefaultKind::NoneValue),
            serialize_presence: SerializePresence::Always,
        },
    }
}

fn ts_ref(target: &str) -> TypeRef {
    TypeRef::Override(TargetOverride::new(BackendId::TypeScript, target))
}

fn source_span() -> SourceSpan {
    SourceSpan::new(file!(), line!(), column!())
}

const TYPE_SPECS: &[TypeSpec] = &[
    TypeSpec::Object {
        name: "Farm",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("pubkey", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::nullable("about", "string"),
            FieldSpec::nullable("website", "string"),
            FieldSpec::nullable("picture", "string"),
            FieldSpec::nullable("banner", "string"),
            FieldSpec::nullable("location_primary", "string"),
            FieldSpec::nullable("location_city", "string"),
            FieldSpec::nullable("location_region", "string"),
            FieldSpec::nullable("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "FarmGcsLocation",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("gcs_location_id", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Union {
        name: "FarmGcsLocationQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("farm_id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("gcs_location_id", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "FarmMember",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("member_pubkey", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "FarmMemberClaim",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("member_pubkey", "string"),
            FieldSpec::required("farm_pubkey", "string"),
        ],
    },
    TypeSpec::Union {
        name: "FarmMemberClaimQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("member_pubkey", "string")]),
            VariantSpec::Object(&[FieldSpec::required("farm_pubkey", "string")]),
        ],
    },
    TypeSpec::Union {
        name: "FarmMemberQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("farm_id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("member_pubkey", "string")]),
        ],
    },
    TypeSpec::Union {
        name: "FarmQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("d_tag", "string")]),
            VariantSpec::Object(&[FieldSpec::required("pubkey", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "FarmTag",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("tag", "string"),
        ],
    },
    TypeSpec::Union {
        name: "FarmTagQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("farm_id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("tag", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "GcsLocation",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("lat", "number"),
            FieldSpec::required("lng", "number"),
            FieldSpec::required("geohash", "string"),
            FieldSpec::required("point", "string"),
            FieldSpec::required("polygon", "string"),
            FieldSpec::nullable("accuracy", "number"),
            FieldSpec::nullable("altitude", "number"),
            FieldSpec::nullable("tag_0", "string"),
            FieldSpec::nullable("label", "string"),
            FieldSpec::nullable("area", "number"),
            FieldSpec::nullable("elevation", "number"),
            FieldSpec::nullable("soil", "string"),
            FieldSpec::nullable("climate", "string"),
            FieldSpec::nullable("gc_id", "string"),
            FieldSpec::nullable("gc_name", "string"),
            FieldSpec::nullable("gc_admin1_id", "string"),
            FieldSpec::nullable("gc_admin1_name", "string"),
            FieldSpec::nullable("gc_country_id", "string"),
            FieldSpec::nullable("gc_country_name", "string"),
        ],
    },
    TypeSpec::Object {
        name: "GcsLocationFarmArgs",
        fields: &[FieldSpec::required("id", "string")],
    },
    TypeSpec::Union {
        name: "GcsLocationFindManyRel",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required(
                "on_trade_product",
                "GcsLocationTradeProductArgs",
            )]),
            VariantSpec::Object(&[FieldSpec::required(
                "off_trade_product",
                "GcsLocationTradeProductArgs",
            )]),
            VariantSpec::Object(&[FieldSpec::required("on_farm", "GcsLocationFarmArgs")]),
            VariantSpec::Object(&[FieldSpec::required("off_farm", "GcsLocationFarmArgs")]),
            VariantSpec::Object(&[FieldSpec::required("on_plot", "GcsLocationPlotArgs")]),
            VariantSpec::Object(&[FieldSpec::required("off_plot", "GcsLocationPlotArgs")]),
        ],
    },
    TypeSpec::Object {
        name: "GcsLocationPlotArgs",
        fields: &[FieldSpec::required("id", "string")],
    },
    TypeSpec::Union {
        name: "GcsLocationQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("d_tag", "string")]),
            VariantSpec::Object(&[FieldSpec::required("geohash", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "GcsLocationTradeProductArgs",
        fields: &[FieldSpec::required("id", "string")],
    },
    TypeSpec::Alias {
        name: "IFarmCreate",
        target: "IFarmFields",
    },
    TypeSpec::Alias {
        name: "IFarmCreateResolve",
        target: "IResult<Farm>",
    },
    TypeSpec::Alias {
        name: "IFarmDelete",
        target: "IFarmFindOne",
    },
    TypeSpec::Alias {
        name: "IFarmDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IFarmFields",
        fields: &[
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("pubkey", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::optional_nullable("about", "string"),
            FieldSpec::optional_nullable("website", "string"),
            FieldSpec::optional_nullable("picture", "string"),
            FieldSpec::optional_nullable("banner", "string"),
            FieldSpec::optional_nullable("location_primary", "string"),
            FieldSpec::optional_nullable("location_city", "string"),
            FieldSpec::optional_nullable("location_region", "string"),
            FieldSpec::optional_nullable("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("d_tag", "string"),
            FieldSpec::optional("pubkey", "string"),
            FieldSpec::optional("name", "string"),
            FieldSpec::optional("about", "string"),
            FieldSpec::optional("website", "string"),
            FieldSpec::optional("picture", "string"),
            FieldSpec::optional("banner", "string"),
            FieldSpec::optional("location_primary", "string"),
            FieldSpec::optional("location_city", "string"),
            FieldSpec::optional("location_region", "string"),
            FieldSpec::optional("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("d_tag", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("about", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("website", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("picture", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("banner", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_primary", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_city", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_region", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_country", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmFindMany",
        target: "IFarmFindManyArgs",
    },
    TypeSpec::Object {
        name: "IFarmFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "IFarmFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "IFarmFindManyResolve",
        target: "IResultList<Farm>",
    },
    TypeSpec::Union {
        name: "IFarmFindOne",
        variants: &[VariantSpec::Ref("IFarmFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IFarmFindOneArgs",
        fields: &[FieldSpec::required("on", "FarmQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IFarmFindOneResolve",
        target: "IResult<Farm | null>",
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationCreate",
        target: "IFarmGcsLocationFields",
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationCreateResolve",
        target: "IResult<FarmGcsLocation>",
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationDelete",
        target: "IFarmGcsLocationFindOne",
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationFields",
        fields: &[
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("gcs_location_id", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("farm_id", "string"),
            FieldSpec::optional("gcs_location_id", "string"),
            FieldSpec::optional("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("farm_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gcs_location_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("role", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationFindMany",
        target: "IFarmGcsLocationFindManyArgs",
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationFindManyArgs",
        fields: &[FieldSpec::nullable(
            "filter",
            "IFarmGcsLocationFieldsFilter",
        )],
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationFindManyResolve",
        target: "IResultList<FarmGcsLocation>",
    },
    TypeSpec::Union {
        name: "IFarmGcsLocationFindOne",
        variants: &[VariantSpec::Ref("IFarmGcsLocationFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationFindOneArgs",
        fields: &[FieldSpec::required("on", "FarmGcsLocationQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationFindOneResolve",
        target: "IResult<FarmGcsLocation | null>",
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationUpdate",
        target: "IFarmGcsLocationUpdateArgs",
    },
    TypeSpec::Object {
        name: "IFarmGcsLocationUpdateArgs",
        fields: &[
            FieldSpec::required("on", "FarmGcsLocationQueryBindValues"),
            FieldSpec::required("fields", "IFarmGcsLocationFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmGcsLocationUpdateResolve",
        target: "IResult<FarmGcsLocation>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimCreate",
        target: "IFarmMemberClaimFields",
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimCreateResolve",
        target: "IResult<FarmMemberClaim>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimDelete",
        target: "IFarmMemberClaimFindOne",
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimFields",
        fields: &[
            FieldSpec::required("member_pubkey", "string"),
            FieldSpec::required("farm_pubkey", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("member_pubkey", "string"),
            FieldSpec::optional("farm_pubkey", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("member_pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("farm_pubkey", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimFindMany",
        target: "IFarmMemberClaimFindManyArgs",
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimFindManyArgs",
        fields: &[FieldSpec::nullable(
            "filter",
            "IFarmMemberClaimFieldsFilter",
        )],
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimFindManyResolve",
        target: "IResultList<FarmMemberClaim>",
    },
    TypeSpec::Union {
        name: "IFarmMemberClaimFindOne",
        variants: &[VariantSpec::Ref("IFarmMemberClaimFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimFindOneArgs",
        fields: &[FieldSpec::required("on", "FarmMemberClaimQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimFindOneResolve",
        target: "IResult<FarmMemberClaim | null>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimUpdate",
        target: "IFarmMemberClaimUpdateArgs",
    },
    TypeSpec::Object {
        name: "IFarmMemberClaimUpdateArgs",
        fields: &[
            FieldSpec::required("on", "FarmMemberClaimQueryBindValues"),
            FieldSpec::required("fields", "IFarmMemberClaimFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmMemberClaimUpdateResolve",
        target: "IResult<FarmMemberClaim>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberCreate",
        target: "IFarmMemberFields",
    },
    TypeSpec::Alias {
        name: "IFarmMemberCreateResolve",
        target: "IResult<FarmMember>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberDelete",
        target: "IFarmMemberFindOne",
    },
    TypeSpec::Alias {
        name: "IFarmMemberDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IFarmMemberFields",
        fields: &[
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("member_pubkey", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmMemberFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("farm_id", "string"),
            FieldSpec::optional("member_pubkey", "string"),
            FieldSpec::optional("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmMemberFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("farm_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("member_pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("role", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmMemberFindMany",
        target: "IFarmMemberFindManyArgs",
    },
    TypeSpec::Object {
        name: "IFarmMemberFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "IFarmMemberFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "IFarmMemberFindManyResolve",
        target: "IResultList<FarmMember>",
    },
    TypeSpec::Union {
        name: "IFarmMemberFindOne",
        variants: &[VariantSpec::Ref("IFarmMemberFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IFarmMemberFindOneArgs",
        fields: &[FieldSpec::required("on", "FarmMemberQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IFarmMemberFindOneResolve",
        target: "IResult<FarmMember | null>",
    },
    TypeSpec::Alias {
        name: "IFarmMemberUpdate",
        target: "IFarmMemberUpdateArgs",
    },
    TypeSpec::Object {
        name: "IFarmMemberUpdateArgs",
        fields: &[
            FieldSpec::required("on", "FarmMemberQueryBindValues"),
            FieldSpec::required("fields", "IFarmMemberFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmMemberUpdateResolve",
        target: "IResult<FarmMember>",
    },
    TypeSpec::Alias {
        name: "IFarmTagCreate",
        target: "IFarmTagFields",
    },
    TypeSpec::Alias {
        name: "IFarmTagCreateResolve",
        target: "IResult<FarmTag>",
    },
    TypeSpec::Alias {
        name: "IFarmTagDelete",
        target: "IFarmTagFindOne",
    },
    TypeSpec::Alias {
        name: "IFarmTagDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IFarmTagFields",
        fields: &[
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("tag", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmTagFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("farm_id", "string"),
            FieldSpec::optional("tag", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IFarmTagFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("farm_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("tag", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmTagFindMany",
        target: "IFarmTagFindManyArgs",
    },
    TypeSpec::Object {
        name: "IFarmTagFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "IFarmTagFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "IFarmTagFindManyResolve",
        target: "IResultList<FarmTag>",
    },
    TypeSpec::Union {
        name: "IFarmTagFindOne",
        variants: &[VariantSpec::Ref("IFarmTagFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IFarmTagFindOneArgs",
        fields: &[FieldSpec::required("on", "FarmTagQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IFarmTagFindOneResolve",
        target: "IResult<FarmTag | null>",
    },
    TypeSpec::Alias {
        name: "IFarmTagUpdate",
        target: "IFarmTagUpdateArgs",
    },
    TypeSpec::Object {
        name: "IFarmTagUpdateArgs",
        fields: &[
            FieldSpec::required("on", "FarmTagQueryBindValues"),
            FieldSpec::required("fields", "IFarmTagFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmTagUpdateResolve",
        target: "IResult<FarmTag>",
    },
    TypeSpec::Alias {
        name: "IFarmUpdate",
        target: "IFarmUpdateArgs",
    },
    TypeSpec::Object {
        name: "IFarmUpdateArgs",
        fields: &[
            FieldSpec::required("on", "FarmQueryBindValues"),
            FieldSpec::required("fields", "IFarmFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IFarmUpdateResolve",
        target: "IResult<Farm>",
    },
    TypeSpec::Alias {
        name: "IGcsLocationCreate",
        target: "IGcsLocationFields",
    },
    TypeSpec::Alias {
        name: "IGcsLocationCreateResolve",
        target: "IResult<GcsLocation>",
    },
    TypeSpec::Alias {
        name: "IGcsLocationDelete",
        target: "IGcsLocationFindOne",
    },
    TypeSpec::Alias {
        name: "IGcsLocationDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IGcsLocationFields",
        fields: &[
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("lat", "number"),
            FieldSpec::required("lng", "number"),
            FieldSpec::required("geohash", "string"),
            FieldSpec::required("point", "string"),
            FieldSpec::required("polygon", "string"),
            FieldSpec::optional_nullable("accuracy", "number"),
            FieldSpec::optional_nullable("altitude", "number"),
            FieldSpec::optional_nullable("tag_0", "string"),
            FieldSpec::optional_nullable("label", "string"),
            FieldSpec::optional_nullable("area", "number"),
            FieldSpec::optional_nullable("elevation", "number"),
            FieldSpec::optional_nullable("soil", "string"),
            FieldSpec::optional_nullable("climate", "string"),
            FieldSpec::optional_nullable("gc_id", "string"),
            FieldSpec::optional_nullable("gc_name", "string"),
            FieldSpec::optional_nullable("gc_admin1_id", "string"),
            FieldSpec::optional_nullable("gc_admin1_name", "string"),
            FieldSpec::optional_nullable("gc_country_id", "string"),
            FieldSpec::optional_nullable("gc_country_name", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IGcsLocationFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("d_tag", "string"),
            FieldSpec::optional("lat", "number"),
            FieldSpec::optional("lng", "number"),
            FieldSpec::optional("geohash", "string"),
            FieldSpec::optional("point", "string"),
            FieldSpec::optional("polygon", "string"),
            FieldSpec::optional("accuracy", "number"),
            FieldSpec::optional("altitude", "number"),
            FieldSpec::optional("tag_0", "string"),
            FieldSpec::optional("label", "string"),
            FieldSpec::optional("area", "number"),
            FieldSpec::optional("elevation", "number"),
            FieldSpec::optional("soil", "string"),
            FieldSpec::optional("climate", "string"),
            FieldSpec::optional("gc_id", "string"),
            FieldSpec::optional("gc_name", "string"),
            FieldSpec::optional("gc_admin1_id", "string"),
            FieldSpec::optional("gc_admin1_name", "string"),
            FieldSpec::optional("gc_country_id", "string"),
            FieldSpec::optional("gc_country_name", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IGcsLocationFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("d_tag", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("lat", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("lng", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("geohash", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("point", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("polygon", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("accuracy", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("altitude", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("tag_0", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("label", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("area", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("elevation", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("soil", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("climate", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_admin1_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_admin1_name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_country_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gc_country_name", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Union {
        name: "IGcsLocationFindMany",
        variants: &[
            VariantSpec::Object(&[FieldSpec::nullable("filter", "IGcsLocationFieldsFilter")]),
            VariantSpec::Object(&[FieldSpec::required("rel", "GcsLocationFindManyRel")]),
        ],
    },
    TypeSpec::Alias {
        name: "IGcsLocationFindManyResolve",
        target: "IResultList<GcsLocation>",
    },
    TypeSpec::Union {
        name: "IGcsLocationFindOne",
        variants: &[
            VariantSpec::Ref("IGcsLocationFindOneArgs"),
            VariantSpec::Ref("IGcsLocationFindOneRelArgs"),
        ],
    },
    TypeSpec::Object {
        name: "IGcsLocationFindOneArgs",
        fields: &[FieldSpec::required("on", "GcsLocationQueryBindValues")],
    },
    TypeSpec::Object {
        name: "IGcsLocationFindOneRelArgs",
        fields: &[FieldSpec::required("rel", "GcsLocationFindManyRel")],
    },
    TypeSpec::Alias {
        name: "IGcsLocationFindOneResolve",
        target: "IResult<GcsLocation | null>",
    },
    TypeSpec::Alias {
        name: "IGcsLocationUpdate",
        target: "IGcsLocationUpdateArgs",
    },
    TypeSpec::Object {
        name: "IGcsLocationUpdateArgs",
        fields: &[
            FieldSpec::required("on", "GcsLocationQueryBindValues"),
            FieldSpec::required("fields", "IGcsLocationFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IGcsLocationUpdateResolve",
        target: "IResult<GcsLocation>",
    },
    TypeSpec::Alias {
        name: "ILogErrorCreate",
        target: "ILogErrorFields",
    },
    TypeSpec::Alias {
        name: "ILogErrorCreateResolve",
        target: "IResult<LogError>",
    },
    TypeSpec::Alias {
        name: "ILogErrorDelete",
        target: "ILogErrorFindOne",
    },
    TypeSpec::Alias {
        name: "ILogErrorDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "ILogErrorFields",
        fields: &[
            FieldSpec::required("error", "string"),
            FieldSpec::required("message", "string"),
            FieldSpec::optional_nullable("stack_trace", "string"),
            FieldSpec::optional_nullable("cause", "string"),
            FieldSpec::required("app_system", "string"),
            FieldSpec::required("app_version", "string"),
            FieldSpec::required("nostr_pubkey", "string"),
            FieldSpec::optional_nullable("data", "string"),
        ],
    },
    TypeSpec::Object {
        name: "ILogErrorFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("error", "string"),
            FieldSpec::optional("message", "string"),
            FieldSpec::optional("stack_trace", "string"),
            FieldSpec::optional("cause", "string"),
            FieldSpec::optional("app_system", "string"),
            FieldSpec::optional("app_version", "string"),
            FieldSpec::optional("nostr_pubkey", "string"),
            FieldSpec::optional("data", "string"),
        ],
    },
    TypeSpec::Object {
        name: "ILogErrorFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("error", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("message", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("stack_trace", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("cause", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("app_system", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("app_version", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("nostr_pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("data", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "ILogErrorFindMany",
        target: "ILogErrorFindManyArgs",
    },
    TypeSpec::Object {
        name: "ILogErrorFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "ILogErrorFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "ILogErrorFindManyResolve",
        target: "IResultList<LogError>",
    },
    TypeSpec::Union {
        name: "ILogErrorFindOne",
        variants: &[VariantSpec::Ref("ILogErrorFindOneArgs")],
    },
    TypeSpec::Object {
        name: "ILogErrorFindOneArgs",
        fields: &[FieldSpec::required("on", "LogErrorQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "ILogErrorFindOneResolve",
        target: "IResult<LogError | null>",
    },
    TypeSpec::Alias {
        name: "ILogErrorUpdate",
        target: "ILogErrorUpdateArgs",
    },
    TypeSpec::Object {
        name: "ILogErrorUpdateArgs",
        fields: &[
            FieldSpec::required("on", "LogErrorQueryBindValues"),
            FieldSpec::required("fields", "ILogErrorFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "ILogErrorUpdateResolve",
        target: "IResult<LogError>",
    },
    TypeSpec::Alias {
        name: "IMediaImageCreate",
        target: "IMediaImageFields",
    },
    TypeSpec::Alias {
        name: "IMediaImageCreateResolve",
        target: "IResult<MediaImage>",
    },
    TypeSpec::Alias {
        name: "IMediaImageDelete",
        target: "IMediaImageFindOne",
    },
    TypeSpec::Alias {
        name: "IMediaImageDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IMediaImageFields",
        fields: &[
            FieldSpec::required("file_path", "string"),
            FieldSpec::required("mime_type", "string"),
            FieldSpec::required("res_base", "string"),
            FieldSpec::required("res_path", "string"),
            FieldSpec::optional_nullable("label", "string"),
            FieldSpec::optional_nullable("description", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IMediaImageFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("file_path", "string"),
            FieldSpec::optional("mime_type", "string"),
            FieldSpec::optional("res_base", "string"),
            FieldSpec::optional("res_path", "string"),
            FieldSpec::optional("label", "string"),
            FieldSpec::optional("description", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IMediaImageFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("file_path", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("mime_type", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("res_base", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("res_path", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("label", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("description", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Union {
        name: "IMediaImageFindMany",
        variants: &[
            VariantSpec::Object(&[FieldSpec::nullable("filter", "IMediaImageFieldsFilter")]),
            VariantSpec::Object(&[FieldSpec::required("rel", "MediaImageFindManyRel")]),
        ],
    },
    TypeSpec::Alias {
        name: "IMediaImageFindManyResolve",
        target: "IResultList<MediaImage>",
    },
    TypeSpec::Union {
        name: "IMediaImageFindOne",
        variants: &[
            VariantSpec::Ref("IMediaImageFindOneArgs"),
            VariantSpec::Ref("IMediaImageFindOneRelArgs"),
        ],
    },
    TypeSpec::Object {
        name: "IMediaImageFindOneArgs",
        fields: &[FieldSpec::required("on", "MediaImageQueryBindValues")],
    },
    TypeSpec::Object {
        name: "IMediaImageFindOneRelArgs",
        fields: &[FieldSpec::required("rel", "MediaImageFindManyRel")],
    },
    TypeSpec::Alias {
        name: "IMediaImageFindOneResolve",
        target: "IResult<MediaImage | null>",
    },
    TypeSpec::Alias {
        name: "IMediaImageUpdate",
        target: "IMediaImageUpdateArgs",
    },
    TypeSpec::Object {
        name: "IMediaImageUpdateArgs",
        fields: &[
            FieldSpec::required("on", "MediaImageQueryBindValues"),
            FieldSpec::required("fields", "IMediaImageFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IMediaImageUpdateResolve",
        target: "IResult<MediaImage>",
    },
    TypeSpec::Alias {
        name: "INostrEventHeadCreate",
        target: "INostrEventHeadFields",
    },
    TypeSpec::Alias {
        name: "INostrEventHeadCreateResolve",
        target: "IResult<NostrEventHead>",
    },
    TypeSpec::Alias {
        name: "INostrEventHeadDelete",
        target: "INostrEventHeadFindOne",
    },
    TypeSpec::Alias {
        name: "INostrEventHeadDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "INostrEventHeadFields",
        fields: &[
            FieldSpec::required("key", "string"),
            FieldSpec::required("kind", "number"),
            FieldSpec::required("pubkey", "string"),
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("last_event_id", "string"),
            FieldSpec::required("last_created_at", "number"),
            FieldSpec::required("content_hash", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrEventHeadFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("key", "string"),
            FieldSpec::optional("kind", "number"),
            FieldSpec::optional("pubkey", "string"),
            FieldSpec::optional("d_tag", "string"),
            FieldSpec::optional("last_event_id", "string"),
            FieldSpec::optional("last_created_at", "number"),
            FieldSpec::optional("content_hash", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrEventHeadFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("key", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("kind", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("d_tag", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("last_event_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("last_created_at", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("content_hash", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "INostrEventHeadFindMany",
        target: "INostrEventHeadFindManyArgs",
    },
    TypeSpec::Object {
        name: "INostrEventHeadFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "INostrEventHeadFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "INostrEventHeadFindManyResolve",
        target: "IResultList<NostrEventHead>",
    },
    TypeSpec::Union {
        name: "INostrEventHeadFindOne",
        variants: &[VariantSpec::Ref("INostrEventHeadFindOneArgs")],
    },
    TypeSpec::Object {
        name: "INostrEventHeadFindOneArgs",
        fields: &[FieldSpec::required("on", "NostrEventHeadQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "INostrEventHeadFindOneResolve",
        target: "IResult<NostrEventHead | null>",
    },
    TypeSpec::Alias {
        name: "INostrEventHeadUpdate",
        target: "INostrEventHeadUpdateArgs",
    },
    TypeSpec::Object {
        name: "INostrEventHeadUpdateArgs",
        fields: &[
            FieldSpec::required("on", "NostrEventHeadQueryBindValues"),
            FieldSpec::required("fields", "INostrEventHeadFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "INostrEventHeadUpdateResolve",
        target: "IResult<NostrEventHead>",
    },
    TypeSpec::Alias {
        name: "INostrProfileCreate",
        target: "INostrProfileFields",
    },
    TypeSpec::Alias {
        name: "INostrProfileCreateResolve",
        target: "IResult<NostrProfile>",
    },
    TypeSpec::Alias {
        name: "INostrProfileDelete",
        target: "INostrProfileFindOne",
    },
    TypeSpec::Alias {
        name: "INostrProfileDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "INostrProfileFields",
        fields: &[
            FieldSpec::required("public_key", "string"),
            FieldSpec::required("profile_type", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::optional_nullable("display_name", "string"),
            FieldSpec::optional_nullable("about", "string"),
            FieldSpec::optional_nullable("website", "string"),
            FieldSpec::optional_nullable("picture", "string"),
            FieldSpec::optional_nullable("banner", "string"),
            FieldSpec::optional_nullable("nip05", "string"),
            FieldSpec::optional_nullable("lud06", "string"),
            FieldSpec::optional_nullable("lud16", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrProfileFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("public_key", "string"),
            FieldSpec::optional("profile_type", "string"),
            FieldSpec::optional("name", "string"),
            FieldSpec::optional("display_name", "string"),
            FieldSpec::optional("about", "string"),
            FieldSpec::optional("website", "string"),
            FieldSpec::optional("picture", "string"),
            FieldSpec::optional("banner", "string"),
            FieldSpec::optional("nip05", "string"),
            FieldSpec::optional("lud06", "string"),
            FieldSpec::optional("lud16", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrProfileFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("public_key", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("profile_type", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("display_name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("about", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("website", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("picture", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("banner", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("nip05", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("lud06", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("lud16", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Union {
        name: "INostrProfileFindMany",
        variants: &[
            VariantSpec::Object(&[FieldSpec::nullable("filter", "INostrProfileFieldsFilter")]),
            VariantSpec::Object(&[FieldSpec::required("rel", "NostrProfileFindManyRel")]),
        ],
    },
    TypeSpec::Alias {
        name: "INostrProfileFindManyResolve",
        target: "IResultList<NostrProfile>",
    },
    TypeSpec::Union {
        name: "INostrProfileFindOne",
        variants: &[
            VariantSpec::Ref("INostrProfileFindOneArgs"),
            VariantSpec::Ref("INostrProfileFindOneRelArgs"),
        ],
    },
    TypeSpec::Object {
        name: "INostrProfileFindOneArgs",
        fields: &[FieldSpec::required("on", "NostrProfileQueryBindValues")],
    },
    TypeSpec::Object {
        name: "INostrProfileFindOneRelArgs",
        fields: &[FieldSpec::required("rel", "NostrProfileFindManyRel")],
    },
    TypeSpec::Alias {
        name: "INostrProfileFindOneResolve",
        target: "IResult<NostrProfile | null>",
    },
    TypeSpec::Object {
        name: "INostrProfileRelayRelation",
        fields: &[
            FieldSpec::required("nostr_profile", "NostrProfileQueryBindValues"),
            FieldSpec::required("nostr_relay", "NostrRelayQueryBindValues"),
        ],
    },
    TypeSpec::Alias {
        name: "INostrProfileRelayResolve",
        target: "IResultPass",
    },
    TypeSpec::Alias {
        name: "INostrProfileUpdate",
        target: "INostrProfileUpdateArgs",
    },
    TypeSpec::Object {
        name: "INostrProfileUpdateArgs",
        fields: &[
            FieldSpec::required("on", "NostrProfileQueryBindValues"),
            FieldSpec::required("fields", "INostrProfileFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "INostrProfileUpdateResolve",
        target: "IResult<NostrProfile>",
    },
    TypeSpec::Alias {
        name: "INostrRelayCreate",
        target: "INostrRelayFields",
    },
    TypeSpec::Alias {
        name: "INostrRelayCreateResolve",
        target: "IResult<NostrRelay>",
    },
    TypeSpec::Alias {
        name: "INostrRelayDelete",
        target: "INostrRelayFindOne",
    },
    TypeSpec::Alias {
        name: "INostrRelayDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "INostrRelayFields",
        fields: &[
            FieldSpec::required("url", "string"),
            FieldSpec::optional_nullable("relay_id", "string"),
            FieldSpec::optional_nullable("name", "string"),
            FieldSpec::optional_nullable("description", "string"),
            FieldSpec::optional_nullable("pubkey", "string"),
            FieldSpec::optional_nullable("contact", "string"),
            FieldSpec::optional_nullable("supported_nips", "string"),
            FieldSpec::optional_nullable("software", "string"),
            FieldSpec::optional_nullable("version", "string"),
            FieldSpec::optional_nullable("data", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrRelayFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("url", "string"),
            FieldSpec::optional("relay_id", "string"),
            FieldSpec::optional("name", "string"),
            FieldSpec::optional("description", "string"),
            FieldSpec::optional("pubkey", "string"),
            FieldSpec::optional("contact", "string"),
            FieldSpec::optional("supported_nips", "string"),
            FieldSpec::optional("software", "string"),
            FieldSpec::optional("version", "string"),
            FieldSpec::optional("data", "string"),
        ],
    },
    TypeSpec::Object {
        name: "INostrRelayFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("url", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("relay_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("description", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("pubkey", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("contact", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("supported_nips", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("software", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("version", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("data", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Union {
        name: "INostrRelayFindMany",
        variants: &[
            VariantSpec::Object(&[FieldSpec::nullable("filter", "INostrRelayFieldsFilter")]),
            VariantSpec::Object(&[FieldSpec::required("rel", "NostrRelayFindManyRel")]),
        ],
    },
    TypeSpec::Alias {
        name: "INostrRelayFindManyResolve",
        target: "IResultList<NostrRelay>",
    },
    TypeSpec::Union {
        name: "INostrRelayFindOne",
        variants: &[
            VariantSpec::Ref("INostrRelayFindOneArgs"),
            VariantSpec::Ref("INostrRelayFindOneRelArgs"),
        ],
    },
    TypeSpec::Object {
        name: "INostrRelayFindOneArgs",
        fields: &[FieldSpec::required("on", "NostrRelayQueryBindValues")],
    },
    TypeSpec::Object {
        name: "INostrRelayFindOneRelArgs",
        fields: &[FieldSpec::required("rel", "NostrRelayFindManyRel")],
    },
    TypeSpec::Alias {
        name: "INostrRelayFindOneResolve",
        target: "IResult<NostrRelay | null>",
    },
    TypeSpec::Alias {
        name: "INostrRelayUpdate",
        target: "INostrRelayUpdateArgs",
    },
    TypeSpec::Object {
        name: "INostrRelayUpdateArgs",
        fields: &[
            FieldSpec::required("on", "NostrRelayQueryBindValues"),
            FieldSpec::required("fields", "INostrRelayFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "INostrRelayUpdateResolve",
        target: "IResult<NostrRelay>",
    },
    TypeSpec::Alias {
        name: "IPlotCreate",
        target: "IPlotFields",
    },
    TypeSpec::Alias {
        name: "IPlotCreateResolve",
        target: "IResult<Plot>",
    },
    TypeSpec::Alias {
        name: "IPlotDelete",
        target: "IPlotFindOne",
    },
    TypeSpec::Alias {
        name: "IPlotDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IPlotFields",
        fields: &[
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::optional_nullable("about", "string"),
            FieldSpec::optional_nullable("location_primary", "string"),
            FieldSpec::optional_nullable("location_city", "string"),
            FieldSpec::optional_nullable("location_region", "string"),
            FieldSpec::optional_nullable("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("d_tag", "string"),
            FieldSpec::optional("farm_id", "string"),
            FieldSpec::optional("name", "string"),
            FieldSpec::optional("about", "string"),
            FieldSpec::optional("location_primary", "string"),
            FieldSpec::optional("location_city", "string"),
            FieldSpec::optional("location_region", "string"),
            FieldSpec::optional("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("d_tag", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("farm_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("name", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("about", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_primary", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_city", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_region", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("location_country", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotFindMany",
        target: "IPlotFindManyArgs",
    },
    TypeSpec::Object {
        name: "IPlotFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "IPlotFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "IPlotFindManyResolve",
        target: "IResultList<Plot>",
    },
    TypeSpec::Union {
        name: "IPlotFindOne",
        variants: &[VariantSpec::Ref("IPlotFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IPlotFindOneArgs",
        fields: &[FieldSpec::required("on", "PlotQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IPlotFindOneResolve",
        target: "IResult<Plot | null>",
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationCreate",
        target: "IPlotGcsLocationFields",
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationCreateResolve",
        target: "IResult<PlotGcsLocation>",
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationDelete",
        target: "IPlotGcsLocationFindOne",
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationFields",
        fields: &[
            FieldSpec::required("plot_id", "string"),
            FieldSpec::required("gcs_location_id", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("plot_id", "string"),
            FieldSpec::optional("gcs_location_id", "string"),
            FieldSpec::optional("role", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("plot_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("gcs_location_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("role", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationFindMany",
        target: "IPlotGcsLocationFindManyArgs",
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationFindManyArgs",
        fields: &[FieldSpec::nullable(
            "filter",
            "IPlotGcsLocationFieldsFilter",
        )],
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationFindManyResolve",
        target: "IResultList<PlotGcsLocation>",
    },
    TypeSpec::Union {
        name: "IPlotGcsLocationFindOne",
        variants: &[VariantSpec::Ref("IPlotGcsLocationFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationFindOneArgs",
        fields: &[FieldSpec::required("on", "PlotGcsLocationQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationFindOneResolve",
        target: "IResult<PlotGcsLocation | null>",
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationUpdate",
        target: "IPlotGcsLocationUpdateArgs",
    },
    TypeSpec::Object {
        name: "IPlotGcsLocationUpdateArgs",
        fields: &[
            FieldSpec::required("on", "PlotGcsLocationQueryBindValues"),
            FieldSpec::required("fields", "IPlotGcsLocationFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotGcsLocationUpdateResolve",
        target: "IResult<PlotGcsLocation>",
    },
    TypeSpec::Alias {
        name: "IPlotTagCreate",
        target: "IPlotTagFields",
    },
    TypeSpec::Alias {
        name: "IPlotTagCreateResolve",
        target: "IResult<PlotTag>",
    },
    TypeSpec::Alias {
        name: "IPlotTagDelete",
        target: "IPlotTagFindOne",
    },
    TypeSpec::Alias {
        name: "IPlotTagDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "IPlotTagFields",
        fields: &[
            FieldSpec::required("plot_id", "string"),
            FieldSpec::required("tag", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotTagFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("plot_id", "string"),
            FieldSpec::optional("tag", "string"),
        ],
    },
    TypeSpec::Object {
        name: "IPlotTagFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("plot_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("tag", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotTagFindMany",
        target: "IPlotTagFindManyArgs",
    },
    TypeSpec::Object {
        name: "IPlotTagFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "IPlotTagFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "IPlotTagFindManyResolve",
        target: "IResultList<PlotTag>",
    },
    TypeSpec::Union {
        name: "IPlotTagFindOne",
        variants: &[VariantSpec::Ref("IPlotTagFindOneArgs")],
    },
    TypeSpec::Object {
        name: "IPlotTagFindOneArgs",
        fields: &[FieldSpec::required("on", "PlotTagQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "IPlotTagFindOneResolve",
        target: "IResult<PlotTag | null>",
    },
    TypeSpec::Alias {
        name: "IPlotTagUpdate",
        target: "IPlotTagUpdateArgs",
    },
    TypeSpec::Object {
        name: "IPlotTagUpdateArgs",
        fields: &[
            FieldSpec::required("on", "PlotTagQueryBindValues"),
            FieldSpec::required("fields", "IPlotTagFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotTagUpdateResolve",
        target: "IResult<PlotTag>",
    },
    TypeSpec::Alias {
        name: "IPlotUpdate",
        target: "IPlotUpdateArgs",
    },
    TypeSpec::Object {
        name: "IPlotUpdateArgs",
        fields: &[
            FieldSpec::required("on", "PlotQueryBindValues"),
            FieldSpec::required("fields", "IPlotFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "IPlotUpdateResolve",
        target: "IResult<Plot>",
    },
    TypeSpec::Alias {
        name: "ITradeProductCreate",
        target: "ITradeProductFields",
    },
    TypeSpec::Alias {
        name: "ITradeProductCreateResolve",
        target: "IResult<TradeProduct>",
    },
    TypeSpec::Alias {
        name: "ITradeProductDelete",
        target: "ITradeProductFindOne",
    },
    TypeSpec::Alias {
        name: "ITradeProductDeleteResolve",
        target: "IResult<string>",
    },
    TypeSpec::Object {
        name: "ITradeProductFields",
        fields: &[
            FieldSpec::required("key", "string"),
            FieldSpec::required("category", "string"),
            FieldSpec::required("title", "string"),
            FieldSpec::required("summary", "string"),
            FieldSpec::required("process", "string"),
            FieldSpec::required("lot", "string"),
            FieldSpec::required("profile", "string"),
            FieldSpec::required("year", "bigint"),
            FieldSpec::required("qty_amt", "number"),
            FieldSpec::required("qty_amt_exact", "string"),
            FieldSpec::required("qty_unit", "string"),
            FieldSpec::optional_nullable("qty_label", "string"),
            FieldSpec::optional_nullable("qty_avail", "bigint"),
            FieldSpec::required("price_amt", "number"),
            FieldSpec::required("price_amt_exact", "string"),
            FieldSpec::required("price_currency", "string"),
            FieldSpec::required("price_qty_amt", "number"),
            FieldSpec::required("price_qty_amt_exact", "string"),
            FieldSpec::required("price_qty_unit", "string"),
            FieldSpec::optional_nullable("listing_addr", "string"),
            FieldSpec::optional_nullable("primary_bin_id", "string"),
            FieldSpec::optional_nullable("verified_primary_bin_id", "string"),
            FieldSpec::optional_nullable("notes", "string"),
        ],
    },
    TypeSpec::Object {
        name: "ITradeProductFieldsFilter",
        fields: &[
            FieldSpec::optional("id", "string"),
            FieldSpec::optional("created_at", "string"),
            FieldSpec::optional("updated_at", "string"),
            FieldSpec::optional("key", "string"),
            FieldSpec::optional("category", "string"),
            FieldSpec::optional("title", "string"),
            FieldSpec::optional("summary", "string"),
            FieldSpec::optional("process", "string"),
            FieldSpec::optional("lot", "string"),
            FieldSpec::optional("profile", "string"),
            FieldSpec::optional("year", "bigint"),
            FieldSpec::optional("qty_amt", "number"),
            FieldSpec::optional("qty_amt_exact", "string"),
            FieldSpec::optional("qty_unit", "string"),
            FieldSpec::optional("qty_label", "string"),
            FieldSpec::optional("qty_avail", "bigint"),
            FieldSpec::optional("price_amt", "number"),
            FieldSpec::optional("price_amt_exact", "string"),
            FieldSpec::optional("price_currency", "string"),
            FieldSpec::optional("price_qty_amt", "number"),
            FieldSpec::optional("price_qty_amt_exact", "string"),
            FieldSpec::optional("price_qty_unit", "string"),
            FieldSpec::optional("listing_addr", "string"),
            FieldSpec::optional("primary_bin_id", "string"),
            FieldSpec::optional("verified_primary_bin_id", "string"),
            FieldSpec::optional("notes", "string"),
        ],
    },
    TypeSpec::Object {
        name: "ITradeProductFieldsPartial",
        fields: &[
            FieldSpec::optional_nullable("key", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("category", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("title", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("summary", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("process", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("lot", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("profile", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("year", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("qty_amt", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("qty_amt_exact", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("qty_unit", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("qty_label", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("qty_avail", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_amt", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_amt_exact", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_currency", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_qty_amt", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_qty_amt_exact", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("price_qty_unit", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("listing_addr", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("primary_bin_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("verified_primary_bin_id", "ReplicaDbJsonValue"),
            FieldSpec::optional_nullable("notes", "ReplicaDbJsonValue"),
        ],
    },
    TypeSpec::Alias {
        name: "ITradeProductFindMany",
        target: "ITradeProductFindManyArgs",
    },
    TypeSpec::Object {
        name: "ITradeProductFindManyArgs",
        fields: &[FieldSpec::nullable("filter", "ITradeProductFieldsFilter")],
    },
    TypeSpec::Alias {
        name: "ITradeProductFindManyResolve",
        target: "IResultList<TradeProduct>",
    },
    TypeSpec::Union {
        name: "ITradeProductFindOne",
        variants: &[VariantSpec::Ref("ITradeProductFindOneArgs")],
    },
    TypeSpec::Object {
        name: "ITradeProductFindOneArgs",
        fields: &[FieldSpec::required("on", "TradeProductQueryBindValues")],
    },
    TypeSpec::Alias {
        name: "ITradeProductFindOneResolve",
        target: "IResult<TradeProduct | null>",
    },
    TypeSpec::Object {
        name: "ITradeProductLocationRelation",
        fields: &[
            FieldSpec::required("trade_product", "TradeProductQueryBindValues"),
            FieldSpec::required("gcs_location", "GcsLocationQueryBindValues"),
        ],
    },
    TypeSpec::Alias {
        name: "ITradeProductLocationResolve",
        target: "IResultPass",
    },
    TypeSpec::Object {
        name: "ITradeProductMediaRelation",
        fields: &[
            FieldSpec::required("trade_product", "TradeProductQueryBindValues"),
            FieldSpec::required("media_image", "MediaImageQueryBindValues"),
        ],
    },
    TypeSpec::Alias {
        name: "ITradeProductMediaResolve",
        target: "IResultPass",
    },
    TypeSpec::Alias {
        name: "ITradeProductUpdate",
        target: "ITradeProductUpdateArgs",
    },
    TypeSpec::Object {
        name: "ITradeProductUpdateArgs",
        fields: &[
            FieldSpec::required("on", "TradeProductQueryBindValues"),
            FieldSpec::required("fields", "ITradeProductFieldsPartial"),
        ],
    },
    TypeSpec::Alias {
        name: "ITradeProductUpdateResolve",
        target: "IResult<TradeProduct>",
    },
    TypeSpec::Object {
        name: "LogError",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("error", "string"),
            FieldSpec::required("message", "string"),
            FieldSpec::nullable("stack_trace", "string"),
            FieldSpec::nullable("cause", "string"),
            FieldSpec::required("app_system", "string"),
            FieldSpec::required("app_version", "string"),
            FieldSpec::required("nostr_pubkey", "string"),
            FieldSpec::nullable("data", "string"),
        ],
    },
    TypeSpec::Union {
        name: "LogErrorQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("nostr_pubkey", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "MediaImage",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("file_path", "string"),
            FieldSpec::required("mime_type", "string"),
            FieldSpec::required("res_base", "string"),
            FieldSpec::required("res_path", "string"),
            FieldSpec::nullable("label", "string"),
            FieldSpec::nullable("description", "string"),
        ],
    },
    TypeSpec::Union {
        name: "MediaImageFindManyRel",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required(
                "on_trade_product",
                "MediaImageTradeProductArgs",
            )]),
            VariantSpec::Object(&[FieldSpec::required(
                "off_trade_product",
                "MediaImageTradeProductArgs",
            )]),
        ],
    },
    TypeSpec::Union {
        name: "MediaImageQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("file_path", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "MediaImageTradeProductArgs",
        fields: &[FieldSpec::required("id", "string")],
    },
    TypeSpec::Object {
        name: "NostrEventHead",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("key", "string"),
            FieldSpec::required("kind", "number"),
            FieldSpec::required("pubkey", "string"),
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("last_event_id", "string"),
            FieldSpec::required("last_created_at", "number"),
            FieldSpec::required("content_hash", "string"),
        ],
    },
    TypeSpec::Union {
        name: "NostrEventHeadQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("key", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "NostrProfile",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("public_key", "string"),
            FieldSpec::required("profile_type", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::nullable("display_name", "string"),
            FieldSpec::nullable("about", "string"),
            FieldSpec::nullable("website", "string"),
            FieldSpec::nullable("picture", "string"),
            FieldSpec::nullable("banner", "string"),
            FieldSpec::nullable("nip05", "string"),
            FieldSpec::nullable("lud06", "string"),
            FieldSpec::nullable("lud16", "string"),
        ],
    },
    TypeSpec::Union {
        name: "NostrProfileFindManyRel",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("on_relay", "NostrProfileRelayArgs")]),
            VariantSpec::Object(&[FieldSpec::required("off_relay", "NostrProfileRelayArgs")]),
        ],
    },
    TypeSpec::Union {
        name: "NostrProfileQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("public_key", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "NostrProfileRelayArgs",
        fields: &[FieldSpec::required("id", "string")],
    },
    TypeSpec::Object {
        name: "NostrRelay",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("url", "string"),
            FieldSpec::nullable("relay_id", "string"),
            FieldSpec::nullable("name", "string"),
            FieldSpec::nullable("description", "string"),
            FieldSpec::nullable("pubkey", "string"),
            FieldSpec::nullable("contact", "string"),
            FieldSpec::nullable("supported_nips", "string"),
            FieldSpec::nullable("software", "string"),
            FieldSpec::nullable("version", "string"),
            FieldSpec::nullable("data", "string"),
        ],
    },
    TypeSpec::Union {
        name: "NostrRelayFindManyRel",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("on_profile", "NostrRelayProfileArgs")]),
            VariantSpec::Object(&[FieldSpec::required("off_profile", "NostrRelayProfileArgs")]),
        ],
    },
    TypeSpec::Object {
        name: "NostrRelayProfileArgs",
        fields: &[FieldSpec::required("public_key", "string")],
    },
    TypeSpec::Union {
        name: "NostrRelayQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("url", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "Plot",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("d_tag", "string"),
            FieldSpec::required("farm_id", "string"),
            FieldSpec::required("name", "string"),
            FieldSpec::nullable("about", "string"),
            FieldSpec::nullable("location_primary", "string"),
            FieldSpec::nullable("location_city", "string"),
            FieldSpec::nullable("location_region", "string"),
            FieldSpec::nullable("location_country", "string"),
        ],
    },
    TypeSpec::Object {
        name: "PlotGcsLocation",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("plot_id", "string"),
            FieldSpec::required("gcs_location_id", "string"),
            FieldSpec::required("role", "string"),
        ],
    },
    TypeSpec::Union {
        name: "PlotGcsLocationQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("plot_id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("gcs_location_id", "string")]),
        ],
    },
    TypeSpec::Union {
        name: "PlotQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("d_tag", "string")]),
            VariantSpec::Object(&[FieldSpec::required("farm_id", "string")]),
        ],
    },
    TypeSpec::Object {
        name: "PlotTag",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("plot_id", "string"),
            FieldSpec::required("tag", "string"),
        ],
    },
    TypeSpec::Union {
        name: "PlotTagQueryBindValues",
        variants: &[
            VariantSpec::Object(&[FieldSpec::required("id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("plot_id", "string")]),
            VariantSpec::Object(&[FieldSpec::required("tag", "string")]),
        ],
    },
    TypeSpec::Alias {
        name: "ReplicaDbJsonValue",
        target: "null | boolean | number | string | Array<ReplicaDbJsonValue> | { [key: string]: ReplicaDbJsonValue }",
    },
    TypeSpec::Object {
        name: "TradeProduct",
        fields: &[
            FieldSpec::required("id", "string"),
            FieldSpec::required("created_at", "string"),
            FieldSpec::required("updated_at", "string"),
            FieldSpec::required("key", "string"),
            FieldSpec::required("category", "string"),
            FieldSpec::required("title", "string"),
            FieldSpec::required("summary", "string"),
            FieldSpec::required("process", "string"),
            FieldSpec::required("lot", "string"),
            FieldSpec::required("profile", "string"),
            FieldSpec::required("year", "bigint"),
            FieldSpec::required("qty_amt", "number"),
            FieldSpec::nullable("qty_amt_exact", "string"),
            FieldSpec::required("qty_unit", "string"),
            FieldSpec::nullable("qty_label", "string"),
            FieldSpec::nullable("qty_avail", "bigint"),
            FieldSpec::required("price_amt", "number"),
            FieldSpec::nullable("price_amt_exact", "string"),
            FieldSpec::required("price_currency", "string"),
            FieldSpec::required("price_qty_amt", "number"),
            FieldSpec::nullable("price_qty_amt_exact", "string"),
            FieldSpec::required("price_qty_unit", "string"),
            FieldSpec::nullable("listing_addr", "string"),
            FieldSpec::nullable("primary_bin_id", "string"),
            FieldSpec::nullable("verified_primary_bin_id", "string"),
            FieldSpec::nullable("notes", "string"),
        ],
    },
    TypeSpec::Union {
        name: "TradeProductQueryBindValues",
        variants: &[VariantSpec::Object(&[FieldSpec::required("id", "string")])],
    },
];

#[cfg(test)]
mod tests {
    use super::{FieldSpec, TYPE_SPECS, TypeSpec, VariantSpec, dto_registry, type_inventory};

    #[test]
    fn registry_exports_known_schema_types() {
        let registry = dto_registry();
        assert!(!registry.has_errors(), "{:?}", registry.diagnostics);
        assert!(type_inventory().contains(&"Farm"));
        assert!(type_inventory().contains(&"NostrEventHead"));
        assert!(type_inventory().contains(&"ReplicaDbJsonValue"));
    }

    #[test]
    fn source_find_one_resolves_preserve_nullable_result() {
        assert!(TYPE_SPECS.iter().any(|spec| matches!(
            spec,
            TypeSpec::Alias {
                name: "IFarmFindOneResolve",
                target: "IResult<Farm | null>"
            }
        )));
    }

    #[test]
    fn relation_find_many_inputs_preserve_filter_and_rel_variants() {
        for (name, filter, rel) in [
            (
                "IGcsLocationFindMany",
                "IGcsLocationFieldsFilter",
                "GcsLocationFindManyRel",
            ),
            (
                "IMediaImageFindMany",
                "IMediaImageFieldsFilter",
                "MediaImageFindManyRel",
            ),
            (
                "INostrProfileFindMany",
                "INostrProfileFieldsFilter",
                "NostrProfileFindManyRel",
            ),
            (
                "INostrRelayFindMany",
                "INostrRelayFieldsFilter",
                "NostrRelayFindManyRel",
            ),
        ] {
            assert!(TYPE_SPECS.iter().any(|spec| matches!(
                spec,
                TypeSpec::Union { name: actual_name, variants }
                    if *actual_name == name
                        && variants.len() == 2
                        && matches!(variants[0], VariantSpec::Object(fields) if fields.len() == 1 && fields[0] == FieldSpec::nullable("filter", filter))
                        && matches!(variants[1], VariantSpec::Object(fields) if fields.len() == 1 && fields[0] == FieldSpec::required("rel", rel))
            )));
        }
    }

    #[test]
    fn serde_json_value_policy_is_explicit() {
        assert!(TYPE_SPECS.iter().any(|spec| matches!(
            spec,
            TypeSpec::Alias { name: "ReplicaDbJsonValue", target } if target.contains("[key: string]: ReplicaDbJsonValue")
        )));
        assert!(TYPE_SPECS.iter().any(|spec| matches!(
            spec,
            TypeSpec::Object { name: "ITradeProductFieldsPartial", fields } if fields.iter().any(|field| field.name == "year" && field.target == "ReplicaDbJsonValue" && field.optional && field.nullable)
        )));
    }

    #[test]
    fn trade_product_large_integer_policy_is_explicit() {
        assert!(TYPE_SPECS.iter().any(|spec| matches!(
            spec,
            TypeSpec::Object { name: "TradeProduct", fields } if fields.iter().any(|field| field.name == "year" && field.target == "bigint")
        )));
        assert!(TYPE_SPECS.iter().any(|spec| matches!(
            spec,
            TypeSpec::Object { name: "ITradeProductFieldsFilter", fields } if fields.iter().any(|field| field.name == "qty_avail" && field.target == "bigint" && field.optional)
        )));
    }
}
