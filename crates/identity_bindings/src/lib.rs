pub use radroots_identity as upstream;

use radroots_sdk_binding_model::{self as ts, TsValue};

pub fn constants_module() -> ts::TsModule {
    ts::module(vec![
        ts::const_number("RADROOTS_USERNAME_MIN_LEN", 3),
        ts::const_number("RADROOTS_USERNAME_MAX_LEN", 30),
        ts::const_decl(
            "RADROOTS_USERNAME_REGEX",
            None,
            TsValue::String(r#"^(?!.*\.\.)(?!\.)(?!.*\.$)[a-z0-9._-]{3,30}$"#.to_owned()),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::constants_module;

    #[test]
    fn preserves_username_constant_exports() {
        let rendered = constants_module().render();
        assert!(rendered.contains("RADROOTS_USERNAME_MIN_LEN"));
        assert!(rendered.contains("RADROOTS_USERNAME_MAX_LEN"));
        assert!(rendered.contains("RADROOTS_USERNAME_REGEX"));
    }
}
