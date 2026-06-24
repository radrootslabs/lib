pub use radroots_identity as upstream;

use radroots_identity::{
    RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
};

pub fn constants_module() -> String {
    format!(
        "export const RADROOTS_USERNAME_MIN_LEN = {RADROOTS_USERNAME_MIN_LEN};\nexport const RADROOTS_USERNAME_MAX_LEN = {RADROOTS_USERNAME_MAX_LEN};\nexport const RADROOTS_USERNAME_REGEX = {RADROOTS_USERNAME_REGEX:?};"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        RADROOTS_USERNAME_MAX_LEN, RADROOTS_USERNAME_MIN_LEN, RADROOTS_USERNAME_REGEX,
        constants_module,
    };

    #[test]
    fn preserves_username_constant_exports() {
        let rendered = constants_module();
        assert!(rendered.contains("RADROOTS_USERNAME_MIN_LEN"));
        assert!(rendered.contains(&RADROOTS_USERNAME_MIN_LEN.to_string()));
        assert!(rendered.contains("RADROOTS_USERNAME_MAX_LEN"));
        assert!(rendered.contains(&RADROOTS_USERNAME_MAX_LEN.to_string()));
        assert!(rendered.contains("RADROOTS_USERNAME_REGEX"));
        assert!(rendered.contains(&format!("{RADROOTS_USERNAME_REGEX:?}")));
    }
}
