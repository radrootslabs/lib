pub use radroots_identity as upstream;

use dto_bindgen_backend_ts::{TypeScriptDeclaration, TypeScriptModule, TypeScriptValue};
use radroots_identity::{
    RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
};

pub fn constants_module() -> TypeScriptModule {
    TypeScriptModule::new("src/generated/constants.ts")
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_USERNAME_MIN_LEN",
            None,
            usize_value(RADROOTS_USERNAME_MIN_LEN),
        ))
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_USERNAME_MAX_LEN",
            None,
            usize_value(RADROOTS_USERNAME_MAX_LEN),
        ))
        .with_declaration(TypeScriptDeclaration::constant(
            "RADROOTS_USERNAME_REGEX",
            None,
            TypeScriptValue::string(RADROOTS_USERNAME_REGEX),
        ))
}

fn usize_value(value: usize) -> TypeScriptValue {
    TypeScriptValue::number(i64::try_from(value).expect("TypeScript constant fits in i64"))
}

#[cfg(test)]
mod tests {
    use super::{
        RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
        constants_module,
    };

    #[test]
    fn preserves_username_constant_exports() {
        let rendered = constants_module().render_source();
        assert!(rendered.contains("RADROOTS_USERNAME_MIN_LEN"));
        assert!(rendered.contains(&RADROOTS_USERNAME_MIN_LEN.to_string()));
        assert!(rendered.contains("RADROOTS_USERNAME_MAX_LEN"));
        assert!(rendered.contains(&RADROOTS_USERNAME_MAX_LEN.to_string()));
        assert!(rendered.contains("RADROOTS_USERNAME_REGEX"));
        assert!(rendered.contains(&format!("{RADROOTS_USERNAME_REGEX:?}")));
    }
}
