pub use radroots_replica_schema as upstream;

pub fn dto_registry() -> dto_bindgen_core::Registry {
    upstream::dto::dto_registry()
}

#[cfg(test)]
mod tests {
    use super::dto_registry;

    #[test]
    fn preserves_replica_schema_registry_exports() {
        let registry = dto_registry();
        let actual = registry
            .types_by_id
            .values()
            .map(|type_def| match type_def {
                dto_bindgen_core::TypeDef::Struct(def) => def.export_name.as_str(),
                dto_bindgen_core::TypeDef::Enum(def) => def.export_name.as_str(),
            })
            .collect::<Vec<_>>();

        assert!(actual.contains(&"Farm"));
        assert!(actual.contains(&"GcsLocation"));
        assert!(actual.contains(&"IGcsLocationFindMany"));
        assert!(actual.contains(&"IGcsLocationFindManyResolve"));
        assert!(actual.contains(&"IMediaImageFindMany"));
        assert!(actual.contains(&"INostrProfileFindMany"));
        assert!(actual.contains(&"INostrRelayFindMany"));
    }
}
