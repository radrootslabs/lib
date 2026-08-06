pub use radroots_identity as upstream;

use dto_bindgen_backend_ts::{TypeScriptDeclaration, TypeScriptModule, TypeScriptValue};
use radroots_identity::username::{MAX_LENGTH, MIN_LENGTH};

pub fn constants_module() -> TypeScriptModule {
    TypeScriptModule::new("src/generated/constants.ts")
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_USERNAME_MIN_LENGTH",
            None,
            usize_value(MIN_LENGTH),
        ))
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_USERNAME_MAX_LENGTH",
            None,
            usize_value(MAX_LENGTH),
        ))
}

fn usize_value(value: usize) -> TypeScriptValue {
    TypeScriptValue::number(i64::try_from(value).expect("TypeScript constant fits in i64"))
}

#[cfg(test)]
mod tests {
    use super::{MAX_LENGTH, MIN_LENGTH, constants_module};

    #[test]
    fn preserves_username_constant_exports() {
        let rendered = constants_module().render_source();
        assert!(rendered.contains("RADROOTS_USERNAME_MIN_LENGTH"));
        assert!(rendered.contains(&MIN_LENGTH.to_string()));
        assert!(rendered.contains("RADROOTS_USERNAME_MAX_LENGTH"));
        assert!(rendered.contains(&MAX_LENGTH.to_string()));
        assert!(!rendered.contains("REGEX"));
    }
}
